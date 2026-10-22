# MorphForge点阵字库

使用 GNU Unifont 18.0.01（默认中西文字形）及 Unifont Japanese 18.0.01（日文字形），选择 SIL Open Font License 1.1 授权分发。保留原作者版权与许可；衍生打包资源以 MorphForge Pixel16 标识。

Copyright (C) 1998–2026 Roman Czyborra, Paul Hardy, Qianqian Fang, Andrew Miller, Johnnie Weaver, David Corbett, Ælla Chiana Moskopp, Rebecca Bettencourt, Minseo Lee, Ho-Seok Ee, et al.

来源：https://unifoundry.com/unifont/index.html

源HEX/BDF由`tools/build_pixel_font.py`无损转换为逐码点32字节二值字形；每行两个字节、高位在左。全BMP位图每字体2MiB，宽度表64KiB；位图图像本身无灰度。zh/jp不是操作系统字体调用，不依赖安装商业字体。未涵盖的BMP外字符显示缺字提示。

源文件：`unifont-18.0.01.hex.gz`、`unifont_jp-18.0.01.bdf.gz`；重建参数见脚本。相关许可证见`UNIFONT-OFL-1.1.txt`与`UNIFONT-LICENSE.txt`。
