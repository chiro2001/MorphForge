# 共享材质与资源包

## 家族与实现顺序

P0：哑光纯色/塑料/橡胶/布料；石土草沙雪；木材/砖/混凝土/瓦；钢铁铜金/拉丝/锈蚀；自发光灯/霓虹/晶体/熔岩；裁切树叶/网格。

P1：Java完整立方体资源包（六面差异、常用模型继承、染色遮罩、mcmeta翻页）；基础体素云与柔和明暗/噪声边缘；水的颜色/透明/法线波纹/UV流动；玻璃/冰。

P2：烟雾火焰、真正体积云、折射/泡沫/几何波浪、第三方PBR/连纹理/异形模型兼容。用户需要的云/水/金属有基础预设先可用，高级效果逐项验证。

## 定义与发布

稳定材质UID、名称/标签/版本、基础颜色、六面纹理、metallic/roughness/opacity/emission、renderMode、染色遮罩、格纹理方向/重复、动画与性能等级。前端、Rust和VRC实现使用同一manifest；渲染引擎可以不同，兼容范围明确。

通用作品不受styleId数量限制。VRC发布将活动目录映射到12bit styleId（最多4096种编码），tintId/motion预设使用原有共享刷子字段，不为每个物理参数新增长期同步位。新纹理/Shader须预打包到Avatar重新上传；本地导入可以即时预览。已预置主题切换可研究共用命令，不假定已实现。

按不透明、裁切、半透明、叠加模式固定少量渲染池，避免每种材质建立Animator。池容量与总容量均在manifest声明；16k不透明结果不能推断16k透明同样性能。统计overdraw/屏幕覆盖、透明层数、贴图内存、帧时间。水大体积优先合并；格纹理在长方体重复而非拉伸。自发光表面不默认增加实时Light。

## Minecraft导入

首版Java资源包ZIP/目录，识别pack.mcmeta及assets/<namespace>/textures/block、常见cube模型纹理引用、顶/底/侧差异和.mcmeta动画。未知模型明确报告/可配置替代，不声称支持任意shader pack。Bedrock和第三方扩展独立适配。

路径规范化、拒绝ZIP穿越/符号链接、限制解压大小/图片尺寸/文件数；保存来源/许可/哈希。默认内置原创或可分发纹理，用户导入原版/第三方内容按其授权处理，不以非盈利当作自动授权。

材质面板支持搜索、分类、收藏、最近使用、资源包、单位块/长方体/水面预览、当前目标可用性与成本。作品绑定目录版本；艺术预览和Avatar兼容预览标明模式。

## 当前落地契约

当前交付采用以下范围；上面的 P0/P1/P2 是扩展规划，不能全部当作已实现：

- 已有 24 种基础预设、按名称/资源包搜索、统一染色编号、金属/发光/透明/cutout、云水表面近似，及 Java ZIP 的常见立方体继承、六面贴图、显式动画帧序/时长/插值规范化。
- 染色目前作用于整个方块，不读取 Minecraft `tintindex` 或生物群系颜色，不提供逐面染色遮罩。法线贴图、物理波纹、折射和体积云未实现。
- Web 端按材质实例化；Avatar 当前为一个持久粒子池和一个双 Pass 材质，按 metadata 区分 opaque/cutout/transparent。严格透明排序及独立分池仍属后续扩展。
- 当前面板提供搜索和资源包导入，收藏、最近使用、专门的成本/预览面板尚未提供。已有主视口可查看单位块、长方体及水体。

Material.color 为归一化 sRGB 三通道；渲染器转线性空间后着色。普通艺术预览使用灯光和色调映射，VRC基础兼容预览关闭色调映射以便核对纹理色与方向。VRC格长固定0.25，独立艺术格长保存在Scene中。

每个revision额外保存不可变 `.materials.json`，历史截图和恢复读取该版本目录。原型旧数据没有的目录快照只能回退到迁移时现有材质，不能恢复此前未记录的变更。

`GET /api/vrc/catalog` 导出ZIP，包含catalog.json及引用的artifacts/*.png；不执行上传。解压到Unity工程Docs/Materials，然后通过CatalogEntry构建预上传资产。`GET /api/diagnostics`仅导出白名单文本，排除provider.env、浏览器目录与二进制缓存，并替换已知Key；截图仍以artifact哈希引用。

## 2026-10-19 Unity验证快照

Catalog已在Unity中完成构建和原生验证：100材质、Atlas 384×96、metadata 16384×1；严格视觉40项通过，覆盖三帧动画、六面方向、金属/水/云/cutout、缩放及根刚体变换。16,384个混合材质的临时原生fixture可见，Editor p50 8.92ms/p95 11.80ms，但不是VRChat客户端、UDP或产品Avatar Store基准。

当前单个`ParticleSystemRenderer`仍不保证逐粒子透明排序，水与云是表面近似；独立透明池、折射和体积效果尚未实现。证据与限制见[native-validation](../materials/native-validation.md)、[native-strict-validation.json](../materials/native-strict-validation.json)和[catalog-16k-performance.json](../materials/catalog-16k-performance.json)。

## 2026-10-19 服务端布局修复

上文原生验证使用的16384×1 metadata已被完整256×64布局取代，全部4096样式的
原始字节和资源GUID保持不变。100目录材质像素对照MAE=0、16项新视觉验证、
16k显示夹具及实际FullFT上传的服务端三项检查都通过。原布局失败及对照记录见
[安全诊断](../security-check-failure/README.md)。本轮没有缩减材质编号空间或改为旧LocalUV显示。
