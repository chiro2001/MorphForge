// Ray distance to a Y-aligned capsule with endpoint centers at +/-length/2.
// CPU picking uses the same diameter and endpoints as the GPU deformation.
export function capsuleRayDistance(origin, direction, length, radius = .5) {
  const [x,y,z]=origin,[dx,dy,dz]=direction,half=length/2,candidates=[];
  const a=dx*dx+dz*dz,b=x*dx+z*dz,c=x*x+z*z-radius*radius;
  if(a>1e-12){const h=b*b-a*c;if(h>=0)for(const t of [(-b-Math.sqrt(h))/a,(-b+Math.sqrt(h))/a])if(t>=0&&Math.abs(y+t*dy)<=half)candidates.push(t);}
  for(const center of [-half,half]){const cy=y-center,b=x*dx+cy*dy+z*dz,c=x*x+cy*cy+z*z-radius*radius,h=b*b-c;
    if(h>=0)for(const t of [-b-Math.sqrt(h),-b+Math.sqrt(h)])if(t>=0&&(center<0?y+t*dy<=-half:y+t*dy>=half))candidates.push(t);
  }
  return candidates.length?Math.min(...candidates):null;
}
