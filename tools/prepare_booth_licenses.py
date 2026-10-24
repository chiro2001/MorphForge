#!/usr/bin/env python3
"""Collect exact Cargo dependency notices; fetch omitted root notices at recorded commits."""
import argparse, concurrent.futures, hashlib, json, re, tomllib, urllib.error, urllib.request, zipfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
PATTERN=re.compile(r'^(licen[cs]e|copying|notice|copyright|authors)([-._]|$)',re.I)

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--metadata',type=Path,action='append',required=True)
    p.add_argument('--out',type=Path,default=ROOT/'dist/licenses/MorphForge-0.27.0-third-party-notices.zip')
    a=p.parse_args(); packages={}; contents={}; cache={}
    if a.out.is_file():
        with zipfile.ZipFile(a.out)as old:
            for row in json.loads(old.read('Cargo-dependencies.json'))['packages']:
                if row.get('repository') and row.get('revision') and row.get('upstreamNotices'):
                    cache[(row['repository'].removesuffix('.git').rstrip('/'),row['revision'])]={Path(n['file']).name:{'url':n['url'],'data':old.read(row['name']+'-'+row['version']+'/'+n['file'])}for n in row['upstreamNotices']}
    for file in a.metadata:
        for item in json.loads(file.read_text())['packages']:
            if item['source']:packages[(item['name'],item['version'])]=item
    locked={}
    for lock in (ROOT/'Cargo.lock',ROOT/'desktop/Cargo.lock'):
        for row in tomllib.loads(lock.read_text())['package']:
            if row.get('checksum'):locked[(row['name'],row['version'])]=row['checksum']
    def prepare(item):
        root=Path(item['manifest_path']).parent
        files=[f for f in root.iterdir() if f.is_file() and (PATTERN.match(f.name) or f.name=='ABOUT.md')]
        for folder in ('licenses','LICENSES','license'):
            if (root/folder).is_dir():files.extend(f for f in (root/folder).rglob('*') if f.is_file())
        if item['license_file'] and (root/item['license_file']).is_file():files.append(root/item['license_file'])
        record={k:item[k] for k in ('name','version','license','repository','authors')}
        record['source']='https://crates.io/crates/'+item['name']+'/'+item['version']
        prefix=item['name']+'-'+item['version']
        data={prefix+'/'+f.relative_to(root).as_posix():f.read_bytes() for f in set(files)}
        vcsfile=root/'.cargo_vcs_info.json'
        vcs=json.loads(vcsfile.read_text()) if vcsfile.exists() else {}
        record['revision']=vcs.get('git',{}).get('sha1')
        record['noticeOrigin']='crate archive'
        if not any(PATTERN.match(f.name) for f in files):
            repo=(item['repository'] or '').removesuffix('.git').rstrip('/')
            if item['name']=='libappindicator-sys':repo='https://github.com/tauri-apps/libappindicator-rs'
            rev=record['revision']
            if repo.startswith('https://github.com/') and rev:
                key=(repo,rev)
                if key not in cache:
                    # Concurrent duplicate retrievals are harmless; all URLs pin the same revision.
                    parents=Path(vcs.get('path_in_vcs','')).parents
                    dirs=['']+[str(d)+'/' for d in parents if str(d)!='.']
                    found={}
                    for directory in dict.fromkeys(dirs):
                        for name in ('LICENSE-MIT','LICENSE','LICENSE.txt','LICENSE.md','LICENSE-MPL','COPYING','LICENSE-APACHE','LICENSE-APACHE2','license'):
                            url='https://raw.githubusercontent.com/'+repo.split('github.com/',1)[1]+'/'+rev+'/'+directory+name
                            try:
                                with urllib.request.urlopen(url,timeout=12) as r:raw=r.read()
                                if len(raw)>100 and b'<html' not in raw[:100].lower():found[name]={'url':url,'data':raw}
                            except (urllib.error.URLError,TimeoutError):continue
                        if found:break
                    cache[key]=found
                for name,value in cache[key].items():data[prefix+'/upstream/'+name]=value['data']
                record['upstreamNotices']=[{'file':'upstream/'+name,'url':value['url'],'sha256':hashlib.sha256(value['data']).hexdigest()} for name,value in cache[key].items()]
                if cache[key]:record['noticeOrigin']='exact upstream revision'
        # Some crates publish their notice only in the source header. Preserve
        # that exact header and a matching complete license already supplied by
        # another locked dependency; never invent a copyright holder.
        if not any(PATTERN.match(Path(k).name)for k in data):
            declared=item['license']or''
            terms=None
            if 'Apache-2.0' in declared:terms=ROOT/'licenses/Sherpa-ONNX-Apache-2.0.txt'
            elif declared=='MPL-2.0':
                terms=next((f for d in sorted(root.parent.glob('cssparser-*'))for f in sorted(d.iterdir())if f.is_file()and PATTERN.match(f.name)),None)
            source=None
            for candidate in (root/'src/lib.rs',root/'lib.rs'):
                if candidate.is_file():
                    header='\n'.join(candidate.read_text().splitlines()[:30])
                    if any(word in header.lower()for word in ('license','copyright')):source=(candidate,header);break
            if terms and not source:source=(root/'Cargo.toml',(root/'Cargo.toml').read_text())
            if source and terms:
                data[prefix+'/LICENSE-DECLARATION.txt']=(source[1]+'\n').encode()
                data[prefix+'/LICENSE-TERMS.txt']=terms.read_bytes()
                record['noticeOrigin']='original crate source header plus complete declared license text'
                record['sourceNoticeFile']=source[0].relative_to(root).as_posix()
                record['termsSource']=terms.relative_to(ROOT).as_posix()if terms.is_relative_to(ROOT)else terms.parent.name+'/'+terms.name
                record['termsSha256']=hashlib.sha256(terms.read_bytes()).hexdigest()
                record['standaloneUpstreamNoticeNotLocated']=True
        record['noticeFiles']=sorted(k.split('/',1)[1] for k in data)
        record['noticeFilePresent']=any(PATTERN.match(Path(k).name)for k in data)
        if 'MPL' in (item['license']or'') or not record['noticeFilePresent'] or record.get('standaloneUpstreamNoticeNotLocated'):
            archive=root.parents[2]/'cache'/root.parent.name/(root.name+'.crate')
            assert archive.is_file(),archive
            name='SourceArchives/'+archive.name;data[name]=archive.read_bytes()
            record['sourceArchive']=name;record['sourceArchiveSha256']=hashlib.sha256(data[name]).hexdigest()
            assert record['sourceArchiveSha256']==locked[(item['name'],item['version'])]
            record['sourceArchiveMatchesRegistryChecksum']=True
        # Cargo's original metadata preserves exact author declarations without local paths.
        data[prefix+'/Cargo.toml']= (root/'Cargo.toml').read_bytes()
        return record,data
    records=[]
    with concurrent.futures.ThreadPoolExecutor(max_workers=12)as pool:
        for record,data in pool.map(prepare,packages.values()):records.append(record);contents.update(data)
    report={'scope':'Complete locked Cargo dependency graphs for core and desktop, including build/test/non-Windows dependencies; not a claim that every dependency is in the Windows binary. Original license expressions are preserved. No dependency is relicensed.','packages':sorted(records,key=lambda x:(x['name'],x['version']))}
    report['missingNoticeFiles']=[x['name']+'-'+x['version']for x in records if not x['noticeFilePresent']]
    contents['Cargo-dependencies.json']=(json.dumps(report,ensure_ascii=False,indent=2)+'\n').encode()
    contents['README.txt']=b'Original third-party license notices. Cargo metadata includes all locked platforms and development dependencies. Sources of MPL dependencies are supplied unmodified in SourceArchives/. Consult Cargo-dependencies.json for exact versions and origins. This archive does not change any original license.\n'
    a.out.parent.mkdir(parents=True,exist_ok=True)
    with zipfile.ZipFile(a.out,'w',zipfile.ZIP_DEFLATED,compresslevel=9)as z:
        for name,data in sorted(contents.items()):
            info=zipfile.ZipInfo(name,date_time=(2026,10,25,0,0,0));info.compress_type=zipfile.ZIP_DEFLATED;z.writestr(info,data)
    result={'success':not report['missingNoticeFiles'],'packages':len(records),'missingNoticeFiles':report['missingNoticeFiles'],'sha256':hashlib.sha256(a.out.read_bytes()).hexdigest(),'bytes':a.out.stat().st_size}
    sidecar=a.out.with_suffix('.json');sidecar.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print(json.dumps(result))

if __name__=='__main__':main()
