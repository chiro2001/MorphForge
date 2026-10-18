#!/usr/bin/env python3
"""Browser UI -> Windows HTTP engine -> real paced UDP OSC -> native Avatar particles."""
import json,time
from pathlib import Path
from check_editor_browser import Browser
from check_stage2 import pipeline
ROOT=Path(__file__).resolve().parents[1]
def capture():
    r=pipeline('run_script','--file','AgentScripts/PlacementEntry.cs','--entry','PlacementEntry.Capture','--timeout_ms','60000')
    if not r.get('success'):raise RuntimeError(r)
    return json.loads(r['result'])
def main():
    b=Browser();out=ROOT/'docs/editor';out.mkdir(parents=True,exist_ok=True);checks=[];captures=[]
    def check(n,ok):
        checks.append({'name':n,'pass':bool(ok)});print(('PASS ' if ok else 'FAIL ')+n,flush=True)
        if not ok:raise AssertionError(n)
    def change(id,value):b.js("const e=document.getElementById(arguments[0]);e.value=arguments[1];e.dispatchEvent(new Event('change',{bubbles:true}));",id,value)
    def settle():
        b.wait('const s=window.__morphforge?.state; return s?.connection.enabled && s.connection.sending && !s.connection.pending && s.connection.sentVersion===s.version',20);time.sleep(.4)
    def snap(label):
        r=capture();r['label']=label;captures.append(r);return r
    try:
        b.call(b.path+'/url',{'url':'http://127.0.0.1:18765'});b.wait('return window.__morphforge?.ready')
        change('scale','1');b.wait('return window.__morphforge.state.placement.scale===1')
        b.click('clearScene');b.wait('return window.__morphforge.state.voxelCount===0')
        b.js("document.querySelector('details').open=true; for(const [id,v] of Object.entries({brushX:4,brushY:3,brushZ:2,coordX:0,coordY:0,coordZ:0,styleId:2}))document.getElementById(id).value=v")
        b.click('applyPosition');b.wait('return window.__morphforge.state.voxelCount===24')
        change('target','9012');b.wait('return window.__morphforge.state.connection.port===9012');b.click('connect');settle()
        r=snap('full scale');check('Web command creates one native 4x3x2 box',r['count']==1 and max(abs(a-z) for a,z in zip(r['particles'][0]['size'],[1,.75,.5]))<.002)
        a=r['anchor'];p=r['particles'][0]['worldPosition'];check('Wire Y=0 rests at bottom-center handle',max(abs(p[i]-a[i]-[.5,.375,.25][i]) for i in range(3))<.003)
        baseBounds=r['renderedMeshSize'];change('scale','0.25');b.wait('return window.__morphforge.state.placement.scale===0.25');settle();r=snap('quarter scale')
        check('Web scale OSC controls native parameter and Transform',abs(r['scaleParameter']-.25)<.002 and abs(r['contentWorldScale'][0]-.25)<.002)
        p=r['particles'][0]['worldPosition'];a=r['anchor'];check('Particle center scales around bottom handle',max(abs(p[i]-a[i]-[.125,.09375,.0625][i]) for i in range(3))<.004)
        check('Native baked rendering geometry scales by one quarter',max(abs(v-base*.25) for v,base in zip(r['renderedMeshSize'],baseBounds))<.005)
        b.click('world');settle();r=snap('avatar follow');check('Web world switch reaches native constraint',not r['world'] and not r['frozen'])
        b.click('world');b.click('visible');settle();r=snap('hidden handle');check('Web bool OSC hides only handle',not r['handle'] and r['count']==1)
        b.click('visible');change('scale','1');b.click('resetAnchor');settle()
        b.click('eraseTool');b.js("for(const [id,v] of Object.entries({brushX:1,brushY:1,brushZ:1,coordX:1,coordY:1,coordZ:0}))document.getElementById(id).value=v")
        b.click('applyPosition');b.wait('return window.__morphforge.state.voxelCount===23');settle();r=snap('hole erase')
        check('Arbitrary one-voxel erase compiles to native disjoint boxes',r['count']==b.js('return window.__morphforge.state.boxCount') and r['count']>1)
        check('All box volumes total 23 voxels',abs(sum(p['size'][0]*p['size'][1]*p['size'][2]/(.25**3) for p in r['particles'])-23)<.01)
        b.click('stairsDemo');b.wait('return window.__morphforge.state.voxelCount===144');settle();r=snap('stairs');check('Web stairs reaches eight native particles',r['count']==8)
        b.screenshot(out/'unity-linked-editor.png');report={'success':True,'scope':'Actual Windows browser service OSC 9012 to Gesture Manager/native constraints/particles; no VRChat client','checks':checks,'captures':captures}
    except Exception as e:
        report={'success':False,'error':str(e),'checks':checks,'captures':captures};raise
    finally:
        try:
            # Stop outgoing OSC and leave the normal VRChat target selected offline.
            b.js("return fetch('/api/connect',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({enabled:false,port:9000})}).then(r=>r.json())")
        finally:(out/'unity-integration.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));b.close()
if __name__=='__main__':main()
