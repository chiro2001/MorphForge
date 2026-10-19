# 0.4 参考图验证

日期：2026-10-19。内置8张参考、上传与个人图库、两类独立开关、多图消息、队列快照、compact保持、来源记录与取消参考已实现。

## 自动与浏览器

Rust与Node检查摘要见automated-checks.json。真实Rust+Chromium+本机Chat协议服务验证图库选择、上传、关闭/组合来源、刷新恢复、图片仅随发送交给上游、每步附图、作品来源追溯、取消旧参考；[报告](provider-presets-browser.json)。原插话/打断、暂停、保护与字体回归报告在regression-validation.json。

首次测试发现已有artifact类型白名单不含WebP，补充后上传缩略图与不可变快照通过。无效图片、尺寸/字节保护、重复图片归一化去重、移出图库保留历史、参考不冒充当前作品截图均有测试。

## 真实DeepSeek与美术边界

使用两张已核实许可的女仆全身与V4F参考，标准预设、reasoning none、18步/300k预算。约69秒完成14个请求，290,192 Token，生成437 Box，6次真实截图、多轮修改及入库，未调用Files上传。[原始报告](live-reference/report.json)、[主视](live-reference/hero.png)、[正面](live-reference/front.png)。

**链路与保存通过，美术验收未通过**：角色脸和围裙被放在-Z面，而front/hero相机从+Z看；也超出了输入要求的400 Box。模型文字自评不能作为外观完成证据。

第二轮以明确的+Z面部约定续作，生成385 Box，脸和围裙已在front可见；但180k预算耗尽后停止，未把本轮强行标记为自动入库成功。[续作报告](live-reference-correction/report.json)、[正面](live-reference-correction/front.png)。已保留草稿、截图、实际usage和失败状态。

最后把相机约定加入get_context.viewConventions并在system4.1中明确：较大的Z更靠近front，脸、眼睛、围裙等应放在+Z外表面，颜色参考材质RGB值。该提示改动未冒充已经完成新的独立美术A/B。两轮的开发二进制system4.0与最终system4.1应区分；软件的图片输入功能已验证，复杂角色仍需继续优化提示词和工具。

## Windows候选

最终冻结、Windows原生与隔离桌面验证及包内容核对将记录在本目录。程序与图片分别许可；安装器携带CC许可全文与作者/原帖清单。默认安装和现有作品不由测试覆盖。
