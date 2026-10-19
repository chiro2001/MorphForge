# 服务端安全检查参考

核对日期：2026-10-19。仅使用VRChat官方帮助、创作者文档及员工开发更新。

- [My avatar is showing Security checks failed](https://help.vrchat.com/hc/en-us/articles/31264259544211-My-avatar-is-showing-Security-checks-failed)：服务端检查失败会阻止加载；帮助页建议核对上传加工插件与SDK版本，常见问题包含过高内存开销。没有给出Animator层数或粒子数的完整服务端阈值。
- [Performance Ranks](https://creators.vrchat.com/avatars/avatar-performance-ranking-system/)：性能评级是静态统计与用户显示过滤的一部分。Very Poor并不意味着绕过其他检查；评级表中的Animators统计Animator组件数量，不能当作AnimatorController层数限制。
- [Avatar Size Limits](https://creators.vrchat.com/avatars/avatar-size-limits/)：PC下载大小200MB、解包大小500MB。解包大小指资产包解压后的字节量，不是运行时全部资源/动画内存；SDK3.7.0及以后在上传前执行大小检查。两者不能混为一谈。
- [Developer Update — 31 October 2024](https://ask.vrchat.com/t/developer-update-31-october-2024/27596)：说明2024-11-01起服务器对超出PC下载/解包大小的Avatar标记Security Checks Failed。
- [Developer Update — 7 December 2023](https://ask.vrchat.com/t/developer-update-7-december-2023/21274)：服务端扫描包含损坏资产包、极高内存及恶意模式，部分规则不公开且可调整。因此不能凭通用错误码推导具体命中项。
- [Avatar Particle System Limits](https://docs.vrchat.com/docs/avatar-particle-system-limits)：描述本地粒子limiter及其历史推荐配置；该表不是服务端安全检查硬限。不能把文档里的50,000默认值当作服务器保证接受16,384粒子的证据。

本次诊断以客户端实际错误、当前Stage3人物、最近上传资产包及构建后资源为准；FullFT的SDK预算验收不是Stage3上传包通过服务端扫描的证据。

- [Unity 2022.3 startLifetime API](https://docs.unity3d.com/2022.3/Documentation/ScriptReference/ParticleSystem.MainModule-startLifetime.html)：Unity明确支持`float.PositiveInfinity`实现永久粒子，因此该值不能称作Unity非法设置。VRChat服务端是否接受这一具体资产组合，仍需单变量对照上传；目前没有官方证据表明此项就是失败原因。
