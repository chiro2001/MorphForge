//! Versioned virtual recipe files; paths never grant host filesystem access.
use crate::store::{hash, write_json, Workspace};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
#[derive(Default, Serialize, Deserialize)]
struct Files {
    files: BTreeMap<String, String>,
}
fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 128
        && path.ends_with(".js")
        && path.split('/').all(|s| {
            !s.is_empty()
                && s != "."
                && s != ".."
                && s.chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'))
        })
}
fn storage_path(w: &Workspace) -> std::path::PathBuf {
    w.root
        .join("runs")
        .join(format!("{}-sources.json", hash(w.project.id.as_bytes())))
}
fn load(w: &Workspace) -> Result<Files> {
    let mut path = storage_path(w);
    if !path.exists() && uuid::Uuid::parse_str(&w.project.id).is_ok() {
        path = w
            .root
            .join("runs")
            .join(format!("{}-sources.json", w.project.id));
    }
    if !path.exists() {
        return Ok(Files::default());
    }
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}
fn summary(path: &str, code: &str) -> Value {
    json!({"path":path,"hash":hash(code.as_bytes()),"bytes":code.len()})
}
pub fn list(w: &Workspace) -> Result<Value> {
    let files = load(w)?;
    Ok(
        json!({"files":files.files.iter().map(|(path,code)|summary(path,code)).collect::<Vec<_>>(),"sourceHash":hash(&serde_json::to_vec(&files)?)}),
    )
}
pub fn action(w: &mut Workspace, args: &Value, run: Option<&str>) -> Result<Value> {
    let action = args["action"].as_str().unwrap_or("list");
    if action == "list" {
        return list(w);
    }
    let path = args["path"].as_str().context("需要虚拟文件path")?;
    if !valid_path(path) {
        bail!("SOURCE_PATH：只允许工程内的相对.js路径")
    }
    let mut files = load(w)?;
    if action == "read" {
        let content = files.files.get(path).context("源码文件不存在")?;
        return Ok(json!({"file":summary(path,content),"content":content}));
    }
    let base = args["baseRevision"]
        .as_u64()
        .context("写入源码需要baseRevision")?;
    if base != w.project.scene.revision {
        bail!("REVISION_CONFLICT")
    }
    if let Some(old) = files.files.get(path) {
        if action == "write" && args["content"].as_str() == Some(old) {
            return Ok(json!({"file":summary(path,old),"replayed":true}));
        }
        if args["expectedHash"].as_str() != Some(hash(old.as_bytes()).as_str()) {
            bail!("SOURCE_CONFLICT：expectedHash需要read/write返回的file.hash（当前{}），不是sourceHash；确认文件内容后再修改",hash(old.as_bytes()))
        }
    } else if args["expectedHash"].is_string() {
        bail!("SOURCE_CONFLICT：源码不存在")
    }
    let result = match action {
        "write" => {
            let content = args["content"].as_str().context("需要content")?;
            if content.len() > 131_072 {
                bail!("源码文件超过128KiB")
            }
            files.files.insert(path.into(), content.into());
            if files.files.len() > 32
                || files.files.values().map(String::len).sum::<usize>() > 524_288
            {
                bail!("源码工作区超过32文件/512KiB")
            };
            summary(path, content)
        }
        "patch" => {
            let edits = args["edits"].as_array().context("patch需要edits数组")?;
            if edits.is_empty() || edits.len() > 32 {
                bail!("patch需要1至32处编辑")
            }
            let mut content = files.files.get(path).context("源码文件不存在")?.clone();
            for edit in edits {
                let find = edit["find"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .context("patch.find不能为空")?;
                let replacement = edit["replace"]
                    .as_str()
                    .context("patch.replace需要字符串")?;
                let count = content.matches(find).count();
                if count == 0 || count > 1 && edit["all"] != true {
                    bail!("PATCH_MATCH：find匹配{count}处；需要唯一匹配或明确all=true")
                }
                content = if edit["all"] == true {
                    content.replace(find, replacement)
                } else {
                    content.replacen(find, replacement, 1)
                };
                if content.len() > 131072 {
                    bail!("源码文件超过128KiB")
                }
            }
            if files.files.values().map(String::len).sum::<usize>()
                - files.files.get(path).map_or(0, String::len)
                + content.len()
                > 524288
            {
                bail!("源码工作区超过512KiB")
            }
            let result = summary(path, &content);
            files.files.insert(path.into(), content);
            result
        }
        "delete" => {
            files.files.remove(path);
            json!({"path":path,"deleted":true})
        }
        _ => bail!("未知源码操作"),
    };
    let source_hash = hash(&serde_json::to_vec(&files)?);
    let snapshot = w.artifact(&serde_json::to_vec(&files)?, "json")?;
    write_json(&storage_path(w), &files)?;
    w.event(
        "source.changed",
        run,
        json!({"action":action,"file":result,"sourceHash":source_hash,"sourceSnapshot":snapshot}),
    )?;
    Ok(json!({"ok":true,"file":result,"sourceHash":source_hash}))
}
pub fn program(w: &Workspace, args: &Value) -> Result<(String, String)> {
    let files = load(w)?;
    let entry = args["path"].as_str().unwrap_or("main.js");
    if !valid_path(entry) || !files.files.contains_key(entry) {
        bail!("入口源码不存在")
    }
    let params = args.get("params").cloned().unwrap_or(json!({}));
    if !params.is_object() {
        bail!("params必须是对象")
    };
    if serde_json::to_vec(&params)?.len() > 16_384 {
        bail!("params超过16KiB")
    }
    let source_hash = hash(&serde_json::to_vec(&files)?);
    let factories = files
        .files
        .iter()
        .map(|(path, source)| {
            Ok(format!(
                "{}:function(mf,params,module,exports,require){{\n{}\n}}",
                serde_json::to_string(path)?,
                source
            ))
        })
        .collect::<Result<Vec<_>>>()?
        .join(",\n");
    let code = format!(
        r#"const modules={{{factories}}},params={params},cache={{}};
 function loadModule(request,from=''){{
  let name=request;if(name.startsWith('.'))name=from.split('/').slice(0,-1).concat(name.split('/')).join('/');
  const parts=[];for(const part of name.split('/')){{if(!part||part==='.')continue;if(part==='..'){{if(!parts.length)throw Error('Virtual module leaves workspace');parts.pop();}}else parts.push(part);}}
  name=parts.join('/');if(!name.endsWith('.js'))name+='.js';
  if(!Object.prototype.hasOwnProperty.call(modules,name))throw Error('Unknown virtual module: '+name);
  if(cache[name])return cache[name].exports;const module={{exports:{{}}}};cache[name]=module;
  modules[name](mf,params,module,module.exports,next=>loadModule(next,name));return module.exports;
 }}
 const entry=loadModule({entry});if(typeof entry==='function')entry(mf,params);else if(typeof entry.build==='function')entry.build(mf,params);
 "#,
        params = params,
        entry = serde_json::to_string(entry)?
    );
    Ok((code, source_hash))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn files_are_versioned_bounded_and_survive_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().into()).unwrap();
        let a=action(&mut w,&json!({"action":"write","path":"main.js","baseRevision":0,"content":"mf.box([0,0,0]);"}),None).unwrap();
        assert!(action(&mut w,&json!({"action":"write","path":"main.js","baseRevision":0,"content":"different","expectedHash":"stale"}),None).is_err());
        assert!(action(
            &mut w,
            &json!({"action":"write","path":"../escape.js","baseRevision":0,"content":"x"}),
            None
        )
        .is_err());
        let w = Workspace::open(dir.path().into()).unwrap();
        assert_eq!(list(&w).unwrap()["files"][0]["hash"], a["file"]["hash"]);
        assert!(program(&w, &json!({"path":"main.js"}))
            .unwrap()
            .0
            .contains("function loadModule"));
    }
    #[test]
    fn real_model_module_runs_without_dynamic_function_environment_panics() {
        std::thread::Builder::new().stack_size(16*1024*1024).spawn(||{
            let dir=tempfile::tempdir().unwrap();let mut w=Workspace::open(dir.path().into()).unwrap();
            let source=include_str!("../tests/fixtures/quality-r1-coral-whale-main.js");
            action(&mut w,&json!({"action":"write","path":"coral-whale/main.js","baseRevision":0,"content":source}),None).unwrap();
            let (code,_)=program(&w,&json!({"path":"coral-whale/main.js"})).unwrap();
            assert!(!code.contains("new Function("));
            let result=crate::recipe::worker(crate::recipe::WorkerInput{request:crate::recipe::RecipeRequest{code,seed:1,mode:"apply".into(),base_revision:Some(0),request_id:None,max_ops:100000,dry_run:false},styles:BTreeMap::new()}).unwrap();
            assert!(result["items"].as_array().unwrap().len()>100);
        }).unwrap().join().unwrap();
    }
    #[test]
    fn exact_patches_are_atomic_and_support_unicode_virtual_paths() {
        let dir = tempfile::tempdir().unwrap();
        let mut w = Workspace::open(dir.path().into()).unwrap();
        let saved=action(&mut w,&json!({"action":"write","path":"鲸鱼/main.js","baseRevision":0,"content":"const radius=10; const depth=10;"}),None).unwrap();
        let result=action(&mut w,&json!({"action":"patch","path":"鲸鱼/main.js","baseRevision":0,"expectedHash":saved["file"]["hash"],"edits":[{"find":"radius=10","replace":"radius=12"}]}),None).unwrap();
        assert_ne!(result["file"]["hash"], saved["file"]["hash"]);
        assert!(action(&mut w,&json!({"action":"patch","path":"鲸鱼/main.js","baseRevision":0,"expectedHash":result["file"]["hash"],"edits":[{"find":"depth=10","replace":"depth=3"},{"find":"missing","replace":"x"}]}),None).is_err());
        let current = action(
            &mut w,
            &json!({"action":"read","path":"鲸鱼/main.js"}),
            None,
        )
        .unwrap();
        assert!(current["content"].as_str().unwrap().contains("depth=10"));
    }
}
