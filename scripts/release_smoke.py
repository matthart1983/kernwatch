"""Exercise native release startup, rendering and recording/replay on each target."""
from pathlib import Path
import json
import os
import shutil
import subprocess
import sys
import tempfile

target = sys.argv[1]
windows = 'windows' in target
exe = 'kernwatch.exe' if windows else 'kernwatch'
binary = (Path('target') / target / 'release' / exe).resolve()
Path('target/release').mkdir(exist_ok=True)
shutil.copy2(binary, Path('target/release') / exe)
with tempfile.TemporaryDirectory(prefix='kernwatch-release-') as work:
    env = {**os.environ, 'XDG_STATE_HOME': str(Path(work) / 'state')}
    def run(*args):
        return subprocess.run([str(binary), *args], cwd=work, env=env, check=True, capture_output=True, text=True, encoding="utf-8").stdout
    assert 'kernwatch' in run('--help')
    assert json.loads(run('--demo', '--snapshot'))['demo']
    assert 'kernwatch' in run('--demo-tour', '--render', '160x52')
    recording = Path(work) / 'fixture.kwr'
    subprocess.run(['cargo', 'run', '--locked', '--target', target, '--example', 'record_fixture', '--', str(recording)], check=True)
    replay = json.loads(run('--replay', str(recording), '--at', '1000', '--snapshot'))
    assert replay['demo']
    if 'linux' not in target:
        result = subprocess.run([str(binary), '--snapshot'], cwd=work, env=env, capture_output=True, text=True, encoding="utf-8")
        assert result.returncode != 0 and 'Live monitoring requires Linux' in result.stderr
    if 'musl' in target:
        assert 'INTERP' not in subprocess.check_output(['readelf', '-l', str(binary)], text=True, encoding="utf-8")
        assert 'NEEDED' not in subprocess.check_output(['readelf', '-d', str(binary)], text=True, encoding="utf-8")
print(f'PASS native release smoke: {target}')
