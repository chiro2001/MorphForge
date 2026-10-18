# 本机网页体素编辑器

> 历史 Python/MF2 原型。当前 Rust/MF3 共享工作台见 [用户指南](../USER-GUIDE.md)，不要把本页8765端口与MF2格式套用到新版Avatar。

2026-10-19。Windows 本机浏览器打开 **http://127.0.0.1:8765**。这是本地网页服务，无需上传到公网；默认只预览、关闭 OSC。

## 操作

1. 使用本轮 Stage3 Avatar。上传并切换模型后，目标选 `VRChat · 本机 9000`，版本选“定位球版 · 底部中心”。旧 MF1 双阶段 Avatar 不能用本编辑器；“Stage2 兼容”仅支持上一版 MF2。
2. 点击“开启实时显示”。默认每条命令 0.25 秒。网页只知道本机发包完成，没有 VRChat 或远端 ACK。
3. 左键点击网格放置，点击方块表面继续向外搭；Shift＋点击擦除。也可选择擦除工具、修改长宽高，或展开精确坐标输入。右键旋转、中键平移、滚轮缩放。
4. 支持撤销／重做、阶梯／小屋、16–256 等网格尺度、JSON 导入／导出。实际网格也支持 8。
5. 右侧可控制世界固定、显示定位球、缩放、移到面前。Avatar 菜单内的修改不自动回读网页；网页预览保持在本地原点，不显示真实手或锚点世界坐标。
6. 遇到丢包，点击“重新显示”清空再重建当前草稿。修改结束可“停止发送”。

连接时先发送两个不同序号的 Clear，再发送定位设置和完整目标；会替换当前 Avatar 中已有体素。关闭实时编辑开关后，点击“发送当前草稿”手动提交；点击“清空模型”会发送 Clear。

## 文档、几何与发送器

Python 标准库后端 `tools/editor_server.py`，Three.js 0.180.0 静态资源随项目本地提供，启动无需 CDN。服务只监听回环地址；允许本机 VRChat 9000 和 Unity 9012 两种 OSC 目标。Unity 目标需要 Stage3 Play Mode 中的 `PlacementEntry.StartOsc`；不向 9012 发包就不能预期 Editor 出现方块。

文档存互不重叠的稀疏盒体，自动合并完整相邻同样式面；256³ 全实心仅一条盒体，不分配稠密数组。任意区域擦除由外部相减拆成最多六片，再合并。超过当前 Avatar 64 个合并盒体容量会拒绝整次编辑，保留旧草稿；不同样式会阻碍合并。64 是显示槽位上限，不是总体素体积。

OSC 使用一个独占写入线程与共享锁，连接期间持锁；后台草稿编辑不会等待逐条 OSC。发送器维护最新目标及已发送台账：先删除过时几何，再补新盒；同一盒几何仅颜色变化可直接 SetBox。发送中继续编辑会合并后续任务并最终收敛，不要求排队重播每个过期草稿版本。发送不是原子场景替换，拆片过程中可见暂时缺口。队列、定期刷新、ACK、完整 AI 后端仍不是全部实现。

草稿和定位设置自动保存在 Windows `Docs/Editor/document.json`；重启仍为断开发送状态。撤销历史仅保留当前运行期最近 32 次。导出格式 `MorphForgeEditor/1`，X/Z 线上坐标中性点 128，Y 底面 0；网页精确位置输入使用 X/Z 相对中心的有符号格坐标。

## 启动与停止

在 Windows 工程目录：

```powershell
pwsh -NoProfile -File tools/start_manual_editor.ps1
```

按需启动任务 `MorphForge-ManualEditor`，没有定时／开机触发，不会重启 VRChat。日志 `Logs/manual-editor.log`。

也可前台运行：

```powershell
python -u tools/editor_server.py --listen 127.0.0.1 --port 8765
```

停止服务：创建 `%USERPROFILE%\tmp\morphforge\editor.stop`。通常测试只需网页“停止发送”，可保持网页服务供下一次使用。

## 验证

- `python3 tools/test_editor.py`：8 项模型／发送器测试，含 250 次随机绘制／擦除与独立稠密格集合比对、容量拒绝原子性、大盒、合并、存档、独占失败、节拍与最终收敛。
- `browser-validation.json`：真实 Chromium 鼠标操作、Shift 擦除、撤销重做、示例、无 JS/WebGL 错误；截图 `stairs-browser.png` / `house-browser.png`。
- `unity-integration.json`：Windows 网页服务→UDP9012→Gesture Manager→MF2 FX→原生粒子；包括长方体、底部原点、缩放、定位开关、拆片挖洞和示例。

测试浏览器使用 Linux ChromeDriver 与 SSH 回环转发；不是对真实 VRChat 客户端的替代。启动 ChromeDriver 后可运行 `tools/check_editor_browser.py` / `tools/check_editor_unity.py`。后者会操作草稿、临时连接 9012，结束停发并把目标恢复为 9000。

## HTTP API

JSON 请求均在同源 `/api/` 下；目标服务绑定本机。

| 方法与地址 | 内容 |
| --- | --- |
| `GET /api/state` | 草稿、版本、容量、连接和本机已发送状态 |
| `GET /api/export` | 当前导出文档 |
| `POST /api/edit` | `op=box/erase/clear/size/undo/redo/import/demo`；box/erase 使用 `min,size,styleId,tintId` |
| `POST /api/connect` | `enabled,port,profile,interval,auto`；port 9000/9012，profile anchor/stage2 |
| `POST /api/publish` | 冻结当前草稿为发送目标；`resync:true` 清空后重建 |
| `POST /api/placement` | `world,visible,scale,reset`；可局部修改 |
