# 点线与快速演示（Desktop 0.22.0）

顶部“演示”按钮打开独立预览窗口。选择图元、输入坐标、颜色、分组与大小，点击“预览”，确认面数与目标范围后“加入作品”。“预览组合演示”可一次生成坐标轴、散点、折线、虚线、箭头和圆环；预览不修改工程。默认追加，不清空已有作品；勾选“替换同名分组”后仅替换指定组，可用来更新同一张图。

| 图元 | 参数 |
|---|---|
| 点／点集 | position或points；size为直径，shape为sphere/cube |
| 线段 | start、end；width为直径；可选虚线 |
| 折线 | points；可闭合，虚线间隔沿路径连续 |
| 箭头 | start、end、width；箭头头部随粗细和长度确定 |
| 圆环 | position、radius、width、plane=xy/xz/yz、segments |
| 坐标轴 | position、length、width；红X、绿Y、蓝Z |
| 线框盒 | position为最小角、extent为XYZ尺寸、width |

坐标和大小使用逻辑格，生成顶点对齐现有整数网格；回执显示最大取整误差和移除的退化面。建议点直径、线直径至少2格，过小导致整体退化会明确拒绝。球是低面数球体。线宽属于三维几何，在远处会变细；本轮没有固定屏幕像素线宽或自动朝向相机的点精灵。

这些图元编译为普通RGB8三角面，编辑器、工程保存、撤销、分组选择/复制/删除、动画、模型导出和既有MF6传输共用相同几何。每面计入已有组合几何容量；发布仍需目标支持多边形和满足编码范围，不新增同步参数，不改变Avatar包1.2.0。创建后以三角面保存，原始参数不会作为独立可变形对象保存；修改图形可重新生成并替换同名组。

AI工具为`draw_primitives`，action为catalog/preview/apply。preview/apply提供projectId、baseRevision和primitives；apply可传requestId防止重复。已有对话需要显式升级工具/提示词版本，保持历史前缀保护。

```json
{
  "action":"preview", "baseRevision":0, "group":"demo",
  "primitives":[
    {"kind":"point","position":[0,24,0],"size":6,"color":[255,160,60]},
    {"kind":"segment","start":[0,24,0],"end":[40,40,0],"width":2,"color":[88,190,255]},
    {"kind":"arrow","start":[0,8,0],"end":[32,8,0],"width":2}
  ]
}
```

本轮不自动创建GitHub Release、不上传Avatar、不启动真实VRChat；真实客户端与远端网络体验不由本机发送成功代替。
