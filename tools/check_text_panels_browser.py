#!/usr/bin/env python3
"""Production WebGL text: mixed widths, real controls and readable back faces."""
import argparse,base64,json,socket,subprocess,time,urllib.request
from pathlib import Path
from check_editor_browser import Browser

def main():
 p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--out',type=Path,required=True);a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
 sock=socket.socket();sock.bind(('127.0.0.1',0));port=sock.getsockname()[1];sock.close();driver=subprocess.Popen(['chromedriver',f'--port={port}','--allowed-ips=127.0.0.1'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);b=None;checks=[]
 def check(name,value):
  checks.append({'name':name,'passed':bool(value)});print(('PASS 'if value else 'FAIL ')+name,flush=True)
  if not value:raise AssertionError(name)
 def api(path,body=None):
  req=urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
  with urllib.request.urlopen(req,timeout=30)as r:return json.load(r)
 try:
  for _ in range(50):
   try:b=Browser(f'http://127.0.0.1:{port}');break
   except(OSError,RuntimeError):time.sleep(.1)
  b.call(b.path+'/url',{'url':a.url});b.wait('return window.__morphforge?.ready',30)
  check('OSC disabled',not api('state')['connection']['enabled'])
  cleared=api('edit',{'op':'clear'});b.wait('return window.__morphforge.state.revision==='+str(cleared['revision']))
  b.js('document.querySelector("[data-tab=text]").click()');b.click('panelTextMode');b.click('panelCreate')
  b.wait('return window.__morphforge.renderer.textPanels.items.size===1',30)
  scene=api('scene');panel=scene['scene']['textPanels'][0]
  check('new text panel persists defaults and mixed CJK layout',panel['position']==[0,24,0]and panel['fontSize']==8 and not scene['textLayouts'][0]['layout']['missing'])
  b.js('document.getElementById("panelText").value="ABC 123\\n中文 日本語";document.getElementById("panelText").dispatchEvent(new Event("input"));')
  time.sleep(.5);b.click('panelApply');b.wait('return window.__morphforge.renderer.textPanels.panels[0].text.startsWith("ABC")',20)
  check('text input applies to persisted scene',api('scene')['scene']['textPanels'][0]['text']=='ABC 123\n中文 日本語')
  revision=api('state')['revision'];api('undo',{})
  b.wait('return !window.__morphforge.renderer.textPanels.panels[0].text.startsWith("ABC")')
  check('text edits participate in scene undo',api('state')['revision']>revision)
  api('edit',{'op':'redo'});b.wait('return window.__morphforge.renderer.textPanels.panels[0].text.startsWith("ABC")')
  before=api('scene')['scene'];b.js('document.querySelector("[data-panel-move=Y][data-delta=\\"1\\"]").click()')
  b.wait('return window.__morphforge.renderer.textPanels.panels[0].position[1]===25')
  check('directional text move controls work',api('scene')['scene']['textPanels'][0]['position'][1]==25)
  images=b.js('''return (async()=>{const r=window.__morphforge.renderer;r.renderer.setAnimationLoop(null);const images=await r.capture(['read-front','read-back'],512,192,[{id:'read-front',projection:'orthographic',position:[0,41,128],target:[0,41,0],orthoHeight:48},{id:'read-back',projection:'orthographic',position:[0,41,-128],target:[0,41,0],orthoHeight:48}],{supersampling:1});return images})()''')
  for im in images:(a.out/(im['view']+'.png')).write_bytes(base64.b64decode(im['data'].split(',',1)[1]))
  metrics=b.js("""return(async()=>{const pixels=[];for(const url of arguments[0]){const image=new Image();image.src=url;await image.decode();const c=document.createElement('canvas');c.width=image.width;c.height=image.height;const ctx=c.getContext('2d');ctx.drawImage(image,0,0);pixels.push(ctx.getImageData(0,0,c.width,c.height).data);}let error=0,ink=0;for(let i=0;i<pixels[0].length;i+=4){if(Math.min(pixels[0][i],pixels[0][i+1],pixels[0][i+2])>150)ink++;for(let j=0;j<3;j++)error+=Math.abs(pixels[0][i+j]-pixels[1][i+j]);}return{meanError:error/(pixels[0].length/4*3),ink}})()""",[i['data']for i in images])
  check('front and back show the same reading order and glyphs',metrics['meanError']<.2)
  check('actual glyph ink is visible',metrics['ink']>100)
  b.screenshot(a.out/'text-editor.png')
  logs=b.call(b.path+'/log',{'type':'browser'});errors=[r for r in logs if r['level']=='SEVERE'and'favicon.ico'not in r['message']]
  check('no JavaScript or font shader errors',not errors)
 finally:
  (a.out/'validation.json').write_text(json.dumps({'success':bool(checks)and all(x['passed']for x in checks),'checks':checks},ensure_ascii=False,indent=2))
  if b:b.close()
  driver.terminate()
if __name__=='__main__':main()
