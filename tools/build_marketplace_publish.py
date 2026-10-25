#!/usr/bin/env python3
"""Prepare BOOTH/Xianyu copy, shared validated downloads and localized media offline."""
import argparse,hashlib,html,json,re,shutil,subprocess,zipfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];DOCS=ROOT/'docs/marketplace/20261025'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,value):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(value.rstrip()+'\n',encoding='utf-8')
def jsave(p,value):save(p,json.dumps(value,ensure_ascii=False,indent=2))
def copy(a,b):b.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(a,b)

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--out',type=Path,default=ROOT/'Releases/MorphForge-Marketplace-20261025');a=p.parse_args();out=a.out
 if out.exists():assert (out/'marketplace-preparation.json').is_file(),'Unrecognized output directory'
 out.mkdir(parents=True,exist_ok=True);config=json.loads((DOCS/'publish-config.json').read_text());assert config['priceConfirmed'] and [v['priceJpy'] for v in config['booth']['variants']]==[0,100] and config['xianyu']['priceCny']==0
 promo=ROOT/'Releases/MorphForge-Promo-20261025-CN-JP';prior=json.loads((promo/'verification.json').read_text());assert prior['status']=='passed' and prior['durationSeconds']==111.2
 old=ROOT/'Releases/MorphForge-0.27.0/BOOTH-Publish/Download/MorphForge_Ver.0.27.0.zip';assert sha(old)=='5b6ed57ca4a009d62a3d0bee9c38cca83e4e1eb04f09535a1e45d36a6baae5d8'
 buyer=out/'Download/MorphForge_Desktop0.27.0_Avatar1.4.0.zip';buyer.parent.mkdir(exist_ok=True)
 with zipfile.ZipFile(old) as source:
  assert source.testzip() is None;entries={n:source.read(n) for n in source.namelist()}
 entries['Readme.txt']=('''MorphForge Desktop 0.27.0 + Avatar 1.4.0
無料版 / 免费版 / Free edition: 0 JPY
応援版 / 支持版 / Optional support: 100 JPY — same files and features.
Source / ソースコード / 开源代码: https://github.com/chiro2001/MorphForge

Documentation/Quickstart.ja.md
Documentation/Quickstart.zh-CN.md
Documentation/Quickstart.en.md

Windows/: validated Windows x64 installer.
Unity/: validated public Avatar plugin and matching material catalog.
Licenses/: separate original component licenses, attribution and dependency notices.
SHA256SUMS.txt: package checksums.

Windows PC/PCVR. Standard module: 253 synchronized bits. Very Poor rank expected.
Character avatar, SDK/MA/NDMF, API keys and service credits are not included.
Manual editing needs no API key. AI/translation uses your own configured service.
Support via the platform's internal messages. Do not send keys or credentials.
Optional support does not unlock extra features or change third-party licenses.
''').encode()
 entries['LICENSE.txt']=('''MorphForge license map / ライセンス / 许可说明

Source: https://github.com/chiro2001/MorphForge
Program Cargo metadata declares MIT. Existing original notices are in Licenses/.
Original textures, fonts, reference images, samples and third-party libraries each
retain their own terms. This file does not relicense any third-party material.

Whale references and the whale-maid derivative sample retain CC BY-NC-SA 4.0.
Original character: 上善无形. Derivative references: ZipZipPipe.
See Licenses/References-ATTRIBUTION.md and Licenses/Examples-ATTRIBUTION.md.
Do not describe all included assets as CC0, MIT or unrestricted commercial content.

Free and optional 100 JPY support editions contain identical files and features.
A support payment does not grant extra rights in third-party material.
All original license notices and conditions remain effective.
''').encode()
 for name in list(entries):
  if name.endswith(('.txt','.md')):
   text=entries[name].decode();text=text.replace('最終商品規約、価格、問い合わせ先は公開者の確認待ちです。','無料版0円、任意の応援版100円は同内容です。お問い合わせは利用プラットフォーム内メッセージへ。').replace('Final shop terms are pending publisher confirmation; see ../LICENSE.txt.','See ../LICENSE.txt for component terms; free and support editions have identical content.');entries[name]=text.encode()
 info=json.loads(entries['PackageInfo.json']);info.pop('priceJpyDraft',None);info.update(priceJpy=0,priceConfirmed=True,optionalSupportJpy=100,sameContentForAllVariants=True,sourceRepository=config['sourceRepository']);entries['PackageInfo.json']=(json.dumps(info,ensure_ascii=False,indent=2)+'\n').encode()
 entries.pop('SHA256SUMS.txt');entries['SHA256SUMS.txt']=''.join(hashlib.sha256(data).hexdigest()+'  '+name+'\n' for name,data in sorted(entries.items())).encode()
 with zipfile.ZipFile(buyer,'w',zipfile.ZIP_DEFLATED,compresslevel=9) as z:
  for name,data in sorted(entries.items()):
   entry=zipfile.ZipInfo(name,date_time=(2026,10,25,0,0,0));entry.compress_type=zipfile.ZIP_DEFLATED;entry.external_attr=0o100644<<16;z.writestr(entry,data)
 with zipfile.ZipFile(buyer) as z:
  assert z.testzip() is None
  assert hashlib.sha256(z.read('Windows/MorphForge Desktop_0.27.0_x64-setup.exe')).hexdigest()=='7d763ea397bf853c920f0005878e521c2b8f90607672f0e629dac511234e9979'
  assert hashlib.sha256(z.read('Unity/MorphForgeAvatar-1.4.0.unitypackage')).hexdigest()=='7e60412f16cbc3e9858e7c85c7d02ec9c16d15af94f6c37805190b59c5a39264'
 jsave(out/'publish-config.json',config)
 for code,platform in [('ja','BOOTH'),('zh-CN','Xianyu')]:
  video=promo/f'MorphForge-Promo-{code}-1080p60.mp4';copy(video,out/platform/'Video'/video.name)
  for path in sorted((DOCS/'media').glob(f'*-{code}.jpg')):copy(path,out/platform/'Images'/path.name)
  copy(DOCS/f'{platform}-Description.{code}.txt',out/platform/f'Description.{code}.txt')
 copy(DOCS/'BOOTH-Description.zh-CN.txt',out/'BOOTH/Description.zh-CN.txt');copy(DOCS/'Xianyu-Title.zh-CN.txt',out/'Xianyu/Title.zh-CN.txt')
 save(out/'BOOTH/Title.ja.txt','MorphForge｜自作モデルをOSCでVRChatへ【無料・オープンソース】')
 fields=dict(itemType='Digital',name=(out/'BOOTH/Title.ja.txt').read_text().strip(),category={'id':215,'label':'3D Tools & Systems'},adult=False,event=None,tags=['VRChat','OSC','ボクセル','ローポリ','制作ツール'],variants=[dict(v,download='Download/'+buyer.name,sha256=sha(buyer)) for v in config['booth']['variants']],description='BOOTH/Description.ja.txt',images=[p.relative_to(out).as_posix() for p in sorted((out/'BOOTH/Images').glob('*.jpg'))],video='BOOTH/Video/MorphForge-Promo-ja-1080p60.mp4',videoEmbedding='Upload to an allowed video host first, then use Description > Embedded Widget. No live video URL has been created.',overseasShipping='Not used for digital downloads',published=False)
 jsave(out/'BOOTH/Fields.json',fields)
 save(out/'BOOTH/Fields.zh-CN.md','''# 依照保存页面填写

| 页面栏目 | 填写内容 |
| --- | --- |
| Item Type | Digital（下载商品） |
| Name | 复制Title.ja.txt |
| Item image | Images内01至06，按编号顺序 |
| Category | 3D Tools & Systems，快照分类ID215 |
| Adult | All Age |
| Description | 复制Description.ja.txt；中文另有Description.zh-CN.txt |
| Embedded Widget／埋め込みコンテンツ | 按Video/EMBED-GUIDE.zh-CN.md放入视频嵌入代码 |
| Tag | VRChat、OSC、ボクセル、ローポリ、制作ツール |
| Event | None |
| Price／variation | 免费版0日元；新增支持档100日元，名称标明同内容 |
| Add/Edit Files | 两档都使用根目录Download/内同一份ZIP |
| Sell out of Japan | 数字商品不使用实体转运资料 |

视频MP4用于商品介绍，不当作收费下载内容。不要将PublisherKit、检查日志或原始BOOTH网页作为买家文件。
保存草稿可检查排版，公开按钮由发布者最后操作。尚未填写真实YouTube地址，不可把模板中的占位符当成可播放链接。
''')
 credit=(promo/'MUSIC-CREDIT.txt').read_text();copy(promo/'MUSIC-CREDIT.txt',out/'Shared/MUSIC-CREDIT.txt')
 save(out/'BOOTH/Video/YouTube-title.ja.txt','MorphForge｜自作モデルをOSCでVRChatへ｜AIモデリング・実機デモ')
 save(out/'BOOTH/Video/YouTube-description.ja.txt',(DOCS/'BOOTH-Description.ja.txt').read_text()+'\n\nMusic / 音楽\n'+credit)
 save(out/'BOOTH/Video/EMBED-GUIDE.zh-CN.md','''# BOOTH视频放置

本包提供日文完整版MP4及YouTube标题、介绍。先在自己的视频账号上传该MP4，再复制该视频的嵌入代码。BOOTH的Description区域选择Embedded Widget／埋め込みコンテンツ，粘贴嵌入代码并预览。保存页面本身提示YouTube videos。

本次没有上传视频，也没有创建真实视频URL。下面只是格式模板，必须替换为实际视频ID，或直接使用视频网站提供的真实嵌入代码：

```html
<iframe width="560" height="315" src="https://www.youtube.com/embed/REPLACE_WITH_REAL_VIDEO_ID" title="MorphForge" frameborder="0" allowfullscreen></iframe>
```

上传视频时保留完整片尾，并将YouTube-description.ja.txt的音乐署名放在视频介绍中。支持档使用同一段演示视频。
''')
 light=out/'Xianyu/Video/MorphForge-Promo-zh-CN-1080p30-light.mp4'
 if not light.exists():
  with (out/'light-encode.log').open('wb') as log:
   subprocess.run(['ffmpeg','-hide_banner','-loglevel','warning','-y','-filter_threads','2','-threads','4','-i',str(out/'Xianyu/Video/MorphForge-Promo-zh-CN-1080p60.mp4'),'-map','0:v:0','-map','0:a:0','-vf','fps=30','-c:v','libx264','-preset','fast','-crf','23','-threads','4','-pix_fmt','yuv420p','-c:a','aac','-b:a','128k','-sn','-movflags','+faststart',str(light)],stdout=log,stderr=subprocess.STDOUT,check=True)
 save(out/'Xianyu/Fields.zh-CN.md','''# 闲鱼填写参考

标题：复制Title.zh-CN.txt。
价格：用户指定0元；不自动改为1元或其他收费价格。
正文：复制Description.zh-CN.txt，包含公开源码地址。
图片：Images内01至06，按编号顺序。
视频：先用Video内1080p30-light轻量版；同内容1080p60完整版也已准备。两版都是111.2秒，轻量版只降低帧率和码率。
交付资料：根目录Download内买家ZIP；安装问题使用闲鱼站内消息。

当前没有用户的闲鱼发布页，未核实该账号对应类目、0元提交或视频上限。按实际页面提供的合法软件／数字资料类目选择；如果不接受0元或该类内容，保存草稿，不能用无关类目、虚假1元或二维码绕过。
公开源码地址是产品资料，不是站外收款入口；如果页面不接受链接，可使用Source-link.txt记录的源码信息，不采用绕过审核的引流写法。
''')
 save(out/'Xianyu/Source-link.txt',config['sourceRepository']);save(out/'Xianyu/Video/Video-description.zh-CN.txt','MorphForge 免费开源工具：AI建模，通过OSC把自定义模型带进VRChat。实录包含倍速与等待删减。\n开源代码：'+config['sourceRepository']+'\n\n'+credit)
 for lang in ['zh-CN','ja','en']:
  for ext in ['srt','ass']:copy(promo/f'MorphForge-Promo.{lang}.{ext}',out/'Shared/Subtitles'/f'MorphForge-Promo.{lang}.{ext}')
 for name in ['booth-form-reference.json','PLATFORM-NOTES.zh-CN.md','sources.json','source-repository.json'] :copy(DOCS/name,out/'Checks'/name)
 copy(DOCS/'media/sources.json',out/'Checks/image-sources.json');copy(promo/'verification.json',out/'Checks/promo-verification.json')
 save(out/'先看这里.md','''# MorphForge 双平台发布资料

先打开index.html离线审阅。

BOOTH/：日文标题与文案、中文对照、6张商品图、111.2秒日文视频、YouTube介绍及嵌入步骤、逐项填写表。免费0日元和100日元自愿支持两档内容相同。
Xianyu/：中文标题与文案、6张商品图、相同中文完整版和轻量版视频、0元填写参考与源码地址。
Download/：两平台共用买家ZIP。安装器与Unity包保持已验证原文件，只更新价格与开源说明等文字。
Shared/：三语字幕和必须保留的音乐署名。
Checks/：表单、媒体、许可与校验资料。

尚未实际发布到任何平台或上传视频。BOOTH嵌入需真实的视频托管地址；刺猬试用头像尚无公开链接，不虚构试用入口。支持档不会重授第三方素材权利；原包非商业素材条件保留，具体事项见Checks/PLATFORM-NOTES.zh-CN.md。
''')
 preview='''<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>MorphForge 双平台发布资料</title><style>body{background:#09111b;color:#edf4fa;font:18px/1.7 system-ui,'Microsoft YaHei',sans-serif;margin:0}main{max-width:1100px;margin:auto;padding:36px 24px}a{color:#74e4c4}video{width:100%;max-height:620px;background:black}img{width:100%;height:auto}.gallery{display:grid;grid-template-columns:repeat(3,1fr);gap:14px}section,details{padding:20px;background:#14212e;border-radius:12px;margin:22px 0}pre{white-space:pre-wrap;overflow-wrap:anywhere;font:inherit}pre[lang=ja]{font-family:'Yu Gothic','Meiryo',sans-serif}.note{color:#ebce83}h1,h2{line-height:1.3}</style><main><h1>MorphForge 双平台发布资料</h1><p>Desktop 0.27.0 / Avatar 1.4.0 · 免费0日元／支持100日元 · 闲鱼0元</p><p class="note">资料准备完成，尚未在线发布。BOOTH视频托管地址待上传取得；原始许可保留。</p><p><a href="Download/'''+buyer.name+'''">买家ZIP</a> · <a href="publish-config.json">价格配置</a> · <a href="Checks/PLATFORM-NOTES.zh-CN.md">发布者事项</a></p>'''
 for code,platform,title in [('ja','BOOTH','BOOTH 日文'),('zh-CN','Xianyu','闲鱼中文')]:
  preview+='<section><h2>'+title+'</h2><p><a href="'+platform+'/Fields.zh-CN.md">按页面栏目填写</a></p><video controls preload="metadata" poster="'+platform+'/Images/01-cover-'+code+'.jpg" src="'+platform+'/Video/MorphForge-Promo-'+code+'-1080p60.mp4"></video><div class="gallery">'
  for image in sorted((out/platform/'Images').glob('*.jpg')):preview+='<a href="'+image.relative_to(out).as_posix()+'"><img loading="lazy" src="'+image.relative_to(out).as_posix()+'"></a>'
  preview+='</div><details open><summary>可复制正文</summary><pre lang="'+code+'">'+html.escape((out/platform/f'Description.{code}.txt').read_text())+'</pre></details></section>'
 preview+='<p><a href="Xianyu/Video/MorphForge-Promo-zh-CN-1080p30-light.mp4">闲鱼轻量版视频</a> · <a href="BOOTH/Video/EMBED-GUIDE.zh-CN.md">BOOTH嵌入步骤</a> · <a href="Shared/MUSIC-CREDIT.txt">音乐署名</a></p></main></html>';save(out/'index.html',preview)
 report=dict(generatedBy='tools/build_marketplace_publish.py',status='PREPARED_NOT_PUBLISHED',published=False,priceConfirmed=True,boothPricesJpy=[0,100],xianyuPriceCny=0,sameBuyerFileForFreeAndSupport=True,buyerFile=buyer.relative_to(out).as_posix(),buyerSha256=sha(buyer),buyerBytes=buyer.stat().st_size,validatedBinariesUnchanged=True,sourceRepository=config['sourceRepository'],sourcePagePrivateHtmlExcluded=True,pending=['Upload the Japanese video to a supported host and insert its real embed code.','Review optional-support distribution against the retained third-party noncommercial terms.','Verify current Xianyu category/zero-price/video acceptance in the seller UI.','Source link included; public main/local0.27.0 alignment unverified. No push or Release created.'])
 jsave(out/'marketplace-preparation.json',report);jsave(DOCS/'preparation.json',report);print(json.dumps(report,ensure_ascii=False,indent=2))
if __name__=='__main__':main()
