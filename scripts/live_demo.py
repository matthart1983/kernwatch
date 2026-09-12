"""Record kernwatch monitoring the real host, as a GIF and MP4.

Unlike scripts/build_demo.py, which renders the synthetic guided tour scene by
scene, this drives the release binary in a real terminal against live
procfs/sysfs data. A bounded workload runs alongside the recording so the
graphs show movement; it is removed when the recording finishes.

Requires vhs (https://github.com/charmbracelet/vhs) and ffmpeg. The recording
shows this host's process names, cgroups, devices and kernel log, so review the
result before publishing it.
"""
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TAPE = ROOT / 'scripts/live_demo.tape'
# Covers the tape's off-camera warm-up plus the recording itself, so the load
# does not stop part way through.
LOAD_SECONDS = 170

for tool in ('vhs', 'ffmpeg'):
    if not shutil.which(tool):
        sys.exit(f'{tool} is required to record the live demo')

subprocess.run(['cargo', 'build', '--release', '--locked'], cwd=ROOT, check=True)

scratch = Path(tempfile.mkdtemp(prefix='kernwatch-live-load-', dir=Path.home() / '.cache'))
load = subprocess.Popen(
    [sys.executable, str(ROOT / 'scripts/live_load.py'), str(LOAD_SECONDS), str(scratch)],
    cwd=ROOT,
)
try:
    subprocess.run(['vhs', str(TAPE)], cwd=ROOT, check=True)
finally:
    load.terminate()
    try:
        load.wait(timeout=15)
    except subprocess.TimeoutExpired:
        load.kill()
    shutil.rmtree(scratch, ignore_errors=True)

for name in ('kernwatch-live.gif', 'kernwatch-live.mp4'):
    path = ROOT / 'screenshots/demo' / name
    print(f'{path} ({path.stat().st_size / 1e6:.1f} MB)')
