using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Threading;
using System.Threading.Tasks;
using BlackStartX.GestureManager;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using MorphForge.Editor;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.Core;
using VRC.SDK3A.Editor;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase.Editor;
using VRC.SDKBase.Editor.Api;

/// <summary>
/// Creates a private FullFT diagnostic scene and updates only the established
/// diagnostic Avatar ID. The source FullFT scene/prefab and original Avatar ID
/// remain untouched. Preparation never uploads.
/// </summary>
public static class FinalAvatarPublish
{
    private const string DiagnosticAvatarId =
        "avtr_df2ff222-b6ff-47fb-80be-e060868596a2";
    private const string ExpectedUserId =
        "usr_c0dd064e-ac42-47a7-a17e-da6540382c55";
    private const string FullFtPrefabPath =
        "Assets/MorphForge/FullFT/Avatar.prefab";
    private const string FullFtScenePath =
        "Assets/MorphForge/FullFT/MorphForge_FullFT.unity";
    private const string JointBuildReportPath =
        "Docs/FullFT/joint-build.json";
    private const string FinalBudgetReportPath =
        "Docs/FullFT/final-budget.json";
    private const string SceneBuildReportPath =
        "Docs/FullFT/scene-build.json";
    private const string MaterialPath =
        "Assets/MorphForge/MaterialsCatalog/ParticleBoxCatalog.mat";
    private const string ShaderPath =
        "Assets/MorphForge/Shaders/ParticleBoxCatalog.shader";
    private const string ShaderName =
        "MorphForge/ParticleBoxCatalog";
    private const string MigrationReportPath =
        "Docs/Security/catalog-production-migration.json";
    private const string ProductionVisualReportPath =
        "Docs/Security/catalog-production-visual/report.json";
    private const string RootNameMigrationReportPath =
        "Docs/Security/fullft-scene-name-migration.json";
    private const string ActualSceneBuildReportPath =
        "Docs/Security/fullft-actual-scene-build.json";
    private const string ProfilePath =
        "Assets/MorphForge/FullFT/FullFT_SyncProfile.asset";
    private const string Root =
        "Assets/MorphForge/Diagnostics/FinalFullFT";
    private const string DiagnosticScenePath =
        Root + "/MorphForge_FinalFullFT_Diagnostic.unity";
    private const string PreflightReportPath =
        "Docs/Security/final-fullft-preflight.json";
    private const string PreparedReportPath =
        "Docs/Security/final-fullft-prepared.json";
    private const string PreRenamePreparedReportPath =
        "Docs/Security/final-fullft-prepared-pre-root-rename.json";
    private const string CollisionFixPreparedReportPath =
        "Docs/Security/final-fullft-prepared-pre-collision-fix.json";
    private const string CollisionFixReportPath =
        "Docs/Security/fullft-collision-force-fix.json";
    private const int ExpectedCapacity = 16384;
    private const string ExpectedDiagnosticRootSuffix = "_Avatar";
    private const string FinalDisplayName =
        "MorphForge FullFT 16K";
    private const string RootNameGateName =
        "FullFTSyncProfile.avatarNamePrefix (same predicate as " +
        "FullFTBudgetUtil.IsFullFTTarget)";

    private static readonly object OperationLock = new object();
    private static IVRCSdkAvatarBuilderApi _activeBuilder;
    private static CancellationTokenSource _activeCancellation;
    private static string _activeOperationId;

    private static string StatePath => Path.Combine(
        Path.GetTempPath(),
        "morphforge",
        "final-fullft",
        "operation.json");

    public static string Inspect()
    {
        RequireIdle();
        var report = new Dictionary<string, object>();
        AddEvidence(report, true);
        AddFiles(report);
        var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(
            FullFtPrefabPath);
        if (prefab != null)
            AddPrefabAudit(report, prefab);
        var diagnosticScene = SceneManager.GetSceneByPath(
            DiagnosticScenePath);
        if (diagnosticScene.IsValid() && diagnosticScene.isLoaded)
        {
            var descriptors = SceneComponents<VRCAvatarDescriptor>(
                diagnosticScene);
            var rootName = descriptors.Length == 1
                ? descriptors[0].gameObject.name
                : null;
            var rootNameMatches = descriptors.Length == 1 &&
                                  string.Equals(
                                      rootName,
                                      GetFullFtRootName(),
                                      StringComparison.Ordinal);
            var profileSource = LoadProfileAndValidate();
            report["diagnosticScene"] = new
            {
                path = diagnosticScene.path,
                isLoaded = true,
                isDirty = diagnosticScene.isDirty,
                isActive = SceneManager.GetActiveScene() == diagnosticScene,
                avatarCount = descriptors.Length,
                rootName,
                expectedRootName = GetFullFtRootName(),
                fullFtTarget = descriptors.Length == 1 &&
                               IsFullFtTarget(
                                   descriptors[0].gameObject,
                                   profileSource),
                rootNameMatches,
                blueprintId = descriptors.Length == 1
                    ? descriptors[0].GetComponent<PipelineManager>()?.blueprintId
                    : null,
                particleCollisionForceSystems =
                    descriptors.Length == 1
                        ? ParticleCollisionForceViolations(
                            descriptors[0].gameObject)
                        : new string[0]
            };
            if (report["preflightFailures"] is List<string> liveFailures)
            {
                if (descriptors.Length != 1)
                    liveFailures.Add(
                        "Diagnostic scene must contain exactly one Avatar.");
                else
                {
                    if (!rootNameMatches)
                        liveFailures.Add(
                            "Diagnostic scene root name is not exactly " +
                            GetFullFtRootName() + ".");
                    if (!IsFullFtTarget(
                            descriptors[0].gameObject,
                            profileSource))
                        liveFailures.Add(
                            "Diagnostic scene does not satisfy the FullFT " +
                            "profile root-name predicate.");
                    var livePipeline =
                        descriptors[0].GetComponent<PipelineManager>();
                    if (livePipeline == null ||
                        !string.Equals(
                            livePipeline.blueprintId,
                            DiagnosticAvatarId,
                            StringComparison.Ordinal))
                        liveFailures.Add(
                            "Diagnostic scene Avatar is not bound to the " +
                            "permitted private diagnostic ID.");
                    if (diagnosticScene.isDirty)
                        liveFailures.Add(
                            "Diagnostic scene has unsaved changes.");
                    var collisionViolations =
                        ParticleCollisionForceViolations(
                            descriptors[0].gameObject);
                    if (collisionViolations.Length != 0)
                        liveFailures.Add(
                            "Diagnostic scene has forbidden particle " +
                            "collision force: " +
                            string.Join(", ", collisionViolations));
                }
            }
        }
        else
        {
            report["diagnosticScene"] = null;
        }
        report["activeOperation"] = ReadStateOrNull();
        report["scope"] =
            "Read-only preflight; no scene creation, asset save, build, " +
            "upload, Play Mode, VRChat launch, or OSC.";
        report["success"] =
            report["preflightFailures"] is List<string> failures &&
            failures.Count == 0;
        var json = JsonConvert.SerializeObject(
            report,
            Formatting.Indented);
        WriteReport(PreflightReportPath, json);
        return json;
    }

    /// <summary>
    /// Saves the source FullFT scene, creates an independent diagnostic copy,
    /// and binds its Avatar to the private diagnostic ID. It does not build or
    /// upload. The source scene must already be clean and saved.
    /// </summary>
    public static string PrepareDiagnosticScene()
    {
        RequireIdle();
        var evidence = CollectEvidence();
        var sourceScene = SceneManager.GetSceneByPath(FullFtScenePath);
        if (!sourceScene.IsValid() || !sourceScene.isLoaded)
            throw new InvalidOperationException(
                "The saved FullFT source scene must be open before preparing " +
                "the diagnostic copy: " + FullFtScenePath + ".");
        if (sourceScene.isDirty)
            throw new InvalidOperationException(
                "The FullFT source scene has unsaved changes. Save it first; " +
                "this entry will not save or discard user edits.");
        if (AssetDatabase.LoadAssetAtPath<SceneAsset>(DiagnosticScenePath) != null)
            throw new InvalidOperationException(
                "Diagnostic scene already exists and will not be overwritten: " +
                DiagnosticScenePath + ".");

        var sourceHashBefore = Sha256File(Absolute(FullFtScenePath));
        var prefabAudit = evidence["prefab"];
        if (prefabAudit == null)
            throw new InvalidOperationException(
                "FullFT prefab audit failed before scene creation.");

        EnsureFolder(Root);
        var temporaryPath =
            Root + "/__diagnostic_" + Guid.NewGuid().ToString("N") + ".unity";
        var copiedScene = default(Scene);
        var moved = false;
        var success = false;
        try
        {
            if (!EditorSceneManager.SaveScene(sourceScene, temporaryPath, true))
                throw new InvalidOperationException(
                    "Could not create an independent FullFT scene copy.");
            copiedScene = EditorSceneManager.OpenScene(
                temporaryPath, OpenSceneMode.Additive);
            SceneManager.SetActiveScene(copiedScene);

            var descriptors = SceneComponents<VRCAvatarDescriptor>(
                copiedScene);
            var managers = SceneComponents<GestureManager>(copiedScene);
            var receivers = SceneComponents<MorphForgePlayReceiver>(
                copiedScene);
            if (descriptors.Length != 1 ||
                managers.Length != 1 ||
                receivers.Length != 1)
                throw new InvalidOperationException(
                    "Copied FullFT scene must contain exactly one Avatar, " +
                    "GestureManager, and receiver; found " +
                    descriptors.Length + "/" + managers.Length + "/" +
                    receivers.Length + ".");
            var avatar = descriptors[0].gameObject;
            var instancePath =
                PrefabUtility.GetPrefabAssetPathOfNearestInstanceRoot(avatar);
            if (!string.Equals(
                    instancePath,
                    FullFtPrefabPath,
                    StringComparison.Ordinal))
                throw new InvalidOperationException(
                    "Copied Avatar is not an instance of " +
                    FullFtPrefabPath + "; found " +
                    (instancePath ?? "<not a prefab instance>") + ".");

            var pipeline = avatar.GetComponent<PipelineManager>();
            if (pipeline == null)
                throw new InvalidOperationException(
                    "Copied Avatar has no PipelineManager.");
            var expectedRootName = GetFullFtRootName();
            avatar.name = expectedRootName;
            EditorUtility.SetDirty(avatar);
            PrefabUtility.RecordPrefabInstancePropertyModifications(avatar);
            pipeline.blueprintId = DiagnosticAvatarId;
            EditorUtility.SetDirty(pipeline);
            PrefabUtility.RecordPrefabInstancePropertyModifications(pipeline);
            if (!ReferenceEquals(
                    managers[0].settings.favourite,
                    descriptors[0]))
                throw new InvalidOperationException(
                    "GestureManager does not reference the copied FullFT " +
                    "Avatar.");
            receivers[0].Avatar = descriptors[0];
            receivers[0].ReceivePort = 9012;
            receivers[0].SendPort = 9013;
            receivers[0].AutoStart = true;
            EditorUtility.SetDirty(receivers[0]);
            PrefabUtility.RecordPrefabInstancePropertyModifications(
                receivers[0]);
            ValidateFinalAvatar(avatar, true);

            EditorSceneManager.MarkSceneDirty(copiedScene);
            if (!EditorSceneManager.SaveScene(copiedScene))
                throw new InvalidOperationException(
                    "Could not save the independent diagnostic scene.");
            EditorSceneManager.CloseScene(copiedScene, true);
            copiedScene = default(Scene);
            var moveError = AssetDatabase.MoveAsset(
                temporaryPath,
                DiagnosticScenePath);
            if (!string.IsNullOrEmpty(moveError))
                throw new InvalidOperationException(
                    "Could not move diagnostic scene: " + moveError);
            moved = true;
            AssetDatabase.ImportAsset(
                DiagnosticScenePath,
                ImportAssetOptions.ForceUpdate);

            var created = EditorSceneManager.OpenScene(
                DiagnosticScenePath,
                OpenSceneMode.Additive);
            SceneManager.SetActiveScene(created);
            var createdAvatar = SceneComponents<VRCAvatarDescriptor>(
                created).Single().gameObject;
            ValidateFinalAvatar(createdAvatar, true);
            var createdPipeline =
                createdAvatar.GetComponent<PipelineManager>();
            if (!string.Equals(
                    createdPipeline.blueprintId,
                    DiagnosticAvatarId,
                    StringComparison.Ordinal))
                throw new InvalidOperationException(
                    "Diagnostic scene did not retain the permitted Avatar ID.");

            var sourceHashAfter = Sha256File(Absolute(FullFtScenePath));
            var sourceStillOpen = SceneManager.GetSceneByPath(
                FullFtScenePath).IsValid();
            if (!string.Equals(
                    sourceHashBefore,
                    sourceHashAfter,
                    StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException(
                    "Source FullFT scene file changed during diagnostic copy.");
            if (SceneManager.GetSceneByPath(FullFtScenePath).isDirty)
                throw new InvalidOperationException(
                    "Source FullFT scene became dirty during diagnostic copy.");
            if (!sourceStillOpen)
                throw new InvalidOperationException(
                    "Source FullFT scene was closed during diagnostic copy.");

            var result = new Dictionary<string, object>
            {
                ["success"] = true,
                ["scope"] =
                    "Independent FullFT diagnostic scene; no build or upload.",
                ["generatedAtUtc"] = DateTime.UtcNow.ToString("O"),
                ["sourceScene"] = FullFtScenePath,
                ["sourceSceneSha256"] = sourceHashAfter,
                ["sourceSceneUnchanged"] = true,
                ["sourceSceneStillOpen"] = sourceStillOpen,
                ["sourceSceneSavedOrDiscarded"] = false,
                ["targetScene"] = DiagnosticScenePath,
                ["targetSceneSha256"] = Sha256File(
                    Absolute(DiagnosticScenePath)),
                ["avatarPrefab"] = FullFtPrefabPath,
                ["avatarPrefabSha256"] =
                    Sha256File(Absolute(FullFtPrefabPath)),
                ["targetAvatarId"] = DiagnosticAvatarId,
                ["sourceSceneRootName"] = SceneRootName(FullFtScenePath),
                ["diagnosticSceneRootName"] =
                    SceneRootName(DiagnosticScenePath),
                ["expectedRootName"] = expectedRootName,
                ["rootNameGate"] = RootNameGateName,
                ["avatarCount"] = 1,
                ["gestureManagerCount"] = 1,
                ["receiverCount"] = 1,
                ["receivePort"] = 9012,
                ["sendPort"] = 9013,
                ["preflight"] = evidence,
                ["networkUploadStarted"] = false,
                ["vrChatLaunched"] = false,
                ["sourcePrefabWritten"] = false
            };
            success = true;
            WriteReport(
                PreparedReportPath,
                JsonConvert.SerializeObject(
                    result,
                    Formatting.Indented));
            return JsonConvert.SerializeObject(
                result,
                Formatting.Indented);
        }
        finally
        {
            if (copiedScene.IsValid())
                EditorSceneManager.CloseScene(copiedScene, true);
            if (!success && moved &&
                AssetDatabase.LoadAssetAtPath<SceneAsset>(
                    DiagnosticScenePath) != null)
                AssetDatabase.DeleteAsset(DiagnosticScenePath);
            if (!success &&
                AssetDatabase.LoadAssetAtPath<SceneAsset>(temporaryPath) != null)
                AssetDatabase.DeleteAsset(temporaryPath);
        }
    }

    public static async Task<string> StartUpload()
    {
        RequireIdle();
        var scene = SceneManager.GetSceneByPath(DiagnosticScenePath);
        if (!scene.IsValid() || !scene.isLoaded)
            throw new InvalidOperationException(
                "Open the prepared diagnostic scene before upload.");
        ReloadDiagnosticSceneIfTransientlyDirty();
        scene = SceneManager.GetSceneByPath(DiagnosticScenePath);
        if (!scene.IsValid() || !scene.isLoaded || scene.isDirty)
            throw new InvalidOperationException(
                "Diagnostic scene is not loaded and clean.");
        var avatar = SceneComponents<VRCAvatarDescriptor>(scene).Single()
            .gameObject;
        ValidatePreparedEvidence(avatar);

        var account = await CurrentAccount();
        if (!string.Equals(
                Convert.ToString(account["id"]),
                ExpectedUserId,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Unity SDK account does not match the expected user.");
        if (!Convert.ToBoolean(account["canPublishAvatars"]))
            throw new InvalidOperationException(
                "Unity SDK account cannot publish avatars.");
        var remote = await VRCApi.GetAvatar(
            DiagnosticAvatarId,
            forceRefresh: true);
        if (!string.Equals(
                remote.AuthorId,
                ExpectedUserId,
                StringComparison.Ordinal) ||
            !string.Equals(
                remote.ReleaseStatus,
                "private",
                StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException(
                "Target diagnostic Avatar is not the expected private " +
                "Avatar owned by this account.");
        if (remote.PendingUpload)
            throw new InvalidOperationException(
                "Target diagnostic Avatar already has a pending upload.");
        var versionBefore = remote.Version;
        remote.Name = FinalDisplayName;
        var builder = RequireBuilder(true);
        var operationId =
            "final-fullft-" + DateTime.UtcNow.ToString("yyyyMMddTHHmmssfffZ") +
            "-" + Guid.NewGuid().ToString("N").Substring(0, 8);
        lock (OperationLock)
        {
            if (_activeBuilder != null)
                throw new InvalidOperationException(
                    "Another SDK operation is active.");
            _activeBuilder = builder;
            _activeCancellation = new CancellationTokenSource();
            _activeOperationId = operationId;
        }
        var state = BaseState(operationId, avatar);
        state["status"] = "running";
        state["phase"] = "starting";
        state["networkUploadStarted"] = true;
        state["targetAvatarId"] = DiagnosticAvatarId;
        state["targetDisplayName"] = FinalDisplayName;
        state["avatarVersionBefore"] = versionBefore;
        state["message"] =
            "Operation accepted. BuildAndUpload starts through delayCall.";
        WriteState(state);
        EditorApplication.delayCall += () =>
            RunUploadAsync(builder, remote, operationId);
        return JsonConvert.SerializeObject(new
        {
            success = true,
            accepted = true,
            operationId,
            phase = "starting",
            stateFile = StatePath,
            targetAvatarId = DiagnosticAvatarId,
            existingPrivateAvatarUpdate = true,
            sdkStartsAfterHandleViaDelayCall = true
        }, Formatting.Indented);
    }

    /// <summary>
    /// One-time evidence refresh for the root-name migration of exactly the
    /// source FullFT scene and the prepared diagnostic scene. Every non-scene
    /// input hash must remain identical. The method is read-only and never
    /// saves either scene.
    /// </summary>
    public static string RefreshPreparedEvidenceAfterRootRename()
    {
        RequireIdle();
        if (!File.Exists(Absolute(PreparedReportPath)))
            throw new InvalidOperationException(
                "Prepared evidence is missing; no refresh is possible.");

        var sourceScene = SceneManager.GetSceneByPath(FullFtScenePath);
        var targetScene = SceneManager.GetSceneByPath(DiagnosticScenePath);
        if (!sourceScene.IsValid() || !sourceScene.isLoaded)
            throw new InvalidOperationException(
                "The migrated source FullFT scene must be loaded and saved " +
                "for the one-time evidence refresh.");
        if (!targetScene.IsValid() || !targetScene.isLoaded)
            throw new InvalidOperationException(
                "The migrated diagnostic scene must be loaded and saved for " +
                "the one-time evidence refresh.");
        if (sourceScene.isDirty || targetScene.isDirty)
            throw new InvalidOperationException(
                "Both scenes must be saved before refreshing prepared " +
                "evidence; this entry never saves or discards scene changes.");

        var sourceAvatar = SingleAvatar(
            sourceScene,
            "source FullFT scene");
        var targetAvatar = SingleAvatar(
            targetScene,
            "diagnostic scene");
        RequireFullFtTarget(sourceAvatar);
        RequireFullFtTarget(targetAvatar);
        RequireExpectedRootName(sourceAvatar, "source FullFT scene");
        RequireExpectedRootName(targetAvatar, "diagnostic scene");
        RequireSceneRoot(sourceAvatar, "source FullFT scene");
        RequireSceneRoot(targetAvatar, "diagnostic scene");
        var targetPipeline = targetAvatar.GetComponent<PipelineManager>();
        if (targetPipeline == null ||
            !string.Equals(
                targetPipeline.blueprintId,
                DiagnosticAvatarId,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Migrated diagnostic scene is not bound to the permitted " +
                "diagnostic Avatar ID.");

        var currentEvidence = CollectEvidence(true);
        var previous = JObject.Parse(
            File.ReadAllText(Absolute(PreparedReportPath)));
        if (previous["rootRenameMigration"] != null)
            throw new InvalidOperationException(
                "Prepared evidence has already been refreshed for a root " +
                "rename; refusing a second migration.");
        if (previous.Value<bool?>("success") != true ||
            !string.Equals(
                previous.Value<string>("targetScene"),
                DiagnosticScenePath,
                StringComparison.Ordinal) ||
            !string.Equals(
                previous.Value<string>("avatarPrefab"),
                FullFtPrefabPath,
                StringComparison.Ordinal) ||
            !string.Equals(
                previous.Value<string>("targetAvatarId"),
                DiagnosticAvatarId,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Existing prepared evidence does not identify the expected " +
                "final FullFT diagnostic scene.");

        var previousFiles =
            previous.SelectToken("preflight.files") as JObject ??
            throw new InvalidOperationException(
                "Existing prepared evidence has no file hash map.");
        var currentFiles =
            currentEvidence["files"] as Dictionary<string, object> ??
            throw new InvalidOperationException(
                "Current preflight evidence has no file hash map.");
        if (!currentFiles.ContainsKey(DiagnosticScenePath))
            throw new InvalidOperationException(
                "Current evidence does not include the diagnostic scene.");
        var oldSourceHash = previous.Value<string>(
            "sourceSceneSha256");
        var oldTargetHash = previous.Value<string>(
            "targetSceneSha256");
        var currentSourceHash = Sha256File(
            Absolute(FullFtScenePath));
        var currentTargetHash = Sha256File(
            Absolute(DiagnosticScenePath));
        if (string.IsNullOrEmpty(oldSourceHash) ||
            string.IsNullOrEmpty(oldTargetHash) ||
            string.Equals(
                oldSourceHash,
                currentSourceHash,
                StringComparison.OrdinalIgnoreCase) ||
            string.Equals(
                oldTargetHash,
                currentTargetHash,
                StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException(
                "Root-name migration requires both scene hashes to change.");

        var unchangedNonSceneFiles = new List<string>();
        var rebuiltEvidenceReports = new List<string>();
        var jointBuildSemanticsUnchanged = false;
        foreach (var property in previousFiles.Properties())
        {
            var path = property.Name;
            if (string.Equals(
                    path,
                    FullFtScenePath,
                    StringComparison.Ordinal))
                continue;
            if (!currentFiles.ContainsKey(path))
                throw new InvalidOperationException(
                    "Previously evidenced input is missing from fresh " +
                    "evidence: " + path + ".");
            var previousHash =
                property.Value?["sha256"]?.Value<string>();
            var currentHash = EvidenceFileHash(currentFiles, path);
            if (string.IsNullOrEmpty(previousHash) ||
                !string.Equals(
                    previousHash,
                    currentHash,
                    StringComparison.OrdinalIgnoreCase))
            {
                if (string.Equals(
                        path,
                        JointBuildReportPath,
                        StringComparison.Ordinal))
                {
                    var oldSemantic =
                        previous.SelectToken("preflight.jointBuild");
                    var currentSemantic = JObject.FromObject(
                        currentEvidence["jointBuild"]);
                    if (oldSemantic != null &&
                        JToken.DeepEquals(oldSemantic, currentSemantic))
                    {
                        jointBuildSemanticsUnchanged = true;
                        rebuiltEvidenceReports.Add(path);
                        continue;
                    }
                }
                throw new InvalidOperationException(
                    "Non-scene evidence changed unexpectedly: " + path +
                    ". Only the two migrated scene hashes may change.");
            }
            unchangedNonSceneFiles.Add(path);
        }
        if (!currentFiles.ContainsKey(DiagnosticScenePath))
            throw new InvalidOperationException(
                "Fresh evidence must include the migrated diagnostic scene.");
        if (!string.Equals(
                currentSourceHash,
                EvidenceFileHash(currentFiles, FullFtScenePath),
                StringComparison.OrdinalIgnoreCase) ||
            !string.Equals(
                currentTargetHash,
                EvidenceFileHash(currentFiles, DiagnosticScenePath),
                StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException(
                "Current scene hashes do not match the preflight evidence.");

        var sourcePrefabUnchanged =
            unchangedNonSceneFiles.Contains(FullFtPrefabPath);
        var shaderUnchanged =
            unchangedNonSceneFiles.Contains(ShaderPath);
        var budgetEvidenceUnchanged =
            (unchangedNonSceneFiles.Contains(JointBuildReportPath) ||
             jointBuildSemanticsUnchanged) &&
            unchangedNonSceneFiles.Contains(FinalBudgetReportPath);
        if (!sourcePrefabUnchanged ||
            !shaderUnchanged ||
            !budgetEvidenceUnchanged)
            throw new InvalidOperationException(
                "Prefab, shader, or budget evidence was not proven unchanged.");

        var preparedAbsolute = Absolute(PreparedReportPath);
        if (!currentFiles.ContainsKey(DiagnosticScenePath))
            throw new InvalidOperationException(
                "Root-name refresh requires the diagnostic scene in fresh " +
                "evidence.");
        var previousRawHash = Sha256File(preparedAbsolute);
        if (File.Exists(Absolute(PreRenamePreparedReportPath)))
        {
            if (!string.Equals(
                    Sha256File(Absolute(PreRenamePreparedReportPath)),
                    previousRawHash,
                    StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException(
                    "Pre-root-rename archive exists but does not match the " +
                    "current prepared evidence.");
        }
        else
        {
            var archiveAbsolute =
                Absolute(PreRenamePreparedReportPath);
            Directory.CreateDirectory(
                Path.GetDirectoryName(archiveAbsolute));
            File.Copy(
                preparedAbsolute,
                archiveAbsolute,
                false);
        }
        var refreshed = new Dictionary<string, object>
        {
            ["success"] = true,
            ["scope"] =
                "One-time root rename evidence refresh; no build or upload.",
            ["generatedAtUtc"] = DateTime.UtcNow.ToString("O"),
            ["sourceScene"] = FullFtScenePath,
            ["sourceSceneSha256"] = currentSourceHash,
            ["sourceSceneRootName"] =
                sourceAvatar.name,
            ["sourceSceneStillOpen"] = true,
            ["sourceSceneSavedOrDiscarded"] = false,
            ["targetScene"] = DiagnosticScenePath,
            ["targetSceneSha256"] = currentTargetHash,
            ["diagnosticSceneRootName"] =
                targetAvatar.name,
            ["avatarPrefab"] = FullFtPrefabPath,
            ["avatarPrefabSha256"] =
                Sha256File(Absolute(FullFtPrefabPath)),
            ["targetAvatarId"] = DiagnosticAvatarId,
            ["avatarCount"] = 1,
            ["gestureManagerCount"] = 1,
            ["receiverCount"] = 1,
            ["receivePort"] = 9012,
            ["sendPort"] = 9013,
            ["rootNameGate"] = RootNameGateName,
            ["rootRenameMigration"] = new
            {
                previousPreparedReport = PreparedReportPath,
                previousPreparedReportSha256 = previousRawHash,
                archivedPreviousPreparedReport =
                    PreRenamePreparedReportPath,
                previousSourceSceneSha256 = oldSourceHash,
                currentSourceSceneSha256 = currentSourceHash,
                previousTargetSceneSha256 = oldTargetHash,
                currentTargetSceneSha256 = currentTargetHash,
                sourcePrefabUnchanged,
                shaderUnchanged,
                budgetEvidenceUnchanged,
                rebuiltEvidenceReports,
                onlyAllowedSceneHashesChanged = true,
                nonSceneFileHashesUnchanged = true,
                unchangedNonSceneFiles
            },
            ["preflight"] = currentEvidence,
            ["networkUploadStarted"] = false,
            ["vrChatLaunched"] = false,
            ["sourcePrefabWritten"] = false
        };
        var json = JsonConvert.SerializeObject(
            refreshed,
            Formatting.Indented);
        WriteReport(PreparedReportPath, json);
        return json;
    }

    /// <summary>
    /// One-time evidence refresh after the explicitly scoped wand particle
    /// collision-force fix. Only the FullFT prefab, scene build proof, SDK
    /// budget reports, collision-fix report and scene copies may change.
    /// </summary>
    public static string RefreshPreparedEvidenceAfterCollisionForceFix()
    {
        RequireIdle();
        var preparedPath = Absolute(PreparedReportPath);
        if (!File.Exists(preparedPath))
            throw new InvalidOperationException(
                "Prepared evidence is missing; no refresh is possible.");
        var previousPrepared = JObject.Parse(
            File.ReadAllText(preparedPath));
        if (previousPrepared["collisionForceFix"] != null)
            throw new InvalidOperationException(
                "Prepared evidence was already refreshed for the collision " +
                "force fix; refusing a second migration.");

        var sourceScene = SceneManager.GetSceneByPath(FullFtScenePath);
        var targetScene = SceneManager.GetSceneByPath(DiagnosticScenePath);
        if (!sourceScene.IsValid() || !sourceScene.isLoaded ||
            !targetScene.IsValid() || !targetScene.isLoaded)
            throw new InvalidOperationException(
                "Both the source FullFT and diagnostic scene must be loaded.");
        if (sourceScene.isDirty || targetScene.isDirty)
            throw new InvalidOperationException(
                "Both scenes must be saved before refreshing prepared " +
                "evidence.");
        var sourceAvatar = SingleAvatar(sourceScene, "source FullFT scene");
        var targetAvatar = SingleAvatar(targetScene, "diagnostic scene");
        RequireFullFtTarget(sourceAvatar);
        RequireFullFtTarget(targetAvatar);
        RequireExpectedRootName(sourceAvatar, "source FullFT scene");
        RequireExpectedRootName(targetAvatar, "diagnostic scene");
        RequireSceneRoot(sourceAvatar, "source FullFT scene");
        RequireSceneRoot(targetAvatar, "diagnostic scene");
        var targetPipeline = targetAvatar.GetComponent<PipelineManager>();
        if (targetPipeline == null ||
            !string.Equals(
                targetPipeline.blueprintId,
                DiagnosticAvatarId,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Diagnostic Avatar is not bound to the permitted private ID.");

        var currentEvidence = CollectEvidence(true);
        var failures = (List<string>)currentEvidence["preflightFailures"];
        if (failures.Count != 0)
            throw new InvalidOperationException(
                "Post-fix preflight is not clean: " +
                string.Join(" | ", failures));
        var fix = JObject.Parse(
            File.ReadAllText(Absolute(CollisionFixReportPath)));
        Require(fix, "success", true, failures, "Collision fix success");
        if (string.IsNullOrWhiteSpace(fix.Value<string>("scope")))
            failures.Add("Collision fix scope is missing.");
        Require(
            fix,
            "nemesisSourceModified",
            false,
            failures,
            "Collision fix Nemesis source");
        Require(
            fix,
            "fullFtSourceModified",
            true,
            failures,
            "Collision fix FullFT source");
        if (fix.Value<float?>("colliderForceAfter") != 0f)
            failures.Add(
                "Collision force after fix expected 0, found " +
                (fix["colliderForceAfter"]?.ToString() ?? "<missing>") + ".");
        Require(
            fix,
            "pickupForceDisabled",
            true,
            failures,
            "Collision pickup force disabled");
        Require(
            fix,
            "disabledPickupLayers",
            false,
            failures,
            "Collision layers preserved");
        var fixedPath =
            fix.Value<string>("targetPath");
        if (string.IsNullOrEmpty(fixedPath) ||
            !fixedPath.TrimStart('/').EndsWith(
                "NemesisWandPrefab/WandMenu/LusterRailGun/" +
                "LusterRailGun_Toggle/Shot/Coin/Light",
                StringComparison.Ordinal))
            failures.Add(
                "Collision fix report has an unexpected target path.");
        if (!string.Equals(
                fix.Value<string>("prefabSha256Before"),
                previousPrepared.Value<string>("avatarPrefabSha256"),
                StringComparison.OrdinalIgnoreCase) ||
            !string.Equals(
                fix.Value<string>("prefabSha256After"),
                Sha256File(Absolute(FullFtPrefabPath)),
                StringComparison.OrdinalIgnoreCase))
            failures.Add(
                "Collision fix prefab hashes do not bracket the prepared " +
                "and current FullFT prefab.");
        if (failures.Count != 0)
            throw new InvalidOperationException(
                "Collision fix evidence failed: " +
                string.Join(" | ", failures));

        var previousFiles =
            previousPrepared.SelectToken("preflight.files") as JObject ??
            throw new InvalidOperationException(
                "Previous prepared evidence has no file hash map.");
        var currentFiles =
            currentEvidence["files"] as Dictionary<string, object> ??
            throw new InvalidOperationException(
                "Current preflight evidence has no file hash map.");
        var expectedChangedFiles = new HashSet<string>(
            new[]
            {
                FullFtPrefabPath,
                FullFtScenePath,
                DiagnosticScenePath,
                JointBuildReportPath,
                FinalBudgetReportPath,
                SceneBuildReportPath,
                CollisionFixReportPath
            },
            StringComparer.Ordinal);
        var changedFiles = new List<string>();
        foreach (var property in previousFiles.Properties())
        {
            var path = property.Name;
            var previousHash =
                property.Value?["sha256"]?.Value<string>();
            var currentHash = EvidenceFileHash(currentFiles, path);
            if (string.Equals(
                    previousHash,
                    currentHash,
                    StringComparison.OrdinalIgnoreCase))
                continue;
            if (!expectedChangedFiles.Contains(path))
                throw new InvalidOperationException(
                    "Unexpected evidence changed during collision fix: " +
                    path + ".");
            changedFiles.Add(path);
        }
        if (!changedFiles.Contains(FullFtPrefabPath))
            throw new InvalidOperationException(
                "FullFT prefab hash did not change for the collision fix.");
        var previousTargetSceneHash =
            previousPrepared.Value<string>("targetSceneSha256");
        var currentTargetSceneHash =
            Sha256File(Absolute(DiagnosticScenePath));
        var previousSourceSceneHash =
            previousPrepared.Value<string>("sourceSceneSha256");
        var currentSourceSceneHash =
            Sha256File(Absolute(FullFtScenePath));
        if (string.IsNullOrEmpty(previousTargetSceneHash) ||
            string.Equals(
                previousTargetSceneHash,
                currentTargetSceneHash,
                StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException(
                "Diagnostic scene was not refreshed to the collision-fixed " +
                "FullFT prefab.");
        if (string.IsNullOrEmpty(previousSourceSceneHash) ||
            !string.Equals(
                previousSourceSceneHash,
                currentSourceSceneHash,
                StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException(
                "Source FullFT scene changed during collision fix.");

        var budget = JObject.Parse(
            File.ReadAllText(Absolute(FinalBudgetReportPath)));
        var ft = budget["ft"] as JObject;
        var controls = budget["controls"] as JObject;
        var morphForge = budget["morphForge"] as JObject;
        var geometry = morphForge?["geometry"] as JObject;
        var placement = morphForge?["placement"] as JObject;
        if (budget.Value<bool?>("success") != true ||
            budget.Value<int?>("finalBits") != 256 ||
            budget.Value<int?>("remainingBits") != 0 ||
            ft?.Value<int?>("representedBits") != 161 ||
            ft?.Value<bool?>("allDirect") != true ||
            controls?.Value<int?>("representedBits") != 8 ||
            controls?.Value<bool?>("allDirect") != true ||
            geometry?.Value<int?>("representedBits") != 56 ||
            geometry?.Value<bool?>("allDirect") != true ||
            placement?.Value<int?>("representedBits") != 14 ||
            placement?.Value<bool?>("allDirect") != true)
            throw new InvalidOperationException(
                "Post-collision-fix budget is not 256/161/8/56/14.");

        var oldPreparedHash = Sha256File(preparedPath);
        if (File.Exists(Absolute(CollisionFixPreparedReportPath)))
        {
            if (!string.Equals(
                    Sha256File(Absolute(CollisionFixPreparedReportPath)),
                    oldPreparedHash,
                    StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException(
                    "Existing pre-collision-fix archive does not match " +
                    "prepared evidence.");
        }
        else
        {
            var archiveAbsolute =
                Absolute(CollisionFixPreparedReportPath);
            Directory.CreateDirectory(
                Path.GetDirectoryName(archiveAbsolute));
            File.Copy(preparedPath, archiveAbsolute, false);
        }
        var refreshed = new Dictionary<string, object>
        {
            ["success"] = true,
            ["scope"] =
                "One-time particle collision-force evidence refresh; no " +
                "build or upload.",
            ["generatedAtUtc"] = DateTime.UtcNow.ToString("O"),
            ["sourceScene"] = FullFtScenePath,
            ["sourceSceneSha256"] =
                Sha256File(Absolute(FullFtScenePath)),
            ["sourceSceneRootName"] = sourceAvatar.name,
            ["sourceSceneStillOpen"] = true,
            ["sourceSceneSavedOrDiscarded"] = false,
            ["targetScene"] = DiagnosticScenePath,
            ["targetSceneSha256"] =
                Sha256File(Absolute(DiagnosticScenePath)),
            ["diagnosticSceneRootName"] = targetAvatar.name,
            ["avatarPrefab"] = FullFtPrefabPath,
            ["avatarPrefabSha256"] =
                Sha256File(Absolute(FullFtPrefabPath)),
            ["targetAvatarId"] = DiagnosticAvatarId,
            ["avatarCount"] = 1,
            ["gestureManagerCount"] = 1,
            ["receiverCount"] = 1,
            ["receivePort"] = 9012,
            ["sendPort"] = 9013,
            ["rootNameGate"] = RootNameGateName,
            ["collisionForceFix"] = new
            {
                previousPreparedReport = PreparedReportPath,
                previousPreparedReportSha256 = oldPreparedHash,
                archivedPreviousPreparedReport =
                    CollisionFixPreparedReportPath,
                fixReport = CollisionFixReportPath,
                targetPath = fixedPath,
                colliderForceAfter = 0,
                pickupForceDisabled = true,
                disabledPickupLayers = false,
                changedFiles,
                onlyExpectedArtifactsChanged = true
            },
            ["preflight"] = currentEvidence,
            ["networkUploadStarted"] = false,
            ["vrChatLaunched"] = false,
            ["sourcePrefabWritten"] = false
        };
        var json = JsonConvert.SerializeObject(
            refreshed,
            Formatting.Indented);
        WriteReport(PreparedReportPath, json);
        return json;
    }

    public static async Task<string> InspectRemote()
    {
        RequireIdle();
        var account = await CurrentAccount();
        var avatar = await VRCApi.GetAvatar(
            DiagnosticAvatarId,
            forceRefresh: true);
        var accountMatches = string.Equals(
            Convert.ToString(account["id"]),
            ExpectedUserId,
            StringComparison.Ordinal);
        var ownerMatches = string.Equals(
            avatar.AuthorId,
            ExpectedUserId,
            StringComparison.Ordinal);
        var privateAvatar = string.Equals(
            avatar.ReleaseStatus,
            "private",
            StringComparison.OrdinalIgnoreCase);
        var result = new
        {
            success =
                accountMatches &&
                Convert.ToBoolean(account["canPublishAvatars"]) &&
                ownerMatches &&
                privateAvatar &&
                !avatar.PendingUpload,
            generatedAtUtc = DateTime.UtcNow.ToString("O"),
            avatarId = avatar.ID,
            avatarName = avatar.Name,
            avatarVersion = avatar.Version,
            releaseStatus = avatar.ReleaseStatus,
            pendingUpload = avatar.PendingUpload,
            expectedUserIdMatches = accountMatches,
            ownerMatches,
            privateAvatar,
            canPublishAvatars =
                Convert.ToBoolean(account["canPublishAvatars"]),
            networkUploadStarted = false
        };
        return JsonConvert.SerializeObject(
            result,
            Formatting.Indented);
    }

    public static async Task<string> InspectAnalysis()
    {
        RequireIdle();
        var avatar = await VRCApi.GetAvatar(
            DiagnosticAvatarId,
            forceRefresh: true);
        if (!string.Equals(
                avatar.AuthorId,
                ExpectedUserId,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Refusing analysis for an Avatar not owned by the expected " +
                "user.");
        var assetUrl = avatar.GetLatestAssetUrlForCurrentPlatform();
        if (!TryAnalysisRequest(
                assetUrl,
                out var securityPath,
                out var fileId,
                out var fileVersion))
            throw new InvalidOperationException(
                "Could not derive analysis file/version from the asset URL.");
        var prefix = "analysis/" + fileId + "/" + fileVersion;
        var endpoints = new[]
        {
            prefix,
            prefix + "/standard",
            securityPath
        };
        var results = new List<object>();
        var allTerminal = true;
        var allPassed = true;
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
                var error = raw["error"] as JObject;
                var errorCode = error?.Value<int?>("status_code");
                var pending =
                    errorCode == 202 ||
                    string.Equals(
                        error?.Value<string>("message"),
                        "Analysis not yet available",
                        StringComparison.OrdinalIgnoreCase);
                var passed = !pending &&
                             raw.Value<bool?>("success") == true;
                if (pending)
                    allTerminal = false;
                if (!passed)
                    allPassed = false;
                results.Add(new
                {
                    endpoint,
                    transport = "OK",
                    pending,
                    passed,
                    errorCode,
                    errorMessage = error?.Value<string>("message"),
                    fileSize = raw.Value<long?>("fileSize"),
                    uncompressedSize =
                        raw.Value<long?>("uncompressedSize"),
                    performanceRating =
                        raw.Value<string>("performanceRating"),
                    createdAt = raw.Value<string>("created_at"),
                    avatarStats = raw["avatarStats"]
                });
            }
            catch (ApiErrorException error)
            {
                allPassed = false;
                results.Add(new
                {
                    endpoint,
                    transport = "ApiErrorException",
                    pending = false,
                    passed = false,
                    errorCode = error.StatusCode,
                    errorMessage = error.ErrorMessage
                });
            }
        }
        var report = new
        {
            success = allPassed && allTerminal,
            generatedAtUtc = DateTime.UtcNow.ToString("O"),
            avatarId = avatar.ID,
            avatarVersion = avatar.Version,
            releaseStatus = avatar.ReleaseStatus,
            fileId,
            fileVersion,
            terminal = allTerminal,
            allEndpointsPassed = allPassed,
            isStillProcessing = !allTerminal,
            results,
            secretsIncluded = false,
            networkUploadStarted = false
        };
        var json = JsonConvert.SerializeObject(
            report,
            Formatting.Indented);
        WriteReport(
            "Docs/Security/final-fullft-analysis.json",
            json);
        return json;
    }

    public static string GetOperation()
    {
        var state = ReadStateOrNull();
        if (state == null)
            return JsonConvert.SerializeObject(
                new { success = false, error = "No operation state exists." },
                Formatting.Indented);
        return JsonConvert.SerializeObject(
            state,
            Formatting.Indented);
    }

    public static string Cancel()
    {
        _activeBuilder?.CancelUpload();
        _activeCancellation?.Cancel();
        var state = ReadStateOrNull();
        if (state != null)
        {
            state["cancelRequested"] = true;
            state["cancelRequestedAtUtc"] = DateTime.UtcNow.ToString("O");
            WriteState(state);
        }
        return GetOperation();
    }

    private static Dictionary<string, object> CollectEvidence(
        bool requireDiagnosticScene = false)
    {
        var result = new Dictionary<string, object>();
        AddEvidence(result);
        AddFiles(result, requireDiagnosticScene);
        var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(
            FullFtPrefabPath);
        AddPrefabAudit(result, prefab);
        var failures = (List<string>)result["preflightFailures"];
        if (failures.Count > 0)
            throw new InvalidOperationException(
                "FullFT preflight failed: " + string.Join(" | ", failures));
        return result;
    }

    private static void AddFiles(
        Dictionary<string, object> result,
        bool requireDiagnosticScene = false)
    {
        var files = new Dictionary<string, object>
        {
            [FullFtPrefabPath] = FileRecord(FullFtPrefabPath),
            [FullFtScenePath] = FileRecord(FullFtScenePath),
            [DiagnosticScenePath] = FileRecord(DiagnosticScenePath),
            [MaterialPath] = FileRecord(MaterialPath),
            [ShaderPath] = FileRecord(ShaderPath),
            [JointBuildReportPath] = FileRecord(JointBuildReportPath),
            [FinalBudgetReportPath] = FileRecord(FinalBudgetReportPath),
            [SceneBuildReportPath] = FileRecord(SceneBuildReportPath),
            [MigrationReportPath] = FileRecord(MigrationReportPath),
            [ProductionVisualReportPath] =
                FileRecord(ProductionVisualReportPath),
            [RootNameMigrationReportPath] =
                FileRecord(RootNameMigrationReportPath),
            [ActualSceneBuildReportPath] =
                FileRecord(ActualSceneBuildReportPath),
            [ProfilePath] = FileRecord(ProfilePath)
        };
        result["files"] = files;
        if (result["preflightFailures"] is List<string> failures)
        {
            foreach (var pair in files)
            {
                var record = pair.Value as Dictionary<string, object>;
                if (record == null ||
                    !(record["exists"] is bool exists) ||
                    !exists)
                {
                    if (requireDiagnosticScene ||
                        !string.Equals(
                            pair.Key,
                            DiagnosticScenePath,
                            StringComparison.Ordinal))
                        failures.Add(
                            "Required file is missing: " + pair.Key);
                }
            }
        }
    }

    private static void AddEvidence(
        Dictionary<string, object> result,
        bool requireMigratedDiagnosticScene = false)
    {
        var failures = new List<string>();
        result["preflightFailures"] = failures;
        result["targetAvatarId"] = DiagnosticAvatarId;
        result["expectedUserId"] = ExpectedUserId;
        result["targetMustBePrivate"] = true;
        result["originalAvatarIdChanged"] = false;
        result["sourcePrefabWillBeModified"] = false;
        result["sourceSceneWillBeModified"] = false;
        result["autoUploadInPrepare"] = false;

        var joint = ReadJson(
            JointBuildReportPath,
            failures,
            "JointBuild report");
        var budget = ReadJson(
            FinalBudgetReportPath,
            failures,
            "Final budget");
        var scene = ReadJson(
            SceneBuildReportPath,
            failures,
            "Scene build report");
        var migration = ReadJson(
            MigrationReportPath,
            failures,
            "Catalog migration report");
        var productionVisual = ReadJson(
            ProductionVisualReportPath,
            failures,
            "Production visual report");
        var rootMigration = ReadJson(
            RootNameMigrationReportPath,
            failures,
            "Root-name migration report");
        var actualSceneBuild = ReadJson(
            ActualSceneBuildReportPath,
            failures,
            "Actual scene build report");
        var profile = LoadProfileAndValidate();
        result["profile"] = new
        {
            path = ProfilePath,
            schemaVersion = profile?.schemaVersion,
            profileVersion = profile?.profileVersion,
            enabled = profile?.enabled,
            avatarNamePrefix = profile?.avatarNamePrefix,
            expectedFtSyncedBits = profile?.expectedFtSyncedBits,
            expectedVrcftControlParameters =
                profile?.expectedVrcftControlParameters,
            expectedGeometryBits = profile?.expectedGeometryBits,
            expectedPlacementBits = profile?.expectedPlacementBits,
            maxFinalBits = profile?.maxFinalBits
        };
        if (profile == null)
            failures.Add("FullFT sync profile is missing or invalid.");
        else if (!profile.enabled)
            failures.Add("FullFT sync profile is disabled.");
        var migrationMetadataGuid =
            migration?.Value<string>("metadataGuid");
        var migrationMaterialGuid =
            migration?.Value<string>("materialGuid");

        if (joint != null)
        {
            Require(joint, "success", true, failures, "JointBuild success");
            Require(
                joint,
                "avatar",
                FullFtPrefabPath,
                failures,
                "JointBuild Avatar path");
            var expectedHash = joint.Value<string>("prefabSha256");
            var actualHash = Sha256File(Absolute(FullFtPrefabPath));
            if (string.IsNullOrEmpty(expectedHash) ||
                !string.Equals(
                    expectedHash,
                    actualHash,
                    StringComparison.OrdinalIgnoreCase))
                failures.Add("FullFT prefab changed after JointBuild.");
            var expectedBudgetHash =
                joint.Value<string>("finalBudgetSha256");
            var actualBudgetHash =
                Sha256File(Absolute(FinalBudgetReportPath));
            if (string.IsNullOrEmpty(expectedBudgetHash) ||
                !string.Equals(
                    expectedBudgetHash,
                    actualBudgetHash,
                    StringComparison.OrdinalIgnoreCase))
                failures.Add(
                    "Final budget report changed after JointBuild.");
            result["jointBuild"] = new
            {
                success = true,
                prefabSha256 = expectedHash,
                finalBudgetSha256 = expectedBudgetHash,
                finalBits = joint.Value<int?>("finalBits")
            };
        }
        if (budget != null)
        {
            Require(
                budget,
                "success",
                true,
                failures,
                "Final budget success");
            Require(
                budget,
                "finalBits",
                256,
                failures,
                "Final budget bits");
            Require(
                budget,
                "maxBits",
                256,
                failures,
                "Final budget maximum");
            Require(
                budget,
                "remainingBits",
                0,
                failures,
                "Final budget remaining");
            var ft = budget["ft"] as JObject;
            var controls = budget["controls"] as JObject;
            var morphForge = budget["morphForge"] as JObject;
            var geometry = morphForge?["geometry"] as JObject;
            var placement = morphForge?["placement"] as JObject;
            if (ft == null || ft.Value<int?>("representedBits") != 161 ||
                ft.Value<bool?>("allDirect") != true)
                failures.Add("FT is not 161 direct bits.");
            if (controls == null ||
                controls.Value<int?>("representedBits") != 8 ||
                controls.Value<bool?>("allDirect") != true)
                failures.Add("VRCFT controls are not 8 direct bits.");
            if (geometry == null ||
                geometry.Value<int?>("representedBits") != 56 ||
                geometry.Value<bool?>("allDirect") != true)
                failures.Add("MF3 geometry is not 56 direct bits.");
            if (placement == null ||
                placement.Value<int?>("representedBits") != 14 ||
                placement.Value<bool?>("allDirect") != true)
                failures.Add("Placement is not 14 direct bits.");
            result["budget"] = new
            {
                finalBits = budget.Value<int?>("finalBits"),
                ftBits = ft?.Value<int?>("representedBits"),
                ftAllDirect = ft?.Value<bool?>("allDirect"),
                controlsBits = controls?.Value<int?>("representedBits"),
                controlsAllDirect = controls?.Value<bool?>("allDirect"),
                geometryBits = geometry?.Value<int?>("representedBits"),
                geometryAllDirect = geometry?.Value<bool?>("allDirect"),
                placementBits =
                    placement?.Value<int?>("representedBits"),
                placementAllDirect =
                    placement?.Value<bool?>("allDirect")
            };
        }
        if (scene != null)
        {
            Require(
                scene,
                "success",
                true,
                failures,
                "Scene build success");
            Require(
                scene,
                "sourceSceneHashUnchanged",
                true,
                failures,
                "Source scene hash");
            Require(scene, "sceneAvatarCount", 1, failures, "Avatar count");
            Require(
                scene,
                "targetScene",
                FullFtScenePath,
                failures,
                "Scene build target");
            var sceneHash = scene.Value<string>("targetSceneSha256");
            var scenePrefabHash =
                scene.SelectToken("jointBuild.prefabSha256")?.Value<string>();
            var sceneProofMatchesCurrentPrefab =
                !string.IsNullOrEmpty(scenePrefabHash) &&
                string.Equals(
                    scenePrefabHash,
                    Sha256File(Absolute(FullFtPrefabPath)),
                    StringComparison.OrdinalIgnoreCase);
            result["sceneBuild"] = new
            {
                targetScene = scene.Value<string>("targetScene"),
                creationSceneSha256 = sceneHash,
                currentSceneSha256 =
                    Sha256File(Absolute(FullFtScenePath)),
                creationHashMatchesCurrent =
                    !string.IsNullOrEmpty(sceneHash) &&
                    string.Equals(
                        sceneHash,
                        Sha256File(Absolute(FullFtScenePath)),
                        StringComparison.OrdinalIgnoreCase),
                sourceSceneSha256 =
                    scene.Value<string>("sourceSceneSha256"),
                proofPrefabSha256 = scenePrefabHash,
                proofMatchesCurrentPrefab =
                    sceneProofMatchesCurrentPrefab,
                interpretation = sceneProofMatchesCurrentPrefab
                    ? "Scene creation proof also matches the current prefab."
                    : "The scene was created before the latest in-place " +
                      "prefab build. Live scene and prefab validation remain " +
                      "authoritative for the diagnostic copy."
            };
        }
        if (migration != null)
        {
            Require(
                migration,
                "success",
                true,
                failures,
                "Catalog migration success");
            Require(
                migration,
                "allDataPreserved",
                true,
                failures,
                "Catalog data preservation");
            var size = migration["afterSize"] as JArray;
            if (size == null || size.Count != 2 ||
                size[0].Value<int>() != 256 || size[1].Value<int>() != 64)
                failures.Add("Production metadata is not 256x64.");
            result["catalogMigration"] = new
            {
                metadataGuid = migration.Value<string>("metadataGuid"),
                materialGuid = migration.Value<string>("materialGuid"),
                shader = migration.Value<string>("shader"),
                allDataPreserved = true
            };
        }
        if (productionVisual != null)
        {
            Require(
                productionVisual,
                "success",
                true,
                failures,
                "Production visual success");
            var checks = productionVisual["checks"] as JArray;
            var failedChecks = checks?
                .Where(item => item.Value<bool?>("pass") != true)
                .ToArray() ?? new JToken[0];
            if (checks == null || checks.Count < 16 ||
                failedChecks.Length != 0)
                failures.Add(
                    "Production visual suite is incomplete or has failures.");
            result["productionVisual"] = new
            {
                checkCount = checks?.Count ?? 0,
                failedCheckCount = failedChecks.Length,
                allPassed = checks != null &&
                            checks.Count >= 16 &&
                            failedChecks.Length == 0
            };
        }
        var expectedRootName = GetFullFtRootName();
        if (rootMigration != null)
        {
            Require(
                rootMigration,
                "success",
                true,
                failures,
                "Root-name migration success");
            Require(
                rootMigration,
                "prefabWritten",
                false,
                failures,
                "Root-name migration prefab write");
            Require(
                rootMigration,
                "uploaded",
                false,
                failures,
                "Root-name migration upload");
            Require(
                rootMigration,
                "originalStage3Changed",
                false,
                failures,
                "Root-name migration Stage3");
            var migrationChanges = rootMigration["changes"] as JArray;
            if (migrationChanges == null || migrationChanges.Count != 2)
                failures.Add(
                    "Root-name migration must contain exactly two scenes.");
            foreach (var path in new[]
                     {
                         FullFtScenePath,
                         DiagnosticScenePath
                     })
            {
                var change = migrationChanges?
                    .FirstOrDefault(item =>
                        string.Equals(
                            item.Value<string>("path"),
                            path,
                            StringComparison.Ordinal));
                if (change == null ||
                    !string.Equals(
                        change.Value<string>("beforeName"),
                        "Avatar",
                        StringComparison.Ordinal) ||
                    !string.Equals(
                        change.Value<string>("afterName"),
                        expectedRootName,
                        StringComparison.Ordinal) ||
                    change.Value<bool?>("profileMatches") != true ||
                    change.Value<bool?>("placementUnchanged") != true ||
                    (string.Equals(
                         path,
                         FullFtScenePath,
                         StringComparison.Ordinal) &&
                     !string.Equals(
                         change.Value<string>("afterSha256"),
                         Sha256File(Absolute(path)),
                         StringComparison.OrdinalIgnoreCase)))
                    failures.Add(
                        "Root-name migration evidence does not match " +
                        path + ".");
            }
            result["rootNameMigration"] = new
            {
                expectedRootName,
                changeCount = migrationChanges?.Count ?? 0,
                prefabWritten = false,
                uploaded = false
            };
        }
        if (actualSceneBuild != null)
        {
            Require(
                actualSceneBuild,
                "success",
                true,
                failures,
                "Actual scene build success");
            Require(
                actualSceneBuild,
                "status",
                "completed",
                failures,
                "Actual scene build status");
            Require(
                actualSceneBuild,
                "scene",
                DiagnosticScenePath,
                failures,
                "Actual scene build target");
            Require(
                actualSceneBuild,
                "rootName",
                expectedRootName,
                failures,
                "Actual scene build root");
            Require(
                actualSceneBuild,
                "cloneNameRewritten",
                false,
                failures,
                "Actual scene clone name rewrite");
            Require(
                actualSceneBuild,
                "profileMatches",
                true,
                failures,
                "Actual scene profile gate");
            Require(
                actualSceneBuild,
                "bits",
                256,
                failures,
                "Actual scene bit budget");
            var cloneName =
                actualSceneBuild.Value<string>("buildCloneName");
            if (string.IsNullOrEmpty(cloneName) ||
                !cloneName.StartsWith(
                    expectedRootName,
                    StringComparison.Ordinal))
                failures.Add(
                    "Actual scene build clone name is not the natural " +
                    "prefixed clone.");
            var sceneBudget =
                actualSceneBuild["finalBudget"] as JObject;
            if (sceneBudget == null ||
                sceneBudget.Value<bool?>("success") != true ||
                sceneBudget.Value<int?>("finalBits") != 256 ||
                sceneBudget.Value<int?>("remainingBits") != 0 ||
                !string.Equals(
                    sceneBudget.Value<string>("avatar"),
                    cloneName,
                    StringComparison.Ordinal))
                failures.Add(
                    "Actual scene final budget does not match the natural " +
                    "prefixed build clone.");
            var sceneFt = sceneBudget?["ft"] as JObject;
            var sceneControls = sceneBudget?["controls"] as JObject;
            var sceneMorphForge =
                sceneBudget?["morphForge"] as JObject;
            var sceneGeometry =
                sceneMorphForge?["geometry"] as JObject;
            var scenePlacement =
                sceneMorphForge?["placement"] as JObject;
            if (sceneFt?.Value<int?>("representedBits") != 161 ||
                sceneFt?.Value<bool?>("allDirect") != true)
                failures.Add(
                    "Actual scene build FT is not 161 direct bits.");
            if (sceneControls?.Value<int?>("representedBits") != 8 ||
                sceneControls?.Value<bool?>("allDirect") != true)
                failures.Add(
                    "Actual scene build VRCFT controls are not 8 direct bits.");
            if (sceneGeometry?.Value<int?>("representedBits") != 56 ||
                sceneGeometry?.Value<bool?>("allDirect") != true)
                failures.Add(
                    "Actual scene build geometry is not 56 direct bits.");
            if (scenePlacement?.Value<int?>("representedBits") != 14 ||
                scenePlacement?.Value<bool?>("allDirect") != true)
                failures.Add(
                    "Actual scene build placement is not 14 direct bits.");
            var sceneChecks = sceneBudget?["checks"] as JArray;
            if (sceneChecks == null ||
                sceneChecks.Count == 0 ||
                sceneChecks.Any(item =>
                    item.Value<bool?>("pass") != true))
                failures.Add(
                    "Actual scene build has failed or missing budget checks.");
            if (actualSceneBuild.Value<bool?>("uploaded") != false ||
                actualSceneBuild.Value<bool?>("sceneUnchanged") != true)
                failures.Add(
                    "Actual scene build must be non-uploading and leave the " +
                    "scene unchanged.");
            result["actualSceneBuild"] = new
            {
                rootName = actualSceneBuild.Value<string>("rootName"),
                buildCloneName = cloneName,
                cloneNameRewritten = false,
                profileMatches = true,
                finalBits = sceneBudget?.Value<int?>("finalBits"),
                ftBits = sceneFt?.Value<int?>("representedBits"),
                controlsBits =
                    sceneControls?.Value<int?>("representedBits"),
                geometryBits =
                    sceneGeometry?.Value<int?>("representedBits"),
                placementBits =
                    scenePlacement?.Value<int?>("representedBits"),
                budgetCheckCount = sceneChecks?.Count ?? 0,
                sceneUnchanged = true,
                uploaded = false
            };
        }

        var material = AssetDatabase.LoadAssetAtPath<Material>(MaterialPath);
        if (material == null)
        {
            failures.Add("Production material is missing.");
            result["material"] = null;
        }
        else
        {
            var shader = material.shader;
            var meta = material.GetTexture("_CatalogMeta") as Texture2D;
            var atlas = material.GetTexture("_CatalogAtlas") as Texture2D;
            var metadataPath = meta == null
                ? null
                : AssetDatabase.GetAssetPath(meta);
            var metadataGuid = string.IsNullOrEmpty(metadataPath)
                ? null
                : AssetDatabase.AssetPathToGUID(metadataPath);
            var materialGuid =
                AssetDatabase.AssetPathToGUID(MaterialPath);
            if (shader == null ||
                !string.Equals(
                    AssetDatabase.GetAssetPath(shader),
                    ShaderPath,
                    StringComparison.Ordinal) ||
                !string.Equals(shader.name, ShaderName, StringComparison.Ordinal))
                failures.Add(
                    "Production material does not use the repaired Grid shader.");
            if (meta == null || meta.width != 256 || meta.height != 64 ||
                meta.format != TextureFormat.RGBAFloat)
                failures.Add("Production metadata is not 256x64 RGBAFloat.");
            if (atlas == null || atlas.width != 384 || atlas.height != 96)
                failures.Add("Production Atlas is not 384x96.");
            if (!string.IsNullOrEmpty(migrationMetadataGuid) &&
                !string.Equals(
                    migrationMetadataGuid,
                    metadataGuid,
                    StringComparison.Ordinal))
                failures.Add(
                    "Production metadata GUID differs from migration proof.");
            if (!string.IsNullOrEmpty(migrationMaterialGuid) &&
                !string.Equals(
                    migrationMaterialGuid,
                    materialGuid,
                    StringComparison.Ordinal))
                failures.Add(
                    "Production material GUID differs from migration proof.");
            if (!material.HasProperty("_MetaWidth") ||
                !Mathf.Approximately(material.GetFloat("_MetaWidth"), 256f) ||
                !material.HasProperty("_MetaHeight") ||
                !Mathf.Approximately(material.GetFloat("_MetaHeight"), 64f))
                failures.Add("Production Grid metadata dimensions are wrong.");
            result["material"] = new
            {
                path = MaterialPath,
                shader = shader?.name,
                metaWidth = meta?.width,
                metaHeight = meta?.height,
                metaFormat = meta?.format.ToString(),
                metadataPath,
                metadataGuid,
                materialGuid,
                atlasWidth = atlas?.width,
                atlasHeight = atlas?.height,
                metaWidthProperty = material.HasProperty("_MetaWidth")
                    ? material.GetFloat("_MetaWidth")
                    : (float?)null,
                metaHeightProperty = material.HasProperty("_MetaHeight")
                    ? material.GetFloat("_MetaHeight")
                    : (float?)null
            };
        }
    }

    private static void AddPrefabAudit(
        Dictionary<string, object> result,
        GameObject prefab)
    {
        if (prefab == null)
        {
            result["prefab"] = null;
            if (result["preflightFailures"] is List<string> failures)
                failures.Add("FullFT prefab is missing.");
            return;
        }
        try
        {
            var descriptors = prefab.GetComponentsInChildren<
                VRCAvatarDescriptor>(true);
            var stores = prefab.GetComponentsInChildren<ParticleSystem>(true)
                .Where(item => item.name == "Store")
                .ToArray();
            var material = AssetDatabase.LoadAssetAtPath<Material>(
                MaterialPath);
            var missingScripts = prefab
                .GetComponentsInChildren<Transform>(true)
                .Sum(item =>
                    GameObjectUtility.
                        GetMonoBehavioursWithMissingScriptCount(
                            item.gameObject));
            var collisionForceSystems =
                ParticleCollisionForceViolations(prefab);
            var resultRecord = new Dictionary<string, object>
            {
                ["rootName"] = prefab.name,
                ["avatarDescriptorCount"] = descriptors.Length,
                ["storeCount"] = stores.Length,
                ["missingScripts"] = missingScripts,
                ["morphForgeRuntimeScriptCount"] =
                    prefab.GetComponentsInChildren<MonoBehaviour>(true)
                        .Count(item =>
                            item != null &&
                            item.GetType().Namespace != null &&
                            item.GetType().Namespace.StartsWith(
                                "MorphForge",
                                StringComparison.Ordinal)),
                ["capacity"] = stores.Select(item =>
                    item.main.maxParticles).ToArray(),
                ["lifetime"] = stores.Select(item =>
                    item.main.startLifetime.constant.ToString("R"))
                    .ToArray(),
                ["materialsMatchProduction"] =
                    stores.Length == 1 &&
                    stores[0].GetComponent<ParticleSystemRenderer>()
                        .sharedMaterial == material,
                ["hasPlacement"] = prefab.transform.Find(
                    "MorphForgePlacement") != null,
                ["geometryModuleCount"] = prefab
                    .GetComponentsInChildren<Transform>(true)
                    .Count(item => item.name == "MorphForgeGeometry"),
                ["particleCollisionForceSystems"] =
                    collisionForceSystems
            };
            result["prefab"] = resultRecord;
            var failures = result["preflightFailures"] as List<string>;
            if (failures == null)
                return;
            if (descriptors.Length != 1)
                failures.Add("FullFT prefab must have exactly one descriptor.");
            if (stores.Length != 1 ||
                stores[0].main.maxParticles != ExpectedCapacity ||
                !float.IsPositiveInfinity(
                    stores[0].main.startLifetime.constant))
                failures.Add(
                    "FullFT prefab Store must be 16384 with Infinity lifetime.");
            if (!(resultRecord["materialsMatchProduction"] is bool match) ||
                !match)
                failures.Add(
                    "FullFT prefab Store does not use the production material.");
            if (missingScripts != 0)
                failures.Add("FullFT prefab has missing scripts.");
            if (!(resultRecord["hasPlacement"] is bool placement) ||
                !placement ||
                (int)resultRecord["geometryModuleCount"] != 1)
                failures.Add(
                    "FullFT prefab is missing its placement/geometry module.");
            if (collisionForceSystems.Length != 0)
                failures.Add(
                    "FullFT prefab has particle systems applying collision " +
                    "force to avatar layers: " +
                    string.Join(", ", collisionForceSystems));
        }
        catch (Exception error)
        {
            result["prefabAuditError"] = error.ToString();
            var failures = result["preflightFailures"] as List<string>;
            failures?.Add("Prefab audit failed: " + error.Message);
        }
    }

    private static void ValidateFinalAvatar(
        GameObject avatar,
        bool requireDiagnosticIdentity)
    {
        if (avatar == null)
            throw new InvalidOperationException(
                "Final FullFT Avatar is missing.");
        var descriptor = avatar.GetComponent<VRCAvatarDescriptor>();
        if (descriptor == null)
            throw new InvalidOperationException(
                "Final FullFT Avatar has no VRCAvatarDescriptor.");
        var pipeline = avatar.GetComponent<PipelineManager>();
        if (pipeline == null)
            throw new InvalidOperationException(
                "Final FullFT Avatar has no PipelineManager.");
        if (avatar.transform.parent != null)
            throw new InvalidOperationException(
                "Final FullFT Avatar must be a scene root object.");
        var prefabPath =
            PrefabUtility.GetPrefabAssetPathOfNearestInstanceRoot(avatar);
        if (!string.Equals(
                prefabPath,
                FullFtPrefabPath,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Final diagnostic Avatar must be an instance of " +
                FullFtPrefabPath + "; found " +
                (string.IsNullOrEmpty(prefabPath)
                    ? "<not a prefab instance>"
                    : prefabPath) + ".");
        RequireFullFtTarget(avatar);
        RequireExpectedRootName(avatar, "final diagnostic scene");
        if (requireDiagnosticIdentity &&
            !string.Equals(
                pipeline.blueprintId,
                DiagnosticAvatarId,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Final diagnostic Avatar is not bound to " +
                DiagnosticAvatarId + ".");
        var stores = avatar.GetComponentsInChildren<ParticleSystem>(true)
            .Where(item => item.name == "Store")
            .ToArray();
        if (stores.Length != 1 ||
            stores[0].main.maxParticles != ExpectedCapacity ||
            !float.IsPositiveInfinity(
                stores[0].main.startLifetime.constant))
            throw new InvalidOperationException(
                "Final FullFT Store must be exactly one 16384-particle " +
                "Infinity-lifetime system.");
        var material = AssetDatabase.LoadAssetAtPath<Material>(
            MaterialPath);
        var renderer = stores[0].GetComponent<ParticleSystemRenderer>();
        if (renderer == null || renderer.sharedMaterial != material)
            throw new InvalidOperationException(
                "Final FullFT Store does not use the production material.");
        if (avatar.GetComponentsInChildren<Transform>(true)
            .Count(item => item.name == "MorphForgeGeometry") != 1)
            throw new InvalidOperationException(
                "Final FullFT must have exactly one MorphForgeGeometry.");
        var missingScripts = avatar
            .GetComponentsInChildren<Transform>(true)
            .Sum(item =>
                GameObjectUtility.
                    GetMonoBehavioursWithMissingScriptCount(
                        item.gameObject));
        if (missingScripts != 0)
            throw new InvalidOperationException(
                "Final FullFT Avatar has " + missingScripts +
                " missing scripts.");
        var collisionForceSystems =
            ParticleCollisionForceViolations(avatar);
        if (collisionForceSystems.Length != 0)
            throw new InvalidOperationException(
                "VRChat forbids particle systems applying collision force " +
                "to Pickup/Item layers. Found: " +
                string.Join(", ", collisionForceSystems));
    }

    private static string[] ParticleCollisionForceViolations(
        GameObject avatar)
    {
        if (avatar == null)
            return new string[0];
        var forbiddenMask =
            ParticleCollisionForceForbiddenMask();
        return avatar
            .GetComponentsInChildren<ParticleSystem>(true)
            .Where(item =>
            {
                var collision = item.collision;
                return collision.type ==
                           ParticleSystemCollisionType.World &&
                       !Mathf.Approximately(
                           collision.colliderForce,
                           0f) &&
                       (collision.collidesWith.value & forbiddenMask) != 0;
            })
            .Select(item => HierarchyPath(item.transform))
            .ToArray();
    }

    private static int ParticleCollisionForceForbiddenMask()
    {
        return VRC.Utility.LayerHelper
            .Mask_ParticleCollisionForceForbiddenLayerMask;
    }

    private static string HierarchyPath(Transform transform)
    {
        var names = new List<string>();
        for (var current = transform; current != null;
             current = current.parent)
            names.Add(current.name);
        names.Reverse();
        return string.Join("/", names);
    }

    private static void ValidatePreparedEvidence(GameObject avatar)
    {
        if (!File.Exists(Absolute(PreparedReportPath)))
            throw new InvalidOperationException(
                "Run PrepareDiagnosticScene and preflight before upload.");
        var scene = SceneManager.GetSceneByPath(DiagnosticScenePath);
        if (!scene.IsValid() || !scene.isLoaded || scene.isDirty)
            throw new InvalidOperationException(
                "The prepared diagnostic scene must remain loaded and clean.");
        if (avatar == null)
            avatar = SceneComponents<VRCAvatarDescriptor>(scene)
                .Single().gameObject;
        ValidateFinalAvatar(avatar, true);
        var preflight = JObject.Parse(
            File.ReadAllText(Absolute(PreparedReportPath)));
        if (preflight.Value<bool?>("success") != true ||
            !string.Equals(
                preflight.Value<string>("targetScene"),
                DiagnosticScenePath,
                StringComparison.Ordinal) ||
            !string.Equals(
                preflight.Value<string>("targetSceneSha256"),
                Sha256File(Absolute(DiagnosticScenePath)),
                StringComparison.OrdinalIgnoreCase) ||
            !string.Equals(
                preflight.Value<string>("avatarPrefab"),
                FullFtPrefabPath,
                StringComparison.Ordinal) ||
            !string.Equals(
                preflight.Value<string>("avatarPrefabSha256"),
                Sha256File(Absolute(FullFtPrefabPath)),
                StringComparison.OrdinalIgnoreCase) ||
            !string.Equals(
                preflight.Value<string>("targetAvatarId"),
                DiagnosticAvatarId,
                StringComparison.Ordinal) ||
            !string.Equals(
                preflight.Value<string>("rootNameGate"),
                RootNameGateName,
                StringComparison.Ordinal) ||
            !string.Equals(
                preflight.Value<string>("sourceSceneRootName"),
                GetFullFtRootName(),
                StringComparison.Ordinal) ||
            !string.Equals(
                preflight.Value<string>("diagnosticSceneRootName"),
                avatar.name,
                StringComparison.Ordinal) ||
            !string.Equals(
                preflight.Value<string>("sourceSceneSha256"),
                Sha256File(Absolute(FullFtScenePath)),
                StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException(
                "Prepared evidence no longer matches the live scene, source " +
                "scene, or FullFT prefab.");
        var currentEvidence = CollectEvidence(true);
        WriteReport(
            "Docs/Security/final-fullft-upload-preflight.json",
            JsonConvert.SerializeObject(
                currentEvidence,
                Formatting.Indented));
    }

    private static void ReloadDiagnosticSceneIfTransientlyDirty()
    {
        var scene = SceneManager.GetSceneByPath(DiagnosticScenePath);
        if (!scene.IsValid() || !scene.isLoaded || !scene.isDirty)
            return;
        if (!File.Exists(Absolute(PreparedReportPath)))
            throw new InvalidOperationException(
                "Diagnostic scene is dirty and prepared evidence is missing; " +
                "refusing to discard it.");
        var prepared = JObject.Parse(
            File.ReadAllText(Absolute(PreparedReportPath)));
        var expectedSceneHash =
            prepared.Value<string>("targetSceneSha256");
        var diskSceneHash =
            Sha256File(Absolute(DiagnosticScenePath));
        if (string.IsNullOrEmpty(expectedSceneHash) ||
            !string.Equals(
                expectedSceneHash,
                diskSceneHash,
                StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException(
                "Diagnostic scene is dirty and its disk hash no longer " +
                "matches prepared evidence; refusing to discard it.");
        EditorSceneManager.OpenScene(
            DiagnosticScenePath,
            OpenSceneMode.Single);
        SceneManager.SetActiveScene(
            SceneManager.GetSceneByPath(DiagnosticScenePath));
        var state = ReadStateOrNull();
        if (state != null)
        {
            state["reloadedDiagnosticScene"] = true;
            state["reloadReason"] =
                "Discarded transient SDK/editor dirty state after proving " +
                "that the disk hash still matches preparation.";
            WriteState(state);
        }
    }

    private static FullFTSyncProfile LoadProfileOrNull()
    {
        return AssetDatabase.LoadAssetAtPath<FullFTSyncProfile>(
            FullFTSyncProfile.DefaultAssetPath);
    }

    private static FullFTSyncProfile LoadProfileAndValidate()
    {
        var profile = LoadProfileOrNull();
        if (profile == null)
            return null;
        if (profile.schemaVersion != FullFTSyncProfile.CurrentSchemaVersion ||
            !profile.enabled ||
            !string.Equals(
                profile.avatarNamePrefix,
                FullFTSyncProfile.DefaultAvatarNamePrefix,
                StringComparison.Ordinal) ||
            profile.expectedFtSyncedBits != 161 ||
            profile.expectedFtSyncedParameters != 112 ||
            profile.expectedVrcftControlParameters != 8 ||
            profile.expectedGeometryBits != 56 ||
            profile.expectedPlacementBits != 14 ||
            profile.maxFinalBits != 256 ||
            !profile.requireExactFtManifest ||
            !profile.forceGeometryDirect ||
            !profile.forcePlacementDirect)
            return null;
        return profile;
    }

    private static string GetFullFtRootName()
    {
        var profile = LoadProfileAndValidate();
        if (profile == null)
            throw new InvalidOperationException(
                "FullFT sync profile is missing.");
        if (string.IsNullOrWhiteSpace(profile.avatarNamePrefix))
            throw new InvalidOperationException(
                "FullFT sync profile has no Avatar name prefix.");
        return profile.avatarNamePrefix + ExpectedDiagnosticRootSuffix;
    }

    private static void RequireFullFtTarget(GameObject avatar)
    {
        var profile = LoadProfileAndValidate();
        if (profile == null)
            throw new InvalidOperationException(
                "FullFT sync profile is missing or not in the tested budget " +
                "configuration.");
        if (!IsFullFtTarget(avatar, profile))
            throw new InvalidOperationException(
                "Avatar root '" + (avatar ? avatar.name : "<null>") +
                "' is not recognized by the production FullFT root-name " +
                "predicate " +
                "for prefix '" + profile.avatarNamePrefix + "'.");
    }

    private static void RequireExpectedRootName(
        GameObject avatar,
        string label)
    {
        var expected = GetFullFtRootName();
        if (avatar == null ||
            !string.Equals(
                avatar.name,
                expected,
                StringComparison.Ordinal))
            throw new InvalidOperationException(
                "Avatar root in " + label + " must be exactly '" +
                expected + "'; found '" +
                (avatar ? avatar.name : "<null>") + "'.");
    }

    private static bool IsFullFtTarget(
        GameObject avatar,
        FullFTSyncProfile profile)
    {
        return avatar != null &&
               profile != null &&
               !string.IsNullOrEmpty(profile.avatarNamePrefix) &&
               avatar.name.StartsWith(
                   profile.avatarNamePrefix,
                   StringComparison.Ordinal);
    }

    private static GameObject SingleAvatar(
        Scene scene,
        string label)
    {
        if (!scene.IsValid() || !scene.isLoaded)
            throw new InvalidOperationException(
                "Scene is not loaded: " + label + ".");
        var avatars = SceneComponents<VRCAvatarDescriptor>(scene);
        if (avatars.Length != 1)
            throw new InvalidOperationException(
                "Expected exactly one Avatar in " + label + "; found " +
                avatars.Length + ".");
        return avatars[0].gameObject;
    }

    private static void RequireSceneRoot(
        GameObject avatar,
        string label)
    {
        if (avatar.transform.parent != null)
            throw new InvalidOperationException(
                "Avatar must be a scene root object in " + label + ".");
    }

    private static string SceneRootName(string scenePath)
    {
        var scene = SceneManager.GetSceneByPath(scenePath);
        return SingleAvatar(scene, scenePath).name;
    }

    private static async void RunUploadAsync(
        IVRCSdkAvatarBuilderApi builder,
        VRCAvatar remote,
        string operationId)
    {
        var progress = OperationProgress.Start(builder);
        GameObject avatar = null;
        try
        {
            ReloadDiagnosticSceneIfTransientlyDirty();
            avatar = SceneComponents<VRCAvatarDescriptor>(
                SceneManager.GetSceneByPath(DiagnosticScenePath))
                .Single().gameObject;
            ValidatePreparedEvidence(avatar);
            builder.SelectAvatar(avatar);
            await builder.BuildAndUpload(
                avatar,
                remote,
                null,
                _activeCancellation.Token);
            var refreshed = await VRCApi.GetAvatar(
                DiagnosticAvatarId,
                forceRefresh: true);
            var state = ReadStateOrNull() ?? BaseState(operationId, avatar);
            state["status"] = "success";
            state["phase"] = "complete";
            state["completedAtUtc"] = DateTime.UtcNow.ToString("O");
            state["avatarVersion"] = refreshed.Version;
            state["pendingUpload"] = refreshed.PendingUpload;
            state["releaseStatus"] = refreshed.ReleaseStatus;
            state["latestAssetUrl"] =
                refreshed.GetLatestAssetUrlForCurrentPlatform();
            WriteState(state);
            WriteReport(
                "Docs/Security/final-fullft-upload.json",
                JsonConvert.SerializeObject(
                    state,
                    Formatting.Indented));
        }
        catch (Exception error)
        {
            var state = ReadStateOrNull() ?? BaseState(operationId, avatar);
            state["status"] = "failure";
            state["phase"] = "failed";
            state["error"] = error.ToString();
            WriteState(state);
            WriteReport(
                "Docs/Security/final-fullft-upload.json",
                JsonConvert.SerializeObject(
                    state,
                    Formatting.Indented));
        }
        finally
        {
            progress.Dispose();
            lock (OperationLock)
            {
                if (string.Equals(
                        _activeOperationId,
                        operationId,
                        StringComparison.Ordinal))
                {
                    _activeBuilder = null;
                    _activeCancellation?.Dispose();
                    _activeCancellation = null;
                    _activeOperationId = null;
                }
            }
            AssetDatabase.Refresh();
        }
    }

    private sealed class OperationProgress : IDisposable
    {
        private readonly IVRCSdkAvatarBuilderApi _builder;
        private bool _disposed;

        private OperationProgress(
            IVRCSdkAvatarBuilderApi builder)
        {
            _builder = builder;
        }

        public static OperationProgress Start(
            IVRCSdkAvatarBuilderApi builder)
        {
            var progress = new OperationProgress(builder);
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

        private void Update(string phase, string text, float? fraction)
        {
            var state = ReadStateOrNull();
            if (state == null)
                return;
            state["phase"] = phase;
            state["message"] = text;
            state["progressText"] = text;
            state["progressFraction"] = fraction;
            state["updatedAtUtc"] = DateTime.UtcNow.ToString("O");
            WriteState(state);
        }

        private void OnBuildStart(object sender, object target) =>
            Update("build", "SDK build started.", null);

        private void OnBuildProgress(object sender, string text) =>
            Update("build", text, null);

        private void OnBuildSuccess(object sender, string path)
        {
            var state = ReadStateOrNull();
            if (state != null)
            {
                state["bundlePath"] = path;
                state["bundleExists"] = File.Exists(path);
                state["bundleBytes"] = File.Exists(path)
                    ? new FileInfo(path).Length
                    : 0;
                state["bundleSha256"] = File.Exists(path)
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
    }

    private static Dictionary<string, object> BaseState(
        string operationId,
        GameObject avatar)
    {
        return new Dictionary<string, object>
        {
            ["operationId"] = operationId,
            ["targetAvatarId"] = DiagnosticAvatarId,
            ["targetName"] = avatar ? avatar.name : null,
            ["avatarRootName"] = avatar ? avatar.name : null,
            ["rootNameGate"] = RootNameGateName,
            ["scenePath"] = DiagnosticScenePath,
            ["sceneSha256"] = Sha256File(Absolute(DiagnosticScenePath)),
            ["prefabSha256"] = Sha256File(Absolute(FullFtPrefabPath)),
            ["startedAtUtc"] = DateTime.UtcNow.ToString("O")
        };
    }

    private static async Task<Dictionary<string, object>> CurrentAccount()
    {
        if (APIUser.CurrentUser == null && APIUser.IsLoggedIn)
            APIUser.InitialFetchCurrentUser(_ => { }, null);
        else if (APIUser.CurrentUser == null && ApiCredentials.Load())
            APIUser.InitialFetchCurrentUser(_ => { }, null);
        var deadline = DateTime.UtcNow.AddSeconds(15);
        while (APIUser.CurrentUser == null &&
               DateTime.UtcNow < deadline)
            await Task.Delay(250);
        var user = APIUser.CurrentUser;
        return new Dictionary<string, object>
        {
            ["id"] = user?.id,
            ["canPublishAvatars"] = user?.canPublishAvatars ?? false
        };
    }

    private static IVRCSdkAvatarBuilderApi RequireBuilder(bool open)
    {
        if (!VRCSdkControlPanel.TryGetBuilder(
                out IVRCSdkAvatarBuilderApi builder))
        {
            if (!open)
                throw new InvalidOperationException(
                    "VRChat SDK avatar builder is unavailable.");
            VRCSdkControlPanel panel =
                EditorWindow.GetWindow<VRCSdkControlPanel>(
                    false,
                    "VRChat SDK",
                    true);
            panel.Show();
            if (!VRCSdkControlPanel.TryGetBuilder(out builder))
                throw new InvalidOperationException(
                    "VRChat SDK avatar builder is unavailable.");
        }
        if (builder.BuildState != SdkBuildState.Idle ||
            builder.UploadState != SdkUploadState.Idle)
            throw new InvalidOperationException(
                "VRChat SDK is busy.");
        return builder;
    }

    private static void RequireIdle()
    {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling ||
            EditorApplication.isUpdating)
            throw new InvalidOperationException(
                "FullFT publishing requires idle Edit Mode.");
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
        const string prefix = "/file_";
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

    private static JObject ReadJson(
        string assetPath,
        List<string> failures,
        string label)
    {
        var absolute = Absolute(assetPath);
        if (!File.Exists(absolute))
        {
            failures.Add(label + " is missing: " + assetPath);
            return null;
        }
        try
        {
            return JObject.Parse(File.ReadAllText(absolute));
        }
        catch (Exception error)
        {
            failures.Add(label + " is invalid JSON: " + error.Message);
            return null;
        }
    }

    private static void Require(
        JObject report,
        string property,
        object expected,
        List<string> failures,
        string label)
    {
        var actual = report[property];
        if (actual == null ||
            !JToken.DeepEquals(actual, JToken.FromObject(expected)))
            failures.Add(label + " expected " + expected + ", found " +
                         (actual?.ToString() ?? "<missing>") + ".");
    }

    private static Dictionary<string, object> FileRecord(string path)
    {
        var absolute = Absolute(path);
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

    private static string EvidenceFileHash(
        Dictionary<string, object> files,
        string path)
    {
        if (files == null ||
            !files.TryGetValue(path, out var value) ||
            !(value is Dictionary<string, object> record) ||
            !(record["sha256"] is string hash))
            return null;
        return hash;
    }

    private static string Sha256File(string path)
    {
        using (var stream = File.OpenRead(path))
        using (var sha = SHA256.Create())
            return string.Concat(
                sha.ComputeHash(stream)
                    .Select(value => value.ToString("x2")));
    }

    private static string Absolute(string assetPath)
    {
        return Path.GetFullPath(
            Path.Combine(Application.dataPath, "..", assetPath));
    }

    private static void WriteReport(string relativePath, string json)
    {
        var absolute = Absolute(relativePath);
        Directory.CreateDirectory(Path.GetDirectoryName(absolute));
        File.WriteAllText(absolute, json + Environment.NewLine);
    }

    private static Dictionary<string, object> ReadStateOrNull()
    {
        try
        {
            return File.Exists(StatePath)
                ? JsonConvert.DeserializeObject<
                    Dictionary<string, object>>(File.ReadAllText(StatePath))
                : null;
        }
        catch
        {
            return null;
        }
    }

    private static void WriteState(Dictionary<string, object> state)
    {
        var directory = Path.GetDirectoryName(StatePath);
        Directory.CreateDirectory(directory);
        var temporary = StatePath + ".tmp";
        File.WriteAllText(
            temporary,
            JsonConvert.SerializeObject(state, Formatting.Indented));
        System.IO.File.Copy(temporary, StatePath, true);
        System.IO.File.Delete(temporary);
    }

    private static void EnsureFolder(string path)
    {
        var parent = "Assets";
        foreach (var part in path.Split('/').Skip(1))
        {
            var next = parent + "/" + part;
            if (!AssetDatabase.IsValidFolder(next))
                AssetDatabase.CreateFolder(parent, part);
            parent = next;
        }
    }

    private static T[] SceneComponents<T>(Scene scene)
        where T : Component
    {
        return scene.GetRootGameObjects()
            .SelectMany(root =>
                root.GetComponentsInChildren<T>(true))
            .ToArray();
    }

}
