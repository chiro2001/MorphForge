// Material orientation is applied in canonical art coordinates, independently of lighting.
export function inverseTextureRotation(p, code) {
  let [x,y,z]=p;
  switch(code%4){case 1:[x,z]=[z,-x];break;case 2:[x,z]=[-x,-z];break;case 3:[x,z]=[-z,x];break;}
  switch(Math.floor(code/4)){case 1:return [y,-x,z];case 2:return [x,z,-y];case 3:return [x,-z,y];case 4:return [-x,-y,z];case 5:return [-y,x,z];default:return [x,y,z];}
}
export const textureOrientationShader = `
vec3 mfInverseRotation(vec3 p,float code){
 float yaw=mod(code,4.);float axis=floor(code/4.);
 if(yaw>.5&&yaw<1.5)p=vec3(p.z,p.y,-p.x);
 else if(yaw>1.5&&yaw<2.5)p=vec3(-p.x,p.y,-p.z);
 else if(yaw>2.5)p=vec3(-p.z,p.y,p.x);
 if(axis>.5&&axis<1.5)return vec3(p.y,-p.x,p.z);
 if(axis>1.5&&axis<2.5)return vec3(p.x,p.z,-p.y);
 if(axis>2.5&&axis<3.5)return vec3(p.x,-p.z,p.y);
 if(axis>3.5&&axis<4.5)return vec3(-p.x,-p.y,p.z);
 if(axis>4.5)return vec3(-p.y,p.x,p.z);
 return p;
}
`;
