"""Generate a bounded, self-terminating workload so a live recording has something to show.

Runs CPU, block I/O and memory workers for a fixed number of seconds, then exits
and removes its scratch directory. It writes only inside the directory it is
given and never touches host configuration.
"""
import multiprocessing
import os
import shutil
import sys
import time
from pathlib import Path

CHUNK = 8 << 20


def burn(deadline, phase):
    """Pulse rather than pin, so the CPU history has a shape instead of a ceiling."""
    value = 0
    time.sleep(phase)
    while time.monotonic() < deadline:
        until = min(deadline, time.monotonic() + 7)
        while time.monotonic() < until:
            for _ in range(100_000):
                value = (value * 6364136223846793005 + 1442695040888963407) & ((1 << 64) - 1)
        time.sleep(4)


def churn_disk(deadline, directory, worker):
    path = Path(directory) / f'io-{worker}.bin'
    payload = os.urandom(CHUNK)
    while time.monotonic() < deadline:
        with open(path, 'wb') as handle:
            for _ in range(8):
                handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        # Read it back through the block layer rather than the page cache.
        fd = os.open(path, os.O_RDONLY | os.O_DIRECT)
        try:
            buffer = bytearray(CHUNK)
            while os.readv(fd, [memoryview(buffer)]) > 0:
                pass
        finally:
            os.close(fd)
        path.unlink(missing_ok=True)


def churn_memory(deadline):
    while time.monotonic() < deadline:
        block = bytearray(192 << 20)
        for offset in range(0, len(block), 4096):
            block[offset] = 1
        time.sleep(0.5)
        del block
        time.sleep(0.5)


def main():
    seconds = float(sys.argv[1])
    directory = Path(sys.argv[2])
    directory.mkdir(parents=True, exist_ok=True)
    deadline = time.monotonic() + seconds
    cpus = max(2, min(6, multiprocessing.cpu_count() // 4))
    workers = [multiprocessing.Process(target=burn, args=(deadline, n * 1.5)) for n in range(cpus)]
    workers += [multiprocessing.Process(target=churn_disk, args=(deadline, directory, n)) for n in range(2)]
    workers.append(multiprocessing.Process(target=churn_memory, args=(deadline,)))
    for worker in workers:
        worker.start()
    for worker in workers:
        worker.join(seconds + 30)
        if worker.is_alive():
            worker.terminate()
    shutil.rmtree(directory, ignore_errors=True)


if __name__ == '__main__':
    main()
