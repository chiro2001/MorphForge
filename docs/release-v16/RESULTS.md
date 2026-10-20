# 0.16.0 正式安装与发布

日期：2026-10-21。本轮用户明确要求升级gamepc主程序并发布GitHub Release。

## 正式安装包

使用已验收0.16.0的核心与桌面二进制，Tauri独立bundle命令生成正式NSIS安装包；没有把测试副本直接复制覆盖主程序。229项输入冻结核对通过，清单现在同时覆盖内嵌材质目录与许可。运行功能与[0.16验收版](../stream-v16/RESULTS.md)一致。

文件：`MorphForge Desktop_0.16.0_x64-setup.exe`，18,319,713字节。GitHub上传时将空格规范化为句点，实际下载名为`MorphForge.Desktop_0.16.0_x64-setup.exe`；Release中的校验清单按实际下载名生成。

SHA-256：`046cafbd09f9a37ef724926ea0a3bebc0767eeb817c3137343ddd12c5c4b2cb4`。

NSIS会调整shell内的安装包标记。已独立解包，对安装后的核心和shell分别验证：

- 核心：`40282db83401d43e00fd7633fbc53358641ef1c181f2dff855837574b3fefa22`
- 安装包内shell：`02f27379dde71929e5b370c57d759b2eafc323a213ae0010f42ecc6c0efa8f7c`

公开安装包不包含人物商业资源、原版Minecraft JAR/PNG或用户密钥。完整包与实际安装证据见[package.json](package.json)、[main-install.json](main-install.json)。

## gamepc 主程序

安装前主程序已经关闭；仍在运行的测试副本处于idle，保持运行。主安装更新至0.16.0，Windows安装注册、桌面和开始菜单入口均验证。

更新前完整备份：

`E:\tmp\morphforge\release-v0160\backup-main-20261021-074248`

C盘剩余不足1GB，本轮为安装工具增加可指定的备份目录，完整备份使用E盘。安装前后6955个原数据文件、1,422,596,499字节的组合SHA256一致；原作品、对话、API配置及历史未被安装器改写。

安装验证后，只补充当前私人Avatar的材质目录缓存、内容寻址PNG和原版方块状态索引，保留当前选中的旧目录，不复制测试副本工程／上下文。自动发现新Avatar后可匹配该缓存。[资源缓存记录](main-material-cache.json)。

随后正常启动主窗口，确认0.16.0、原项目ID与revision、真实原生窗口及新工作台入口；OSC保持关闭。独立临时工作区还使用安装后的`morphforge.exe`验证了Recipe子进程：生成1个Box、24格，未修改用户作品、未调用模型或发送OSC。[启动验证](main-startup.json)。

## SSH复查

5次新建IP直连和5次别名连接全部成功，本次未复现之前的连接重置。IP连接没有继承`gpu5080`别名中的身份与连接复用配置；别名有`ControlMaster=auto`、1800秒连接保留，实测延迟也较低。

远程工具现在默认使用`gpu5080`；可由`MORPHFORGE_SSH_HOST`或`--host`覆盖。保留`MORPHFORGE_SSH_CONTROL`显式控制socket能力。没有更改gamepc的SSH服务器配置，也不能仅凭本次成功就认定此前服务器重置的原因已完全查清。[连接检查](ssh-check.json)。

## GitHub

版本：`v0.16.0`，正式Release。安装器、校验清单和229项构建输入清单随Release提供；源码由版本标签提供。完整说明见[RELEASE-NOTES.md](RELEASE-NOTES.md)。
