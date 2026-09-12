"""Native full-application CPU capture acceptance and repeated throughput pairs.

Requires permission to load BPF; do not treat emulated-VM timings as overhead.
Records only aggregate timing/quality in output; temporary host reports are removed.
"""
import fcntl
import json
import mmap
import os
from pathlib import Path
import pty
import select
import signal
import statistics
import struct
import subprocess
import tempfile
import termios
import time
from monitor_bench import sample

root = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='kernwatch-full-profile-') as work:
    work = Path(work)
    workload = subprocess.Popen([str(root/'target/release/examples/observer_workload'), str(work/'counter')])
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 48, 160, 0, 0))
    monitor = subprocess.Popen([str(root/'target/release/kernwatch'), '--view', 'dense'], stdin=slave, stdout=slave, stderr=slave,
                               cwd=work, env={**os.environ, 'TERM': 'xterm-256color', 'XDG_STATE_HOME': str(work)})
    def drain(seconds):
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            if select.select([master], [], [], min(.02, max(0, until-time.monotonic())))[0]:
                os.read(master, 262144)
            assert monitor.poll() is None and workload.poll() is None, 'early exit'
    def command(value):
        os.write(master, (':'+value+'\r').encode())
        drain(1)
    try:
        drain(2)
        with (work/'counter').open('rb') as file:
            counter = mmap.mmap(file.fileno(), 16, access=mmap.ACCESS_READ)
            def count(): return sum(struct.unpack('=QQ', counter[:]))
            def window():
                before = sample(monitor.pid); start = time.monotonic(); initial = count()
                drain(6)
                elapsed = time.monotonic()-start; rate = (count()-initial)/elapsed
                after = sample(monitor.pid)
                cpu = 100*(after['cpu_seconds']+after['reaped_child_seconds']-before['cpu_seconds']-before['reaped_child_seconds'])/elapsed
                return rate, cpu
            results = []
            for repetition in range(3):
                for hz in ([49,99] if repetition % 2 == 0 else [99,49]):
                    before, idle_cpu = window()
                    command(f'probe cpu tgid={workload.pid} seconds=12 hz={hz}')
                    drain(1)
                    during, capture_cpu = window()
                    command('probe stop')
                    after, _ = window()
                    old = set(work.glob('kernwatch-report-*'))
                    os.write(master, b'e'); drain(1)
                    reports = set(work.glob('kernwatch-report-*'))-old
                    assert len(reports) == 1, 'capture report missing'
                    report = reports.pop()
                    profile = json.loads((report/'profile.json').read_text())
                    assert profile['metadata']['kind'] == 'cpu' and profile['root']['samples'] > 0
                    assert profile['tasks'] and all(k.startswith(f'{workload.pid}:') for k in profile['tasks']), 'scope mismatch'
                    ratio = during / statistics.mean([before,after])
                    row = {'hz':hz,'repetition':repetition,'throughput_ratio':ratio,'monitor_idle_cpu_pct':idle_cpu,'monitor_capture_cpu_pct':capture_cpu,
                           'samples':profile['root']['samples'],'failed':profile['quality']['failed']}
                    results.append(row); print(json.dumps(row), flush=True)
            # System-wide capture also retains the busy process; no implicit self exclusion.
            command('probe cpu seconds=10 hz=49'); drain(6); command('probe stop')
            old = set(work.glob('kernwatch-report-*')); os.write(master,b'e'); drain(1)
            profile = json.loads(((set(work.glob('kernwatch-report-*'))-old).pop()/'profile.json').read_text())
            assert any(k.startswith(f'{workload.pid}:') for k in profile['tasks']), 'system scope missed workload'
            print(json.dumps({'summary': {str(hz): {'median_throughput_ratio': statistics.median(r['throughput_ratio'] for r in results if r['hz']==hz),
                                                  'ratios': [r['throughput_ratio'] for r in results if r['hz']==hz]} for hz in [49,99]}, 'system_samples':profile['root']['samples']}), flush=True)
            counter.close()
    finally:
        for child in [monitor, workload]:
            child.send_signal(signal.SIGTERM)
            try: child.wait(timeout=5)
            except subprocess.TimeoutExpired: child.kill(); child.wait()
        os.close(master); os.close(slave)
