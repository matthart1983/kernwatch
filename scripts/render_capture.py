"""Rasterize the actual Ratatui buffer using fixed terminal cell geometry."""
import json,sys,os
from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
s=json.load(open(sys.argv[1]));cw,ch=12,24
im=Image.new('RGB',(s['width']*cw,s['height']*ch),'#0b1015');d=ImageDraw.Draw(im)
candidates=[os.environ.get('KERNWATCH_FONT',''),'/usr/share/fonts/adwaita-mono-fonts/AdwaitaMono-Regular.ttf','/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf','/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf']
font=next((p for p in candidates if p and Path(p).is_file()),None)
if font is None:raise SystemExit('Install Adwaita Mono or DejaVu Sans Mono, or set KERNWATCH_FONT to a font file')
f=ImageFont.truetype(font,18)
for i,(text,fg,bg) in enumerate(s['cells']):
 x,y=(i%s['width'])*cw,(i//s['width'])*ch
 d.rectangle((x,y,x+cw-1,y+ch-1),fill=bg)
 if len(text)==1 and 0x2800<=ord(text)<=0x28ff:
  bits=ord(text)-0x2800
  for column,seq in enumerate([[0,1,2,6],[3,4,5,7]]):
   for row,bit in enumerate(seq):
    if bits&(1<<bit):d.ellipse((x+column*6+2,y+row*6+2,x+column*6+3,y+row*6+3),fill=fg)
 else:d.text((x,y+1),text,font=f,fill=fg)
im.save(sys.argv[2])
