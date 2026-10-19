#!/usr/bin/env python3
"""Exercise project/session controls in the real shared frontend; isolated data only."""
import argparse,json,urllib.request
from pathlib import Path
from check_editor_browser import Browser

def main():
 p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
 b=Browser();checks=[]
 def check(name,value):
  checks.append({'name':name,'passed':bool(value)})
  if not value:raise AssertionError(name)
 def api(path,body=None):
  r=urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
  with urllib.request.urlopen(r,timeout=30) as response:return json.load(response)
 try:
  b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready')
  original=api('state');project=original['project']['id'];session=original['harness']['context']['id']
  api('edit',{'op':'demo','name':'stairs'});b.wait('return window.__morphforge.state.stats.boxCount > 0')
  boxes=api('scene')['scene']['boxes'];revision=api('state')['revision']
  check('visible new project and new conversation controls',b.js("return ['newProject','newContext','workspaceOpen'].every(id=>document.getElementById(id).getBoundingClientRect().width>0)"))
  b.click('newContext');b.wait('return document.getElementById("workspaceDialog").open');check('new conversation opens correct creation mode',b.js('return document.getElementById("workspaceKind").value')=='new_session')
  b.js('document.getElementById("workspaceName").value="局部修改对话"');b.click('workspaceCreate')
  b.wait('return window.__morphforge?.ready && window.__morphforge.state.harness.context.id !== '+json.dumps(session))
  second=api('state');check('new conversation keeps artwork and starts empty context',second['project']['id']==project and api('scene')['scene']['boxes']==boxes and second['harness']['context']['messages']==0)
  check('new conversation changes client storage scope',b.js('return window.__morphforge.harnessUi.chat().length')==0)
  b.click('workspaceOpen');b.wait('return document.querySelectorAll("#workspaceSessions button").length === 2')
  b.screenshot(a.out/'sessions.png')
  b.js('document.querySelector("#workspaceSessions button:not([disabled])").click()')
  b.wait('return window.__morphforge?.ready && window.__morphforge.state.harness.context.id === '+json.dumps(session))
  check('old conversation can be reopened from UI',api('state')['harness']['context']['id']==session)
  b.click('newProject');b.wait('return document.getElementById("workspaceDialog").open');b.js('document.getElementById("workspaceName").value="验收空白项目"');b.click('workspaceCreate')
  b.wait('return window.__morphforge?.ready && window.__morphforge.state.project.id !== '+json.dumps(project))
  check('new project shows empty geometry at revision zero',api('state')['stats']['boxCount']==0 and api('state')['revision']==0 and b.js('return window.__morphforge.renderer.boxes.length')==0)
  b.click('workspaceOpen');b.wait('return document.querySelectorAll("#workspaceProjects button").length === 2');b.screenshot(a.out/'projects.png')
  b.js('document.querySelector("#workspaceProjects button:not([disabled])").click()')
  b.wait('return window.__morphforge?.ready && window.__morphforge.state.project.id === '+json.dumps(project))
  b.wait('return window.__morphforge.renderer.boxes.length > 0')
  check('reopening project restores geometry and revision in viewport',api('scene')['scene']['boxes']==boxes and api('state')['revision']==revision)
  for width in [1440,1000,900]:
   b.call(b.path+'/window/rect',{'width':width,'height':920})
   check(f'header controls fit at {width}px',b.js('return [...document.querySelectorAll("header button")].every(e=>e.getBoundingClientRect().right <= innerWidth && e.getBoundingClientRect().left>=0)'))
  b.screenshot(a.out/'workbench.png')
  logs=b.call(b.path+'/log',{'type':'browser'});check('no browser errors',not [x for x in logs if x['level']=='SEVERE'])
  check('OSC remains disconnected',not api('state')['connection']['enabled'])
  report={'success':True,'scope':'Real Chromium clicks, local isolated data, no model requests or OSC','checks':checks,'browserLogs':logs}
 except Exception as e:
  b.screenshot(a.out/'failure.png');report={'success':False,'checks':checks,'error':str(e)};raise
 finally:
  (a.out/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));b.close()
 print(json.dumps(report,ensure_ascii=False,indent=2))
if __name__=='__main__':main()
