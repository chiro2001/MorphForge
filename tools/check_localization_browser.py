#!/usr/bin/env python3
"""Check language switching in real Chromium against an explicitly isolated core."""
import argparse, json, socket, subprocess, time, urllib.request
from pathlib import Path
from check_editor_browser import Browser

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--session',type=Path,required=True);p.add_argument('--out',type=Path,required=True)
    a=p.parse_args();session=json.loads(a.session.read_text(encoding='utf-8-sig'))
    if not any(s in session['data'] for s in ['localization-20261025','/i18n/']):raise ValueError('Isolated localization data required')
    base=session['base'];a.out.mkdir(parents=True,exist_ok=True);checks=[];b=None
    def api(path,value=None):
        req=urllib.request.Request(base+'/api/'+path,data=None if value is None else json.dumps(value).encode(),headers={'Content-Type':'application/json'})
        with urllib.request.urlopen(req,timeout=30) as r:return json.load(r)
    def check(name,value):
        checks.append({'name':name,'passed':bool(value)});print(('PASS 'if value else 'FAIL ')+name,flush=True)
        if not value:raise AssertionError(name)
    initial=api('state');check('explicit isolated project and OSC off',initial['project']['id']==session['projectId'] and not initial['connection']['enabled'])
    sock=socket.socket();sock.bind(('127.0.0.1',0));port=sock.getsockname()[1];sock.close()
    driver=subprocess.Popen(['chromedriver',f'--port={port}','--allowed-ips=127.0.0.1'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
    try:
        for _ in range(50):
            try:b=Browser(f'http://127.0.0.1:{port}');break
            except(OSError,RuntimeError):time.sleep(.1)
        api('project',{'name':'保存 / 日本語 / English','baseRevision':initial['revision']})
        before=api('scene');speech=api('speech')['settings'];prompt_language=api('state')['harness']['settings']['language']
        b.call(b.path+'/url',{'url':base});b.wait('return !!window.__morphforge?.ready',30)
        main_handle=b.call(b.path+'/window');b.js("window.__localeSentinel='same-page';prompt.value='保存 / 日本語 / English';providerKey.value='not-a-real-key';")
        b.click('settingsOpen');b.wait('return !settingsPage.hidden')
        mini=b.call(b.path+'/window/new',{'type':'tab'})['handle'];b.call(b.path+'/window',{'handle':mini});b.call(b.path+'/url',{'url':base+'/mini.html?mode=progress'})
        b.wait("return document.documentElement.lang&&document.getElementById('miniRun').textContent.length>0")
        b.js("window.__miniLocaleSentinel='same-mini'")
        b.call(b.path+'/window',{'handle':main_handle})
        for lang,settings,publish in [('en','Settings','Publish current version'),('ja','設定','現在のバージョンを送信'),('zh-CN','设置','发布当前版本')]:
            b.js("uiLanguage.value=arguments[0];uiLanguage.dispatchEvent(new Event('change',{bubbles:true}))",lang)
            b.wait('return document.documentElement.lang===arguments[0]'.replace('arguments[0]',json.dumps(lang)))
            b.wait('return !uiLanguage.disabled')
            check(lang+' applies immediately without navigation',b.js('return window.__localeSentinel')=='same-page' and b.js('return settingsOpen.textContent')==settings)
            check(lang+' dynamic publishing label',b.js('return publish.textContent')==publish)
            check(lang+' user data and unsaved input preserved',b.js('return prompt.value')=='保存 / 日本語 / English' and b.js('return providerKey.value')=='not-a-real-key' and b.js('return projectName.value')=='保存 / 日本語 / English')
            check(lang+' saved in core preferences',api('ui-settings')['language']==lang)
            missing=b.js("return (async()=>{const {messages}=await import('/locales/messages.js');const lang=document.documentElement.lang;return [...document.querySelectorAll('[data-i18n]')].filter(n=>{const key=n.getAttribute('data-i18n');return n.textContent!==(lang==='zh-CN'?key:messages[key]?.[lang]);}).map(n=>n.getAttribute('data-i18n')).slice(0,10);})()")
            check(lang+' marked interface labels all translated',not missing)
            b.screenshot(a.out/(lang+'-settings.png'))
            b.call(b.path+'/window',{'handle':mini});b.wait('return document.documentElement.lang==='+json.dumps(lang))
            check(lang+' mini synchronizes without reload',b.js('return window.__miniLocaleSentinel')=='same-mini')
            b.call(b.path+'/window',{'handle':main_handle})
        b.call(b.path+'/refresh',{});b.wait('return !!window.__morphforge?.ready',30)
        check('saved manual preference survives reload',b.js('return document.documentElement.lang')=='zh-CN')
        b.js("uiLanguage.value='auto';uiLanguage.dispatchEvent(new Event('change',{bubbles:true}))");b.wait('return !uiLanguage.disabled')
        expected=b.js("return (async()=>{const m=await import('/i18n.js');const s=await(await fetch('/api/ui-settings')).json();return m.resolveLanguage('auto',[...s.systemLanguages,...navigator.languages]);})()")
        b.wait('return document.documentElement.lang==='+json.dumps(expected))
        check('auto follows operating-system language',api('ui-settings')['language']=='auto')
        check('language switching leaves scene, speech translation and prompt language unchanged',api('scene')==before and api('speech')['settings']==speech and api('state')['harness']['settings']['language']==prompt_language)
        state=api('state');check('no microphone, model calls or OSC started',not state['speechStatus']['active'] and state['harness']['interaction']['phase']=='idle' and not state['connection']['enabled'])
        errors=[v for v in b.call(b.path+'/log',{'type':'browser'}) if v['level']=='SEVERE' and 'favicon.ico' not in v['message']]
        check('no JavaScript or WebGL errors',not errors)
    finally:
        if b:b.close()
        driver.terminate()
        (a.out/'validation.json').write_text(json.dumps({'success':bool(checks)and all(c['passed']for c in checks),'checks':checks,'scope':'Real Chromium and isolated packaged HTTP workbench. No paid calls, microphone or OSC.'},ensure_ascii=False,indent=2)+'\n')

if __name__=='__main__':main()
