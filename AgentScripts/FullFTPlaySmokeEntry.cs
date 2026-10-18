using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Threading.Tasks;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
using MorphForge.Editor;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.SDK3.Avatars.Components;

public static class FullFTPlaySmokeEntry
{
    const string ReportDir = "Docs/FullFT";
    const string IsolationPath = ReportDir + "/play-isolation.json";
    const string RestorePath = ReportDir + "/play-isolation-restore.json";

    sealed class SceneObjectState
    {
        public string scenePath;
        public string path;
        public bool activeSelf;
    }

    public static string Prepare()
    {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling)
            throw new InvalidOperationException("Play isolation requires idle Edit Mode.");

        var targetScene = SceneManager.GetSceneByPath(FullFTBuild.ScenePath);
        if (!targetScene.IsValid())
            throw new InvalidOperationException(
                $"FullFT target scene is not open: {FullFTBuild.ScenePath}");
        if (targetScene.isDirty)
            throw new InvalidOperationException(
                "The FullFT target scene has unsaved edits. Save it before Play smoke.");

        var sourceScene = SceneManager.GetSceneByPath(
            "Assets/MorphForge/Scenes/MorphForge_Stage3.unity");
        if (!sourceScene.IsValid())
            throw new InvalidOperationException(
                "The saved user Stage3 source scene is not open.");
        if (!sourceScene.isDirty)
            throw new InvalidOperationException(
                "The user Stage3 source scene is expected to retain its existing dirty state.");

        var sourceHashBefore = Sha256(AssetPathToAbsolute(sourceScene.path));
        var states = new List<SceneObjectState>();
        foreach (var scene in EnumerateScenes().Where(scene => scene != targetScene))
        {
            var objects = SceneComponents<VRCAvatarDescriptor>(scene)
                .Select(component => component.gameObject)
                .Concat(SceneComponents<GestureManager>(scene)
                    .Select(component => component.gameObject))
                .Concat(SceneComponents<MorphForgePlayReceiver>(scene)
                    .Select(component => component.gameObject))
                .Distinct()
                .ToArray();
            foreach (var gameObject in objects)
            {
                states.Add(new SceneObjectState
                {
                    scenePath = scene.path,
                    path = HierarchyPath(gameObject.transform),
                    activeSelf = gameObject.activeSelf,
                });
                if (gameObject.activeSelf)
                    gameObject.SetActive(false);
            }
        }

        SceneManager.SetActiveScene(targetScene);
        var activeAvatars = ActiveSceneComponents<VRCAvatarDescriptor>(targetScene);
        var activeManagers = ActiveSceneComponents<GestureManager>(targetScene);
        var activeReceivers = ActiveSceneComponents<MorphForgePlayReceiver>(targetScene);
        if (activeAvatars.Length != 1 ||
            activeManagers.Length != 1 ||
            activeReceivers.Length != 1)
            throw new InvalidOperationException(
                "Isolation did not leave exactly one active Avatar/GM/Receiver " +
                $"in the FullFT scene: {activeAvatars.Length}/" +
                $"{activeManagers.Length}/{activeReceivers.Length}.");
        var receiver = activeReceivers[0];
        if (!receiver.Avatar ||
            receiver.Avatar != activeAvatars[0] ||
            !receiver.AutoStart ||
            receiver.ReceivePort != 9012)
            throw new InvalidOperationException(
                "The isolated FullFT receiver is not bound to the target avatar on 9012.");
        if (activeManagers[0].settings == null ||
            !ReferenceEquals(activeManagers[0].settings.favourite, activeAvatars[0]))
            throw new InvalidOperationException(
                "The isolated FullFT GM does not reference the target avatar.");

        var sourceHashAfter = Sha256(AssetPathToAbsolute(sourceScene.path));
        if (sourceHashAfter != sourceHashBefore)
            throw new InvalidOperationException(
                "The Stage3 source file changed while isolating the Play scenes.");
        var report = new JObject
        {
            ["success"] = true,
            ["phase"] = "prepare",
            ["capturedAtUtc"] = DateTime.UtcNow.ToString("O"),
            ["targetScene"] = targetScene.path,
            ["sourceScene"] = sourceScene.path,
            ["sourceSceneSha256Before"] = sourceHashBefore,
            ["sourceSceneSha256AfterPrepare"] = sourceHashAfter,
            ["sourceSceneDirtyBefore"] = true,
            ["capturedUnsavedSource"] = true,
            ["sourceSceneSavedOrDiscarded"] = false,
            ["sourceSceneStillOpen"] = true,
            ["disabledObjects"] = JArray.FromObject(states),
            ["activeAvatarCount"] = 1,
            ["activeGestureManagerCount"] = 1,
            ["activeReceiverCount"] = 1,
            ["receivePort"] = receiver.ReceivePort,
            ["sendPort"] = receiver.SendPort,
        };
        WriteJson(IsolationPath, report);
        return report.ToString(Newtonsoft.Json.Formatting.None);
    }

    public static string Restore()
    {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling)
            throw new InvalidOperationException("Restore Play isolation after Play Mode stops.");
        var statePath = Path.Combine(ProjectRoot, IsolationPath);
        if (!File.Exists(statePath))
            throw new InvalidOperationException("No recorded Play isolation state exists.");
        var state = JObject.Parse(File.ReadAllText(statePath));
        var targetScene = SceneManager.GetSceneByPath(
            state.Value<string>("targetScene"));
        if (!targetScene.IsValid())
            throw new InvalidOperationException("The FullFT target scene is no longer open.");
        var sourceScene = SceneManager.GetSceneByPath(
            state.Value<string>("sourceScene"));
        if (!sourceScene.IsValid())
            throw new InvalidOperationException(
                "The Stage3 source scene is no longer open; refusing to reopen or reload it.");

        var restored = 0;
        var missing = new List<string>();
        foreach (var item in state["disabledObjects"] as JArray ?? new JArray())
        {
            var scenePath = item.Value<string>("scenePath");
            var scene = SceneManager.GetSceneByPath(scenePath);
            var path = item.Value<string>("path") ?? string.Empty;
            var transform = scene.IsValid() ? FindHierarchy(scene, path) : null;
            if (!transform)
            {
                missing.Add($"{scenePath}:{path}");
                continue;
            }
            var wantActive = item.Value<bool>("activeSelf");
            if (transform.gameObject.activeSelf != wantActive)
                transform.gameObject.SetActive(wantActive);
            restored++;
        }
        if (missing.Count != 0)
            throw new InvalidOperationException(
                "Failed to restore isolated objects: " + string.Join(", ", missing));

        EditorSceneManager.MarkSceneDirty(sourceScene);
        SceneManager.SetActiveScene(targetScene);
        var sourceHashAfter = Sha256(AssetPathToAbsolute(sourceScene.path));
        var sourceHashBefore = state.Value<string>("sourceSceneSha256Before");
        if (!string.Equals(
                sourceHashAfter, sourceHashBefore, StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException(
                "The Stage3 source file SHA changed during Play smoke.");
        if (!sourceScene.isDirty)
            throw new InvalidOperationException(
                "The Stage3 source scene did not retain its dirty state.");

        var targetAvatar = SceneComponents<VRCAvatarDescriptor>(targetScene).Single();
        var missingScripts = targetAvatar
            .GetComponentsInChildren<Transform>(true)
            .Sum(transform => GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(
                transform.gameObject));
        var morphForgeScripts = targetAvatar
            .GetComponentsInChildren<MonoBehaviour>(true)
            .Where(component => component &&
                                component.GetType().Namespace != null &&
                                component.GetType().Namespace.StartsWith(
                                    "MorphForge", StringComparison.Ordinal))
            .Select(component => component.GetType().FullName)
            .Distinct()
            .ToArray();

        var report = new JObject
        {
            ["success"] = true,
            ["phase"] = "restore",
            ["capturedAtUtc"] = DateTime.UtcNow.ToString("O"),
            ["targetScene"] = targetScene.path,
            ["sourceScene"] = sourceScene.path,
            ["restoredObjectCount"] = restored,
            ["sourceSceneSha256Before"] = sourceHashBefore,
            ["sourceSceneSha256After"] = sourceHashAfter,
            ["sourceSceneHashUnchanged"] = true,
            ["sourceSceneDirty"] = sourceScene.isDirty,
            ["sourceSceneDirtyPreserved"] = true,
            ["sourceSceneStillOpen"] = true,
            ["targetSceneAvatarCount"] =
                SceneComponents<VRCAvatarDescriptor>(targetScene).Length,
            ["targetSceneGestureManagerCount"] =
                SceneComponents<GestureManager>(targetScene).Length,
            ["targetSceneReceiverCount"] =
                SceneComponents<MorphForgePlayReceiver>(targetScene).Length,
            ["avatarMissingScripts"] = missingScripts,
            ["morphForgeRuntimeScriptsInAvatar"] = JArray.FromObject(morphForgeScripts),
            ["activeScene"] = SceneManager.GetActiveScene().path,
        };
        WriteJson(RestorePath, report);
        return report.ToString(Newtonsoft.Json.Formatting.None);
    }

    public static async Task<string> SmokeFacialTracking()
    {
        if (!EditorApplication.isPlaying)
            throw new InvalidOperationException("FT tracking smoke requires Play Mode.");
        var scene = SceneManager.GetActiveScene();
        var managers = SceneComponents<GestureManager>(scene)
            .Where(manager => manager.gameObject.activeInHierarchy)
            .ToArray();
        if (managers.Length != 1)
            throw new InvalidOperationException(
                $"Expected one active Gesture Manager, found {managers.Length}.");
        var module = managers[0].Module as ModuleVrc3;
        if (module == null)
            throw new InvalidOperationException(
                "The FullFT Gesture Manager is not using ModuleVrc3.");
        var avatar = module.Avatar;
        if (!avatar)
            throw new InvalidOperationException("The GM module has no avatar.");

        async Task Set(string name, float value)
        {
            var parameter = module.Params[name];
            parameter.Set(module, value);
            await Task.Delay(120);
        }

        await Set("IsLocal", 0);
        await Set("EyeTrackingActive", 0);
        await Set("LipTrackingActive", 0);
        await Set("VisemesEnable", 0);
        await Set("FT/v2/EyeLeftX", 0);
        await Set("FT/v2/EyeRightX", 0);
        await Set("FT/v2/JawOpen", 0);
        await Set("FT/v2/MouthClosed", 1);
        await Task.Delay(350);
        var neutral = FacialSignature(avatar);

        await Set("EyeTrackingActive", 1);
        await Set("LipTrackingActive", 1);
        await Set("VisemesEnable", 1);
        await Set("FT/v2/EyeLeftX", 1);
        await Set("FT/v2/EyeRightX", 1);
        await Set("FT/v2/JawOpen", 1);
        await Set("FT/v2/MouthClosed", 0);
        await Task.Delay(500);
        var active = FacialSignature(avatar);
        var changed = new List<string>();
        foreach (var key in neutral.Keys.Union(active.Keys))
        {
            var before = neutral.TryGetValue(key, out var b) ? b : string.Empty;
            var after = active.TryGetValue(key, out var a) ? a : string.Empty;
            if (!string.Equals(before, after, StringComparison.Ordinal))
                changed.Add(key);
        }

        await Set("EyeTrackingActive", 0);
        await Set("LipTrackingActive", 0);
        await Set("VisemesEnable", 0);
        await Set("FT/v2/EyeLeftX", 0);
        await Set("FT/v2/EyeRightX", 0);
        await Set("FT/v2/JawOpen", 0);
        await Set("FT/v2/MouthClosed", 1);
        await Set("IsLocal", 1);

        var result = new JObject
        {
            ["success"] = changed.Count > 0,
            ["phase"] = "remote-facial-tracking",
            ["avatar"] = avatar.name,
            ["isLocal"] = 0,
            ["eyeTrackingActive"] = 1,
            ["lipTrackingActive"] = 1,
            ["visemesEnable"] = 1,
            ["ftInputs"] = new JArray(
                "FT/v2/EyeLeftX",
                "FT/v2/EyeRightX",
                "FT/v2/JawOpen",
                "FT/v2/MouthClosed"),
            ["changedOutputCount"] = changed.Count,
            ["changedOutputs"] = new JArray(changed.Take(128).ToArray()),
            ["neutralSamples"] = new JArray(neutral.Select(pair => new JObject
            {
                ["path"] = pair.Key,
                ["value"] = pair.Value,
            })),
            ["activeSamples"] = new JArray(active.Select(pair => new JObject
            {
                ["path"] = pair.Key,
                ["value"] = pair.Value,
            })),
        };
        WriteJson(ReportDir + "/ft-remote-smoke.json", result);
        if (changed.Count == 0)
            throw new InvalidOperationException(
                "Remote FT stimulus produced no visible blend-shape or facial-bone change.");
        return result.ToString(Newtonsoft.Json.Formatting.None);
    }

    public static string CloseSavedScene()
    {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling)
            throw new InvalidOperationException(
                "Close the saved FullFT scene only after Play Mode stops.");
        var targetScene = SceneManager.GetSceneByPath(FullFTBuild.ScenePath);
        var sourceScene = SceneManager.GetSceneByPath(
            "Assets/MorphForge/Scenes/MorphForge_Stage3.unity");
        if (!sourceScene.IsValid() || !sourceScene.isDirty)
            throw new InvalidOperationException(
                "The user Stage3 source scene must remain open and dirty.");
        if (!targetScene.IsValid())
            throw new InvalidOperationException("The FullFT target scene is not open.");

        var wasDirty = targetScene.isDirty;
        if (wasDirty && !EditorSceneManager.SaveScene(targetScene))
            throw new InvalidOperationException(
                "Could not save the FullFT scene before closing it.");
        if (!EditorSceneManager.CloseScene(targetScene, true))
            throw new InvalidOperationException("Could not close the FullFT scene.");

        var previewClosed = false;
        var preview = SceneManager.GetSceneByPath(
            "Packages/nadena.dev.ndmf/Runtime/___NDMF Preview___.unity");
        if (preview.IsValid() && !preview.isDirty)
        {
            previewClosed = EditorSceneManager.CloseScene(preview, true);
        }

        SceneManager.SetActiveScene(sourceScene);
        EditorSceneManager.MarkSceneDirty(sourceScene);
        var remaining = Enumerable.Range(0, SceneManager.sceneCount)
            .Select(index => SceneManager.GetSceneAt(index))
            .Select(scene => new
            {
                scene.path,
                scene.isDirty,
                isActive = scene == SceneManager.GetActiveScene(),
            })
            .ToArray();
        var report = new
        {
            success = true,
            phase = "close-saved-fullft-scene",
            capturedAtUtc = DateTime.UtcNow.ToString("O"),
            fullFtSceneWasDirty = wasDirty,
            fullFtSceneSaved = wasDirty,
            fullFtSceneClosed = !SceneManager.GetSceneByPath(
                FullFTBuild.ScenePath).IsValid(),
            fullFtSceneAssetRetained = File.Exists(
                AssetPathToAbsolute(FullFTBuild.ScenePath)),
            ndmfPreviewClosed = previewClosed,
            sourceScene = sourceScene.path,
            sourceSceneStillOpen = sourceScene.IsValid(),
            sourceSceneStillDirty = sourceScene.isDirty,
            sourceSceneSavedOrDiscarded = false,
            activeScene = SceneManager.GetActiveScene().path,
            remainingScenes = remaining,
        };
        WriteJson(ReportDir + "/scene-close.json", JObject.FromObject(report));
        return Newtonsoft.Json.JsonConvert.SerializeObject(report);
    }

    static Dictionary<string, string> FacialSignature(GameObject avatar)
    {
        var values = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var renderer in avatar.GetComponentsInChildren<SkinnedMeshRenderer>(true))
        {
            if (!renderer.sharedMesh) continue;
            var path = HierarchyPath(renderer.transform);
            for (var i = 0; i < renderer.sharedMesh.blendShapeCount; i++)
                values[$"blend:{path}:{renderer.sharedMesh.GetBlendShapeName(i)}"] =
                    renderer.GetBlendShapeWeight(i).ToString("R");
        }
        foreach (var transform in avatar.GetComponentsInChildren<Transform>(true)
                     .Where(transform =>
                         transform.name.IndexOf("eye", StringComparison.OrdinalIgnoreCase) >= 0 ||
                         transform.name.IndexOf("jaw", StringComparison.OrdinalIgnoreCase) >= 0 ||
                         transform.name.IndexOf("mouth", StringComparison.OrdinalIgnoreCase) >= 0))
        {
            var path = HierarchyPath(transform);
            values[$"bone:{path}:position"] = transform.localPosition.ToString("R");
            values[$"bone:{path}:rotation"] = transform.localRotation.ToString("R");
            values[$"bone:{path}:scale"] = transform.localScale.ToString("R");
        }
        return values;
    }

    static IEnumerable<Scene> EnumerateScenes()
    {
        for (var i = 0; i < SceneManager.sceneCount; i++)
            yield return SceneManager.GetSceneAt(i);
    }

    static T[] SceneComponents<T>(Scene scene) where T : Component
    {
        return scene.GetRootGameObjects()
            .SelectMany(root => root.GetComponentsInChildren<T>(true))
            .ToArray();
    }

    static T[] ActiveSceneComponents<T>(Scene scene) where T : Component
    {
        return SceneComponents<T>(scene)
            .Where(component => component.gameObject.activeInHierarchy)
            .ToArray();
    }

    static Transform FindHierarchy(Scene scene, string path)
    {
        var parts = path.Split('/');
        var current = scene.GetRootGameObjects()
            .FirstOrDefault(root => root.name == parts[0])?.transform;
        for (var i = 1; current && i < parts.Length; i++)
            current = current.Find(parts[i]);
        return current;
    }

    static string HierarchyPath(Transform transform)
    {
        var parts = new List<string>();
        for (var current = transform; current; current = current.parent)
            parts.Add(current.name);
        parts.Reverse();
        return string.Join("/", parts);
    }

    static string AssetPathToAbsolute(string assetPath)
    {
        return Path.Combine(ProjectRoot, assetPath.Replace('/', Path.DirectorySeparatorChar));
    }

    static string ProjectRoot =>
        Directory.GetParent(Application.dataPath)?.FullName ??
        throw new InvalidOperationException("Cannot resolve Unity project root.");

    static string Sha256(string path)
    {
        using (var stream = File.OpenRead(path))
        using (var sha = SHA256.Create())
            return BitConverter.ToString(sha.ComputeHash(stream)).Replace("-", "");
    }

    static void WriteJson(string relative, JObject value)
    {
        var path = Path.Combine(ProjectRoot, relative);
        Directory.CreateDirectory(Path.GetDirectoryName(path) ?? ProjectRoot);
        File.WriteAllText(path, value.ToString(Newtonsoft.Json.Formatting.Indented));
    }
}
