using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Threading;
using System.Threading.Tasks;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.Core;
using VRC.SDK3A.Editor;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase.Editor;
using VRC.SDKBase.Editor.Api;

public static class AvatarDiagnosticPublish
{
    private const string SourceScenePath =
        "Assets/MorphForge/Scenes/MorphForge_Stage3.unity";
    private const string ProbeScenePath =
        "Assets/MorphForge/Diagnostics/Security4096/" +
        "MorphForge_Stage3_Avatar_4096Probe.unity";
    private const string ProbePrefabPath =
        "Assets/MorphForge/Diagnostics/Security4096/" +
        "MorphForge_Stage3_Avatar_4096Probe.prefab";
    private const string ProbeAvatarName =
        "MorphForge_Stage3_Avatar_4096Probe";
    private const string ExpectedUserId =
        "usr_c0dd064e-ac42-47a7-a17e-da6540382c55";
    private const string AvatarName =
        "MorphForge Stage3 4096 Probe";
    private const string TargetProbeAvatarId =
        "avtr_df2ff222-b6ff-47fb-80be-e060868596a2";
    private const string CatalogMaterialPath =
        "Assets/MorphForge/MaterialsCatalog/ParticleBoxCatalog.mat";
    private const string LegacyMaterialPath =
        "Assets/MorphForge/Stage2/Box.mat";
    private const string DiagnosticGridMaterialPath =
        "Assets/MorphForge/Diagnostics/CatalogSecurity/CatalogGrid.mat";
    private const string DiagnosticGridShaderPath =
        "Assets/MorphForge/Shaders/ParticleBoxCatalogMetadataGrid.shader";
    private const string DiagnosticGridShaderName =
        "MorphForge/ParticleBoxCatalogMetadataGrid";
    private const string DiagnosticGridCapacityOperationPrefix =
        "diagnostic-grid-capacity-";

    private static readonly object StateLock = new object();
    private static readonly object OperationLock = new object();
    private static IVRCSdkAvatarBuilderApi ActiveBuilder;
    private static CancellationTokenSource ActiveCancellation;
    private static string ActiveOperationId;

    private static string TempRoot
    {
        get
        {
            var path = Path.Combine(
                Path.GetTempPath(),
                "morphforge",
                "security4096");
            Directory.CreateDirectory(path);
            return path;
        }
    }

    private static string UploadStatePath =>
        Path.Combine(TempRoot, "upload4096-operation.json");

    private static string ThumbnailPath =>
        Path.Combine(TempRoot, "probe-thumbnail.png");

    public static async Task<string> Inspect()
    {
        var sceneStateBefore = SceneSnapshot();
        var account = await CurrentAccount();
        var builder = GetBuilder(false);
        var probe = FindProbeAvatar(false);
        var source = FindSourceAvatar(false);
        var research = PublicApiResearch();
        var result = new Dictionary<string, object>
        {
            ["success"] = true,
            ["generatedAtUtc"] = DateTime.UtcNow.ToString("O"),
            ["scope"] =
                "Read-only account/API inspection; no build, upload, " +
                "scene save, Play Mode, VRChat launch, or OSC.",
            ["account"] = account,
            ["accountMatchesExpected"] =
                account.ContainsKey("id") &&
                string.Equals(
                    Convert.ToString(account["id"]),
                    ExpectedUserId,
                    StringComparison.Ordinal),
            ["accountIdPrintedOrReadFromCredentials"] = false,
            ["builder"] = new
            {
                available = builder != null,
                type = builder?.GetType().FullName,
                buildState = builder == null
                    ? null
                    : builder.BuildState.ToString(),
                uploadState = builder == null
                    ? null
                    : builder.UploadState.ToString(),
                selectedAvatar = builder?.SelectedAvatar == null
                    ? null
                    : HierarchyPath(builder.SelectedAvatar.transform),
                sdkPanelWindowType =
                    VRCSdkControlPanel.window?.GetType().FullName,
                sdkPanelWindowTitle =
                    VRCSdkControlPanel.window?.titleContent?.text
            },
            ["sourceScene"] = source == null
                ? null
                : SceneRecord(source.Value.scene),
            ["probeScene"] = probe == null
                ? null
                : SceneRecord(probe.Value.scene),
            ["probePrefab"] = AssetRecord(ProbePrefabPath),
            ["probeAvatar"] = probe == null
                ? null
                : AvatarRecord(probe.Value.avatar),
            ["thumbnail"] = new
            {
                path = ThumbnailPath,
                exists = File.Exists(ThumbnailPath),
                bytes = File.Exists(ThumbnailPath)
                    ? new FileInfo(ThumbnailPath).Length
                    : 0,
                sha256 = File.Exists(ThumbnailPath)
                    ? Sha256File(ThumbnailPath)
                    : null
            },
            ["activeOperation"] = ReadOperationOrNull(),
            ["publicApiResearch"] = research,
            ["potentialModals"] = new[]
            {
                "Copyright ownership agreement: first upload of a new " +
                "content ID may open an SDK modal. It requires a human OK; " +
                "this tool does not auto-agree.",
                "Avatar publishing permission: if canPublishAvatars is false, " +
                "the SDK shows its publishing-permissions dialog and blocks.",
                "Do not switch/close scenes during the operation; the source " +
                "Stage3 scene is dirty and must remain loaded.",
                "The web login/2FA pages are separate from the Unity SDK " +
                "session and are handled outside this tool."
            },
            ["sceneStateUnchangedDuringInspection"] =
                JsonConvert.SerializeObject(sceneStateBefore) ==
                JsonConvert.SerializeObject(SceneSnapshot())
        };
        return JsonConvert.SerializeObject(result, Formatting.Indented);
    }

    public static async Task<string> RecoverAccount()
    {
        if (APIUser.CurrentUser == null && APIUser.IsLoggedIn)
        {
            APIUser.InitialFetchCurrentUser(_ => { }, null);
        }
        else if (APIUser.CurrentUser == null)
        {
            var loaded = ApiCredentials.Load();
            if (loaded)
                APIUser.InitialFetchCurrentUser(_ => { }, null);
        }
        var deadline = DateTime.UtcNow.AddSeconds(15);
        while (APIUser.CurrentUser == null &&
               DateTime.UtcNow < deadline)
            await Task.Delay(250);
        var account = await CurrentAccount();
        return JsonConvert.SerializeObject(new
        {
            success = account["id"] != null,
            generatedAtUtc = DateTime.UtcNow.ToString("O"),
            account,
            matchesExpected =
                string.Equals(
                    Convert.ToString(account["id"]),
                    ExpectedUserId,
                    StringComparison.Ordinal),
            credentialsLoaded = ApiCredentials.IsLoaded(),
            credentialValuesReadOrPrinted = false,
            builder = GetBuilder(false) == null
                ? null
                : new
                {
                    build = GetBuilder(false).BuildState.ToString(),
                    upload = GetBuilder(false).UploadState.ToString()
                }
        }, Formatting.Indented);
    }

    public static string GenerateThumbnail()
    {
        RequireIdle();
        var probe = RequireProbeAvatar();
        var scene = probe.scene;
        var previousActive = SceneManager.GetActiveScene();
        var sceneHashBefore = Sha256File(
            AbsoluteAssetPath(ProbeScenePath));
        SceneManager.SetActiveScene(scene);
        var camera = scene.GetRootGameObjects()
            .SelectMany(root => root.GetComponentsInChildren<Camera>(true))
            .FirstOrDefault(item =>
                item.CompareTag("MainCamera") ||
                item.name == "Probe Camera");
        if (camera == null)
            throw new InvalidOperationException(
                "Probe scene has no Probe Camera/MainCamera.");

        var previousTarget = camera.targetTexture;
        var previousActiveTexture = RenderTexture.active;
        var target = RenderTexture.GetTemporary(
            1024,
            1024,
            24,
            RenderTextureFormat.ARGB32,
            RenderTextureReadWrite.sRGB);
        Texture2D image = null;
        try
        {
            camera.targetTexture = target;
            camera.Render();
            RenderTexture.active = target;
            image = new Texture2D(
                1024,
                1024,
                TextureFormat.RGB24,
                false,
                false);
            image.ReadPixels(new Rect(0, 0, 1024, 1024), 0, 0);
            image.Apply();
            var bytes = image.EncodeToPNG();
            File.WriteAllBytes(ThumbnailPath, bytes);
        }
        finally
        {
            camera.targetTexture = previousTarget;
            RenderTexture.active = previousActiveTexture;
            RenderTexture.ReleaseTemporary(target);
            if (image != null)
                UnityEngine.Object.DestroyImmediate(image);
        }
        SceneManager.SetActiveScene(scene);
        Selection.activeGameObject = probe.avatar;

        var sceneHashAfter = Sha256File(
            AbsoluteAssetPath(ProbeScenePath));
        if (!string.Equals(
                sceneHashBefore,
                sceneHashAfter,
                StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException(
                "Thumbnail rendering changed the saved probe scene.");
        if (previousActive.IsValid() &&
            previousActive != scene &&
            previousActive != SceneManager.GetSceneByPath(SourceScenePath))
            SceneManager.SetActiveScene(previousActive);

        return JsonConvert.SerializeObject(new
        {
            success = true,
            path = ThumbnailPath,
            bytes = new FileInfo(ThumbnailPath).Length,
            sha256 = Sha256File(ThumbnailPath),
            width = 1024,
            height = 1024,
            sceneSha256 = sceneHashAfter,
            sceneSavedOrModified = false,
            probeSceneActive = SceneManager.GetActiveScene() == scene,
            sourceStage3StillLoaded = FindSourceAvatar(false) != null,
            sourceStage3Dirty = SceneManager
                .GetSceneByPath(SourceScenePath).isDirty
        }, Formatting.Indented);
    }

    public static string StartLocalBuild()
    {
        RequireIdle();
        var builder = RequireBuilder(true);
        var probe = RequireProbeAvatar();
        var operationId = NewOperationId("build");
        lock (OperationLock)
        {
            if (ActiveBuilder != null)
                throw new InvalidOperationException(
                    "Another SDK operation is active in this Editor session.");
            ActiveBuilder = builder;
            ActiveCancellation = new CancellationTokenSource();
            ActiveOperationId = operationId;
        }

        var initialState = BaseState(
            operationId,
            "local-build",
            builder,
            probe.avatar);
        initialState["status"] = "running";
        initialState["phase"] = "starting";
        initialState["networkUploadStarted"] = false;
        initialState["message"] =
            "Starting SDK IVRCSdkAvatarBuilderApi.Build for the 4096 Probe.";
        WriteState(initialState);
        Selection.activeGameObject = probe.avatar;
        builder.SelectAvatar(probe.avatar);
        EditorApplication.delayCall += () =>
            RunLocalBuildAsync(builder, probe, operationId);
        return JsonConvert.SerializeObject(new
        {
            success = true,
            accepted = true,
            operationId,
            stateFile = UploadStatePath,
            pollWith = "AvatarDiagnosticPublish.GetOperation",
            phase = "starting",
            networkUploadStarted = false,
            sdkStartsAfterHandleViaDelayCall = true
        }, Formatting.Indented);
    }

    public static async Task<string> StartUpload()
    {
        RequireIdle();
        if (!File.Exists(ThumbnailPath))
            throw new InvalidOperationException(
                "Thumbnail is missing; run GenerateThumbnail first.");
        var account = await CurrentAccount();
        if (!string.Equals(
                Convert.ToString(account["id"]),
                ExpectedUserId,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Unity SDK account does not match expected VRChat user.");
        if (!Convert.ToBoolean(account["canPublishAvatars"]))
            throw new InvalidOperationException(
                "Unity SDK account cannot publish avatars.");
        var probe = RequireProbeAvatar();
        var pipeline = probe.avatar.GetComponent<PipelineManager>();
        if (pipeline == null)
            throw new InvalidOperationException(
                "Probe Avatar has no PipelineManager.");
        if (!string.IsNullOrWhiteSpace(pipeline.blueprintId))
            throw new InvalidOperationException(
                "Probe already has a blueprintId. This first upload must " +
                "create a separate private Avatar and must not overwrite an ID.");
        var builder = RequireBuilder(true);
        var operationId = NewOperationId("upload");
        lock (OperationLock)
        {
            if (ActiveBuilder != null)
                throw new InvalidOperationException(
                    "Another SDK operation is active in this Editor session.");
            ActiveBuilder = builder;
            ActiveCancellation = new CancellationTokenSource();
            ActiveOperationId = operationId;
        }
        var state = BaseState(
            operationId,
            "build-and-upload",
            builder,
            probe.avatar);
        state["status"] = "running";
        state["phase"] = "starting";
        state["networkUploadStarted"] = true;
        state["releaseStatus"] = "private";
        state["thumbnailPath"] = ThumbnailPath;
        state["thumbnailSha256"] = Sha256File(ThumbnailPath);
        state["message"] =
            "Starting SDK BuildAndUpload. SDK reserves a new Avatar ID " +
            "because the probe blueprintId is empty.";
        WriteState(state);
        Selection.activeGameObject = probe.avatar;
        builder.SelectAvatar(probe.avatar);
        EditorApplication.delayCall += () =>
            RunUploadAsync(builder, probe, operationId);
        return JsonConvert.SerializeObject(new
        {
            success = true,
            accepted = true,
            operationId,
            stateFile = UploadStatePath,
            pollWith = "AvatarDiagnosticPublish.GetOperation",
            phase = "starting",
            networkUploadStarted = true,
            sdkStartsAfterHandleViaDelayCall = true
        }, Formatting.Indented);
    }

    public static async Task<string> StartUpdate64Catalog()
    {
        RequireIdle();
        if (!File.Exists(ThumbnailPath))
            throw new InvalidOperationException(
                "Thumbnail is missing; run GenerateThumbnail first.");
        var account = await CurrentAccount();
        if (!string.Equals(
                Convert.ToString(account["id"]),
                ExpectedUserId,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Unity SDK account does not match expected VRChat user.");
        if (!Convert.ToBoolean(account["canPublishAvatars"]))
            throw new InvalidOperationException(
                "Unity SDK account cannot publish avatars.");
        return ScheduleRestrictedUpdate(
            "capacity-64-catalog",
            64,
            64,
            false);
    }

    public static async Task<string> StartUpdate64Legacy()
    {
        RequireIdle();
        if (!File.Exists(ThumbnailPath))
            throw new InvalidOperationException(
                "Thumbnail is missing; run GenerateThumbnail first.");
        var account = await CurrentAccount();
        if (!string.Equals(
                Convert.ToString(account["id"]),
                ExpectedUserId,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Unity SDK account does not match expected VRChat user.");
        if (!Convert.ToBoolean(account["canPublishAvatars"]))
            throw new InvalidOperationException(
                "Unity SDK account cannot publish avatars.");
        return ScheduleRestrictedUpdate(
            "capacity-64-legacy",
            64,
            64,
            true);
    }

    public static async Task<string> StartUpdate64DiagnosticMaterial(
        string materialAssetPath)
    {
        RequireIdle();
        if (string.IsNullOrWhiteSpace(materialAssetPath) ||
            !materialAssetPath.StartsWith(
                "Assets/MorphForge/Diagnostics/",
                StringComparison.Ordinal) ||
            AssetDatabase.LoadAssetAtPath<Material>(
                materialAssetPath) == null)
            throw new InvalidOperationException(
                "Diagnostic material must be an existing Material under " +
                "Assets/MorphForge/Diagnostics/.");
        var account = await CurrentAccount();
        if (!string.Equals(
                Convert.ToString(account["id"]),
                ExpectedUserId,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Unity SDK account does not match expected VRChat user.");
        if (!Convert.ToBoolean(account["canPublishAvatars"]))
            throw new InvalidOperationException(
                "Unity SDK account cannot publish avatars.");
        return ScheduleRestrictedUpdate(
            "diagnostic-material-64",
            64,
            64,
            false,
            materialAssetPath);
    }

    public static async Task<string> StartUpdateDiagnosticGridCapacity(
        int capacity)
    {
        RequireIdle();

        int expectedCapacityBefore;
        switch (capacity)
        {
            case 4096:
                expectedCapacityBefore = 64;
                break;
            case 16384:
                expectedCapacityBefore = 4096;
                break;
            default:
                throw new InvalidOperationException(
                    "Diagnostic Grid capacity update only permits the ordered " +
                    "transitions 64 -> 4096 and 4096 -> 16384.");
        }

        var probe = RequireProbeAvatar();
        if (!string.Equals(
                probe.scene.path,
                ProbeScenePath,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Diagnostic Grid capacity update only permits the original " +
                "probe scene: " + ProbeScenePath + ".");
        var pipeline = probe.avatar.GetComponent<PipelineManager>();
        if (pipeline == null ||
            !string.Equals(
                pipeline.blueprintId,
                TargetProbeAvatarId,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Diagnostic Grid capacity update only permits diagnostic " +
                "Avatar " + TargetProbeAvatarId + ".");

        var store = FindStore(probe.avatar);
        var renderer = store.GetComponent<ParticleSystemRenderer>();
        if (renderer == null || renderer.sharedMaterial == null)
            throw new InvalidOperationException(
                "Probe Store has no renderer material.");
        var currentMaterialPath = AssetDatabase.GetAssetPath(
            renderer.sharedMaterial);
        if (!string.Equals(
                currentMaterialPath,
                DiagnosticGridMaterialPath,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Diagnostic capacity update requires the current Store " +
                "material to be " + DiagnosticGridMaterialPath + "; found " +
                (currentMaterialPath ?? "<null>") + ".");
        ValidateDiagnosticGridMaterial();
        if (store.main.maxParticles != expectedCapacityBefore)
            throw new InvalidOperationException(
                "Capacity " + capacity + " is only permitted after " +
                expectedCapacityBefore + "; found " +
                store.main.maxParticles + ".");
        if (!float.IsPositiveInfinity(
                store.main.startLifetime.constant))
            throw new InvalidOperationException(
                "Diagnostic Store lifetime must remain +Infinity.");

        var account = await CurrentAccount();
        if (!string.Equals(
                Convert.ToString(account["id"]),
                ExpectedUserId,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Unity SDK account does not match expected VRChat user.");
        if (!Convert.ToBoolean(account["canPublishAvatars"]))
            throw new InvalidOperationException(
                "Unity SDK account cannot publish avatars.");

        return ScheduleRestrictedUpdate(
            DiagnosticGridCapacityOperationPrefix +
            capacity.ToString(CultureInfo.InvariantCulture),
            capacity,
            expectedCapacityBefore,
            false,
            DiagnosticGridMaterialPath);
    }

    private static string ScheduleRestrictedUpdate(
        string operationKind,
        int capacity,
        int expectedCapacityBefore,
        bool useLegacyMaterialVariant,
        string diagnosticMaterialPath = null)
    {
        var probe = RequireProbeAvatar();
        var pipeline = probe.avatar.GetComponent<PipelineManager>();
        if (pipeline == null ||
            !string.Equals(
                pipeline.blueprintId,
                TargetProbeAvatarId,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Probe is not bound to the one permitted diagnostic Avatar " +
                TargetProbeAvatarId + ".");
        if (string.Equals(
                probe.scene.path,
                SourceScenePath,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Refusing to update from the original Stage3 scene.");
        var prestore = FindStore(probe.avatar);
        var renderer = prestore.GetComponent<ParticleSystemRenderer>();
        if (renderer == null || renderer.sharedMaterial == null)
            throw new InvalidOperationException(
                "Probe Store has no renderer material.");
        if (!float.IsPositiveInfinity(
                prestore.main.startLifetime.constant))
            throw new InvalidOperationException(
                "Probe Store lifetime is not +Infinity.");
        if (prestore.main.maxParticles != expectedCapacityBefore)
            throw new InvalidOperationException(
                "Expected probe capacity " + expectedCapacityBefore +
                " before " + operationKind + "; found " +
                prestore.main.maxParticles + ".");
        if (useLegacyMaterialVariant &&
            AssetDatabase.LoadAssetAtPath<Material>(
                LegacyMaterialPath) == null)
            throw new InvalidOperationException(
                "Legacy material is missing: " + LegacyMaterialPath);
        var builder = RequireBuilder(true);
        var operationId = NewOperationId(operationKind);
        lock (OperationLock)
        {
            if (ActiveBuilder != null)
                throw new InvalidOperationException(
                    "Another SDK operation is active in this Editor session.");
            ActiveBuilder = builder;
            ActiveCancellation = new CancellationTokenSource();
            ActiveOperationId = operationId;
        }
        var state = BaseState(
            operationId,
            operationKind,
            builder,
            probe.avatar);
        state["status"] = "running";
        state["phase"] = "starting";
        state["networkUploadStarted"] = true;
        state["sdkStartsAfterHandleViaDelayCall"] = true;
        state["targetAvatarId"] = TargetProbeAvatarId;
        state["targetCapacity"] = capacity;
        state["catalogOrLegacyMaterial"] =
            diagnosticMaterialPath != null
                ? "diagnostic-material"
                : useLegacyMaterialVariant
                    ? "legacy-localuv"
                    : "catalog";
        state["materialPathBefore"] =
            AssetDatabase.GetAssetPath(renderer.sharedMaterial);
        state["storeMaxParticlesBefore"] =
            prestore.main.maxParticles;
        state["storeStartLifetimeBefore"] =
            prestore.main.startLifetime.constant.ToString(
                "R",
                CultureInfo.InvariantCulture);
        state["prefabSha256BeforeUpdate"] =
            Sha256File(AbsoluteAssetPath(ProbePrefabPath));
        state["legacyMaterialSourcePath"] = LegacyMaterialPath;
        state["diagnosticMaterialPath"] = diagnosticMaterialPath;
        state["thumbnailPathForUpdate"] = null;
        state["thumbnailBehavior"] =
            "Update passes null to BuildAndUpload and preserves the " +
            "existing remote image URL.";
        state["message"] =
            "Operation handle returned. SDK update will start through " +
            "EditorApplication.delayCall after this RPC.";
        WriteState(state);
        Selection.activeGameObject = probe.avatar;
        builder.SelectAvatar(probe.avatar);
        EditorApplication.delayCall += () =>
            RunRestrictedCapacityUpdateAsync(
                builder,
                probe,
                operationId,
                operationKind,
                capacity,
                expectedCapacityBefore,
                useLegacyMaterialVariant,
                diagnosticMaterialPath);
        return JsonConvert.SerializeObject(new
        {
            success = true,
            accepted = true,
            operationId,
            stateFile = UploadStatePath,
            pollWith = "AvatarDiagnosticPublish.GetOperation",
            phase = "starting",
            networkUploadStarted = true,
            sdkStartsAfterHandleViaDelayCall = true,
            targetAvatarId = TargetProbeAvatarId,
            targetCapacity = capacity,
            sdkUploadOnExistingAvatar = true
        }, Formatting.Indented);
    }

    public static string GetOperation()
    {
        var state = ReadOperationOrNull();
        if (state == null)
            return JsonConvert.SerializeObject(new
            {
                success = false,
                error = "No operation state file exists.",
                stateFile = UploadStatePath
            }, Formatting.Indented);
        return JsonConvert.SerializeObject(state, Formatting.Indented);
    }

    public static string Cancel()
    {
        var builder = ActiveBuilder ?? GetBuilder(false);
        if (builder != null)
            builder.CancelUpload();
        ActiveCancellation?.Cancel();
        var state = ReadOperationOrNull();
        if (state != null)
        {
            state["cancelRequestedAtUtc"] =
                DateTime.UtcNow.ToString("O");
            state["cancelRequested"] = true;
            WriteState(state);
        }
        return GetOperation();
    }

    public static async Task<string> RefreshAvatarRecord()
    {
        var probe = RequireProbeAvatar();
        var pipeline = probe.avatar.GetComponent<PipelineManager>();
        if (pipeline == null || string.IsNullOrWhiteSpace(pipeline.blueprintId))
            throw new InvalidOperationException(
                "Probe has no uploaded blueprintId.");
        var remote = await VRCApi.GetAvatar(
            pipeline.blueprintId,
            forceRefresh: true);
        var assetUrl = remote.GetLatestAssetUrlForCurrentPlatform();
        var result = new Dictionary<string, object>
        {
            ["success"] = true,
            ["avatarId"] = remote.ID,
            ["name"] = remote.Name,
            ["authorId"] = remote.AuthorId,
            ["releaseStatus"] = remote.ReleaseStatus,
            ["pendingUpload"] = remote.PendingUpload,
            ["version"] = remote.Version,
            ["createdAt"] = remote.CreatedAt.ToString("O"),
            ["updatedAt"] = remote.UpdatedAt.ToString("O"),
            ["imageUrl"] = remote.ImageUrl,
            ["thumbnailImageUrl"] = remote.ThumbnailImageUrl,
            ["latestAssetUrl"] = assetUrl,
            ["unityPackages"] = remote.UnityPackages?.Select(item => new
            {
                item.ID,
                item.AssetUrl,
                item.UnityVersion,
                item.Platform,
                item.AssetVersion,
                item.Variant,
                item.CreatedAt
            }).ToArray(),
            ["analysisApiAvailability"] =
                PublicApiResearch()["analysisStatus"],
            ["analysisEndpointHint"] = AnalysisEndpointHint(assetUrl),
            ["securityStatus"] =
                "Not exposed by IVRCSdkAvatarBuilderApi or the typed VRCApi. " +
                "Read the avatar/file analysis state from the authenticated " +
                "VRChat web session or client log API requests."
        };
        return JsonConvert.SerializeObject(result, Formatting.Indented);
    }

    public static async Task<string> InspectRemoteAvatar(string avatarId)
    {
        if (string.IsNullOrWhiteSpace(avatarId))
            throw new InvalidOperationException("Avatar ID is required.");
        var raw = await VRCApi.Get<JObject>(
            "avatars/" + avatarId,
            new Dictionary<string, string>
            {
                ["diagnosticCacheBust"] =
                    DateTime.UtcNow.Ticks.ToString(CultureInfo.InvariantCulture)
            },
            forceRefresh: true);
        var typed = await VRCApi.GetAvatar(
            avatarId,
            forceRefresh: true);
        var assetUrl = typed.GetLatestAssetUrlForCurrentPlatform();
        var interesting = new Dictionary<string, object>();
        foreach (var property in raw.Properties())
        {
            var name = property.Name;
            if (name.IndexOf("asset", StringComparison.OrdinalIgnoreCase) >= 0 ||
                name.IndexOf("bundle", StringComparison.OrdinalIgnoreCase) >= 0 ||
                name.IndexOf("package", StringComparison.OrdinalIgnoreCase) >= 0 ||
                name.IndexOf("security", StringComparison.OrdinalIgnoreCase) >= 0 ||
                name.IndexOf("analysis", StringComparison.OrdinalIgnoreCase) >= 0 ||
                name.IndexOf("status", StringComparison.OrdinalIgnoreCase) >= 0 ||
                name.IndexOf("version", StringComparison.OrdinalIgnoreCase) >= 0)
            {
                interesting[name] = JTokenOrNull(property.Value);
            }
        }
        var packageSummary = typed.UnityPackages?.Select(item => new
        {
            item.ID,
            unityVersion = item.UnityVersion,
            platform = item.Platform,
            assetVersion = item.AssetVersion,
            variant = item.Variant,
            createdAt = item.CreatedAt,
            assetUrl = item.AssetUrl
        }).ToArray();
        return JsonConvert.SerializeObject(new
        {
            success = true,
            avatarId,
            name = typed.Name,
            authorId = typed.AuthorId,
            releaseStatus = typed.ReleaseStatus,
            pendingUpload = typed.PendingUpload,
            version = typed.Version,
            updatedAt = typed.UpdatedAt,
            latestWindowsAssetUrl = assetUrl,
            analysisEndpointHint = AnalysisEndpointHint(assetUrl),
            unityPackages = packageSummary,
            typedPublicFieldsWithSecurityStatus =
                new string[0],
            rawTopLevelInterestingFields = interesting,
            rawResponseHasAssetBundleStatus =
                raw.Property("assetBundleStatus") != null,
            rawResponseHasSecurityStatus =
                raw.Property("security") != null ||
                raw.Property("securityStatus") != null ||
                raw.Property("analysisStatus") != null,
            apiConclusion =
                "The public typed VRCAvatar exposes UnityPackages and asset URLs, " +
                "but no security-analysis status. The raw avatar record is " +
                "inspected only for status-like fields; if absent, the " +
                "authenticated /analysis/<fileId>/<version>/security endpoint " +
                "is still the authoritative security state."
        }, Formatting.Indented);
    }

    public static async Task<string> InspectOwnedAvatarAnalysis(string avatarId)
    {
        var typed = await VRCApi.GetAvatar(
            avatarId,
            forceRefresh: true);
        var authorMatches =
            string.Equals(
                typed.AuthorId,
                ExpectedUserId,
                StringComparison.Ordinal);
        if (!authorMatches)
        {
            return JsonConvert.SerializeObject(new
            {
                success = false,
                avatarId,
                name = typed.Name,
                authorId = typed.AuthorId,
                authorMatches,
                analysisQueried = false,
                conclusion =
                    "Refused analysis request because the Avatar is not owned " +
                    "by the expected account."
            }, Formatting.Indented);
        }
        var assetUrl = typed.GetLatestAssetUrlForCurrentPlatform();
        if (!TryAnalysisRequest(
                assetUrl,
                out var request,
                out var fileId,
                out var fileVersion))
            throw new InvalidOperationException(
                "Could not derive the Windows asset analysis path.");
        try
        {
            var raw = await VRCApi.Get<JObject>(
                request,
                new Dictionary<string, string>
                {
                    ["diagnosticCacheBust"] =
                        DateTime.UtcNow.Ticks.ToString(
                            CultureInfo.InvariantCulture)
                },
                forceRefresh: true);
            return JsonConvert.SerializeObject(new
            {
                success = true,
                avatarId,
                name = typed.Name,
                authorId = typed.AuthorId,
                authorMatches,
                releaseStatus = typed.ReleaseStatus,
                version = typed.Version,
                fileId,
                fileVersion,
                analysisEndpoint = request,
                httpStatus = "OK",
                analysisResponse = raw
            }, Formatting.Indented);
        }
        catch (ApiErrorException error)
        {
            return JsonConvert.SerializeObject(new
            {
                success = false,
                avatarId,
                name = typed.Name,
                authorId = typed.AuthorId,
                authorMatches,
                releaseStatus = typed.ReleaseStatus,
                version = typed.Version,
                fileId,
                fileVersion,
                analysisEndpoint = request,
                httpStatus = error.StatusCode.ToString(),
                apiError = error.ErrorMessage,
                analysisQueried = true
            }, Formatting.Indented);
        }
    }

    public static async Task<string> InspectAnalysisEndpoints(string avatarId)
    {
        var typed = await VRCApi.GetAvatar(
            avatarId,
            forceRefresh: true);
        if (!string.Equals(
                typed.AuthorId,
                ExpectedUserId,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Refusing analysis for an Avatar not owned by the expected account.");
        var assetUrl = typed.GetLatestAssetUrlForCurrentPlatform();
        if (!TryAnalysisRequest(
                assetUrl,
                out var securityPath,
                out var fileId,
                out var fileVersion))
            throw new InvalidOperationException(
                "Could not derive analysis file/version from asset URL.");
        var prefix = "analysis/" + fileId + "/" + fileVersion;
        var endpoints = new[]
        {
            prefix,
            prefix + "/standard",
            securityPath
        };
        var results = new List<object>();
        foreach (var endpoint in endpoints)
        {
            try
            {
                var raw = await VRCApi.Get<JObject>(
                    endpoint,
                    new Dictionary<string, string>
                    {
                        ["diagnosticCacheBust"] =
                            DateTime.UtcNow.Ticks.ToString(
                                CultureInfo.InvariantCulture)
                    },
                    forceRefresh: true);
                results.Add(new
                {
                    endpoint,
                    httpStatus = "OK",
                    response = raw
                });
            }
            catch (ApiErrorException error)
            {
                results.Add(new
                {
                    endpoint,
                    httpStatus = error.StatusCode.ToString(),
                    apiError = error.ErrorMessage
                });
            }
        }
        return JsonConvert.SerializeObject(new
        {
            success = true,
            avatarId,
            name = typed.Name,
            authorId = typed.AuthorId,
            releaseStatus = typed.ReleaseStatus,
            pendingUpload = typed.PendingUpload,
            version = typed.Version,
            fileId,
            fileVersion,
            results
        }, Formatting.Indented);
    }

    public static async Task<string> InspectFileRecord(string fileId)
    {
        var file = await VRCApi.Get<VRCFile>(
            "file/" + fileId,
            new Dictionary<string, string>
            {
                ["diagnosticCacheBust"] =
                    DateTime.UtcNow.Ticks.ToString(
                        CultureInfo.InvariantCulture)
            },
            forceRefresh: true);
        return JsonConvert.SerializeObject(new
        {
            success = true,
            fileId = file.ID,
            ownerId = file.OwnerId,
            name = file.Name,
            extension = file.Extension,
            versions = file.Versions?.Select(version => new
            {
                version = version.Version,
                status = version.Status,
                createdAt = version.CreatedAt,
                deleted = version.Deleted,
                fileStatus = version.File?.Status,
                fileCategory = version.File?.Category,
                fileMd5 = version.File?.MD5,
                fileBytes = version.File?.SizeInBytes,
                signatureStatus = version.Signature?.Status,
                signatureMd5 = version.Signature?.MD5,
                signingBytes = version.Signature?.SizeInBytes
            }).ToArray(),
            note = "Signed URLs, cookies, upload IDs, and CDN URLs are intentionally omitted."
        }, Formatting.Indented);
    }

    public static async Task<string> InspectAnalysisStatus(string avatarId)
    {
        var typed = await VRCApi.GetAvatar(
            avatarId,
            forceRefresh: true);
        var assetUrl = typed.GetLatestAssetUrlForCurrentPlatform();
        if (!TryAnalysisRequest(
                assetUrl,
                out var analysisRequest,
                out var fileId,
                out var fileVersion))
            throw new InvalidOperationException(
                "Could not derive analysis file ID/version from the asset URL.");
        try
        {
            var raw = await VRCApi.Get<JObject>(
                analysisRequest,
                new Dictionary<string, string>
                {
                    ["diagnosticCacheBust"] =
                        DateTime.UtcNow.Ticks.ToString(
                            CultureInfo.InvariantCulture)
                },
                forceRefresh: true);
            return JsonConvert.SerializeObject(new
            {
                success = true,
                avatarId,
                fileId,
                fileVersion,
                requestPath = analysisRequest,
                response = raw,
                conclusion =
                    "The authenticated Unity SDK VRCApi session successfully " +
                    "read the server-side security analysis response. This is " +
                    "a generic public VRCApi.Get<T> call, not a typed SDK helper."
            }, Formatting.Indented);
        }
        catch (ApiErrorException error)
        {
            return JsonConvert.SerializeObject(new
            {
                success = false,
                avatarId,
                fileId,
                fileVersion,
                requestPath = analysisRequest,
                errorType = error.GetType().FullName,
                httpStatusCode = error.StatusCode.ToString(),
                error = error.ErrorMessage ?? error.Message,
                conclusion =
                    "The Unity SDK session could not read the server analysis " +
                    "response through generic VRCApi.Get<T>. The typed avatar " +
                    "record is still available, but the security state may " +
                    "require the web session or a later server state."
            }, Formatting.Indented);
        }
        catch (Exception error)
        {
            return JsonConvert.SerializeObject(new
            {
                success = false,
                avatarId,
                fileId,
                fileVersion,
                requestPath = analysisRequest,
                errorType = error.GetType().FullName,
                error = error.Message,
                innerError = error.InnerException?.Message,
                conclusion =
                    "The Unity SDK session could not read the server analysis " +
                    "response through generic VRCApi.Get<T>."
            }, Formatting.Indented);
        }
    }

    private static async void RunLocalBuildAsync(
        IVRCSdkAvatarBuilderApi builder,
        Probe probe,
        string operationId)
    {
        var progress = OperationProgress.Start(builder, operationId);
        try
        {
            progress.Update("build", "Invoking public SDK Build API.", null);
            var bundlePath = await builder.Build(probe.avatar);
            var state = ReadOperationOrNull() ??
                        BaseState(operationId, "local-build", builder, probe.avatar);
            state["status"] = "success";
            state["phase"] = "complete";
            state["completedAtUtc"] = DateTime.UtcNow.ToString("O");
            state["bundlePath"] = bundlePath;
            state["bundleExists"] =
                !string.IsNullOrWhiteSpace(bundlePath) && File.Exists(bundlePath);
            state["bundleBytes"] = state["bundleExists"].Equals(true)
                ? new FileInfo(bundlePath).Length
                : 0;
            state["bundleSha256"] = state["bundleExists"].Equals(true)
                ? Sha256File(bundlePath)
                : null;
            state["buildSignature"] =
                "Public Build API returns only the bundle path. The " +
                "cryptographic signature is generated internally later by " +
                "the upload API and is not exposed via the public event.";
            AddSceneBoundary(state, probe);
            WriteState(state);
        }
        catch (Exception error)
        {
            WriteFailure(
                operationId,
                "local-build",
                builder,
                probe.avatar,
                error);
        }
        finally
        {
            progress.Dispose();
            EndOperation(operationId);
        }
    }

    private static async void RunUploadAsync(
        IVRCSdkAvatarBuilderApi builder,
        Probe probe,
        string operationId)
    {
        var progress = OperationProgress.Start(builder, operationId);
        try
        {
            var content = new VRCAvatar
            {
                Name = AvatarName,
                Description =
                    "Security differential probe: Store particle capacity " +
                    "4096, +Infinity lifetime, otherwise cloned from the " +
                    "current Stage3 authoring state.",
                Tags = new List<string>(),
                ReleaseStatus = "private"
            };
            progress.Update(
                "upload",
                "Calling public SDK BuildAndUpload. The SDK will reserve a " +
                "new Avatar ID, build, validate, upload, and query analysis.",
                null);
            await builder.BuildAndUpload(
                probe.avatar,
                content,
                ThumbnailPath,
                ActiveCancellation.Token);

            var pipeline = probe.avatar.GetComponent<PipelineManager>();
            var avatarId = pipeline?.blueprintId;
            if (string.IsNullOrWhiteSpace(avatarId))
                throw new InvalidOperationException(
                    "BuildAndUpload succeeded but the probe has no new Avatar ID.");

            // This is the only permitted scene write: persist the identity
            // assigned to the independent test scene.
            var savedScene = false;
            if (probe.scene.isDirty)
                savedScene = EditorSceneManager.SaveScene(
                    probe.scene,
                    ProbeScenePath);

            var remote = await VRCApi.GetAvatar(
                avatarId,
                forceRefresh: true);
            var state = ReadOperationOrNull() ??
                        BaseState(
                            operationId,
                            "build-and-upload",
                            builder,
                            probe.avatar);
            state["status"] = "success";
            state["phase"] = "complete";
            state["completedAtUtc"] = DateTime.UtcNow.ToString("O");
            state["avatarId"] = avatarId;
            state["avatarName"] = remote.Name;
            state["authorId"] = remote.AuthorId;
            state["releaseStatus"] = remote.ReleaseStatus;
            state["pendingUpload"] = remote.PendingUpload;
            state["imageUrl"] = remote.ImageUrl;
            state["latestAssetUrl"] =
                remote.GetLatestAssetUrlForCurrentPlatform();
            state["analysisEndpointHint"] =
                AnalysisEndpointHint(state["latestAssetUrl"] as string);
            state["probeSceneIdentitySaved"] = savedScene;
            state["probeSceneSha256"] = Sha256File(
                AbsoluteAssetPath(ProbeScenePath));
            state["originalStage3SavedOrClosed"] = false;
            AddSceneBoundary(state, probe);
            WriteState(state);
        }
        catch (Exception error)
        {
            var state = WriteFailure(
                operationId,
                "build-and-upload",
                builder,
                probe.avatar,
                error);
            var pipeline = probe.avatar.GetComponent<PipelineManager>();
            if (pipeline != null &&
                !string.IsNullOrWhiteSpace(pipeline.blueprintId) &&
                probe.scene.isDirty)
                state["probeSceneIdentitySaved"] =
                    EditorSceneManager.SaveScene(
                        probe.scene,
                        ProbeScenePath);
            else
                state["probeSceneIdentitySaved"] = false;
            WriteState(state);
        }
        finally
        {
            progress.Dispose();
            EndOperation(operationId);
        }
    }

    private static async void RunRestrictedCapacityUpdateAsync(
        IVRCSdkAvatarBuilderApi builder,
        Probe probe,
        string operationId,
        string operationKind,
        int capacity,
        int expectedCapacityBefore,
        bool useLegacyMaterialVariant,
        string diagnosticMaterialPath)
    {
        var progress = OperationProgress.Start(builder, operationId);
        try
        {
            var pipeline = probe.avatar.GetComponent<PipelineManager>();
            if (pipeline == null ||
                !string.Equals(
                    pipeline.blueprintId,
                    TargetProbeAvatarId,
                    StringComparison.Ordinal))
                throw new InvalidOperationException(
                    "Probe target changed before delayed SDK execution.");
            var remote = await VRCApi.GetAvatar(
                TargetProbeAvatarId,
                forceRefresh: true);
            if (!string.Equals(
                    remote.AuthorId,
                    ExpectedUserId,
                    StringComparison.Ordinal))
                throw new InvalidOperationException(
                    "Probe Avatar owner does not match the expected account.");
            if (!string.Equals(
                    remote.ReleaseStatus,
                    "private",
                    StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException(
                    "Probe Avatar is not private.");

            var store = FindStore(probe.avatar);
            var renderer = store.GetComponent<ParticleSystemRenderer>();
            if (renderer == null)
                throw new InvalidOperationException(
                    "Probe Store has no ParticleSystemRenderer.");
            if (!float.IsPositiveInfinity(
                    store.main.startLifetime.constant))
                throw new InvalidOperationException(
                    "Probe Store lifetime must remain +Infinity.");
            if (store.main.maxParticles != expectedCapacityBefore)
                throw new InvalidOperationException(
                    "Probe capacity changed before delayed SDK execution. " +
                    "Expected " + expectedCapacityBefore + ", found " +
                    store.main.maxParticles + ".");

            string materialPath;
            if (diagnosticMaterialPath != null)
            {
                if (operationKind.StartsWith(
                        DiagnosticGridCapacityOperationPrefix,
                        StringComparison.Ordinal))
                {
                    if (!string.Equals(
                            diagnosticMaterialPath,
                            DiagnosticGridMaterialPath,
                            StringComparison.Ordinal))
                        throw new InvalidOperationException(
                            "Diagnostic Grid capacity operation changed its " +
                            "material path.");
                    ValidateDiagnosticGridMaterial();
                }
                var diagnosticMaterial =
                    AssetDatabase.LoadAssetAtPath<Material>(
                        diagnosticMaterialPath);
                if (diagnosticMaterial == null)
                    throw new InvalidOperationException(
                        "Diagnostic material disappeared before SDK update: " +
                        diagnosticMaterialPath);
                renderer.sharedMaterial = diagnosticMaterial;
                PrefabUtility.RecordPrefabInstancePropertyModifications(
                    renderer);
                materialPath = diagnosticMaterialPath;
            }
            else if (useLegacyMaterialVariant)
            {
                materialPath = EnsureLegacyMaterialVariant();
                var legacy = AssetDatabase.LoadAssetAtPath<Material>(
                    materialPath);
                renderer.sharedMaterial = legacy;
                PrefabUtility.RecordPrefabInstancePropertyModifications(
                    renderer);
            }
            else
            {
                materialPath = AssetDatabase.GetAssetPath(
                    renderer.sharedMaterial);
                if (!string.Equals(
                        materialPath,
                        CatalogMaterialPath,
                        StringComparison.Ordinal))
                    throw new InvalidOperationException(
                        "First capacity update requires the unchanged " +
                        "Catalog material, found: " +
                        (materialPath ?? "<null>") + ".");
            }

            var serialized = new SerializedObject(store);
            var maxParticles = serialized.FindProperty(
                "InitialModule.maxNumParticles");
            if (maxParticles == null)
                throw new InvalidOperationException(
                    "Could not find serialized Store.maxParticles.");
            maxParticles.intValue = capacity;
            serialized.ApplyModifiedPropertiesWithoutUndo();
            PrefabUtility.RecordPrefabInstancePropertyModifications(store);
            SceneManager.SetActiveScene(probe.scene);
            EditorSceneManager.MarkSceneDirty(probe.scene);
            if (FindStore(probe.avatar).main.maxParticles != capacity)
                throw new InvalidOperationException(
                    "Store capacity mutation did not take effect.");

            var state = ReadOperationOrNull() ??
                        BaseState(
                            operationId,
                            operationKind,
                            builder,
                            probe.avatar);
            state["phase"] = "upload";
            state["materialPathAfter"] = materialPath;
            state["storeMaxParticlesAfter"] = capacity;
            state["storeStartLifetimeAfter"] =
                store.main.startLifetime.constant.ToString(
                    "R",
                    CultureInfo.InvariantCulture);
            state["prefabSha256BeforeSdkUpdate"] =
                Sha256File(AbsoluteAssetPath(ProbePrefabPath));
            state["message"] =
                "Calling public SDK BuildAndUpload on existing private " +
                "diagnostic Avatar " + TargetProbeAvatarId + ".";
            WriteState(state);
            progress.Update(
                "upload",
                "Updating the existing private diagnostic Avatar with " +
                "capacity " + capacity + " and " +
                (diagnosticMaterialPath != null
                    ? "diagnostic material " + diagnosticMaterialPath
                    : useLegacyMaterialVariant
                        ? "legacy LocalUV"
                        : "Catalog") +
                " renderer.",
                null);

            var content = remote;
            await builder.BuildAndUpload(
                probe.avatar,
                content,
                null,
                ActiveCancellation.Token);

            var savedScene = false;
            if (probe.scene.isDirty)
                savedScene = EditorSceneManager.SaveScene(
                    probe.scene,
                    ProbeScenePath);
            var updated = await VRCApi.GetAvatar(
                TargetProbeAvatarId,
                forceRefresh: true);
            state = ReadOperationOrNull() ?? state;
            state["status"] = "success";
            state["phase"] = "complete";
            state["completedAtUtc"] = DateTime.UtcNow.ToString("O");
            state["avatarId"] = updated.ID;
            state["avatarName"] = updated.Name;
            state["authorId"] = updated.AuthorId;
            state["releaseStatus"] = updated.ReleaseStatus;
            state["pendingUpload"] = updated.PendingUpload;
            state["version"] = updated.Version;
            state["latestAssetUrl"] =
                updated.GetLatestAssetUrlForCurrentPlatform();
            state["analysisEndpointHint"] =
                AnalysisEndpointHint(state["latestAssetUrl"] as string);
            state["probeSceneIdentitySaved"] = savedScene;
            state["probeSceneSha256"] = Sha256File(
                AbsoluteAssetPath(ProbeScenePath));
            state["probePrefabSha256After"] = Sha256File(
                AbsoluteAssetPath(ProbePrefabPath));
            state["sourceSceneSavedOrClosed"] = false;
            AddSceneBoundary(state, probe);
            WriteState(state);
        }
        catch (Exception error)
        {
            var state = WriteFailure(
                operationId,
                operationKind,
                builder,
                probe.avatar,
                error);
            if (probe.scene.IsValid() &&
                probe.scene.isDirty)
                state["probeSceneSavedAfterFailure"] =
                    EditorSceneManager.SaveScene(
                        probe.scene,
                        ProbeScenePath);
            state["probePrefabSha256After"] = File.Exists(
                AbsoluteAssetPath(ProbePrefabPath))
                ? Sha256File(AbsoluteAssetPath(ProbePrefabPath))
                : null;
            WriteState(state);
        }
        finally
        {
            progress.Dispose();
            EndOperation(operationId);
        }
    }

    private static void ValidateDiagnosticGridMaterial()
    {
        var material = AssetDatabase.LoadAssetAtPath<Material>(
            DiagnosticGridMaterialPath);
        if (material == null)
            throw new InvalidOperationException(
                "Diagnostic Grid material is missing: " +
                DiagnosticGridMaterialPath + ".");
        var shader = material.shader;
        if (shader == null ||
            !string.Equals(
                AssetDatabase.GetAssetPath(shader),
                DiagnosticGridShaderPath,
                StringComparison.Ordinal) ||
            !string.Equals(
                shader.name,
                DiagnosticGridShaderName,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Diagnostic Grid material must use " +
                DiagnosticGridShaderPath + ".");
        var metadata = material.GetTexture("_CatalogMeta") as Texture2D;
        if (metadata == null ||
            metadata.width != 256 ||
            metadata.height != 64 ||
            metadata.format != TextureFormat.RGBAFloat)
            throw new InvalidOperationException(
                "Diagnostic Grid material must reference a 256x64 " +
                "RGBAFloat _CatalogMeta texture.");
        if (!material.HasProperty("_MetaWidth") ||
            !Mathf.Approximately(material.GetFloat("_MetaWidth"), 256f))
            throw new InvalidOperationException(
                "Diagnostic Grid material _MetaWidth must be 256.");
    }

    private static string EnsureLegacyMaterialVariant()
    {
        const string path =
            "Assets/MorphForge/Diagnostics/Security4096/" +
            "MorphForgeLegacyBox64.mat";
        var existing = AssetDatabase.LoadAssetAtPath<Material>(path);
        if (existing != null)
            return path;
        var source = AssetDatabase.LoadAssetAtPath<Material>(
            LegacyMaterialPath);
        if (source == null)
            throw new InvalidOperationException(
                "Missing legacy material source: " + LegacyMaterialPath);
        var copy = new Material(source)
        {
            name = "MorphForge Legacy Box 64"
        };
        AssetDatabase.CreateAsset(copy, path);
        return path;
    }

    private static Dictionary<string, object> WriteFailure(
        string operationId,
        string kind,
        IVRCSdkAvatarBuilderApi builder,
        GameObject avatar,
        Exception error)
    {
        var state = ReadOperationOrNull() ??
                    BaseState(operationId, kind, builder, avatar);
        state["status"] = "failure";
        state["phase"] = "failed";
        state["completedAtUtc"] = DateTime.UtcNow.ToString("O");
        state["errorType"] = error.GetType().FullName;
        state["error"] = error.Message;
        state["stackTrace"] = error.ToString();
        var probe = FindProbeAvatar(false);
        if (probe != null)
            AddSceneBoundary(state, probe.Value);
        WriteState(state);
        return state;
    }

    private static Dictionary<string, object> BaseState(
        string operationId,
        string kind,
        IVRCSdkAvatarBuilderApi builder,
        GameObject target)
    {
        var account = APIUser.CurrentUser;
        var scene = target.scene;
        var source = SceneManager.GetSceneByPath(SourceScenePath);
        return new Dictionary<string, object>
        {
            ["operationId"] = operationId,
            ["kind"] = kind,
            ["startedAtUtc"] = DateTime.UtcNow.ToString("O"),
            ["targetName"] = target.name,
            ["targetScene"] = scene.path,
            ["targetSceneDirty"] = scene.isDirty,
            ["targetStoreMaxParticles"] =
                target.GetComponentsInChildren<ParticleSystem>(true)
                    .Single(item => item.name == "Store")
                    .main.maxParticles,
            ["targetStoreStartLifetime"] =
                target.GetComponentsInChildren<ParticleSystem>(true)
                    .Single(item => item.name == "Store")
                    .main.startLifetime.constant.ToString(
                        "R",
                        CultureInfo.InvariantCulture),
            ["targetBlueprintId"] =
                target.GetComponent<PipelineManager>()?.blueprintId,
            ["sdkUserId"] = account?.id,
            ["sdkDisplayName"] = account?.displayName,
            ["sdkCanPublishAvatars"] = account?.canPublishAvatars,
            ["builderType"] = builder.GetType().FullName,
            ["sourceScene"] = source.IsValid()
                ? SceneRecord(source)
                : null,
            ["sourceSceneDiskSha256Before"] =
                source.IsValid()
                    ? Sha256File(AbsoluteAssetPath(SourceScenePath))
                    : null,
            ["sourceSceneDirtyBefore"] =
                source.IsValid() && source.isDirty,
            ["probePrefabSha256"] =
                Sha256File(AbsoluteAssetPath(ProbePrefabPath)),
            ["probeSceneDiskSha256Before"] =
                Sha256File(AbsoluteAssetPath(ProbeScenePath)),
            ["progressText"] = null,
            ["progressFraction"] = null,
            ["events"] = new List<object>()
        };
    }

    private static void AddSceneBoundary(
        Dictionary<string, object> state,
        Probe probe)
    {
        var source = SceneManager.GetSceneByPath(SourceScenePath);
        state["sourceScene"] = source.IsValid()
            ? SceneRecord(source)
            : null;
        state["sourceSceneDirtyAfter"] =
            source.IsValid() && source.isDirty;
        state["sourceSceneDiskSha256After"] =
            source.IsValid()
                ? Sha256File(AbsoluteAssetPath(SourceScenePath))
                : null;
        state["sourceSceneHashUnchanged"] =
            string.Equals(
                Convert.ToString(state["sourceSceneDiskSha256Before"]),
                Convert.ToString(state["sourceSceneDiskSha256After"]),
                StringComparison.OrdinalIgnoreCase);
        state["probeSceneDirtyAfter"] =
            probe.scene.IsValid() && probe.scene.isDirty;
        state["probeSceneDiskSha256After"] =
            File.Exists(AbsoluteAssetPath(ProbeScenePath))
                ? Sha256File(AbsoluteAssetPath(ProbeScenePath))
                : null;
        state["probePrefabSha256After"] =
            File.Exists(AbsoluteAssetPath(ProbePrefabPath))
                ? Sha256File(AbsoluteAssetPath(ProbePrefabPath))
                : null;
    }

    private static Task<Dictionary<string, object>> CurrentAccount()
    {
        var user = APIUser.CurrentUser;
        return Task.FromResult(new Dictionary<string, object>
        {
            ["isLoggedIn"] = APIUser.IsLoggedIn,
            ["id"] = user?.id,
            ["displayName"] = user?.displayName,
            ["canPublishAvatars"] = user?.canPublishAvatars ?? false,
            ["canPublishWorlds"] = user?.canPublishWorlds ?? false
        });
    }

    public static string Prepare64Catalog()
    {
        RequireIdle();
        var probe = RequireProbeAvatar();
        var pipeline = probe.avatar.GetComponent<PipelineManager>();
        if (pipeline == null ||
            !string.Equals(
                pipeline.blueprintId,
                TargetProbeAvatarId,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Probe is not bound to the permitted diagnostic Avatar " +
                TargetProbeAvatarId + ".");
        var store = FindStore(probe.avatar);
        var renderer = store.GetComponent<ParticleSystemRenderer>();
        var catalog = AssetDatabase.LoadAssetAtPath<Material>(
            CatalogMaterialPath);
        if (renderer == null || catalog == null)
            throw new InvalidOperationException(
                "Missing Store renderer or Catalog material.");
        var before = new
        {
            capacity = store.main.maxParticles,
            lifetime = store.main.startLifetime.constant.ToString(
                "R",
                CultureInfo.InvariantCulture),
            material = AssetDatabase.GetAssetPath(
                renderer.sharedMaterial)
        };
        renderer.sharedMaterial = catalog;
        PrefabUtility.RecordPrefabInstancePropertyModifications(renderer);
        var serialized = new SerializedObject(store);
        var capacity = serialized.FindProperty(
            "InitialModule.maxNumParticles");
        capacity.intValue = 64;
        serialized.ApplyModifiedPropertiesWithoutUndo();
        PrefabUtility.RecordPrefabInstancePropertyModifications(store);
        SceneManager.SetActiveScene(probe.scene);
        EditorSceneManager.MarkSceneDirty(probe.scene);
        if (!EditorSceneManager.SaveScene(probe.scene, ProbeScenePath))
            throw new InvalidOperationException(
                "Could not save the 64+Catalog diagnostic scene.");
        var after = new
        {
            capacity = store.main.maxParticles,
            lifetime = store.main.startLifetime.constant.ToString(
                "R",
                CultureInfo.InvariantCulture),
            material = AssetDatabase.GetAssetPath(
                renderer.sharedMaterial),
            sceneSha256 = Sha256File(
                AbsoluteAssetPath(ProbeScenePath)),
            prefabSha256 = Sha256File(
                AbsoluteAssetPath(ProbePrefabPath)),
            originalSceneSha256 = Sha256File(
                AbsoluteAssetPath(SourceScenePath))
        };
        return JsonConvert.SerializeObject(new
        {
            success =
                after.capacity == 64 &&
                after.lifetime == "Infinity" &&
                after.material == CatalogMaterialPath,
            before,
            after,
            onlyDiagnosticSceneChanged = true,
            originalAvatarId = TargetProbeAvatarId,
            originalModelAssetsModified = false
        }, Formatting.Indented);
    }

    private sealed class OperationProgress : IDisposable
    {
        private readonly IVRCSdkAvatarBuilderApi _builder;
        private readonly string _operationId;
        private bool _disposed;

        private OperationProgress(
            IVRCSdkAvatarBuilderApi builder,
            string operationId)
        {
            _builder = builder;
            _operationId = operationId;
        }

        public static OperationProgress Start(
            IVRCSdkAvatarBuilderApi builder,
            string operationId)
        {
            var progress = new OperationProgress(builder, operationId);
            builder.OnSdkBuildStart += progress.OnBuildStart;
            builder.OnSdkBuildProgress += progress.OnBuildProgress;
            builder.OnSdkBuildSuccess += progress.OnBuildSuccess;
            builder.OnSdkBuildError += progress.OnBuildError;
            builder.OnSdkBuildFinish += progress.OnBuildFinish;
            builder.OnSdkUploadStart += progress.OnUploadStart;
            builder.OnSdkUploadProgress += progress.OnUploadProgress;
            builder.OnSdkUploadSuccess += progress.OnUploadSuccess;
            builder.OnSdkUploadError += progress.OnUploadError;
            builder.OnSdkUploadFinish += progress.OnUploadFinish;
            return progress;
        }

        public void Update(string phase, string text, float? fraction)
        {
            var state = ReadOperationOrNull();
            if (state == null)
                return;
            state["phase"] = phase;
            state["message"] = text;
            state["progressText"] = text;
            state["progressFraction"] = fraction;
            state["updatedAtUtc"] = DateTime.UtcNow.ToString("O");
            AppendEvent(state, phase, text, fraction);
            WriteState(state);
        }

        private void OnBuildStart(object sender, object target) =>
            Update("build", "SDK build started.", null);

        private void OnBuildProgress(object sender, string text) =>
            Update("build", text, null);

        private void OnBuildSuccess(object sender, string path)
        {
            var state = ReadOperationOrNull();
            if (state != null)
            {
                state["bundlePath"] = path;
                state["bundleExists"] =
                    !string.IsNullOrWhiteSpace(path) && File.Exists(path);
                state["bundleBytes"] =
                    state["bundleExists"].Equals(true)
                        ? new FileInfo(path).Length
                        : 0;
                state["bundleSha256"] =
                    state["bundleExists"].Equals(true)
                        ? Sha256File(path)
                        : null;
                WriteState(state);
            }
            Update("build", "SDK build succeeded: " + path, 1f);
        }

        private void OnBuildError(object sender, string text) =>
            Update("build", "SDK build error: " + text, null);

        private void OnBuildFinish(object sender, string text) =>
            Update("build", "SDK build callback finished: " + text, null);

        private void OnUploadStart(object sender, EventArgs args) =>
            Update("upload", "SDK upload started.", 0f);

        private void OnUploadProgress(
            object sender,
            (string status, float percentage) value) =>
            Update("upload", value.status, value.percentage);

        private void OnUploadSuccess(object sender, string text) =>
            Update("upload", "SDK upload succeeded: " + text, 1f);

        private void OnUploadError(object sender, string text) =>
            Update("upload", "SDK upload error: " + text, null);

        private void OnUploadFinish(object sender, string text) =>
            Update("upload", "SDK upload callback finished: " + text, null);

        public void Dispose()
        {
            if (_disposed)
                return;
            _disposed = true;
            _builder.OnSdkBuildStart -= OnBuildStart;
            _builder.OnSdkBuildProgress -= OnBuildProgress;
            _builder.OnSdkBuildSuccess -= OnBuildSuccess;
            _builder.OnSdkBuildError -= OnBuildError;
            _builder.OnSdkBuildFinish -= OnBuildFinish;
            _builder.OnSdkUploadStart -= OnUploadStart;
            _builder.OnSdkUploadProgress -= OnUploadProgress;
            _builder.OnSdkUploadSuccess -= OnUploadSuccess;
            _builder.OnSdkUploadError -= OnUploadError;
            _builder.OnSdkUploadFinish -= OnUploadFinish;
        }

        private static void AppendEvent(
            Dictionary<string, object> state,
            string phase,
            string text,
            float? fraction)
        {
            var events = state["events"] as List<object>;
            if (events == null)
            {
                events = new List<object>();
                state["events"] = events;
            }
            events.Add(new
            {
                atUtc = DateTime.UtcNow.ToString("O"),
                phase,
                text,
                fraction
            });
            if (events.Count > 500)
                events.RemoveRange(0, events.Count - 500);
        }
    }

    private static Dictionary<string, object> ReadOperationOrNull()
    {
        try
        {
            if (!File.Exists(UploadStatePath))
                return null;
            return JsonConvert.DeserializeObject<
                Dictionary<string, object>>(File.ReadAllText(UploadStatePath));
        }
        catch (IOException)
        {
            return null;
        }
    }

    private static void WriteState(Dictionary<string, object> state)
    {
        lock (StateLock)
        {
            var json = JsonConvert.SerializeObject(
                state,
                Formatting.Indented);
            var temporary = UploadStatePath + ".tmp";
            File.WriteAllText(temporary, json);
            if (File.Exists(UploadStatePath))
                File.Delete(UploadStatePath);
            File.Move(temporary, UploadStatePath);
        }
    }

    private static void EndOperation(string operationId)
    {
        lock (OperationLock)
        {
            if (!string.Equals(
                    ActiveOperationId,
                    operationId,
                    StringComparison.Ordinal))
                return;
            ActiveBuilder = null;
            ActiveCancellation?.Dispose();
            ActiveCancellation = null;
            ActiveOperationId = null;
        }
    }

    private static string NewOperationId(string kind)
    {
        return kind + "-" +
               DateTime.UtcNow.ToString("yyyyMMddTHHmmssfffZ") +
               "-" + Guid.NewGuid().ToString("N").Substring(0, 8);
    }

    private static IVRCSdkAvatarBuilderApi RequireBuilder(bool openPanel)
    {
        var builder = GetBuilder(openPanel);
        if (builder == null)
            throw new InvalidOperationException(
                "VRChat SDK avatar builder is unavailable. Open the SDK " +
                "Control Panel and ensure the Avatars SDK loaded.");
        if (builder.BuildState != SdkBuildState.Idle ||
            builder.UploadState != SdkUploadState.Idle)
            throw new InvalidOperationException(
                "SDK builder is busy: build=" + builder.BuildState +
                ", upload=" + builder.UploadState + ".");
        return builder;
    }

    private static IVRCSdkAvatarBuilderApi GetBuilder(bool openPanel)
    {
        if (VRCSdkControlPanel.TryGetBuilder(
                out IVRCSdkAvatarBuilderApi builder))
            return builder;
        if (!openPanel)
            return null;
        var panel = EditorWindow.GetWindow<VRCSdkControlPanel>(
            false,
            "VRChat SDK",
            true);
        panel.Show();
        return VRCSdkControlPanel.TryGetBuilder(out builder)
            ? builder
            : null;
    }

    private static Probe RequireProbeAvatar()
    {
        var result = FindProbeAvatar(true);
        if (result == null)
            throw new InvalidOperationException(
                "The 4096 probe scene or Avatar is not loaded.");
        return result.Value;
    }

    private static ParticleSystem FindStore(GameObject avatar)
    {
        var stores = avatar.GetComponentsInChildren<ParticleSystem>(true)
            .Where(item => item.name == "Store")
            .ToArray();
        if (stores.Length != 1)
            throw new InvalidOperationException(
                "Expected exactly one Store ParticleSystem, found " +
                stores.Length + ".");
        return stores[0];
    }

    private static Probe? FindProbeAvatar(bool requireActive)
    {
        var scene = SceneManager.GetSceneByPath(ProbeScenePath);
        if (!scene.IsValid() || !scene.isLoaded)
            return null;
        if (requireActive && SceneManager.GetActiveScene() != scene)
            SceneManager.SetActiveScene(scene);
        var avatars = scene.GetRootGameObjects()
            .SelectMany(root =>
                root.GetComponentsInChildren<VRCAvatarDescriptor>(true))
            .ToArray();
        if (avatars.Length != 1 ||
            avatars[0].gameObject.name != ProbeAvatarName)
            return null;
        return new Probe(scene, avatars[0].gameObject);
    }

    private static AvatarSource? FindSourceAvatar(bool requireLoaded)
    {
        var scene = SceneManager.GetSceneByPath(SourceScenePath);
        if (!scene.IsValid() ||
            (requireLoaded && !scene.isLoaded))
            return null;
        var avatars = scene.GetRootGameObjects()
            .SelectMany(root =>
                root.GetComponentsInChildren<VRCAvatarDescriptor>(true))
            .ToArray();
        return avatars.Length == 1
            ? new AvatarSource(scene, avatars[0].gameObject)
            : null;
    }

    private static void RequireIdle()
    {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling ||
            EditorApplication.isUpdating)
            throw new InvalidOperationException(
                "SDK operation requires an idle Editor: playing=" +
                EditorApplication.isPlaying +
                ", compiling=" + EditorApplication.isCompiling +
                ", updating=" + EditorApplication.isUpdating + ".");
    }

    private static Dictionary<string, object> PublicApiResearch()
    {
        var builderMethods = typeof(IVRCSdkAvatarBuilderApi)
            .GetMethods()
            .Where(method =>
                method.DeclaringType == typeof(IVRCSdkAvatarBuilderApi) ||
                method.DeclaringType == typeof(IVRCSdkBuilderApi))
            .OrderBy(method => method.Name, StringComparer.Ordinal)
            .Select(MethodSignature)
            .ToArray();
        var apiMethods = typeof(VRCApi)
            .GetMethods()
            .Where(method => method.IsPublic &&
                             method.IsStatic &&
                             method.CustomAttributes.All(attribute =>
                                 attribute.AttributeType.Name !=
                                 "CompilerGeneratedAttribute"))
            .Where(method =>
                method.Name.IndexOf(
                    "Avatar",
                    StringComparison.OrdinalIgnoreCase) >= 0 ||
                method.Name == "Get")
            .OrderBy(method => method.Name, StringComparer.Ordinal)
            .ThenBy(method => method.GetParameters().Length)
            .Select(MethodSignature)
            .ToArray();
        return new Dictionary<string, object>
        {
            ["builderInterface"] = typeof(IVRCSdkAvatarBuilderApi).FullName,
            ["builderMethods"] = builderMethods,
            ["vrchatApiMethods"] = apiMethods,
            ["analysisStatus"] =
                "No public typed method returns VRChat server-side security " +
                "analysis status. Public VRCApi.GetAvatar returns the avatar " +
                "record and asset file URL; browser/client security requests " +
                "are a separate authenticated analysis API.",
            ["signatureMeaning"] =
                "The listed signatures are exact public C# method signatures. " +
                "The cryptographic bundle signature is an internal upload-file " +
                "artifact and is not returned by the public Build method."
        };
    }

    private static string MethodSignature(System.Reflection.MethodInfo method)
    {
        var parameters = string.Join(
            ", ",
            method.GetParameters().Select(parameter =>
                TypeName(parameter.ParameterType) + " " + parameter.Name +
                (parameter.HasDefaultValue ? " = default" : "")));
        return TypeName(method.ReturnType) + " " +
               method.DeclaringType.Name + "." + method.Name +
               "(" + parameters + ")";
    }

    private static string TypeName(Type type)
    {
        if (!type.IsGenericType)
            return type.Name;
        return type.Name.Split('`')[0] + "<" +
               string.Join(", ", type.GetGenericArguments().Select(TypeName)) +
               ">";
    }

    private static string AnalysisEndpointHint(string assetUrl)
    {
        if (!TryAnalysisRequest(
                assetUrl,
                out var request,
                out _,
                out _))
            return null;
        return "https://api.vrchat.cloud/api/1/" + request;
    }

    private static bool TryAnalysisRequest(
        string assetUrl,
        out string request,
        out string fileId,
        out string fileVersion)
    {
        request = null;
        fileId = null;
        fileVersion = null;
        if (string.IsNullOrWhiteSpace(assetUrl))
            return false;
        var prefix = "/file_";
        var index = assetUrl.IndexOf(prefix, StringComparison.Ordinal);
        if (index < 0)
            return false;
        var end = assetUrl.IndexOf('/', index + prefix.Length);
        if (end < 0)
            end = assetUrl.Length;
        fileId = assetUrl.Substring(index + 1, end - index - 1);
        var remainder = assetUrl.Substring(end);
        var parts = remainder.Split(
            new[] { '/' },
            StringSplitOptions.RemoveEmptyEntries);
        if (parts.Length < 2)
            return false;
        fileVersion = parts[0];
        request = "analysis/" + fileId + "/" + fileVersion + "/security";
        return true;
    }

    private static object JTokenOrNull(JToken token)
    {
        if (token == null || token.Type == JTokenType.Null)
            return null;
        if (token is JValue value)
            return value.Value;
        return token;
    }

    private static Dictionary<string, object> SceneRecord(Scene scene)
    {
        return new Dictionary<string, object>
        {
            ["path"] = scene.path,
            ["name"] = scene.name,
            ["isLoaded"] = scene.isLoaded,
            ["isDirty"] = scene.isDirty,
            ["isActive"] = SceneManager.GetActiveScene() == scene,
            ["rootCount"] = scene.rootCount
        };
    }

    private static Dictionary<string, object> AvatarRecord(GameObject avatar)
    {
        var pipeline = avatar.GetComponent<PipelineManager>();
        var store = avatar.GetComponentsInChildren<ParticleSystem>(true)
            .Single(item => item.name == "Store");
        return new Dictionary<string, object>
        {
            ["name"] = avatar.name,
            ["blueprintId"] = pipeline?.blueprintId,
            ["storeMaxParticles"] = store.main.maxParticles,
            ["storeStartLifetime"] =
                store.main.startLifetime.constant.ToString(
                    "R",
                    CultureInfo.InvariantCulture),
            ["missingScripts"] =
                avatar.GetComponentsInChildren<Transform>(true)
                    .Sum(transform =>
                        GameObjectUtility
                            .GetMonoBehavioursWithMissingScriptCount(
                                transform.gameObject))
        };
    }

    private static Dictionary<string, object> AssetRecord(string path)
    {
        var absolute = AbsoluteAssetPath(path);
        return new Dictionary<string, object>
        {
            ["path"] = path,
            ["exists"] = File.Exists(absolute),
            ["bytes"] = File.Exists(absolute)
                ? new FileInfo(absolute).Length
                : 0,
            ["sha256"] = File.Exists(absolute)
                ? Sha256File(absolute)
                : null
        };
    }

    private static object[] SceneSnapshot()
    {
        return Enumerable.Range(0, SceneManager.sceneCount)
            .Select(index => SceneManager.GetSceneAt(index))
            .Select(scene => (object)SceneRecord(scene))
            .ToArray();
    }

    private static string HierarchyPath(Transform transform)
    {
        var names = new List<string>();
        var current = transform;
        while (current != null)
        {
            names.Add(current.name);
            current = current.parent;
        }
        names.Reverse();
        return string.Join("/", names);
    }

    private static string AbsoluteAssetPath(string path)
    {
        return Path.GetFullPath(
            Path.Combine(Application.dataPath, "..", path));
    }

    private static string Sha256File(string path)
    {
        using (var stream = File.OpenRead(path))
        using (var sha = SHA256.Create())
            return string.Concat(
                sha.ComputeHash(stream)
                    .Select(value => value.ToString("x2")));
    }

    private readonly struct Probe
    {
        public readonly Scene scene;
        public readonly GameObject avatar;

        public Probe(Scene scene, GameObject avatar)
        {
            this.scene = scene;
            this.avatar = avatar;
        }
    }

    private readonly struct AvatarSource
    {
        public readonly Scene scene;
        public readonly GameObject avatar;

        public AvatarSource(Scene scene, GameObject avatar)
        {
            this.scene = scene;
            this.avatar = avatar;
        }
    }
}
