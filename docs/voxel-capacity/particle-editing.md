# 粒子体素的增加、删除与替换

日期：2026-10-18。本文为方向④的设计分析；没有创建Unity资产或运行本轮粒子实验。结论更新：粒子可以借助原生Trigger Kill按空间位置删除，不必先拥有粒子ID到内部数组索引的映射。原作者VRLabs Marker已有Avatar粒子笔迹、Collider擦除与Custom simulation space的实现依据。

## 首选语义

外部后端以整数格坐标为键保存权威场景，例如 `scene[x,y,z]=style`。Avatar端粒子只承载显示。每格使用统一的绝对覆盖操作：

```text
SetVoxel(x,y,z,style):
  固定并锁存本条命令输入
  将擦除体定位到该格中心，清除该格已有粒子
  结束擦除并撤离/关闭擦除体
  若 style != 0，在同一格中心发射新体素
```

删除就是style=0；换色、换外观就是先删除再按新样式生成。重复执行完整操作具有最终状态幂等性；这不保证网络中途丢步骤也能正确，更不等于发射动作本身幂等。

发射和擦除应在同一条锁存命令上完成，执行期间不能让随后输入改变目标。显示状态是否最终一致，需要等待本地执行完成并独立验证远端。

## 各种增删方法

| 方法 | 优点 | 代价/限制 | 用途 |
| --- | --- | --- | --- |
| 小型Trigger擦除体定位到单格 | 原生杀死粒子；可以清掉同格重复粒子；不需逐粒子Animator路由 | 要校准粒子触发半径、空间基准和执行顺序 | 首选单格删除与覆盖 |
| 可变尺寸Box触发体 | 一个区域即可删除许多体素 | 边界误删与Collider尺寸动画需验证 | EraseBox与清空区域 |
| 脏分区重新发布 | 更新逻辑简单，能恢复碎片/重复与不同步内容 | 需清空/重启该区并回放，存在传输和显示延迟 | 修复和压缩；作为局部删除不可靠时的备选 |
| 双套分区粒子系统 | 后台准备新分区，完成后切换可见性 | 约双份粒子容量；后台模拟与清空方式需验证 | 降低分区重建的视觉闪烁 |
| 短寿命、持续刷新 | 不再刷新即可自然消失 | 静态建筑也持续消耗OSC带宽；漏刷造成缺块 | 不作为主方案 |
| Shader按当前擦除体裁剪 | 当前区域可立即隐藏 | 普通Shader没有历史；擦除体离开后旧块会再次显示 | 临时预览，不能代替持久删除 |

## 原生Trigger设置依据

Unity2022.3 Triggers模块支持Inside/Outside/Enter/Exit分别配置Callback、Kill、Ignore。Inside=Kill由引擎直接删除内部粒子，独立于用户的OnParticleTrigger回调。建议Inside=Kill，Outside与Exit=Ignore，Enter是否也Kill由实验决定。无脚本回调需要时Collider Query Mode可设Disabled，不额外采集碰撞体信息。

首轮单格可使用小球形擦除体，参考VRLabs Marker；区域擦除再增加盒形版本。优先按体素中心匹配，避免用一个填满整个体素的擦除体与相邻块边缘碰撞。Radius Scale要较小，并用Visualize Bounds检查实际粒子触发边界。

擦除体尺寸和Radius Scale应一起调整，不能只按可见Cube尺寸猜测。应测试目标格周围26个相邻格，确保只删除目标及该格重叠副本。区域擦除同样按格中心集合定义边界。

## 发射与坐标要求

- 使用一个持续运行的保存粒子系统。发射器可以移动，已有内容必须保留在固定Custom simulation space根或World空间中；Local空间可能使旧粒子随发射器移动。
- 原生粒子位置用作逻辑格中心。外观大小由Cube Mesh/出生尺寸表达，关闭速度、重力和随机出生偏移。
- 颜色需要写入Start Color等出生数据；持续修改共享材质颜色可能改变所有旧粒子。材质类型可探索把样式ID写入出生颜色，由统一Shader选择预上传样式。
- 先验证不重启保存系统的发射脉冲如何产生固定数量粒子。不能把ParticleSystem.Emit脚本API当作Avatar运行时能力，也不能只用发射率乘以一个动画时长就宣称每次恰好产生一个粒子。
- 不通过重新启用整个保存系统来默认获得单次Burst；必须确认这种操作不会清掉旧内容。普通Avatar画笔的连续发射实现，也不自动证明精确单次发射成立。

## 本地执行与OSC协议

推荐本地执行阶段：准备/锁存 → 擦除保持若干模拟更新 → 结束擦除 → 发射 → 待机。新增粒子必须在擦除结束后出生；输入切换时擦除体应先处于无效位置或可靠关闭，避免误删中间位置。

擦除门控的具体动画绑定需验证。仅停放到远处不保证没有Trigger扫描开销；仅禁用Collider是否停止被粒子Trigger模块查询，也不能未经验证就假定。

接收器可研究小型提交状态机和有限命令序号/翻转位，以抑制相同提交重复触发。翻转位可能漏掉被网络跳过的偶数次变化；序号也不提供可靠传输或全观看者回执。地址和数据先稳定、再提交的两阶段方法需按远端实测确定保持时间，不能沿用理想字段吞吐直接承诺体素速度。

恢复以周期性的绝对场景刷新/分区快照为基础。SetVoxel覆盖能修复重复与旧样式，但只发送非空格无法恢复错存的多余块，仍需清空分区或重发必要空格。

## Ring Buffer的关键风险

Unity官方Main Module说明Pause Until Replaced可在寿命结束后保留粒子，满容量后新粒子替换旧粒子。它应作为保留机制，而不是任意位置删除的算法。

Unity Issue Tracker UUM-32881记录过Trigger与Ring Buffer组合中粒子删除后重新出现的问题，并列有修复分支/版本信息。该历史问题不表示当前2022.3.22f1一定仍有缺陷，但需要按原问题进行回归。

首轮对照：Ring Buffer关闭、较长有限寿命，验证基本Trigger删除；然后启用Pause Until Replaced，并在粒子已进入暂停阶段后删除。观察是否真正消失、是否复活、粒子计数是否减少，以及删除后的槽位是否可重新利用。

还应在远低于容量时反复覆盖同一格，累计操作次数超过Max Particles，检查其他未编辑格是否被意外淘汰。外部非空格数低于上限，不足以证明引擎内部容量没有被重复发射或无效槽消耗。

## 性能和验证顺序

### 容量补充核查

Unity通过Max Particles配置容量，64只是首轮实验规模。VRChat官方粒子限制器文档当前列出的默认示例为50,000粒子/单系统，同时有Mesh、Collision、Trails惩罚及观看者配置；不能把该数值当作通用可用容量或统一硬上限。

若每块使用12三角形Cube、每粒子一块，PC的Mesh Particle Active Polys评级阈值为Good 1,000 / Medium 2,000 / Poor 5,000，分别粗算约83 / 166 / 416块；这是仅该统计项的预算。256块对应3,072三角形，1,024块为12,288，4,096块为49,152。整体Avatar评级还取决于其他项目。

Total Particles Active按所有系统的maxParticles总和计算，预留容量和备用系统也会影响评级；没有发满不能让该统计项忽略容量。实际三角形绘制、模拟和Trigger开销仍需实测。建议256–1,024作为后续工程测试目标，4,096作为压力档，不承诺帧率或客户端容量。

依据：VRChat Performance Ranks（https://creators.vrchat.com/avatars/avatar-performance-ranking-system/）和Avatar Particle System Limits（https://docs.vrchat.com/docs/avatar-particle-system-limits）。

这条路线省去逐体素Animator路由，但仍有粒子模拟、Trigger查询和Mesh绘制成本。先使用一个保存系统、一个擦除体、64个体素容量；证据成立后再比较256/1024，以及少量分区。不要一开始为每块创建一个ParticleSystem。

| 验证 | 判定目标 |
| --- | --- |
| 移动发射器后旧块保持 | 粒子空间与根坐标设置正确 |
| 多种帧率下单次发射计数 | 保持旧内容且每次发射数量可控 |
| 删除3×3×3中间块 | 26个邻居均保持 |
| 同格重复Set/改色 | 最终样式正确、重复数量受控 |
| Pause Until Replaced暂停后删除 | 不复活且容量行为明确 |
| 低占用、超容量次数反复覆盖单格 | 不影响其他格，不逐渐耗尽可用槽 |
| EraseBox后再新建 | 擦除边界、撤离时机与重建正确 |
| 看向别处再返回、重载与晚加入 | 不漏本地操作，外部快照能恢复 |
| Idle/编辑两态CPU/GPU对照 | 量化原生模拟与Trigger成本 |

先分析并锁定上述机制，再实施原型；当前不报告这些检查已通过。

## 一手来源

- VRLabs Marker：https://github.com/VRLabs/Marker — 原作者说明粒子绘制、Collider擦除、Custom simulation space。
- Unity2022.3 Triggers：https://docs.unity3d.com/2022.3/Documentation/Manual/PartSysTriggersModule.html — Kill、Ignore、Radius Scale、Query Mode。
- Unity2022.3 Main Module：https://docs.unity3d.com/2022.3/Documentation/Manual/PartSysMainModule.html — 出生属性、模拟空间、Ring Buffer与Culling。
- Unity Issue Tracker UUM-32881：https://issuetracker.unity3d.com/issues/particles-do-not-die-when-using-a-trigger-and-ring-buffer — 删除后复活历史问题。
