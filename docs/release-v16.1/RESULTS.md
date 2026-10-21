# 0.16.1 发布验证

日期：2026-10-22。重新构建 Windows x64 NSIS 安装包，发布独立 Avatar 1.0.0 插件与配套公共材质目录。本次没有更新主安装或上传 Avatar。

- 231 项构建输入在 Windows 构建前后与 Linux 冻结清单完全一致。[输入清单](../../dist/windows/MorphForge-0.16.1-build-inputs.json)。
- 安装包独立解包；核心版本为 0.16.1；隔离数据启动正常，OSC 关闭；内嵌 renderer.js、linear-compositor.js、studio.html 的 SHA-256 与源码一致；8 张内置参考图目录可读。[包验证](package-check.json)。
- Web 前端 58 项 Node 测试通过；Chromium 14 组透明度测试最大误差 0.00405，截图生成与相机恢复通过。[透明度测试](web-alpha.json)。
- Unity 插件包 133 项资产与源码一致，544 项材质定义和 54 张原创源贴图；未含第三方包源码、用户材质变体或私人 Avatar 绑定。插件干净工程、预算与 Unity 渲染验证见[插件验证](../avatar-plugin/README.md)。本次复用该已验证的 1.0.0 包，未重新生成 Unity 资源。
- 构建脚本支持 CARGO_TARGET_DIR，打包前检查核心版本，打包后检查核心哈希，避免构建目录重定向时混入其他版本。

安装包 18,315,884 字节。附件哈希见 [SHA256SUMS.txt](SHA256SUMS.txt)。独立解包验收未执行安装器，因此没有改写现有安装、用户工程或模型配置；未请求上游模型或向 VRChat 发送数据。

本版未更改 MF6 协议；没有新增贴图文本或动画功能。透明排序与远端漏收限制仍见 Release 说明。
