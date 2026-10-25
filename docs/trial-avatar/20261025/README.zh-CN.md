# MorphForge × 256Hedgehog 试用头像准备

独立工程：`D:\Projects\VRChatProjects2\MorphForgeHedgehogDemo_1_4_0`。

Unity 2022.3.22f1，SDK 3.10.5、Modular Avatar 1.18.7、NDMF 1.14.8、VRCFury 1.1430.0，复用现有工程的包目录。工程由Unity CLI新建，模型与插件通过Pipeline及AssetDatabase导入，没有复制Roadster场景、上传ID或商业人物资产。

输入为原作者256Hedgehog 1.2.0包及已验证的MorphForge Avatar1.4.0公共包。来源与摘要见 `provenance.json`；原作者Readme声明CC0 1.0，保留在 `256Hedgehog-Readme.txt`。

默认日文。安装窗口保留中文、日文、英文选择；当前选择用于插件界面和所安装Avatar菜单。改变VRChat内菜单需要重新上传头像。静态引导牌以日文和英文说明启用OSC、在Desktop连接识别并发布作品；其显示开关为本地参数，不增加同步位数。引导文字使用NotoSansCJK字体光栅化，没有引入新的运行时C#组件。

场景与Prefab：

- `Assets/MorphForgeHedgehogTrial/MorphForgeHedgehogTrial.unity`
- `Assets/MorphForgeHedgehogTrial/MorphForgeHedgehogTrial.prefab`

当前进度：**本地制作、SDK前处理与原生头像构建已完成**，`validation.json` 为 `passed / ready-for-upload-review`。8项SDK／构建检查和15项准备检查全部通过；实际菜单包含「ワールドに固定」「本人プレビュー」等日文标签。SDK窗口已打开，但本次只读检查时尚未登录，未进行在线上传或取得公开试用链接。

首次SDK检查保留刺猬原有32bit参数，与MorphForge默认253bit合计285bit；VRCFury提示最多只能压到278bit。用户确认 `Warning / OK` 后，脚本只在试用副本移除了 `VRCEmote`、`VRCFaceBlendH`、`VRCFaceBlendV` 和旧Emote菜单。四套换色、原作者动画控制器、手势与口型设置保留；最终SDK前处理为 **255/256bit**，30个Int／240bit几何通道继续直接同步，换色和定位等低频菜单通过VRCFury桥同步。

原作者包的24个非文件夹资产与工程文件逐项SHA256一致，见 `author-assets.json`。试用Prefab和场景已保存，并记录Unity依赖摘要，供上传前核验。全部9份显示材质及Shader有效；16个只发射、不绘制的Pump具有有效子发射器且Renderer关闭，按已有公共包验收规则独立检查，不要求为其分配显示材质。

正式原生SDK生成的本地包为 `TrialReview/MorphForge-Hedgehog-Trial-local.vrca`，2,595,751字节。构建使用临时克隆与一次性本地ID；保存的试用头像仍没有在线ID，构建未改写试用Prefab。源文件和构建产物摘要见 `artifact-manifest.json`。此结果证明本地构建完成；在线服务端分析及实际VRChat功能验收尚未执行。

预览图为 `hedgehog-thumbnail.png` 和 `hedgehog-guide.png`，均由Unity相机生成。前者不显示引导牌，用作上传缩略图；后者展示双面引导牌。静态截图为编辑模式的原始姿态，不代表VRChat内实际站姿或功能验收。头像支持PC／PCVR，上传说明标注预期为Very Poor性能等级。

`AgentScripts/HedgehogTrialBootstrap.cs`负责原包导入；`HedgehogTrialBuilder.cs`负责场景、预算检查、SDK本地构建和只读准备检查；`HedgehogTrialUpload.cs`提供显式公开上传入口，已通过实际工程编译及只读入口调用。上传代码支持保留SDK已分配ID并核验重试场景，防止中断后重复创建头像；写入上传入口尚未执行。没有初始化自动上传、请求文件监视器或自动接受版权弹窗。

后续由用户在此试用工程的SDK Account页登录，并确认公开上传这一份新试用头像；[仓库协作说明](../../../AGENTS.md)要求「不自动上传 Avatar」。确认后才调用正式SDK上传、回读所有权与公开状态，待在线检查完成后再写入BOOTH的 `publicTrialAvatarUrl`。现有私人头像、主安装和其他Unity工程未改动。诊断弹窗用的两项临时计划任务已清理。
