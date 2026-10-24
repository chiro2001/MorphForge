#!/usr/bin/env python3
"""Lay out BOOTH cards using verified real captures; requires Pillow."""
import hashlib, json
from pathlib import Path
from PIL import Image, ImageChops, ImageDraw, ImageFont, ImageOps

ROOT=Path(__file__).resolve().parents[1]
DOCS=ROOT/'docs/releases/0.27.0'
FONT='/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc'
BOLD='/usr/share/fonts/noto-cjk/NotoSansCJK-Bold.ttc'
BG='#080f1a'; PANEL='#132132'; WHITE='#edf5fd'; MUTED='#a8bbce'; ACCENT='#63e1c5'

def font(n,bold=False):return ImageFont.truetype(BOLD if bold else FONT,n)
def base(kicker,title):
    im=Image.new('RGB',(1600,1000),BG);d=ImageDraw.Draw(im)
    d.text((76,45),kicker,font=font(22,True),fill=ACCENT)
    d.text((72,90),title,font=font(54,True),fill=WHITE)
    d.line((76,190,1524,190),fill='#253a50',width=2)
    return im,d
def save(im,name,sources,records):
    out=DOCS/'marketing';out.mkdir(exist_ok=True);path=out/name;im.save(path,optimize=True)
    records.append({'file':name,'bytes':path.stat().st_size,'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'size':list(im.size),'sources':sources,'layout':'Pillow; actual product captures only; no generated artwork'})
def paste_fit(im,path,box):
    source=Image.open(path).convert('RGB');source=ImageOps.contain(source,(box[2]-box[0],box[3]-box[1]),Image.Resampling.LANCZOS)
    im.paste(source,(box[0]+(box[2]-box[0]-source.width)//2,box[1]+(box[3]-box[1]-source.height)//2))
def main():
    records=[];src=DOCS/'media';sources=json.loads((src/'sources.json').read_text())
    for r in sources['images']:assert hashlib.sha256((src/r['file']).read_bytes()).hexdigest()==r['sha256']
    hero=Image.open(src/'leviathan-lantern-editor.png').convert('RGB')
    diff=ImageChops.difference(hero,Image.new('RGB',hero.size,hero.getpixel((0,0)))).convert('L').point(lambda v:255 if v>10 else 0)
    cropped=hero.crop(diff.getbbox())
    for lang,lines,sub in [
        ('ja',['つくる。動かす。','Avatarに表示。'],'ボクセル・図形・テキストの制作ワークベンチ'),
        ('zh-CN',['创作，让它动起来。','显示在你的 Avatar 上。'],'体素、几何与文字的创作工作台'),
        ('en',['Create. Animate.','Display on your avatar.'],'A workbench for voxels, geometry and text')]:
        im=Image.new('RGB',(1600,1000),hero.getpixel((0,0)));d=ImageDraw.Draw(im)
        d.text((76,55),'WINDOWS WORKBENCH + UNITY AVATAR MODULE',font=font(22,True),fill=ACCENT)
        d.text((69,115),'MorphForge',font=font(105,True),fill=WHITE)
        for i,line in enumerate(lines):d.text((76,300+i*86),line,font=font(46 if lang=='en'else 49,True),fill=WHITE)
        d.text((78,525),sub,font=font(25),fill=MUTED)
        fitted=ImageOps.contain(cropped,(690,680),Image.Resampling.LANCZOS);im.paste(fitted,(855+(670-fitted.width)//2,190+(680-fitted.height)//2))
        d.text((80,660),'OSC  /  Live text  /  Group animation',font=font(27),fill=ACCENT)
        d.text((80,717),'中文  /  日本語  /  English',font=font(30,True),fill=WHITE)
        d.text((80,785),'Desktop 0.27.0  +  Avatar 1.4.0',font=font(26),fill=MUTED)
        d.line((76,885,1524,885),fill='#253a50',width=2)
        d.text((80,914),'PC / PCVR   ·   253 sync bits   ·   Very Poor rank expected',font=font(24),fill=MUTED)
        d.text((1065,840),'Actual Desktop render',font=font(22),fill=MUTED)
        save(im,'01-cover-'+lang+'.png',['media/leviathan-lantern-editor.png'],records)
    im,d=base('02 / WORKBENCH','日本語・简体中文・English')
    paste_fit(im,src/'workbench-ja.png',(62,210,1538,905))
    d.text((78,933),'実際の Desktop 画面 / Actual Desktop screenshot · UI language switches immediately',font=font(23),fill=MUTED)
    save(im,'02-workbench.png',['media/workbench-ja.png'],records)
    im,d=base('03 / DRAWING','点と線を、端点と太さで指定。')
    paste_fit(im,src/'drawing-controls-en.png',(35,205,1190,905))
    for y,t in [(290,'Point diameter'),(350,'Line endpoints'),(410,'Line thickness'),(520,'Native commands'),(580,'No per-frame'),(625,'vertex streaming')]:d.text((1190,y),t,font=font(27),fill=WHITE if y<500 else ACCENT)
    d.text((78,934),'実際の Desktop デモ画面 / Actual drawing dialog · Geometry shares the avatar capacity',font=font(23),fill=MUTED)
    save(im,'03-points-lines.png',['media/drawing-controls-en.png'],records)
    im,d=base('04 / READ BEFORE INSTALLING','導入前の仕様チェック')
    cards=[('253 bits','標準モジュールの同期コスト','既存の服装・面捕と容量を共有'),('4 × 128','テキスト枠 × UTF-16スロット','字幕も同じ4枠を使用'),('3 × 4','アニメーショングループ × キーフレーム','移動・回転・正の一様拡縮'),('PC / PCVR','Windows x64 + WebView2','Quest / Android 単体は未検証')]
    for i,(big,label,detail)in enumerate(cards):
        x=76+(i%2)*740;y=225+(i//2)*320
        d.rounded_rectangle((x,y,x+708,y+290),radius=18,fill=PANEL)
        d.text((x+30,y+24),big,font=font(58,True),fill=ACCENT)
        d.text((x+30,y+130),label,font=font(29,True),fill=WHITE)
        d.text((x+30,y+195),detail,font=font(26),fill=MUTED)
    d.text((80,910),'Very Poor ランク前提 / FPS保証なし / 人物Avatar・SDK・API Keyは別途用意',font=font(25),fill=MUTED)
    save(im,'04-requirements.png',['Quickstart.ja.md'],records)
    im,d=base('05 / SETUP','編集して、Avatarへ送信。')
    for i,(title,body)in enumerate([('Desktop','Windowsアプリで制作'),('Unity','自分のAvatarに導入・アップロード'),('VRChat','OSCを有効にして送信')]):
        x=76+i*500;d.rounded_rectangle((x,255,x+460,540),radius=18,fill=PANEL);d.text((x+25,270),f'0{i+1}',font=font(58,True),fill=ACCENT);d.text((x+25,365),title,font=font(40,True),fill=WHITE);d.text((x+25,460),body,font=font(21),fill=MUTED)
    d.text((80,615),'単一の保存済みアニメーションは自動選択。',font=font(35,True),fill=WHITE)
    d.text((80,680),'メイン画面の送信ボタンで、モデルと選択中の動作を一緒に送信。',font=font(29),fill=MUTED)
    d.text((80,780),'完了表示 = ローカルUDP送出完了。受信ACKはなく、途中参加には再送を使用。',font=font(27),fill=ACCENT)
    d.text((80,920),'Unity 2022.3.22f1 / SDK 3.10.5 / MA 1.18.7 / NDMF 1.14.8',font=font(24),fill=MUTED)
    save(im,'05-setup.png',['Quickstart.ja.md'],records)
    (DOCS/'marketing/sources.json').write_text(json.dumps({'images':records,'disclosure':'Actual Desktop captures and typography; no VRChat client image or third-party shop artwork.'},ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'success':True,'images':len(records)}))

if __name__=='__main__':main()
