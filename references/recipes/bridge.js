mf.box([-24,0,-12],[48,1,24],{style:3});mf.box([-16,1,-5],[32,9,10],{style:1});
for(let y=1;y<=6;y++){const r=Math.floor(Math.sqrt(100-((y-1)*1.8)**2));mf.erase([-r,y,-5],[2*r,1,10]);}
mf.box([-18,10,-6],[36,1,12],{style:6});
for(const z of [-6,5]){for(let x=-17;x<=15;x+=4)mf.box([x,11,z],[1,3,1],{style:2});mf.box([-18,14,z],[36,1,1],{style:2});}
