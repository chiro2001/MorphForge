use crate::{
    capture::CaptureBroker,
    fonts::{self, FontCatalog, MissingGlyphsError},
    harness::Harness,
    provider::Provider,
    references,
    store::{validate_id, Workspace},
    transport::Transport,
};
use anyhow::{bail, Context, Result};
use axum::{
    extract::Request,
    extract::{DefaultBodyLimit, Multipart, Path as AxPath, Query, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use include_dir::{include_dir, Dir};
use morphforge_core::text::VoxelTextRequest;
use morphforge_core::{Box3, Operation, Scene};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
static WEB: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../web");
#[derive(Clone)]
pub struct App {
    pub workspace: Arc<Mutex<Workspace>>,
    pub transport: Transport,
    pub capture: CaptureBroker,
    pub harness: Harness,
    pub browser: Arc<Mutex<Option<u32>>>,
    pub fonts: Arc<FontCatalog>,
    workspace_gate: Arc<tokio::sync::RwLock<()>>,
    text_jobs: Arc<tokio::sync::Semaphore>,
    reference_jobs: Arc<tokio::sync::Semaphore>,
    minecraft_jobs: Arc<tokio::sync::Semaphore>,
    export_jobs: Arc<tokio::sync::Semaphore>,
}
pub struct ApiError(anyhow::Error);
impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let message = self.0.to_string();
        let status = if [
            "WORKSPACE_CONFLICT",
            "REVISION_CONFLICT",
            "RUN_CONFLICT",
            "REQUEST_ID_CONFLICT",
            "PREVIEW_CONFLICT",
            "PROTECTED_GROUP_CONFLICT",
        ]
        .iter()
        .any(|code| message.starts_with(code))
        {
            StatusCode::CONFLICT
        } else {
            StatusCode::BAD_REQUEST
        };
        let mut body = json!({"ok":false,"error":message});
        if let Some(error) = self.0.downcast_ref::<MissingGlyphsError>() {
            body["code"] = json!("FONT_MISSING_GLYPHS");
            body["missingGlyphs"] = json!(error
                .characters
                .iter()
                .map(char::to_string)
                .collect::<Vec<_>>());
        }
        (status, Json(body)).into_response()
    }
}
type ApiResult = std::result::Result<Json<Value>, ApiError>;
#[derive(Default, Deserialize)]
pub struct Revision {
    revision: Option<u64>,
    after: Option<u64>,
    run: Option<String>,
    brief: Option<bool>,
}
#[derive(Default, Deserialize)]
pub struct ToolQuery {
    language: Option<String>,
}
impl App {
    pub fn open(root: PathBuf, env: Option<PathBuf>) -> Result<Self> {
        crate::harness::lifecycle::recover(&root)?;
        let conversation_path = root.join("runs/conversation.json");
        if conversation_path.exists() {
            let _: crate::harness::Conversation =
                serde_json::from_slice(&std::fs::read(&conversation_path)?)
                    .context("对话数据损坏；未自动重置上下文")?;
        }
        let workspace = Arc::new(Mutex::new(Workspace::open(root.clone())?));
        let transport = Transport::new(workspace.clone());
        let capture = CaptureBroker::new();
        let provider = Provider::new(&root, env.as_deref())?;
        let fonts = fonts::shared_default()?;
        let harness = Harness::new_with_fonts(
            workspace.clone(),
            provider,
            capture.clone(),
            transport.clone(),
            fonts.clone(),
        );
        harness.initialize_workspace()?;
        Ok(Self {
            workspace_gate: Arc::new(tokio::sync::RwLock::new(())),
            workspace,
            transport,
            capture,
            harness,
            browser: Arc::new(Mutex::new(None)),
            fonts,
            text_jobs: Arc::new(tokio::sync::Semaphore::new(2)),
            reference_jobs: Arc::new(tokio::sync::Semaphore::new(2)),
            minecraft_jobs: Arc::new(tokio::sync::Semaphore::new(2)),
            export_jobs: Arc::new(tokio::sync::Semaphore::new(1)),
        })
    }
    pub fn router(&self) -> Router {
        Router::new()
            .route("/api/vrc/catalog", get(catalog))
            .route("/api/vrc/catalog/budget", get(catalog_budget))
            .route("/api/vrc/profile", post(avatar_profile))
            .route("/api/vrc/discover", post(avatar_discover))
            .route("/api/vrc/materials", post(avatar_materials))
            .route("/api/vrc/profile/import", post(avatar_profile_import))
            .route("/api/packs", get(packs))
            .route("/api/diagnostics", get(diagnostics))
            .route("/api/metrics", get(metrics))
            .route("/api/state", get(state))
            .route(
                "/api/view-settings",
                get(view_settings).post(view_settings_save),
            )
            .route("/api/scene", get(scene))
            .route("/api/text-panels", get(text_panels_list).post(text_panels_action))
            .route("/pixel-fonts/{font}/{kind}", get(pixel_font_asset))
            .route("/api/creative", get(creative_list))
            .route("/api/creative/{domain}", post(creative_action))
            .route("/api/creative/animation/{id}/export", get(animation_export))
            .route("/api/export", get(export))
            .route("/api/export/model", get(export_model))
            .route("/api/project", post(project))
            .route("/api/workspaces", get(workspaces).post(workspace_action))
            .route("/api/edit", post(edit))
            .route("/api/mesh", post(mesh_edit))
            .route(
                "/api/minecraft",
                get(minecraft_sources).post(minecraft_action),
            )
            .route("/api/minecraft/upload", post(minecraft_upload))
            .route("/api/minecraft/preview/{id}", get(minecraft_preview_scene))
            .route("/api/minecraft/source/{id}", get(minecraft_original))
            .route("/api/import", post(import))
            .route("/api/history", get(history))
            .route("/api/undo", post(undo))
            .route("/api/connect", post(connect))
            .route("/api/publish", post(publish))
            .route("/api/placement", post(placement))
            .route("/api/styles", get(styles))
            .route("/api/fonts", get(font_list))
            .route("/api/text/preview", post(text_preview))
            .route("/api/text/apply", post(text_apply))
            .route("/api/events", get(events))
            .route("/api/timeline", get(timeline_page))
            .route("/api/timeline/{seq}", get(timeline_detail))
            .route("/api/artworks", get(artworks).post(artwork_action))
            .route(
                "/api/artworks/{id}",
                get(artwork_get).delete(artwork_delete),
            )
            .route("/api/packs/import", post(pack_import))
            .route("/api/harness/tools", get(harness_tools))
            .route("/api/harness/state", get(harness_state))
            .route("/api/harness/config", post(harness_config))
            .route("/api/harness/start", post(harness_start))
            .route("/api/harness/message", post(harness_message))
            .route("/api/harness/control", post(harness_control))
            .route("/api/harness/tool", post(harness_tool))
            .route("/api/provider/models", get(models))
            .route("/api/references", get(reference_list))
            .route("/api/references/remove", post(reference_remove))
            .route("/api/references/upload", post(reference_upload))
            .route("/api/references/{id}/image", get(reference_image))
            .route("/api/references/{id}/thumbnail", get(reference_thumbnail))
            .route("/api/provider/config", post(provider_config))
            .route("/api/capture/jobs", get(capture_jobs))
            .route("/api/capture/result", post(capture_result))
            .route("/artifacts/{id}", get(artifact))
            .fallback(get(asset))
            .layer(DefaultBodyLimit::max(70 * 1024 * 1024))
            .layer(middleware::from_fn_with_state(self.clone(), workspace_gate))
            .layer(middleware::from_fn(origin_guard))
            .with_state(self.clone())
    }
}
async fn workspace_gate(State(a): State<App>, request: Request, next: Next) -> Response {
    if request.method()==axum::http::Method::POST && matches!(request.uri().path(),"/api/edit"|"/api/mesh"|"/api/import"|"/api/undo"|"/api/text/apply") {
        if let Err(error)=a.harness.ensure_manual_edit(){return ApiError(error).into_response();}
    }
    if request.method() == axum::http::Method::POST
        && ["/api/workspaces", "/api/harness/control"].contains(&request.uri().path())
    {
        let _guard = a.workspace_gate.write().await;
        next.run(request).await
    } else if request.method() == axum::http::Method::POST
        && request.uri().path() != "/api/capture/result"
    {
        let _guard = a.workspace_gate.read().await;
        next.run(request).await
    } else {
        next.run(request).await
    }
}
async fn creative_list(State(a): State<App>) -> ApiResult {
    Ok(Json(a.workspace.lock().unwrap().project.creative.summary()))
}
async fn creative_action(
    State(a): State<App>,
    AxPath(domain): AxPath<String>,
    Json(v): Json<Value>,
) -> ApiResult {
    let tool = match domain.as_str() {
        "references" => "reference_workbench",
        "components" => "components",
        "animation" => "animation",
        _ => return Err(anyhow::anyhow!("未知创作功能").into()),
    };
    Ok(Json(a.harness.tool(tool, &v, None).await?))
}
async fn animation_export(
    State(a): State<App>,
    AxPath(id): AxPath<String>,
) -> Result<Response, ApiError> {
    let _permit = a.export_jobs.acquire().await?;
    let (scene, materials, root, clip) = {
        let w = a.workspace.lock().unwrap();
        (
            w.project.scene.clone(),
            w.project.materials.clone(),
            w.root.clone(),
            w.project
                .creative
                .clips
                .get(&id)
                .cloned()
                .context("动画不存在")?,
        )
    };
    let bytes = tokio::task::spawn_blocking(move || {
        crate::animation_export::export(&scene, &materials, &root, &clip)
    })
    .await??;
    Ok((
        [
            ("Content-Type", "model/gltf-binary"),
            ("Content-Disposition", "attachment; filename=animation.glb"),
        ],
        bytes,
    )
        .into_response())
}
async fn view_settings(State(a): State<App>) -> ApiResult {
    Ok(Json(crate::view_settings::load(
        &a.workspace.lock().unwrap().root,
    )?))
}
async fn view_settings_save(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    let mut w = a.workspace.lock().unwrap();
    let settings = crate::view_settings::patch(&crate::view_settings::load(&w.root)?, &v)?;
    crate::store::write_json(&w.root.join("view-settings.json"), &settings)?;
    w.event("view.settings", None, settings.clone())?;
    Ok(Json(settings))
}
async fn workspaces(State(a): State<App>) -> ApiResult {
    Ok(Json(a.harness.workspaces()?))
}
async fn workspace_action(State(a): State<App>, Json(value): Json<Value>) -> ApiResult {
    Ok(Json(a.harness.workspace_action(&value)?))
}
async fn origin_guard(request: Request, next: Next) -> Response {
    let headers = request.headers();
    let host = headers
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let name = host.split(':').next().unwrap_or("");
    if !["127.0.0.1", "localhost", "[::1]"].contains(&name) {
        return (StatusCode::FORBIDDEN, "Host not allowed").into_response();
    };
    if let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        if origin != format!("http://{host}") && origin != format!("https://{host}") {
            return (StatusCode::FORBIDDEN, "Origin not allowed").into_response();
        }
    }
    next.run(request).await
}
async fn state(State(a): State<App>) -> ApiResult {
    let mut value = {
        let w = a.workspace.lock().unwrap();
        let mut value = w.state();
        value["workspaceInstance"] = json!({"independent":w.root.parent().is_some_and(|p|p.file_name().is_some_and(|n|n=="instances")),"sharedArtworks":true});
        value["targetCompatibility"] =
            crate::scene_tools::target_report(&w.project.scene, a.transport.config().capacity);
        value["materialCompatibility"] = a
            .transport
            .material_report(&w.project.scene, &w.project.materials);
        value
    };
    value["software"] = json!({
        "name": "MorphForge",
        "version": env!("CARGO_PKG_VERSION"),
        "apiVersion": "mf-http/2"
    });
    value["connection"] = a.transport.state();
    value["harness"] = a.harness.state();
    value["capture"] = json!({"browserPid":*a.browser.lock().unwrap()});
    Ok(Json(value))
}
async fn scene(State(a): State<App>, Query(q): Query<Revision>) -> ApiResult {
    let w = a.workspace.lock().unwrap();
    let revision = q.revision.unwrap_or(w.project.scene.revision);
    let scene = w.scene_at(revision)?;
    let materials = w.materials_at(revision)?;
    let surfaces = morphforge_core::surfaces::transparent_interfaces(&scene, &materials);
    let transport = a.transport.inner.lock().unwrap();
    Ok(Json(
        json!({"scene":crate::target::decorate_preview(&scene,&materials,transport.avatar_catalog.as_ref()),"materials":crate::target::preview_materials(&materials,transport.avatar_catalog.as_ref()),"surfaces":surfaces,"textLayouts":crate::text_panels::layouts(&scene)?}),
    ))
}
async fn text_panels_list(State(a):State<App>)->ApiResult {Ok(Json(crate::text_panels::apply(&mut a.workspace.lock().unwrap(),&json!({"action":"list"}),None)?))}
async fn text_panels_action(State(a):State<App>,Json(v):Json<Value>)->ApiResult {Ok(Json(a.harness.tool("text_panels",&v,None).await?))}
async fn pixel_font_asset(AxPath((font,kind)):AxPath<(String,String)>)->Response {
    match crate::text_panels::asset(&font,&kind){Some(bytes)=>([("content-type","application/octet-stream"),("cache-control","public, max-age=86400")],bytes).into_response(),None=>StatusCode::NOT_FOUND.into_response()}
}
async fn avatar_discover(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    let port = v
        .get("port")
        .filter(|v| !v.is_null())
        .map(|v| {
            v.as_u64()
                .and_then(|n| u16::try_from(n).ok())
                .context("查询端口须为1..65535")
        })
        .transpose()?;
    let report = discover_avatar(&a, port, false).await?;
    Ok(Json(report))
}
async fn discover_avatar(a: &App, port: Option<u16>, automatic: bool) -> anyhow::Result<Value> {
    let mut report = crate::avatar_discovery::query(port).await?;
    let _gate = if automatic {
        Some(a.workspace_gate.read().await)
    } else {
        None
    };
    if automatic && !a.transport.config().auto_detect {
        return Ok(json!({"status":"disabled"}));
    }
    let root = a.workspace.lock().unwrap().root.clone();
    let shared = a.workspace.lock().unwrap().library_root.clone();
    let hash = report["info"]["catalogHash"]
        .as_str()
        .context("目录指纹缺失")?;
    let current = a.transport.inner.lock().unwrap().avatar_catalog.clone();
    let features = [
        (morphforge_core::mf6::REGION_CAPABILITY, "region8"),
        (crate::target::SURFACE_CAPABILITY, "surfaceMasks"),
        ("virtual-light/1", "virtualLight"),
    ];
    let already_selected = current.as_ref().is_some_and(|c| {
        c.identity_hash() == hash
            && features.iter().all(|(cap, field)| {
                c.capabilities.iter().any(|v| v == cap) == (report["info"][*field] == true)
            })
    });
    if let Some(c) = &current {
        crate::avatar_discovery::cache(&shared, c)?;
    }
    let target = current
        .filter(|c| c.identity_hash() == hash)
        .or(crate::avatar_discovery::lookup(&shared, hash)?)
        .or(crate::avatar_discovery::lookup(&root, hash)?);
    report["catalogKnown"] = json!(target.is_some());
    let signature = report["info"]["signature"].clone();
    {
        let mut state = a.transport.inner.lock().unwrap();
        let old = &state.discovery["info"]["signature"];
        if old.is_string() && old != &signature {
            state.config.enabled = false;
            state.generation += 1;
            state.sent_revision = None;
            state.unified.clear_sent();
            state.last_error = "Avatar能力/构建已改变，已停止发送；确认新目标后重新开启".into();
            report["senderStopped"] = json!(true);
        }
    }
    if let Some(mut c) = target {
        let expected_layered = c
            .texture_options
            .as_ref()
            .is_some_and(|v| v["layeredOnly"] == true);
        if expected_layered != (report["info"]["layeredOnly"] == true) {
            anyhow::bail!("Avatar纹理布局标记与所声明目录不符");
        }
        for (cap, field) in features {
            c.capabilities.retain(|v| v != cap);
            if report["info"][field] == true {
                c.capabilities.push(cap.into());
            }
        }
        if !a.transport.config().enabled && !already_selected {
            a.transport.set_catalog(Some(c), &root)?;
            a.transport
                .configure(&json!({"profile":"mf6"}), &mut a.workspace.lock().unwrap())?;
            report["applied"] = json!(true);
        }
    }
    a.transport.inner.lock().unwrap().discovery = report.clone();
    Ok(report)
}

async fn avatar_materials(State(a): State<App>) -> ApiResult {
    let catalog = a
        .transport
        .inner
        .lock()
        .unwrap()
        .avatar_catalog
        .clone()
        .context("请先识别Avatar或导入上传目录")?;
    let mut w = a.workspace.lock().unwrap();
    let mut materials = w.project.materials.clone();
    let mut used = materials
        .iter()
        .map(|m| m.id)
        .collect::<std::collections::BTreeSet<_>>();
    let mut next = 1;
    let mut added = 0;
    for mut m in catalog.materials.clone() {
        if materials.iter().any(|old| {
            old.uid == m.uid
                && crate::target::signature(old, false) == crate::target::signature(&m, false)
        }) {
            continue;
        }
        for path in &m.textures {
            let name = path.strip_prefix("/artifacts/").context("无效目标纹理")?;
            validate_id(name)?;
            if !w.artifact_path(name).is_file() {
                bail_api("目录只有材质声明，缺少图像；请导入包含artifacts的Avatar构建包")?;
            }
        }
        if materials.iter().any(|old| old.uid == m.uid) {
            m.uid = format!("{}@avatar-{}", m.uid, &catalog.identity_hash()[..8]);
        }
        if used.contains(&m.id) {
            while used.contains(&next) && next <= 65535 {
                next += 1;
            }
            if next > 65535 {
                bail_api("项目材质编号已满")?;
            }
            m.id = next;
        }
        used.insert(m.id);
        materials.push(m);
        added += 1;
    }
    if added > 0 {
        w.update_materials(materials)?;
    }
    Ok(Json(
        json!({"ok":true,"materialsAdded":added,"revision":w.project.scene.revision,"catalogHash":catalog.identity_hash(),"geometryChanged":false}),
    ))
}

async fn avatar_profile(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    let w = a.workspace.lock().unwrap();
    let catalog = match v["action"].as_str() {
        Some("clear") => None,
        Some("bundled_mf6") => Some(crate::target::AvatarCatalog::from_bytes(include_bytes!(
            "../../../materials/mf6/catalog.json"
        ))?),
        _ => {
            bail_api("请选择配套统一协议目录、清除，或通过文件导入目录")?;
            None
        }
    };
    a.transport.set_catalog(catalog, &w.root)?;
    Ok(Json(
        a.transport
            .material_report(&w.project.scene, &w.project.materials),
    ))
}
async fn avatar_profile_import(State(a): State<App>, mut files: Multipart) -> ApiResult {
    let mut bytes = None;
    while let Some(field) = files.next_field().await? {
        if field.name() == Some("file") {
            bytes = Some(field.bytes().await?);
        }
    }
    let bytes = bytes.context("请选择目录JSON或ZIP")?;
    let catalog = crate::target::AvatarCatalog::from_bytes(&bytes)?;
    let mut w = a.workspace.lock().unwrap();
    crate::avatar_discovery::import_assets(&w.library_root, &bytes, &catalog)?;
    crate::avatar_discovery::cache(&w.library_root, &catalog)?;
    a.transport.set_catalog(Some(catalog), &w.root)?;
    w.event(
        "target.catalog.imported",
        None,
        json!({"remoteVerified":false}),
    )?;
    Ok(Json(
        a.transport
            .material_report(&w.project.scene, &w.project.materials),
    ))
}
async fn export(State(a): State<App>) -> ApiResult {
    let w = a.workspace.lock().unwrap();
    Ok(Json(
        json!({"format":"MorphForgeProject/1","project":w.project}),
    ))
}
async fn export_model(
    State(a): State<App>,
    Query(options): Query<crate::model_export::Options>,
) -> Result<Response, ApiError> {
    let _permit = a.export_jobs.acquire().await?;
    let (scene, materials, root) = {
        let w = a.workspace.lock().unwrap();
        (
            w.project.scene.clone(),
            w.project.materials.clone(),
            w.root.clone(),
        )
    };
    let result = tokio::task::spawn_blocking(move || {
        crate::model_export::export(&scene, &materials, &root, &options)
    })
    .await??;
    Ok((
        [
            ("Content-Type", result.mime.to_owned()),
            (
                "Content-Disposition",
                format!("attachment; filename=\"morphforge.{}\"", result.extension),
            ),
            ("Cache-Control", "no-store".into()),
        ],
        result.bytes,
    )
        .into_response())
}
async fn project(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    let mut w = a.workspace.lock().unwrap();
    if let Some(groups) = v.get("protectedGroups") {
        if v.as_object().is_none_or(|fields| {
            fields
                .keys()
                .any(|key| !["protectedGroups", "baseRevision"].contains(&key.as_str()))
        }) {
            bail_api("保护分组须作为独立设置提交")?;
        }
        let base = v["baseRevision"]
            .as_u64()
            .context("设置保护分组必须提供baseRevision")?;
        if base != w.project.scene.revision {
            bail_api("REVISION_CONFLICT：工程已改变，请刷新后重试保护设置")?;
        }
        let groups: Vec<String> = serde_json::from_value(groups.clone())?;
        w.set_protected_groups(groups)?;
        w.event("project.settings", None, v)?;
        return Ok(Json(w.state()));
    }
    let name = v
        .get("name")
        .map(|x| x.as_str().context("名称须为文本"))
        .transpose()?;
    let notes = v
        .get("pinnedNotes")
        .map(|x| x.as_str().context("固定要求须为文本"))
        .transpose()?;
    if name.is_some_and(|s| s.len() > 256) || notes.is_some_and(|s| s.len() > 32000) {
        bail_api("名称或固定要求过长")?;
    }
    let modeling_style = v
        .get("modelingStyle")
        .map(|s| s.as_str().context("建模风格须为文本"))
        .transpose()?;
    if modeling_style.is_some_and(|style| !["auto", "voxel", "lowpoly", "mixed"].contains(&style)) {
        bail_api("建模风格需要auto/voxel/lowpoly/mixed")?;
    }
    let modeling_extent = if let Some(value) = v.get("modelingExtent") {
        let extent: Option<[u32; 3]> = serde_json::from_value(value.clone())?;
        if extent.is_some_and(|e| e.iter().any(|n| *n == 0 || *n > 4096)) {
            bail_api("目标尺寸每轴须为1..4096格，或null由模型决定")?;
        }
        Some(extent)
    } else {
        None
    };
    let mut next = w.project.scene.clone();
    if let Some(value) = v.get("cellSize") {
        next.cell_size = value.as_f64().context("格长须为数字")?;
    }
    if let Some(value) = v.get("size") {
        let size = value.as_u64().context("网格边长须为正整数")?;
        if !(8..=65536).contains(&size) {
            bail_api("网格边长8..65536")?;
        }
        next.size = size as u32;
    }
    next.validate(&w.project.limits)?;
    if next.cell_size != w.project.scene.cell_size || next.size != w.project.scene.size {
        w.replace(next, v["baseRevision"].as_u64(), None)?;
    }
    if let Some(name) = name {
        w.project.name = name.into();
    }
    if let Some(notes) = notes {
        w.project.pinned_notes = notes.into();
    }
    if let Some(extent) = modeling_extent {
        w.project.modeling_extent = extent;
    }
    if let Some(style) = modeling_style {
        w.project.modeling_style = style.into();
    }
    if let Some(value) = v.get("autoCenterOnFinish") {
        w.project.auto_center_on_finish = value.as_bool().context("自动居中须为布尔值")?;
    }

    w.persist()?;
    w.event("project.settings", None, v)?;
    Ok(Json(w.state()))
}
fn bail_api(s: &str) -> Result<()> {
    bail!("{s}")
}
async fn edit(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    let mut w = a.workspace.lock().unwrap();
    let result = if v["op"] == "demo" {
        let ops = demo(v["name"].as_str().unwrap_or("house"))?;
        w.apply(
            &ops,
            v["baseRevision"].as_u64(),
            v["requestId"].as_str(),
            None,
        )?
    } else if v["op"] == "undo" || v["op"] == "redo" {
        w.undo(v["op"] == "redo", None)?
    } else {
        let operations: Vec<Operation> = if let Some(values) = v.get("operations") {
            serde_json::from_value(values.clone())?
        } else {
            vec![serde_json::from_value(v.clone())?]
        };
        w.apply(
            &operations,
            v["baseRevision"].as_u64(),
            v["requestId"].as_str(),
            None,
        )?
    };
    if a.transport.config().enabled && a.transport.config().auto {
        let _ = w.commit(None, "手动编辑自动发布", None)?;
        if let Err(e) =
            a.transport
                .publish_with_materials(&w.project.scene, &w.project.materials, false)
        {
            w.event("publish.rejected", None, json!({"error":e.to_string()}))?;
            return Ok(Json(
                json!({"ok":true,"revision":w.project.scene.revision,"warning":e.to_string(),"draftSaved":true}),
            ));
        }
    }
    Ok(Json(result))
}
async fn minecraft_sources(State(a): State<App>) -> ApiResult {
    let root = a.workspace.lock().unwrap().root.clone();
    Ok(Json(crate::minecraft::imports::list(&root)?))
}
async fn minecraft_upload(State(a): State<App>, mut multipart: Multipart) -> ApiResult {
    let _gate = a.workspace_gate.read().await;
    let permit = a.minecraft_jobs.clone().acquire_owned().await?;
    let project_id = a.workspace.lock().unwrap().project.id.clone();
    let mut file = None;
    while let Some(field) = multipart.next_field().await? {
        if field.name() != Some("file") {
            continue;
        }
        if file.is_some() {
            bail_api("一次只导入一个建筑文件")?;
        }
        let name = field.file_name().unwrap_or("building.nbt").to_owned();
        let bytes = field.bytes().await?;
        if bytes.len() > 64 * 1024 * 1024 {
            bail_api("建筑文件超过64MiB")?;
        }
        file = Some((name, bytes.to_vec()));
    }
    let (name, bytes) = file.context("请选择.litematic/.schem/.schematic/Java结构.nbt建筑文件")?;
    let (name, bytes, building) = tokio::task::spawn_blocking(move || -> Result<_> {
        let _permit = permit;
        let building = crate::minecraft::formats::parse(&bytes, &name)?;
        Ok((name, bytes, building))
    })
    .await??;
    let mut w = a.workspace.lock().unwrap();
    if w.project.id != project_id {
        bail_api("WORKSPACE_CONFLICT：工程已切换")?;
    }
    Ok(Json(crate::minecraft::imports::register(
        &mut w, &bytes, &name, &building,
    )?))
}
async fn minecraft_action(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    let _gate = a.workspace_gate.read().await;
    match v["action"].as_str().unwrap_or("list") {
        "list" => minecraft_sources(State(a.clone())).await,
        "inspect" => {
            let root = a.workspace.lock().unwrap().root.clone();
            Ok(Json(
                tokio::task::spawn_blocking(move || crate::minecraft::imports::inspect(&root, &v))
                    .await??,
            ))
        }
        "preview" => {
            let _permit = a.minecraft_jobs.clone().acquire_owned().await?;
            Ok(Json(
                crate::minecraft::imports::preview(a.workspace.clone(), v, None).await?,
            ))
        }
        "apply" => {
            let mut w = a.workspace.lock().unwrap();
            let mut result = crate::minecraft::imports::apply(&mut w, &v, None)?;
            if result["replayed"] != true
                && a.transport.config().enabled
                && a.transport.config().auto
            {
                w.commit(None, "Minecraft建筑导入", None)?;
                if let Err(error) = a.transport.publish_with_materials(
                    &w.project.scene,
                    &w.project.materials,
                    false,
                ) {
                    result["warning"] = json!(error.to_string());
                    result["draftSaved"] = json!(true);
                }
            }
            Ok(Json(result))
        }
        _ => Err(anyhow::anyhow!("未知Minecraft导入操作").into()),
    }
}
async fn minecraft_preview_scene(State(a): State<App>, AxPath(id): AxPath<String>) -> ApiResult {
    let root = a.workspace.lock().unwrap().root.clone();
    Ok(Json(
        tokio::task::spawn_blocking(move || crate::minecraft::imports::preview_scene(&root, &id))
            .await??,
    ))
}
async fn minecraft_original(
    State(a): State<App>,
    AxPath(id): AxPath<String>,
) -> Result<Response, ApiError> {
    let root = a.workspace.lock().unwrap().root.clone();
    let (bytes, name) =
        tokio::task::spawn_blocking(move || crate::minecraft::imports::original(&root, &id))
            .await??;
    Ok((
        [
            ("Content-Type", "application/octet-stream".to_string()),
            (
                "Content-Disposition",
                format!("attachment; filename=\"{name}\""),
            ),
        ],
        bytes,
    )
        .into_response())
}
async fn mesh_edit(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    let mut w = a.workspace.lock().unwrap();
    let mut result = crate::mesh_tools::execute(&mut w, &v, None)?;
    if v["dryRun"] != true
        && v["action"] != "inspect"
        && a.transport.config().enabled
        && a.transport.config().auto
    {
        w.commit(None, "多边形编辑自动发布", None)?;
        if let Err(error) =
            a.transport
                .publish_with_materials(&w.project.scene, &w.project.materials, false)
        {
            result["warning"] = json!(error.to_string());
            result["draftSaved"] = json!(true);
        }
    }
    Ok(Json(result))
}
async fn import(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    let mut w = a.workspace.lock().unwrap();
    let value = v
        .get("project")
        .and_then(|p| p.get("scene"))
        .or_else(|| v.get("scene"))
        .or_else(|| v.get("document"))
        .unwrap_or(&v);
    let scene = Scene::import_json(value, &w.project.limits)?;
    let materials = v
        .get("project")
        .and_then(|p| p.get("materials"))
        .or_else(|| v.get("materials"))
        .map(|m| serde_json::from_value::<Vec<morphforge_core::materials::Material>>(m.clone()))
        .transpose()?;
    if let Some(ref values) = materials {
        validate_materials(values)?;
    }
    let creative = v
        .get("project")
        .and_then(|p| p.get("creative"))
        .map(|v| serde_json::from_value::<crate::creative::Creative>(v.clone()))
        .transpose()?;
    if let Some(c) = &creative {
        c.validate(&w.project.limits)?;
    }
    let old_creative = w.project.creative.clone();
    if let Some(mut c) = creative {
        c.revision = w
            .project
            .creative
            .revision
            .checked_add(1)
            .context("创作版本溢出")?;
        w.project.creative = c;
    }
    let materials = materials.unwrap_or_else(|| w.project.materials.clone());
    let result = match w.replace_with_materials(scene, materials, v["baseRevision"].as_u64(), None)
    {
        Ok(v) => v,
        Err(e) => {
            w.project.creative = old_creative;
            return Err(e.into());
        }
    };
    Ok(Json(result))
}
fn validate_materials(values: &[morphforge_core::materials::Material]) -> Result<()> {
    if values.len() > 16384 {
        bail!("材质数量超过保护值")
    };
    let mut ids = std::collections::BTreeSet::new();
    for m in values {
        if !ids.insert(m.id)
            || m.name.len() > 1024
            || m.uid.len() > 1024
            || !m
                .color
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            || ![m.metallic, m.roughness, m.opacity]
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            || !m.emission.is_finite()
            || !(0.0..=100.).contains(&m.emission)
        {
            bail!("材质字段无效或id重复")
        };
        if !["opaque", "cutout", "transparent"].contains(&m.render_mode.as_str())
            || m.textures.len() > 6
            || m.textures.iter().any(|v| {
                !v.starts_with("/artifacts/")
                    || v[11..].contains('/')
                    || crate::store::validate_id(&v[11..]).is_err()
            })
        {
            bail!("材质渲染模式或纹理路径无效")
        }
    }
    Ok(())
}
async fn undo(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    Ok(Json(a.workspace.lock().unwrap().undo(
        v["redo"].as_bool().unwrap_or(false),
        v["revision"].as_u64(),
    )?))
}
async fn history(State(a): State<App>) -> ApiResult {
    let w = a.workspace.lock().unwrap();
    Ok(Json(
        json!({"revisions":w.revisions.keys().collect::<Vec<_>>(),"commits":w.project.commits}),
    ))
}
async fn connect(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    let mut w = a.workspace.lock().unwrap();
    Ok(Json(a.transport.configure(&v, &mut w)?))
}
async fn publish(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    if v["repair"].as_bool().unwrap_or(false)
        && (a.transport.config().profile != "mf6" || v["resync"].as_bool().unwrap_or(false))
    {
        return Err(anyhow::anyhow!("补播不可与清空重建同时使用").into());
    }
    let mut w = a.workspace.lock().unwrap();
    let rev = v["revision"].as_u64().unwrap_or(w.project.scene.revision);
    let _ = w.commit(Some(rev), v["title"].as_str().unwrap_or("手动发布"), None)?;
    let scene = w.scene_at(rev)?;
    let materials = w.materials_at(rev)?;
    let result = if v["repair"].as_bool().unwrap_or(false) {
        a.transport.publish_for_repair(&scene, &materials)?
    } else {
        a.transport.publish_with_materials(
            &scene,
            &materials,
            v["resync"].as_bool().unwrap_or(false),
        )?
    };
    w.event(
        "revision.published",
        None,
        json!({"revision":rev,"state":result}),
    )?;
    Ok(Json(result))
}
async fn placement(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    let result = a.transport.placement(&v)?;
    let mut w = a.workspace.lock().unwrap();
    let mut config = a.transport.config();
    config.enabled = false;
    crate::store::write_json(&w.root.join("connection.json"), &config)?;
    w.event("placement.changed", None, v)?;
    Ok(Json(result))
}
async fn styles(State(a): State<App>) -> ApiResult {
    Ok(Json(
        json!({"styles":a.workspace.lock().unwrap().project.materials}),
    ))
}
async fn events(State(a): State<App>, Query(q): Query<Revision>) -> ApiResult {
    let w = a.workspace.lock().unwrap();
    let values: Vec<_> = w
        .events
        .iter()
        .filter(|e| {
            e.project_id.as_deref() == Some(&w.project.id)
                && e.session_id.as_deref() == Some(&w.session_id)
                && e.seq > q.after.unwrap_or(0)
                && q.run.as_ref().is_none_or(|r| e.run_id.as_ref() == Some(r))
        })
        .take(500)
        .map(|e| {
            if q.brief == Some(true) {
                crate::timeline::brief(e)
            } else {
                serde_json::to_value(e).unwrap_or(Value::Null)
            }
        })
        .collect();
    Ok(Json(json!({"events":values,"lastSeq":w.next_seq-1})))
}
async fn timeline_page(
    State(a): State<App>,
    Query(mut q): Query<crate::timeline::Query>,
) -> ApiResult {
    let root = {
        let w = a.workspace.lock().unwrap();
        q.project_id = Some(w.project.id.clone());
        if q.session.as_deref() == Some("current") {
            q.session = Some(w.session_id.clone());
        }
        w.root.clone()
    };
    let value = tokio::task::spawn_blocking(move || crate::timeline::page(&root, &q)).await??;
    Ok(Json(a.harness.redact_display(value)))
}
async fn timeline_detail(State(a): State<App>, AxPath(seq): AxPath<u64>) -> ApiResult {
    let root = a.workspace.lock().unwrap().root.clone();
    let value = tokio::task::spawn_blocking(move || crate::timeline::detail(&root, seq)).await??;
    Ok(Json(a.harness.redact_display(value)))
}
async fn artworks(State(a): State<App>) -> ApiResult {
    let mut workspace = a.workspace.lock().unwrap();
    Ok(Json(
        json!({"artworks":workspace.library()?,"examples":crate::examples::list(&mut workspace)?}),
    ))
}
async fn artwork_action(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    Ok(Json(a.harness.tool("artworks", &v, None).await?))
}
async fn artwork_delete(State(a): State<App>, AxPath(id): AxPath<String>) -> ApiResult {
    let mut w = a.workspace.lock().unwrap();
    Ok(Json(w.remove_artwork(&id)?))
}
async fn artwork_get(State(a): State<App>, AxPath(id): AxPath<String>) -> ApiResult {
    if id.starts_with("builtin:") {
        return Ok(Json(crate::examples::get(
            &mut a.workspace.lock().unwrap(),
            &id,
        )?));
    }
    validate_id(&id)?;
    let w = a.workspace.lock().unwrap();
    Ok(Json(w.read_artwork(&id)?))
}
async fn pack_import(State(a): State<App>, mut multipart: Multipart) -> ApiResult {
    let mut data = None;
    let mut name = "资源包".to_owned();
    while let Some(field) = multipart.next_field().await? {
        if field.name() == Some("file") {
            name = field.file_name().unwrap_or("资源包").to_owned();
            data = Some(field.bytes().await?.to_vec());
        }
    }
    let bytes = data.context("缺少ZIP文件")?;
    let result =
        crate::resources::import_java_pack(&mut a.workspace.lock().unwrap(), &bytes, &name)?;
    Ok(Json(result))
}
async fn harness_state(State(a): State<App>) -> ApiResult {
    Ok(Json(a.harness.state()))
}
async fn provider_config(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    Ok(Json(a.harness.configure_provider(&v)?))
}
async fn harness_config(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    Ok(Json(a.harness.configure(&v)?))
}
async fn harness_start(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    Ok(Json(
        a.harness
            .start(v["prompt"].as_str().context("缺少prompt")?)?,
    ))
}
async fn harness_control(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    Ok(Json(
        a.harness
            .control(v["action"].as_str().context("缺少action")?)?,
    ))
}
async fn harness_message(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    Ok(Json(a.harness.message(&v)?))
}
async fn harness_tool(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    let name = v["name"].as_str().context("缺少工具名")?;
    let arguments = v.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let id = uuid::Uuid::new_v4().to_string();
    let start = std::time::Instant::now();
    a.workspace.lock().unwrap().event("tool.request",None,json!({"callId":id,"name":name,"arguments":arguments,"toolVersion":crate::harness::TOOL_SCHEMA_VERSION,"source":"manual"}))?;
    let result = a.harness.tool(name, &arguments, None).await;
    let value = match &result {
        Ok(value) => value.clone(),
        Err(error) => json!({"ok":false,"error":{"code":"TOOL_ERROR","message":error.to_string()}}),
    };
    a.workspace.lock().unwrap().event("tool.result",None,json!({"callId":id,"name":name,"elapsedMs":start.elapsed().as_secs_f64()*1000.,"result":value,"source":"manual"}))?;
    Ok(Json(result?))
}
async fn models(State(a): State<App>) -> ApiResult {
    Ok(Json(a.harness.provider.models().await?))
}
async fn capture_jobs(State(a): State<App>) -> ApiResult {
    Ok(Json(a.capture.claim()))
}
async fn capture_result(State(a): State<App>, Json(v): Json<Value>) -> ApiResult {
    Ok(Json(
        a.capture.complete(&mut a.workspace.lock().unwrap(), &v)?,
    ))
}
async fn reference_list(State(a): State<App>) -> ApiResult {
    Ok(Json(references::list(&a.workspace.lock().unwrap())?))
}
async fn reference_remove(State(a): State<App>, Json(value): Json<Value>) -> ApiResult {
    Ok(Json(references::remove(
        &mut a.workspace.lock().unwrap(),
        value["id"].as_str().context("缺少图片ID")?,
    )?))
}
async fn reference_upload(State(a): State<App>, mut multipart: Multipart) -> ApiResult {
    let mut file = None;
    while let Some(mut field) = multipart.next_field().await? {
        if field.name() != Some("file") {
            bail_api("图片上传只接受file字段")?;
        }
        if file.is_some() {
            bail_api("每次上传一张图片")?;
        }
        let name = field.file_name().unwrap_or("参考图片").to_owned();
        let mut bytes = Vec::new();
        while let Some(chunk) = field.chunk().await? {
            if bytes.len().saturating_add(chunk.len()) > references::MAX_UPLOAD_BYTES {
                bail_api("图片超过8MiB")?;
            }
            bytes.extend_from_slice(&chunk);
        }
        file = Some((name, bytes));
    }
    let (name, bytes) = file.context("请选择图片文件")?;
    let permit = a.reference_jobs.clone().acquire_owned().await?;
    let prepared = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        references::prepare(&bytes)
    })
    .await??;
    Ok(Json(references::import(
        &mut a.workspace.lock().unwrap(),
        &name,
        prepared,
    )?))
}
async fn reference_bytes(a: App, id: String, thumbnail: bool) -> Response {
    match references::image(&a.workspace.lock().unwrap(), &id, thumbnail) {
        Ok((bytes, mime)) => (
            [
                ("Content-Type", mime),
                (
                    "Cache-Control",
                    if id.starts_with("builtin:") {
                        "private, no-cache"
                    } else {
                        "private, max-age=31536000, immutable"
                    },
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
async fn reference_image(State(a): State<App>, AxPath(id): AxPath<String>) -> Response {
    reference_bytes(a, id, false).await
}
async fn reference_thumbnail(State(a): State<App>, AxPath(id): AxPath<String>) -> Response {
    reference_bytes(a, id, true).await
}
async fn artifact(State(a): State<App>, AxPath(id): AxPath<String>) -> Response {
    if validate_id(&id).is_err() {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let path = a.workspace.lock().unwrap().artifact_path(&id);
    match tokio::fs::read(path).await {
        Ok(bytes) => (
            [
                (
                    "Content-Type",
                    mime_guess::from_path(&id)
                        .first_or_octet_stream()
                        .as_ref()
                        .to_owned(),
                ),
                (
                    "Cache-Control",
                    "public, max-age=31536000, immutable".into(),
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
async fn asset(uri: axum::http::Uri, _headers: HeaderMap) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "studio.html" } else { path };
    if path.contains("..") || path.contains('\\') {
        return StatusCode::BAD_REQUEST.into_response();
    };
    match WEB.get_file(path) {
        Some(file) => (
            [
                (
                    "Content-Type",
                    mime_guess::from_path(path)
                        .first_or_octet_stream()
                        .as_ref()
                        .to_owned(),
                ),
                ("Cache-Control", "no-cache".into()),
            ],
            file.contents(),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
fn demo(name: &str) -> Result<Vec<Operation>> {
    let mut ops = vec![Operation::Clear];
    match name {
        "stairs" => {
            for i in 0..8 {
                ops.push(Operation::Box {
                    value: Box3::new([-8 + i * 2, 0, 0], [2, (i + 1) as u32, 2], i as u32 % 4 + 1),
                })
            }
        }
        "directions" => {
            for (p, s, c) in [
                ([-4, 0, 0], [1, 3, 1], 15),
                ([4, 0, 0], [1, 1, 1], 3),
                ([0, 0, 4], [1, 2, 1], 4),
                ([0, 0, -4], [1, 4, 1], 11),
            ] {
                ops.push(Operation::Box {
                    value: Box3::new(p, s, c),
                })
            }
        }
        "stress16k" | "stress5k" => {
            let count = if name == "stress5k" { 5000 } else { 16384 };
            for i in 0..count {
                let x = i % 32;
                let y = i / (32 * 32);
                let z = (i / 32) % 32;
                ops.push(Operation::Box {
                    value: Box3::new([x * 3 - 48, y * 3, z * 3 - 48], [1; 3], (i % 4 + 1) as u32),
                })
            }
        }
        "house" => {
            for (p, s, c) in [
                ([-4, 0, -4], [8, 1, 8], 1),
                ([-4, 1, -4], [8, 5, 1], 2),
                ([-4, 1, 3], [8, 5, 1], 2),
                ([-4, 1, -3], [1, 5, 6], 2),
                ([3, 1, -3], [1, 5, 6], 2),
            ] {
                ops.push(Operation::Box {
                    value: Box3::new(p, s, c),
                })
            }
            ops.push(Operation::Erase {
                min: [-1, 1, 3],
                size: [2, 3, 1],
            });
            for i in 0..3 {
                ops.push(Operation::Box {
                    value: Box3::new([-5 + i, 6 + i, -5], [(10 - 2 * i) as u32, 1, 10], 4),
                })
            }
        }
        _ => bail!("未知示例"),
    };
    Ok(ops)
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ServerOptions {
    pub root: PathBuf,
    pub env_file: Option<PathBuf>,
    pub port: u16,
    pub capture: bool,
    pub isolate_if_busy: bool,
}
pub async fn serve(mut options: ServerOptions) -> Result<()> {
    use fs2::FileExt;
    std::fs::create_dir_all(&options.root)?;
    let shared_root = std::env::var_os("MORPHFORGE_LIBRARY_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| options.root.clone());
    let lock_at = |root: &std::path::Path| -> anyhow::Result<std::fs::File> {
        std::fs::create_dir_all(root)?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join("workspace.lock"))?;
        file.try_lock_exclusive()
            .context("同一作品目录已有后端运行")?;
        Ok(file)
    };
    let lock = match lock_at(&options.root) {
        Ok(file) => file,
        Err(e)
            if options.isolate_if_busy
                && e.downcast_ref::<std::io::Error>().is_some_and(|io| {
                    io.raw_os_error() == fs2::lock_contended_error().raw_os_error()
                }) =>
        {
            let primary = options.root.clone();
            let instances = primary.join("instances");
            std::fs::create_dir_all(&instances)?;
            let mut candidates = std::fs::read_dir(&instances)?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_type().is_ok_and(|t| t.is_dir())
                        && uuid::Uuid::parse_str(&e.file_name().to_string_lossy()).is_ok()
                })
                .map(|e| e.path())
                .collect::<Vec<_>>();
            candidates.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.created()).ok());
            let mut existing = None;
            for path in candidates {
                if let Ok(file) = lock_at(&path) {
                    existing = Some((path, file));
                    break;
                }
            }
            let (path, file) = if let Some(pair) = existing {
                pair
            } else {
                let path = instances.join(uuid::Uuid::new_v4().to_string());
                let file = lock_at(&path)?;
                (path, file)
            };
            options.root = path;
            let fresh = !options.root.join("project.json").is_file();
            // Connection settings only, never an active task, conversation or
            // cache ledger. Each instance owns its fresh project and context.
            for name in [
                "provider-connection.json",
                "provider.env",
                "harness.json",
                "avatar-catalog.json",
                "connection.json",
            ] {
                let src = primary.join(name);
                if fresh && src.is_file() {
                    std::fs::copy(src, options.root.join(name))?;
                }
            }
            eprintln!("Independent workspace selected; original workspace stays locked: {e}");
            file
        }
        Err(e) => return Err(e),
    };
    let app = App::open(options.root.clone(), options.env_file)?;
    app.workspace
        .lock()
        .unwrap()
        .use_shared_library(shared_root)?;
    let listener =
        tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, options.port)).await?;
    let port = listener.local_addr()?.port();
    let mut browser = None;
    if options.capture {
        if let Some(binary) = browser_binary() {
            let profile = options.root.join("capture-browser");
            let log = std::fs::File::create(options.root.join("capture-browser.log"))?;
            let mut cmd = std::process::Command::new(binary);
            cmd.args([
                "--headless=new",
                "--no-sandbox",
                "--disable-dev-shm-usage",
                "--enable-unsafe-swiftshader",
                "--no-first-run",
                "--disable-background-timer-throttling",
                "--disable-renderer-backgrounding",
            ])
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg(format!("http://127.0.0.1:{port}/?capture=1"))
            .stdout(log.try_clone()?)
            .stderr(log);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000);
            }
            let child = cmd.spawn()?;
            *app.browser.lock().unwrap() = Some(child.id());
            browser = Some(child);
        }
    }
    println!(
        "MorphForge Rust ready: http://127.0.0.1:{port} · data={} · OSC disabled",
        options.root.display()
    );
    let result = axum::serve(listener, app.router())
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await;
    app.transport.inner.lock().unwrap().stop = true;
    if let Some(mut child) = browser {
        let _ = child.kill();
        let _ = child.wait();
    }
    result?;
    drop(lock);
    Ok(())
}
fn browser_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("MORPHFORGE_BROWSER") {
        return Some(path.into());
    }
    let candidates = if cfg!(windows) {
        vec![
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        ]
    } else {
        vec![
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/usr/bin/google-chrome",
        ]
    };
    candidates
        .into_iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
}

async fn diagnostics(State(a): State<App>) -> Result<Response, ApiError> {
    let root = a.workspace.lock().unwrap().root.clone();
    let bytes = crate::diagnostics::bundle(&root, |s| a.harness.provider.redact(s))?;
    Ok((
        [
            ("Content-Type", "application/zip"),
            (
                "Content-Disposition",
                "attachment; filename=morphforge-diagnostics.zip",
            ),
        ],
        bytes,
    )
        .into_response())
}
async fn metrics(State(a): State<App>) -> ApiResult {
    let root = a.workspace.lock().unwrap().root.clone();
    Ok(Json(crate::diagnostics::metrics(&root)?))
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogQuery {
    #[serde(default)]
    builtin_minecraft: bool,
    #[serde(default)]
    used_only: bool,
    #[serde(default = "default_catalog_pixels")]
    cell_pixels: u32,
}
fn default_catalog_pixels() -> u32 {
    32
}
async fn catalog_budget(State(a): State<App>, Query(o): Query<CatalogQuery>) -> ApiResult {
    let w = a.workspace.lock().unwrap();
    let materials = crate::catalog_budget::selected(&w, o.used_only);
    Ok(Json(crate::catalog_budget::estimate(
        &w,
        &materials,
        o.cell_pixels,
    )?))
}
async fn catalog(
    State(a): State<App>,
    Query(options): Query<CatalogQuery>,
) -> Result<Response, ApiError> {
    let bytes = {
        let mut w = a.workspace.lock().unwrap();
        if options.builtin_minecraft {
            let source = w.project.materials.clone();
            let materials = crate::minecraft::compile::prepare_builtin_variants(&mut w, &source)?;
            crate::catalog::export_configured(&w, &materials, options.cell_pixels)?
        } else {
            let materials = crate::catalog_budget::selected(&w, options.used_only);
            crate::catalog::export_configured(&w, &materials, options.cell_pixels)?
        }
    };
    Ok((
        [
            ("Content-Type", "application/zip"),
            (
                "Content-Disposition",
                "attachment; filename=morphforge-materials.zip",
            ),
        ],
        bytes,
    )
        .into_response())
}
async fn packs(State(a): State<App>) -> ApiResult {
    let root = a.workspace.lock().unwrap().root.clone();
    let mut values = vec![];
    for e in std::fs::read_dir(root.join("packs"))? {
        let p = e?.path();
        if p.extension().is_some_and(|x| x == "json") {
            values.push(serde_json::from_slice::<Value>(&std::fs::read(p)?)?)
        }
    }
    Ok(Json(json!({"packs":values})))
}

async fn font_list(State(a): State<App>) -> ApiResult {
    Ok(Json(json!({
        "ok": true,
        "version": "mf-fonts/1",
        "defaultFontId": "builtin:noto-sans-sc",
        "fonts": a.fonts.list()
    })))
}

fn parse_text_request(value: &Value) -> Result<VoxelTextRequest> {
    let mut fields = value.as_object().context("文字请求须为对象")?.clone();
    for key in [
        "requestId",
        "baseRevision",
        "previewRequestId",
        "previewFingerprint",
    ] {
        fields.remove(key);
    }
    const TEXT_FIELDS: &[&str] = &[
        "text",
        "fontId",
        "fontSize",
        "depth",
        "origin",
        "plane",
        "facing",
        "align",
        "letterSpacing",
        "lineSpacing",
        "maxWidth",
        "styleId",
        "tintId",
        "motionPhase",
        "group",
    ];
    if let Some(unknown) = fields
        .keys()
        .find(|key| !TEXT_FIELDS.contains(&key.as_str()))
    {
        bail!("未知文字字段：{unknown}")
    }
    Ok(serde_json::from_value(Value::Object(fields))?)
}

async fn prepare_text(a: &App, value: &Value) -> Result<fonts::TextPreview> {
    let request = parse_text_request(value)?;
    if !a
        .workspace
        .lock()
        .unwrap()
        .project
        .materials
        .iter()
        .any(|style| style.id == request.style_id)
    {
        bail!("未知材质编号{}，请从材质目录选择", request.style_id)
    }
    let permit = a.text_jobs.clone().acquire_owned().await?;
    let catalog = a.fonts.clone();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        catalog.preview(&request)
    })
    .await
    .context("文字排版任务异常")?
}

async fn text_preview(State(a): State<App>, Json(value): Json<Value>) -> ApiResult {
    let preview = prepare_text(&a, &value).await?;
    let mut response = serde_json::to_value(preview)?;
    response["requestId"] = value.get("requestId").cloned().unwrap_or(Value::Null);
    response["revision"] = json!(a.workspace.lock().unwrap().project.scene.revision);
    Ok(Json(response))
}

async fn text_apply(State(a): State<App>, Json(value): Json<Value>) -> ApiResult {
    let base = value["baseRevision"]
        .as_u64()
        .context("应用文字必须提供baseRevision")?;
    let request_id = value["requestId"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 256)
        .context("应用文字必须提供不超过256字节的requestId")?;
    let request = parse_text_request(&value)?;
    let fingerprint = fonts::request_fingerprint(&request)?;
    if let Some(expected) = value.get("previewFingerprint") {
        if expected.as_str() != Some(fingerprint.as_str()) {
            bail_api("PREVIEW_CONFLICT：文字设置已改变，请重新预览")?;
        }
    }
    {
        let mut workspace = a.workspace.lock().unwrap();
        if let Some(mut replay) = workspace.text_receipt(request_id, &fingerprint)? {
            replay["ok"] = json!(true);
            let revision = replay["revision"]
                .as_u64()
                .context("文字请求回执缺少revision")?;
            replay["scene"] = serde_json::to_value(workspace.scene_at(revision)?)?;
            return Ok(Json(replay));
        }
        workspace.event(
            "text.request",
            None,
            json!({
                "requestId": request_id,
                "requestFingerprint": fingerprint,
                "request": request,
                "baseRevision": base
            }),
        )?;
    }
    let preview = prepare_text(&a, &value).await?;
    let mut workspace = a.workspace.lock().unwrap();
    let mut result = workspace.apply_text(
        &preview.operations,
        base,
        request_id,
        &fingerprint,
        &serde_json::to_value(&preview.font)?,
        &serde_json::to_value(&preview.layout)?,
        None,
    )?;
    result["ok"] = json!(true);
    if a.transport.config().enabled && a.transport.config().auto {
        let publish = workspace
            .commit(None, "体素文字自动发布", None)
            .and_then(|_| {
                a.transport.publish_with_materials(
                    &workspace.project.scene,
                    &workspace.project.materials,
                    false,
                )
            });
        if let Err(error) = publish {
            let _ = workspace.event("publish.rejected", None, json!({"error":error.to_string()}));
            result["warning"] = json!(error.to_string());
            result["draftSaved"] = json!(true);
        }
    }
    result["scene"] = serde_json::to_value(
        workspace.scene_at(
            result["revision"]
                .as_u64()
                .context("文字回执缺少revision")?,
        )?,
    )?;
    Ok(Json(result))
}

async fn harness_tools(State(a): State<App>, Query(query): Query<ToolQuery>) -> ApiResult {
    let language = query.language.unwrap_or_else(|| {
        a.harness.state()["settings"]["language"]
            .as_str()
            .unwrap_or("zh-CN")
            .to_owned()
    });
    if !["zh-CN", "en"].contains(&language.as_str()) {
        bail_api("工具说明语言只支持zh-CN或en")?;
    }
    Ok(Json(json!({
        "version":crate::harness::TOOL_SCHEMA_VERSION,
        "promptVersion":crate::harness::SYSTEM_PROMPT_VERSION,
        "language":language,
        "tools":crate::harness::tools_for_language(&language)
    })))
}
