"""Allowlisted measurement fixtures; deliberately not a product submission API."""
import ctypes
import hashlib
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import tempfile
import time

ctypes.CDLL(None).prctl(36, 1, 0, 0, 0)  # PR_SET_CHILD_SUBREAPER
ALLOWED = set(os.sched_getaffinity(0))
CORE = int(os.environ.get('BENCH_CPU', min(ALLOWED)))
if CORE not in ALLOWED:
    raise RuntimeError('Requested CPU is unavailable')


def info():
    cpu = Path('/proc/cpuinfo').read_text()
    models = sorted(set(s.strip() for s in cpu.splitlines() if s.startswith(
        ('model name', 'CPU implementer', 'CPU part', 'CPU variant', 'CPU revision'))))
    return {'cpu_model': models, 'architecture': os.uname().machine,
            'cpus_visible': os.cpu_count(), 'affinity': sorted(ALLOWED), 'bench_cpu': CORE,
            'compiler': subprocess.check_output(['gcc', '--version'], text=True).splitlines()[0],
            'python': os.sys.version.split()[0],
            'binary_sha256': hashlib.sha256(Path('/app/fixture').read_bytes()).hexdigest()}


def group_cpu(group):
    ticks = 0
    for proc in Path('/proc').iterdir():
        if not proc.name.isdigit():
            continue
        try:
            fields = (proc / 'stat').read_text().rsplit(')', 1)[1].split()
            if int(fields[2]) == group:
                ticks += sum(int(fields[k]) for k in [11, 12, 13, 14])
        except (OSError, ValueError, IndexError):
            continue
    return ticks / os.sysconf('SC_CLK_TCK')


def execute(command, directory, wall, budget=None):
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.monotonic()
    with open(directory / 'stdout', 'wb') as out, open(directory / 'stderr', 'wb') as err:
        proc = subprocess.Popen(['/app/sandbox', '3', '256', *command], cwd=directory,
            stdin=subprocess.DEVNULL, stdout=out, stderr=err, start_new_session=True,
            env={'PATH': '/usr/bin:/bin', 'HOME': str(directory), 'LANG': 'C.UTF-8', 'TMPDIR': str(directory)},
            preexec_fn=lambda: os.sched_setaffinity(0, {CORE}))
        reason = None
        while True:
            pid, status, _ = os.wait4(proc.pid, os.WNOHANG)
            if pid:
                proc.returncode = os.waitstatus_to_exitcode(status)
                break
            elapsed = time.monotonic() - start
            if elapsed > wall or (budget is not None and group_cpu(proc.pid) >= budget):
                reason = 'wall_watchdog' if elapsed > wall else 'cpu_budget'
                os.killpg(proc.pid, signal.SIGKILL)
                _, status, _ = os.wait4(proc.pid, 0)
                proc.returncode = os.waitstatus_to_exitcode(status)
                break
            time.sleep(0.003)
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        # Reap adopted orphan descendants; their rusage contributes to the group total.
        while True:
            try:
                os.wait4(-proc.pid, 0)
            except ChildProcessError:
                break
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    cpu = after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime
    verdict = 'WATCHDOG' if reason == 'wall_watchdog' else 'TLE' if (
        reason == 'cpu_budget' or (budget is not None and cpu > budget)) else 'OK' if proc.returncode == 0 else 'RE'
    return {'cpu_seconds': cpu, 'wall_seconds': time.monotonic() - start,
            'exit': proc.returncode, 'reason': reason, 'verdict': verdict,
            'output': (directory / 'stdout').read_text(errors='replace')[:256],
            'stderr_excerpt': (directory / 'stderr').read_text(errors='replace')[:512]}


def run(body):
    mode = body.get('mode', 0)
    count = body.get('iterations', 100_000_000)
    samples = body.get('samples', 1)
    if type(mode) is not int or mode not in range(4) or type(count) is not int or not 1 <= count <= 300_000_000:
        raise ValueError('Invalid fixture')
    if type(samples) is not int or not 1 <= samples <= 40:
        raise ValueError('Invalid sample count')
    budget = body.get('cpu_budget')
    if budget is not None and (type(budget) not in [int, float] or not 0.01 <= budget <= 2):
        raise ValueError('Invalid CPU budget')
    start = time.monotonic()
    values = []
    with tempfile.TemporaryDirectory(prefix='fairness-') as temp:
        folder = Path(temp); folder.chmod(0o777)
        binary = '/app/fixture'
        compile_result = None
        if body.get('compile'):
            compile_result = execute(['gcc', '-O2', '-std=c17', '-fno-tree-vectorize', '-march=armv8-a',
                '/app/fixture.c', '-o', str(folder / 'fixture')], folder, 10)
            if compile_result['verdict'] != 'OK':
                raise RuntimeError('Fixture compilation failed: ' + json.dumps(compile_result))
            binary = str(folder / 'fixture')
        for _ in range(samples):
            values.append(execute([binary, str(mode), str(count)], folder, body.get('wall_seconds', 5), budget))
    return {'samples': values, 'compile': compile_result, 'server_seconds': time.monotonic() - start,
            'environment': info()}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        self.reply(200, info())

    def do_POST(self):
        try:
            # Hooks are lifecycle operations, never benchmark samples.
            if self.path.startswith('/aws/'):
                value = {'ready': True}
            else:
                body = json.loads(self.rfile.read(int(self.headers.get('Content-Length', 0))))
                value = run(body)
            self.reply(200, value)
        except Exception as exc:
            self.reply(500, {'infrastructure_error': type(exc).__name__, 'detail': str(exc)[:1200]})

    def reply(self, status, value):
        data = json.dumps(value).encode()
        self.send_response(status); self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data))); self.end_headers(); self.wfile.write(data)


if __name__ == '__main__':
    if '--job' in os.sys.argv:
        print(json.dumps(run(json.load(os.sys.stdin))))
    else:
        HTTPServer(('0.0.0.0', 8080), Handler).serve_forever()
