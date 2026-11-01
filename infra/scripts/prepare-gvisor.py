#!/usr/bin/env python3
"""Download the pinned official gVisor distribution for the private CDK asset."""
import hashlib
import json
from pathlib import Path
import urllib.request

folder = Path(__file__).resolve().parents[1] / "pool"
version = json.loads((folder / "gvisor-version.json").read_text())
target = folder / "gvisor.tar.bz2"
if not target.exists() or hashlib.sha512(target.read_bytes()).hexdigest() != version["sha512"]:
    data = urllib.request.urlopen(version["source"], timeout=60).read()
    if hashlib.sha512(data).hexdigest() != version["sha512"]:
        raise ValueError("gVisor distribution checksum mismatch")
    target.write_bytes(data)
print("gVisor distribution checksum verified")
