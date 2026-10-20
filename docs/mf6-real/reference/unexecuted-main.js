// 红顶小屋 · 参数化体素复刻（参考图：分层四坡红瓦、左后浅灰烟囱、橙墙、正面双侧蓝窗、门洞与浅灰台阶、方形草地底座）
module.exports = (mf, params) => {
  const p = Object.assign({
    houseW: 60,
    houseD: 44,
    wallH: 26,
    baseW: 96,
    eaveOver: 8,
    roofTier: 4,     // 每级瓦楞厚度（更块面的台阶感）
    roofZStep: 4,    // 每级 z 退进
    roofXStep: 2.8,  // 每级 x 退进
    chimneyTop: 70,
  }, params || {});
  const C = { red: 25, redDark: 26, wall: 27, grass: 28, stone: 29, stoneDark: 30, win: 31, door: 32 };
  const g = mf.group;
  const box = (min, size, style) => mf.box(min, size, { style });

  const hw = p.houseW / 2, hd = p.houseD / 2;
  const bw = p.baseW / 2;
  const wallTop = 3 + p.wallH;        // 29
  const x0 = -hw, z0 = -hd, zFace = hd - 1;  // 21

  // ---------- 方形草地底座 ----------
  g('base', () => {
    box([-bw, 0, -bw], [p.baseW, 3, p.baseW], C.grass);
  });

  // ---------- 墙体 ----------
  g('walls', () => {
    box([x0, 3, z0], [p.houseW, p.wallH, p.houseD], C.wall);
  });

  // ---------- 门洞 / 门板 ----------
  const doorX0 = -6, doorX1 = 6, doorY0 = 9, doorY1 = 23;
  g('door', () => {
    mf.erase([doorX0, doorY0, zFace - 5], [doorX1 - doorX0, doorY1 - doorY0, 6]);
    box([doorX0, doorY0, zFace - 4], [doorX1 - doorX0, doorY1 - doorY0, 2], C.door);
    box([doorX0 - 2, doorY0, zFace], [2, doorY1 - doorY0 + 1, 2], C.stone);
    box([doorX1, doorY0, zFace], [2, doorY1 - doorY0 + 1, 2], C.stone);
    box([doorX0 - 2, doorY1, zFace], [doorX1 - doorX0 + 4, 2, 2], C.stone);
    box([doorX0, doorY0 - 1, zFace - 4], [doorX1 - doorX0, 1, 4], C.stone);
  });

  // ---------- 入户台阶 ----------
  g('steps', () => {
    box([-8, 3, zFace + 1], [16, 6, 6], C.stone);     // 平台（顶面 9）
    box([-10, 3, zFace + 7], [20, 4, 5], C.stone);    // 第二级（顶面 7）
    box([-12, 3, zFace + 12], [24, 2, 5], C.stone);   // 第三级（顶面 5）
    box([-14, 3, zFace + 17], [28, 1, 4], C.stone);   // 第四级（顶面 4）
    box([-10, 6, zFace + 11], [20, 1, 1], C.stoneDark);
    box([-8, 8, zFace + 6], [16, 1, 1], C.stoneDark);
  });

  // ---------- 正面两侧蓝窗 ----------
  const winY0 = 10, winY1 = 22, winH = winY1 - winY0;
  const windows = [[-26, -17], [17, 26]];
  g('windows', () => {
    for (const [wx0, wx1] of windows) {
      const w = wx1 - wx0;
      mf.erase([wx0, winY0, zFace - 2], [w, winH, 3]);
      box([wx0, winY0, zFace - 2], [w, winH, 1], C.win);
      box([wx0 - 1, winY0 - 1, zFace], [w + 2, 1, 2], C.stone);  // 窗台
      box([wx0 - 1, winY1, zFace], [w + 2, 1, 2], C.stone);      // 窗楣
      box([wx0 - 1, winY0 - 1, zFace], [1, winH + 2, 2], C.stone);
      box([wx1, winY0 - 1, zFace], [1, winH + 2, 2], C.stone);
      box([wx0, winY0 + 6, zFace - 2], [w, 1, 1], C.stone);       // 横棂
      box([wx0 + Math.floor(w / 2), winY0, zFace - 2], [1, winH, 1], C.stone); // 竖棂
    }
  });

  // ---------- 分层四坡屋顶 ----------
  const eaveX = hw + p.eaveOver;   // 38
  const eaveZ = hd + p.eaveOver;   // 30
  g('roof', () => {
    let i = 0, lastX = 6;
    while (true) {
      const zh = Math.round(eaveZ - p.roofZStep * i);
      const xh = Math.round(eaveX - p.roofXStep * i);
      if (zh < 4 || xh < 6) break;
      const y = wallTop + p.roofTier * i;
      box([-xh, y, -zh], [2 * xh, p.roofTier, 2 * zh], C.red);
      // 每级下沿外圈的暗红，勾出瓦楞立面阴影
      const h = p.roofTier - 1;
      box([-xh, y, -zh], [2 * xh, h, 2], C.redDark);
      box([-xh, y, zh - 2], [2 * xh, h, 2], C.redDark);
      box([-xh, y, -zh + 2], [2, h, 2 * zh - 4], C.redDark);
      box([xh - 2, y, -zh + 2], [2, h, 2 * zh - 4], C.redDark);
      lastX = Math.round(eaveX - p.roofXStep * (i + 1));
      i++;
    }
    // 长屋脊
    const ridgeY = wallTop + p.roofTier * i;
    box([-lastX, ridgeY - p.roofTier, -4], [2 * lastX, p.roofTier, 8], C.red);
    box([-lastX + 2, ridgeY - 1, -3], [2 * lastX - 4, 1, 6], C.redDark);
    box([-lastX + 2, ridgeY, -2], [2 * lastX - 4, 2, 4], C.red);
  });

  // ---------- 左后方浅灰烟囱 ----------
  g('chimney', () => {
    box([-29, 25, -19], [11, 42, 11], C.stone);
    for (let y = 30; y < p.chimneyTop - 3; y += 6) {
      box([-29, y, -19], [11, 1, 11], C.stoneDark);
    }
    box([-30, p.chimneyTop - 3, -20], [13, 2, 13], C.stone);
    box([-29, p.chimneyTop - 1, -19], [11, 1, 11], C.stoneDark);
  });
};
