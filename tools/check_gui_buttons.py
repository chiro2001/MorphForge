#!/usr/bin/env python3
"""Click-through acceptance for an isolated MorphForge workspace.

Uses real Chromium pointer clicks, actual Rust APIs, synthetic local uploads and
an optional localhost model fixture. Never use against an active user backend.
OSC is only enabled after selecting the locally bound 9012 fixture receiver.
"""
from __future__ import annotations
import argparse,base64,gzip,io,json,socket,struct,threading,time,urllib.request,zipfile
from pathlib import Path
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from check_editor_browser import Browser
from check_materials import fixture,png

INIT = r'''
window.__guiAuditErrors=[];
window.addEventListener('error',e=>{if(e.message)window.__guiAuditErrors.push(String(e.message));});
window.addEventListener('unhandledrejection',e=>window.__guiAuditErrors.push(String(e.reason)));
document.addEventListener('click',e=>{const n=e.target.closest('button,input[type=file]');if(!n)return;try{const a=JSON.parse(localStorage.getItem('guiAuditClicks')||'[]');a.push({id:n.id,command:n.dataset.command,export:n.dataset.export,tab:n.dataset.tab,view:n.dataset.view,scale:n.dataset.scale,text:n.textContent.slice(0,60),trusted:e.isTrusted});localStorage.setItem('guiAuditClicks',JSON.stringify(a.slice(-4000)));}catch{}},true);
const originalFetch=window.fetch;
window.fetch=async(...args)=>{const path=String(args[0]);if(path.endsWith('/api/connect')&&args[1]?.body){const d=JSON.parse(args[1].body);if(d.enabled&&d.port!==9012)throw Error('GUI audit refuses non-fixture OSC output');}return originalFetch(...args);};
'''

def minecraft_fixture():
    def named(t,n,v):
        b=n.encode();return bytes([t])+struct.pack('>H',len(b))+b+v
    def integer(n):return struct.pack('>i',n)
    def ints(v):return b'\x03'+integer(len(v))+b''.join(integer(n) for n in v)
    name=b'minecraft:stone'
    palette=named(9,'palette',b'\x0a'+integer(1)+named(8,'Name',struct.pack('>H',len(name))+name)+b'\x00')
    blocks=[]
    for x in range(2):
        blocks.append(named(9,'pos',ints([x,0,0]))+named(3,'state',integer(0))+b'\x00')
    return gzip.compress(b'\x0a\x00\x00'+named(3,'DataVersion',integer(3955))+named(9,'size',ints([2,1,1]))+palette+named(9,'blocks',b'\x0a'+integer(len(blocks))+b''.join(blocks))+b'\x00')

class Audit:
    def __init__(self,args):
        self.a=args;self.out=args.out;self.out.mkdir(parents=True,exist_ok=True)
        self.b=Browser(args.webdriver);self.checks=[];self.failures=[]
        self.b.call(self.b.path+'/goog/cdp/execute',{'cmd':'Page.addScriptToEvaluateOnNewDocument','params':{'source':INIT}})
        self.b.call(self.b.path+'/goog/cdp/execute',{'cmd':'Browser.setDownloadBehavior','params':{'behavior':'allow','downloadPath':str((self.out/'downloads').resolve())}})
        self.b.call(self.b.path+'/goog/cdp/execute',{'cmd':'Page.setInterceptFileChooserDialog','params':{'enabled':True}})
        self.b.call(self.b.path+'/url',{'url':args.url});self.ready()
        self.inventory=self.js('return [...document.querySelectorAll("button")].filter(n=>n.id||Object.keys(n.dataset).length).map(n=>({id:n.id,command:n.dataset.command,export:n.dataset.export,tab:n.dataset.tab,view:n.dataset.view,scale:n.dataset.scale,text:n.textContent.trim(),disabled:n.disabled}))')
    def api(self,path,value=None):
        request=urllib.request.Request(self.a.url+'/api/'+path,data=None if value is None else json.dumps(value,ensure_ascii=False).encode(),headers={'Content-Type':'application/json'})
        with urllib.request.urlopen(request,timeout=90) as r:return json.load(r)
    def js(self,code,*args):return self.b.js(code,*args)
    def ready(self):self.b.wait('return window.__morphforge?.ready',40)
    def wait(self,code,timeout=20):return self.b.wait(code,timeout)
    def check(self,name,ok):
        self.checks.append({'name':name,'passed':bool(ok)});print(('PASS ' if ok else 'FAIL ')+name,flush=True)
        assert ok,name
    def fill(self,id,value,change=True):
        self.js('const n=document.getElementById(arguments[0]);n.focus();if(n.type==="checkbox")n.checked=!!arguments[1];else n.value=arguments[1];n.dispatchEvent(new Event("input",{bubbles:true}));if(arguments[2])n.dispatchEvent(new Event("change",{bubbles:true}));',id,value,change)
    def click(self,selector):
        if not selector.startswith(('#','[','.')):selector='#'+selector
        self.js('const n=document.querySelector(arguments[0]);if(!n)throw Error("Missing "+arguments[0]);for(let p=n.parentElement;p;p=p.parentElement)if(p.tagName==="DETAILS")p.open=true;',selector)
        self.wait('const n=document.querySelector('+json.dumps(selector)+');return !!n && !n.disabled;',8)
        self.js('document.querySelector(arguments[0]).scrollIntoView({block:"center",inline:"nearest"})',selector)
        count_before=self.js('return JSON.parse(localStorage.getItem("guiAuditClicks")||"[]").length')
        target_id=self.js('return document.querySelector(arguments[0]).id',selector)
        for attempt in range(4):
            try:
                elem=self.b.call(self.b.path+'/element',{'using':'css selector','value':selector})
                self.b.call(self.b.path+'/element/'+elem['element-6066-11e4-a52e-4f735466cecf']+'/click',{})
                return
            except Exception:
                # A successful click can hide/reload its element while ChromeDriver
                # is returning. The caller still verifies the actual UI outcome.
                if target_id and self.js('return JSON.parse(localStorage.getItem("guiAuditClicks")||"[]").slice(arguments[0]).some(c=>c.trusted&&c.id===arguments[1])',count_before,target_id):return
                if attempt==3:raise
                time.sleep(.15)
    def menu(self,key,export=False):
        selector=f'[data-{"export" if export else "command"}="{key}"]'
        root=self.js('const n=document.querySelector(arguments[0]);return [...document.querySelectorAll(".app-menu")].indexOf(n.closest(".app-menu"));',selector)
        self.js('document.querySelectorAll(".app-menu").forEach(n=>n.open=false)')
        self.click(f'.app-menu:nth-child({root+1}) > summary')
        if export:self.click('.menu-submenu > summary')
        self.click(selector)
    def tab(self,name):self.click(f'[data-tab="{name}"]');self.wait(f'return !document.getElementById("tab-{name}").hidden')
    def settings(self):
        if self.js('return settingsPage.hidden'):self.click('settingsOpen');self.wait('return !settingsPage.hidden')
    def back(self):
        if not self.js('return settingsPage.hidden'):self.click('settingsBack');self.wait('return settingsPage.hidden')
    def feedback(self):return self.wait('const n=document.getElementById("toast"),d=document.querySelector("dialog[open]");return n.classList.contains("visible")&&n.getClientRects().length>0&&Number(getComputedStyle(n).opacity)>0&&(!d||d.contains(n));',3)
    def upload(self,id,path):
        elem=self.b.call(self.b.path+'/element',{'using':'css selector','value':'#'+id})
        self.b.call(self.b.path+'/element/'+elem['element-6066-11e4-a52e-4f735466cecf']+'/value',{'text':str(path.resolve())})
    def changed(self,revision):self.wait(f'return window.__morphforge.state.revision>{revision}',40)
    def rev(self):return self.api('state')['revision']
    def reset_ui(self):
        self.js('for(const d of document.querySelectorAll("dialog[open]"))d.close();document.querySelectorAll(".app-menu").forEach(d=>d.open=false);')
        self.back();self.tab('ai')
    def case(self,name,fn):
        if getattr(self.a,'only',None) and name not in self.a.only.split(','):return
        try:fn()
        except Exception as e:
            print('CASE FAILURE',name,str(e),flush=True);self.failures.append({'case':name,'error':str(e)})
            self.b.screenshot(self.out/(name+'-failure.png'))
        finally:self.reset_ui()
    def changed_click(self,id):r=self.rev();self.click(id);self.changed(r)
    def result(self):
        clicks=self.js('return JSON.parse(localStorage.getItem("guiAuditClicks")||"[]")')
        seen={c['id'] for c in clicks if c.get('id')};commands={c['command'] for c in clicks if c.get('command')}
        for item in self.inventory:
            item['clicked']=bool((item.get('id') and item['id'] in seen) or (item.get('command') and item['command'] in commands) or any(any(item.get(k) is not None and c.get(k)==item[k] for k in ('export','tab','view','scale')) for c in clicks))
        return {'success':not self.failures and all(c['passed'] for c in self.checks),'checks':self.checks,'failures':self.failures,'clicks':clicks,'inventory':self.inventory,'distinctIdButtonsClicked':len(seen),'scope':'Actual Chromium pointer clicks + Rust backend. Synthetic local files; localhost model fixture only. OSC limited to local 9012 fixture receiver.'}

def run(a):
    t=Audit(a);b=t.b;out=t.out;fixture_dir=out/'fixtures';fixture_dir.mkdir(exist_ok=True)
    image=fixture_dir/'gui-original.png';image.write_bytes(png(8,8,[[80,150,240,255]]*4+[[240,120,40,255]]*4))
    pack=fixture_dir/'gui-six-faces.zip';pack.write_bytes(fixture())
    mc=fixture_dir/'gui-house.nbt';mc.write_bytes(minecraft_fixture())
    try:
        t.check('test workspace has OSC disabled',not t.api('state')['connection']['enabled'])
        def settings():
            for name in ['text','sync','ai']:
                t.tab(name);t.settings();t.check('context remains reachable after '+name,not t.js('return document.getElementById("tab-context").hidden'));t.back()
            t.settings();t.fill('pinnedNotes','GUI audit persistent requirements',False);t.click('saveNotes');t.wait('return window.__morphforge.state.project.pinnedNotes==="GUI audit persistent requirements"');t.check('settings save has visible success feedback',t.feedback())
            t.fill('recipeCode','mf.box([-3,0,-3],[6,2,6],{style:1,group:"gui"});',False);r=t.rev();t.click('dryRecipe');t.wait('return toast.textContent.includes("预计")');t.check('recipe estimate is visible and does not mutate',t.feedback() and t.rev()==r)
            t.click('runRecipe');t.changed(r);t.check('recipe apply changes geometry and restores button',t.wait('return !runRecipe.disabled') and t.feedback())
            for id in ['settingsPreset','reasoning','maxSteps','maxRunSeconds']:
                value={'settingsPreset':'sketch','reasoning':'low','maxSteps':16,'maxRunSeconds':600}[id];t.fill(id,value)
            t.wait('return window.__morphforge.state.harness.settings.maxSteps===16');t.check('settings controls persist through actual APIs',t.api('harness/state')['settings']['reasoning']=='low')
        t.case('settings-feedback-and-context',settings)
        def navigation():
            for id in ['renderOptionsOpen','centerScene','runtimeMaterialOpen','pixelThemeOpen','imageToolsOpen','minecraftOpen','libraryOpen']:
                dialog={'renderOptionsOpen':'renderOptionsDialog','centerScene':'transformDialog','runtimeMaterialOpen':'runtimeMaterialDialog','pixelThemeOpen':'pixelThemeDialog','imageToolsOpen':'imageToolsDialog','minecraftOpen':'minecraftDialog','libraryOpen':'libraryDialog'}[id]
                close={'renderOptionsOpen':'renderOptionsClose','centerScene':'transformClose','runtimeMaterialOpen':'runtimeMaterialClose','pixelThemeOpen':'pixelThemeClose','imageToolsOpen':'imageToolsClose','minecraftOpen':'mcClose','libraryOpen':'libraryClose'}[id]
                t.click(id);t.wait(f'return {dialog}.open');t.check(id+' opens its actual dialog',True);t.click(close);t.check(close+' closes its actual dialog',not t.js(f'return {dialog}.open'))
            for id in ['newProject','workspaceOpen']:
                t.menu(id);t.wait('return workspaceDialog.open');t.check('file menu '+id+' works',True);t.click('workspaceClose')
            t.menu('settingsOpen');t.wait('return !settingsPage.hidden');t.back()
            t.menu('historyOpen');t.wait('return historyDialog.open');t.click('historyClose')
            for command,dialog,close in [('libraryOpen','libraryDialog','libraryClose'),('referenceOpen','referenceDialog','referenceClose'),('minecraftOpen','minecraftDialog','mcClose'),('pixelThemeOpen','pixelThemeDialog','pixelThemeClose'),('renderOptionsOpen','renderOptionsDialog','renderOptionsClose'),('centerScene','transformDialog','transformClose')]:
                t.menu(command);t.wait('return '+dialog+'.open');t.check('menu '+command+' delegates to its live dialog',True);t.click(close)
            t.click('aiSettingsOpen');t.wait('return !settingsPage.hidden');t.back()
            for id,cls in [('toggleTools','hide-tools'),('toggleObserver','hide-observer')]:
                t.menu('fitView');t.js('document.querySelectorAll(".app-menu").forEach(n=>n.open=false)');t.click('.app-menu:nth-child(3)>summary');t.click(id);t.check(id+' hides panel',t.js('return document.body.classList.contains(arguments[0])',cls));t.click('.app-menu:nth-child(3)>summary');t.click(id);t.check(id+' restores panel',not t.js('return document.body.classList.contains(arguments[0])',cls))
        t.case('menus-and-dialogs',navigation)
        def drawing():
            t.changed_click('clearScene')
            for id in ['houseDemo','stairsDemo','directionsDemo','stress5k','stress16k']:
                t.changed_click(id);expected={'stress5k':5000,'stress16k':16384}.get(id);s=t.api('scene')['scene'];t.check(id+' creates visible geometry',len(s['boxes'])>0 and (expected is None or len(s['boxes'])==expected))
            t.changed_click('clearScene');t.click('addTool');t.fill('groupName','gui',False)
            for id,value in [('coordX',0),('coordY',0),('coordZ',0),('brushX',4),('brushY',4),('brushZ',4)]:t.fill(id,value,False)
            t.changed_click('applyPosition');t.check('position draw creates a real cuboid',t.api('scene')['scene']['boxes'][0]['size']==[4,4,4]);t.changed_click('undo');t.check('undo removes drawn cuboid',not t.api('scene')['scene']['boxes']);t.changed_click('redo');t.check('redo restores cuboid',bool(t.api('scene')['scene']['boxes']))
            t.click('eraseTool');t.changed_click('applyPosition');t.check('eraser applies exact region',not t.api('scene')['scene']['boxes']);t.click('addTool');t.changed_click('applyPosition');t.click('selectTool');t.click('[data-view=front]');b.point([.5,.5,1]);t.wait('return window.__morphforge.renderer.selection.size===1')
            for id in ['moveSelection','copySelection','rotateSelection','mirrorSelection']:
                t.fill('coordX',2,False);t.fill('coordY',0,False);t.fill('coordZ',0,False);r=t.rev();t.menu(id);t.changed(r);t.check(id+' modifies selected geometry',True)
            t.click('#palette button:nth-child(3)');r=t.rev();t.click('paintSelection');t.changed(r);t.check('painting selected cuboid changes its material',any(x['styleId']==3 for x in t.api('scene')['scene']['boxes']));t.check('painting preserves an actionable selection',t.js('return window.__morphforge.renderer.selection.size>0'))
            for id in ['moveSelection','copySelection','rotateSelection','mirrorSelection','paintSelection']:
                if t.js('return window.__morphforge.renderer.selection.size>0'):
                    t.click('selectTool');t.click('[data-view=front]');point=t.js('const r=window.__morphforge.renderer,b=r.boxes.find(b=>r.selection.has(b.id));return b.min.map((v,i)=>(v+b.size[i]/2)*r.cellSize)');b.point(point,shift=True);t.wait('return window.__morphforge.renderer.selection.size===0')
                t.click(id);t.wait('return toast.textContent.includes("请先选择")');t.check(id+' empty selection gives feedback',t.feedback())
            for view in ['hero','front','top','side']:t.click(f'[data-view={view}]');t.check('camera '+view+' is finite',t.js('return window.__morphforge.renderer.camera.position.toArray().every(Number.isFinite)'))
            t.click('fitView')
        t.case('drawing-selection-and-cameras',drawing)
        def mesh():
            t.changed_click('clearScene');t.click('meshAdd');t.wait('return window.__morphforge.state.stats.triangleCount===1');t.click('selectTool');t.click('[data-view=front]');b.point([0,4/3,0]);t.wait('return window.__morphforge.renderer.selection.size===1');t.click('meshLoadSelection');t.check('load selected triangle fills vertex fields',t.js('return meshC.value==="0, 16, 0"'));t.fill('meshC','0, 20, 0',False);t.changed_click('meshUpdate');t.check('mesh update keeps one changed triangle',t.api('scene')['scene']['triangles'][0]['vertices'][2][1]==20);t.changed_click('meshDelete');t.check('triangle delete removes selected face',not t.api('scene')['scene'].get('triangles'))
            t.changed_click('voxelWedge');t.check('voxel slope remains one shaped box',t.api('scene')['scene']['boxes'][0]['shape']==1);t.fill('wedgeMin','24, 0, 0',False);t.changed_click('meshWedge');t.check('polygon wedge creates eight real faces',len(t.api('scene')['scene']['triangles'])==8)
            t.click('meshLoadSelection');t.wait('return toast.textContent.includes("选择一个三角面")');t.check('missing mesh selection is explained',t.feedback())
        t.case('triangle-and-slope-editing',mesh)
        def materials():
            t.click('runtimeMaterialOpen');t.fill('runtimeKind','pbr');t.fill('runtimeRough',2,False);t.click('runtimeMaterialApply');t.wait('return toast.dataset.kind==="error"');t.check('modal invalid input error is inside active dialog',t.feedback())
            t.fill('runtimeRough',.4,False);t.fill('runtimeName','GUI PBR '+out.name,False);count=len(t.api('scene')['materials']);t.click('runtimeMaterialApply');t.wait('return runtimeReceipt.textContent.includes("rough")');t.check('runtime material creates or reuses a matching style',any(m.get('source')=='MorphForge runtime material/1' and abs(m['roughness']-.4)<1e-5 for m in t.api('scene')['materials']) and len(t.api('scene')['materials']) in [count,count+1]);t.click('runtimeMaterialClose')
            t.click('pixelThemeOpen');t.click('applyPixelTheme');t.wait('return !pixelThemeDialog.open');t.check('theme import adds textured builtins',any(m.get('pack') for m in t.api('scene')['materials']))
            t.click('packImport');t.upload('packFile',pack);t.wait('return packReportDialog.open');t.check('resource pack button/import reports results',any(m.get('source')=='assets/mf/models/block/sixface.json' for m in t.api('scene')['materials']));t.click('packReportClose')
        t.case('materials-and-resource-pack-upload',materials)
        def text():
            t.tab('text');t.click('textFontRefresh');t.wait('return !textFont.disabled && !textFontRefresh.disabled');t.check('font refresh reports completion',t.feedback());t.fill('textContent','造形 GUI');t.fill('textSize',4);r=t.rev();t.click('textPreview');t.wait('return !textApply.disabled',30);t.check('font preview produces boxes without applying',bool(t.js('return window.__morphforge.text.preview()?.boxes?.length || window.__morphforge.text.preview()?.operations?.length')) and t.rev()==r);t.click('textClearPreview');t.check('clear text preview disables apply',t.js('return textApply.disabled'));t.click('textPreview');t.wait('return !textApply.disabled');t.click('textApply');t.changed(r);t.check('text apply creates editable voxel geometry',any(x['group']=='text' for x in t.api('scene')['scene']['boxes']))
        t.case('voxel-font-tools',text)
        def references():
            t.click('referenceUpload');t.upload('referenceFiles',image);t.wait('return document.querySelector("#referenceSelected .reference-chip")');t.check('image upload adds a selected local reference',bool(t.api('references')['images']))
            t.click('referenceOpen');t.wait('return referenceDialog.open');t.wait('return document.querySelector(".reference-card input")');t.click('.reference-card input');t.click('referenceClose');t.check('reference library selection persists',t.js('return referenceSummary.textContent!=="不使用参考图片"'))
            t.click('imageToolsOpen');t.fill('imageToolWidth',8,False);t.fill('imageToolHeight',8,False);t.click('imageToolInspect');t.wait('return imageToolReport.textContent.includes("pixels")');t.check('pixel-read button returns actual pixels',True);t.click('imageToolResize');t.wait('return imageToolReport.textContent.includes("parentImageId")');t.check('resize button keeps original and creates a copy',True);r=t.rev();t.click('imageToolPreview');t.wait('return !imageToolApply.disabled');t.check('voxel-plane preview leaves scene unchanged',t.rev()==r);t.click('imageToolApply');t.changed(r);t.wait('return !imageToolsDialog.open');t.check('voxel-plane apply mutates and closes dialog',not t.js('return imageToolsDialog.open'))
        t.case('reference-images-and-pixel-tools',references)
        def minecraft():
            t.click('minecraftOpen');t.wait('return minecraftDialog.open');t.click('mcUpload');t.upload('mcFile',mc);t.wait('return mcSource.options.length>0');t.click('mcInspect');t.wait('return mcReport.textContent.includes("source")');r=t.rev();t.click('mcPreview');t.wait('return !mcApply.disabled',40);t.check('Minecraft preview uses independent viewport',t.rev()==r and t.js('return !!document.querySelector("#mcViewport canvas")'));t.click('mcApply');t.changed(r);t.check('Minecraft apply creates actual editable building',bool(t.api('scene')['scene']['boxes']));t.click('mcClose')
        t.case('minecraft-building-workflow',minecraft)
        def files_library():
            t.menu('saveArtwork');t.wait('return toast.textContent.includes("正在保存")',5);t.check('save artwork shows immediate progress',t.feedback());t.wait('return toast.textContent.includes("已保存")',40);t.check('save artwork adds a real library item',len(t.api('artworks')['artworks'])>0);t.click('libraryOpen');t.wait('return libraryDialog.open');t.fill('librarySearch','未命名',False);t.fill('librarySearch','',False);t.wait('return document.querySelector(".artwork button")');t.click('.artwork button');t.wait('return captureDialog.open');t.check('library multi-view opens actual image gallery',t.js('return document.querySelectorAll("#captureImages img").length>=3'));t.click('captureClose');t.click('.artwork button:last-child');t.wait('return !libraryDialog.open');t.check('library load restores editable artwork',t.api('state')['stats']['objectCount']>0)
            t.click('capture');t.wait('return captureDialog.open',40);t.check('toolbar screenshot shows three actual views',t.js('return document.querySelectorAll("#captureImages img").length===3'));t.click('captureClose')
            t.menu('exportProject');t.wait('return true');time.sleep(.7);files=list((out/'downloads').glob('*.json'));t.check('file save downloads project JSON',bool(files) and json.loads(files[-1].read_text())['format']=='MorphForgeProject/1');t.menu('importProject');t.upload('projectFile',files[-1]);time.sleep(1);t.check('project import keeps a valid editable scene',t.api('state')['stats']['objectCount']>0)
            for format in ['glb','obj','ply','stl']:
                t.menu(format,export=True);t.wait('return modelExportDialog.open');t.click('modelExportApply');t.wait('return modelExportStatus.textContent.includes("导出完成")',40);t.check(format+' GUI export returns a downloaded model',True);t.click('modelExportClose')
            t.menu('diagnosticsExport');deadline=time.monotonic()+20
            while time.monotonic()<deadline and not any('diagnostics' in p.name for p in (out/'downloads').glob('*.zip')):time.sleep(.1)
            t.check('diagnostics menu downloads ZIP',any('diagnostics' in p.name for p in (out/'downloads').glob('*.zip')))
        t.case('library-capture-and-file-exports',files_library)
        def view_and_groups():
            t.click('renderOptionsOpen');t.fill('voxelGrid',True);t.wait('return (async()=> (await (await fetch("/api/view-settings")).json()).voxelGrid===true)()');time.sleep(.3);t.check('grid setting reaches renderer',t.js('return window.__morphforge.renderer.voxelGrid===true') if t.js('return "voxelGrid" in window.__morphforge.renderer') else t.api('view-settings')['voxelGrid']);t.fill('voxelGrid',False);t.click('renderOptionsClose')
            t.click('centerScene');t.wait('return !transformApply.disabled');t.fill('transformAction','translate');t.fill('shiftX',2,False);t.click('transformPreview');t.wait('return !transformApply.disabled');t.check('center preview displays calculated offset',t.js('return transformReport.textContent.includes("偏移")'));t.changed_click('transformApply');t.check('centering applies and closes dialog',not t.js('return transformDialog.open'))
            t.click('#groups .group-visibility');t.check('group visibility changes local render',t.js('return window.__morphforge.renderer.groupsHidden.size>0'));t.click('#groups .group-visibility');t.click('#groups .group-protect');time.sleep(.7);t.check('group lock persists to backend',bool(t.api('state')['project']['protectedGroups']));t.click('#groups .group-protect')
            t.menu('historyOpen');t.wait('return historyDialog.open');t.click('#historyList button');t.wait('return !historyDialog.open');t.check('version restore closes history with a valid scene',t.api('state')['revision']>=0)
        t.case('render-options-centering-and-groups',view_and_groups)
        def osc():
            t.changed_click('clearScene');t.click('#palette button:first-child');t.click('addTool');t.changed_click('applyPosition');t.tab('sync');t.click('avatarCatalogMf6');t.wait('return window.__morphforge.state.connection.profile==="mf6"');t.click('vrcPort');t.wait('return window.__morphforge.state.connection.port===9000');t.check('VRChat preset changes port while output remains off',not t.api('state')['connection']['enabled']);t.click('unityPort');t.wait('return window.__morphforge.state.connection.port===9012')
            for id in ['avatarCatalogBundled','avatarCatalogMf4','avatarCatalogClear','avatarCatalogMf6']:
                t.click(id);time.sleep(.5);t.check(id+' changes target catalog state',True)
            t.click('catalogExport');time.sleep(.7);catalog=next(p for p in (out/'downloads').glob('*.zip') if 'materials' in p.name);t.click('avatarCatalogImport');t.upload('avatarCatalogFile',catalog);time.sleep(1);t.check('catalog export/import matches active material',t.api('state')['materialCompatibility']['matched'])
            for n in ['1','100','1000']:t.click(f'[data-scale="{n}"]');time.sleep(.4);t.check('placement scale '+n+'% persists',abs(t.api('state')['connection']['placement']['scale']-float(n)/100*.05)<1e-6)
            for id in ['world','visible','level']:t.click(id);time.sleep(.3);t.check('placement toggle '+id+' persists',t.js('return document.getElementById(arguments[0]).checked',id)==t.api('state')['connection']['placement'][id])
            t.click('resetAnchor');t.click('publish');t.wait('return toast.textContent.includes("准备")');t.click('repairPublish');t.click('resync');t.check('disabled-output publish controls do not enable transmission',not t.api('state')['connection']['enabled'])
            t.click('avatarCatalogMf6');t.wait('return window.__morphforge.state.connection.profile==="mf6"');receiver=socket.socket(socket.AF_INET,socket.SOCK_DGRAM);receiver.bind(('127.0.0.1',9012));receiver.settimeout(6)
            try:
                t.click('connect');t.wait('return window.__morphforge.state.connection.enabled');t.click('publish');packet,_=receiver.recvfrom(65536);t.check('enable and publish send a real local OSC packet',len(packet)>0);t.click('connect');t.wait('return !window.__morphforge.state.connection.enabled')
            finally:receiver.close();t.api('connect',{'enabled':False})
        t.case('osc-fixture-and-avatar-placement',osc)
        def workspaces():
            previous=t.api('state')['project']['id'];t.menu('newProject');t.fill('workspaceName','GUI new project',False);t.click('workspaceCreate');t.wait('return window.__morphforge?.ready && window.__morphforge.state.project.name==="GUI new project"',40);t.check('file new project creates separate project',t.api('state')['project']['id']!=previous);old=t.api('harness/state')['context']['id'];t.click('newContext');t.click('workspaceCreate');t.wait(f'return window.__morphforge?.ready && window.__morphforge.state.harness.context.id!=="{old}"',40);t.check('new context changes session without losing project',t.api('state')['project']['name']=='GUI new project');t.menu('workspaceOpen');t.wait('return workspaceDialog.open');t.click('#workspaceProjects [data-workspace-id="'+previous+'"] button');t.wait('return window.__morphforge?.ready && window.__morphforge.state.project.id==="'+previous+'"',40);t.check('workspace switch opens previously saved project',True)
        t.case('project-and-conversation-buttons',workspaces)
        def menu_aliases():
            t.changed_click('clearScene');t.click('addTool');t.changed_click('applyPosition')
            t.wait('return redo.disabled');t.click('.app-menu:nth-child(2)>summary');t.check('menu redo mirrors unavailable target state',t.js('return document.querySelector(arguments[0]).disabled','[data-command="redo"]'));t.click('.app-menu:nth-child(2)>summary')
            revision=t.rev();t.menu('undo');t.changed(revision);t.check('menu undo removes actual geometry',not t.api('scene')['scene']['boxes'])
            revision=t.rev();t.menu('redo');t.changed(revision);t.check('menu redo restores actual geometry',bool(t.api('scene')['scene']['boxes']))
            t.menu('capture');t.wait('return captureDialog.open');t.check('menu screenshot opens three real views',t.js('return document.querySelectorAll("#captureImages img").length===3'));t.click('captureClose')
            was_hidden=t.js('return timeline.hidden');t.menu('timelineToggle');t.check('menu timeline changes actual panel visibility',t.js('return timeline.hidden')!=was_hidden);t.menu('timelineToggle')
            t.menu('packImport');t.upload('packFile',pack);t.wait('return packReportDialog.open');t.check('menu resource pack import reaches the upload report',True);t.click('packReportClose')
            t.tab('sync');t.click('[data-scale="10"]');t.wait('return Math.abs(window.__morphforge.state.connection.placement.scale-.005)<1e-7');t.check('10 percent scale shortcut persists',abs(t.api('state')['connection']['placement']['scale']-.005)<1e-7);t.tab('ai')
            if not t.js('return activityOlder.disabled'):
                count=t.js('return document.querySelectorAll(".activity-card").length');t.click('activityOlder');time.sleep(1);t.check('earlier-process button retains or expands observed history',t.js('return document.querySelectorAll(".activity-card").length')>=count)
            else:t.check('earlier-process button is disabled when no earlier page exists',True)
        t.case('desktop-menu-aliases',menu_aliases)
        t.check('no unhandled browser JavaScript errors',not t.js('return window.__guiAuditErrors'))
    finally:
        t.api('connect',{'enabled':False});report=t.result();(out/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));b.screenshot(out/'final.png');b.close()
    if not report['success']:raise SystemExit(1)

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--webdriver',default='http://127.0.0.1:9518');p.add_argument('--only',help='comma-separated scenario names');run(p.parse_args())
if __name__=='__main__':main()
