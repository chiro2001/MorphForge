mf.box([-12,0,-11],[24,1,22],{style:4});
mf.box([-7,1,-6],[14,9,12],{style:2});mf.erase([-6,2,-5],[12,7,10]);
mf.erase([-2,1,5],[4,6,1]);
for(const x of [-5,3]){mf.erase([x,4,5],[2,3,1]);mf.box([x,4,5],[2,3,1],{style:3});mf.box([x-1,3,5],[4,1,2],{style:1});}
mf.roof([-9,10,-8],18,16,8,{style:8});mf.box([4,11,-3],[2,9,2],{style:1});
for(let i=0;i<3;i++)mf.box([-3,1,6+i*2],[6,3-i,2],{style:1});
