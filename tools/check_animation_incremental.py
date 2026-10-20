#!/usr/bin/env python3
"""Pixel comparison: incremental animation capture must match a fresh renderer."""
import argparse,hashlib,json,time,urllib.request
from pathlib import Path
from check_editor_browser import Browser

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--webdriver',default='http://127.0.0.1:9519');p.add_argument('--expect-stale',action='store_true');a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True);worker=None;report={'success':False,'expectedStale':a.expect_stale}
    def api(path,args=None):
        q=urllib.request.Request(a.url+'/api/'+path,data=None if args is None else json.dumps(args).encode(),headers={'Content-Type':'application/json'})
        with urllib.request.urlopen(q,timeout=90) as r:return json.load(r)
    def tool(name,args):return api('harness/tool',{'name':name,'arguments':args})
    def start():
        b=Browser(a.webdriver);b.call(b.path+'/url',{'url':a.url+'/?capture'});b.wait('return window.__morphforge?.ready',30);return b
    def capture(args,name):
        result=tool('capture_views',args);data=urllib.request.urlopen(a.url+result['images'][0]['url']).read();(a.out/name).write_bytes(data);return hashlib.sha256(data).hexdigest()
    try:
        for _ in range(300):
            try:api('state');break
            except Exception:time.sleep(.1)
        assert not api('state')['connection']['enabled'];worker=start()
        tool('run_recipe',{'mode':'replace','baseRevision':api('state')['revision'],'code':"mf.box([-4,0,-4],[8,1,8],{style:2,group:'base'});mf.box([0,1,3],[2,6,1],{style:2,group:'door'});mf.box([5,0,0],[1,1,1],{style:2,group:'marker'});"})
        tool('animation',{'action':'save','id':'incremental-door','baseCreativeRevision':api('creative')['revision'],'clip':{'name':'GPU partial update regression','duration':2,'tracks':[{'group':'door','pivot':[0,1,3],'interpolation':'linear','keys':[{'time':0,'translation':[0,0,0],'rotation':[0,0,0],'scale':[1,1,1]},{'time':2,'translation':[0,0,0],'rotation':[0,180,0],'scale':[1,1,1]}]}]}})
        options={'width':640,'height':480,'shots':[{'id':'fixed','position':[20,15,30],'target':[0,3,0],'fov':42}]};capture(options,'static.png')
        tool('run_recipe',{'baseRevision':api('state')['revision'],'code':"mf.removeGroup('marker');mf.box([6,0,0],[1,1,1],{style:2,group:'marker'});"})
        options['animation']={'clipId':'incremental-door','time':1};first=capture(options,'incremental.png');worker.close();worker=start();fresh=capture(options,'fresh.png');same=first==fresh
        tool('run_recipe',{'baseRevision':api('state')['revision'],'code':"mf.removeGroup('marker');mf.box([7,0,0],[1,1,1],{style:2,group:'marker'});"});options.pop('animation');static_first=capture(options,'static-incremental.png');worker.close();worker=start();static_fresh=capture(options,'static-fresh.png');static_same=static_first==static_fresh
        report.update(success=((not same and not static_same) if a.expect_stale else (same and static_same)),incrementalSha256=first,freshSha256=fresh,pixelsMatch=same,staticAfterPosePixelsMatch=static_same,staticIncrementalSha256=static_first,staticFreshSha256=static_fresh,realModelRequests=0,scope='Actual Rust snapshot/partial instance update, same material batch, frozen pose; real Chromium PNG pixels compared to a fresh worker')
        assert report['success'],report
        print(json.dumps(report))
    finally:
        if worker:worker.close()
        (a.out/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
if __name__=='__main__':main()
