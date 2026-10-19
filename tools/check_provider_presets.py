#!/usr/bin/env python3
"""Check the real Rust/browser path with a local Chat fixture; never sends OSC.

Run against a disposable, empty MorphForge data directory. Uses ChromeDriver on
9515 and writes only sanitized results/screenshot to the requested output folder.
"""
import argparse
import json
import threading
import time
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from check_editor_browser import Browser

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--url', required=True)
    parser.add_argument('--with-references', action='store_true')
    parser.add_argument('--with-palette', action='store_true')
    parser.add_argument('--out', type=Path, default=ROOT / 'docs/quality-v3')
    args = parser.parse_args()
    requests = []
    paths = []
    test_key = 'mf-local-browser-fixture-only'
    checks = []
    creation_requests = 5 if args.with_palette else 4
    creation_tokens = creation_requests * 1100

    def api(path, body=None):
        request = urllib.request.Request(args.url + '/api/' + path,
            data=None if body is None else json.dumps(body).encode(),
            headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_POST(self):
            paths.append(self.path)
            if self.path != '/chat/completions':
                self.send_error(404)
                return
            body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
            requests.append(body)
            actions = [
                ('context', 'get_context', {}),
                ('build', 'run_recipe', {'mode': 'apply', 'baseRevision': 0, 'code': "mf.group('house',()=>{ mf.box([-8,0,-6],[16,1,12],{style:'stone'}); mf.box([-6,1,-4],[12,9,8],{style:'wood'}); mf.erase([-2,1,3],[4,6,1]); for(let i=0;i<4;i++)mf.box([-8+i,10+i,-6+i],[16-2*i,1,12-2*i],{style:'copper'}); });"}),
                ('capture', 'capture_views', {'revision': 1, 'views': ['hero', 'front']}),
                ('finish', 'finish_artwork', {'title': '兼容接口测试小屋', 'summary': '已通过本机 Chat 接口完成工具循环、真实双视角截图和作品入库。', 'tags': ['fixture', 'house']}),
            ]
            if args.with_palette:
                actions.insert(1, ('palette', 'define_palette', {'baseRevision': 0,
                    'colors': [{'name': '藏蓝', 'color': '#172D59'}]}))
                actions[2][2]['baseRevision'] = 1
                actions[2][2]['code'] = actions[2][2]['code'].replace("style:'copper'", "style:'mf:palette-172d59'")
                actions[3][2]['revision'] = 2
            index = len(requests) - 1
            if index == len(actions) and args.with_references:
                payload=json.dumps({'id':'clear-reference','choices':[{'index':0,'message':{'role':'assistant','content':'已取消视觉参考，继续当前几何。'},'finish_reason':'stop'}],'usage':{'prompt_tokens':1000,'completion_tokens':100,'total_tokens':1100,'prompt_cache_hit_tokens':600,'prompt_cache_miss_tokens':400}}).encode()
                self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(payload)));self.end_headers();self.wfile.write(payload);return
            if index >= len(actions):
                self.send_error(500)
                return
            call_id, name, arguments = actions[index]
            value = {'id': f'fixture-{index}', 'choices': [{'index': 0,
                'message': {'role': 'assistant', 'tool_calls': [{'id': call_id, 'type': 'function',
                    'function': {'name': name, 'arguments': json.dumps(arguments, ensure_ascii=False)}}]},
                'finish_reason': 'tool_calls'}], 'usage': {'prompt_tokens': 1000, 'completion_tokens': 100, 'total_tokens': 1100, 'prompt_cache_hit_tokens': 600, 'prompt_cache_miss_tokens': 400}}
            payload = json.dumps(value, ensure_ascii=False).encode()
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

    def check(name, value):
        checks.append({'name': name, 'passed': bool(value)})
        if not value:
            raise AssertionError(name)

    initial = api('state')
    if initial['stats']['boxCount'] or initial['revision']:
        raise RuntimeError('Use a disposable empty data directory (revision 0)')
    check('OSC remains disabled', not initial['connection']['enabled'])
    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    browser = Browser()
    capture_browser = Browser()
    args.out.mkdir(parents=True, exist_ok=True)
    try:
        capture_browser.call(capture_browser.path + '/url', {'url': args.url + '/?capture=1'})
        capture_browser.wait('return window.__morphforge?.ready', timeout=30)
        browser.call(browser.path + '/url', {'url': args.url})
        browser.wait('return window.__morphforge?.ready', timeout=30)
        for preset, steps, goal in [('detailed', 128, True), ('sketch', 12, False)]:
            browser.js("const e=document.getElementById('preset');e.value=arguments[0];e.dispatchEvent(new Event('change',{bubbles:true}));", preset)
            browser.wait(f"return window.__morphforge.state.harness.settings.maxSteps==={steps}")
            check(f'{preset} preset applies budget and goal', api('harness/state')['settings']['goalMode'] == goal)
        browser.js("document.getElementById('prompt').value='配置页切换保留的草稿'")
        browser.click('settingsOpen')
        browser.wait("return !document.getElementById('settingsPage').hidden && document.getElementById('workbenchMain').hidden")
        check('advanced settings have a dedicated page', browser.js("return document.getElementById('providerBase').closest('#settingsPage')!==null && document.getElementById('goalMode').closest('#settingsPage')!==null"))
        browser.js("document.getElementById('providerConnection').open=true")
        base = f'http://127.0.0.1:{server.server_port}'
        browser.js("const e=document.getElementById('providerBase');e.value=arguments[0];e.dispatchEvent(new Event('input',{bubbles:true}));", base)
        time.sleep(1.5)
        check('provider form draft survives state polling', browser.js("return document.getElementById('providerBase').value") == base)
        browser.js("document.getElementById('providerApi').value='chat';document.getElementById('providerImages').value='inline';document.getElementById('providerStream').checked=false;document.getElementById('providerReasoning').checked=false;document.getElementById('providerKey').value=arguments[0]", test_key)
        check('nonofficial provider hides Files settings', browser.js("return document.getElementById('providerImageSetting').hidden && document.getElementById('providerImages').value==='inline'"))
        browser.click('saveProvider')
        browser.wait("return document.getElementById('providerKey').value===''")
        status = api('harness/state')['provider']
        check('key saved locally and never returned by state', status['keyConfigured'] and test_key not in json.dumps(status))
        browser.js("const e=document.getElementById('model');e.value='fixture-vision';e.dispatchEvent(new Event('change',{bubbles:true}));document.getElementById('providerConnection').open=false")
        browser.wait("return window.__morphforge.state.harness.settings.model==='fixture-vision'")
        browser.click('settingsBack')
        browser.wait("return document.getElementById('settingsPage').hidden && !document.getElementById('workbenchMain').hidden")
        check('settings navigation keeps draft and artwork', browser.js("return document.getElementById('prompt').value==='配置页切换保留的草稿'"))
        if args.with_references:
            browser.js("document.getElementById('referenceComposer').open=true")
            browser.click('referenceOpen')
            browser.wait("return document.getElementById('referenceDialog').open && document.querySelectorAll('.reference-card').length>=8")
            browser.js("document.querySelector('[data-reference-id=\"builtin:chair\"] input').click()")
            browser.click('referenceClose')
            check('built-in reference selection appears in composer',browser.js("return document.getElementById('referenceSummary').textContent.includes('1/6')"))
            element=browser.call(browser.path+'/element',{'using':'css selector','value':'#referenceFiles'})
            browser.call(browser.path+'/element/'+element['element-6066-11e4-a52e-4f735466cecf']+'/value',{'text':str(ROOT/'references/images/pine.png')})
            browser.wait("return document.getElementById('referenceSummary').textContent.includes('2/6') && !document.getElementById('sendMessage').disabled",timeout=20)
            check('current image uploaded and combined with built-in reference',len(api('references')['images'])>=9)
            for id in ['referenceUseLibrary','referenceUseInput']:browser.click(id)
            check('both reference sources can be disabled',browser.js("return document.getElementById('referenceSummary').textContent==='不使用参考图片'"))
            for id in ['referenceUseLibrary','referenceUseInput']:browser.click(id)
            browser.call(browser.path+'/refresh',{})
            browser.wait("return window.__morphforge?.ready && document.getElementById('referenceSummary').textContent.includes('2/6')")
            check('reference choices survive refresh without upstream submission',not requests)
        browser.js("document.getElementById('prompt').value=arguments[0]",'' if args.with_references else '搭建一个小屋并截图检查，最后保存作品。')
        browser.click('sendMessage')
        browser.wait("return window.__morphforge.state.harness.run?.status==='completed'||window.__morphforge.state.harness.run?.status==='failed'", timeout=40)
        state = api('state')
        check('real Rust recipe, capture and finish loop completes', state['harness']['run']['status'] == 'completed')
        check('finish skips unnecessary followup', len(requests) == creation_requests)
        check('geometry created and library saved', state['stats']['boxCount'] > 5 and len(api('artworks')['artworks']) == 1)
        messages = requests[-1]['messages']
        images = [p for m in messages if isinstance(m.get('content'), list) for p in m['content'] if p['type'] == 'image_url']
        check('real screenshots delivered inline without files endpoint', len(images) == (4 if args.with_references else 2) and all(p['image_url']['url'].startswith('data:image/png;base64,') for p in images) and paths == ['/chat/completions'] * creation_requests)
        check('actual provider usage counted', state['harness']['run']['tokens'] == creation_tokens)
        if args.with_palette:
            artwork = api('artworks')['artworks'][0]
            saved = api('artworks/' + artwork['id'])
            palette = next(m for m in saved['project']['materials'] if m['uid'] == 'mf:palette-172d59')
            check('defined RGB palette survives artwork save', all(abs(a-b)<1e-6 for a,b in zip(palette['color'],[23/255,45/255,89/255])))
            check('real geometry uses returned palette material', any(b['styleId']==palette['id'] for b in saved['project']['scene']['boxes']))
            labels = [p.get('text','') for m in messages if isinstance(m.get('content'),list) for p in m['content'] if p['type']=='text']
            check('Chat image blocks retain revision and view labels', any('revision=2' in s and 'view=front' in s for s in labels))
        if args.with_references:
            refs=state['harness']['context']['referenceImages']
            check('selected references reach every model request as images',len(refs)==2 and all('当前用户选中的视觉参考图' in json.dumps(request,ensure_ascii=False) for request in requests))
            artwork=api('artworks')['artworks'][0];saved=api('artworks/'+artwork['id'])
            check('saved artwork records reference provenance',len(saved['referenceImages'])==2)
            browser.js("localStorage.clear()")
            browser.call(browser.path+'/refresh',{})
            browser.wait("return window.__morphforge?.ready && document.getElementById('referenceSummary').textContent.includes('2/6')")
            check('active references recover from backend when browser storage is new',True)

            for id in ['referenceUseLibrary','referenceUseInput']:browser.js("const e=document.getElementById(arguments[0]);e.checked=false;e.dispatchEvent(new Event('change'))",id)
            browser.js("document.getElementById('prompt').value='取消参考，继续当前几何'")
            browser.click('sendMessage')
            browser.wait(f"return window.__morphforge.state.harness.run?.status==='completed' && window.__morphforge.state.harness.usage.total.requests==={creation_requests + 1}",timeout=20)
            old_messages = requests[-2]['messages']
            new_messages = requests[-1]['messages']
            check('clearing references appends instruction while retaining cacheable history',api('harness/state')['context']['referenceImages']==[] and new_messages[:len(old_messages)]==old_messages and '当前视觉参考已取消' in json.dumps(new_messages,ensure_ascii=False))

        displayed_tokens = format(creation_tokens + (1100 if args.with_references else 0), ',')
        browser.wait("return document.getElementById('usageTokens').textContent.includes('"+displayed_tokens+"') && document.getElementById('usagePrefix').textContent.includes('60.0%')")
        check('usage displayed beside composer with weighted cache rate', state['harness']['usage']['total']['totalTokens']==creation_tokens and state['harness']['usage']['total']['prefixHitRate']==0.6)
        check('OSC still disabled after creation' , not state['connection']['enabled'])
        logs = browser.call(browser.path + '/log', {'type': 'browser'})
        errors = [x for x in logs if x['level'] == 'SEVERE' and 'favicon.ico' not in x['message']]
        check('no browser or WebGL errors', not errors)
        browser.screenshot(args.out / 'provider-presets-browser.png')
        report = {'success': True, 'scope': 'Real Rust server + Chromium; local Chat JSON protocol fixture, not a live model aesthetic evaluation', 'checks': checks, 'requests': len(requests), 'run': state['harness']['run'], 'boxCount': state['stats']['boxCount']}
        (args.out / 'provider-presets-browser.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
        print(json.dumps({'success': True, 'checks': len(checks), 'boxCount': report['boxCount']}))
    except Exception as error:
        (args.out / 'provider-presets-browser-failure.json').write_text(json.dumps({'success': False, 'checks': checks, 'error': str(error)},ensure_ascii=False,indent=2)+'\n')
        raise
    finally:
        capture_browser.close()
        browser.close()
        server.shutdown()


if __name__ == '__main__':
    main()
