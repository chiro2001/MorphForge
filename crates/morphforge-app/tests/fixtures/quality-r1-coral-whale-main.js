// 《珊瑚湾·鲸鱼潜航器》参数化主体源码
// 坐标约定：鲸头朝 -X，尾鳍朝 +X，主要可见侧面朝 +Z。
module.exports = (mf, params = {}) => {
  const P = Object.assign({
    NOSE: -68, TAIL: 68, CY: 46, R0: 22,
    // 调色板
    navy: 25, blue: 26, ivory: 27, copperL: 28,
    coralP: 29, coralO: 30, algae: 31,
    // 内置材质
    copper: 12, lamp: 13, ice: 18, glass: 17,
    sand: 6, soil: 5, stone: 1, concrete: 9,
    wood: 2, gold: 11, snow: 7, leaves: 21, water: 16
  }, params);

  const { NOSE, TAIL, CY, R0 } = P;
  const LEN = TAIL - NOSE;

  const B = (x, y, z, w, h, d, style) => {
    const W = Math.round(w), H = Math.round(h), D = Math.round(d);
    if (!(W > 0 && H > 0 && D > 0)) return;
    mf.box([Math.round(x), Math.round(y), Math.round(z)], [W, H, D], { style });
  };

  // ---------- 鲸体轮廓参数 ----------
  const CP = [[0,0.10],[0.04,0.45],[0.10,0.72],[0.18,0.90],[0.28,1.00],
              [0.42,1.00],[0.55,0.96],[0.66,0.86],[0.76,0.70],[0.85,0.50],
              [0.92,0.32],[1.00,0.15]];
  function prof(t) {
    t = Math.min(1, Math.max(0, t));
    for (let i = 0; i < CP.length - 1; i++) {
      const a = CP[i], b = CP[i + 1];
      if (t <= b[0]) {
        const u = (b[0] - a[0]) === 0 ? 0 : (t - a[0]) / (b[0] - a[0]);
        const s = 0.5 - 0.5 * Math.cos(Math.PI * u);
        return a[1] + (b[1] - a[1]) * s;
      }
    }
    return 0.15;
  }
  const rAt = x => R0 * prof((x - NOSE) / LEN);

  // 某一 2 高横带在 x 处的半宽（保守取内缘，使外轮廓平滑）
  function dzRow(x, yb) {
    const dyIn = Math.max(0, Math.abs(yb + 1 - CY) - 1);
    const r = rAt(x);
    if (dyIn >= r) return 0;
    return Math.round(r * 0.94 * Math.sqrt(Math.max(0, 1 - (dyIn / r) ** 2)));
  }
  const topBandY = x => CY + Math.floor(rAt(x)) + 1;

  function hullStyle(x, yb) {
    const r = rAt(x);
    const ny = (yb + 1 - CY) / Math.max(1, r);
    const wave = 0.05 * Math.sin(x * 0.09) + 0.03 * Math.sin(x * 0.27 + 1.3);
    if (ny <= -0.30 + wave) return P.ivory;
    if (ny >= 0.24 + wave) return P.navy;
    return P.blue;
  }

  // ---------- 船体 ----------
  mf.group('hull', () => {
    for (let yb = CY - R0; yb < CY + R0; yb += 2) {
      for (let x = NOSE; x <= TAIL; x++) {
        const d = dzRow(x, yb);
        if (d < 1) continue;
        B(x, yb, -d, 1, 2, 2 * d + 1, hullStyle(x, yb));
      }
    }
    // 水线暖铜带
    for (let x = NOSE + 8; x <= TAIL - 10; x += 6) {
      const r = rAt(x);
      const yt = CY + Math.round(-0.30 * r) - 1;
      const d = dzRow(x, Math.floor((yt - (CY - R0)) / 2) * 2 + (CY - R0));
      if (d < 2) continue;
      B(x, yt, d, 6, 2, 2, P.copper);      // +Z
      B(x, yt, -d - 1, 6, 2, 2, P.copper); // -Z
    }
  });

  // ---------- 尾鳍（明显分叉）与侧鳍 ----------
  mf.group('fins', () => {
    for (const sgn of [1, -1]) {
      // 水平尾鳍：越向外后缘越靠后，中缝形成分叉
      for (let z = 3; z < 36; z += 3) {
        const za = z + 1.5;
        let xEnd = 72 + 0.55 * (za - 3);
        let xStart = 56 + 0.30 * (za - 3);
        if (za > 26) { const k = (za - 26) / 10; xEnd -= k * k * 11; xStart += k * k * 5; }
        if (xEnd <= xStart) continue;
        const yc = CY - 3 + Math.round(0.14 * za);
        const zi = sgn > 0 ? z : -(z + 3);
        B(xStart, yc - 2, zi, xEnd - xStart, 5, 3, P.blue);
        // 铜质后缘
        B(xEnd - 2, yc - 2, zi, 2, 5, 3, P.copper);
      }
      // 宽大侧鳍（胸鳍）：s 向外伸展
      for (let s = 0; s <= 32; s += 4) {
        const z0 = 14 + s;
        const xc = -26 + 0.55 * s;
        const chord = Math.max(6, 18 - 0.34 * s);
        const th = Math.max(2, Math.round(6 - 0.12 * s));
        const yb = CY - 14 - Math.round(0.16 * s);
        const zi = sgn > 0 ? z0 : -(z0 + 4);
        B(xc - chord / 2, yb, zi, chord, th, 4, s > 26 ? P.copper : P.blue);
      }
      // 背鳍（小幅）
    }
    for (let i = 0; i < 5; i++) {
      const x = 24 + i * 4;
      const ty = topBandY(x) + 2;
      B(x, ty, -3 + i * 0, 4, Math.max(2, 11 - i * 2), Math.max(2, 6 - i), P.navy);
    }
  });

  // ---------- 侧面细节：舷窗 / 舱门 / 鳃格栅 ----------
  mf.group('side-detail', () => {
    // 圆舷窗
    const portholes = [-46, -30, -14, 2, 18];
    portholes.forEach((xp, idx) => {
      const yp = CY + 4;
      const dz = dzRow(xp, Math.floor((yp - (CY - R0)) / 2) * 2 + (CY - R0));
      for (let k = 0; k < 12; k++) {
        const a = k * Math.PI / 6;
        const px = xp + Math.round(5 * Math.cos(a));
        const py = yp + Math.round(5 * Math.sin(a));
        const db = dzRow(px, Math.floor((py - (CY - R0)) / 2) * 2 + (CY - R0));
        if (db < 1) continue;
        B(px - 1, py - 1, db + 1, 2, 2, 3, P.copper);
      }
      for (let r = -3; r <= 3; r += 3) {
        const py = yp + r;
        const db = dzRow(xp, Math.floor((py - (CY - R0)) / 2) * 2 + (CY - R0));
        const st = (idx % 3 === 2) ? P.ice : P.lamp;
        B(xp - 3, py, db + 1, 7, 3, 1, st);
      }
      // 窗下暖铜铭牌
      B(xp - 4, yp - 9, dz + 1, 9, 2, 1, P.copperL);
    });

    // 舱门
    const xh = 40, yh = CY - 2;
    const rowY = y => Math.floor((y - (CY - R0)) / 2) * 2 + (CY - R0);
    for (let yy = yh - 9; yy <= yh + 9; yy += 2) {
      const db = dzRow(xh, rowY(yy));
      if (db < 1) continue;
      B(xh - 6, yy, db + 1, 13, 2, 1, (yy - yh + 9) % 6 === 0 ? P.copper : P.copperL);
    }
    [[-7, 20], [7, 20]].forEach(([ox, hh]) => {
      const db = dzRow(xh, rowY(yh));
      B(xh + ox - 1, yh - hh / 2, db + 1, 2, hh, 3, P.copper);
    });
    const dbTop = dzRow(xh, rowY(yh + 9));
    B(xh - 6, yh + 9, dbTop + 1, 13, 2, 3, P.copper);
    const dbBot = dzRow(xh, rowY(yh - 10));
    B(xh - 6, yh - 10, dbBot + 1, 13, 2, 3, P.copper);
    const dbH = dzRow(xh + 3, rowY(yh));
    B(xh + 2, yh - 1, dbH + 3, 4, 2, 2, P.copper);

    // 鳃状格栅
    for (let i = 0; i < 5; i++) {
      const xg = 28 + i * 4;
      for (const yy of [CY - 1, CY + 5]) {
        const db = dzRow(xg, rowY(yy));
        if (db < 1) continue;
        B(xg - 1, yy, db + 1, 2, 6, 2, P.copper);
        B(xg - 1, yy + 5, db + 2, 2, 1, 1, P.lamp);
      }
    }

    // 腹部探照灯
    for (const xl of [-24, 4, 30]) {
      const by = CY - Math.floor(rAt(xl));
      B(xl - 3, by - 3, -3, 7, 3, 7, P.copper);
      B(xl - 2, by - 5, -2, 5, 2, 5, P.lamp);
    }

    // 尾部铜箍
    const tx = 58, tr = Math.ceil(rAt(tx));
    B(tx, CY - tr, -tr - 1, 3, 2 * tr + 1, 2 * tr + 3, P.copper);
  });

  // ---------- 顶部驾驶舱 / 天线 / 管线 ----------
  mf.group('deck', () => {
    const XC = -28;
    // 铜质甲板唇口
    for (let x = -45; x <= -11; x += 3) {
      const d = dzRow(x, CY + 16);
      if (d < 2) continue;
      B(x, 62, -d - 1, 3, 3, 2 * d + 3, P.copper);
    }
    // 舱罩：椭球穹顶 + 竖向铜肋
    for (let x = -44; x <= -12; x++) {
      const fx = Math.sqrt(Math.max(0, 1 - ((x - XC) / 16.5) ** 2));
      if (fx <= 0.05) continue;
      const isRib = Math.abs(x + 40) <= 1 || Math.abs(x + 32) <= 1 ||
                    Math.abs(x + 24) <= 1 || Math.abs(x + 16) <= 1;
      for (let yb = 64; yb <= 80; yb += 2) {
        const k = (yb - 64) / 16;
        const fy = Math.sqrt(Math.max(0, 1 - k * k));
        const w = Math.round(13 * fx * fy);
        if (w < 1) continue;
        B(x, yb, -w, 1, 2, 2 * w + 1, isRib ? P.copper : P.ice);
      }
    }
    // 舱顶舱盖
    B(XC - 3, 80, -5, 7, 2, 11, P.copper);
    B(XC - 1, 82, -2, 3, 2, 5, P.copperL);
    B(XC + 4, 64, -12, 2, 16, 2, P.copper); // 侧支撑

    // 天线
    const ax = -4, aty = topBandY(ax);
    B(ax - 1, aty, -1, 3, 22, 3, P.copper);
    B(ax - 1, aty + 22, -1, 3, 3, 3, P.lamp);
    B(ax - 7, aty + 16, -1, 15, 1, 3, P.copperL);
    B(ax - 1, aty + 9, -3, 2, 2, 7, P.copperL);
    // 天线底座
    B(ax - 3, aty - 2, -3, 7, 3, 7, P.copper);

    // 顶部铜管
    for (const sgn of [1, -1]) {
      for (let x = 2; x <= 52; x += 8) {
        const ty = topBandY(x) - 4;
        B(x, ty, sgn > 0 ? 7 : -9, 9, 3, 3, P.copper);
        if (x % 16 === 2) B(x, ty - 1, sgn > 0 ? 6 : -10, 2, 5, 5, P.copperL);
      }
    }
    // 通气管
    B(10, topBandY(10) - 2, -3, 4, 10, 4, P.copperL);
    B(10, topBandY(10) + 8, -2, 3, 3, 3, P.copper);
  });

  // ---------- 海床与珊瑚场景 ----------
  mf.group('scene', () => {
    // 海床基底
    B(-102, -6, -64, 210, 6, 128, P.soil);
    for (let x = -102; x < 106; x += 14) {
      for (let z = -64; z < 62; z += 14) {
        const h = 2 + ((Math.abs((x * 7 + z * 13) % 5)) % 2);
        B(x, 0, z, 14, h, 14, (x + z) % 28 === 0 ? P.soil : P.sand);
      }
    }
    // 礁石
    const rocks = [[-88, 20, 10], [-70, -34, 14], [-40, -44, 18], [-6, -40, 12],
                   [30, -46, 16], [66, -30, 12], [86, 18, 14], [-96, -8, 9], [58, 26, 9]];
    rocks.forEach(([x, z, w], i) => {
      B(x, 2, z, w, 6 + (i % 3) * 3, w - 3, i % 2 ? P.stone : P.concrete);
      B(x + 2, 2 + 6 + (i % 3) * 3, z + 2, w - 6, 3, w - 8, P.concrete);
    });

    // 枝状珊瑚
    const branch = (x, z, h, st, tip) => {
      B(x, 2, z, 5, h, 5, st);
      const dirs = [[1, 0], [-1, 1], [0, -1], [1, -1]];
      dirs.forEach(([dx, dz], i) => {
        const bx = x + dx * 2, bz = z + dz * 2;
        const by = 2 + Math.floor(h * (0.5 + 0.12 * i));
        B(bx + (dx > 0 ? 4 : -4), by, bz, 4, 3, 5, st);
        B(bx + (dx > 0 ? 7 : -7), by + 3, bz, 3, 8 + (i % 2) * 4, 4, st);
        B(bx + (dx > 0 ? 7 : -7), by + 11 + (i % 2) * 4, bz, 3, 3, 4, tip);
      });
      B(x + 1, 2 + h, z + 1, 3, 4, 3, tip);
    };
    // 扇形珊瑚
    const fan = (x, z, w, h, st) => {
      B(x, 2, z, 4, h, 4, P.copperL);
      for (let i = 0; i < 3; i++) {
        B(x - w / 2 + i, 3 + i * Math.round(h / 3), z, w - i * 2, Math.round(h / 3) + 2, 3, st);
      }
    };
    // 管状珊瑚丛
    const tubes = (x, z, st) => {
      for (let i = 0; i < 4; i++) {
        const hh = 7 + (i % 3) * 5;
        B(x + i * 3 - 5, 2, z + (i % 2) * 4 - 2, 3, hh, 3, st);
        B(x + i * 3 - 6, 2 + hh, z + (i % 2) * 4 - 3, 5, 2, 5, P.snow);
      }
    };

    // 身后（-Z）与两侧远端，避免遮挡 +Z 主视角
    branch(-84, -26, 22, P.coralP, P.coralO);
    branch(-46, -40, 16, P.coralO, P.coralP);
    branch(14, -42, 20, P.coralP, P.snow);
    branch(62, -36, 14, P.coralO, P.coralP);
    fan(-64, -20, 16, 20, P.coralP);
    fan(40, -24, 14, 16, P.coralO);
    tubes(-30, -30, P.algae);
    tubes(76, -18, P.algae);
    tubes(-14, 32, P.algae);
    // 主视角下方前侧低矮珊瑚（不高于船腹）
    branch(-72, 30, 12, P.coralO, P.coralP);
    branch(48, 34, 11, P.coralP, P.snow);
    fan(-52, 26, 12, 12, P.coralO);
    fan(22, 30, 12, 11, P.coralP);

    // 海草
    for (let i = 0; i < 8; i++) {
      const x = -90 + i * 24, z = -12 + (i % 3) * 9;
      B(x, 2, z, 2, 6 + (i % 4) * 3, 2, P.leaves);
      B(x + 2, 2, z + 2, 2, 4 + (i % 3) * 3, 2, P.leaves);
    }

    // 小型系泊设施
    const mx = -88, mz = 14;
    B(mx, 2, mz, 11, 4, 11, P.concrete);
    B(mx + 2, 6, mz + 2, 7, 3, 7, P.stone);
    B(mx + 4, 9, mz + 4, 3, 13, 3, P.copper);
    B(mx + 2, 21, mz + 2, 7, 2, 7, P.copperL);
    B(mx + 5, 23, mz + 5, 5, 4, 5, P.copperL); // 缆桩
    // 松弛缆绳通向浮标
    for (let i = 0; i < 10; i++) {
      const t = i / 9;
      const rx = mx + 8 + t * 24;
      const ry = 24 - Math.round(8 * Math.sin(t * Math.PI * 0.9));
      const rz = mz + 4 + t * 12;
      B(rx, ry, rz, 2, 2, 2, P.wood);
    }
    B(mx + 33, 12, mz + 17, 4, 7, 4, P.copperL);
    B(mx + 33, 19, mz + 17, 4, 3, 4, P.lamp);

    // 小鱼群
    const fish = [[-58, 18, -16], [-52, 22, -20], [-46, 19, -15], [12, 16, -22], [18, 20, -18]];
    fish.forEach(([fx, fy, fz], i) => {
      B(fx, fy, fz, 5, 3, 2, i % 2 ? P.snow : P.gold);
      B(fx + 5, fy + 1, fz, 3, 3, 1, i % 2 ? P.gold : P.snow);
    });
  });
};
