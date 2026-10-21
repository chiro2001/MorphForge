import * as THREE from "three";
// Unity-style orbit/pan plus a right-button fly camera. Input fields keep keys.
export class SceneNavigation {
  constructor(renderer) {
    this.r=renderer;this.canvas=renderer.renderer.domElement;this.keys=new Set();this.flying=false;this.speed=1;this.lastTime=null;this.listeners=[];
    const on=(target,type,fn,options)=>{target.addEventListener(type,fn,options);this.listeners.push(()=>target.removeEventListener(type,fn,options));};
    const editing=()=>document.querySelector("dialog[open]")||["INPUT","TEXTAREA","SELECT"].includes(document.activeElement?.tagName)||document.activeElement?.isContentEditable;
    on(this.canvas,"pointerdown",e=>{
      if(!renderer.controls.enabled||editing())return;
      renderer.controls.mouseButtons.LEFT=e.altKey?THREE.MOUSE.ROTATE:null;
      renderer.controls.mouseButtons.RIGHT=e.altKey?THREE.MOUSE.DOLLY:THREE.MOUSE.ROTATE;
      if(e.button!==2||e.altKey)return;
      e.preventDefault();e.stopImmediatePropagation();this.flying=true;this.pointer=e.pointerId;this.last=[e.clientX,e.clientY];
      this.distance=Math.max(1e-5,renderer.camera.position.distanceTo(renderer.controls.target));
      this.savedEnabled=renderer.controls.enabled;renderer.controls.enabled=false;
      this.euler=new THREE.Euler().setFromQuaternion(renderer.camera.quaternion,"YXZ");
      this.canvas.setPointerCapture(e.pointerId);
    },true);
    on(this.canvas,"pointermove",e=>{if(!this.flying)return;e.preventDefault();e.stopImmediatePropagation();this.euler.y-=(e.clientX-this.last[0])*.004;this.euler.x=Math.max(-Math.PI/2+.001,Math.min(Math.PI/2-.001,this.euler.x-(e.clientY-this.last[1])*.004));this.last=[e.clientX,e.clientY];renderer.camera.quaternion.setFromEuler(this.euler);this.alignTarget();},true);
    const stop=()=>{if(!this.flying)return;this.flying=false;this.keys.clear();renderer.controls.enabled=this.savedEnabled;};
    on(this.canvas,"pointerup",stop,true);on(this.canvas,"pointercancel",stop,true);on(window,"blur",stop);
    on(this.canvas,"wheel",e=>{if(!this.flying)return;e.preventDefault();e.stopImmediatePropagation();this.speed=Math.max(.05,Math.min(20,this.speed*Math.exp(-e.deltaY*.002)));},{capture:true,passive:false});
    on(window,"keydown",e=>{if(editing())return;if(e.code==="KeyF"&&!this.flying&&!e.ctrlKey&&!e.metaKey){e.preventDefault();const selected=renderer.cameraObjects().filter(b=>renderer.selection.has(b.id));if(selected.length){const min=[0,1,2].map(a=>Math.min(...selected.map(b=>b.min[a]))),max=[0,1,2].map(a=>Math.max(...selected.map(b=>b.min[a]+b.size[a])));renderer.view("hero",min.map((v,a)=>(v+max[a])/2));}else renderer.view();}if(this.flying&&["KeyW","KeyA","KeyS","KeyD","KeyQ","KeyE","ShiftLeft","ShiftRight"].includes(e.code)){e.preventDefault();this.keys.add(e.code);}});
    on(window,"keyup",e=>this.keys.delete(e.code));
  }
  alignTarget(){const r=this.r,d=new THREE.Vector3();r.camera.getWorldDirection(d);r.controls.target.copy(r.camera.position).addScaledVector(d,this.distance);}
  update(time){const dt=Math.min(.05,Math.max(0,this.lastTime===null?0:time-this.lastTime));this.lastTime=time;if(!this.flying)return;const r=this.r,forward=new THREE.Vector3();r.camera.getWorldDirection(forward);const right=new THREE.Vector3().crossVectors(forward,r.camera.up).normalize(),move=new THREE.Vector3();
    const key=(positive,negative)=>(this.keys.has(positive)?1:0)-(this.keys.has(negative)?1:0);
    move.addScaledVector(forward,key("KeyW","KeyS")).addScaledVector(right,key("KeyD","KeyA"));move.y+=key("KeyE","KeyQ");
    const rate=Math.max(r.cellSize*r.scale*2,this.distance*.7)*this.speed*(this.keys.has("ShiftLeft")||this.keys.has("ShiftRight")?3:1);
    if(move.lengthSq())r.camera.position.addScaledVector(move.normalize(),rate*dt);this.alignTarget();
  }
  dispose(){for(const off of this.listeners)off();this.keys.clear();}
}
