using System;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using MorphForge.Editor;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using Object = UnityEngine.Object;
using VRC.Core;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase.Editor.BuildPipeline;

/// <summary>Repair the explicit FullFT profile opt-in on our two saved authoring scenes.</summary>
public static class FullFTSceneNameMigration
{
    const string DiagnosticScene = "Assets/MorphForge/Diagnostics/FinalFullFT/MorphForge_FinalFullFT_Diagnostic.unity";
    const string DiagnosticId = "avtr_df2ff222-b6ff-47fb-80be-e060868596a2";
    const string ReportRoot = "Docs/Security";

    public static string Run()
    {
        RequireIdle();
        var expectedName = Profile().avatarNamePrefix + "_Avatar";
        var paths = new[] { FullFTBuild.ScenePath, DiagnosticScene };
        var changes = new System.Collections.Generic.List<object>();
        foreach (var path in paths)
        {
            var scene = EditorSceneManager.OpenScene(path, OpenSceneMode.Single);
            var avatar = Avatar(scene);
            var beforeHash = Hash(path);
            var beforeName = avatar.name;
            if (beforeName != "Avatar" && beforeName != expectedName)
                throw new InvalidOperationException("Unexpected Avatar root name; refusing automatic change: " + beforeName);
            if (path == DiagnosticScene && avatar.GetComponent<PipelineManager>().blueprintId != DiagnosticId)
                throw new InvalidOperationException("Diagnostic target ID changed.");
            var position = avatar.transform.position;
            var rotation = avatar.transform.rotation;
            var scale = avatar.transform.localScale;
            avatar.name = expectedName;
            PrefabUtility.RecordPrefabInstancePropertyModifications(avatar);
            EditorSceneManager.MarkSceneDirty(scene);
            if (!EditorSceneManager.SaveScene(scene)) throw new InvalidOperationException("Scene save failed.");
            if (!Matches(avatar) ||
                avatar.transform.position != position || avatar.transform.rotation != rotation || avatar.transform.localScale != scale)
                throw new InvalidOperationException("Name migration changed placement or failed opt-in.");
            changes.Add(new { path, beforeName, afterName = avatar.name, beforeSha256 = beforeHash,
                afterSha256 = Hash(path), profileMatches = true, placementUnchanged = true });
        }
        return Write("fullft-scene-name-migration.json", new { success = true,
            generatedAtUtc = DateTime.UtcNow.ToString("O"), changes,
            originalStage3Changed = false, prefabWritten = false, uploaded = false });
    }

    public static string StartActualSceneBuild()
    {
        RequireIdle();
        var scene = SceneManager.GetActiveScene();
        if (scene.path != DiagnosticScene || scene.isDirty)
            throw new InvalidOperationException("Open the clean final diagnostic scene first.");
        var avatar = Avatar(scene);
        if (!Matches(avatar))
            throw new InvalidOperationException("Actual scene does not opt into its FullFT budget profile.");
        Write("fullft-actual-scene-build.json", new { success = false, status = "running",
            startedAtUtc = DateTime.UtcNow.ToString("O"), scene = scene.path, rootName = avatar.name });
        EditorApplication.delayCall += () => Build(avatar, scene);
        return JsonConvert.SerializeObject(new { success = true, accepted = true,
            report = ReportRoot + "/fullft-actual-scene-build.json", rootName = avatar.name });
    }

    static void Build(GameObject avatar, Scene scene)
    {
        GameObject clone = null;
        var sceneHash = Hash(scene.path);
        try
        {
            clone = Object.Instantiate(avatar);
            clone.hideFlags = HideFlags.HideAndDontSave;
            // Deliberately retain the real scene name (including Unity's Clone suffix).
            var cloneName = clone.name;
            var matched = Matches(clone);
            var passed = matched && VRCBuildPipelineCallbacks.OnPreprocessAvatar(clone);
            var descriptor = clone.GetComponent<VRCAvatarDescriptor>();
            var bits = descriptor.expressionParameters.CalcTotalCost();
            var finalBudget = JObject.Parse(File.ReadAllText("Docs/FullFT/final-budget.json"));
            Write("fullft-actual-scene-build.json", new { success = passed && bits == 256 && (bool)finalBudget["success"],
                status = "completed", finishedAtUtc = DateTime.UtcNow.ToString("O"), scene = scene.path,
                rootName = avatar.name, buildCloneName = cloneName, cloneNameRewritten = false,
                profileMatches = matched, bits, finalBudget,
                sceneUnchanged = sceneHash == Hash(scene.path), uploaded = false });
        }
        catch (Exception ex)
        {
            Write("fullft-actual-scene-build.json", new { success = false, status = "failed", error = ex.ToString() });
        }
        finally { if (clone) Object.DestroyImmediate(clone); }
    }

    static GameObject Avatar(Scene scene)
    {
        var descriptors = scene.GetRootGameObjects().SelectMany(root => root.GetComponentsInChildren<VRCAvatarDescriptor>(true)).ToArray();
        if (descriptors.Length != 1) throw new InvalidOperationException("Expected exactly one scene Avatar.");
        var avatar = descriptors[0].gameObject;
        if (PrefabUtility.GetPrefabAssetPathOfNearestInstanceRoot(avatar) != FullFTBuild.TestAvatarPath)
            throw new InvalidOperationException("Scene Avatar is not the final FullFT prefab instance.");
        return avatar;
    }

    static FullFTSyncProfile Profile() => AssetDatabase.LoadAssetAtPath<FullFTSyncProfile>(FullFTSyncProfile.DefaultAssetPath);
    static bool Matches(GameObject target) => target && Profile() &&
        !string.IsNullOrEmpty(Profile().avatarNamePrefix) &&
        target.name.StartsWith(Profile().avatarNamePrefix, StringComparison.Ordinal);

    static void RequireIdle()
    {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling)
            throw new InvalidOperationException("Requires idle Edit Mode.");
    }
    static string Write(string file, object value)
    {
        Directory.CreateDirectory(ReportRoot);
        var json = JsonConvert.SerializeObject(value, Formatting.Indented);
        File.WriteAllText(ReportRoot + "/" + file, json + Environment.NewLine);
        return json;
    }
    static string Hash(string path)
    {
        using (var sha = SHA256.Create())
            return BitConverter.ToString(sha.ComputeHash(File.ReadAllBytes(path))).Replace("-", "").ToLowerInvariant();
    }
}
