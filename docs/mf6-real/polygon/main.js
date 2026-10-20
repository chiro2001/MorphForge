module.exports = (mf, params) => {
  const P = Object.assign({ yaw: 16, groundY: 22 }, params || {});
  const BY = -5;   // 鸟体整体下沉：短腿、腹部贴近石头
  const C = {
    azure:'#1E78DC', teal:'#2FB4D2', mid:'#1659A8', deep:'#103F8C',
    orange:'#F2962C', amber:'#CF6C15', cream:'#F7F0DF',
    black:'#15171A', leg:'#33373C',
    warmRock:'#94897C', coolRock:'#78838D', darkRock:'#4E5761',
    water:'#2F8AA6', water2:'#4BA0B8', foam:'#CFE9EE'
  };

  const G = {}; let cur = null;
  const T = { c:1, s:0, ox:0, oy:0, oz:0, sx:1 };
  const setT = (deg, ox, oy, oz) => { const r = deg*Math.PI/180; T.c=Math.cos(r); T.s=Math.sin(r); T.ox=ox; T.oy=oy; T.oz=oz; T.sx=1; };
  const unsetT = () => { T.c=1; T.s=0; T.ox=0; T.oy=0; T.oz=0; T.sx=1; };
  function useGroup(n){ cur = G[n] || (G[n] = { V:[], F:[], FC:[] }); }
  function draw(m, group, colorFn){
    useGroup(group);
    const base = cur.V.length;
    const rv = m.verts.map(p => {
      const x0 = p[0]*T.sx;
      return [ Math.round(x0*T.c + p[2]*T.s + T.ox),
               Math.round(p[1] + T.oy),
               Math.round(-x0*T.s + p[2]*T.c + T.oz) ];
    });
    for(const p of rv) cur.V.push(p);
    for(const f of m.faces){
      const a=rv[f[0]], b=rv[f[1]], d=rv[f[2]];
      const ux=b[0]-a[0], uy=b[1]-a[1], uz=b[2]-a[2];
      const vx=d[0]-a[0], vy=d[1]-a[1], vz=d[2]-a[2];
      const nx=uy*vz-uz*vy, ny=uz*vx-ux*vz, nz=ux*vy-uy*vx;
      if(nx*nx+ny*ny+nz*nz < 0.2) continue;
      const p0=m.verts[f[0]], p1=m.verts[f[1]], p2=m.verts[f[2]];
      const cen=[(p0[0]+p1[0]+p2[0])/3,(p0[1]+p1[1]+p2[1])/3,(p0[2]+p1[2]+p2[2])/3];
      const col = colorFn ? colorFn(cen) : C.azure;
      if(T.sx < 0) cur.F.push([base+f[0], base+f[2], base+f[1]]);
      else cur.F.push([base+f[0], base+f[1], base+f[2]]);
      cur.FC.push(col);
    }
  }

  function loftZ(rings, seg, opt){
    opt = opt || {};
    const phase = opt.phase || 0, verts=[], faces=[], idx=[];
    rings.forEach(r => {
      const row=[];
      for(let j=0;j<seg;j++){
        const a = phase + 2*Math.PI*j/seg;
        row.push(verts.length);
        verts.push([ (r.cx||0) + r.rx*Math.cos(a), r.cy + r.ry*Math.sin(a), r.z ]);
      }
      idx.push(row);
    });
    for(let i=0;i<rings.length-1;i++)
      for(let j=0;j<seg;j++){
        const j2=(j+1)%seg;
        faces.push([idx[i][j], idx[i][j2], idx[i+1][j]]);
        faces.push([idx[i][j2], idx[i+1][j2], idx[i+1][j]]);
      }
    const last = rings.length-1;
    const c0 = verts.length; verts.push([rings[0].cx||0, rings[0].cy, rings[0].z]);
    for(let j=0;j<seg;j++){ const j2=(j+1)%seg; faces.push([c0, idx[0][j2], idx[0][j]]); }
    const c1 = verts.length; verts.push([rings[last].cx||0, rings[last].cy, rings[last].z]);
    for(let j=0;j<seg;j++){ const j2=(j+1)%seg; faces.push([c1, idx[last][j], idx[last][j2]]); }
    return { verts, faces };
  }

  function loftY(rings, seg, opt){
    opt = opt || {};
    const verts=[], faces=[], idx=[];
    rings.forEach((r,ri) => {
      const row=[];
      for(let j=0;j<seg;j++){
        const a = 2*Math.PI*j/seg;
        const m = opt.mult ? opt.mult(ri, j, a) : 1;
        row.push(verts.length);
        verts.push([ (r.cx||0) + r.rx*m*Math.cos(a), r.y, (r.cz||0) - r.rz*m*Math.sin(a) ]);
      }
      idx.push(row);
    });
    for(let i=0;i<rings.length-1;i++)
      for(let j=0;j<seg;j++){
        const j2=(j+1)%seg;
        faces.push([idx[i][j], idx[i][j2], idx[i+1][j]]);
        faces.push([idx[i][j2], idx[i+1][j2], idx[i+1][j]]);
      }
    const last = rings.length-1;
    const cb = verts.length; verts.push([rings[0].cx||0, rings[0].y, rings[0].cz||0]);
    for(let j=0;j<seg;j++){ const j2=(j+1)%seg; faces.push([cb, idx[0][j2], idx[0][j]]); }
    const ct = verts.length; verts.push([rings[last].cx||0, rings[last].y, rings[last].cz||0]);
    for(let j=0;j<seg;j++){ const j2=(j+1)%seg; faces.push([ct, idx[last][j], idx[last][j2]]); }
    return { verts, faces };
  }

  function ccwYZ(poly){ let s=0; for(let i=0;i<poly.length;i++){const a=poly[i],b=poly[(i+1)%poly.length]; s+=a[0]*b[1]-b[0]*a[1];} return s<0?poly.slice().reverse():poly; }
  function ccwXZ(poly){ let s=0; for(let i=0;i<poly.length;i++){const a=poly[i],b=poly[(i+1)%poly.length]; s+=a[0]*b[1]-b[0]*a[1];} return s<0?poly.slice().reverse():poly; }

  function prismX(poly, x0, x1, taper){
    const p = ccwYZ(poly), n = p.length;
    let cy=0, cz=0; p.forEach(q=>{ cy+=q[0]; cz+=q[1]; }); cy/=n; cz/=n;
    const k = (taper===undefined)?1:taper;
    const verts=[], faces=[], A=[], B=[];
    p.forEach(q => { A.push(verts.length); verts.push([x0, q[0], q[1]]); });
    p.forEach(q => { B.push(verts.length); verts.push([x1, cy+(q[0]-cy)*k, cz+(q[1]-cz)*k]); });
    for(let j=1;j<n-1;j++) faces.push([B[0], B[j], B[j+1]]);
    for(let j=1;j<n-1;j++) faces.push([A[0], A[j+1], A[j]]);
    for(let j=0;j<n;j++){ const j2=(j+1)%n; faces.push([A[j], A[j2], B[j2]]); faces.push([A[j], B[j2], B[j]]); }
    return { verts, faces };
  }

  function prismY(poly, y0, y1){
    const p = ccwXZ(poly), n = p.length;
    const verts=[], faces=[], A=[], B=[];
    p.forEach(q => { A.push(verts.length); verts.push([q[0], y0, q[1]]); });
    p.forEach(q => { B.push(verts.length); verts.push([q[0], y1, q[1]]); });
    for(let j=1;j<n-1;j++) faces.push([B[0], B[j+1], B[j]]);
    for(let j=1;j<n-1;j++) faces.push([A[0], A[j], A[j+1]]);
    for(let j=0;j<n;j++){ const j2=(j+1)%n; faces.push([A[j], B[j2], A[j2]]); faces.push([A[j], B[j], B[j2]]); }
    return { verts, faces };
  }

  function segYZ(y0,z0,y1,z1,w0,w1){
    const dy=y1-y0, dz=z1-z0, L=Math.hypot(dy,dz) || 1, py=-dz/L, pz=dy/L;
    return [ [y0+py*w0/2, z0+pz*w0/2], [y1+py*w1/2, z1+pz*w1/2],
             [y1-py*w1/2, z1-pz*w1/2], [y0-py*w0/2, z0-pz*w0/2] ];
  }
  function segXZ(x0,z0,x1,z1,w0,w1){
    const dx=x1-x0, dz=z1-z0, L=Math.hypot(dx,dz) || 1, px=-dz/L, pz=dx/L;
    return [ [x0+px*w0/2, z0+pz*w0/2], [x1+px*w1/2, z1+pz*w1/2],
             [x1-px*w1/2, z1-pz*w1/2], [x0-px*w0/2, z0-pz*w0/2] ];
  }
  function discYZ(cy, cz, r, n, phase){
    const p=[]; for(let i=0;i<n;i++){ const a=(phase||0)+2*Math.PI*i/n; p.push([cy+r*Math.sin(a), cz+r*Math.cos(a)]); }
    return p;
  }

  // ---------- colour rules（作者坐标 y = c[1]-BY） ----------
  const axisY = z => 22.0 + 0.21*z;
  const bodyCol = c => {
    const y = c[1] - BY, z = c[2], dy = y - axisY(z);
    if (z > 17 && Math.abs(c[0]) < 4.0 && y > 26) return C.cream;   // 喉下小胸斑
    if (dy > -1.0) return C.azure;
    if (dy > -7.0) return C.orange;
    return C.amber;
  };
  const headCol = c => {
    const y = c[1] - BY, z = c[2], x = Math.abs(c[0]);
    if (y < 33.0 && z > 10) return C.cream;                          // 下颌白斑
    if (z > 9 && y >= 33.0 && y < 38.5 && x > 3.0) return C.orange;   // 颊部橙带
    if (y > 42.0) return C.teal;                                     // 冠部亮青
    return C.azure;
  };
  const tailCol = c => ((c[1] - BY) > 20.0 ? C.mid : C.deep);
  const hash = (x,y,z) => { const h = Math.sin(x*12.9898 + y*78.233 + z*37.719)*43758.5453; return h - Math.floor(h); };
  const rockCol = c => {
    const f = hash(c[0], c[1], c[2]);
    if (c[1] < 6) return f < 0.55 ? C.darkRock : C.coolRock;
    if (c[0] > 5) return f < 0.30 ? C.warmRock : (f < 0.75 ? C.coolRock : C.darkRock);
    if (c[0] < -5) return f < 0.20 ? C.warmRock : (f < 0.58 ? C.coolRock : C.darkRock);
    return f < 0.28 ? C.coolRock : (f < 0.55 ? C.warmRock : C.darkRock);
  };

  // ================= BIRD =================
  setT(P.yaw, 0, P.groundY + BY, 0);

  const BODY = [
    { z:-24, cy:19.5, rx:1.6, ry:1.8 },
    { z:-20, cy:20.0, rx:4.5, ry:4.6 },
    { z:-14, cy:21.0, rx:7.6, ry:8.0 },
    { z:-7,  cy:21.5, rx:9.2, ry:9.6 },
    { z:0,   cy:22.0, rx:9.5, ry:10.0 },
    { z:7,   cy:23.5, rx:9.2, ry:9.6 },
    { z:13,  cy:25.5, rx:8.0, ry:8.4 },
    { z:18,  cy:28.0, rx:6.0, ry:6.4 },
    { z:22,  cy:30.5, rx:3.0, ry:3.2 }
  ];
  const HEAD = [
    { z:-2, cy:32.5, rx:6.0,  ry:6.0  },
    { z:2,  cy:35.0, rx:9.5,  ry:9.5  },
    { z:7,  cy:36.5, rx:11.0, ry:10.5 },
    { z:12, cy:36.5, rx:10.5, ry:10.0 },
    { z:17, cy:35.0, rx:8.0,  ry:8.0  },
    { z:21, cy:33.5, rx:4.5,  ry:4.6  }
  ];
  const BEAK = [
    { z:18, cy:34.0, rx:3.6, ry:4.2 },
    { z:31, cy:33.6, rx:1.9, ry:2.2 },
    { z:44, cy:33.2, rx:0.8, ry:1.0 }
  ];
  // 收拢翼：三层羽片，各自闭合、贴合体侧
  const WING_A = [
    { z:7,   cx:6.8, cy:29.5, rx:2.8, ry:4.5 },
    { z:0,   cx:7.0, cy:29.0, rx:2.8, ry:5.2 },
    { z:-7,  cx:6.8, cy:28.0, rx:2.6, ry:5.0 },
    { z:-13, cx:6.0, cy:26.5, rx:1.8, ry:3.6 }
  ];
  const WING_B = [
    { z:4,   cx:7.4, cy:25.0, rx:3.0, ry:5.0 },
    { z:-2,  cx:7.6, cy:24.4, rx:3.0, ry:5.8 },
    { z:-9,  cx:7.2, cy:23.4, rx:2.8, ry:5.4 },
    { z:-17, cx:6.0, cy:21.5, rx:2.0, ry:3.6 }
  ];
  const WING_C = [
    { z:-1,  cx:6.6, cy:20.5, rx:2.6, ry:4.6 },
    { z:-7,  cx:6.8, cy:20.0, rx:2.6, ry:5.0 },
    { z:-14, cx:6.2, cy:18.5, rx:2.2, ry:4.4 },
    { z:-21, cx:5.0, cy:16.5, rx:1.2, ry:2.6 }
  ];

  draw(loftZ(BODY, 12), 'kingfisher-bird', bodyCol);
  draw(loftZ(HEAD, 12), 'kingfisher-bird', headCol);
  draw(loftZ(BEAK, 4, { phase: Math.PI/4 }), 'kingfisher-bird', () => C.black);
  for(const side of [1, -1]){
    T.sx = side;
    draw(loftZ(WING_A, 6), 'kingfisher-bird', () => C.azure);
    draw(loftZ(WING_B, 6), 'kingfisher-bird', () => C.mid);
    draw(loftZ(WING_C, 6), 'kingfisher-bird', () => C.deep);
    // 眼：黑眼 + 眼后浅斑
    draw(prismX(discYZ(38.0, 12.0, 2.5, 6, 0.3), 9.4, 11.0, 0.5), 'kingfisher-bird', () => C.black);
    draw(prismX(discYZ(40.0, 5.0, 2.3, 6, 0.3), 8.6, 10.1, 0.6), 'kingfisher-bird', () => C.cream);
  }
  T.sx = 1;

  // 短尾（锥形，略带分叉）
  draw(loftZ([
    { z:-20, cy:19.5, rx:5.0, ry:2.6 },
    { z:-27, cy:18.5, rx:5.6, ry:2.0 },
    { z:-33, cy:17.5, rx:3.4, ry:1.2 }
  ], 6), 'kingfisher-bird', tailCol);

  // 腿脚（贴石面）
  setT(P.yaw, 0, P.groundY, 0);
  for(const side of [1, -1]){
    T.sx = side;
    draw(loftY([
      { y:0.5, cx:3.0, cz:0.6, rx:1.4, rz:1.4 },
      { y:3.5, cx:3.0, cz:0.6, rx:1.4, rz:1.4 },
      { y:7.5, cx:3.0, cz:0.8, rx:2.0, rz:2.0 }
    ], 5), 'kingfisher-bird', () => C.leg);
    const toes = [
      segXZ(3.0, 0.6, 2.0, 6.0, 1.9, 1.2),
      segXZ(3.0, 0.6, 3.5, 6.3, 1.9, 1.2),
      segXZ(3.0, 0.6, 4.8, 5.4, 1.9, 1.2),
      segXZ(3.0, 0.6, 2.8, -2.6, 1.7, 1.0)
    ];
    for(const tp of toes) draw(prismY(tp, -0.5, 1.8), 'kingfisher-bird', () => C.leg);
  }
  T.sx = 1;

  // ================= BASE =================
  unsetT();

  const ROCK = [
    { y:0,  cx:0.0,  cz:0.0,  rx:19.0, rz:17.5 },
    { y:4,  cx:0.5,  cz:-0.5, rx:22.0, rz:20.0 },
    { y:9,  cx:1.0,  cz:-1.0, rx:21.0, rz:19.0 },
    { y:14, cx:0.5,  cz:-1.5, rx:17.5, rz:16.0 },
    { y:18, cx:0.0,  cz:-1.5, rx:13.5, rz:12.5 },
    { y:21, cx:0.5,  cz:-1.0, rx:10.5, rz:10.0 },
    { y:22, cx:0.5,  cz:-0.5, rx:9.2,  rz:8.6  }
  ];
  draw(loftY(ROCK, 11, { mult:(ri,j,a)=> 1 + 0.11*Math.sin(3*a + 1.3*ri) + 0.06*Math.sin(5*a - 0.7*ri) }),
       'kingfisher-rock', rockCol);

  const WATER = [
    { y:1.0, rx:33, rz:31 },
    { y:2.6, rx:31, rz:29 }
  ];
  draw(loftY(WATER, 11, { mult:(ri,j,a)=> 1 + 0.05*Math.sin(4*a + ri) }),
       'kingfisher-water', c => (hash(c[0], 0, c[2]) < 0.5 ? C.water : C.water2));

  const ripples = [
    segXZ(-14, 22, -1, 24, 1.3, 1.0),
    segXZ(12, 19, 23, 15, 1.2, 0.9),
    segXZ(21, -10, 13, -17, 1.1, 0.8)
  ];
  for(const r of ripples) draw(prismY(r, 2.7, 3.15), 'kingfisher-water', () => C.foam);

  const out = {};
  for(const name of Object.keys(G)){
    const b = G[name];
    if(!b.F.length) continue;
    mf.mesh(b.V, b.F, { colors: b.FC, group: name });
    out[name] = { verts: b.V.length, faces: b.F.length };
  }
  return { groups: out };
};
