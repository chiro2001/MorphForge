#!/usr/bin/env python3
"""Render the trial avatar's static instructions with an OFL font."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT=Path(__file__).resolve().parents[1]

def main():
    out=ROOT/'docs/trial-avatar/20261025';out.mkdir(parents=True,exist_ok=True)
    face='/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc'
    def f(size):return ImageFont.truetype(face,size)
    im=Image.new('RGBA',(1024,1024),(0,0,0,0));d=ImageDraw.Draw(im)
    d.rounded_rectangle((8,8,1016,1016),radius=55,fill=(10,22,34,242),outline=(91,218,190,255),width=5)
    d.text((65,52),'MorphForge',font=f(99),fill='#edf6ff')
    d.text((69,200),'体験版 / TRIAL',font=f(47),fill='#69e1c6')
    d.text((70,298),'PC / PCVR  ·  Desktop 0.27.0+',font=f(34),fill='#adbfce')
    for i,(title,sub)in enumerate([('1  VRChat の OSC を ON','Enable OSC in VRChat'),('2  Desktop で接続・認識','Connect and detect the avatar'),('3  作品と動きを送信','Publish your model and animation')]):
        y=405+i*148;d.text((70,y),title,font=f(43),fill='#edf6ff');d.text((98,y+68),sub,font=f(28),fill='#adbfce')
    d.line((70,867,954,867),fill='#365365',width=2)
    d.text((70,900),'メニューからガイド表示を切替可能',font=f(28),fill='#adbfce')
    d.text((70,953),'Base: 256Hedgehog / messier_num · CC0',font=f(23),fill='#adbfce')
    path=out/'guide.ja.png';im.save(path,optimize=True);print(path)

if __name__=='__main__':main()
