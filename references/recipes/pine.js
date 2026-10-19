mf.box([-18,0,-14],[36,1,28],{style:4});
function pine(x,z,h){mf.box([x-1,1,z-1],[2,h,2],{style:2});for(let i=0;i<6;i++){const r=6-i;mf.box([x-r,3+i*2,z-r],[r*2,2,r*2],{style:4});}}
pine(-6,2,11);pine(8,-5,10);mf.box([-14,1,-10],[4,2,3],{style:1});
