#!/usr/bin/env python3
"""Run the full creative tool loop against a local Responses SSE provider.

Validates actual harness dispatch, same-conversation reuse, capture rendering and
append-only wire input. Usage numbers are synthetic, not an upstream cache test.
"""
import argparse, json, threading, time, urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from check_editor_browser import Browser


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--webdriver',default='http://127.0.0.1:9519');a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
    checks=[];requests=[];worker=None;provider=None
    def api(path,body=None):
        r=urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
        with urllib.request.urlopen(r,timeout=90) as response:return json.load(response)
    def check(name,ok):
        checks.append({'name':name,'passed':bool(ok)});print(('PASS ' if ok else 'FAIL ')+name,flush=True)
        if not ok:raise AssertionError(name)
    def call(index):
        state=api('state');creative=api('creative');base=state['revision'];assets=creative['revision']
        if index==0:return 'get_context',{'styleLimit':1}
        if index==1:return 'run_recipe',{'baseRevision':base,'mode':'replace','code':"mf.box([-10,0,-4],[20,1,8],{style:1,group:'base'});mf.box([-2,1,0],[4,6,1],{style:2,group:'door'});"}
        if index==2:return 'components',{'action':'save','id':'fixture-door','name':'重复门','groups':['door'],'baseRevision':base,'baseCreativeRevision':assets}
        if index==3:return 'components',{'action':'place','id':'fixture-door','baseRevision':base,'instances':[{'offset':[-7,1,-3],'group':'door-left'},{'offset':[7,1,-3],'group':'door-right','turns':1}]}
        if index==4:return 'components',{'action':'inspect','id':'fixture-door'}
        if index==5:return 'animation',{'action':'save','id':'fixture-open','baseCreativeRevision':assets,'clip':{'name':'开门','duration':2,'looping':True,'tracks':[{'group':'door','pivot':[-2,1,0],'interpolation':'linear','keys':[{'time':0,'translation':[0,0,0],'rotation':[0,0,0],'scale':[1,1,1]},{'time':2,'translation':[0,2,0],'rotation':[0,100,0],'scale':[1,1,1]}]}]}}
        if index==6:
            images=api('references')['images'];image=next((i for i in images if 'MorphForge' in i.get('author','')),images[0])
            return 'reference_workbench',{'action':'save','id':'fixture-front','baseCreativeRevision':assets,'bookmark':{'name':'匹配正面','shot':{'projection':'orthographic','yaw':0,'pitch':0},'comparison':{'imageId':image['id'],'mode':'split','opacity':.5,'zoom':1,'offset':[0,0],'crop':[0,0,1,1]}}}
        if index==7:return 'reference_workbench',{'action':'capture','bookmarkId':'fixture-front','compare':True,'width':960,'height':640}
        if index==8:return 'capture_views',{'animation':{'clipId':'fixture-open','time':1},'views':['hero'],'width':768,'height':640}
        return None
    class Upstream(BaseHTTPRequestHandler):
        def log_message(self,*_):pass
        def do_GET(self):
            body=json.dumps({'data':[{'id':'creative-fixture','context_window':1048576,'max_output_tokens':393216,'input_modalities':['text','image']}]}).encode();self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
        def do_POST(self):
            body=json.loads(self.rfile.read(int(self.headers['Content-Length'])));index=len(requests);requests.append(body);action=call(index)
            output=[{'type':'function_call','call_id':'creative-'+str(index),'name':action[0],'arguments':json.dumps(action[1],ensure_ascii=False)}] if action else [{'type':'message','role':'assistant','content':[{'type':'output_text','text':'本地夹具已完成组件复用、参考对照与动画姿态检查。'}]}]
            self.send_response(200);self.send_header('Content-Type','text/event-stream');self.end_headers()
            for event in [{'type':'response.reasoning_summary_text.delta','delta':'本地测试工具循环，不调用真实模型。'},{'type':'response.completed','response':{'status':'completed','output':output,'usage':{'input_tokens':20000,'output_tokens':100,'total_tokens':20100,'input_tokens_details':{'cached_tokens':19000}}}}]:
                self.wfile.write(('data: '+json.dumps(event,ensure_ascii=False)+'\n\n').encode());self.wfile.flush()
    report={'success':False,'checks':checks,'realModelRequests':0,'scope':'Local synthetic SSE provider + actual Rust harness + Chromium image worker; no OSC. Cache metadata is synthetic.'}
    try:
        check('fixture output cannot reach a real Avatar',not api('state')['connection']['enabled'])
        worker=Browser(a.webdriver);worker.call(worker.path+'/url',{'url':a.url+'/?capture'});worker.wait('return window.__morphforge?.ready',30)
        provider=ThreadingHTTPServer(('127.0.0.1',0),Upstream);threading.Thread(target=provider.serve_forever,daemon=True).start()
        api('provider/config',{'baseUrl':f'http://127.0.0.1:{provider.server_port}','apiType':'responses','stream':True,'apiKey':'creative-local-only'})
        api('harness/config',{'model':'creative-fixture','maxSteps':16,'goalMode':False,'cacheGuard':'strict','maxRunTokens':1000000,'maxRunSeconds':120,'sendImages':True,'uploadImages':False})
        before=api('harness/state')['context']['id'];api('harness/start',{'prompt':'测试同一对话保存并重复使用组件、建立参考对照书签、制作并截图分组动画。'})
        end=time.monotonic()+120
        while time.monotonic()<end:
            state=api('harness/state')
            if state['run']['status'] in ['completed','failed','paused','stopped']:break
            time.sleep(.15)
        check('harness reaches completion through all nine actual tool calls',state['run']['status']=='completed' and len(requests)==10)
        check('component reuse stays inside one conversation',state['context']['id']==before)
        groups=api('state')['stats']['groups'];check('saved component is placed into two new groups',all(g in groups for g in ['door-left','door-right']))
        context=api('harness/tool',{'name':'get_context','arguments':{'styleLimit':1}});check('context discovers named assets without embedding component geometry',any(c['id']=='fixture-door' for c in context['creative']['components']) and 'scene' not in context['creative']['components'][0])
        check('tool schemas stay stable across provider requests',all(r['tools']==requests[0]['tools'] for r in requests))
        check('every subsequent wire context is strictly append-only',all(now['input'][:len(old['input'])]==old['input'] and len(now['input'])>len(old['input']) for old,now in zip(requests,requests[1:])))
        outputs=[i for r in requests for i in r['input'] if i.get('type')=='function_call_output'];check('tool loop reports no tool failure',all(not (isinstance(i.get('output'),str) and '"ok":false' in i['output']) for i in outputs))
        timeline=api('timeline?limit=200')['events'];captures=[e for e in timeline if e['kind']=='capture.completed'];check('comparison and frozen-motion captures enter real history',any(any(i.get('comparison') for i in e['body'].get('images',[])) for e in captures) and any(any(i.get('animationTime')==1 for i in e['body'].get('images',[])) for e in captures))
        check('new geometry placement receives an injected call-id requestId',any(e['kind']=='geometry.applied' and e['body'].get('requestId')=='creative-3' for e in timeline))
        report['success']=True;report['requests']=len(requests);report['wirePrefixAudit']=state['cacheMonitor']['prefix'];report['assets']=api('creative')
    except Exception as e:report['error']=str(e);raise
    finally:
        try:api('harness/control',{'action':'stop'})
        except Exception:pass
        if worker:worker.close()
        if provider:provider.shutdown()
        (a.out/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')

if __name__=='__main__':main()
