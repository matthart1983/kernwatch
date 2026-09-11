"""Package one tested native release target using netwatch asset naming."""
import hashlib
from pathlib import Path
import shutil
import sys
import tarfile
import zipfile

target, asset = sys.argv[1:]
windows = 'windows' in target
binary = Path('target') / target / 'release' / ('kernwatch.exe' if windows else 'kernwatch')
output = Path('dist')
output.mkdir(exist_ok=True)
stage = output / 'package'
stage.mkdir(exist_ok=True)
shutil.copy2(binary, stage / asset)
archive = output / (asset + ('.zip' if windows else '.tar.gz'))
files = [(stage / asset, asset), (Path('LICENSE'), 'LICENSE'), (Path('README.md'), 'README.md')]
if windows:
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as handle:
        for source, name in files:
            handle.write(source, name)
else:
    with tarfile.open(archive, 'w:gz') as handle:
        for source, name in files:
            handle.add(source, arcname=name)
checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
archive.with_name(archive.name + '.sha256').write_text(f'{checksum}  {archive.name}\n')
shutil.rmtree(stage)
print(archive)
