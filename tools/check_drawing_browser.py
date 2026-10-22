#!/usr/bin/env python3
"""Exercise demonstration geometry through real Chromium clicks, OSC disabled."""
import argparse
import json
from pathlib import Path
import socket
import subprocess
import time
import urllib.request
from check_editor_browser import Browser


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--url', required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    checks, browser = [], None

    def api(path, body=None):
        request = urllib.request.Request(args.url + '/api/' + path,
            data=None if body is None else json.dumps(body).encode(),
            headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)

    def check(name, passed):
        checks.append({'name': name, 'passed': bool(passed)})
        print(('PASS ' if passed else 'FAIL ') + name, flush=True)
        if not passed:
            raise AssertionError(name)

    sock = socket.socket()
    sock.bind(('127.0.0.1', 0))
    port = sock.getsockname()[1]
    sock.close()
    driver = subprocess.Popen(['chromedriver', f'--port={port}', '--allowed-ips=127.0.0.1'],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        check('isolated workspace, OSC off', not api('state')['connection']['enabled'])
        original = api('state')['revision']
        for _ in range(50):
            try:
                browser = Browser(f'http://127.0.0.1:{port}')
                break
            except (OSError, RuntimeError):
                time.sleep(.1)
        if browser is None:
            raise RuntimeError('ChromeDriver unavailable')
        browser.call(browser.path + '/url', {'url': args.url})
        browser.wait('return window.__morphforge?.ready', 30)
        browser.click('drawingOpen')
        browser.wait('return drawingDialog.open')
        check('preview initially requires an explicit action', browser.js('return drawApply.disabled'))
        browser.click('drawPreview')
        browser.wait('return !drawApply.disabled', 30)
        check('point preview uses actual canvas and reports triangles', browser.js('return !!drawingViewport.querySelector("canvas") && drawMetrics.textContent.includes("面")'))
        check('preview preserves authoritative revision', api('state')['revision'] == original)
        browser.js('drawSize.value="14";drawSize.dispatchEvent(new Event("input",{bubbles:true}));')
        check('changing point size invalidates old preview', browser.js('return drawApply.disabled'))
        browser.click('drawPreview')
        browser.wait('return !drawApply.disabled', 30)
        browser.click('drawApply')
        browser.wait('return !drawingDialog.open', 30)
        first = api('scene')['scene']['drawings']
        check('point size changes saved geometric extent', len(first)==1 and first[0]['diameter64']==14*64)
        browser.click('drawingOpen')
        browser.js('drawKind.value="segment";drawKind.dispatchEvent(new Event("input",{bubbles:true}));drawWidth.value="6";drawWidth.dispatchEvent(new Event("input",{bubbles:true}));')
        check('line controls expose width and endpoints', browser.js('return !drawWidth.closest("[data-draw-field]").hidden && !drawStart.closest("[data-draw-field]").hidden && drawSize.closest("[data-draw-field]").hidden'))
        browser.click('drawPreview')
        browser.wait('return !drawApply.disabled', 30)
        browser.click('drawApply')
        browser.wait('return !drawingDialog.open', 30)
        check('adding a segment preserves the point', len(api('scene')['scene']['drawings']) > len(first))
        browser.click('drawingOpen')
        browser.click('drawExample')
        browser.wait('return !drawApply.disabled && drawMetrics.textContent.includes("组合演示")', 30)
        browser.screenshot(args.out / 'drawing-preview.png')
        check('combination preview reports all six primitives', browser.js('return drawMetrics.textContent.includes("6个图元")'))
        browser.click('drawReplace')
        check('replacement mode invalidates old preview', browser.js('return drawApply.disabled'))
        browser.click('drawExample')
        browser.wait('return !drawApply.disabled', 30)
        browser.click('drawApply')
        browser.wait('return !drawingDialog.open', 30)
        scene = api('scene')['scene']
        check('combination becomes one editable group with separate colors', {t['group'] for t in scene.get('triangles',[])+scene.get('drawings',[])} == {'demo'} and len({tuple(t['color']) for t in scene.get('triangles',[])+scene.get('drawings',[])}) >= 5)
        check('main camera frames the newly loaded combination', browser.js('''return (async()=>{const {Vector3}=await import('three');const r=window.__morphforge.renderer,b=r.bounds();return [0,1,2,3,4,5,6,7].every(i=>{const p=new Vector3(i&1?b.max.x:b.min.x,i&2?b.max.y:b.min.y,i&4?b.max.z:b.min.z).project(r.camera);return Math.abs(p.x)<=1.03&&Math.abs(p.y)<=1.03&&p.z>=-1&&p.z<=1;});})()'''))
        browser.screenshot(args.out / 'drawing-scene.png')
        browser.click('drawingOpen')
        browser.js('drawStart.value="0,24,0";drawEnd.value="0,24,0";drawStart.dispatchEvent(new Event("input",{bubbles:true}));')
        browser.click('drawPreview')
        browser.wait('return !drawError.hidden', 30)
        check('degenerate lines show an actionable error', browser.js('return drawApply.disabled && drawError.textContent.includes("重合")'))
        browser.js('drawingDialog.close()')
        check('closing disposes the extra preview canvas', browser.js('return !drawingViewport.querySelector("canvas")'))
        check('all operations keep OSC and paid model requests inactive', not api('state')['connection']['enabled'] and api('state')['harness']['interaction']['phase'] == 'idle')
        logs = browser.call(browser.path + '/log', {'type': 'browser'})
        (args.out / 'browser-logs.json').write_text(json.dumps(logs, ensure_ascii=False, indent=2))
        errors = [entry for entry in logs if entry['level'] == 'SEVERE' and 'favicon.ico' not in entry['message'] and '400 (Bad Request)' not in entry['message']]
        check('no unhandled JavaScript or WebGL errors', not errors)
    finally:
        report = {'success': bool(checks) and all(c['passed'] for c in checks), 'checks': checks,
            'scope': 'Actual Chromium/WebDriver and Rust geometry API; isolated data, no microphone/model/VRChat OSC.'}
        (args.out / 'validation.json').write_text(json.dumps(report, ensure_ascii=False, indent=2))
        if browser:
            browser.close()
        driver.terminate()


if __name__ == '__main__':
    main()
