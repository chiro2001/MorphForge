#!/usr/bin/env python3
"""Read-only observer/menu/export acceptance on an isolated backend with history.
No provider calls or OSC. The final session-isolation check creates a fresh local
conversation in the test workspace; never point this at a user's active backend.
"""
import argparse,json,time,urllib.request,base64
from pathlib import Path
from check_editor_browser import Browser

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--url',required=True);p.add_argument('--out',required=True,type=Path);p.add_argument('--webdriver',default='http://127.0.0.1:9518');a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True);b=Browser(a.webdriver);checks=[];report={}
 def api(path,body=None):
  req=urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
  with urllib.request.urlopen(req,timeout=90) as r:return json.load(r)
 def check(name,yes):
  checks.append({'name':name,'passed':bool(yes)});print(('PASS ' if yes else 'FAIL ')+name,flush=True);assert yes,name
 try:
  start=api('state');check('isolated backend keeps OSC off',not start['connection']['enabled']);b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready',30);b.wait('return document.querySelectorAll(".activity-card").length>2',20)
  check('history renders thought, tools, text and screenshot cards',b.js('return !!document.querySelector(".activity-thought:not([hidden])") && !!document.querySelector(".activity-tool") && !!document.querySelector(".activity-image img") && !!document.querySelector(".activity-text:not([hidden])")'))
  b.js("""(async()=>{const {ActivityView}=await import('/activity-ui.js');const v=Object.create(ActivityView.prototype);Object.assign(v,{busy:false,generation:0,cursor:null,before:null,rows:new Map(),nodes:new Map(),follow:{following:true},render(){if(this.rows.has(10))this.nodes.set('found',{});}});const pages=[{events:[],previousCursor:200,nextCursor:900,hasOlder:true,hasMore:false},{events:[],previousCursor:100,nextCursor:200,hasOlder:true,hasMore:true},{events:[{seq:10}],previousCursor:0,nextCursor:100,hasOlder:false,hasMore:true},{events:[{seq:11}],previousCursor:900,nextCursor:1000,hasOlder:true,hasMore:false}];const calls=[];v.api=async path=>{calls.push(path);return pages.shift();};await v.load();const historical=v.cursor===900&&v.rows.has(10);await v.load();window.__backfillProbe={ok:historical&&v.cursor===1000&&v.rows.has(11)&&calls.length===4&&calls[1].includes('before=200')&&calls[3].includes('cursor=900'),calls};})().catch(e=>window.__backfillProbe={ok:false,error:String(e)});""")
  b.wait('return !!window.__backfillProbe');check('restoring an older session searches behind other sessions and keeps the live cursor',b.js('return window.__backfillProbe.ok'))
  check('observer is an independently scrollable visible pane',b.js('return activityFeed.clientHeight>=130 && activityFeed.scrollHeight>activityFeed.clientHeight && activityFeed.getBoundingClientRect().bottom<innerHeight'))
  b.js('activityFeed.scrollTop=0;');time.sleep(.2);old=b.js('return activityFeed.scrollTop');before=b.js('return document.querySelectorAll(".activity-card").length')
  for _ in range(3):api('harness/tool',{'name':'get_context','arguments':{'styleLimit':1}})
  time.sleep(2.3)
  check('new events do not pull a reader to the end',abs(b.js('return activityFeed.scrollTop')-old)<3 and b.js('return !activityLatest.hidden'))
  check('new operations are actually rendered during that scroll check',b.js('return document.querySelectorAll(".activity-card").length')>before)
  b.click('activityLatest');check('return-to-latest explicitly resumes follow',b.js('return activityFeed.scrollHeight-activityFeed.scrollTop-activityFeed.clientHeight<40 && activityLatest.hidden'))
  b.js('document.querySelector(".activity-thought:not([hidden])").open=true;activityFeed.scrollTop=30;');time.sleep(.25);position=b.js('return activityFeed.scrollTop');api('harness/tool',{'name':'get_context','arguments':{'styleLimit':1}});time.sleep(2)
  check('expanded thought stays open when output updates',b.js('return document.querySelector(".activity-thought:not([hidden])").open'))
  check('expanded history keeps its reading position',abs(b.js('return activityFeed.scrollTop')-position)<3)
  b.js('document.querySelector(".activity-image").click()');check('screenshots open in a full-size viewer',b.js('return activityImageDialog.open && activityImage.naturalWidth>0'));b.click('activityImageClose')
  b.click('timelineToggle');b.wait('return document.querySelectorAll(".timeline-card").length>3',10);b.js('timeline.scrollTop=160;');time.sleep(.2);position=b.js('return timeline.scrollTop');api('harness/tool',{'name':'get_context','arguments':{'styleLimit':1}});time.sleep(2)
  check('raw event pane also stops following after scrolling up',abs(b.js('return timeline.scrollTop')-position)<3 and not b.js('return timelineFollow.checked'))
  b.click('timelineToggle');b.click('settingsOpen');b.wait('return !settingsPage.hidden');check('context and manual recipe remain available in settings',b.js('return !!document.querySelector("#settingsPage #tab-context") && !!document.querySelector("#settingsPage #recipeCode")'));b.click('settingsBack');b.wait('return settingsPage.hidden')
  b.js('document.querySelector(".app-menu > summary").click();');b.screenshot(a.out/'file-menu.png');check('file menu exposes nested model exports',b.js('return !!document.querySelector(".menu-submenu [data-export=glb]")'))
  b.js('document.querySelector(".menu-submenu").open=true;document.querySelector("[data-export=glb]").click();');check('GLB menu opens a format and scale dialog',b.js('return modelExportDialog.open && modelExportFormat.value==="glb"'))
  b.call(b.path+'/goog/cdp/execute',{'cmd':'Browser.setDownloadBehavior','params':{'behavior':'allow','downloadPath':str(a.out.resolve()/'downloads')}})
  b.click('modelExportApply');b.wait('return modelExportStatus.textContent.includes("导出完成")',30);check('real menu export succeeds',True);b.screenshot(a.out/'export-dialog.png');b.click('modelExportClose')
  deadline=time.monotonic()+10
  while time.monotonic()<deadline and not list((a.out/'downloads').glob('*.glb')):time.sleep(.1)
  files=list((a.out/'downloads').glob('*.glb'));check('download is an actual GLB file',bool(files) and files[0].read_bytes().startswith(b'glTF'))
  check('read-only operations leave geometry unchanged',api('state')['revision']==start['revision'])
  b.click('activityLatest') if b.js('return !activityLatest.hidden') else None
  b.screenshot(a.out/'observer.png')
  current=api('state');api('workspaces',{'action':'new_session','name':'Observer isolation check','baseProjectId':current['project']['id'],'baseSessionId':current['harness']['context']['id']})
  b.wait('return document.querySelectorAll(".activity-card").length===0',20);check('new conversation does not leak another session activity',True)
  check('no provider request was started by viewing or export',not api('harness/state')['run'] or api('harness/state')['run']['status']!='running')
  logs=b.call(b.path+'/log',{'type':'browser'});check('no JavaScript or WebGL errors',not [r for r in logs if r['level']=='SEVERE' and 'favicon.ico' not in r['message']]);report={'success':True,'checks':checks,'scope':'Actual Chromium, historical model output, real read-only tools, download and session isolation. No paid model calls or OSC.'}
 except Exception as error:
  b.screenshot(a.out/'failure.png');report={'success':False,'checks':checks,'error':str(error),'logs':b.call(b.path+'/log',{'type':'browser'})};raise
 finally:
  (a.out/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));b.close()
if __name__=='__main__':main()
