"""Record real profiling in a networkless disposable QEMU VM, then check evidence.

Requires cc, QEMU x86-64, an x86-64 BTF-enabled Linux kernel, vhs and ffmpeg.
KERNWATCH_DEMO_KERNEL selects the kernel; KERNWATCH_DEMO_WORK selects scratch.
Only the demo workload is built with frame pointers; the app is the release binary.
"""
import argparse
import gzip
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess

ROOT = Path(__file__).resolve().parents[1]
WORK = Path(os.environ.get('KERNWATCH_DEMO_WORK', '/tmp/kernwatch-profile-demo'))
# Loaded in the tour so its Modules view has real entries. None of them adds a disk.
TOUR_MODULES = ('drivers/net/dummy.ko.xz', 'drivers/net/tun.ko.xz', 'drivers/net/veth.ko.xz',
                'drivers/block/loop.ko.xz')


def kernel():
    kernels = sorted(Path('/boot').glob('vmlinuz-*'), key=lambda p: p.stat().st_mtime)
    return os.environ.get('KERNWATCH_DEMO_KERNEL') or str(kernels[-1])


def build(workload, tour=False):
    WORK.mkdir(parents=True, exist_ok=True)
    subprocess.run(['cargo', 'build', '--offline', '--locked', '--release'], cwd=ROOT, check=True)
    subprocess.run(['cargo', 'build', '--offline', '--locked', '--release', '--example', workload],
                   cwd=ROOT, env={**os.environ, 'RUSTFLAGS': '-C force-frame-pointers=yes'}, check=True)
    subprocess.run(['cc', '-O2', '-Wall', '-Werror', str(ROOT / 'scripts/profile_demo_init.c'),
                    '-o', str(WORK / 'init')], check=True)
    files = {'init': WORK / 'init', 'kernwatch': ROOT / 'target/release/kernwatch',
             'profile-demo': ROOT / 'target/release/examples' / workload}
    for binary in list(files.values()):
        for lib in re.findall(r'(/[^\s()]+)', subprocess.check_output(['ldd', str(binary)], text=True)):
            path = Path(lib)
            if path.exists():
                files[str(path).lstrip('/')] = path
    if tour:
        (WORK / 'tour').touch()
        files['tour'] = WORK / 'tour'
        version = Path(kernel()).name.removeprefix('vmlinuz-')
        for module in TOUR_MODULES:
            path = Path('/lib/modules') / version / 'kernel' / module
            if path.exists():
                files['modules/' + path.name] = path
    archive = bytearray()
    inode = 0

    def entry(name, data, mode, major=0, minor=0):
        nonlocal inode
        inode += 1
        encoded = name.encode() + b'\0'
        fields = [inode, mode, 0, 0, 1, 0, len(data), 0, 0, major, minor, len(encoded), 0]
        archive.extend(b'070701' + ''.join(f'{value:08x}' for value in fields).encode() + encoded)
        archive.extend(b'\0' * (-len(archive) % 4))
        archive.extend(data)
        archive.extend(b'\0' * (-len(archive) % 4))

    directories = {'dev', 'proc', 'sys', 'tmp'}
    for name in files:
        directories.update(str(p) for p in Path(name).parents if str(p) != '.')
    for name in sorted(directories, key=lambda p: len(p.split('/'))):
        entry(name, b'', stat.S_IFDIR | 0o755)
    entry('dev/console', b'', stat.S_IFCHR | 0o600, 5, 1)
    for name, path in files.items():
        entry(name, path.read_bytes(), stat.S_IFREG | 0o755)
    entry('TRAILER!!!', b'', 0)
    with gzip.open(WORK / 'initramfs.gz', 'wb') as output:
        output.write(archive)


def run(tour=False):
    machine = ['-accel', 'tcg', '-m', '1024', '-smp', '2']
    if tour:
        disk = WORK / 'disk.img'
        with disk.open('wb') as image:
            image.truncate(1 << 30)
        # KVM where available; four CPUs give the per-CPU views something to compare.
        machine = ['-accel', 'kvm', '-accel', 'tcg', '-cpu', 'max', '-m', '2048', '-smp', '4',
                   '-drive', f'file={disk},if=virtio,format=raw']
    subprocess.run(['qemu-system-x86_64', *machine,
                    '-kernel', kernel(), '-initrd', str(WORK / 'initramfs.gz'),
                    '-append', 'console=hvc0 rdinit=/init quiet loglevel=0 panic=-1',
                    '-display', 'none', '-monitor', 'none',
                    '-device', 'virtio-serial', '-chardev', 'stdio,id=terminal,signal=off',
                    '-device', 'virtconsole,chardev=terminal',
                    '-serial', 'file:' + str(WORK / 'evidence.log'), '-nic', 'none', '-no-reboot'],
                   check=True, timeout=180)


def verify(required=(), compare=True):
    text = (WORK / 'evidence.log').read_text()
    reports = {}
    for name, payload in re.findall(r'BEGIN ([^\n]+)\n(.*?)\nEND', text, re.S):
        reports[name] = json.loads(payload)
    profiles = [p for name, p in reports.items() if name.endswith('/profile.json')]
    expected = 2 if compare else 1
    assert len(profiles) == expected, f'Expected {expected} actual captures, got {len(profiles)}'
    for profile in profiles:
        assert profile['root']['samples'] > 0, 'Capture is empty'
        assert profile['metadata']['unit'] == 'cpu_samples'
    frames = {key: frame for profile in profiles for key, frame in profile['frames'].items()}
    if compare:
        comparisons = [p for name, p in reports.items() if name.endswith('/comparison.json')]
        assert len(comparisons) == 1, 'Comparison was not exported'
        for function, sign in [('parse_headers', 1), ('cache_lookup', -1)]:
            changes = [change for change in comparisons[0]['changes']
                       if function in frames.get(change['path'][-1], {}).get('display', '')]
            assert changes and sum(change['delta'] for change in changes) * sign > 20, \
                f'{function} did not change by at least 20 percentage points in the expected direction'
        assert any(frame['raw'] != frame['display'] and 'parse_headers' in frame['display']
                   for frame in frames.values()), 'Demangled workload symbol is missing'
    for function in required:
        assert any(function in frame['display'] for frame in frames.values()), f'{function} was never sampled'
    (WORK / 'verified-reports.json').write_text(json.dumps(reports, indent=2))
    print(f'Verified {expected} real CPU capture(s)' + (' and the exported baseline comparison.' if compare else '.'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', action='store_true', help='Run the prepared VM in this terminal')
    parser.add_argument('--build', action='store_true', help='Only prepare the VM')
    parser.add_argument('--verify', action='store_true', help='Only validate the exported profiles')
    parser.add_argument('--flame', action='store_true', help='Use the flame graph workload and tape')
    parser.add_argument('--tour', action='store_true', help='Use the general tour: disk, services, modules and probes')
    args = parser.parse_args()
    if args.tour:
        workload, tape, required = 'webapp', 'tour_demo.tape', ('handle_request', 'compare_keys')
    elif args.flame:
        workload, tape, required = 'webapp', 'flame_demo.tape', ('expand_block', 'siphash_round', 'compare_keys', 'vfs_write')
    else:
        workload, tape, required = 'profile_demo_workload', 'profile_demo.tape', ()
    if args.run:
        run(args.tour)
    elif args.verify:
        verify(required, not args.tour)
    else:
        build(workload, args.tour)
        if not args.build:
            for tool in ('vhs', 'ffmpeg'):
                if not shutil.which(tool):
                    raise SystemExit(f'{tool} is required')
            subprocess.run(['vhs', str(ROOT / 'scripts' / tape)], cwd=ROOT, check=True)
            verify(required, not args.tour)


if __name__ == '__main__':
    main()
