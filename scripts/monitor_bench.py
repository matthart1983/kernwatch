"""Measure whole-process CPU/RSS and terminal traffic without enabling probes.

Run on the actual host (not a PID namespace containing only the benchmark).
Results contain aggregate counts and timings, never task names or host records.
"""
import argparse
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import tempfile
import termios
import time


def sample(pid):
    fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
    status = dict(line.split(':', 1) for line in Path(f'/proc/{pid}/status').read_text().splitlines())
    return {'user_seconds': int(fields[11]) / os.sysconf('SC_CLK_TCK'),
            'system_seconds': int(fields[12]) / os.sysconf('SC_CLK_TCK'),
            'reaped_child_seconds': (int(fields[13]) + int(fields[14])) / os.sysconf('SC_CLK_TCK'),
            'cpu_seconds': (int(fields[11]) + int(fields[12])) / os.sysconf('SC_CLK_TCK'),
            'rss_kib': int(status['VmRSS'].split()[0]), 'threads': int(status['Threads'])}


def thread_cpu(pid):
    result = {}
    for path in Path(f'/proc/{pid}/task').iterdir():
        try:
            text = (path / 'stat').read_text()
            fields = text.rsplit(')', 1)[1].split()
            result[path.name] = (text.split('(', 1)[1].rsplit(')', 1)[0],
                                 (int(fields[11]) + int(fields[12])) / os.sysconf('SC_CLK_TCK'))
        except FileNotFoundError:
            pass
    return result


def run(binary, seconds, warmup, mode):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 48, 160, 0, 0))
    with tempfile.TemporaryDirectory(prefix='kernwatch-monitor-bench-') as work:
        args = [str(binary), '--view', 'dense']
        if mode == 'demo':
            args.append('--demo')
        child = subprocess.Popen(args, stdin=slave, stdout=slave, stderr=slave, cwd=work,
                                 env={**os.environ, 'XDG_STATE_HOME': work, 'TERM': 'xterm-256color'})
        traffic = 0

        def drain(duration):
            nonlocal traffic
            until = time.monotonic() + duration
            while time.monotonic() < until:
                if select.select([master], [], [], min(0.05, max(0, until - time.monotonic())))[0]:
                    traffic += len(os.read(master, 262144))
                if child.poll() is not None:
                    raise RuntimeError(f'kernwatch exited early: {child.returncode}')

        try:
            drain(warmup)
            start = time.monotonic()
            before = sample(child.pid)
            threads_before = thread_cpu(child.pid)
            traffic_before = traffic
            samples = []
            while time.monotonic() - start < seconds:
                drain(min(1, seconds - (time.monotonic() - start)))
                samples.append({'seconds': round(time.monotonic() - start, 3), **sample(child.pid)})
            elapsed = time.monotonic() - start
            after = samples[-1]
            per_thread = {}
            for tid, (name, cpu) in thread_cpu(child.pid).items():
                delta = cpu - threads_before.get(tid, (name, 0))[1]
                per_thread[name] = per_thread.get(name, 0) + 100 * delta / elapsed
            return {'binary': str(binary), 'mode': mode, 'warmup_seconds': warmup,
                    'elapsed_seconds': elapsed,
                    'cpu_pct_one_core': 100 * (after['cpu_seconds'] - before['cpu_seconds']) / elapsed,
                    'user_cpu_pct': 100 * (after['user_seconds'] - before['user_seconds']) / elapsed,
                    'system_cpu_pct': 100 * (after['system_seconds'] - before['system_seconds']) / elapsed,
                    'reaped_helper_cpu_pct': 100 * (after['reaped_child_seconds'] - before['reaped_child_seconds']) / elapsed,
                    'observer_cpu_pct': 100 * (after['cpu_seconds'] + after['reaped_child_seconds'] - before['cpu_seconds'] - before['reaped_child_seconds']) / elapsed,
                    'rss_start_kib': before['rss_kib'], 'rss_end_kib': after['rss_kib'],
                    'rss_peak_kib': max(s['rss_kib'] for s in samples),
                    'terminal_bytes_per_second': (traffic - traffic_before) / elapsed,
                    'threads': after['threads'], 'thread_cpu_pct': per_thread, 'samples': samples}
        finally:
            child.send_signal(signal.SIGTERM)
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()
            os.close(master)
            os.close(slave)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binaries', nargs='+', type=Path, help='Measured in the specified order')
    parser.add_argument('--seconds', type=int, default=30)
    parser.add_argument('--warmup', type=int, default=10)
    parser.add_argument('--mode', choices=['live', 'demo'], default='live')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.seconds < 1 or args.warmup < 0:
        parser.error('--seconds must be positive and --warmup nonnegative')
    results = {'logical_cpus': os.cpu_count(), 'kernel': os.uname().release,
               'visible_processes': sum(p.name.isdigit() for p in Path('/proc').iterdir()),
               'runs': []}
    for binary in args.binaries:
        result = run(binary.resolve(), args.seconds, args.warmup, args.mode)
        results['runs'].append(result)
        args.output.write_text(json.dumps(results, indent=2))
        print(json.dumps({k: v for k, v in result.items() if k != 'samples'}), flush=True)
