#!/usr/bin/env python3
"""Exercise real Rust requests through a byte-transparent relay and local upstream.

Use an empty disposable backend with its capture renderer enabled. No real model
requests, credentials, OSC traffic or private-network access are involved.
Optional Qwen HF tokenizer files also verify the text Chat-template token prefix.
"""
import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import threading
import time
import urllib.request
import uuid

from prefix_proxy import running_proxy


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--url', required=True)
    parser.add_argument('--api', choices=['chat', 'responses'], required=True)
    parser.add_argument('--images', action='store_true')
    parser.add_argument('--stream', action='store_true')
    parser.add_argument('--tokenizer-dir', type=Path)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    checks = []
    received = []
    returned = []
    api_origin = args.url.rstrip('/')

    def api(path, data=None):
        request = urllib.request.Request(api_origin + '/api/' + path,
            data=None if data is None else json.dumps(data).encode(),
            headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(request, timeout=90) as response:
            return json.load(response)

    def check(name, passed):
        checks.append({'name': name, 'passed': bool(passed)})
        if not passed:
            raise AssertionError(name)

    def terminal():
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline:
            state = api('harness/state')
            if state['run'] and state['run']['status'] in ('completed', 'failed', 'stopped', 'blocked'):
                check('run completes', state['run']['status'] == 'completed')
                return state
            time.sleep(.15)
        raise TimeoutError('Harness did not complete')

    actions = [
        ('ctx', 'get_context', {}),
        ('build', 'run_recipe', {'mode':'replace','baseRevision':0,'seed':7,
            'code':"mf.box([-4,0,-3],[8,6,6],{style:'wood'});mf.erase([-1,0,2],[2,4,1]);"}),
        ('view1', 'capture_views', {'revision':1,'views':['front','hero']}),
        ('refine', 'run_recipe', {'mode':'apply','baseRevision':1,'seed':7,
            'code':"mf.box([-5,6,-4],[10,1,8],{style:'blue'});"}),
        ('view2', 'capture_views', {'revision':2,'views':['front','hero']}),
        ('finish', 'finish_artwork', {'title':'Prefix probe house','summary':'Real geometry and captures; protocol fixture only.'}),
    ]

    class Upstream(BaseHTTPRequestHandler):
        def log_message(self, *_args):
            pass
        def do_POST(self):
            raw = self.rfile.read(int(self.headers['Content-Length']))
            body = json.loads(raw)
            received.append(hashlib.sha256(raw).hexdigest())
            index = len(received) - 1
            usage = {'prompt_tokens':100,'completion_tokens':10,'total_tokens':110,
                     'prompt_tokens_details':{'cached_tokens':0}}
            if index < len(actions):
                call_id, name, arguments = actions[index]
                chat_message = {'role':'assistant','tool_calls':[{'id':call_id,'type':'function',
                    'function':{'name':name,'arguments':json.dumps(arguments)}}]}
                output = [{'type':'function_call','call_id':call_id,'name':name,
                           'arguments':json.dumps(arguments)}]
            elif index == 9:
                chat_message = {'role':'assistant','tool_calls':[{'id':'post-compact','type':'function',
                    'function':{'name':'get_context','arguments':'{}'}}]}
                output = [{'type':'function_call','call_id':'post-compact','name':'get_context','arguments':'{}'}]
            else:
                text = '保留作品和目标的简洁摘要。' if index == 8 else '本阶段结束。'
                chat_message = {'role':'assistant','content':text}
                output = [{'type':'message','role':'assistant','content':[{'type':'output_text','text':text}]}]
            if args.api == 'chat':
                finish = 'tool_calls' if 'tool_calls' in chat_message else 'stop'
                value = {'id':f'fixture-{index}','choices':[{'index':0,'message':chat_message,
                         'finish_reason':finish}],'usage':usage}
                if body.get('stream'):
                    delta = dict(chat_message)
                    if 'tool_calls' in delta:
                        delta['tool_calls'] = [dict(call,index=i) for i,call in enumerate(delta['tool_calls'])]
                    frames = [{'id':f'fixture-{index}','choices':[{'index':0,'delta':delta,'finish_reason':None}]},
                              {'choices':[{'index':0,'delta':{},'finish_reason':finish}],'usage':usage}]
                    payload = ''.join('data: '+json.dumps(frame)+'\n\n' for frame in frames).encode()+b'data: [DONE]\n\n'
                else:
                    payload = json.dumps(value).encode()
            else:
                value = {'id':f'fixture-{index}','status':'completed','output':output,
                         'usage':{'input_tokens':100,'output_tokens':10,'total_tokens':110,
                                  'input_tokens_details':{'cached_tokens':0}}}
                payload = (('data: '+json.dumps({'type':'response.completed','response':value})+'\n\n').encode()
                           if body.get('stream') else json.dumps(value).encode())
            returned.append(hashlib.sha256(payload).hexdigest())
            self.send_response(200)
            self.send_header('Content-Type','text/event-stream' if body.get('stream') else 'application/json')
            self.send_header('Content-Length',str(len(payload)));self.end_headers()
            self.wfile.write(payload)

    args.out.mkdir(parents=True,exist_ok=True)
    initial = api('state')
    check('disposable empty scene and OSC disabled',initial['revision']==0 and not initial['connection']['enabled'])
    upstream = ThreadingHTTPServer(('127.0.0.1',0),Upstream)
    thread = threading.Thread(target=upstream.serve_forever,daemon=True);thread.start()
    report = {'success':False,'api':args.api,'images':args.images,'stream':args.stream,'checks':checks}
    try:
        with running_proxy(f'http://127.0.0.1:{upstream.server_port}',args.tokenizer_dir) as (proxy, recorder):
            api('provider/config',{'baseUrl':proxy,'apiType':args.api,'apiKey':'mf-prefix-fixture',
                                   'imageDelivery':'inline','stream':args.stream,'sendReasoning':False})
            api('harness/config',{'preset':'standard','reasoning':'none','sendImages':args.images,
                                 'autoCompact':False,'maxSteps':12,'maxRunTokens':250000})
            refs = ['builtin:chair'] if args.images else []
            api('harness/message',{'prompt':'搭建一座小屋，截图修正后保存。','referenceIds':refs,'requestId':str(uuid.uuid4())})
            terminal()
            check('real Recipe/capture/finish loop ran',len(received)==6 and len(api('artworks')['artworks'])==1)
            api('harness/message',{'prompt':'继续当前作品，不改几何。','referenceIds':refs})
            terminal()
            api('harness/message',{'prompt':'取消参考，继续当前作品。','referenceIds':[]})
            terminal()
            before_compact = recorder.report()['requests']
            check('eight actual requests captured',len(before_compact)==8)
            check('every ordinary wire prefix appends',all(row.get('comparison',{}).get('appendOnly') for row in before_compact[1:]))
            check('request bytes unchanged by relay',[row['requestSha256'] for row in before_compact]==received)
            if args.images:
                check('old captures remain and references are not duplicated', [row['images'] for row in before_compact]==[1,1,1,3,3,5,5,5])
            elif args.api=='chat' and args.tokenizer_dir:
                pairs=[row.get('template') for row in before_compact[1:]]
                check('official text template inspected for all consecutive requests',all(pairs))
                check('rendered text and token IDs preserve complete prefix',all(row['appendOnlyText'] and row['tokenPrefixUnchanged'] for row in pairs))
            api('harness/control',{'action':'compact'});terminal()
            api('harness/message',{'prompt':'compact后继续检查，不改几何。','referenceIds':[]});terminal()
            rows=recorder.report()['requests']
            check('compact is the explicit reset boundary',len(rows)==11 and not rows[8]['comparison']['appendOnly'] and not rows[9]['comparison']['appendOnly'])
            check('new context resumes append-only',rows[10]['comparison']['appendOnly'])
            check('all request bodies were forwarded unchanged',[row['requestSha256'] for row in rows]==received)
            check('upstream JSON or SSE bytes reached relay output unchanged',[row.get('responseSha256') for row in rows]==returned)
            state=api('state')
            check('OSC remains disabled',not state['connection']['enabled'])
            report.update({'success':True,'relay':recorder.report(),'upstreamRequestSha256':received,
                           'upstreamResponseSha256':returned,'boxCount':state['stats']['boxCount'],
                           'compactCount':state['harness']['context']['compactCount']})
    except Exception as error:
        report['error']=str(error)
        if 'recorder' in locals():
            report['relay']=recorder.report()
        report['upstreamRequestSha256']=received
        report['upstreamResponseSha256']=returned
        raise
    finally:
        upstream.shutdown();upstream.server_close();thread.join()
        (args.out/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({'success':True,'checks':len(checks),'requests':len(received)}))


if __name__=='__main__':
    main()
