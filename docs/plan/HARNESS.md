# Harness、可观测性与compact契约

## 领域工具

| 工具 | 契约 |
|---|---|
| get_context | 返回revision、预算、选区、分组、可用材质、相机和目标同步摘要；不倾倒全量Box |
| run_recipe | code、seed、mode(replace/apply)、baseRevision、requestId；受限JS SDK批量产出操作，事务校验后提交draft |
| inspect_region | 区域/分组的数量、占用、bounds、样式和选取摘要 |
| capture_views | revision、hero/front/top/side/detail、尺寸；返回artifact引用、相机和统计 |
| commit | 固化指定draft版本及描述，生成不可变快照 |
| publish | 提交已commit版本至指定目标；队列异步追最新目标 |
| sync_status | published/sent/pending/ETA/error；明确ack=false |
| undo_to | 从指定版本建立当前draft，记录回退事件 |
| artworks | search/get/save/load/fork，加载默认只修改draft |
| compact_context | 请求在完整工具链边界压缩上下文，不改几何或发布状态 |

所有结果包含ok、requestId、revision、changed、generationMs、warnings或结构化error；大图、快照、recipe作为artifact。run_recipe支持函数与受限循环；SDK至少clear/box/erase/paint、group/repeat/mirror，逐步提供line/wall/stairs/frame/roof。复用是代码/逻辑组，不生成Unity对象。

独立受限进程执行recipe：无DOM、网络、文件系统、Tauri桥或密钥；限制运行时间、操作数、内存和输出字节。异常给行列/栈摘要；完整失败事务回滚。固定seed和SDK版本保证几何重放。

## 模型循环

读取上下文→模型工具请求→聚合/验证完整参数→幂等执行→工具结果/图片反馈→继续；支持多次用户输入接续工程。SSE碎片不能直接执行。思考模式保留provider要求的实际返回字段；不伪造模型内部信息。工具错误可恢复，持续失败/预算耗尽有明确结束状态。暂停在安全工具边界，停止取消请求并保留已提交内容。

用户可设置模型、max steps、Token/时间预算、自动发布和compact阈值。人工编辑引发版本冲突时模型重新读取上下文。默认断开发送；连接后可分批自动发布，生成停止与发送暂停分别控制。

官方当前文档支持Chat Completions和无状态Responses；实现provider层隔离能力差异。图片可内嵌URL/base64，或官方/files上传purpose=user_data取得file_id；记录图片hash到provider file_id映射复用，不把本机127.0.0.1地址交给云端取图。对模型/文件接口的文档与实际服务差异分别记录，不靠虚构端点。

## 完整记录

每条事件具有sessionId/runId/turnId/toolCallId、递增seq、UTC时间、schema/tool/prompt版本。记录实际请求/响应、流式汇总、用量、首token/总延迟、重试、错误；请求头与密钥脱敏。记录工具参数、校验、耗时、几何前后revision/diff、recipe/seed、截图artifact/hash/相机、发布队列/OSC序号/发送时间、compact来源范围与Token前后变化。

JSONL追加事件和不可变artifact/snapshot按hash去重。前端支持run/tool/revision筛选、事件详情、差异、回放、从历史分支、导出诊断包。API优化比较调用数/字节数/P50-P95耗时/错误率/返工/Token/作品完成时间；不能把模型调用重放当作确定性模型输出。

## compact

compact是客户端活跃上下文压缩；官方无状态接口不替Harness保存项目记忆。完整日志永不被compact删除。触发：自动阈值+手动请求；只在assistant工具请求对应结果完整后进行，避免孤立tool消息。

保留：系统/工具约束、用户原始目标与美术约束、关键决策/失败教训、作品与材质版本、当前任务/未完成项、最近完整消息。旧图留artifact引用，按需重新附图。压缩后重新查询实时draft/published/sent状态，摘要中的旧状态不是事实源。

摘要有结构化schema、来源事件范围/哈希、生成配置、前后用量；固定约束来自程序不依赖摘要模型记忆。压缩失败保留旧上下文并明确状态；可查看前后摘要、重新压缩和恢复。上下文压缩与几何optimize是两个独立操作。

## 作品库

作品有稳定ID、标题、标签、缩略图、多视角、成品快照、recipe/seed、材质清单/版本、Box/体素统计和关联run。支持search/load/save/fork/import/export；加载直接读快照，不强迫再次调用模型生成。保留原始艺术呈现与VRC可发布版本，并记录转换损失。

优先导入E:\tmp\voxel_art及用户约5k Box作品；精确Box数在转换后统计。原有目录/图不覆盖。运行程序密钥从环境或指定.env读取，前端只见“已配置”状态，日志/作品/compact均不得包含密钥。

## 已实现的补充边界

get_context 的材质摘要默认最多128条，支持styleQuery/styleOffset/styleLimit；完整材质与纹理仍保存在工程，避免大资源包挤占模型上下文。运行预算maxRunSeconds默认900，覆盖暂停、模型请求和工具等待。手动Recipe也产生tool.request/tool.result记录。作品支持最多5张artifact多视图；旧作品缺少views字段仍可读。
