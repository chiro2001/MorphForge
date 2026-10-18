#!/usr/bin/env python3
"""Independent MF3 UDP -> native GM/FX/particles checks, no C# emission shortcut."""
import argparse,json,sys,base64,time
from pathlib import Path
from check_stage2 import pipeline,remote_file
from unity_remote import run_pwsh,pwsh_literal,DEFAULT_PROJECT
ROOT=Path(__file__).resolve().parents[1]
def entry(name):
 r=pipeline('run_script','--file','AgentScripts/Stage4Entry.cs','--entry','Stage4Entry.'+name,'--timeout_ms','60000')
 if not r.get('success'):raise RuntimeError(r)
 return json.loads(r['result'])
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--interval',type=float,default=.25);parser.add_argument('--output',type=Path,default=ROOT/'docs/stage4/protocol-validation.json');options=parser.parse_args()
 if not .2<=options.interval<=2:parser.error('interval must be between 0.2 and 2 seconds')
 options.output.parent.mkdir(parents=True,exist_ok=True)
 out=ROOT/'docs/stage4';out.mkdir(parents=True,exist_ok=True);seq=1;serial=0;checks=[];captures=[];completed=0
 def check(name,value):
  checks.append({'name':name,'pass':bool(value)});print(('PASS 'if value else'FAIL ')+name,flush=True)
  if not value:raise AssertionError(name)
 def send(commands,reuse=None):
  nonlocal seq,serial,completed
  serial+=1;path=f'Docs/Stage4/probe-{serial}.json';remote_file(path,commands)
  args=['tools/send_mf3.py','--input',path,'--seq',str(seq if reuse is None else reuse),'--interval',str(options.interval)]
  r=run_pwsh('Set-Location -LiteralPath '+pwsh_literal(DEFAULT_PROJECT)+"; & python "+' '.join(pwsh_literal(a)for a in args)+'; exit $LASTEXITCODE',capture_output=True)
  if r.returncode:raise RuntimeError(r.stderr.decode(errors='replace'))
  sent=json.loads(r.stdout.decode('utf-8-sig'))
  if reuse is None:seq=sent['nextSeq'];completed+=len(commands)
 def capture(label,boxes):
  # A command is counted as received before the native particle lifecycle
  # reaches Done. Poll the product Store instead of sampling one instant
  # before its last Animator evaluation.
  s=None
  for _ in range(60):
   s=entry('Capture')
   if s['received']==completed and s['completed']==completed and not s['busy'] and not s['inbox']:
    break
   time.sleep(.1)
  s['label']=label;captures.append(s);check(label+' count',s['count']==len(boxes));check(label+' every command completed',s['received']==completed and s['completed']==completed and not s['busy'] and not s['inbox'])
  actual=s['particles'].copy()
  for b in boxes:
   center=[(v+extent/2-128)*.25 for v,extent in zip(b['min'],b['size'])]
   p=next((p for p in actual if max(abs(a-z)for a,z in zip(p['position'],center))<.001),None);check(label+' native center '+str(b['min']),p is not None)
   check(label+' native size',max(abs(a-z*.25)for a,z in zip(p['size'],b['size']))<.001);check(label+' native metadata',p['codes']==[b['styleId']&63,b['styleId']>>6,b.get('tintId',0),b.get('motionPhase',0)]);actual.remove(p)
  return s
 def box(pos,size=[1,1,1],style=1,tint=0,motion=0):return{'min':pos,'size':size,'styleId':style,'tintId':tint,'motionPhase':motion}
 try:
  check('Play receiver auto started',entry('Receiver')['Running'])
  initial=entry('Capture');check('New MF3 starts with zero commands',initial['received']==0)
  send([{'op':'clear'}]);capture('clear',[])
  a=box([128,0,128],[2,3,4],4095,63,63);send([{'op':'brush','styleId':4095,'tintId':63,'motionPhase':63},{'op':'box',**a}]);capture('brush-max',[a])
  b=box([133,1,128],style=2,motion=5);send([{'op':'brush','styleId':2,'motionPhase':5},{'op':'voxel',**b,'unitSourceSizes':[255,254,128]}]);capture('brush-preserves-old-and-voxel-forces-size',[a,b])
  send([{'op':'box','min':a['min'],'size':a['size']}]);a['styleId']=2;a['tintId']=0;a['motionPhase']=5;capture('overwrite-no-duplicate',[a,b])
  duplicate=(seq-1)%16;send([{'op':'clear'}],reuse=duplicate);capture('duplicate-sequence-ignored',[a,b])
  send([{'op':'remove',**a}]);capture('remove-exact',[b])
  send([{'op':'erase','min':[132,0,127],'size':[3,4,3]}]);capture('erase-region',[])
  send([{'op':'clear'}]);capture('clear-retains-brush',[])
  values=[box([i*3,0,128],style=2,motion=5)for i in range(40)];send([{'op':'voxel',**b}for b in values]);capture('sequence-wrap-40',values)
  send([{'op':'clear'}]);capture('finish-clear',[])
  report={'success':True,'interval':options.interval,'scope':'Actual independent Windows UDP9012 -> automatic GM -> MF3 native FX -> native particles','checks':checks,'captures':captures,'commands':completed}
 except Exception as e:report={'success':False,'error':str(e),'checks':checks,'captures':captures};raise
 finally:options.output.write_text(json.dumps(report,ensure_ascii=False,indent=2))
if __name__=='__main__':main()
