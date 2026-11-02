"""Prepare CDK assets before the one-hour disposable AWS stack is created.

Requires cargo build -p traojudge-judge-probe and a Docker daemon. On vfs,
flatten the pinned Nix base first to avoid materializing 100 ancestor layers.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import urllib.request

ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).resolve().parent
DOCKER=['docker','--host=unix:///var/run/docker.sock']
ENV={k:v for k,v in os.environ.items() if k not in ['DOCKER_HOST','DOCKER_CONTEXT','DOCKER_TLS','DOCKER_TLS_VERIFY','DOCKER_CERT_PATH']}
ENV['BUILDX_CONFIG']=str(ROOT/'work/buildx')
BASE='nixos/nix@sha256:e623d73af9cac82d1b50784c83e0cf2a4b83bfd2cfe8d5b67809a2fc94e043ac'

def docker(*args):
    subprocess.run(DOCKER+list(args),check=True,env=ENV)


def prepare(flatten):
    host=ROOT/'work/pg-pool-host';host.mkdir(parents=True,exist_ok=True)
    control=ROOT/'work/pg-pool-control';(control/'lib').mkdir(parents=True,exist_ok=True)
    binary=ROOT/'target/debug/traojudge-judge-probe'
    if flatten:
        docker('pull',BASE)
        docker('create','--name','codex-judge-seed',BASE,'sleep','infinity')
        with tempfile.TemporaryDirectory(prefix='judge-base-',dir='/tmp') as tmp:
            archive=Path(tmp)/'base.tar'
            docker('export','--output',str(archive),'codex-judge-seed')
            docker('rm','codex-judge-seed')
            # No force: do not remove a base referenced by anyone else.
            subprocess.run(DOCKER+['image','rm',BASE],env=ENV,check=False)
            docker('import','--change','ENV PATH=/nix/var/nix/profiles/default/bin:/root/.nix-profile/bin:/bin',str(archive),'codex-judge-nix-base:probe')
    with tempfile.TemporaryDirectory(prefix='judge-build-',dir=str(ROOT/'work')) as tmp:
        context=Path(tmp)
        shutil.copytree(ROOT/'judge-command',context/'baked-command')
        shutil.copy(binary,context/'judge-probe')
        shutil.copy(HERE/'supervisor.py',context/'supervisor.py')
        args=['build','--build-arg','HTTPS_PROXY','--build-arg','HTTP_PROXY','--build-arg','ALL_PROXY']
        if flatten:args+=['--build-arg','BASE=codex-judge-nix-base:probe']
        if os.environ.get('CODEX_PROXY_CERT'):args+=['--secret','id=proxy_ca,src='+os.environ['CODEX_PROXY_CERT']]
        docker(*args,'-f',str(HERE/'Dockerfile'),'-t','codex-judge-pool:proof',str(context))
    docker('build','-f',str(HERE/'empty.Dockerfile'),'-t','judge-empty:probe',str(HERE))
    with (host/'images.tar.gz').open('wb') as stream:
        source=subprocess.Popen(DOCKER+['save','codex-judge-pool:proof','judge-empty:probe'],env=ENV,stdout=subprocess.PIPE)
        subprocess.run(['gzip','-1'],stdin=source.stdout,stdout=stream,check=True)
        source.stdout.close()
        if source.wait():raise RuntimeError('docker save failed')
    for destination in [host/'judge-probe',control/'judge-probe']:
        shutil.copy(binary,destination);subprocess.run(['strip','--strip-debug',str(destination)],check=True);destination.chmod(0o755)
    shutil.copy(HERE/'watchdog.py',host/'watchdog.py')
    shutil.copy(HERE/'control.py',control/'control.py')
    dependency_env=os.environ.copy();dependency_env['UV_CACHE_DIR']=str(ROOT/'work/uv-cache')
    subprocess.run(['uv','pip','install','--target',str(control),'-r',str(HERE/'collector-requirements.txt')],env=dependency_env,check=True)
    with urllib.request.urlopen('https://truststore.pki.rds.amazonaws.com/global/global-bundle.pem',timeout=30) as response:
        certificates=response.read()
    (host/'rds-ca.pem').write_bytes(certificates);(control/'rds-ca.pem').write_bytes(certificates)
    # Only the trusted controller needs these libraries. Judge toolchains and
    # their runtime libraries come exclusively from the locked Nix closure.
    libraries=subprocess.check_output(['ldd',str(binary)],text=True)
    import re
    for path in re.findall(r'(/[^\s]+)',libraries):
        source=Path(path)
        if source.is_file():shutil.copy(source.resolve(),control/'lib'/source.name)
    for path in control.rglob('*'):
        path.chmod(0o755 if path.is_dir() or path.name in ['judge-probe','ld-linux-x86-64.so.2'] else 0o644)
    base='https://storage.googleapis.com/gvisor/releases/release/20261026.0/x86_64/gvisor.tar.bz2'
    for suffix,name in [('', 'gvisor.tar.bz2'),('.sha512','gvisor.sha512')]:
        with urllib.request.urlopen(base+suffix,timeout=120) as response:(host/name).write_bytes(response.read())
    expected=(host/'gvisor.sha512').read_text().split()[0]
    assert hashlib.sha512((host/'gvisor.tar.bz2').read_bytes()).hexdigest()==expected
    (ROOT/'work/pg-pool-asset-manifest.json').write_text(json.dumps({p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in host.iterdir()},indent=2))


if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--flatten-base',action='store_true')
    prepare(parser.parse_args().flatten_base)
