# MorphForge 0.3 软件候选验证

日期：2026-10-19。开发分支 `feat/provider-presets-and-goals`。本次实现自定义 Chat Completions / Responses 连接、内嵌图片、三档中英创作预设、可配置大上下文、有界 Goal 与 README 展示图。

## 本机验证

- Linux Rust：86项通过；包含新协议、流截断、图片/工具顺序、真实usage及缺失回退、Key隔离与日志脱敏、Goal复核/无进展/compact测试。Clippy `-D warnings`、Rust格式检查通过。
- Node：24项通过。真实 Chromium 的原有交互回归31项通过，覆盖插话、打断、暂停、保护分组、文字预览和撤销；[报告](mock-validation.json)。
- 真实 Rust + Chromium + 本机 Chat JSON 服务：12项通过。实际执行 Recipe，生成9 Box小屋；双视角渲染通过data URL进入下一工具循环，未调用 `/files`，实际usage累计4400，finish一次提交和入库；[报告](provider-presets-browser.json)。这是协议测试，不是模型美术评分。
- README 工作台截图为新版界面实际载入旧星港791 Box，截图未修改UI或伪造建模过程；[原PNG](readme-workbench.png)。两张作品图取自旧真实DeepSeek记录，压缩WebP副本共约57KB，三张展示图合计约162KB；原PNG保留。

自动检查的原输出摘要与SHA256见 [automated-checks.json](automated-checks.json)。原始命令日志位于本机 `~/tmp/morphforge/v3-*.txt`。

## 真实 DeepSeek 流程与提示词迭代

使用隔离数据目录、真实DeepSeek Responses、内嵌截图及精细预设，缩减为24步/350k Token/600秒/8192输出。约58.3秒完成21步、8轮不同几何截图和70 Box作品入库。实际usage379,453：第20步累计349,027，剩余973作为最后一次输出上限，最后完整响应的实际输入跨过总门限；随后不再发起模型请求。这是已记录的“一次完整响应可能跨线”边界，不是350k精确账单硬上限。

[输入/设置/结果](live-goal/report.json)、[事件](live-goal/events.jsonl)、[场景](live-goal/scene.json)、[正面](live-goal/front.png)、[主视图](live-goal/hero.png)均保留。真实流程中发生一次跨Recipe引用变量导致的失败，模型自行调整继续。没有调用专用Files上传接口，事件中没有内嵌base64正文。

**该轮流程验收通过，美术验收未通过。** 人工审图认为入口/门窗的可见性仍不够清晰，模型自评偏乐观。不能把Goal achieved视为独立审美证明，也不能把截图次数当质量分。最终system从3.0迭代到3.1：明确每次Recipe独立执行、不保留变量；按front逐项核验可见元素，无法满足的项目必须如实说明。保留原3.0开发二进制hash，未将其冒充最终3.1美术评测。

三档完整同题、多种模型的速度/美术A/B尚未执行；预设是可预期的创作取向和消耗上限，不承诺固定耗时或质量。新增预算较大时，实际费用仍由所选服务的计费决定。

## Windows与交付

Windows x64的0.3.0 NSIS候选安装器已构建。78项冻结输入在构建前后及最终检查中完全一致；[冻结清单](source-freeze.json)对应运行时代码提交 `08072cf`。

- Windows核心86项、桌面2项测试通过；[构建摘要](windows-build.json)。
- 隔离原生HTTP验收59项通过，包含Recipe子进程、字体、16k几何、嵌入前端和许可hash；[报告](windows-native.json)。首次检查被旧脚本固定69输入拦住，改成可配置预期数量后重跑通过；[初次报告](windows-native-initial-failure.json)。
- 隔离Tauri副本13项通过：真实交互会话窗口、0.3.0核心、预设/Goal、连接Key不回显、本机私有配置、最终system3.1/tools3.0、正常关闭与子进程清理；[报告](desktop-candidate.json)、[截图](desktop-candidate.png)。测试未运行安装器或修改既有用户安装。
- NSIS通过解包核对：包内核心SHA256与已验收原生核心一致，字体OFL许可与冻结来源一致。包大小13,714,846字节，SHA256为 `2932050758575efe2029ff2f16436bccc43f38c4dbb95155a2f1f81bff75d7ae`。保留0.2历史安装器；没有冒充本版进行了全新安装/卸载验收。

本轮不重传Avatar，也不将软件验证替代仍待用户完成的客户端、多人与抓取验收。
