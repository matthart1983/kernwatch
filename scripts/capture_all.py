"""Capture every view at its declared reference grid. Run from any directory."""
from pathlib import Path
import subprocess
root=Path(__file__).resolve().parents[1]
subprocess.run(['cargo','build','--offline','--manifest-path',str(root/'Cargo.toml'),'--example','capture'],check=True)
out=root/'screenshots/current';out.mkdir(parents=True,exist_ok=True)
names=['dense','overview','tasks','scheduler','memory','block','syscalls','irq','cgroups','modules','ebpf','dmesg','diagnose']
for i,name in enumerate(names):
 tab=12 if i==0 else i-1
 height=52 if i==0 else 68 if i==1 else 60 if name in ['tasks','memory','cgroups','ebpf'] else 50
 path=out/f'{i:02}-{name}.json'
 with path.open('w') as f:subprocess.run([str(root/'target/debug/examples/capture'),str(tab),'160',str(height)],stdout=f,check=True)
 subprocess.run(['python3',str(root/'scripts/render_capture.py'),str(path),str(path.with_suffix('.png'))],check=True)
print('Captured 13 Ratatui buffers and PNGs')
