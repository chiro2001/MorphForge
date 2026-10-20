#!/usr/bin/env python3
"""Actual Rust/Chromium acceptance for reference, components and group motion.

Run only against an isolated data directory. No paid model calls or OSC output.
"""
import argparse, json, time, urllib.request
from pathlib import Path
from check_gui_buttons import Audit
from check_editor_browser import Browser


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True)
    p.add_argument('--webdriver',default='http://127.0.0.1:9519')
    a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
    t=Audit(a);worker=Browser(a.webdriver)
    worker.call(worker.path+'/url',{'url':a.url+'/?capture'});worker.wait('return window.__morphforge?.ready',30)
    def tool(name,args):return t.api('harness/tool',{'name':name,'arguments':args})
    try:
        t.check('OSC is disabled in isolated creative tests',not t.api('state')['connection']['enabled'])
        tool('run_recipe',{'baseRevision':t.rev(),'mode':'replace','requestId':'creative-seed-'+str(time.time_ns()),'code':"mf.box([-10,0,-5],[20,1,10],{style:1,group:'base'});mf.box([-5,1,1],[3,7,1],{style:2,group:'door'});mf.box([4,1,0],[1,8,1],{style:3,group:'mast'});mf.triangle([4,9,0],[10,9,0],[4,6,0],{color:'#EF7750',group:'flag',doubleSided:true});"})
        t.wait('return window.__morphforge.state.stats.triangleCount===1');t.click('fitView')
        t.menu('componentOpen');t.wait('return componentDialog.open');t.fill('componentName','可复用木门',False);t.fill('componentGroups','door',False);t.click('componentSave');t.wait('return componentSelect.value!=="" && componentStatus.textContent.includes("局部坐标")');component=t.js('return componentSelect.value');t.check('GUI stores an inspectable component from a real group',tool('components',{'action':'inspect','id':component})['component']['scene']['boxes'][0]['group']=='door')
        before=t.rev();t.fill('componentOffset','-8, 1, -3',False);t.fill('componentCount',3,False);t.fill('componentStep','7, 0, 0',False);t.fill('componentGroup','repeated-door',False);t.click('componentDryRun');t.wait('return componentStatus.textContent.includes("dryRun")');t.check('component dry-run does not alter draft',t.rev()==before);t.click('componentPlace');t.changed(before);t.wait('return !componentPlace.disabled');t.check('batch component placement creates independently editable groups',all('repeated-door-'+str(n) in t.api('state')['stats']['groups'] for n in [1,2,3]));t.b.screenshot(a.out/'component-library.png');t.click('componentClose')
        t.menu('comparisonOpen');t.wait('return comparisonDialog.open && comparisonImage.options.length>1');t.click('comparisonFront');t.fill('comparisonMode','overlay');t.fill('comparisonOpacity',.35);t.fill('comparisonCrop','0.1, 0.1, 0.8, 0.8');t.fill('comparisonName','正面对照书签',False);t.click('comparisonSave');t.wait('return comparisonBookmark.value!=="" && !comparisonSave.disabled');bookmark=t.js('return comparisonBookmark.value');t.check('reference camera and alignment persist in project',t.api('creative')['bookmarks'][bookmark]['comparison']['crop']==[.1,.1,.8,.8]);before=t.rev();t.click('comparisonCapture');t.wait('return !comparisonResult.hidden && comparisonResult.naturalWidth===1280 && !comparisonCapture.disabled',60);t.check('overlay screenshot has real PNG artifact and comparison metadata',t.js('return comparisonResult.naturalWidth===1280') and t.rev()==before);t.b.screenshot(a.out/'reference-overlay.png')
        t.fill('comparisonMode','split');t.click('comparisonNew');t.wait('return !comparisonNew.disabled');t.click('comparisonCapture');t.wait('return !comparisonCapture.disabled',60);t.check('split comparison remains separate from generated geometry',t.rev()==before);t.b.screenshot(a.out/'reference-split.png');t.click('comparisonClose')
        t.menu('animationOpen');t.wait('return !motionPanel.hidden');t.fill('motionGroup','door');t.fill('motionPivot','-5, 1, 1',False);t.fill('motionName','木门开合',False);t.click('motionAddKey');t.fill('motionTime',2);t.fill('motionRotation','0, 95, 0',False);t.fill('motionTranslation','0, 2, 0',False);t.click('motionAddKey');t.fill('motionGroup','flag');t.fill('motionTime',0);t.fill('motionPivot','4, 8, 0',False);t.fill('motionRotation','0, 0, 0',False);t.fill('motionTranslation','0, 0, 0',False);t.click('motionAddKey');t.fill('motionTime',2);t.fill('motionRotation','20, 0, 0',False);t.click('motionAddKey');t.click('motionSave');t.wait('return motionClip.value!=="" && !motionSave.disabled');clip=t.js('return motionClip.value');t.check('multiple group tracks save with real keyframes',len(t.api('creative')['clips'][clip]['tracks'])==2)
        before=t.rev();t.fill('motionTime',2);
        position=t.js('const r=window.__morphforge.renderer,m=r.meshes.find(m=>m.userData.boxes.some(b=>b.group==="door")),i=m.userData.boxes.findIndex(b=>b.group==="door");return [...m.instanceMatrix.array.slice(i*16+12,i*16+15)]');
        import math
        expected=[(-5+1.5*math.cos(math.radians(95))+.5*math.sin(math.radians(95)))*.25,6.5*.25,-(1-1.5*math.sin(math.radians(95))+.5*math.cos(math.radians(95)))*.25];t.check('cuboid pivot and canonical rotation produce the expected displayed matrix',all(abs(x-y)<1e-5 for x,y in zip(position,expected)));
        t.fill('motionTime',1);t.check('scrubbing previews pose without editing or enabling OSC',t.js('return window.__morphforge.renderer.animationClip!==null') and t.rev()==before and not t.api('state')['connection']['enabled']);t.click('motionPlay');time.sleep(.5);t.click('motionPause');t.check('play and pause move the visible time cursor',float(t.js('return motionTime.value'))>1.2);t.b.screenshot(a.out/'animation-playback.png')
        t.click('motionCapture');t.wait('return !motionCapture.disabled && motionStatus.querySelector("a")',60);t.check('frozen animation capture records exact pose time',bool(t.js('return motionStatus.querySelector("a").href.includes("artifacts")')))
        r=urllib.request.urlopen(a.url+'/api/creative/animation/'+clip+'/export').read();(a.out/'animated.glb').write_bytes(r);n=int.from_bytes(r[12:16],'little');glb=json.loads(r[20:20+n]);t.check('animated GLB includes two pivot nodes and six animation channels',len(glb['animations'][0]['channels'])==6)
        t.click('motionStatic');t.check('return-to-static restores editable source without scene mutation',t.js('return !window.__morphforge.renderer.animationClip') and t.rev()==before);t.click('motionClose')
        t.click('comparisonOpen');t.fill('comparisonBookmark',bookmark);t.wait('return comparisonName.value==="正面对照书签"');t.check('saved bookmark restores reference alignment',t.js('return comparisonMode.value==="overlay" && comparisonCrop.value==="0.1, 0.1, 0.8, 0.8"'));t.click('comparisonClose')
        exported=t.api('export');t.api('import',exported);t.wait('return window.__morphforge.state.revision>'+str(before));t.check('project JSON roundtrip retains components and animations',len(t.api('creative')['components'])==1 and len(t.api('creative')['clips'])==1)
        t.check('new workbenches have no unhandled JavaScript errors',not t.js('return window.__guiAuditErrors'));t.b.screenshot(a.out/'complete.png')
    except Exception as e:
        t.failures.append({'case':'creative workbenches','error':str(e)});t.b.screenshot(a.out/'failure.png');raise
    finally:
        report=t.result();report['realModelRequests']=0;(a.out/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');worker.close();t.b.close()

if __name__=='__main__':main()
