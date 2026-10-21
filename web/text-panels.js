import * as THREE from 'three';
const fontBytes=new Map();
function bytes(font){if(!fontBytes.has(font))fontBytes.set(font,fetch(`/pixel-fonts/${font}/bitmap`).then(r=>{if(!r.ok)throw Error('点阵字库读取失败');return r.arrayBuffer();}).then(b=>new Uint8Array(b)).catch(e=>{fontBytes.delete(font);throw e;}));return fontBytes.get(font);}
const vertex=`attribute vec2 glyphData;varying vec2 vGlyphUV;varying vec2 vGlyphData;varying vec2 vPanel;
void main(){vec3 p=position;vec3 origin=(modelMatrix*vec4(0.,0.,0.,1.)).xyz;vec3 front=(modelMatrix*vec4(0.,0.,-1.,0.)).xyz;float side=dot(cameraPosition-origin,front)>=0.?1.:-1.;p.x*=side;vGlyphUV=uv;vGlyphData=glyphData;vPanel=position.xy;gl_Position=projectionMatrix*modelViewMatrix*vec4(p,1.);}`;
const fragment=`uniform sampler2D fontAtlas;uniform vec3 inkColor;uniform float opacity;uniform vec2 panelSize;varying vec2 vGlyphUV;varying vec2 vGlyphData;varying vec2 vPanel;
void main(){if(abs(vPanel.x)>panelSize.x*.5||vPanel.y<0.||vPanel.y>panelSize.y)discard;float col=min(vGlyphData.y-1.,floor(vGlyphUV.x*vGlyphData.y));float row=min(15.,floor((1.-vGlyphUV.y)*16.));float address=vGlyphData.x*32.+row*2.+floor(col/8.);vec2 pixel=vec2(mod(address,2048.),floor(address/2048.));float packed=floor(texture2D(fontAtlas,(pixel+.5)/vec2(2048.,1024.)).r*255.+.5);float bit=mod(floor(packed/exp2(7.-mod(col,8.))),2.);if(bit<.5)discard;gl_FragColor=vec4(inkColor,opacity);
#include <colorspace_fragment>
}`;
export class TextPanelRenderer {
 constructor(renderer){this.renderer=renderer;this.root=new THREE.Group();this.root.name='Text panels';renderer.content.add(this.root);this.textures=new Map();this.items=new Map();this.signature='';this.panels=[];}
 async texture(font){if(this.textures.has(font))return this.textures.get(font);const data=await bytes(font);const t=new THREE.DataTexture(data,2048,1024,THREE.RedFormat,THREE.UnsignedByteType);t.magFilter=t.minFilter=THREE.NearestFilter;t.generateMipmaps=false;t.colorSpace=THREE.NoColorSpace;t.flipY=false;t.needsUpdate=true;this.textures.set(font,t);return t;}
 clear(){for(const n of this.root.children.slice()){n.traverse(o=>{o.geometry?.dispose();if(o.material)for(const m of(Array.isArray(o.material)?o.material:[o.material]))m.dispose();});this.root.remove(n);}this.items.clear();}
 async load(panels,layouts,unit){const signature=JSON.stringify([panels,layouts,unit,[...this.renderer.groupsHidden].sort()]);if(signature===this.signature)return;this.signature=signature;this.clear();this.panels=panels;for(const row of layouts||[]){if(!this.renderer.groupsHidden.has(row.panel.group))await this.create(row.panel,row.layout,unit);}}
 async create(panel,layout,unit){
  const root=new THREE.Group();root.name=panel.name;root.userData.textPanel=panel;root.position.set(panel.position[0]*unit,panel.position[1]*unit,-panel.position[2]*unit);
  const q=new THREE.Quaternion().setFromEuler(new THREE.Euler(...panel.rotation.map(v=>v*Math.PI/180),'XYZ'));root.quaternion.set(-q.x,-q.y,q.z,q.w);
  const bg=new THREE.Mesh(new THREE.PlaneGeometry(panel.size[0]*unit,panel.size[1]*unit),new THREE.MeshBasicMaterial({color:new THREE.Color().setRGB(...panel.background.slice(0,3).map(v=>v/255),THREE.SRGBColorSpace),transparent:true,opacity:panel.background[3]/255*panel.opacity,side:THREE.DoubleSide,depthWrite:false}));bg.position.y=panel.size[1]*unit/2;bg.userData.textPanelId=panel.id;bg.renderOrder=10;root.add(bg);
  const pos=[],uv=[],data=[];
  for(const g of layout.glyphs){const x=panel.size[0]/2-g.x,y=g.y,w=g.width,h=g.height;for(const [dx,dy,u,v]of [[0,0,0,0],[-w,0,1,0],[-w,h,1,1],[0,0,0,0],[-w,h,1,1],[0,h,0,1]]){pos.push((x+dx)*unit,(y+dy)*unit,0);uv.push(u,v);data.push(g.glyph,g.pixels);}}
  const geometry=new THREE.BufferGeometry();geometry.setAttribute('position',new THREE.Float32BufferAttribute(pos,3));geometry.setAttribute('uv',new THREE.Float32BufferAttribute(uv,2));geometry.setAttribute('glyphData',new THREE.Float32BufferAttribute(data,2));geometry.computeBoundingSphere();
  const material=new THREE.ShaderMaterial({vertexShader:vertex,fragmentShader:fragment,uniforms:{fontAtlas:{value:await this.texture(panel.font)},inkColor:{value:new THREE.Color().setRGB(...panel.color.map(v=>v/255),THREE.SRGBColorSpace)},opacity:{value:panel.opacity},panelSize:{value:new THREE.Vector2(...panel.size.map(v=>v*unit))}},side:THREE.DoubleSide,transparent:true,depthWrite:false,toneMapped:false});
  const mesh=new THREE.Mesh(geometry,material);mesh.userData.textPanelId=panel.id;mesh.renderOrder=11;root.add(mesh);this.root.add(root);this.items.set(panel.id,root);return root;
 }
 async preview(panel,layout){const old=this.items.get(panel.id);if(old){old.traverse(o=>{o.geometry?.dispose();o.material?.dispose();});this.root.remove(old);this.items.delete(panel.id);}return this.create(panel,layout,this.renderer.cellSize);}
 pick(event){const r=this.renderer,rect=r.renderer.domElement.getBoundingClientRect(),ray=new THREE.Raycaster();ray.setFromCamera(new THREE.Vector2((event.clientX-rect.left)/rect.width*2-1,-(event.clientY-rect.top)/rect.height*2+1),r.camera);const hit=ray.intersectObjects(this.root.children,true).find(h=>h.object.userData.textPanelId);return hit?{id:hit.object.userData.textPanelId,point:hit.point,ray:ray.ray}:null;}
 dispose(){this.clear();for(const t of this.textures.values())t.dispose();this.root.removeFromParent();}
}
