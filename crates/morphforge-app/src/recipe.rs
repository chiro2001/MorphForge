use anyhow::{bail, Context, Result};
use boa_engine::{Context as JsContext, Source};
use morphforge_core::Operation;
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
const SDK: &str = include_str!("recipe_sdk.js");
pub fn worker(input: WorkerInput) -> Result<Value> {
    if input.request.code.len() > 1_000_000 || input.request.max_ops > 100000 {
        bail!("recipe资源预算越界")
    }
    let mut context = JsContext::default();
    context
        .runtime_limits_mut()
        .set_loop_iteration_limit(1_000_000);
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
    let operations: Vec<Operation> = serde_json::from_str(&text).context("recipe输出校验失败")?;
    if operations.len() > input.request.max_ops {
        bail!("操作数超限")
    };
    Ok(json!({"operations":operations}))
}
pub async fn execute(
    request: RecipeRequest,
    styles: BTreeMap<String, u32>,
) -> Result<Vec<Operation>> {
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
    Ok(serde_json::from_value(value["operations"].clone())?)
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
