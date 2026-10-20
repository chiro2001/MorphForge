//! Portable, static surface exports. Authoring data remains the project JSON.
use anyhow::{bail, Context, Result};
use morphforge_core::{materials::Material, Scene};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::{BTreeMap, HashMap},
    io::{Cursor, Write},
    path::Path,
};
use zip::{write::SimpleFileOptions, ZipWriter};

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    pub format: String,
    pub cell_size: Option<f64>,
}
pub struct Export {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
    pub extension: &'static str,
}
#[derive(Clone)]
struct Surface {
    name: String,
    color: [f32; 3],
    metallic: f32,
    roughness: f32,
    opacity: f32,
    emission: f32,
    mode: String,
    double: bool,
    image: Option<usize>,
    clamp: bool,
}
#[derive(Clone)]
struct Face {
    positions: [[f32; 3]; 3],
    normal: [f32; 3],
    uv: [[f32; 2]; 3],
    tint: [f32; 3],
    material: usize,
    group: String,
}
struct Mesh {
    faces: Vec<Face>,
    materials: Vec<Surface>,
    images: Vec<Vec<u8>>,
    cell_size: f64,
}
fn linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
fn srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}
fn normal(p: &[[f32; 3]; 3]) -> Option<[f32; 3]> {
    let u: [f32; 3] = std::array::from_fn(|a| p[1][a] - p[0][a]);
    let v: [f32; 3] = std::array::from_fn(|a| p[2][a] - p[0][a]);
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let len = n.iter().map(|x| x * x).sum::<f32>().sqrt();
    if len < 1e-20 {
        None
    } else {
        Some(n.map(|x| x / len))
    }
}
fn rotate(p: [f64; 3], code: u8) -> [f64; 3] {
    let [mut x, y, mut z] = p;
    match code % 4 {
        1 => (x, z) = (z, -x),
        2 => (x, z) = (-x, -z),
        3 => (x, z) = (-z, x),
        _ => {}
    }
    match code / 4 {
        1 => [y, -x, z],
        2 => [x, z, -y],
        3 => [x, -z, y],
        4 => [-x, -y, z],
        5 => [-y, x, z],
        _ => [x, y, z],
    }
}
fn texture(root: &Path, url: &str) -> Result<Vec<u8>> {
    let name = url
        .strip_prefix("/artifacts/")
        .context("导出贴图必须为本地artifact")?;
    if name.len() != 68
        || !name.ends_with(".png")
        || !name[..64].bytes().all(|c| c.is_ascii_hexdigit())
    {
        bail!("贴图路径无效")
    }
    let bytes = std::fs::read(root.join("artifacts").join(name)).context("导出需要的贴图不存在")?;
    if bytes.len() > 16 * 1024 * 1024 {
        bail!("贴图文件过大")
    }
    let reader = image::ImageReader::new(Cursor::new(&bytes)).with_guessed_format()?;
    let (w, h) = reader.into_dimensions()?;
    if w == 0 || h < w || w > 4096 || w as u64 * h as u64 > 16_777_216 {
        bail!("贴图尺寸超过导出保护范围")
    }
    let image = image::load_from_memory(&bytes)?.crop_imm(0, 0, w, w);
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png)?;
    Ok(out.into_inner())
}
fn build(scene: &Scene, materials: &[Material], root: &Path, cell_size: f64) -> Result<Mesh> {
    if !cell_size.is_finite() || !(0.000001..=1000.).contains(&cell_size) {
        bail!("每格米数须为0.000001至1000")
    }
    if scene.boxes.len().saturating_mul(12) + scene.triangles.len() > 500_000 {
        bail!("导出最多500,000三角面；请拆分大型作品")
    }
    let styles: HashMap<_, _> = materials.iter().map(|m| (m.id, m)).collect();
    let mut mesh = Mesh {
        faces: vec![],
        materials: vec![],
        images: vec![],
        cell_size,
    };
    let mut surface_ids = HashMap::new();
    let mut image_ids = HashMap::<String, usize>::new();
    let corners = [
        [0., 0., 0.],
        [1., 0., 0.],
        [1., 1., 0.],
        [0., 1., 0.],
        [0., 0., 1.],
        [1., 0., 1.],
        [1., 1., 1.],
        [0., 1., 1.],
    ];
    let sides = [
        [1, 2, 6, 5],
        [0, 4, 7, 3],
        [3, 7, 6, 2],
        [0, 1, 5, 4],
        [4, 5, 6, 7],
        [0, 3, 2, 1],
    ];
    let normals = [
        [1., 0., 0.],
        [-1., 0., 0.],
        [0., 1., 0.],
        [0., -1., 0.],
        [0., 0., 1.],
        [0., 0., -1.],
    ];
    for b in &scene.boxes {
        let m = styles.get(&b.style_id).context("作品使用了不存在的材质")?;
        let positions: Vec<[f32; 3]> = corners
            .iter()
            .map(|p| {
                let mut p = *p;
                let height = match b.shape {
                    1 => p[0],
                    2 => 1. - p[0],
                    3 => p[2],
                    4 => 1. - p[2],
                    _ => 1.,
                };
                p[1] *= height;
                std::array::from_fn(|a| {
                    ((b.min[a] as f64 + p[a] * b.size[a] as f64)
                        * cell_size
                        * if a == 2 { -1. } else { 1. }) as f32
                })
            })
            .collect();
        let tint = if b.tint_id == 0 {
            [1.; 3]
        } else {
            [0.31, 0.53, 0.79].map(|v| 0.45 + 0.55 * ((b.tint_id as f32 * v) % 1.))
        };
        for (side, quad) in sides.iter().enumerate() {
            let source_normal = rotate(normals[side], m.texture_rotation);
            let axis = if source_normal[0].abs() > 0.5 {
                0
            } else if source_normal[1].abs() > 0.5 {
                1
            } else {
                2
            };
            let face = axis * 2 + usize::from(source_normal[axis] < 0.);
            let key = (m.id, if m.textures.is_empty() { 0 } else { face });
            let material = if let Some(id) = surface_ids.get(&key) {
                *id
            } else {
                let image = if m.textures.is_empty() {
                    None
                } else {
                    let url = &m.textures[face % m.textures.len()];
                    Some(if let Some(id) = image_ids.get(url) {
                        *id
                    } else {
                        let bytes = texture(root, url)?;
                        let id = mesh.images.len();
                        mesh.images.push(bytes);
                        image_ids.insert(url.clone(), id);
                        id
                    })
                };
                let id = mesh.materials.len();
                mesh.materials.push(Surface {
                    name: format!("{} · {}", m.name, face),
                    color: m.color,
                    metallic: m.metallic,
                    roughness: m.roughness,
                    opacity: m.opacity,
                    emission: m.emission,
                    mode: m.render_mode.clone(),
                    double: m.opacity < 1.,
                    image,
                    clamp: m.texture_scale < 0.5,
                });
                surface_ids.insert(key, id);
                id
            };
            let dims = rotate(b.size.map(|x| x as f64), m.texture_rotation).map(f64::abs);
            let uv = |i: usize| {
                let p = rotate(corners[i].map(|v| v - 0.5), m.texture_rotation).map(|v| v + 0.5);
                let axes = if axis == 0 {
                    [2, 1]
                } else if axis == 1 {
                    [0, 2]
                } else {
                    [0, 1]
                };
                let u = axes.map(|a| {
                    (if m.texture_scale < 0.5 {
                        p[a]
                    } else {
                        p[a] * dims[a] / m.texture_scale as f64
                    }) as f32
                });
                [u[0], 1. - u[1]]
            };
            for tri in [[quad[0], quad[2], quad[1]], [quad[0], quad[3], quad[2]]] {
                let p = tri.map(|i| positions[i]);
                if let Some(n) = normal(&p) {
                    mesh.faces.push(Face {
                        positions: p,
                        normal: n,
                        uv: tri.map(uv),
                        tint,
                        material,
                        group: b.group.clone(),
                    });
                }
            }
        }
    }
    let mut triangle_materials = HashMap::new();
    for t in &scene.triangles {
        let material = *triangle_materials.entry(t.double_sided).or_insert_with(|| {
            let id = mesh.materials.len();
            mesh.materials.push(Surface {
                name: format!("纯色三角面{}", if t.double_sided { " · 双面" } else { "" }),
                color: [1.; 3],
                metallic: 0.,
                roughness: 0.85,
                opacity: 1.,
                emission: 0.,
                mode: "opaque".into(),
                double: t.double_sided,
                image: None,
                clamp: false,
            });
            id
        });
        let positions = [0, 2, 1].map(|i| {
            std::array::from_fn(|a| {
                (t.vertices[i][a] as f64 * cell_size * if a == 2 { -1. } else { 1. }) as f32
            })
        });
        if let Some(n) = normal(&positions) {
            mesh.faces.push(Face {
                positions,
                normal: n,
                uv: [[0.; 2]; 3],
                tint: t.color.map(|x| linear(x as f32 / 255.)),
                material,
                group: t.group.clone(),
            });
        }
    }
    if mesh.faces.is_empty() {
        bail!("作品为空或没有可导出的有效面")
    }
    Ok(mesh)
}
fn append_aligned(buffer: &mut Vec<u8>, data: &[u8]) -> (usize, usize) {
    while !buffer.len().is_multiple_of(4) {
        buffer.push(0)
    }
    let offset = buffer.len();
    buffer.extend_from_slice(data);
    (offset, data.len())
}
fn floats(values: impl IntoIterator<Item = f32>) -> Vec<u8> {
    values.into_iter().flat_map(f32::to_le_bytes).collect()
}
fn glb(mesh: &Mesh) -> Result<Vec<u8>> {
    let mut bin = Vec::new();
    let mut views = vec![];
    let mut accessors = vec![];
    let mut nodes = vec![];
    let mut meshes = vec![];
    let mut batches = BTreeMap::<(&str, usize), Vec<&Face>>::new();
    for face in &mesh.faces {
        batches
            .entry((&face.group, face.material))
            .or_default()
            .push(face);
    }
    for ((group, material), faces) in batches {
        let mut attrs = serde_json::Map::new();
        let positions = faces.iter().flat_map(|f| f.positions).collect::<Vec<_>>();
        let min: [f32; 3] =
            std::array::from_fn(|a| positions.iter().map(|p| p[a]).fold(f32::INFINITY, f32::min));
        let max: [f32; 3] = std::array::from_fn(|a| {
            positions
                .iter()
                .map(|p| p[a])
                .fold(f32::NEG_INFINITY, f32::max)
        });
        for (name, ty, values) in [
            (
                "POSITION",
                "VEC3",
                floats(positions.iter().flatten().copied()),
            ),
            (
                "NORMAL",
                "VEC3",
                floats(faces.iter().flat_map(|f| [f.normal; 3]).flatten()),
            ),
            (
                "TEXCOORD_0",
                "VEC2",
                floats(faces.iter().flat_map(|f| f.uv).flatten()),
            ),
            (
                "COLOR_0",
                "VEC3",
                floats(faces.iter().flat_map(|f| [f.tint; 3]).flatten()),
            ),
        ] {
            if name == "TEXCOORD_0" && mesh.materials[material].image.is_none() {
                continue;
            }
            let (offset, length) = append_aligned(&mut bin, &values);
            let view = views.len();
            views.push(json!({"buffer":0,"byteOffset":offset,"byteLength":length,"target":34962}));
            let accessor = accessors.len();
            let mut data =
                json!({"bufferView":view,"componentType":5126,"count":positions.len(),"type":ty});
            if name == "POSITION" {
                data["min"] = json!(min);
                data["max"] = json!(max);
            }
            accessors.push(data);
            attrs.insert(name.into(), json!(accessor));
        }
        let id = meshes.len();
        meshes.push(
            json!({"name":group,"primitives":[{"attributes":attrs,"material":material,"mode":4}]}),
        );
        nodes.push(json!({"name":group,"mesh":id}));
    }
    let mut images = vec![];
    for image in &mesh.images {
        let (offset, length) = append_aligned(&mut bin, image);
        let id = views.len();
        views.push(json!({"buffer":0,"byteOffset":offset,"byteLength":length}));
        images.push(json!({"bufferView":id,"mimeType":"image/png"}));
    }
    let mut textures = vec![];
    let mut mats = vec![];
    let mut emission_extension = false;
    for m in &mesh.materials {
        let mut pbr = json!({"baseColorFactor":[linear(m.color[0]),linear(m.color[1]),linear(m.color[2]),m.opacity],"metallicFactor":m.metallic,"roughnessFactor":m.roughness});
        if let Some(image) = m.image {
            let id = textures.len();
            textures.push(json!({"sampler":usize::from(m.clamp),"source":image}));
            pbr["baseColorTexture"] = json!({"index":id});
        }
        let mut v = json!({"name":m.name,"pbrMetallicRoughness":pbr,"doubleSided":m.double,"alphaMode":if m.mode=="cutout"{"MASK"}else if m.opacity<1.||m.mode=="transparent"{"BLEND"}else{"OPAQUE"},"alphaCutoff":0.5});
        if m.mode != "cutout" {
            v.as_object_mut().unwrap().remove("alphaCutoff");
        }
        if m.emission > 0. {
            v["emissiveFactor"] = json!(m.color.map(|x| linear(x) * m.emission.min(1.)));
            if m.emission > 1. {
                emission_extension = true;
                v["extensions"] =
                    json!({"KHR_materials_emissive_strength":{"emissiveStrength":m.emission}});
            }
        }
        mats.push(v);
    }
    let mut document = json!({"asset":{"version":"2.0","generator":"MorphForge"},"scene":0,"scenes":[{"nodes":(0..nodes.len()).collect::<Vec<_>>()}],"nodes":nodes,"meshes":meshes,"materials":mats,"buffers":[{"byteLength":bin.len()}],"bufferViews":views,"accessors":accessors,"extras":{"cellSizeMeters":mesh.cell_size,"note":"Static surface export; animated textures use first frame. Procedural materials use base color. Independent shells and internal faces are retained."}});
    if !images.is_empty() {
        document["images"] = json!(images);
        document["textures"] = json!(textures);
        document["samplers"] = json!([{"magFilter":9728,"minFilter":9728,"wrapS":10497,"wrapT":10497},{"magFilter":9728,"minFilter":9728,"wrapS":33071,"wrapT":33071}]);
    }
    if emission_extension {
        document["extensionsUsed"] = json!(["KHR_materials_emissive_strength"]);
    }
    let mut text = serde_json::to_vec(&document)?;
    while !text.len().is_multiple_of(4) {
        text.push(b' ')
    }
    while !bin.len().is_multiple_of(4) {
        bin.push(0)
    }
    let length = 12 + 8 + text.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(length);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(length as u32).to_le_bytes());
    out.extend_from_slice(&(text.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend(text);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend(bin);
    Ok(out)
}
fn archive(files: Vec<(String, Vec<u8>)>) -> Result<Vec<u8>> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        zip.start_file(
            name,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        )?;
        zip.write_all(&bytes)?;
    }
    Ok(zip.finish()?.into_inner())
}
fn obj(mesh: &Mesh) -> Result<Vec<u8>> {
    let mut obj = String::from("# MorphForge; right-handed, Y-up, meters\nmtllib model.mtl\n");
    let mut mtl = String::new();
    let mut ids = HashMap::new();
    let mut group = String::new();
    for (i, face) in mesh.faces.iter().enumerate() {
        if group != face.group {
            group = face.group.clone();
            obj.push_str(&format!(
                "g {}\n",
                group
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() || c == '_' {
                        c
                    } else {
                        '_'
                    })
                    .collect::<String>()
            ));
        }
        let m = &mesh.materials[face.material];
        let color: [f32; 3] = std::array::from_fn(|a| srgb(linear(m.color[a]) * face.tint[a]));
        let key = (face.material, color.map(f32::to_bits));
        let next = ids.len();
        let id = *ids.entry(key).or_insert_with(|| {
            let d = if m.image.is_some() {
                "# Nearest sampling; first animation frame\n"
            } else {
                ""
            };
            mtl.push_str(&format!(
                "newmtl material_{next}\nKd {} {} {}\nd {}\nPr {}\nPm {}\n{d}",
                color[0], color[1], color[2], m.opacity, m.roughness, m.metallic
            ));
            if let Some(image) = m.image {
                mtl.push_str(&format!(
                    "map_Kd -clamp {} textures/image_{image}.png\n",
                    if m.clamp { "on" } else { "off" }
                ));
            }
            mtl.push('\n');
            next
        });
        obj.push_str(&format!("usemtl material_{id}\n"));
        for p in face.positions {
            obj.push_str(&format!("v {} {} {}\n", p[0], p[1], p[2]));
        }
        for uv in face.uv {
            obj.push_str(&format!("vt {} {}\n", uv[0], 1. - uv[1]));
        }
        obj.push_str(&format!(
            "vn {} {} {}\n",
            face.normal[0], face.normal[1], face.normal[2]
        ));
        let n = i * 3 + 1;
        let norm = i + 1;
        obj.push_str(&format!(
            "f {n}/{n}/{norm} {}/{}/{norm} {}/{}/{norm}\n",
            n + 1,
            n + 1,
            n + 2,
            n + 2
        ));
    }
    let mut files=vec![("model.obj".into(),obj.into_bytes()),("model.mtl".into(),mtl.into_bytes()),("README.txt".into(),"MorphForge静态导出。右手坐标，Y向上，单位米；保留独立壳体和内部面，未做布尔并集。图片纹理首帧；程序纹理使用基础色。OBJ的透明/PBR支持取决于导入器。STL需要自行准备封闭实体。\n".as_bytes().to_vec())];
    for (i, image) in mesh.images.iter().enumerate() {
        files.push((format!("textures/image_{i}.png"), image.clone()));
    }
    archive(files)
}
fn ply(mesh: &Mesh) -> Result<Vec<u8>> {
    let mut out=format!("ply\nformat binary_little_endian 1.0\ncomment MorphForge meters Y-up; static base colors\nelement vertex {}\nproperty float x\nproperty float y\nproperty float z\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nproperty uchar alpha\nelement face {}\nproperty list uchar uint vertex_indices\nend_header\n",mesh.faces.len()*3,mesh.faces.len()).into_bytes();
    let averages: Vec<[f32; 3]> = mesh
        .images
        .iter()
        .map(|bytes| {
            let image = image::load_from_memory(bytes)
                .expect("validated PNG")
                .to_rgba8();
            let mut sums = [0f64; 3];
            for p in image.pixels() {
                for a in 0..3 {
                    sums[a] += linear(p[a] as f32 / 255.) as f64;
                }
            }
            std::array::from_fn(|a| (sums[a] / image.pixels().len() as f64) as f32)
        })
        .collect();
    for face in &mesh.faces {
        let m = &mesh.materials[face.material];
        let color: [u8; 3] = std::array::from_fn(|a| {
            (srgb(linear(m.color[a]) * face.tint[a] * m.image.map_or(1., |id| averages[id][a]))
                .clamp(0., 1.)
                * 255.)
                .round() as u8
        });
        for p in face.positions {
            out.extend(floats(p));
            out.extend(color);
            out.push((m.opacity * 255.).round() as u8);
        }
    }
    for i in 0..mesh.faces.len() {
        out.push(3);
        for j in 0..3 {
            out.extend_from_slice(&((i * 3 + j) as u32).to_le_bytes());
        }
    }
    Ok(out)
}
fn stl(mesh: &Mesh) -> Vec<u8> {
    let mut out = vec![0u8; 80];
    let name = b"MorphForge binary STL; units millimeters; geometry only, independent shells";
    out[..name.len()].copy_from_slice(name);
    out.extend_from_slice(&(mesh.faces.len() as u32).to_le_bytes());
    for f in &mesh.faces {
        out.extend(floats(f.normal));
        for p in f.positions {
            out.extend(floats(p.map(|v| v * 1000.)));
        }
        out.extend_from_slice(&0u16.to_le_bytes());
    }
    out
}
pub fn export(
    scene: &Scene,
    materials: &[Material],
    root: &Path,
    options: &Options,
) -> Result<Export> {
    if !matches!(options.format.as_str(), "glb" | "obj" | "ply" | "stl") {
        bail!("支持glb、obj、ply、stl导出")
    }
    let mesh = build(
        scene,
        materials,
        root,
        options.cell_size.unwrap_or(scene.cell_size),
    )?;
    let (bytes, mime, extension) = match options.format.as_str() {
        "glb" => (glb(&mesh)?, "model/gltf-binary", "glb"),
        "obj" => (obj(&mesh)?, "application/zip", "obj.zip"),
        "ply" => (ply(&mesh)?, "application/octet-stream", "ply"),
        _ => (stl(&mesh), "model/stl", "stl"),
    };
    Ok(Export {
        bytes,
        mime,
        extension,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use morphforge_core::{materials::builtins, Box3, Triangle3};
    fn scene(shape: u8) -> Scene {
        let mut s = Scene::default();
        let mut b = Box3::new([-3, 2, -7], [5, 6, 8], 1);
        b.shape = shape;
        b.group = "模型 / 主体".into();
        s.boxes.push(b);
        s
    }
    fn document(bytes: &[u8]) -> Value {
        assert_eq!(&bytes[..4], b"glTF");
        assert_eq!(
            u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize,
            bytes.len()
        );
        let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        serde_json::from_slice(&bytes[20..20 + length]).unwrap()
    }
    use serde_json::Value;
    #[test]
    fn box_and_slopes_have_outward_non_degenerate_faces() {
        let dir = tempfile::tempdir().unwrap();
        for shape in 0..=4 {
            let mesh = build(&scene(shape), &builtins(), dir.path(), 0.25).unwrap();
            assert_eq!(mesh.faces.len(), if shape == 0 { 12 } else { 8 });
            let volume: f32 = mesh
                .faces
                .iter()
                .map(|f| {
                    let [a, b, c] = f.positions;
                    (a[0] * (b[1] * c[2] - b[2] * c[1])
                        + a[1] * (b[2] * c[0] - b[0] * c[2])
                        + a[2] * (b[0] * c[1] - b[1] * c[0]))
                        / 6.
                })
                .sum();
            assert!(volume > 0.);
            for f in mesh.faces {
                assert!((f.normal.iter().map(|v| v * v).sum::<f32>() - 1.).abs() < 0.0001);
            }
        }
    }
    #[test]
    fn formats_preserve_geometry_units_and_color() {
        let dir = tempfile::tempdir().unwrap();
        let mut scene = scene(0);
        scene.triangles.push(Triangle3 {
            id: 99,
            vertices: [[0, 0, 0], [4, 0, 0], [0, 4, 0]],
            color: [12, 98, 201],
            double_sided: true,
            group: "彩色面".into(),
        });
        let mesh = build(&scene, &builtins(), dir.path(), 0.01).unwrap();
        assert_eq!(mesh.faces.len(), 13);
        let bin = glb(&mesh).unwrap();
        let d = document(&bin);
        assert_eq!(d["asset"]["version"], "2.0");
        assert_eq!(d["extras"]["cellSizeMeters"], 0.01);
        assert!(d["materials"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["doubleSided"] == true));
        assert_eq!(mesh.faces.last().unwrap().normal, [0., 0., -1.]);
        let stl = stl(&mesh);
        assert_eq!(stl.len(), 84 + 50 * 13);
        assert_eq!(u32::from_le_bytes(stl[80..84].try_into().unwrap()), 13);
        let ply = ply(&mesh).unwrap();
        let header = String::from_utf8_lossy(&ply[..400]);
        assert!(header.contains("element vertex 39"));
        assert!(header.contains("element face 13"));
        let obj = obj(&mesh).unwrap();
        let mut zip = zip::ZipArchive::new(Cursor::new(obj)).unwrap();
        assert!(zip.by_name("model.mtl").is_ok());
        let mut text = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("model.obj").unwrap(), &mut text).unwrap();
        assert_eq!(
            text.lines().filter(|line| line.starts_with("f ")).count(),
            13
        );
    }
    #[test]
    fn png_animation_is_static_and_embedded_without_external_paths() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("artifacts")).unwrap();
        let image = image::RgbaImage::from_fn(2, 4, |_, y| {
            if y < 2 {
                image::Rgba([255, 0, 0, 255])
            } else {
                image::Rgba([0, 0, 255, 255])
            }
        });
        let name = format!("{}.png", "a".repeat(64));
        image
            .save(dir.path().join("artifacts").join(&name))
            .unwrap();
        let mut materials = builtins();
        materials[0].textures = vec![format!("/artifacts/{name}"); 6];
        let mesh = build(&scene(0), &materials, dir.path(), 1.).unwrap();
        assert_eq!(mesh.images.len(), 1);
        let tex = image::load_from_memory(&mesh.images[0]).unwrap().to_rgba8();
        assert_eq!(tex.height(), 2);
        assert_eq!(tex.get_pixel(0, 0).0, [255, 0, 0, 255]);
        let d = document(&glb(&mesh).unwrap());
        assert_eq!(d["images"][0]["mimeType"], "image/png");
        assert!(d["images"][0].get("uri").is_none());
        assert!(texture(dir.path(), "/artifacts/../../secret.png").is_err());
    }
    #[test]
    fn export_rejects_invalid_limits_and_empty_scene() {
        let dir = tempfile::tempdir().unwrap();
        for n in [f64::NAN, 0., -1., 1001.] {
            assert!(build(&scene(0), &builtins(), dir.path(), n).is_err());
        }
        assert!(build(&Scene::default(), &builtins(), dir.path(), 1.).is_err());
    }
}
