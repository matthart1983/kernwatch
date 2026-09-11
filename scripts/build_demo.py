"""Build a GIF and MP4 from the actual Ratatui guided-demo scenes."""
from pathlib import Path
import json, subprocess, tempfile
from PIL import Image
root=Path(__file__).resolve().parents[1]
subprocess.run(['cargo','build','--locked','--example','capture'],cwd=root,check=True)
out=root/'screenshots/demo';out.mkdir(parents=True,exist_ok=True)
frames=[]
for scene in range(8):
    path=out/f'{scene:02}.json'
    with path.open('w') as f:
        subprocess.run([str(root/'target/debug/examples/capture'),'12','160','52',str(scene)],stdout=f,check=True)
    png=path.with_suffix('.png')
    subprocess.run(['python3',str(root/'scripts/render_capture.py'),str(path),str(png)],check=True)
    frames.append(Image.open(png).convert('RGB'))
frames[0].save(out/'kernwatch-demo.gif',save_all=True,append_images=frames[1:],duration=5000,loop=0,optimize=True)
subprocess.run(['ffmpeg','-y','-loglevel','error','-framerate','1/5','-i',str(out/'%02d.png'),'-c:v','libx264','-r','24','-pix_fmt','yuv420p','-movflags','+faststart',str(out/'kernwatch-demo.mp4')],check=True)
print('Demo GIF:',out/'kernwatch-demo.gif')
print('Demo video:',out/'kernwatch-demo.mp4')
