"""Measure key-to-output bursts and live process CPU/RSS in a real PTY."""
import os,pty,fcntl,termios,struct,subprocess,time,select,tempfile,statistics
from pathlib import Path
root=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='kernwatch-perf-') as work:
 master,slave=pty.openpty()
 fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',60,160,0,0))
 p=subprocess.Popen([str(root/'target/release/kernwatch'),'--view','dense'],stdin=slave,stdout=slave,stderr=slave,cwd=work,env={**os.environ,'XDG_STATE_HOME':str(Path(work)/'state')})
 def drain(duration):
  end=time.monotonic()+duration
  while time.monotonic()<end:
   if select.select([master],[],[],0.01)[0]:os.read(master,262144)
 drain(1.5)
 samples=[]
 for key in b'1234567890mdb'*8:
  drain(0.025)
  start=time.monotonic();os.write(master,bytes([key]));last=None
  deadline=start+1
  while time.monotonic()<deadline:
   if select.select([master],[],[],0.004)[0]:
    os.read(master,262144);last=time.monotonic()
   elif last is not None:break
  assert last is not None,'no output after key'
  samples.append((last-start)*1000)
 def ticks():
  v=Path(f'/proc/{p.pid}/stat').read_text().rsplit(')',1)[1].split()
  return int(v[11])+int(v[12])
 before=ticks();start=time.monotonic();drain(5);elapsed=time.monotonic()-start
 cpu=(ticks()-before)/os.sysconf('SC_CLK_TCK')/elapsed*100
 rss=next(v for v in Path(f'/proc/{p.pid}/status').read_text().splitlines() if v.startswith('VmRSS:'))
 os.write(master,b'q');drain(0.3);assert p.wait(timeout=5)==0
 os.close(master);os.close(slave)
 samples.sort();p95=samples[int(len(samples)*.95)]
 print(f'Live unprivileged PTY, 160x60, {len(samples)} navigation keys: p50={statistics.median(samples):.2f}ms p95={p95:.2f}ms max={max(samples):.2f}ms')
 print(f'Input-to-last-byte of output burst; 4ms idle boundary; excludes physical terminal paint. p95 target <50ms: {"PASS" if p95<50 else "FAIL"}')
 print(f'Live monitor over {elapsed:.2f}s: CPU={cpu:.2f}% of one core; {rss}; probes off. Host/workload specific.')
 assert p95<50,'input responsiveness target exceeded'
