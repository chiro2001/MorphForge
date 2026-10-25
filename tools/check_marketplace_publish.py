#!/usr/bin/env python3
"""Validate the prepared market materials without accessing either seller account."""
import argparse,hashlib,json,re,subprocess,zipfile
from fractions import Fraction
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote,urlparse
ROOT=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
class Links(HTMLParser):
 def __init__(self):super().__init__();self.links=[]
 def handle_starttag(self,t,a):self.links.extend(v for k,v in a if k in ['src','href','poster'])
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--directory',type=Path,default=ROOT/'Releases/MorphForge-Marketplace-20261025');p.add_argument('--work',type=Path,required=True);a=p.parse_args();base=a.directory;a.work.mkdir(parents=True,exist_ok=True);checks=[]
 def check(name,ok,detail=None):
  checks.append(dict(name=name,passed=bool(ok),detail=detail));print(name,bool(ok),flush=True)
  if not ok:raise AssertionError(name)
 config=json.loads((base/'publish-config.json').read_text());prep=json.loads((base/'marketplace-preparation.json').read_text());fields=json.loads((base/'BOOTH/Fields.json').read_text());url='https://github.com/chiro2001/MorphForge'
 check('Preparation remains unpublished',not prep['published'] and not config['published'])
 check('User-confirmed free and support prices',config['priceConfirmed'] and [v['priceJpy'] for v in config['booth']['variants']]==[0,100] and config['xianyu']['priceCny']==0)
 check('Both BOOTH variations use exactly the same buyer download',len(fields['variants'])==2 and len({v['download'] for v in fields['variants']})==1 and {v['sha256'] for v in fields['variants']}=={prep['buyerSha256']})
 check('BOOTH fields follow saved Digital / 3D Tools / All Age form',fields['itemType']=='Digital' and fields['category']=={'id':215,'label':'3D Tools & Systems'} and fields['adult'] is False and fields['event'] is None)
 check('GitHub source appears in all product descriptions',all(url in (base/p).read_text() for p in ['BOOTH/Description.ja.txt','BOOTH/Description.zh-CN.txt','Xianyu/Description.zh-CN.txt','Xianyu/Source-link.txt']))
 buyer=base/prep['buyerFile'];check('Buyer ZIP hash and size match preparation',sha(buyer)==prep['buyerSha256'] and buyer.stat().st_size==prep['buyerBytes'] and buyer.stat().st_size<1200000000)
 with zipfile.ZipFile(buyer) as z:
  check('Buyer ZIP CRC and paths valid',z.testzip() is None and all(not Path(n).is_absolute() and '..' not in Path(n).parts for n in z.namelist()))
  sums={line.split('  ',1)[1]:line.split('  ',1)[0] for line in z.read('SHA256SUMS.txt').decode().splitlines()}
  check('All buyer payload files covered by correct SHA256',set(sums)==set(z.namelist())-{'SHA256SUMS.txt'} and all(hashlib.sha256(z.read(n)).hexdigest()==h for n,h in sums.items()))
  check('Validated desktop and Unity binaries unchanged',hashlib.sha256(z.read('Windows/MorphForge Desktop_0.27.0_x64-setup.exe')).hexdigest()=='7d763ea397bf853c920f0005878e521c2b8f90607672f0e629dac511234e9979' and hashlib.sha256(z.read('Unity/MorphForgeAvatar-1.4.0.unitypackage')).hexdigest()=='7e60412f16cbc3e9858e7c85c7d02ec9c16d15af94f6c37805190b59c5a39264')
  check('Public material catalog unchanged',z.read('Unity/MorphForge-public-catalog.zip')==(ROOT/'dist/avatar/MorphForge-public-catalog.zip').read_bytes())
  meta=json.loads(z.read('PackageInfo.json'));check('Buyer metadata reflects free plus voluntary identical support',meta['priceJpy']==0 and meta['optionalSupportJpy']==100 and meta['priceConfirmed'] and meta['sameContentForAllVariants'] and meta['sourceRepository']==url)
  check('Three guides and original third-party terms retained',all(n in z.namelist() for n in ['Documentation/Quickstart.zh-CN.md','Documentation/Quickstart.ja.md','Documentation/Quickstart.en.md','Licenses/CC-BY-NC-SA-4.0.txt','Licenses/Rust-dependencies.zip','Licenses/NotoSansSC-OFL.txt','Licenses/References-ATTRIBUTION.md']))
  check('Confirmed prices replace stale buyer price-pending text',all('価格、問い合わせ先は公開者の確認待ち' not in z.read(n).decode() for n in z.namelist() if n.endswith(('.txt','.md'))))
  check('Buyer ZIP contains no private project or publisher account HTML',not any(re.search(r'\.vrca$|\.unity$|\.env$|credentials|provider-connection|booth-edit|\.html$',n,re.I) for n in z.namelist()))
 images=json.loads((base/'Checks/image-sources.json').read_text())['images'];check('Twelve product images have correct regional fonts and matching origins',len(images)==12 and all(r['fontFamily']==('Noto Sans CJK JP' if r['language']=='ja' else 'Noto Sans CJK SC') and sha(base/('BOOTH' if r['language']=='ja' else 'Xianyu')/'Images'/r['file'])==r['sha256'] for r in images))
 from PIL import Image
 check('All product images decode and fit planned dimensions',all(Image.open(base/('BOOTH' if r['language']=='ja' else 'Xianyu')/'Images'/r['file']).size==tuple(r['size']) and r['size'][0]==1600 and r['size'][1] in [1600,2000] for r in images))
 for code,platform in [('ja','BOOTH'),('zh-CN','Xianyu')]:check(platform+' full video byte-identical to reviewed master',sha(base/platform/'Video'/f'MorphForge-Promo-{code}-1080p60.mp4')==sha(ROOT/'Releases/MorphForge-Promo-20261025-CN-JP'/f'MorphForge-Promo-{code}-1080p60.mp4'))
 light=base/'Xianyu/Video/MorphForge-Promo-zh-CN-1080p30-light.mp4';probe=json.loads(subprocess.check_output(['ffprobe','-v','error','-show_streams','-show_format','-of','json',str(light)]));v=next(s for s in probe['streams'] if s['codec_type']=='video');audio=next(s for s in probe['streams'] if s['codec_type']=='audio')
 check('Light video retains full 111.2 seconds at 1080p30 H.264',v['codec_name']=='h264' and (v['width'],v['height'])==(1920,1080) and Fraction(v['avg_frame_rate'])==30 and int(v['nb_frames'])==3336 and abs(float(probe['format']['duration'])-111.2)<.03)
 check('Light video has compatible stereo AAC and no subtitle track',len(probe['streams'])==2 and audio['codec_name']=='aac' and audio['channels']==2 and audio['sample_rate']=='48000')
 decoded=subprocess.run(['ffmpeg','-v','error','-xerror','-threads','4','-i',str(light),'-f','null','-'],capture_output=True,text=True);(a.work/'light-decode.log').write_text(decoded.stderr);check('New light video completely decodes',decoded.returncode==0)
 loud=subprocess.run(['ffmpeg','-hide_banner','-nostats','-i',str(light),'-vn','-af','loudnorm=I=-16:TP=-1.5:LRA=11:print_format=json','-f','null','-'],capture_output=True,text=True,check=True);stats=json.loads(loud.stderr[loud.stderr.rfind('{'):loud.stderr.rfind('}')+1]);check('Light video audio loudness and peak acceptable',-19<=float(stats['input_i'])<=-14 and float(stats['input_tp'])<-.2,stats)
 page=(base/'index.html').read_text();links=Links();links.feed(page);check('Offline preview links and media resolve locally',all(not urlparse(u).scheme and (base/unquote(urlparse(u).path)).is_file() for u in links.links))
 check('Both video upload descriptions retain music attribution',all('Kevin MacLeod' in (base/p).read_text() and 'creativecommons.org/licenses/by/4.0/' in (base/p).read_text() for p in ['BOOTH/Video/YouTube-description.ja.txt','Xianyu/Video/Video-description.zh-CN.txt']))
 public=[p for p in base.rglob('*') if p.is_file() and p.suffix.lower() in ['.txt','.md','.json','.html']];bad=[]
 for p in public:
  text=p.read_text()
  if re.search(r'avtr_[0-9a-fA-F-]{36}|sk-[A-Za-z0-9]{24,}|AKID[A-Za-z0-9]{24,}|csrf-token|authenticity_token|<script[^>]+src=',text):bad.append(p.relative_to(base).as_posix())
 check('Public materials exclude session state, credentials and private avatar IDs',not bad,bad)
 check('Original saved BOOTH account page excluded',not any('booth-edit-private' in p.name or p.name=='Edit Item - BOOTH.html' for p in base.rglob('*')))
 report=dict(success=True,checks=checks,checkCount=len(checks),buyerSha256=sha(buyer),buyerBytes=buyer.stat().st_size,lightVideoBytes=light.stat().st_size,scope='Offline publication material and packaging validation. Not a platform publication or account/category acceptance claim.')
 (base/'Checks/verification.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');(ROOT/'docs/marketplace/20261025/verification.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print('Passed',len(checks),'checks')
if __name__=='__main__':main()
