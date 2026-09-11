"""Exercise guided demo startup, takeover and terminal restoration in an isolated PTY."""
from pathlib import Path
import re, os, pty, fcntl, termios, struct, subprocess, tempfile, time, select
root=Path(__file__).resolve().parents[1]
exe=root/'target/release/kernwatch'
with tempfile.TemporaryDirectory(prefix='kernwatch-demo-test-') as work:
    master,slave=pty.openpty()
    original=termios.tcgetattr(slave)
    fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',52,160,0,0))
    process=subprocess.Popen([str(exe),'--demo-tour'],stdin=slave,stdout=slave,stderr=slave,cwd=work,env={**os.environ,'XDG_STATE_HOME':str(Path(work)/'state')})
    output=bytearray()
    def drain(seconds):
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            if select.select([master],[],[],0.02)[0]:
                try:output.extend(os.read(master,262144))
                except OSError:break
    try:
        drain(0.8)
        assert b'DEMO TOUR 1/8' in output,'guided demo did not start'
        drain(5.2)
        # Resize forces a full redraw; ordinary terminal diffs omit unchanged letters.
        fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',52,159,0,0))
        drain(0.4)
        assert b"Tasks: inspect Envoy worker wake latency" in re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]", b"", output),"tour did not advance to Tasks"
        os.write(master,b'0');drain(0.4)
        assert b'paused' in output,'manual takeover was not acknowledged'
        os.write(master,b'q');drain(0.4)
        assert process.wait(timeout=5)==0
        assert termios.tcgetattr(slave)==original,'terminal attributes not restored'
        assert b'\x1b[?1049l' in output,'alternate screen not restored'
    finally:
        if process.poll() is None:process.kill();process.wait()
        os.close(master);os.close(slave)
    rejected=subprocess.run([str(exe),'--demo-tour','--trace'],capture_output=True,text=True)
    assert rejected.returncode!=0 and 'interactive live mode' in rejected.stderr
print('PASS guided demo: startup, scene advance, manual takeover, quit, terminal restoration, live-trace exclusion')
