# Unity 粒子材质目录

本文件定义 MorphForge 的 Unity 侧材质目录准备流程。2026-10-19 已经在 Unity 2022.3 中完成 Catalog 构建、Stage4 接入、原生视觉验证和基本 NDMF。执行结果见 `native-validation-status.json`、`native-strict-validation.json` 与 `native-finish.json`；真实 VRChat 客户端和 FullFT 联合预算仍是独立验证项。

## 输入

主代理导出的 ZIP 解压到 Unity 工程：

```text
Docs/Materials/
  catalog.json
  artifacts/
    <sha256>.png
```

`catalog.json` 的根结构：

```json
{
  "format": "MorphForgeMaterialCatalog/1",
  "materials": [
    {
      "id": 1,
      "uid": "mf:stone",
      "name": "石灰岩",
      "family": "natural",
      "color": [0.62, 0.67, 0.73],
      "metallic": 0,
      "roughness": 0.9,
      "opacity": 1,
      "emission": 0,
      "renderMode": "opaque",
      "textures": [],
      "frames": 1,
      "frameTime": 0.1,
      "pack": null,
      "source": null
    }
  ]
}
```

字段与 Rust `morphforge_core::materials::Material` 的 camelCase 序列化一致。`textures` 中的 URL 必须是 `/artifacts/<hash>.png`；ZIP 内的实际文件是 `artifacts/<hash>.png`。导入器也接受不带前导斜杠的同一路径，但不会读取 `catalog.json` 目录外的文件、绝对路径、`..` 路径或符号链接。

构建器只读取这个目录中的 manifest 和已生成的 `artifacts/`；不会复制原始 Minecraft 资源包、商业模型或未经授权的源素材。

导出端必须先拒绝 `styleId > 4095` 的材质；Unity 构建器不会把这个值静默回绕或合并。

## 编码约定

每个粒子颜色仍保存 4 个 6 bit 数据码：

```text
styleId = Rcode + 64 * Gcode    (0..4095)
tint    = Bcode                 (0..63)
motion  = Acode                 (0..63)
```

`styleId=0` 是保留的空槽；当前 manifest 要求材质 ID 在 **1..4095**，且不得重复。`tint` 用于确定性的颜色调制，`motion` 用于水面/云等材质的时间相位；A 通道不是透明度。渲染时透明度来自材质 manifest 的 `opacity` 和贴图 alpha。

阶段 4 的粒子颜色写入必须继续关闭 `ParticleSystemRenderer` 的 Apply Active Color Space，否则 Unity 会改变这 4 路数据码。颜色空间约定如下：

- manifest 的 `color` 是 sRGB 归一化三通道；
- Shader 在 `UNITY_COLORSPACE_GAMMA` 工程中直接使用 sRGB 值；
- Shader 只在 Linear 工程中执行一次 `GammaToLinearSpace`，不对 Gamma 工程做转换；
- PNG Atlas 由 Unity 的 sRGB 纹理导入自动处理颜色空间；
- metadata 使用 RGBAFloat linear data texture，颜色字段是 sRGB 数值，绝不对 metadata 颜色再追加第二次 Gamma 转换。

## 网格与本地 UV

沿用 Stage2 预烘焙的立方体网格和粒子流：

- `uvFace.xy`：模型本地面的 UV，使用 `v.z/v.y`、`v.x/v.z` 或 `v.x/v.y` 生成；
- `uvFace.z`：轴向，`0=X`、`1=Y`、`2=Z`；
- `uvFace.w`：当前 Stage2 网格写入为 `0`，**没有可用的正负面符号**；
- `SizeXYZ`：粒子出生尺寸；
- `NORMAL`：由粒子顶点流提供，用于确定正负面。

因此新 Shader 用 `uvFace.z` 选择 UV extent，用 `NORMAL` 与轴向量点积决定面符号，再从固定的六面顺序取样。若法线流缺失，才回退到立方体本地坐标的符号。不会使用世界坐标或世界法线生成 UV。

`uvFace.xy` 乘以局部面 extent，再除以逻辑格长得到逐格重复坐标；旋转、平移和等比例缩放不会改变这个采样方向。新的 `ParticleBoxCatalog` Shader 不修改现有 `ParticleBoxLocalUV`，也不会改变模型网格或现有 Stage4 构建。

后续把材质挂到 Stage4 的 `ParticleSystemRenderer` 时，必须保留 `ParticleSystemVertexStream.Normal`；当前 Stage2/Stage4 网格和流列表已经包含它。若故意移除，正负面会退化到本地坐标回退路径，不能保证所有面正确。

本任务的 Unity 侧格长固定为 `0.25m`，Builder 写入 `_CellSize=0.25`。浏览器 `renderer.js` 的 `vrc` 模式由主代理固定 `cellSize=.25`、艺术模式继续使用 `scene.cellSize`；本次没有修改渲染器代码，也没有覆盖主代理的这项变更。

## 六面顺序

Shader 的槽位顺序与 Rust 资源导入器和 Minecraft 模型约定一致：

| 索引 | 面 |
| ---: | --- |
| 0 | `+X` / east |
| 1 | `-X` / west |
| 2 | `+Y` / up |
| 3 | `-Y` / down |
| 4 | `+Z` / south |
| 5 | `-Z` / north |

canonical Rust/Unity 坐标的 `+Z` 是槽位 **4**，`-Z` 是槽位 **5**。Three.js 在进入场景边界时镜像 Z（`toThree` 使用 `-z`），所以浏览器的 +Z 面显示会对应到相反的 three-space 方向；WebGL 侧的 5/4 映射是镜像后的结果，不能反推成 Unity 的 4/5。资源包导入器把 `cube` 父模型的 `east, west, up, down, south, north` 展开成上述 canonical 顺序。无纹理的内置材质继续使用程序化格纹理。

## Atlas 与 metadata

构建器新增以下生成资产，位置固定：

```text
Assets/MorphForge/MaterialsCatalog/
  CatalogAtlas.png
  CatalogMetadata.asset
  ParticleBoxCatalog.mat
```

`CatalogAtlas.png` 使用 sRGB 导入、Point filtering、无 mipmap、Clamp。每个 style 占据 `6 × 32` 像素的一行内的六面块；动画帧在 style 块内向上堆叠。当前单元为 32 像素，Atlas 最大宽度 8192，实际高度受 GPU `SystemInfo.maxTextureSize` 限制。构建器计算所有 style 的矩形，超出最大纹理尺寸时直接失败并报告 style ID，不会截断材质。

`CatalogMetadata.asset` 是 `RGBAFloat`、linear、Point、Clamp、无 mipmap 的 256×64 纹理（共 16384 texel，覆盖全部 4096 styles）。每个 style 使用四个 texel：

| texel | 内容 |
| --- | --- |
| 0 | `offsetX, offsetY, frames, frameTime` |
| 1 | `color.r, color.g, color.b, opacity`，颜色为 sRGB 值 |
| 2 | `metallic, roughness, emission, flags` |
| 3 | `cellPixels, 0, 0, 0` |

flags：

| bit | 意义 |
| ---: | --- |
| 0 | 有 Atlas 贴图 |
| 1 | transparent pass |
| 2 | alpha cutout |
| 3 | water 程序效果 |
| 4 | cloud 程序效果 |

metadata 的线性索引固定为 `styleId * 4 + slot`，再按行展开：

```text
x = index % 256
y = floor(index / 256)
uv = ((x + 0.5) / 256, (y + 0.5) / 64)
```

材质同时设置 `_MetaWidth=256` 和 `_MetaHeight=64`。styleId 4095 占用最后一行的最后四个 texel，编号空间不缩减。重建 metadata 时保留资产 GUID，避免已有材质引用断开。

2026-10-19 的服务端对照中，原 `16384×1` 布局被拒，`4096×1` 诊断子集及完整 `256×64` 布局均通过；后者原始数据字节完全不变。详见[实际上传对照](../security-check-failure/README.md)。这是已观察到的资源布局差异，不是对 VRChat 未公开尺寸阈值的推断。

## Shader 行为

`ParticleBoxCatalog.shader` 只有一个材质和两个 Pass：

1. **Opaque/Cutout pass**：不透明的金属、发光、云和 cutout；cutout 使用 Atlas alpha 做 `clip`。
2. **Transparent pass**：水、玻璃和 `renderMode=transparent` 材质，使用常规 `SrcAlpha OneMinusSrcAlpha` 混合，关闭 ZWrite。

Pass 通过 metadata flags 筛选材质模式。普通不透明材质不会进入透明 pass，透明材质不会进入不透明 pass。

这是**单粒子池的可接受近似**，不是完整透明排序系统：

- Unity 材质只有一个整体 render queue。本 Shader 选择透明队列，不透明与 cutout 粒子也在这个晚队列中绘制，并且不透明 pass 仍开启 ZWrite；这避免了透明材质过早绘制，但会让不透明粒子的队列顺序晚于常规不透明物体；
- 一个 ParticleSystemRenderer 的透明粒子不保证逐粒子严格从远到近排序；
- 透明物体互相重叠、与不透明表面交叉时会出现排序和深度边界误差；
- 水体只是表面色彩、alpha 和 UV 波动，没有折射、体积吸收或物理水面；
- 云只是表面噪声和明暗，不是体积云或体积散射；
- 金属/发光是简化的表面 PBR 近似，不是完整 IBL 或实时 GI；
- 两个 Pass 都可能对同一粒子执行顶点阶段，透明池的 overdraw 需要 16k 性能回归。

如果以后需要更严格的透明排序或水体折射，必须增加独立的透明粒子池/排序阶段；这不在当前单个 ParticleSystemRenderer 的承诺范围内。

动画时间由 `frameTime` 和 `_Time.y` 计算，`motion` 给动画增加相位。静态材质只有一帧；动画材质必须保持 Atlas 中所有六个面的帧数一致。资源导入器已经把 `.png.mcmeta` 的显式帧序、逐帧时长和插值规范化为相同 tick 时间轴，因此 Shader 不需要在运行时读取 JSON 或创建脚本。

## 构建入口

在 Unity 工程中，等长测结束并且主代理允许后执行：

```powershell
python tools/unity_remote.py --check-result run_script --file AgentScripts/CatalogEntry.cs --entry CatalogEntry.Validate --timeout_ms 60000
python tools/unity_remote.py --check-result run_script --file AgentScripts/CatalogEntry.cs --entry CatalogEntry.Build --timeout_ms 60000
```

`Validate` 只读取 manifest 和 PNG 并检查范围、路径、帧数和材质字段，不创建或修改 Unity 资产。`Build` 才创建三个生成资产。菜单入口分别为 `Tools/MorphForge/Materials/Validate catalog` 和 `Tools/MorphForge/Materials/Build catalog material`。

构建完成后使用 `Stage4FinishBuild` 的独立 Edit Mode 收尾步骤把 `ParticleBoxCatalog.mat` 分配给 `MorphForgeGeometry` 下的 `Store`，并同步 Pump 寿命和顶点流。该收尾器不修改 `Mf3DisplayBuild`、`PlacementStage4Build`、Shader 或 Runtime 源文件，不自动上传 Avatar；Prefab 的具体修改清单写入 `Docs/Materials/native-finish.json`。

## 已执行验证

以下项目已执行并通过：

1. Unity 2022.3 编译 Shader 和 Editor 构建器；
2. 100 材质 union 和 7 个 artifact 的 Catalog 构建；
3. Atlas `384×96`、metadata `256×64`、Material 引用；
4. Stage4 单一 Renderer 的六面、局部 UV、scale 0.5 和根刚体变换稳定性；
5. 金属、水、云、cutout 的像素可见性；
6. 16,384 混合材质的临时原生 rig：可见，p50 `8.92ms`、p95 `11.80ms`；
7. 基本 NDMF：无 missing script、无 MorphForge Runtime 脚本，基础 Avatar 总计 159bit。

尚未覆盖：

- VRChat 客户端 GPU、逐粒子透明排序与用户过滤；
- FullFT 联合参数预算；
- 真实玩家移动和多人晚加入；
- 首次 16k UDP 首遍偶发漏 Birth 的无损保证。
