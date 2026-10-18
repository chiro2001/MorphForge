using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Newtonsoft.Json;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

public static class FullFTSceneNoSleepAuditEntry
{
    private const string ScenePath = "Assets/MorphForge/FullFT/MorphForge_FullFT.unity";
    private const string PrefabPath = "Assets/MorphForge/FullFT/Avatar.prefab";
    private const string OldControllerPath =
        "Assets/Nemesis/Nemesis_Avatar/Controller/Nemesis_Locomotion.controller";
    private const string NewControllerPath =
        "Assets/MorphForge/FullFT/Nemesis_Locomotion_NoSleep.controller";
    private const string ForbiddenPackage = "Packages/minminmean.supine/";

    public static string Run()
    {
        var activeBefore = SceneState();
        var oldGuid = AssetDatabase.AssetPathToGUID(OldControllerPath);
        var newGuid = AssetDatabase.AssetPathToGUID(NewControllerPath);
        var prefabGuid = AssetDatabase.AssetPathToGUID(PrefabPath);
        var dependencies = AssetDatabase.GetDependencies(ScenePath, true)
            .OrderBy(path => path, StringComparer.Ordinal)
            .ToArray();
        var sceneFile = AbsoluteAssetPath(ScenePath);
        var raw = File.ReadAllText(sceneFile);
        var modifications = ParseModificationPaths(raw);
        var baseLayerModifications = modifications
            .Where(path => path.IndexOf("baseAnimationLayers", StringComparison.OrdinalIgnoreCase) >= 0 ||
                           path.IndexOf("specialAnimationLayers", StringComparison.OrdinalIgnoreCase) >= 0 ||
                           path.IndexOf("animatorController", StringComparison.OrdinalIgnoreCase) >= 0)
            .ToArray();
        var controllerDependencies = dependencies
            .Where(path => path.EndsWith(".controller", StringComparison.OrdinalIgnoreCase))
            .ToArray();
        var packageDependencies = dependencies
            .Where(path => path.StartsWith(
                ForbiddenPackage, StringComparison.OrdinalIgnoreCase))
            .ToArray();
        var oldGuidFound = !string.IsNullOrEmpty(oldGuid) &&
                           raw.IndexOf(oldGuid, StringComparison.OrdinalIgnoreCase) >= 0;
        var newGuidFound = !string.IsNullOrEmpty(newGuid) &&
                           raw.IndexOf(newGuid, StringComparison.OrdinalIgnoreCase) >= 0;
        var forbiddenTextHits = new[] { "minminmean", "VRCSupine", "Supine" }
            .Select(text => new
            {
                text,
                count = CountOrdinalIgnoreCase(raw, text)
            })
            .ToArray();
        var usesNewController = dependencies.Contains(
            NewControllerPath, StringComparer.Ordinal);
        var usesOldController = dependencies.Contains(
            OldControllerPath, StringComparer.Ordinal);
        var report = new Dictionary<string, object>
        {
            ["success"] = usesNewController &&
                          !usesOldController &&
                          packageDependencies.Length == 0 &&
                          baseLayerModifications.Length == 0 &&
                          !oldGuidFound,
            ["scope"] = "Read-only saved scene dependency and prefab-instance override audit",
            ["generatedAtUtc"] = DateTime.UtcNow.ToString("O"),
            ["scenePath"] = ScenePath,
            ["sceneSha256"] = Sha256File(sceneFile),
            ["sceneLastWriteTimeUtc"] = File.GetLastWriteTimeUtc(sceneFile).ToString("O"),
            ["prefabPath"] = PrefabPath,
            ["prefabGuid"] = prefabGuid,
            ["oldControllerPath"] = OldControllerPath,
            ["oldControllerGuid"] = oldGuid,
            ["newControllerPath"] = NewControllerPath,
            ["newControllerGuid"] = newGuid,
            ["dependencyCount"] = dependencies.Length,
            ["controllerDependencies"] = controllerDependencies,
            ["usesNoSleepController"] = usesNewController,
            ["usesOriginalLocomotionController"] = usesOldController,
            ["packageDependencies"] = packageDependencies,
            ["prefabInstanceModificationCount"] = modifications.Count,
            ["baseLayerOrControllerModificationPaths"] = baseLayerModifications,
            ["oldControllerGuidInSceneText"] = oldGuidFound,
            ["newControllerGuidInSceneText"] = newGuidFound,
            ["forbiddenTextHits"] = forbiddenTextHits,
            ["interpretation"] = usesNewController &&
                                 !usesOldController &&
                                 packageDependencies.Length == 0 &&
                                 baseLayerModifications.Length == 0
                ? "The saved scene inherits the repaired prefab and has no Base layer controller override."
                : "The saved scene still contains a controller override or a forbidden dependency.",
            ["activeScenesBefore"] = activeBefore,
            ["activeScenesAfter"] = SceneState(),
            ["activeSceneStateChanged"] = !JsonConvert.SerializeObject(activeBefore)
                .Equals(JsonConvert.SerializeObject(SceneState()), StringComparison.Ordinal),
            ["nonActions"] = new[]
            {
                "No scene was opened, switched, saved, closed, or marked clean.",
                "No asset was created, modified, imported, or deleted.",
                "No Play Mode, build, upload, VRChat launch, or OSC send occurred."
            }
        };
        var reportPath = Path.Combine(
            Application.dataPath, "..", "Docs/Avatars/fullft-scene-no-sleep-audit.json");
        Directory.CreateDirectory(Path.GetDirectoryName(reportPath));
        File.WriteAllText(
            reportPath,
            JsonConvert.SerializeObject(report, Formatting.Indented) + "\n");
        return JsonConvert.SerializeObject(report, Formatting.Indented);
    }

    private static List<string> ParseModificationPaths(string raw)
    {
        var paths = new List<string>();
        foreach (var line in raw.Split('\n'))
        {
            var trimmed = line.Trim();
            if (trimmed.StartsWith("propertyPath:", StringComparison.Ordinal))
                paths.Add(trimmed.Substring("propertyPath:".Length).Trim());
        }
        return paths;
    }

    private static int CountOrdinalIgnoreCase(string haystack, string needle)
    {
        var count = 0;
        var index = 0;
        while ((index = haystack.IndexOf(
                   needle, index, StringComparison.OrdinalIgnoreCase)) >= 0)
        {
            count++;
            index += needle.Length;
        }
        return count;
    }

    private static string AbsoluteAssetPath(string path)
    {
        return Path.GetFullPath(Path.Combine(Application.dataPath, "..", path));
    }

    private static string Sha256File(string path)
    {
        using (var stream = File.OpenRead(path))
        using (var sha = System.Security.Cryptography.SHA256.Create())
            return string.Concat(sha.ComputeHash(stream).Select(value => value.ToString("x2")));
    }

    private static List<object> SceneState()
    {
        var scenes = new List<object>();
        for (var index = 0; index < UnityEngine.SceneManagement.SceneManager.sceneCount; index++)
        {
            var scene = UnityEngine.SceneManagement.SceneManager.GetSceneAt(index);
            scenes.Add(new
            {
                path = scene.path,
                name = scene.name,
                isLoaded = scene.isLoaded,
                isDirty = scene.isDirty,
                isActive = EditorSceneManager.GetActiveScene() == scene
            });
        }
        return scenes;
    }
}
