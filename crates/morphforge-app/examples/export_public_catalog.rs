//! Reproduce the distributable catalog textures; refuses non-original assets.
use anyhow::{bail, Context, Result};
use morphforge_app::{builtin_pixels, store::Workspace};
use serde_json::Value;
use std::{collections::BTreeSet, path::PathBuf};
fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let catalog_path = PathBuf::from(args.next().context("catalog.json required")?);
    let output = PathBuf::from(args.next().context("output directory required")?);
    if output.exists() {
        bail!("Output must be a new isolated directory");
    }
    let mut catalog: Value = serde_json::from_slice(&std::fs::read(catalog_path)?)?;
    let manifest = catalog.as_object_mut().context("catalog object required")?;
    // Public modules belong to the installer's Avatar, not the private source
    // Avatar/project used to develop the catalog. These are not hash inputs.
    for field in ["targetAvatarId", "sourceProject", "sourceRevision"] {
        manifest.remove(field);
    }
    let mut workspace = Workspace::open(output.join(".generation"))?;
    let mut original = BTreeSet::new();
    for theme in ["pixel-workshop", "block-workshop"] {
        let (materials, _, _) = builtin_pixels::prepare_theme(&mut workspace, theme, &[])?;
        for material in materials {
            original.extend(material.textures.into_iter());
        }
    }
    let textures: BTreeSet<_> = catalog["materials"]
        .as_array()
        .context("materials missing")?
        .iter()
        .flat_map(|m| m["textures"].as_array().into_iter().flatten())
        .map(|v| v.as_str().context("texture path"))
        .collect::<Result<_>>()?;
    for texture in &textures {
        if !original.contains(*texture) {
            bail!("Catalog contains non-original texture {texture}");
        }
    }
    std::fs::create_dir_all(output.join("artifacts"))?;
    for texture in &textures {
        let name = texture
            .strip_prefix("/artifacts/")
            .context("artifact path required")?;
        morphforge_app::store::validate_id(name)?;
        std::fs::copy(
            workspace.root.join("artifacts").join(name),
            output.join("artifacts").join(name),
        )?;
    }
    std::fs::write(
        output.join("catalog.json"),
        serde_json::to_vec_pretty(&catalog)?,
    )?;
    std::fs::write(output.join("LICENSE.txt"), builtin_pixels::LICENSE)?;
    std::fs::remove_dir_all(output.join(".generation"))?;
    println!("Exported {} verified original textures", textures.len());
    Ok(())
}
