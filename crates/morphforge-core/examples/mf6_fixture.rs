//! Emit real production OSC bundles and semantic expectations for the Unity
//! native test. Does not open UDP, mutate projects, or call an AI provider.
use morphforge_core::{material_wire::Surface, mf6::*};
use serde_json::{json, Value};
fn geometry(p: u8) -> Geometry {
    Geometry {
        min: [40 + p * 12, 120, 100],
        extent: [1 + p % 3, 2, 1],
    }
}
fn record(pool: u8, mode: u8) -> BoxRecord {
    BoxRecord {
        pool,
        geometry: geometry(pool),
        surface: Surface {
            mode,
            bytes: [31 + pool * 17, 117, 221 - pool * 13],
        },
    }
}
fn push(cases: &mut Vec<Value>, name: String, frame: Frame, verify: bool) {
    let seq = ((cases.len() + 1) % 16) as u8;
    let data = frame.encode(seq).unwrap();
    let packet = frame.packet(seq).unwrap();
    cases.push(
        json!({"name":name,"frame":frame,"data":data.as_slice(),"packet":packet,"verify":verify}),
    );
}
fn main() {
    let capacity = std::env::args().any(|arg| arg == "--capacity");
    let random = std::env::args().any(|arg| arg == "--random");
    let regions = std::env::args().any(|arg| arg == "--regions");
    let mut cases = Vec::new();
    push(&mut cases, "clear".into(), Frame::Clear, true);
    if regions {
        for shape in 0..4 {
            for mode in 0..8 {
                push(
                    &mut cases,
                    format!("region8 shape {shape} mode {mode}"),
                    Frame::Region {
                        region: Region {
                            origin: [180 + shape * 12, 40 + mode * 5, 180],
                            shape,
                            first_pool: (5 + mode) % 8,
                            surfaces: (0..8)
                                .map(|i| Surface {
                                    mode,
                                    bytes: [31 + i * 23, 17 + mode * 11, 221 - i * 13],
                                })
                                .collect(),
                        },
                    },
                    true,
                );
            }
        }
        push(
            &mut cases,
            "clear before region burst".into(),
            Frame::Clear,
            true,
        );
        for i in 0..64u8 {
            push(
                &mut cases,
                format!("continuous variable-color region {i}"),
                Frame::Region {
                    region: Region {
                        origin: [16 + (i % 8) * 8, 12 + (i / 8) * 2, 100],
                        shape: 0,
                        first_pool: i % 8,
                        surfaces: (0..8)
                            .map(|n| Surface {
                                mode: 1,
                                bytes: [i * 3, n * 31, 200 - n * 19],
                            })
                            .collect(),
                    },
                },
                i % 16 == 15,
            );
        }
    } else if random {
        let mut state = 17u32;
        let mut next = || {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            (state >> 24) as u8
        };
        for batch in 0..128u16 {
            let records = (0..2)
                .map(|offset| TriangleRecord {
                    id: 2048 + batch * 2 + offset,
                    vertices: std::array::from_fn(|_| std::array::from_fn(|_| next())),
                    color: std::array::from_fn(|_| next()),
                    double_sided: true,
                })
                .collect();
            push(
                &mut cases,
                format!("random full-byte triangles {batch}"),
                Frame::Triangles { records },
                batch % 8 == 7,
            );
        }
    } else if capacity {
        for frame in 0..2048 {
            let records = (0..8)
                .map(|pool| {
                    let i = frame * 8 + pool;
                    BoxRecord {
                        pool: pool as u8,
                        geometry: Geometry {
                            min: [32 + (i % 128) as u8, 48, 32 + (i / 128) as u8],
                            extent: [0; 3],
                        },
                        surface: Surface {
                            mode: 1,
                            bytes: [
                                (i % 251) as u8,
                                ((i * 37) % 251) as u8,
                                ((i * 97) % 251) as u8,
                            ],
                        },
                    }
                })
                .collect::<Vec<_>>();
            // RGB varies by pool: raw3 cannot hold8; use a shared color per batch.
            let surface = Surface {
                mode: 1,
                bytes: [
                    (frame % 251) as u8,
                    ((frame * 37) % 251) as u8,
                    ((frame * 97) % 251) as u8,
                ],
            };
            let records = records
                .into_iter()
                .map(|mut r| {
                    r.surface = surface;
                    r
                })
                .collect();
            push(
                &mut cases,
                format!("capacity batch {frame}"),
                Frame::Voxels { records },
                frame % 128 == 127,
            );
        }
    } else {
        for mode in 0..8 {
            push(
                &mut cases,
                format!("raw3 surface {mode}"),
                Frame::Boxes {
                    shape: 0,
                    records: [7, 2, 5].map(|p| record(p, mode)).to_vec(),
                },
                true,
            );
        }
        for mode in 0..3 {
            let mut records = [6, 3, 4, 1].map(|p| record(p, mode)).to_vec();
            let surface = records[0].surface;
            for r in &mut records {
                r.surface = surface;
            }
            push(
                &mut cases,
                format!("shared4 surface {mode}"),
                Frame::SharedBoxes { records },
                true,
            );
        }
        for mode in 0..8 {
            let records = (0..8)
                .map(|pool| BoxRecord {
                    pool,
                    geometry: Geometry {
                        min: [40 + pool * 10, 145 + mode * 2, 110],
                        extent: [0; 3],
                    },
                    surface: Surface {
                        mode,
                        bytes: [71, 129, 203],
                    },
                })
                .collect();
            push(
                &mut cases,
                format!("voxels8 surface {mode}"),
                Frame::Voxels { records },
                true,
            );
        }
        let records = [0, 4, 7]
            .map(|pool| BoxRecord {
                pool,
                geometry: Geometry {
                    min: [40 + pool * 10, 170, 110],
                    extent: [0; 3],
                },
                surface: Surface {
                    mode: 1,
                    bytes: [255, 40, 91],
                },
            })
            .to_vec();
        push(
            &mut cases,
            "sparse voxel mask".into(),
            Frame::Voxels { records },
            true,
        );
        push(
            &mut cases,
            "independent triangle flags".into(),
            Frame::Triangles {
                records: vec![
                    TriangleRecord {
                        id: 253,
                        vertices: [[0, 0, 0], [255, 255, 255], [0, 255, 0]],
                        color: [255, 30, 40],
                        double_sided: false,
                    },
                    TriangleRecord {
                        id: 254,
                        vertices: [[255, 255, 255], [0, 0, 0], [255, 0, 255]],
                        color: [40, 210, 91],
                        double_sided: true,
                    },
                ],
            },
            true,
        );
        let strip = Strip {
            first_id: 254,
            vertices: vec![
                [64, 100, 120],
                [96, 120, 120],
                [128, 100, 120],
                [160, 120, 120],
                [192, 100, 120],
            ],
            colors: vec![[255, 40, 40], [40, 255, 40], [40, 40, 255]],
            double_sided: true,
        };
        push(
            &mut cases,
            "global strip crossing ID byte".into(),
            Frame::TriangleStrip { strip },
            true,
        );
        let strip = Strip {
            first_id: 510,
            vertices: vec![
                [120, 130, 120],
                [122, 134, 120],
                [124, 130, 120],
                [126, 134, 120],
                [128, 130, 120],
                [130, 134, 120],
            ],
            colors: vec![[255, 40, 40], [40, 255, 40], [40, 40, 255], [250, 180, 30]],
            double_sided: true,
        };
        push(
            &mut cases,
            "local strip four faces crossing ID byte".into(),
            Frame::LocalTriangleStrip {
                origin: [120, 130, 120],
                strip,
            },
            true,
        );
        for shape in 1..5 {
            push(
                &mut cases,
                format!("wedge template {shape}"),
                Frame::Boxes {
                    shape,
                    records: [0, 3, 7].map(|p| record(p, 1)).to_vec(),
                },
                true,
            );
        }
        push(
            &mut cases,
            "remove precise boxes".into(),
            Frame::RemoveBoxes {
                records: [0, 3, 7]
                    .map(|p| EraseRecord {
                        pool: p,
                        geometry: geometry(p),
                    })
                    .to_vec(),
            },
            true,
        );
        push(
            &mut cases,
            "remove precise triangle IDs".into(),
            Frame::RemoveTriangles {
                ids: vec![512, 513, 253, 510, 511],
            },
            true,
        );
        push(
            &mut cases,
            "clear before sequence regression".into(),
            Frame::Clear,
            true,
        );
        push(
            &mut cases,
            "sparse pool initial".into(),
            Frame::Boxes {
                shape: 0,
                records: vec![record(0, 1)],
            },
            true,
        );
        for i in 0..15 {
            let mut r = record(1, 1);
            r.surface.bytes[0] = i;
            push(
                &mut cases,
                format!("other pool idle gap {i}"),
                Frame::Boxes {
                    shape: 0,
                    records: vec![r],
                },
                false,
            );
        }
        let mut r = record(0, 1);
        r.surface.bytes = [200, 220, 240];
        push(
            &mut cases,
            "sparse pool after exactly sixteen global frames".into(),
            Frame::Boxes {
                shape: 0,
                records: vec![r],
            },
            true,
        );
        push(
            &mut cases,
            "clear before consecutive burst".into(),
            Frame::Clear,
            true,
        );
        for i in 0..128 {
            let records = (0..8)
                .map(|pool| BoxRecord {
                    pool,
                    geometry: Geometry {
                        min: [24 + (i % 32) * 4, 20 + (i / 32) * 4, 64 + pool * 8],
                        extent: [0; 3],
                    },
                    surface: Surface {
                        mode: 1,
                        bytes: [i, 255 - i, 137],
                    },
                })
                .collect();
            push(
                &mut cases,
                format!("consecutive 250ms burst {i}"),
                Frame::Voxels { records },
                i % 32 == 31,
            );
        }
    }
    println!("{}",serde_json::to_string(&json!({"format":"MorphForgeNativeFixture/1","intervalMs":250,"capacity":capacity,"cases":cases})).unwrap());
}
