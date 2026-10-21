//! OSCQuery capability discovery only. Never uses execution counters or ACKs.
use crate::target::AvatarCatalog;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{path::Path, time::Duration};
fn number(tree: &Value, name: &str) -> Result<u32> {
    let v = tree["CONTENTS"][name]["VALUE"][0]
        .as_f64()
        .with_context(|| format!("CAPABILITY_MISSING：缺少不同步能力参数{name}"))?;
    if !v.is_finite() || v < 0. || v > 255. || v.fract() != 0. {
        bail!("能力参数{name}超出精确整数范围")
    }
    Ok(v as u32)
}
pub fn parse(tree: &Value) -> Result<Value> {
    let version = number(tree, "MF_InfoVersion")?;
    if version != 7 {
        bail!("UNSUPPORTED_AVATAR：当前Avatar未提供受支持的能力声明，协议版本{version}")
    }
    let flags = number(tree, "MF_InfoFeatures")?;
    if flags & !15 != 0 {
        bail!("UNSUPPORTED_AVATAR：能力位包含未知扩展")
    }
    let build = number(tree, "MF_InfoBuild0")?
        | (number(tree, "MF_InfoBuild1")? << 8)
        | (number(tree, "MF_InfoBuild2")? << 16);
    let mut bytes = Vec::new();
    for i in 0..32 {
        bytes.push(number(tree, &format!("MF_Catalog{i:02}"))? as u8);
    }
    let fingerprint = bytes[..32]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    Ok(
        json!({"protocol":"mf6","version":version,"region8":flags&1!=0,"surfaceMasks":flags&2!=0,"virtualLight":flags&4!=0,"layeredOnly":flags&8!=0,"textPanels":flags&16!=0,"placementControlsSeparated":flags&32!=0,"catalogHash":fingerprint,"build":build,"signature":format!("{version}:{flags}:{build}:{fingerprint}"),"executionAck":false}),
    )
}
pub fn native_ports() -> Result<Vec<u16>> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let script="$v=@(Get-Process VRChat -ErrorAction SilentlyContinue|ForEach-Object {$_.Id});if($v.Count){@(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue|Where-Object {$_.OwningProcess -in $v}|Select-Object -ExpandProperty LocalPort)|ConvertTo-Json -Compress}else{'[]'}";
        let out = std::process::Command::new("powershell.exe")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                script,
            ])
            .creation_flags(0x08000000)
            .output()?;
        if !out.status.success() {
            bail!("读取VRChat OSCQuery监听端口失败")
        }
        let value: Value = serde_json::from_slice(&out.stdout)?;
        let values = value.as_array().cloned().unwrap_or_else(|| vec![value]);
        Ok(values
            .iter()
            .filter_map(|v| v.as_u64().and_then(|n| u16::try_from(n).ok()))
            .collect())
    }
    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
}
pub async fn query(port: Option<u16>) -> Result<Value> {
    let ports = if let Some(p) = port {
        if p == 0 {
            bail!("OSCQuery端口不能为0")
        }
        vec![p]
    } else {
        tokio::task::spawn_blocking(native_ports).await??
    };
    let http = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()?;
    let mut last = None;
    for port in ports.into_iter().take(32) {
        let base = format!("http://127.0.0.1:{port}");
        let read=async {
            let host:Value=http.get(format!("{base}/?HOST_INFO")).send().await?.error_for_status()?.json().await?;
            if host["OSC_TRANSPORT"]!="UDP"||host["EXTENSIONS"]["VALUE"]!=true {bail!("不是支持VALUE的OSCQuery服务")}
            let one:Value=http.get(format!("{base}/avatar/parameters")).send().await?.error_for_status()?.json().await?;
            let info=parse(&one)?;
            let two:Value=http.get(format!("{base}/avatar/parameters")).send().await?.error_for_status()?.json().await?;
            if info!=parse(&two)?{bail!("Avatar能力切换中，请稍后重试")}
            Ok::<_,anyhow::Error>(json!({"status":"identified","port":port,"info":info,"localOnly":true,"remoteAcknowledged":false}))
        }.await;
        match read {
            Ok(v) => return Ok(v),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| anyhow::anyhow!("未发现VRChat OSCQuery；可在高级设置指定查询端口")))
}
pub fn cache(root: &Path, catalog: &AvatarCatalog) -> Result<()> {
    let path = root
        .join("known-avatar-catalogs")
        .join(format!("{}.json", catalog.identity_hash()));
    crate::store::write_json(&path, catalog)
}
pub fn lookup(root: &Path, hash: &str) -> Result<Option<AvatarCatalog>> {
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("无效目录指纹")
    }
    let path = root
        .join("known-avatar-catalogs")
        .join(format!("{hash}.json"));
    if !path.is_file() {
        return Ok(None);
    }
    let c = AvatarCatalog::from_bytes(&std::fs::read(path)?)?;
    if c.identity_hash() != hash {
        bail!("本地目录缓存内容与指纹不符")
    }
    Ok(Some(c))
}

/// Keep imported catalog textures beside the shared catalog registry so another
/// independent window can preview the same declared Avatar without reimporting.
pub fn import_assets(root: &Path, bytes: &[u8], catalog: &AvatarCatalog) -> Result<usize> {
    use std::{
        collections::BTreeSet,
        io::{Cursor, Read},
    };
    if !bytes.starts_with(b"PK") {
        return Ok(0);
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))?;
    let mut copied = 0;
    let mut total = 0u64;
    let textures: BTreeSet<_> = catalog.materials.iter().flat_map(|m| &m.textures).collect();
    for texture in textures {
        let name = texture
            .strip_prefix("/artifacts/")
            .context("目录贴图必须为artifact")?;
        crate::store::validate_id(name)?;
        if !name.ends_with(".png") {
            bail!("目录只接受PNG贴图")
        }
        let mut entry = zip
            .by_name(&format!("artifacts/{name}"))
            .with_context(|| format!("构建包缺少纹理{name}"))?;
        if entry.size() > 16 * 1024 * 1024 {
            bail!("单张纹理超过16MiB")
        }
        let mut data = Vec::new();
        entry
            .by_ref()
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut data)?;
        total = total.saturating_add(data.len() as u64);
        if total > 128 * 1024 * 1024 {
            bail!("目录贴图超过128MiB")
        }
        let hash = name.strip_suffix(".png").unwrap();
        if hash.len() != 64 || crate::store::hash(&data) != hash {
            bail!("目录贴图内容哈希不匹配")
        }
        let target = root.join("artifacts").join(name);
        if !target.is_file() {
            crate::store::atomic(&target, &data)?;
            copied += 1;
        }
    }
    cache(root, catalog)?;
    Ok(copied)
}
