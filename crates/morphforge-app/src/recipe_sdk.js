const ops = [];
let currentGroup = "main",
  offset = [0, 0, 0],
  mirrors = [1, 1, 1],
  seed = settings.seed >>> 0;
const random = () => {
  seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
  return seed / 4294967296;
};
Math.random = random;
globalThis.Date = undefined;
const style = (v) => {
  if (v === undefined) return 1;
  if (typeof v === "number" && Number.isInteger(v) && v >= 0) return v;
  const n = settings.styles[v] ?? settings.styles["mf:" + v];
  if (n === undefined) throw new Error("Unknown material: " + v);
  return n;
};
const vector = (v, name) => {
  if (
    !Array.isArray(v) ||
    v.length !== 3 ||
    v.some((x) => !Number.isInteger(x) || Math.abs(x) > 1000000)
  )
    throw new Error(name + " requires 3 bounded integers");
  return v.slice();
};
const emit = (op, min, size, o = {}) => {
  if (ops.length >= settings.maxOps) throw new Error("MAX_OPS exceeded");
  min = vector(min, "min");
  size = vector(size, "size");
  if (size.some((x) => x < 1))
    throw new Error(
      "size must be positive; op=" +
        op +
        " min=" +
        JSON.stringify(min) +
        " size=" +
        JSON.stringify(size) +
        " group=" +
        (o.group ?? currentGroup) +
        " operation=" +
        ops.length,
    );
  min = min.map((x, i) => offset[i] + (mirrors[i] === 1 ? x : -x - size[i]));
  const value = { op, min, size };
  if (op === "box" || op === "paint") {
    value.styleId = style(o.style ?? o.styleId);
    value.tintId = o.tint ?? o.tintId ?? 0;
  }
  if (op === "box") {
    value.motionPhase = o.motion ?? o.motionPhase ?? 0;
    value.group = o.group ?? currentGroup;
  }
  ops.push(value);
};
const boundedTextInt = (value, name, min, max) => {
  if (!Number.isInteger(value) || value < min || value > max)
    throw new Error(name + " out of range");
  return value;
};
const boundedOptionalTextInt = (value, name, min, max) =>
  value === undefined || value === null
    ? undefined
    : boundedTextInt(value, name, min, max);
const textAligns = ["left", "center", "right"],
  textFacings = ["positive", "negative"];
const text = (content, origin = [0, 0, 0], o = {}) => {
  if (
    typeof content !== "string" ||
    content.length === 0 ||
    content.length > 16384 ||
    [...content].length > 4096
  )
    throw new Error("Invalid text");
  origin = vector(origin, "origin");
  if (!["xy", "xz", "yz"].includes(o.plane ?? "xy"))
    throw new Error("Invalid text plane");
  if (!textAligns.includes(o.align ?? "left"))
    throw new Error("Invalid text align");
  if (!textFacings.includes(o.facing ?? "positive"))
    throw new Error("Invalid text facing");
  if (ops.length >= settings.maxOps) throw new Error("MAX_OPS exceeded");
  const request = {
    text: content,
    fontId: o.fontId ?? "builtin:noto-sans-sc",
    fontSize: boundedTextInt(o.fontSize ?? 32, "fontSize", 4, 512),
    depth: boundedTextInt(o.depth ?? 1, "depth", 1, 256),
    origin,
    plane: o.plane ?? "xy",
    align: o.align ?? "left",
    letterSpacing: boundedTextInt(
      o.letterSpacing ?? 0,
      "letterSpacing",
      -512,
      512,
    ),
    lineSpacing: boundedTextInt(o.lineSpacing ?? 0, "lineSpacing", -512, 512),
    maxWidth: boundedOptionalTextInt(o.maxWidth, "maxWidth", 1, 1000000),
    styleId: style(o.style ?? o.styleId),
    tintId: boundedTextInt(o.tint ?? o.tintId ?? 0, "tint", 0, 63),
    motionPhase: boundedTextInt(
      o.motion ?? o.motionPhase ?? 0,
      "motion",
      0,
      63,
    ),
    group: o.group ?? currentGroup,
    facing: o.facing ?? "positive",
  };
  ops.push({
    op: "text",
    request,
    offset: offset.slice(),
    mirrors: mirrors.slice(),
  });
};
const finitePoint = (v, name) => {
  if (
    !Array.isArray(v) ||
    v.length !== 3 ||
    v.some(
      (n) =>
        typeof n !== "number" || !Number.isFinite(n) || Math.abs(n) > 1000000,
    )
  )
    throw Error(name + " requires 3 finite numbers");
  return v;
};
const positive = (v, name, max = 1024) => {
  if (typeof v !== "number" || !Number.isFinite(v) || v <= 0 || v > max)
    throw Error(name + " must be positive and bounded");
  return v;
};
const voxelize = (bounds, predicate, o = {}) => {
  const lo = vector(bounds.min, "bounds.min"),
    hi = vector(bounds.max, "bounds.max"),
    sizes = hi.map((v, i) => v - lo[i]);
  if (sizes.some((v) => v <= 0) || sizes.reduce((a, b) => a * b, 1) > 2000000)
    throw Error("voxelize bounds must contain 1..2000000 cells");
  if (typeof predicate !== "function")
    throw Error("voxelize requires predicate");
  for (let y = lo[1]; y < hi[1]; y++)
    for (let z = lo[2]; z < hi[2]; z++) {
      let start = lo[0],
        active = null;
      for (let x = lo[0]; x <= hi[0]; x++) {
        const v = x === hi[0] ? false : predicate(x + 0.5, y + 0.5, z + 0.5);
        const next =
          v === false || v === null || v === undefined
            ? null
            : style(v === true ? (o.style ?? o.styleId) : v);
        if (next !== active) {
          if (active !== null)
            emit("box", [start, y, z], [x - start, 1, 1], {
              ...o,
              style: active,
            });
          start = x;
          active = next;
        }
      }
    }
};
const ellipsoid = (center, radii, o = {}) => {
  finitePoint(center, "center");
  finitePoint(radii, "radii");
  radii.forEach((v) => positive(v, "radius", 512));
  const bounds = {
    min: center.map((v, i) => Math.floor(v - radii[i])),
    max: center.map((v, i) => Math.ceil(v + radii[i])),
  };
  voxelize(
    bounds,
    (x, y, z) =>
      ((x - center[0]) / radii[0]) ** 2 +
        ((y - center[1]) / radii[1]) ** 2 +
        ((z - center[2]) / radii[2]) ** 2 <=
      1,
    o,
  );
};
const capsule = (a, b, radius, o = {}) => {
  finitePoint(a, "a");
  finitePoint(b, "b");
  positive(radius, "radius", 256);
  const d = b.map((v, i) => v - a[i]),
    length = d.reduce((n, v) => n + v * v, 0);
  const bounds = {
    min: a.map((v, i) => Math.floor(Math.min(v, b[i]) - radius)),
    max: a.map((v, i) => Math.ceil(Math.max(v, b[i]) + radius)),
  };
  voxelize(
    bounds,
    (x, y, z) => {
      const p = [x, y, z],
        t = length
          ? Math.max(
              0,
              Math.min(
                1,
                d.reduce((n, v, i) => n + (p[i] - a[i]) * v, 0) / length,
              ),
            )
          : 0;
      return (
        p.reduce((n, v, i) => n + (v - a[i] - t * d[i]) ** 2, 0) <=
        radius * radius
      );
    },
    o,
  );
};
const cylinder = (base, height, radius, o = {}) => {
  finitePoint(base, "base");
  positive(height, "height");
  positive(radius, "radius", 512);
  voxelize(
    {
      min: [
        Math.floor(base[0] - radius),
        Math.floor(base[1]),
        Math.floor(base[2] - radius),
      ],
      max: [
        Math.ceil(base[0] + radius),
        Math.ceil(base[1] + height),
        Math.ceil(base[2] + radius),
      ],
    },
    (x, y, z) =>
      y >= base[1] &&
      y < base[1] + height &&
      (x - base[0]) ** 2 + (z - base[2]) ** 2 <= radius * radius,
    o,
  );
};
const tube = (points, radius, o = {}) => {
  if (!Array.isArray(points) || points.length < 2 || points.length > 128)
    throw Error("tube requires 2..128 points");
  for (let i = 1; i < points.length; i++)
    capsule(points[i - 1], points[i], radius, o);
};
const extrude = (points, origin, depth, o = {}) => {
  origin = vector(origin, "origin");
  if (!Number.isInteger(depth) || depth < 1 || depth > 1024)
    throw Error("extrude depth must be integer 1..1024");
  if (
    !Array.isArray(points) ||
    points.length < 3 ||
    points.length > 128 ||
    points.some(
      (p) =>
        !Array.isArray(p) ||
        p.length !== 2 ||
        p.some(
          (n) =>
            typeof n !== "number" || !Number.isFinite(n) || Math.abs(n) > 1024,
        ),
    )
  )
    throw Error("polygon requires 3..128 bounded 2D points");
  const axes = { xy: [0, 1, 2], xz: [0, 2, 1], yz: [1, 2, 0] }[o.plane ?? "xy"];
  if (!axes) throw Error("extrude plane must be xy/xz/yz");
  const low = Math.floor(Math.min(...points.map((p) => p[1]))),
    high = Math.ceil(Math.max(...points.map((p) => p[1])));
  for (let row = low; row < high; row++) {
    const intersections = [];
    for (let i = 0, j = points.length - 1; i < points.length; j = i++) {
      const a = points[j],
        b = points[i];
      if (a[1] > row + 0.5 !== b[1] > row + 0.5)
        intersections.push(
          a[0] + ((row + 0.5 - a[1]) * (b[0] - a[0])) / (b[1] - a[1]),
        );
    }
    intersections.sort((a, b) => a - b);
    for (let i = 0; i + 1 < intersections.length; i += 2) {
      const lo = Math.ceil(intersections[i] - 0.5),
        hi = Math.ceil(intersections[i + 1] - 0.5);
      if (hi <= lo) continue;
      const at = origin.slice(),
        size = [1, 1, 1];
      at[axes[0]] += lo;
      at[axes[1]] += row;
      size[axes[0]] = hi - lo;
      size[axes[2]] = depth;
      emit("box", at, size, o);
    }
  }
};
const removeGroup = (name) => {
  if (typeof name !== "string" || !name.length || name.length > 128)
    throw Error("Invalid group");
  if (ops.length >= settings.maxOps) throw Error("MAX_OPS exceeded");
  ops.push({ op: "remove_group", name });
};
const faceColor = (value = "#c0c0c0") => {
  if (typeof value === "string" && /^#[0-9a-f]{6}$/i.test(value))
    return [1, 3, 5].map(i => parseInt(value.slice(i, i + 2), 16));
  if (!Array.isArray(value) || value.length !== 3 || value.some(n => !Number.isInteger(n) || n < 0 || n > 255))
    throw Error("triangle color requires #rrggbb or RGB8 array");
  return value.slice();
};
const triangle = (a, b, c, o = {}) => {
  if (ops.length >= settings.maxOps) throw Error("MAX_OPS exceeded");
  const vertices = [a, b, c].map(p => vector(p, "triangle vertex").map((v, i) => offset[i] + mirrors[i] * v));
  if (mirrors.reduce((a,b) => a*b, 1) < 0) [vertices[1], vertices[2]] = [vertices[2], vertices[1]];
  const ab = vertices[1].map((v,i)=>v-vertices[0][i]), ac = vertices[2].map((v,i)=>v-vertices[0][i]);
  if ([ab[1]*ac[2]-ab[2]*ac[1], ab[2]*ac[0]-ab[0]*ac[2], ab[0]*ac[1]-ab[1]*ac[0]].every(v=>v===0))
    throw Error("degenerate triangle");
  ops.push({op:"triangle", vertices, color:faceColor(o.color), doubleSided:!!o.doubleSided, group:o.group ?? currentGroup});
};
const mesh = (vertices, faces, o = {}) => {
  if (!Array.isArray(vertices) || vertices.length > 262144 || !Array.isArray(faces) || faces.length > settings.maxOps)
    throw Error("bounded mesh vertices/faces required");
  vertices = vertices.map(p=>vector(p,"mesh vertex"));
  if (o.colors && (!Array.isArray(o.colors) || o.colors.length !== faces.length)) throw Error("colors must match face count");
  faces.forEach((face,i) => {
    if (!Array.isArray(face) || face.length !== 3 || face.some(n=>!Number.isInteger(n)||n<0||n>=vertices.length)) throw Error("mesh faces require three valid vertex indices");
    triangle(...face.map(n=>vertices[n]), {...o, color:o.colors?.[i] ?? o.color});
  });
};
const triangleStrip = (vertices, o = {}) => {
  if (!Array.isArray(vertices) || vertices.length < 3 || vertices.length > settings.maxOps + 2) throw Error("invalid triangle strip");
  const faces = [];
  for (let i=0;i<vertices.length-2;i++) faces.push(i%2 ? [i+1,i,i+2] : [i,i+1,i+2]);
  mesh(vertices,faces,o);
};
const wedge = (min, size, o = {}) => {
  min=vector(min,"wedge min");size=vector(size,"wedge size");
  if (size.some(n=>n<1)) throw Error("wedge size must be positive");
  const [x,y,z]=min,[ex,ey,ez]=min.map((v,i)=>v+size[i]);
  const options={
    "+x":[[x,y,z],[ex,y,z],[ex,ey,z],[x,y,ez],[ex,y,ez],[ex,ey,ez]],
    "-x":[[ex,y,ez],[x,y,ez],[x,ey,ez],[ex,y,z],[x,y,z],[x,ey,z]],
    "+z":[[ex,y,z],[ex,y,ez],[ex,ey,ez],[x,y,z],[x,y,ez],[x,ey,ez]],
    "-z":[[x,y,ez],[x,y,z],[x,ey,z],[ex,y,ez],[ex,y,z],[ex,ey,z]],
  }, vertices=options[o.direction ?? "+x"];
  if (!vertices) throw Error("wedge direction requires +x/-x/+z/-z");
  const center=[0,1,2].map(a=>vertices.reduce((sum,p)=>sum+p[a],0)/6);
  const faces=[[0,1,2],[3,5,4],[0,3,4],[0,4,1],[1,4,5],[1,5,2],[2,5,3],[2,3,0]].map(face=>{
    const [a,b,c]=face.map(i=>vertices[i]),ab=b.map((v,i)=>v-a[i]),ac=c.map((v,i)=>v-a[i]);
    const n=[ab[1]*ac[2]-ab[2]*ac[1],ab[2]*ac[0]-ab[0]*ac[2],ab[0]*ac[1]-ab[1]*ac[0]];
    const facing=n.reduce((s,v,i)=>s+v*((a[i]+b[i]+c[i])/3-center[i]),0);
    return facing<0 ? [face[0],face[2],face[1]] : face;
  });
  mesh(vertices,faces,o);
};
const mf = Object.freeze({
  triangle,
  mesh,
  triangleStrip,
  wedge,
  voxelize,
  ellipsoid,
  capsule,
  cylinder,
  tube,
  extrude,
  removeGroup,
  sphere(center, radius, o = {}) {
    positive(radius, "radius", 512);
    ellipsoid(center, [radius, radius, radius], o);
  },
  replaceGroup(name, fn) {
    removeGroup(name);
    mf.group(name, fn);
  },
  clear() {
    if (ops.length >= settings.maxOps) throw new Error("MAX_OPS exceeded");
    ops.push({ op: "clear" });
  },
  box(min, size = [1, 1, 1], o = {}) {
    emit("box", min, size, o);
  },
  erase(min, size = [1, 1, 1]) {
    emit("erase", min, size);
  },
  paint(min, size, o = {}) {
    emit("paint", min, size, o);
  },
  random,
  group(name, fn) {
    if (typeof name !== "string" || name.length > 128)
      throw new Error("Invalid group");
    const old = currentGroup;
    currentGroup = name;
    try {
      fn(mf);
    } finally {
      currentGroup = old;
    }
  },
  repeat(count, fn) {
    if (!Number.isInteger(count) || count < 0 || count > 100000)
      throw new Error("Invalid repeat count");
    for (let i = 0; i < count; i++) fn(i, mf);
  },
  translate(at, fn) {
    at = vector(at, "offset");
    const old = offset;
    offset = offset.map((x, i) => x + at[i]);
    try {
      fn(mf);
    } finally {
      offset = old;
    }
  },
  mirror(axis, fn) {
    const a = ["x", "y", "z"].indexOf(axis);
    if (a < 0) throw new Error("Invalid axis");
    mirrors[a] *= -1;
    try {
      fn(mf);
    } finally {
      mirrors[a] *= -1;
    }
  },
  line(a, b, o = {}) {
    a = vector(a, "a");
    b = vector(b, "b");
    const steps = Math.max(...a.map((v, i) => Math.abs(v - b[i])));
    if (steps > 100000) throw new Error("Line too long");
    const t = o.thickness ?? 1;
    for (let i = 0; i <= steps; i++)
      emit(
        "box",
        a.map((v, j) => Math.round(v + (b[j] - v) * (steps ? i / steps : 0))),
        [t, t, t],
        o,
      );
  },
  stairs(at, count, o = {}) {
    at = vector(at, "origin");
    if (!Number.isInteger(count) || count < 1 || count > 10000)
      throw new Error("Invalid steps");
    for (let i = 0; i < count; i++)
      emit(
        "box",
        [at[0] + i * (o.depth ?? 1), at[1], at[2]],
        [o.depth ?? 1, (i + 1) * (o.rise ?? 1), o.width ?? 3],
        o,
      );
  },
  wall(at, width, height, o = {}) {
    emit("box", at, [width, height, o.thickness ?? 1], o);
  },
  roof(at, width, depth, height, o = {}) {
    for (let y = 0; y < height && width - 2 * y > 0; y++)
      emit("box", [at[0] + y, at[1] + y, at[2]], [width - 2 * y, 1, depth], o);
  },
  frame(at, size, o = {}) {
    const t = o.thickness ?? 1;
    emit("box", at, [size[0], t, size[2]], o);
    emit("box", [at[0], at[1] + size[1] - t, at[2]], [size[0], t, size[2]], o);
    emit("box", at, [t, size[1], size[2]], o);
    emit("box", [at[0] + size[0] - t, at[1], at[2]], [t, size[1], size[2]], o);
  },
  text,
  comment(text) {},
});
