#!/usr/bin/env python3
"""GPU regression: VRC preview blends catalog and texture alpha in linear light.

Serves only repository web files, on loopback. No project, provider or OSC API.
Requires Chromium and ChromeDriver. Saves public diagnostic values, no artwork.
"""
import argparse
import json
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import socket
import subprocess
import threading
import time
from check_editor_browser import Browser

ROOT = Path(__file__).resolve().parents[1]
PAGE = b'''<!doctype html><html><head><meta charset="utf-8"><script type="importmap">{"imports":{"three":"/vendor/three.module.js"}}</script></head><body style="margin:0"><div id="v" style="width:512px;height:512px"></div><script type="module">import{VoxelRenderer}from'/renderer.js';window.renderer=new VoxelRenderer(document.getElementById('v'));window.ready=true;</script></body></html>'''

class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(ROOT / 'web'), **kwargs)
    def log_message(self, *args):
        pass
    def do_GET(self):
        if self.path == '/probe':
            self.send_response(200)
            self.send_header('Content-Type', 'text/html; charset=utf-8')
            self.end_headers()
            self.wfile.write(PAGE)
        else:
            super().do_GET()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT/'docs/avatar-plugin/web-alpha.json')
    args = parser.parse_args()
    server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    driver = subprocess.Popen(['chromedriver', f'--port={port}', '--allowed-ips=127.0.0.1'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    browser = None
    try:
        for _ in range(50):
            try:
                browser = Browser(f'http://127.0.0.1:{port}')
                break
            except (OSError, RuntimeError):
                time.sleep(.1)
        if browser is None:
            raise RuntimeError('ChromeDriver did not start')
        browser.call(browser.path+'/url', {'url': f'http://127.0.0.1:{server.server_port}/probe'})
        browser.wait('return window.ready', 20)
        values = browser.js('''return (async()=>{
          const THREE=await import('three'), r=window.renderer;
          r.renderer.setAnimationLoop(null);r.controls.enabled=false;r.helpers.visible=false;
          const scene={revision:1,cellSize:.25,size:256,boxes:[{id:1,min:[-2,-2,-2],size:[4,4,4],styleId:900,group:'probe'}],triangles:[]};
          const canvas=document.createElement('canvas');canvas.width=canvas.height=4;
          const context=canvas.getContext('2d');context.fillStyle='rgba(128,128,128,0.5)';context.fillRect(0,0,4,4);const texture=canvas.toDataURL();
          const rows=[];
          for(const textured of [false,true])for(const opacity of [0,.1,1/3,.5,2/3,.9,1]){
            const style={id:900,uid:'probe:alpha',name:'alpha',color:[.5,.5,.5],metallic:0,roughness:1,opacity,emission:0,renderMode:opacity<1||textured?'transparent':'opaque',textures:textured?[texture]:[]};
            await r.load(scene,[style],{mode:'vrc',scale:1});r.camera.position.set(0,0,-3);r.controls.target.set(0,0,0);r.camera.lookAt(0,0,0);r.camera.updateMatrixWorld();
            const samples=[];
            for(const background of [0,1]){r.scene.background=new THREE.Color(background,background,background);r.render();const size=r.renderer.getDrawingBufferSize(new THREE.Vector2()),p=new Uint8Array(4),gl=r.renderer.getContext();gl.readPixels(size.x/2,size.y/2,1,1,gl.RGBA,gl.UNSIGNED_BYTE,p);samples.push([...p].map(v=>v/255));}
            const linear=x=>x<=.04045?x/12.92:((x+.055)/1.055)**2.4;
            const measured=1-(linear(samples[1][0])-linear(samples[0][0]));
            const textureAlpha=textured?context.getImageData(1,1,1,1).data[3]/255:1;
            const expected=opacity*textureAlpha;
            rows.push({textured,opacity,expected,measured,error:Math.abs(measured-expected),samples});
          }
          const before={position:r.camera.position.toArray(),target:r.controls.target.toArray()};
          const screenshots=await r.capture(['front'],128,128,null,{supersampling:2});
          const restored=before.position.every((v,i)=>Math.abs(v-r.camera.position.toArray()[i])<1e-8)&&before.target.every((v,i)=>Math.abs(v-r.controls.target.toArray()[i])<1e-8);
          const result={rows,capture:{count:screenshots.length,png:screenshots[0].data.startsWith('data:image/png;base64,'),restored}};r.dispose();return result;
        })();''')
        errors = [row for row in browser.call(browser.path+'/log', {'type':'browser'}) if row['level']=='SEVERE' and 'favicon' not in row['message']]
        success = all(row['error'] < .012 and all(abs(p[3]-1) < .005 for p in row['samples']) for row in values['rows']) and values['capture']['png'] and values['capture']['restored'] and not errors
        report = {'success':success,'scope':'Actual Chromium WebGL, source renderer, precise catalog opacity, with/without 50% texture alpha; no project/OSC service',**values,'browserErrors':errors}
        args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2))
        print(json.dumps({'success':success,'cases':len(values['rows']),'maximumError':max(row['error'] for row in values['rows']),'capture':values['capture'],'browserErrors':errors},ensure_ascii=False))
        if not success: raise SystemExit(1)
    finally:
        if browser: browser.close()
        driver.terminate()
        driver.wait(timeout=10)
        server.shutdown()

if __name__ == '__main__': main()
