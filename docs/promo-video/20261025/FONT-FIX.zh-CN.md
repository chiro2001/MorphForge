# 宣传片中日文字体修正

用户指出中文可能使用了日文字体。核对确认原渲染脚本读取 `NotoSansCJK-Regular.ttc`／`Bold.ttc` 时未选择字体面，两者默认面0均为 `Noto Sans CJK JP`；因此原片新增的中文标题、说明与封面使用了日文字形。

现在按实际字体族查找字体面：中文常规／粗体为 `Noto Sans CJK SC`，本次文件对应面2；日文常规／粗体为 `Noto Sans CJK JP`，对应面0。混排中的「日本語」和三语界面的日文标签单独使用JP。缺少所需字体面会明确报错，避免静默选用其他语言。规则同时写入AGENTS.md。

中文主片、多语字幕版、封面、分镜已重新渲染。三语SRT的文案和时间不变，新增三语ASS，其样式明确指定区域字体；MP4字幕字体表已核对包含所需字体族。另导出 `MorphForge-Promo-ja-1080p60.mp4`，将日文字幕用JP字体烧录进画面，播放无需依赖播放器字体。SRT本身不记录字体，其显示字体仍由播放器选择。原录制内容和软件截图内的文字保留实拍像素，没有覆盖或替换。

检查结果：原有13项媒体检查、12项字体／字幕检查、4项日文烧录版检查全部通过。核对了真实字体族与粗细、SC／JP在同一组汉字上的不同字形、字幕文本与时间保持、MP4字体表、libass实际选中JP、无缺字日志与完整解码。证据见 `font-fix-verification.json`、`font-fix-typography.json`、`font-fix-japanese-video.json`。

修正版输出为 `Releases/MorphForge-Promo-20261025-FontFix/`，交付到 `D:\Movies\Projects\MorphForge-Promo-20261025-FontFix`。`Font-Comparison.png` 是区域字形对照，`Japanese-Subtitle-Proof.jpg` 是成片抽帧。旧版留在原目录供对照，未上传到平台或商品页。
