# 当前候选源码与完整历史

2026-10-19最新包为`MorphForge-0.2.0-rc.1.bundle`，附同名`.bundle.sha256`。
它包含所有阶段提交、Avatar材质/场景/SDK校验修复及最终FullFT上传通过证据。
候选标签`v0.2.0-rc.1`；真实客户端确认后再建立最终`v0.2.0`。

在独立新目录执行：

```powershell
git clone .\MorphForge-0.2.0-rc.1.bundle MorphForge-source
cd MorphForge-source
git checkout v0.2.0-rc.1
```

不覆盖当前Unity工作区。商业Nemesis资产与生成的完整Avatar继续只在Windows工程中。

下面保留较早的局部历史快照说明；它不包含后续修复。

# 可审查源码快照

快照：`a9e2c08e9ef37397114f73c87ff97101547ff6bd`，分支`feat/harness-interaction-and-voxel-text`。

文件：`MorphForge-review-a9e2c08.bundle`，包含基线、所有阶段提交、源码、许可字体、成功/失败评测与4096容量对照生成脚本。没有商业Nemesis资产、API Key或用户工程数据。没有推送远端。

SHA-256：`5e183eb40ee70f65ae0d888346841d41f27f312782c1ee8c1f699ecf77c6207f`，校验副本在同名`.bundle.sha256`。

在新的源码目录中查看：

```powershell
git clone .\MorphForge-review-a9e2c08.bundle MorphForge-source
cd MorphForge-source
git log --oneline --decorate
```

bundle已通过`git bundle verify`、独立clone和`git fsck`。它是可审查快照；最终`v0.2.0`标签仍等待当前Stage3客户端安全检查问题收尾。Unity测试Prefab/场景只留在原Windows工程。
