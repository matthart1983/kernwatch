"""Package a locally validated build. Does not install services or modify host settings."""
from pathlib import Path
import hashlib, json, re, subprocess, tarfile, tempfile, shutil, datetime, os
root=Path(__file__).resolve().parents[1]
logs=root/'tests'
tests=(logs/'cargo-test.txt').read_text()
assert 'FAILED' not in tests and 'Doc-tests' in tests, 'Complete passing cargo test evidence required'
test_count=sum(map(int,re.findall(r'test result: ok\. (\d+) passed',tests)))
assert test_count >= 61, 'Gap regression tests must be included'
for name in ['clippy.txt','build.txt']:
    content=(logs/name).read_text();assert 'Finished' in content and 'error:' not in content,name
assert not (logs/'fmt.txt').read_text().strip(), 'Formatting check must pass'
assert 'PASS keyboard' in (logs/'pty.txt').read_text() and 'PASS SIGTERM' in (logs/'pty.txt').read_text()
assert 'p95 target <50ms: PASS' in (logs/'pty-performance.txt').read_text()
for name in ['latest.log','kernel-7.1.log']:
    content=(logs/'vm'/name).read_text();assert 'KERNWATCH_PROBES_RESULT=PASS' in content and 'KERNWATCH_GUEST_EXIT=0' in content,name
name='kernwatch-0.1.0-linux-x86_64'
out=root/'dist';out.mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix='kernwatch-release-') as tmp:
    stage=Path(tmp)/name;stage.mkdir()
    for item in ['Cargo.toml','Cargo.lock','LICENSE','README.md','src','probes','examples','scripts','docs','screenshots','tests']:
        source=root/item;dest=stage/item
        if source.is_dir():shutil.copytree(source,dest,ignore=shutil.ignore_patterns('__pycache__','*.pyc'))
        else:shutil.copy2(source,dest)
    (stage/'bin').mkdir();shutil.copy2(root/'target/release/kernwatch',stage/'bin/kernwatch')
    manifest={'version':'0.1.0','built_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),'platform':'Linux x86_64','minimum_glibc':'2.39','tests_passed':test_count,'probe_kernels':['6.19.10-300.fc44.x86_64','7.1.13-200.fc44.x86_64'],'files':{}}
    for file in sorted(stage.rglob('*')):
        if file.is_file():manifest['files'][str(file.relative_to(stage))]={'bytes':file.stat().st_size,'sha256':hashlib.sha256(file.read_bytes()).hexdigest()}
    (stage/'BUILD.json').write_text(json.dumps(manifest,indent=2)+'\n')
    archive=out/(name+'.tar.gz');temporary=out/(name+'.tar.gz.tmp')
    with tarfile.open(temporary,'w:gz') as tar:tar.add(stage,arcname=name)
    os.replace(temporary,archive)
    checksum=hashlib.sha256(archive.read_bytes()).hexdigest()
    (out/(name+'.tar.gz.sha256')).write_text(f'{checksum}  {archive.name}\n')
    with tarfile.open(archive) as tar:
        for path,entry in manifest['files'].items():
            data=tar.extractfile(name+'/'+path).read()
            assert len(data)==entry['bytes'] and hashlib.sha256(data).hexdigest()==entry['sha256'],path
    print(f'Verified {len(manifest["files"])} packaged files: {archive} ({archive.stat().st_size} bytes)')
