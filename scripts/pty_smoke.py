"""Exercise the real Crossterm process, resize, recording and terminal cleanup."""
import os,pty,fcntl,termios,struct,subprocess,time,select,signal,tempfile,json
from pathlib import Path
root=Path(__file__).resolve().parents[1]
exe=root/'target/release/kernwatch'
def run(terminate=False):
 master,slave=pty.openpty();original=termios.tcgetattr(slave)
 fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',60,160,0,0))
 with tempfile.TemporaryDirectory(prefix='kernwatch-pty-') as work:
  proc=subprocess.Popen([str(exe),'--demo'],stdin=slave,stdout=slave,stderr=slave,cwd=work,env={**os.environ,"XDG_STATE_HOME":str(Path(work)/"state")})
  data=bytearray()
  def drain(seconds):
   end=time.monotonic()+seconds
   while time.monotonic()<end:
    if select.select([master],[],[],0.02)[0]:
     try:data.extend(os.read(master,65536))
     except OSError:break
  drain(0.25)
  for key in [b'1',b'2',b'/envoy\r',b'\r',b'\x1b',b'3',b'h',b'4',b'g',b'5',b'6',b'7',b'8',b' ',b' ',b'9',b'0',b'm',b'd',b'b',b' ']:
   os.write(master,key);drain(0.035)
  fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',24,80,0,0));os.kill(proc.pid,signal.SIGWINCH);drain(0.1)
  os.write(master,b'\x1b');drain(0.05)
  fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',60,160,0,0));os.kill(proc.pid,signal.SIGWINCH)
  os.write(master,b'rf');drain(1.2);os.write(master,b're');drain(0.25)
  if terminate:proc.send_signal(signal.SIGTERM)
  else:os.write(master,b'q')
  drain(0.4);code=proc.wait(timeout=5);drain(0.05)
  assert code==0,(code,data[-400:])
  assert b'\x1b[?1049l' in data,'alternate screen not restored'
  assert data.count(b'\x1b[?2026h') > 0,'frames are not synchronized'
  assert data.count(b'\x1b[?2026l') >= data.count(b'\x1b[?2026h'),'synchronized update left open'
  assert termios.tcgetattr(slave)==original,'terminal attributes not restored'
  records=list(Path(work).glob('*.kwr'));reports=list(Path(work).glob('kernwatch-report-*'))
  assert records and reports,'record/export commands failed'
  replay=subprocess.run([str(exe),'--replay',str(records[0]),'--snapshot'],capture_output=True,text=True,check=True)
  assert json.loads(replay.stdout)['demo'],'recorded demo cannot be replayed'
  manifest=json.loads((reports[0]/'manifest.json').read_text());assert len(manifest['files'])==9
  (root/'tests'/('pty-signal.ansi' if terminate else 'pty-tour.ansi')).write_bytes(data)
  print(f'PASS {"SIGTERM" if terminate else "keyboard"}: resize, navigation, record/freeze/export, terminal cleanup; {len(data)} output bytes')
 os.close(master);os.close(slave)
run();run(True)
