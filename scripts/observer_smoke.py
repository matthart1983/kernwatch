"""Verify dirty rendering and measure keyboard/resize latency in a real PTY."""
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

root = Path(__file__).resolve().parents[1]
master, slave = pty.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 48, 160, 0, 0))
with tempfile.TemporaryDirectory(prefix='kernwatch-observer-smoke-') as work:
    process = subprocess.Popen([str(root / 'target/release/kernwatch'), '--demo'],
                               stdin=slave, stdout=slave, stderr=slave, cwd=work,
                               env={**os.environ, 'TERM': 'xterm-256color', 'XDG_STATE_HOME': work})
    def drain(seconds):
        data = bytearray()
        until = time.monotonic() + seconds
        while time.monotonic() < until:
            if select.select([master], [], [], min(.02, max(0, until - time.monotonic())))[0]:
                data.extend(os.read(master, 262144))
        assert process.poll() is None, 'early exit'
        return data

    def frame(action):
        start = time.monotonic()
        action()
        data = bytearray()
        while b'\x1b[?2026l' not in data:
            assert time.monotonic() - start < 2, 'input did not redraw'
            if select.select([master], [], [], .01)[0]:
                data.extend(os.read(master, 262144))
        return (time.monotonic() - start) * 1000

    try:
        drain(.4)
        frame(lambda: os.write(master, b'f'))
        drain(.15)
        static = drain(1.5)
        assert b'\x1b[?2026h' not in static, 'frozen screen redrew without a change'
        keys = []
        resizes = []
        for i in range(24):
            keys.append(frame(lambda: os.write(master, b'1' if i % 2 else b'2')))
            width = 160 if i % 2 else 80
            def resize():
                fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 48, width, 0, 0))
                process.send_signal(signal.SIGWINCH)
            resizes.append(frame(resize))
        def result(values):
            values.sort()
            return {'median_ms': values[len(values)//2], 'p95_ms': values[int(len(values)*.95)], 'max_ms': max(values)}
        print(json.dumps({'frozen_redraws': 0, 'keyboard': result(keys), 'resize': result(resizes)}))
    finally:
        process.send_signal(signal.SIGTERM)
        process.wait(timeout=5)
os.close(master)
os.close(slave)
