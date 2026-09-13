"""Package one tested native release target using netwatch asset naming."""
import hashlib
from pathlib import Path
import shutil
import sys
import tarfile

target, asset = sys.argv[1:]
binary = Path('target') / target / 'release' / 'kernwatch'
output = Path('dist')
output.mkdir(exist_ok=True)
stage = output / 'package'
stage.mkdir(exist_ok=True)
shutil.copy2(binary, stage / asset)
archive = output / (asset + '.tar.gz')
files = [(stage / asset, asset), (Path('LICENSE'), 'LICENSE'), (Path('README.md'), 'README.md')]
files.extend((Path('licenses') / ('cpp_demangle-' + name), 'licenses/cpp_demangle-' + name) for name in ['LICENSE-MIT', 'LICENSE-APACHE'])
provenance = Path('probes/provenance.json')
if provenance.exists():
    files.append((provenance, 'bpf-provenance.json'))
with tarfile.open(archive, 'w:gz') as handle:
    for source, name in files:
        handle.add(source, arcname=name)
checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
archive.with_name(archive.name + '.sha256').write_text(f'{checksum}  {archive.name}\n')
shutil.rmtree(stage)
print(archive)
