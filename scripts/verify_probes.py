"""Rebuild both probes in isolation and reject stale checked-in objects."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
COMPILER = os.environ.get("CLANG", "clang")
PIN = "22.1.8"
version = subprocess.check_output([COMPILER, "--version"], text=True).splitlines()[0]
if f"clang version {PIN}" not in version:
    raise SystemExit(f"Probe verification requires Clang {PIN}; got {version}")
objects = ["kernwatch.bpf.o", "kernwatch-aarch64.bpf.o"]
with tempfile.TemporaryDirectory(prefix="kernwatch-probes-") as directory:
    tmp = Path(directory)
    for name in ["build.sh", "kernwatch.bpf.c"]:
        shutil.copy2(ROOT / "probes" / name, tmp / name)
    subprocess.run(["sh", str(tmp / "build.sh")], check=True)
    for name in objects:
        if (tmp / name).read_bytes() != (ROOT / "probes" / name).read_bytes():
            raise SystemExit(f"Stale probe: {name}; run probes/build.sh with the pinned toolchain")
manifest = {
    "compiler": version,
    "source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
    "source_sha256": hashlib.sha256((ROOT / "probes/kernwatch.bpf.c").read_bytes()).hexdigest(),
    "objects": {name: hashlib.sha256((ROOT / "probes" / name).read_bytes()).hexdigest() for name in objects},
}
out = ROOT / "dist/probes"
out.mkdir(parents=True, exist_ok=True)
for name in objects:
    shutil.copy2(ROOT / "probes" / name, out / name)
(out / "provenance.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(json.dumps(manifest, indent=2))
