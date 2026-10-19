#!/usr/bin/env python3
"""Render project-authored common references using an isolated existing workbench.
No model calls, Unity access, or OSC transmission; run against a disposable data dir.
"""
import argparse,json,urllib.request,subprocess
from pathlib import Path
from check_editor_browser import Browser
ROOT=Path(__file__).resolve().parents[1]
def main():
 p=argparse.ArgumentParser();p.add_argument('--url',required=True);a=p.parse_args()
 def api(path,body=None):
  request=urllib.request.Request(a.url+'/api/'+path,data=None if body is None else json.dumps(body).encode(),headers={'Content-Type':'application/json'})
  with urllib.request.urlopen(request,timeout=60) as response:return json.load(response)
 assert not api('state')['connection']['enabled']
 browser=Browser();browser.call(browser.path+'/url',{'url':a.url+'/?capture=1'});browser.wait('return window.__morphforge?.ready',30)
 titles={'chair':'木椅与坐垫','pine':'松树与林地','cottage':'屋顶、门窗与小屋','bridge':'石拱桥与水面','lighthouse':'条纹灯塔与灯室'}
 manifest=json.loads((ROOT/'references/manifest.json').read_text())
 try:
  for key,title in titles.items():
   revision=api('state')['revision'];code=(ROOT/'references/recipes'/f'{key}.js').read_text()
   result=api('harness/tool',{'name':'run_recipe','arguments':{'code':code,'mode':'replace','baseRevision':revision,'seed':1}})
   assert result.get('ok'),result
   result=api('harness/tool',{'name':'capture_views','arguments':{'views':['hero'],'width':1024,'height':832}})
   image=result['images'][0];png=ROOT/'references/images'/f'{key}.png';thumb=ROOT/'references/thumbnails'/f'{key}.webp'
   with urllib.request.urlopen(a.url+image['url']) as response:png.write_bytes(response.read())
   subprocess.run(['magick',str(png),'-strip',str(png)],check=True)
   subprocess.run(['magick',str(png),'-thumbnail','224x224>','-strip','-quality','82',str(thumb)],check=True)
   manifest=[item for item in manifest if item['id']!='builtin:'+key]
   manifest.append(dict(id='builtin:'+key,title=title,category='常用结构',origin='builtin',author='MorphForge 项目自制',sourceUrl='',license='MIT（项目自制参考）',licenseUrl='',notes=f'由 references/recipes/{key}.js 参数化生成，使用项目自有材质。',width=1024,height=832,artifact=f'images/{key}.png',thumbnail=f'thumbnails/{key}.webp'))
  (ROOT/'references/manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')
  print(json.dumps({'success':True,'generated':list(titles),'builtinCount':len(manifest)}))
 finally:browser.close()
if __name__=='__main__':main()
