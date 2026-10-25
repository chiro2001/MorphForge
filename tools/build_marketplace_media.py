#!/usr/bin/env python3
"""Compose CN/JP shop images from existing MorphForge captures, without AI assets."""
import argparse,hashlib,json,subprocess
from pathlib import Path
from PIL import Image,ImageDraw,ImageOps
import render_promo_video as r
ROOT=Path(__file__).resolve().parents[1]

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--promo',type=Path,default=ROOT/'Releases/MorphForge-Promo-20261025-CN-JP');p.add_argument('--out',type=Path,default=ROOT/'docs/marketplace/20261025/media');a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True);raw=a.out/'captures';raw.mkdir(exist_ok=True);records=[]
 def save(im,name,language,sources):
  dest=a.out/name;im.save(dest,quality=94,optimize=True);records.append(dict(file=name,language=language,size=list(im.size),bytes=dest.stat().st_size,sha256=hashlib.sha256(dest.read_bytes()).hexdigest(),sources=sources,fontFamily=r.font_face(language)[2]))
 def base(title,lang,height=1600):
  im=Image.new('RGB',(1600,height),r.BG);d=ImageDraw.Draw(im);r.text(d,(70,45),'MorphForge',38,r.MINT,True,language=lang);r.text(d,(67,120),title,57,r.WHITE,True,1450,language=lang);return im,d
 def fit(im,path,box):
  x,y,w,h=box;source=ImageOps.contain(Image.open(path).convert('RGB'),(w,h),Image.Resampling.LANCZOS);im.paste(source,(x+(w-source.width)//2,y+(h-source.height)//2))
 def capture(lang,time):
  target=raw/f'{lang}-{time:g}.jpg'
  if not target.exists():subprocess.run(['ffmpeg','-v','error','-y','-ss',str(time),'-i',str(a.promo/f'MorphForge-Promo-{lang}-1080p60.mp4'),'-frames:v','1','-q:v','2',str(target)],check=True)
  return target
 for lang in ['zh-CN','ja']:
  ja=lang=='ja';im,d=base('自作モデルをOSCでVRChatへ' if ja else '自己的模型，通过OSC带进VRChat',lang)
  r.text(d,(72,235),'無料・オープンソース' if ja else '免费 · 开源',38,r.MINT,language=lang)
  photo=Image.open(ROOT/'docs/promo-video/20261025/cn-jp/master-sword-cover.jpg').crop((400,0,1480,1080)).resize((1120,1120),Image.Resampling.LANCZOS);im.paste(photo,(240,370));r.text(d,(72,1520),'ボクセル / ローポリ · Windows + Unity Avatar' if ja else '体素 / 低多边形 · Windows + Unity Avatar',30,r.MUTED,language=lang);save(im,f'01-cover-{lang}.jpg',lang,['docs/promo-video/20261025/cn-jp/master-sword-cover.jpg'])
  im,d=base('AIモデリング → OSC → VRChat' if ja else 'AI建模 → OSC → VRChat',lang,2000)
  for y,t,label in [(240,23,'制作画面' if ja else '在工作台创作'),(1110,49,'VRChatで構築' if ja else '在VRChat中搭建')]:
   r.text(d,(74,y-55),label,34,r.MINT,language=lang);fit(im,capture(lang,t),(80,y,1440,810))
  r.text(d,(74,1940),'実録の抜粋。待ち時間の省略・倍速あり。' if ja else '真实录制节选，包含倍速与等待删减。',27,r.MUTED,language=lang);save(im,f'02-workflow-{lang}.jpg',lang,[f'promo-{lang}@23s',f'promo-{lang}@49s'])
  im,d=base('自分の作品をつくる' if ja else '自定义模型，自己的创作',lang)
  names=['sports-car.webp','ramen.webp','night-heron.webp','maple.webp','village-house.webp','hangover.webp']
  for i,name in enumerate(names):fit(im,ROOT/'docs/media/showcase'/name,(55+i%3*510,285+i//3*590,470,545))
  r.text(d,(73,1520),'保存済み作品のエディタープレビュー' if ja else '个人作品库保存的编辑器预览',28,r.MUTED,language=lang);save(im,f'03-works-{lang}.jpg',lang,['docs/media/showcase/'+n for n in names])
  im,d=base('あそび方、いろいろ。' if ja else '把创作变成更多玩法',lang)
  features=[(77,'アニメーション' if ja else '模型动画'),(69,'ローポリの作品' if ja else '低多边形作品'),(100,'字幕・翻訳' if ja else '字幕与翻译'),(87,'つかむ・拡大縮小' if ja else '抓握与缩放')]
  for i,(t,label) in enumerate(features):
   x=65+i%2*770;y=280+i//2*610;r.text(d,(x,y-25),label,34,r.MINT,language=lang);fit(im,capture(lang,t),(x,y+50,705,485))
  r.text(d,(72,1530),'VRChat実機映像からの抜粋' if ja else '截取自VRChat实机录制',27,r.MUTED,language=lang);save(im,f'04-features-{lang}.jpg',lang,[f'promo-{lang}@{t}s' for t,_ in features])
  im,d=base('作品をVRChatで楽しむ' if ja else '连接你的创作与VRChat',lang)
  cards=[('PC / PCVR','Windows x64','制作アプリ＋Avatar表示' if ja else '桌面创作＋Avatar展示'),('OSC','作品をAvatarの周囲に表示' if ja else '让作品出现在Avatar周围','つかむ・拡大縮小・空間配置' if ja else '抓握、缩放与空间摆放'),('アニメーション' if ja else '模型动画','移動・回転・一様拡縮' if ja else '平移、旋转与等比缩放','モデルと動きを一緒に送信' if ja else '模型和动作一起发布'),('AI API','AIモデリング・翻訳に対応' if ja else '接入AI建模与字幕翻译','好みのモデルAPIを接続' if ja else '连接你选择的模型服务')]
  for i,(big,one,two) in enumerate(cards):
   x=70+i%2*760;y=310+i//2*475;d.rounded_rectangle((x,y,x+700,y+405),radius=18,fill=(17,31,43));r.text(d,(x+26,y+35),big,58,r.MINT,True,language=lang);r.text(d,(x+26,y+175),one,31,r.WHITE,True,650,language=lang);r.text(d,(x+26,y+270),two,29,r.MUTED,max_width=650,language=lang)
  r.text(d,(75,1370),'自分のAvatarへUnityで導入・アップロード' if ja else '在Unity为自己的Avatar安装模块并上传',34,r.WHITE,True,language=lang);r.text(d,(75,1480),'導入ガイド・材質カタログ / 253bit / Very Poor' if ja else '导入指南与材质目录 / 标准253bit / Very Poor',29,r.MUTED,language=lang);save(im,f'05-requirements-{lang}.jpg',lang,['docs/releases/0.27.0/Quickstart.'+lang+'.md'])
  im,d=base('表示言語を切り替え' if ja else '中日英界面，随时切换',lang);source=ROOT/'docs/releases/0.27.0/media'/f'workbench-{lang}.png';fit(im,source,(65,330,1470,1020));r.text(d,(75,1440),'Desktop 0.27.0 / Avatar 1.4.0',31,r.MINT,language=lang);r.text(d,(75,1510),'実際のDesktop表示画面' if ja else '真实Desktop界面截图',28,r.MUTED,language=lang);save(im,f'06-interface-{lang}.jpg',lang,[source.relative_to(ROOT).as_posix()])
 (a.out/'sources.json').write_text(json.dumps(dict(images=records,fonts={c:r.font_face(c)[2] for c in ['zh-CN','ja']},method='Actual captures and saved artwork; Pillow composition. No generated product image.'),ensure_ascii=False,indent=2)+'\n');print('Created',len(records),'localized product images')
if __name__=='__main__':main()
