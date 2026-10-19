// All input/output positions are signed voxel-grid coordinates (+Z front, +Y up).
const add = (a, b) => a.map((v, i) => v + b[i]);
const sub = (a, b) => a.map((v, i) => v - b[i]);
const mul = (a, s) => a.map((v) => v * s);
const dot = (a, b) => a.reduce((s, v, i) => s + v * b[i], 0);
const cross = (a, b) => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
const norm = (a) => {
  const length = Math.hypot(...a);
  if (length < 1e-9) throw Error("相机方向不能为零");
  return mul(a, 1 / length);
};
export function fitBounds(boxes, fit = {}) {
  if (fit.bounds) return fit.bounds;
  const chosen = fit.group ? boxes.filter((b) => b.group === fit.group) : boxes;
  if (!chosen.length) throw Error("取景区域或分组为空");
  return {
    min: [0, 1, 2].map((i) => Math.min(...chosen.map((b) => b.min[i]))),
    max: [0, 1, 2].map((i) =>
      Math.max(...chosen.map((b) => b.min[i] + b.size[i])),
    ),
  };
}
export function resolveCamera(bounds, shot = {}, aspect = 1) {
  const center = bounds.min.map((v, i) => (v + bounds.max[i]) / 2),
    target = shot.target || center;
  const yaw = ((shot.yaw ?? 35) * Math.PI) / 180,
    pitch = ((shot.pitch ?? 25) * Math.PI) / 180;
  const direction = shot.position
    ? norm(sub(shot.position, target))
    : [
        Math.sin(yaw) * Math.cos(pitch),
        Math.sin(pitch),
        Math.cos(yaw) * Math.cos(pitch),
      ];
  const preferredUp = Math.abs(direction[1]) > 0.999 ? [0, 0, -1] : [0, 1, 0];
  const right = norm(cross(preferredUp, direction)),
    up = norm(cross(direction, right));
  const corners = [];
  for (const x of [bounds.min[0], bounds.max[0]])
    for (const y of [bounds.min[1], bounds.max[1]])
      for (const z of [bounds.min[2], bounds.max[2]])
        corners.push(sub([x, y, z], target));
  const fov = shot.fov ?? 42,
    tanY = Math.tan((fov * Math.PI) / 360),
    tanX = tanY * aspect,
    margin = 1 + (shot.margin ?? 0.12);
  const span = Math.max(...bounds.max.map((v, i) => v - bounds.min[i]), 1);
  const fitDistance = Math.max(
    ...corners.map(
      (p) =>
        dot(p, direction) +
        Math.max(
          (Math.abs(dot(p, right)) * margin) / tanX,
          (Math.abs(dot(p, up)) * margin) / tanY,
        ),
    ),
    span * 0.1,
  );
  const distance = shot.position
    ? Math.hypot(...sub(shot.position, target))
    : fitDistance;
  const halfY =
    Math.max(
      ...corners.map((p) =>
        Math.max(Math.abs(dot(p, up)), Math.abs(dot(p, right)) / aspect),
      ),
      0.5,
    ) * margin;
  return {
    projection: shot.projection || "perspective",
    position: shot.position || add(target, mul(direction, distance)),
    target,
    up,
    fov,
    orthographic: {
      left: -halfY * aspect,
      right: halfY * aspect,
      top: halfY,
      bottom: -halfY,
    },
    near: 0.001,
    far: Math.max(5000, distance + span * 10),
    bounds,
    coordinateSpace: "voxel-grid",
  };
}
