# BOOTH／闲鱼发布资料

最新文案按用户反馈改为能力与玩法导向：先介绍可创作的模型、OSC展示、抓握摆放、动画、图示和字幕，再集中给出规格与许可。BOOTH中日正文、闲鱼正文、YouTube介绍和3张商品图已同步，Windows入口保持原路径；文案审查见 `copy-review.json`。

Windows目录：`D:\Movies\Projects\MorphForge-Marketplace-20261025`，先打开 `index.html` 离线审阅。
Linux副本：`Releases/MorphForge-Marketplace-20261025/`。本轮没有在线发布或上传视频。

按用户保存的 `Edit Item - BOOTH.html` 读取并整理了Digital类型、标题、商品图、分类、全年龄、介绍、嵌入视频、标签、活动、价格、作品文件和保存入口；原始含账号状态的HTML只留临时私有副本，不放发布包或Git。公开资料仅保留可见栏目和页面摘要。

已按用户明确要求准备BOOTH免费0日元＋100日元自愿支持两档，绑定相同买家ZIP，功能与内容相同；闲鱼按0元准备。两平台文案和买家Readme都附项目源码地址。公开页面抓取能返回仓库内容，但未确认公开main与本地0.27.0一致，本次没有推送或创建Release。

## 使用文件

- `BOOTH/`：日文标题、日文正文、中文对照、6张JP字体商品图、日文111.2秒1080p60视频、YouTube标题／介绍与嵌入步骤、`Fields.zh-CN.md` 和 `Fields.json`。
- `Xianyu/`：中文标题、正文、6张SC字体商品图、相同内容的中文111.2秒1080p60完整版与1080p30轻量版（约40.29MiB）、源码地址和填写参考。
- `Download/MorphForge_Desktop0.27.0_Avatar1.4.0.zip`：两平台共用买家文件，38,702,748字节，约36.91MiB。免费与支持档都使用这一份。
- `Shared/`：三语可选字幕与必须保留的配乐署名；`Checks/`：表单来源、图像来源、离线与媒体校验及发布事项。

商品图突出自定义模型、真实AI→OSC流程、6件作品、VRChat玩法、制作与展示能力及实际多语言界面。编辑器预览与VRChat实机来源分别标注；原始GUI内容保留，新增中文用SC、日文用JP。未生成或伪造软件画面。

买家ZIP在原验收包基础上只更新Readme、价格／源码和许可索引等文字及摘要。Desktop0.27.0安装器SHA256仍为 `7d763ea397bf853c920f0005878e521c2b8f90607672f0e629dac511234e9979`，Avatar1.4.0仍为 `7e60412f16cbc3e9858e7c85c7d02ec9c16d15af94f6c37805190b59c5a39264`。配套材质目录和原始依赖许可保持不变。新ZIP SHA256：`28cf3df5b993a8ae3375d039ea3d6df556e792bd9ed11c69b9d28d8cc7d82b09`。

26项包装／媒体检查与6项实际Chromium检查通过。完整版视频与上一轮成片字节一致，沿用已通过的完整解码／字体／树镜头检查；仅对新轻量版做全片解码与音频检查。浏览器使用临时独立会话，12图加载、两个111.2秒视频元数据、价格及源码显示、桌面／手机布局和Console均正常。Windows复制后52个文件逐项核对大小和SHA256，外加匹配的MANIFEST。证据为 `verification.json`、`offline-browser.json`、`visual-review.json` 和 `windows-copy.json`。

## 发布前的具体事项

BOOTH介绍视频需先上传视频托管平台后，将真实嵌入代码填入Embedded Widget；本轮提供模板但没有虚构URL。保存页的Add/Edit Files是买家文件入口，不等于商品介绍视频已经显示。

现有安装器保留CC BY-NC-SA鲸鱼参考／二创示例，100日元可选支持不会重授这些素材的权利；支持档分发方式的许可事项已单列，不作“自愿支持必然无商业用途”的结论。试用刺猬尚无公开链接，未在商品页虚构上线状态。

没有用户当前的闲鱼发布页，未核实该账号的类目准入、0元提交或视频上限。资料按0元和公开源码说明准备，不代改收费价、不用不相干类目或二维码绕过。具体见 [PLATFORM-NOTES.zh-CN.md](PLATFORM-NOTES.zh-CN.md)。支持渠道采用各平台站内消息，没有添加私人电话、地址、微信或未授权服务承诺。

## 重建

在完整仓库和已验收成片存在时运行：

```sh
/home/chiro/tmp/morphforge/booth-venv/bin/python tools/build_marketplace_media.py
python3 tools/build_marketplace_publish.py
/home/chiro/tmp/morphforge/booth-venv/bin/python tools/check_marketplace_publish.py --work /home/chiro/tmp/morphforge/marketplace-20261025/verify
```

大体积安装器、视频和生成的买家包留在忽略的Releases目录；正文、价格配置、表单提要、商品图、校验记录与生成工具纳入Git。未操作主安装、Unity、VRChat或其他商店账号。
