use super::{
    compile::{self, Compiled, Options},
    formats::Building,
};
use crate::store::{self, Workspace};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
fn id(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        bail!("无效的建筑内容ID");
    }
    Ok(())
}
fn directory(root: &Path, source: &str) -> Result<PathBuf> {
    id(source)?;
    Ok(root.join("imports").join(source))
}
fn compressed<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    serde_json::to_writer(&mut encoder, value)?;
    Ok(encoder.finish()?)
}
fn decompress(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut data)?;
    if data.len() > 256 * 1024 * 1024 {
        bail!("建筑缓存超过256MiB保护值");
    }
    Ok(data)
}
pub fn register(w: &mut Workspace, bytes: &[u8], name: &str, building: &Building) -> Result<Value> {
    if name.len() > 512 || bytes.len() > 64 * 1024 * 1024 {
        bail!("建筑文件名或文件体积超限");
    }
    let source = store::hash(bytes);
    let dir = directory(&w.root, &source)?;
    fs::create_dir_all(&dir)?;
    let cache = compressed(building)?;
    store::atomic(&dir.join("source.bin"), bytes)?;
    store::atomic(&dir.join("building.json.gz"), &cache)?;
    let summary = json!({"id":source,"name":name,"format":building.format,"dataVersion":building.data_version,"sourceBlocks":building.blocks.len(),"sourceCells":building.source_cells,"paletteCount":building.palette.len(),"regions":building.regions,"bounds":building.bounds,"entities":building.entities,"blockEntities":building.block_entities,"warnings":building.warnings,"bytes":bytes.len(),"createdAt":store::now(),"sourceHash":source,"cacheHash":store::hash(&cache),"projectId":w.project.id});
    store::write_json(&dir.join("source.json"), &summary)?;
    w.event("minecraft.source.imported", None, summary.clone())?;
    Ok(json!({"ok":true,"source":summary}))
}
pub fn list(root: &Path) -> Result<Value> {
    let path = root.join("imports");
    let mut sources = Vec::new();
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let metadata = entry.path().join("source.json");
            if metadata.is_file() {
                let bytes = fs::read(metadata)?;
                if bytes.len() > 1024 * 1024 {
                    bail!("建筑源记录异常过大");
                }
                sources.push(serde_json::from_slice::<Value>(&bytes)?);
            }
        }
    }
    sources.sort_by(|a, b| b["createdAt"].as_str().cmp(&a["createdAt"].as_str()));
    Ok(json!({"ok":true,"sources":sources}))
}
pub fn load(root: &Path, source: &str) -> Result<(Value, Building)> {
    let dir = directory(root, source)?;
    let summary: Value = serde_json::from_slice(
        &fs::read(dir.join("source.json")).context("建筑源不存在，请先上传")?,
    )?;
    let original = fs::read(dir.join("source.bin"))?;
    let cache = fs::read(dir.join("building.json.gz"))?;
    if store::hash(&original) != source || summary["cacheHash"] != store::hash(&cache) {
        bail!("MC_SOURCE_CORRUPT：建筑源或解析缓存损坏，请重新导入同一文件；工程未改变");
    }
    let building: Building = serde_json::from_slice(&decompress(&cache)?)?;
    Ok((summary, building))
}
pub fn inspect(root: &Path, args: &Value) -> Result<Value> {
    let source = args["sourceId"].as_str().context("需要sourceId")?;
    let (summary, building) = load(root, source)?;
    let offset = args["offset"].as_u64().unwrap_or(0).min(usize::MAX as u64) as usize;
    let limit = args["limit"].as_u64().unwrap_or(32).clamp(1, 128) as usize;
    let mut counts = vec![0usize; building.palette.len()];
    for block in &building.blocks {
        counts[block.state] += 1;
    }
    Ok(
        json!({"ok":true,"source":summary,"palette":building.palette.iter().enumerate().skip(offset).take(limit).map(|(i,s)|json!({"index":i,"state":s.canonical(),"blocks":counts[i]})).collect::<Vec<_>>(),"nextOffset":if offset.saturating_add(limit)<building.palette.len(){Some(offset+limit)}else{None},"sampleBlocks":building.blocks.iter().take(8).collect::<Vec<_>>(),"originalPreserved":true}),
    )
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub source_id: String,
    pub project_id: String,
    pub base_revision: u64,
    pub base_scene_hash: String,
    pub base_material_hash: String,
    pub compiled: Compiled,
}
fn preview_path(root: &Path, preview: &str) -> Result<PathBuf> {
    id(preview)?;
    Ok(root
        .join("imports/previews")
        .join(format!("{preview}.json.gz")))
}
pub fn load_preview(root: &Path, preview: &str) -> Result<Preview> {
    let bytes = fs::read(preview_path(root, preview)?).context("建筑预览不存在，请重新预览")?;
    let data = decompress(&bytes)?;
    if store::hash(&data) != preview {
        bail!("MC_PREVIEW_CORRUPT：预览缓存校验失败");
    }
    Ok(serde_json::from_slice(&data)?)
}
pub async fn preview(
    workspace: Arc<Mutex<Workspace>>,
    args: Value,
    run: Option<String>,
) -> Result<Value> {
    let source = args["sourceId"]
        .as_str()
        .context("需要sourceId")?
        .to_owned();
    let options: Options =
        serde_json::from_value(args.get("options").cloned().unwrap_or_else(|| json!({})))?;
    let (root, project_id, scene, limits, materials, original_material_count, material_hash) = {
        let mut w = workspace.lock().unwrap();
        if args["baseRevision"]
            .as_u64()
            .is_some_and(|r| r != w.project.scene.revision)
        {
            bail!("REVISION_CONFLICT：工程已改变，请刷新后重试");
        }
        if args["projectId"]
            .as_str()
            .is_some_and(|id| id != w.project.id)
        {
            bail!("WORKSPACE_CONFLICT：工程已切换");
        }
        let originals = w.project.materials.clone();
        let material_hash = store::hash(&serde_json::to_vec(&originals)?);
        let (materials, _, _) =
            crate::builtin_pixels::prepare_theme(&mut w, "block-workshop", &originals)?;
        (
            w.root.clone(),
            w.project.id.clone(),
            w.project.scene.clone(),
            w.project.limits.clone(),
            materials,
            originals.len(),
            material_hash,
        )
    };
    let base = scene.revision;
    let scene_hash = store::hash(&serde_json::to_vec(&scene)?);
    let worker_root = root.clone();
    let worker_source = source.clone();
    let compiled = tokio::task::spawn_blocking(move || {
        let (_, building) = load(&worker_root, &worker_source)?;
        compile::compile_prepared(
            &scene,
            &limits,
            materials,
            original_material_count,
            &building,
            options,
        )
    })
    .await??;
    let preview = Preview {
        source_id: source.clone(),
        project_id: project_id.clone(),
        base_revision: base,
        base_scene_hash: scene_hash,
        base_material_hash: material_hash,
        compiled,
    };
    let data = serde_json::to_vec(&preview)?;
    if data.len() > 256 * 1024 * 1024 {
        bail!("建筑预览超过256MiB保护值");
    }
    let preview_id = store::hash(&data);
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(&data)?;
    store::atomic(&preview_path(&root, &preview_id)?, &encoder.finish()?)?;
    let response = json!({"ok":true,"previewId":preview_id,"sourceId":source,"projectId":project_id,"baseRevision":base,"report":preview.compiled.report,"sceneUrl":format!("/api/minecraft/preview/{preview_id}"),"canApply":!preview.compiled.scene.boxes.is_empty()||!preview.compiled.scene.triangles.is_empty(),"draftChanged":false});
    let mut w = workspace.lock().unwrap();
    if w.project.id == project_id {
        w.event("minecraft.previewed", run.as_deref(), response.clone())?;
    }
    Ok(response)
}
pub fn apply(w: &mut Workspace, args: &Value, run: Option<&str>) -> Result<Value> {
    let preview_id = args["previewId"]
        .as_str()
        .context("应用建筑需要previewId")?;
    id(preview_id)?;
    let request = args["requestId"].as_str().unwrap_or(preview_id);
    if request.is_empty() || request.len() > 256 {
        bail!("无效的导入requestId");
    }
    if let Some(receipt) = w.project.import_receipts.get(request) {
        if receipt["previewId"] != preview_id {
            bail!("REQUEST_ID_CONFLICT：导入请求ID已对应其它预览");
        }
        let mut receipt = receipt.clone();
        receipt["replayed"] = json!(true);
        return Ok(receipt);
    }
    let preview = load_preview(&w.root, preview_id)?;
    let base = args["baseRevision"]
        .as_u64()
        .context("应用建筑需要baseRevision")?;
    if w.project.id != preview.project_id
        || args["projectId"]
            .as_str()
            .is_some_and(|id| id != w.project.id)
    {
        bail!("WORKSPACE_CONFLICT：预览属于其它工程");
    }
    if base != w.project.scene.revision
        || base != preview.base_revision
        || store::hash(&serde_json::to_vec(&w.project.scene)?) != preview.base_scene_hash
        || store::hash(&serde_json::to_vec(&w.project.materials)?) != preview.base_material_hash
    {
        bail!("REVISION_CONFLICT：建筑预览已过期，请重新预览");
    }
    if preview.compiled.scene.boxes.is_empty()
        && preview.compiled.scene.triangles.is_empty()
        && args["allowEmpty"] != true
    {
        bail!("建筑预览为空；如需显式清空请提供allowEmpty=true");
    }
    w.ensure_protected_groups_unchanged(&w.project.scene, &preview.compiled.scene)?;
    let previous = w.project.clone();
    let result = json!({"ok":true,"previewId":preview_id,"sourceId":preview.source_id,"revision":base+1,"report":preview.compiled.report,"originalPreserved":true});
    w.project
        .import_receipts
        .insert(request.into(), result.clone());
    if !w
        .project
        .source_credits
        .iter()
        .any(|v| v["minecraftSourceId"] == preview.source_id)
    {
        w.project.source_credits.push(json!({"kind":"minecraft_building","minecraftSourceId":preview.source_id,"format":preview.compiled.report["format"],"sourceBlocks":preview.compiled.report["sourceBlocks"],"originalPreserved":true,"options":preview.compiled.options}));
    }
    if let Err(error) = w.replace_with_materials(
        preview.compiled.scene,
        preview.compiled.materials,
        Some(base),
        run,
    ) {
        w.project = previous;
        return Err(error);
    }
    let mut result = result;
    if let Err(error) = w.event("minecraft.applied", run, result.clone()) {
        result["logWarning"] = json!(error.to_string());
    }
    Ok(result)
}
pub fn preview_scene(root: &Path, preview_id: &str) -> Result<Value> {
    let preview = load_preview(root, preview_id)?;
    Ok(
        json!({"ok":true,"preview":true,"previewId":preview_id,"baseRevision":preview.base_revision,"projectId":preview.project_id,"scene":preview.compiled.scene,"materials":preview.compiled.materials,"report":preview.compiled.report}),
    )
}
pub fn original(root: &Path, source: &str) -> Result<(Vec<u8>, String)> {
    let (metadata, _) = load(root, source)?;
    let extension = match metadata["format"].as_str().unwrap_or("") {
        "litematic" => "litematic",
        "mcedit_schematic" => "schematic",
        s if s.starts_with("sponge_") => "schem",
        _ => "nbt",
    };
    Ok((
        fs::read(directory(root, source)?.join("source.bin"))?,
        format!("morphforge-source-{}.{}", &source[..12], extension),
    ))
}
