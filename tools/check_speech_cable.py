#!/usr/bin/env python3
"""Windows Cable acceptance on a NEW data directory; no VRChat OSC.

SAPI fixtures -> explicit CABLE Input playback -> native CABLE Output capture.
Only the optional --online test uses Tencent (one short utterance). Keyword
submission goes to an in-process fake LLM, never the user's model API.
"""
import argparse,base64,json,subprocess,threading,time,wave,urllib.request,urllib.error,uuid,statistics
from pathlib import Path
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer

def main():
 p=argparse.ArgumentParser();p.add_argument('--exe',type=Path,required=True);p.add_argument('--data',type=Path,required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--online',action='store_true');a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
 if a.data.exists():raise RuntimeError('Acceptance data directory must be new')
 checks=[];rows=[];report={'success':False};process=None
 def check(name,value):
  checks.append({'name':name,'passed':bool(value)});print(('PASS 'if value else 'FAIL ')+name,flush=True)
  if not value:raise AssertionError(name)
 requests=[]
 class Handler(BaseHTTPRequestHandler):
  def log_message(self,*_):pass
  def do_GET(self):
   body=json.dumps({'data':[{'id':'fixture-model','context_window':1000000,'max_output_tokens':4096}]}).encode();self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
  def do_POST(self):
   body=json.loads(self.rfile.read(int(self.headers['Content-Length'])));requests.append(body)
   frame={'type':'response.completed','response':{'id':'fixture-'+str(len(requests)),'status':'completed','output':[{'type':'message','role':'assistant','content':[{'type':'output_text','text':'关键词提交已接收，仅用于测试。'}]}],'usage':{'input_tokens':100,'output_tokens':10,'total_tokens':110,'input_tokens_details':{'cached_tokens':80}}}}
   data=('data: '+json.dumps(frame,ensure_ascii=False)+'\n\n').encode();self.send_response(200);self.send_header('Content-Type','text/event-stream');self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
 fixture=ThreadingHTTPServer(('127.0.0.1',0),Handler);threading.Thread(target=fixture.serve_forever,daemon=True).start()
 def wait(fn,seconds=20):
  deadline=time.monotonic()+seconds
  while time.monotonic()<deadline:
   v=fn()
   if v:return v
   time.sleep(.05)
  raise TimeoutError('Condition timed out')
 try:
  process=subprocess.Popen([str(a.exe),'serve','--data',str(a.data),'--port','0','--no-capture'],stdout=subprocess.PIPE,stderr=(a.out/'core-stderr.log').open('w'),text=True,encoding='utf-8')
  line=process.stdout.readline();import re
  match=re.search(r'http://127\.0\.0\.1:\d+',line)
  if not match:raise RuntimeError('Core failed startup')
  url=match[0]
  def api(path,data=None):
   req=urllib.request.Request(url+'/api/'+path,data=None if data is None else json.dumps(data,ensure_ascii=False).encode(),headers={'Content-Type':'application/json'})
   try:
    with urllib.request.urlopen(req,timeout=30)as f:return json.load(f)
   except urllib.error.HTTPError as e:raise RuntimeError(json.load(e).get('error','HTTP failure'))from None
  state=api('state');check('v0.19 isolated core and OSC disabled',state['software']['version']=='0.19.0'and not state['connection']['enabled'])
  api('provider/config',{'baseUrl':f'http://127.0.0.1:{fixture.server_port}','apiType':'responses','apiKey':'fixture-local-only','imageDelivery':'inline','stream':True})
  api('harness/config',{'model':'fixture-model','cacheGuard':'observe','strictAppendOnly':True,'maxSteps':2,'goalMode':False})
  import numpy as np,sounddevice as sd
  capture=next((d['name']for d in api('speech/devices')['devices']if 'CABLE Output'in d['name']),None);check('native Cable capture device exists',bool(capture))
  outputs=[(i,d)for i,d in enumerate(sd.query_devices())if 'CABLE Input'in d['name']and d['max_output_channels']>0]
  outputs.sort(key=lambda x:'WASAPI'not in sd.query_hostapis(x[1]['hostapi'])['name']);usable=[]
  for candidate,device in outputs:
   try:sd.check_output_settings(device=candidate,samplerate=16000,channels=1,dtype='float32');usable.append((candidate,device))
   except Exception:pass
  check('explicit Cable playback device exists',bool(usable));output=usable[0][0]
  api('speech/config',{'device':capture,'provider':'offline','language':'zh','keywordEnabled':False,'autoCaption':False,'threads':2})
  check('embedded VAD and reused local models ready',all(api('speech')['models'][k]for k in ['vad','senseVoice','zipformer']))
  texts=['这是一条语音识别测试。','建造一座红色屋顶的小房子。','窗户在左边，门在右边。','开始建模，建造一座红色屋顶的小房子。','不要开始建模。']
  files=[]
  for i,text in enumerate(texts):
   dest=a.out/f'fixture-{i}.wav';script=a.out/f'synthesize-{i}.ps1';quoted=str(dest.resolve()).replace("'","''")
   script.write_text("$ErrorActionPreference='Stop';Add-Type -AssemblyName System.Speech\n$s=[System.Speech.Synthesis.SpeechSynthesizer]::new()\n$v=$s.GetInstalledVoices() | Where-Object {$_.VoiceInfo.Culture.Name -eq 'zh-CN'} | Select-Object -First 1\nif(-not $v){throw 'Chinese SAPI voice missing'}\n$s.SelectVoice($v.VoiceInfo.Name)\n$f=[System.Speech.AudioFormat.SpeechAudioFormatInfo]::new(16000,[System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen,[System.Speech.AudioFormat.AudioChannel]::Mono)\n$s.SetOutputToWaveFile('"+quoted+"',$f)\n$s.Speak('"+text.replace("'","''")+"')\n$s.Dispose()\n",encoding='utf-8-sig')
   r=subprocess.run(['powershell','-NoProfile','-File',str(script)],capture_output=True)
   if r.returncode:raise RuntimeError('Fixture generation failed: '+r.stderr.decode(errors='replace'))
   with wave.open(str(dest))as f:check(f'fixture {i} format',f.getframerate()==16000 and f.getnchannels()==1 and f.getsampwidth()==2);samples=np.frombuffer(f.readframes(f.getnframes()),dtype='<i2').astype(np.float32)/32768
   files.append(samples)
  def clean(t):return ''.join(c for c in t if c.isalnum())
  def cer(expected,actual):
   x,y=clean(expected),clean(actual);d=list(range(len(y)+1))
   for i,c in enumerate(x,1):
    nxt=[i]
    for j,k in enumerate(y,1):nxt.append(min(nxt[-1]+1,d[j]+1,d[j-1]+(c!=k)))
    d=nxt
   return d[-1]/max(1,len(x))
  def run_case(mode,index,target=None):
   api('speech/config',{'provider':mode});v=api('speech/start',{'source':'device','target':target or {'kind':'modeling'}});session=v['state']['sessionId']
   def recording_ready():
    state=api('speech')['state']
    if state.get('error'):raise RuntimeError('Recording startup failed: '+state['error'])
    return state['phase']=='recording'
   wait(recording_ready,30)
   last={};done=threading.Event();started=time.perf_counter()
   def monitor():
    while not done.wait(.03):
     s=api('speech')['state']
     if s['sessionId']!=session:return
     if s.get('partial')and'partialTime'not in last:last['partialTime']=time.perf_counter()
     if s['sequence']and'finalTime'not in last:last['finalTime']=time.perf_counter();last['state']=s
   t=threading.Thread(target=monitor,daemon=True);t.start()
   samples=files[index];audio=np.concatenate([np.zeros(4800,np.float32),samples,np.zeros(16000,np.float32)])
   playback=time.perf_counter();sd.play(audio,16000,device=output);sd.wait()
   wait(lambda:'finalTime'in last or api('speech')['state'].get('error'),15)
   status=api('speech')['state'];done.set();t.join(2)
   api('speech/control',{'action':'stop'});wait(lambda:not api('speech')['state']['active'],15)
   status=api('speech')['state'];check(f'{mode} fixture {index} final result',status['sequence']>=1 and not status.get('error'))
   final=''.join(x['text']for x in status['sentences']);accuracy=cer(texts[index],final)
   nz=np.flatnonzero(np.abs(samples)>.005);speech_end=playback+.3+((int(nz[-1])+1)/16000 if len(nz)else len(samples)/16000)
   row={'provider':mode,'fixture':index,'expected':texts[index],'recognized':final,'cer':accuracy,'loadingMs':status['metrics'].get('loadingMs'),'warmModel':status['metrics'].get('warmModel'),'firstPartialMs':(last['partialTime']-playback)*1000 if'partialTime'in last else None,'finalAfterEstimatedSpeechEndMs':(last.get('finalTime',time.perf_counter())-speech_end)*1000,'inferenceMs':[x.get('inferenceMs')for x in status['sentences']],'receiverEstimatedLatencyMs':[x.get('finalLatencyMs')for x in status['sentences']],'cloudSessions':status['metrics'].get('cloudSessions',0),'cloudAudioSeconds':status['metrics'].get('cloudAudioSamples',0)/16000}
   rows.append(row);check(f'{mode} fixture {index} intelligible',accuracy<=.35);return row
  for mode in ['offline','hybrid']:
   for i in range(3):run_case(mode,i)
  api('speech/config',{'provider':'offline','keywordEnabled':True})
  run_case('offline',3);wait(lambda:len(requests)>=1,10);check('final keyword submits exactly once to local fixture',len(requests)==1);wait(lambda:api('state')['harness']['interaction']['phase']not in ['running','compacting'],10)
  run_case('offline',4);time.sleep(.3);check('negative keyword never submits',len(requests)==1)
  api('speech/config',{'keywordEnabled':False,'autoCaption':True})
  state=api('state');api('text-panels',{'action':'create','id':'cable-caption','baseRevision':state['revision'],'panel':{'text':'','name':'Cable验收字幕'}})
  run_case('offline',0,{'kind':'text_panel','panelId':'cable-caption'});panel=api('text-panels')['panels'][0]['panel'];check('voice final updates selected text object',cer(texts[0],panel['text'])<=.35)
  revision=api('state')['revision'];time.sleep(.6);check('polling never repeats text mutations',api('state')['revision']==revision)
  api('speech/config',{'autoCaption':False,'provider':'tencent','credentials':{'appId':'123456','secretId':'silence-unused','secretKey':'silence-unused'}})
  api('speech/start',{'source':'device','target':{'kind':'modeling'}});wait(lambda:api('speech')['state']['phase']=='recording',10);sd.play(np.zeros(32000,np.float32),16000,device=output);sd.wait();api('speech/control',{'action':'cancel'});wait(lambda:not api('speech')['state']['active'],10);check('native Cable silence opens no cloud session',api('speech')['state']['metrics']['cloudSessions']==0)
  if a.online:
   api('speech/config',{'importReference':True});run_case('tencent',0)
  summary={}
  for mode in ['offline','hybrid']:
   cohort=[r for r in rows if r['provider']==mode and r['fixture']<3];lat=sorted(r['finalAfterEstimatedSpeechEndMs']for r in cohort);summary[mode]={'samples':len(cohort),'medianFinalMs':statistics.median(lat),'maxFinalMs':max(lat),'meanCER':statistics.mean(r['cer']for r in cohort),'partialResults':sum(r['firstPartialMs']is not None for r in cohort)}
  preferred='offline'if summary['hybrid']['maxFinalMs']>summary['offline']['maxFinalMs']+250 or summary['hybrid']['partialResults']<3 else'hybrid'
  # User preference: prioritize dependable final sentences; benchmark is a small
  # cohort, not a claim of universal latency or full language coverage.
  api('speech/config',{'provider':'offline','keywordEnabled':False,'autoCaption':False})
  check('no real VRChat OSC and no real LLM requests',not api('state')['connection']['enabled']and len(requests)==1)
  report={'success':True,'checks':checks,'rows':rows,'summary':summary,'preferredBySmallCohort':preferred,'defaultProvider':'offline','llmFixtureRequests':len(requests),'scope':'Physical Windows Cable loopback with local SAPI fixtures. Main install and default audio devices unchanged. Optional one short Tencent utterance only.'}
 except Exception as e:
  report={'success':False,'checks':checks,'rows':rows,'error':str(e)}
  if 'api'in locals():
   try:
    state=api('speech')['state'];report['lastSpeechState']={k:state.get(k)for k in ['phase','active','error','metrics','provider','source']}
   except Exception:pass
  raise
 finally:
  (a.out/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8');fixture.shutdown()
  if process:process.terminate();process.wait(timeout=10)
if __name__=='__main__':main()
