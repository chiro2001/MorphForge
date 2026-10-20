# MF6 验证记录（进行中）

本页只记录已发生的验证；未完成本轮交付。

## 已通过

- 核心固定帧编解码：所有布局和8种表面模式往返、16个序号、保留位和填充拒绝、坐标边界、三角带绕序、1万组随机字节输入无panic。
- Rust工作区完整回归一轮通过；随后新增统一调度两项测试通过：8体素共外观、4Box共外观、局部三角带、池平衡、增量删除屏障和不清空补播。
- 三角面核心事务：混合对象ID、更新/删除、镜像绕序、复制、整面区域删除、导入、容量和失败回滚。
- Recipe SDK：索引网格、逐面颜色、三角带、镜像/平移、闭合八面楔形通过Node验证。
- 单池原生OSC出生验证：20个三角面，含24bit最大值与序号回绕；顶点/颜色逐字段精确一致；按ID重写无重复、删除及空闲保留通过。发送间隔0.30s，不能据此宣称全帧0.25s已通过。见`native-worker-smoke.json`。
- 实际粒子Renderer三视图像素检查通过：正/背面46217像素、斜视39335像素，无运行时Bounds覆盖，无SetParticles/Emit注入。见`native-visual.json`与`visual/`。
- GPU数据流读回确认世界坐标/单位ObjectToWorld、SizeXYZ、Center与InvStartLifetime布局，见`stream-probe.json`。

## 保留的失败与修正

- 直接将24bit整数放入粒子尺寸被Unity截到100000，且删除半径误伤其它粒子；见`probe-unnormalized-failure.json`。
- 单纯0..1归一化虽然保留数据，但极小载体不利于参考系重建。改为1..4的两段指数编码。
- 只扩大Renderer.localBounds不持久化，初次真实显示失败；改为持久化载体Mesh Bounds。
- 初次材质复制仍指向旧ParticleBoxCatalog Shader；MF6现在明确使用已扩展的MetadataGrid Shader。

## 待完成

- 统一帧解码器与8池实际端到端、全部压缩格式、0.25s及稀疏池序号、丢帧补播。
- 16K容量、旋转/缩放/定位、单面/双面、Box与三角面混合显示。
- 完整前端/Harness/Windows、Minecraft导入与资源包、配套Avatar上传和服务端终态分析。
- 当前没有真实多人验证，所有发送统计仍无ACK。
