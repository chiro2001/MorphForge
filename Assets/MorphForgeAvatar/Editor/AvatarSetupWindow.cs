using System;
using System.IO;
using System.Linq;
using System.Reflection;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using VRC.SDK3.Avatars.Components;

namespace MorphForgeAvatar.Editor {
    public sealed class AvatarSetupWindow : EditorWindow {
        public const string Root = "Assets/MorphForgeAvatar";
        public const string PrefabPath = Root + "/Generated/MorphForge.prefab";
        VRCAvatarDescriptor avatar;
        bool fury = true;
        string status;
        Vector2 scroll;
        [MenuItem("Tools/MorphForge Avatar/安装与检查")]
        public static void ShowWindow() { GetWindow<AvatarSetupWindow>("MorphForge Avatar"); }
        public static Type FindType(string name) {
            return AppDomain.CurrentDomain.GetAssemblies().Select(a => a.GetType(name, false)).FirstOrDefault(t => t != null);
        }
        public static bool HasMA => FindType("nadena.dev.modular_avatar.core.ModularAvatarMergeAnimator") != null;
        public static bool HasFury => FindType("com.vrcfury.api.FuryComponents") != null;
        void OnEnable() { if (Selection.activeGameObject) avatar = Selection.activeGameObject.GetComponentInParent<VRCAvatarDescriptor>(); }
        void OnGUI() {
            scroll = EditorGUILayout.BeginScrollView(scroll);
            EditorGUILayout.LabelField("MorphForge Avatar 1.0 · MF6 / 16K", EditorStyles.boldLabel);
            EditorGUILayout.HelpBox("在 VCC Avatar 工程安装 Modular Avatar 后，选择人物并安装。上传后由 MorphForge Desktop 经 OSC 驱动。PC / Windows；总容量 16,384 个 box 或三角面。", MessageType.Info);
            avatar = (VRCAvatarDescriptor)EditorGUILayout.ObjectField("目标 Avatar", avatar, typeof(VRCAvatarDescriptor), true);
            EditorGUILayout.LabelField("Modular Avatar", HasMA ? "已检测" : "未安装（必需）");
            EditorGUILayout.LabelField("VRCFury", HasFury ? "已检测（可选）" : "未安装（可选）");
            fury = EditorGUILayout.Toggle("启用 VRCFury 参数压缩桥", fury);
            EditorGUILayout.HelpBox("几何固定使用 240 bit。定位默认使用 13 bit；光球可见性仅本人本地控制。VRCFury 可压缩菜单中的定位/服装等低频参数，不能压缩逐帧面捕和 OSC 数据。预算超出会阻止构建，不会擅自取消任何其他参数的同步。", MessageType.Info);
            if (avatar) {
                int cost = avatar.expressionParameters ? avatar.expressionParameters.CalcTotalCost() : 0;
                EditorGUILayout.LabelField("当前人物参数（合并前）", cost + " / 256 bit");
                if (!avatar.GetComponentInChildren<MorphForgeAvatarModule>(true))
                    EditorGUILayout.LabelField("直接相加估计（压缩前）", (cost + 253) + " bit");
                EditorGUILayout.LabelField("最终预算", "以 SDK 构建检查报告为准");
            }
            using (new EditorGUI.DisabledScope(!avatar || !HasMA || EditorApplication.isPlaying || !AssetDatabase.LoadAssetAtPath<GameObject>(PrefabPath))) {
                if (GUILayout.Button("一键安装到所选 Avatar")) Try(() => {
                    var module = Install(avatar, fury);
                    status = "已安装：" + module.name + "。请在 SDK 构建后查看最终预算。";
                });
            }
            using (new EditorGUI.DisabledScope(!avatar || EditorApplication.isPlaying)) {
                if (GUILayout.Button("移除所选 Avatar 的 MorphForge 模块")) Try(() => {
                    var module = avatar.GetComponentInChildren<MorphForgeAvatarModule>(true);
                    if (module) { Undo.DestroyObjectImmediate(module.gameObject); EditorSceneManager.MarkSceneDirty(avatar.gameObject.scene); }
                    status = "模块已移除，可通过 Undo 恢复。";
                });
            }
            EditorGUILayout.Space();
            EditorGUILayout.LabelField("贴图库", EditorStyles.boldLabel);
            EditorGUILayout.HelpBox("内置原创方块工坊材质，不包含 Mojang 或人物资产。自定义目录由 Desktop 导出，选择包含 catalog.json 与 artifacts 的文件夹；插件会生成新的独立变体，不覆盖已安装模块。纹理变化需要重新上传 Avatar，实时纯色/程序材质不需要。", MessageType.None);
            using (new EditorGUI.DisabledScope(EditorApplication.isPlaying)) {
                if (GUILayout.Button("导入 Desktop 材质目录并生成安装变体…")) Try(() => {
                    var directory = EditorUtility.OpenFolderPanel("选择包含 catalog.json 的目录", "", "");
                    if (string.IsNullOrEmpty(directory)) return;
                    var path = CatalogVariant.Build(directory);
                    Selection.activeObject = AssetDatabase.LoadAssetAtPath<GameObject>(path);
                    status = "已生成 " + path + "。可将该 Prefab 拖到 Avatar 下；也可选择它后使用“安装所选变体”。";
                });
                if (GUILayout.Button("导出内置 Desktop 配套材质目录 ZIP…")) Try(() => {
                    var path = EditorUtility.SaveFilePanel("导出材质目录", "", "MorphForge-public-catalog.zip", "zip");
                    if (!string.IsNullOrEmpty(path)) { CatalogVariant.ExportBuiltin(path); status = "已导出 " + path; }
                });
            }
            using (new EditorGUI.DisabledScope(!avatar || !HasMA || EditorApplication.isPlaying)) {
                if (GUILayout.Button("安装 Project 中所选的 MorphForge 变体")) Try(() => {
                    var prefab = Selection.activeObject as GameObject;
                    if (!prefab || !PrefabUtility.IsPartOfPrefabAsset(prefab) || !prefab.GetComponent<MorphForgeAvatarModule>())
                        throw new InvalidOperationException("请在 Project 中选择 MorphForge 模块 Prefab。");
                    Install(avatar, fury, AssetDatabase.GetAssetPath(prefab)); status = "已安装所选材质变体。";
                });
            }
            if (!string.IsNullOrEmpty(status)) EditorGUILayout.HelpBox(status, MessageType.Info);
            EditorGUILayout.EndScrollView();
        }
        void Try(Action action) { try { action(); } catch (Exception e) { status = e.GetBaseException().Message; Debug.LogException(e); } }
        public static GameObject Install(VRCAvatarDescriptor avatar, bool enableFury, string prefabPath = PrefabPath) {
            if (!avatar || !HasMA || EditorApplication.isPlaying) throw new InvalidOperationException("需要 Edit Mode、Avatar Descriptor 和 Modular Avatar。");
            if (EditorUtility.IsPersistent(avatar)) throw new InvalidOperationException("请安装到场景中的 Avatar 实例。");
            if (avatar.GetComponentInChildren<MorphForgeAvatarModule>(true)) throw new InvalidOperationException("此 Avatar 已安装 MorphForge，请先移除旧模块（支持 Undo）。");
            if (avatar.GetComponentsInChildren<ParticleSystemRenderer>(true).Any(r =>
                r.sharedMaterials.Any(m => m && m.HasProperty("_WireEncoding") && m.GetFloat("_WireEncoding") == 6)))
                throw new InvalidOperationException("此 Avatar 已有 MF6 显示模块，请先移除旧模块，避免重复接收同一 OSC 命令。");
            if (avatar.expressionParameters && avatar.expressionParameters.parameters.Any(p =>
                p.name != null && (p.name.StartsWith("MF6_", StringComparison.Ordinal) || p.name.StartsWith("MF_", StringComparison.Ordinal))))
                throw new InvalidOperationException("人物参数已使用 MorphForge 保留名称 MF6_* / MF_*，请先处理命名冲突。");
            var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(prefabPath);
            if (!prefab || !prefab.GetComponent<MorphForgeAvatarModule>()) throw new InvalidOperationException("安装资源不完整。");
            Undo.IncrementCurrentGroup();
            int undo = Undo.GetCurrentGroup(); Undo.SetCurrentGroupName("Install MorphForge Avatar");
            var module = (GameObject)PrefabUtility.InstantiatePrefab(prefab, avatar.transform);
            Undo.RegisterCreatedObjectUndo(module, "Install MorphForge Avatar");
            module.transform.localPosition = Vector3.zero; module.transform.localRotation = Quaternion.identity; module.transform.localScale = Vector3.one;
            try {
                if (enableFury && HasFury) AddFuryBridge(module);
                var marker = module.GetComponent<MorphForgeAvatarModule>(); marker.vrcFuryBridge = enableFury && HasFury;
                EditorUtility.SetDirty(marker); PrefabUtility.RecordPrefabInstancePropertyModifications(marker);
                EditorSceneManager.MarkSceneDirty(avatar.gameObject.scene); Undo.CollapseUndoOperations(undo); return module;
            } catch { Undo.RevertAllDownToGroup(undo); throw; }
        }
        public static void AddFuryBridge(GameObject module) {
            var api = FindType("com.vrcfury.api.FuryComponents");
            if (api == null) throw new InvalidOperationException("未安装 VRCFury。");
            var bridge = new GameObject("MorphForge VRCFury compression bridge");
            bridge.transform.SetParent(module.transform, false);
            // Official public API. An empty FullController activates VRCFury's
            // automatic menu compressor without obsolete UnlimitedParameters.
            api.GetMethod("CreateFullController", BindingFlags.Static | BindingFlags.Public).Invoke(null, new object[] { bridge });
        }
    }
}
