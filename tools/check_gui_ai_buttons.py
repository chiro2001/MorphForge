#!/usr/bin/env python3
"""AI GUI controls using a localhost SSE fixture; no real model billing."""
import argparse,json,threading,time
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from pathlib import Path
from check_gui_buttons import Audit

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--webdriver',default='http://127.0.0.1:9518');p.add_argument('--only-observation',action='store_true');a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
 mode={'value':'hold','requests':0,'zero':False};t=None
 class Handler(BaseHTTPRequestHandler):
  def log_message(self,*_):pass
  def do_GET(self):
   data=json.dumps({'data':[{'id':'gui-fixture','context_window':1048576,'max_output_tokens':393216,'input_modalities':['text','image']}]}).encode();self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
  def do_POST(self):
   json.loads(self.rfile.read(int(self.headers['Content-Length'])));mode['requests']+=1;active=mode['value'];count=mode['requests']
   self.send_response(200);self.send_header('Content-Type','text/event-stream');self.end_headers()
   try:
    def emit(v):self.wfile.write(('data: '+json.dumps(v,ensure_ascii=False)+'\n\n').encode());self.wfile.flush()
    emit({'type':'response.reasoning_summary_text.delta','delta':'本地GUI夹具思考 <img src=x onerror="window.__buttonInjected=1">'})
    if active=='hold':
     for _ in range(80):time.sleep(.15);emit({'type':'response.reasoning_summary_text.delta','delta':'等待用户控制。'})
    output=[{'type':'function_call','call_id':'gui-'+str(count),'name':'get_context','arguments':'{"styleLimit":1}'}] if active=='cache' else [{'type':'message','role':'assistant','content':[{'type':'output_text','text':'本地GUI测试摘要：保留作品与固定要求，未调用真实模型。'}]}]
    emit({'type':'response.completed','response':{'status':'completed','output':output,'usage':{'input_tokens':20000,'output_tokens':20,'total_tokens':20020,'input_tokens_details':{'cached_tokens':0 if mode['zero'] else 19000}}}})
   except (BrokenPipeError,ConnectionResetError):pass
 server=ThreadingHTTPServer(('127.0.0.1',0),Handler);threading.Thread(target=server.serve_forever,daemon=True).start()
 try:
  t=Audit(a)
  if not a.only_observation:
   t.check('test never connects to a paid model',True);t.settings()
   if t.api('harness/state')['cacheMonitor']['alert']:t.click('cacheAcknowledge');t.wait('return !window.__morphforge.state.harness.cacheMonitor.alert')
   for id,value in [('providerBase',f'http://127.0.0.1:{server.server_port}'),('providerApi','responses'),('providerKey','gui-local-only-fixture')]:t.fill(id,value,False)
   t.click('saveProvider');t.wait('return !saveProvider.disabled && toast.textContent.includes("已保存")');t.check('save API connection has visible confirmation',t.feedback());t.fill('model','gui-fixture');t.fill('cacheGuard','observe');t.fill('goalMode',False);t.fill('maxSteps',32);t.back();t.tab('ai')
   t.check('idle pause/resume/stop buttons are intentionally disabled',t.js('return pauseAI.disabled&&resumeAI.disabled&&stopAI.disabled'))
   t.fill('prompt','GUI本地按钮验证。'+('保留作品和固定要求；'*200),False);t.click('sendMessage');t.wait('return !pauseAI.disabled');t.check('send starts a real harness request against local fixture',mode['requests']>0)
   t.wait('return [...document.querySelectorAll(".activity-thought")].some(n=>!n.hidden)');t.check('reasoning/text display does not execute embedded HTML',not t.js('return !!window.__buttonInjected || !!document.querySelector("#activityFeed img[src=x]")'))
   t.click('pauseAI');t.wait('return !resumeAI.disabled');t.check('pause cancels current request and exposes resume',t.api('harness/state')['paused']);t.click('resumeAI');t.wait('return !pauseAI.disabled');t.check('resume makes the running controls active again',not t.api('harness/state')['paused'])
   t.fill('messageMode','steer');t.fill('prompt','插话：只检查，不加几何。',False);t.click('sendMessage');t.wait('return Number(pendingCount.textContent)>0');t.check('steer message is acknowledged while request continues',bool(t.api('harness/state')['interaction']['queue']))
   t.fill('messageMode','queue');t.fill('prompt','下一轮：本地排队检查。',False);t.click('sendMessage');t.check('queue mode retains a second pending message',len(t.api('harness/state')['interaction']['queue'])>=2)
   t.fill('messageMode','interrupt');t.fill('prompt','打断后继续本地验证。',False);epoch=t.api('harness/state')['interaction']['epoch'];t.click('sendMessage');t.wait(f'return window.__morphforge.state.harness.interaction.epoch>{epoch}');t.check('interrupt supersedes active request epoch',True)
   mode['value']='complete';t.click('stopAI');t.wait('return stopAI.disabled',30);t.check('stop returns control to an idle or terminal run',t.api('harness/state')['run']['status'] in ['stopped','completed','cancelled'])
   t.settings();t.click('compact');t.wait('return window.__morphforge.state.harness.context.compactCount>0',30);t.check('compact button returns a visible summary and completion',t.feedback() and '本地GUI' in t.js('return contextSummary.textContent'));t.back()
   t.settings();t.fill('cacheGuard','strict');t.back();mode['value']='cache';mode['zero']=True;t.fill('prompt','本地缓存告警按钮验证。',False);t.click('sendMessage');t.wait('return !cacheShowConfig.hidden',30)
   deadline=time.monotonic()+30
   while not t.api('harness/state')['cacheMonitor']['alert'] and time.monotonic()<deadline:time.sleep(.1)
   t.check('actual guard state exposes the cache warning action',t.api('harness/state')['cacheMonitor']['alert'] is not None);t.click('cacheShowConfig');t.wait('return !settingsPage.hidden');t.click('cacheAcknowledge');t.wait('return !window.__morphforge.state.harness.cacheMonitor.alert');t.check('cache acknowledgement clears the pending alert without bypassing explicit resume',True)
   mode['zero']=False;mode['value']='complete';t.back();t.click('stopAI') if not t.js('return stopAI.disabled') else None;t.wait('return stopAI.disabled',20)
  rect=t.js('return activityFeed.getBoundingClientRect().toJSON()');t.b.call(t.b.path+'/actions',{'actions':[{'type':'wheel','id':'observer-wheel','actions':[{'type':'scroll','x':round(rect['x']+rect['width']/2),'y':round(rect['y']+rect['height']/2),'deltaX':0,'deltaY':-2000,'duration':150}]}]});t.wait('return !activityLatest.hidden',5);t.click('activityLatest');t.check('return-to-latest action resumes observation',t.js('return activityLatest.hidden'))
  t.click('timelineToggle');t.wait('return document.querySelectorAll(".timeline-card").length>1');t.click('timelineLatest');t.click('timelineOlder') if not t.js('return timelineOlder.disabled') else None
  t.js('timeline.scrollTop=0');t.wait('return !timelineFollow.checked');t.click('timelineResume');t.check('raw timeline latest button restores follow',t.js('return timelineFollow.checked'))
  t.fill('eventFilter','tool.');t.wait('return document.querySelectorAll(".timeline-card").length>0 && [...document.querySelectorAll(".timeline-card")].every(n=>n.textContent.includes("工具"))');t.check('event-type filter produces a visible tool history',t.js('return [...document.querySelectorAll(".timeline-card")].every(n=>n.textContent.includes("工具"))'))
  t.fill('eventFilter','');time.sleep(.7);t.click('#events .timeline-card .row > button:last-child');t.wait('return timelinePreview.open');revision=t.rev();t.check('history preview opens a real read-only renderer',t.js('return !!document.querySelector("#timelinePreviewViewport canvas")'));t.click('timelinePreviewClose');t.check('historical observation leaves the draft unchanged',t.rev()==revision)
  t.click('timelineToggle')
  t.click('capture');t.wait('return captureDialog.open',40);t.click('captureClose')
  for _ in range(5):
   if t.js('return !!document.querySelector(".activity-image")'):break
   if t.js('return activityOlder.disabled'):break
   t.click('activityOlder');time.sleep(.6)
  if t.js('return !!document.querySelector(".activity-image")'):
   t.click('.activity-image');t.wait('return activityImageDialog.open');t.click('activityImageClose');t.check('observer image open and close actions work',not t.js('return activityImageDialog.open'))
  t.check('no unhandled JavaScript errors in model controls' ,not t.js('return window.__guiAuditErrors'));t.b.screenshot(a.out/'model-controls.png')
 except Exception as error:
  if t:
   t.failures.append({'case':'AI controls','error':str(error)});t.b.screenshot(a.out/'failure.png')
  raise
 finally:
  if t:
   try:t.api('harness/control',{'action':'stop'})
   except Exception:pass
   report=t.result();report['fixtureRequests']=mode['requests'];report['realModelRequests']=0;(a.out/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));t.b.close()
  server.shutdown()
if __name__=='__main__':main()
