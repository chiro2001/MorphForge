//! Offline research only: actual MF6 planner vs proposed region wire budgets.
//! Does not send OSC or implement a new Avatar decoder. Run with an output path.
use morphforge_app::transport::unified::Session;
use morphforge_core::{material_wire::Surface, merge, mf6::Frame, protocol::key, Box3, Scene};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("output JSON path");
    let cases = [
        ("plane_uniform", [64usize, 1, 64]),
        ("plane_stripes16", [64, 1, 64]),
        ("plane_checker2", [64, 1, 64]),
        ("plane_noise16", [64, 1, 64]),
        ("plane_unique_rgb", [64, 1, 64]),
        ("volume_layers16", [16, 16, 16]),
        ("volume_noise16", [16, 16, 16]),
    ];
    let mut reports = Vec::new();
    for (name, size) in cases {
        let mut boxes = Vec::new();
        let mut colors = Vec::new();
        let mut random = 7u32;
        for z in 0..size[2] {
            for y in 0..size[1] {
                for x in 0..size[0] {
                    random ^= random << 13;
                    random ^= random >> 17;
                    random ^= random << 5;
                    let color = match name {
                        "plane_uniform" => 0,
                        "plane_stripes16" => x / 4,
                        "plane_checker2" => (x + z) % 2,
                        "volume_layers16" => y,
                        "plane_unique_rgb" => colors.len(),
                        _ => random as usize % 16,
                    } as u32;
                    colors.push(color);
                    boxes.push(Box3::new(
                        [
                            x as i32 - size[0] as i32 / 2,
                            y as i32,
                            z as i32 - size[2] as i32 / 2,
                        ],
                        [1; 3],
                        color + 1,
                    ));
                }
            }
        }
        let scene = Scene {
            boxes: merge(boxes),
            ..Default::default()
        };
        let surfaces = scene
            .boxes
            .iter()
            .map(|b| {
                let color = b.style_id - 1;
                (
                    key(b),
                    Surface {
                        mode: 1,
                        bytes: [color as u8, (color >> 8) as u8, (color >> 16) as u8],
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut session = Session::default();
        session.set_target(&scene, &surfaces)?;
        let mut frames = 0usize;
        let mut histogram = BTreeMap::new();
        while let Some(plan) = session.next("volume")? {
            plan.frame.encode((frames % 16) as u8)?;
            let kind = match &plan.frame {
                Frame::Voxels { .. } => "voxels",
                Frame::SharedBoxes { .. } => "shared_boxes",
                Frame::Boxes { .. } => "boxes",
                _ => unreachable!("initial Box construction"),
            };
            *histogram.entry(kind).or_insert(0usize) += 1;
            session.commit(&plan);
            frames += 1;
        }
        let palette = colors.iter().collect::<BTreeSet<_>>().len();
        let index_bits = (usize::BITS - (palette - 1).leading_zeros()).max(1) as usize;
        // Hypothesis: 8 control bits, 16-bit region-generation tag,
        // 16-bit absolute cursor and 8-bit count leave 192 data bits.
        let data_bits = 192usize;
        let palette_supported = palette <= 256;
        let palette_frames = if palette_supported {
            palette.div_ceil(7)
        } else {
            0
        };
        let indexed_frames = palette_supported
            .then(|| 1 + palette_frames + colors.len().div_ceil(data_bits / index_bits));
        let mut best_runs = usize::MAX;
        let mut best_axes = [0; 3];
        for axes in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let mut runs = Vec::<(u32, usize)>::new();
            for outer in 0..size[axes[2]] {
                for middle in 0..size[axes[1]] {
                    for inner in 0..size[axes[0]] {
                        let mut p = [0; 3];
                        p[axes[0]] = inner;
                        p[axes[1]] = middle;
                        p[axes[2]] = outer;
                        let color = colors[p[0] + size[0] * (p[1] + size[1] * p[2])];
                        if let Some((last, length)) = runs.last_mut() {
                            if *last == color && *length < 256 {
                                *length += 1;
                                continue;
                            }
                        }
                        runs.push((color, 1));
                    }
                }
            }
            if runs.len() < best_runs {
                best_runs = runs.len();
                best_axes = axes;
            }
        }
        let rle_frames = palette_supported
            .then(|| 1 + palette_frames + best_runs.div_ceil(data_bits / (index_bits + 8)));
        let rgb_frames = 1 + colors.len().div_ceil(data_bits / 24);
        reports.push(json!({
            "case":name,"dimensions":size,"cells":colors.len(),"paletteColors":palette,
            "mergedBoxes":scene.boxes.len(),"mf6ActualPlannerFrames":frames,"mf6WireSeconds":frames as f64*0.25,
            "mf6FrameFormats":histogram,"proposedRawRgbFrames":rgb_frames,
            "proposedIndexedFrames":indexed_frames,"proposedRleFrames":rle_frames,"rleRuns":best_runs,"bestRleScanAxesFastFirst":best_axes,
            "proposedIndexedWireSeconds":indexed_frames.map(|n|n as f64*0.25),
            "proposedRawRgbWireSeconds":rgb_frames as f64*0.25,
            "unchangedCellBirthBackendRounds":colors.len().div_ceil(8),
            "note":"Proposed numbers measure wire budget only. Indexed/RLE descriptors and palette uploads are included, initial Clear and maintenance replay are excluded for all candidates. Palette entries are 27-bit mode+payload with a 32-bit upload header, at most 7 per frame. RLE chooses the best of six axis orders and uses 1..256 cell runs. Dense decoding still needs native emitter work; no speedup claim for the current Avatar."
        }));
    }
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&json!({
            "scope":"Offline deterministic fixtures; current geometry merge + actual MF6 planner and encoder vs proposed bit budgets. No model calls, network traffic or native receiver performance measurement.",
            "intervalSeconds":0.25,"controlBits":8,"regionHeaderBits":40,"dataBits":192,"cases":reports
        }))?,
    )?;
    Ok(())
}
