# Harness 强化与创作时间轴

0.6.0候选版本，工具 `morphforge-tools/3.5`、提示词 `morphforge-system/4.8`。默认1M上下文、384K输出保持；实际请求仍受上游窗口及运行预算约束。

## 时间轴

底部点击「创作时间轴」。可按运行、事件类型、版本和关键词筛选，加载更早记录，展开工具参数/源码/错误，查看真实截图与用量。流式思考只展示上游实际返回的文本或摘要；没有返回可读内容时明确标注，不能从reasoning Token数量推造思考文本。

「查看此版本」打开独立只读3D预览，不覆盖当前草稿、不调用模型、不发OSC；恢复编辑仍通过原有版本历史按钮显式操作。运行ID按钮可筛选该次运行。时间按浏览器本地时区显示。

原始记录在数据目录的 `events/events.jsonl`，思考、工具参数、截图和源码artifact持续保留。界面读取独立的轻量投影，不轮询传输整个请求/base64；更早记录不受内存事件窗口限制，投影可在重启时从原始日志重建。大段内容按需展开，显示截断会保留标记，完整原始数据可导出诊断记录。查看时间轴不改写模型历史。

## 常驻缓存保护

配置页的「缓存保护」：

- **自动**：DeepSeek官方接口严格检查，其他兼容接口观察并显示告警。
- **严格**：异常时暂停后续模型请求。建议有缓存能力的内网vLLM测试使用此模式。
- **观察**：适用于不上报缓存、或明确不支持缓存的服务，保留客户端前缀检查和用量记录。

客户端每次在协议/图片转换之后检查出站前缀。未经明确配置操作或上下文边界解释的历史/system/schema变化，会在发送前拦截；诊断文件损坏不会被静默当作首次请求。

缓存率只取上游usage，缺失显示未知；同时显示当前请求、最近窗口和覆盖率，输入框旁常驻显示告警。对至少4K输入的稳定前缀，连续三次零命中会触发保护；连续缺字段及低命中率也有检查。输入达到128K的零命中/未知会立即暂停。已建立有效命中的会话，稳定前缀首次退化为零命中/未知也会暂停。compact不绕过已经存在的缓存暂停。

暂停后检查服务端缓存、模型路由和usage，再在配置页确认告警并恢复；确认本身不会发起模型请求。客户端前缀一致性不等于真实KV-cache命中，服务端也可能淘汰缓存。已发送请求的费用不能靠暂停追回。

严格模式发送前为**预计输入**预留Token，再计算输出余量。估算仍有误差，真实usage是最终记账依据；这里不是货币费用保证。累计Token包含缓存输入，因此高命中率也可能用完总Token预算。未配置可信价格时不虚构金额。执行器内部崩溃直接暂停，避免模型付费尝试修复底层运行时。

## 绘制与截图合并

`run_recipe` 和 `recipe_sources(action=run)` 可选 `capture`，成功绘制后直接返回对应版本图片，节省一轮模型往返：

```json
{
  "code": "mf.group('orb',()=>mf.ellipsoid([0,24,0],[18,12,10],{style:3}));",
  "mode": "apply",
  "baseRevision": 0,
  "seed": 1,
  "capture": {"views": ["hero", "front", "top"]}
}
```

材质ID使用当前目录返回值。`capture`不需要填写revision；它取本次成功提交的版本。dryRun不提交也不截图。截图失败会返回 `geometryCommitted/captureError`，表示几何已经成功，只应重试截图，不应重复绘制。

模型必须在后续响应实际收到当前截图，再调用 `finish_artwork`；同一批次先截图再盲目完成会被拒绝。关闭发送图片时工具结果明确标注未附图，不能声称已做视觉检查。

## 自由相机与局部视图

原 `views`兼容。新 `shots` 可与views组合，每次合计1～5个不同视角，同名shots覆盖预设：

```json
{
  "revision": 17,
  "shots": [
    {"id":"whole","projection":"perspective","yaw":35,"pitch":25},
    {"id":"face","fit":{"group":"face"},"projection":"orthographic","yaw":0,"pitch":0,"margin":0.12}
  ],
  "width": 1024,
  "height": 832
}
```

坐标是体素格，+Y向上，+Z正面；yaw=0、pitch=0从+Z平视。支持分组/`fit.bounds`取景、正交/透视、margin、fov，以及显式position/target。回执提供实际相机与bounds；不存在的分组报错，不偷偷改成全景。局部视图用于检查细节，完整场景用于检查遮挡。

`read_artifact` 可重读已保存图片，或用像素 `crop:{x,y,width,height}` 放大局部，也能读JS/JSON/文本。重读不新增几何截图或Goal复核轮数，不能凭一张旧图证明当前状态。

## 持久源码

`recipe_sources` 支持 `list/read/write/patch/delete/run`。虚拟文件只属于当前工程，不提供主机文件系统或外部npm包。支持CommonJS相对require：

```javascript
const body = require('./parts/body.js');
module.exports = (mf, params) => {
  mf.replaceGroup('body', () => body(mf, params));
};
```

修改已有文件使用read/write返回的 **file.hash** 作为expectedHash，不能用整个工作区的sourceHash。小改动用精确patch，避免重发整份代码：

```json
{
  "action":"patch",
  "path":"main.js",
  "baseRevision":12,
  "expectedHash":"从read/write复制file.hash",
  "edits":[{"find":"radius: 10","replace":"radius: 12"}]
}
```

find默认必须唯一，明确 `all:true` 才替换全部；多处编辑任意失败都不写文件。支持中文路径。最多32个文件、每文件128KiB、总计512KiB；run的params是最多16KiB的对象，源码跨compact/重启保留，参数随每次run显式传入，运行记录保留。

源码有不可变快照和hash，运行后的几何有revision与执行artifact。几何撤销不会自动撤销源码；两者分开保存，历史源码快照可从时间轴下载。每次执行仍在新JS沙箱和有界子进程中，不依赖不可追踪的长驻内存。

## 曲面与局部检查

- `mf.ellipsoid(center,radii,options)`、`mf.sphere(center,radius,options)`。
- `mf.capsule(a,b,radius,options)`、`mf.tube(points,radius,options)`，tube最多128个点。
- `mf.cylinder(base,height,radius,options)`，底面中心为base，沿+Y。
- `mf.extrude(points2d,origin,depth,{plane:'xy',style})`，可选xy/xz/yz，支持凹多边形。
- `mf.voxelize({min,max},predicate,options)` 在格中心采样，单次最多200万候选格；false/null不生成，true使用options材质，也可返回材质ID/UID。
- `mf.removeGroup(name)`、`mf.replaceGroup(name,fn)`。移除只影响该组；新增几何仍遵守普通空间覆盖规则，重要邻组仍应受保护。

结果始终是普通整数网格Box。大Box适合平面与实体填充，不能用Box数量代替形状分辨率；曲面、剪影和识别细节需要实际的细网格结构。

`inspect_region` 增加分页和 `slice:{axis:'y',index:12}`，返回一格厚截面。普通region统计仍是相交Box集合，回执的statsScope明确区分；不会把整体相交盒体积冒充精确裁剪区域体积。

[本轮验证与真实模型记录](quality-v6/RESULTS.md) · [完整迭代计划](plan/HARNESS-QUALITY-ITERATION.md)

## 受控评测

`tools/evaluate_harness.py` 可加 `--require-cache-hit --lifetime-token-limit 100000`。先完成独立的小型缓存探测；脚本不会为探测偷偷发模型请求。模型/提示词等设置变化须重新探测，目录累计用量包含探测。脚本夹紧本run预算并检查累计实际用量，未知或有误差的收费仍如实记录，不声称精确金额上限。
