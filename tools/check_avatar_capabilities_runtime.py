#!/usr/bin/env python3
"""Packaged core + local OSCQuery fixture. No VRChat, OSC UDP, model API or secrets."""
import argparse,json,re,subprocess,threading,urllib.request,urllib.error,hashlib
from pathlib import Path
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer

def main():
 p=argparse.ArgumentParser();p.add_argument('--exe',type=Path,required=True);p.add_argument('--data',type=Path,required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--expected-version',default='0.21.1');p.add_argument('--hash',required=True);a=p.parse_args()
 if a.data.exists() or not re.fullmatch('[0-9a-f]{64}',a.hash):raise RuntimeError('Fresh isolated data and material fingerprint required')
 a.out.mkdir(parents=True,exist_ok=True);checks=[];process=None;flags=119;build=1
 def check(name,value):
  checks.append({'name':name,'passed':bool(value)});print(('PASS 'if value else 'FAIL ')+name,flush=True)
  if not value:raise AssertionError(name)
 class Handler(BaseHTTPRequestHandler):
  def log_message(self,*_):pass
  def do_GET(self):
   if self.path.split('?',1)[0]=='/':v={'OSC_TRANSPORT':'UDP','EXTENSIONS':{'VALUE':True}}
   else:
    values={'MF_InfoVersion':7,'MF_InfoFeatures':flags,'MF_InfoBuild0':build,'MF_InfoBuild1':0,'MF_InfoBuild2':0}
    values.update({f'MF_Catalog{i:02}':int(a.hash[i*2:i*2+2],16)for i in range(32)})
    v={'CONTENTS':{k:{'VALUE':[n]}for k,n in values.items()}}
   data=json.dumps(v).encode();self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
 fixture=ThreadingHTTPServer(('127.0.0.1',0),Handler);threading.Thread(target=fixture.serve_forever,daemon=True).start()
 try:
  process=subprocess.Popen([str(a.exe),'serve','--data',str(a.data),'--port','0','--no-capture'],stdout=subprocess.PIPE,stderr=(a.out/'stderr.txt').open('w'),text=True,encoding='utf-8')
  line=process.stdout.readline();match=re.search(r'http://127\.0\.0\.1:\d+',line)
  if not match:raise RuntimeError('Packaged core failed startup')
  base=match[0]
  def api(path,body=None):
   req=urllib.request.Request(base+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
   with urllib.request.urlopen(req,timeout=30)as r:return json.load(r)
  state=api('state');check('packaged version and no UDP',state['software']['version']==a.expected_version and not state['connection']['enabled'])
  source=Path(__file__).resolve().parents[1]/'web'
  embedded=[]
  for name in ['renderer.js','creative-ui.js','text-panels.js']:
   with urllib.request.urlopen(base+'/'+name,timeout=20)as r:embedded.append(hashlib.sha256(r.read()).hexdigest()==hashlib.sha256((source/name).read_bytes()).hexdigest())
  check('packaged renderer motion and text UI match frozen sources',all(embedded))
  result=api('vrc/discover',{'port':fixture.server_port});check('fresh install recognizes bundled material hash without prior import',result['catalogKnown'] and result['applied'])
  check('full native text placement motion flags accepted',all(result['info'][k]for k in ['region8','textPanels','placementControlsSeparated','motion4']))
  api('edit',{'op':'box','min':[-4,24,-4],'size':[8,8,8],'styleId':1,'group':'rotor'})
  clip={'name':'原生检测测试','duration':1,'looping':True,'tracks':[{'group':'rotor','pivot':[0,28,0],'interpolation':'linear','keys':[{'time':t,'translation':[0,0,0],'rotation':[0,y,0],'scale':[1,1,1]} for t,y in [(0,0),(1,90)]]}]}
  revision=api('creative')['revision'];api('creative/animation',{'action':'save','id':'detected','baseCreativeRevision':revision,'clip':clip})
  published=api('creative/animation',{'action':'publish','id':'detected','playing':False});check('detected port9000 target permits motion',published['motion']['groups'].get('rotor')==1)
  check('publication remains explicit and does not start UDP',not api('state')['connection']['enabled'])
  api('edit',{'op':'box','min':[96,26,-64],'size':[32,8,128],'styleId':1,'group':'rotor'})
  try:api('publish',{});bounded=False
  except urllib.error.HTTPError as e:bounded=e.code==400 and '绑定动画' in e.read().decode()
  check('later geometry edits revalidate attached motion envelope',bounded)
  detached=api('creative/animation',{'action':'detach'});check('detach republishes static draft and clears group tags',not detached['motion']['attached'] and detached['motion']['groups']=={})
  api('edit',{'op':'undo'});republished=api('creative/animation',{'action':'publish','id':'detected','playing':False});check('animation can be published again after detach',republished['motion']['attached'])
  flags=7;build=2;result=api('vrc/discover',{'port':fixture.server_port});check('switch to static-only receiver removes motion',not result['info']['motion4'] and api('state')['connection']['motion']['groups']=={})
  flags=128;build=3
  try:api('vrc/discover',{'port':fixture.server_port});unknown=False
  except urllib.error.HTTPError as e:unknown=e.code==400 and '未知扩展' in e.read().decode()
  check('reserved future capability rejected',unknown)
  check('all checks leave model and OSC inactive',not api('state')['connection']['enabled'])
 finally:
  fixture.shutdown()
  if process:
   process.terminate()
   try:process.wait(timeout=10)
   except subprocess.TimeoutExpired:process.kill()
  (a.out/'validation.json').write_text(json.dumps({'success':bool(checks)and all(c['passed']for c in checks),'version':a.expected_version,'exeSha256':hashlib.sha256(a.exe.read_bytes()).hexdigest(),'checks':checks,'scope':'Packaged Rust core with localhost OSCQuery fixture; no VRChat, no UDP/model requests'},ensure_ascii=False,indent=2))
if __name__=='__main__':main()
