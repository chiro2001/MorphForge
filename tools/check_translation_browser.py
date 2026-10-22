#!/usr/bin/env python3
"""Real browser and real core against a local fake LLM; no microphone/paid calls/OSC."""
import argparse,json,socket,subprocess,threading,time,urllib.request
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from pathlib import Path
from check_editor_browser import Browser
p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True);requests=[]
class LLM(BaseHTTPRequestHandler):
 def log_message(self,*args):pass
 def do_GET(self):
  self.send_response(200);self.send_header("Content-Type","application/json");self.end_headers();self.wfile.write(b'{"data":[{"id":"deepseek-flash"}]}')
 def do_POST(self):
  body=json.loads(self.rfile.read(int(self.headers['Content-Length'])));requests.append(body);source=body['input'][0]['content'][0]['text'];system=body['instructions'];text='你好，世界。'if'Simplified Chinese'in system else'こんにちは、世界。'if'Japanese'in system else'Hello, world.'
  fail=source=='error fixture';self.send_response(429 if fail else 200);self.send_header('Content-Type','application/json');self.end_headers();self.wfile.write(json.dumps({'error':{'message':'fixture-only-secret'}}if fail else{'status':'completed','output':[{'type':'message','role':'assistant','content':[{'type':'output_text','text':text}]}],'usage':{'input_tokens':20,'output_tokens':8}}).encode())
server=ThreadingHTTPServer(('127.0.0.1',0),LLM);threading.Thread(target=server.serve_forever,daemon=True).start()
def api(path,value=None):
 req=urllib.request.Request(a.url+'/api/'+path,data=None if value is None else json.dumps(value).encode(),headers={'Content-Type':'application/json'})
 with urllib.request.urlopen(req,timeout=30)as r:return json.load(r)
s=socket.socket();s.bind(('127.0.0.1',0));port=s.getsockname()[1];s.close();driver=subprocess.Popen(['chromedriver',f'--port={port}','--allowed-ips=127.0.0.1'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);b=None;checks=[]
def check(name,condition):
 checks.append({'name':name,'passed':bool(condition)});print(('PASS 'if condition else'FAIL ')+name,flush=True)
 if not condition:raise AssertionError(name)
def wait(fn):
 end=time.monotonic()+15
 while time.monotonic()<end:
  if fn():return
  time.sleep(.05)
 raise TimeoutError('translation result')
try:
 check('isolated workspace and OSC disabled',not api('state')['connection']['enabled'])
 api('provider/config',{'baseUrl':f'http://127.0.0.1:{server.server_port}','apiType':'responses','apiKey':'local-browser-fixture'})
 state=api('state');api('text-panels',{'action':'create','id':'translation-caption','preset':'caption','baseRevision':state['revision']})
 for _ in range(50):
  try:b=Browser(f'http://127.0.0.1:{port}');break
  except(OSError,RuntimeError):time.sleep(.1)
 b.call(b.path+'/url',{'url':a.url+'/mini.html'});b.wait("return [...speechTarget.options].some(o=>o.value==='panel:translation-caption')",30)
 b.js("speechTarget.value='panel:translation-caption';speechTarget.dispatchEvent(new Event('change'));")
 b.wait('return !speechTranslate.disabled',10)
 check('mini offers Chinese Japanese and English',b.js("return [...speechTranslationLanguage.options].map(o=>o.value).join(',')==='zh,ja,en'"))
 b.click('speechTranslate');wait(lambda:api('speech')['settings']['translationEnabled'])
 for i,(lang,result)in enumerate([('ja','こんにちは、世界。'),('zh','你好，世界。'),('en','Hello, world.')]):
  b.js('speechTranslationLanguage.value=arguments[0];speechTranslationLanguage.dispatchEvent(new Event("change"));',lang);wait(lambda:api('speech')['settings']['translationLanguage']==lang)
  b.js("speechText.value=arguments[0];speechText.dispatchEvent(new Event('input'));",'hello world'if i<2 else'你好，世界')
  b.click('speechSend');wait(lambda:api('scene')['scene']['textPanels'][0]['text']==result)
  check('target '+lang+' uses actual model API and persisted subtitle',api('speech')['state']['translation']['language']==lang)
  check('source remains editable for '+lang,b.js('return speechText.value')==('hello world'if i<2 else'你好，世界'))
 b.wait("return speechTranslation.textContent.includes('Hello, world.')",10);b.screenshot(a.out/'mini-translation.png')
 b.js("speechText.value='error fixture';speechText.dispatchEvent(new Event('input')); ");b.click('speechSend');wait(lambda:api('speech')['state']['translation']['phase']=='error')
 check('API failure preserves original without exposing secrets',api('scene')['scene']['textPanels'][0]['text']=='error fixture'and'fixture-only-secret'not in json.dumps(api('speech')))
 b.wait("return speechError.textContent.includes('429')",10)
 b.click('speechTranslate');wait(lambda:not api('speech')['settings']['translationEnabled']);count=len(requests)
 b.js("speechText.value='原文输出';speechText.dispatchEvent(new Event('input'))");b.click('speechSend');wait(lambda:api('scene')['scene']['textPanels'][0]['text']=='原文输出');check('disabled translation sends original without API request',len(requests)==count)
 b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready',30);b.click('settingsOpen')
 b.js("const n=document.getElementById('voiceTranslationModel').closest('details');n.open=true;");b.wait("return voiceTranslationLanguage.value==='en'",10)
 b.js("voiceTranslationModel.value='caption-fixture-model'");b.click('voiceSave');wait(lambda:api('speech')['settings']['translationModel']=='caption-fixture-model')
 check('main settings persist a dedicated translation model',True)
 check('translation never starts modeling or microphone',api('state')['harness']['interaction']['phase']=='idle'and not api('speech')['state']['active']and not api('state')['connection']['enabled'])
 logs=b.call(b.path+'/log',{'type':'browser'});check('no browser JavaScript error',not[v for v in logs if v['level']=='SEVERE'and'favicon.ico'not in v['message']])
finally:
 (a.out/'validation.json').write_text(json.dumps({'success':bool(checks)and all(c['passed']for c in checks),'checks':checks,'apiRequests':len(requests),'scope':'Real Chromium controls -> packaged core -> local fake Responses API -> persisted subtitle. No paid requests, microphone or OSC.'},ensure_ascii=False,indent=2))
 if b:b.close()
 driver.terminate();server.shutdown()
