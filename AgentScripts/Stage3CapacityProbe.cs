using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using Newtonsoft.Json;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.Core;
using VRC.SDK3.Avatars.Components;

public static class Stage3CapacityProbe
{
    private const string SourceScenePath =
        "Assets/MorphForge/Scenes/MorphForge_Stage3.unity";
    private const string FolderPath =
        "Assets/MorphForge/Diagnostics/Security4096";
    private const string ProbeRootName =
        "MorphForge_Stage3_Avatar_4096Probe";
    private const string PrefabPath =
        FolderPath + "/MorphForge_Stage3_Avatar_4096Probe.prefab";
    private const string ProbeScenePath =
        FolderPath + "/MorphForge_Stage3_Avatar_4096Probe.unity";
    private const int TargetStoreCapacity = 4096;

    private static readonly string[] GuardedSourceAssets =
    {
        "Assets/MorphForge/Scenes/MorphForge_Stage3.unity",
        "Assets/MorphForge/Stage3/Avatar.prefab",
        "Assets/MorphForge/MaterialsCatalog/ParticleBoxCatalog.mat",
        "Assets/MorphForge/Shaders/ParticleBoxCatalog.shader",
        "Assets/MorphForge/MaterialsCatalog/CatalogAtlas.png",
        "Assets/MorphForge/MaterialsCatalog/CatalogMetadata.asset",
        "Assets/Nemesis/Nemesis_Avatar/Controller/Nemesis_FX_v_1_1_0.controller",
        "Assets/Nemesis/Nemesis_Avatar/ExMenu&Param/Nemesis_Parameters_v_1_1_0.asset",
        "Assets/Nemesis/Nemesis_Avatar/ExMenu&Param/Nemesis_MainMenu.asset"
    };

    public static string Run()
    {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling)
            throw new InvalidOperationException(
                "Capacity probe requires idle Edit Mode with compilation complete.");
        if (AssetDatabase.LoadMainAssetAtPath(PrefabPath) != null ||
            AssetDatabase.LoadAssetAtPath<SceneAsset>(ProbeScenePath) != null)
            throw new InvalidOperationException(
                "Probe output already exists; refusing to overwrite " +
                PrefabPath + " or " + ProbeScenePath + ".");

        var sourceScene = SceneManager.GetSceneByPath(SourceScenePath);
        if (!sourceScene.IsValid() || !sourceScene.isLoaded)
            throw new InvalidOperationException(
                "The dirty Stage3 source scene is not loaded: " + SourceScenePath);
        if (SceneManager.GetActiveScene() != sourceScene)
            throw new InvalidOperationException(
                "Stage3 must be the active scene before cloning.");
        if (!sourceScene.isDirty)
            throw new InvalidOperationException(
                "Stage3 must remain dirty so the in-memory authoring state is cloned.");

        var sourceAvatar = SourceAvatar(sourceScene);
        var sourceParticleSnapshot = ParticleSnapshot(sourceAvatar);
        var sourceAssetHashesBefore = HashAssets(GuardedSourceAssets);
        var sourceSceneStateBefore = SceneState(sourceScene);
        var sourceSceneHashBefore = HashFile(AbsoluteAssetPath(SourceScenePath));
        var sourcePipeline = OnePipelineManager(sourceAvatar);
        if (sourcePipeline == null)
            throw new InvalidOperationException(
                "Source Avatar has no PipelineManager.");
        var sourceBlueprintId = sourcePipeline.blueprintId;

        EnsureFolder(FolderPath);
        var probeScene = EditorSceneManager.NewScene(
            NewSceneSetup.EmptyScene,
            NewSceneMode.Additive);
        if (!probeScene.IsValid() || !probeScene.isLoaded)
            throw new InvalidOperationException("Could not create probe scene.");

        SceneManager.SetActiveScene(probeScene);
        var clone = UnityEngine.Object.Instantiate(sourceAvatar);
        clone.name = ProbeRootName;
        SceneManager.MoveGameObjectToScene(clone, probeScene);

        var cloneParticleSnapshotBefore = ParticleSnapshot(clone);
        var cloneSourceMismatches = CompareParticleSnapshots(
            sourceParticleSnapshot,
            cloneParticleSnapshotBefore,
            false);
        if (cloneSourceMismatches.Count != 0)
            throw new InvalidOperationException(
                "The in-memory clone is not particle-identical to the source: " +
                string.Join("; ", cloneSourceMismatches.Take(16)));

        var cloneStore = FindStore(clone);
        var sourceStore = FindStore(sourceAvatar);
        var storeSerialized = new SerializedObject(cloneStore);
        var maxParticles = storeSerialized.FindProperty(
            "InitialModule.maxNumParticles");
        if (maxParticles == null || maxParticles.propertyType !=
            SerializedPropertyType.Integer)
            throw new InvalidOperationException(
                "Could not find serialized Store.maxParticles.");
        var sourceCapacity = sourceStore.main.maxParticles;
        if (sourceCapacity != 16384)
            throw new InvalidOperationException(
                "Expected source Store.maxParticles=16384, found " +
                sourceCapacity + ".");
        maxParticles.intValue = TargetStoreCapacity;
        storeSerialized.ApplyModifiedPropertiesWithoutUndo();

        var cloneStoreMain = cloneStore.main;
        if (cloneStoreMain.maxParticles != TargetStoreCapacity)
            throw new InvalidOperationException(
                "Store.maxParticles did not become 4096.");
        if (!float.IsPositiveInfinity(
                cloneStoreMain.startLifetime.constant))
            throw new InvalidOperationException(
                "Changing Store.maxParticles altered its +Infinity lifetime.");

        var clonePipelines = clone.GetComponentsInChildren<PipelineManager>(true);
        if (clonePipelines.Length != 1)
            throw new InvalidOperationException(
                "Expected exactly one PipelineManager on the clone, found " +
                clonePipelines.Length + ".");
        var cloneBlueprintBefore = clonePipelines[0].blueprintId;
        clonePipelines[0].blueprintId = string.Empty;

        var cloneParticleSnapshotAfterCapacity = ParticleSnapshot(clone);
        var cloneCapacityDiffs = CompareParticleSnapshots(
            sourceParticleSnapshot,
            cloneParticleSnapshotAfterCapacity,
            false);
        if (cloneCapacityDiffs.Count != 1 ||
            cloneCapacityDiffs[0].IndexOf(
                "ParticleSystem:/MorphForgePlacement/WorldFrame/AnchorPose/" +
                "ContentLeveling/ContentScale/ProtocolCenter/MorphForgeGeometry/" +
                "Store/InitialModule.maxNumParticles",
                StringComparison.Ordinal) < 0)
            throw new InvalidOperationException(
                "Expected exactly the Store capacity difference before saving; " +
                "found: " + string.Join("; ", cloneCapacityDiffs.Take(16)));

        var controllerReferencesBefore = ControllerReferences(clone);
        var rendererReferencesBefore = RendererReferences(clone);
        var savedPrefab = PrefabUtility.SaveAsPrefabAsset(
            clone,
            PrefabPath);
        if (savedPrefab == null)
            throw new InvalidOperationException(
                "PrefabUtility.SaveAsPrefabAsset returned null.");

        var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(PrefabPath);
        if (prefab == null)
            throw new InvalidOperationException(
                "Prefab was not created: " + PrefabPath);
        var prefabParticleSnapshot = ParticleSnapshot(prefab);
        var prefabDiffs = CompareParticleSnapshots(
            sourceParticleSnapshot,
            prefabParticleSnapshot,
            true);
        if (prefabDiffs.Count != 0)
            throw new InvalidOperationException(
                "Saved prefab has unexpected particle differences: " +
                string.Join("; ", prefabDiffs.Take(16)));
        if (prefab.GetComponentsInChildren<VRCAvatarDescriptor>(true).Length != 1)
            throw new InvalidOperationException(
                "Saved prefab does not contain exactly one Avatar descriptor.");
        var prefabPipeline = OnePipelineManager(prefab);
        if (prefabPipeline == null || !string.IsNullOrEmpty(prefabPipeline.blueprintId))
            throw new InvalidOperationException(
                "Probe prefab must have an empty PipelineManager.blueprintId.");
        var prefabStore = FindStore(prefab);
        if (prefabStore.main.maxParticles != TargetStoreCapacity ||
            !float.IsPositiveInfinity(prefabStore.main.startLifetime.constant))
            throw new InvalidOperationException(
                "Probe prefab does not have Store maxParticles=4096 and " +
                "+Infinity lifetime.");
        if (!EqualControllerReferences(
                controllerReferencesBefore, ControllerReferences(prefab)))
            throw new InvalidOperationException(
                "Avatar controller references changed while saving the probe prefab.");
        if (!EqualRendererReferences(
                rendererReferencesBefore, RendererReferences(prefab)))
            throw new InvalidOperationException(
                "Avatar renderer/material references changed while saving the probe prefab.");
        if (GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(prefab) != 0 ||
            prefab.GetComponentsInChildren<Transform>(true).Any(transform =>
                GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(
                    transform.gameObject) != 0))
            throw new InvalidOperationException(
                "Probe prefab contains missing scripts.");

        var cameraObject = new GameObject(
            "Probe Camera",
            typeof(Camera));
        cameraObject.tag = "MainCamera";
        var camera = cameraObject.GetComponent<Camera>();
        camera.clearFlags = CameraClearFlags.SolidColor;
        camera.backgroundColor = new Color(0.04f, 0.055f, 0.08f, 1f);
        camera.nearClipPlane = 0.01f;
        camera.farClipPlane = 100f;
        camera.transform.position = new Vector3(2.8f, 1.8f, -4.5f);
        camera.transform.LookAt(new Vector3(0f, 1.2f, 0f));

        var lightObject = new GameObject(
            "Probe Light",
            typeof(Light));
        var light = lightObject.GetComponent<Light>();
        light.type = LightType.Directional;
        light.intensity = 1.15f;
        lightObject.transform.rotation = Quaternion.Euler(35f, -35f, 0f);

        SceneManager.SetActiveScene(probeScene);
        EditorSceneManager.MarkSceneDirty(probeScene);
        if (!EditorSceneManager.SaveScene(probeScene, ProbeScenePath))
            throw new InvalidOperationException(
                "Could not save probe scene: " + ProbeScenePath);

        var savedProbeScene = SceneManager.GetSceneByPath(ProbeScenePath);
        if (!savedProbeScene.IsValid() || !savedProbeScene.isLoaded)
            throw new InvalidOperationException(
                "Saved probe scene is not retained open.");
        var sceneAvatars = savedProbeScene.GetRootGameObjects()
            .SelectMany(root =>
                root.GetComponentsInChildren<VRCAvatarDescriptor>(true))
            .ToArray();
        if (sceneAvatars.Length != 1 ||
            sceneAvatars[0].gameObject.name != ProbeRootName)
            throw new InvalidOperationException(
                "Probe scene must contain exactly one Avatar named " +
                ProbeRootName + ".");
        Selection.activeGameObject = sceneAvatars[0].gameObject;
        Selection.SetActiveObjectWithContext(
            sceneAvatars[0].gameObject,
            null);
        var sceneParticleSnapshot = ParticleSnapshot(sceneAvatars[0].gameObject);
        var sceneDiffs = CompareParticleSnapshots(
            sourceParticleSnapshot,
            sceneParticleSnapshot,
            true);
        if (sceneDiffs.Count != 0)
            throw new InvalidOperationException(
                "Saved probe scene has unexpected particle differences: " +
                string.Join("; ", sceneDiffs.Take(16)));

        var sourceAfter = SourceAvatar(sourceScene);
        var sourceParticleSnapshotAfter = ParticleSnapshot(sourceAfter);
        var sourceParticleDiffs = CompareParticleSnapshots(
            sourceParticleSnapshot,
            sourceParticleSnapshotAfter,
            true);
        if (sourceParticleDiffs.Count != 0)
            throw new InvalidOperationException(
                "Source Avatar particle data changed during probe generation: " +
                string.Join("; ", sourceParticleDiffs.Take(16)));
        var sourcePipelineAfter = OnePipelineManager(sourceAfter);
        if (sourcePipelineAfter == null ||
            sourcePipelineAfter.blueprintId != sourceBlueprintId)
            throw new InvalidOperationException(
                "Source Avatar blueprint identity changed.");
        var sourceSceneHashAfter = HashFile(AbsoluteAssetPath(SourceScenePath));
        if (!string.Equals(
                sourceSceneHashBefore,
                sourceSceneHashAfter,
                StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException(
                "Source Stage3 scene file hash changed.");
        if (!sourceScene.isLoaded || !sourceScene.isDirty)
            throw new InvalidOperationException(
                "Source Stage3 must remain loaded and dirty.");
        var sourceSceneStateAfter = SceneState(sourceScene);
        if (sourceSceneStateBefore == null ||
            sourceSceneStateBefore.GetType() != sourceSceneStateAfter.GetType())
            throw new InvalidOperationException(
                "Could not compare source Stage3 scene state.");
        var sourceBeforeProperties = sourceSceneStateBefore.GetType()
            .GetProperties()
            .ToDictionary(property => property.Name, property =>
                property.GetValue(sourceSceneStateBefore));
        var sourceAfterProperties = sourceSceneStateAfter.GetType()
            .GetProperties()
            .ToDictionary(property => property.Name, property =>
                property.GetValue(sourceSceneStateAfter));
        var changedSourceState = sourceBeforeProperties
            .Where(pair => !Equals(
                pair.Value,
                sourceAfterProperties[pair.Key]))
            .Select(pair => pair.Key)
            .ToArray();
        if (changedSourceState.Length != 0)
            throw new InvalidOperationException(
                "Source Stage3 scene state changed: " +
                string.Join(", ", changedSourceState));
        var sourceAssetHashesAfter = HashAssets(GuardedSourceAssets);
        var sourceAssetChanges = GuardedSourceAssets
            .Where(path => sourceAssetHashesBefore[path] !=
                           sourceAssetHashesAfter[path])
            .ToArray();
        if (sourceAssetChanges.Length != 0)
            throw new InvalidOperationException(
                "Guarded source/shared assets changed: " +
                string.Join(", ", sourceAssetChanges));

        var dependencies = AssetDatabase.GetDependencies(PrefabPath, true)
            .OrderBy(path => path, StringComparer.Ordinal)
            .ToArray();
        var report = new
        {
            success = true,
            phase = "generated",
            generatedAtUtc = DateTime.UtcNow.ToString("O"),
            scope = "Read-only source clone into independent diagnostics Prefab and scene; no build, upload, Play Mode, OSC, or source save.",
            paths = new
            {
                sourceScene = SourceScenePath,
                sourceAvatar = HierarchyPath(sourceAvatar.transform),
                probePrefab = PrefabPath,
                probeScene = ProbeScenePath
            },
            source = new
            {
                sceneLoaded = sourceScene.isLoaded,
                sceneDirty = sourceScene.isDirty,
                sceneStateBefore = sourceSceneStateBefore,
                sceneStateAfter = sourceSceneStateAfter,
                sceneStateFieldsCompared = sourceBeforeProperties.Keys
                    .OrderBy(name => name, StringComparer.Ordinal)
                    .ToArray(),
                sceneActiveChangedByProbeActivation = true,
                sceneLoadedMustRemainTrue = true,
                sceneDirtyMustRemainTrue = true,
                sceneSha256Before = sourceSceneHashBefore,
                sceneSha256After = sourceSceneHashAfter,
                sceneHashUnchanged = true,
                blueprintId = sourceBlueprintId,
                storeMaxParticles = sourceCapacity,
                storeStartLifetime =
                    sourceStore.main.startLifetime.constant,
                avatarCount = 1,
                authoringInstanceWasDirty = true
            },
            probe = new
            {
                avatarCountInPrefab =
                    prefab.GetComponentsInChildren<VRCAvatarDescriptor>(true).Length,
                avatarCountInScene = sceneAvatars.Length,
                sceneRetainedOpen = savedProbeScene.isLoaded,
                sceneIsActive = SceneManager.GetActiveScene() == savedProbeScene,
                prefabStoreMaxParticles = prefabStore.main.maxParticles,
                prefabStoreStartLifetime =
                    prefabStore.main.startLifetime.constant,
                prefabStoreStartLifetimeIsPositiveInfinity =
                    float.IsPositiveInfinity(
                        prefabStore.main.startLifetime.constant),
                prefabBlueprintId = prefabPipeline.blueprintId,
                prefabMissingScriptCount =
                    prefab.GetComponentsInChildren<Transform>(true)
                        .Sum(transform =>
                            GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(
                                transform.gameObject)),
                sceneRootNames = savedProbeScene.GetRootGameObjects()
                    .Select(root => root.name)
                    .ToArray(),
                assets = new
                {
                    prefabBytes = new FileInfo(
                        AbsoluteAssetPath(PrefabPath)).Length,
                    prefabSha256 = HashFile(AbsoluteAssetPath(PrefabPath)),
                    sceneBytes = new FileInfo(
                        AbsoluteAssetPath(ProbeScenePath)).Length,
                    sceneSha256 = HashFile(AbsoluteAssetPath(ProbeScenePath))
                },
                dependencies
            },
            differences = new
            {
                operational = new[]
                {
                    "Store.maxParticles: 16384 -> 4096"
                },
                identityOnly = new[]
                {
                    "PipelineManager.blueprintId: " + sourceBlueprintId +
                    " -> <empty> so the upload creates or attaches a separate Avatar identity"
                },
                sourceStoreStartLifetime =
                    sourceStore.main.startLifetime.constant,
                probeStoreStartLifetime =
                    prefabStore.main.startLifetime.constant,
                sourceBlueprintId,
                cloneBlueprintIdBeforeClearing = cloneBlueprintBefore,
                probeBlueprintId = prefabPipeline.blueprintId,
                identityClearingIsNotAnOperationalParameterDifference = true
            },
            particleSerializedAudit = new
            {
                sourceToCloneMismatches = cloneSourceMismatches,
                sourceToChangedCloneExpectedDifferentFields = cloneCapacityDiffs,
                allowedDifferentialCount = 1,
                allowedDifferential =
                    "Store.InitialModule.maxNumParticles",
                sourceToSavedPrefabUnexpectedDifferences = prefabDiffs,
                sourceToSavedSceneUnexpectedDifferences = sceneDiffs,
                sourceUnchangedDifferences = sourceParticleDiffs,
                comparisonRule =
                    "All leaf fields on every ParticleSystem and " +
                    "ParticleSystemRenderer are compared. Only Store." +
                    "InitialModule.maxNumParticles may differ.",
                exactFieldCounts = new
                {
                    sourceFields = CountFields(sourceParticleSnapshot),
                    prefabFields = CountFields(prefabParticleSnapshot),
                    sceneFields = CountFields(sceneParticleSnapshot)
                }
            },
            referenceAudit = new
            {
                controllerReferencesBefore = controllerReferencesBefore,
                controllerReferencesAfter = ControllerReferences(prefab),
                controllerReferencesUnchanged = true,
                rendererReferencesBefore = rendererReferencesBefore,
                rendererReferencesAfter = RendererReferences(prefab),
                rendererReferencesUnchanged = true
            },
            guardedSourceAssets = new
            {
                paths = GuardedSourceAssets,
                hashesBefore = sourceAssetHashesBefore,
                hashesAfter = sourceAssetHashesAfter,
                unchanged = true
            },
            nonActions = new[]
            {
                "No source scene save, mark-clean, close, reload, build, upload, VRChat launch, or OSC send occurred.",
                "No AssetDatabase.SaveAssets call was made.",
                "Only PrefabUtility.SaveAsPrefabAsset and EditorSceneManager.SaveScene were used for the new probe assets.",
                "No original scene, source Prefab, controller, material, shader, texture, or menu asset was rewritten.",
                "The original Stage3 scene remains loaded and dirty.",
                "The saved probe scene is retained open, active, and its single 4096Probe Avatar is selected for manual upload in the SDK panel."
            }
        };
        return JsonConvert.SerializeObject(
            report,
            Formatting.Indented);
    }

    // The first save produced the Prefab, then stopped on harmless internal
    // object-reference remapping. Resume from that exact Prefab without
    // rewriting it, and save only the still-missing scene.
    public static string ResumeFromExistingPrefab()
    {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling)
            throw new InvalidOperationException(
                "Capacity probe resume requires idle Edit Mode.");
        if (AssetDatabase.LoadAssetAtPath<SceneAsset>(ProbeScenePath) != null)
            throw new InvalidOperationException(
                "Probe scene already exists; refusing to overwrite it.");
        var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(PrefabPath);
        if (prefab == null)
            throw new InvalidOperationException(
                "Existing probe Prefab not found: " + PrefabPath);
        var sourceScene = SceneManager.GetSceneByPath(SourceScenePath);
        if (!sourceScene.IsValid() || !sourceScene.isLoaded ||
            !sourceScene.isDirty)
            throw new InvalidOperationException(
                "Source Stage3 must remain loaded and dirty.");
        var sourceAvatar = SourceAvatar(sourceScene);
        var sourceParticleSnapshot = ParticleSnapshot(sourceAvatar);
        var sourceStore = FindStore(sourceAvatar);
        var sourcePipeline = OnePipelineManager(sourceAvatar);
        if (sourcePipeline == null || sourceStore.main.maxParticles != 16384)
            throw new InvalidOperationException(
                "Unexpected source Avatar state.");
        var sourceBlueprintId = sourcePipeline.blueprintId;
        var sourceStateBefore = SceneState(sourceScene);
        var sourceSceneHashBefore = HashFile(AbsoluteAssetPath(SourceScenePath));
        var sourceAssetHashesBefore = HashAssets(GuardedSourceAssets);
        var prefabHashBefore = HashFile(AbsoluteAssetPath(PrefabPath));

        if (prefab.GetComponentsInChildren<VRCAvatarDescriptor>(true).Length != 1)
            throw new InvalidOperationException(
                "Existing probe Prefab does not have one Avatar.");
        var prefabPipeline = OnePipelineManager(prefab);
        if (prefabPipeline == null ||
            !string.IsNullOrEmpty(prefabPipeline.blueprintId))
            throw new InvalidOperationException(
                "Existing probe Prefab blueprint identity is not empty.");
        var prefabStore = FindStore(prefab);
        if (prefabStore.main.maxParticles != TargetStoreCapacity ||
            !float.IsPositiveInfinity(prefabStore.main.startLifetime.constant))
            throw new InvalidOperationException(
                "Existing probe Prefab capacity/lifetime is incorrect.");
        if (prefab.GetComponentsInChildren<Transform>(true).Any(transform =>
            GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(
                transform.gameObject) != 0))
            throw new InvalidOperationException(
                "Existing probe Prefab contains missing scripts.");
        var prefabParticleSnapshot = ParticleSnapshot(prefab);
        var prefabDiffs = CompareParticleSnapshots(
            sourceParticleSnapshot,
            prefabParticleSnapshot,
            true);
        if (prefabDiffs.Count != 0)
            throw new InvalidOperationException(
                "Existing probe Prefab particle fields differ unexpectedly: " +
                string.Join("; ", prefabDiffs.Take(16)));
        if (!EqualControllerReferences(
                ControllerReferences(sourceAvatar),
                ControllerReferences(prefab)))
            throw new InvalidOperationException(
                "Existing probe Prefab controller references changed.");
        if (!EqualRendererReferences(
                RendererReferences(sourceAvatar),
                RendererReferences(prefab)))
            throw new InvalidOperationException(
                "Existing probe Prefab renderer/material references changed.");

        var closedTemporaryScenes = CloseTemporaryProbeScenes(
            sourceScene,
            default(Scene));
        SceneManager.SetActiveScene(sourceScene);
        EnsureFolder(FolderPath);
        var probeScene = EditorSceneManager.NewScene(
            NewSceneSetup.EmptyScene,
            NewSceneMode.Additive);
        SceneManager.SetActiveScene(probeScene);
        var instance = (GameObject)PrefabUtility.InstantiatePrefab(
            prefab,
            probeScene);
        instance.name = ProbeRootName;
        if (instance.GetComponentsInChildren<VRCAvatarDescriptor>(true).Length != 1)
            throw new InvalidOperationException(
                "Probe scene Prefab instance does not have one Avatar.");
        var sceneParticleSnapshotBeforeSave = ParticleSnapshot(instance);
        var sceneParticleDiffsBeforeSave = CompareParticleSnapshots(
            sourceParticleSnapshot,
            sceneParticleSnapshotBeforeSave,
            true);
        if (sceneParticleDiffsBeforeSave.Count != 0)
            throw new InvalidOperationException(
                "Probe scene instance particle fields differ unexpectedly: " +
                string.Join("; ", sceneParticleDiffsBeforeSave.Take(16)));

        var cameraObject = new GameObject(
            "Probe Camera",
            typeof(Camera));
        cameraObject.tag = "MainCamera";
        var camera = cameraObject.GetComponent<Camera>();
        camera.clearFlags = CameraClearFlags.SolidColor;
        camera.backgroundColor = new Color(0.04f, 0.055f, 0.08f, 1f);
        camera.nearClipPlane = 0.01f;
        camera.farClipPlane = 100f;
        camera.transform.position = new Vector3(2.8f, 1.8f, -4.5f);
        camera.transform.LookAt(new Vector3(0f, 1.2f, 0f));

        var lightObject = new GameObject(
            "Probe Light",
            typeof(Light));
        var light = lightObject.GetComponent<Light>();
        light.type = LightType.Directional;
        light.intensity = 1.15f;
        lightObject.transform.rotation = Quaternion.Euler(35f, -35f, 0f);

        EditorSceneManager.MarkSceneDirty(probeScene);
        if (!EditorSceneManager.SaveScene(probeScene, ProbeScenePath))
            throw new InvalidOperationException(
                "Could not save probe scene: " + ProbeScenePath);
        var savedScene = SceneManager.GetSceneByPath(ProbeScenePath);
        var sceneAvatars = savedScene.GetRootGameObjects()
            .SelectMany(root =>
                root.GetComponentsInChildren<VRCAvatarDescriptor>(true))
            .ToArray();
        if (!savedScene.IsValid() || !savedScene.isLoaded ||
            sceneAvatars.Length != 1 ||
            sceneAvatars[0].gameObject.name != ProbeRootName)
            throw new InvalidOperationException(
                "Saved probe scene does not contain exactly one 4096Probe Avatar.");
        var sceneParticleSnapshot = ParticleSnapshot(sceneAvatars[0].gameObject);
        var sceneDiffs = CompareParticleSnapshots(
            sourceParticleSnapshot,
            sceneParticleSnapshot,
            true);
        if (sceneDiffs.Count != 0)
            throw new InvalidOperationException(
                "Saved probe scene particle fields differ unexpectedly: " +
                string.Join("; ", sceneDiffs.Take(16)));
        if (!EqualControllerReferences(
                ControllerReferences(sourceAvatar),
                ControllerReferences(sceneAvatars[0].gameObject)))
            throw new InvalidOperationException(
                "Saved probe scene controller references changed.");
        if (!EqualRendererReferences(
                RendererReferences(sourceAvatar),
                RendererReferences(sceneAvatars[0].gameObject)))
            throw new InvalidOperationException(
                "Saved probe scene renderer/material references changed.");

        Selection.activeGameObject = sceneAvatars[0].gameObject;
        SceneManager.SetActiveScene(savedScene);

        var sourceAfter = SourceAvatar(sourceScene);
        var sourceParticleDiffs = CompareParticleSnapshots(
            sourceParticleSnapshot,
            ParticleSnapshot(sourceAfter),
            false);
        if (sourceParticleDiffs.Count != 0)
            throw new InvalidOperationException(
                "Source Avatar changed during probe scene generation: " +
                string.Join("; ", sourceParticleDiffs.Take(16)));
        var sourcePipelineAfter = OnePipelineManager(sourceAfter);
        if (sourcePipelineAfter == null ||
            sourcePipelineAfter.blueprintId != sourceBlueprintId)
            throw new InvalidOperationException(
                "Source blueprint identity changed.");
        if (!sourceScene.isLoaded || !sourceScene.isDirty)
            throw new InvalidOperationException(
                "Source Stage3 must remain loaded and dirty.");
        var sourceStateAfter = SceneState(sourceScene);
        if (!SceneStateEqual(sourceStateBefore, sourceStateAfter))
            throw new InvalidOperationException(
                "Source Stage3 scene state changed.");
        var sourceSceneHashAfter = HashFile(AbsoluteAssetPath(SourceScenePath));
        if (!string.Equals(
                sourceSceneHashBefore,
                sourceSceneHashAfter,
                StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException(
                "Source Stage3 scene file hash changed.");
        var sourceAssetHashesAfter = HashAssets(GuardedSourceAssets);
        var changedSourceAssets = GuardedSourceAssets
            .Where(path => sourceAssetHashesBefore[path] !=
                           sourceAssetHashesAfter[path])
            .ToArray();
        if (changedSourceAssets.Length != 0)
            throw new InvalidOperationException(
                "Guarded source/shared assets changed: " +
                string.Join(", ", changedSourceAssets));
        var prefabHashAfter = HashFile(AbsoluteAssetPath(PrefabPath));
        if (!string.Equals(
                prefabHashBefore,
                prefabHashAfter,
                StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException(
                "Existing probe Prefab was modified while creating the scene.");

        SceneManager.SetActiveScene(savedScene);

        var dependencies = AssetDatabase.GetDependencies(PrefabPath, true)
            .OrderBy(path => path, StringComparer.Ordinal)
            .ToArray();
        var report = new
        {
            success = true,
            phase = "resumed-and-generated-scene-only",
            generatedAtUtc = DateTime.UtcNow.ToString("O"),
            scope = "Reused the already-generated probe Prefab without rewriting it; saved only the new single-Avatar probe scene. No build, upload, Play Mode, OSC, or source save.",
            resumedExistingPrefabWithoutOverwrite = true,
            prefabSha256BeforeResume = prefabHashBefore,
            prefabSha256AfterResume = prefabHashAfter,
            prefabUnchangedDuringSceneGeneration = true,
            paths = new
            {
                sourceScene = SourceScenePath,
                sourceAvatar = HierarchyPath(sourceAvatar.transform),
                probePrefab = PrefabPath,
                probeScene = ProbeScenePath
            },
            source = new
            {
                sceneLoaded = sourceScene.isLoaded,
                sceneDirty = sourceScene.isDirty,
                sceneStateBefore = sourceStateBefore,
                sceneStateAfter = sourceStateAfter,
                sceneSha256Before = sourceSceneHashBefore,
                sceneSha256After = sourceSceneHashAfter,
                sceneHashUnchanged = true,
                blueprintId = sourceBlueprintId,
                storeMaxParticles = sourceStore.main.maxParticles,
                storeStartLifetime =
                    sourceStore.main.startLifetime.constant,
                activeSceneChangedToProbe = SceneManager.GetActiveScene() ==
                                            savedScene,
                authoringInstanceWasDirty = true
            },
            probe = new
            {
                avatarCountInPrefab =
                    prefab.GetComponentsInChildren<VRCAvatarDescriptor>(true).Length,
                avatarCountInScene = sceneAvatars.Length,
                sceneRetainedOpen = savedScene.isLoaded,
                sceneIsActive = SceneManager.GetActiveScene() == savedScene,
                sceneRootNames = savedScene.GetRootGameObjects()
                    .Select(root => root.name)
                    .ToArray(),
                prefabStoreMaxParticles = prefabStore.main.maxParticles,
                prefabStoreStartLifetime =
                    prefabStore.main.startLifetime.constant,
                prefabStoreStartLifetimeIsPositiveInfinity =
                    float.IsPositiveInfinity(
                        prefabStore.main.startLifetime.constant),
                prefabBlueprintId = prefabPipeline.blueprintId,
                sceneStoreMaxParticles =
                    FindStore(sceneAvatars[0].gameObject).main.maxParticles,
                sceneStoreStartLifetime =
                    FindStore(sceneAvatars[0].gameObject)
                        .main.startLifetime.constant,
                sceneStoreStartLifetimeIsPositiveInfinity =
                    float.IsPositiveInfinity(
                        FindStore(sceneAvatars[0].gameObject)
                            .main.startLifetime.constant),
                prefabMissingScriptCount =
                    prefab.GetComponentsInChildren<Transform>(true)
                        .Sum(transform =>
                            GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(
                                transform.gameObject)),
                assets = new
                {
                    prefabBytes = new FileInfo(
                        AbsoluteAssetPath(PrefabPath)).Length,
                    prefabSha256 = prefabHashAfter,
                    sceneBytes = new FileInfo(
                        AbsoluteAssetPath(ProbeScenePath)).Length,
                    sceneSha256 = HashFile(AbsoluteAssetPath(ProbeScenePath))
                },
                dependencies
            },
            differences = new
            {
                operational = new[]
                {
                    "Store.maxParticles: 16384 -> 4096"
                },
                identityOnly = new[]
                {
                    "PipelineManager.blueprintId: " + sourceBlueprintId +
                    " -> <empty> so the upload creates a separate Avatar identity"
                },
                sourceStoreStartLifetime =
                    sourceStore.main.startLifetime.constant,
                probeStoreStartLifetime =
                    prefabStore.main.startLifetime.constant,
                sourceBlueprintId,
                probeBlueprintId = prefabPipeline.blueprintId,
                identityClearingIsNotAnOperationalParameterDifference = true
            },
            particleSerializedAudit = new
            {
                allowedDifferentialCount = 1,
                allowedDifferential =
                    "Store.InitialModule.maxNumParticles",
                sourceToSavedPrefabUnexpectedDifferences = prefabDiffs,
                sourceToSavedSceneUnexpectedDifferences = sceneDiffs,
                sourceUnchangedDifferences = sourceParticleDiffs,
                comparisonRule =
                    "All leaf fields on every ParticleSystem and " +
                    "ParticleSystemRenderer are compared after normalizing " +
                    "intra-Avatar object references to relative paths. " +
                    "Only Store.InitialModule.maxNumParticles may differ.",
                exactFieldCounts = new
                {
                    sourceFields = CountFields(sourceParticleSnapshot),
                    prefabFields = CountFields(prefabParticleSnapshot),
                    sceneFields = CountFields(sceneParticleSnapshot)
                }
            },
            referenceAudit = new
            {
                controllerReferencesUnchanged = true,
                rendererReferencesUnchanged = true,
                controllers = ControllerReferences(sceneAvatars[0].gameObject),
                renderers = RendererReferences(sceneAvatars[0].gameObject)
            },
            guardedSourceAssets = new
            {
                paths = GuardedSourceAssets,
                hashesBefore = sourceAssetHashesBefore,
                hashesAfter = sourceAssetHashesAfter,
                unchanged = true
            },
            closedTemporaryScenes = closedTemporaryScenes.ToArray(),
            nonActions = new[]
            {
                "The source Stage3 scene was not saved, marked clean, closed, or reloaded.",
                "The already-generated probe Prefab was not rewritten during resume.",
                "No AssetDatabase.SaveAssets call was made.",
                "No original scene, source Prefab, controller, material, shader, texture, or menu asset was rewritten.",
                "No build, upload, VRChat launch, Play Mode, or OSC send occurred.",
                "The saved probe scene is retained open and active; its single 4096Probe Avatar is selected for manual SDK upload."
            }
        };
        return JsonConvert.SerializeObject(report, Formatting.Indented);
    }

    public static string CleanupFailedAttempt()
    {
        var sourceScene = SceneManager.GetSceneByPath(SourceScenePath);
        if (!sourceScene.IsValid() || !sourceScene.isLoaded)
            throw new InvalidOperationException(
                "The source Stage3 scene is not loaded.");
        var sourceHashBefore = HashFile(AbsoluteAssetPath(SourceScenePath));
        var closed = CloseTemporaryProbeScenes(
            sourceScene,
            default(Scene));
        if (sourceScene.isDirty)
            SceneManager.SetActiveScene(sourceScene);
        var sourceHashAfter = HashFile(AbsoluteAssetPath(SourceScenePath));
        if (!string.Equals(
                sourceHashBefore,
                sourceHashAfter,
                StringComparison.OrdinalIgnoreCase))
            throw new InvalidOperationException(
                "Source scene hash changed while cleaning the failed probe.");
        return JsonConvert.SerializeObject(new
        {
            success = true,
            closedAnonymousProbeScenes = closed.ToArray(),
            sourceScene = sourceScene.path,
            sourceLoaded = sourceScene.isLoaded,
            sourceDirty = sourceScene.isDirty,
            sourceSha256 = sourceHashAfter,
            activeScene = SceneManager.GetActiveScene().path
        }, Formatting.Indented);
    }

    private static List<string> CloseTemporaryProbeScenes(
        Scene sourceScene,
        Scene keepScene)
    {
        var closed = new List<string>();
        for (var index = SceneManager.sceneCount - 1; index >= 0; index--)
        {
            var scene = SceneManager.GetSceneAt(index);
            if (scene == sourceScene ||
                (keepScene.IsValid() && scene == keepScene) ||
                !scene.isLoaded ||
                !string.IsNullOrEmpty(scene.path))
                continue;
            var roots = scene.GetRootGameObjects();
            if (!roots.Any(root => root.name == ProbeRootName))
                continue;
            if (!EditorSceneManager.CloseScene(scene, true))
                throw new InvalidOperationException(
                    "Could not close the temporary probe scene.");
            closed.Add("<temporary-probe-scene>");
        }
        return closed;
    }

    private static GameObject SourceAvatar(Scene scene)
    {
        var descriptors = scene.GetRootGameObjects()
            .SelectMany(root =>
                root.GetComponentsInChildren<VRCAvatarDescriptor>(true))
            .ToArray();
        if (descriptors.Length != 1)
            throw new InvalidOperationException(
                "Expected one source Avatar, found " + descriptors.Length + ".");
        return descriptors[0].gameObject;
    }

    private static PipelineManager OnePipelineManager(GameObject root)
    {
        var managers = root.GetComponentsInChildren<PipelineManager>(true);
        return managers.Length == 1 ? managers[0] : null;
    }

    private static ParticleSystem FindStore(GameObject root)
    {
        var stores = root.GetComponentsInChildren<ParticleSystem>(true)
            .Where(particle => particle.name == "Store")
            .ToArray();
        if (stores.Length != 1)
            throw new InvalidOperationException(
                "Expected one particle system named Store, found " +
                stores.Length + ".");
        return stores[0];
    }

    private static Dictionary<string, Dictionary<string, string>>
        ParticleSnapshot(GameObject avatar)
    {
        var result = new Dictionary<string, Dictionary<string, string>>(
            StringComparer.Ordinal);
        foreach (var component in avatar
            .GetComponentsInChildren<Component>(true)
            .Where(component =>
                component is ParticleSystem ||
                component is ParticleSystemRenderer)
            .OrderBy(component => RelativePath(
                component.transform, avatar.transform), StringComparer.Ordinal)
            .ThenBy(component => component.GetType().Name, StringComparer.Ordinal))
        {
            var key = component.GetType().Name + ":" +
                      RelativePath(component.transform, avatar.transform);
            if (result.ContainsKey(key))
                throw new InvalidOperationException(
                    "Duplicate particle component key: " + key);
            result[key] = SerializedLeafValues(component, avatar.transform);
        }
        return result;
    }

    private static Dictionary<string, string> SerializedLeafValues(
        Component component,
        Transform avatarRoot)
    {
        var result = new Dictionary<string, string>(StringComparer.Ordinal);
        var serialized = new SerializedObject(component);
        var property = serialized.GetIterator();
        var enterChildren = true;
        while (property.Next(enterChildren))
        {
            var isContainer =
                property.propertyType == SerializedPropertyType.Generic;
            enterChildren = property.depth < 40 && isContainer;
            if (isContainer)
                continue;
            if (IsInstanceBookkeeping(property.propertyPath))
                continue;
            result[property.propertyPath] = SerializedValue(
                property,
                avatarRoot);
        }
        return result;
    }

    private static bool IsInstanceBookkeeping(string propertyPath)
    {
        return propertyPath == "m_GameObject" ||
               propertyPath == "m_CorrespondingSourceObject" ||
               propertyPath == "m_PrefabInstance" ||
               propertyPath == "m_PrefabAsset";
    }

    private static string SerializedValue(
        SerializedProperty property,
        Transform avatarRoot)
    {
        switch (property.propertyType)
        {
            case SerializedPropertyType.Boolean:
                return property.boolValue ? "true" : "false";
            case SerializedPropertyType.Integer:
            case SerializedPropertyType.Enum:
            case SerializedPropertyType.LayerMask:
                return property.intValue.ToString(CultureInfo.InvariantCulture);
            case SerializedPropertyType.Float:
                var value = property.floatValue;
                return "bits=" + BitConverter.SingleToInt32Bits(value)
                    .ToString("x8", CultureInfo.InvariantCulture) +
                    ";value=" + value.ToString("R", CultureInfo.InvariantCulture);
            case SerializedPropertyType.String:
                return property.stringValue ?? "<null>";
            case SerializedPropertyType.Color:
                var color = property.colorValue;
                return ColorValue(color.r) + "," + ColorValue(color.g) + "," +
                       ColorValue(color.b) + "," + ColorValue(color.a);
            case SerializedPropertyType.Vector2:
                return VectorValue(property.vector2Value.x) + "," +
                       VectorValue(property.vector2Value.y);
            case SerializedPropertyType.Vector3:
                return VectorValue(property.vector3Value.x) + "," +
                       VectorValue(property.vector3Value.y) + "," +
                       VectorValue(property.vector3Value.z);
            case SerializedPropertyType.Vector4:
                return VectorValue(property.vector4Value.x) + "," +
                       VectorValue(property.vector4Value.y) + "," +
                       VectorValue(property.vector4Value.z) + "," +
                       VectorValue(property.vector4Value.w);
            case SerializedPropertyType.Quaternion:
                var rotation = property.quaternionValue;
                return VectorValue(rotation.x) + "," +
                       VectorValue(rotation.y) + "," +
                       VectorValue(rotation.z) + "," +
                       VectorValue(rotation.w);
            case SerializedPropertyType.ObjectReference:
                var referenced = property.objectReferenceValue;
                if (referenced == null)
                    return "<null>";
                if (referenced is Component referencedComponent &&
                    referencedComponent.transform == avatarRoot)
                    return "intra-avatar|<root>|" +
                           referencedComponent.GetType().FullName;
                if (referenced is Component nestedComponent &&
                    nestedComponent.transform.IsChildOf(avatarRoot))
                    return "intra-avatar|" +
                           RelativePath(
                               nestedComponent.transform,
                               avatarRoot) +
                           "|" + nestedComponent.GetType().FullName;
                if (referenced is GameObject referencedObject &&
                    referencedObject.transform == avatarRoot)
                    return "intra-avatar|<root>|GameObject";
                if (referenced is GameObject nestedObject &&
                    nestedObject.transform.IsChildOf(avatarRoot))
                    return "intra-avatar|" +
                           RelativePath(nestedObject.transform, avatarRoot) +
                           "|GameObject";
                var path = AssetDatabase.GetAssetPath(referenced);
                return referenced.GetType().FullName + "|" +
                       (string.IsNullOrEmpty(path)
                           ? "scene-object:" + referenced.name
                           : path);
            case SerializedPropertyType.ManagedReference:
                return property.managedReferenceFullTypename ?? "<null>";
            default:
                return property.propertyType + ":" + property.ToString();
        }
    }

    private static string ColorValue(float value)
    {
        return VectorValue(value);
    }

    private static string VectorValue(float value)
    {
        return BitConverter.SingleToInt32Bits(value)
                   .ToString("x8", CultureInfo.InvariantCulture) +
               ":" + value.ToString("R", CultureInfo.InvariantCulture);
    }

    private static List<string> CompareParticleSnapshots(
        Dictionary<string, Dictionary<string, string>> source,
        Dictionary<string, Dictionary<string, string>> target,
        bool allowStoreCapacity)
    {
        var differences = new List<string>();
        foreach (var key in source.Keys.Union(target.Keys)
            .OrderBy(value => value, StringComparer.Ordinal))
        {
            if (!source.TryGetValue(key, out var sourceFields))
            {
                differences.Add(key + ": missing from source");
                continue;
            }
            if (!target.TryGetValue(key, out var targetFields))
            {
                differences.Add(key + ": missing from target");
                continue;
            }
            foreach (var path in sourceFields.Keys.Union(targetFields.Keys)
                .OrderBy(value => value, StringComparer.Ordinal))
            {
                if (allowStoreCapacity &&
                    key == "ParticleSystem:/MorphForgePlacement/WorldFrame/" +
                           "AnchorPose/ContentLeveling/ContentScale/" +
                           "ProtocolCenter/MorphForgeGeometry/Store" &&
                    path == "InitialModule.maxNumParticles")
                    continue;
                sourceFields.TryGetValue(path, out var sourceValue);
                targetFields.TryGetValue(path, out var targetValue);
                if (!string.Equals(
                        sourceValue, targetValue, StringComparison.Ordinal))
                    differences.Add(
                        key + "/" + path + ": " + sourceValue +
                        " -> " + targetValue);
            }
        }
        return differences;
    }

    private static int CountFields(
        Dictionary<string, Dictionary<string, string>> snapshot)
    {
        return snapshot.Sum(component => component.Value.Count);
    }

    private static string[] ControllerReferences(GameObject avatar)
    {
        var references = new List<string>();
        var animator = avatar.GetComponent<Animator>();
        references.Add(
            "rootAnimator=" +
            (animator == null || animator.runtimeAnimatorController == null
                ? "<null>"
                : AssetDatabase.GetAssetPath(
                    animator.runtimeAnimatorController)));
        var descriptor = avatar.GetComponent<VRCAvatarDescriptor>();
        if (descriptor != null)
        {
            for (var index = 0; index < descriptor.baseAnimationLayers.Length;
                 index++)
            {
                var layer = descriptor.baseAnimationLayers[index];
                references.Add(
                    "base[" + index + "]/" + layer.type + "/controller=" +
                    AssetPath(layer.animatorController) + "/mask=" +
                    AssetPath(layer.mask));
            }
            for (var index = 0;
                 index < descriptor.specialAnimationLayers.Length;
                 index++)
            {
                var layer = descriptor.specialAnimationLayers[index];
                references.Add(
                    "special[" + index + "]/" + layer.type + "/controller=" +
                    AssetPath(layer.animatorController) + "/mask=" +
                    AssetPath(layer.mask));
            }
        }
        return references.ToArray();
    }

    private static string[] RendererReferences(GameObject avatar)
    {
        return avatar.GetComponentsInChildren<Renderer>(true)
            .OrderBy(renderer => RelativePath(
                renderer.transform, avatar.transform), StringComparer.Ordinal)
            .Select(renderer =>
            {
                var materialPaths = renderer.sharedMaterials
                    .Select(AssetPath)
                    .ToArray();
                var mesh = renderer is SkinnedMeshRenderer skinned
                    ? skinned.sharedMesh
                    : renderer.GetComponent<MeshFilter>() == null
                        ? null
                        : renderer.GetComponent<MeshFilter>().sharedMesh;
                return RelativePath(renderer.transform, avatar.transform) +
                       "|" + renderer.GetType().Name +
                       "|mesh=" + AssetPath(mesh) +
                       "|materials=" + string.Join(",", materialPaths);
            })
            .ToArray();
    }

    private static bool EqualControllerReferences(
        string[] before,
        string[] after)
    {
        return before.SequenceEqual(after, StringComparer.Ordinal);
    }

    private static bool EqualRendererReferences(
        string[] before,
        string[] after)
    {
        return before.SequenceEqual(after, StringComparer.Ordinal);
    }

    private static string AssetPath(UnityEngine.Object value)
    {
        return value == null
            ? "<null>"
            : AssetDatabase.GetAssetPath(value);
    }

    private static string RelativePath(Transform transform, Transform root)
    {
        var names = new List<string>();
        var current = transform;
        while (current != null && current != root)
        {
            names.Add(current.name);
            current = current.parent;
        }
        if (current != root)
            return "<outside-avatar>/" + HierarchyPath(transform);
        names.Reverse();
        return names.Count == 0 ? "<root>" : "/" + string.Join("/", names);
    }

    private static string HierarchyPath(Transform transform)
    {
        var names = new List<string>();
        var current = transform;
        while (current != null)
        {
            names.Add(current.name + "[" + current.GetSiblingIndex() + "]");
            current = current.parent;
        }
        names.Reverse();
        return string.Join("/", names);
    }

    private static object SceneState(Scene scene)
    {
        return new
        {
            path = scene.path,
            name = scene.name,
            isLoaded = scene.isLoaded,
            isDirty = scene.isDirty,
            rootCount = scene.rootCount
        };
    }

    private static bool SceneStateEqual(object before, object after)
    {
        if (before == null || after == null ||
            before.GetType() != after.GetType())
            return false;
        var beforeProperties = before.GetType().GetProperties()
            .ToDictionary(property => property.Name, property =>
                property.GetValue(before));
        var afterProperties = after.GetType().GetProperties()
            .ToDictionary(property => property.Name, property =>
                property.GetValue(after));
        return beforeProperties.Count == afterProperties.Count &&
               beforeProperties.All(pair =>
                   afterProperties.TryGetValue(pair.Key, out var value) &&
                   Equals(pair.Value, value));
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

    private static Dictionary<string, string> HashAssets(
        IEnumerable<string> paths)
    {
        var result = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var path in paths)
        {
            var absolute = AbsoluteAssetPath(path);
            result[path] = File.Exists(absolute)
                ? HashFile(absolute)
                : "<missing>";
        }
        return result;
    }

    private static string AbsoluteAssetPath(string path)
    {
        return Path.GetFullPath(
            Path.Combine(Application.dataPath, "..", path));
    }

    private static string HashFile(string path)
    {
        using (var stream = File.OpenRead(path))
        using (var sha = SHA256.Create())
            return string.Concat(
                sha.ComputeHash(stream)
                    .Select(value => value.ToString("x2")));
    }
}
