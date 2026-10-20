//! Deterministic reference-image operations. Pixels stay local; no provider calls.
use crate::{references, store::Workspace};
use anyhow::{bail, Context, Result};
use image::{imageops::FilterType, DynamicImage, ImageFormat, RgbaImage};
use morphforge_core::{Box3, Operation};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Cursor,
};
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    action: String,
    image_id: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    crop: Option<Crop>,
    filter: Option<String>,
    points: Option<Vec<[u32; 2]>>,
    grid: Option<[u32; 2]>,
    base_revision: Option<u64>,
    dry_run: Option<bool>,
    plane: Option<String>,
    origin: Option<[i32; 3]>,
    thickness: Option<u32>,
    alpha_threshold: Option<u8>,
    max_colors: Option<usize>,
    material_ids: Option<Vec<u32>>,
    group: Option<String>,
    canvas_mode: Option<String>,
    title: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Crop {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}
fn pixels(w: &Workspace, r: &Request) -> Result<(RgbaImage, String)> {
    let id = r
        .image_id
        .as_deref()
        .context("需要imageId；先使用list或get_context获取参考图ID")?;
    let (bytes, _) = references::image(w, id, false)?;
    let mut im = image::load_from_memory(&bytes)?.to_rgba8();
    if let Some(c) = &r.crop {
        if c.width == 0
            || c.height == 0
            || c.x as u64 + c.width as u64 > im.width() as u64
            || c.y as u64 + c.height as u64 > im.height() as u64
        {
            bail!("裁剪超出源图边界");
        }
        im = image::imageops::crop_imm(&im, c.x, c.y, c.width, c.height).to_image();
    }
    let filter = match r.filter.as_deref().unwrap_or("area") {
        "nearest" => FilterType::Nearest,
        "area" => FilterType::Triangle,
        "lanczos" => FilterType::Lanczos3,
        _ => bail!("filter需要nearest/area/lanczos"),
    };
    let (width, height) = match (r.width, r.height) {
        (Some(a), Some(b)) => (a, b),
        (Some(a), None) => (
            a,
            ((a as u64 * im.height() as u64 + im.width() as u64 / 2) / im.width() as u64).max(1)
                as u32,
        ),
        (None, Some(b)) => (
            ((b as u64 * im.width() as u64 + im.height() as u64 / 2) / im.height() as u64).max(1)
                as u32,
            b,
        ),
        _ if r.action == "voxel_plane" => {
            if im.width() >= im.height() {
                (
                    64,
                    ((64u64 * im.height() as u64 + im.width() as u64 / 2) / im.width() as u64)
                        .max(1) as u32,
                )
            } else {
                (
                    ((64u64 * im.width() as u64 + im.height() as u64 / 2) / im.height() as u64)
                        .max(1) as u32,
                    64,
                )
            }
        }
        _ => (im.width(), im.height()),
    };
    let max = if r.action == "voxel_plane" { 256 } else { 1024 };
    if width == 0 || height == 0 || width > max || height > max {
        bail!("输出尺寸须为1..{max}；只指定一边时等比缩放");
    }
    if width != im.width() || height != im.height() {
        im = image::imageops::resize(&im, width, height, filter);
    }
    Ok((im, id.to_owned()))
}
fn artifact(w: &mut Workspace, im: &RgbaImage) -> Result<String> {
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(im.clone()).write_to(&mut bytes, ImageFormat::Png)?;
    Ok(format!(
        "/artifacts/{}",
        w.artifact(bytes.get_ref(), "png")?
    ))
}
fn rgb(p: &image::Rgba<u8>) -> [u8; 3] {
    [p[0], p[1], p[2]]
}
fn representative_color(
    w: &Workspace,
    m: &morphforge_core::materials::Material,
) -> Result<[u8; 3]> {
    let mut value = m.color;
    if let Some(texture) = m.textures.first() {
        let name = texture
            .strip_prefix("/artifacts/")
            .context("材质纹理必须为本机artifact")?;
        crate::store::validate_id(name)?;
        let path = w.root.join("artifacts").join(name);
        if std::fs::metadata(&path)?.len() > 8 * 1024 * 1024 {
            bail!("代表色纹理超过8MiB")
        }
        let bytes = std::fs::read(path)?;
        let reader = image::ImageReader::new(Cursor::new(&bytes)).with_guessed_format()?;
        let (width, height) = reader.into_dimensions()?;
        if width as u64 * height as u64 > 16_777_216 {
            bail!("代表色纹理超过16M像素")
        }
        let image = image::load_from_memory(&bytes)?.to_rgba8();
        let frame_height = image.height().min(image.width());
        let mut sum = [0u64; 3];
        let mut alpha = 0u64;
        for y in 0..frame_height {
            for x in 0..image.width() {
                let p = image.get_pixel(x, y);
                alpha += p[3] as u64;
                for a in 0..3 {
                    sum[a] += p[a] as u64 * p[3] as u64;
                }
            }
        }
        if alpha > 0 {
            value = std::array::from_fn(|a| m.color[a] * sum[a] as f32 / alpha as f32 / 255.);
        }
    }
    Ok(value.map(|v| (v.clamp(0., 1.) * 255.).round() as u8))
}
fn nearest(c: [u8; 3], palette: &[[u8; 3]]) -> [u8; 3] {
    *palette
        .iter()
        .min_by_key(|p| {
            (0..3)
                .map(|a| {
                    let d = c[a] as i32 - p[a] as i32;
                    d * d
                })
                .sum::<i32>()
        })
        .unwrap()
}
/// Weighted median cut: deterministic, and gradients do not collapse to the
/// first frequent/dark colors when every input pixel has the same frequency.
fn quantize(hist: &BTreeMap<[u8; 3], u64>, limit: usize) -> BTreeMap<[u8; 3], [u8; 3]> {
    if hist.len() <= limit {
        return hist.keys().map(|c| (*c, *c)).collect();
    }
    let mut buckets = vec![hist.iter().map(|(c, n)| (*c, *n)).collect::<Vec<_>>()];
    fn score(b: &[([u8; 3], u64)]) -> u64 {
        if b.len() < 2 {
            return 0;
        }
        let range = (0..3)
            .map(|a| {
                b.iter().map(|x| x.0[a]).max().unwrap() as u64
                    - b.iter().map(|x| x.0[a]).min().unwrap() as u64
            })
            .max()
            .unwrap();
        range * b.iter().map(|x| x.1).sum::<u64>()
    }
    while buckets.len() < limit {
        let index = (0..buckets.len())
            .max_by_key(|i| score(&buckets[*i]))
            .unwrap();
        if score(&buckets[index]) == 0 {
            break;
        }
        let mut b = buckets.swap_remove(index);
        let axis = (0..3)
            .max_by_key(|a| {
                b.iter().map(|x| x.0[*a]).max().unwrap() - b.iter().map(|x| x.0[*a]).min().unwrap()
            })
            .unwrap();
        b.sort_by_key(|x| (x.0[axis], x.0));
        let half = b.iter().map(|x| x.1).sum::<u64>().div_ceil(2);
        let mut sum = 0;
        let mut at = 1;
        for (i, x) in b.iter().enumerate() {
            sum += x.1;
            if sum >= half {
                at = (i + 1).min(b.len() - 1);
                break;
            }
        }
        let tail = b.split_off(at);
        buckets.push(b);
        buckets.push(tail);
    }
    let mut result = BTreeMap::new();
    for b in buckets {
        let n = b.iter().map(|x| x.1).sum::<u64>();
        let mean = std::array::from_fn(|a| {
            ((b.iter().map(|x| x.0[a] as u64 * x.1).sum::<u64>() + n / 2) / n) as u8
        });
        for (c, _) in b {
            result.insert(c, mean);
        }
    }
    result
}
pub fn action(w: &mut Workspace, args: &Value, run: Option<&str>) -> Result<Value> {
    let r: Request = serde_json::from_value(args.clone())?;
    if r.action == "list" {
        return references::list(w);
    }
    if !["inspect", "resize", "voxel_plane"].contains(&r.action.as_str()) {
        bail!("action需要list/inspect/resize/voxel_plane")
    }
    let (mut im, id) = pixels(w, &r)?;
    let (width, height) = im.dimensions();
    if r.action == "inspect" {
        let points = r.points.unwrap_or_default();
        if points.len() > 256 {
            bail!("最多读取256个采样点")
        }
        let mut sampled = vec![];
        for [x, y] in points {
            if x >= width || y >= height {
                bail!("像素坐标超界，原点在左上角")
            };
            sampled.push(json!({"x":x,"y":y,"rgba":im.get_pixel(x,y).0}));
        }
        let [gx, gy] = r.grid.unwrap_or([8, 8]);
        if gx == 0 || gy == 0 || gx > 64 || gy > 64 {
            bail!("采样grid每边1..64")
        }
        let mut colors = Vec::new();
        let mut index = BTreeMap::new();
        let mut rows = vec![];
        for y in 0..gy {
            let mut row = vec![];
            for x in 0..gx {
                let px = (((2 * x + 1) as u64 * width as u64) / (2 * gx) as u64)
                    .min(width as u64 - 1) as u32;
                let py = (((2 * y + 1) as u64 * height as u64) / (2 * gy) as u64)
                    .min(height as u64 - 1) as u32;
                let c = im.get_pixel(px, py).0;
                let next = colors.len();
                let i = *index.entry(c).or_insert_with(|| {
                    colors.push(c);
                    next
                });
                row.push(i)
            }
            rows.push(row)
        }
        return Ok(
            json!({"ok":true,"imageId":id,"width":width,"height":height,"coordinateSpace":"processed image pixels, top-left origin, x right/y down; RGBA8 sRGB","points":sampled,"grid":{"size":[gx,gy],"sampling":"nearest pixel at cell center","paletteRgba":colors,"indices":rows}}),
        );
    }
    if r.action == "resize" {
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(im).write_to(&mut bytes, ImageFormat::Png)?;
        let prepared = references::prepare(bytes.get_ref())?;
        let mut result =
            references::derive(w, &id, r.title.as_deref().unwrap_or("缩放参考图"), prepared)?;
        result["imageRole"] = json!("reinspection");
        result["images"] =
            json!([{"view":"resized-reference","artifact":result["artifact"],"url":result["url"]}]);
        return Ok(result);
    }
    let base = r.base_revision.context("voxel_plane需要baseRevision")?;
    if base != w.project.scene.revision {
        bail!("REVISION_CONFLICT")
    }
    let max_colors = r.max_colors.unwrap_or(256);
    if !(1..=4096).contains(&max_colors) {
        bail!("maxColors范围1..4096")
    }
    let threshold = r.alpha_threshold.unwrap_or(16);
    if threshold == 0 {
        bail!("alphaThreshold范围1..255")
    }
    let mut hist = BTreeMap::new();
    for p in im.pixels() {
        if p[3] >= threshold {
            *hist.entry(rgb(p)).or_insert(0u64) += 1;
        }
    }
    if hist.is_empty() {
        bail!("阈值内没有不透明像素；不会擦除当前作品")
    }
    let quant = quantize(&hist, max_colors);
    let mut materials = w.project.materials.clone();
    let mut used: BTreeSet<_> = materials.iter().map(|m| m.id).collect();
    let mut palette = BTreeMap::new();
    let mut new_colors = 0;
    let selected = if let Some(ids) = r.material_ids {
        if ids.is_empty() || ids.len() > 256 {
            bail!("materialIds须含1..256种材质")
        };
        let mut values = vec![];
        for id in ids {
            values.push(
                materials
                    .iter()
                    .find(|m| m.id == id)
                    .cloned()
                    .context("未知materialId")?,
            )
        }
        Some(values)
    } else {
        None
    };
    let selected_colors = selected
        .as_ref()
        .map(|values| {
            values
                .iter()
                .map(|m| representative_color(w, m))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;
    for c in quant.values().copied().collect::<BTreeSet<_>>() {
        let style = if let Some(values) = &selected {
            let colors = selected_colors.as_ref().unwrap();
            let near = nearest(c, colors);
            values[colors.iter().position(|v| *v == near).unwrap()].id
        } else {
            let color = c.map(|v| v as f32 / 255.);
            if let Some(m) = materials.iter().find(|m| {
                m.color == color
                    && m.textures.is_empty()
                    && m.procedural.is_none()
                    && m.metallic == 0.
                    && m.opacity == 1.
                    && m.emission == 0.
                    && (m.roughness - 0.9).abs() < 1e-4
            }) {
                m.id
            } else {
                if materials.len() >= 16384 {
                    bail!("工程材质超过16384，请减少maxColors或复用materialIds")
                }
                let id = (25..=65535)
                    .find(|id| !used.contains(id))
                    .context("材质ID空间已满")?;
                used.insert(id);
                let mut m = morphforge_core::materials::builtins().remove(0);
                m.id = id;
                m.uid = format!("mf:runtime-image-{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
                m.name = format!("图像 #{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
                m.family = "matte".into();
                m.color = color;
                m.source = Some("MorphForge image plane/1".into());
                materials.push(m);
                new_colors += 1;
                id
            }
        };
        palette.insert(c, style);
    }
    let plane = r.plane.as_deref().unwrap_or("xy");
    let (u, v, n) = match plane {
        "xy" => (0, 1, 2),
        "xz" => (0, 2, 1),
        "yz" => (2, 1, 0),
        _ => bail!("plane需要xy/xz/yz"),
    };
    let origin = r.origin.unwrap_or_else(|| {
        let mut p = [0; 3];
        p[u] = -(width as i32) / 2;
        if v != 1 {
            p[v] = -(height as i32) / 2;
        }
        p
    });
    let thickness = r.thickness.unwrap_or(1);
    if !(1..=64).contains(&thickness) {
        bail!("thickness范围1..64")
    }
    let group = r.group.unwrap_or_else(|| "image-plane".into());
    if group.is_empty() || group.len() > 128 {
        bail!("group长度1..128字节")
    }
    let mut extent = [1; 3];
    extent[u] = width;
    extent[v] = height;
    extent[n] = thickness;
    Box3::new(origin, extent, 1).validate(&w.project.limits)?;
    let mut boxes = vec![];
    for y in 0..height {
        let mut x = 0;
        while x < width {
            let p = im.get_pixel(x, y);
            if p[3] < threshold {
                x += 1;
                continue;
            }
            let c = quant[&rgb(p)];
            let style = palette[&c];
            let start = x;
            x += 1;
            while x < width {
                let p = im.get_pixel(x, y);
                if p[3] < threshold || palette[&quant[&rgb(p)]] != style {
                    break;
                }
                x += 1
            }
            let mut min = origin;
            min[u] += start as i32;
            min[v] += (height - 1 - y) as i32;
            let mut size = [1; 3];
            size[u] = x - start;
            size[n] = thickness;
            let mut b = Box3::new(min, size, style);
            b.group = group.clone();
            boxes.push(b);
        }
    }
    let canvas_mode = r.canvas_mode.as_deref().unwrap_or("overlay");
    if !["overlay", "replace"].contains(&canvas_mode) {
        bail!("canvasMode需要overlay/replace");
    }
    let mut generated = morphforge_core::merge(boxes);
    let generated_count = generated.len();
    let scene = if canvas_mode == "overlay" {
        let ops = generated
            .into_iter()
            .map(|value| Operation::Box { value })
            .collect::<Vec<_>>();
        w.project.scene.apply(&ops, &w.project.limits)?
    } else {
        let mut scene = w.project.scene.apply(
            &[Operation::Erase {
                min: origin,
                size: extent,
            }],
            &w.project.limits,
        )?;
        let mut next_id = scene.boxes.iter().map(|b| b.id).max().unwrap_or(0) + 1;
        for b in &mut generated {
            b.id = next_id;
            next_id += 1;
        }
        scene.boxes.extend(generated);
        scene
    };
    scene.validate(&w.project.limits)?;
    w.ensure_protected_groups_unchanged(&w.project.scene, &scene)?;
    for p in im.pixels_mut() {
        if p[3] >= threshold {
            let c = quant[&rgb(p)];
            let style = palette[&c];
            let color = if let Some(values) = &selected {
                selected_colors.as_ref().unwrap()
                    [values.iter().position(|m| m.id == style).unwrap()]
            } else {
                c
            };
            p.0 = [color[0], color[1], color[2], 255]
        } else {
            p.0 = [0; 4]
        }
    }
    let preview_url = artifact(w, &im)?;
    let target = crate::scene_tools::target_report(&scene, w.project.limits.target_boxes);
    let dry = r.dry_run.unwrap_or(true);
    let mut revision = base;
    if !dry {
        let record = references::record(w, &id)?;
        let credit = json!({"kind":"reference-image-plane","imageId":id,"title":record.title,"author":record.author,"license":record.license,"sourceUrl":record.source_url,"artifact":record.artifact});
        let previous = w.project.source_credits.clone();
        if !w.project.source_credits.contains(&credit) {
            w.project.source_credits.push(credit)
        }
        match w.replace_with_materials(scene, materials, Some(base), run) {
            Ok(result) => revision = result["revision"].as_u64().unwrap(),
            Err(e) => {
                w.project.source_credits = previous;
                return Err(e);
            }
        }
    }
    let result = json!({"ok":true,"action":"voxel_plane","dryRun":dry,"revision":revision,"imageId":id,"size":[width,height],"plane":plane,"origin":origin,"thickness":thickness,"generatedBoxes":generated_count,"paletteColors":palette.values().collect::<BTreeSet<_>>().len(),"materialsAdded":new_colors,"previewUrl":preview_url,"imageRole":"reinspection","images":[{"view":"pixel-plane-preview","artifact":preview_url.trim_start_matches("/artifacts/"),"url":preview_url}],"target":target,"alphaMode":"threshold cutout then opaque; no texture upload","orientation":"image top row maps to positive v, left column maps to minimum u","canvasMode":canvas_mode,"canvasReplacesRegion":canvas_mode=="replace","note":"图片本身保留；像素平面不是三维重建。MF5可直接发送新RGB；其他Avatar需要烘焙目录。"});
    if !dry {
        w.event("image.voxel_plane", run, result.clone())?;
    }
    Ok(result)
}
