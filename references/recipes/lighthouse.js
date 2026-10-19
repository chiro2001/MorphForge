mf.box([-12,0,-12],[24,2,24],{style:1});
for(let i=0;i<9;i++){const r=5-Math.floor(i/3);mf.box([-r,2+2*i,-r],[2*r,2,2*r],{style:i%3===0?8:7});}
mf.box([-5,20,-5],[10,1,10],{style:2});mf.box([-4,21,-4],[8,5,8],{style:3});
for(const x of [-4,3])for(const z of [-4,3])mf.box([x,21,z],[1,5,1],{style:2});
mf.box([-1,22,-1],[2,3,2],{style:17});mf.roof([-6,26,-6],12,12,5,{style:8});
mf.box([-2,2,4],[4,5,1],{style:2});
