using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
using VRC.SDK3.Avatars.ScriptableObjects;

namespace MorphForge.Editor
{
    internal static class FullFTBudgetUtil
    {
        internal sealed class DuplicateParameterGroup
        {
            public string name;
            public int count;
            public string[] types;
            public float[] defaults;
        }

        internal sealed class ManifestEntryState
        {
            public string name;
            public string expectedType;
            public int expectedBits;
            public string actualType;
            public bool present;
            public bool networkSynced;
            public bool compressed;
            public string representation;
        }

        internal sealed class ManifestAudit
        {
            public int expectedCount;
            public int expectedBits;
            public int representedCount;
            public int representedBits;
            public int directCount;
            public int directBits;
            public int compressedCount;
            public int compressedBits;
            public bool exact;
            public bool allRepresented;
            public bool allDirect;
            public string[] missing;
            public string[] typeMismatches;
            public string[] unexpectedSynced;
            public DuplicateParameterGroup[] duplicates;
            public ManifestEntryState[] entries;

            public object ToReport()
            {
                return new
                {
                    expectedCount,
                    expectedBits,
                    representedCount,
                    representedBits,
                    directCount,
                    directBits,
                    compressedCount,
                    compressedBits,
                    exact,
                    allRepresented,
                    allDirect,
                    missing,
                    typeMismatches,
                    unexpectedSynced,
                    duplicates,
                    entries,
                };
            }
        }

        internal sealed class MenuControlChange
        {
            public VRCExpressionsMenu menu;
            public VRCExpressionsMenu.Control control;
            public VRCExpressionsMenu.Control.ControlType originalType;
            public string menuName;
            public string label;
            public string[] parameters;
        }

        internal sealed class MenuDirectOverridePlan
        {
            public GameObject avatar;
            public VRCAvatarDescriptor descriptor;
            public VRCExpressionsMenu originalMenu;
            public VRCExpressionsMenu clonedMenu;
            public VRCExpressionsMenu[] clonedMenus;
            public string[] targetParameters;
            public MenuControlChange[] changes;
            public bool restored;

            public object ToReport()
            {
                return new
                {
                    targetParameters,
                    originalMenu = originalMenu ? originalMenu.name : null,
                    clonedMenu = clonedMenu ? clonedMenu.name : null,
                    clonedMenuCount = clonedMenus?.Length ?? 0,
                    changedControls = changes.Select(change => new
                    {
                        menu = change.menuName,
                        label = change.label,
                        originalType = change.originalType.ToString(),
                        temporaryType = VRCExpressionsMenu.Control.ControlType.Button.ToString(),
                        change.parameters
                    }).ToArray(),
                };
            }
        }

        private sealed class MenuControlReference
        {
            public VRCExpressionsMenu menu;
            public VRCExpressionsMenu.Control control;
        }

        private static readonly object PendingMenuOverridesLock = new object();
        private static readonly Dictionary<int, MenuDirectOverridePlan> PendingMenuOverrides =
            new Dictionary<int, MenuDirectOverridePlan>();

        public static int Bits(VRCExpressionParameters.Parameter parameter)
        {
            return parameter.valueType == VRCExpressionParameters.ValueType.Bool ? 1 : 8;
        }

        public static int Cost(IEnumerable<VRCExpressionParameters.Parameter> parameters)
        {
            return parameters.Sum(Bits);
        }

        public static string ProjectRoot =>
            Directory.GetParent(Application.dataPath)?.FullName ??
            throw new InvalidOperationException("Cannot resolve Unity project root.");

        public static void WriteReport(string relativePath, object report)
        {
            var path = Path.Combine(ProjectRoot, relativePath.Replace('/', Path.DirectorySeparatorChar));
            Directory.CreateDirectory(Path.GetDirectoryName(path) ??
                                      throw new InvalidOperationException("No report directory."));
            File.WriteAllText(path, Newtonsoft.Json.JsonConvert.SerializeObject(
                report,
                Newtonsoft.Json.Formatting.Indented));
        }

        public static FullFTSyncProfile LoadProfile()
        {
            return AssetDatabase.LoadAssetAtPath<FullFTSyncProfile>(
                FullFTSyncProfile.DefaultAssetPath);
        }

        public static bool IsFullFTTarget(GameObject target, FullFTSyncProfile profile)
        {
            return target &&
                   profile &&
                   !string.IsNullOrEmpty(profile.avatarNamePrefix) &&
                   target.name.StartsWith(profile.avatarNamePrefix, StringComparison.Ordinal);
        }

        public static VRCExpressionParameters GetParameters(GameObject target)
        {
            var descriptor = target ? target.GetComponent<VRCAvatarDescriptor>() : null;
            return descriptor ? descriptor.expressionParameters : null;
        }

        public static int SetNetworkSynced(
            VRCExpressionParameters parameters,
            IEnumerable<string> names,
            bool networkSynced)
        {
            var wanted = new HashSet<string>(names ?? Enumerable.Empty<string>(), StringComparer.Ordinal);
            if (wanted.Count == 0) return 0;

            var serialized = new SerializedObject(parameters);
            var array = serialized.FindProperty("parameters") ??
                        throw new InvalidOperationException(
                            "VRCExpressionParameters.parameters was not serializable.");
            var changed = 0;
            for (var i = 0; i < array.arraySize; i++)
            {
                var entry = array.GetArrayElementAtIndex(i);
                var name = entry.FindPropertyRelative("name")?.stringValue;
                if (name == null || !wanted.Contains(name)) continue;
                var sync = entry.FindPropertyRelative("networkSynced") ??
                           throw new InvalidOperationException(
                               "VRCExpressionParameters.Parameter.networkSynced was not found.");
                if (sync.boolValue == networkSynced) continue;
                sync.boolValue = networkSynced;
                changed++;
            }

            if (changed > 0) serialized.ApplyModifiedPropertiesWithoutUndo();
            return changed;
        }

        public static DuplicateParameterGroup[] DuplicateParameterGroups(
            VRCExpressionParameters parameters)
        {
            return parameters.parameters
                .Where(parameter => parameter != null && !string.IsNullOrEmpty(parameter.name))
                .GroupBy(parameter => parameter.name, StringComparer.Ordinal)
                .Where(group => group.Count() > 1)
                .Select(group =>
                {
                    var entries = group.ToArray();
                    return new DuplicateParameterGroup
                    {
                        name = group.Key,
                        count = entries.Length,
                        types = entries.Select(parameter => parameter.valueType.ToString()).ToArray(),
                        defaults = entries.Select(parameter => parameter.defaultValue).ToArray(),
                    };
                })
                .OrderBy(group => group.name, StringComparer.Ordinal)
                .ToArray();
        }

        public static ManifestAudit AuditManifest(
            VRCExpressionParameters parameters,
            IEnumerable<FullFTSyncManifest.ExpectedParameter> expectations,
            HashSet<string> compressedNames,
            string exactNamespacePrefix)
        {
            FullFTSyncManifest.Validate();
            var expected = expectations.ToArray();
            var expectedNames = new HashSet<string>(
                expected.Select(parameter => parameter.name), StringComparer.Ordinal);
            var byName = parameters.parameters
                .Where(parameter => parameter != null && !string.IsNullOrEmpty(parameter.name))
                .GroupBy(parameter => parameter.name, StringComparer.Ordinal)
                .ToDictionary(group => group.Key, group => group.ToArray(), StringComparer.Ordinal);
            var compressed = compressedNames ??
                             new HashSet<string>(StringComparer.Ordinal);
            var duplicates = DuplicateParameterGroups(parameters);

            var entries = expected.Select(specification =>
            {
                byName.TryGetValue(specification.name, out var matches);
                matches = matches ?? Array.Empty<VRCExpressionParameters.Parameter>();
                var single = matches.Length == 1 ? matches[0] : null;
                var representation = Representation(single, compressed);
                return new ManifestEntryState
                {
                    name = specification.name,
                    expectedType = specification.valueType.ToString(),
                    expectedBits = specification.Bits,
                    actualType = matches.Length == 0
                        ? "missing"
                        : matches.Length > 1
                            ? "duplicate"
                            : single.valueType.ToString(),
                    present = single != null,
                    networkSynced = single != null && single.networkSynced,
                    compressed = single != null && compressed.Contains(single.name),
                    representation = representation,
                };
            }).ToArray();

            var missing = entries.Where(entry => !entry.present)
                .Select(entry => entry.name).ToArray();
            var typeMismatches = entries
                .Where(entry => entry.present &&
                                !string.Equals(
                                    entry.actualType,
                                    entry.expectedType,
                                    StringComparison.Ordinal))
                .Select(entry => $"{entry.name}:{entry.actualType}/{entry.expectedType}")
                .ToArray();
            var unexpectedSynced = string.IsNullOrEmpty(exactNamespacePrefix)
                ? Array.Empty<string>()
                : parameters.parameters
                    .Where(parameter => parameter != null &&
                                        parameter.networkSynced &&
                                        !string.IsNullOrEmpty(parameter.name) &&
                                        parameter.name.StartsWith(
                                            exactNamespacePrefix, StringComparison.Ordinal) &&
                                        !expectedNames.Contains(parameter.name))
                    .Select(parameter => parameter.name)
                    .Distinct(StringComparer.Ordinal)
                    .OrderBy(name => name, StringComparer.Ordinal)
                    .ToArray();
            var exact = duplicates.Length == 0 &&
                        missing.Length == 0 &&
                        typeMismatches.Length == 0 &&
                        unexpectedSynced.Length == 0;
            var representedEntries = entries
                .Where(entry => entry.representation != "missing")
                .ToArray();
            var directEntries = representedEntries
                .Where(entry => entry.representation == "direct")
                .ToArray();
            var compressedEntries = representedEntries
                .Where(entry => entry.representation == "compressed")
                .ToArray();

            return new ManifestAudit
            {
                expectedCount = expected.Length,
                expectedBits = expected.Sum(parameter => parameter.Bits),
                representedCount = representedEntries.Length,
                representedBits = representedEntries.Sum(entry => entry.expectedBits),
                directCount = directEntries.Length,
                directBits = directEntries.Sum(entry => entry.expectedBits),
                compressedCount = compressedEntries.Length,
                compressedBits = compressedEntries.Sum(entry => entry.expectedBits),
                exact = exact,
                allRepresented = exact && representedEntries.Length == expected.Length,
                allDirect = exact && directEntries.Length == expected.Length,
                missing = missing,
                typeMismatches = typeMismatches,
                unexpectedSynced = unexpectedSynced,
                duplicates = duplicates,
                entries = entries,
            };
        }

        public static string Representation(
            VRCExpressionParameters.Parameter parameter,
            HashSet<string> compressedNames)
        {
            if (parameter == null) return "missing";
            if (parameter.networkSynced) return "direct";
            if (compressedNames != null && compressedNames.Contains(parameter.name))
                return "compressed";
            return "missing";
        }

        public static string VrcFuryPackageVersion()
        {
            var vrcFuryAssembly = AppDomain.CurrentDomain.GetAssemblies()
                .FirstOrDefault(assembly => string.Equals(
                    assembly.GetName().Name, "VRCFury", StringComparison.Ordinal));
            if (vrcFuryAssembly != null)
            {
                var package = UnityEditor.PackageManager.PackageInfo.FindForAssembly(
                    vrcFuryAssembly);
                if (package != null && !string.IsNullOrEmpty(package.version))
                    return package.version;
            }

            var packageJson = Path.Combine(
                ProjectRoot, "Packages", "com.vrcfury.vrcfury", "package.json");
            if (!File.Exists(packageJson)) return string.Empty;
            try
            {
                var json = Newtonsoft.Json.Linq.JObject.Parse(File.ReadAllText(packageJson));
                return (string)json["version"] ?? string.Empty;
            }
            catch
            {
                return string.Empty;
            }
        }

        public static MenuDirectOverridePlan PlanDirectMenuOverrides(
            GameObject avatar,
            IEnumerable<string> parameterNames)
        {
            var descriptor = avatar ? avatar.GetComponent<VRCAvatarDescriptor>() : null;
            var sourceMenu = descriptor ? descriptor.expressionsMenu : null;
            var targets = new HashSet<string>(
                parameterNames ?? Enumerable.Empty<string>(), StringComparer.Ordinal);
            var hasTarget = sourceMenu != null && EnumerateMenuControls(sourceMenu)
                .Any(reference => ControlParameters(reference.control).Any(targets.Contains));

            var plan = new MenuDirectOverridePlan
            {
                avatar = avatar,
                descriptor = descriptor,
                originalMenu = sourceMenu,
                targetParameters = targets
                    .OrderBy(name => name, StringComparer.Ordinal)
                    .ToArray(),
                changes = Array.Empty<MenuControlChange>(),
            };
            if (!hasTarget) return plan;

            var clonedMenus = CloneMenuGraph(sourceMenu);
            plan.clonedMenus = clonedMenus;
            plan.clonedMenu = clonedMenus[0];
            descriptor.expressionsMenu = plan.clonedMenu;
            EditorUtility.SetDirty(descriptor);
            try
            {
                var changes = new List<MenuControlChange>();
                foreach (var reference in EnumerateMenuControls(plan.clonedMenu))
                {
                    var control = reference.control;
                    var parameters = ControlParameters(control);
                    if (!parameters.Any(targets.Contains)) continue;
                    if (EditorUtility.IsPersistent(reference.menu))
                        throw new InvalidOperationException(
                            "In-memory menu clone unexpectedly contains persistent asset " +
                            $"'{AssetDatabase.GetAssetPath(reference.menu)}'.");
                    changes.Add(new MenuControlChange
                    {
                        menu = reference.menu,
                        control = control,
                        originalType = control.type,
                        menuName = reference.menu.name,
                        label = control.name,
                        parameters = parameters,
                    });
                }
                plan.changes = changes.ToArray();
                return plan;
            }
            catch
            {
                descriptor.expressionsMenu = sourceMenu;
                EditorUtility.SetDirty(descriptor);
                DestroyClonedMenus(clonedMenus);
                throw;
            }
        }

        public static void ActivateDirectMenuOverrides(MenuDirectOverridePlan plan)
        {
            if (plan == null || !plan.avatar)
                throw new InvalidOperationException(
                    "Cannot activate a menu override without a build-clone avatar.");

            foreach (var change in plan.changes)
                change.control.type = VRCExpressionsMenu.Control.ControlType.Button;
            foreach (var menu in plan.changes.Select(change => change.menu).Distinct())
                EditorUtility.SetDirty(menu);
            plan.restored = false;

            var avatarId = plan.avatar.GetInstanceID();
            lock (PendingMenuOverridesLock)
            {
                if (PendingMenuOverrides.TryGetValue(avatarId, out var previous))
                    previous.Restore();
                PendingMenuOverrides[avatarId] = plan;
            }
        }

        public static MenuDirectOverridePlan TakeDirectMenuOverrides(GameObject avatar)
        {
            if (!avatar) return null;
            var avatarId = avatar.GetInstanceID();
            lock (PendingMenuOverridesLock)
            {
                if (!PendingMenuOverrides.TryGetValue(avatarId, out var plan))
                    return null;
                PendingMenuOverrides.Remove(avatarId);
                return plan;
            }
        }

        public static void CancelDirectMenuOverrides(GameObject avatar)
        {
            var plan = TakeDirectMenuOverrides(avatar);
            if (plan != null) plan.Restore();
        }

        public static void Restore(this MenuDirectOverridePlan plan)
        {
            if (plan == null || plan.restored) return;
            foreach (var change in plan?.changes ?? Array.Empty<MenuControlChange>())
                change.control.type = change.originalType;
            foreach (var menu in plan.changes.Select(change => change.menu).Distinct())
                EditorUtility.SetDirty(menu);
            var failed = plan.changes
                .Where(change => change.control == null ||
                                 change.control.type != change.originalType)
                .Select(change => change.label)
                .ToArray();
            if (failed.Length != 0)
                throw new InvalidOperationException(
                    "Failed to restore temporary Button menu controls: " +
                    string.Join(", ", failed));
            if (plan.descriptor)
            {
                plan.descriptor.expressionsMenu = plan.originalMenu;
                EditorUtility.SetDirty(plan.descriptor);
            }
            var clonedMenus = plan.clonedMenus;
            plan.clonedMenu = null;
            plan.clonedMenus = null;
            plan.restored = true;
            DestroyClonedMenus(clonedMenus);
        }

        private static VRCExpressionsMenu[] CloneMenuGraph(VRCExpressionsMenu root)
        {
            if (!root)
                throw new InvalidOperationException(
                    "Cannot clone an empty VRCFury final menu graph.");
            var clones = new Dictionary<VRCExpressionsMenu, VRCExpressionsMenu>();
            var order = new List<VRCExpressionsMenu>();
            try
            {
                Clone(root);
                return order.ToArray();
            }
            catch
            {
                DestroyClonedMenus(order);
                throw;
            }

            VRCExpressionsMenu Clone(VRCExpressionsMenu source)
            {
                if (!source) return null;
                if (clones.TryGetValue(source, out var existing)) return existing;
                var clone = UnityEngine.Object.Instantiate(source);
                clone.name = source.name + " (FullFT in-memory)";
                clone.hideFlags = HideFlags.HideAndDontSave;
                clones[source] = clone;
                order.Add(clone);
                if (clone.controls.Count != source.controls.Count)
                    throw new InvalidOperationException(
                        $"VRCExpressionsMenu.Instantiate changed the control count " +
                        $"from {source.controls.Count} to {clone.controls.Count}.");
                for (var i = 0; i < source.controls.Count; i++)
                {
                    var sourceChild = source.controls[i]?.subMenu;
                    if (source.controls[i] != null && clone.controls[i] == null)
                        throw new InvalidOperationException(
                            "VRCExpressionsMenu.Instantiate left a null cloned control.");
                    if (source.controls[i] != null &&
                        ReferenceEquals(source.controls[i], clone.controls[i]))
                        throw new InvalidOperationException(
                            "VRCExpressionsMenu.Instantiate left Control entries shared " +
                            "with the source menu.");
                    if (!sourceChild) continue;
                    clone.controls[i].subMenu = Clone(sourceChild);
                }
                return clone;
            }
        }

        private static void DestroyClonedMenus(IEnumerable<VRCExpressionsMenu> menus)
        {
            if (menus == null) return;
            foreach (var menu in menus.Reverse())
            {
                if (!menu) continue;
                menu.hideFlags = HideFlags.HideAndDontSave;
                UnityEngine.Object.DestroyImmediate(menu);
            }
        }

        private static IEnumerable<MenuControlReference> EnumerateMenuControls(
            VRCExpressionsMenu root)
        {
            var visited = new HashSet<VRCExpressionsMenu>();
            var stack = new Stack<VRCExpressionsMenu>();
            stack.Push(root);
            while (stack.Count != 0)
            {
                var menu = stack.Pop();
                if (!menu || !visited.Add(menu)) continue;
                foreach (var control in menu.controls)
                {
                    if (control == null) continue;
                    yield return new MenuControlReference { menu = menu, control = control };
                    if (control.subMenu)
                        stack.Push(control.subMenu);
                }
            }
        }

        private static string[] ControlParameters(VRCExpressionsMenu.Control control)
        {
            var names = new List<string>();
            if (!string.IsNullOrEmpty(control.parameter?.name))
                names.Add(control.parameter.name);
            if (control.subParameters != null)
            {
                foreach (var parameter in control.subParameters)
                    if (!string.IsNullOrEmpty(parameter?.name))
                        names.Add(parameter.name);
            }
            return names.Distinct(StringComparer.Ordinal).ToArray();
        }

        public static HashSet<string> CollectCompressedParameterNames(GameObject target)
        {
            var names = new HashSet<string>(StringComparer.Ordinal);
            var descriptor = target ? target.GetComponent<VRCAvatarDescriptor>() : null;
            if (!descriptor) return names;

            var controllers = descriptor.baseAnimationLayers
                .Concat(descriptor.specialAnimationLayers)
                .Select(layer => layer.animatorController)
                .OfType<AnimatorController>()
                .Distinct();

            var allParameterNames = descriptor.expressionParameters?.parameters
                .Select(parameter => parameter.name)
                .Where(name => !string.IsNullOrEmpty(name))
                .Distinct()
                .ToArray() ?? Array.Empty<string>();

            foreach (var controller in controllers)
            {
                foreach (var layer in controller.layers)
                {
                    if (!layer.name.Contains("Parameter Compressor") &&
                        !layer.stateMachine.name.Contains("Parameter Compressor"))
                        continue;

                    foreach (var stateName in EnumerateStateNames(layer.stateMachine))
                    {
                        foreach (var parameterName in allParameterNames)
                        {
                            var pattern = $@"(?<![A-Za-z0-9_/]){Regex.Escape(parameterName)}(?![A-Za-z0-9_/])";
                            if (Regex.IsMatch(stateName, pattern, RegexOptions.CultureInvariant))
                                names.Add(parameterName);
                        }
                    }
                }
            }

            return names;
        }

        private static IEnumerable<string> EnumerateStateNames(AnimatorStateMachine machine)
        {
            if (!machine) yield break;
            foreach (var child in machine.states)
                if (child.state)
                    yield return child.state.name;
            foreach (var child in machine.stateMachines)
                foreach (var name in EnumerateStateNames(child.stateMachine))
                    yield return name;
        }

        public static bool IsRepresented(
            VRCExpressionParameters.Parameter parameter,
            HashSet<string> compressedNames)
        {
            return parameter.networkSynced || compressedNames.Contains(parameter.name);
        }

        public static object Snapshot(VRCExpressionParameters parameters, HashSet<string> compressedNames)
        {
            return parameters.parameters
                .Select(parameter => new
                {
                    name = parameter.name,
                    type = parameter.valueType.ToString(),
                    bits = Bits(parameter),
                    networkSynced = parameter.networkSynced,
                    represented = compressedNames.Contains(parameter.name) || parameter.networkSynced,
                    compressed = compressedNames.Contains(parameter.name),
                    saved = parameter.saved,
                    defaultValue = parameter.defaultValue,
                })
                .OrderBy(item => item.name, StringComparer.Ordinal)
                .ToArray();
        }
    }
}
