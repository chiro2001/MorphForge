// MorphForge project-authored MIT example. No model request or external image.
mf.box([-13, 0, -9], [26, 2, 18], {style: 4, group: 'ground'});
mf.box([-10, 2, -7], [20, 1, 14], {style: 1, group: 'foundation'});
mf.box([-7, 3, -5], [14, 18, 10], {style: 6, group: 'tower'});
mf.box([-2, 3, 5], [4, 7, 1], {style: 2, group: 'door'});
mf.box([-1, 4, 6], [1, 1, 1], {style: 7, group: 'door'});
mf.roof([-9, 21, -7], 18, 14, 8, {style: 8, group: 'roof'});
mf.box([-6, 11, 5], [4, 5, 1], {style: 2, group: 'window-prototype'});
mf.box([-5, 12, 6], [2, 3, 1], {style: 3, group: 'window-prototype'});
mf.box([-4, 12, 7], [1, 3, 1], {style: 7, group: 'window-prototype'});
mf.box([-5, 13, 7], [2, 1, 1], {style: 7, group: 'window-prototype'});
// All rotor elements share one motion group and one pivot at [0,18,9].
mf.box([-1, 17, 7], [2, 2, 4], {style: 11, group: 'rotor'});
mf.box([1, 17, 9], [12, 2, 1], {style: 2, group: 'rotor'});
mf.box([-13, 17, 9], [12, 2, 1], {style: 2, group: 'rotor'});
mf.box([-1, 19, 9], [2, 12, 1], {style: 2, group: 'rotor'});
mf.box([-1, 5, 9], [2, 12, 1], {style: 2, group: 'rotor'});
for (const [x,y,w,h] of [[3,19,9,3],[-12,14,9,3],[-4,21,3,9],[1,6,3,9]]) {
  mf.box([x,y,9], [w,h,1], {style: 7, group: 'rotor'});
}
mf.triangle([3,19,11],[12,19,11],[12,22,11],{color:'#E5A74B',group:'rotor',doubleSided:true});
mf.triangle([-12,14,11],[-3,14,11],[-12,17,11],{color:'#E5A74B',group:'rotor',doubleSided:true});
