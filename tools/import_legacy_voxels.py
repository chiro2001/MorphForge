#!/usr/bin/env python3
"""Exact dense VoxelField -> disjoint greedy boxes; never execute source code here."""
import argparse,base64,hashlib,itertools,json,time,urllib.request
from pathlib import Path

def convert(raw):
    cells={};counts={}
    for field in raw['fields']:
        data=base64.b64decode(field['data']);sx,sy,sz=field['size'];lo=field['min']
        if len(data)!=sx*sy*sz:raise ValueError('Dense array length mismatch')
        count=0
        for i,style in enumerate(data):
            if not style:continue
            x=lo[0]+i%sx;y=lo[1]+i//(sx*sz);z=lo[2]+i//sx%sz
            # Preserve the source Three.js visual handedness at the canonical boundary.
            p=(x,y,-z-1)
            cells[p]=(256+style,field['name']);count+=1
        counts[field['name']]=count
    bottom=min(p[1]for p in cells);cells={(p[0],p[1]-bottom,p[2]):v for p,v in cells.items()}
    def greedy(axes):
        left=cells.copy();boxes=[]
        for p in sorted(cells,key=lambda p:tuple(p[a]for a in axes[::-1])):
            if p not in left:continue
            style,group=left[p];size=[1,1,1]
            for axis in axes:
                while True:
                    slab=[];valid=True
                    ranges=[range(size[a])if a!=axis else [size[a]]for a in range(3)]
                    for delta in itertools.product(*ranges):
                        q=tuple(p[a]+delta[a]for a in range(3))
                        if left.get(q)!=(style,group):valid=False;break
                        slab.append(q)
                    if not valid:break
                    size[axis]+=1
            for delta in itertools.product(*(range(s)for s in size)):
                del left[tuple(p[a]+delta[a]for a in range(3))]
            boxes.append({'id':len(boxes)+1,'min':list(p),'size':size,'styleId':style,'group':group,'tintId':0,'motionPhase':0})
        return boxes
    candidates=[(axes,greedy(axes))for axes in itertools.permutations(range(3))]
    axes,boxes=min(candidates,key=lambda p:len(p[1]))
    expanded={}
    for b in boxes:
        for delta in itertools.product(*(range(s)for s in b['size'])):
            q=tuple(b['min'][a]+delta[a]for a in range(3))
            if q in expanded:raise AssertionError('overlap')
            expanded[q]=(b['styleId'],b['group'])
    if expanded!=cells:raise AssertionError('conversion changed occupancy/style')
    materials=[]
    for p in raw['palette']:
        if not p or p['id']==0:continue
        color=[int(p['hex'][i:i+2],16)/255 for i in [1,3,5]];family=p['mat'];water=family=='water';metal=family=='metal';glow=family=='glow'
        materials.append({'id':256+p['id'],'uid':'leviathan:'+p['name'],'name':p['name'],'family':'water'if water else'metal'if metal else'emissive'if glow else'matte','color':color,'metallic':.85 if metal else .05,'roughness':.28 if metal else .24 if water else .9,'opacity':.75 if water else 1.,'emission':p['emissive']if glow else 0.,'renderMode':'transparent'if water else'opaque','textures':[],'frames':1,'frameTime':.1})
    scene={'format':'MorphForgeScene/1','cellSize':raw['cellSize'],'size':256,'revision':0,'boxes':boxes}
    report={'success':True,'sourceCounts':counts,'uniqueVoxels':len(cells),'boxes':len(boxes),'mergeAxes':axes,'allCandidates':[{'axes':a,'boxes':len(b)}for a,b in candidates],'exactOccupancyAndStyles':True,'bottomOffset':-bottom,'coordinates':'canonical x=x, y=y-bottom, z=-sourceZ-1','excludes':raw.get('excludes',[])}
    return scene,materials,report

def main():
    p=argparse.ArgumentParser();p.add_argument('input',type=Path);p.add_argument('--output',type=Path,required=True);p.add_argument('--report',type=Path,required=True);p.add_argument('--url');a=p.parse_args();start=time.monotonic();scene,materials,report=convert(json.loads(a.input.read_text()));report['elapsedSeconds']=time.monotonic()-start;report['sourceSha256']=hashlib.sha256(a.input.read_bytes()).hexdigest()
    project={'format':'MorphForgeProject/1','project':{'scene':scene,'materials':materials}};a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(project));a.report.parent.mkdir(parents=True,exist_ok=True);a.report.write_text(json.dumps(report,indent=2))
    if a.url:
        request=urllib.request.Request(a.url.rstrip('/')+'/api/import',data=json.dumps(project).encode(),headers={'Content-Type':'application/json'});print(urllib.request.urlopen(request,timeout=60).read().decode())
    print(json.dumps(report))
if __name__=='__main__':main()
