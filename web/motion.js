// Pure animation math shared by playback, camera bounds and tests.
export function quaternionXYZ(rotation) {
  const [x, y, z] = rotation.map((v) => (v * Math.PI) / 360),
    s1 = Math.sin(x),
    c1 = Math.cos(x),
    s2 = Math.sin(y),
    c2 = Math.cos(y),
    s3 = Math.sin(z),
    c3 = Math.cos(z);
  return [
    s1 * c2 * c3 + c1 * s2 * s3,
    c1 * s2 * c3 - s1 * c2 * s3,
    c1 * c2 * s3 + s1 * s2 * c3,
    c1 * c2 * c3 - s1 * s2 * s3,
  ];
}
export function slerp(a, b, t) {
  let dot = a.reduce((s, v, i) => s + v * b[i], 0);
  if (dot < 0) {
    b = b.map((v) => -v);
    dot = -dot;
  }
  let q;
  if (dot > 0.9995) q = a.map((v, i) => v + (b[i] - v) * t);
  else {
    const theta = Math.acos(Math.max(-1, Math.min(1, dot))),
      s = Math.sin(theta);
    q = a.map(
      (v, i) =>
        (v * Math.sin((1 - t) * theta) + b[i] * Math.sin(t * theta)) / s,
    );
  }
  const n = Math.hypot(...q);
  return q.map((v) => v / n);
}
export function sampleTrack(track, time) {
  const keys = track.keys;
  let a = keys[0],
    b = a;
  if (time >= keys.at(-1).time) a = b = keys.at(-1);
  else if (time > a.time) {
    for (let i = 1; i < keys.length; i++)
      if (time < keys[i].time) {
        a = keys[i - 1];
        b = keys[i];
        break;
      } else if (time === keys[i].time) {
        a = b = keys[i];
        break;
      }
  }
  const t =
    a === b || track.interpolation === "step"
      ? 0
      : (time - a.time) / (b.time - a.time);
  return {
    translation: a.translation.map((v, i) => v + (b.translation[i] - v) * t),
    scale: a.scale.map((v, i) => v + (b.scale[i] - v) * t),
    quaternion: slerp(quaternionXYZ(a.rotation), quaternionXYZ(b.rotation), t),
  };
}
export function transformPoint(point, track, pose) {
  const v = point.map((n, i) => (n - track.pivot[i]) * pose.scale[i]),
    [x, y, z, w] = pose.quaternion;
  const cross = [y * v[2] - z * v[1], z * v[0] - x * v[2], x * v[1] - y * v[0]],
    cross2 = [
      y * cross[2] - z * cross[1],
      z * cross[0] - x * cross[2],
      x * cross[1] - y * cross[0],
    ];
  return v.map(
    (n, i) =>
      n +
      2 * w * cross[i] +
      2 * cross2[i] +
      track.pivot[i] +
      pose.translation[i],
  );
}
export function poseBounds(boxes, triangles, clip, time) {
  const tracks = new Map(
    clip.tracks.map((t) => [t.group, { track: t, pose: sampleTrack(t, time) }]),
  );
  const points = (group, ps) => {
    const t = tracks.get(group);
    return t ? ps.map((p) => transformPoint(p, t.track, t.pose)) : ps;
  };
  const bounds = (id, group, ps) => {
    ps = points(group, ps);
    const min = [0, 1, 2].map((a) => Math.min(...ps.map((p) => p[a]))),
      max = [0, 1, 2].map((a) => Math.max(...ps.map((p) => p[a])));
    return { id, group, min, size: max.map((v, a) => v - min[a]) };
  };
  return boxes
    .map((b) =>
      bounds(
        b.id,
        b.group,
        [0, 1, 2, 3, 4, 5, 6, 7].map((i) =>
          b.min.map((v, a) => v + ((i >> a) & 1) * b.size[a]),
        ),
      ),
    )
    .concat(triangles.map((t) => bounds(t.id, t.group, t.vertices)));
}

// Keep authored full turns at exact keys; expose the actual interpolated pose
// when inserting a new key between them (XYZ decomposition, matching the viewer).
export function keyframeAt(track, time) {
  const exact = track.keys.find((k) => Math.abs(k.time - time) < 1e-7);
  if (exact)
    return {
      ...exact,
      translation: [...exact.translation],
      rotation: [...exact.rotation],
      scale: [...exact.scale],
    };
  const pose = sampleTrack(track, time),
    [x, y, z, w] = pose.quaternion;
  const m13 = 2 * (x * z + y * w),
    pitch = Math.asin(Math.max(-1, Math.min(1, m13)));
  const rx =
    Math.abs(m13) < 0.9999999
      ? Math.atan2(2 * (x * w - y * z), 1 - 2 * (x * x + y * y))
      : Math.atan2(2 * (x * w + y * z), 1 - 2 * (x * x + z * z));
  const rz =
    Math.abs(m13) < 0.9999999
      ? Math.atan2(2 * (z * w - x * y), 1 - 2 * (y * y + z * z))
      : 0;
  return {
    time,
    translation: pose.translation,
    rotation: [rx, pitch, rz].map((v) => (v * 180) / Math.PI),
    scale: pose.scale,
  };
}
