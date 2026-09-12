"""Build an isolated guest initramfs; no host mounts, devices, or settings changed."""
import pathlib,subprocess,re,stat,gzip,os
root=pathlib.Path(__file__).resolve().parents[2]
subprocess.run(['cc',str(root/'tests/vm/init.c'),'-O2','-o','/tmp/kernwatch-vm-init'],check=True)
files={'init':pathlib.Path('/tmp/kernwatch-vm-init'),'probe_smoke':root/os.environ.get('KERNWATCH_GUEST_BINARY','target/release/examples/probe_smoke')}
if os.environ.get('KERNWATCH_GUEST_PERF') == '1':
 files['perf']=pathlib.Path('/usr/bin/perf')
 files['sleep']=pathlib.Path('/usr/bin/sleep')
for binary in list(files.values()):
 for lib in re.findall(r'(/[^\s()]+)',subprocess.check_output(['ldd',str(binary)],text=True)):
  p=pathlib.Path(lib)
  if p.exists():files[str(p).lstrip('/')]=p
out=bytearray();ino=1
def entry(name,data,mode,rmajor=0,rminor=0):
 global ino
 b=name.encode()+b'\0';header=[ino,mode,0,0,1,0,len(data),0,0,rmajor,rminor,len(b),0];ino+=1
 out.extend(b'070701'+''.join(f'{x:08x}' for x in header).encode()+b);out.extend(b'\0'*(-len(out)%4));out.extend(data);out.extend(b'\0'*(-len(out)%4))
dirs={'dev','proc','sys','tmp'}
for name in files:
 for p in pathlib.PurePosixPath(name).parents:
  if str(p)!='.':dirs.add(str(p))
for d in sorted(dirs,key=lambda p:len(p.split('/'))):entry(d,b'',stat.S_IFDIR|0o755)
entry('dev/console',b'',stat.S_IFCHR|0o600,5,1)
for name,p in files.items():entry(name,p.read_bytes(),stat.S_IFREG|0o755)
entry('TRAILER!!!',b'',0)
with gzip.open('/tmp/kernwatch-test-initramfs.gz','wb')as f:f.write(out)
print('Built /tmp/kernwatch-test-initramfs.gz')
