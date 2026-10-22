# 本轮交付验证

2026-10-22：Desktop0.21.1 / Avatar1.2.0。源代码、安装包和Unity包均已形成；不发布GitHub Release，不自动上传Avatar。

| 验证 | 结果 | 证据 |
|---|---|---|
| Rust全量 | 283项/30套通过 | rust-tests.txt |
| Windows成品内核 | 12项通过，含内嵌前端哈希/首次OSCQuery识别/动画解绑和改稿范围 | windows/validation.json |
| 实际Tauri mini | 15项通过 | ../stage-d-mini/v0211/validation.json |
| Windows Cable离线语音 | 33项通过 | ../stage-c-speech/v021/validation.json |
| 动画按钮实际Chromium | 13项通过 | browser/validation.json |
| 纯文本动画/GLB | 9项通过 | text-browser/validation.json |
| 原生接收/时钟/GPU/清空恢复 | 四阶段整套通过 | public/native-motion-suite.json |
| 单次动画末帧 | 从半相位开始且停在末帧，GPU宽64像素 | public/native-motion-once.json |
| 三角面/文字共享变换 | 三角宽160→113，位移64px；文本90度边视消失 | public/native-motion-surfaces.json |
| 本人预览原型 | 8项通过，包含自身RT排除逐像素一致 | public/local-preview-owner-play.json |
| 公共Unity包 | 158项匹配，无重复GUID/私人Avatar绑定 | package-audit.json |
| 主安装 | 正式NSIS、数据备份及哈希校验 | install-report.json |

Avatar独立模块合并预算253/256bit，240bit数据Int保持直接同步。Unity已回到原MF6场景、Edit Mode、单Avatar、无未保存内容。其他人物参数未改动，未连接真实VRChat上传/发包。

VR头显舒适度、第一人称/镜子/拍照的真实客户端体验和远端网络可靠性仍由实机体验确认。本人预览是默认关闭的实验功能，可能包含取景内人物/世界；此轮没有实现SteamVR HUD、通用物理碰撞或网络执行ACK。
