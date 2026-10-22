# 点线演示交付验证

2026-10-22：Desktop **0.22.0** 已通过正式 NSIS 更新 gamepc 主安装并重新启动。主窗口可见，原项目保留，OSC关闭，AI空闲。Avatar继续使用1.2.0，未改接收协议或新增同步参数。本轮未上传Avatar、未创建GitHub Release。

功能包含点、点集、线段、折线、箭头、圆环、XYZ坐标轴和线框盒；点直径、线直径、坐标、颜色、分组、线型可配置。顶部“演示”打开独立实际几何预览，“预览组合演示”提供一套可直接加入作品的示例。支持追加、替换同名组、保存、撤销、导出及既有分组动画。

| 验证 | 结果 | 证据 |
|---|---|---|
| 图元几何算法 | 5项通过；轴向/斜向、粗细、箭头端点、真实虚线空隙、折线连续相位及异常输入 | `crates/morphforge-app/src/drawing.rs` |
| HTTP与持久化 | 4项通过；预览不改场景、重复请求、重开、撤销、保护组、版本、AI锁、GLB与范围回执 | `crates/morphforge-app/tests/drawing_http.rs` |
| 已有Rust回归 | 原有128项lib、2项动画传输、48项provider_harness、4项text_http通过 | 下述测试命令 |
| 实际Chromium | 16项通过；预览、尺寸/粗细、添加、组合、替换、完整取景及资源释放 | `browser/validation.json` |
| 实际Windows Tauri/WebView2 | 14项通过，含内嵌文件哈希和整数MF6顶点检查 | `native/validation.json` |
| 构建输入一致性 | 276文件，Windows构建前后与Linux一致 | `source-freeze.json`、`windows-build.txt` |
| 正式主安装 | 14项检查通过，NSIS有效载荷哈希、注册版本、桌面入口与原数据保留 | `install-report.json` |
| 主程序启动 | 0.22.0、演示入口、8种图元、原项目、主窗口可见 | `main-smoke.json` |

执行过的Rust命令为`cargo test -p morphforge-app --lib`、`cargo test -p morphforge-app --lib drawing::tests`、`cargo test -p morphforge-app --test drawing_http`及`cargo test -p morphforge-app --test avatar_motion_transport --test text_http --test provider_harness`，均使用项目`tools/cargo_local.sh`和`--offline`。首次lib回归132项包含4个新增算法测试，随后补充连续虚线相位用例后重跑5个算法测试；不重复累计测试数。

修复了新增图元后相机可能先于场景加载而取景的问题；浏览器和Windows原生验收均检查新作品完整入镜。所有自动验收使用隔离数据与独立程序副本，没有调用付费模型、现实麦克风或VRChat OSC。

主安装前后原数据共11,912个文件、2,809,559,121字节，整体哈希一致。完整备份位于`E:\tmp\morphforge\main-backup-v022-drawing`。安装后启动程序会正常更新应用状态文件，安装保留数据的哈希比较在启动前完成。

限制：顶点使用现有整数网格，取整误差与退化面有回执；线宽是实际三维粗细。点为低面数球或方点。保存后是普通三角面，原始参数不作为独立可变形对象保存，可用重新生成并替换同名组来更新。未新增固定屏幕像素宽度、抗锯齿点精灵或原生圆柱协议。真实VRChat客户端和远端网络效果未在本轮重新实测。

本轮源码以`b9e2548`为基线。构建确切内容以`source-freeze.json`的文件哈希为准；其中gitCommit记录冻结时基线，交付元数据将记录包含全部冻结输入的功能提交。
