# 私有源码仓库与可复现克隆

GitHub仓库：`chiro2001/MorphForge`，可见性为private。默认分支`main`，保留开发分支
`feat/harness-interaction-and-voxel-text`、基线标签和全部阶段提交。

```powershell
gh repo clone chiro2001/MorphForge MorphForge-source
cd MorphForge-source
git checkout v0.2.0-rc.2
python tools/release_manifest.py check docs/quality-v2/source-freeze.json
```

建议克隆到独立目录；现有Unity工程继续使用
`D:\Projects\VRChatProjects2\MorphForge`中的商业资源及生成资产。仓库管理工具、
Rust/前端、Unity源脚本、Shader、许可、文档和证据，未纳入Nemesis人物资源或密钥。
完整历史发布前的检查见[历史审计](quality-v2/git-publication-audit.json)。

Windows新克隆需保留仓库的`.gitattributes`。它让源码使用LF，即使本机Git配置为
`core.autocrlf=true`，69项冻结构建输入仍一致；报告和字体保留原始字节。
[实际Windows克隆验证](quality-v2/windows-git-clone-validation.json)已通过，
[首次RC1失败](quality-v2/windows-git-clone-rc1-failure.json)也保留供追溯。

离线完整历史包和SHA-256也在`dist/windows/MorphForge-0.2.0-rc.2.bundle`及同名
`.bundle.sha256`中。RC1历史标签和包保留，不移动已有标签。安装器仍为0.2.0，
源码行尾修复不改变该程序的69项构建输入。

真实客户端验收仍待用户确认；[最终Avatar](security-check-failure/FINAL-FULLFT.md)
已通过实际上传和服务端三项分析，候选源码版本不代表客户端/多人已验收。

## 0.3 软件候选更新

新功能分支 `feat/provider-presets-and-goals` 提供兼容 API、三档创作预设和有界 Goal。对应安装器为0.3.0，新的构建冻结清单与验证在 [quality-v3](quality-v3/RESULTS.md)。0.2 的Avatar验收状态与历史标签保持独立，软件升级不代表已完成客户端现场验证。
