//! MF6's self-contained 240-bit transport. A codec capability does not imply
//! that a selected avatar implements it; negotiate the native receiver first.
use crate::{material_wire::Surface, protocol, Box3, GeometryError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const CAPABILITY: &str = "mf6-unified/1";
pub const REGION_CAPABILITY: &str = "mf6-region8/1";
pub const BYTES: usize = 30;
pub const POOLS: usize = 8;
pub const ID_LIMIT: u16 = 16384;

fn invalid(message: &str) -> GeometryError {
    GeometryError(format!("MF6：{message}"))
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Geometry {
    pub min: [u8; 3],
    pub extent: [u8; 3], // size minus one
}
impl Geometry {
    pub fn from_box(value: &Box3) -> Result<Self> {
        protocol::validate_geometry(value)?;
        Ok(Self {
            min: std::array::from_fn(|a| (value.min[a] + if a == 1 { 0 } else { 128 }) as u8),
            extent: value.size.map(|n| (n - 1) as u8),
        })
    }
    pub fn validate(&self) -> Result<()> {
        if (0..3).any(|a| self.min[a] as u16 + self.extent[a] as u16 + 1 > 256) {
            return Err(invalid("长方体越出256³空间"));
        }
        Ok(())
    }
    fn write(&self, out: &mut [u8]) {
        out[..3].copy_from_slice(&self.min);
        out[3..6].copy_from_slice(&self.extent);
    }
    fn read(input: &[u8]) -> Self {
        Self {
            min: input[..3].try_into().unwrap(),
            extent: input[3..6].try_into().unwrap(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoxRecord {
    pub pool: u8,
    pub geometry: Geometry,
    pub surface: Surface,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EraseRecord {
    pub pool: u8,
    pub geometry: Geometry,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriangleRecord {
    pub id: u16,
    pub vertices: [[u8; 3]; 3],
    pub color: [u8; 3],
    pub double_sided: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Strip {
    pub first_id: u16,
    pub vertices: Vec<[u8; 3]>,
    pub colors: Vec<[u8; 3]>,
    pub double_sided: bool,
}
impl Strip {
    pub fn triangles(&self) -> Result<Vec<TriangleRecord>> {
        if self.colors.is_empty()
            || self.colors.len() > 4
            || self.vertices.len() != self.colors.len() + 2
        {
            return Err(invalid("三角带须有1..4个面且顶点数=面数+2"));
        }
        if self.first_id as usize + self.colors.len() > ID_LIMIT as usize {
            return Err(invalid("三角带连续ID越界"));
        }
        self.colors
            .iter()
            .enumerate()
            .map(|(i, color)| {
                let vertices = if i % 2 == 0 {
                    [self.vertices[i], self.vertices[i + 1], self.vertices[i + 2]]
                } else {
                    [self.vertices[i + 1], self.vertices[i], self.vertices[i + 2]]
                };
                validate_triangle(&vertices)?;
                Ok(TriangleRecord {
                    id: self.first_id + i as u16,
                    vertices,
                    color: *color,
                    double_sided: self.double_sided,
                })
            })
            .collect()
    }
}
fn validate_triangle(v: &[[u8; 3]; 3]) -> Result<()> {
    let a: [i32; 3] = std::array::from_fn(|i| v[1][i] as i32 - v[0][i] as i32);
    let b: [i32; 3] = std::array::from_fn(|i| v[2][i] as i32 - v[0][i] as i32);
    if [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ] == [0; 3]
    {
        return Err(invalid("不发送退化三角面"));
    }
    Ok(())
}
fn pools(values: impl IntoIterator<Item = u8>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for value in values {
        if value >= POOLS as u8 || !seen.insert(value) {
            return Err(invalid("同帧池须为互不重复的0..7"));
        }
    }
    Ok(())
}
fn count(n: usize, max: usize) -> Result<()> {
    if n == 0 || n > max {
        Err(invalid("记录数量越界"))
    } else {
        Ok(())
    }
}
fn tagged_id(id: u16, double_sided: bool) -> Result<u16> {
    if id >= ID_LIMIT {
        return Err(invalid("三角面ID须为0..16383"));
    }
    Ok(id | ((double_sided as u16) << 14))
}
fn get_u16(input: &[u8]) -> u16 {
    u16::from_le_bytes(input[..2].try_into().unwrap())
}
fn put_u16(out: &mut [u8], value: u16) {
    out[..2].copy_from_slice(&value.to_le_bytes());
}

/// Self-contained eight-worker region. Shape 0..2 is a line along X/Y/Z;
/// shape 3 is the X-fast 2x2x2 brick. Absolute origin prevents packet loss
/// from moving the cursor of every later packet. No persistent palette needed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    pub origin: [u8; 3],
    pub shape: u8,
    pub first_pool: u8,
    pub surfaces: Vec<Surface>,
}
impl Region {
    pub fn offset(shape: u8, index: usize) -> [u8; 3] {
        if shape < 3 {
            let mut p = [0; 3];
            p[shape as usize] = index as u8;
            p
        } else {
            [
                (index & 1) as u8,
                ((index >> 1) & 1) as u8,
                ((index >> 2) & 1) as u8,
            ]
        }
    }
    pub fn records(&self) -> Result<Vec<BoxRecord>> {
        count(self.surfaces.len(), 8)?;
        if self.shape > 3
            || self.first_pool > 7
            || self
                .surfaces
                .iter()
                .any(|s| s.mode > 7 || s.mode != self.surfaces[0].mode)
        {
            return Err(invalid("区域类型/池/表面模式无效"));
        }
        self.surfaces
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let delta = Self::offset(self.shape, i);
                let mut min = self.origin;
                for a in 0..3 {
                    min[a] = min[a]
                        .checked_add(delta[a])
                        .ok_or_else(|| invalid("区域越过256³边界"))?;
                }
                Ok(BoxRecord {
                    pool: (self.first_pool + i as u8) % 8,
                    geometry: Geometry {
                        min,
                        extent: [0; 3],
                    },
                    surface: *s,
                })
            })
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "format", rename_all = "snake_case")]
pub enum Frame {
    Idle,
    Region { region: Region },
    Boxes { shape: u8, records: Vec<BoxRecord> },
    SharedBoxes { records: Vec<BoxRecord> },
    Voxels { records: Vec<BoxRecord> },
    Triangles { records: Vec<TriangleRecord> },
    TriangleStrip { strip: Strip },
    LocalTriangleStrip { origin: [u8; 3], strip: Strip },
    RemoveBoxes { records: Vec<EraseRecord> },
    RemoveTriangles { ids: Vec<u16> },
    Clear,
}
impl Frame {
    pub fn encode(&self, sequence: u8) -> Result<[u8; BYTES]> {
        if sequence >= 16 {
            return Err(invalid("序号须为0..15"));
        }
        let mut out = [0; BYTES];
        let data = &mut out[..29];
        let format = match self {
            Self::Idle => 0,
            Self::Region { region } => {
                region.records()?;
                data[..3].copy_from_slice(&region.origin);
                data[3] = region.shape
                    | (((region.surfaces.len() - 1) as u8) << 2)
                    | (region.surfaces[0].mode << 5);
                for (i, s) in region.surfaces.iter().enumerate() {
                    data[4 + i * 3..7 + i * 3].copy_from_slice(&s.bytes);
                }
                data[28] = 0xa8 | region.first_pool;
                0
            }
            Self::Clear => 15,
            Self::Boxes { shape, records } => {
                count(records.len(), 3)?;
                if *shape > 4 {
                    return Err(invalid("未知Box模板"));
                }
                pools(records.iter().map(|r| r.pool))?;
                let mode = records[0].surface.mode;
                if mode > 7 || records.iter().any(|r| r.surface.mode != mode) {
                    return Err(invalid("独立Box帧须共用表面模式"));
                }
                let mut meta = (((records.len() - 1) as u16) << 9) | ((mode as u16) << 11);
                for (i, r) in records.iter().enumerate() {
                    r.geometry.validate()?;
                    r.geometry.write(&mut data[i * 9..]);
                    data[i * 9 + 6..i * 9 + 9].copy_from_slice(&r.surface.bytes);
                    meta |= (r.pool as u16) << (i * 3);
                }
                put_u16(&mut data[27..], meta);
                if *shape == 0 {
                    1
                } else {
                    10 + *shape
                }
            }
            Self::SharedBoxes { records } => {
                count(records.len(), 4)?;
                pools(records.iter().map(|r| r.pool))?;
                let surface = records[0].surface;
                if surface.mode > 2 || records.iter().any(|r| r.surface != surface) {
                    return Err(invalid("共外观Box须为同一目录/RGB/PBR"));
                }
                let mut meta = ((records.len() - 1) as u16) << 12;
                for (i, r) in records.iter().enumerate() {
                    r.geometry.validate()?;
                    r.geometry.write(&mut data[i * 6..]);
                    meta |= (r.pool as u16) << (i * 3);
                }
                data[24..27].copy_from_slice(&surface.bytes);
                put_u16(&mut data[27..], meta);
                2 + surface.mode
            }
            Self::Voxels { records } => {
                count(records.len(), 8)?;
                pools(records.iter().map(|r| r.pool))?;
                let surface = records[0].surface;
                if surface.mode > 7
                    || records
                        .iter()
                        .any(|r| r.surface != surface || r.geometry.extent != [0; 3])
                {
                    return Err(invalid("单位体素帧须共用外观且尺寸为1"));
                }
                for r in records {
                    data[r.pool as usize * 3..r.pool as usize * 3 + 3]
                        .copy_from_slice(&r.geometry.min);
                    data[27] |= 1 << r.pool;
                }
                data[24..27].copy_from_slice(&surface.bytes);
                data[28] = surface.mode;
                5
            }
            Self::Triangles { records } => {
                count(records.len(), 2)?;
                pools(records.iter().map(|r| (r.id % POOLS as u16) as u8))?;
                for (i, r) in records.iter().enumerate() {
                    validate_triangle(&r.vertices)?;
                    let offset = i * 14;
                    put_u16(&mut data[offset..], tagged_id(r.id, r.double_sided)?);
                    for j in 0..3 {
                        data[offset + 2 + j * 3..offset + 5 + j * 3]
                            .copy_from_slice(&r.vertices[j]);
                    }
                    data[offset + 11..offset + 14].copy_from_slice(&r.color);
                }
                data[28] = records.len() as u8;
                6
            }
            Self::TriangleStrip { strip } => {
                count(strip.colors.len(), 3)?;
                strip.triangles()?;
                put_u16(data, tagged_id(strip.first_id, strip.double_sided)?);
                for (i, v) in strip.vertices.iter().enumerate() {
                    data[2 + i * 3..5 + i * 3].copy_from_slice(v);
                }
                for (i, c) in strip.colors.iter().enumerate() {
                    data[17 + i * 3..20 + i * 3].copy_from_slice(c);
                }
                data[26] = strip.colors.len() as u8;
                7
            }
            Self::LocalTriangleStrip { origin, strip } => {
                count(strip.colors.len(), 4)?;
                strip.triangles()?;
                put_u16(data, tagged_id(strip.first_id, strip.double_sided)?);
                data[2..5].copy_from_slice(origin);
                for (i, v) in strip.vertices.iter().enumerate() {
                    for a in 0..3 {
                        let delta = v[a]
                            .checked_sub(origin[a])
                            .filter(|d| *d < 16)
                            .ok_or_else(|| invalid("局部三角带顶点须在原点起16³范围内"))?;
                        let nibble = i * 3 + a;
                        data[5 + nibble / 2] |= delta << ((nibble % 2) * 4);
                    }
                }
                for (i, c) in strip.colors.iter().enumerate() {
                    data[14 + i * 3..17 + i * 3].copy_from_slice(c);
                }
                data[26] = strip.colors.len() as u8;
                10
            }
            Self::RemoveBoxes { records } => {
                count(records.len(), 4)?;
                pools(records.iter().map(|r| r.pool))?;
                let mut meta = ((records.len() - 1) as u16) << 12;
                for (i, r) in records.iter().enumerate() {
                    r.geometry.validate()?;
                    r.geometry.write(&mut data[i * 6..]);
                    meta |= (r.pool as u16) << (i * 3);
                }
                put_u16(&mut data[24..], meta);
                8
            }
            Self::RemoveTriangles { ids } => {
                count(ids.len(), 8)?;
                pools(ids.iter().map(|id| (id % POOLS as u16) as u8))?;
                for id in ids {
                    let pool = (*id % POOLS as u16) as usize;
                    put_u16(&mut data[pool * 2..], tagged_id(*id, false)?);
                    data[16] |= 1 << pool;
                }
                9
            }
        };
        out[29] = (sequence << 4) | format;
        Ok(out)
    }

    pub fn decode(input: &[u8; BYTES]) -> Result<(u8, Self)> {
        let sequence = input[29] >> 4;
        let format = input[29] & 15;
        let d = &input[..29];
        let frame = match format {
            0 if d[28] & 0xf8 == 0xa8 => Self::Region {
                region: Region {
                    origin: d[..3].try_into().unwrap(),
                    shape: d[3] & 3,
                    first_pool: d[28] & 7,
                    surfaces: (0..(((d[3] >> 2) & 7) + 1) as usize)
                        .map(|i| Surface {
                            mode: d[3] >> 5,
                            bytes: d[4 + i * 3..7 + i * 3].try_into().unwrap(),
                        })
                        .collect(),
                },
            },
            0 => Self::Idle,
            15 => Self::Clear,
            1 | 11..=14 => {
                let meta = get_u16(&d[27..]);
                let n = ((meta >> 9) & 3) as usize + 1;
                count(n, 3)?;
                let records = (0..n)
                    .map(|i| BoxRecord {
                        pool: ((meta >> (i * 3)) & 7) as u8,
                        geometry: Geometry::read(&d[i * 9..]),
                        surface: Surface {
                            mode: ((meta >> 11) & 7) as u8,
                            bytes: d[i * 9 + 6..i * 9 + 9].try_into().unwrap(),
                        },
                    })
                    .collect();
                Self::Boxes {
                    shape: if format == 1 { 0 } else { format - 10 },
                    records,
                }
            }
            2..=4 => {
                let meta = get_u16(&d[27..]);
                let n = ((meta >> 12) & 3) as usize + 1;
                let surface = Surface {
                    mode: format - 2,
                    bytes: d[24..27].try_into().unwrap(),
                };
                Self::SharedBoxes {
                    records: (0..n)
                        .map(|i| BoxRecord {
                            pool: ((meta >> (i * 3)) & 7) as u8,
                            geometry: Geometry::read(&d[i * 6..]),
                            surface,
                        })
                        .collect(),
                }
            }
            5 => {
                let surface = Surface {
                    mode: d[28],
                    bytes: d[24..27].try_into().unwrap(),
                };
                Self::Voxels {
                    records: (0..8)
                        .filter(|i| d[27] & (1 << i) != 0)
                        .map(|i| BoxRecord {
                            pool: i as u8,
                            geometry: Geometry {
                                min: d[i * 3..i * 3 + 3].try_into().unwrap(),
                                extent: [0; 3],
                            },
                            surface,
                        })
                        .collect(),
                }
            }
            6 => {
                let n = d[28] as usize;
                count(n, 2)?;
                Self::Triangles {
                    records: (0..n)
                        .map(|i| {
                            let p = i * 14;
                            let id = get_u16(&d[p..]);
                            TriangleRecord {
                                id: id & 16383,
                                double_sided: id & 16384 != 0,
                                vertices: std::array::from_fn(|j| {
                                    d[p + 2 + j * 3..p + 5 + j * 3].try_into().unwrap()
                                }),
                                color: d[p + 11..p + 14].try_into().unwrap(),
                            }
                        })
                        .collect(),
                }
            }
            7 | 10 => {
                let n = d[26] as usize;
                count(n, if format == 7 { 3 } else { 4 })?;
                let id = get_u16(d);
                let origin: [u8; 3] = d[2..5].try_into().unwrap();
                let vertices = if format == 7 {
                    (0..n + 2)
                        .map(|i| d[2 + i * 3..5 + i * 3].try_into().unwrap())
                        .collect()
                } else {
                    let mut vertices = Vec::new();
                    for i in 0..n + 2 {
                        let mut vertex = [0; 3];
                        for a in 0..3 {
                            let nibble = i * 3 + a;
                            let delta = (d[5 + nibble / 2] >> ((nibble % 2) * 4)) & 15;
                            vertex[a] = origin[a]
                                .checked_add(delta)
                                .ok_or_else(|| invalid("局部顶点越界"))?;
                        }
                        vertices.push(vertex);
                    }
                    vertices
                };
                let color_offset = if format == 7 { 17 } else { 14 };
                let colors = (0..n)
                    .map(|i| {
                        d[color_offset + i * 3..color_offset + i * 3 + 3]
                            .try_into()
                            .unwrap()
                    })
                    .collect();
                let strip = Strip {
                    first_id: id & 16383,
                    double_sided: id & 16384 != 0,
                    vertices,
                    colors,
                };
                if format == 7 {
                    Self::TriangleStrip { strip }
                } else {
                    Self::LocalTriangleStrip { origin, strip }
                }
            }
            8 => {
                let meta = get_u16(&d[24..]);
                let n = ((meta >> 12) & 3) as usize + 1;
                Self::RemoveBoxes {
                    records: (0..n)
                        .map(|i| EraseRecord {
                            pool: ((meta >> (i * 3)) & 7) as u8,
                            geometry: Geometry::read(&d[i * 6..]),
                        })
                        .collect(),
                }
            }
            9 => Self::RemoveTriangles {
                ids: (0..8)
                    .filter(|i| d[16] & (1 << i) != 0)
                    .map(|i| get_u16(&d[i * 2..]))
                    .collect(),
            },
            _ => unreachable!(),
        };
        if frame.encode(sequence)? != *input {
            return Err(invalid("非规范帧：保留位或无效记录含数据"));
        }
        Ok((sequence, frame))
    }

    pub fn packet(&self, sequence: u8) -> Result<Vec<u8>> {
        let bytes = self.encode(sequence)?;
        let mut values: Vec<_> = bytes[..29]
            .iter()
            .enumerate()
            .map(|(i, b)| (format!("MF6_D{i:02}"), protocol::OscValue::Int(*b as i32)))
            .collect();
        values.push(("MF6_Ctl".into(), protocol::OscValue::Int(bytes[29] as i32)));
        Ok(protocol::bundle(&values))
    }
    pub fn objects(&self) -> usize {
        match self {
            Self::Idle | Self::Clear => 0,
            Self::Region { region } => region.surfaces.len(),
            Self::Boxes { records, .. }
            | Self::SharedBoxes { records }
            | Self::Voxels { records } => records.len(),
            Self::Triangles { records } => records.len(),
            Self::TriangleStrip { strip } | Self::LocalTriangleStrip { strip, .. } => {
                strip.colors.len()
            }
            Self::RemoveBoxes { records } => records.len(),
            Self::RemoveTriangles { ids } => ids.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(pool: u8, mode: u8) -> BoxRecord {
        BoxRecord {
            pool,
            geometry: Geometry {
                min: [pool * 10, 2, 3],
                extent: [1, 2, 3],
            },
            surface: Surface {
                mode,
                bytes: [10, 20, 30],
            },
        }
    }
    fn strip(n: usize) -> Strip {
        Strip {
            first_id: 253,
            vertices: (0..n + 2)
                .map(|i| [120 + i as u8, 120 + (i % 2) as u8, 120])
                .collect(),
            colors: (0..n).map(|i| [i as u8, 127, 255]).collect(),
            double_sided: true,
        }
    }
    fn round(frame: Frame) {
        for seq in 0..16 {
            let bytes = frame.encode(seq).unwrap();
            assert_eq!(Frame::decode(&bytes).unwrap(), (seq, frame.clone()));
            assert!(frame.packet(seq).unwrap().starts_with(b"#bundle\0"));
        }
    }
    #[test]
    fn regions_roundtrip_with_absolute_positions_and_all_modes() {
        for shape in 0..4 {
            for mode in 0..8 {
                for n in 1..=8 {
                    let region = Region {
                        origin: [128, 12, 128],
                        shape,
                        first_pool: 6,
                        surfaces: (0..n)
                            .map(|i| Surface {
                                mode,
                                bytes: [i as u8, 255, 17],
                            })
                            .collect(),
                    };
                    let r = region.records().unwrap();
                    assert_eq!(r.len(), n);
                    for (i, v) in r.iter().enumerate() {
                        assert_eq!(v.pool, (6 + i as u8) % 8);
                        assert_eq!(v.geometry.extent, [0; 3]);
                    }
                    round(Frame::Region { region });
                }
            }
        }
        let invalid = Region {
            origin: [255; 3],
            shape: 0,
            first_pool: 0,
            surfaces: vec![Surface::catalog(1, 0, 0); 2],
        };
        assert!(Frame::Region { region: invalid }.encode(1).is_err());
        let a = Frame::Region {
            region: Region {
                origin: [0; 3],
                shape: 0,
                first_pool: 0,
                surfaces: vec![Surface::catalog(1, 0, 0); 2],
            },
        };
        let mut wire = a.encode(1).unwrap();
        wire[27] = 1;
        assert!(Frame::decode(&wire).is_err(), "unused bytes are canonical");
    }

    #[test]
    fn every_layout_and_surface_roundtrips() {
        round(Frame::Idle);
        round(Frame::Clear);
        for shape in 0..5 {
            for mode in 0..8 {
                round(Frame::Boxes {
                    shape,
                    records: (0..3).map(|p| record(p, mode)).collect(),
                });
            }
        }
        for mode in 0..3 {
            round(Frame::SharedBoxes {
                records: (0..4).map(|p| record(p, mode)).collect(),
            });
        }
        for mode in 0..8 {
            let records = (0..8)
                .map(|p| {
                    let mut r = record(p, mode);
                    r.geometry.extent = [0; 3];
                    r
                })
                .collect();
            round(Frame::Voxels { records });
        }
        round(Frame::RemoveBoxes {
            records: (0..4)
                .map(|p| EraseRecord {
                    pool: p,
                    geometry: record(p, 0).geometry,
                })
                .collect(),
        });
        round(Frame::RemoveTriangles {
            ids: (16376..16384).collect(),
        });
        for n in 1..4 {
            round(Frame::TriangleStrip { strip: strip(n) });
        }
        for n in 1..5 {
            round(Frame::LocalTriangleStrip {
                origin: [120; 3],
                strip: strip(n),
            });
        }
        round(Frame::Triangles {
            records: strip(2).triangles().unwrap(),
        });
    }
    #[test]
    fn strip_winding_is_consistent() {
        let triangles = strip(4).triangles().unwrap();
        let mut signs = Vec::new();
        for t in triangles {
            let v = t.vertices;
            signs.push(
                (v[1][0] as i32 - v[0][0] as i32) * (v[2][1] as i32 - v[0][1] as i32)
                    - (v[1][1] as i32 - v[0][1] as i32) * (v[2][0] as i32 - v[0][0] as i32),
            );
        }
        assert!(signs.iter().all(|x| *x < 0));
    }
    #[test]
    fn invalid_data_rejected_without_panics() {
        assert!(Frame::Idle.encode(16).is_err());
        assert!(Frame::Boxes {
            shape: 0,
            records: vec![]
        }
        .encode(0)
        .is_err());
        assert!(Frame::Boxes {
            shape: 0,
            records: vec![record(1, 0), record(1, 0)]
        }
        .encode(0)
        .is_err());
        assert!(Frame::Boxes {
            shape: 0,
            records: vec![record(0, 0), record(1, 1)]
        }
        .encode(0)
        .is_err());
        let mut s = strip(4);
        s.first_id = 16383;
        assert!(Frame::LocalTriangleStrip {
            origin: [120; 3],
            strip: s
        }
        .encode(0)
        .is_err());
        assert!(Frame::LocalTriangleStrip {
            origin: [121; 3],
            strip: strip(4)
        }
        .encode(0)
        .is_err());
        let mut t = strip(1).triangles().unwrap();
        t[0].vertices = [[0; 3]; 3];
        assert!(Frame::Triangles { records: t }.encode(0).is_err());
        assert!(Frame::RemoveTriangles { ids: vec![0, 8] }
            .encode(0)
            .is_err());
        for format in 0..16 {
            let mut data = [0; 30];
            data[29] = format;
            data[0] = 255;
            let _ = Frame::decode(&data);
        }
    }
    #[test]
    fn reserved_bits_and_padding_rejected() {
        let frame = Frame::Boxes {
            shape: 0,
            records: vec![record(0, 1)],
        };
        let mut wire = frame.encode(0).unwrap();
        wire[28] |= 128;
        assert!(Frame::decode(&wire).is_err());
        let mut wire = frame.encode(0).unwrap();
        wire[9] = 1;
        assert!(Frame::decode(&wire).is_err());
        let mut wire = Frame::Triangles {
            records: strip(1).triangles().unwrap(),
        }
        .encode(0)
        .unwrap();
        wire[1] |= 128;
        assert!(Frame::decode(&wire).is_err());
        let mut wire = Frame::Idle.encode(0).unwrap();
        wire[28] = 1;
        assert!(Frame::decode(&wire).is_err());
    }
    #[test]
    fn geometry_boundaries() {
        assert!(Geometry {
            min: [0; 3],
            extent: [255; 3]
        }
        .validate()
        .is_ok());
        assert!(Geometry {
            min: [1; 3],
            extent: [255; 3]
        }
        .validate()
        .is_err());
        assert!(Geometry::from_box(&Box3::new([-128, 0, -128], [256; 3], 1)).is_ok());
        assert!(Geometry::from_box(&Box3::new([128, 0, 0], [1; 3], 1)).is_err());
    }
    #[test]
    fn random_bytes_never_panic_and_canonical_frames_reencode() {
        use rand::{RngCore, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(240);
        for _ in 0..10000 {
            let mut data = [0; 30];
            rng.fill_bytes(&mut data);
            if let Ok((seq, frame)) = Frame::decode(&data) {
                assert_eq!(frame.encode(seq).unwrap(), data);
            }
        }
    }
}
