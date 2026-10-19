using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using Newtonsoft.Json;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Profiling;
using UnityEngine.SceneManagement;
using VRC.SDK3.Avatars.Components;

// Read-only diagnostics for the current Stage3 scene. This intentionally never
// saves, marks clean, enters Play Mode, builds, uploads, or mutates an asset.
public static class Stage3SecurityAudit
{
    public static string Run()
    {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling)
            throw new InvalidOperationException(
                "Stage3 audit requires idle Edit Mode with compilation complete.");

        var scenesBefore = SceneStates();
        var scene = SceneManager.GetActiveScene();
        var roots = scene.GetRootGameObjects();
        var descriptors = roots
            .SelectMany(root => root.GetComponentsInChildren<VRCAvatarDescriptor>(true))
            .ToArray();
        if (descriptors.Length != 1)
            throw new InvalidOperationException(
                "Expected exactly one VRCAvatarDescriptor in active scene; found " +
                descriptors.Length + ".");

        var descriptor = descriptors[0];
        var avatar = descriptor.gameObject;
        var transforms = avatar.GetComponentsInChildren<Transform>(true);
        var components = avatar.GetComponentsInChildren<Component>(true);
        var avatarRenderers = avatar.GetComponentsInChildren<Renderer>(true);
        var particleSystems =
            avatar.GetComponentsInChildren<ParticleSystem>(true);
        var missingScripts = transforms
            .Select(transform => new
            {
                path = HierarchyPath(transform),
                count = GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(
                    transform.gameObject)
            })
            .Where(item => item.count > 0)
            .ToArray();

        var componentCounts = components
            .Where(component => component != null)
            .GroupBy(component => component.GetType().FullName)
            .OrderByDescending(group => group.Count())
            .ThenBy(group => group.Key, StringComparer.Ordinal)
            .Select(group => new
            {
                type = group.Key,
                count = group.Count()
            })
            .ToArray();

        var morphForgeScripts = components
            .OfType<MonoBehaviour>()
            .Where(component => component != null &&
                component.GetType().Namespace != null &&
                component.GetType().Namespace.StartsWith(
                    "MorphForge", StringComparison.Ordinal))
            .Select(component => new
            {
                type = component.GetType().FullName,
                path = HierarchyPath(component.transform),
                script = AssetDatabase.GetAssetPath(
                    MonoScript.FromMonoBehaviour(component))
            })
            .ToArray();

        var sceneCustomBehaviours = roots
            .SelectMany(root => root.GetComponentsInChildren<MonoBehaviour>(true))
            .Where(component => component != null)
            .Select(component => new
            {
                type = component.GetType().FullName,
                assembly = component.GetType().Assembly.GetName().Name,
                path = HierarchyPath(component.transform),
                insideAvatar = component.transform.IsChildOf(avatar.transform),
                editorOnly = component.gameObject.CompareTag("EditorOnly")
            })
            .Where(item =>
                item.type.IndexOf("MorphForge", StringComparison.OrdinalIgnoreCase) >= 0 ||
                item.type.IndexOf("GestureManager", StringComparison.OrdinalIgnoreCase) >= 0)
            .ToArray();

        var particleAudits = particleSystems
            .Select(particle => ParticleAudit(particle))
            .ToArray();

        var meshes = avatarRenderers
            .Select(renderer => renderer.GetComponent<MeshFilter>())
            .Where(filter => filter != null && filter.sharedMesh != null)
            .Select(filter => filter.sharedMesh)
            .Concat(avatar.GetComponentsInChildren<SkinnedMeshRenderer>(true)
                .Where(renderer => renderer.sharedMesh != null)
                .Select(renderer => renderer.sharedMesh))
            .Distinct()
            .ToArray();
        var meshAudits = meshes
            .Select(MeshAudit)
            .ToArray();

        var materials = avatarRenderers
            .SelectMany(renderer => renderer.sharedMaterials)
            .Where(material => material != null)
            .Distinct()
            .ToArray();
        var textures = materials
            .SelectMany(material => material.GetTexturePropertyNames()
                .Select(property => material.GetTexture(property)))
            .OfType<Texture2D>()
            .Distinct()
            .Select(TextureAudit)
            .ToArray();

        var animator = avatar.GetComponent<Animator>();
        var controllerPath = animator == null ||
                             animator.runtimeAnimatorController == null
            ? string.Empty
            : AssetDatabase.GetAssetPath(animator.runtimeAnimatorController);
        var controllerAudit = animator == null ||
                              animator.runtimeAnimatorController == null
            ? null
            : ControllerAudit(
                animator.runtimeAnimatorController as AnimatorController,
                controllerPath);

        var animationLayers = descriptor.baseAnimationLayers
            .Select((layer, index) => AnimationLayerAudit("base", index, layer))
            .Concat(descriptor.specialAnimationLayers
                .Select((layer, index) => AnimationLayerAudit("special", index, layer)))
            .ToArray();
        var animationLayerControllerAudits = descriptor.baseAnimationLayers
            .Select(layer => layer.animatorController)
            .Concat(descriptor.specialAnimationLayers
                .Select(layer => layer.animatorController))
            .Where(controller => controller != null)
            .Distinct()
            .Select(controller => ControllerAudit(
                controller as AnimatorController,
                AssetDatabase.GetAssetPath(controller)))
            .ToArray();

        var expressionParameters = descriptor.expressionParameters == null ||
                                   descriptor.expressionParameters.parameters == null
            ? Array.Empty<object>()
            : descriptor.expressionParameters.parameters
                .Select(parameter => (object)new
                {
                    name = parameter.name,
                    type = parameter.valueType.ToString(),
                    synced = parameter.networkSynced,
                    defaultValue = parameter.defaultValue
                })
                .ToArray();
        var syncedBits = descriptor.expressionParameters == null ||
                         descriptor.expressionParameters.parameters == null
            ? 0
            : descriptor.expressionParameters.parameters
                .Where(parameter => parameter.networkSynced)
                .Sum(parameter => parameter.valueType.ToString() == "Bool"
                    ? 1
                    : 8);

        var scenePathAbsolute = AbsoluteAssetPath(scene.path);
        var animatorAudits = avatar.GetComponentsInChildren<Animator>(true)
            .Select(item => item == animator
                ? new
                {
                    role = "avatar_main",
                    path = HierarchyPath(item.transform),
                    controllerPath,
                    controllerName = item.runtimeAnimatorController == null
                        ? null
                        : item.runtimeAnimatorController.name,
                    isHuman = item.isHuman,
                    applyRootMotion = item.applyRootMotion,
                    avatarName = item.avatar == null ? null : item.avatar.name
                }
                : new
                {
                    role = item.transform.IsChildOf(avatar.transform)
                        ? "avatar_auxiliary"
                        : "scene_auxiliary",
                    path = HierarchyPath(item.transform),
                    controllerPath = item.runtimeAnimatorController == null
                        ? null
                        : AssetDatabase.GetAssetPath(item.runtimeAnimatorController),
                    controllerName = item.runtimeAnimatorController == null
                        ? null
                        : item.runtimeAnimatorController.name,
                    isHuman = item.isHuman,
                    applyRootMotion = item.applyRootMotion,
                    avatarName = item.avatar == null ? null : item.avatar.name
                })
            .ToArray();
        var report = new
        {
            success = true,
            scope = "Read-only active-scene audit; no save, mark-clean, Play Mode, build, upload, OSC, or asset mutation.",
            generatedAtUtc = DateTime.UtcNow.ToString("O"),
            activeScene = scene.path,
            activeSceneDirty = scene.isDirty,
            activeSceneRootCount = roots.Length,
            sceneFile = new
            {
                exists = File.Exists(scenePathAbsolute),
                sizeBytes = File.Exists(scenePathAbsolute)
                    ? new FileInfo(scenePathAbsolute).Length
                    : 0,
                lastWriteTimeUtc = File.Exists(scenePathAbsolute)
                    ? File.GetLastWriteTimeUtc(scenePathAbsolute).ToString("O")
                    : null,
                sha256 = File.Exists(scenePathAbsolute)
                    ? Sha256File(scenePathAbsolute)
                    : null
            },
            avatar = new
            {
                name = avatar.name,
                path = HierarchyPath(avatar.transform),
                activeSelf = avatar.activeSelf,
                activeInHierarchy = avatar.activeInHierarchy,
                transformCount = transforms.Length,
                componentCount = components.Count(component => component != null),
                missingScriptObjectCount = missingScripts.Length,
                missingScripts,
                componentCounts,
                morphForgeScripts,
                animator = animator == null
                    ? null
                    : new
                    {
                        controllerPath,
                        controllerName = animator.runtimeAnimatorController == null
                            ? null
                            : animator.runtimeAnimatorController.name,
                        isHuman = animator.isHuman,
                        applyRootMotion = animator.applyRootMotion
                    },
                animators = animatorAudits,
                controller = controllerAudit,
                animationLayers,
                animationLayerControllerAudits,
                descriptor = new
                {
                    pipelineId = descriptor.GetComponent<VRC.Core.PipelineManager>() == null
                        ? null
                        : descriptor.GetComponent<VRC.Core.PipelineManager>().blueprintId,
                    customizeAnimationLayers = descriptor.customizeAnimationLayers,
                    expressionParametersPath = descriptor.expressionParameters == null
                        ? null
                        : AssetDatabase.GetAssetPath(descriptor.expressionParameters),
                    expressionsMenuPath = descriptor.expressionsMenu == null
                        ? null
                        : AssetDatabase.GetAssetPath(descriptor.expressionsMenu),
                    syncedBits,
                    expressionParameters
                },
                renderers = new
                {
                    rendererCount = avatar.GetComponentsInChildren<Renderer>(true).Length,
                    meshRendererCount =
                        avatar.GetComponentsInChildren<MeshRenderer>(true).Length,
                    skinnedMeshRendererCount =
                        avatar.GetComponentsInChildren<SkinnedMeshRenderer>(true).Length,
                    particleSystemRendererCount =
                        avatar.GetComponentsInChildren<ParticleSystemRenderer>(true).Length,
                    meshCount = meshAudits.Length,
                    totalVertices = meshes.Sum(mesh => mesh.vertexCount),
                    meshes = meshAudits,
                    materialCount = materials.Length,
                    materials = materials.Select(MaterialAudit).ToArray(),
                    uniqueTextureCount = textures.Length,
                    textures
                },
                particleSystems = particleAudits
            },
            sceneCustomBehaviours,
            sceneStatesBefore = scenesBefore,
            sceneStatesAfter = SceneStates(),
            nonActions = new[]
            {
                "No scene was saved, marked clean, closed, switched, or reloaded.",
                "No Play Mode, build, upload, VRChat launch, or OSC send occurred.",
                "No serialized asset or component was modified."
            }
        };

        return JsonConvert.SerializeObject(report, Formatting.Indented);
    }

    private static object ParticleAudit(ParticleSystem particle)
    {
        var main = particle.main;
        var emission = particle.emission;
        var bursts = new ParticleSystem.Burst[emission.burstCount];
        emission.GetBursts(bursts);
        var renderer = particle.GetComponent<ParticleSystemRenderer>();
        var nonFinite = new List<object>();
        var serialized = new SerializedObject(particle);
        var property = serialized.GetIterator();
        var enterChildren = true;
        while (property.NextVisible(enterChildren))
        {
            enterChildren = property.depth < 24 &&
                            property.propertyType == SerializedPropertyType.Generic;
            if (property.propertyType != SerializedPropertyType.Float)
                continue;
            var value = property.floatValue;
            if (!float.IsNaN(value) && !float.IsInfinity(value))
                continue;
            if (nonFinite.Count < 128)
            {
                nonFinite.Add(new
                {
                    propertyPath = property.propertyPath,
                    value = float.IsNaN(value)
                        ? "NaN"
                        : value > 0 ? "+Infinity" : "-Infinity"
                });
            }
        }

        return new
        {
            name = particle.name,
            path = HierarchyPath(particle.transform),
            activeSelf = particle.gameObject.activeSelf,
            activeInHierarchy = particle.gameObject.activeInHierarchy,
            isPlaying = particle.isPlaying,
            isPaused = particle.isPaused,
            isStopped = particle.isStopped,
            particleCount = particle.particleCount,
            lengthInSeconds = main.duration,
            looping = main.loop,
            prewarm = main.prewarm,
            playOnAwake = main.playOnAwake,
            simulationSpace = main.simulationSpace.ToString(),
            scalingMode = main.scalingMode.ToString(),
            ringBufferMode = main.ringBufferMode.ToString(),
            startLifetime = Curve(main.startLifetime),
            startSpeed = Curve(main.startSpeed),
            startSize = Curve(main.startSize),
            startSize3D = main.startSize3D,
            startRotation3D = main.startRotation3D,
            maxParticles = main.maxParticles,
            emission = new
            {
                enabled = emission.enabled,
                rateOverTime = Curve(emission.rateOverTime),
                rateOverDistance = Curve(emission.rateOverDistance),
                bursts = bursts.Select(burst => new
                {
                    time = burst.time,
                    count = Curve(burst.count),
                    cycleCount = burst.cycleCount,
                    repeatInterval = burst.repeatInterval,
                    probability = burst.probability
                }).ToArray()
            },
            collision = new
            {
                enabled = particle.collision.enabled,
                type = particle.collision.type.ToString(),
                colliderForce = particle.collision.colliderForce
            },
            subEmitters = new
            {
                enabled = particle.subEmitters.enabled,
                count = particle.subEmitters.subEmittersCount,
                systems = Enumerable.Range(0, particle.subEmitters.subEmittersCount)
                    .Select(index => particle.subEmitters.GetSubEmitterSystem(index) == null
                        ? null
                        : HierarchyPath(
                            particle.subEmitters.GetSubEmitterSystem(index).transform))
                    .ToArray()
            },
            trails = particle.trails.enabled,
            lights = particle.lights.enabled,
            renderer = renderer == null
                ? null
                : new
                {
                    renderMode = renderer.renderMode.ToString(),
                    alignment = renderer.alignment.ToString(),
                    sortMode = renderer.sortMode.ToString(),
                    meshPath = renderer.mesh == null
                        ? null
                        : AssetDatabase.GetAssetPath(renderer.mesh),
                    meshVertexCount = renderer.mesh == null
                        ? 0
                        : renderer.mesh.vertexCount,
                    materialPaths = renderer.sharedMaterials
                        .Where(material => material != null)
                        .Select(material => AssetDatabase.GetAssetPath(material))
                        .ToArray(),
                    activeVertexStreams = ActiveVertexStreams(renderer)
                },
            nonFiniteSerializedFloats = nonFinite
        };
    }

    private static object ControllerAudit(
        AnimatorController controller,
        string path)
    {
        if (controller == null)
        {
            return new
            {
                loaded = false,
                path
            };
        }

        var layerAudits = controller.layers
            .Select((layer, index) => new
            {
                index,
                name = layer.name,
                weight = layer.defaultWeight,
                blendingMode = layer.blendingMode.ToString(),
                syncedLayerIndex = layer.syncedLayerIndex,
                syncedLayerAffectsTiming = layer.syncedLayerAffectsTiming,
                ikPass = layer.iKPass,
                stateCount = CountStates(layer.stateMachine),
                anyStateTransitionCount = CountTransitions(layer.stateMachine),
                subStateMachineCount = CountSubStateMachines(layer.stateMachine),
                blendTreeCount = CountBlendTrees(layer.stateMachine),
                missingMotionCount = CountMissingMotions(layer.stateMachine)
            })
            .ToArray();

        var uniqueClips = controller.animationClips
            .Where(clip => clip != null)
            .Distinct()
            .ToArray();
        var nonFinite = new List<object>();
        var serialized = new SerializedObject(controller);
        var property = serialized.GetIterator();
        var enterChildren = true;
        while (property.NextVisible(enterChildren))
        {
            enterChildren = property.depth < 24 &&
                            property.propertyType == SerializedPropertyType.Generic;
            if (property.propertyType != SerializedPropertyType.Float)
                continue;
            var value = property.floatValue;
            if (!float.IsNaN(value) && !float.IsInfinity(value))
                continue;
            if (nonFinite.Count < 64)
            {
                nonFinite.Add(new
                {
                    propertyPath = property.propertyPath,
                    value = float.IsNaN(value)
                        ? "NaN"
                        : value > 0 ? "+Infinity" : "-Infinity"
                });
            }
        }

        return new
        {
            loaded = true,
            path,
            name = controller.name,
            parameterCount = controller.parameters.Length,
            parameters = controller.parameters.Select(parameter => new
            {
                name = parameter.name,
                type = parameter.type.ToString(),
                defaultFloat = parameter.defaultFloat,
                defaultInt = parameter.defaultInt,
                defaultBool = parameter.defaultBool
            }).ToArray(),
            layerCount = controller.layers.Length,
            layers = layerAudits,
            totalStateCount = layerAudits.Sum(layer => layer.stateCount),
            totalTransitionCount =
                layerAudits.Sum(layer => layer.anyStateTransitionCount),
            uniqueClipCount = uniqueClips.Length,
            uniqueClips = uniqueClips.Select(clip => new
            {
                name = clip.name,
                path = AssetDatabase.GetAssetPath(clip),
                length = clip.length,
                frameRate = clip.frameRate,
                bindingCount = AnimationUtility.GetCurveBindings(clip).Length +
                               AnimationUtility.GetObjectReferenceCurveBindings(clip).Length
            }).ToArray(),
            nonFiniteSerializedFloats = nonFinite
        };
    }

    private static object AnimationLayerAudit(
        string group,
        int index,
        VRCAvatarDescriptor.CustomAnimLayer layer)
    {
        return new
        {
            group,
            index,
            type = layer.type.ToString(),
            enabled = layer.isEnabled,
            isDefault = layer.isDefault,
            controllerPath = layer.animatorController == null
                ? null
                : AssetDatabase.GetAssetPath(layer.animatorController),
            controllerName = layer.animatorController == null
                ? null
                : layer.animatorController.name
        };
    }

    private static object MeshAudit(Mesh mesh)
    {
        return new
        {
            name = mesh.name,
            path = AssetDatabase.GetAssetPath(mesh),
            vertexCount = mesh.vertexCount,
            subMeshCount = mesh.subMeshCount,
            blendShapeCount = mesh.blendShapeCount,
            bindPoseCount = mesh.bindposes.Length,
            bounds = new
            {
                center = new
                {
                    x = mesh.bounds.center.x,
                    y = mesh.bounds.center.y,
                    z = mesh.bounds.center.z
                },
                size = new
                {
                    x = mesh.bounds.size.x,
                    y = mesh.bounds.size.y,
                    z = mesh.bounds.size.z
                }
            }
        };
    }

    private static object MaterialAudit(Material material)
    {
        return new
        {
            name = material.name,
            path = AssetDatabase.GetAssetPath(material),
            shader = material.shader == null ? null : material.shader.name,
            texturePropertyCount = material.GetTexturePropertyNames().Length
        };
    }

    private static object TextureAudit(Texture2D texture)
    {
        return new
        {
            name = texture.name,
            path = AssetDatabase.GetAssetPath(texture),
            width = texture.width,
            height = texture.height,
            format = texture.format.ToString(),
            mipmapCount = texture.mipmapCount,
            estimatedRuntimeMemoryBytes =
                Profiler.GetRuntimeMemorySizeLong(texture)
        };
    }

    private static object Curve(ParticleSystem.MinMaxCurve curve)
    {
        return new
        {
            mode = curve.mode.ToString(),
            constant = curve.constant,
            constantMin = curve.constantMin,
            constantMax = curve.constantMax,
            curveMultiplier = curve.curveMultiplier,
            isNonFinite = !float.IsFinite(curve.constant) ||
                          !float.IsFinite(curve.constantMin) ||
                          !float.IsFinite(curve.constantMax)
        };
    }

    private static string[] ActiveVertexStreams(
        ParticleSystemRenderer renderer)
    {
        var streams = new List<ParticleSystemVertexStream>();
        renderer.GetActiveVertexStreams(streams);
        return streams.Select(stream => stream.ToString()).ToArray();
    }

    private static int CountStates(AnimatorStateMachine machine)
    {
        if (machine == null)
            return 0;
        return machine.states.Length +
               machine.stateMachines.Sum(child => CountStates(child.stateMachine));
    }

    private static int CountTransitions(AnimatorStateMachine machine)
    {
        if (machine == null)
            return 0;
        return machine.anyStateTransitions.Length +
               machine.entryTransitions.Length +
               machine.states.Sum(state => state.state == null
                   ? 0
                   : state.state.transitions.Length) +
               machine.stateMachines.Sum(child =>
                   CountTransitions(child.stateMachine));
    }

    private static int CountSubStateMachines(AnimatorStateMachine machine)
    {
        if (machine == null)
            return 0;
        return machine.stateMachines.Length +
               machine.stateMachines.Sum(child =>
                   CountSubStateMachines(child.stateMachine));
    }

    private static int CountBlendTrees(AnimatorStateMachine machine)
    {
        if (machine == null)
            return 0;
        var count = machine.states.Count(state =>
            state.state != null && state.state.motion is BlendTree);
        return count + machine.stateMachines.Sum(child =>
            CountBlendTrees(child.stateMachine));
    }

    private static int CountMissingMotions(AnimatorStateMachine machine)
    {
        if (machine == null)
            return 0;
        return machine.states.Count(state =>
                   state.state != null && state.state.motion == null) +
               machine.stateMachines.Sum(child =>
                   CountMissingMotions(child.stateMachine));
    }

    private static object[] SceneStates()
    {
        return Enumerable.Range(0, SceneManager.sceneCount)
            .Select(index => SceneManager.GetSceneAt(index))
            .Select(scene => (object)new
            {
                path = scene.path,
                name = scene.name,
                isLoaded = scene.isLoaded,
                isDirty = scene.isDirty,
                isActive = EditorSceneManager.GetActiveScene() == scene
            })
            .ToArray();
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

    private static string AbsoluteAssetPath(string assetPath)
    {
        return Path.GetFullPath(
            Path.Combine(Application.dataPath, "..", assetPath));
    }

    private static string Sha256File(string path)
    {
        using (var stream = File.OpenRead(path))
        using (var sha = SHA256.Create())
        {
            return string.Concat(
                sha.ComputeHash(stream).Select(value => value.ToString("x2")));
        }
    }
}
