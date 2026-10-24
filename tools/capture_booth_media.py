#!/usr/bin/env python3
"""Capture public examples from a disposable 0.27.0 workbench; never send OSC."""
import argparse, base64, hashlib, json, socket, subprocess, time, urllib.request
from pathlib import Path
from check_editor_browser import Browser

ROOT = Path(__file__).resolve().parents[1]

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--url', default='http://127.0.0.1:18797')
    p.add_argument('--out', type=Path, default=ROOT/'docs/releases/0.27.0/media')
    a = p.parse_args()
    def api(route, body=None):
        request = urllib.request.Request(a.url+'/api/'+route, data=None if body is None else json.dumps(body).encode(), headers={'Content-Type':'application/json'})
        with urllib.request.urlopen(request, timeout=40) as r: return json.load(r)
    state = api('state')
    assert state['software']['version'] == '0.27.0' and not state['connection']['enabled']
    # The server reports its data root in diagnostics. Require an operator-created
    # marker in the disposable root, rather than trusting a localhost port alone.
    marker = Path('/home/chiro/tmp/morphforge/booth-20261025/capture-project.json')
    assert json.loads(marker.read_text())['projectId'] == state['project']['id']
    a.out.mkdir(parents=True, exist_ok=True)
    sock=socket.socket(); sock.bind(('127.0.0.1',0)); port=sock.getsockname()[1]; sock.close()
    driver=subprocess.Popen(['chromedriver',f'--port={port}','--allowed-ips=127.0.0.1'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    b=None; records=[]
    try:
        for _ in range(80):
            try: b=Browser(f'http://127.0.0.1:{port}'); break
            except (OSError,RuntimeError): time.sleep(.1)
        assert b
        b.call(b.path+'/window/rect', {'width':1600,'height':1100})
        for example in ('leviathan-lantern','kinetic-windmill'):
            api('artworks', {'action':'fork','id':'builtin:'+example,'baseRevision':api('state')['revision']})
            b.call(b.path+'/url', {'url':a.url}); b.wait('return !!window.__morphforge?.ready',40)
            frame=b.js("return (async()=>{return (await window.__morphforge.renderer.capture(['hero'],1600,1200,null,{supersampling:1,voxelGrid:false,surfaceCulling:true}))[0]})()")
            path=a.out/(example+'-editor.png'); path.write_bytes(base64.b64decode(frame['data'].split(',',1)[1]))
            records.append({'file':path.name,'source':'builtin:'+example,'kind':'actual Desktop editor render, not VRChat','license':'MIT','camera':frame.get('camera'),'rendering':frame.get('rendering')})
            if example=='leviathan-lantern':
                for lang in ('ja','zh-CN','en'):
                    b.js("return (async()=>{await(await import('/i18n.js')).setLanguage(arguments[0])})()",lang)
                    b.wait('return document.documentElement.lang==='+json.dumps(lang))
                    time.sleep(.4); path=a.out/('workbench-'+lang+'.png'); b.screenshot(path)
                    records.append({'file':path.name,'source':'builtin:'+example,'kind':'actual Desktop workbench screenshot','language':lang,'license':'MIT'})
        b.click('drawingOpen');b.wait('return drawingDialog.open');b.click('drawExample');b.wait('return !drawApply.disabled',30)
        b.screenshot(a.out/'drawing-controls-en.png')
        records.append({'file':'drawing-controls-en.png','source':'built-in drawing demo','kind':'actual Desktop drawing dialog','language':'en','license':'MIT'})
        b.js('drawingDialog.close()')
        final=api('state'); assert not final['connection']['enabled'] and not final['speechStatus']['active'] and final['harness']['interaction']['phase']=='idle'
        for item in records: item['sha256']=hashlib.sha256((a.out/item['file']).read_bytes()).hexdigest()
        (a.out/'sources.json').write_text(json.dumps({'version':'0.27.0','noOscOrModelCalls':True,'images':records},ensure_ascii=False,indent=2)+'\n')
        print(json.dumps({'success':True,'images':len(records)}))
    finally:
        if b:b.close()
        driver.terminate();driver.wait(timeout=10)

if __name__=='__main__': main()
