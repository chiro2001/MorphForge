# Java 资源包材质导入与预览

本页记录 MorphForge 当前 Minecraft Java 资源包导入、材质规范和浏览器渲染路径。实现范围仅涉及：

- `crates/morphforge-app/src/resources.rs`
- `web/renderer.js` 的资源贴图采样部分
- `crates/morphforge-app/tests/resources.rs`
- `web/renderer-resources.test.mjs`

没有修改服务端路由、工作区存储、Harness、截图服务或 Unity 工程。

## 实际渲染路径

```text
浏览器上传 ZIP
  → POST /api/packs/import
  → 安全读取 ZIP
  → 解析 block PNG 与 .png.mcmeta
  → 合并 block model 父链和纹理别名
  → 按 20 Hz tick 时间轴规范化六面动画
  → 写入 /artifacts/<sha256>.png
  → 写入 Material.textures / frames / frame_time
  → /api/scene 返回 scene + materials
  → VoxelRenderer.load()
  → atlas() 按六面和时间轴合成图集
  → 实例化 Mesh 的着色器按 frameTime 循环采样
```

现有 `server.rs` 已经有 `POST /api/packs/import`，`studio.js` 上传后也会刷新场景。导入后的材质可以在编辑器、Harness 截图和作品预览中使用；VRChat Avatar 仍需要单独的 Unity 预上传和材质映射，导入本身不会上传或修改 Avatar。

## 支持的 Java 立方体模型

模型父链最多 16 层，子模型的 `textures` 会覆盖父模型。支持：

| 父模型 | 六面纹理键 |
| --- | --- |
| `block/cube_all` | `all` × 6 |
| `block/cube_mirrored_all` | `all` × 6 |
| `block/cube_column` | `side, side, end, end, side, side` |
| `block/cube_bottom_top` | `side, side, top, bottom, side, side` |
| `block/cube_top` | `side, side, top, top, side, side` |
| `block/cube` | `east, west, up, down, south, north` |

纹理别名 `#alias` 会沿模型纹理表解析，未带命名空间的资源名按 Minecraft 约定归入 `minecraft:`。
上述六项在 MorphForge canonical 坐标中的槽位固定为 `+X, -X, +Y, -Y, +Z, -Z`；`south` 对应 canonical `+Z`，`north` 对应 canonical `-Z`。Three.js 在镜像 Z 后使用相反的可见面顺序，不能把浏览器侧 5/4 当作 Unity 面槽。

含自定义 `elements` 的模型、楼梯、台阶、十字植物、自定义面旋转、`cullface` 和 `tintindex` 尚未转换为体素盒子的实际几何或色层。导入器会对无法安全解释的模型给出 warning 并跳过，不会把异形模型静默当成完整立方体。

没有 `.png.mcmeta` 动画定义的方块贴图要求为正方形；非正方形静态贴图会被跳过并记录 warning，避免把长图误判成竖排动画。

## 动画支持范围

支持 `.png.mcmeta` 的：

- 默认 `frametime`；
- 显式 `frames` 顺序；
- `{ "index": n, "time": ticks }` 逐帧时长；
- `interpolate: true`；
- 每帧宽度/高度能被贴图尺寸整除的帧表；通常使用竖排帧条。

导入器把动画展开成 **20 Hz tick 时间轴**：

- 每个 tick 生成一个预览帧；
- `interpolate` 在每帧持续时间内按 tick 位置混合当前帧与下一帧；
- 六个面各自保留自己的帧序和时长；
- 六面周期不同，使用 tick 数最小公倍数生成共同时间轴；
- `Material.frames` 是共同时间轴 tick 数，`Material.frameTime` 是 `0.05` 秒；
- 静态贴图保持单帧，渲染器在动画时间轴中重复该静态面。

当前上限是每轮 **1,024 ticks（51.2 秒）**。超过上限或六面周期公倍数超过上限时，导入器保留原素材但会给出 warning，避免生成超出 WebGL 纹理高度的时间轴。

这与官方格式的 `frametime`、`frames` 和 `interpolate` 语义一致；时间轴展开是本项目的预览实现，不改变原 ZIP。

参考：[Minecraft Wiki: Texture animation](https://minecraft.wiki/w/Resource_pack#Texture_animation)。

## ZIP 与资源安全

导入前执行以下限制：

| 项目 | 上限／规则 |
| --- | --- |
| 原始 ZIP | 64 MiB |
| ZIP 条目 | 16,384 |
| 单文件解压后 | 16 MiB |
| 相关文件解压总量 | 256 MiB |
| PNG 单边 | 16,384 px |
| 单 PNG 像素 | 4,194,304 px |
| 全部解码贴图像素 | 67,108,864 px |
| 贴图数量 | 4,096 |
| 路径长度 | 512 bytes |

同时拒绝：

- 绝对路径、`.`、`..`、空路径段和反斜杠路径；
- ZIP 条目中的符号链接或特殊文件类型；
- 精确或大小写不敏感的重名路径；
- 不符合 Java 资源定位规则的命名空间和大写资源名；
- 非 PNG 的 block 贴图、损坏 PNG、超出尺寸或解码上限的 PNG。

读取时会以 `read_to_end` 的硬上限截住伪造的 ZIP 头部尺寸，不使用中央目录声明值直接分配内存。

## 渲染端约定

`renderer.js` 现在使用材质声明的 `frames` 和 `frameTime`，不再只从图片高度猜测帧数。图集分辨率同时受以下限制：

- 最大 128 px 单元；
- WebGL 最大纹理宽度除以 6；
- WebGL 最大纹理高度除以时间轴帧数。

这样 1,024 帧时间轴在常见 16,384 px 限制下会降到 16 px 单元，而不是生成无法上传的超高图集。

资源包材质在 `art` 和 `vrc` 渲染模式下都使用同一张规范化图集。`capture()` 在每次截图前把动画时间归零，保证同一 revision 的截图可复现到第一帧。

## 与主服务的接口差距

当前 `/api/packs/import` 是直接提交接口，缺少以下管理能力：

| 需要补充的接口 | 建议 |
| --- | --- |
| 预检 | `POST /api/packs/inspect`，返回材质预览、warning、预计新增数，不写工程 |
| 包目录 | `GET /api/packs`，返回 packId、名称、材质数、时间轴上限和导入时间 |
| 删除／禁用 | `DELETE /api/packs/{id}` 或 pack-level enable 状态 |
| 替换／更新 | 以 packId 做幂等替换，并保留材质 ID 映射 |
| 告警呈现 | 前端显示 report.warnings 和 `avatarRequiresBuild` |
| Avatar 映射 | 预览材质到 VRChat 预上传材质槽的显式绑定接口 |

这些接口需要主代理在 `server.rs`、`store.rs` 或 UI 层连接；本轮不越界修改。

## 验证

Rust：

```bash
~/.cargo/bin/cargo test -p morphforge-app --test resources
```

覆盖静态材质、父链和六面顺序、显式帧序、逐帧时长、插值、幂等重导入、ZIP 路径穿越、符号链接、反斜杠路径和无效动画帧。

浏览器资源辅助函数：

```bash
node --test web/renderer-resources.test.mjs
node --check web/renderer.js
```

本轮没有启动服务、浏览器或 Unity，也没有声称真实 WebGL 动画已通过视觉验收。渲染辅助函数只验证帧数、图集分辨率上限和循环帧选择；最终纹理采样仍应在下一轮服务启动后做截图回归。
