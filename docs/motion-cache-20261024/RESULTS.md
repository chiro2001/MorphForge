# Avatar动画缓存修正与连续循环（2026-10-24）

用户报告瀑布在编辑器单向平移后跳回，在VRChat出现旋转和往返。旧版Editor和SDK定点回放未复现，不能把Editor通过当作实机方向已修复。

本轮消除两个具体风险：120个关键帧接收直接混合树状态关闭Write Defaults，234个缓存字同时由接收层和输出层写入。现在123个接收/保持状态都开启Write Defaults并完整写入本组缓存，由一个接收层负责保持/替换，材质层只读取。共享234个保持Clip，参数与网络位不增加。

直接混合树要求Write Defaults On的依据是[VRCFury技术文档](https://vrcfury.com/technical/wd/)。这是通用风险说明，尚不能证明它是本次实机异常的唯一根因。

|检查项|修正前|修正后|
|---|---:|---:|
|关闭Write Defaults的接收直接混合树状态|120|0|
|被多个层共同写入的缓存字|234|0|
|动画控制器参数|462|462|
|SDK合并FX参数|1926|1926|
|总同步位|254|254|

[结构审计](structure-audit.json)、[构建](build.json)、[修改前备份](backup.json)。13个人物原Prefab依赖保持不变。Unity资源通过Pipeline生成，仓库公共生成资源为Unity导出包原始快照，未手改YAML或GUID。

## 验证

实际SDK处理后的头像接收生产250ms节拍数据包。四轮旧旋转/X运动与新Y-/Z+平移替换覆盖双bank及复用，[492个材质值与24个GPU位置对比](sdk-transition.json)通过。生产seek/resume启动原生时钟，[连续三轮](sdk-continuous.json)三组均无反向步骤，102次正面/俯视GPU对比通过，最大误差小于1像素；没有手改相位、材质或缓存替代接收。[SDK结构](sdk-prepare.json)。

## 在线与安装

同一私人Avatar保持ID、名称、可见性，上传为 **v12 / file v10**，pendingUpload=false。[线上信息](online-avatar.json)、[generic/standard/security三项分析](online-analysis.json)均通过。SDK临时场景另存，已[恢复干净源场景](postflight.json)。

接收能力增至 **9**，Desktop0.26.1支持并在真实9000动画发布时要求该版本，以核对客户端是否加载修正。公开插件 **1.3.1** 在`dist/avatar/MorphForgeAvatar-1.3.1.unitypackage`，[159项包/源码一致性审计](package-audit.json)通过，不含私人头像、用户变体或商业人物资产。

## 实机复验

2026-10-24用户确认动画方向与循环表现已正常，下一轮以此作为回归基线。以下保留原复验步骤。

1. 在VRChat切到其他头像，再换回`MorphForge MF6 16K`，等加载完成。
2. 桌面“识别当前Avatar”应显示能力版本9。唯一片段`flow-loop`自动选中。
3. 开启发送，点击主界面“发布当前版本”，等待队列完成，首次完整发送可能约两分钟。
4. 背景两组向下，前方流水沿河道前进，两秒末尾跳回起点，无旋转、无平滑往返。

**实机视觉仍待用户复验**；工程检查不等同于用户实机已通过。未替用户切换头像、未调用模型API或麦克风、未推送或创建Release。
