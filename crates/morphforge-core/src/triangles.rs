//! Editable, per-face RGB geometry. No texture coordinates or hidden quantization.
use crate::{Bounds, Box3, GeometryError, Limits, Result};
use serde::{Deserialize, Serialize};
fn group() -> String {
    "main".into()
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Triangle3 {
    #[serde(default)]
    pub id: u64,
    pub vertices: [[i32; 3]; 3],
    pub color: [u8; 3],
    #[serde(default)]
    pub double_sided: bool,
    #[serde(default = "group")]
    pub group: String,
}
impl Triangle3 {
    pub fn new(vertices: [[i32; 3]; 3], color: [u8; 3]) -> Self {
        Self {
            id: 0,
            vertices,
            color,
            double_sided: false,
            group: group(),
        }
    }
    pub fn validate(&self, limits: &Limits) -> Result<()> {
        if self.group.is_empty()
            || self.group.len() > 128
            || self
                .vertices
                .iter()
                .flatten()
                .any(|v| (*v as i64).abs() > limits.max_coord as i64)
        {
            return Err(GeometryError("三角面坐标或分组超出工程限制".into()));
        }
        let a: [i64; 3] =
            std::array::from_fn(|i| self.vertices[1][i] as i64 - self.vertices[0][i] as i64);
        let b: [i64; 3] =
            std::array::from_fn(|i| self.vertices[2][i] as i64 - self.vertices[0][i] as i64);
        if [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ] == [0; 3]
        {
            return Err(GeometryError("三角面三个顶点不得重合或共线".into()));
        }
        Ok(())
    }
    pub fn bounds(&self) -> Bounds {
        Bounds {
            min: std::array::from_fn(|a| self.vertices.iter().map(|v| v[a]).min().unwrap()),
            max: std::array::from_fn(|a| self.vertices.iter().map(|v| v[a]).max().unwrap()),
        }
    }
    /// Region editing selects whole faces by AABB. It never silently cuts a
    /// triangle into a new topology; exact selection uses stable face IDs.
    pub fn intersects(&self, region: &Box3) -> bool {
        let b = self.bounds();
        let end = region.end();
        (0..3).all(|a| b.min[a] < end[a] && b.max[a] >= region.min[a])
    }
    pub fn wire_vertices(&self) -> Result<[[u8; 3]; 3]> {
        let mut out = [[0; 3]; 3];
        for (i, v) in self.vertices.iter().enumerate() {
            for a in 0..3 {
                let n = v[a] as i64 + if a == 1 { 0 } else { 128 };
                if !(0..=255).contains(&n) {
                    return Err(GeometryError(
                        "三角面顶点超出MF6整数坐标范围：X/Z -128..127，Y 0..255".into(),
                    ));
                }
                out[i][a] = n as u8;
            }
        }
        Ok(out)
    }
}

/// A closed triangular prism with a slope rising in the selected direction.
/// Keeps shared vertices in authoring code; the transport may select a template.
pub fn wedge(
    min: [i32; 3],
    size: [u32; 3],
    direction: &str,
    color: [u8; 3],
    name: &str,
) -> Result<Vec<Triangle3>> {
    let box3 = Box3::new(min, size, 1);
    box3.validate(&Limits::default())?;
    let [x, y, z] = min;
    let end = box3.end();
    let [ex, ey, ez] = end;
    let vertices = match direction {
        "+x" => vec![
            [x, y, z],
            [ex, y, z],
            [ex, ey, z],
            [x, y, ez],
            [ex, y, ez],
            [ex, ey, ez],
        ],
        "-x" => vec![
            [ex, y, ez],
            [x, y, ez],
            [x, ey, ez],
            [ex, y, z],
            [x, y, z],
            [x, ey, z],
        ],
        "+z" => vec![
            [ex, y, z],
            [ex, y, ez],
            [ex, ey, ez],
            [x, y, z],
            [x, y, ez],
            [x, ey, ez],
        ],
        "-z" => vec![
            [x, y, ez],
            [x, y, z],
            [x, ey, z],
            [ex, y, ez],
            [ex, y, z],
            [ex, ey, z],
        ],
        _ => return Err(GeometryError("斜面方向须为+x/-x/+z/-z".into())),
    };
    // Orient each face outwards using its centroid relative to the prism center.
    let center: [i64; 3] = std::array::from_fn(|a| vertices.iter().map(|v| v[a] as i64).sum());
    let mut out = Vec::new();
    for indices in [
        [0, 1, 2],
        [3, 5, 4],
        [0, 3, 4],
        [0, 4, 1],
        [1, 4, 5],
        [1, 5, 2],
        [2, 5, 3],
        [2, 3, 0],
    ] {
        let mut points = indices.map(|i| vertices[i]);
        let a: [i64; 3] = std::array::from_fn(|j| points[1][j] as i64 - points[0][j] as i64);
        let b: [i64; 3] = std::array::from_fn(|j| points[2][j] as i64 - points[0][j] as i64);
        let normal = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let facing: i64 = (0..3)
            .map(|j| normal[j] * (2 * points.iter().map(|v| v[j] as i64).sum::<i64>() - center[j]))
            .sum();
        if facing < 0 {
            points.swap(1, 2);
        }
        let mut triangle = Triangle3::new(points, color);
        triangle.group = name.into();
        triangle.validate(&Limits::default())?;
        out.push(triangle);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wedge_is_closed_non_degenerate_and_all_directions_fit() {
        for direction in ["+x", "-x", "+z", "-z"] {
            let faces = wedge([-5, 0, -5], [10, 8, 10], direction, [40, 80, 120], "roof").unwrap();
            assert_eq!(faces.len(), 8);
            let mut edges = std::collections::BTreeMap::new();
            for face in faces {
                face.validate(&Limits::default()).unwrap();
                for i in 0..3 {
                    let a = face.vertices[i];
                    let b = face.vertices[(i + 1) % 3];
                    let edge = if a < b { (a, b) } else { (b, a) };
                    *edges.entry(edge).or_insert(0) += 1;
                }
            }
            assert!(edges.values().all(|n| *n == 2));
        }
    }
    #[test]
    fn wire_does_not_wrap_and_degenerate_faces_fail() {
        let t = Triangle3::new([[0, 0, 0], [1, 0, 0], [0, 1, 0]], [255; 3]);
        assert_eq!(t.wire_vertices().unwrap()[0], [128, 0, 128]);
        let mut bad = t.clone();
        bad.vertices[0][0] = 128;
        assert!(bad.wire_vertices().is_err());
        bad.vertices = [[0; 3]; 3];
        assert!(bad.validate(&Limits::default()).is_err());
    }
}
