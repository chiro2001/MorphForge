using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Threading.Tasks;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.SDKBase.Editor.Api;
using MorphForge.Editor;
using nadena.dev.modular_avatar.core;

// Read-only inspection of the user's existing private MF6 Avatar. No upload,
// build, scene mutation, or credential export is performed by these entries.
public static class Mf6Security20261022
{
    const string Folder = "Docs/Security20261022";
    static string Save(string name, JObject report)
    {
        Directory.CreateDirectory(Folder);
        File.WriteAllText(Folder + "/" + name + ".json", report.ToString());
        return report.ToString();
    }

    public static async Task<string> Analysis(string requestedVersion)
    {
        var avatar = await VRCApi.GetAvatar(Mf6AvatarBuild.AvatarId, forceRefresh: true);
        if (avatar.AuthorId != "usr_c0dd064e-ac42-47a7-a17e-da6540382c55" ||
            avatar.ReleaseStatus != "private" || avatar.Name != "MorphForge MF6 16K")
            throw new Exception("Unexpected Avatar identity/owner.");
        var url = avatar.GetLatestAssetUrlForCurrentPlatform();
        int start = url.IndexOf("/file_", StringComparison.Ordinal);
        if (start < 0) throw new Exception("No platform bundle.");
        var parts = url.Substring(start + 1).Split('/');
        int latest = int.Parse(parts[1]);
        int version = requestedVersion == "latest" ? latest : int.Parse(requestedVersion);
        if (version < 1 || version > latest) throw new Exception("Invalid historical file version.");
        var results = new JArray();
        foreach (var suffix in new[] { "", "/standard", "/security" })
        {
            string endpoint = "analysis/" + parts[0] + "/" + version + suffix;
            try
            {
                var response = await VRCApi.Get<JObject>(endpoint,
                    new Dictionary<string, string> { { "diagnosticCacheBust", DateTime.UtcNow.Ticks.ToString() } },
                    forceRefresh: true);
                // Analysis responses can include bundle encryption material.
                // Persist only the diagnostic fields used for this comparison.
                var diagnostic = new JObject();
                foreach (var name in new[] { "success", "error", "fileSize", "uncompressedSize",
                    "performanceRating", "avatarStats", "created_at" })
                    if (response[name] != null) diagnostic[name] = response[name].DeepClone();
                results.Add(new JObject { ["endpoint"] = endpoint, ["httpStatus"] = 200, ["response"] = diagnostic });
            }
            catch (ApiErrorException error)
            {
                results.Add(new JObject { ["endpoint"] = endpoint, ["httpStatus"] = (int)error.StatusCode,
                    ["error"] = error.ErrorMessage });
            }
        }
        return Save("analysis-file-" + version, new JObject {
            ["avatarId"] = avatar.ID, ["avatarVersion"] = avatar.Version,
            ["fileId"] = parts[0], ["fileVersion"] = version, ["latestFileVersion"] = latest,
            ["pendingUpload"] = avatar.PendingUpload, ["timeUtc"] = DateTime.UtcNow.ToString("O"),
            ["results"] = results, ["networkUploadStarted"] = false
        });
    }

    public static string Source()
    {
        var scene = SceneManager.GetActiveScene();
        if (scene.path != Mf6AvatarBuild.ScenePath || EditorApplication.isPlaying)
            throw new Exception("MF6 source scene in Edit Mode required.");
        var paths = AssetDatabase.GetDependencies(scene.path, true);
        var textures = new JArray();
        var controllers = new JArray();
        foreach (var path in paths)
        {
            var texture = AssetDatabase.LoadAssetAtPath<Texture>(path);
            if (texture != null && (path.Contains("MorphForge") || texture.width > 8192 || texture.height > 8192))
                textures.Add(new JObject { ["path"] = path, ["name"] = texture.name,
                    ["type"] = texture.GetType().Name, ["width"] = texture.width, ["height"] = texture.height,
                    ["bytes"] = UnityEngine.Profiling.Profiler.GetRuntimeMemorySizeLong(texture) });
            var controller = AssetDatabase.LoadAssetAtPath<AnimatorController>(path);
            if (controller != null)
            {
                var clips = controller.animationClips.Distinct().ToArray();
                controllers.Add(new JObject { ["path"] = path, ["layers"] = controller.layers.Length,
                    ["parameters"] = controller.parameters.Length, ["clips"] = clips.Length,
                    ["bindings"] = clips.Sum(c => AnimationUtility.GetCurveBindings(c).Length),
                    ["fileBytes"] = File.Exists(path) ? new FileInfo(path).Length : 0 });
            }
        }
        return Save("source", new JObject { ["scene"] = scene.path, ["dirty"] = scene.isDirty,
            ["timeUtc"] = DateTime.UtcNow.ToString("O"), ["textures"] = textures,
            ["controllers"] = controllers, ["prefabHash"] = AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString() });
    }
}
