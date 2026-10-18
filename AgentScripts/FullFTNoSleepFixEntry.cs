using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Newtonsoft.Json;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;
using VRC.SDK3.Avatars.Components;

public static class FullFTNoSleepFixEntry
{
    private const string AvatarPath = "Assets/MorphForge/FullFT/Avatar.prefab";
    private const string OriginalController =
        "Assets/Nemesis/Nemesis_Avatar/Controller/Nemesis_Locomotion.controller";
    private const string DerivedController =
        "Assets/MorphForge/FullFT/Nemesis_Locomotion_NoSleep.controller";
    private const string ReportPath = "Docs/Avatars/no-sleep-fix.json";
    private const string ForbiddenPackage = "Packages/minminmean.supine/";
    private static readonly HashSet<string> RemoveStateNames =
        new HashSet<string>(new[] { "Supine", "Prepare Supine" }, StringComparer.Ordinal);
    private static readonly HashSet<string> SleepParameters =
        new HashSet<string>(new[] { "Supine", "VRCSupine", "VRCSupinePoseAdjust" },
            StringComparer.Ordinal);

    public static string Inspect()
    {
        var controller = AssetDatabase.LoadAssetAtPath<AnimatorController>(OriginalController);
        if (controller == null)
        {
            throw new InvalidOperationException("Original locomotion controller is missing.");
        }
        var report = BuildInspection(controller, OriginalController);
        return JsonConvert.SerializeObject(report, Formatting.Indented);
    }

    public static string Apply()
    {
        RequireIdle();
        var original = AssetDatabase.LoadAssetAtPath<AnimatorController>(OriginalController);
        var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(AvatarPath);
        if (original == null) throw new InvalidOperationException("Original locomotion controller is missing.");
        if (prefab == null) throw new InvalidOperationException("FullFT Avatar prefab is missing.");
        var descriptor = prefab.GetComponent<VRCAvatarDescriptor>();
        if (descriptor == null) throw new InvalidOperationException("Avatar has no VRCAvatarDescriptor.");
        var baseIndex = Array.FindIndex(
            descriptor.baseAnimationLayers,
            layer => layer.animatorController == original);
        if (baseIndex < 0)
        {
            throw new InvalidOperationException(
                "Avatar does not reference the original locomotion controller in a base layer.");
        }

        var beforeHash = Sha256File(AbsoluteAssetPath(AvatarPath));
        var backup = BackupFile(AvatarPath, beforeHash);
        var originalControllerBackup = BackupFile(
            OriginalController, Sha256File(AbsoluteAssetPath(OriginalController)));

        if (!AssetDatabase.LoadAssetAtPath<AnimatorController>(DerivedController))
        {
            EnsureFolder("Assets/MorphForge/FullFT");
            if (!AssetDatabase.CopyAsset(OriginalController, DerivedController))
                throw new InvalidOperationException(
                    $"Failed to copy controller to {DerivedController}.");
            AssetDatabase.ImportAsset(DerivedController, ImportAssetOptions.ForceSynchronousImport);
        }

        var derived = AssetDatabase.LoadAssetAtPath<AnimatorController>(DerivedController);
        if (derived == null)
            throw new InvalidOperationException($"Derived controller failed to load: {DerivedController}");
        if (derived == original)
            throw new InvalidOperationException("Derived controller unexpectedly aliases the original.");

        var cleanup = CleanController(derived);
        EditorUtility.SetDirty(derived);
        AssetDatabase.SaveAssetIfDirty(derived);
        AssetDatabase.ImportAsset(DerivedController, ImportAssetOptions.ForceSynchronousImport);
        derived = AssetDatabase.LoadAssetAtPath<AnimatorController>(DerivedController);
        var remainingControllerDependencies = AssetDatabase.GetDependencies(DerivedController, true)
            .Where(path => path.StartsWith(ForbiddenPackage, StringComparison.OrdinalIgnoreCase))
            .ToArray();
        if (remainingControllerDependencies.Length != 0)
        {
            throw new InvalidOperationException(
                "Derived controller still depends on Supine: " +
                string.Join(", ", remainingControllerDependencies));
        }

        var contents = PrefabUtility.LoadPrefabContents(AvatarPath);
        try
        {
            var nextDescriptor = contents.GetComponent<VRCAvatarDescriptor>();
            if (nextDescriptor == null)
                throw new InvalidOperationException("Loaded prefab contents have no VRCAvatarDescriptor.");
            var layers = nextDescriptor.baseAnimationLayers;
            if (layers[baseIndex].animatorController != original)
                throw new InvalidOperationException(
                    "Prefab changed while preparing the controller replacement.");
            var beforeLayers = layerSnapshot(nextDescriptor);
            layers[baseIndex].animatorController = derived;
            nextDescriptor.baseAnimationLayers = layers;
            var afterLayers = layerSnapshot(nextDescriptor);
            EditorUtility.SetDirty(nextDescriptor);
            PrefabUtility.SaveAsPrefabAsset(contents, AvatarPath);
            cleanup["layerBefore"] = beforeLayers;
            cleanup["layerAfter"] = afterLayers;
        }
        finally
        {
            PrefabUtility.UnloadPrefabContents(contents);
        }
        AssetDatabase.ImportAsset(AvatarPath, ImportAssetOptions.ForceSynchronousImport);

        var savedPrefab = AssetDatabase.LoadAssetAtPath<GameObject>(AvatarPath);
        var savedDescriptor = savedPrefab?.GetComponent<VRCAvatarDescriptor>();
        var savedDerived = savedDescriptor?.baseAnimationLayers[baseIndex].animatorController;
        if (savedDerived == null || AssetDatabase.GetAssetPath(savedDerived) != DerivedController)
            throw new InvalidOperationException("Saved prefab does not reference the derived controller.");
        var remainingPrefabDependencies = AssetDatabase.GetDependencies(AvatarPath, true)
            .Where(path => path.StartsWith(ForbiddenPackage, StringComparison.OrdinalIgnoreCase))
            .ToArray();
        if (remainingPrefabDependencies.Length != 0)
        {
            throw new InvalidOperationException(
                "Saved prefab still depends on Supine: " +
                string.Join(", ", remainingPrefabDependencies));
        }

        var afterHash = Sha256File(AbsoluteAssetPath(AvatarPath));
        var report = new Dictionary<string, object>
        {
            ["success"] = true,
            ["scope"] = "FullFT-specific prefab/controller repair; no scene save, Play, build, upload, or VRChat action",
            ["completedAtUtc"] = DateTime.UtcNow.ToString("O"),
            ["avatarPath"] = AvatarPath,
            ["prefabSha256Before"] = beforeHash,
            ["prefabSha256After"] = afterHash,
            ["prefabBackup"] = backup,
            ["originalController"] = OriginalController,
            ["originalControllerBackup"] = originalControllerBackup,
            ["originalControllerSha256"] = Sha256File(AbsoluteAssetPath(OriginalController)),
            ["derivedController"] = DerivedController,
            ["derivedControllerGuid"] = AssetDatabase.AssetPathToGUID(DerivedController),
            ["derivedControllerSha256"] = Sha256File(AbsoluteAssetPath(DerivedController)),
            ["baseLayerIndex"] = baseIndex,
            ["cleanup"] = cleanup,
            ["forbiddenPackage"] = ForbiddenPackage,
            ["remainingControllerDependencies"] = remainingControllerDependencies,
            ["remainingPrefabDependencies"] = remainingPrefabDependencies,
            ["sceneWasNotTouched"] = true,
        };
        WriteReport(report);
        return JsonConvert.SerializeObject(report, Formatting.Indented);
    }

    private static Dictionary<string, object> CleanController(AnimatorController controller)
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
            var doomed = owners.Keys.Where(state =>
                    RemoveStateNames.Contains(state.name) ||
                    MotionUsesForbiddenPackage(state.motion))
                .ToArray();
            var doomedSet = new HashSet<AnimatorState>(doomed);

            foreach (var state in owners.Keys)
            {
                foreach (var transition in state.transitions.ToArray())
                {
                    if (doomedSet.Contains(state) ||
                        (transition.destinationState != null &&
                         doomedSet.Contains(transition.destinationState)) ||
                        UsesSleepParameter(transition.conditions))
                    {
                        removedTransitions.Add(new
                        {
                            layer = layer.name,
                            source = state.name,
                            destination = transition.destinationState?.name,
                            reason = UsesSleepParameter(transition.conditions)
                                ? "sleep-condition"
                                : doomedSet.Contains(state)
                                    ? "source-state"
                                    : "destination-state"
                        });
                        state.RemoveTransition(transition);
                    }
                }
            }
            foreach (var machine in machines)
            {
                foreach (var transition in machine.anyStateTransitions.ToArray())
                {
                    if (transition.destinationState != null &&
                        doomedSet.Contains(transition.destinationState) ||
                        UsesSleepParameter(transition.conditions))
                    {
                        removedTransitions.Add(new
                        {
                            layer = layer.name,
                            source = "<Any State>",
                            destination = transition.destinationState.name,
                            reason = UsesSleepParameter(transition.conditions)
                                ? "sleep-condition"
                                : "destination-state"
                        });
                        machine.RemoveAnyStateTransition(transition);
                    }
                }
                foreach (var transition in machine.entryTransitions.ToArray())
                {
                    if (transition.destinationState != null &&
                        doomedSet.Contains(transition.destinationState) ||
                        UsesSleepParameter(transition.conditions))
                    {
                        removedTransitions.Add(new
                        {
                            layer = layer.name,
                            source = "<Entry>",
                            destination = transition.destinationState.name,
                            reason = UsesSleepParameter(transition.conditions)
                                ? "sleep-condition"
                                : "destination-state"
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
            if (!SleepParameters.Contains(parameter.name)) continue;
            if (ParameterStillReferenced(controller, parameter.name))
            {
                throw new InvalidOperationException(
                    $"Sleep parameter '{parameter.name}' is still referenced after state removal.");
            }
            controller.RemoveParameter(parameter);
            removedParameters.Add(parameter.name);
        }
        EditorUtility.SetDirty(controller);
        return new Dictionary<string, object>
        {
            ["removedStates"] = removedStates,
            ["removedTransitions"] = removedTransitions,
            ["removedParameters"] = removedParameters,
            ["remainingParameters"] = controller.parameters.Select(p => p.name).ToArray(),
            ["dependencyPathsAfter"] = AssetDatabase.GetDependencies(
                DerivedController, true)
                .Where(path => path.IndexOf("minminmean.supine", StringComparison.OrdinalIgnoreCase) >= 0)
                .ToArray(),
        };
    }

    private static bool UsesSleepParameter(AnimatorCondition[] conditions)
    {
        return conditions != null &&
               conditions.Any(condition => SleepParameters.Contains(condition.parameter));
    }

    private static void CollectMachines(
        AnimatorStateMachine machine,
        List<AnimatorStateMachine> machines,
        Dictionary<AnimatorState, AnimatorStateMachine> owners)
    {
        if (machine == null) return;
        machines.Add(machine);
        foreach (var child in machine.states)
        {
            if (child.state != null)
                owners[child.state] = machine;
        }
        foreach (var child in machine.stateMachines)
            CollectMachines(child.stateMachine, machines, owners);
    }

    private static bool ParameterStillReferenced(AnimatorController controller, string name)
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
                if (state.cycleOffsetParameterActive && state.cycleOffsetParameter == name) return true;
                if (state.mirrorParameterActive && state.mirrorParameter == name) return true;
                if (state.timeParameterActive && state.timeParameter == name) return true;
                if (state.motion is BlendTree tree && BlendTreeUsesParameter(tree, name)) return true;
                if (state.transitions.Any(t => t.conditions.Any(c => c.parameter == name)))
                    return true;
            }
            foreach (var machine in machines)
            {
                if (machine.anyStateTransitions.Any(t =>
                        t.destinationState != null &&
                        t.conditions.Any(condition => condition.parameter == name)))
                    return true;
                foreach (var transition in machine.entryTransitions)
                {
                    if (transition.conditions.Any(condition => condition.parameter == name))
                        return true;
                }
            }
        }
        return false;
    }

    private static bool BlendTreeUsesParameter(BlendTree tree, string name)
    {
        if (tree.blendParameter == name || tree.blendParameterY == name) return true;
        return tree.children.Any(child => child.motion is BlendTree nested &&
                                         BlendTreeUsesParameter(nested, name));
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

    private static Dictionary<string, object> BuildInspection(
        AnimatorController controller,
        string path)
    {
        var layers = new List<object>();
        var sleepParameterUsage = new List<object>();
        foreach (var layer in controller.layers)
        {
            var machines = new List<AnimatorStateMachine>();
            var owners = new Dictionary<AnimatorState, AnimatorStateMachine>();
            if (layer.stateMachine != null)
                CollectMachines(layer.stateMachine, machines, owners);
            var transitions = new List<object>();
            foreach (var state in owners.Keys)
            {
                foreach (var transition in state.transitions)
                {
                    foreach (var condition in transition.conditions)
                    {
                        if (SleepParameters.Contains(condition.parameter))
                        {
                            sleepParameterUsage.Add(new
                            {
                                layer = layer.name,
                                source = state.name,
                                destination = transition.destinationState?.name,
                                condition.parameter
                            });
                        }
                    }
                }
                foreach (var transition in state.transitions)
                {
                    if (transition.destinationState == null ||
                        !RemoveStateNames.Contains(transition.destinationState.name) &&
                        !RemoveStateNames.Contains(state.name))
                        continue;
                    transitions.Add(new
                    {
                        source = state.name,
                        destination = transition.destinationState.name,
                        conditions = transition.conditions.Select(c => new
                        {
                            c.parameter,
                            mode = c.mode.ToString(),
                            c.threshold
                        }).ToArray()
                    });
                }
            }
            foreach (var machine in machines)
            {
                foreach (var transition in machine.anyStateTransitions)
                {
                    foreach (var condition in transition.conditions)
                    {
                        if (SleepParameters.Contains(condition.parameter))
                        {
                            sleepParameterUsage.Add(new
                            {
                                layer = layer.name,
                                source = "<Any State>",
                                destination = transition.destinationState?.name,
                                condition.parameter
                            });
                        }
                    }
                }
                foreach (var transition in machine.entryTransitions)
                {
                    foreach (var condition in transition.conditions)
                    {
                        if (SleepParameters.Contains(condition.parameter))
                        {
                            sleepParameterUsage.Add(new
                            {
                                layer = layer.name,
                                source = "<Entry>",
                                destination = transition.destinationState?.name,
                                condition.parameter
                            });
                        }
                    }
                }
                foreach (var transition in machine.anyStateTransitions)
                {
                    if (transition.destinationState == null ||
                        !RemoveStateNames.Contains(transition.destinationState.name))
                        continue;
                    transitions.Add(new
                    {
                        source = "<Any State>",
                        destination = transition.destinationState.name,
                        conditions = transition.conditions.Select(c => new
                        {
                            c.parameter,
                            mode = c.mode.ToString(),
                            c.threshold
                        }).ToArray()
                    });
                }
                foreach (var transition in machine.entryTransitions)
                {
                    if (transition.destinationState == null ||
                        !RemoveStateNames.Contains(transition.destinationState.name))
                        continue;
                    transitions.Add(new
                    {
                        source = "<Entry>",
                        destination = transition.destinationState.name,
                        conditions = transition.conditions.Select(c => new
                        {
                            c.parameter,
                            mode = c.mode.ToString(),
                            c.threshold
                        }).ToArray()
                    });
                }
            }
            layers.Add(new
            {
                name = layer.name,
                states = owners.Keys.Select(state => new
                {
                    state.name,
                    motion = DescribeMotion(state.motion),
                    forbiddenMotion = MotionUsesForbiddenPackage(state.motion),
                    removeByName = RemoveStateNames.Contains(state.name),
                    speedParameter = state.speedParameter,
                    cycleOffsetParameter = state.cycleOffsetParameter,
                    mirrorParameter = state.mirrorParameter,
                    timeParameter = state.timeParameter
                }).ToArray(),
                relevantTransitions = transitions
            });
        }
        return new Dictionary<string, object>
        {
            ["path"] = path,
            ["guid"] = AssetDatabase.AssetPathToGUID(path),
            ["sha256"] = Sha256File(AbsoluteAssetPath(path)),
            ["parameters"] = controller.parameters.Select(p => new
            {
                p.name,
                type = p.type.ToString()
            }).ToArray(),
            ["layers"] = layers,
            ["sleepParameterUsage"] = sleepParameterUsage,
            ["forbiddenDependencies"] = AssetDatabase.GetDependencies(path, true)
                .Where(value => value.StartsWith(ForbiddenPackage, StringComparison.OrdinalIgnoreCase))
                .ToArray(),
        };
    }

    private static Dictionary<string, string> layerSnapshot(VRCAvatarDescriptor descriptor)
    {
        var output = new Dictionary<string, string>(StringComparer.Ordinal);
        for (var index = 0; index < descriptor.baseAnimationLayers.Length; index++)
        {
            output["base:" + index + ":" + descriptor.baseAnimationLayers[index].type] =
                AssetDatabase.GetAssetPath(descriptor.baseAnimationLayers[index].animatorController);
        }
        for (var index = 0; index < descriptor.specialAnimationLayers.Length; index++)
        {
            output["special:" + index + ":" + descriptor.specialAnimationLayers[index].type] =
                AssetDatabase.GetAssetPath(descriptor.specialAnimationLayers[index].animatorController);
        }
        return output;
    }

    private static void RequireIdle()
    {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling ||
            EditorApplication.isUpdating)
            throw new InvalidOperationException("Unity must be idle Edit Mode.");
    }

    private static void EnsureFolder(string path)
    {
        if (AssetDatabase.IsValidFolder(path)) return;
        var parent = Path.GetDirectoryName(path)?.Replace('\\', '/');
        var name = Path.GetFileName(path);
        if (string.IsNullOrEmpty(parent) || string.IsNullOrEmpty(name))
            throw new InvalidOperationException("Invalid asset folder: " + path);
        EnsureFolder(parent);
        AssetDatabase.CreateFolder(parent, name);
    }

    private static string BackupFile(string assetPath, string hash)
    {
        var source = AbsoluteAssetPath(assetPath);
        var root = Path.Combine(Application.dataPath, "..", "Backups", "FullFT");
        Directory.CreateDirectory(root);
        var name = Path.GetFileName(assetPath);
        var destination = Path.Combine(
            root, DateTime.UtcNow.ToString("yyyyMMddTHHmmssfffZ") + "-" + hash.Substring(0, 12) + "-" + name);
        File.Copy(source, destination, false);
        return Path.GetRelativePath(Path.Combine(Application.dataPath, ".."), destination)
            .Replace('\\', '/');
    }

    private static string AbsoluteAssetPath(string path)
    {
        return Path.GetFullPath(Path.Combine(Application.dataPath, "..", path));
    }

    private static string Sha256File(string path)
    {
        using (var sha = System.Security.Cryptography.SHA256.Create())
        using (var stream = File.OpenRead(path))
            return string.Concat(sha.ComputeHash(stream).Select(item => item.ToString("x2")));
    }

    private static void WriteReport(object report)
    {
        var path = Path.Combine(Application.dataPath, "..", ReportPath);
        Directory.CreateDirectory(Path.GetDirectoryName(path));
        File.WriteAllText(path, JsonConvert.SerializeObject(report, Formatting.Indented) + "\n");
    }
}
