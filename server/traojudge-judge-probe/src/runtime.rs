//! Rust owns trusted Nix roots and per-attempt sandbox lifecycle. The submitted
//! source never becomes a derivation and receives no host credentials or network.
use crate::queue::{Lease, Queue};
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::{io::AsyncReadExt, process::Command};

#[derive(Deserialize)]
struct Manifest {
    schema: u32,
    language: String,
    source: String,
    compile: Vec<String>,
    run: Vec<String>,
}
pub struct Runtime {
    pub flake: PathBuf,
    pub work: PathBuf,
    pub cpu: String,
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
async fn output(mut cmd: Command) -> Result<String> {
    cmd.kill_on_drop(true);
    let out = tokio::time::timeout(Duration::from_secs(10), cmd.output())
        .await
        .context("trusted command timeout")??;
    ensure!(
        out.status.success(),
        "trusted command failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}
impl Runtime {
    pub async fn execute(&self, q: &Queue, lease: &Lease) -> Result<Value> {
        let phase = lease.payload["phase"].as_str().context("phase missing")?;
        if phase == "check" {
            let id = serde_json::from_value(lease.payload["run"].clone())?;
            let run = q.result(id).await?;
            let verdict = if run["verdict"] != "OK" {
                run["verdict"].clone()
            } else if run["stdout_b64"]
                == json!(
                    STANDARD.encode(
                        lease.payload["expected"]
                            .as_str()
                            .context("expected missing")?
                    )
                )
            {
                json!("AC")
            } else {
                json!("WA")
            };
            return Ok(json!({"verdict":verdict}));
        }
        let language = lease.payload["language"]
            .as_str()
            .context("language missing")?;
        ensure!(
            ["cpp", "python", "rust"].contains(&language),
            "unknown language"
        );
        let name = format!("judge-probe-{}", lease.token);
        let dir = self.work.join(lease.token.to_string());
        tokio::fs::create_dir_all(dir.join("inputs")).await?;
        let expiry = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
            + 30;
        tokio::fs::write(dir.join(".deadline.tmp"), expiry.to_string()).await?;
        tokio::fs::rename(dir.join(".deadline.tmp"), dir.join(".deadline")).await?;
        tokio::fs::set_permissions(dir.join("inputs"), std::fs::Permissions::from_mode(0o755))
            .await?;
        let mut result = tokio::time::timeout(
            Duration::from_secs(20),
            self.execute_inner(q, lease, phase, language, &name, &dir),
        )
        .await
        .context("attempt setup/runtime timeout")
        .and_then(|r| r);
        if result.is_err() && phase == "run" {
            let mut inspect = Command::new("docker");
            inspect.args(["inspect", &name]);
            if let Ok(text) = output(inspect).await {
                if let Ok(info) = serde_json::from_str::<Value>(&text) {
                    if info[0]["State"]["OOMKilled"] == true {
                        result = Ok(json!({"verdict":"MLE","evidence":"host_cgroup_oom"}));
                    }
                }
            }
        }
        // gVisor's control path must be live for OCI delete. A paused sandbox
        // otherwise leaves docker rm waiting for its frozen runtime.
        let mut unpause = Command::new("docker");
        unpause.args(["unpause", &name]).kill_on_drop(true);
        let _ = tokio::time::timeout(Duration::from_secs(3), unpause.output()).await;
        // Kill first: a process must never outlive its input files / GC roots.
        let mut remove = Command::new("docker");
        remove.args(["rm", "-f", &name]);
        remove.kill_on_drop(true);
        let removed = tokio::time::timeout(Duration::from_secs(10), remove.output())
            .await
            .context("FATAL_CLEANUP: docker rm timeout")?
            .context("FATAL_CLEANUP: docker rm failed")?;
        // Compilation skipped for Python or failed before docker create.
        ensure!(
            removed.status.success()
                || String::from_utf8_lossy(&removed.stderr).contains("No such container"),
            "FATAL_CLEANUP: sandbox removal failed"
        );
        tokio::fs::remove_dir_all(&dir)
            .await
            .context("FATAL_CLEANUP: job root removal failed")?;
        result
    }
    async fn execute_inner(
        &self,
        q: &Queue,
        lease: &Lease,
        phase: &str,
        language: &str,
        name: &str,
        dir: &Path,
    ) -> Result<Value> {
        let root = dir.join("toolchain");
        let mut nix = Command::new("nix");
        nix.args([
            "--extra-experimental-features",
            "nix-command flakes",
            "build",
            "--offline",
            "--no-update-lock-file",
            "--out-link",
        ])
        .arg(&root)
        .arg(format!("path:{}#runtime-{language}", self.flake.display()));
        output(nix).await?;
        let env = tokio::fs::read_link(&root).await?;
        ensure!(env.starts_with("/nix/store"), "unexpected Nix root");
        let manifest: Manifest = serde_json::from_slice(
            &tokio::fs::read(env.join("share/traojudge/language.json")).await?,
        )?;
        ensure!(
            manifest.schema == 1 && manifest.language == language,
            "manifest mismatch"
        );
        let mut nix = Command::new("nix-store");
        nix.arg("--query").arg("--requisites").arg(&env);
        let closure = output(nix).await?;
        let inputs = dir.join("inputs");
        let mut command;
        if phase == "compile" {
            let source = lease.payload["source"].as_str().context("source missing")?;
            ensure!(source.len() <= 65536, "source cap");
            if manifest.compile.is_empty() {
                return Ok(
                    json!({"verdict":"OK","artifact":STANDARD.encode(source),"runtime":env}),
                );
            }
            write_input(&inputs.join(&manifest.source), source.as_bytes()).await?;
            let argv = manifest
                .compile
                .iter()
                .map(|s| quote(s))
                .collect::<Vec<_>>()
                .join(" ");
            command = format!(
                "cp /inputs/{} .; {argv} 1>&2 || exit 42; base64 -w0 program",
                quote(&manifest.source)
            );
        } else if phase == "run" {
            let compiled = q
                .result(serde_json::from_value(lease.payload["compile"].clone())?)
                .await?;
            if compiled["verdict"] != "OK" {
                return Ok(json!({"verdict":"CE"}));
            }
            ensure!(
                compiled["runtime"] == json!(env),
                "toolchain changed between nodes"
            );
            let artifact =
                STANDARD.decode(compiled["artifact"].as_str().context("artifact missing")?)?;
            ensure!(artifact.len() <= 8 * 1024 * 1024, "artifact cap");
            let file = if language == "python" {
                &manifest.source
            } else {
                "program"
            };
            write_input(&inputs.join(file), &artifact).await?;
            write_input(
                &inputs.join("input"),
                lease.payload["input"]
                    .as_str()
                    .context("input missing")?
                    .as_bytes(),
            )
            .await?;
            let argv = manifest
                .run
                .iter()
                .map(|s| quote(s))
                .collect::<Vec<_>>()
                .join(" ");
            command = format!(
                "cp /inputs/{} .; chmod u+x {}; exec {argv} < /inputs/input",
                quote(file),
                quote(file)
            );
        } else {
            bail!("unknown phase");
        }
        // Send compile failure diagnostics separately from the bounded artifact.
        command = format!("set -eu; ulimit -u 64; ulimit -n 64; {command}");
        let mut create = Command::new("docker");
        create
            .args([
                "create",
                "--name",
                name,
                "--label",
                "traojudge.probe=true",
                "--runtime=runsc",
                "--network=none",
                "--read-only",
                "--cap-drop=ALL",
                "--security-opt=no-new-privileges",
                "--user=0:0",
                "--memory=512m",
                "--memory-swap=512m",
                "--pids-limit=256",
                "--ulimit=nofile=1024:1024",
                "--ulimit=fsize=67108864:67108864",
                "--log-driver=none",
                "--workdir=/job",
                "--tmpfs=/job:rw,exec,nosuid,nodev,size=64m,uid=65534,gid=65534,mode=700",
                "--tmpfs=/tmp:rw,noexec,nosuid,nodev,size=32m,mode=1777",
                "--env=HOME=/tmp",
            ])
            .arg("--label")
            .arg(format!(
                "traojudge.expires={}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs()
                    + 8
            ))
            .arg(format!("--cpuset-cpus={}", self.cpu))
            .arg(format!("--env=PATH={}/bin", env.display()))
            .arg("--mount")
            .arg(format!(
                "type=bind,src={},dst=/inputs,readonly",
                inputs.display()
            ));
        for path in closure.lines() {
            ensure!(
                Path::new(path).starts_with("/nix/store"),
                "unexpected closure path"
            );
            create
                .arg("--mount")
                .arg(format!("type=bind,src={path},dst={path},readonly"));
        }
        // A root-owned sleeping keeper cannot be signalled by the untrusted
        // UID. Keeping the sandbox alive permits final host-cgroup accounting.
        create
            .arg("--entrypoint")
            .arg(env.join("bin/sleep"))
            .args(["judge-empty:probe", "infinity"]);
        output(create).await?;
        let mut boot = Command::new("docker");
        boot.args(["start", name]);
        output(boot).await?;
        let mut inspect = Command::new("docker");
        inspect.args(["inspect", name]);
        let info: Value = serde_json::from_str(&output(inspect).await?)?;
        let pid = info[0]["State"]["Pid"]
            .as_u64()
            .context("sandbox PID missing")?;
        let membership = tokio::fs::read_to_string(format!("/host-proc/{pid}/cgroup")).await?;
        let relative = membership
            .lines()
            .find_map(|line| line.strip_prefix("0::"))
            .context("cgroup v2 required")?;
        let container_id = info[0]["Id"].as_str().context("container ID missing")?;
        ensure!(
            !Path::new(relative)
                .components()
                .any(|part| part == std::path::Component::ParentDir),
            "cgroup namespace is not the host namespace"
        );
        ensure!(
            relative.contains(container_id),
            "PID cgroup does not identify this sandbox"
        );
        let counter = Path::new("/host-cgroup")
            .join(relative.trim_start_matches('/'))
            .join("cpu.stat");
        let cpu_start = cpu_usec(&counter).await?;
        let mut start = Command::new("docker");
        start
            .args(["exec", "--user=65534:65534", name])
            .arg(env.join("bin/bash"))
            .args(["-c", &command])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = start.spawn()?;
        let stdout = child.stdout.take().context("stdout")?;
        let stderr = child.stderr.take().context("stderr")?;
        let cap = if phase == "compile" {
            11_184_812
        } else {
            65536
        };
        let begin = Instant::now();
        let run = async {
            // Read both streams concurrently; neither can block the other.
            let (out, err) =
                match tokio::try_join!(read_capped(stdout, cap), read_capped(stderr, 65536)) {
                    Ok(streams) => streams,
                    Err(error) if error.kind() == std::io::ErrorKind::FileTooLarge => {
                        return Ok((None, Vec::new(), Vec::new()));
                    }
                    Err(error) => return Err(error.into()),
                };
            let status = child.wait().await?;
            Ok::<_, anyhow::Error>((Some(status), out, err))
        };
        let monitor = async {
            loop {
                tokio::time::sleep(Duration::from_millis(2)).await;
                let used = cpu_usec(&counter).await?.saturating_sub(cpu_start);
                if used > 3_000_000 {
                    return Ok::<u64, anyhow::Error>(used);
                }
            }
        };
        let (status, out, err) = tokio::select! {
            result=tokio::time::timeout(Duration::from_secs(5),run)=> result.context("wall watchdog: infrastructure retry, not CPU TLE")??,
            used=monitor=>return Ok(json!({"verdict":"CPU_LIMIT","host_cpu_usec":used?})),
        };
        let mut pause = Command::new("docker");
        pause.args(["pause", name]);
        output(pause).await?;
        let used = cpu_usec(&counter).await?.saturating_sub(cpu_start);
        let memory_events =
            tokio::fs::read_to_string(counter.with_file_name("memory.events")).await?;
        let oom = memory_events
            .lines()
            .filter_map(|line| {
                line.strip_prefix("oom ")
                    .or_else(|| line.strip_prefix("oom_kill "))
            })
            .any(|value| value.parse::<u64>().unwrap_or(0) > 0);
        if oom {
            return Ok(json!({"verdict":"MLE","evidence":"host_cgroup_oom"}));
        }
        if used > 3_000_000 {
            return Ok(json!({"verdict":"CPU_LIMIT","host_cpu_usec":used}));
        }
        let Some(status) = status else {
            return Ok(
                json!({"verdict":if phase=="compile" {"CE"}else{"OLE"},"diagnostic":"output limit exceeded"}),
            );
        };
        // Whole sandbox host CPU includes Sentry/exec overhead. Report it
        // separately from wall time and never substitute the guest tick clock.
        if phase == "compile" {
            if status.code() == Some(42) {
                return Ok(json!({"verdict":"CE","diagnostic":String::from_utf8_lossy(&err)}));
            }
            ensure!(
                status.success(),
                "sandbox infrastructure failed: {}",
                String::from_utf8_lossy(&err)
            );
            let binary = STANDARD.decode(&out).context("invalid compile artifact")?;
            if binary.len() > 8 * 1024 * 1024 {
                return Ok(json!({"verdict":"CE","diagnostic":"artifact limit exceeded"}));
            }
            Ok(json!({"verdict":"OK","artifact":String::from_utf8(out)?,"runtime":env}))
        } else {
            ensure!(status.code() != Some(125), "Docker/runtime failed");
            Ok(
                json!({"verdict":if status.success(){"OK"}else{"RE"},"stdout_b64":STANDARD.encode(&out),"stdout":String::from_utf8_lossy(&out),
                "stderr":String::from_utf8_lossy(&err),"wall_seconds":begin.elapsed().as_secs_f64(),"host_cpu_usec":used}),
            )
        }
    }
}
async fn write_input(path: &Path, bytes: &[u8]) -> Result<()> {
    tokio::fs::write(path, bytes).await?;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644)).await?;
    Ok(())
}

async fn cpu_usec(path: &Path) -> Result<u64> {
    let stat = tokio::fs::read_to_string(path)
        .await
        .context("host CPU accounting unavailable")?;
    let value = stat
        .lines()
        .find_map(|line| line.strip_prefix("usage_usec "))
        .context("cpu.stat usage_usec missing")?;
    Ok(value.parse()?)
}
async fn read_capped(
    reader: impl tokio::io::AsyncRead + Unpin,
    cap: u64,
) -> std::io::Result<Vec<u8>> {
    let mut data = Vec::new();
    reader.take(cap + 1).read_to_end(&mut data).await?;
    if data.len() > cap as usize {
        return Err(std::io::Error::new(
            std::io::ErrorKind::FileTooLarge,
            "output cap",
        ));
    }
    Ok(data)
}
