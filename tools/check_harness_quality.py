#!/usr/bin/env python3
"""Real Rust/Chromium integration with a LOCAL fixture; no paid model or OSC."""
import argparse,json,threading,time,urllib.request
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from pathlib import Path
from check_editor_browser import Browser

def main():
 p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--webdriver',default='http://127.0.0.1:9516');p.add_argument('--out',type=Path,required=True);a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
 checks=[];requests=[];actions=[];zero=False;fixture_key='mf-quality-local-fixture-key';lock=threading.Lock()
 def api(path,body=None):
  request=urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
  with urllib.request.urlopen(request,timeout=90) as response:return json.load(response)
 def check(name,passed):
  checks.append({'name':name,'passed':bool(passed)})
  if not passed:raise AssertionError(name)
 def tool(name,args):return api('harness/tool',{'name':name,'arguments':args})
 def terminal():
  end=time.monotonic()+40
  while time.monotonic()<end:
   s=api('harness/state')
   if s.get('run',{}).get('status') in ['completed','paused','failed','stopped']:return s
   time.sleep(.1)
  raise TimeoutError('fixture run timed out')
 class Upstream(BaseHTTPRequestHandler):
  def log_message(self,*_):pass
  def do_GET(self):
   payload=json.dumps({'data':[{'id':'deepseek-flash','context_window':1048576,'max_output_tokens':393216}]}).encode();self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(payload)));self.end_headers();self.wfile.write(payload)
  def do_POST(self):
   body=json.loads(self.rfile.read(int(self.headers['Content-Length'])));
   with lock:
    index=len(requests);requests.append(body);action=actions.pop(0) if actions else None;cached=0 if zero else 19000
   output=[] if not action else [{'type':'function_call','call_id':f'quality-{index}','name':action[0],'arguments':json.dumps(action[1],ensure_ascii=False)}]
   thought='测试上游实际返回的思考片段 '+('<img src=x onerror="window.__timelineInjected=true">' if index==0 else '')
   if index==0:thought+='观察 ' * 1500
   self.send_response(200);self.send_header('Content-Type','text/event-stream');self.end_headers()
   try:
    self.wfile.write(('data: '+json.dumps({'type':'response.reasoning_summary_text.delta','delta':thought})+'\n\n').encode());self.wfile.flush()
    if index==0:time.sleep(2)
    value={'status':'completed','output':output,'usage':{'input_tokens':20000,'output_tokens':100,'total_tokens':20100,'input_tokens_details':{'cached_tokens':cached}}}
    self.wfile.write(('data: '+json.dumps({'type':'response.completed','response':value})+'\n\n').encode());self.wfile.flush()
   except (BrokenPipeError,ConnectionResetError):pass
 upstream=ThreadingHTTPServer(('127.0.0.1',0),Upstream);threading.Thread(target=upstream.serve_forever,daemon=True).start();b=Browser(a.webdriver);capture=Browser(a.webdriver);report={'success':False,'scope':'Real Rust + Chromium; LOCAL fixture reasoning only, no paid generation or OSC','checks':checks}
 try:
  initial=api('state');check('isolated empty scene and OSC disabled',initial['revision']==0 and not initial['connection']['enabled'])
  capture.call(capture.path+'/url',{'url':a.url+'/?capture=1'});capture.wait('return window.__morphforge?.ready',timeout=30)
  b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready',timeout=30)
  helper='module.exports=(mf,p)=>mf.ellipsoid([0,12,0],[p.radius,8,5],{style:3});'
  main="const body=require('body.js'); module.exports=(mf,p)=>{mf.replaceGroup('body',()=>body(mf,p));mf.replaceGroup('face',()=>{mf.box([-4,14,5],[2,2,1],{style:1});mf.box([2,14,5],[2,2,1],{style:1});});mf.replaceGroup('marker',()=>mf.box([16,0,0],[1,4,1],{style:2}));};"
  h=tool('recipe_sources',{'action':'write','path':'body.js','baseRevision':0,'content':helper});tool('recipe_sources',{'action':'write','path':'main.js','baseRevision':0,'content':main})
  result=tool('recipe_sources',{'action':'run','path':'main.js','baseRevision':0,'mode':'replace','params':{'radius':10}})
  check('virtual CommonJS modules execute through actual recipe worker',result['revision']==1 and api('state')['stats']['boxCount']>10)
  tool('recipe_sources',{'action':'run','path':'main.js','baseRevision':1,'params':{'radius':11},'dryRun':True});check('dry run retains current revision',api('state')['revision']==1)
  tool('recipe_sources',{'action':'patch','path':'body.js','baseRevision':1,'expectedHash':h['file']['hash'],'edits':[{'find':'p.radius','replace':'(p.radius)'}]})
  combined=tool('recipe_sources',{'action':'run','path':'main.js','baseRevision':1,'mode':'apply','params':{'radius':12},'capture':{'views':['hero','front']}})
  check('draw and capture share one tool result',combined['revision']==2 and len(combined['images'])==2)
  check('source edits and parameter changes continue the project',api('state')['revision']==2 and len(tool('recipe_sources',{'action':'list'})['files'])==2)
  section=tool('inspect_region',{'group':'body','slice':{'axis':'y','index':12},'limit':128});check('slice inspection returns real one-voxel sections',section['sample'] and all(x['min'][1]==12 and x['size'][1]==1 for x in section['sample']))
  shots=[{'id':'front-wide','projection':'orthographic','yaw':0,'pitch':0},{'id':'body-close','fit':{'group':'body'},'projection':'perspective','yaw':25,'pitch':8}]
  views=tool('capture_views',{'revision':2,'shots':shots,'width':900,'height':700})
  check('custom shots are distinct and contain camera receipts',len(views['images'])==2 and views['images'][0]['camera']['projection']=='orthographic' and views['images'][1]['camera']['bounds']['max'][0]==12)
  mixed=tool('capture_views',{'revision':2,'views':['hero','front'],'shots':[{'id':'hero','fit':{'group':'body'},'projection':'orthographic','yaw':25,'pitch':8},{'id':'fin-detail','fit':{'group':'face'},'projection':'perspective','yaw':0,'pitch':0}]})
  check('mixed presets and custom shots override aliases without changing later projection',len(mixed['images'])==3 and mixed['images'][0]['camera']['projection']=='orthographic' and mixed['images'][1]['camera']['projection']=='perspective')
  for img in views['images']:
   (a.out/(img['view']+'.png')).write_bytes(urllib.request.urlopen(a.url+img['url']).read())
  historic=tool('capture_views',{'revision':1,'views':['hero']})
  historical_rows=[row for row in api('timeline')['events'] if row['kind']=='capture.completed' and row['body'].get('revision')==1]
  check('historical capture timeline targets the rendered revision without restoring it',historical_rows and historical_rows[-1]['revision']==1 and historical_rows[-1]['recordedRevision']==2 and api('state')['revision']==2)
  crop=tool('read_artifact',{'artifact':views['images'][0]['artifact'],'crop':{'x':200,'y':100,'width':400,'height':400}})
  check('image crop is labeled reinspection without scene edits',crop['imageRole']=='reinspection' and api('state')['revision']==2)
  api('provider/config',{'baseUrl':f'http://127.0.0.1:{upstream.server_port}','apiType':'responses','apiKey':fixture_key,'stream':True})
  api('harness/config',{'cacheGuard':'strict','maxSteps':10,'maxRunTokens':250000,'maxOutputTokens':4096,'goalMode':False})
  actions[:]=[('get_context',{}),('capture_views',{'revision':2,'shots':shots,'width':900,'height':700}),('finish_artwork',{'title':'Local quality fixture','summary':'本地协议验证，不是真实模型生成。'})]
  b.click('timelineToggle');api('harness/message',{'prompt':'本地fixture检查，不增加几何。'})
  b.wait("return [...document.querySelectorAll('.timeline-card')].some(e=>e.textContent.includes('测试上游实际返回的思考片段'))",timeout=15)
  check('reasoning is visible while request is still streaming',api('harness/state')['run']['status']=='running')
  b.screenshot(a.out/'timeline-streaming.png')
  end=terminal();check('fixture finishes and saves artwork',end['run']['status']=='completed' and len(api('artworks')['artworks'])==1)
  b.wait("return [...document.querySelectorAll('.timeline-card')].some(e=>e.textContent.includes('真实截图'))",timeout=15)
  check('upstream hit rate is separate from prefix checks',end['cacheMonitor']['recentHitRate']==.95 and end['cacheMonitor']['prefix']['appendOnly'])
  check('ordinary outgoing context is append only',all(requests[i]['input'][:len(requests[i-1]['input'])]==requests[i-1]['input'] for i in range(1,len(requests))))
  check('model text is rendered safely without HTML execution',not b.js('return !!window.__timelineInjected') and b.js("return !document.querySelector('#events img[src=x]')"))
  check('known provider key is redacted',fixture_key not in json.dumps(api('timeline'),ensure_ascii=False))
  before=api('scene');b.js("document.querySelector('.timeline-card button').click()")
  b.wait("return document.getElementById('timelinePreview').open",timeout=10);time.sleep(.5)
  check('historical preview does not edit current project',api('scene')==before)
  b.screenshot(a.out/'timeline-history.png');b.click('timelinePreviewClose')
  b.screenshot(a.out/'timeline-complete.png')
  source=(Path(__file__).resolve().parents[1]/'crates/morphforge-app/tests/fixtures/quality-r1-coral-whale-main.js').read_text()
  bad='const modules='+json.dumps({'main.js':source})+";const params={},cache={};function require(name){if(cache[name])return cache[name].exports;const module={exports:{}};cache[name]=module;new Function('mf','params','module','exports','require',modules[name])(mf,params,module,module.exports,require);return module.exports;}const entry=require('main.js');entry(mf,params);"
  actions[:]=[('run_recipe',{'code':bad,'mode':'apply','baseRevision':2,'seed':1}),('get_context',{})];before_fault=len(requests)
  api('harness/message',{'prompt':'本地fixture模拟执行器内部崩溃，请勿继续调用模型。'})
  fault=terminal();check('engine fault pauses without a paid-model retry',fault['run']['status']=='paused' and fault['run']['error'].startswith('RECIPE_ENGINE_FAULT') and len(requests)-before_fault==1)
  b.screenshot(a.out/'engine-fault-paused.png');api('harness/control',{'action':'stop'})
  zero=True;actions[:]=[('get_context',{})]*4;before_requests=len(requests);api('harness/message',{'prompt':'本地零命中保护用例，只查询状态。'})
  stopped=terminal();check('established cache loss pauses before another request',stopped['run']['status']=='paused' and len(requests)-before_requests==1)
  check('cached-zero warning is always visible beside input',b.wait("return !document.getElementById('cacheShowConfig').hidden",timeout=10))
  check('partial/final reasoning and tools remain queryable',any(x['kind']=='provider.reasoning_delta' for x in api('timeline')['events']))
  b.screenshot(a.out/'cache-guard-paused.png')
  check('OSC remains off',not api('state')['connection']['enabled'])
  severe=[x for x in b.call(b.path+'/log',{'type':'browser'}) if x['level']=='SEVERE' and 'favicon.ico' not in x['message']];check('no browser or WebGL errors',not severe)
  report.update(success=True,requests=len(requests),boxCount=api('state')['stats']['boxCount'])
 except Exception as error:
  report['error']=str(error);b.screenshot(a.out/'failure.png');raise
 finally:
  (a.out/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');b.close();capture.close();upstream.shutdown();upstream.server_close()
 print(json.dumps({'success':True,'checks':len(checks)}))
if __name__=='__main__':main()
