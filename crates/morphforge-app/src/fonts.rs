//! Read-only font catalog and rasterization for voxel text.
//!
//! The catalog exposes stable IDs instead of filesystem paths. Built-in fonts
//! are embedded in the executable; system fonts are discovered from the
//! conventional Windows font directories (or the host font directories in
//! tests). Recipe workers never call this module: the parent process expands
//! their text markers after the isolated worker has returned.

use anyhow::{bail, Context, Result};
use fontdue::{Font, FontSettings};
use morphforge_core::text::{
    compile_text_mask, validate_text_request, TextAlign, TextCompilation, TextMask,
    VoxelTextRequest, MAX_RASTER_AREA,
};
use morphforge_core::{Limits, Operation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use ttf_parser::{name_id, Face};

const BUILTIN_ID: &str = "builtin:noto-sans-sc";
const BUILTIN_FONT: &[u8] = include_bytes!("../../../fonts/NotoSansSC-Regular.otf");
const BUILTIN_SHA256: &str = "faa6c9df652116dde789d351359f3d7e5d2285a2b2a1f04a2d7244df706d5ea9";
const MAX_FONT_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_SYSTEM_FONTS: usize = 1024;
const MAX_CACHED_FONTS: usize = 8;

struct ParsedFonts {
    entries: BTreeMap<String, Arc<Font>>,
    recent: VecDeque<String>,
}

impl ParsedFonts {
    fn get(&mut self, id: &str) -> Option<Arc<Font>> {
        let font = self.entries.get(id)?.clone();
        self.recent.retain(|key| key != id);
        self.recent.push_back(id.to_owned());
        Some(font)
    }

    fn insert(&mut self, id: String, font: Arc<Font>) {
        self.recent.retain(|key| key != &id);
        self.recent.push_back(id.clone());
        self.entries.insert(id, font);
        // Keep the built-in Chinese font hot; system-font browsing must not
        // retain the parsed outlines of up to 1024 catalog entries forever.
        while self.entries.len() > MAX_CACHED_FONTS {
            let Some(oldest) = self.recent.pop_front() else {
                break;
            };
            if oldest == BUILTIN_ID {
                self.recent.push_back(oldest);
            } else {
                self.entries.remove(&oldest);
            }
        }
    }
}

static SHARED: OnceLock<std::result::Result<Arc<FontCatalog>, String>> = OnceLock::new();

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingGlyphsError {
    pub characters: Vec<char>,
}

impl fmt::Display for MissingGlyphsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "FONT_MISSING_GLYPHS:")?;
        for (index, character) in self.characters.iter().enumerate() {
            if index != 0 {
                formatter.write_str(",")?;
            }
            write!(formatter, "U+{:04X}", *character as u32)?;
        }
        Ok(())
    }
}

impl std::error::Error for MissingGlyphsError {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FontDescriptor {
    pub id: String,
    pub family: String,
    pub style: String,
    pub source: String,
    pub sha256: String,
    pub face_index: u32,
    pub license: String,
}

#[derive(Clone, Debug)]
enum FontSource {
    Embedded(&'static [u8]),
    File(PathBuf),
}

#[derive(Clone, Debug)]
struct FontEntry {
    descriptor: FontDescriptor,
    source: FontSource,
}

/// Immutable catalog shared by HTTP previews, Harness text tools and Recipes.
pub struct FontCatalog {
    entries: Vec<FontEntry>,
    by_id: BTreeMap<String, usize>,
    fonts: Mutex<ParsedFonts>,
}

/// Load the process-wide read-only catalog once.
///
/// The result is cached even when loading fails, so callers do not repeatedly
/// scan and hash the same system font directories after a deterministic error.
pub fn shared_default() -> Result<Arc<FontCatalog>> {
    match SHARED.get_or_init(|| {
        FontCatalog::load_default()
            .map(Arc::new)
            .map_err(|error| format!("{error:#}"))
    }) {
        Ok(catalog) => Ok(catalog.clone()),
        Err(error) => bail!("字体目录加载失败：{error}"),
    }
}

impl FontCatalog {
    /// Embedded fonts only. This is deterministic and suitable for tests.
    pub fn load_builtin() -> Result<Self> {
        let descriptor = descriptor_from_bytes(
            BUILTIN_FONT,
            0,
            "builtin",
            "OFL-1.1",
            &hex_sha256(BUILTIN_FONT),
        )?;
        if descriptor.sha256 != BUILTIN_SHA256 {
            bail!(
                "内置字体校验失败：期望{}，实际{}",
                BUILTIN_SHA256,
                descriptor.sha256
            )
        }
        let entry = FontEntry {
            descriptor: FontDescriptor {
                id: BUILTIN_ID.into(),
                ..descriptor
            },
            source: FontSource::Embedded(BUILTIN_FONT),
        };
        let font = Font::from_bytes(
            BUILTIN_FONT,
            FontSettings {
                collection_index: 0,
                scale: 64.0,
                load_substitutions: false,
            },
        )
        .map_err(|error| anyhow::anyhow!("内置字体解析失败：{error}"))?;
        Ok(Self {
            entries: vec![entry],
            by_id: BTreeMap::from([(BUILTIN_ID.into(), 0)]),
            fonts: Mutex::new(ParsedFonts {
                entries: BTreeMap::from([(BUILTIN_ID.into(), Arc::new(font))]),
                recent: VecDeque::from([BUILTIN_ID.into()]),
            }),
        })
    }

    /// Built-in fonts plus read-only system font discovery.
    pub fn load_default() -> Result<Self> {
        let mut catalog = Self::load_builtin()?;
        catalog.add_system_directories(system_font_directories());
        Ok(catalog)
    }

    pub fn list(&self) -> Vec<FontDescriptor> {
        self.entries
            .iter()
            .map(|entry| entry.descriptor.clone())
            .collect()
    }

    pub fn descriptor(&self, font_id: &str) -> Option<&FontDescriptor> {
        self.by_id
            .get(font_id)
            .and_then(|index| self.entries.get(*index))
            .map(|entry| &entry.descriptor)
    }

    /// Rasterize text to the font-independent foreground mask used by core.
    pub fn render_mask(&self, request: &VoxelTextRequest) -> Result<TextMask> {
        validate_text_request(request, &Limits::default())?;
        let entry = self
            .by_id
            .get(&request.font_id)
            .and_then(|index| self.entries.get(*index))
            .with_context(|| format!("未知字体：{}", request.font_id))?;
        let font = self.font(entry)?;
        rasterize_text(&font, request)
    }

    /// Return canonical operations and exact layout statistics.
    pub fn compile(&self, request: &VoxelTextRequest) -> Result<TextCompilation> {
        let mask = self.render_mask(request)?;
        Ok(compile_text_mask(request, &mask, &Limits::default())?)
    }

    /// Stable preview contract for the shared editor and Harness.
    pub fn preview(&self, request: &VoxelTextRequest) -> Result<TextPreview> {
        let descriptor = self
            .descriptor(&request.font_id)
            .with_context(|| format!("未知字体：{}", request.font_id))?
            .clone();
        let compiled = self.compile(request)?;
        let request_fingerprint = request_fingerprint(request)?;
        Ok(TextPreview {
            ok: true,
            request_fingerprint,
            font: descriptor,
            layout: compiled.layout,
            operations: compiled.operations,
            warnings: vec![],
        })
    }

    fn add_system_directories(&mut self, directories: Vec<PathBuf>) {
        let mut seen_files = HashSet::new();
        let mut seen_faces = HashSet::new();
        let mut files = Vec::new();
        for directory in directories {
            collect_font_files(&directory, &mut files, 0);
        }
        files.sort();
        files.dedup();

        for path in files {
            if self.entries.len() > MAX_SYSTEM_FONTS {
                break;
            }
            let Ok(canonical) = path.canonicalize() else {
                continue;
            };
            if !seen_files.insert(canonical.clone()) {
                continue;
            }
            let Ok(metadata) = std::fs::metadata(&canonical) else {
                continue;
            };
            if metadata.len() == 0 || metadata.len() > MAX_FONT_FILE_BYTES {
                continue;
            }
            let bytes = match read_font_file(&canonical) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let sha256 = hex_sha256(&bytes);
            let face_count = ttf_parser::fonts_in_collection(&bytes).unwrap_or(1);
            for face_index in 0..face_count {
                if self.entries.len() > MAX_SYSTEM_FONTS {
                    break;
                }
                let Ok(descriptor) =
                    descriptor_from_bytes(&bytes, face_index, "system", "system", &sha256)
                else {
                    continue;
                };
                let id = descriptor.id.clone();
                if !seen_faces.insert(id.clone()) {
                    continue;
                }
                let next = self.entries.len();
                self.entries.push(FontEntry {
                    descriptor,
                    source: FontSource::File(canonical.clone()),
                });
                self.by_id.insert(id, next);
            }
        }
    }

    fn font(&self, entry: &FontEntry) -> Result<Arc<Font>> {
        if let Some(font) = self
            .fonts
            .lock()
            .map_err(|_| anyhow::anyhow!("字体缓存锁已损坏"))?
            .get(&entry.descriptor.id)
        {
            return Ok(font);
        }
        let bytes = entry
            .source
            .bytes()
            .with_context(|| format!("读取字体失败：{}", entry.descriptor.id))?;
        if hex_sha256(bytes.as_ref()) != entry.descriptor.sha256 {
            bail!("FONT_CHANGED：系统字体文件已改变，请重启工作台刷新字体目录")
        }
        let font = Arc::new(
            Font::from_bytes(
                bytes.as_ref(),
                FontSettings {
                    collection_index: entry.descriptor.face_index,
                    scale: 64.0,
                    load_substitutions: false,
                },
            )
            .map_err(|error| anyhow::anyhow!("字体解析失败：{error}"))?,
        );
        self.fonts
            .lock()
            .map_err(|_| anyhow::anyhow!("字体缓存锁已损坏"))?
            .insert(entry.descriptor.id.clone(), font.clone());
        Ok(font)
    }
}

impl FontSource {
    fn bytes(&self) -> Result<Cow<'static, [u8]>> {
        match self {
            Self::Embedded(bytes) => Ok(Cow::Borrowed(bytes)),
            Self::File(path) => Ok(Cow::Owned(read_font_file(path)?)),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPreview {
    pub ok: bool,
    /// SHA-256 of the canonical request JSON. Preview clients must only apply
    /// a result whose fingerprint still matches the current request.
    pub request_fingerprint: String,
    pub font: FontDescriptor,
    pub layout: morphforge_core::text::TextLayout,
    /// Canonical operation list, using the same `{op:"box",...}` shape as the
    /// Recipe and apply APIs.
    pub operations: Vec<Operation>,
    pub warnings: Vec<String>,
}

fn descriptor_from_bytes(
    bytes: &[u8],
    face_index: u32,
    source: &str,
    license: &str,
    sha256: &str,
) -> Result<FontDescriptor> {
    let face = Face::parse(bytes, face_index).context("字体face解析失败")?;
    let family = face_name(&face, name_id::TYPOGRAPHIC_FAMILY)
        .or_else(|| face_name(&face, name_id::FAMILY))
        .or_else(|| face_name(&face, name_id::FULL_NAME))
        .unwrap_or_else(|| format!("Font face {face_index}"));
    let style = face_name(&face, name_id::TYPOGRAPHIC_SUBFAMILY)
        .or_else(|| face_name(&face, name_id::SUBFAMILY))
        .unwrap_or_else(|| "Regular".into());
    Ok(FontDescriptor {
        id: format!("system:{}:{face_index}", &sha256[..16]),
        family,
        style,
        source: source.into(),
        sha256: sha256.into(),
        face_index,
        license: license.into(),
    })
}

fn face_name(face: &Face<'_>, id: u16) -> Option<String> {
    face.names()
        .into_iter()
        .find(|name| name.name_id == id && name.is_unicode())
        .and_then(|name| name.to_string())
        .filter(|name| !name.trim().is_empty())
}

/// Hash the normalized request without resolving a font or current material.
/// Successful retries must remain replayable after either dependency changes.
pub fn request_fingerprint(request: &VoxelTextRequest) -> Result<String> {
    Ok(hex_sha256(&serde_json::to_vec(request)?))
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read_font_file(path: &Path) -> Result<Vec<u8>> {
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("打开字库文件失败：{}", path.display()))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_FONT_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FONT_FILE_BYTES {
        bail!("字库文件超过64MiB")
    }
    Ok(bytes)
}

fn supported_font_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "ttf" | "otf" | "ttc"
            )
        })
}

fn collect_font_files(directory: &Path, files: &mut Vec<PathBuf>, depth: usize) {
    if depth > 8 || files.len() >= MAX_SYSTEM_FONTS * 4 {
        return;
    }
    let Ok(read_dir) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in read_dir.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            collect_font_files(&path, files, depth + 1);
        } else if file_type.is_file() && supported_font_file(&path) {
            files.push(path);
        }
    }
}

fn system_font_directories() -> Vec<PathBuf> {
    if let Some(value) = std::env::var_os("MORPHFORGE_FONT_DIRS") {
        return std::env::split_paths(&value).collect();
    }
    let mut directories = vec![];
    #[cfg(windows)]
    {
        if let Some(windows) = std::env::var_os("WINDIR") {
            directories.push(PathBuf::from(windows).join("Fonts"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            directories.push(PathBuf::from(local).join("Microsoft/Windows/Fonts"));
        }
    }
    #[cfg(not(windows))]
    {
        directories.push(PathBuf::from("/usr/share/fonts"));
        directories.push(PathBuf::from("/usr/local/share/fonts"));
        if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
            directories.push(PathBuf::from(data).join("fonts"));
        } else if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            directories.push(home.join(".local/share/fonts"));
            directories.push(home.join(".fonts"));
        }
    }
    directories
}

#[derive(Clone, Debug)]
struct Line {
    chars: Vec<char>,
    advance: f32,
}

fn rasterize_text(font: &Font, request: &VoxelTextRequest) -> Result<TextMask> {
    let normalized = normalize_text(&request.text);
    let mut missing = Vec::new();
    let mut missing_seen = HashSet::new();
    for character in normalized.chars() {
        if character.is_whitespace() {
            continue;
        }
        if !font.has_glyph(character) && missing_seen.insert(character) {
            missing.push(character);
        }
    }
    if !missing.is_empty() {
        return Err(MissingGlyphsError {
            characters: missing,
        }
        .into());
    }

    let px = request.font_size as f32;
    let lines = layout_lines(font, &normalized, px, request)?;
    let line_metrics = font.horizontal_line_metrics(px);
    let ascent = line_metrics
        .map(|metrics| metrics.ascent.ceil())
        .unwrap_or(px * 0.8)
        .max(1.0) as i32;
    let line_size = line_metrics
        .map(|metrics| metrics.new_line_size.ceil())
        .unwrap_or(px)
        .max(1.0);
    let line_height = line_size.round() as i32 + request.line_spacing;
    if line_height <= 0 {
        bail!("lineSpacing使行高小于1")
    }
    let estimated_width = lines
        .iter()
        .map(|line| line.advance.ceil().max(0.0) as u64)
        .max()
        .unwrap_or(0)
        .saturating_add(px.ceil() as u64 * 2);
    let estimated_height = (lines.len() as u64)
        .saturating_mul(line_height as u64)
        .saturating_add(px.ceil() as u64 * 2);
    if estimated_width.saturating_mul(estimated_height) > MAX_RASTER_AREA {
        bail!("文字栅格超过{MAX_RASTER_AREA}像素预算")
    }
    let block_width = request
        .max_width
        .map(|width| width as f32)
        .unwrap_or_else(|| lines.iter().map(|line| line.advance).fold(0.0, f32::max));

    struct Glyph {
        x: i32,
        y: i32,
        width: usize,
        height: usize,
        bitmap: Vec<u8>,
    }
    let mut glyphs = Vec::new();
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for (line_index, line) in lines.iter().enumerate() {
        let align_offset = match request.align {
            TextAlign::Left => 0.0,
            TextAlign::Center => (block_width - line.advance) * 0.5,
            TextAlign::Right => block_width - line.advance,
        };
        let mut pen_x = align_offset;
        let mut previous = None;
        let baseline = line_index as i32 * line_height + ascent;
        for character in &line.chars {
            if let Some(previous) = previous {
                pen_x += font
                    .horizontal_kern(previous, *character, px)
                    .unwrap_or(0.0)
                    + request.letter_spacing as f32;
            }
            let (metrics, bitmap) = font.rasterize(*character, px);
            let left = (pen_x + metrics.xmin as f32).round() as i32;
            let top = baseline - metrics.ymin - metrics.height as i32;
            if metrics.width > 0 && metrics.height > 0 {
                min_x = min_x.min(left);
                min_y = min_y.min(top);
                max_x = max_x.max(left + metrics.width as i32);
                max_y = max_y.max(top + metrics.height as i32);
                glyphs.push(Glyph {
                    x: left,
                    y: top,
                    width: metrics.width,
                    height: metrics.height,
                    bitmap,
                });
            }
            pen_x += metrics.advance_width;
            previous = Some(*character);
        }
    }
    if glyphs.is_empty() {
        bail!("文字没有可见前景")
    }
    let width = (max_x - min_x) as u32;
    let height = (max_y - min_y) as u32;
    if width as u64 * height as u64 > MAX_RASTER_AREA {
        bail!("文字栅格超过{MAX_RASTER_AREA}像素预算")
    }
    let mut coverage = vec![0u8; width as usize * height as usize];
    for glyph in glyphs {
        let x0 = glyph.x - min_x;
        let y0 = glyph.y - min_y;
        for row in 0..glyph.height {
            let destination = (y0 as usize + row) * width as usize + x0 as usize;
            let source = row * glyph.width;
            for column in 0..glyph.width {
                let target = &mut coverage[destination + column];
                *target = (*target).max(glyph.bitmap[source + column]);
            }
        }
    }
    let trimmed = trim_mask(width, height, coverage);
    if trimmed.0 == 0 || trimmed.1 == 0 {
        bail!("文字没有可见前景")
    }
    Ok(TextMask::new(trimmed.0, trimmed.1, trimmed.2)?)
}

fn normalize_text(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\r' => {
                if characters.peek() == Some(&'\n') {
                    characters.next();
                }
                normalized.push('\n');
            }
            '\t' => normalized.push_str("    "),
            _ => normalized.push(character),
        }
    }
    normalized
}

fn layout_lines(font: &Font, text: &str, px: f32, request: &VoxelTextRequest) -> Result<Vec<Line>> {
    let mut lines = Vec::new();
    for explicit in text.split('\n') {
        let chars: Vec<char> = explicit.chars().collect();
        if chars.is_empty() {
            lines.push(Line {
                chars: vec![],
                advance: 0.0,
            });
            continue;
        }
        let mut current = Vec::new();
        let mut current_advance = 0.0f32;
        for character in chars {
            let candidate = if let Some(previous) = current.last() {
                current_advance
                    + font
                        .horizontal_kern(*previous, character, px)
                        .unwrap_or(0.0)
                    + request.letter_spacing as f32
                    + font.metrics(character, px).advance_width
            } else {
                font.metrics(character, px).advance_width
            };
            if !candidate.is_finite() || candidate <= 0.0 {
                bail!("letterSpacing或字体步进产生无效行宽")
            }
            if request
                .max_width
                .is_some_and(|width| candidate.ceil() > width as f32)
            {
                if current.is_empty() {
                    if character.is_whitespace() {
                        continue;
                    }
                    bail!("单个字形超过maxWidth")
                }
                lines.push(Line {
                    chars: std::mem::take(&mut current),
                    advance: current_advance,
                });
                current_advance = 0.0;
                if character.is_whitespace() {
                    continue;
                }
                let alone = font.metrics(character, px).advance_width;
                if !alone.is_finite() || alone <= 0.0 {
                    bail!("单个字形的步进无效")
                }
                if request
                    .max_width
                    .is_some_and(|width| alone.ceil() > width as f32)
                {
                    bail!("单个字形超过maxWidth")
                }
                current_advance = alone;
                current.push(character);
            } else {
                current_advance = candidate;
                current.push(character);
            }
        }
        lines.push(Line {
            chars: current,
            advance: current_advance,
        });
    }
    if lines.iter().all(|line| line.chars.is_empty())
        || lines.iter().map(|line| line.advance).fold(0.0, f32::max) <= 0.0
    {
        bail!("文字没有可排版的可见内容")
    }
    Ok(lines)
}

fn trim_mask(width: u32, height: u32, coverage: Vec<u8>) -> (u32, u32, Vec<u8>) {
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    for y in 0..height {
        for x in 0..width {
            if coverage[y as usize * width as usize + x as usize] > 127 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x + 1);
                max_y = max_y.max(y + 1);
            }
        }
    }
    if min_x == width || min_y == height {
        return (0, 0, Vec::new());
    }
    let new_width = max_x - min_x;
    let new_height = max_y - min_y;
    let mut trimmed = vec![0u8; new_width as usize * new_height as usize];
    for y in 0..new_height {
        let source = (min_y + y) as usize * width as usize + min_x as usize;
        let destination = y as usize * new_width as usize;
        trimmed[destination..destination + new_width as usize]
            .copy_from_slice(&coverage[source..source + new_width as usize]);
    }
    (new_width, new_height, trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use morphforge_core::text::{compile_text_mask, TextAlign, TextFacing, TextPlane};
    use std::collections::BTreeSet;

    fn request(text: &str) -> VoxelTextRequest {
        VoxelTextRequest {
            text: text.into(),
            font_size: 20,
            ..VoxelTextRequest::default()
        }
    }

    fn occupied(compiled: &TextCompilation) -> BTreeSet<[i32; 3]> {
        let mut cells = BTreeSet::new();
        for operation in &compiled.operations {
            let Operation::Box { value } = operation else {
                panic!("text must emit boxes")
            };
            let end = value.end();
            for x in value.min[0]..end[0] {
                for y in value.min[1]..end[1] {
                    for z in value.min[2]..end[2] {
                        cells.insert([x, y, z]);
                    }
                }
            }
        }
        cells
    }

    #[test]
    fn builtin_catalog_has_stable_identity_and_renders_cjk() {
        let catalog = FontCatalog::load_builtin().unwrap();
        let descriptor = catalog.descriptor(BUILTIN_ID).unwrap();
        assert_eq!(descriptor.sha256, BUILTIN_SHA256);
        assert_eq!(descriptor.face_index, 0);
        assert!(descriptor.family.contains("Noto"));
        let preview = catalog.preview(&request("MorphForge 中文")).unwrap();
        assert!(preview.layout.voxel_count > 100);
        assert!(!preview.operations.is_empty());
        assert_eq!(preview.request_fingerprint.len(), 64);
    }

    #[test]
    fn mask_and_boxes_are_identical_after_plane_transform() {
        let catalog = FontCatalog::load_builtin().unwrap();
        let request = VoxelTextRequest {
            origin: [2, 3, 5],
            plane: TextPlane::Xz,
            facing: TextFacing::Negative,
            depth: 2,
            ..request("F")
        };
        let mask = catalog.render_mask(&request).unwrap();
        let compiled = compile_text_mask(&request, &mask, &Limits::default()).unwrap();
        let cells = occupied(&compiled);
        let expected = mask
            .coverage
            .iter()
            .enumerate()
            .filter(|(_, coverage)| **coverage > 127)
            .count() as u64
            * request.depth as u64;
        assert_eq!(cells.len() as u64, expected);
        assert_eq!(compiled.layout.voxel_count, expected);
    }

    #[test]
    fn positive_facing_matches_the_renderer_view_basis() {
        let catalog = FontCatalog::load_builtin().unwrap();
        let request = request("F/R 中文");
        let mask = catalog.render_mask(&request).unwrap();
        let expected: BTreeSet<(i32, i32)> = mask
            .coverage
            .iter()
            .enumerate()
            .filter(|(_, value)| **value > 127)
            .map(|(index, _)| {
                (
                    (index % mask.width as usize) as i32,
                    (index / mask.width as usize) as i32,
                )
            })
            .collect();

        for plane in [TextPlane::Xy, TextPlane::Xz, TextPlane::Yz] {
            let request = VoxelTextRequest {
                plane,
                ..request.clone()
            };
            let compiled = catalog.compile(&request).unwrap();
            let mut projected = BTreeSet::new();
            for operation in &compiled.operations {
                let Operation::Box { value } = operation else {
                    unreachable!()
                };
                let end = value.end();
                for x in value.min[0]..end[0] {
                    for y in value.min[1]..end[1] {
                        for z in value.min[2]..end[2] {
                            projected.insert(match plane {
                                // Project onto (screen right, screen down). The
                                // mask itself is top-left-origin, row-major.
                                // Camera from +Z: right=-X, up=+Y.
                                TextPlane::Xy => (-x, -y),
                                // Camera from +Y: right=-X, up=-Z.
                                TextPlane::Xz => (-x, z),
                                // Camera from +X: right=+Z, up=+Y.
                                TextPlane::Yz => (z, -y),
                            });
                        }
                    }
                }
            }
            let min_x = projected.iter().map(|cell| cell.0).min().unwrap();
            let min_y = projected.iter().map(|cell| cell.1).min().unwrap();
            let normalized: BTreeSet<_> = projected
                .into_iter()
                .map(|(x, y)| (x - min_x, y - min_y))
                .collect();
            assert_eq!(normalized, expected, "wrong orientation for {plane:?}");
        }
    }

    #[test]
    fn wrapping_alignment_spacing_and_missing_glyphs_are_deterministic() {
        let catalog = FontCatalog::load_builtin().unwrap();
        let wrapped = VoxelTextRequest {
            text: "MORPH FORGE".into(),
            font_size: 20,
            max_width: Some(50),
            align: TextAlign::Right,
            letter_spacing: 1,
            line_spacing: 2,
            ..VoxelTextRequest::default()
        };
        let first = catalog.render_mask(&wrapped).unwrap();
        let second = catalog.render_mask(&wrapped).unwrap();
        assert_eq!(first.coverage, second.coverage);
        assert!(first.height > 25);
        let fingerprint = catalog.preview(&wrapped).unwrap().request_fingerprint;
        assert_eq!(fingerprint.len(), 64);
        assert_ne!(
            fingerprint,
            catalog
                .preview(&VoxelTextRequest {
                    align: TextAlign::Left,
                    ..wrapped.clone()
                })
                .unwrap()
                .request_fingerprint
        );

        let compact = catalog.render_mask(&request("III")).unwrap();
        let spaced = catalog
            .render_mask(&VoxelTextRequest {
                letter_spacing: 4,
                ..request("III")
            })
            .unwrap();
        assert!(spaced.width > compact.width + 5);
        let single = catalog.render_mask(&request("I\nI")).unwrap();
        let with_line_spacing = catalog
            .render_mask(&VoxelTextRequest {
                line_spacing: 8,
                ..request("I\nI")
            })
            .unwrap();
        assert!(with_line_spacing.height >= single.height + 6);

        let reference = catalog.render_mask(&request("I\nWWW")).unwrap();
        let reference_lines = line_boxes(&reference);
        assert_eq!(reference_lines.len(), 2);
        for (align, edge) in [
            (TextAlign::Left, LineEdge::Left),
            (TextAlign::Right, LineEdge::Right),
        ] {
            let mask = catalog
                .render_mask(&VoxelTextRequest {
                    align,
                    ..request("I\nWWW")
                })
                .unwrap();
            let lines = line_boxes(&mask);
            assert_eq!(lines.len(), 2);
            let values = lines
                .iter()
                .map(|(min_x, max_x, _, _)| match edge {
                    LineEdge::Left => *min_x,
                    LineEdge::Right => *max_x,
                })
                .collect::<Vec<_>>();
            assert!(
                values[0].abs_diff(values[1]) <= 3,
                "alignment {align:?} did not share its {edge:?} edge: {values:?}"
            );
        }

        let mut missing = request("MorphForge \u{10FFFF}");
        let error = catalog.render_mask(&missing).unwrap_err();
        let missing_error = error
            .downcast_ref::<MissingGlyphsError>()
            .expect("typed missing glyph error");
        assert_eq!(missing_error.characters, vec!['\u{10FFFF}']);
        missing.text = "F\nR\n中".into();
        assert!(catalog.render_mask(&missing).is_ok());
    }

    #[derive(Clone, Copy, Debug)]
    enum LineEdge {
        Left,
        Right,
    }

    fn line_boxes(mask: &TextMask) -> Vec<(u32, u32, u32, u32)> {
        let rows: Vec<bool> = (0..mask.height)
            .map(|y| {
                (0..mask.width)
                    .any(|x| mask.coverage[y as usize * mask.width as usize + x as usize] > 127)
            })
            .collect();
        let mut output = vec![];
        let mut start = None;
        for (y, occupied) in rows
            .iter()
            .copied()
            .chain(std::iter::once(false))
            .enumerate()
        {
            match (start, occupied) {
                (None, true) => start = Some(y),
                (Some(top), false) => {
                    let bottom = y as u32;
                    let mut min_x = mask.width;
                    let mut max_x = 0;
                    for row in top as u32..bottom {
                        for x in 0..mask.width {
                            if mask.coverage[row as usize * mask.width as usize + x as usize] > 127
                            {
                                min_x = min_x.min(x);
                                max_x = max_x.max(x);
                            }
                        }
                    }
                    output.push((min_x, max_x, top as u32, bottom));
                    start = None;
                }
                _ => {}
            }
        }
        output
    }
}
