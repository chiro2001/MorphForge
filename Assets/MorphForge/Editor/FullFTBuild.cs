using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using BlackStartX.GestureManager;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase.Editor.BuildPipeline;

namespace MorphForge.Editor
{
    public static class FullFTBuild
    {
        public const string FullFTRoot = "Assets/MorphForge/FullFT";
        public const string TestAvatarPath = FullFTRoot + "/Avatar.prefab";
        public const string ScenePath = FullFTRoot + "/MorphForge_FullFT.unity";
        public const string BaseAvatarPath = "Assets/Nemesis/Nemesis_Full_FT.prefab";
        public const string PlacementPath =
            "Assets/MorphForge/Stage4/Placement/Placement.prefab";
        public const string VrcftPrefabPath =
            "Assets/MoeWhite/VRCFT/Prefab/VRCFT_UE_Nemesis_VRCFur.prefab";
        public const string MixExpressionsPath =
            "Assets/MoeWhite/VRCFT/Prefab/Optional/MixExpressions.prefab";
        public const string FtMeshPath = "Assets/MoeWhite/VRCFT/FBX/Nemesis_FT.fbx";
        public const string OriginalLocomotionPath =
            "Assets/Nemesis/Nemesis_Avatar/Controller/Nemesis_Locomotion.controller";
        public const string NoSleepLocomotionPath =
            FullFTRoot + "/Nemesis_Locomotion_NoSleep.controller";
        private const string JointBuildReportPath = "Docs/FullFT/joint-build.json";
        private const string FinalBudgetReportPath = "Docs/FullFT/final-budget.json";

        public static string EnsureProfile()
        {
            EnsureFolder(FullFTRoot);
            var profile = AssetDatabase.LoadAssetAtPath<FullFTSyncProfile>(
                FullFTSyncProfile.DefaultAssetPath);
            if (!profile)
            {
                profile = FullFTSyncProfile.CreateDefault();
                AssetDatabase.CreateAsset(profile, FullFTSyncProfile.DefaultAssetPath);
            }
            else if (profile.schemaVersion != FullFTSyncProfile.CurrentSchemaVersion)
            {
                throw new InvalidOperationException(
                    $"FullFT profile schema is {profile.schemaVersion}; expected " +
                    $"{FullFTSyncProfile.CurrentSchemaVersion}.");
            }

            EditorUtility.SetDirty(profile);
            AssetDatabase.SaveAssetIfDirty(profile);
            var report = new
            {
                success = true,
                asset = FullFTSyncProfile.DefaultAssetPath,
                profile.schemaVersion,
                profile.profileVersion,
                profile.avatarNamePrefix,
                profile.expectedFtSyncedBits,
                profile.expectedFtSyncedParameters,
                controls = FullFTSyncProfile.VrcftControlParameters,
                profile.disableGroups,
                profile.preserveUnknownSync,
            };
            FullFTBudgetUtil.WriteReport("Docs/FullFT/profile.json", report);
            return Newtonsoft.Json.JsonConvert.SerializeObject(report);
        }

        public static string ResetProfile()
        {
            EnsureFolder(FullFTRoot);
            var profile = AssetDatabase.LoadAssetAtPath<FullFTSyncProfile>(
                FullFTSyncProfile.DefaultAssetPath);
            if (!profile)
            {
                profile = FullFTSyncProfile.CreateDefault();
                AssetDatabase.CreateAsset(profile, FullFTSyncProfile.DefaultAssetPath);
            }
            else
            {
                profile.ResetToDefaults();
                EditorUtility.SetDirty(profile);
            }
            AssetDatabase.SaveAssetIfDirty(profile);
            var result = new
            {
                success = true,
                action = "reset-to-defaults",
                asset = FullFTSyncProfile.DefaultAssetPath,
                profileVersion = profile.profileVersion,
            };
            FullFTBudgetUtil.WriteReport("Docs/FullFT/profile-reset.json", result);
            return Newtonsoft.Json.JsonConvert.SerializeObject(result);
        }

        public static string Inspect()
        {
            var profile = FullFTBudgetUtil.LoadProfile();
            object profileReport = null;
            if (profile)
            {
                profileReport = new
                {
                    asset = FullFTSyncProfile.DefaultAssetPath,
                    profile.profileVersion,
                    profile.enabled,
                    profile.avatarNamePrefix,
                    profile.expectedFtSyncedBits,
                    profile.expectedFtSyncedParameters,
                    profile.expectedVrcftControlParameters,
                    profile.expectedGeometryBits,
                    profile.expectedPlacementBits,
                    profile.maxFinalBits,
                    profile.preserveUnknownSync,
                };
            }
            var dependencies = new
            {
                baseAvatar = Exists(BaseAvatarPath),
                ftMesh = Exists(FtMeshPath),
                vrcftInjection = Exists(VrcftPrefabPath),
                mixExpressions = Exists(MixExpressionsPath),
                morphForgePlacement = Exists(PlacementPath),
                vrcfury = AppDomain.CurrentDomain.GetAssemblies()
                    .Any(assembly => assembly.GetName().Name == "VRCFury"),
                modularAvatar = AppDomain.CurrentDomain.GetAssemblies()
                    .Any(assembly => assembly.GetName().Name == "nadena.dev.modular-avatar.core"),
            };
            var report = new
            {
                profile = profileReport,
                dependencies,
                testAvatar = Exists(TestAvatarPath),
                fullFtScene = Exists(ScenePath),
                jointBuildReport = ExistsOnDisk(JointBuildReportPath),
                finalBudgetReport = ExistsOnDisk(FinalBudgetReportPath),
                sceneBuildReport = ExistsOnDisk("Docs/FullFT/scene-build.json"),
            };
            FullFTBudgetUtil.WriteReport("Docs/FullFT/inspect.json", report);
            return Newtonsoft.Json.JsonConvert.SerializeObject(report);
        }

        /// <summary>
        /// Creates an independent prefab. The active scene and every source prefab
        /// remain untouched.
        /// </summary>
        public static string BuildTestAvatar()
        {
            if (EditorApplication.isPlaying || EditorApplication.isCompiling)
                throw new InvalidOperationException("FullFT asset build requires idle Edit Mode.");

            EnsureProfile();
            RequireAsset<GameObject>(BaseAvatarPath, "base FullFT avatar");
            RequireAsset<GameObject>(PlacementPath, "Stage4 MorphForge placement module");
            RequireAsset<GameObject>(VrcftPrefabPath, "VRCFT VRCFury injection prefab");
            RequireAsset<GameObject>(MixExpressionsPath, "MixExpressions prefab");
            RequireAsset<GameObject>(FtMeshPath, "FT skinned mesh FBX");

            EnsureFolder(FullFTRoot);
            var contents = PrefabUtility.LoadPrefabContents(BaseAvatarPath);
            Dictionary<string, object> noSleepCleanup = null;
            object particleCollisionCleanup = null;
            try
            {
                contents.name = "MorphForge_FullFT_Avatar";
                var pipeline = contents.GetComponent<VRC.Core.PipelineManager>();
                if (pipeline) pipeline.blueprintId = string.Empty;

                foreach (var child in contents.transform.Cast<Transform>().ToArray())
                {
                    if (child.name == "VRCFT_UE_Nemesis_VRCFur" ||
                        child.name == "MorphForgePlacement" ||
                        child.name == "Placement" ||
                        child.name == "MixExpressions")
                        UnityEngine.Object.DestroyImmediate(child.gameObject);
                }

                InstantiateChild(VrcftPrefabPath, contents.transform, "VRCFT_UE_Nemesis_VRCFur");
                InstantiateChild(MixExpressionsPath, contents.transform, "MixExpressions");
                InstantiateChild(PlacementPath, contents.transform, "MorphForgePlacement");
                noSleepCleanup = FullFTNoSleepController.Apply(contents);
                particleCollisionCleanup = SanitizePickupParticleForces(contents);

                var ftRenderer = contents.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                    .FirstOrDefault(renderer => renderer.sharedMesh &&
                                                AssetDatabase.GetAssetPath(renderer.sharedMesh)
                                                    .EndsWith("Nemesis_FT.fbx",
                                                        StringComparison.OrdinalIgnoreCase));
                if (!ftRenderer)
                    throw new InvalidOperationException(
                        "After dependency import, the base avatar still has no Nemesis_FT.fbx " +
                        "SkinnedMeshRenderer. Do not continue to joint build.");

                var missingScripts = contents.GetComponentsInChildren<Transform>(true)
                    .Sum(transform => GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(
                        transform.gameObject));
                if (missingScripts != 0)
                    throw new InvalidOperationException(
                        $"FullFT test avatar has {missingScripts} missing scripts. " +
                        "Install the required packages and rerun the dependency import.");

                PrefabUtility.SaveAsPrefabAsset(contents, TestAvatarPath);
            }
            finally
            {
                PrefabUtility.UnloadPrefabContents(contents);
            }

            AssetDatabase.ImportAsset(TestAvatarPath, ImportAssetOptions.ForceUpdate);
            var report = new
            {
                success = true,
                avatar = TestAvatarPath,
                baseAvatar = BaseAvatarPath,
                injections = new[] { VrcftPrefabPath, MixExpressionsPath, PlacementPath },
                ftMesh = FtMeshPath,
                derivedLocomotion = NoSleepLocomotionPath,
                noSleepCleanup,
                particleCollisionCleanup,
                excluded = new[]
                {
                    "outfits",
                    "sleep/Supine plugin states, clips and parameters",
                    "VRCFury generated build directory",
                },
                missingScripts = 0,
                activeSceneWritten = false,
                sourcePrefabWritten = false,
            };
            FullFTBudgetUtil.WriteReport("Docs/FullFT/avatar-build.json", report);
            return Newtonsoft.Json.JsonConvert.SerializeObject(report);
        }

        /// <summary>
        /// SDK validation rejects particles that apply collision forces to pickup layers.
        /// Apply only to our final avatar copy; retain particle visuals and collision events.
        /// </summary>
        public static object SanitizePickupParticleForces(GameObject avatar)
        {
            if (!avatar) throw new ArgumentNullException(nameof(avatar));
            int pickupLayers = VRC.Utility.LayerHelper.Mask_ParticleCollisionForceForbiddenLayerMask;
            var changes = new List<object>();
            foreach (var system in avatar.GetComponentsInChildren<ParticleSystem>(true))
            {
                var collision = system.collision;
                if (collision.type != ParticleSystemCollisionType.World ||
                    collision.colliderForce == 0f || (collision.collidesWith.value & pickupLayers) == 0)
                    continue;
                changes.Add(new
                {
                    path = AnimationUtility.CalculateTransformPath(system.transform, avatar.transform),
                    forceBefore = collision.colliderForce,
                    forceAfter = 0f,
                    collisionEnabled = collision.enabled,
                    collisionMask = collision.collidesWith.value
                });
                collision.colliderForce = 0f;
                EditorUtility.SetDirty(system);
                if (PrefabUtility.IsPartOfPrefabInstance(system))
                    PrefabUtility.RecordPrefabInstancePropertyModifications(system);
            }
            return new { changed = changes.Count, changes, sourceNemesisAssetsWritten = false };
        }

        /// <summary>
        /// Runs the real VRChat SDK avatar preprocess callback chain on a temporary
        /// clone. This includes MA/NDMF, VRCFury, the Max-200 sync override,
        /// VRCFury Parameter Compressor and the Max-50 final budget gate.
        /// It does not upload and does not modify the active scene.
        /// </summary>
        public static string JointBuild()
        {
            if (EditorApplication.isPlaying || EditorApplication.isCompiling)
                throw new InvalidOperationException(
                    "FullFT joint build requires idle Edit Mode. Exit the long Play test first.");
            EnsureProfile();
            var prefab = RequireAsset<GameObject>(TestAvatarPath, "FullFT test avatar");
            ValidateNoSleepAvatar(prefab);
            DeleteReport(JointBuildReportPath);
            var clone = UnityEngine.Object.Instantiate(prefab);
            clone.name = "MorphForge_FullFT_Avatar_Build";
            clone.hideFlags = HideFlags.HideAndDontSave;
            try
            {
                var success = VRCBuildPipelineCallbacks.OnPreprocessAvatar(clone);
                if (!success)
                    throw new InvalidOperationException(
                        "VRChat SDK preprocess callback chain returned false. " +
                        "Read Docs/FullFT/pre-compression.json, final-budget.json and the Console.");

                var descriptor = clone.GetComponent<VRCAvatarDescriptor>();
                var finalCost = descriptor?.expressionParameters?.CalcTotalCost() ?? -1;
                var report = new
                {
                    success = true,
                    scope = "VRChat SDK preprocess callback chain, no upload",
                    avatar = TestAvatarPath,
                    completedAtUtc = DateTime.UtcNow.ToString("O"),
                    prefabSha256 = Sha256File(AssetPathToAbsolute(TestAvatarPath)),
                    finalBits = finalCost,
                    preCompression = "Docs/FullFT/pre-compression.json",
                    finalBudget = "Docs/FullFT/final-budget.json",
                    finalBudgetSha256 = ExistsOnDisk(FinalBudgetReportPath)
                        ? Sha256File(AssetPathToAbsolute(FinalBudgetReportPath))
                        : string.Empty,
                    activeSceneWritten = false,
                    sourcePrefabWritten = false,
                };
                FullFTBudgetUtil.WriteReport(JointBuildReportPath, report);
                return Newtonsoft.Json.JsonConvert.SerializeObject(report);
            }
            catch (Exception exception)
            {
                FullFTBudgetUtil.WriteReport("Docs/FullFT/joint-build-error.json", new
                {
                    success = false,
                    scope = "VRChat SDK preprocess callback chain, no upload",
                    completedAtUtc = DateTime.UtcNow.ToString("O"),
                    avatar = TestAvatarPath,
                    error = exception.ToString(),
                    activeSceneWritten = false,
                    sourcePrefabWritten = false,
                });
                throw;
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(clone);
            }
        }

        /// <summary>
        /// Creates an independent review scene after a successful JointBuild of the
        /// exact current FullFT prefab. The active source scene must be saved and
        /// clean. A copy is edited through Unity scene and prefab APIs; the source
        /// scene, its open tab and the prefab assets are never saved or replaced.
        /// </summary>
        public static string CreateScene()
        {
            if (EditorApplication.isPlaying || EditorApplication.isCompiling)
                return SkipScene(
                    "edit-mode-required",
                    "请先退出 Play Mode 并等待脚本编译完成。");

            var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(TestAvatarPath);
            if (!prefab)
                return SkipScene(
                    "prefab-missing",
                    $"缺少 {TestAvatarPath}；先执行 BuildTestAvatar。");
            if (!TryReadJointBuildProof(prefab, out var proof, out var proofError))
                return SkipScene(
                    "joint-build-proof-invalid",
                    $"{proofError} 请对当前 prefab 重新执行 JointBuild。");
            if (AssetDatabase.LoadAssetAtPath<SceneAsset>(ScenePath))
                return SkipScene(
                    "target-scene-exists",
                    $"目标场景已存在，未覆盖：{ScenePath}");

            var sourceScene = SceneManager.GetActiveScene();
            if (!sourceScene.IsValid() || string.IsNullOrEmpty(sourceScene.path))
                return SkipScene(
                    "source-scene-not-saved",
                    "当前活动场景没有磁盘路径；请先由用户保存源场景。");
            if (!sourceScene.path.StartsWith("Assets/", StringComparison.OrdinalIgnoreCase))
                return SkipScene(
                    "source-scene-not-project-asset",
                    $"源场景必须位于项目的 Assets 下，当前为：{sourceScene.path}");
            var sourceAbsolutePath = AssetPathToAbsolute(sourceScene.path);
            if (!File.Exists(sourceAbsolutePath))
                return SkipScene(
                    "source-scene-file-missing",
                    $"活动场景路径存在但文件不可读：{sourceAbsolutePath}");
            var sourceSceneHadUnsavedChanges = sourceScene.isDirty;
            if (string.Equals(sourceScene.path, ScenePath, StringComparison.OrdinalIgnoreCase))
                return SkipScene(
                    "source-is-target-scene",
                    "当前活动场景就是 FullFT 目标场景；请切回用户源场景。");

            var sourceAvatars = SceneComponents<VRCAvatarDescriptor>(sourceScene);
            if (sourceAvatars.Length != 1)
                return SkipScene(
                    "source-avatar-count",
                    $"源场景必须恰好有一个 VRCAvatarDescriptor，当前为 {sourceAvatars.Length}。");
            var sourceAvatar = sourceAvatars[0].gameObject;
            var sourceManagers = SceneComponents<GestureManager>(sourceScene);
            var sourceReceivers = SceneComponents<MorphForgePlayReceiver>(sourceScene);
            var sourceCameras = SceneComponents<Camera>(sourceScene);
            var sourceLights = SceneComponents<Light>(sourceScene);
            if (sourceManagers.Length != 1)
                return SkipScene(
                    "source-gesture-manager-count",
                    $"源场景必须恰好有一个 Gesture Manager，当前为 {sourceManagers.Length}。");
            if (sourceManagers[0].settings == null)
                return SkipScene(
                    "source-gesture-manager-settings-missing",
                    "Gesture Manager 缺少 settings，无法安全复用配置。");
            if (sourceReceivers.Length != 1)
                return SkipScene(
                    "source-receiver-count",
                    $"源场景必须恰好有一个 MorphForgePlayReceiver，当前为 {sourceReceivers.Length}。");
            if (sourceCameras.Length == 0 || sourceLights.Length == 0)
                return SkipScene(
                    "source-camera-or-light-missing",
                    $"源场景需要至少一个相机和一个灯光；当前相机 {sourceCameras.Length}、灯光 {sourceLights.Length}。");
            if (sourceManagers[0].transform.IsChildOf(sourceAvatar.transform))
                return SkipScene(
                    "gesture-manager-inside-avatar",
                    "Gesture Manager 位于旧 Avatar 内部，替换时无法复用。");
            if (sourceReceivers[0].transform.IsChildOf(sourceAvatar.transform))
                return SkipScene(
                    "receiver-inside-avatar",
                    "MorphForge 测试接收器位于旧 Avatar 内部，替换时无法复用。");

            EnsureFolder(FullFTRoot);
            var sourceHashBefore = Sha256File(sourceAbsolutePath);
            var tempScenePath =
                $"{FullFTRoot}/MorphForge_FullFT.__building_{Guid.NewGuid():N}.unity";
            Scene copiedScene = default;
            Scene finalScene = default;
            var copiedOpen = false;
            var finalSceneOpen = false;
            var moved = false;
            var finalValidated = false;
            try
            {
                if (!EditorSceneManager.SaveScene(sourceScene, tempScenePath, true))
                    throw new InvalidOperationException(
                        "Unity 未能创建源场景的独立副本。源场景未被保存或修改。");
                if (sourceSceneHadUnsavedChanges && !sourceScene.isDirty)
                    EditorSceneManager.MarkSceneDirty(sourceScene);
                copiedScene = EditorSceneManager.OpenScene(
                    tempScenePath, OpenSceneMode.Additive);
                copiedOpen = true;
                SceneManager.SetActiveScene(copiedScene);

                var copiedAvatar = SceneComponents<VRCAvatarDescriptor>(copiedScene)
                    .Single().gameObject;
                var copiedManager = SceneComponents<GestureManager>(copiedScene).Single();
                var copiedReceiver =
                    SceneComponents<MorphForgePlayReceiver>(copiedScene).Single();
                var oldParent = copiedAvatar.transform.parent;
                var oldSibling = copiedAvatar.transform.GetSiblingIndex();
                var localPosition = copiedAvatar.transform.localPosition;
                var localRotation = copiedAvatar.transform.localRotation;
                var localScale = copiedAvatar.transform.localScale;
                var worldPosition = copiedAvatar.transform.position;
                var worldRotation = copiedAvatar.transform.rotation;
                var worldScale = copiedAvatar.transform.lossyScale;
                var oldTag = copiedAvatar.tag;
                var oldLayer = copiedAvatar.layer;
                var oldActive = copiedAvatar.activeSelf;

                UnityEngine.Object.DestroyImmediate(copiedAvatar);
                var replacement = (GameObject)PrefabUtility.InstantiatePrefab(prefab);
                if (!replacement)
                    throw new InvalidOperationException(
                        "PrefabUtility 未能实例化 FullFT Avatar。");
                // A prefab named Avatar.prefab may instantiate as "Avatar".
                // The build profile intentionally opts in by root-name prefix.
                replacement.name = FullFTBudgetUtil.LoadProfile().avatarNamePrefix + "_Avatar";
                if (replacement.scene != copiedScene)
                    SceneManager.MoveGameObjectToScene(replacement, copiedScene);
                replacement.transform.SetParent(oldParent, false);
                replacement.transform.localPosition = localPosition;
                replacement.transform.localRotation = localRotation;
                replacement.transform.localScale = localScale;
                var maxSibling = oldParent
                    ? Math.Max(0, oldParent.childCount - 1)
                    : Math.Max(0, copiedScene.rootCount - 1);
                replacement.transform.SetSiblingIndex(
                    Math.Min(oldSibling, maxSibling));
                replacement.tag = oldTag;
                replacement.layer = oldLayer;
                replacement.SetActive(oldActive);
                ValidateWorldPose(
                    replacement.transform,
                    worldPosition,
                    worldRotation,
                    worldScale,
                    "副本中的 FullFT Avatar");

                var descriptor = replacement.GetComponent<VRCAvatarDescriptor>();
                if (!descriptor)
                    throw new InvalidOperationException(
                        $"{TestAvatarPath} 的根对象没有 VRCAvatarDescriptor。");
                copiedReceiver.Avatar = descriptor;
                copiedReceiver.ReceivePort = 9012;
                copiedReceiver.SendPort = 9013;
                copiedReceiver.AutoStart = true;
                copiedManager.settings.favourite = descriptor;
                copiedManager.Module = null;
                MarkSceneComponentDirty(copiedReceiver);
                MarkSceneComponentDirty(copiedManager);

                ValidateReviewScene(
                    copiedScene, replacement, copiedManager, copiedReceiver);
                EditorSceneManager.MarkSceneDirty(copiedScene);
                if (!EditorSceneManager.SaveScene(copiedScene))
                    throw new InvalidOperationException(
                        "FullFT 场景副本保存失败；最终目标场景未创建。");
                EditorSceneManager.CloseScene(copiedScene, true);
                copiedOpen = false;

                var moveError = AssetDatabase.MoveAsset(tempScenePath, ScenePath);
                if (!string.IsNullOrEmpty(moveError))
                    throw new InvalidOperationException(
                        $"无法将已验证场景副本移动到 {ScenePath}: {moveError}");
                moved = true;
                AssetDatabase.ImportAsset(
                    ScenePath, ImportAssetOptions.ForceUpdate);

                finalScene = EditorSceneManager.OpenScene(
                    ScenePath, OpenSceneMode.Additive);
                finalSceneOpen = true;
                SceneManager.SetActiveScene(finalScene);
                var finalAvatar = SceneComponents<VRCAvatarDescriptor>(finalScene)
                    .Single().gameObject;
                var finalManager = SceneComponents<GestureManager>(finalScene).Single();
                var finalReceiver =
                    SceneComponents<MorphForgePlayReceiver>(finalScene).Single();
                var finalDescriptor = ValidateReviewScene(
                    finalScene, finalAvatar, finalManager, finalReceiver);
                ValidateWorldPose(
                    finalAvatar.transform,
                    worldPosition,
                    worldRotation,
                    worldScale,
                    "最终 FullFT 场景中的 Avatar");
                var sourceAfter = SceneManager.GetSceneByPath(sourceScene.path);
                var sourceHashAfter = Sha256File(sourceAbsolutePath);
                if (sourceHashAfter != sourceHashBefore)
                    throw new InvalidOperationException(
                        "源场景文件在创建副本期间发生变化；入口未保存或丢弃源场景。");
                if (sourceAfter.IsValid() &&
                    sourceSceneHadUnsavedChanges &&
                    !sourceAfter.isDirty)
                    EditorSceneManager.MarkSceneDirty(sourceAfter);
                var sourceDirtyAfter = sourceAfter.IsValid() && sourceAfter.isDirty;
                if (sourceSceneHadUnsavedChanges &&
                    sourceAfter.IsValid() &&
                    !sourceDirtyAfter)
                    throw new InvalidOperationException(
                        "无法恢复源场景原有 dirty 状态；入口没有保存或丢弃源场景。");
                var sceneSha256 = Sha256File(AssetPathToAbsolute(ScenePath));

                var result = new
                {
                    success = true,
                    skipped = false,
                    scope =
                        "Independent FullFT review scene; no upload or VRChat launch",
                    sourceScene = sourceScene.path,
                    sourceSceneSha256 = sourceHashBefore,
                    sourceSceneHashUnchanged = true,
                    capturedUnsavedSource = sourceSceneHadUnsavedChanges,
                    sourceSceneSavedOrDiscarded = false,
                    sourceAvatarModified = false,
                    sourceSceneStillOpen = sourceAfter.IsValid(),
                    sourceSceneStillClean =
                        !sourceAfter.IsValid() || !sourceAfter.isDirty,
                    sourceSceneDirtyBefore = sourceSceneHadUnsavedChanges,
                    sourceSceneDirtyAfter = sourceDirtyAfter,
                    sourceSceneDirtyPreserved =
                        !sourceAfter.IsValid() ||
                        sourceSceneHadUnsavedChanges == sourceDirtyAfter,
                    targetScene = ScenePath,
                    targetSceneSha256 = sceneSha256,
                    avatar = TestAvatarPath,
                    avatarInstance = finalAvatar.name,
                    avatarTransformPreserved = new
                    {
                        before = new
                        {
                            position = Vec(worldPosition),
                            rotation = Quat(worldRotation),
                            scale = Vec(worldScale),
                        },
                        after = new
                        {
                            position = Vec(finalAvatar.transform.position),
                            rotation = Quat(finalAvatar.transform.rotation),
                            scale = Vec(finalAvatar.transform.lossyScale),
                        },
                    },
                    sceneAvatarCount = 1,
                    gestureManagerCount = 1,
                    gestureManagerFavouriteRedirected =
                        ReferenceEquals(finalManager.settings.favourite, finalDescriptor),
                    receiverCount = 1,
                    receiverAvatarRedirected =
                        ReferenceEquals(finalReceiver.Avatar, finalDescriptor),
                    receivePort = finalReceiver.ReceivePort,
                    sendPort = finalReceiver.SendPort,
                    receiverAutoStart = finalReceiver.AutoStart,
                    cameraCount = SceneComponents<Camera>(finalScene).Length,
                    lightCount = SceneComponents<Light>(finalScene).Length,
                    jointBuild = new
                    {
                        completedAtUtc = proof.Value<string>("completedAtUtc"),
                        prefabSha256 = proof.Value<string>("prefabSha256"),
                        finalBudgetSha256 = proof.Value<string>("finalBudgetSha256"),
                    },
                    autoUpload = false,
                    vrChatLaunched = false,
                    sourcePrefabWritten = false,
                    activeScene = SceneManager.GetActiveScene().path,
                };
                FullFTBudgetUtil.WriteReport(
                    "Docs/FullFT/scene-build.json", result);
                Selection.activeGameObject = finalAvatar;
                EditorGUIUtility.PingObject(finalAvatar);
                finalValidated = true;
                return Newtonsoft.Json.JsonConvert.SerializeObject(result);
            }
            catch (Exception exception)
            {
                FullFTBudgetUtil.WriteReport("Docs/FullFT/scene-build-error.json", new
                {
                    success = false,
                    skipped = false,
                    sourceScene = sourceScene.path,
                    sourceSceneWasSavedOrDiscarded = false,
                    targetScene = ScenePath,
                    error = exception.ToString(),
                });
                throw;
            }
            finally
            {
                if (copiedOpen && copiedScene.IsValid())
                    EditorSceneManager.CloseScene(copiedScene, true);
                if (!finalValidated && finalSceneOpen && finalScene.IsValid())
                    EditorSceneManager.CloseScene(finalScene, true);
                if (!moved && Exists(tempScenePath))
                    AssetDatabase.DeleteAsset(tempScenePath);
                if (!finalValidated && moved && Exists(ScenePath))
                    AssetDatabase.DeleteAsset(ScenePath);
                if (!finalValidated && sourceScene.IsValid())
                {
                    if (sourceSceneHadUnsavedChanges && !sourceScene.isDirty)
                        EditorSceneManager.MarkSceneDirty(sourceScene);
                    SceneManager.SetActiveScene(sourceScene);
                }
            }
        }

        private static bool TryReadJointBuildProof(
            GameObject prefab,
            out JObject proof,
            out string error)
        {
            proof = null;
            var absolute = AssetPathToAbsolute(JointBuildReportPath);
            if (!File.Exists(absolute))
            {
                error = "缺少 JointBuild 成功报告。";
                return false;
            }
            try
            {
                proof = JObject.Parse(File.ReadAllText(absolute));
            }
            catch (Exception exception)
            {
                error = $"JointBuild 报告无法解析：{exception.Message}";
                return false;
            }
            if (proof.Value<bool?>("success") != true)
            {
                error = "JointBuild 报告未标记 success=true。";
                return false;
            }
            if (!string.Equals(
                    proof.Value<string>("avatar"),
                    TestAvatarPath,
                    StringComparison.OrdinalIgnoreCase))
            {
                error = "JointBuild 报告不是针对当前 FullFT Avatar.prefab。";
                return false;
            }
            var prefabHash = proof.Value<string>("prefabSha256");
            var currentPrefabHash = Sha256File(AssetPathToAbsolute(TestAvatarPath));
            if (string.IsNullOrEmpty(prefabHash) ||
                !string.Equals(
                    prefabHash, currentPrefabHash, StringComparison.OrdinalIgnoreCase))
            {
                error =
                    "Avatar.prefab 在 JointBuild 后发生变化，或证明缺少 prefabSha256。";
                return false;
            }
            var budgetAbsolute = AssetPathToAbsolute(FinalBudgetReportPath);
            if (!File.Exists(budgetAbsolute))
            {
                error = "缺少最终同步预算报告。";
                return false;
            }
            try
            {
                var budget = JObject.Parse(File.ReadAllText(budgetAbsolute));
                if (budget.Value<bool?>("success") != true)
                {
                    error = "最终同步预算报告未标记 success=true。";
                    return false;
                }
            }
            catch (Exception exception)
            {
                error = $"最终同步预算报告无法解析：{exception.Message}";
                return false;
            }
            var recordedBudgetHash = proof.Value<string>("finalBudgetSha256");
            if (!string.IsNullOrEmpty(recordedBudgetHash) &&
                !string.Equals(
                    recordedBudgetHash,
                    Sha256File(budgetAbsolute),
                    StringComparison.OrdinalIgnoreCase))
            {
                error = "最终同步预算报告在 JointBuild 后发生变化。";
                return false;
            }
            if (!prefab)
            {
                error = "FullFT Avatar.prefab 未加载。";
                return false;
            }
            error = string.Empty;
            return true;
        }

        private static VRCAvatarDescriptor ValidateReviewScene(
            Scene scene,
            GameObject expectedAvatar,
            GestureManager manager,
            MorphForgePlayReceiver receiver)
        {
            var descriptors = SceneComponents<VRCAvatarDescriptor>(scene);
            var managers = SceneComponents<GestureManager>(scene);
            var receivers = SceneComponents<MorphForgePlayReceiver>(scene);
            if (descriptors.Length != 1 ||
                managers.Length != 1 ||
                receivers.Length != 1)
                throw new InvalidOperationException(
                    "FullFT 场景必须恰好有 1 Avatar / 1 GM / 1 Receiver；" +
                    $"当前 {descriptors.Length} / {managers.Length} / {receivers.Length}。");
            var descriptor = descriptors[0];
            if (descriptor.gameObject != expectedAvatar)
                throw new InvalidOperationException(
                    "FullFT 场景中的唯一 Avatar 不是新实例化的 Prefab。");
            if (!FullFTBudgetUtil.IsFullFTTarget(
                    descriptor.gameObject, FullFTBudgetUtil.LoadProfile()))
                throw new InvalidOperationException(
                    "FullFT Avatar 根名称不匹配同步 profile；实际 Play/上传将跳过参数精简。");
            var prefabPath =
                PrefabUtility.GetPrefabAssetPathOfNearestInstanceRoot(
                    descriptor.gameObject);
            if (!string.Equals(
                    prefabPath, TestAvatarPath, StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException(
                    $"Avatar 不是 {TestAvatarPath} 的 Prefab 实例，而是 '{prefabPath}'。");
            if (!descriptor.transform.Find("MorphForgePlacement"))
                throw new InvalidOperationException(
                    "FullFT Avatar 缺少 MorphForgePlacement 子对象，MF3 链路不完整。");
            var geometryModules = descriptor.GetComponentsInChildren<Transform>(true)
                .Where(transform => transform.name == "MorphForgeGeometry")
                .ToArray();
            if (geometryModules.Length != 1)
                throw new InvalidOperationException(
                    $"FullFT Avatar 必须恰好有一个 MorphForgeGeometry，当前 {geometryModules.Length}。");
            if (manager.settings == null)
                throw new InvalidOperationException("Gesture Manager settings 缺失。");
            if (!ReferenceEquals(manager.settings.favourite, descriptor))
                throw new InvalidOperationException(
                    "Gesture Manager favourite 未重定向到 FullFT Avatar。");
            if (!ReferenceEquals(receiver.Avatar, descriptor))
                throw new InvalidOperationException(
                    "MorphForgePlayReceiver.Avatar 未重定向到 FullFT Avatar。");
            if (receiver.ReceivePort != 9012 ||
                receiver.SendPort != 9013 ||
                !receiver.AutoStart)
                throw new InvalidOperationException(
                    $"接收器配置错误：receive={receiver.ReceivePort}, " +
                    $"send={receiver.SendPort}, autoStart={receiver.AutoStart}。");
            return descriptor;
        }

        private static void ValidateWorldPose(
            Transform transform,
            Vector3 expectedPosition,
            Quaternion expectedRotation,
            Vector3 expectedScale,
            string label)
        {
            var positionError = Vector3.Distance(
                transform.position, expectedPosition);
            var rotationError = Quaternion.Angle(
                transform.rotation, expectedRotation);
            var scaleError = Vector3.Distance(
                transform.lossyScale, expectedScale);
            if (positionError > 1e-4f ||
                rotationError > 1e-2f ||
                scaleError > 1e-4f)
            {
                throw new InvalidOperationException(
                    $"{label}未保持世界 Pose：positionError={positionError:R}, " +
                    $"rotationErrorDegrees={rotationError:R}, " +
                    $"scaleError={scaleError:R}");
            }
        }

        private static T[] SceneComponents<T>(Scene scene) where T : Component
        {
            return scene.GetRootGameObjects()
                .SelectMany(root => root.GetComponentsInChildren<T>(true))
                .ToArray();
        }

        private static void MarkSceneComponentDirty(Component component)
        {
            if (!component) return;
            EditorUtility.SetDirty(component);
            if (PrefabUtility.IsPartOfPrefabInstance(component))
                PrefabUtility.RecordPrefabInstancePropertyModifications(component);
        }

        private static string SkipScene(string reason, string detail)
        {
            var activeScene = SceneManager.GetActiveScene();
            var result = new
            {
                success = false,
                skipped = true,
                reason,
                detail,
                prefab = TestAvatarPath,
                prefabExists = Exists(TestAvatarPath),
                targetScene = ScenePath,
                targetSceneExists = Exists(ScenePath),
                sourceScene = activeScene.path,
                sourceSceneDirty = activeScene.IsValid() && activeScene.isDirty,
                sourceSceneSavedOrDiscarded = false,
                activeSceneWritten = false,
                sourcePrefabWritten = false,
                autoUpload = false,
                vrChatLaunched = false,
            };
            FullFTBudgetUtil.WriteReport("Docs/FullFT/scene-build.json", result);
            return Newtonsoft.Json.JsonConvert.SerializeObject(result);
        }

        private static string AssetPathToAbsolute(string assetPath)
        {
            return Path.Combine(
                FullFTBudgetUtil.ProjectRoot,
                assetPath.Replace('/', Path.DirectorySeparatorChar));
        }

        private static string Sha256File(string path)
        {
            if (!File.Exists(path)) return string.Empty;
            using (var stream = File.OpenRead(path))
            using (var sha = SHA256.Create())
                return BitConverter.ToString(sha.ComputeHash(stream))
                    .Replace("-", "");
        }

        private static void DeleteReport(string relativePath)
        {
            var absolute = AssetPathToAbsolute(relativePath);
            if (File.Exists(absolute)) File.Delete(absolute);
        }

        private static float[] Vec(Vector3 value)
        {
            return new[] { value.x, value.y, value.z };
        }

        private static float[] Quat(Quaternion value)
        {
            return new[] { value.x, value.y, value.z, value.w };
        }

        private static void InstantiateChild(string prefabPath, Transform parent, string name)
        {
            var prefab = RequireAsset<GameObject>(prefabPath, prefabPath);
            var instance = (GameObject)PrefabUtility.InstantiatePrefab(prefab, parent);
            instance.name = name;
            instance.transform.localPosition = Vector3.zero;
            instance.transform.localRotation = Quaternion.identity;
            instance.transform.localScale = Vector3.one;
        }

        private static void ValidateNoSleepAvatar(GameObject prefab)
        {
            var descriptor = prefab.GetComponent<VRCAvatarDescriptor>();
            if (!descriptor)
                throw new InvalidOperationException(
                    "FullFT test avatar has no VRCAvatarDescriptor.");
            if (!descriptor.baseAnimationLayers.Any(layer =>
                    AssetDatabase.GetAssetPath(layer.animatorController) ==
                    NoSleepLocomotionPath))
                throw new InvalidOperationException(
                    "FullFT test avatar does not use the generated NoSleep locomotion. " +
                    "Run BuildTestAvatar before JointBuild.");
            var forbidden = AssetDatabase.GetDependencies(TestAvatarPath, true)
                .Where(path => path.StartsWith(
                    "Packages/minminmean.supine/", StringComparison.OrdinalIgnoreCase))
                .ToArray();
            if (forbidden.Length != 0)
                throw new InvalidOperationException(
                    "FullFT test avatar still depends on minminmean.supine: " +
                    string.Join(", ", forbidden) +
                    ". Run BuildTestAvatar before JointBuild.");
        }

        private static T RequireAsset<T>(string path, string description) where T : UnityEngine.Object
        {
            var asset = AssetDatabase.LoadAssetAtPath<T>(path);
            if (!asset)
                throw new InvalidOperationException(
                    $"Missing {description} at '{path}'. Run the dependency import " +
                    "and Stage4 builder after the Unity long test ends.");
            return asset;
        }

        private static bool Exists(string assetPath)
        {
            return !string.IsNullOrEmpty(AssetDatabase.AssetPathToGUID(assetPath));
        }

        private static bool ExistsOnDisk(string relative)
        {
            return File.Exists(Path.Combine(
                FullFTBudgetUtil.ProjectRoot,
                relative.Replace('/', Path.DirectorySeparatorChar)));
        }

        private static void EnsureFolder(string path)
        {
            var current = "Assets";
            foreach (var part in path.Split('/').Skip(1))
            {
                var next = current + "/" + part;
                if (!AssetDatabase.IsValidFolder(next))
                    AssetDatabase.CreateFolder(current, part);
                current = next;
            }
        }
    }
}

namespace MorphForge.Editor
{
    /// <summary>
    /// Produces the FullFT-only locomotion variant. The source Nemesis controller
    /// remains untouched for historical scenes; only the generated FullFT prefab
    /// references the cleaned copy.
    /// </summary>
    internal static class FullFTNoSleepController
    {
        private const string ForbiddenPackage = "Packages/minminmean.supine/";
        private const string TemporaryPath =
            FullFTBuild.FullFTRoot + "/Nemesis_Locomotion_NoSleep.__building.controller";
        private static readonly HashSet<string> RemoveStateNames =
            new HashSet<string>(new[] { "Supine", "Prepare Supine" }, StringComparer.Ordinal);
        private static readonly HashSet<string> RemoveParameters =
            new HashSet<string>(
                new[] { "Supine", "VRCSupine", "VRCSupinePoseAdjust" },
                StringComparer.Ordinal);

        internal static Dictionary<string, object> Apply(GameObject avatar)
        {
            var original = AssetDatabase.LoadAssetAtPath<AnimatorController>(
                FullFTBuild.OriginalLocomotionPath);
            if (!original)
                throw new InvalidOperationException(
                    $"Missing source locomotion controller at {FullFTBuild.OriginalLocomotionPath}.");
            var descriptor = avatar.GetComponent<VRCAvatarDescriptor>();
            if (!descriptor)
                throw new InvalidOperationException(
                    "FullFT avatar clone has no VRCAvatarDescriptor.");

            var baseIndex = Array.FindIndex(
                descriptor.baseAnimationLayers,
                layer => layer.animatorController == original);
            if (baseIndex < 0)
                throw new InvalidOperationException(
                    "The FullFT base avatar does not reference the expected locomotion controller.");

            if (AssetDatabase.LoadAssetAtPath<AnimatorController>(TemporaryPath))
                AssetDatabase.DeleteAsset(TemporaryPath);
            if (!AssetDatabase.CopyAsset(FullFTBuild.OriginalLocomotionPath, TemporaryPath))
                throw new InvalidOperationException(
                    $"Unable to create temporary NoSleep controller at {TemporaryPath}.");
            AssetDatabase.ImportAsset(TemporaryPath, ImportAssetOptions.ForceSynchronousImport);

            var generated = AssetDatabase.LoadAssetAtPath<AnimatorController>(TemporaryPath);
            if (!generated)
                throw new InvalidOperationException(
                    $"Temporary NoSleep controller failed to load at {TemporaryPath}.");
            var cleanup = Clean(generated);
            EditorUtility.SetDirty(generated);
            AssetDatabase.SaveAssetIfDirty(generated);
            AssetDatabase.ImportAsset(TemporaryPath, ImportAssetOptions.ForceSynchronousImport);
            var remaining = AssetDatabase.GetDependencies(TemporaryPath, true)
                .Where(path => path.StartsWith(
                    ForbiddenPackage, StringComparison.OrdinalIgnoreCase))
                .ToArray();
            if (remaining.Length != 0)
                throw new InvalidOperationException(
                    "Generated NoSleep controller still depends on minminmean.supine: " +
                    string.Join(", ", remaining));

            if (AssetDatabase.LoadAssetAtPath<AnimatorController>(
                    FullFTBuild.NoSleepLocomotionPath))
                AssetDatabase.DeleteAsset(FullFTBuild.NoSleepLocomotionPath);
            var moveError = AssetDatabase.MoveAsset(
                TemporaryPath, FullFTBuild.NoSleepLocomotionPath);
            if (!string.IsNullOrEmpty(moveError))
                throw new InvalidOperationException(
                    $"Unable to publish generated NoSleep controller: {moveError}");
            AssetDatabase.ImportAsset(
                FullFTBuild.NoSleepLocomotionPath,
                ImportAssetOptions.ForceSynchronousImport);
            var derived = AssetDatabase.LoadAssetAtPath<AnimatorController>(
                FullFTBuild.NoSleepLocomotionPath);
            if (!derived)
                throw new InvalidOperationException(
                    $"Generated NoSleep controller failed to load at " +
                    $"{FullFTBuild.NoSleepLocomotionPath}.");

            var layers = descriptor.baseAnimationLayers;
            layers[baseIndex].animatorController = derived;
            descriptor.baseAnimationLayers = layers;
            EditorUtility.SetDirty(descriptor);

            remaining = AssetDatabase.GetDependencies(
                FullFTBuild.NoSleepLocomotionPath, true)
                .Where(path => path.StartsWith(
                    ForbiddenPackage, StringComparison.OrdinalIgnoreCase))
                .ToArray();
            if (remaining.Length != 0)
                throw new InvalidOperationException(
                    "Published NoSleep controller still depends on minminmean.supine: " +
                    string.Join(", ", remaining));

            cleanup["derivedController"] = FullFTBuild.NoSleepLocomotionPath;
            cleanup["derivedControllerGuid"] =
                AssetDatabase.AssetPathToGUID(FullFTBuild.NoSleepLocomotionPath);
            cleanup["baseLayerIndex"] = baseIndex;
            cleanup["sourceController"] = FullFTBuild.OriginalLocomotionPath;
            cleanup["sourceControllerGuid"] =
                AssetDatabase.AssetPathToGUID(FullFTBuild.OriginalLocomotionPath);
            cleanup["remainingMinMinmeanSupineDependencies"] = remaining;
            return cleanup;
        }

        private static Dictionary<string, object> Clean(AnimatorController controller)
        {
            var removedStates = new List<object>();
            var removedTransitions = new List<object>();
            var removedParameters = new List<string>();
            var layers = controller.layers;
            foreach (var layer in layers)
            {
                if (layer.stateMachine == null) continue;
                var machines = new List<AnimatorStateMachine>();
                var owners = new Dictionary<AnimatorState, AnimatorStateMachine>();
                CollectMachines(layer.stateMachine, machines, owners);
                var doomed = owners.Keys
                    .Where(state =>
                        RemoveStateNames.Contains(state.name) ||
                        MotionUsesForbiddenPackage(state.motion))
                    .ToArray();
                var doomedSet = new HashSet<AnimatorState>(doomed);

                foreach (var state in owners.Keys)
                {
                    foreach (var transition in state.transitions.ToArray())
                    {
                        var sleepCondition = UsesSleepParameter(transition.conditions);
                        if (sleepCondition ||
                            doomedSet.Contains(state) ||
                            (transition.destinationState != null &&
                             doomedSet.Contains(transition.destinationState)))
                        {
                            removedTransitions.Add(new
                            {
                                layer = layer.name,
                                source = state.name,
                                destination = transition.destinationState?.name,
                                reason = sleepCondition ? "sleep-condition" :
                                    doomedSet.Contains(state) ? "source-state" : "destination-state"
                            });
                            state.RemoveTransition(transition);
                        }
                    }
                }

                foreach (var machine in machines)
                {
                    foreach (var transition in machine.anyStateTransitions.ToArray())
                    {
                        var sleepCondition = UsesSleepParameter(transition.conditions);
                        if (sleepCondition ||
                            (transition.destinationState != null &&
                             doomedSet.Contains(transition.destinationState)))
                        {
                            removedTransitions.Add(new
                            {
                                layer = layer.name,
                                source = "<Any State>",
                                destination = transition.destinationState?.name,
                                reason = sleepCondition ? "sleep-condition" : "destination-state"
                            });
                            machine.RemoveAnyStateTransition(transition);
                        }
                    }
                    foreach (var transition in machine.entryTransitions.ToArray())
                    {
                        var sleepCondition = UsesSleepParameter(transition.conditions);
                        if (sleepCondition ||
                            (transition.destinationState != null &&
                             doomedSet.Contains(transition.destinationState)))
                        {
                            removedTransitions.Add(new
                            {
                                layer = layer.name,
                                source = "<Entry>",
                                destination = transition.destinationState?.name,
                                reason = sleepCondition ? "sleep-condition" : "destination-state"
                            });
                            machine.RemoveEntryTransition(transition);
                        }
                    }
                }

                foreach (var state in doomed)
                {
                    removedStates.Add(new
                    {
                        layer = layer.name,
                        name = state.name,
                        motion = DescribeMotion(state.motion),
                        reason = RemoveStateNames.Contains(state.name)
                            ? "sleep-state-name"
                            : "forbidden-package-motion"
                    });
                    owners[state].RemoveState(state);
                }
            }
            controller.layers = layers;

            foreach (var parameter in controller.parameters.ToArray())
            {
                if (!RemoveParameters.Contains(parameter.name)) continue;
                if (ParameterStillReferenced(controller, parameter.name))
                    throw new InvalidOperationException(
                        $"Sleep parameter '{parameter.name}' is still referenced after state cleanup.");
                controller.RemoveParameter(parameter);
                removedParameters.Add(parameter.name);
            }

            return new Dictionary<string, object>
            {
                ["removedStates"] = removedStates,
                ["removedTransitionCount"] = removedTransitions.Count,
                ["removedTransitions"] = removedTransitions,
                ["removedParameters"] = removedParameters,
                ["remainingParameters"] =
                    controller.parameters.Select(parameter => parameter.name).ToArray(),
            };
        }

        private static void CollectMachines(
            AnimatorStateMachine machine,
            List<AnimatorStateMachine> machines,
            Dictionary<AnimatorState, AnimatorStateMachine> owners)
        {
            if (machine == null) return;
            machines.Add(machine);
            foreach (var child in machine.states)
                if (child.state)
                    owners[child.state] = machine;
            foreach (var child in machine.stateMachines)
                CollectMachines(child.stateMachine, machines, owners);
        }

        private static bool UsesSleepParameter(AnimatorCondition[] conditions)
        {
            return conditions != null &&
                   conditions.Any(condition => RemoveParameters.Contains(condition.parameter));
        }

        private static bool ParameterStillReferenced(
            AnimatorController controller,
            string name)
        {
            foreach (var layer in controller.layers)
            {
                if (layer.stateMachine == null) continue;
                var machines = new List<AnimatorStateMachine>();
                var owners = new Dictionary<AnimatorState, AnimatorStateMachine>();
                CollectMachines(layer.stateMachine, machines, owners);
                foreach (var state in owners.Keys)
                {
                    if (state.speedParameterActive && state.speedParameter == name) return true;
                    if (state.cycleOffsetParameterActive && state.cycleOffsetParameter == name)
                        return true;
                    if (state.mirrorParameterActive && state.mirrorParameter == name) return true;
                    if (state.timeParameterActive && state.timeParameter == name) return true;
                    if (state.motion is BlendTree tree && BlendTreeUsesParameter(tree, name))
                        return true;
                    if (state.transitions.Any(transition =>
                            transition.conditions.Any(condition => condition.parameter == name)))
                        return true;
                }
                foreach (var machine in machines)
                {
                    if (machine.anyStateTransitions.Any(transition =>
                            transition.conditions.Any(condition => condition.parameter == name)))
                        return true;
                    if (machine.entryTransitions.Any(transition =>
                            transition.conditions.Any(condition => condition.parameter == name)))
                        return true;
                }
            }
            return false;
        }

        private static bool BlendTreeUsesParameter(BlendTree tree, string name)
        {
            if (tree.blendParameter == name || tree.blendParameterY == name) return true;
            return tree.children.Any(child =>
                child.motion is BlendTree nested && BlendTreeUsesParameter(nested, name));
        }

        private static bool MotionUsesForbiddenPackage(Motion motion)
        {
            if (motion == null) return false;
            if (motion is BlendTree tree)
                return tree.children.Any(child => MotionUsesForbiddenPackage(child.motion));
            var path = AssetDatabase.GetAssetPath(motion) ?? "";
            return path.StartsWith(ForbiddenPackage, StringComparison.OrdinalIgnoreCase);
        }

        private static string DescribeMotion(Motion motion)
        {
            if (motion == null) return "";
            return motion.name + " @ " + (AssetDatabase.GetAssetPath(motion) ?? "");
        }
    }
}
