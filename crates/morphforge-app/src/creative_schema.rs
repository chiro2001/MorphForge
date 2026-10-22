//! Static tool schemas: assets are dynamic tool results, never injected into system.
use serde_json::{json, Value};
fn vector(kind: &str, n: usize) -> Value {
    json!({"type":"array","items":{"type":kind},"minItems":n,"maxItems":n})
}
fn tool(name: &str, description: &str, properties: Value) -> Value {
    json!({"type":"function","name":name,"description":description,"parameters":{"type":"object","properties":properties,"required":["action"],"additionalProperties":false}})
}
pub fn tools(en: bool) -> Vec<Value> {
    let point = vector("number", 3);
    let grid = vector("integer", 3);
    let key = json!({"type":"object","properties":{"time":{"type":"number","minimum":0},"translation":point,"rotation":point,"scale":point},"required":["time","translation","rotation","scale"],"additionalProperties":false});
    let track = json!({"type":"object","properties":{"group":{"type":"string"},"pivot":point,"interpolation":{"type":"string","enum":["linear","step"]},"keys":{"type":"array","items":key,"minItems":1,"maxItems":256}},"required":["group","pivot","interpolation","keys"],"additionalProperties":false});
    vec![
        tool(
            "components",
            if en {
                "Reusable project components. list returns metadata only; inspect returns one component's local geometry. save selects existing ids and/or groups at baseRevision, storing geometry about anchor (default bottom center). save/remove require baseCreativeRevision. place requires id, instances, baseRevision; requestId is auto-injected for model calls. Each instance has offset XYZ, positive integer scale 1..64, turns (quarter-turns about Y), optional mirror x/y/z, destination group, optional box styleId. Geometry expands into ordinary editable boxes/triangles; no OSC bandwidth savings. dryRun validates capacity and protected groups without mutation. Reuse components within the same conversation; assets persist with the project. Example save id='window', groups=['window']; then place id='window', instances=[{offset:[8,4,0],group:'window-2'}]."
            } else {
                "项目可复用组件：list只返回摘要，inspect按id读取局部几何。save用ids和/或groups保存当前已绘制部分，需要baseRevision；anchor默认地面中心，组件坐标相对此原点。save/remove需baseCreativeRevision。place用id+instances+baseRevision放置，模型调用自动注入requestId；每个instance含offset整数XYZ、group目标分组，可选scale整数1..64、turns绕Y四分之一圈次数、mirror=x/y/z、styleId替换方块材质。先dryRun检查容量/保护分组，再实际放置；支持撤销和重试幂等。保存一次可在本次对话反复调用，项目重开仍保留。实例展开为普通体素/面，不减少OSC字节。例：先save id='window',groups=['window']；再place id='window',instances=[{offset:[8,4,0],group:'window-2'}]。"
            },
            json!({
                "action":{"type":"string","enum":["list","inspect","save","remove","place"]},"id":{"type":"string"},"name":{"type":"string"},"description":{"type":"string"},"baseRevision":{"type":"integer"},"baseCreativeRevision":{"type":"integer"},"projectId":{"type":"string"},"requestId":{"type":"string"},"dryRun":{"type":"boolean"},"ids":{"type":"array","items":{"type":"integer"}},"groups":{"type":"array","items":{"type":"string"}},"anchor":grid,
                "instances":{"type":"array","minItems":1,"maxItems":128,"items":{"type":"object","properties":{"offset":grid,"group":{"type":"string"},"turns":{"type":"integer"},"scale":{"type":"integer","minimum":1,"maximum":64},"mirror":{"type":"string","enum":["x","y","z"]},"styleId":{"type":"integer"}},"required":["offset","group"],"additionalProperties":false}}
            }),
        ),
        tool(
            "reference_workbench",
            if en {
                "Named reference/camera alignment. list returns bookmarks. save/remove require id and baseCreativeRevision; bookmark={name,shot,comparison?}. shot uses capture_views custom camera fields; id is replaced by the bookmark ID (ASCII letters/digits/-/_). comparison={imageId,mode:'split'|'overlay',opacity:0..1,zoom:0.1..10,offset:[normalizedX,normalizedY],crop:[x,y,width,height]} crops within normalized image coordinates. capture uses bookmarkId, compare:true, optional width/height and returns actual paired/overlaid screenshots for visual review. Reference is explicitly marked; it is not generated geometry. capture_views also accepts bookmarkId/compare. Use matched views for proportion and silhouette correction."
            } else {
                "参考图/相机对照工作台：list返回已保存书签；save/remove需id和baseCreativeRevision。bookmark={name,shot,comparison?}，shot采用capture_views自定义相机参数，id会替换为书签id（英文数字横线下划线）。comparison={imageId,mode:'split'或'overlay',opacity:0..1,zoom:0.1..10,offset:[归一化横移,纵移],crop:[x,y,宽,高]}，crop须位于原图0..1范围内。capture配合bookmarkId、compare:true、可选width/height返回真实并排/叠加截图，用于轮廓比例对照；参考图不属于生成几何。capture_views也支持bookmarkId/compare。先匹配视角，识别差异后局部调整，不把参考图混入作品。"
            },
            json!({
                "action":{"type":"string","enum":["list","save","remove","capture"]},"id":{"type":"string"},"bookmarkId":{"type":"string"},"baseCreativeRevision":{"type":"integer"},"projectId":{"type":"string"},"compare":{"type":"boolean"},"width":{"type":"integer","minimum":128,"maximum":2048},"height":{"type":"integer","minimum":128,"maximum":2048},
                "bookmark":{"type":"object","properties":{"name":{"type":"string"},"shot":{"type":"object","properties":{"id":{"type":"string"},"projection":{"type":"string","enum":["perspective","orthographic"]},"position":point,"target":point,"up":point,"fov":{"type":"number"},"yaw":{"type":"number"},"pitch":{"type":"number"},"margin":{"type":"number"},"orthoHeight":{"type":"number"}},"additionalProperties":false},"comparison":{"type":"object","properties":{"imageId":{"type":"string"},"mode":{"type":"string","enum":["split","overlay"]},"opacity":{"type":"number"},"zoom":{"type":"number"},"offset":vector("number",2),"crop":vector("number",4)},"required":["imageId","mode","opacity","zoom","offset","crop"],"additionalProperties":false}},"required":["name","shot"],"additionalProperties":false}
            }),
        ),
        tool(
            "animation",
            if en {
                "Non-destructive group animation for editor playback and animated GLB export. list returns clips; save/remove require id and baseCreativeRevision; save requires clip={name,duration:0.05..3600,looping,tracks}. One track per existing group: {group,pivot:[grid XYZ],interpolation:'linear'|'step',keys:[{time,translation:[grid XYZ],rotation:[XYZ Euler degrees],scale:[positive XYZ]}]}. Times strictly increase within duration; outside the key interval pose clamps. Linear rotation uses shortest-path quaternion interpolation: add intermediate keys for full turns. Pivot transforms p as pivot+translation+rotation*scale*(p-pivot). capture_views animation:{clipId,time} captures a frozen pose. Does not modify the static scene. Avatar playback is explicit: publish(id,speed=1,position=0,playing=true,dryRun=false); then play/resume/pause/stop/seek(position seconds)/status. Requires mf6-motion4/1. Avatar limit: 3 tracks, 4 keys per track, positive uniform scale 1/16..8, durations 0.125/0.25/0.5/1/2/4/8/16/32/64 seconds, speed 0.25/0.5/1/2; swept pose must stay in the 256-cell target. dryRun validates before sending. Subsequent geometry publication validates the attached motion envelope too. detach republishes the current draft without motion binding; use it before static edits that invalidate the clip. Editor/GLB limits remain unchanged."
            } else {
                "非破坏分组动画，用于编辑器播放及带动画GLB导出。list返回片段；save/remove需id和baseCreativeRevision；save的clip={name,duration:0.05..3600秒,looping,tracks}。每个现有group一条轨道：{group,pivot:[逻辑格XYZ],interpolation:'linear'或'step',keys:[{time,translation:[逻辑格XYZ],rotation:[XYZ欧拉角度],scale:[正数XYZ]}]}。关键帧时间严格递增且位于时长内；首末关键帧外保持端点。linear的旋转采用最短路径四元数插值，完整转圈需中间关键帧。变换=pivot+translation+rotation*scale*(p-pivot)。用capture_views的animation:{clipId,time}冻结截图检查。动画不改变静态草稿，不自动发送OSC。Avatar播放需显式publish(id,speed默认1,position默认0秒,playing默认true)，可dryRun先校验；然后play/resume/pause/stop/seek(position秒)/status。要求mf6-motion4/1。Avatar最多3轨道、每轨4帧、1/16..8倍正均匀缩放，时长仅0.125/0.25/0.5/1/2/4/8/16/32/64秒，速度0.25/0.5/1/2；整个运动包络须在256格范围内。后续改稿发布也会复查绑定动画的包络；需要静态改稿时用detach解除绑定并发布当前草稿，之后可重新publish动画。编辑器/GLB原能力保留。"
            },
            json!({
                "action":{"type":"string","enum":["list","save","remove","publish","play","resume","pause","stop","seek","status","detach"]},"speed":{"type":"number","enum":[0.25,0.5,1,2]},"position":{"type":"number","minimum":0},"playing":{"type":"boolean"},"dryRun":{"type":"boolean"},"id":{"type":"string"},"baseCreativeRevision":{"type":"integer"},"projectId":{"type":"string"},"clip":{"type":"object","properties":{"name":{"type":"string"},"duration":{"type":"number","minimum":0.05,"maximum":3600},"looping":{"type":"boolean"},"tracks":{"type":"array","minItems":1,"maxItems":64,"items":track}},"required":["name","duration","tracks"],"additionalProperties":false}
            }),
        ),
    ]
}
