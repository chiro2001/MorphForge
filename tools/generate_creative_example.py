#!/usr/bin/env python3
"""Generate the project-authored animated example using an isolated actual backend."""
import argparse, hashlib, json, subprocess, urllib.request
from pathlib import Path
from check_editor_browser import Browser
ROOT=Path(__file__).resolve().parents[1]
def main():
    p=argparse.ArgumentParser();p.add_argument('--url',required=True);p.add_argument('--webdriver',default='http://127.0.0.1:9519');p.add_argument('--out',type=Path,required=True);a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
    def api(path,value=None):
        q=urllib.request.Request(a.url+'/api/'+path,data=None if value is None else json.dumps(value,ensure_ascii=False).encode(),headers={'Content-Type':'application/json'})
        with urllib.request.urlopen(q,timeout=90) as r:return json.load(r)
    def tool(name,args):return api('harness/tool',{'name':name,'arguments':args})
    worker=Browser(a.webdriver)
    try:
        assert not api('state')['connection']['enabled']
        worker.call(worker.path+'/url',{'url':a.url+'/?capture'});worker.wait('return window.__morphforge?.ready',30)
        doc=api('export');doc['project']['creative']={'revision':0,'bookmarks':{},'components':{},'clips':{}};api('import',doc)
        tool('run_recipe',{'mode':'replace','baseRevision':api('state')['revision'],'code':(ROOT/'examples/recipes/kinetic-windmill.js').read_text()})
        tool('components',{'action':'save','id':'windmill-window','name':'风车窗户','description':'木框、蓝色玻璃和十字窗棂；地面中心为组件原点。','groups':['window-prototype'],'baseRevision':api('state')['revision'],'baseCreativeRevision':api('creative')['revision']})
        tool('components',{'action':'place','id':'windmill-window','baseRevision':api('state')['revision'],'requestId':'windmill-second-window-'+str(api('state')['revision']),'instances':[{'offset':[4,11,6],'group':'window-copy'}]})
        tool('animation',{'action':'save','id':'windmill-spin','baseCreativeRevision':api('creative')['revision'],'clip':{'name':'风车匀速转动','duration':8,'looping':True,'tracks':[{'group':'rotor','pivot':[0,18,9],'interpolation':'linear','keys':[{'time':t,'translation':[0,0,0],'rotation':[0,0,r],'scale':[1,1,1]} for t,r in [(0,0),(2,90),(4,180),(6,270),(8,360)]]}]}})
        tool('reference_workbench',{'action':'save','id':'windmill-front','baseCreativeRevision':api('creative')['revision'],'bookmark':{'name':'风车正交正面','shot':{'projection':'orthographic','yaw':0,'pitch':0}}})
        shots=tool('capture_views',{'views':['hero','front','top'],'width':960,'height':720})
        doc=api('export');doc['project']['name']='会转动的风车';doc['project']['scene']['revision']=0;doc['project']['commits']=[];doc['project']['textReceipts']={};doc['project']['importReceipts']={}
        credit={'id':'builtin:kinetic-windmill','title':'会转动的风车','author':'MorphForge项目自制','license':'MIT','source':'examples/recipes/kinetic-windmill.js'};doc['project']['sourceCredits']=[credit]
        project_path=ROOT/'examples/projects/kinetic-windmill.json';project_path.write_text(json.dumps(doc,ensure_ascii=False,indent=2)+'\n')
        images=[]
        for shot in shots['images']:
            name='kinetic-windmill-'+shot['view'];png=a.out/(name+'.png');png.write_bytes(urllib.request.urlopen(a.url+shot['url']).read());target=ROOT/'examples/images'/(name+'.webp');subprocess.run(['magick',str(png),'-quality','82',str(target)],check=True);images.append({'view':shot['view'],'file':'images/'+target.name,'originalArtifact':shot['artifact'],'sha256':hashlib.sha256(target.read_bytes()).hexdigest()})
        scene=doc['project']['scene'];artwork={'id':'builtin:kinetic-windmill','title':'会转动的风车','tags':['内置示例','动画','组件','风车'],'createdAt':'2026-10-20T12:00:00Z','boxCount':len(scene['boxes']),'triangleCount':len(scene.get('triangles',[])),'voxelCount':api('state')['stats']['voxelCount'],'thumbnail':None,'sourceRun':None,'origin':'builtin','sourceCredits':[credit],'description':'含可复用窗户组件、正面相机书签和8秒循环旋转轨道。载入后打开动画，选择“风车匀速转动”播放；当前Avatar只发布静态几何。'}
        manifest=ROOT/'examples/manifest.json';m=json.loads(manifest.read_text());m['examples']=[e for e in m['examples'] if e['id']!=artwork['id']];m['examples'].append({'id':artwork['id'],'projectFile':'projects/kinetic-windmill.json','artwork':artwork,'images':images,'sourceRecord':'examples/recipes/kinetic-windmill.js'});m['version']=2;manifest.write_text(json.dumps(m,ensure_ascii=False,indent=2)+'\n')
        report={'success':True,'artwork':artwork,'creative':doc['project']['creative'],'realModelRequests':0};(a.out/'validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print(artwork['boxCount'],artwork['triangleCount'])
    finally:worker.close()
if __name__=='__main__':main()
