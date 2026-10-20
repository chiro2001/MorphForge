# 0.15 创作资产接口

新增工具为 `components`、`reference_workbench`、`animation`，系统提示词5.1、工具schema4.1。中英文工具表具有相同参数结构，说明优先中文。资产属于项目，在同一对话反复使用，也跨该项目的其他对话/重启保留。`get_context.creative`仅返回可发现的摘要，不把组件完整几何塞入每次上下文。

## 状态与并发

- `GET /api/creative`：`revision`、`bookmarks`、组件摘要数组、`clips`；组件详情按工具inspect读取。
- `POST /api/creative/components|references|animation`：与相应Harness工具相同的参数。所有调用沿用运行取消守卫；UI带上`projectId`避免跨项目提交。
- 资产保存/删除需要`baseCreativeRevision`，与几何`baseRevision`独立。成功返回更新后的资产状态。错误的版本或无效数据不修改项目。
- 组件保存和放置需要当前几何`baseRevision`。放置有`dryRun`、保护组、几何撤销及持久`requestId`回执；模型调用自动注入call ID。相同请求重试不重复绘制，不同内容复用同一requestId会拒绝。
- 组件放置若传入`baseCreativeRevision`，也检查组件库是否已改变。
- 三类资产随工程JSON、作品另存/载入保存；切换项目隔离。资产定义的编辑不属于几何撤销，几何放置可以撤销。作品载入采用作品携带的资产定义；老作品缺少该字段时使用空默认值。

## 组件

保存现有分组，默认选区地面中心作为原点；也可传`ids`保存选中对象，或传整数`anchor`显式指定原点。`id`由调用方指定，重复save在通过版本检查后更新同名资产。

```json
{"action":"save","id":"arch-window","name":"拱形窗","groups":["prototype-window"],"baseRevision":12,"baseCreativeRevision":0}
```

用新返回的几何/资产版本放置；`scale`为整数1..64，`turns`为绕Y四分之一圈次数，可选`mirror`为x/y/z。`styleId`只替换方块材质，三角面保留原逐面颜色。所有坐标为逻辑格。

```json
{"action":"place","id":"arch-window","baseRevision":12,"requestId":"windows-row-1","instances":[{"offset":[-8,4,8],"group":"window-left"},{"offset":[8,4,8],"group":"window-right","scale":1,"turns":0}]}
```

先加`dryRun:true`可检查展开后的数量与保护组，实际应用使用独立的requestId。每次1..128个实例；最多128个组件，总计65,536个源几何对象。组件局部几何保留方块斜面、材质ID、染色、三角面颜色/单双面；实例展开为普通可编辑对象，**不会自动减少OSC数据量**。组件属于当前项目的材质目录，尚无跨项目材质重映射或嵌套组件依赖。

## 参考图对照与相机

`reference_workbench action=save`接收`id`、`baseCreativeRevision`和`bookmark`。`bookmark`含`name`、`shot`和可选`comparison`。shot复用截图相机字段，新增持久`up`与`orthoHeight`以保留滚转方向、正交缩放；保存时shot.id统一为书签id。书签id限ASCII英文数字、横线、下划线。

```json
{"action":"save","id":"front-reference","baseCreativeRevision":1,"bookmark":{"name":"正面比例对照","shot":{"projection":"orthographic","yaw":0,"pitch":0},"comparison":{"imageId":"实际图库ID","mode":"overlay","opacity":0.4,"zoom":1,"offset":[0,0],"crop":[0,0,1,1]}}}
```

comparison支持`split|overlay`；crop为归一化x,y,w,h，必须位于图片内；offset按对应显示区域宽高归一化，zoom为0.1..10。参考图仅来自已有图库，删除原图后需重新选择。

```json
{"bookmarkId":"front-reference","compare":true,"width":1280,"height":720}
```

以上作为`capture_views`参数，或增加`action:"capture"`调用`reference_workbench`。`capture_views`也可直接接收`comparison`配合自定义shots。worker真实渲染后合成，PNG保存到artifact并进入时间轴；标注参考图身份，场景不变。并排模式最终画布一半宽度分给作品，一半给参考；叠加模式按opacity合成。截图结果保留相机、裁剪和对照元数据。

## 分组动画

`animation action=save`接收`id`、`baseCreativeRevision`与clip：

```json
{"action":"save","id":"door-open","baseCreativeRevision":2,"clip":{"name":"开门","duration":4,"looping":true,"tracks":[{"group":"door","pivot":[-4,0,8],"interpolation":"linear","keys":[{"time":0,"translation":[0,0,0],"rotation":[0,0,0],"scale":[1,1,1]},{"time":2,"translation":[0,0,0],"rotation":[0,90,0],"scale":[1,1,1]},{"time":4,"translation":[0,0,0],"rotation":[0,0,0],"scale":[1,1,1]}]}]}}
```

每个现有分组一条轨道，枢轴与平移为逻辑格，旋转为XYZ欧拉角度；变换为`pivot + translation + rotation * scale * (point - pivot)`。`linear`平移/缩放线性、旋转最短路径四元数球面插值；`step`保持上一关键帧，到下一帧时切换。首末关键帧之外保持端点。界面拖动后同步显示实际插值数值，插入中间关键帧不会继承上一帧的旧表单值；非法关键帧不新增空轨道。完整转圈应像风车示例添加0/90/180/270/360中间帧，不能仅用0和360期待完整旋转。

时长0.05..3600秒，最多32片段；每片段64轨道、4096关键帧，每轨道256帧。时间严格递增，缩放各轴0.001..1000，坐标/旋转有有限值保护。父子骨骼、轨道混合、负缩放、自动物理和运动碰撞不在首版范围。

```json
{"animation":{"clipId":"door-open","time":1.5},"views":["hero","front"]}
```

以上调用`capture_views`冻结指定姿态；时间不自动循环取模，应处于clip时长内。它不修改草稿、不触发发布，不作为Goal静态几何收尾截图计数。

`GET /api/creative/animation/{id}/export`返回带所选片段的GLB，按分组挂枢轴父节点，使用标准TRS动画通道。位置使用工程格长换算米，反射Z并转换旋转四元数，与工作台一致。循环意图写入extras；播放器决定播放/循环。贴图首帧、程序材质基础色、独立壳体等边界与原GLB导出相同。此接口不写场景、历史或上下文。

当前动画属于编辑器、截图和GLB，**现有MF6 Avatar只发布静态几何**。[后续原生运动条件](AVATAR-MOTION.md)。
