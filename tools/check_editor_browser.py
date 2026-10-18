#!/usr/bin/env python3
"""Exercise the actual browser UI with W3C WebDriver mouse input, using local ChromeDriver."""
import argparse,base64,json,time,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
class Browser:
    def __init__(self,url='http://127.0.0.1:9515'):
        self.base=url
        value=self.call('/session',{'capabilities':{'alwaysMatch':{'browserName':'chrome','goog:chromeOptions':{'args':['--headless=new','--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader','--use-angle=swiftshader','--window-size=1500,1000']},'goog:loggingPrefs':{'browser':'ALL'}}}})
        self.sid=value['sessionId'];self.path='/session/'+self.sid
    def call(self,path,data=None,method=None):
        req=urllib.request.Request(self.base+path,data=None if data is None else json.dumps(data).encode(),headers={'Content-Type':'application/json'},method=method or ('POST' if data is not None else 'GET'))
        with urllib.request.urlopen(req,timeout=40) as r:value=json.load(r)['value']
        if isinstance(value,dict) and 'error' in value:raise RuntimeError(value)
        return value
    def js(self,source,*args):return self.call(self.path+'/execute/sync',{'script':source,'args':list(args)})
    def wait(self,js,timeout=10):
        deadline=time.monotonic()+timeout
        while time.monotonic()<deadline:
            value=self.js(js)
            if value:return value
            time.sleep(.1)
        raise AssertionError('Timed out: '+js)
    def click(self,identifier):
        el=self.call(self.path+'/element',{'using':'css selector','value':'#'+identifier})
        self.call(self.path+'/element/'+el['element-6066-11e4-a52e-4f735466cecf']+'/click',{})
    def point(self,p,shift=False):
        q=self.js('return window.__morphforge.projectPoint(arguments[0])',p)
        if shift:self.call(self.path+'/actions',{'actions':[{'type':'key','id':'keyboard','actions':[{'type':'keyDown','value':'\ue008'}]}]})
        self.call(self.path+'/actions',{'actions':[{'type':'pointer','id':'mouse','parameters':{'pointerType':'mouse'},'actions':[{'type':'pointerMove','duration':100,'x':round(q['x']),'y':round(q['y'])},{'type':'pointerDown','button':0},{'type':'pointerUp','button':0}]}]})
        if shift:self.call(self.path+'/actions',{'actions':[{'type':'key','id':'keyboard','actions':[{'type':'keyUp','value':'\ue008'}]}]})
    def screenshot(self,path):Path(path).write_bytes(base64.b64decode(self.call(self.path+'/screenshot')))
    def close(self):self.call(self.path,method='DELETE')
def main():
    p=argparse.ArgumentParser();p.add_argument('--url',default='http://127.0.0.1:18765');a=p.parse_args()
    out=ROOT/'docs/editor';out.mkdir(parents=True,exist_ok=True);b=Browser();checks=[]
    def check(name,condition):
        checks.append({'name':name,'pass':bool(condition)})
        if not condition:raise AssertionError(name)
    try:
        b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready')
        check('Default OSC disabled',not b.js('return window.__morphforge.state.connection.enabled'))
        b.click('clearScene');b.wait('return window.__morphforge.state.voxelCount===0')
        b.point([.125,0,.125]);b.wait('return window.__morphforge.state.voxelCount===1')
        check('Real pointer adds grid voxel',b.js('return window.__morphforge.state.document.boxes[0].min.join()')=='128,0,128')
        b.point([.125,.25,.125]);b.wait('return window.__morphforge.state.voxelCount===2')
        check('Adjacent voxels merge into one box',b.js('return window.__morphforge.state.boxCount')==1)
        b.point([.125,.5,.125],shift=True);b.wait('return window.__morphforge.state.voxelCount===1')
        check('Shift pointer removes top voxel',b.js('return window.__morphforge.state.document.boxes[0].size.join()')=='1,1,1')
        b.click('undo');b.wait('return window.__morphforge.state.voxelCount===2');b.click('redo');b.wait('return window.__morphforge.state.voxelCount===1');check('Undo and redo buttons work',True)
        b.click('stairsDemo');b.wait('return window.__morphforge.state.voxelCount===144');check('Stairs demo uses eight cuboids',b.js('return window.__morphforge.state.boxCount')==8)
        b.screenshot(out/'stairs-browser.png')
        b.click('houseDemo');b.wait('return window.__morphforge.state.voxelCount===438');check('House includes exact doorway subtraction',b.js('return window.__morphforge.state.boxCount')==16)
        b.screenshot(out/'house-browser.png')
        logs=b.call(b.path+'/log',{'type':'browser'});errors=[x for x in logs if x['level']=='SEVERE' and 'favicon.ico' not in x['message']]
        check('No browser JavaScript or WebGL errors',not errors)
        report={'success':True,'checks':checks,'browserLogs':logs,'scope':'Real Chromium/WebDriver clicks against Windows editor service; OSC remains disabled'}
    except Exception as e:
        b.screenshot(out/'browser-failure.png');report={'success':False,'checks':checks,'error':str(e)};raise
    finally:
        (out/'browser-validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2));b.close()
    print(json.dumps(report,ensure_ascii=False,indent=2))
if __name__=='__main__':main()
