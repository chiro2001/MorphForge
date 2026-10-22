//! Demonstration primitives compiled into the existing editable RGB triangles.
//! The integer mesh is shared by the editor, exports and MF6; no screen-only lines.
use crate::store::Workspace;
use anyhow::{bail, ensure, Context, Result};
use morphforge_core::{Limits, Operation, Scene, Triangle3};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::f64::consts::TAU;

type V = [f64; 3];
fn add(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] + b[i])
}
fn sub(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] - b[i])
}
fn mul(a: V, s: f64) -> V {
    a.map(|v| v * s)
}
fn dot(a: V, b: V) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: V) -> Result<V> {
    let length = dot(a, a).sqrt();
    ensure!(length.is_finite() && length >= 1e-9, "线段两端不能重合");
    Ok(mul(a, 1. / length))
}
fn frame(a: V, b: V) -> Result<(V, V, V)> {
    let d = unit(sub(b, a))?;
    let axis = (0..3)
        .min_by(|a, b| d[*a].abs().total_cmp(&d[*b].abs()))
        .unwrap();
    let mut helper = [0.; 3];
    helper[axis] = 1.;
    let u = unit(cross(d, helper))?;
    Ok((d, u, cross(d, u)))
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Primitive {
    pub kind: String,
    pub position: V,
    pub start: V,
    pub end: V,
    pub points: Vec<V>,
    pub size: f64,
    pub width: f64,
    pub radius: f64,
    pub length: f64,
    pub extent: V,
    pub color: [u8; 3],
    pub group: String,
    pub shape: String,
    pub plane: String,
    pub sides: usize,
    pub segments: usize,
    pub closed: bool,
    pub dashed: bool,
    pub dash_length: f64,
    pub gap_length: f64,
}
impl Default for Primitive {
    fn default() -> Self {
        Self {
            kind: "point".into(),
            position: [0., 24., 0.],
            start: [-24., 24., 0.],
            end: [24., 24., 0.],
            points: vec![],
            size: 6.,
            width: 2.,
            radius: 24.,
            length: 48.,
            extent: [32., 24., 24.],
            color: [88, 190, 255],
            group: "demo".into(),
            shape: "sphere".into(),
            plane: "xy".into(),
            sides: 8,
            segments: 32,
            closed: false,
            dashed: false,
            dash_length: 8.,
            gap_length: 4.,
        }
    }
}

struct Mesh {
    faces: Vec<Triangle3>,
    drawings: Vec<morphforge_core::drawing::Drawing>,
    native: bool,
    collapsed: usize,
    max_error: f64,
    limit: usize,
}
impl Mesh {
    fn triangle(&mut self, vertices: [V; 3], normal: V, p: &Primitive) -> Result<()> {
        ensure!(
            self.faces.len() < self.limit,
            "演示图元超过本次三角面预算（{}），请减少点数或圆周细分",
            self.limit
        );
        for v in vertices.iter().flatten() {
            ensure!(
                v.is_finite() && v.abs() <= 1_000_000.,
                "图元顶点超过工程坐标保护范围"
            );
            self.max_error = self.max_error.max((v - v.round()).abs());
        }
        let mut vertices = vertices.map(|v| v.map(|n| n.round() as i32));
        let a = sub(vertices[1].map(f64::from), vertices[0].map(f64::from));
        let b = sub(vertices[2].map(f64::from), vertices[0].map(f64::from));
        let n = cross(a, b);
        if dot(n, n) == 0. {
            self.collapsed += 1;
            return Ok(());
        }
        if dot(n, normal) < 0. {
            vertices.swap(1, 2);
        }
        let mut face = Triangle3::new(vertices, p.color);
        face.group = p.group.clone();
        face.validate(&Limits::default())?;
        self.faces.push(face);
        Ok(())
    }
    fn point(&mut self, center: V, size: f64, p: &Primitive) -> Result<()> {
        if self.native {
            return self.native_drawing(
                center,
                center,
                size,
                if p.shape == "cube" {
                    "cube_point"
                } else {
                    "point"
                },
                p,
            );
        }
        if p.shape == "cube" {
            let v = (0..8)
                .map(|i| {
                    std::array::from_fn(|a| {
                        center[a]
                            + if i & (1 << a) == 0 {
                                -size / 2.
                            } else {
                                size / 2.
                            }
                    })
                })
                .collect::<Vec<V>>();
            for q in [
                [0, 1, 3, 2],
                [4, 6, 7, 5],
                [0, 4, 5, 1],
                [2, 3, 7, 6],
                [0, 2, 6, 4],
                [1, 5, 7, 3],
            ] {
                let normal = sub(mul(add(v[q[0]], v[q[2]]), 0.5), center);
                self.triangle([v[q[0]], v[q[1]], v[q[2]]], normal, p)?;
                self.triangle([v[q[0]], v[q[2]], v[q[3]]], normal, p)?;
            }
        } else {
            let t = (1. + 5_f64.sqrt()) / 2.;
            let raw = [
                [-1., t, 0.],
                [1., t, 0.],
                [-1., -t, 0.],
                [1., -t, 0.],
                [0., -1., t],
                [0., 1., t],
                [0., -1., -t],
                [0., 1., -t],
                [t, 0., -1.],
                [t, 0., 1.],
                [-t, 0., -1.],
                [-t, 0., 1.],
            ];
            let v = raw.map(|v| add(center, mul(v, size / 2. / dot(v, v).sqrt())));
            for f in [
                [0, 11, 5],
                [0, 5, 1],
                [0, 1, 7],
                [0, 7, 10],
                [0, 10, 11],
                [1, 5, 9],
                [5, 11, 4],
                [11, 10, 2],
                [10, 7, 6],
                [7, 1, 8],
                [3, 9, 4],
                [3, 4, 2],
                [3, 2, 6],
                [3, 6, 8],
                [3, 8, 9],
                [4, 9, 5],
                [2, 4, 11],
                [6, 2, 10],
                [8, 6, 7],
                [9, 8, 1],
            ] {
                let points = f.map(|i| v[i]);
                self.triangle(
                    points,
                    sub(add(add(points[0], points[1]), points[2]), mul(center, 3.)),
                    p,
                )?;
            }
        }
        Ok(())
    }
    fn tube(&mut self, a: V, b: V, width: f64, p: &Primitive) -> Result<()> {
        if self.native {
            return self.native_drawing(a, b, width, "segment", p);
        }
        let (d, u, v) = frame(a, b)?;
        ensure!(
            sub(a.map(f64::round), b.map(f64::round)) != [0.; 3],
            "线段短于当前整数网格精度，请拉开端点"
        );
        for i in 0..p.sides {
            let ring = |j: usize| {
                let theta = TAU * j as f64 / p.sides as f64;
                mul(add(mul(u, theta.cos()), mul(v, theta.sin())), width / 2.)
            };
            let r = ring(i);
            let s = ring(i + 1);
            self.triangle([add(a, r), add(a, s), add(b, s)], add(r, s), p)?;
            self.triangle([add(a, r), add(b, s), add(b, r)], add(r, s), p)?;
            self.triangle([a, add(a, s), add(a, r)], mul(d, -1.), p)?;
            self.triangle([b, add(b, r), add(b, s)], d, p)?;
        }
        Ok(())
    }
    fn native_drawing(
        &mut self,
        a: V,
        b: V,
        diameter: f64,
        kind: &str,
        p: &Primitive,
    ) -> Result<()> {
        ensure!(
            self.faces.len() + self.drawings.len() < self.limit,
            "点线超过本次对象预算"
        );
        for v in a.iter().chain(b.iter()) {
            ensure!(v.is_finite() && v.abs() <= 1_000_000., "点线坐标越界");
            self.max_error = self.max_error.max((v - v.round()).abs());
        }
        let value = morphforge_core::drawing::Drawing {
            id: 0,
            kind: kind.into(),
            start: a.map(|v| v.round() as i32),
            end: b.map(|v| v.round() as i32),
            diameter64: (diameter * 64.).round() as u32,
            color: p.color,
            group: p.group.clone(),
        };
        value.validate(&Limits::default())?;
        self.drawings.push(value);
        Ok(())
    }
    fn segment(&mut self, a: V, b: V, p: &Primitive) -> Result<()> {
        self.stroke(a, b, p, 0.)
    }
    fn stroke(&mut self, a: V, b: V, p: &Primitive, phase: f64) -> Result<()> {
        if !p.dashed {
            return self.tube(a, b, p.width, p);
        }
        let d = sub(b, a);
        let length = dot(d, d).sqrt();
        unit(d)?;
        let period = p.dash_length + p.gap_length;
        let phase = phase.rem_euclid(period);
        let count = ((length + phase) / period).ceil() as usize;
        ensure!(count <= 512, "虚线段数超过512，请增大虚线长度或间隔");
        for i in 0..count {
            let from = i as f64 * period - phase;
            let to = (from + p.dash_length).min(length);
            let from = from.max(0.);
            if to - from >= 1. {
                self.tube(
                    add(a, mul(d, from / length)),
                    add(a, mul(d, to / length)),
                    p.width,
                    p,
                )?;
            }
        }
        Ok(())
    }
    fn arrow(&mut self, a: V, b: V, p: &Primitive) -> Result<()> {
        let (d, u, v) = frame(a, b)?;
        let length = dot(sub(b, a), sub(b, a)).sqrt();
        ensure!(
            length >= p.width * 2. + 1.,
            "箭头长度需要至少为线宽的2倍再加1格"
        );
        let head = (p.width * 4.).min(length * 0.4).max(2.);
        let base = sub(b, mul(d, head));
        self.segment(a, base, p)?;
        for i in 0..p.sides {
            let ring = |j: usize| {
                let theta = TAU * j as f64 / p.sides as f64;
                add(
                    base,
                    mul(add(mul(u, theta.cos()), mul(v, theta.sin())), p.width * 1.5),
                )
            };
            let x = ring(i);
            let y = ring(i + 1);
            self.triangle(
                [x, y, b],
                add(sub(add(x, y), mul(base, 2.)), mul(d, p.width)),
                p,
            )?;
            self.triangle([base, y, x], mul(d, -1.), p)?;
        }
        Ok(())
    }
    fn polyline(&mut self, points: &[V], closed: bool, p: &Primitive) -> Result<()> {
        ensure!((2..=256).contains(&points.len()), "折线需要2..256个点");
        let points = if closed && points.first() == points.last() {
            &points[..points.len() - 1]
        } else {
            points
        };
        ensure!(!closed || points.len() >= 3, "闭合折线需要至少3个不同的点");
        let mut phase = 0.;
        for pair in points.windows(2) {
            self.stroke(pair[0], pair[1], p, phase)?;
            phase += dot(sub(pair[1], pair[0]), sub(pair[1], pair[0])).sqrt();
        }
        if closed {
            self.stroke(*points.last().unwrap(), points[0], p, phase)?;
        }
        if !p.dashed && !self.native {
            for point in if closed {
                points
            } else {
                &points[1..points.len() - 1]
            } {
                self.point(*point, p.width, p)?;
            }
        }
        Ok(())
    }
    fn primitive(&mut self, p: &Primitive) -> Result<()> {
        let start = self.faces.len() + self.drawings.len();
        ensure!(
            !p.group.is_empty() && p.group.len() <= 128,
            "分组名称需要1..128字节"
        );
        ensure!((4..=16).contains(&p.sides), "圆周细分sides需要4..16");
        for v in [&p.position, &p.start, &p.end, &p.extent]
            .into_iter()
            .chain(p.points.iter())
            .flatten()
        {
            ensure!(
                v.is_finite() && v.abs() <= 999_000.,
                "图元坐标需要有界的有限数值"
            );
        }
        for (name, value) in [
            ("size", p.size),
            ("width", p.width),
            ("radius", p.radius),
            ("length", p.length),
            ("dashLength", p.dash_length),
            ("gapLength", p.gap_length),
        ] {
            ensure!(
                value.is_finite()
                    && ((if self.native && matches!(name, "size" | "width") {
                        1. / 64.
                    } else {
                        1.
                    })..=4096.)
                        .contains(&value),
                "{name}超出图元大小范围；原生点线直径最小1/64格"
            );
        }
        ensure!(
            matches!(p.shape.as_str(), "sphere" | "cube"),
            "点形状需要sphere/cube"
        );
        match p.kind.as_str() {
            "point" => self.point(p.position, p.size, p)?,
            "points" => {
                ensure!((1..=256).contains(&p.points.len()), "点集需要1..256个点");
                for v in &p.points {
                    self.point(*v, p.size, p)?;
                }
            }
            "segment" => self.segment(p.start, p.end, p)?,
            "polyline" => self.polyline(&p.points, p.closed, p)?,
            "arrow" => self.arrow(p.start, p.end, p)?,
            "axes" => {
                for (axis, color) in [[235, 85, 85], [92, 210, 118], [85, 145, 255]]
                    .into_iter()
                    .enumerate()
                {
                    let mut q = p.clone();
                    q.color = color;
                    let mut end = p.position;
                    end[axis] += p.length;
                    self.arrow(p.position, end, &q)?;
                }
            }
            "circle" => {
                ensure!((8..=128).contains(&p.segments), "圆环segments需要8..128");
                ensure!(
                    p.radius > p.width / 2. + 1.,
                    "圆环半径必须大于半线宽再加1格"
                );
                let (u, v) = match p.plane.as_str() {
                    "xy" => ([1., 0., 0.], [0., 1., 0.]),
                    "xz" => ([1., 0., 0.], [0., 0., 1.]),
                    "yz" => ([0., 1., 0.], [0., 0., 1.]),
                    _ => bail!("圆环平面需要xy/xz/yz"),
                };
                if p.dashed || self.native {
                    let points = (0..p.segments)
                        .map(|i| {
                            let t = TAU * i as f64 / p.segments as f64;
                            add(
                                p.position,
                                mul(add(mul(u, t.cos()), mul(v, t.sin())), p.radius),
                            )
                        })
                        .collect::<Vec<_>>();
                    self.polyline(&points, true, p)?;
                } else {
                    let n = cross(u, v);
                    let vertex = |i: usize, j: usize| {
                        let a = TAU * i as f64 / p.segments as f64;
                        let b = TAU * j as f64 / p.sides as f64;
                        let r = add(mul(u, a.cos()), mul(v, a.sin()));
                        add(
                            p.position,
                            add(
                                mul(r, p.radius + p.width / 2. * b.cos()),
                                mul(n, p.width / 2. * b.sin()),
                            ),
                        )
                    };
                    for i in 0..p.segments {
                        for j in 0..p.sides {
                            let a = vertex(i, j);
                            let b = vertex(i + 1, j);
                            let c = vertex(i + 1, j + 1);
                            let d = vertex(i, j + 1);
                            let mid = TAU * (i as f64 + 0.5) / p.segments as f64;
                            let center = add(
                                p.position,
                                mul(add(mul(u, mid.cos()), mul(v, mid.sin())), p.radius),
                            );
                            let normal = sub(mul(add(a, c), 0.5), center);
                            self.triangle([a, b, c], normal, p)?;
                            self.triangle([a, c, d], normal, p)?;
                        }
                    }
                }
            }
            "wire_box" => {
                ensure!(
                    p.extent.iter().all(|x| *x >= 1. && *x <= 4096.),
                    "线框尺寸extent需要三个1..4096格的数"
                );
                let v = (0..8)
                    .map(|i| {
                        std::array::from_fn(|a| {
                            p.position[a] + if i & (1 << a) == 0 { 0. } else { p.extent[a] }
                        })
                    })
                    .collect::<Vec<V>>();
                for i in 0..8 {
                    for axis in 0..3 {
                        if i & (1 << axis) == 0 {
                            self.segment(v[i], v[i | (1 << axis)], p)?;
                        }
                    }
                }
            }
            _ => bail!("图元kind需要point/points/segment/polyline/arrow/circle/axes/wire_box"),
        }
        ensure!(
            self.faces.len() + self.drawings.len() > start,
            "图元小于整数网格精度，请增大大小/粗细或拉开端点"
        );
        Ok(())
    }
}

pub fn catalog() -> Value {
    json!({"kinds":["point","points","segment","polyline","arrow","circle","axes","wire_box"],
        "units":"逻辑格；点线端点为整数，直径精度1/64格",
        "color":"RGB8，不透明逐面颜色","maxPrimitives":64,"maxPoints":256,
        "supportsDashed":["segment","polyline","arrow","circle","axes","wire_box"],
        "representations":["native","mesh"],"defaultRepresentation":"native","note":"size为点直径、width为线直径；默认保存原生点线，每个点/线段一个传输对象；mesh为旧头像三角面兼容模式。均计入几何容量，不增加同步参数。"})
}

pub fn execute(w: &mut Workspace, args: &Value, run: Option<&str>) -> Result<Value> {
    if args["projectId"]
        .as_str()
        .is_some_and(|id| id != w.project.id)
    {
        bail!("WORKSPACE_CONFLICT：工程已切换")
    }
    let action = args["action"].as_str().unwrap_or("catalog");
    if action == "catalog" {
        return Ok(catalog());
    }
    ensure!(
        matches!(action, "preview" | "apply"),
        "action需要catalog/preview/apply"
    );
    let base = args["baseRevision"]
        .as_u64()
        .context("演示图元需要baseRevision")?;
    let mut primitives: Vec<Primitive> =
        serde_json::from_value(args["primitives"].clone()).context("primitives需要图元数组")?;
    ensure!(
        (1..=64).contains(&primitives.len()),
        "每次需要1..64个演示图元"
    );
    let common = args["group"].as_str();
    if let Some(group) = common {
        for p in &mut primitives {
            p.group = group.into();
        }
    }
    let mode = args["mode"].as_str().unwrap_or("append");
    ensure!(
        matches!(mode, "append" | "replace_group"),
        "mode需要append/replace_group"
    );
    if mode == "replace_group" {
        ensure!(
            common.is_some_and(|g| !g.is_empty()),
            "替换同名分组需要显式group"
        );
    }
    let fingerprint = crate::store::hash(&serde_json::to_vec(
        &json!({"projectId":w.project.id,"baseRevision":base,"mode":mode,"group":common,"representation":args["representation"],"primitives":primitives}),
    )?);
    let request = args["requestId"].as_str();
    if let Some(id) = request {
        ensure!(!id.is_empty() && id.len() <= 128, "requestId需要1..128字节");
        if action == "apply" {
            if let Some(event) = w.events.iter().rev().find(|e| {
                e.kind == "drawing.applied"
                    && e.data["projectId"] == w.project.id
                    && e.data["requestId"] == id
            }) {
                ensure!(
                    event.data["fingerprint"] == fingerprint,
                    "REQUEST_ID_CONFLICT：requestId已用于不同演示图元"
                );
                let mut result = event.data["result"].clone();
                result["replayed"] = json!(true);
                return Ok(result);
            }
        }
    }
    ensure!(
        base == w.project.scene.revision,
        "REVISION_CONFLICT：工程已变化，请重新预览演示图元"
    );
    let mut mesh = Mesh {
        faces: vec![],
        drawings: vec![],
        native: args["representation"] != "mesh",
        collapsed: 0,
        max_error: 0.,
        limit: w.project.limits.max_ops.saturating_sub(1).min(32768),
    };
    for p in &primitives {
        mesh.primitive(p)?;
    }
    let mut operations = Vec::new();
    if mode == "replace_group" {
        operations.push(Operation::RemoveGroup {
            name: common.unwrap().into(),
        });
    }
    operations.extend(
        mesh.faces
            .iter()
            .cloned()
            .map(|value| Operation::Triangle { value }),
    );
    operations.extend(
        mesh.drawings
            .iter()
            .cloned()
            .map(|value| Operation::Drawing { value }),
    );
    let preview = w.project.scene.apply(&operations, &w.project.limits)?;
    w.ensure_protected_groups_unchanged(&w.project.scene, &preview)?;
    let target = crate::scene_tools::target_report(&preview, w.project.limits.target_boxes);
    let mut generated = Scene::default();
    generated.triangles = mesh.faces.clone();
    generated.drawings = mesh.drawings.clone();
    let report = json!({"primitiveCount":primitives.len(),"triangleCount":mesh.faces.len(),"drawingCount":mesh.drawings.len(),"representation":if mesh.native{"native"}else{"mesh"},
        "bounds":generated.stats(&w.project.limits).bounds,"skippedCollapsedFaces":mesh.collapsed,
        "maximumCoordinateRounding":mesh.max_error,"units":"逻辑格，顶点对齐整数网格",
        "note":"点线保留端点与直径，每个点或线段按一个原生对象传输；粗细随距离缩放。箭头尖端保留多边形。可按分组移动/复制/删除/动画。"});
    if action == "preview" {
        return Ok(
            json!({"ok":true,"dryRun":true,"revision":base,"projectId":w.project.id,"report":report,"target":target,"triangles":mesh.faces,"drawings":mesh.drawings,"primitives":primitives}),
        );
    }
    // Namespaced transaction keys prevent accidental replay of unrelated editors.
    let key = request.map(|id| {
        format!(
            "drawing:{}:{}",
            crate::store::hash(id.as_bytes()),
            fingerprint
        )
    });
    let mut result = w.apply(&operations, Some(base), key.as_deref(), run)?;
    result["report"] = report;
    result["target"] = target;
    result["projectId"] = json!(w.project.id);
    w.event("drawing.applied",run,json!({"projectId":w.project.id,"requestId":request,"fingerprint":fingerprint,"primitives":primitives,"result":result}))?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mesh(p: Value) -> Mesh {
        let mut m = Mesh {
            faces: vec![],
            drawings: vec![],
            native: false,
            collapsed: 0,
            max_error: 0.,
            limit: 32768,
        };
        m.primitive(&serde_json::from_value(p).unwrap()).unwrap();
        m
    }
    #[test]
    fn points_and_every_segment_direction_generate_valid_colored_solid_faces() {
        for p in [
            json!({"kind":"point","size":8,"color":[22,80,144]}),
            json!({"kind":"point","shape":"cube","size":8,"color":[22,80,144]}),
        ] {
            let m = mesh(p);
            assert!(m.faces.len() >= 12);
            assert!(m.faces.iter().all(|t| t.color == [22, 80, 144]));
        }
        for end in [
            [24., 0., 0.],
            [0., 24., 0.],
            [0., 0., 24.],
            [-24., -18., 8.],
        ] {
            let m = mesh(json!({"kind":"segment","start":[0,0,0],"end":end,"width":4}));
            assert!(m.faces.len() >= 16);
            for t in m.faces {
                t.validate(&Limits::default()).unwrap();
            }
        }
    }
    #[test]
    fn widths_change_geometry_and_arrow_tip_is_the_requested_endpoint() {
        let a = mesh(json!({"kind":"segment","start":[0,8,0],"end":[24,8,0],"width":2}));
        let b = mesh(json!({"kind":"segment","start":[0,8,0],"end":[24,8,0],"width":8}));
        let span = |m: &Mesh| {
            m.faces
                .iter()
                .flat_map(|t| t.vertices)
                .map(|v| v[2])
                .max()
                .unwrap()
                - m.faces
                    .iter()
                    .flat_map(|t| t.vertices)
                    .map(|v| v[2])
                    .min()
                    .unwrap()
        };
        assert!(span(&b) > span(&a));
        let m = mesh(json!({"kind":"arrow","start":[0,8,0],"end":[32,8,0],"width":2}));
        assert!(m.faces.iter().any(|t| t.vertices.contains(&[32, 8, 0])));
        assert!(m.faces.iter().flat_map(|t| t.vertices).all(|v| v[0] <= 32));
    }
    #[test]
    fn dashed_segments_leave_real_gaps_and_other_demonstration_shapes_work() {
        let m = mesh(
            json!({"kind":"segment","start":[0,8,0],"end":[32,8,0],"width":2,"dashed":true,"dashLength":8,"gapLength":8}),
        );
        assert!(m.faces.iter().all(|t| t.vertices.iter().all(|v| v[0] <= 8)
            || t.vertices.iter().all(|v| v[0] >= 16 && v[0] <= 24)));
        for p in [
            json!({"kind":"points","points":[[0,8,0],[8,16,0]]}),
            json!({"kind":"polyline","points":[[0,8,0],[16,8,0],[16,24,0]],"closed":true}),
            json!({"kind":"circle","position":[0,32,0],"radius":16,"width":2}),
            json!({"kind":"axes","position":[0,8,0],"length":32}),
            json!({"kind":"wire_box","position":[-16,8,-16]}),
        ] {
            assert!(!mesh(p).faces.is_empty());
        }
    }
    #[test]
    fn dashed_polyline_keeps_its_phase_across_short_edges() {
        let m = mesh(
            json!({"kind":"polyline","points":[[0,8,0],[6,8,0],[12,8,0],[18,8,0]],"width":2,"dashed":true,"dashLength":8,"gapLength":8}),
        );
        assert!(m
            .faces
            .iter()
            .flat_map(|t| t.vertices)
            .all(|v| v[0] <= 8 || v[0] >= 16));
        assert!(m.faces.iter().flat_map(|t| t.vertices).any(|v| v[0] == 18));
    }
    #[test]
    fn invalid_or_collapsed_primitives_fail_without_modifying_a_workspace() {
        let root = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(root.path().into()).unwrap();
        for p in [
            json!({"kind":"segment","start":[0,8,0],"end":[0,8,0]}),
            json!({"kind":"point","size":0}),
            json!({"kind":"polyline","points":[[0,0,0]]}),
            json!({"kind":"circle","segments":100000}),
            json!({"kind":"point","position":[1e20,0,0]}),
        ] {
            assert!(execute(
                &mut w,
                &json!({"action":"apply","baseRevision":0,"primitives":[p]}),
                None
            )
            .is_err());
            assert_eq!(w.project.scene.revision, 0);
            assert!(w.project.scene.triangles.is_empty());
        }
    }
}
