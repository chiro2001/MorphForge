# 体素文字

本模块把真字体栅格化成体素，不把每个字符预先做成固定字模。内置
Noto Sans SC Regular，并提供只读系统字体目录。文字先变成前景覆盖率
mask，再按行合并成不重叠的长方体，最后输出普通 canonical `Box3`
操作；后续擦除、染色、分组、平移、镜像和发布都使用现有几何管线。

## API

`VoxelTextRequest` 定义在 `morphforge_core::text`：

```json
{
  "text": "你好\nMorphForge",
  "fontId": "builtin:noto-sans-sc",
  "fontSize": 32,
  "depth": 1,
  "origin": [0, 0, 0],
  "plane": "xy",
  "facing": "positive",
  "align": "left",
  "letterSpacing": 0,
  "lineSpacing": 0,
  "maxWidth": null,
  "styleId": 1,
  "tintId": 0,
  "motionPhase": 0,
  "group": "main"
}
```

- `origin` 是最终文字墨水包围盒的最小角，与 `Box3.min` 一致；三轴输出均位于
  `origin..origin+size` 的非负包围盒内。`maxWidth` 不等于返回宽度，排版后还会
  裁掉空白边距，`layout.width/height` 是实际墨水尺寸。
- `maxWidth` 是按字形步进（advance）计算的硬排印宽度：显式换行优先，超出的
  位置按 Unicode 字符边界换行。中文和拉丁文可以逐字符断行；当前不做单词级
  断行。最终墨水 bbox 可能因字形外扩与步进宽度有少量差异。
- `fontSize` 为栅格名义 em 像素高度，允许 `4..512`；`depth` 为挤出厚度，允许
  `1..256`。覆盖率阈值为 `>127`。
- `letterSpacing`、`lineSpacing` 可为 `-512..512` 格；负值使行高或字符步进小于 1
  时整体报错。
- 缺字是整体错误，不产生半套作品。错误类型是
  `morphforge_app::fonts::MissingGlyphsError`，可通过 `anyhow::Error::downcast_ref`
  读取 `characters`。HTTP 层可据此返回 `code: "FONT_MISSING_GLYPHS"`。
- 暂不支持 RTL、阿拉伯文、Indic shaping、组合标记与复杂连字，收到这些字符会
  明确拒绝。当前也不做 GSUB 连字替换。

`morphforge_app::fonts::FontCatalog` 提供：

```text
FontCatalog::load_builtin() -> Result<Self>
FontCatalog::load_default() -> Result<Self>
fonts::shared_default() -> Result<Arc<FontCatalog>>
catalog.list() -> Vec<FontDescriptor>
catalog.descriptor(fontId) -> Option<&FontDescriptor>
catalog.render_mask(&request) -> Result<TextMask>
catalog.compile(&request) -> Result<TextCompilation>
catalog.preview(&request) -> Result<TextPreview>
```

`shared_default()` 是进程级只读缓存。解析后的字体也按 `fontId` 做有界缓存，
同一字体不会在每次输入时重复解析。不会预加载全部系统字体；只有选中的字体会
解析。字体目录只接受 `fontId`，不向调用方暴露任意文件路径。

内置身份为 `builtin:noto-sans-sc`。系统字体身份为
`system:<字体文件SHA-256前16位>:<TTC face索引>`；同一 TTC 的每个 face 是独立条目。
目录条目包含 family、style、source、完整 SHA-256、faceIndex 和 license。

`TextPreview` 的精确形状：

```json
{
  "ok": true,
  "requestFingerprint": "64位小写SHA-256",
  "font": {
    "id": "builtin:noto-sans-sc",
    "family": "Noto Sans SC",
    "style": "Regular",
    "source": "builtin",
    "sha256": "faa6c9df...",
    "faceIndex": 0,
    "license": "OFL-1.1"
  },
  "layout": {
    "width": 120,
    "height": 64,
    "depth": 1,
    "voxelCount": 8421,
    "boxCount": 173,
    "bounds": {"min": [0, 0, 0], "max": [120, 64, 1]},
    "missingGlyphs": []
  },
  "operations": [
    {"op": "box", "min": [0, 0, 0], "size": [3, 1, 1], "styleId": 1}
  ],
  "warnings": []
}
```

`operations` 是可直接应用的 canonical 操作，不使用 Three 的 `(x,y,-z)` 视图转换。
`requestFingerprint` 是后端对完整请求序列化后的 SHA-256；前端应把服务端返回的
指纹与当前已发送的预览请求关联，不要在浏览器用另一种 JSON 编码重算后比较。

## 平面与朝向

文字 mask 原点在左上角。`positive` 是面向现有默认视图的可读方向：

| plane | 屏幕右向 u | 屏幕下向 v | 挤出正轴 |
|---|---|---|---|
| `xy` | `-X` | `-Y` | `+Z` |
| `xz` | `-X` | `+Z` | `+Y` |
| `yz` | `+Z` | `-Y` | `+X` |

`facing: "negative"` 只翻转 u 轴；v 轴和挤出方向不变。以上方向由
`text::tests::positive_facing_has_explicit_renderer_basis` 与
`fonts::tests::positive_facing_matches_the_renderer_view_basis` 用非对称字形和
三个相机基验证，不依赖视觉直觉。

## Recipe API

```js
mf.text("你好\nMorphForge", [0, 20, 0], {
  fontId: "builtin:noto-sans-sc",
  fontSize: 24,
  depth: 2,
  plane: "xy",
  facing: "positive",
  align: "center",
  maxWidth: 200,
  letterSpacing: 1,
  lineSpacing: 2,
  style: "metal",
  tint: 0,
  motion: 0,
  group: "sign"
});
```

Recipe worker 无文件系统和环境访问；它只记录有预算限制的 `text` marker，以及当时
的 `translate`/`mirror` 帧。父进程验证 marker 后，从共享只读字体目录展开为普通
Box，并保持 marker 在操作序列中的原位置。因此 `mf.text` 可以和其他绘制、擦除、
`clear`、分组操作组合。

## 预算

| 项目 | 上限 |
|---|---:|
| UTF-8 文本 | 16 KiB |
| Unicode 字符 | 4096 |
| fontSize | 4..512 |
| depth | 1..256 |
| 栅格面积 | 16,777,216 像素 |
| 合并 Box | 100,000 |
| 体素量 | 16,000,000 |
| 坐标/包围盒 | 根工程 `Limits` |

任一检查失败都返回错误，不产生部分操作。HTTP/编辑器应先 preview，确认
`requestFingerprint`、revision 和 Box 容量后再 apply。

## 内置字体来源

- 文件：`fonts/NotoSansSC-Regular.otf`
- 来源：<https://github.com/notofonts/noto-cjk/blob/165c01b46ea533872e002e0785ff17e44f6d97d8/Sans/SubsetOTF/SC/NotoSansSC-Regular.otf>
- 下载日期：2026-10-19
- 字体 SHA-256：`faa6c9df652116dde789d351359f3d7e5d2285a2b2a1f04a2d7244df706d5ea9`
- 许可证：SIL Open Font License 1.1，`fonts/OFL-NotoSansSC.txt`
- 许可证来源：<https://github.com/notofonts/noto-cjk/blob/a99a4354c68964f6bfac488d01010b1fb6d9178a/Sans/LICENSE>
- 许可证 SHA-256：`6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2`

内置字体被打包进程序，保证中文在没有额外系统字体的环境中也能预览和生成。

## 验证

```text
cargo test -p morphforge-core text
cargo test -p morphforge-app fonts
cargo test -p morphforge-app recipe::tests
cargo check -p morphforge-app --all-targets
```

测试覆盖：矩形合并与孔洞、ASCII/CJK、多行、缺字类型、复杂脚本拒绝、预算、
三平面、正反朝向、深度、换行/对齐/间距、mask 与 Box 覆盖一致、Recipe 顺序、
材质/分组以及 translate/mirror 帧。
