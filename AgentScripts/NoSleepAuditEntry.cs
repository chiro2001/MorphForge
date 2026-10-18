using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;
using Newtonsoft.Json;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using VRC.SDK3.Avatars.Components;

public static class NoSleepAuditEntry
{
    private const string PrefabPath = "Assets/MorphForge/FullFT/Avatar.prefab";
    private static readonly Regex Keyword = new Regex(
        "sleep|supine|crouch|rootheight|footanchor|lockpose",
        RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
    private static readonly Regex Definitive = new Regex(
        "supine|minminmean",
        RegexOptions.IgnoreCase | RegexOptions.CultureInvariant);
    private static readonly string[] SupineParameters =
    {
        "Supine",
        "VRCSupine",
        "VRCSupinePoseAdjust"
    };
    private static readonly string[] RetainedPoseParameters =
    {
        "CrouchPose",
        "RootHeight",
        "SetRootHeight",
        "VRCFootAnchor",
        "VRCLockPose"
    };

    public static string Run()
    {
        var activeBefore = SceneState();
        var output = new Dictionary<string, object>
        {
            ["success"] = true,
            ["scope"] = "read-only Unity audit; no asset, prefab, scene, play mode, build, or VRChat action",
            ["generatedAtUtc"] = DateTime.UtcNow.ToString("O"),
            ["prefabPath"] = PrefabPath,
            ["prefabGuid"] = AssetDatabase.AssetPathToGUID(PrefabPath),
            ["activeScenesBefore"] = activeBefore,
            ["installedMatchingPackages"] = InstalledPackages(),
        };

        var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(PrefabPath);
        if (prefab == null)
        {
            output["success"] = false;
            output["error"] = "Prefab missing or failed to load";
            output["activeScenesAfter"] = SceneState();
            return JsonConvert.SerializeObject(output, Formatting.Indented);
        }

        var directDependencies = AssetDatabase.GetDependencies(PrefabPath, false)
            .OrderBy(value => value, StringComparer.Ordinal).ToArray();
        var dependencies = AssetDatabase.GetDependencies(PrefabPath, true)
            .OrderBy(value => value, StringComparer.Ordinal).ToArray();
        var matchingDependencyPaths = dependencies.Where(path => Keyword.IsMatch(path)).ToArray();
        var supineNamedDependencies = dependencies.Where(path => Definitive.IsMatch(path)).ToArray();

        var components = new List<ComponentRecord>();
        var objects = new List<ObjectRecord>();
        var missingScripts = new List<string>();
        var keywordComponents = new List<ComponentRecord>();
        var keywordObjects = new List<ObjectRecord>();
        var propertyMatches = new List<PropertyMatch>();
        var allComponents = prefab.GetComponentsInChildren<Component>(true);
        foreach (var component in allComponents)
        {
            var transform = component == null ? null : component.transform;
            if (transform == null)
            {
                continue;
            }
            if (component == null)
            {
                missingScripts.Add(HierarchyPath(transform));
                recordComponent(components, null, transform, "MISSING_SCRIPT", "", "", false);
                continue;
            }
            var type = component.GetType();
            var fullName = type.FullName ?? type.Name;
            var assembly = type.Assembly.GetName().Name ?? "";
            var scriptPath = "";
            if (component is MonoBehaviour behaviour)
            {
                var monoScript = MonoScript.FromMonoBehaviour(behaviour);
                if (monoScript != null)
                {
                    scriptPath = AssetDatabase.GetAssetPath(monoScript) ?? "";
                }
            }
            var record = recordComponent(
                components, component, transform, fullName, assembly, scriptPath,
                !component.gameObject.activeInHierarchy);
            if (Keyword.IsMatch(fullName) || Keyword.IsMatch(scriptPath) || Keyword.IsMatch(component.name))
            {
                keywordComponents.Add(record);
            }

            if (Keyword.IsMatch(component.gameObject.name))
            {
                keywordObjects.Add(new ObjectRecord
                {
                    path = HierarchyPath(transform),
                    name = component.gameObject.name,
                    activeSelf = component.gameObject.activeSelf,
                    activeInHierarchy = component.gameObject.activeInHierarchy
                });
            }
            inspectSerializedReferences(component, HierarchyPath(transform), propertyMatches);
        }

        objects.AddRange(prefab.GetComponentsInChildren<Transform>(true).Select(transform => new ObjectRecord
        {
            path = HierarchyPath(transform),
            name = transform.gameObject.name,
            activeSelf = transform.gameObject.activeSelf,
            activeInHierarchy = transform.gameObject.activeInHierarchy
        }));

        var controllerAudits = new List<object>();
        var matchingControllerParameters = new List<string>();
        var matchingControllerStates = new List<string>();
        var controllerLayers = new Dictionary<string, string[]>(StringComparer.Ordinal);
        foreach (var path in dependencies.Where(path => path.EndsWith(".controller", StringComparison.OrdinalIgnoreCase)))
        {
            var controller = AssetDatabase.LoadAssetAtPath<AnimatorController>(path);
            if (controller == null)
            {
                controllerAudits.Add(new
                {
                    path,
                    loaded = false
                });
                continue;
            }
            var parameters = controller.parameters.Select(parameter => parameter.name).ToArray();
            var states = collectStates(controller);
            controllerLayers[path] = controller.layers.Select(layer => layer.name).ToArray();
            var matchingParameters = parameters.Where(name => Keyword.IsMatch(name)).ToArray();
            var matchingStates = states.Where(name => Keyword.IsMatch(name)).ToArray();
            matchingControllerParameters.AddRange(matchingParameters.Select(name => path + " :: " + name));
            matchingControllerStates.AddRange(matchingStates.Select(name => path + " :: " + name));
            if (matchingParameters.Length > 0 || matchingStates.Length > 0 || Definitive.IsMatch(path))
            {
                controllerAudits.Add(new
                {
                    path,
                    loaded = true,
                    parameterCount = parameters.Length,
                    matchingParameters,
                    matchingStates
                });
            }
        }

        var expressionParameters = inspectExpressionParameterAssets(dependencies);
        var descriptor = prefab.GetComponent<VRCAvatarDescriptor>();
        var animationLayers = descriptor == null
            ? null
            : animationLayerAudit(descriptor, controllerLayers);
        var installedPackages = (List<PackageRecord>)output["installedMatchingPackages"];
        var packageDependencies = dependencies.Where(path =>
            path.StartsWith("Packages/minminmean.supine/", StringComparison.OrdinalIgnoreCase) ||
            path.IndexOf("minminmean.supine", StringComparison.OrdinalIgnoreCase) >= 0).ToArray();
        var definitiveComponents = keywordComponents.Where(record =>
            Definitive.IsMatch(record.type) ||
            Definitive.IsMatch(record.scriptPath) ||
            Definitive.IsMatch(record.gameObject)).ToArray();
        var generatorComponents = components.Where(record =>
            record.type.IndexOf("supine", StringComparison.OrdinalIgnoreCase) >= 0 &&
            record.type.IndexOf("generator", StringComparison.OrdinalIgnoreCase) >= 0).ToArray();
        var sleepParameterFindings = expressionParameters.parameters
            .Where(name => SupineParameters.Contains(name, StringComparer.Ordinal))
            .Concat(matchingControllerParameters
                .Where(value => SupineParameters.Any(parameter =>
                    value.IndexOf(parameter, StringComparison.OrdinalIgnoreCase) >= 0)))
            .Distinct(StringComparer.Ordinal)
            .ToArray();
        var retainedPoseParameterFindings = matchingControllerParameters
            .Where(value => RetainedPoseParameters.Any(parameter =>
                value.IndexOf(parameter, StringComparison.OrdinalIgnoreCase) >= 0))
            .Distinct(StringComparer.Ordinal)
            .ToArray();
        var hasDefinitiveDependency =
            packageDependencies.Length > 0 ||
            definitiveComponents.Length > 0 ||
            generatorComponents.Length > 0 ||
            sleepParameterFindings.Length > 0;
        var officialSupineSamples = supineNamedDependencies
            .Where(path => path.StartsWith(
                "Packages/com.vrchat.avatars/", StringComparison.OrdinalIgnoreCase))
            .ToArray();
        var unexplainedSupineNames = supineNamedDependencies
            .Where(path => !officialSupineSamples.Contains(path))
            .ToArray();

        var findings = new List<string>();
        if (packageDependencies.Length > 0)
        {
            findings.Add("Final prefab recursively depends on the minminmean.supine package.");
        }
        if (definitiveComponents.Length > 0)
        {
            findings.Add("Final prefab contains Supine/minminmean components.");
        }
        if (generatorComponents.Length > 0)
        {
            findings.Add("Final prefab contains a Supine generator component.");
        }
        if (sleepParameterFindings.Length > 0)
        {
            findings.Add("Controller or expression-parameter assets contain the known sleep/Supine parameter group.");
        }
        if (officialSupineSamples.Length > 0)
        {
            findings.Add(
                "Supine-named animation samples remain in the official VRChat avatars package; " +
                "they are stock action clips and do not depend on minminmean.supine.");
        }
        if (unexplainedSupineNames.Length > 0)
        {
            findings.Add(
                "Supine-named assets outside the official VRChat package exist; inspect them, " +
                "but they are not a minminmean.supine package dependency.");
        }
        if (findings.Count == 0)
        {
            findings.Add("No sleep/Supine package, component, controller parameter, expression parameter, state, or recursive dependency was found.");
        }

        output["prefab"] = new
        {
            root = prefab.name,
            sha256 = sha256(File.ReadAllBytes(Path.Combine(
                Application.dataPath, "..", PrefabPath))),
            prefabAssetType = PrefabUtility.GetPrefabAssetType(prefab).ToString(),
            gameObjectCount = objects.Count,
            componentCount = components.Count,
            missingScriptCount = missingScripts.Count,
            missingScripts
        };
        output["installedMatchingPackages"] = installedPackages;
        output["keywordComponentHits"] = keywordComponents;
        output["keywordGameObjectHits"] = keywordObjects;
        output["serializedPropertyHits"] = propertyMatches;
        output["dependencyAudit"] = new
        {
            directCount = directDependencies.Length,
            recursiveCount = dependencies.Length,
            sortedPathsSha256 = sha256(string.Join("\n", dependencies)),
            keywordMatches = matchingDependencyPaths,
            supineNamedMatches = supineNamedDependencies,
            officialVrchatSupineSamples = officialSupineSamples,
            unexplainedSupineNames = unexplainedSupineNames,
            packageMatches = packageDependencies
        };
        output["controllerAudit"] = new
        {
            controllersInspected = dependencies.Count(path =>
                path.EndsWith(".controller", StringComparison.OrdinalIgnoreCase)),
            matchingParameters = matchingControllerParameters,
            matchingStates = matchingControllerStates,
            relevantControllers = controllerAudits
        };
        output["expressionParameterAudit"] = expressionParameters;
        output["avatarDescriptor"] = descriptor == null
            ? null
            : new
            {
                found = true,
                customizeAnimationLayers = descriptor.customizeAnimationLayers,
                expressionParameters = AssetDatabase.GetAssetPath(descriptor.expressionParameters) ?? "",
                expressionsMenu = AssetDatabase.GetAssetPath(descriptor.expressionsMenu) ?? "",
                baseAnimationLayers = animationLayers.baseLayers,
                specialAnimationLayers = animationLayers.specialLayers
            };
        output["verdict"] = new
        {
            dependsOnSleepOrSupinePlugin = hasDefinitiveDependency,
            dependsOnMinminmeanSupinePackage = packageDependencies.Length > 0,
            hasSupineComponent = definitiveComponents.Length > 0,
            hasSupineGenerator = generatorComponents.Length > 0,
            hasKnownSleepSupineParameters = sleepParameterFindings.Length > 0,
            officialVrchatSupineSampleCount = officialSupineSamples.Length,
            retainedGeneralPoseParameters = retainedPoseParameterFindings,
            findings
        };
        output["activeScenesAfter"] = SceneState();
        output["sceneStateChanged"] = !JsonConvert.SerializeObject(activeBefore)
            .Equals(JsonConvert.SerializeObject(output["activeScenesAfter"]), StringComparison.Ordinal);
        output["success"] = !hasDefinitiveDependency && missingScripts.Count == 0;
        return JsonConvert.SerializeObject(output, Formatting.Indented);
    }

    private static ComponentRecord recordComponent(
        List<ComponentRecord> records,
        Component component,
        Transform transform,
        string type,
        string assembly,
        string scriptPath,
        bool inactive)
    {
        var record = new ComponentRecord
        {
            gameObject = HierarchyPath(transform),
            type = type,
            assembly = assembly,
            scriptPath = scriptPath,
            inactive = inactive
        };
        records.Add(record);
        return record;
    }

    private static void inspectSerializedReferences(
        Component component,
        string gameObject,
        List<PropertyMatch> matches)
    {
        try
        {
            var serialized = new SerializedObject(component);
            var property = serialized.GetIterator();
            var enterChildren = true;
            while (property.NextVisible(enterChildren))
            {
                enterChildren = property.depth < 16 && property.propertyType == SerializedPropertyType.Generic;
                if (Keyword.IsMatch(property.propertyPath) || Keyword.IsMatch(property.displayName))
                {
                    matches.Add(new PropertyMatch
                    {
                        gameObject = gameObject,
                        component = component.GetType().FullName ?? component.GetType().Name,
                        propertyPath = property.propertyPath,
                        reason = "property name"
                    });
                }
                if (property.propertyType == SerializedPropertyType.String &&
                    Keyword.IsMatch(property.stringValue ?? ""))
                {
                    matches.Add(new PropertyMatch
                    {
                        gameObject = gameObject,
                        component = component.GetType().FullName ?? component.GetType().Name,
                        propertyPath = property.propertyPath,
                        value = property.stringValue,
                        reason = "string value"
                    });
                }
                if (property.propertyType == SerializedPropertyType.ObjectReference &&
                    property.objectReferenceValue != null)
                {
                    var referencedPath = AssetDatabase.GetAssetPath(property.objectReferenceValue) ?? "";
                    var referencedName = property.objectReferenceValue.name ?? "";
                    if (Keyword.IsMatch(referencedPath) || Keyword.IsMatch(referencedName))
                    {
                        matches.Add(new PropertyMatch
                        {
                            gameObject = gameObject,
                            component = component.GetType().FullName ?? component.GetType().Name,
                            propertyPath = property.propertyPath,
                            referencedPath = referencedPath,
                            referencedName = referencedName,
                            reason = "object reference"
                        });
                    }
                }
            }
        }
        catch (Exception error)
        {
            matches.Add(new PropertyMatch
            {
                gameObject = gameObject,
                component = component.GetType().FullName ?? component.GetType().Name,
                propertyPath = "<inspection failed>",
                value = error.Message,
                reason = "inspection error"
            });
        }
    }

    private static List<PackageRecord> InstalledPackages()
    {
        var values = new List<PackageRecord>();
        try
        {
            foreach (var package in UnityEditor.PackageManager.PackageInfo.GetAllRegisteredPackages())
            {
                if (Keyword.IsMatch(package.name) || Keyword.IsMatch(package.displayName ?? "") ||
                    Keyword.IsMatch(package.resolvedPath ?? ""))
                {
                    values.Add(new PackageRecord
                    {
                        name = package.name,
                        displayName = package.displayName,
                        version = package.version,
                        source = package.source.ToString(),
                        resolvedPath = package.resolvedPath
                    });
                }
            }
        }
        catch (Exception error)
        {
            values.Add(new PackageRecord
            {
                name = "<package enumeration failed>",
                displayName = error.Message
            });
        }
        return values;
    }

    private static ExpressionAudit inspectExpressionParameterAssets(string[] dependencies)
    {
        var audit = new ExpressionAudit();
        foreach (var path in dependencies.Where(path =>
                     path.EndsWith(".asset", StringComparison.OrdinalIgnoreCase)))
        {
            var asset = AssetDatabase.LoadMainAssetAtPath(path);
            if (asset == null)
            {
                continue;
            }
            var serialized = new SerializedObject(asset);
            var property = serialized.GetIterator();
            var enterChildren = true;
            while (property.NextVisible(enterChildren))
            {
                enterChildren = property.depth < 16 && property.propertyType == SerializedPropertyType.Generic;
                if (property.propertyType == SerializedPropertyType.String &&
                    SupineParameters.Contains(property.stringValue, StringComparer.Ordinal))
                {
                    audit.parameters.Add(property.stringValue);
                    audit.assets.Add(path);
                }
                if (Keyword.IsMatch(property.propertyPath) || Keyword.IsMatch(property.displayName))
                {
                    audit.propertyHits.Add(path + " :: " + property.propertyPath);
                }
            }
        }
        audit.parameters = audit.parameters.Distinct(StringComparer.Ordinal).OrderBy(value => value).ToList();
        audit.assets = audit.assets.Distinct(StringComparer.Ordinal).OrderBy(value => value).ToList();
        audit.propertyHits = audit.propertyHits.Distinct(StringComparer.Ordinal).OrderBy(value => value).ToList();
        return audit;
    }

    private static List<string> collectStates(AnimatorController controller)
    {
        var states = new List<string>();
        var visited = new HashSet<int>();
        foreach (var layer in controller.layers)
        {
            if (layer.stateMachine != null)
            {
                collectStateMachine(layer.stateMachine, layer.name, states, visited);
            }
        }
        return states;
    }

    private static AnimationLayerAudit animationLayerAudit(
        VRCAvatarDescriptor descriptor,
        Dictionary<string, string[]> controllerLayers)
    {
        var baseLayers = new List<object>();
        for (var index = 0; index < descriptor.baseAnimationLayers.Length; index++)
        {
            var layer = descriptor.baseAnimationLayers[index];
            baseLayers.Add(layerRecord(index, layer.type.ToString(), layer.isEnabled,
                layer.isDefault, layer.animatorController, controllerLayers));
        }
        var specialLayers = new List<object>();
        for (var index = 0; index < descriptor.specialAnimationLayers.Length; index++)
        {
            var layer = descriptor.specialAnimationLayers[index];
            specialLayers.Add(layerRecord(index, layer.type.ToString(), layer.isEnabled,
                layer.isDefault, layer.animatorController, controllerLayers));
        }
        return new AnimationLayerAudit
        {
            baseLayers = baseLayers,
            specialLayers = specialLayers
        };
    }

    private static Dictionary<string, object> layerRecord(
        int index,
        string type,
        bool enabled,
        bool isDefault,
        RuntimeAnimatorController animatorController,
        Dictionary<string, string[]> controllerLayers)
    {
        var path = AssetDatabase.GetAssetPath(animatorController) ?? "";
        var record = new Dictionary<string, object>
        {
            ["index"] = index,
            ["type"] = type,
            ["enabled"] = enabled,
            ["isDefault"] = isDefault,
            ["controllerPath"] = path,
            ["controllerName"] = animatorController == null ? "" : animatorController.name
        };
        if (controllerLayers.TryGetValue(path, out var layers))
        {
            record["layers"] = layers;
            record["matchingLayers"] = layers.Where(name => Keyword.IsMatch(name)).ToArray();
        }
        return record;
    }

    private static void collectStateMachine(
        AnimatorStateMachine machine,
        string layer,
        List<string> states,
        HashSet<int> visited)
    {
        if (machine == null || !visited.Add(machine.GetInstanceID()))
        {
            return;
        }
        foreach (var child in machine.states)
        {
            if (child.state == null)
            {
                continue;
            }
            states.Add(layer + "/state:" + child.state.name);
            if (child.state.motion != null)
            {
                var motionPath = AssetDatabase.GetAssetPath(child.state.motion) ?? "";
                if (Keyword.IsMatch(child.state.motion.name) || Keyword.IsMatch(motionPath))
                {
                    states.Add(layer + "/motion:" + child.state.motion.name + " @ " + motionPath);
                }
            }
        }
        foreach (var child in machine.stateMachines)
        {
            collectStateMachine(child.stateMachine, layer + "/" + child.stateMachine.name, states, visited);
        }
    }

    private static List<SceneRecord> SceneState()
    {
        var output = new List<SceneRecord>();
        for (var index = 0; index < UnityEngine.SceneManagement.SceneManager.sceneCount; index++)
        {
            var scene = UnityEngine.SceneManagement.SceneManager.GetSceneAt(index);
            output.Add(new SceneRecord
            {
                path = scene.path,
                name = scene.name,
                isLoaded = scene.isLoaded,
                isDirty = scene.isDirty,
                isActive = EditorSceneManager.GetActiveScene() == scene
            });
        }
        return output;
    }

    private static string HierarchyPath(Transform transform)
    {
        var parts = new List<string>();
        var current = transform;
        while (current != null)
        {
            parts.Add(current.name + "[" + current.GetSiblingIndex() + "]");
            current = current.parent;
        }
        parts.Reverse();
        return string.Join("/", parts);
    }

    private static string sha256(string value)
    {
        return sha256(Encoding.UTF8.GetBytes(value));
    }

    private static string sha256(byte[] value)
    {
        using (var sha = SHA256.Create())
        {
            return string.Concat(sha.ComputeHash(value)
                .Select(item => item.ToString("x2")));
        }
    }

    private sealed class ComponentRecord
    {
        public string gameObject;
        public string type;
        public string assembly;
        public string scriptPath;
        public bool inactive;
    }

    private sealed class ObjectRecord
    {
        public string path;
        public string name;
        public bool activeSelf;
        public bool activeInHierarchy;
    }

    private sealed class PropertyMatch
    {
        public string gameObject;
        public string component;
        public string propertyPath;
        public string value;
        public string referencedPath;
        public string referencedName;
        public string reason;
    }

    private sealed class PackageRecord
    {
        public string name;
        public string displayName;
        public string version;
        public string source;
        public string resolvedPath;
    }

    private sealed class SceneRecord
    {
        public string path;
        public string name;
        public bool isLoaded;
        public bool isDirty;
        public bool isActive;
    }

    private sealed class ExpressionAudit
    {
        public List<string> parameters = new List<string>();
        public List<string> assets = new List<string>();
        public List<string> propertyHits = new List<string>();
    }

    private sealed class AnimationLayerAudit
    {
        public List<object> baseLayers;
        public List<object> specialLayers;
    }
}
