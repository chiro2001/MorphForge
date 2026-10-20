//! Reference images are explicit visual inputs, separate from scene captures.
use crate::store::{validate_id, write_json, Workspace};
use anyhow::{bail, Context, Result};
use image::{imageops::FilterType, DynamicImage, ImageFormat, ImageReader};
use include_dir::{include_dir, Dir};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{io::Cursor, path::Path};

static BUILTIN: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../references");
pub const MAX_UPLOAD_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_SELECTED: usize = 6;
const MAX_SELECTED_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceImage {
    pub id: String,
    pub title: String,
    pub category: String,
    pub origin: String,
    pub author: String,
    pub source_url: String,
    pub license: String,
    pub license_url: String,
    pub notes: String,
    pub width: u32,
    pub height: u32,
    pub artifact: String,
    pub thumbnail: String,
}
impl ReferenceImage {
    fn public(&self) -> Value {
        let mut value = serde_json::to_value(self).unwrap();
        value["url"] = json!(format!("/api/references/{}/image", self.id));
        value["previewUrl"] = json!(format!("/api/references/{}/thumbnail", self.id));
        value
    }
}

pub fn builtin() -> Result<Vec<ReferenceImage>> {
    Ok(serde_json::from_slice(
        BUILTIN
            .get_file("manifest.json")
            .context("内置参考图库清单缺失")?
            .contents(),
    )?)
}
fn local(root: &Path) -> Result<Vec<ReferenceImage>> {
    let path = root.join("references/library.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    serde_json::from_slice(&std::fs::read(path)?).context("本机参考图库清单无效")
}
pub fn list(workspace: &Workspace) -> Result<Value> {
    let mut items = builtin()?;
    items.extend(local(&workspace.root)?);
    Ok(
        json!({"images":items.iter().map(ReferenceImage::public).collect::<Vec<_>>(),"maxSelected":MAX_SELECTED,
        "sources":[
            {"title":"本机收藏","url":"","note":"可用添加图片导入自己的素材；不会自动扫描或上传原始目录。"},
            {"title":"蓝色大肥鱼档案馆","url":"https://github.com/EDMOK/blue-fish-archive","note":"可查找梗图出处；图库收录不等于逐图授权。"},
            {"title":"鲸鱼娘角色素材与三视图","url":"https://github.com/TreapGoGo/deepseek-whale-girl","note":"角色设定图不属于网站代码MIT许可，按每图来源使用。"}
        ]}),
    )
}
fn lookup(workspace: &Workspace, id: &str) -> Result<ReferenceImage> {
    let items = if id.starts_with("builtin:") {
        builtin()?
    } else {
        local(&workspace.root)?
    };
    items
        .into_iter()
        .find(|item| item.id == id)
        .context("参考图片不存在，请重新选择")
}
pub fn record(workspace: &Workspace, id: &str) -> Result<ReferenceImage> {
    lookup(workspace, id)
}
pub fn derive(w: &mut Workspace, id: &str, title: &str, prepared: PreparedImage) -> Result<Value> {
    let parent = lookup(w, id)?;
    let old = local(&w.root)?;
    let mut result = import(w, title, prepared)?;
    let child_id = result["id"].as_str().unwrap().to_owned();
    if !old.iter().any(|r| r.id == child_id) {
        let mut records = local(&w.root)?;
        let child = records.iter_mut().find(|r| r.id == child_id).unwrap();
        child.author = parent.author.clone();
        child.source_url = parent.source_url.clone();
        child.license = parent.license.clone();
        child.license_url = parent.license_url.clone();
        child.notes = format!("派生自 {} ({})；{}", parent.title, parent.id, parent.notes);
        result = child.public();
        write_json(&w.root.join("references/library.json"), &records)?;
    }
    result["parentImageId"] = json!(id);
    w.event("reference.derived", None, result.clone())?;
    Ok(result)
}
pub fn image(workspace: &Workspace, id: &str, thumbnail: bool) -> Result<(Vec<u8>, &'static str)> {
    let record = lookup(workspace, id)?;
    let filename = if thumbnail {
        &record.thumbnail
    } else {
        &record.artifact
    };
    let bytes = if record.origin == "builtin" {
        BUILTIN
            .get_file(filename)
            .context("内置参考图片缺失")?
            .contents()
            .to_vec()
    } else {
        validate_id(filename)?;
        std::fs::read(workspace.artifact_path(filename))?
    };
    Ok((bytes, if thumbnail { "image/webp" } else { "image/png" }))
}

pub fn ids(value: &Value) -> Result<Vec<String>> {
    let Some(value) = value.get("referenceIds") else {
        return Ok(Vec::new());
    };
    let ids = value.as_array().context("referenceIds须为数组")?;
    if ids.len() > MAX_SELECTED {
        bail!("最多选择{MAX_SELECTED}张参考图片")
    }
    let mut result = Vec::new();
    for id in ids {
        let id = id
            .as_str()
            .filter(|id| id.len() <= 128)
            .context("参考图片ID无效")?;
        if result.iter().any(|previous| previous == id) {
            bail!("不能重复选择同一张参考图片")
        }
        result.push(id.into());
    }
    Ok(result)
}
/// Snapshot selected files into immutable artifacts before acknowledging a message.
pub fn resolve(workspace: &mut Workspace, ids: &[String]) -> Result<Vec<ReferenceImage>> {
    let mut total = 0u64;
    let mut result = Vec::new();
    for id in ids {
        let mut record = lookup(workspace, id)?;
        let (bytes, _) = image(workspace, id, false)?;
        total = total.saturating_add(bytes.len() as u64);
        if total > MAX_SELECTED_BYTES {
            bail!("所选参考图片合计超过16MiB，请减少图片")
        }
        record.artifact = workspace.artifact(&bytes, "png")?;
        if record.origin == "builtin" {
            let (bytes, _) = image(workspace, id, true)?;
            record.thumbnail = workspace.artifact(&bytes, "webp")?;
        }
        result.push(record);
    }
    Ok(result)
}

pub struct PreparedImage {
    png: Vec<u8>,
    thumbnail: Vec<u8>,
    width: u32,
    height: u32,
}
pub fn prepare(bytes: &[u8]) -> Result<PreparedImage> {
    if bytes.len() > MAX_UPLOAD_BYTES {
        bail!("图片超过8MiB，请先缩小")
    }
    let format = image::guess_format(bytes).context("无法识别图片格式")?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP | ImageFormat::Gif
    ) {
        bail!("仅支持PNG、JPEG、WebP或GIF图片")
    }
    let reader = || ImageReader::with_format(Cursor::new(bytes), format);
    let (width, height) = reader().into_dimensions().context("图片尺寸读取失败")?;
    if width == 0
        || height == 0
        || width > 8192
        || height > 8192
        || width as u64 * height as u64 > 16_777_216
    {
        bail!("图片尺寸超过8192边长或1600万像素，请先缩小")
    }
    let mut decoder = reader().into_decoder().context("图片解码失败")?;
    use image::ImageDecoder;
    let orientation = decoder.orientation()?;
    let mut image = DynamicImage::from_decoder(decoder).context("图片解码失败")?;
    image.apply_orientation(orientation);
    if image.width() > 1024 || image.height() > 1024 {
        image = image.resize(1024, 1024, FilterType::Lanczos3);
    }
    image = DynamicImage::ImageRgba8(image.to_rgba8());
    let mut png = Cursor::new(Vec::new());
    image.write_to(&mut png, ImageFormat::Png)?;
    let mut thumbnail = Cursor::new(Vec::new());
    image
        .thumbnail(224, 224)
        .write_to(&mut thumbnail, ImageFormat::WebP)?;
    Ok(PreparedImage {
        png: png.into_inner(),
        thumbnail: thumbnail.into_inner(),
        width: image.width(),
        height: image.height(),
    })
}
pub fn import(workspace: &mut Workspace, title: &str, image: PreparedImage) -> Result<Value> {
    let artifact = workspace.artifact(&image.png, "png")?;
    let mut records = local(&workspace.root)?;
    if let Some(record) = records.iter().find(|record| record.artifact == artifact) {
        return Ok(record.public());
    }
    if records.len() >= 4096 {
        bail!("本机图库已达4096张，请先整理图库")
    }
    let thumbnail = workspace.artifact(&image.thumbnail, "webp")?;
    let title = title
        .chars()
        .filter(|c| !c.is_control())
        .take(120)
        .collect::<String>();
    let record = ReferenceImage {
        id: format!("local:{}", uuid::Uuid::new_v4()),
        title: if title.trim().is_empty() {
            "上传参考图".into()
        } else {
            title
        },
        category: "我的图片".into(),
        origin: "local".into(),
        author: "本机导入".into(),
        source_url: String::new(),
        license: "用户提供，按原素材许可使用".into(),
        license_url: String::new(),
        notes:
            "上传原图已等比缩小至1024边长，去除EXIF；动图仅使用首帧。只在发送选中参考时交给上游。"
                .into(),
        width: image.width,
        height: image.height,
        artifact,
        thumbnail,
    };
    records.push(record.clone());
    write_json(&workspace.root.join("references/library.json"), &records)?;
    workspace.event("reference.imported", None, json!(&record))?;
    Ok(record.public())
}

pub fn remove(workspace: &mut Workspace, id: &str) -> Result<Value> {
    if !id.starts_with("local:") {
        bail!("内置参考图请取消选择，不可移出内置图库")
    }
    let mut records = local(&workspace.root)?;
    let before = records.len();
    records.retain(|record| record.id != id);
    if records.len() == before {
        bail!("参考图片不存在")
    }
    write_json(&workspace.root.join("references/library.json"), &records)?;
    workspace.event(
        "reference.removed",
        None,
        json!({"id":id,"historicalArtifactsRetained":true}),
    )?;
    Ok(json!({"ok":true,"id":id}))
}

pub(crate) fn artwork_records(w: &Workspace, extra: &Value) -> Result<Value> {
    let mut wanted = w
        .project
        .creative
        .bookmarks
        .values()
        .filter_map(|b| b.comparison.as_ref().map(|c| c.image_id.clone()))
        .collect::<std::collections::BTreeSet<_>>();
    for record in extra.as_array().into_iter().flatten() {
        if let Some(id) = record["id"].as_str().or(record["imageId"].as_str()) {
            wanted.insert(id.into());
        }
    }
    Ok(json!(local(&w.root)?
        .into_iter()
        .filter(|r| wanted.contains(&r.id))
        .collect::<Vec<_>>()))
}
pub(crate) fn restore_artwork_records(w: &Workspace, value: &Value) -> Result<()> {
    if value.is_null() {
        return Ok(());
    }
    let additions: Vec<ReferenceImage> = serde_json::from_value(value.clone())?;
    if additions.len() > 256 {
        bail!("作品参考图记录超过256项")
    }
    let mut records = local(&w.root)?;
    let mut changed = false;
    for record in additions {
        if !record.id.starts_with("local:") || uuid::Uuid::parse_str(&record.id[6..]).is_err() {
            bail!("作品参考图ID无效")
        }
        for name in [&record.artifact, &record.thumbnail] {
            validate_id(name)?;
            if !w.artifact_path(name).is_file() {
                bail!("作品参考图文件缺失")
            }
        }
        if let Some(old) = records.iter().find(|r| r.id == record.id) {
            if old.artifact != record.artifact {
                bail!("作品参考图ID内容冲突")
            };
            continue;
        }
        if records.len() >= 4096 {
            bail!("本机图库已满")
        }
        records.push(record);
        changed = true;
    }
    if changed {
        write_json(&w.root.join("references/library.json"), &records)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn builtin_entries_have_images_credits_and_fixed_dimensions() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(dir.path().into()).unwrap();
        let items = builtin().unwrap();
        assert!(items.len() >= 8);
        for item in items {
            assert!(!item.author.is_empty() && !item.license.is_empty());
            let (bytes, _) = image(&workspace, &item.id, false).unwrap();
            let decoded = image::load_from_memory(&bytes).unwrap();
            assert_eq!(
                (decoded.width(), decoded.height()),
                (item.width, item.height)
            );
            assert!(decoded.width() <= 1024 && decoded.height() <= 1024);
            assert!(image(&workspace, &item.id, true).unwrap().0.len() > 20);
        }
    }
    #[test]
    fn imports_are_normalized_deduplicated_and_removal_keeps_queued_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().into()).unwrap();
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::new_rgb8(1400, 700)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        let a = import(&mut w, "test.png", prepare(bytes.get_ref()).unwrap()).unwrap();
        let b = import(&mut w, "different.png", prepare(bytes.get_ref()).unwrap()).unwrap();
        assert_eq!(a["id"], b["id"]);
        assert_eq!(a["width"], 1024);
        assert_eq!(a["height"], 512);
        let id = a["id"].as_str().unwrap();
        let frozen = resolve(&mut w, &[id.into()]).unwrap();
        remove(&mut w, id).unwrap();
        assert!(image(&w, id, false).is_err());
        assert!(w.root.join("artifacts").join(&frozen[0].artifact).is_file());
    }
    #[test]
    fn rejects_invalid_images_and_selection_limits() {
        assert!(prepare(b"not an image").is_err());
        assert!(prepare(&vec![0; MAX_UPLOAD_BYTES + 1]).is_err());
        assert!(ids(&json!({"referenceIds":["x","x"]})).is_err());
        assert!(ids(&json!({"referenceIds":vec!["x";MAX_SELECTED+1]})).is_err());
    }
}
