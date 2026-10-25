# MorphForge 宣传片交付

当前请使用 [OSC / AI 建模新版](osc-workflow/RESULTS.md)：`D:\Movies\Projects\MorphForge-Promo-20261025-OSC`。104.8秒，主线已改为“自定义模型 → AI真实建模 → OSC实机逐步搭建”，附神经网络动态演示；保留精简文字与SC／JP字体。27项检查通过，旧版保留。以下为首次交付历史。

已完成并复制到 `D:\Movies\Projects\MorphForge-Promo-20261025`。40个交付文件逐项核对大小与SHA256一致，额外的清单文件为 `MANIFEST.json`。Linux副本在 `Releases/MorphForge-Promo-20261025/`，大体积视频和用户原片不纳入Git。

- `MorphForge-Promo-zh-CN-1080p60.mp4`：中文主片，96秒／1920×1080／60fps，约57.31MiB。
- `MorphForge-Promo-Multisub-1080p60.mp4`：相同音画，另有日文、英文可切换字幕；默认字幕表现由播放器决定。
- `MorphForge-Promo.zh-CN.srt`、`.ja.srt`、`.en.srt`：三语各16段字幕。
- `Cover.jpg`、`Storyboard.jpg`：封面和17个镜头的分镜总览。
- `timeline.json`、`render_promo_video.py`、`assets/`：可修改文案、镜头和速度后重新导出，包含重建所需截图、音乐、字体及许可。
- `PUBLISH-DESCRIPTION.*.txt`、`MUSIC-CREDIT.txt`：三语发布文案与必须保留的音乐署名。

选用了5份中文说明原片中的风车、材质、道具、缩放、小景观、像素画和字幕镜头，结合现有真实软件截图与原生点线验收图。按文件名要求筛选、加速、防抖；两处加速明确标注4倍／10.5倍。字幕原片的不完整日语段落已排除，保留完整可核对的短句，没有合成实机板上的文字。BGM为从作者官网下载的Kevin MacLeod《Cipher》，使用CC BY4.0并在片尾及文案保留署名。

13项媒体检查通过，包括完整音画解码、帧数、时长、字幕轨道、文本与时间戳、音频响度和峰值；Windows原片5项摘要复核一致。证据为 `verification.json`、`original-preservation.json`、`windows-copy.json`，剪辑取舍为 `EDIT_NOTES.zh-CN.md`，配乐来源为 `music-license.json`。

视频没有上传到平台或商品页；未运行付费模型或操作用户的主程序、Unity场景、VRChat客户端。
