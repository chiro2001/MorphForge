use anyhow::{bail, Context, Result};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use morphforge_core::materials::Material;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};
use std::path::Path;

use crate::store::Workspace;

const MAX_ZIP_BYTES: usize = 128 * 1024 * 1024;
const MAX_ZIP_ENTRIES: usize = 131_072;
const MAX_ENTRY_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
const MAX_PATH_BYTES: usize = 512;
const MAX_PNG_PIXELS: u64 = 4_194_304;
const MAX_PNG_SIDE: u32 = 16_384;
const MAX_DECODED_TEXTURE_PIXELS: u64 = 64 * 1024 * 1024;
const MAX_TEXTURES: usize = 4_096;
const MAX_ANIMATION_TICKS: u32 = 1_024;
const MAX_MODEL_PARENT_DEPTH: usize = 16;
const MAX_REPORT_WARNINGS: usize = 200;
const TICK_SECONDS: f32 = 0.05;

#[derive(Clone)]
struct TextureAsset {
    uid: String,
    image: RgbaImage,
    original: Vec<u8>,
    frame_width: u32,
    frame_height: u32,
    ordered_frames: Vec<usize>,
    frame_times: Vec<u32>,
    interpolate: bool,
    animated: bool,
}

impl TextureAsset {
    fn total_ticks(&self) -> u32 {
        self.frame_times.iter().copied().sum()
    }

    fn source_frame(&self, index: usize) -> (u32, u32) {
        let columns = self.image.width() / self.frame_width;
        (
            (index as u32 % columns) * self.frame_width,
            (index as u32 / columns) * self.frame_height,
        )
    }

    fn sample_tick(&self, tick: u32) -> (usize, usize, f32) {
        let total = self.total_ticks().max(1);
        let tick = tick % total;
        let mut cursor = 0;
        for (index, duration) in self.frame_times.iter().copied().enumerate() {
            if tick < cursor + duration {
                let offset = tick - cursor;
                let fraction = if self.interpolate && duration > 1 {
                    offset as f32 / duration as f32
                } else {
                    0.0
                };
                let current = self.ordered_frames[index];
                let next = self.ordered_frames[(index + 1) % self.ordered_frames.len()];
                return (current, next, fraction);
            }
            cursor += duration;
        }
        let index = self.ordered_frames.len().saturating_sub(1);
        (self.ordered_frames[index], self.ordered_frames[0], 0.0)
    }
}

#[derive(Clone)]
struct FaceLayer {
    texture: String,
    tint: bool,
    rotation: u32,
}
struct ResolvedModel {
    layers: Option<Vec<Vec<FaceLayer>>>,
    archetype: String,
    textures: BTreeMap<String, String>,
}

fn warning(warnings: &mut Vec<String>, message: impl Into<String>) {
    if warnings.len() < MAX_REPORT_WARNINGS {
        warnings.push(message.into());
    } else if warnings.len() == MAX_REPORT_WARNINGS {
        warnings.push("更多导入警告已省略".to_owned());
    }
}

fn valid_path_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && segment.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_.-".contains(&byte)
        })
}

fn valid_resource_path(path: &str, allow_slash: bool) -> bool {
    !path.is_empty()
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && (allow_slash
                    || part.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_.-".contains(&byte)
                    }))
        })
        && path.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_.-/:".contains(&byte)
        })
}

fn texture_path(path: &str) -> Option<(String, String)> {
    let rest = path.strip_prefix("assets/")?;
    let mut parts = rest.split('/');
    let namespace = parts.next()?;
    if parts.next()? != "textures" || parts.next()? != "block" {
        return None;
    }
    let relative = format!("block/{}", parts.collect::<Vec<_>>().join("/"));
    if !relative.ends_with(".png") && !relative.ends_with(".png.mcmeta") {
        return None;
    }
    valid_path_segment(namespace).then(|| (namespace.to_owned(), relative))
}

fn model_path(path: &str) -> Option<(String, String)> {
    let rest = path.strip_prefix("assets/")?.strip_suffix(".json")?;
    let mut parts = rest.split('/');
    let namespace = parts.next()?;
    if parts.next()? != "models" || parts.next()? != "block" {
        return None;
    }
    let relative = parts.collect::<Vec<_>>().join("/");
    valid_path_segment(namespace).then(|| (namespace.to_owned(), relative))
}

fn wanted_entry(path: &str) -> bool {
    path == "pack.mcmeta"
        || path == "version.json"
        || texture_path(path).is_some()
        || model_path(path).is_some()
        || (path.starts_with("assets/")
            && path.contains("/blockstates/")
            && path.ends_with(".json"))
}

fn safe_entry_path(name: &str) -> Result<String> {
    if name.is_empty() || name.len() > MAX_PATH_BYTES || name.contains('\0') || name.contains('\\')
    {
        bail!("ZIP包含过长或无效路径");
    }
    if name.starts_with('/') || Path::new(name).is_absolute() {
        bail!("ZIP包含绝对路径");
    }
    if name
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        bail!("ZIP包含路径穿越");
    }
    Ok(name.to_owned())
}

type PackFiles = BTreeMap<String, Vec<u8>>;
fn read_pack(bytes: &[u8]) -> Result<(PackFiles, Vec<String>)> {
    if bytes.len() > MAX_ZIP_BYTES {
        bail!("资源包压缩文件超过128MiB");
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).context("无法读取资源包ZIP")?;
    if archive.len() > MAX_ZIP_ENTRIES {
        bail!("资源包文件数超过保护值");
    }

    let mut files = BTreeMap::new();
    let mut warnings = Vec::new();
    let mut folded_paths = BTreeSet::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).context("无法读取ZIP条目")?;
        let unix_mode = entry.unix_mode();
        if unix_mode.is_some_and(|mode| {
            let kind = mode & 0o170000;
            kind != 0 && kind != 0o100000 && kind != 0o040000
        }) {
            bail!("不接受ZIP符号链接或特殊文件");
        }
        if entry.is_dir() {
            continue;
        }
        let path = safe_entry_path(entry.name())?;
        if !wanted_entry(&path) {
            continue;
        }
        let folded = path.to_ascii_lowercase();
        if !folded_paths.insert(folded) {
            bail!("ZIP包含重复或仅大小写不同的路径：{path}");
        }
        if entry.size() > MAX_ENTRY_BYTES {
            bail!("资源包单文件超过16MiB：{path}");
        }
        let mut data = Vec::with_capacity(entry.size().min(MAX_ENTRY_BYTES) as usize);
        entry
            .by_ref()
            .take(MAX_ENTRY_BYTES + 1)
            .read_to_end(&mut data)
            .with_context(|| format!("读取资源包文件失败：{path}"))?;
        if data.len() as u64 > MAX_ENTRY_BYTES {
            bail!("资源包文件解压后过大：{path}");
        }
        total = total
            .checked_add(data.len() as u64)
            .context("资源包解压总量溢出")?;
        if total > MAX_TOTAL_BYTES {
            bail!("资源包解压内容超过256MiB");
        }
        files.insert(path, data);
    }
    if !files.contains_key("pack.mcmeta") {
        warning(&mut warnings, "缺少根目录 pack.mcmeta");
    }
    Ok((files, warnings))
}

fn canonical_model(value: &str) -> String {
    if value.contains(':') {
        value.to_owned()
    } else {
        format!("minecraft:{value}")
    }
}

fn canonical_texture(value: &str) -> String {
    if value.contains(':') {
        value.to_owned()
    } else {
        format!("minecraft:{value}")
    }
}

fn parse_png(path: &str, bytes: &[u8]) -> Result<RgbaImage> {
    png_dimensions(path, bytes)?;
    Ok(image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .with_context(|| format!("资源包PNG解码失败：{path}"))?
        .to_rgba8())
}

fn png_dimensions(path: &str, bytes: &[u8]) -> Result<(u32, u32)> {
    if bytes.len() < 24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        bail!("不是有效PNG：{path}");
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into()?);
    if width == 0
        || height == 0
        || width > MAX_PNG_SIDE
        || height > MAX_PNG_SIDE
        || width as u64 * height as u64 > MAX_PNG_PIXELS
    {
        bail!("PNG尺寸超过保护值：{path}");
    }
    Ok((width, height))
}

fn positive_u32(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .filter(|number| *number > 0)
}

fn parse_texture(
    uid: &str,
    path: &str,
    bytes: &[u8],
    metadata: Option<&[u8]>,
    warnings: &mut Vec<String>,
) -> Option<TextureAsset> {
    let image = match parse_png(path, bytes) {
        Ok(image) => image,
        Err(error) => {
            warning(warnings, format!("跳过无效PNG {path}：{error}"));
            return None;
        }
    };
    let mut asset = TextureAsset {
        uid: uid.to_owned(),
        frame_width: image.width(),
        frame_height: image.height(),
        image,
        original: bytes.to_vec(),
        ordered_frames: vec![0],
        frame_times: vec![1],
        interpolate: false,
        animated: false,
    };
    let Some(metadata) = metadata else {
        if asset.frame_width != asset.frame_height {
            warning(
                warnings,
                format!("静态方块贴图必须为正方形，已跳过：{path}"),
            );
            return None;
        }
        return Some(asset);
    };
    let json: Value = match serde_json::from_slice(metadata) {
        Ok(value) => value,
        Err(error) => {
            warning(
                warnings,
                format!("忽略无效动画元数据 {path}.mcmeta：{error}"),
            );
            return Some(asset);
        }
    };
    let Some(animation) = json.get("animation").and_then(Value::as_object) else {
        if asset.frame_width != asset.frame_height {
            warning(
                warnings,
                format!("静态方块贴图必须为正方形，已跳过：{path}"),
            );
            return None;
        }
        return Some(asset);
    };
    let image_width = asset.image.width();
    let image_height = asset.image.height();
    let default_side = image_width.min(image_height);
    let explicit_width = animation.get("width").and_then(positive_u32);
    let explicit_height = animation.get("height").and_then(positive_u32);
    let (frame_width, frame_height) = match (explicit_width, explicit_height) {
        (Some(width), Some(height)) => (width, height),
        (Some(width), None) => (width, width),
        (None, Some(height)) => (height, height),
        (None, None) => (default_side, default_side),
    };
    if frame_width == 0
        || frame_height == 0
        || !image_width.is_multiple_of(frame_width)
        || !image_height.is_multiple_of(frame_height)
    {
        warning(
            warnings,
            format!("动画帧尺寸不能整除贴图 {path}：{frame_width}x{frame_height}"),
        );
        return Some(asset);
    }
    let available = (image_width / frame_width) * (image_height / frame_height);
    if available == 0 || available > MAX_ANIMATION_TICKS {
        warning(warnings, format!("动画帧数超过保护值：{path}"));
        return Some(asset);
    }
    let default_time = animation
        .get("frametime")
        .and_then(positive_u32)
        .filter(|value| *value <= MAX_ANIMATION_TICKS)
        .unwrap_or_else(|| {
            if animation.get("frametime").is_some() {
                warning(
                    warnings,
                    format!("动画frametime无效，已按1 tick处理：{path}"),
                );
            }
            1
        });
    let mut ordered_frames = Vec::new();
    let mut frame_times = Vec::new();
    if let Some(frames) = animation.get("frames").and_then(Value::as_array) {
        if frames.len() > MAX_ANIMATION_TICKS as usize {
            warning(
                warnings,
                format!(
                    "动画 frames 条目超过{}，仅读取前{}项：{path}",
                    MAX_ANIMATION_TICKS, MAX_ANIMATION_TICKS
                ),
            );
        }
        for (position, frame) in frames.iter().take(MAX_ANIMATION_TICKS as usize).enumerate() {
            let (index, time) = if let Some(index) = frame.as_u64() {
                (index, default_time)
            } else if let Some(object) = frame.as_object() {
                let index = object
                    .get("index")
                    .and_then(Value::as_u64)
                    .unwrap_or_default();
                let time = object
                    .get("time")
                    .and_then(positive_u32)
                    .unwrap_or(default_time);
                (index, time)
            } else {
                warning(
                    warnings,
                    format!("忽略无效动画帧 {path} frames[{position}]"),
                );
                continue;
            };
            if index >= available as u64 || time == 0 || time > MAX_ANIMATION_TICKS {
                warning(
                    warnings,
                    format!("忽略越界动画帧 {path} frames[{position}]"),
                );
                continue;
            }
            ordered_frames.push(index as usize);
            frame_times.push(time);
        }
    }
    if ordered_frames.is_empty() {
        ordered_frames = (0..available as usize).collect();
        frame_times = vec![default_time; available as usize];
    }
    let total: u32 = frame_times.iter().sum();
    if total == 0 || total > MAX_ANIMATION_TICKS {
        warning(
            warnings,
            format!("动画周期超过{MAX_ANIMATION_TICKS} ticks，已按静态贴图处理：{path}"),
        );
        return Some(asset);
    }
    asset.frame_width = frame_width;
    asset.frame_height = frame_height;
    asset.ordered_frames = ordered_frames;
    asset.frame_times = frame_times;
    asset.interpolate = animation
        .get("interpolate")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    asset.animated = true;
    Some(asset)
}

fn load_textures(
    files: &BTreeMap<String, Vec<u8>>,
    warnings: &mut Vec<String>,
) -> BTreeMap<String, TextureAsset> {
    let mut textures = BTreeMap::new();
    let mut decoded_pixels = 0u64;
    for (path, bytes) in files {
        let Some((namespace, relative)) = texture_path(path) else {
            continue;
        };
        if !relative.ends_with(".png") {
            if !files.contains_key(path.trim_end_matches(".mcmeta")) {
                warning(warnings, format!("动画元数据缺少对应贴图：{path}"));
            }
            continue;
        }
        if !valid_resource_path(relative.trim_end_matches(".png"), false) {
            warning(warnings, format!("忽略无效资源名：{path}"));
            continue;
        }
        if textures.len() >= MAX_TEXTURES {
            warning(
                warnings,
                format!("贴图数量超过{MAX_TEXTURES}，其余贴图已跳过"),
            );
            break;
        }
        let dimensions = match png_dimensions(path, bytes) {
            Ok(dimensions) => dimensions,
            Err(error) => {
                warning(warnings, format!("跳过无效PNG {path}：{error}"));
                continue;
            }
        };
        let pixels = dimensions.0 as u64 * dimensions.1 as u64;
        if decoded_pixels.saturating_add(pixels) > MAX_DECODED_TEXTURE_PIXELS {
            warning(
                warnings,
                format!("贴图解码总量超过保护值，跳过后续贴图：{path}"),
            );
            break;
        }
        let uid = format!("{namespace}:{}", relative.trim_end_matches(".png"));
        let metadata = files.get(&format!("{path}.mcmeta")).map(Vec::as_slice);
        if let Some(asset) = parse_texture(&uid, path, bytes, metadata, warnings) {
            decoded_pixels += pixels;
            textures.insert(uid, asset);
        }
    }
    textures
}

fn resolve_model(
    uid: &str,
    models: &BTreeMap<String, Value>,
    warnings: &mut Vec<String>,
) -> Option<ResolvedModel> {
    let mut chain = Vec::new();
    let mut current = uid.to_owned();
    let mut seen = BTreeSet::new();
    let mut archetype = None;
    let mut cube_faces: Option<BTreeMap<String, String>> = None;
    let mut face_layers = None;
    for _ in 0..MAX_MODEL_PARENT_DEPTH {
        if !seen.insert(current.clone()) {
            warning(warnings, format!("模型父链循环：{uid}"));
            return None;
        }
        let Some(model) = models.get(&current) else {
            archetype = Some(current.clone());
            break;
        };
        if let Some(elements) = model
            .get("elements")
            .and_then(Value::as_array)
            .filter(|e| !e.is_empty())
        {
            // Official Java packs include the cube template itself. Accept its
            // full 16³ cuboid and six texture references; reject non-cube custom
            // geometry instead of silently treating stairs/doors as cubes.
            if elements.len() <= 4
                && elements.iter().all(|e| {
                    e["from"] == json!([0, 0, 0])
                        && e["to"] == json!([16, 16, 16])
                        && e.get("rotation").is_none()
                })
            {
                let layers = ["east", "west", "up", "down", "south", "north"]
                    .into_iter()
                    .map(|face| {
                        elements
                            .iter()
                            .filter_map(|e| e["faces"].get(face))
                            .map(|f| {
                                Some(FaceLayer {
                                    texture: f["texture"].as_str()?.into(),
                                    tint: f["tintindex"].is_number(),
                                    rotation: f["rotation"].as_u64().unwrap_or(0) as u32,
                                })
                            })
                            .collect::<Option<Vec<_>>>()
                    })
                    .collect::<Option<Vec<_>>>();
                if let Some(layers) = layers.filter(|l| l.iter().all(|f| !f.is_empty())) {
                    cube_faces = Some(
                        ["east", "west", "up", "down", "south", "north"]
                            .into_iter()
                            .zip(layers.iter())
                            .map(|(f, l)| (f.to_string(), l[0].texture.clone()))
                            .collect(),
                    );
                    face_layers = Some(layers);
                    archetype = Some("minecraft:block/cube".into());
                    chain.push(model.clone());
                    break;
                }
            }
            warning(warnings, format!("未展开非整立方体elements：{current}"));
            return None;
        }
        chain.push(model.clone());
        let Some(parent) = model.get("parent").and_then(Value::as_str) else {
            break;
        };
        current = canonical_model(parent);
    }
    if chain.len() >= MAX_MODEL_PARENT_DEPTH && archetype.is_none() {
        warning(warnings, format!("模型父链过深：{uid}"));
        return None;
    }
    let mut textures = BTreeMap::new();
    for model in chain.iter().rev() {
        if let Some(values) = model.get("textures").and_then(Value::as_object) {
            for (key, value) in values {
                if let Some(value) = value.as_str() {
                    textures.insert(key.clone(), value.to_owned());
                }
            }
        }
    }
    if let Some(faces) = cube_faces {
        for (face, reference) in faces {
            if reference != format!("#{face}") {
                textures.insert(face, reference);
            }
        }
    }
    let archetype = archetype.or_else(|| {
        if textures.contains_key("all") {
            Some("minecraft:block/cube_all".to_owned())
        } else if ["east", "west", "up", "down", "south", "north"]
            .iter()
            .all(|key| textures.contains_key(*key))
        {
            Some("minecraft:block/cube".to_owned())
        } else {
            None
        }
    })?;
    Some(ResolvedModel {
        layers: face_layers,
        archetype,
        textures,
    })
}

fn resolve_texture_reference(key: &str, textures: &BTreeMap<String, String>) -> Option<String> {
    let mut value = textures.get(key)?.as_str();
    let mut seen = BTreeSet::new();
    for _ in 0..MAX_MODEL_PARENT_DEPTH {
        if let Some(alias) = value.strip_prefix('#') {
            if !seen.insert(alias) {
                return None;
            }
            value = textures.get(alias)?.as_str();
        } else {
            return Some(canonical_texture(value));
        }
    }
    None
}

fn face_keys(archetype: &str) -> Option<[&'static str; 6]> {
    let archetype = archetype.strip_prefix("minecraft:").unwrap_or(archetype);
    match archetype {
        "block/cube_all" | "block/cube_mirrored_all" => Some(["all"; 6]),
        "block/cube_column" => Some(["side", "side", "end", "end", "side", "side"]),
        "block/cube_bottom_top" => Some(["side", "side", "top", "bottom", "side", "side"]),
        "block/cube_top" => Some(["side", "side", "top", "top", "side", "side"]),
        "block/cube" => Some(["east", "west", "up", "down", "south", "north"]),
        _ => None,
    }
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let next = a % b;
        a = b;
        b = next;
    }
    a
}

fn common_timeline(faces: &[&TextureAsset], warnings: &mut Vec<String>) -> Option<u32> {
    let mut ticks = 1u32;
    for asset in faces.iter().filter(|asset| asset.animated) {
        let total = asset.total_ticks().max(1);
        let factor = total / gcd(ticks, total);
        ticks = ticks.checked_mul(factor)?;
        if ticks > MAX_ANIMATION_TICKS {
            warning(
                warnings,
                format!("六面动画周期公倍数超过{MAX_ANIMATION_TICKS} ticks，跳过材质"),
            );
            return None;
        }
    }
    Some(ticks)
}

fn blend_pixel(current: Rgba<u8>, next: Rgba<u8>, fraction: f32) -> Rgba<u8> {
    let mix = |a: u8, b: u8| {
        (a as f32 * (1.0 - fraction) + b as f32 * fraction)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Rgba([
        mix(current[0], next[0]),
        mix(current[1], next[1]),
        mix(current[2], next[2]),
        mix(current[3], next[3]),
    ])
}

fn bake_texture(workspace: &mut Workspace, asset: &TextureAsset, ticks: u32) -> Result<String> {
    if !asset.animated && ticks == 1 {
        if asset.image.width() == asset.image.height() {
            return workspace.artifact(&asset.original, "png");
        }
        let side = asset.image.width().min(asset.image.height());
        let first = image::imageops::crop_imm(&asset.image, 0, 0, side, side).to_image();
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(first).write_to(&mut encoded, ImageFormat::Png)?;
        return workspace.artifact(encoded.get_ref(), "png");
    }
    let output_height = asset
        .frame_height
        .checked_mul(ticks)
        .context("动画贴图高度溢出")?;
    if output_height > MAX_PNG_SIDE
        || asset.frame_width as u64 * output_height as u64 > MAX_PNG_PIXELS
    {
        bail!("动画规范化贴图超过保护值：{}", asset.uid);
    }
    let mut output = RgbaImage::new(asset.frame_width, output_height);
    for tick in 0..ticks {
        let (current, next, fraction) = asset.sample_tick(tick);
        let (current_x, current_y) = asset.source_frame(current);
        let (next_x, next_y) = asset.source_frame(next);
        for y in 0..asset.frame_height {
            for x in 0..asset.frame_width {
                let current_pixel = *asset.image.get_pixel(current_x + x, current_y + y);
                let pixel = if fraction > 0.0 {
                    let next_pixel = *asset.image.get_pixel(next_x + x, next_y + y);
                    blend_pixel(current_pixel, next_pixel, fraction)
                } else {
                    current_pixel
                };
                output.put_pixel(x, tick * asset.frame_height + y, pixel);
            }
        }
    }
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(output).write_to(&mut encoded, ImageFormat::Png)?;
    workspace.artifact(encoded.get_ref(), "png")
}

fn bake_face(
    workspace: &mut Workspace,
    layers: &[FaceLayer],
    bindings: &BTreeMap<String, String>,
    textures: &BTreeMap<String, TextureAsset>,
    ticks: u32,
    block: &str,
) -> Result<String> {
    let resolved = layers
        .iter()
        .map(|l| {
            let uid = if let Some(key) = l.texture.strip_prefix('#') {
                resolve_texture_reference(key, bindings).context("分层纹理引用未解析")?
            } else {
                canonical_texture(&l.texture)
            };
            let asset = textures.get(&uid).context("分层纹理不存在")?;
            Ok((asset, l))
        })
        .collect::<Result<Vec<_>>>()?;
    let side = resolved
        .iter()
        .map(|(a, _)| a.frame_width)
        .max()
        .context("空白面")?;
    if side as u64 * side as u64 * ticks as u64 > MAX_PNG_PIXELS || side * ticks > MAX_PNG_SIDE {
        bail!("分层纹理尺寸超过限制")
    }
    let mut output = RgbaImage::new(side, side * ticks);
    let tint = if block.contains("spruce") {
        [97u32, 153, 97]
    } else if block.contains("birch") {
        [128, 167, 85]
    } else if block.contains("leaves") {
        [119, 171, 47]
    } else {
        [145, 189, 89]
    };
    for tick in 0..ticks {
        for y in 0..side {
            for x in 0..side {
                let mut result = Rgba([0u8; 4]);
                for (asset, layer) in &resolved {
                    let (mut xx, mut yy) =
                        (x * asset.frame_width / side, y * asset.frame_height / side);
                    match layer.rotation % 360 {
                        90 => (xx, yy) = (yy, asset.frame_width - 1 - xx),
                        180 => {
                            xx = asset.frame_width - 1 - xx;
                            yy = asset.frame_height - 1 - yy
                        }
                        270 => (xx, yy) = (asset.frame_height - 1 - yy, xx),
                        _ => {}
                    }
                    let (cur, next, fraction) = asset.sample_tick(tick);
                    let (cx, cy) = asset.source_frame(cur);
                    let (nx, ny) = asset.source_frame(next);
                    let mut pixel = blend_pixel(
                        *asset.image.get_pixel(cx + xx, cy + yy),
                        *asset.image.get_pixel(nx + xx, ny + yy),
                        fraction,
                    );
                    if layer.tint {
                        for c in 0..3 {
                            pixel[c] = ((pixel[c] as u32 * tint[c] + 127) / 255) as u8;
                        }
                    }
                    let alpha = pixel[3] as u32;
                    let old_alpha = result[3] as u32;
                    let final_alpha = alpha + (old_alpha * (255 - alpha) + 127) / 255;
                    for c in 0..3 {
                        let numerator = pixel[c] as u32 * alpha
                            + result[c] as u32 * old_alpha * (255 - alpha) / 255
                            + final_alpha / 2;
                        if let Some(value) = numerator.checked_div(final_alpha) {
                            result[c] = value.min(255) as u8;
                        }
                    }
                    result[3] = final_alpha as u8;
                }
                output.put_pixel(x, tick * side + y, result);
            }
        }
    }
    let mut encoded = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(output).write_to(&mut encoded, ImageFormat::Png)?;
    workspace.artifact(encoded.get_ref(), "png")
}

pub fn alpha_render_mode(name: &str, partial: bool, holes: bool) -> &'static str {
    if name.contains("water") {
        "transparent"
    } else if name.contains("leaves") {
        "cutout"
    } else if partial {
        "transparent"
    } else if holes {
        "cutout"
    } else {
        "opaque"
    }
}

fn material_uid(uid: &str, pack_id: &str) -> String {
    format!("{uid}@{}", &pack_id[..8])
}

fn import_model_materials(
    workspace: &mut Workspace,
    files: &BTreeMap<String, Vec<u8>>,
    textures: &BTreeMap<String, TextureAsset>,
    pack_id: &str,
    name: &str,
    warnings: &mut Vec<String>,
) -> Result<(Vec<Material>, usize, usize, usize)> {
    let mut models = BTreeMap::new();
    for (path, data) in files {
        let Some((namespace, relative)) = model_path(path) else {
            continue;
        };
        match serde_json::from_slice::<Value>(data) {
            Ok(value) => {
                models.insert(format!("{namespace}:block/{relative}"), value);
            }
            Err(error) => warning(warnings, format!("忽略无效模型 {path}：{error}")),
        }
    }

    let existing: BTreeSet<String> = workspace
        .project
        .materials
        .iter()
        .map(|material| material.uid.clone())
        .collect();
    let mut next = workspace
        .project
        .materials
        .iter()
        .map(|material| material.id)
        .max()
        .unwrap_or(0)
        + 1;
    let mut baked = BTreeMap::<(String, u32), String>::new();
    let mut additions = Vec::new();
    let mut animated_materials = 0usize;
    let mut recognized_models = 0usize;
    for model_uid in models.keys() {
        let uid = material_uid(model_uid, pack_id);
        if existing.contains(&uid) {
            recognized_models += 1;
            continue;
        }
        let Some(model) = resolve_model(model_uid, &models, warnings) else {
            continue;
        };
        let Some(keys) = face_keys(&model.archetype) else {
            warning(
                warnings,
                format!("未支持的模型继承 {model_uid}: {}", model.archetype),
            );
            continue;
        };
        let texture_ids: Option<Vec<String>> = keys
            .iter()
            .map(|key| resolve_texture_reference(key, &model.textures))
            .collect();
        let Some(texture_ids) = texture_ids else {
            warning(warnings, format!("纹理引用未解析 {model_uid}"));
            continue;
        };
        let face_assets: Option<Vec<&TextureAsset>> = texture_ids
            .iter()
            .map(|texture| textures.get(texture))
            .collect();
        let Some(face_assets) = face_assets else {
            warning(warnings, format!("模型引用了未导入贴图 {model_uid}"));
            continue;
        };
        let Some(ticks) = common_timeline(&face_assets, warnings) else {
            continue;
        };
        let mut urls = Vec::with_capacity(6);
        if let Some(layers) = &model.layers {
            let block = model_uid.trim_start_matches("minecraft:block/");
            for face in layers {
                urls.push(format!(
                    "/artifacts/{}",
                    bake_face(workspace, face, &model.textures, textures, ticks, block)?
                ));
            }
        }

        let mut failed = false;
        for asset in face_assets.iter().filter(|_| model.layers.is_none()) {
            let key = (asset.uid.clone(), ticks);
            let url = if let Some(url) = baked.get(&key) {
                url.clone()
            } else {
                match bake_texture(workspace, asset, ticks) {
                    Ok(artifact) => {
                        let url = format!("/artifacts/{artifact}");
                        baked.insert(key, url.clone());
                        url
                    }
                    Err(error) => {
                        warning(warnings, format!("动画贴图规范化失败：{error}"));
                        failed = true;
                        break;
                    }
                }
            };
            urls.push(url);
        }
        if failed {
            continue;
        }
        recognized_models += 1;
        let block = model_uid
            .split_once(':')
            .map(|(_, value)| value)
            .unwrap_or(model_uid)
            .trim_start_matches("block/");
        let is_water = block.contains("water");
        let partial = face_assets
            .iter()
            .any(|a| a.image.pixels().any(|p| p[3] > 0 && p[3] < 255));
        let holes = face_assets
            .iter()
            .any(|a| a.image.pixels().any(|p| p[3] < 255));
        let animation = ticks > 1;
        if animation {
            animated_materials += 1;
        }
        additions.push(Material {
            texture_scale: 1.0,
            texture_rotation: 0,
            procedural: None,
            id: next,
            uid,
            name: format!("{block} · {name}"),
            family: if is_water { "water" } else { "minecraft" }.into(),
            color: [1.0; 3],
            metallic: 0.0,
            roughness: 0.9,
            opacity: if is_water { 0.7 } else { 1.0 },
            emission: if block.contains("lava") || block.contains("glowstone") {
                1.2
            } else {
                0.0
            },
            render_mode: alpha_render_mode(block, partial, holes).into(),
            textures: urls,
            frames: ticks,
            frame_time: if animation { TICK_SECONDS } else { 0.1 },
            pack: Some(pack_id.to_owned()),
            source: Some(format!(
                "assets/{}/models/{}.json",
                model_uid
                    .split_once(':')
                    .map(|(ns, _)| ns)
                    .unwrap_or("minecraft"),
                model_uid
                    .split_once(':')
                    .map(|(_, path)| path)
                    .unwrap_or(model_uid)
            )),
        });
        next += 1;
    }
    Ok((
        additions,
        animated_materials,
        baked.len(),
        recognized_models,
    ))
}

fn import_texture_fallbacks(
    workspace: &mut Workspace,
    textures: &BTreeMap<String, TextureAsset>,
    pack_id: &str,
    name: &str,
    warnings: &mut Vec<String>,
    model_materials: &[Material],
) -> Result<(Vec<Material>, usize)> {
    let existing: BTreeSet<String> = workspace
        .project
        .materials
        .iter()
        .chain(model_materials.iter())
        .map(|material| material.uid.clone())
        .collect();
    let mut next = workspace
        .project
        .materials
        .iter()
        .chain(model_materials.iter())
        .map(|material| material.id)
        .max()
        .unwrap_or(0)
        + 1;
    let mut additions = Vec::new();
    let mut animated = 0usize;
    for asset in textures.values() {
        let uid = material_uid(&asset.uid, pack_id);
        if existing.contains(&uid) {
            continue;
        }
        let ticks = if asset.animated {
            asset.total_ticks().max(1)
        } else {
            1
        };
        let artifact = match bake_texture(workspace, asset, ticks) {
            Ok(artifact) => artifact,
            Err(error) => {
                warning(warnings, format!("动画贴图规范化失败：{error}"));
                continue;
            }
        };
        let url = format!("/artifacts/{artifact}");
        if ticks > 1 {
            animated += 1;
        }
        let block = asset
            .uid
            .split_once(':')
            .map(|(_, value)| value)
            .unwrap_or(&asset.uid)
            .trim_start_matches("block/");
        additions.push(Material {
            texture_scale: 1.0,
            texture_rotation: 0,
            procedural: None,
            id: next,
            uid,
            name: format!("{block} · {name}"),
            family: if block.contains("water") {
                "water"
            } else {
                "minecraft"
            }
            .into(),
            color: if block.contains("water") {
                [63. / 255., 118. / 255., 228. / 255.]
            } else if block.contains("leaves") {
                [119. / 255., 171. / 255., 47. / 255.]
            } else if block.contains("grass_block_top") {
                [145. / 255., 189. / 255., 89. / 255.]
            } else {
                [1.; 3]
            },
            metallic: 0.0,
            roughness: 0.9,
            opacity: if block.contains("water") { 0.7 } else { 1.0 },
            emission: if block.contains("lava") { 1.2 } else { 0. },
            render_mode: alpha_render_mode(
                block,
                asset.image.pixels().any(|p| p[3] > 0 && p[3] < 255),
                asset.image.pixels().any(|p| p[3] < 255),
            )
            .into(),
            textures: vec![url; 6],
            frames: ticks,
            frame_time: if ticks > 1 { TICK_SECONDS } else { 0.1 },
            pack: Some(pack_id.to_owned()),
            source: Some(asset.uid.clone()),
        });
        next += 1;
    }
    if model_materials.is_empty() {
        warning(warnings, "未找到兼容立方体模型，已提供单纹理六面预览材质");
    }
    Ok((additions, animated))
}

fn canonical_pack_hash(files: &BTreeMap<String, Vec<u8>>) -> String {
    let mut manifest = Vec::new();
    for (path, data) in files {
        manifest.extend_from_slice(&(path.len() as u64).to_le_bytes());
        manifest.extend_from_slice(path.as_bytes());
        manifest.extend_from_slice(&(data.len() as u64).to_le_bytes());
        manifest.extend_from_slice(data);
    }
    crate::store::hash(&manifest)
}

pub fn import_java_pack(workspace: &mut Workspace, bytes: &[u8], name: &str) -> Result<Value> {
    let (files, mut warnings) = read_pack(bytes)?;
    let metadata: Value = if let Some(meta) = files
        .get("pack.mcmeta")
        .and_then(|v| serde_json::from_slice(v).ok())
    {
        meta
    } else if let Some(version) = files
        .get("version.json")
        .and_then(|v| serde_json::from_slice::<Value>(v).ok())
    {
        warning(
            &mut warnings,
            "从Java客户端JAR读取原版方块资源，原JAR仅保留于本机",
        );
        json!({"pack":{"pack_format":0,"description":format!("Minecraft Java {} local client assets",version["id"].as_str().unwrap_or("unknown"))},"vanillaClient":true,"version":version["id"]})
    } else {
        bail!("缺少有效pack.mcmeta或Java客户端version.json")
    };
    if metadata.get("pack").and_then(Value::as_object).is_none() {
        bail!("pack.mcmeta缺少pack对象");
    }
    if metadata["pack"]
        .get("pack_format")
        .and_then(Value::as_u64)
        .is_none()
    {
        warning(&mut warnings, "pack.mcmeta缺少pack_format，仍尝试导入");
    }

    let pack_id = canonical_pack_hash(&files);
    let mut blockstates = BTreeMap::<String, Value>::new();
    for (path, data) in &files {
        let Some(rest) = path.strip_prefix("assets/") else {
            continue;
        };
        let Some((namespace, relative)) = rest.split_once("/blockstates/") else {
            continue;
        };
        let Some(block) = relative.strip_suffix(".json") else {
            continue;
        };
        if !valid_resource_path(block, false) {
            continue;
        }
        match serde_json::from_slice(data) {
            Ok(value) => {
                blockstates.insert(format!("{namespace}:{block}"), value);
            }
            Err(e) => warning(&mut warnings, format!("忽略无效blockstate {path}: {e}")),
        }
    }
    let state_bytes = serde_json::to_vec(&blockstates)?;
    if state_bytes.len() > 32 * 1024 * 1024 {
        bail!("方块状态索引超过32MiB保护值");
    }

    let textures = load_textures(&files, &mut warnings);
    if textures.is_empty() {
        bail!("没有可导入的assets/<namespace>/textures/block/*.png");
    }

    let (mut additions, mut animated_materials, baked_textures, recognized_models) =
        import_model_materials(workspace, &files, &textures, &pack_id, name, &mut warnings)?;
    if recognized_models == 0 || metadata["vanillaClient"] == true {
        let (fallback, animated) = import_texture_fallbacks(
            workspace,
            &textures,
            &pack_id,
            name,
            &mut warnings,
            &additions,
        )?;
        additions.extend(fallback);
        animated_materials += animated;
    }

    let count = additions.len();
    let mut materials = workspace.project.materials.clone();
    materials.extend(additions);
    // Preserve the validated source for later re-baking, independently of project edits.
    let source_path = workspace.root.join("packs").join(format!("{pack_id}.zip"));
    crate::store::atomic(&source_path, bytes)?;
    crate::store::atomic(
        &workspace
            .root
            .join("packs")
            .join(format!("{pack_id}-blockstates.json")),
        &state_bytes,
    )?;

    workspace.update_materials(materials)?;
    let report = json!({
        "ok": true,
        "packId": pack_id,
        "blockstateCount":blockstates.len(),
        "blockstatesHash":crate::store::hash(&state_bytes),
        "originalZipPreserved":true,
        "name": name,
        "metadata": metadata,
        "materialsAdded": count,
        "textures": textures.len(),
        "animatedMaterials": animated_materials,
        "bakedTextures": baked_textures,
        "warnings": warnings,
        "avatarRequiresBuild": true,
        "animation": {
            "timelineTicksPerSecond": 20,
            "frameTimeSeconds": TICK_SECONDS,
            "maxTimelineTicks": MAX_ANIMATION_TICKS,
            "explicitFrames": true,
            "perFrameTiming": true,
            "interpolation": true
        }
    });
    crate::store::write_json(
        &workspace.root.join("packs").join(format!("{pack_id}.json")),
        &report,
    )?;
    workspace.event("pack.imported", None, report.clone())?;
    Ok(report)
}
