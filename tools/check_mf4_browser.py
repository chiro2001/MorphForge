#!/usr/bin/env python3
"""Exercise MF4 UI and catalogue gates in an isolated workspace, leaving OSC off."""
import argparse,json,urllib.request,urllib.error
from pathlib import Path
from check_editor_browser import Browser

def main():
    p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args()
    def api(path,body=None):
        return json.load(urllib.request.urlopen(urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})))
    checks=[];b=Browser()
    def check(name,passed):
        checks.append({'name':name,'passed':bool(passed)});print(('PASS ' if passed else 'FAIL ')+name,flush=True)
        assert passed,name
    try:
        b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready',30)
        check('fresh isolated workspace starts with OSC off',not api('state')['connection']['enabled'])
        api('vrc/profile',{'action':'clear'});api('connect',{'enabled':False,'profile':'mf3'})
        b.js('document.querySelector("button[data-tab=sync]").click()');b.wait('return !document.getElementById("tab-sync").hidden')
        b.click('avatarCatalogBundled');b.wait('return window.__morphforge.state.connection.profile==="mf4"',15)
        s=api('state');check('real catalogue button selects MF4 without sending',s['connection']['profile']=='mf4' and not s['connection']['enabled'] and s['materialCompatibility']['matched'])
        check('actual geometry bit budget is displayed',s['connection']['syncedBits']==240)
        try:api('connect',{'profile':'mf3','enabled':True});blocked=False
        except urllib.error.HTTPError as e:blocked='PROTOCOL_MISMATCH' in e.read().decode()
        check('wrong wire protocol is rejected before OSC',blocked)
        try:api('connect',{'profile':'mf4','capacity':16385});blocked=False
        except urllib.error.HTTPError as e:blocked='MF4_CAPACITY' in e.read().decode()
        check('pool capacity cannot be overcommitted',blocked)
        api('edit',{'op':'box','min':[0,0,0],'size':[3,3,3],'styleId':1});api('publish',{})
        check('offline publish remains independent of VRC',not api('state')['connection']['enabled'])
        b.call(b.path+'/refresh',{});b.wait('return window.__morphforge?.ready',20)
        check('catalogue and profile persist through reload',b.js('return window.__morphforge.state.connection.profile==="mf4" && window.__morphforge.state.materialCompatibility.matched'))
        logs=b.call(b.path+'/log',{'type':'browser'});check('no JavaScript or shader errors',not any(x['level']=='SEVERE' for x in logs))
        a.out.parent.mkdir(parents=True,exist_ok=True);a.out.write_text(json.dumps({'success':True,'checks':checks,'scope':'Real Chromium controls; OSC remains off; no upstream model calls.'},ensure_ascii=False,indent=2))
    finally:b.close()
if __name__=='__main__':main()
