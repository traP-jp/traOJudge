"""Trusted control container entrypoint; guests never mount this filesystem."""
import os
import subprocess
import time

probe='/opt/tools/bin/probe'
subprocess.run([probe,'init'],check=True,timeout=60)
children=[subprocess.Popen([probe,'controller']),subprocess.Popen([probe,'worker'])]
while all(p.poll() is None for p in children):
    time.sleep(.2)
for child in children:
    child.terminate()
raise SystemExit(1)
