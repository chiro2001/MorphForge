const ops=[];let currentGroup='main',offset=[0,0,0],mirrors=[1,1,1],seed=settings.seed>>>0;
const random=()=>{seed=(Math.imul(seed,1664525)+1013904223)>>>0;return seed/4294967296;};
Math.random=random;globalThis.Date=undefined;
const style=v=>{if(v===undefined)return 1;if(typeof v==='number'&&Number.isInteger(v)&&v>=0)return v;const n=settings.styles[v]??settings.styles['mf:'+v];if(n===undefined)throw new Error('Unknown material: '+v);return n;};
const vector=(v,name)=>{if(!Array.isArray(v)||v.length!==3||v.some(x=>!Number.isInteger(x)||Math.abs(x)>1000000))throw new Error(name+' requires 3 bounded integers');return v.slice();};
const emit=(op,min,size,o={})=>{if(ops.length>=settings.maxOps)throw new Error('MAX_OPS exceeded');min=vector(min,'min');size=vector(size,'size');if(size.some(x=>x<1))throw new Error('size must be positive');min=min.map((x,i)=>offset[i]+(mirrors[i]===1?x:-x-size[i]));const value={op,min,size};if(op==='box'||op==='paint'){value.styleId=style(o.style??o.styleId);value.tintId=o.tint??o.tintId??0;}if(op==='box'){value.motionPhase=o.motion??o.motionPhase??0;value.group=o.group??currentGroup;}ops.push(value);};
const mf=Object.freeze({
 clear(){if(ops.length>=settings.maxOps)throw new Error('MAX_OPS exceeded');ops.push({op:'clear'});},
 box(min,size=[1,1,1],o={}){emit('box',min,size,o);},
 erase(min,size=[1,1,1]){emit('erase',min,size);},
 paint(min,size,o={}){emit('paint',min,size,o);},
 random,
 group(name,fn){if(typeof name!=='string'||name.length>128)throw new Error('Invalid group');const old=currentGroup;currentGroup=name;try{fn(mf);}finally{currentGroup=old;}},
 repeat(count,fn){if(!Number.isInteger(count)||count<0||count>100000)throw new Error('Invalid repeat count');for(let i=0;i<count;i++)fn(i,mf);},
 translate(at,fn){at=vector(at,'offset');const old=offset;offset=offset.map((x,i)=>x+at[i]);try{fn(mf);}finally{offset=old;}},
 mirror(axis,fn){const a=['x','y','z'].indexOf(axis);if(a<0)throw new Error('Invalid axis');mirrors[a]*=-1;try{fn(mf);}finally{mirrors[a]*=-1;}},
 line(a,b,o={}){a=vector(a,'a');b=vector(b,'b');const steps=Math.max(...a.map((v,i)=>Math.abs(v-b[i])));if(steps>100000)throw new Error('Line too long');const t=o.thickness??1;for(let i=0;i<=steps;i++)emit('box',a.map((v,j)=>Math.round(v+(b[j]-v)*(steps?i/steps:0))),[t,t,t],o);},
 stairs(at,count,o={}){at=vector(at,'origin');if(!Number.isInteger(count)||count<1||count>10000)throw new Error('Invalid steps');for(let i=0;i<count;i++)emit('box',[at[0]+i*(o.depth??1),at[1],at[2]],[o.depth??1,(i+1)*(o.rise??1),o.width??3],o);},
 wall(at,width,height,o={}){emit('box',at,[width,height,o.thickness??1],o);},
 roof(at,width,depth,height,o={}){for(let y=0;y<height&&width-2*y>0;y++)emit('box',[at[0]+y,at[1]+y,at[2]],[width-2*y,1,depth],o);},
 frame(at,size,o={}){const t=o.thickness??1;emit('box',at,[size[0],t,size[2]],o);emit('box',[at[0],at[1]+size[1]-t,at[2]],[size[0],t,size[2]],o);emit('box',at,[t,size[1],size[2]],o);emit('box',[at[0]+size[0]-t,at[1],at[2]],[t,size[1],size[2]],o);},
 comment(text){}
});
