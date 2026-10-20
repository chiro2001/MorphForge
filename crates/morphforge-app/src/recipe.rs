use anyhow::{bail, Context, Result};
use boa_engine::{Context as JsContext, Source};
use morphforge_core::text::VoxelTextRequest;
use morphforge_core::{Box3, Limits, Operation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipeRequest {
    pub code: String,
    #[serde(default = "seed_default")]
    pub seed: u32,
    #[serde(default = "mode_default")]
    pub mode: String,
    #[serde(default)]
    pub base_revision: Option<u64>,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default = "ops_default")]
    pub max_ops: usize,
    #[serde(default)]
    pub dry_run: bool,
}
fn seed_default() -> u32 {
    1
}
fn mode_default() -> String {
    "apply".into()
}
fn ops_default() -> usize {
    100000
}
#[derive(Serialize, Deserialize)]
pub struct WorkerInput {
    pub request: RecipeRequest,
    pub styles: BTreeMap<String, u32>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkerText {
    request: VoxelTextRequest,
    offset: [i32; 3],
    mirrors: [i32; 3],
}
const SDK: &str = include_str!("recipe_sdk.js");
pub const SDK_VERSION: &str = "morphforge-recipe/4";
pub fn worker(input: WorkerInput) -> Result<Value> {
    if input.request.code.len() > 1_000_000 || input.request.max_ops > 100000 {
        bail!("recipe资源预算越界")
    }
    let mut context = JsContext::default();
    context
        .runtime_limits_mut()
        .set_loop_iteration_limit(4_000_000);
    context.runtime_limits_mut().set_recursion_limit(128);
    let settings =
        json!({"seed":input.request.seed,"maxOps":input.request.max_ops,"styles":input.styles});
    let code = input.request.code.trim();
    let execute = if let Some(body) = code.strip_prefix("export default") {
        format!("const recipe=({body});recipe(mf);")
    } else {
        format!("(function(mf){{\n{code}\n;if(typeof build==='function')build(mf);}})(mf);")
    };
    let source=format!("(function(){{'use strict';const settings={settings};\n{SDK}\n{execute}\nreturn JSON.stringify(ops);}})()");
    let result = context.eval(Source::from_bytes(&source)).map_err(|e| {
        anyhow::anyhow!(
            "RECIPE_ERROR: {}",
            e.to_opaque(&mut context)
                .to_string(&mut context)
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_else(|_| e.to_string())
        )
    })?;
    let text = result
        .as_string()
        .context("recipe未返回操作")?
        .to_std_string_escaped();
    if text.len() > 32 * 1024 * 1024 {
        bail!("recipe输出超过32MiB")
    };
    let items: Vec<Value> = serde_json::from_str(&text).context("recipe输出校验失败")?;
    if items.len() > input.request.max_ops {
        bail!("操作数超限")
    };
    Ok(json!({"items":items}))
}
pub async fn execute(
    request: RecipeRequest,
    styles: BTreeMap<String, u32>,
) -> Result<Vec<Operation>> {
    let max_ops = request.max_ops;
    let exe = std::env::current_exe()?;
    let mut command = tokio::process::Command::new(exe);
    command.env_clear().env("MALLOC_ARENA_MAX", "2");
    for name in ["SystemRoot", "WINDIR", "TEMP", "TMP"] {
        if let Some(v) = std::env::var_os(name) {
            command.env(name, v);
        }
    }
    command
        .arg("recipe-worker")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    unsafe {
        command.pre_exec(|| {
            let limit = libc::rlimit {
                rlim_cur: 512 * 1024 * 1024,
                rlim_max: 512 * 1024 * 1024,
            };
            if libc::setrlimit(libc::RLIMIT_AS, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    #[cfg(windows)]
    let _job = WorkerJob::assign(child.id().context("worker pid")?)?;
    let mut input = child.stdin.take().context("worker stdin")?;
    let bytes = serde_json::to_vec(&WorkerInput { request, styles })?;
    input.write_all(&bytes).await?;
    drop(input);
    let out = child.stdout.take().context("worker stdout")?;
    let err = child.stderr.take().context("worker stderr")?;
    let mut bytes = vec![];
    let mut errors = vec![];
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let mut out = out.take(32 * 1024 * 1024 + 1);
        let mut err = err.take(65536);
        let (stdout, stderr) =
            tokio::join!(out.read_to_end(&mut bytes), err.read_to_end(&mut errors));
        stdout?;
        stderr?;
        let status = child.wait().await?;
        Ok::<_, anyhow::Error>(status)
    })
    .await;
    let status = match result {
        Ok(r) => r?,
        Err(_) => {
            let _ = child.kill().await;
            bail!("RECIPE_TIMEOUT: recipe超过10秒资源预算")
        }
    };
    if bytes.len() > 32 * 1024 * 1024 {
        bail!("RECIPE_OUTPUT_LIMIT")
    }
    if !status.success() && status.code() != Some(2) {
        bail!("RECIPE_ENGINE_FAULT：工作子进程异常退出 {status}；已保留源码，请修复执行器后再恢复。{}",String::from_utf8_lossy(&errors).chars().take(1000).collect::<String>());
    }
    let value: Value = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "recipe worker异常退出 {}: {}",
            status,
            String::from_utf8_lossy(&errors)
                .chars()
                .take(2000)
                .collect::<String>()
        )
    })?;
    if !status.success() || value.get("error").is_some() {
        bail!("{}", value["error"].as_str().unwrap_or("recipe worker失败"))
    }
    let items: Vec<Value> = serde_json::from_value(value["items"].clone())?;
    let has_text = items
        .iter()
        .any(|item| item.get("op").and_then(Value::as_str) == Some("text"));
    if has_text {
        let catalog = crate::fonts::shared_default()?;
        expand_items(items, max_ops, Some(catalog.as_ref()))
    } else {
        expand_items(items, max_ops, None)
    }
}

fn expand_items(
    items: Vec<Value>,
    max_ops: usize,
    catalog: Option<&crate::fonts::FontCatalog>,
) -> Result<Vec<Operation>> {
    let limits = Limits::default();
    let mut operations = Vec::new();
    for item in items {
        if item.get("op").and_then(Value::as_str) == Some("text") {
            let marker: WorkerText = serde_json::from_value(item).context("文字marker校验失败")?;
            if marker
                .offset
                .iter()
                .any(|value| value.unsigned_abs() > limits.max_coord as u32)
                || marker.mirrors.iter().any(|value| !matches!(value, -1 | 1))
            {
                bail!("文字marker的变换帧越界")
            }
            let catalog = catalog.context("Recipe文字展开缺少字体目录")?;
            // Text coordinates are canonical and all emitted boxes stay in the
            // non-negative bbox. The worker records the active translate/mirror
            // frame; applying it after rasterization gives the same transform
            // semantics as a single `mf.box` call of the resulting extent.
            let compiled = catalog.compile(&marker.request)?;
            for operation in compiled.operations {
                let Operation::Box { mut value } = operation else {
                    unreachable!("text compilation only emits boxes")
                };
                transform_box(&mut value, &marker.offset, marker.mirrors)?;
                value.validate(&limits)?;
                operations.push(Operation::Box { value });
            }
        } else {
            let operation: Operation = serde_json::from_value(item)?;
            operations.push(operation);
        }
        if operations.len() > max_ops {
            bail!("展开文字后操作数超限")
        }
    }
    Ok(operations)
}

fn transform_box(value: &mut Box3, offset: &[i32; 3], mirrors: [i32; 3]) -> Result<()> {
    for axis in 0..3 {
        let local = if mirrors[axis] == 1 {
            i64::from(value.min[axis])
        } else {
            -i64::from(value.min[axis]) - i64::from(value.size[axis])
        };
        value.min[axis] = i64::from(offset[axis])
            .checked_add(local)
            .and_then(|value| i32::try_from(value).ok())
            .context("文字变换坐标溢出")?;
    }
    Ok(())
}

#[cfg(windows)]
struct WorkerJob(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl WorkerJob {
    fn assign(pid: u32) -> Result<Self> {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::{JobObjects::*, Threading::*},
        };
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                bail!("无法创建recipe资源隔离Job")
            };
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags =
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_PROCESS_MEMORY;
            info.ProcessMemoryLimit = 512 * 1024 * 1024;
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
            {
                CloseHandle(job);
                bail!("无法设置recipe内存保护")
            };
            let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if process.is_null() {
                CloseHandle(job);
                bail!("无法访问recipe进程")
            };
            let ok = AssignProcessToJobObject(job, process);
            CloseHandle(process);
            if ok == 0 {
                CloseHandle(job);
                bail!("无法隔离recipe子进程")
            };
            Ok(Self(job))
        }
    }
}
#[cfg(windows)]
impl Drop for WorkerJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

// The owned Windows job handle may be closed from any thread.
#[cfg(windows)]
unsafe impl Send for WorkerJob {}

#[cfg(test)]
mod tests {
    use super::*;
    use morphforge_core::text::VoxelTextRequest;
    use std::collections::BTreeSet;

    #[test]
    fn invalid_computed_dimensions_identify_the_failed_geometry() {
        let error = worker(WorkerInput {
            request: RecipeRequest {
                code: "mf.group('tower',()=>mf.box([4,20,-3],[2,0,5]));".into(),
                seed: 1,
                mode: "apply".into(),
                base_revision: Some(0),
                request_id: None,
                max_ops: 10,
                dry_run: true,
            },
            styles: BTreeMap::new(),
        })
        .unwrap_err()
        .to_string();
        for detail in [
            "size must be positive",
            "op=box",
            "min=[4,20,-3]",
            "size=[2,0,5]",
            "group=tower",
        ] {
            assert!(
                error.contains(detail),
                "missing actionable detail {detail}: {error}"
            );
        }
    }

    fn worker_items(code: &str, max_ops: usize) -> Vec<Value> {
        let output = worker(WorkerInput {
            request: RecipeRequest {
                code: code.into(),
                seed: 1,
                mode: "apply".into(),
                base_revision: None,
                request_id: None,
                max_ops,
                dry_run: false,
            },
            styles: BTreeMap::from([("wood".into(), 7)]),
        })
        .unwrap();
        serde_json::from_value(output["items"].clone()).unwrap()
    }

    fn cells(operations: &[Operation]) -> BTreeSet<[i32; 3]> {
        let mut output = BTreeSet::new();
        for operation in operations {
            let Operation::Box { value } = operation else {
                continue;
            };
            let end = value.end();
            for x in value.min[0]..end[0] {
                for y in value.min[1]..end[1] {
                    for z in value.min[2]..end[2] {
                        output.insert([x, y, z]);
                    }
                }
            }
        }
        output
    }

    #[test]
    fn text_markers_preserve_order_materials_translation_and_mirror() {
        let catalog = crate::fonts::FontCatalog::load_builtin().unwrap();
        let items = worker_items(
            "mf.box([0,0,0],[1,1,1],{style:'wood'});\
             mf.translate([10,2,-4],()=>mf.mirror('x',()=>mf.text('F',[1,3,5],{style:'wood',depth:2,group:'label'})));",
            10000,
        );
        let operations = expand_items(items, 10000, Some(&catalog)).unwrap();
        assert!(matches!(operations[0], Operation::Box { .. }));
        assert!(operations
            .iter()
            .skip(1)
            .all(|operation| matches!(operation, Operation::Box { .. })));
        for operation in &operations[1..] {
            let Operation::Box { value } = operation else {
                unreachable!()
            };
            assert_eq!(value.style_id, 7);
            assert_eq!(value.group, "label");
            assert_eq!(value.size[2], 2);
            // Mirror around local x=0, then translate.
            assert!(value.min[0] <= 10);
            assert!(value.end()[0] <= 10);
            assert!(value.min[1] >= 1);
            assert!(value.min[2] >= 1);
        }

        let direct = catalog
            .compile(&VoxelTextRequest {
                text: "F".into(),
                origin: [1, 3, 5],
                depth: 2,
                group: "label".into(),
                style_id: 7,
                ..VoxelTextRequest::default()
            })
            .unwrap();
        let direct_cells = cells(&direct.operations);
        let transformed_cells = cells(&operations[1..]);
        assert_eq!(direct_cells.len(), transformed_cells.len());
        assert_eq!(direct_cells.len() as u64, direct.layout.voxel_count);
    }

    #[test]
    fn text_marker_rejects_bad_arguments_and_expanded_budget() {
        let catalog = crate::fonts::FontCatalog::load_builtin().unwrap();
        for code in [
            "mf.text('', [0,0,0]);",
            "mf.text('F', [0,0,0], {fontSize:3});",
            "mf.text('F', [0,0,0], {plane:'nope'});",
            "mf.text('F', [0,0,0], {facing:'left'});",
        ] {
            assert!(crate::recipe::worker(WorkerInput {
                request: RecipeRequest {
                    code: code.into(),
                    seed: 1,
                    mode: "apply".into(),
                    base_revision: None,
                    request_id: None,
                    max_ops: 100,
                    dry_run: false,
                },
                styles: BTreeMap::new(),
            })
            .is_err());
        }
        let items = worker_items("mf.text('MORPH',[0,0,0],{fontSize:24});", 100);
        assert!(expand_items(items.clone(), 5, Some(&catalog)).is_err());
        assert!(expand_items(
            worker_items("mf.text('\\u{10FFFF}',[0,0,0]);", 100),
            10000,
            Some(&catalog)
        )
        .is_err());
    }
}
