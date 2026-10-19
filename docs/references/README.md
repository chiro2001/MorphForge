# 参考图片与内置图库

在「AI创作 → 参考图片」中使用「参考图库」或「添加图片」。分别开关“使用图库图片”和“使用当前图片”，可以选择任一来源、组合两者或全部关闭。选择图片后可以不写提示词直接开始，也可用文字指定参考哪些轮廓、配色和结构。参考选择对下次输入生效，选中后会继续用于续作，取消后不再把旧参考图片附带到新请求中。

内置8张：项目参数化生成的木椅、松树、小屋、石拱桥、灯塔，以及3张有明确原帖来源的鲸鱼娘形象。支持PNG、JPEG、WebP与GIF首帧，每张原图最多8MiB、8192边长/1600万像素；本机归一化为最多1024边长的PNG并去除EXIF。每次最多6张、合计最多16MiB。上传导入的图片留在“我的图片”中，可再次从图库选取；移出图库保留已关联运行的历史文件。

图片先保存在本机，只有发送选中参考时交给上游。设置中关闭图片发送后，携带参考的新输入会明确拒绝，不会静默按文字生成。非DeepSeek官方接口仍使用内嵌图片，不依赖上游Files接口；需要支持视觉和工具调用的模型。

## 连续创作与记录

消息接收时把选中图片冻结为不可变artifact，跟随requestId参与幂等检查和队列、插话/打断。当前参考独立于会话压缩存储，每次模型请求附带当前参考，并明确区别于真实作品截图。compact之后继续附带，重启不会自动付费调用；新浏览器存储可从后端已生效的参考恢复选择。参考图片不能充当finish_artwork要求的当前revision真实截图。

事件记录参考ID、标题、作者、来源、许可和artifact；不把base64保存到会话。Harness收尾的作品文件有referenceImages字段用于追溯。输入框旁的Token统计包含参考图的上游实际usage，实际图片费用仍以服务统计为准。

## 大肥鱼素材调研

已找到用户已有的 `D:\Projects\fatfish`；其中包含2026-10-17整理的语料、出处考证和多个图片仓库。也发现 `E:\tmp\voxel_art\out` 的既有作品图。本轮不将这些目录整体打包或自动发往API。

- 原角色：上善无形的“溟月”公开发布页 https://www.bilibili.com/opus/1231977657712771073 ，发布页标注CC BY-NC-SA 4.0。
- 女仆二次设计与授权声明：ZipZipPipe https://www.bilibili.com/opus/1231980045693616162 。
- 选用原图发布页：女仆全身 https://www.bilibili.com/opus/1233069202138464289 第1图；小肥鱼V4F https://www.bilibili.com/opus/1236803362705375305 第8图。已目视核对，未把同一帖的其他AI角色误认成DeepSeek娘。
- 近期梗图索引：EDMOK/blue-fish-archive、aigengtu.com；角色与设定图索引：TreapGoGo/deepseek-whale-girl。
- Neko3000/deepseek-whalechan的THIRD-PARTY-NOTICES、TreapGoGo项目README与pluveto/DeepSeek-chan均明确提醒：代码/文档许可不自动覆盖全部图片，因此没有用仓库MIT标签给整包图片重新授权。

取图与授权证据在[source-records.json](source-records.json)；三张参考的原作者、具体原帖、许可和修改方式见[内置署名清单](../../references/ATTRIBUTION.md)。CC图片依署名、非商业、相同方式共享条件使用，独立于项目代码MIT许可。安装器携带CC许可全文与署名清单。

## API

- `GET /api/references`：内置/本机图片、来源及预览URL。
- `POST /api/references/upload`：multipart的file字段，先导入本机并返回图片ID。
- `GET /api/references/{id}/image|thumbnail`：本机图片及缩略图。
- `POST /api/references/remove`：`{"id":"local:..."}`，移出本机图库。
- `POST /api/harness/message`：新增`referenceIds:["builtin:chair","local:..."]`；空数组清除之前生效的视觉参考。其他消息控制参数保持。
- `GET /api/harness/state`与`get_context`：`referenceImages`反映已生效参考；get_context.viewConventions明确front从+Z观察，角色脸和正面应在+Z外表面。

[本轮验证与已知边界](../quality-v4/RESULTS.md)。图片辅助建模不是自动摄影测量；遮挡背面、角色比例和颜色仍需模型判断与人工复核。
