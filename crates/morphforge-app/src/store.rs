use anyhow::{bail, Context, Result};
use chrono::Utc;
use morphforge_core::materials::{builtins, Material};
use morphforge_core::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    fs::write(&temp, bytes)?;
    fs::rename(&temp, path).context("原子保存失败")?;
    Ok(())
}
pub fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    atomic(path, &serde_json::to_vec_pretty(value)?)
}
fn verify_snapshot(path: &Path, scene: &Scene) -> Result<()> {
    let existing: Value = serde_json::from_slice(
        &fs::read(path).with_context(|| format!("读取快照失败：{}", path.display()))?,
    )
    .with_context(|| format!("快照JSON无效：{}", path.display()))?;
    if existing != serde_json::to_value(scene)? {
        bail!(
            "IMMUTABLE_SNAPSHOT_CONFLICT：同一revision已有不同场景内容（{}）",
            path.display()
        )
    }
    Ok(())
}
fn write_snapshot_once(path: &Path, scene: &Scene) -> Result<()> {
    if path.exists() {
        return verify_snapshot(path, scene);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(scene)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&temp);
        return Err(error.into());
    }
    drop(file);
    let result = fs::hard_link(&temp, path);
    let _ = fs::remove_file(&temp);
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            verify_snapshot(path, scene)
        }
        Err(error) => Err(error.into()),
    }
}
fn load_snapshots(root: &Path, project_id: &str) -> Result<BTreeMap<u64, ()>> {
    let mut revisions = BTreeMap::new();
    let prefix = format!("{project_id}-");
    for entry in fs::read_dir(root.join("snapshots"))? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if let Some(revision) = name
            .strip_prefix(&prefix)
            .and_then(|s| s.strip_suffix(".json"))
            .and_then(|s| s.parse::<u64>().ok())
        {
            revisions.insert(revision, ());
        }
    }
    Ok(revisions)
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub id: String,
    pub revision: u64,
    pub title: String,
    pub created_at: String,
    pub scene_hash: String,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextReceipt {
    pub revision: u64,
    pub stats: Value,
    pub layout: Value,
    pub font: Value,
    pub request_id: String,
    pub request_fingerprint: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub scene: Scene,
    pub limits: Limits,
    pub materials: Vec<Material>,
    #[serde(default)]
    pub commits: Vec<Commit>,
    #[serde(default)]
    pub pinned_notes: String,
    #[serde(default)]
    pub text_receipts: BTreeMap<String, TextReceipt>,
    #[serde(default)]
    pub protected_groups: BTreeSet<String>,
}
impl Default for Project {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: "未命名作品".into(),
            scene: Scene::default(),
            limits: Limits::default(),
            materials: builtins(),
            commits: vec![],
            pinned_notes: String::new(),
            text_receipts: BTreeMap::new(),
            protected_groups: BTreeSet::new(),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artwork {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub box_count: usize,
    pub voxel_count: u64,
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub views: Vec<String>,
    pub source_run: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub seq: u64,
    pub time: String,
    pub kind: String,
    pub run_id: Option<String>,
    pub revision: u64,
    pub data: Value,
}
pub struct Workspace {
    pub root: PathBuf,
    pub project: Project,
    pub events: Vec<Event>,
    pub next_seq: u64,
    pub revisions: BTreeMap<u64, ()>,
    pub undo: Vec<u64>,
    pub redo: Vec<u64>,
    pub requests: HashMap<String, Value>,
    pub recipes: HashMap<u64, String>,
}
impl Workspace {
    pub fn open(root: PathBuf) -> Result<Self> {
        for d in [
            "snapshots",
            "events",
            "artifacts",
            "artworks",
            "runs",
            "packs",
        ] {
            fs::create_dir_all(root.join(d))?;
        }
        let path = root.join("project.json");
        let project = if path.exists() {
            serde_json::from_slice::<Project>(&fs::read(&path)?)?
        } else {
            Project::default()
        };
        project.scene.validate(&project.limits)?;
        let mut events = VecDeque::with_capacity(5000);
        let mut last_seq = 0u64;
        let log = root.join("events/events.jsonl");
        if log.exists() {
            for line in BufReader::new(fs::File::open(log)?).lines() {
                let Ok(line) = line else { continue };
                if let Ok(event) = serde_json::from_str::<Event>(&line) {
                    last_seq = event.seq;
                    if events.len() == 5000 {
                        events.pop_front();
                    }
                    events.push_back(event);
                }
            }
        }
        let next_seq = last_seq.saturating_add(1).max(1);
        let mut revisions = load_snapshots(&root, &project.id)?;
        revisions.entry(project.scene.revision).or_insert(());
        let mut undo: Vec<u64> = revisions
            .keys()
            .copied()
            .filter(|r| *r < project.scene.revision)
            .collect();
        if undo.len() > 128 {
            undo.drain(..undo.len() - 128);
        }
        let mut w = Self {
            root,
            project,
            events: events.into_iter().collect(),
            next_seq,
            revisions,
            undo,
            redo: vec![],
            requests: HashMap::new(),
            recipes: HashMap::new(),
        };
        w.persist()?;
        Ok(w)
    }
    pub fn snapshot_path(&self, revision: u64) -> PathBuf {
        self.root
            .join("snapshots")
            .join(format!("{}-{revision}.json", self.project.id))
    }
    pub fn scene_at(&self, revision: u64) -> Result<Scene> {
        if revision == self.project.scene.revision {
            return Ok(self.project.scene.clone());
        }
        let scene: Scene = serde_json::from_slice(
            &fs::read(self.snapshot_path(revision)).context("找不到指定版本")?,
        )?;
        if scene.revision != revision {
            bail!("场景快照revision与请求不一致")
        };
        Ok(scene)
    }
    pub fn material_snapshot_path(&self, revision: u64) -> PathBuf {
        self.root
            .join("snapshots")
            .join(format!("{}-{revision}.materials.json", self.project.id))
    }
    pub fn materials_at(&self, revision: u64) -> Result<Vec<Material>> {
        let p = self.material_snapshot_path(revision);
        if p.exists() {
            Ok(serde_json::from_slice(&fs::read(p)?)?)
        } else {
            Ok(self.project.materials.clone())
        }
    }
    pub fn replace_with_materials(
        &mut self,
        scene: Scene,
        materials: Vec<Material>,
        base: Option<u64>,
        run: Option<&str>,
    ) -> Result<Value> {
        scene.validate(&self.project.limits)?;
        if base.is_some_and(|r| r != self.project.scene.revision) {
            bail!("REVISION_CONFLICT")
        }
        let previous = std::mem::replace(&mut self.project.materials, materials);
        match self.replace(scene, base, run) {
            Ok(v) => Ok(v),
            Err(e) => {
                self.project.materials = previous;
                Err(e)
            }
        }
    }
    pub fn update_materials(&mut self, materials: Vec<Material>) -> Result<bool> {
        if serde_json::to_value(&materials)? == serde_json::to_value(&self.project.materials)? {
            return Ok(false);
        };
        self.replace_with_materials(self.project.scene.clone(), materials, None, None)?;
        Ok(true)
    }
    pub fn persist(&mut self) -> Result<()> {
        let p = self.material_snapshot_path(self.project.scene.revision);
        if p.exists() {
            let existing: Vec<Material> = serde_json::from_slice(&fs::read(&p)?)?;
            if serde_json::to_value(existing)? != serde_json::to_value(&self.project.materials)? {
                bail!("不可在同一revision覆写材质目录")
            }
        } else {
            write_json(&p, &self.project.materials)?;
        }
        write_snapshot_once(
            &self.snapshot_path(self.project.scene.revision),
            &self.project.scene,
        )?;
        write_json(&self.root.join("project.json"), &self.project)?;
        Ok(())
    }
    pub fn event(&mut self, kind: &str, run: Option<&str>, data: Value) -> Result<Event> {
        use std::io::Write;
        let event = Event {
            seq: self.next_seq,
            time: now(),
            kind: kind.into(),
            run_id: run.map(str::to_owned),
            revision: self.project.scene.revision,
            data,
        };
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.root.join("events/events.jsonl"))?;
        serde_json::to_writer(&mut f, &event)?;
        f.write_all(b"\n")?;
        f.flush()?;
        self.next_seq += 1;
        self.events.push(event.clone());
        if self.events.len() > 5000 {
            self.events.drain(..1000);
        }
        Ok(event)
    }
    pub fn apply(
        &mut self,
        ops: &[Operation],
        base: Option<u64>,
        request: Option<&str>,
        run: Option<&str>,
    ) -> Result<Value> {
        if let Some(id) = request {
            if self.project.text_receipts.contains_key(id) {
                bail!("REQUEST_ID_CONFLICT：该requestId已经用于文字编辑")
            }
            if let Some(v) = self.requests.get(id) {
                return Ok(v.clone());
            }
        }
        if base.is_some_and(|r| r != self.project.scene.revision) {
            bail!("REVISION_CONFLICT：工程已被修改，请重新读取上下文")
        }
        let before = self.project.scene.clone();
        let start = std::time::Instant::now();
        let next = before.apply(ops, &self.project.limits)?;
        self.ensure_protected_groups_unchanged(&before, &next)?;
        self.undo.push(before.revision);
        self.undo = self
            .undo
            .iter()
            .rev()
            .take(128)
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        self.redo.clear();
        self.revisions.insert(next.revision, ());
        self.project.scene = next;
        self.persist()?;
        let value = json!({"ok":true,"revision":self.project.scene.revision,"stats":self.project.scene.stats(&self.project.limits),"changed":{"before":before.boxes.len(),"after":self.project.scene.boxes.len()},"generationMs":start.elapsed().as_secs_f64()*1000.});
        self.event("geometry.applied",run,json!({"baseRevision":before.revision,"requestId":request,"operations":ops,"result":value}))?;
        if let Some(id) = request {
            self.requests.insert(id.into(), value.clone());
        }
        Ok(value)
    }
    pub fn text_receipt(&self, id: &str, fingerprint: &str) -> Result<Option<Value>> {
        validate_text_receipt_key(id, fingerprint)?;
        let Some(receipt) = self.project.text_receipts.get(id) else {
            return Ok(None);
        };
        if receipt.request_id != id || receipt.request_fingerprint != fingerprint {
            bail!("REQUEST_ID_CONFLICT：同一requestId不能用于不同文字设置")
        }
        Ok(Some(serde_json::to_value(receipt)?))
    }
    #[allow(clippy::too_many_arguments)]
    pub fn apply_text(
        &mut self,
        ops: &[Operation],
        base: u64,
        id: &str,
        fingerprint: &str,
        font: &Value,
        layout: &Value,
        run: Option<&str>,
    ) -> Result<Value> {
        validate_text_receipt_key(id, fingerprint)?;
        if let Some(receipt) = self.text_receipt(id, fingerprint)? {
            self.requests.insert(id.to_owned(), receipt.clone());
            return Ok(receipt);
        }
        if self.requests.contains_key(id)
            || self.events.iter().any(|event| {
                matches!(event.kind.as_str(), "geometry.applied" | "text.applied")
                    && event.data["requestId"].as_str() == Some(id)
            })
        {
            bail!("REQUEST_ID_CONFLICT：该requestId已经用于其他编辑")
        }
        if base != self.project.scene.revision {
            bail!("REVISION_CONFLICT：工程已被修改，请重新读取上下文")
        }

        let before = self.project.scene.clone();
        let start = std::time::Instant::now();
        let next = before.apply(ops, &self.project.limits)?;
        self.ensure_protected_groups_unchanged(&before, &next)?;
        let stats = serde_json::to_value(next.stats(&self.project.limits))?;
        let receipt = TextReceipt {
            revision: next.revision,
            stats: stats.clone(),
            layout: layout.clone(),
            font: font.clone(),
            request_id: id.to_owned(),
            request_fingerprint: fingerprint.to_owned(),
        };

        let previous_undo = self.undo.clone();
        let previous_redo = self.redo.clone();
        let previous_revisions = self.revisions.clone();
        let attempted_revision = next.revision;
        let material_snapshot = self.material_snapshot_path(attempted_revision);
        let scene_snapshot = self.snapshot_path(attempted_revision);
        let material_snapshot_existed = material_snapshot.exists();
        let scene_snapshot_existed = scene_snapshot.exists();
        self.undo.push(before.revision);
        self.undo = self
            .undo
            .iter()
            .rev()
            .take(128)
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        self.redo.clear();
        self.revisions.insert(next.revision, ());
        self.project.scene = next;
        self.project
            .text_receipts
            .insert(id.to_owned(), receipt.clone());
        if let Err(error) = self.persist() {
            self.project.scene = before;
            self.project.text_receipts.remove(id);
            self.undo = previous_undo;
            self.redo = previous_redo;
            self.revisions = previous_revisions;
            if attempted_revision != self.project.scene.revision {
                if !material_snapshot_existed {
                    let _ = fs::remove_file(material_snapshot);
                }
                if !scene_snapshot_existed {
                    let _ = fs::remove_file(scene_snapshot);
                }
            }
            return Err(error);
        }

        let receipt_value = serde_json::to_value(&receipt)?;
        self.requests.insert(id.to_owned(), receipt_value.clone());
        let geometry_result = json!({
            "ok":true,
            "revision":receipt.revision,
            "stats":stats,
            "changed":{
                "before":before.boxes.len(),
                "after":self.project.scene.boxes.len(),
            },
            "generationMs":start.elapsed().as_secs_f64()*1000.,
        });
        self.event(
            "geometry.applied",
            run,
            json!({
                "baseRevision":before.revision,
                "requestId":id,
                "operations":ops,
                "result":geometry_result,
            }),
        )?;
        self.event(
            "text.applied",
            run,
            json!({
                "requestId":id,
                "requestFingerprint":fingerprint,
                "font":font,
                "layout":layout,
                "result":receipt_value,
                "baseRevision":base,
            }),
        )?;
        Ok(receipt_value)
    }
    pub fn replace(
        &mut self,
        mut scene: Scene,
        base: Option<u64>,
        run: Option<&str>,
    ) -> Result<Value> {
        if base.is_some_and(|r| r != self.project.scene.revision) {
            bail!("REVISION_CONFLICT")
        }
        scene.validate(&self.project.limits)?;
        self.ensure_protected_groups_unchanged(&self.project.scene, &scene)?;
        let before = self.project.scene.revision;
        scene.revision = before + 1;
        self.undo.push(before);
        self.redo.clear();
        self.revisions.insert(scene.revision, ());
        self.project.scene = scene;
        self.persist()?;
        self.event("geometry.replaced", run, json!({"baseRevision":before}))?;
        Ok(
            json!({"ok":true,"revision":self.project.scene.revision,"stats":self.project.scene.stats(&self.project.limits)}),
        )
    }
    pub fn undo(&mut self, redo: bool, to: Option<u64>) -> Result<Value> {
        let target = if let Some(v) = to {
            v
        } else if redo {
            *self.redo.last().context("没有可重做版本")?
        } else {
            *self.undo.last().context("没有可撤销版本")?
        };
        let scene = self.scene_at(target)?;
        self.ensure_protected_groups_unchanged(&self.project.scene, &scene)?;
        let current = self.project.scene.revision;
        let restored_materials = self.materials_at(target)?;
        if redo {
            self.redo.pop();
        } else {
            self.undo.pop();
        }
        self.project.materials = restored_materials;
        if redo {
            self.undo.push(current)
        } else {
            self.redo.push(current)
        }
        let mut next = scene;
        next.revision = current + 1;
        self.revisions.insert(next.revision, ());
        self.project.scene = next;
        self.persist()?;
        self.event("history.restored", None, json!({"sourceRevision":target}))?;
        Ok(json!({"ok":true,"revision":self.project.scene.revision}))
    }
    pub fn commit(
        &mut self,
        revision: Option<u64>,
        title: &str,
        run: Option<&str>,
    ) -> Result<Commit> {
        let rev = revision.unwrap_or(self.project.scene.revision);
        let scene = self.scene_at(rev)?;
        let commit = Commit {
            id: Uuid::new_v4().to_string(),
            revision: rev,
            title: title.into(),
            created_at: now(),
            scene_hash: hash(&serde_json::to_vec(&scene)?),
        };
        self.project.commits.push(commit.clone());
        self.persist()?;
        self.event("revision.committed", run, json!(&commit))?;
        Ok(commit)
    }
    pub fn artifact(&mut self, bytes: &[u8], extension: &str) -> Result<String> {
        if !["png", "jpg", "json", "js", "txt", "zip"].contains(&extension) {
            bail!("不支持的artifact类型")
        };
        let name = format!("{}.{}", hash(bytes), extension);
        let path = self.root.join("artifacts").join(&name);
        if !path.exists() {
            atomic(&path, bytes)?;
        }
        Ok(name)
    }
    pub fn library(&self) -> Result<Vec<Artwork>> {
        let mut values = vec![];
        for e in fs::read_dir(self.root.join("artworks"))? {
            let p = e?.path();
            if p.extension().is_some_and(|x| x == "json") {
                if let Ok(v) = serde_json::from_slice::<Value>(&fs::read(p)?) {
                    if let Ok(a) = serde_json::from_value::<Artwork>(v["artwork"].clone()) {
                        values.push(a)
                    }
                }
            }
        }
        values.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(values)
    }
    pub fn save_artwork(
        &mut self,
        title: &str,
        tags: Vec<String>,
        thumbnail: Option<String>,
        run: Option<&str>,
    ) -> Result<Artwork> {
        self.save_artwork_views(title, tags, thumbnail, vec![], run)
    }
    pub fn save_artwork_views(
        &mut self,
        title: &str,
        tags: Vec<String>,
        thumbnail: Option<String>,
        views: Vec<String>,
        run: Option<&str>,
    ) -> Result<Artwork> {
        if title.len() > 1024
            || tags.len() > 64
            || tags.iter().any(|s| s.len() > 128)
            || views.len() > 5
        {
            bail!("作品元数据超过保护值")
        }
        for name in thumbnail.iter().chain(views.iter()) {
            validate_id(name)?;
            if !self.root.join("artifacts").join(name).is_file() {
                bail!("作品图片artifact不存在")
            }
        }
        let stats = self.project.scene.stats(&self.project.limits);
        let artwork = Artwork {
            id: Uuid::new_v4().to_string(),
            title: title.into(),
            tags,
            created_at: now(),
            box_count: stats.box_count,
            voxel_count: stats.voxel_count,
            thumbnail: thumbnail.or_else(|| views.first().cloned()),
            views,
            source_run: run.map(str::to_owned),
        };
        write_json(
            &self
                .root
                .join("artworks")
                .join(format!("{}.json", artwork.id)),
            &json!({"artwork":artwork,"project":self.project,"recipe":self.recipes.get(&self.project.scene.revision)}),
        )?;
        self.event("artwork.saved", run, json!(&artwork))?;
        Ok(artwork)
    }
    pub fn load_artwork(&mut self, id: &str) -> Result<Value> {
        validate_id(id)?;
        let v: Value = serde_json::from_slice(&fs::read(
            self.root.join("artworks").join(format!("{id}.json")),
        )?)?;
        let p: Project = serde_json::from_value(v["project"].clone())?;
        let mut scene = p.scene;
        scene.revision = self.project.scene.revision + 1;
        scene.validate(&self.project.limits)?;
        self.ensure_protected_groups_unchanged(&self.project.scene, &scene)?;
        self.undo.push(self.project.scene.revision);
        self.redo.clear();
        self.project.scene = scene.clone();
        self.project.materials = p.materials;
        self.project.name = p.name;
        self.revisions.insert(scene.revision, ());
        self.persist()?;
        self.event(
            "artwork.loaded",
            None,
            json!({"sourceArtwork":id,"fork":true}),
        )?;
        Ok(json!({"ok":true,"revision":self.project.scene.revision}))
    }
    pub fn set_protected_groups(&mut self, groups: Vec<String>) -> Result<()> {
        if groups.len() > 256 {
            bail!("受保护分组不得超过256个")
        }
        for group in &groups {
            if group.is_empty() || group.len() > 128 {
                bail!("受保护分组名称须为1..128字节")
            }
        }
        let next: BTreeSet<String> = groups.into_iter().collect();
        if next == self.project.protected_groups {
            return Ok(());
        }

        let previous_groups = self.project.protected_groups.clone();
        let previous_revision = self.project.scene.revision;
        let next_revision = previous_revision
            .checked_add(1)
            .context("场景revision溢出")?;
        let revision_was_present = self.revisions.contains_key(&next_revision);
        let material_snapshot = self.material_snapshot_path(next_revision);
        let scene_snapshot = self.snapshot_path(next_revision);
        let material_snapshot_existed = material_snapshot.exists();
        let scene_snapshot_existed = scene_snapshot.exists();

        self.project.protected_groups = next;
        self.project.scene.revision = next_revision;
        self.revisions.insert(next_revision, ());
        if let Err(error) = self.persist() {
            self.project.protected_groups = previous_groups;
            self.project.scene.revision = previous_revision;
            if !revision_was_present {
                self.revisions.remove(&next_revision);
            }
            if !material_snapshot_existed {
                let _ = fs::remove_file(material_snapshot);
            }
            if !scene_snapshot_existed {
                let _ = fs::remove_file(scene_snapshot);
            }
            return Err(error);
        }
        self.event(
            "project.protected_groups",
            None,
            json!({
                "protectedGroups":self.project.protected_groups,
                "revision":self.project.scene.revision,
            }),
        )?;
        Ok(())
    }
    pub fn state(&self) -> Value {
        json!({"project":{"id":self.project.id,"name":self.project.name,"cellSize":self.project.scene.cell_size,"size":self.project.scene.size,"pinnedNotes":self.project.pinned_notes,"protectedGroups":self.project.protected_groups},"revision":self.project.scene.revision,"stats":self.project.scene.stats(&self.project.limits),"canUndo":!self.undo.is_empty(),"canRedo":!self.redo.is_empty(),"commits":self.project.commits,"eventSeq":self.next_seq-1})
    }
    fn ensure_protected_groups_unchanged(&self, before: &Scene, after: &Scene) -> Result<()> {
        if self.project.protected_groups.is_empty() {
            return Ok(());
        }
        let changed: Vec<_> = self
            .project
            .protected_groups
            .iter()
            .filter(|group| protected_boxes(before, group) != protected_boxes(after, group))
            .cloned()
            .collect();
        if !changed.is_empty() {
            bail!(
                "PROTECTED_GROUP_CONFLICT：受保护分组内容发生变化：{}",
                changed.join(", ")
            )
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ProtectedBox {
    id: u64,
    min: [i32; 3],
    size: [u32; 3],
    style_id: u32,
    tint_id: u8,
    motion_phase: u8,
    group: String,
}
fn protected_boxes(scene: &Scene, group: &str) -> Vec<ProtectedBox> {
    let mut boxes: Vec<_> = scene
        .boxes
        .iter()
        .filter(|item| item.group == group)
        .map(|item| ProtectedBox {
            id: item.id,
            min: item.min,
            size: item.size,
            style_id: item.style_id,
            tint_id: item.tint_id,
            motion_phase: item.motion_phase,
            group: item.group.clone(),
        })
        .collect();
    boxes.sort();
    boxes
}
fn validate_text_receipt_key(id: &str, fingerprint: &str) -> Result<()> {
    if id.is_empty() || id.len() > 256 {
        bail!("文字requestId须为1..256字节")
    }
    if fingerprint.is_empty() || fingerprint.len() > 256 {
        bail!("文字请求指纹须为1..256字节")
    }
    Ok(())
}
pub fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 200
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
    {
        bail!("无效资源ID")
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reopen_restores_snapshot_history_and_undo_stack() {
        let dir = tempfile::tempdir().unwrap();
        let mut workspace = Workspace::open(dir.path().to_path_buf()).unwrap();
        for _ in 0..2 {
            let revision = workspace.project.scene.revision;
            let scene = workspace.project.scene.clone();
            workspace.replace(scene, Some(revision), None).unwrap();
        }
        let project_id = workspace.project.id.clone();
        drop(workspace);
        let reopened = Workspace::open(dir.path().to_path_buf()).unwrap();
        assert_eq!(
            reopened.revisions.keys().copied().collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert!(reopened.state()["canUndo"].as_bool().unwrap());
        assert_eq!(reopened.scene_at(1).unwrap().revision, 1);
        assert!(dir
            .path()
            .join("snapshots")
            .join(format!("{project_id}-1.json"))
            .is_file());
    }

    #[test]
    fn same_revision_persist_preserves_snapshot_and_rejects_changed_scene() {
        let dir = tempfile::tempdir().unwrap();
        let mut workspace = Workspace::open(dir.path().to_path_buf()).unwrap();
        let snapshot = workspace.snapshot_path(workspace.project.scene.revision);
        let original = fs::read(&snapshot).unwrap();
        workspace.project.name = "重命名作品".into();
        workspace.persist().unwrap();
        assert_eq!(
            fs::read(&snapshot).unwrap(),
            original,
            "metadata-only persist must leave scene snapshot untouched"
        );
        workspace.project.scene.cell_size = 0.5;
        assert!(
            workspace.persist().is_err(),
            "different scene content must not overwrite an existing revision"
        );
        assert_eq!(fs::read(&snapshot).unwrap(), original);
    }
}
