#!/usr/bin/env python3
"""Actual composer DOM regression with a local speech-service fixture, no audio/network."""
import argparse,base64,json,socket,subprocess,time
from pathlib import Path
from check_editor_browser import Browser

def main():
 p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
 sock=socket.socket();sock.bind(('127.0.0.1',0));port=sock.getsockname()[1];sock.close();driver=subprocess.Popen(['chromedriver',f'--port={port}','--allowed-ips=127.0.0.1'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);b=None;checks=[]
 def check(name,value):
  checks.append({'name':name,'passed':bool(value)});print(('PASS 'if value else 'FAIL ')+name,flush=True)
  if not value:raise AssertionError(name)
 try:
  for _ in range(50):
   try:b=Browser(f'http://127.0.0.1:{port}');break
   except(OSError,RuntimeError):time.sleep(.1)
  b.call(b.path+'/url',{'url':'data:text/html,<meta charset=utf-8><textarea id=prompt></textarea><textarea id=panelText></textarea><textarea id=textContent></textarea>'})
  code=base64.b64encode((Path(__file__).resolve().parents[1]/'web/speech-ui.js').read_bytes()).decode()
  b.js('''window.composer=document.getElementById('prompt');window.fixture={settings:{provider:'offline',autoCaption:false},state:{active:false,phase:'idle',draft:'',draftVersion:0},calls:[]};fixture.api=async(path,body)=>{fixture.calls.push([path,body]);if(path==='speech/devices')return {native:true};if(path==='speech/control'){fixture.state.active=false;fixture.state.phase='idle';fixture.state.draft='新一段终稿';fixture.state.draftVersion++;}if(path==='speech/start'){fixture.state={sessionId:'local-start',active:true,phase:'recording',target:body.target,draft:'',draftVersion:0};}return {settings:fixture.settings,state:fixture.state};};fixture.set=patch=>Object.assign(fixture.state,patch);fixture.project='p1';fixture.toasts=[];''')
  b.js('''const source=new TextDecoder().decode(Uint8Array.from(atob(arguments[0]),c=>c.charCodeAt(0)));const module=await import(URL.createObjectURL(new Blob([source],{type:'text/javascript'})));module.setupComposerSpeech({$:id=>document.getElementById(id),api:fixture.api,getState:()=>({project:{id:fixture.project}}),getPanelId:()=>"caption",toast:x=>fixture.toasts.push(x),fail:e=>{throw e;}});return true;''',code)
  b.js("composer.value='前缀 ';composer.setSelectionRange(composer.value.length,composer.value.length);fixture.set({sessionId:'mini-session',active:true,phase:'recording',target:{kind:'modeling',projectId:'p1'},draft:'第一句',draftVersion:1});")
  b.wait("return composer.value==='前缀 第一句'");check('dictation inserts once at existing caret',True)
  b.js("fixture.set({applyRequestId:'apply-1',draftVersion:2});");time.sleep(1);check('mini apply does not duplicate already inserted dictation',b.js("return composer.value==='前缀 第一句'"))
  b.js("fixture.set({draft:'修正',applyRequestId:'apply-2',draftVersion:3});");b.wait("return composer.value==='前缀 修正'");check('explicit correction replaces the inserted segment',True)
  b.js("composer.value+=' 手工';composer.dispatchEvent(new Event('input'));fixture.set({draft:'后续',draftVersion:4});");time.sleep(1);check('ongoing dictation preserves manual edits',b.js("return composer.value==='前缀 修正 手工'"))
  b.js("composer.setSelectionRange(composer.value.length,composer.value.length);fixture.set({sessionId:'another-mini-session',applyRequestId:'',draft:'新一段',draftVersion:1});");b.wait("return composer.value==='前缀 修正 手工新一段'");check('new session appends at new caret instead of replacing old session',True)
  b.click('voicePrompt');b.wait("return composer.value==='前缀 修正 手工新一段终稿'");check('stop button preserves the final insertion boundary',b.js("return fixture.calls.filter(c=>c[0]==='speech/control').length===1"))
  b.js("fixture.set({target:{kind:'modeling',projectId:'another-project'},draft:'不应插入',draftVersion:3});");time.sleep(1);check('foreign project target never writes the current composer',b.js("return !composer.value.includes('不应插入')"))
  (a.out/'validation.json').write_text(json.dumps({'success':True,'checks':checks,'scope':'Actual Chromium DOM and production speech UI, local API fixture only; no microphone/model/OSC.'},ensure_ascii=False,indent=2))
 finally:
  if b:b.close()
  driver.terminate();driver.wait(timeout=10)
if __name__=='__main__':main()
