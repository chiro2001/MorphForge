//! Voxel text layout.
//!
//! Font parsing and glyph rasterization live in the application layer. This
//! module consumes an already rasterized, top-left-origin coverage mask and
//! turns it into canonical [`Operation::Box`] values. Keeping the mask contract
//! font-independent makes text deterministic and lets the Recipe worker remain
//! unable to read files or environment variables.

use crate::{Bounds, Box3, GeometryError, Limits, Operation, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_TEXT_BYTES: usize = 16 * 1024;
pub const MAX_TEXT_CHARS: usize = 4096;
pub const MAX_FONT_SIZE: u32 = 512;
pub const MAX_TEXT_DEPTH: u32 = 256;
pub const MAX_RASTER_AREA: u64 = 16_777_216;
pub const MAX_TEXT_BOXES: usize = 100_000;
pub const MAX_TEXT_VOXELS: u64 = 16_000_000;
pub const MAX_SPACING: i32 = 512;
pub const MAX_TEXT_WIDTH: u32 = 1_000_000;

fn default_font_id() -> String {
    "builtin:noto-sans-sc".into()
}

fn default_font_size() -> u32 {
    32
}

fn default_depth() -> u32 {
    1
}

fn style_default() -> u32 {
    1
}

fn group_default() -> String {
    "main".into()
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TextPlane {
    #[default]
    Xy,
    Xz,
    Yz,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TextFacing {
    #[default]
    Positive,
    Negative,
}

/// A font-independent request for a text layout.
///
/// `origin` is the minimum corner of the final, empty-free text bounding box,
/// matching the convention used by [`Box3::min`]. The first layout line is at
/// the top of that box. The app layer resolves `font_id`, wraps and rasterizes
/// the text, then calls [`compile_text_mask`].
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoxelTextRequest {
    pub text: String,
    #[serde(default = "default_font_id")]
    pub font_id: String,
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    #[serde(default = "default_depth")]
    pub depth: u32,
    #[serde(default)]
    pub origin: [i32; 3],
    #[serde(default)]
    pub plane: TextPlane,
    #[serde(default)]
    pub align: TextAlign,
    #[serde(default)]
    pub letter_spacing: i32,
    #[serde(default)]
    pub line_spacing: i32,
    #[serde(default)]
    pub max_width: Option<u32>,
    #[serde(default = "style_default")]
    pub style_id: u32,
    #[serde(default)]
    pub tint_id: u8,
    #[serde(default)]
    pub motion_phase: u8,
    #[serde(default = "group_default")]
    pub group: String,
    #[serde(default)]
    pub facing: TextFacing,
}

impl Default for VoxelTextRequest {
    fn default() -> Self {
        Self {
            text: String::new(),
            font_id: default_font_id(),
            font_size: default_font_size(),
            depth: default_depth(),
            origin: [0; 3],
            plane: TextPlane::Xy,
            align: TextAlign::Left,
            letter_spacing: 0,
            line_spacing: 0,
            max_width: None,
            style_id: 1,
            tint_id: 0,
            motion_phase: 0,
            group: group_default(),
            facing: TextFacing::Positive,
        }
    }
}

/// A top-left-origin, row-major coverage bitmap produced by the font layer.
///
/// Coverage values are linear from `0..=255`. The core uses a fixed threshold
/// of `> 127`, so the same glyph raster and request always produce identical
/// geometry.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextMask {
    pub width: u32,
    pub height: u32,
    pub coverage: Vec<u8>,
}

impl TextMask {
    pub fn new(width: u32, height: u32, coverage: Vec<u8>) -> Result<Self> {
        let expected = (width as u64)
            .checked_mul(height as u64)
            .ok_or_else(|| GeometryError("文字栅格尺寸溢出".into()))?;
        if expected != coverage.len() as u64 {
            return Err(GeometryError("文字栅格长度与宽高不一致".into()));
        }
        Ok(Self {
            width,
            height,
            coverage,
        })
    }

    pub fn foreground(&self, x: u32, y: u32) -> bool {
        let index = y as usize * self.width as usize + x as usize;
        self.coverage.get(index).is_some_and(|value| *value > 127)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextLayout {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub voxel_count: u64,
    pub box_count: usize,
    pub bounds: Bounds,
    #[serde(default)]
    pub missing_glyphs: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct TextCompilation {
    pub operations: Vec<Operation>,
    pub layout: TextLayout,
}

#[derive(Clone, Copy, Debug)]
struct Basis {
    u_axis: usize,
    u_sign: i32,
    v_axis: usize,
    v_sign: i32,
    depth_axis: usize,
}

fn basis(plane: TextPlane, facing: TextFacing) -> Basis {
    let flip = if facing == TextFacing::Negative {
        -1
    } else {
        1
    };
    match plane {
        TextPlane::Xy => Basis {
            u_axis: 0,
            u_sign: -flip,
            v_axis: 1,
            v_sign: -1,
            depth_axis: 2,
        },
        TextPlane::Xz => Basis {
            u_axis: 0,
            u_sign: -flip,
            v_axis: 2,
            v_sign: 1,
            depth_axis: 1,
        },
        TextPlane::Yz => Basis {
            u_axis: 2,
            u_sign: flip,
            v_axis: 1,
            v_sign: -1,
            depth_axis: 0,
        },
    }
}

pub fn validate_text_request(request: &VoxelTextRequest, limits: &Limits) -> Result<()> {
    if request.text.is_empty() {
        return Err(GeometryError("文字不能为空".into()));
    }
    if request.text.len() > MAX_TEXT_BYTES {
        return Err(GeometryError(format!(
            "文字超过{}KiB预算",
            MAX_TEXT_BYTES / 1024
        )));
    }
    if request.text.chars().count() > MAX_TEXT_CHARS {
        return Err(GeometryError(format!(
            "文字超过{}个字符预算",
            MAX_TEXT_CHARS
        )));
    }
    if request.font_id.trim().is_empty() || request.font_id.len() > 256 {
        return Err(GeometryError("fontId为空或过长".into()));
    }
    if !(4..=MAX_FONT_SIZE).contains(&request.font_size) {
        return Err(GeometryError("fontSize须为4..512".into()));
    }
    if !(1..=MAX_TEXT_DEPTH).contains(&request.depth) {
        return Err(GeometryError("depth须为1..256".into()));
    }
    if !(-MAX_SPACING..=MAX_SPACING).contains(&request.letter_spacing)
        || !(-MAX_SPACING..=MAX_SPACING).contains(&request.line_spacing)
    {
        return Err(GeometryError("文字间距须为-512..512".into()));
    }
    if request
        .max_width
        .is_some_and(|width| !(1..=MAX_TEXT_WIDTH).contains(&width))
    {
        return Err(GeometryError("maxWidth须为1..1000000".into()));
    }
    if request.style_id > 4095 {
        return Err(GeometryError("样式编号须为0..4095".into()));
    }
    if request.tint_id > 63 || request.motion_phase > 63 {
        return Err(GeometryError("染色/动画预设须为0..63".into()));
    }
    if request.group.len() > 128 {
        return Err(GeometryError("分组名称不得超过128字节".into()));
    }
    if request
        .origin
        .iter()
        .any(|value| value.unsigned_abs() > limits.max_coord as u32)
    {
        return Err(GeometryError("文字原点超出工程资源保护范围".into()));
    }

    for character in request.text.chars() {
        if character == '\n' || character == '\r' || character == '\t' {
            continue;
        }
        if character.is_control() {
            return Err(GeometryError("文字包含拒绝的控制字符".into()));
        }
        if is_complex_script(character) || is_bidi_control(character) {
            return Err(GeometryError(format!(
                "暂不支持需要复杂排版的字符U+{:04X}",
                character as u32
            )));
        }
    }
    if request.text.trim().is_empty() {
        return Err(GeometryError("文字没有可见字符".into()));
    }
    Ok(())
}

fn is_bidi_control(character: char) -> bool {
    matches!(
        character as u32,
        0x200B..=0x200F | 0x202A..=0x202E | 0x2066..=0x2069 | 0xFE00..=0xFE0F
    )
}

fn is_complex_script(character: char) -> bool {
    matches!(
        character as u32,
        0x0300..=0x036F
            | 0x0590..=0x05FF
            | 0x0600..=0x08FF
            | 0x0900..=0x0DFF
            | 0x0E00..=0x0FFF
            | 0x1000..=0x109F
            | 0x1700..=0x18AF
            | 0x1900..=0x1CFF
            | 0x1DC0..=0x1DFF
            | 0x20D0..=0x20FF
            | 0xFE20..=0xFE2F
            | 0xFB1D..=0xFB4F
            | 0xFB50..=0xFDFF
            | 0xFE70..=0xFEFF
            | 0x10E80..=0x10EBF
            | 0x1E900..=0x1E95F
            | 0x1EE00..=0x1EEFF
            | 0x1F3FB..=0x1F3FF
    )
}

/// Converts coverage to exact, non-overlapping boxes.
///
/// Foreground runs are accumulated vertically while their horizontal span is
/// unchanged, then extruded once along the requesting plane's depth axis.
pub fn compile_text_mask(
    request: &VoxelTextRequest,
    mask: &TextMask,
    limits: &Limits,
) -> Result<TextCompilation> {
    validate_text_request(request, limits)?;
    let area = (mask.width as u64)
        .checked_mul(mask.height as u64)
        .ok_or_else(|| GeometryError("文字栅格尺寸溢出".into()))?;
    if area == 0 {
        return Err(GeometryError("文字栅格为空".into()));
    }
    if area > MAX_RASTER_AREA {
        return Err(GeometryError(format!(
            "文字栅格超过{}像素预算",
            MAX_RASTER_AREA
        )));
    }
    if mask.coverage.len() as u64 != area {
        return Err(GeometryError("文字栅格长度与宽高不一致".into()));
    }
    let rectangles = rectangles(mask);
    if rectangles.is_empty() {
        return Err(GeometryError("文字栅格没有前景像素".into()));
    }
    if rectangles.len() > MAX_TEXT_BOXES || rectangles.len() > limits.max_ops {
        return Err(GeometryError(format!(
            "文字合并后超过{}个Box预算",
            MAX_TEXT_BOXES.min(limits.max_ops)
        )));
    }

    let basis = basis(request.plane, request.facing);
    let mut operations = Vec::with_capacity(rectangles.len());
    let mut voxel_count = 0u64;
    for (x0, x1, y0, y1) in rectangles {
        let (u_start, u_size) = axis_interval(
            request.origin[basis.u_axis],
            mask.width,
            x0,
            x1,
            basis.u_sign,
        )?;
        let (v_start, v_size) = axis_interval(
            request.origin[basis.v_axis],
            mask.height,
            y0,
            y1,
            basis.v_sign,
        )?;
        let mut min = [0i32; 3];
        let mut size = [0u32; 3];
        min[basis.u_axis] = u_start;
        size[basis.u_axis] = u_size;
        min[basis.v_axis] = v_start;
        size[basis.v_axis] = v_size;
        min[basis.depth_axis] = request.origin[basis.depth_axis];
        size[basis.depth_axis] = request.depth;

        let mut value = Box3::new(min, size, request.style_id);
        value.tint_id = request.tint_id;
        value.motion_phase = request.motion_phase;
        value.group = request.group.clone();
        value.validate(limits)?;
        voxel_count = voxel_count
            .checked_add(value.volume())
            .ok_or_else(|| GeometryError("文字体素量溢出".into()))?;
        if voxel_count > MAX_TEXT_VOXELS {
            return Err(GeometryError(format!(
                "文字超过{}个体素预算",
                MAX_TEXT_VOXELS
            )));
        }
        operations.push(Operation::Box { value });
    }

    let dimensions = plane_dimensions(request.plane, mask.width, mask.height, request.depth);
    let mut max = [0i32; 3];
    for axis in 0..3 {
        max[axis] = request.origin[axis]
            .checked_add(dimensions[axis] as i32)
            .filter(|value| value.unsigned_abs() <= limits.max_coord as u32)
            .ok_or_else(|| GeometryError("文字包围盒超出工程资源保护范围".into()))?;
    }
    Ok(TextCompilation {
        layout: TextLayout {
            width: mask.width,
            height: mask.height,
            depth: request.depth,
            voxel_count,
            box_count: operations.len(),
            bounds: Bounds {
                min: request.origin,
                max,
            },
            missing_glyphs: vec![],
        },
        operations,
    })
}

fn axis_interval(
    origin: i32,
    canvas_extent: u32,
    start: u32,
    end: u32,
    sign: i32,
) -> Result<(i32, u32)> {
    if start >= end || end > canvas_extent {
        return Err(GeometryError("文字栅格区间无效".into()));
    }
    let offset = if sign > 0 {
        start
    } else {
        canvas_extent
            .checked_sub(end)
            .ok_or_else(|| GeometryError("文字坐标映射溢出".into()))?
    };
    let value = origin
        .checked_add(offset as i32)
        .ok_or_else(|| GeometryError("文字坐标映射溢出".into()))?;
    Ok((value, end - start))
}

fn plane_dimensions(plane: TextPlane, width: u32, height: u32, depth: u32) -> [u32; 3] {
    match plane {
        TextPlane::Xy => [width, height, depth],
        TextPlane::Xz => [width, depth, height],
        TextPlane::Yz => [depth, height, width],
    }
}

type Rect = (u32, u32, u32, u32);

fn rectangles(mask: &TextMask) -> Vec<Rect> {
    let mut active: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    let mut finished = Vec::new();
    for row in 0..mask.height {
        let mut runs = Vec::new();
        let mut column = 0;
        while column < mask.width {
            while column < mask.width && !mask.foreground(column, row) {
                column += 1;
            }
            if column == mask.width {
                break;
            }
            let start = column;
            while column < mask.width && mask.foreground(column, row) {
                column += 1;
            }
            runs.push((start, column));
        }

        let ended: Vec<_> = active
            .keys()
            .copied()
            .filter(|span| !runs.contains(span))
            .collect();
        for span in ended {
            let start_row = active.remove(&span).unwrap_or(row);
            finished.push((span.0, span.1, start_row, row));
        }
        for span in runs {
            active.entry(span).or_insert(row);
        }
    }
    for (span, start_row) in active {
        finished.push((span.0, span.1, start_row, mask.height));
    }
    finished.sort_unstable_by_key(|(x0, _, y0, _)| (*y0, *x0));
    finished
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn request() -> VoxelTextRequest {
        VoxelTextRequest {
            text: "x".into(),
            ..VoxelTextRequest::default()
        }
    }

    fn covered(compilation: &TextCompilation) -> BTreeSet<[i32; 3]> {
        let mut cells = BTreeSet::new();
        for operation in &compilation.operations {
            let Operation::Box { value } = operation else {
                panic!("text must only emit boxes");
            };
            let end = value.end();
            for x in value.min[0]..end[0] {
                for y in value.min[1]..end[1] {
                    for z in value.min[2]..end[2] {
                        assert!(cells.insert([x, y, z]), "overlapping output");
                    }
                }
            }
        }
        cells
    }

    #[test]
    fn rectangle_merge_is_exact_and_does_not_fill_holes() {
        let mask = TextMask::new(
            5,
            4,
            vec![
                255, 255, 0, 0, 0, //
                255, 255, 0, 255, 255, //
                255, 255, 0, 255, 255, //
                0, 0, 0, 0, 0,
            ],
        )
        .unwrap();
        let compiled = compile_text_mask(&request(), &mask, &Limits::default()).unwrap();
        assert_eq!(compiled.operations.len(), 2);
        let occupied = covered(&compiled);
        assert_eq!(occupied.len(), 10);
        assert!(!occupied.contains(&[2, 0, 0]));
        assert_eq!(compiled.layout.voxel_count, 10);
    }

    #[test]
    fn positive_facing_has_explicit_renderer_basis() {
        let mask = TextMask::new(3, 2, vec![255, 0, 0, 0, 0, 0]).unwrap();
        let base = request();
        let compiled = compile_text_mask(&base, &mask, &Limits::default()).unwrap();
        let occupied = covered(&compiled);
        // XY positive, viewed from +Z: screen right is -X and screen up is +Y.
        // The mask's top-left pixel therefore lands at (x=2, y=1).
        assert!(occupied.contains(&[2, 1, 0]));
        assert!(!occupied.contains(&[0, 1, 0]));

        let negative = VoxelTextRequest {
            facing: TextFacing::Negative,
            ..base.clone()
        };
        let flipped = compile_text_mask(&negative, &mask, &Limits::default()).unwrap();
        let occupied = covered(&flipped);
        assert!(occupied.contains(&[0, 1, 0]));
        assert!(!occupied.contains(&[2, 1, 0]));
    }

    #[test]
    fn all_planes_and_depth_stay_inside_origin_bbox() {
        let mask = TextMask::new(3, 2, vec![255, 0, 255, 0, 255, 0]).unwrap();
        for plane in [TextPlane::Xy, TextPlane::Xz, TextPlane::Yz] {
            for facing in [TextFacing::Positive, TextFacing::Negative] {
                let request = VoxelTextRequest {
                    origin: [-3, 4, -5],
                    plane,
                    facing,
                    depth: 2,
                    ..request()
                };
                let compiled = compile_text_mask(&request, &mask, &Limits::default()).unwrap();
                for operation in &compiled.operations {
                    let Operation::Box { value } = operation else {
                        unreachable!()
                    };
                    for axis in 0..3 {
                        assert!(value.min[axis] >= compiled.layout.bounds.min[axis]);
                        assert!(value.end()[axis] <= compiled.layout.bounds.max[axis]);
                    }
                }
            }
        }
    }

    #[test]
    fn budgets_and_complex_scripts_are_rejected() {
        let mut value = request();
        for text in ["مرحبا", "\u{FEFB}", "a\u{200D}b", "\u{FE0F}"] {
            value.text = text.into();
            assert!(validate_text_request(&value, &Limits::default()).is_err());
        }
        value.text = "x".into();
        value.depth = 0;
        assert!(validate_text_request(&value, &Limits::default()).is_err());
        let mask = TextMask::new(1, 1, vec![0]).unwrap();
        assert!(compile_text_mask(&request(), &mask, &Limits::default()).is_err());
    }
}
