using System;
using System.IO;
using System.Linq;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using MorphForge.Editor;
using MorphForgeAvatar.Editor;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase.Editor.BuildPipeline;

public static class Mf6CacheUpgrade20261022 {
    const string Folder = "Docs/Security20261022";
    const string BackupRoot = "Assets/MorphForge/Validation/Security20261022/BeforeCacheFix";
    public static string RestorePlay() {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling) throw new Exception("Stop the owned test first.");
        var scene = SceneManager.GetActiveScene();
        if (scene.path != Mf6AvatarBuild.ScenePath) throw new Exception("Unexpected active scene.");
        var report = JObject.Parse(File.ReadAllText(Folder + "/cache-rebuild.json"));
        if (AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString() != (string)report["prefabHash"])
            throw new Exception("Source Prefab changed during the test.");
        string snapshot = null;
        if (scene.isDirty) {
            snapshot = "Assets/MorphForge/Validation/Security20261022/AfterCachePlay-" + DateTime.UtcNow.ToString("HHmmss") + ".unity";
            if (File.Exists(snapshot) || !EditorSceneManager.SaveScene(scene, snapshot, true)) throw new Exception("Could not preserve test scene state.");
            scene = EditorSceneManager.OpenScene(Mf6AvatarBuild.ScenePath, OpenSceneMode.Single);
        }
        return new JObject { ["success"] = !scene.isDirty, ["snapshot"] = snapshot, ["scene"] = scene.path }.ToString();
    }
    public static string Backup() {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling) throw new Exception("Idle Edit Mode required.");
        var scene = SceneManager.GetActiveScene();
        if (scene.path != Mf6AvatarBuild.ScenePath || scene.isDirty) throw new Exception("Clean saved MF6 source required.");
        for (int i = 0; i < SceneManager.sceneCount; i++) if (SceneManager.GetSceneAt(i).isDirty)
            throw new Exception("Unsaved scene preserved.");
        if (AssetDatabase.IsValidFolder(BackupRoot)) throw new Exception("Backup already exists; preserve it.");
        CatalogVariant.EnsureFolder(BackupRoot);
        var rows = new JArray();
        foreach (var path in new[] { Mf6TextBuild.Root, Mf6MotionBuild.Root, Mf6AvatarBuild.AvatarPath,
            AvatarSetupWindow.PrefabPath, "Assets/MorphForge/MF6/Decoder.controller" }) {
            string copy = BackupRoot + "/" + path.Substring("Assets/".Length);
            CatalogVariant.EnsureFolder(Path.GetDirectoryName(copy).Replace('\\', '/'));
            if (!AssetDatabase.CopyAsset(path, copy)) throw new Exception("Backup failed: " + path);
            rows.Add(new JObject { ["source"] = path, ["backup"] = copy,
                ["sourceHash"] = AssetDatabase.GetAssetDependencyHash(path).ToString() });
        }
        if (!EditorSceneManager.SaveScene(scene, BackupRoot + "/Source.unity", true)) throw new Exception("Source snapshot failed.");
        var characters = new JArray(AssetDatabase.GetDependencies(Mf6AvatarBuild.AvatarPath, true)
            .Where(p => p.StartsWith("Assets/Nemesis/") && p.EndsWith(".prefab"))
            .Select(p => (JToken)new JObject { ["path"] = p, ["hash"] = AssetDatabase.GetAssetDependencyHash(p).ToString() }));
        var report = new JObject { ["success"] = true, ["timeUtc"] = DateTime.UtcNow.ToString("O"),
            ["assets"] = rows, ["characterPrefabs"] = characters, ["sourceScene"] = scene.path };
        Directory.CreateDirectory(Folder); File.WriteAllText(Folder + "/cache-backup.json", report.ToString()); return report.ToString();
    }
    public static string Rebuild() {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling) throw new Exception("Idle Edit Mode required.");
        var scene = SceneManager.GetActiveScene();
        if (scene.path != Mf6AvatarBuild.ScenePath || scene.isDirty) throw new Exception("Clean source scene required.");
        var backup = JObject.Parse(File.ReadAllText(Folder + "/cache-backup.json"));
        var previous = File.Exists(Folder + "/cache-rebuild.json") ? JObject.Parse(File.ReadAllText(Folder + "/cache-rebuild.json")) : null;
        if (previous != null) {
            if (AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString() != (string)previous["prefabHash"])
                throw new Exception("Prefab changed after the previous controlled rebuild.");
            if (previous["publicPrefabHash"] != null && AssetDatabase.GetAssetDependencyHash(AvatarSetupWindow.PrefabPath).ToString() != (string)previous["publicPrefabHash"])
                throw new Exception("Public module changed after the previous controlled rebuild.");
            File.WriteAllText(Folder + "/cache-rebuild-attempt-" + ((int?)previous["attempt"] ?? 1) + ".json", previous.ToString());
        } else foreach (var row in backup["assets"].Where(r => ((string)r["source"]).EndsWith(".prefab")))
            if (AssetDatabase.GetAssetDependencyHash((string)row["source"]).ToString() != (string)row["sourceHash"])
                throw new Exception("Prefab changed after backup: " + row["source"]);
        var words = "State motions latch packed words directly; no ParameterDriver copies from animated parameters.";
        Mf6TextBuild.BuildAssets(); var motion = JObject.Parse(Mf6MotionBuild.Build());
        var decoders=new[]{"Assets/MorphForge/MF6/Decoder.controller"}.Concat(AssetDatabase.GetDependencies(AvatarSetupWindow.PrefabPath,true).Where(p=>p.EndsWith("/Decoder.controller"))).Distinct().ToArray();
        foreach(var path in decoders) {
            if(path.StartsWith(AvatarSetupWindow.Root+"/")) {
                string copy=BackupRoot+"/"+path.Substring("Assets/".Length);
                if(!File.Exists(copy)){CatalogVariant.EnsureFolder(Path.GetDirectoryName(copy).Replace('\\','/'));if(!AssetDatabase.CopyAsset(path,copy))throw new Exception("Public decoder backup failed.");}
            }
            Mf6TextBuild.ExtendInbox(AssetDatabase.LoadAssetAtPath<AnimatorController>(path));
        }
        foreach (var path in new[] { Mf6AvatarBuild.AvatarPath, AvatarSetupWindow.PrefabPath }) {
            var contents = PrefabUtility.LoadPrefabContents(path);
            try {
                var module = path == Mf6AvatarBuild.AvatarPath ? contents.transform.Find("MorphForgePlacement").gameObject : contents;
                var old = module.transform.Find("WordCache");
                if (old) {
                    var merge = old.GetComponent<nadena.dev.modular_avatar.core.ModularAvatarMergeAnimator>();
                    if (!merge || !AssetDatabase.GetAssetPath(merge.animator).EndsWith("/Generated/Words/Words.controller"))
                        throw new Exception("Unexpected object occupying WordCache.");
                    UnityEngine.Object.DestroyImmediate(old.gameObject);
                }
                PrefabUtility.SaveAsPrefabAsset(contents, path);
            } finally { PrefabUtility.UnloadPrefabContents(contents); }
        }
        AssetDatabase.SaveAssets(); EditorSceneManager.MarkSceneDirty(scene); EditorSceneManager.SaveScene(scene);
        var characters = new JArray(backup["characterPrefabs"].Select(row => (JToken)new JObject {
            ["path"] = row["path"], ["unchanged"] = AssetDatabase.GetAssetDependencyHash((string)row["path"]).ToString() == (string)row["hash"] }));
        if (characters.Any(row => (bool)row["unchanged"] != true)) throw new Exception("Character Prefab dependency changed.");
        var preview = EditorSceneManager.NewPreviewScene(); GameObject clone = null; JObject merged;
        try {
            clone = UnityEngine.Object.Instantiate(AssetDatabase.LoadAssetAtPath<GameObject>(Mf6AvatarBuild.AvatarPath));
            SceneManager.MoveGameObjectToScene(clone, preview); clone.name = "MorphForge_MF6_Avatar";
            if (!VRCBuildPipelineCallbacks.OnPreprocessAvatar(clone)) throw new Exception("Native merge failed.");
            var avatar = clone.GetComponent<VRCAvatarDescriptor>();
            var fx = (AnimatorController)avatar.baseAnimationLayers.Single(l => l.type == VRCAvatarDescriptor.AnimLayerType.FX).animatorController;
            merged = new JObject { ["parameters"] = fx.parameters.Length, ["layers"] = fx.layers.Length,
                ["syncedBits"] = avatar.expressionParameters.CalcTotalCost(),
                ["wordParameters"] = fx.parameters.Count(p => p.name.StartsWith("MF6_Word")),
                ["textRenderers"] = clone.GetComponentsInChildren<Renderer>(true).Count(r => r.sharedMaterials.Any(m => m && m.shader && m.shader.name.Contains("PixelText16"))) };
            var bindings = fx.animationClips.SelectMany(AnimationUtility.GetCurveBindings).Where(b => b.type == typeof(Animator) && b.propertyName.StartsWith("MF6_")).ToArray();
            merged["cachedFloatBindings"] = bindings.Length;
            merged["invalidCachePaths"] = new JArray(bindings.Where(b => b.path != "").Select(b => b.path).Distinct());
            if (fx.parameters.Length > 2048 || avatar.expressionParameters.CalcTotalCost() != 254 || (int)merged["wordParameters"] != 0 || bindings.Length == 0 || bindings.Any(b => b.path != ""))
                throw new Exception("Cache reduction or sync budget target failed: " + merged);
        } finally { if (clone) UnityEngine.Object.DestroyImmediate(clone); EditorSceneManager.ClosePreviewScene(preview); }
        var closure = AssetDatabase.GetDependencies(AvatarSetupWindow.PrefabPath, true);
        var outside = closure.Where(p => p.StartsWith("Assets/") && !p.StartsWith(AvatarSetupWindow.Root + "/")).ToArray();
        if (outside.Length != 0) throw new Exception("Public module escaped its root.");
        var text = AssetDatabase.LoadAssetAtPath<AnimatorController>(Mf6TextBuild.Root + "/Text.controller");
        var result = new JObject { ["success"] = true, ["words"] = words, ["textParameters"] = text.parameters.Length,
            ["attempt"] = ((int?)previous?["attempt"] ?? (previous != null ? 1 : 0)) + 1,
            ["motion"] = motion, ["merged"] = merged, ["characterPrefabs"] = characters,
            ["publicDependencyCount"] = closure.Length, ["publicExternalAssets"] = new JArray(outside),
            ["prefabHash"] = AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString(),
            ["publicPrefabHash"] = AssetDatabase.GetAssetDependencyHash(AvatarSetupWindow.PrefabPath).ToString(),
            ["timeUtc"] = DateTime.UtcNow.ToString("O") };
        File.WriteAllText(Folder + "/cache-rebuild.json", result.ToString()); return result.ToString();
    }
}
