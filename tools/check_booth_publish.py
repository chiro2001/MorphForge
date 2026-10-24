#!/usr/bin/env python3
"""Verify the delivered BOOTH files, nested originals, notices, and offline links."""
import argparse, hashlib, json, re, zipfile
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlparse

ROOT=Path(__file__).resolve().parents[1]

def digest(data):return hashlib.sha256(data).hexdigest()

class Links(HTMLParser):
    def __init__(self):super().__init__();self.links=[]
    def handle_starttag(self,tag,attrs):
        self.links.extend(v for k,v in attrs if k in ('href','src'))

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--directory',type=Path,default=ROOT/'Releases/MorphForge-0.27.0/BOOTH-Publish')
    p.add_argument('--output',type=Path,default=ROOT/'docs/releases/0.27.0/verification.json')
    p.add_argument('--seal',action='store_true',help='Include the validation record and regenerate the publisher manifest/kit.')
    a=p.parse_args();base=a.directory.resolve();checks=[]
    def check(name,value):
        checks.append({'name':name,'passed':bool(value)})
        if not value:raise AssertionError(name)
    report=json.loads((base/'publish-preparation.json').read_text())
    config=json.loads((base/'booth-publish-config.json').read_text())
    check('preparation is explicitly unpublished and review pending',report['status']=='PREPARED_FOR_REVIEW'and not report['published']and not config['published'])
    check('price and product terms remain publisher decisions',not config['priceConfirmed']and not config['licenseFinalized'])
    files=json.loads((base/'publisher-files-sha256.json').read_text())
    check('publisher manifest covers exactly the delivery files',set(files)=={f.relative_to(base).as_posix()for f in base.rglob('*')if f.is_file()and f.name!='publisher-files-sha256.json'})
    check('every publisher file matches SHA256',all(digest((base/name).read_bytes())==sha for name,sha in files.items()))
    download=base/report['download']
    check('buyer ZIP matches reported hash and size',digest(download.read_bytes())==report['zipSha256']and download.stat().st_size==report['zipBytes'])
    with zipfile.ZipFile(download)as z:
        names=z.namelist();check('ZIP CRC is valid',z.testzip()is None)
        check('all archive paths are safe and unique',len(names)==len(set(names))and all(not n.startswith('/')and '..'not in Path(n).parts and '\\'not in n for n in names))
        checksums=dict((line.split('  ',1)[1],line.split('  ',1)[0])for line in z.read('SHA256SUMS.txt').decode().splitlines())
        check('buyer checksums cover every payload file',set(checksums)==set(names)-{'SHA256SUMS.txt'})
        check('all buyer payload files match checksums',all(digest(z.read(name))==sha for name,sha in checksums.items()))
        check('validated desktop installer is unchanged',digest(z.read('Windows/MorphForge Desktop_0.27.0_x64-setup.exe'))=='7d763ea397bf853c920f0005878e521c2b8f90607672f0e629dac511234e9979')
        check('validated public Unity package is unchanged',digest(z.read('Unity/MorphForgeAvatar-1.4.0.unitypackage'))=='7e60412f16cbc3e9858e7c85c7d02ec9c16d15af94f6c37805190b59c5a39264')
        check('public catalog is the matching original file',z.read('Unity/MorphForge-public-catalog.zip')==(ROOT/'dist/avatar/MorphForge-public-catalog.zip').read_bytes())
        check('buyer ZIP excludes private and publisher-only files',not any(any(part in n.lower()for part in ('nemesis','minecraftsource','provider-connection','speech-credentials','checks/','productpage/','agent scripts','agentscripts/','.env'))or n.endswith(('.unity','.vrca','.log','.pdb'))for n in names))
        for lang in ('ja','zh-CN','en'):
            text=z.read(f'Documentation/Quickstart.{lang}.md').decode()
            check(lang+' guide has current versions and synchronization limit',all(s in text for s in ('0.27.0','1.4.0','253','ACK','128','CC BY-NC-SA','4.0')))
            check(lang+' product page and guide exist',(base/f'ProductPage/Description.{lang}.txt').is_file()and len(text)>2500)
        check('separate font and noncommercial reference licenses are preserved',all(n in names for n in ('Licenses/CC-BY-NC-SA-4.0.txt','Licenses/Unifont-OFL.txt','Licenses/NotoSansSC-OFL.txt','Licenses/References-ATTRIBUTION.md','Licenses/Examples-ATTRIBUTION.md')))
        check('three.js original MIT notice is included','Licenses/WebVendor/LICENSE.txt'in names)
        textfiles=[n for n in names if n.endswith(('.txt','.md','.json'))]
        bad=[]
        for n in textfiles:
            text=z.read(n).decode()
            if re.search(r'avtr_[0-9a-fA-F-]{36}|sk-[A-Za-z0-9]{24,}|AKID[A-Za-z0-9]{24,}',text):bad.append(n)
        check('public text has no avatar binding or credential-shaped values',not bad)
        from io import BytesIO
        with zipfile.ZipFile(BytesIO(z.read('Licenses/Rust-dependencies.zip')))as deps:
            check('dependency notice archive CRC is valid',deps.testzip()is None)
            meta=json.loads(deps.read('Cargo-dependencies.json'))
            check('all locked registry packages have recorded terms',len(meta['packages'])==623 and not meta['missingNoticeFiles'])
            check('dependency index contains no workstation paths',not re.search(r'/home/|[A-Z]:\\|manifest_path',json.dumps(meta)))
            mpl=[r for r in meta['packages']if 'MPL' in r['license']]
            check('unmodified source archives supplied for all MPL packages',len(mpl)==5 and all(digest(deps.read(r['sourceArchive']))==r['sourceArchiveSha256']for r in mpl))
            check('supplied source archives match locked registry checksums',all(r['sourceArchiveMatchesRegistryChecksum']for r in meta['packages']if r.get('sourceArchive')))
    audit=json.loads((base/'Checks/package-audit.json').read_text())
    check('public plugin source equality and private binding audit passed',audit['success']and audit['sourceMatches']==162 and not audit['privateAvatarBinding']and not audit['includesUserVariants'])
    check('clean-import status is disclosed accurately',report['cleanImport']==json.loads((base/'Checks/clean-import.json').read_text())['success'])
    page=(base/'index.html').read_text();links=Links();links.feed(page)
    check('review page uses local files only',all(not urlparse(link).scheme and not link.startswith('//')for link in links.links))
    check('every offline review link resolves',all((base/unquote(urlparse(link).path)).is_file()for link in links.links))
    source=json.loads((base/'ProductPage/image-sources.json').read_text())
    check('seven image origins and hashes are recorded',len(source['images'])==7)
    from PIL import Image
    for row in source['images']:
        folder='AlternativeCovers/'if row['file']in ('01-cover-en.png','01-cover-zh-CN.png')else''
        path=base/'ProductPage/Images'/folder/row['file']
        check(row['file']+' is valid and matches capture manifest',path.is_file()and digest(path.read_bytes())==row['sha256']and Image.open(path).size==(1600,1000))
    kit=base.parent/'MorphForge-0.27.0-PublisherKit.zip'
    with zipfile.ZipFile(kit)as z:
        check('complete publisher kit CRC is valid',z.testzip()is None)
        check('complete publisher kit contains identical files',set(z.namelist())=={f.relative_to(base).as_posix()for f in base.rglob('*')if f.is_file()}and all(digest(z.read(n))==digest((base/n).read_bytes())for n in z.namelist()))
    result={'success':True,'checks':checks,'checkCount':len(checks),'buyerZipSha256':report['zipSha256'],'pending':report['pending'],'scope':'Packaging integrity and public-source/notice checks. Does not upgrade a blocked clean-import or replace VRChat multiplayer acceptance.'}
    if a.seal:
        from build_booth_publish import archive, jsave, write
        jsave(base/'Checks/packaging-verification.json',result)
        jsave(base/'publisher-files-sha256.json',{f.relative_to(base).as_posix():digest(f.read_bytes())for f in sorted(base.rglob('*'))if f.is_file()and f.name!='publisher-files-sha256.json'})
        archive(base,kit)
        write(kit.with_suffix('.zip.sha256'),digest(kit.read_bytes())+'  '+kit.name)
        final=json.loads((base/'publisher-files-sha256.json').read_text())
        assert all(digest((base/n).read_bytes())==s for n,s in final.items())
    result['publisherKitSha256']=digest(kit.read_bytes())
    a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'success':True,'checks':len(checks),'buyerZipMiB':round(download.stat().st_size/1048576,2),'publisherKitMiB':round(kit.stat().st_size/1048576,2)}))

if __name__=='__main__':main()
