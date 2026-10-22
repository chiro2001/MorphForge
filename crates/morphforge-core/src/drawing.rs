//! Retained point/segment objects. One object is one transport operation;
//! tessellation is only used by export and preview renderers.
use crate::{Bounds, Box3, GeometryError, Limits, Result};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Drawing {
    #[serde(default)]
    pub id: u64,
    pub kind: String,
    pub start: [i32; 3],
    pub end: [i32; 3],
    /// Diameter in 1/64 scene-cell units, avoiding float identity drift.
    pub diameter64: u32,
    pub color: [u8; 3],
    pub group: String,
}
impl Drawing {
    pub fn diameter(&self) -> f64 {
        self.diameter64 as f64 / 64.
    }
    pub fn validate(&self, limits: &Limits) -> Result<()> {
        if !matches!(self.kind.as_str(), "point" | "cube_point" | "segment")
            || self.group.is_empty()
            || self.group.len() > 128
            || self.diameter64 == 0
            || self.diameter64 > 4096 * 64
            || self
                .start
                .iter()
                .chain(self.end.iter())
                .any(|v| (*v as i64).abs() > limits.max_coord as i64)
        {
            return Err(GeometryError("点线类型/坐标/直径或分组无效".into()));
        }
        if self.kind == "segment" && self.start == self.end {
            return Err(GeometryError("线段两端不能重合".into()));
        }
        if self.kind != "segment" && self.start != self.end {
            return Err(GeometryError("点的两个位置必须相同".into()));
        }
        Ok(())
    }
    pub fn bounds(&self) -> Bounds {
        let r = self.diameter() * 0.5;
        Bounds {
            min: std::array::from_fn(|a| {
                (self.start[a].min(self.end[a]) as f64 - r).floor() as i32
            }),
            max: std::array::from_fn(|a| (self.start[a].max(self.end[a]) as f64 + r).ceil() as i32),
        }
    }
    pub fn intersects(&self, b: &Box3) -> bool {
        let bounds = self.bounds();
        let end = b.end();
        (0..3).all(|a| bounds.min[a] < end[a] && bounds.max[a] >= b.min[a])
    }
    pub fn wire(&self) -> Result<([u8; 3], [u8; 3], u16)> {
        self.validate(&Limits::default())?;
        let b = self.bounds();
        if (0..3).any(|a| {
            b.min[a] < if a == 1 { 0 } else { -128 } || b.max[a] > if a == 1 { 256 } else { 128 }
        }) {
            return Err(GeometryError("点线含粗细的范围须位于256格显示区域".into()));
        }
        let mut start = [0; 3];
        let mut end = [0; 3];
        for a in 0..3 {
            let offset = if a == 1 { 0 } else { 128 };
            start[a] = u8::try_from(self.start[a] + offset)
                .map_err(|_| GeometryError("点线端点越界".into()))?;
            end[a] = u8::try_from(self.end[a] + offset)
                .map_err(|_| GeometryError("点线端点越界".into()))?;
        }
        Ok((
            start,
            end,
            u16::try_from(self.diameter64).map_err(|_| GeometryError("点线直径越界".into()))?,
        ))
    }
    pub fn mesh(&self) -> Vec<[[f64; 3]; 3]> {
        let a = self.start.map(f64::from);
        let b = self.end.map(f64::from);
        let r = self.diameter() * 0.5;
        let mut faces = vec![];
        let add = |p: [f64; 3], q: [f64; 3]| std::array::from_fn(|i| p[i] + q[i]);
        let mul = |p: [f64; 3], s: f64| p.map(|v| v * s);
        let cross = |p: [f64; 3], q: [f64; 3]| {
            [
                p[1] * q[2] - p[2] * q[1],
                p[2] * q[0] - p[0] * q[2],
                p[0] * q[1] - p[1] * q[0],
            ]
        };
        let unit = |p: [f64; 3]| {
            let n = p.iter().map(|v| v * v).sum::<f64>().sqrt();
            p.map(|v| v / n)
        };
        if self.kind == "segment" {
            let d = unit(std::array::from_fn(|i| b[i] - a[i]));
            let axis = (0..3)
                .min_by(|i, j| d[*i].abs().total_cmp(&d[*j].abs()))
                .unwrap();
            let mut helper = [0.; 3];
            helper[axis] = 1.;
            let u = unit(cross(d, helper));
            let v = cross(d, u);
            // Two hemispheres and a cylinder, matching the retained capsule preview.
            let rings = (0..8)
                .map(|j| {
                    let (center, phi) = if j < 4 {
                        (
                            a,
                            -std::f64::consts::FRAC_PI_2 + j as f64 * std::f64::consts::PI / 6.,
                        )
                    } else {
                        (b, (j - 4) as f64 * std::f64::consts::PI / 6.)
                    };
                    (add(center, mul(d, r * phi.sin())), r * phi.cos())
                })
                .collect::<Vec<_>>();
            for j in 0..7 {
                for i in 0..8 {
                    let point = |k: usize, n: usize| {
                        let angle = std::f64::consts::TAU * n as f64 / 8.;
                        add(
                            rings[k].0,
                            mul(add(mul(u, angle.cos()), mul(v, angle.sin())), rings[k].1),
                        )
                    };
                    if j > 0 {
                        faces.push([point(j, i), point(j, i + 1), point(j + 1, i)]);
                    }
                    if j < 6 {
                        faces.push([point(j, i + 1), point(j + 1, i + 1), point(j + 1, i)]);
                    }
                }
            }
        } else if self.kind == "cube_point" {
            let p = (0..8)
                .map(|i| {
                    add(
                        a,
                        std::array::from_fn(|k| if i & (1 << k) == 0 { -r } else { r }),
                    )
                })
                .collect::<Vec<_>>();
            for q in [
                [0, 1, 3, 2],
                [4, 6, 7, 5],
                [0, 4, 5, 1],
                [2, 3, 7, 6],
                [0, 2, 6, 4],
                [1, 5, 7, 3],
            ] {
                faces.extend([[p[q[0]], p[q[1]], p[q[2]]], [p[q[0]], p[q[2]], p[q[3]]]]);
            }
        } else {
            for y in 0..6 {
                for x in 0..10 {
                    let point = |xx: usize, yy: usize| {
                        let theta = std::f64::consts::TAU * xx as f64 / 10.;
                        let phi = std::f64::consts::PI * yy as f64 / 6.;
                        add(
                            a,
                            [
                                r * phi.sin() * theta.cos(),
                                r * phi.cos(),
                                r * phi.sin() * theta.sin(),
                            ],
                        )
                    };
                    if y > 0 {
                        faces.push([point(x, y), point(x + 1, y), point(x, y + 1)]);
                    }
                    if y < 5 {
                        faces.push([point(x + 1, y), point(x + 1, y + 1), point(x, y + 1)]);
                    }
                }
            }
        }
        if self.kind != "point" {
            for face in &mut faces {
                let n = cross(
                    std::array::from_fn(|i| face[1][i] - face[0][i]),
                    std::array::from_fn(|i| face[2][i] - face[0][i]),
                );
                let centroid =
                    std::array::from_fn::<_, 3, _>(|i| (face[0][i] + face[1][i] + face[2][i]) / 3.);
                let axis = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
                let norm = axis.iter().map(|v| v * v).sum::<f64>();
                let t = if norm > 0. {
                    (0..3).map(|i| (centroid[i] - a[i]) * axis[i]).sum::<f64>() / norm
                } else {
                    0.
                }
                .clamp(0., 1.);
                let center = std::array::from_fn::<_, 3, _>(|i| centroid[i] - a[i] - axis[i] * t);
                if n.iter().zip(center).map(|(x, y)| x * y).sum::<f64>() < 0. {
                    face.swap(1, 2);
                }
            }
        }
        faces
    }
}
