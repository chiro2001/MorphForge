# Minecraft 建筑与材质导入

工作台可独立用作建筑浏览器、编辑器与AI修改工具。VRChat连接默认关闭；应用导入不会自动启用连接。

## 使用流程

1. 点击「Minecraft 建筑导入与源库」，上传建筑或选择已有源。
2. 选择每MC方块对应的逻辑格数：1／2／4／8／16。默认4；楼梯、台阶、门窗等细节需要更高精度。调高精度也会扩大作品的逻辑坐标范围。
3. 生成独立3D预览；检查报告中的形状近似、精度量化、材质替代、越界和容量情况。
4. 应用到草稿后，可继续手工或用AI修改、撤销、存入作品库。原建筑文件仍保留，可用不同精度重新编译。

「替换草稿」替换整份场景；「覆盖指定区域」按新几何覆盖已有体素。地面中心归零与XYZ偏移只影响编译结果，不改写源建筑。外壳模式仅剔除六邻域都被不透明整块包围的完整方块，源建筑不丢失。

## 格式与保护边界

| 格式 | 范围 |
|---|---|
| `.litematic` | 多区域、负向区域、跨64bit方块状态位打包；重叠区域按名称排序覆盖 |
| `.schem` | Sponge v1/v2/v3、稀疏palette索引、VarInt、Offset |
| `.schematic` | 旧MCEdit ID与metadata；常见旧方块转换为命名状态 |
| `.nbt` | Java structure稀疏blocks与palette；不把任意NBT误判为建筑 |

Bedrock `.mcstructure`、完整世界目录和红石/实体行为不在当前范围。原始实体及方块实体数据保留在源文件中。

压缩文件64MiB、NBT展开128MiB、NBT深度64、2097152源格、65536方块状态；临时几何展开最多2097152对象。先限制再分配，非法或损坏输入不会修改草稿。

## 几何与资源包

内建形状覆盖完整方块、上下台阶、楼梯及内外角、墙／栅栏、玻璃板、门／活板门、雪层、地毯、箱子／床、水位等。小于当前格精度的部件至少量化为一格，并报告；未知形状使用可见替代，源状态仍完整。

默认使用原创、MIT许可的Minecraft风格材质，不包含Mojang贴图。Java资源包ZIP独立导入：解析立方体父模型、纹理引用、六面贴图、动画帧序列／时长／插值；保存原ZIP和带校验的blockstate索引。variants按属性匹配，选择模型别名并应用直角X/Y旋转；加权模型固定选最高权重项以便重现。单个匹配项的multipart可复用；多层叠加、旋转UV锁定、自定义elements、CTM与自定义Shader等会报告近似，未宣称完整Minecraft渲染。

一块MC贴图可跨2／4／8／16逻辑格重复，原木X/Z轴及逐面朝向由材质变体表达。前端和Avatar元数据共享textureScale、textureRotation与材质UID；比例与朝向变体复用纹理数组层。

MF6配套目录预烘焙544项（基础材质、原创主题、精度／轴向／常见染色变体），Avatar静态ID为1..4095。作品逻辑材质ID可更大，导出时保留UID并重映射编号。目录中已有贴图只传编号；新的外部图片仍需烘焙并更新Avatar，不能由OSC传图片并动态创建贴图。

## API 与 Harness

- `POST /api/minecraft/upload`：multipart `file`，注册内容寻址的源建筑。
- `GET /api/minecraft`：源库；`POST /api/minecraft`提供list／inspect／preview／apply。
- `GET /api/minecraft/preview/{id}`：独立场景和材质；`GET /api/minecraft/source/{id}`：原文件。
- Harness `minecraft_import`复用同一套接口。preview不修改草稿；apply需previewId与baseRevision，并受运行取消、工程、保护组和版本守卫约束。

预览绑定工程ID、revision、场景哈希与材质哈希。草稿变化后必须重新预览。apply的requestId持久幂等，重启后也不会重复覆盖。日志记录来源、选项和报告；没有付费模型请求也可完整测试。

```json
{"action":"preview","sourceId":"<源库返回的ID>","baseRevision":0,"options":{"blockScale":4,"center":true,"offset":[0,0,0],"mode":"replace","resourcePack":"builtin","surfaceOnly":false}}
```

```json
{"action":"apply","previewId":"<预览返回的ID>","baseRevision":0,"requestId":"import-building-1"}
```

Source及缓存位于工作台数据的imports目录，资源包ZIP及索引在packs目录。作品保存编译后的几何和来源引用；单独导出的作品JSON不包含原始Minecraft建筑或整个资源包。
