"""Prepare private assets; dependency versions are recorded, never credential values."""
from pathlib import Path
import shutil
import hashlib
import json
import urllib.request

import boto3
import botocore
import dateutil
import jmespath
import s3transfer
import six
import urllib3

root = Path(__file__).resolve().parents[3]
target = root / 'work/fairness-provider'
target.mkdir(parents=True, exist_ok=True)
for module in [boto3, botocore, dateutil, jmespath, s3transfer, urllib3]:
    source = Path(module.__file__).parent
    shutil.copytree(source, target / source.name, dirs_exist_ok=True, ignore=shutil.ignore_patterns('__pycache__'))
shutil.copy(six.__file__, target / 'six.py')
for name in ['provider.py', 'queued.py']:
    shutil.copy(Path(__file__).with_name(name), target / name)
print(f'Prepared SDK boto3={boto3.__version__}, botocore={botocore.__version__}')

folder = Path(__file__).parent
host = root / 'work/fairness-host'
(host / 'image').mkdir(parents=True, exist_ok=True)
for name in ['fixture.c', 'sandbox.c', 'server.py']:
    shutil.copy(folder / name, host / 'image' / name)
dockerfile = (folder / 'Dockerfile').read_text().replace(
    'RUN --mount=type=secret,id=proxy_ca,required=false,mode=0444 \\\n', 'RUN \\\n')
(host / 'image/Dockerfile').write_text(dockerfile)
shutil.copy(folder / 'agent.py', host / 'agent.py')
pin = json.loads((folder / 'gvisor-version.json').read_text())
tar = host / 'gvisor.tar.bz2'
if not tar.exists():
    tar.write_bytes(urllib.request.urlopen(pin['source'], timeout=60).read())
if hashlib.sha512(tar.read_bytes()).hexdigest() != pin['sha512']:
    raise ValueError('gVisor checksum mismatch')
(host / 'gvisor.sha512').write_text(pin['sha512'] + '  gvisor.tar.bz2\n')
print('Prepared pinned gVisor host asset')
