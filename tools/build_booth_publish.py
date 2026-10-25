#!/usr/bin/env python3
"""Build a reviewable BOOTH publisher kit from the exact validated release files."""
import argparse, hashlib, html, json, re, shutil, subprocess, sys, tempfile, tomllib, zipfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
DOCS=ROOT/'docs/releases/0.27.0'
VERSION='0.27.0'
NAME='MorphForge_Ver.0.27.0'
EXPECTED={
    'Windows/MorphForge Desktop_0.27.0_x64-setup.exe':('dist/windows/MorphForge Desktop_0.27.0_x64-setup.exe','7d763ea397bf853c920f0005878e521c2b8f90607672f0e629dac511234e9979'),
    'Unity/MorphForgeAvatar-1.4.0.unitypackage':('dist/avatar/MorphForgeAvatar-1.4.0.unitypackage','7e60412f16cbc3e9858e7c85c7d02ec9c16d15af94f6c37805190b59c5a39264'),
}

def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def write(p,s):p.parent.mkdir(parents=True,exist_ok=True);p.write_text(s.rstrip()+'\n',encoding='utf-8')
def jsave(p,v):write(p,json.dumps(v,ensure_ascii=False,indent=2))
def copy(source,target):target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,target)
def archive(source,out):
    out.parent.mkdir(parents=True,exist_ok=True)
    with zipfile.ZipFile(out,'w',zipfile.ZIP_DEFLATED,compresslevel=9)as z:
        for p in sorted(source.rglob('*')):
            if p.is_file():
                info=zipfile.ZipInfo(p.relative_to(source).as_posix(),date_time=(2026,10,25,0,0,0));info.compress_type=zipfile.ZIP_DEFLATED;info.external_attr=0o100644<<16;z.writestr(info,p.read_bytes())
    with zipfile.ZipFile(out)as z:
        assert z.testzip()is None
        assert set(z.namelist())=={p.relative_to(source).as_posix()for p in source.rglob('*')if p.is_file()}
        for p in source.rglob('*'):
            if p.is_file():assert z.read(p.relative_to(source).as_posix())==p.read_bytes()

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output',type=Path,default=ROOT/'Releases/MorphForge-0.27.0/BOOTH-Publish')
    a=p.parse_args();output=a.output.resolve()
    config=json.loads((DOCS/'booth-publish-config.json').read_text())
    assert config['desktopVersion']==VERSION and config['avatarVersion']=='1.4.0' and not config['published']
    assert tomllib.loads((ROOT/'Cargo.toml').read_text())['workspace']['package']['version']==VERSION
    for _,(name,digest)in EXPECTED.items():assert sha(ROOT/name)==digest,(name,'not the validated file')
    catalog=ROOT/'dist/avatar/MorphForge-public-catalog.zip'
    with zipfile.ZipFile(catalog)as z:
        assert z.testzip()is None
        assert json.loads(z.read('catalog.json'))==json.loads((ROOT/'Assets/MorphForgeAvatar/CatalogSource/catalog.json').read_text())
        for name in z.namelist():
            assert not name.startswith('/')and '..'not in Path(name).parts
            if name.endswith('.png'):assert hashlib.sha256(z.read(name)).hexdigest()==Path(name).stem
    audit=DOCS/'package-audit.json'
    subprocess.run([sys.executable,str(ROOT/'tools/check_avatar_package.py'),'--output',str(audit)],check=True,cwd=ROOT)
    notices=ROOT/'dist/licenses/MorphForge-0.27.0-third-party-notices.zip'
    license_audit=json.loads(notices.with_suffix('.json').read_text())
    assert license_audit['success']and sha(notices)==license_audit['sha256']
    for folder in ('media','marketing'):
        for row in json.loads((DOCS/folder/'sources.json').read_text())['images']:assert sha(DOCS/folder/row['file'])==row['sha256']
    if output.exists():
        marker=output/'publish-preparation.json'
        assert marker.is_file()and json.loads(marker.read_text())['generatedBy']=='tools/build_booth_publish.py','Refusing to replace an unrecognized directory'
    output.parent.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='booth-',dir=output.parent)as temp:
        built=Path(temp)/'BOOTH-Publish';buyer=built/'Download'/NAME;buyer.mkdir(parents=True)
        for dest,(name,_)in EXPECTED.items():copy(ROOT/name,buyer/dest)
        copy(catalog,buyer/'Unity'/catalog.name)
        for lang in ('ja','zh-CN','en'):copy(DOCS/f'Quickstart.{lang}.md',buyer/f'Documentation/Quickstart.{lang}.md')
        license_files={
            'Program-MIT.txt':'examples/licenses/MIT.txt',
            'PixelWorkshop-MIT.txt':'materials/PIXEL-WORKSHOP-LICENSE.txt',
            'UnityPixelTextures-MIT.txt':'Assets/MorphForgeAvatar/LICENSE.txt',
            'NotoSansSC-OFL.txt':'fonts/OFL-NotoSansSC.txt',
            'Unifont-OFL.txt':'fonts/pixel16/UNIFONT-OFL-1.1.txt',
            'Unifont-Original-Licenses.txt':'fonts/pixel16/UNIFONT-LICENSE.txt',
            'PixelFont-ATTRIBUTION.md':'fonts/pixel16/ATTRIBUTION.md',
            'References-ATTRIBUTION.md':'references/ATTRIBUTION.md',
            'Examples-ATTRIBUTION.md':'examples/ATTRIBUTION.md',
            'CC-BY-NC-SA-4.0.txt':'references/licenses/CC-BY-NC-SA-4.0.txt',
            'ONNX-Runtime-MIT.txt':'licenses/ONNX-Runtime-MIT.txt',
            'Sherpa-ONNX-Apache-2.0.txt':'licenses/Sherpa-ONNX-Apache-2.0.txt',
            'Silero-VAD-MIT.txt':'licenses/Silero-VAD-MIT.txt',
        }
        for dest,source in license_files.items():copy(ROOT/source,buyer/'Licenses'/dest)
        for source in sorted((ROOT/'web/licenses').iterdir()):
            if source.is_file():copy(source,buyer/'Licenses/Web'/source.name)
        for source in sorted((ROOT/'web/vendor').rglob('*')):
            if source.is_file()and re.match(r'^(license|notice|copying)',source.name,re.I):copy(source,buyer/'Licenses/WebVendor'/source.relative_to(ROOT/'web/vendor'))
        copy(notices,buyer/'Licenses/Rust-dependencies.zip')
        write(buyer/'Licenses/README.txt','''LICENSE MAP / ライセンス案内 / 许可索引

Program Cargo metadata: MIT. Program-MIT.txt preserves the project's existing MIT notice.
Original textures: PixelWorkshop-MIT.txt and UnityPixelTextures-MIT.txt.
Fonts: NotoSansSC-OFL.txt, Unifont-OFL.txt and PixelFont-ATTRIBUTION.md.
Whale reference images and whale-maid derivative sample: CC-BY-NC-SA-4.0.txt;
credits and original source links are preserved in References-ATTRIBUTION.md and Examples-ATTRIBUTION.md.
Original character: 上善无形. Derivative references: ZipZipPipe.
These images/examples are included in the Desktop installer and retain noncommercial/share-alike terms.
They are not the examples used for this publisher kit's marketing captures.
ONNX Runtime, Sherpa-ONNX and Silero VAD: the named original notices.
Rust-dependencies.zip: full locked Cargo graphs, original notices and source-origin records.
It includes build/test/non-Windows dependencies; it is not a Windows binary composition claim.
Unmodified sources of MPL-2.0 dependencies and source-declaration fallbacks are in SourceArchives/.
Web/: shipped UI notices. Original third-party terms remain in force.

程序、材质、字体、示例和第三方库分别许可，不能把整个下载称为CC0或全部素材可商用。
プログラム・材質・フォント・サンプル・依存ライブラリはそれぞれの条件に従います。
Final shop terms are pending publisher confirmation; see ../LICENSE.txt.
''')
        write(buyer/'LICENSE.txt','''PUBLICATION PREPARATION / 公開準備版 / 发布准备版

Existing licenses are preserved; this document does not relicense third-party content.
The program Cargo metadata declares MIT; original textures have their own MIT notices.
Fonts: SIL OFL 1.1. Whale reference images and the whale-maid derivative example:
CC BY-NC-SA 4.0, with attribution, noncommercial and share-alike conditions.
Libraries retain their own original terms. See Licenses/README.txt and the full notices.

The final shop price, product-specific terms and support contact await publisher review.
The 0 JPY setting is a draft, not confirmation of a public release or universal asset license.
最终商品售价、使用条款和联系方式待发布者确认；原有素材许可保持有效。
価格・最終商品規約・問い合わせ先は公開者の確認待ちです。元の各ライセンスは維持されます。
''')
        write(buyer/'Readme.txt','''MorphForge Desktop 0.27.0 + Avatar 1.4.0
2026-10-25 / Publication preparation / 公開準備版 / 发布准备版

日本語: Documentation/Quickstart.ja.md
简体中文: Documentation/Quickstart.zh-CN.md
English: Documentation/Quickstart.en.md

Windows/: Desktop x64 installer.
Unity/: Avatar Unitypackage + matching public catalog.
Documentation/: three-language installation and verification guides.
Licenses/: original licenses, attribution and dependency notices.
LICENSE.txt: license map and final publisher-review status.
SHA256SUMS.txt: unpacked file hashes (excluding the checksum file itself).

Windows PC/PCVR. Standard module:253 synchronized bits; Very Poor rank expected.
Character avatar, SDK/MA/NDMF/VRCFury, API keys and service credits are not included.
Manual creation needs no API key. Optional AI/translation uses your own paid/free provider.
Desktop defaults to the system language. Unity menu language changes require avatar re-upload.
Read the guide before installation; back up your avatar and existing work.
''')
        write(buyer/'Changelog.txt','''Desktop 0.27.0 / Avatar 1.4.0 — 2026-10-25

日本語
Desktopの表示言語をシステム追従・简体中文・日本語・Englishから即時切替。
Unityの画面とAvatarメニューの言語を選択。各Avatarは独立したメニューを使用。
現行版には原生点線、4枠テキスト、原文付き字幕翻訳、3グループアニメーションを含みます。
唯一の保存済みアニメーションは自動選択され、メイン送信でモデルと同時に送信されます。

简体中文
Desktop支持跟随系统与中日英即时切换；Unity可选择界面及Avatar菜单语言。
每个Avatar菜单独立持久保存；修改VRChat菜单需要重新上传。
当前版本还包含原生点线、四框文字、保留原文的字幕翻译、三组动画。
唯一保存动画默认选中，主界面发布同时发送模型与所选动画。

English
Immediate Desktop language switching: system, Chinese, Japanese and English.
Unity interface and per-avatar menu language selection, with persistent independent menus.
Current features include native points/lines, four text panels, original-plus-translated
captions and three-group animation. A single saved clip is automatically selected and
published with the model from the main publication button.

No real-client FPS claim or full-capacity16K endurance claim is made for this release.
''')
        info={'name':'MorphForge','desktopVersion':VERSION,'avatarVersion':'1.4.0','date':config['date'],'priceJpyDraft':config['priceJpy'],'priceConfirmed':config['priceConfirmed'],'licenseFinalized':config['licenseFinalized'],'publicMaterialCount':544,'synchronizedBits':253,'languages':['zh-CN','ja','en'],'capacityConfigured':16384,'remoteExecutionAck':False,'files':{dest:{'sha256':digest}for dest,(_,digest)in EXPECTED.items()},'catalogSha256':sha(catalog),'dependencyNoticesSha256':sha(notices)}
        jsave(buyer/'PackageInfo.json',info)
        write(buyer/'SHA256SUMS.txt',''.join(sha(f)+'  '+f.relative_to(buyer).as_posix()+'\n'for f in sorted(buyer.rglob('*'))if f.is_file()))
        download=built/'Download'/(NAME+'.zip');archive(buyer,download)
        pages=built/'ProductPage';images=pages/'Images';images.mkdir(parents=True)
        for lang in ('ja','zh-CN','en'):
            text=(DOCS/f'BOOTH.{lang}.md').read_text();text=re.sub(r'^# +','',text,flags=re.M)
            write(pages/f'Description.{lang}.txt',text+f'\nZIP: {download.stat().st_size/1048576:.1f} MiB / Desktop0.27.0 / Avatar1.4.0\n')
        for row in json.loads((DOCS/'marketing/sources.json').read_text())['images']:
            folder=images/'AlternativeCovers'if row['file']in ('01-cover-en.png','01-cover-zh-CN.png')else images
            copy(DOCS/'marketing'/row['file'],folder/row['file'])
        copy(DOCS/'marketing/sources.json',pages/'image-sources.json')
        write(pages/'商品填写参考.zh-CN.txt','''商品标题：MorphForge｜ボクセル・図形・字幕をVRChatへ
商品类型：下载商品。分类请在当前BOOTH后台选择合适的软件/3D工具项。
价格：0円草案，尚未由发布者确认；以booth-publish-config.json为准。
文件：只上传Download/MorphForge_Ver.0.27.0.zip，勿把PublisherKit作为买家下载。
主文案：Description.ja.txt；中文/英文文案可追加或另放说明区。
图片顺序：01-cover-ja.png →02-workbench.png →03-points-lines.png →04-requirements.png →05-setup.png。
AlternativeCovers/内为中英文替代封面，不必重复上传三张相同构图。
建议标签：VRChat、OSC、ボクセル、制作ツール、ModularAvatar、字幕。
标明AI辅助制作；图片为实际Desktop画面，不是VRChat实机截图。
请先确认价格、最终商品条款和支持联系方式，再处理文案末尾“公开者确认待ち”段落。
不要把私人在线头像ID、API Key、云凭据或用户工程上传到商品文件。
''')
        checks=built/'Checks';checks.mkdir()
        for source in (audit,DOCS/'clean-import.json',DOCS/'clean-import-validation.json',DOCS/'clean-import-console.json',DOCS/'offline-browser.json',notices.with_suffix('.json'),ROOT/'docs/localization-20261025/automated-tests.json',ROOT/'docs/localization-20261025/unity-validation.json',ROOT/'docs/localization-20261025/browser/validation.json',ROOT/'docs/localization-20261025/windows-payload.json'):
            if source.is_file():copy(source,checks/('localization-browser.json'if source.parent.name=='browser'else source.name))
        pending=[]
        if not config['priceConfirmed']:pending.append('确认最终售价（当前0円仅为草案）')
        if not config['licenseFinalized']:pending.append('确认最终商品使用条款；保留所有素材与依赖的原许可')
        if not config['supportContact']:pending.append('填写支持联系方式')
        clean=json.loads((DOCS/'clean-import.json').read_text())if(DOCS/'clean-import.json').exists()else{'success':False,'status':'not-recorded'}
        if not clean.get('success'):pending.append('隔离工程原包重新导入验证尚未执行：验证Editor未就绪，根因待查，详见Checks/clean-import.json')
        pending.append('公开前按指南人工复验目标Avatar、VR环境与其他客户端；1.4.0未作完整多人实机验收')
        report={'generatedBy':'tools/build_booth_publish.py','status':'PREPARED_FOR_REVIEW','desktopVersion':VERSION,'avatarVersion':'1.4.0','download':download.relative_to(built).as_posix(),'zipBytes':download.stat().st_size,'zipSha256':sha(download),'verifiedUnchangedValidatedBinaries':True,'catalogMatchesPublicPlugin':True,'zipCrcValid':True,'allZipBytesMatchStaging':True,'fileCount':sum(f.is_file()for f in buyer.rglob('*')),'dependencyLicenseCollection':license_audit,'cleanImport':clean.get('success',False),'pending':pending,'published':False}
        jsave(built/'publish-preparation.json',report);jsave(built/'booth-publish-config.json',config)
        write(built/'先看这里.txt','MorphForge BOOTH发布准备包\n\n先打开index.html离线审阅。\n买家上传文件：'+report['download']+'\n商品页材料：ProductPage/\n验证记录：Checks/\n\n发布前待办：\n'+'\n'.join('- '+x for x in pending)+'\n\n本次只整理发布包，没有上传BOOTH、推送源码或创建GitHub Release。')
        cards=''.join('<figure><img loading="lazy" src="ProductPage/Images/'+html.escape(row['file'])+'"><figcaption>'+html.escape(row['file'])+'</figcaption></figure>'for row in json.loads((DOCS/'marketing/sources.json').read_text())['images']if row['file']not in ('01-cover-en.png','01-cover-zh-CN.png'))
        descriptions=''.join('<details><summary>'+lang+'商品文案</summary><pre>'+html.escape((pages/f'Description.{lang}.txt').read_text())+'</pre></details>'for lang in ('ja','zh-CN','en'))
        write(built/'index.html','''<!doctype html><html lang="zh-CN"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>MorphForge BOOTH 发布审阅</title><style>body{margin:0;background:#09111d;color:#eaf1f8;font:17px/1.75 system-ui,sans-serif}main{max-width:1160px;margin:auto;padding:40px 24px}a{color:#6ce1c7}h1{line-height:1.3}.notice{border:1px solid #b99a4e;border-radius:12px;padding:20px;background:#222319}figure{margin:28px 0}img{display:block;width:100%;border-radius:12px}figcaption{color:#a7bbcf;font-size:14px}details{background:#142334;padding:18px;margin:16px 0;border-radius:10px}pre{white-space:pre-wrap;font:inherit;overflow-wrap:anywhere}code{overflow-wrap:anywhere}nav{display:flex;gap:24px;flex-wrap:wrap}summary{cursor:pointer}</style><main><h1>MorphForge BOOTH 发布审阅</h1><p>Desktop 0.27.0 / Avatar 1.4.0 · 2026-10-25 · 参考 TeslaRoadsterMA 的交付结构</p><div class="notice"><strong>材料已整理，等待发布者审阅</strong><ul>'''+''.join('<li>'+html.escape(x)+'</li>'for x in pending)+'''</ul></div><p>买家ZIP：'''+f'{download.stat().st_size/1048576:.1f} MiB'+'''。原安装器、Unity包摘要未变；包内有三语指南、配套材质目录和独立许可。</p><nav><a href="'''+report['download']+'''">买家下载ZIP</a><a href="ProductPage/商品填写参考.zh-CN.txt">商品填写参考</a><a href="publish-preparation.json">校验与待办</a><a href="ProductPage/image-sources.json">配图来源</a><a href="booth-publish-config.json">价格与条款草案</a></nav><p>SHA256: <code>'''+report['zipSha256']+'''</code></p>'''+cards+descriptions+'''<p>图片均为真实Desktop画面及排版。没有上传BOOTH或创建在线Release。</p></main></html>''')
        jsave(built/'publisher-files-sha256.json',{f.relative_to(built).as_posix():sha(f)for f in sorted(built.rglob('*'))if f.is_file()})
        if output.exists():shutil.rmtree(output)
        shutil.move(str(built),str(output))
    publisher_zip=output.parent/'MorphForge-0.27.0-PublisherKit.zip';archive(output,publisher_zip)
    write(publisher_zip.with_suffix('.zip.sha256'),sha(publisher_zip)+'  '+publisher_zip.name)
    jsave(DOCS/'publish-preparation.json',report)
    print(json.dumps({'success':True,'output':str(output),'download':report['download'],'zipBytes':report['zipBytes'],'publisherKitSha256':sha(publisher_zip),'pending':pending},ensure_ascii=False))

if __name__=='__main__':main()
