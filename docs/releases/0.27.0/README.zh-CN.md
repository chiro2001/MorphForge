# MorphForge BOOTH 发布准备（2026-10-25）

参考相邻 `tesla-roadster-avatar` 的 TeslaRoadsterMA 1.2.0 发布组织方式，使用本项目素材和原始发行文件。Desktop **0.27.0**，Avatar 插件 **1.4.0**。本轮只准备材料，没有上传 BOOTH、推送 Git 或创建 GitHub Release，也没有更新在线 Avatar。

## 交付入口

生成目录：`Releases/MorphForge-0.27.0/`（已忽略，不提交安装器）。

- `BOOTH-Publish/index.html`：离线审阅页，含商品图、三语文案、下载和待办。
- `BOOTH-Publish/Download/MorphForge_Ver.0.27.0.zip`：上传给 BOOTH 买家的文件。
- `MorphForge-0.27.0-PublisherKit.zip`：发布者整套备份，包含展开的买家文件、商品页材料和内部校验。
- `BOOTH-Publish/ProductPage/`：中、日、英纯文本说明，五张默认商品图及中英文替代封面。
- `BOOTH-Publish/Checks/`：原包审计、原有多语言测试、额外导入状态、离线浏览检查和本轮包装检查。

同一套文件复制到 Windows 工程的 `D:\Projects\VRChatProjects2\MorphForge\Releases\MorphForge-0.27.0\`。复制结果见 `windows-copy.json`，不改动 Assets、已安装主程序或用户作品。

## 内容与验证边界

Windows 安装器原始 SHA256：`7d763ea397bf853c920f0005878e521c2b8f90607672f0e629dac511234e9979`。

Unity 原包 SHA256：`7e60412f16cbc3e9858e7c85c7d02ec9c16d15af94f6c37805190b59c5a39264`。162项与源码一致，不含私人绑定、人物或用户 Variants。544项配套材质目录与插件源 JSON 一致，54张 PNG 内容摘要正确。

本轮没有重新构建运行时。此前0.27.0/1.4.0的开发工程测试保持有效：320项Rust、67项前端、24项Chromium、19项Windows原生、3项原生重启、30项Unity/SDK检查。包装检查结果见 `verification.json`；离线审阅页的5项实际Chromium检查见 `offline-browser.json`。

**额外干净导入未完成**：独立 Unity 验证 Editor 在授权连接/启动阶段未暴露 Pipeline。没有执行原包导入，也没有接触原开发工程资产。详见 `clean-import.json`。本次创建的临时启动任务和未就绪 Editor 已清理，原来运行的两个 Editor 保留。不能将开发工程通过的30项等同于这次额外导入通过；也不能将包装检查等同于多人实机验收。

商品图来自隔离0.27.0工作台、项目自制MIT样例和Pillow排版。`media/sources.json` 保留原始截图摘要，`marketing/sources.json` 保留成品图来源。截图由Chromium打开相同内嵌工作台采集，不是Windows原生窗口或VRChat实机截图。没有调用付费模型、真实录音或OSC。临时浏览器和隔离服务均在采集结束后关闭。

## 发布前仍需决定

`booth-publish-config.json` 当前是 **0円免费版草案**，价格尚未由发布者确认。最终商品条款与支持联系方式也待确认，文案末尾明确保留这些事项。程序源码元数据声明MIT不等于整个合集重新授予MIT；特别是安装器中的鲸鱼参考与鲸鱼娘二创示例保留CC BY-NC-SA 4.0，字体和第三方库保留各自条件。若改变发行方式，应先处理附带素材的原始条件。

还需在可正常启动的隔离Unity工程重试导入，并按买家指南人工复验目标Avatar、VR环境与其他客户端。当前公共1.4.0没有完整多人实机验收记录。价格与条款确定后更新配置、三语文案及 `LICENSE.txt` 生成逻辑，再重新构建和校验；不能只删除待确认提示。

## 许可材料

`Licenses/` 收录项目既有许可、参考图和示例署名、Noto/Unifont、三维前端、语音组件和Cargo依赖材料。Cargo清单包括两个锁定依赖图的623个注册表包，包含构建、测试和非Windows依赖，不宣称这些都链接进Windows二进制。

优先使用crate自带许可；缺失时保留对应VCS提交的上游许可。少数无单独上游文件的包使用其原始源码声明或Cargo作者/许可元数据与对应许可全文，并记录来源，未捏造版权归属。5个MPL依赖及使用声明回退的依赖附原始crate源码归档，逐个核对Cargo.lock注册表校验值。原始记录在 `Licenses/Rust-dependencies.zip` 的 `Cargo-dependencies.json`。

## 重建

先准备上述确切SHA的两个原始二进制和匹配目录。编译器变化导致二进制变化时应重新验收并更新冻结输入，不能绕过脚本摘要检查。

```sh
cargo metadata --format-version 1 --locked > /home/chiro/tmp/morphforge/booth-cargo-core.json
cargo metadata --manifest-path desktop/Cargo.toml --format-version 1 --locked > /home/chiro/tmp/morphforge/booth-cargo-desktop.json
python3 tools/prepare_booth_licenses.py --metadata /home/chiro/tmp/morphforge/booth-cargo-core.json --metadata /home/chiro/tmp/morphforge/booth-cargo-desktop.json
# 配图排版需要Pillow和系统NotoSansCJK字体；既有真实截图无需重新采集。
python3 tools/build_booth_media.py
python3 tools/build_booth_publish.py
python3 tools/check_booth_publish.py --seal
```

构建脚本只替换带有本脚本生成标记的发布目录；不会操作BOOTH。`--seal` 将最终包装验证写入发布者目录并更新整套摘要与PublisherKit。
