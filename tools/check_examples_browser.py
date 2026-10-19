#!/usr/bin/env python3
"""Actual workbench gallery/copy/edit/save and upstream-limit UI; local fixture only."""
import argparse,json,threading,time,urllib.request
from pathlib import Path
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from check_editor_browser import Browser

def main():
    p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);args=p.parse_args()
    args.out.mkdir(parents=True,exist_ok=True);checks=[];requests=[];reads=[]
    def api(path,value=None):
        r=urllib.request.Request(args.url+'/api/'+path,data=None if value is None else json.dumps(value).encode(),headers={'Content-Type':'application/json'})
        with urllib.request.urlopen(r,timeout=60) as response:return json.load(response)
    def check(name,value):
        checks.append({'name':name,'passed':bool(value)})
        if not value:raise AssertionError(name)
    class Upstream(BaseHTTPRequestHandler):
        def log_message(self,*_):pass
        def do_GET(self):
            reads.append(self.path)
            payload=json.dumps({'data':[{'id':'deepseek-flash','context_window':65536,'max_output_tokens':8192}]}).encode()
            self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(payload)));self.end_headers();self.wfile.write(payload)
        def do_POST(self):
            body=json.loads(self.rfile.read(int(self.headers['Content-Length'])));requests.append(body)
            if len(requests)==1:message={'role':'assistant','tool_calls':[{'id':'context','type':'function','function':{'name':'get_context','arguments':'{}'}}]};finish='tool_calls'
            else:message={'role':'assistant','content':'已检查当前副本，没有修改几何。'};finish='stop'
            payload=json.dumps({'choices':[{'index':0,'message':message,'finish_reason':finish}],'usage':{'prompt_tokens':1000,'completion_tokens':100,'total_tokens':1100}}).encode()
            self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(payload)));self.end_headers();self.wfile.write(payload)
    upstream=ThreadingHTTPServer(('127.0.0.1',0),Upstream);threading.Thread(target=upstream.serve_forever,daemon=True).start()
    b=Browser();report={'success':False,'scope':'Real Rust + Chromium + local metadata/Chat fixture; no real AI request or OSC','checks':checks}
    try:
        initial=api('state');check('empty isolated project',initial['revision']==0)
        b.call(b.path+'/url',{'url':args.url});b.wait('return window.__morphforge?.ready',timeout=30)
        check('fresh defaults expose full context and output',initial['harness']['settings']['contextWindowTokens']==1048576 and initial['harness']['settings']['maxOutputTokens']==393216)
        b.click('libraryOpen');b.wait("return document.getElementById('libraryDialog').open && document.querySelectorAll('[data-origin=\"builtin\"]').length===6",timeout=15)
        check('six offline examples visible',len(api('artworks')['examples'])==6 and api('state')['revision']==0)
        check('every example thumbnail loaded',b.wait("return [...document.querySelectorAll('#libraryGrid img')].every(image=>image.complete&&image.naturalWidth>0)",timeout=15))
        b.screenshot(args.out/'builtin-library.png')
        b.js("document.querySelector('[data-artwork-id=\"builtin:whale-maid\"] button:last-child').click()")
        b.wait("return !document.getElementById('libraryDialog').open && window.__morphforge.state.stats.boxCount===223",timeout=20)
        check('load creates editable revision without model calls',api('state')['revision']==1 and not requests)
        before=api('artworks/builtin:whale-maid')
        edited=api('edit',{'baseRevision':1,'operations':[{'op':'box','min':[32,0,32],'size':[1,1,1],'styleId':1,'group':'my-detail'}]})
        b.wait("return window.__morphforge.state.stats.boxCount===224",timeout=15)
        b.click('saveArtwork')
        deadline=time.monotonic()+30
        while not api('artworks')['artworks']:
            if time.monotonic()>deadline:raise TimeoutError('save button did not save')
            time.sleep(.2)
        saved=api('artworks')['artworks'][0];copy=api('artworks/'+saved['id'])
        check('modified copy saves actual views and sources',saved['boxCount']==224 and len(saved['views'])==3 and copy['project']['sourceCredits']==before['project']['sourceCredits'])
        check('builtin original remains 223 boxes',len(api('artworks/builtin:whale-maid')['project']['scene']['boxes'])==223)
        b.click('libraryOpen');b.wait("return document.querySelectorAll('[data-origin=\"user\"]').length===1 && document.querySelectorAll('[data-origin=\"builtin\"]').length===6")
        check('personal and builtin works coexist',b.js("return document.getElementById('libraryGrid').textContent.includes('CC BY-NC-SA 4.0')"))
        b.click('libraryClose');b.click('settingsOpen')
        check('settings inputs retain full configured ceilings',b.js("return document.getElementById('maxOutputTokens').value==='393216' && document.getElementById('contextWindowTokens').value==='1048576'"))
        api('provider/config',{'baseUrl':f'http://127.0.0.1:{upstream.server_port}','apiType':'chat','apiKey':'mf-examples-fixture','stream':False,'sendReasoning':False})
        api('provider/models')
        b.wait("return document.getElementById('modelLimitsInfo').textContent.includes('65,536') && document.getElementById('modelLimitsInfo').textContent.includes('8,192')")
        check('upstream limits shown without overwriting defaults',api('harness/state')['settings']['maxOutputTokens']==393216)
        b.screenshot(args.out/'upstream-limits.png')
        api('harness/message',{'prompt':'只检查当前副本，不改几何。','referenceIds':[]})
        deadline=time.monotonic()+30
        while api('harness/state')['run']['status'] not in ('completed','failed'):
            if time.monotonic()>deadline:raise TimeoutError('fixture run incomplete')
            time.sleep(.2)
        state=api('state')
        check('requests honor advertised 8192 output limit',len(requests)==2 and all(r.get('max_tokens')==8192 for r in requests))
        check('metadata discovered once',reads==['/models'])
        previous=requests[0]['messages'];check('prefix remains append-only',requests[1]['messages'][:len(previous)]==previous)
        check('OSC stays disabled and copy retained',not state['connection']['enabled'] and state['stats']['boxCount']==224)
        logs=b.call(b.path+'/log',{'type':'browser'});errors=[x for x in logs if x['level']=='SEVERE' and 'favicon.ico' not in x['message']]
        check('no browser or WebGL errors',not errors)
        report.update(success=True,checks=checks,boxCount=224,builtinCount=6,providerRequests=len(requests),modelReads=len(reads))
    except Exception as e:
        report['error']=str(e);b.screenshot(args.out/'failure.png');raise
    finally:
        (args.out/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');b.close();upstream.shutdown();upstream.server_close()
    print(json.dumps({'success':True,'checks':len(checks)}))
if __name__=='__main__':main()
