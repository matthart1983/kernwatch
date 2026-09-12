"""Capture every view at its declared reference grid. Run from any directory."""
from pathlib import Path
import subprocess
root=Path(__file__).resolve().parents[1]
subprocess.run(['cargo','build','--offline','--manifest-path',str(root/'Cargo.toml'),'--example','capture'],check=True)
out=root/'screenshots/current';out.mkdir(parents=True,exist_ok=True)
# (capture name, view index): named rather than derived, so adding a view
# cannot re-point an existing capture at the wrong screen.
views=[('dense',12),('overview',0),('tasks',1),('scheduler',2),('memory',3),('block',4),('syscalls',5),('irq',6),('cgroups',7),('modules',8),('ebpf',9),('dmesg',10),('diagnose',11),('flame',13)]
for i,(name,tab) in enumerate(views):
 height=52 if i==0 else 68 if i==1 else 60 if name in ['tasks','memory','cgroups','ebpf'] else 40 if name=='flame' else 50
 path=out/f'{i:02}-{name}.json'
 with path.open('w') as f:subprocess.run([str(root/'target/debug/examples/capture'),str(tab),'160',str(height)],stdout=f,check=True)
 subprocess.run(['python3',str(root/'scripts/render_capture.py'),str(path),str(path.with_suffix('.png'))],check=True)
print(f'Captured {len(views)} Ratatui buffers and PNGs')
