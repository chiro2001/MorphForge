# 软件验收意见：渲染、坐标、材质和工具现状

2026-10-19，以 0.6.1 源码为准。此文档记录分析和后续建议，除第 3 项外均未在此次补丁实施。

| 问题 | 当前实现 | 建议 |
|---|---|---|
| 1. 观察/截图抗锯齿 | WebGLRenderer 已设置 `antialias:true`，观察 DPR 上限 1.5；截图 DPR 固定 1，不做超采样。资源 atlas 为 Point/Nearest，关闭 mipmap；默认 shader 还画硬边框和四宫格。 | 近处保留像素风，远处提供安全的每 tile mipmap、各向异性采样；程序线框用导数平滑，截图 2×超采样再降采样。atlas 要解决六面/帧之间串色、UV fract 导数跳变，不能只打开整张 atlas 的 mipmap。WebGL 实际样本数须由设备查询。 |
| 2. 位置小于 0 | 独立创作使用有符号整数格，默认保护界限约 ±1,000,000；scene.size 不是硬裁剪空间。VRC 适配器允许 X/Z 为负：`[-128,128)`；Y 为 `[0,256)`。发布会检查 Box 的完整 extent，越界拒绝，保留草稿。 | UI 分开显示创作空间与 VRC 可发布范围；发布预检给出越界组和包围盒。Y 小于 0 可在完成时抬升到底面 0。超过 256 格的尺寸需真正降采样/裁切/拆分，Avatar 显示缩放不扩展坐标编码。 |
| 3. 新项目/新上下文入口 | 本补丁已实现。 | 见 RESULTS 与用户指南。 |
| 4. 内嵌 Minecraft 风格资源包 | 已有 Java ZIP 导入，能解析部分立方体模型父链、六面贴图及帧动画；尚无正式内置像素贴图主题。异形方块模型不等于立方体材质，不会自动变成异形几何。 | 内置原创或许可明确可再分发的像素主题：石/泥土/草/木材/砖/沙/玻璃/水/叶/金属/发光。原版资源通过现有用户导入入口提供。主题附许可、版本、预览和 Avatar 目录导出。 |
| 5. 黑边框和四宫格 | Web 与 Unity 的无贴图 fallback shader 都主动绘制；不是体素数据或粒子技术要求。真实贴图分支不叠加这个 fallback 格纹。 | 默认纯色/实际纹理，“显示体素格纹”改为可选观察工具；观察用的格纹不应默认进入艺术截图和 VRC 材质。 |
| 6. 材质和作品/VRC 的关系 | Box 保存 styleId、tintId、motionPhase；工程保存材质定义（UID/RGB/粗糙度/金属度/透明度/发光/六面贴图/帧动画），图片在本机 artifact。作品库保存该工程的材质快照；几何历史有对应材质快照。OSC 发送编码而不发送贴图。12-bit styleId、6-bit tint、6-bit motion。 | 当前 791 Box 作品使用的 14 种材质与 Unity 源目录比对均一致（见 material-comparison.json）；未核实已上传 Avatar。映射已有导出/Unity 构建实现，但必须上传相同 styleId→材质目录的 Avatar 才对应。新增 RGB 色板/资源包不自动更新已上传 Avatar。加入目录指纹兼容检查及未打包材质提示；网页艺术光照与 VRC shader 只是近似对应，不能保证相同像素。 |
| 7. Harness 截图 API | `capture_views` 绑定 revision；hero/front/top/side/detail 与自定义 shots 可混用，同名覆盖、合计最多 5 个。支持 yaw/pitch、正交/透视、fov/margin、显式 position/target、按 group/bounds 局部取景；128–2048 像素，默认 768×640。`run_recipe`/源码运行可附带 capture，`read_artifact` 可重读/裁切图片。 | 已满足自选角度和看图改稿的基本需求，但缺质量/超采样参数，模型工具 schema 未暴露 art/vrc 模式选择，也没有视频或内部剖切面的像素渲染。`inspect_region.slice` 是几何数据检查。截图工具返回后，图片须在下一模型请求中被看见；不是同批调用就已看见。 |
| 8. 整体平移/落地居中 | HTTP `/api/edit` 的 transform 可传全部 Box ID 平移现有作品；编辑器是选中移动入口。`mf.translate(offset,fn)` 只平移 fn 新发出的绘制命令，不自动平移已有作品。Harness 暂无现有场景整体平移的专用工具，也无自动落地居中。 | 加 `transform_scene` 和可预览/撤销的 `center_on_ground`。对包围盒 min/max，整数偏移可取 `[-floor((xmin+xmax)/2), -ymin, -floor((zmin+zmax)/2)]`，奇数宽度允许半格中心误差。默认在完成后或首次发布前显式应用，不能每条绘制后动态居中；可在发布适配器保存固定 origin 偏移，使源码坐标不变。后续重新居中需重建已提交的显示。 |
| 9. VRC 虚拟光照 | 网页 art 模式使用环境、方向及填充光；vrc 预览使用 MeshBasicMaterial。Unity catalog shader 有近似方向光/环境/金属高光/发光，缺主光方向时回退固定方向；网页 vrc 预览并不完整复现这些。 | 用 shader 的固定太阳+天空/地面环境项提供稳定美术光照，无需真实 Light 组件；统一网页和 Unity 的公式、色彩空间与朝向。光照参数可先打包为材质配置，避免新增同步位。 |
| 10. 水/云透明叠层 | 相邻同属性盒子已有合并，合成一大 Box 后内部格面自然不绘制；不同 Box 之间没有通用邻接剔面。网页透明实例与 Unity 同粒子池透明 pass 都有排序近似，当前非体积渲染。 | 优先同介质合并、内部界面剔除；只跳过同一个连续介质的内部面，不能只凭 alpha 相同就跳过远处表面。隔开的水层仍应可见。水适合表面/厚度吸收；云可先采用抖动透明或风格化表壳。动态 Avatar 粒子默认无邻接数据，需发送端预处理及 Shader/显示编码配合；不能照搬网页任意重建 Mesh。 |

相关源码：`web/renderer.js`、`web/camera-spec.js`、`crates/morphforge-app/src/{capture,catalog,resources,harness}.rs`、`crates/morphforge-core/src/{geometry,protocol,materials}.rs`、`Assets/MorphForge/Shaders/ParticleBoxCatalog.shader`、`Assets/MorphForge/Editor/CatalogMaterialBuild.cs`。

后续推荐顺序：无默认格纹与观察/截图抗锯齿 → 坐标边界/落地居中 → 内置像素主题与目录兼容检查 → 统一虚拟光照 → 水/云剔面和透明层优化。界面/Shader 改进不会在此补丁中偷偷扩展 OSC 位宽。
