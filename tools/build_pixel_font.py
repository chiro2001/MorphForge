#!/usr/bin/env python3
"""Build lossless 1-bit BMP atlases from official GNU Unifont HEX and JP BDF."""
import argparse,gzip,hashlib,json
from pathlib import Path

def read_hex(path):
 out={}
 for line in gzip.decompress(path.read_bytes()).decode().splitlines():
  code,data=line.split(':',1);cp=int(code,16)
  if cp<65536 and len(data)in(32,64):
   width=8 if len(data)==32 else 16;rows=[data[i:i+width//4] for i in range(0,len(data),width//4)]
   out[cp]=(width,bytes.fromhex(''.join(x+'00' if width==8 else x for x in rows)))
 return out

def read_bdf(path):
 out={};block=[]
 for line in gzip.decompress(path.read_bytes()).decode().splitlines():
  line=line.strip()
  if line.startswith('STARTCHAR'):block=[]
  block.append(line)
  if line!='ENDCHAR':continue
  props={x.split(' ',1)[0]:x.split(' ',1)[1] for x in block if ' 'in x}
  cp=int(props['ENCODING'])
  if not 0<=cp<65536:continue
  w,h,x,y=map(int,props['BBX'].split());advance=int(props['DWIDTH'].split()[0]);advance=8 if advance<=8 else 16
  rows=block[block.index('BITMAP')+1:-1];mask=[0]*16;top=14-(h+y)
  for row,data in enumerate(rows):
   yy=top+row
   if not 0<=yy<16:continue
   bits=int(data,16);padded=((w+7)//8)*8
   for col in range(w):
    xx=x+col
    if 0<=xx<16 and bits&(1<<(padded-col-1)):mask[yy]|=1<<(15-xx)
  out[cp]=(advance,b''.join(v.to_bytes(2,'big')for v in mask))
 return out

def main():
 p=argparse.ArgumentParser();p.add_argument('--source',type=Path,required=True);p.add_argument('--out',type=Path,default=Path('fonts/pixel16'));a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
 zh=read_hex(a.source/'unifont-18.0.01.hex.gz');jp=read_bdf(a.source/'unifont_jp-18.0.01.bdf.gz');report={'format':'MorphForgePixelFont/1','sourceVersion':'GNU Unifont 18.0.01','textureWidth':2048,'textureHeight':1024,'bytesPerGlyph':32,'codepoints':65536,'fonts':{}}
 for name,glyphs in [('zh',zh),('jp',jp)]:
  bitmap=bytearray(65536*32);widths=bytearray([255])*65536
  for cp,(width,mask)in glyphs.items():bitmap[cp*32:(cp+1)*32]=mask;widths[cp]=width
  for cp in [0,9,10,13]:widths[cp]=0
  for suffix,data in [('bin',bitmap),('widths.bin',widths)]: (a.out/f'{name}.{suffix}').write_bytes(data)
  report['fonts'][name]={'glyphs':len(glyphs),'bitmapBytes':len(bitmap),'widthBytes':len(widths),'sha256':hashlib.sha256(bitmap).hexdigest()}
 (a.out/'manifest.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
if __name__=='__main__':main()
