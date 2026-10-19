# 0.4.2：追加式上下文与前缀缓存修复

日期：2026-10-19。代码提交`825bf8e`，提示词`morphforge-system/4.4`，工具定义仍为`morphforge-tools/3.1`。本轮不访问用户内网、不调用真实模型API、不发送OSC；使用临时透明透传捕获实际出站请求，并以旧版二进制做负对照。

## 问题与修复

0.4.1的本机会话记录虽然保留原始内容，但**实际发送的上下文并非append-only**：

1. 每轮把实时工程、同步状态、剩余Token/时间和重试说明拼进开头system，第一条消息持续变化。
2. 参考图临时附到本次请求末尾，没有作为同一位置的历史内容持久化；下轮新工具结果会出现在它之前。
3. 新截图到来后，把旧请求中已经发送过的截图像素替换为文本说明，直接改写历史。

修复后，设置不变时system和工具定义固定；权威状态只发送字段增量，预算和重试说明写入尚未发送的新输入或工具结果。用`requestedInputLength`标记已准备发送的历史边界，运行状态不会修改该边界之前的内容。普通工具轮次把状态附到新工具结果，避免制造假user回合；部分模型模板会根据最后一条user消息删除较早思考内容。

参考图在首次选择、变更或compact后写入历史一次；同一选择不重复追加。取消参考追加取消说明，已有历史图片保持不变，直到compact再移除旧参考像素；这与0.4.1的删除策略不同。历史截图的最近视角筛选也只在compact边界执行。原始事件、图片artifact与作品保留。

`Conversation`新增字段均有缺省值，旧会话可读取；不会在升级时主动发起模型请求。显式compact/重置上下文、更换模型/提示设置、开关图片发送、Provider变化会改变前缀，属于明确边界；官方Files引用到期更新也可能影响历史图片引用。通常的本机兼容接口使用inline图片，避免文件引用轮换。

## 临时透明透传验证

链路为：**真实Rust Harness → 临时loopback透传 → 本机可控上游**。没有在记录后重新序列化出站请求：HTTP请求正文与JSON/SSE响应正文均按字节转发，通过两端SHA256对照。上游只返回固定工具调用；真正执行Recipe、真实Chromium截图、入库、续作和compact的是Rust程序。

每个新版本用例11次请求：读取上下文、搭建、截图、修改、再截图、收尾、续作、取消参考、compact、压缩后读取、继续。普通阶段有7组连续前缀比较；compact调用与随后的新上下文是两个预期边界，新上下文随后再次保持追加。

| 用例 | 结果 | 普通阶段消息前缀 | 检查项 |
|---|---|---|---:|
| [Chat文本](chat-text/report.json) | 通过 | 7/7完整保留 | 17 |
| [Chat图片SSE](chat-images-sse/report.json) | 通过 | 7/7完整保留，参考及两轮截图不改写 | 16 |
| [Responses图片SSE](responses-images-sse/report.json) | 通过 | 7/7完整保留 | 16 |
| [0.4.1负对照](before-v041/report.json) | 按预期被拒绝 | 6/7发生改写，首个变化位于system消息 | 旧缺陷已复现 |

负对照使用此前已验收的0.4.1 Linux二进制，SHA256为`1ca0acb43bf7b50a1832bdb7d0a0d09ca07e88a661448130a931c5509f79ea76`；负对照报告的`success=false`是预期结果，不能算作新版通过。

文本用例进一步把捕获的Chat消息交给**官方Qwen/Qwen3-0.6B的原始chat_template和tokenizer**，只做模板渲染及分词，不运行模型。连续比较中，上一请求的完整token序列分别为5,003、7,738、8,192、8,840、9,274、9,923、10,568，均逐token保留为新请求前缀。旧版同样测试失败，证明该测试能发现开头改写。图片用例验证实际出站消息、图片内容和透传字节，不冒充多模态处理器的token展开结果。

分词器文件来自`https://huggingface.co/Qwen/Qwen3-0.6B`；配置与tokenizer文件hash记录在各文本报告的`relay.templateFiles`。最终新版本测试二进制SHA256为`00e16820cbd4778e3ce37b30cffe25b6a0276fa5608ad2b02fd8c49d95d98aa1`。

这些结果证明了Harness的出站追加契约与本地模板前缀，**不能证明用户vLLM已实际命中KV-cache**。各可控上游故意返回cached_tokens=0，不用模拟数字伪装命中率；实际命中仍由用户内网服务的缓存设置、模板和上报决定。

## 日志与复测

`provider.request.prefix`在协议转换及图片解析之后记录：

- `scope=wire_messages`，只表示出站消息检查；
- `appendOnly`、`staticPrefixUnchanged`、`sharedMessages`、`firstChangedMessage`；
- `reason`区分首次请求、追加、历史改写、compact、Provider及图片策略变化；
- 只保留上一请求的消息指纹用于比较，不额外保存图片正文或密钥。

`harness.request.prepared`提供`contextPolicy=append-only-until-compact`、`contextEpoch`、`runtimePlacement`和`authorityChangedFields`。人工重置上下文会清空比较基线。新运行可继续复用既有会话；跨进程重启后指纹诊断重新建立基线，但不会因此主动改写会话。

在能访问目标上游的电脑启动临时代理：

```bash
python tools/prefix_proxy.py --upstream http://YOUR_SERVER:8000/v1 --port 18790 --report prefix-report.json
```

工作台Base URL填写`http://127.0.0.1:18790`，接口类型与模型名沿用上游配置；代理将Authorization透传，不写入报告。按Ctrl+C停止后生成报告，也可读取`http://127.0.0.1:18790/__prefix__/report`。可选`--tokenizer-dir PATH`读取本机的`tokenizer_config.json`和`tokenizer.json`检查文本模板，需要临时Python环境安装`jinja2`和`tokenizers`。图片或自定义模板仍以真实服务端token日志为准。

自动复测入口：

```bash
python tools/check_prefix_context.py --url http://127.0.0.1:PORT --api chat --out RESULT_DIR
# 图片SSE增加 --images --stream；Responses使用 --api responses。
# 目标必须是空的隔离数据目录，并启用截图渲染器。
```

## 回归与Windows

Linux Rust **107项**、Clippy `-D warnings`、格式检查通过；新增测试覆盖预算/重试消息位置、Chat转换后前缀、图片保持、色板造成的状态变化、续作与取消参考。原有打断、插话、排队、compact和幂等测试通过。实际Rust/Chromium工作台流程**27项**通过，[报告](browser/provider-presets-browser.json)。

Windows 0.4.2候选包使用[115项冻结输入](source-freeze.json)构建，构建前后hash一致。核心/桌面**109项**、原生HTTP **62项**通过；8张内置图和8张缩略图hash匹配，安装器中的核心与已验收核心完全一致，字体/图片许可及署名完整。[构建摘要](windows-build.json)、[HTTP](windows-http.json)、[内置资源](windows-references.json)。没有覆盖用户安装、用户数据或Unity资产，没有启动VRChat。软件修复不改变Avatar协议或参数预算。

安装器`MorphForge Desktop_0.4.2_x64-setup.exe`为16,717,015字节，SHA256为`f8acb195a32f8d90aea0880ab751f6fcecdb66f05da49ef52d1160a621008144`。
