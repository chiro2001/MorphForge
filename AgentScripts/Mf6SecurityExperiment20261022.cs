using System;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Threading;
using System.Threading.Tasks;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using UnityEngine.UIElements;
using MorphForge.Editor;
using MorphForgeAvatar.Editor;
using nadena.dev.modular_avatar.core;
using VRC.Core;
using VRC.SDK3.Avatars.Components;
using VRC.SDK3A.Editor;
using VRC.SDKBase.Editor;
using VRC.SDKBase.Editor.Api;

// Single-variable SDK comparisons for the current, explicitly authorized MF6
// upload. All experiments reuse the existing private Avatar ID. The saved source
// scene and source Prefab are preserved; no SDK or server checks are bypassed.
public static class Mf6SecurityExperiment20261022
{
    const string Folder = "Docs/Security20261022";
    const string AssetsFolder = "Assets/MorphForge/Validation/Security20261022";
    static string StatePath => Folder + "/experiment.json";
    static JObject State() => File.Exists(StatePath) ? JObject.Parse(File.ReadAllText(StatePath)) : new JObject();
    static void Save(JObject state)
    {
        Directory.CreateDirectory(Folder);
        File.WriteAllText(StatePath, state.ToString());
        if (state["variant"] != null) File.WriteAllText(Folder + "/experiment-" + (string)state["variant"] + ".json", state.ToString());
    }
    static string Hash(string path)
    {
        using (var sha = SHA256.Create()) using (var input = File.OpenRead(path))
            return BitConverter.ToString(sha.ComputeHash(input)).Replace("-", "").ToLowerInvariant();
    }
    static void Progress(string phase, string message, float? fraction = null)
    {
        var state = State(); state["phase"] = phase; state["message"] = message;
        state["progress"] = fraction; state["updatedUtc"] = DateTime.UtcNow.ToString("O"); Save(state);
    }
    static GameObject Avatar(Scene scene)
    {
        var avatar = scene.GetRootGameObjects().SelectMany(g => g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single().gameObject;
        if (avatar.name != "MorphForge_MF6_Avatar" || avatar.GetComponent<PipelineManager>().blueprintId != Mf6AvatarBuild.AvatarId)
            throw new Exception("Unexpected Avatar identity.");
        return avatar;
    }
    static void Idle()
    {
        if (Application.dataPath.Replace('\\', '/') != "D:/Projects/VRChatProjects2/MorphForge/Assets" ||
            EditorApplication.isPlaying || EditorApplication.isCompiling) throw new Exception("Expected idle MF6 project.");
        if ((string)State()["status"] == "running") throw new Exception("An experiment is already running.");
    }
    public static string Prepare(string variant)
    {
        Idle();
        if (variant != "no-motion" && variant != "no-text-controller" && variant != "no-preview" &&
            variant != "text-parameters-only" && variant != "full-fixed")
            throw new Exception("Unsupported comparison.");
        var source = SceneManager.GetActiveScene();
        if (source.path != Mf6AvatarBuild.ScenePath || source.isDirty) throw new Exception("Clean saved source required.");
        for (int i = 0; i < SceneManager.sceneCount; i++)
            if (SceneManager.GetSceneAt(i).isDirty) throw new Exception("Unsaved scene preserved.");
        var avatar = Avatar(source);
        if(variant=="full-fixed") {
            var rebuilt=JObject.Parse(File.ReadAllText(Folder+"/cache-rebuild.json"));
            if((bool?)rebuilt["success"]!=true||(int)rebuilt["merged"]["parameters"]>2048||(int)rebuilt["merged"]["syncedBits"]!=254)
                throw new Exception("Packed-cache native merge proof required.");
            foreach(var proof in new[]{"text-receiver-play","native-motion-suite","native-motion-surfaces"}) {
                string file="Docs/TextAnimation/"+proof+".json";
                if((bool?)JObject.Parse(File.ReadAllText(file))["success"]!=true||File.GetLastWriteTimeUtc(file)<DateTime.Parse((string)rebuilt["timeUtc"]).ToUniversalTime())
                    throw new Exception("Current native proof missing: "+proof);
            }
        }
        string controller = variant == "no-text-controller" || variant == "text-parameters-only" ? "/Text.controller" :
            variant == "no-preview" ? "/Preview.controller" : "/Motion.controller";
        var merges = avatar.GetComponentsInChildren<ModularAvatarMergeAnimator>(true)
            .Where(m => AssetDatabase.GetAssetPath(m.animator).EndsWith(controller)).ToArray();
        if (merges.Length != 1) throw new Exception("Expected exactly one comparison controller.");
        CatalogVariant.EnsureFolder(AssetsFolder);
        string path = AssetsFolder + "/" + variant + ".unity";
        if (File.Exists(path)) throw new Exception("Comparison scene already exists; preserve evidence.");
        var state = new JObject { ["variant"] = variant, ["status"] = "preparing",
            ["sourceSceneHash"] = Hash(source.path),
            ["sourcePrefabHash"] = AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString(),
            ["scene"] = path, ["avatarId"] = Mf6AvatarBuild.AvatarId,
            ["createdUtc"] = DateTime.UtcNow.ToString("O") };
        if (!EditorSceneManager.SaveScene(source, path, true)) throw new Exception("Could not create isolated scene.");
        var scene = EditorSceneManager.OpenScene(path, OpenSceneMode.Single);
        avatar = Avatar(scene);
        var merge = avatar.GetComponentsInChildren<ModularAvatarMergeAnimator>(true)
            .Single(m => AssetDatabase.GetAssetPath(m.animator).EndsWith(controller));
        if(variant=="full-fixed") { /* Preserve all modules in the final validated upload. */ }
        else if (variant == "text-parameters-only")
        {
            var original = (AnimatorController)merge.animator;
            var replacement = AnimatorController.CreateAnimatorControllerAtPath(AssetsFolder + "/" + variant + ".controller");
            replacement.layers = new AnimatorControllerLayer[0];
            replacement.parameters = original.parameters;
            EditorUtility.SetDirty(replacement); AssetDatabase.SaveAssets();
            merge.animator = replacement; EditorUtility.SetDirty(merge);
            state["textParametersRetained"] = replacement.parameters.Length;
            state["textLayersRetained"] = replacement.layers.Length;
        }
        else if (variant == "no-preview") UnityEngine.Object.DestroyImmediate(merge.gameObject);
        else UnityEngine.Object.DestroyImmediate(merge);
        EditorSceneManager.MarkSceneDirty(scene); EditorSceneManager.SaveScene(scene);
        state["status"] = "prepared"; state["comparisonSceneHash"] = Hash(path);
        state["removed"] = variant == "full-fixed" ? "No feature removed; packed local cache fix" :
            variant == "text-parameters-only" ? "Text layers/clips only; all text parameters retained" :
            variant == "no-preview" ? "Only the LocalPreview module" :
            "Only the ModularAvatarMergeAnimator referencing " + controller;
        state["remainingMotionControllers"] = avatar.GetComponentsInChildren<ModularAvatarMergeAnimator>(true)
            .Count(m => AssetDatabase.GetAssetPath(m.animator).EndsWith("/Motion.controller"));
        state["textRenderers"] = avatar.GetComponentsInChildren<Renderer>(true)
            .Count(r => r.sharedMaterials.Any(m => m && m.shader && m.shader.name.Contains("PixelText16")));
        state["previewCameras"] = avatar.GetComponentsInChildren<Camera>(true).Count(c => c.targetTexture);
        Save(state); return state.ToString();
    }
    static GameObject Target()
    {
        var state = State(); var scene = SceneManager.GetActiveScene();
        if (scene.path != (string)state["scene"] || scene.isDirty || Hash(scene.path) != (string)state["comparisonSceneHash"])
            throw new Exception("Prepared comparison scene changed.");
        if (Hash(Mf6AvatarBuild.ScenePath) != (string)state["sourceSceneHash"] ||
            AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString() != (string)state["sourcePrefabHash"])
            throw new Exception("Source changed; preserve current work.");
        return Avatar(scene);
    }
    public static async Task<string> Start()
    {
        Idle(); if ((string)State()["status"] != "prepared") throw new Exception("Prepare first.");
        var avatar = Target(); var user = APIUser.CurrentUser;
        if (user == null || !user.canPublishAvatars) throw new Exception("Existing SDK publisher login required.");
        var remote = await VRCApi.GetAvatar(Mf6AvatarBuild.AvatarId, forceRefresh: true);
        if (remote.AuthorId != user.id || remote.ReleaseStatus != "private" || remote.Name != "MorphForge MF6 16K" || remote.PendingUpload)
            throw new Exception("Existing private Avatar owner/name/state mismatch.");
        if (!VRCSdkControlPanel.TryGetBuilder(out IVRCSdkAvatarBuilderApi builder) ||
            builder.BuildState != SdkBuildState.Idle || builder.UploadState != SdkUploadState.Idle)
            throw new Exception("SDK builder unavailable or busy.");
        builder.SelectAvatar(avatar);
        var state = State(); state["status"] = "running"; state["phase"] = "queued";
        state["versionBefore"] = remote.Version; state["startedUtc"] = DateTime.UtcNow.ToString("O"); Save(state);
        EditorApplication.CallbackFunction next = null;
        next = () => { EditorApplication.update -= next; Upload(builder, avatar, remote); };
        EditorApplication.update += next; EditorApplication.QueuePlayerLoopUpdate(); return state.ToString();
    }
    static async void Upload(IVRCSdkAvatarBuilderApi builder, GameObject avatar, VRCAvatar remote)
    {
        EventHandler<string> build = (sender, message) => Progress("build", message);
        EventHandler<(string status, float percentage)> upload = (sender, value) => Progress("upload", value.status, value.percentage);
        builder.OnSdkBuildProgress += build; builder.OnSdkUploadProgress += upload;
        try
        {
            Progress("sdk-invoked", "Normal SDK update of the existing private MF6 Avatar.");
            await builder.BuildAndUpload(avatar, remote, null, CancellationToken.None);
            var latest = await VRCApi.GetAvatar(remote.ID, forceRefresh: true);
            if (latest.ID != remote.ID || latest.Name != remote.Name || latest.ReleaseStatus != "private" || latest.PendingUpload)
                throw new Exception("Unexpected post-upload Avatar state.");
            var state = State(); state["status"] = "sdk-success"; state["phase"] = "awaiting-server-check";
            state["versionAfter"] = latest.Version; state["completedUtc"] = DateTime.UtcNow.ToString("O"); Save(state);
        }
        catch (Exception error)
        {
            var state = State(); state["status"] = "failed"; state["error"] = error.ToString(); Save(state);
        }
        finally { builder.OnSdkBuildProgress -= build; builder.OnSdkUploadProgress -= upload; }
    }
    public static string Status() => State().ToString();
    public static string ContinueAuthorizedUpload()
    {
        var state = State();
        if ((string)state["status"] != "running" || (string)state["avatarId"] != Mf6AvatarBuild.AvatarId)
            throw new Exception("No authorized experiment upload is running.");
        var root = VRCSdkControlPanel.window?.rootVisualElement;
        if (root == null) throw new Exception("SDK window unavailable.");
        var modal = root.Query<VRC.SDKBase.Editor.Elements.Modal>().ToList()
            .Single(m => m.IsOpen && m.Q<Label>("modal-title")?.text == "Copyright ownership agreement");
        if (!modal.Query<Label>().ToList().Any(l => l.text == VRC.SDKBase.VRCCopyrightAgreement.AgreementText))
            throw new Exception("Unexpected SDK confirmation.");
        var button = modal.Q<Button>("modal-action-button");
        if (button == null || button.text != "OK" || !button.enabledInHierarchy) throw new Exception("SDK confirmation unavailable.");
        var click = typeof(Clickable).GetMethod("SimulateSingleClick", System.Reflection.BindingFlags.Instance |
            System.Reflection.BindingFlags.NonPublic, null, new[] { typeof(EventBase), typeof(int) }, null);
        if (click == null) throw new Exception("Unity button simulator unavailable.");
        click.Invoke(button.clickable, new object[] { null, 0 });
        return new JObject { ["confirmationDispatched"] = true, ["avatarId"] = state["avatarId"] }.ToString();
    }
    public static string Restore()
    {
        Idle(); var state = State(); var scene = SceneManager.GetActiveScene();
        if (scene.path != (string)state["scene"]) throw new Exception("Unexpected active scene.");
        if (Hash(Mf6AvatarBuild.ScenePath) != (string)state["sourceSceneHash"] ||
            AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString() != (string)state["sourcePrefabHash"])
            throw new Exception("Source changed; preserve evidence.");
        if (scene.isDirty)
        {
            string snapshot = AssetsFolder + "/" + (string)state["variant"] + "-after-sdk.unity";
            if (File.Exists(snapshot)) throw new Exception("Post-SDK snapshot already exists.");
            if (!EditorSceneManager.SaveScene(scene, snapshot, true)) throw new Exception("Could not preserve SDK state.");
            state["postSdkSnapshot"] = snapshot;
        }
        var restored = EditorSceneManager.OpenScene(Mf6AvatarBuild.ScenePath, OpenSceneMode.Single);
        state["sourceRestored"] = !restored.isDirty; Save(state); return state.ToString();
    }
}
