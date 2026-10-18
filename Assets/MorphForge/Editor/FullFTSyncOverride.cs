using System;
using System.Collections.Generic;
using System.Linq;
using UnityEditor;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase.Editor.BuildPipeline;

namespace MorphForge.Editor
{
    /// <summary>
    /// Runs after MA/NDMF and VRCFury injection, but before VRCFury's compressor.
    /// It modifies only the VRChat build clone selected by the independent profile.
    /// </summary>
    public sealed class FullFTSyncOverride : IVRCSDKPreprocessAvatarCallback
    {
        public int callbackOrder => int.MaxValue - 200;

        public bool OnPreprocessAvatar(GameObject avatarObject)
        {
            var profile = FullFTBudgetUtil.LoadProfile();
            if (!FullFTBudgetUtil.IsFullFTTarget(avatarObject, profile)) return true;

            FullFTBudgetUtil.MenuDirectOverridePlan menuOverridePlan = null;
            try
            {
                if (!profile.enabled)
                    throw new InvalidOperationException(
                        "The FullFT build marker matched, but its sync profile is disabled.");
                if (profile.schemaVersion != FullFTSyncProfile.CurrentSchemaVersion)
                    throw new InvalidOperationException(
                        $"Unsupported FullFT profile schema {profile.schemaVersion}.");

                var descriptor = avatarObject.GetComponent<VRCAvatarDescriptor>();
                if (!descriptor)
                    throw new InvalidOperationException("FullFT target has no VRCAvatarDescriptor.");
                var parameters = descriptor.expressionParameters;
                if (parameters == null)
                    throw new InvalidOperationException(
                        "FullFT target has no Expression Parameters. VRCFury injection did not run.");

                var compressorAlreadyPresent = FullFTBudgetUtil
                    .CollectCompressedParameterNames(avatarObject);
                if (compressorAlreadyPresent.Count != 0)
                    throw new InvalidOperationException(
                        "A Parameter Compressor layer ran before the FullFT override. " +
                        "The build-callback order no longer matches this profile.");

                var actualVrcFuryVersion = FullFTBudgetUtil.VrcFuryPackageVersion();
                var versionOk = !profile.requireTestedVrcFuryVersion ||
                                string.Equals(
                                    actualVrcFuryVersion,
                                    profile.testedVrcFuryVersion,
                                    StringComparison.Ordinal);
                var baselineOk =
                    profile.expectedFtSyncedBits == FullFTSyncManifest.FtBits &&
                    profile.expectedFtSyncedParameters == FullFTSyncManifest.FtParameterCount &&
                    profile.expectedGeometryBits == 56 &&
                    profile.expectedPlacementBits == 14 &&
                    profile.expectedVrcftControlParameters ==
                    (profile.protectedParameters?.Length ?? 0);
                var original = parameters.parameters
                    .Where(parameter => parameter != null &&
                                        !string.IsNullOrEmpty(parameter.name))
                    .GroupBy(parameter => parameter.name, StringComparer.Ordinal)
                    .ToDictionary(
                        group => group.Key,
                        group => group.Select(parameter => parameter.networkSynced).ToArray(),
                        StringComparer.Ordinal);

                var disabledNames = profile.DisabledParameterNames().Distinct(StringComparer.Ordinal).ToArray();
                var localOnlyNames = profile.localOnlyParameters
                    .Distinct(StringComparer.Ordinal).ToArray();
                foreach (var parameter in disabledNames)
                {
                    if (profile.IsProtected(parameter))
                        throw new InvalidOperationException(
                            $"Profile attempts to disable protected parameter '{parameter}'.");
                }

                var ftDebugChanged =
                    FullFTBudgetUtil.SetNetworkSynced(
                        parameters, localOnlyNames, false) > 0;
                FullFTBudgetUtil.SetNetworkSynced(parameters, disabledNames, false);
                EditorUtility.SetDirty(parameters);

                var noCompressedNames = new HashSet<string>(StringComparer.Ordinal);
                var ftAudit = FullFTBudgetUtil.AuditManifest(
                    parameters,
                    FullFTSyncManifest.FtParameters,
                    noCompressedNames,
                    "FT/");
                var controlAudit = FullFTBudgetUtil.AuditManifest(
                    parameters,
                    FullFTSyncManifest.VrcftControls(
                        profile.protectedParameters ?? Array.Empty<string>()),
                    noCompressedNames,
                    null);
                var geometryAudit = FullFTBudgetUtil.AuditManifest(
                    parameters,
                    FullFTSyncManifest.GeometryParameters,
                    noCompressedNames,
                    "MF3_");
                var placementAudit = FullFTBudgetUtil.AuditManifest(
                    parameters,
                    FullFTSyncManifest.PlacementParameters,
                    noCompressedNames,
                    "MF_");
                var duplicates = FullFTBudgetUtil.DuplicateParameterGroups(parameters);
                var missingDisabled = disabledNames
                    .Where(name => !parameters.parameters.Any(
                        parameter => string.Equals(parameter.name, name, StringComparison.Ordinal)))
                    .ToArray();
                var disabledStillRepresented = disabledNames
                    .Where(name => parameters.parameters.Any(parameter =>
                        string.Equals(parameter.name, name, StringComparison.Ordinal) &&
                        FullFTBudgetUtil.IsRepresented(parameter, noCompressedNames)))
                    .ToArray();
                var localStillRepresented = localOnlyNames
                    .Where(name => parameters.parameters.Any(parameter =>
                        string.Equals(parameter.name, name, StringComparison.Ordinal) &&
                        FullFTBudgetUtil.IsRepresented(parameter, noCompressedNames)))
                    .ToArray();

                var checks = new List<object>
                {
                    new { name = "Profile enabled", pass = profile.enabled },
                    new { name = "Tested VRCFury version", pass = versionOk },
                    new { name = "FT/Debug remains local",
                        pass = localStillRepresented.Length == 0 },
                    new { name = "Profile baseline matches exact manifest", pass = baselineOk },
                    new { name = "No duplicate expression parameter names", pass = duplicates.Length == 0 },
                    new { name = "FT exact manifest names and types",
                        pass = !profile.requireExactFtManifest || ftAudit.exact },
                    new { name = "FT represented bits equal 161",
                        pass = ftAudit.representedBits == profile.expectedFtSyncedBits },
                    new { name = "FT represented parameter count equal 112",
                        pass = ftAudit.representedCount == profile.expectedFtSyncedParameters },
                    new { name = "All 8 VRCFT controls present with exact Bool types",
                        pass = controlAudit.exact },
                    new { name = "All VRCFT controls synchronized",
                        pass = controlAudit.representedCount ==
                               profile.expectedVrcftControlParameters },
                    new { name = "MorphForge geometry exact manifest synchronized",
                        pass = !profile.requireMorphForgeGeometry ||
                               (geometryAudit.allRepresented &&
                                geometryAudit.representedBits == profile.expectedGeometryBits) },
                    new { name = "MorphForge placement exact manifest synchronized",
                        pass = !profile.requireMorphForgeGeometry ||
                               (placementAudit.allRepresented &&
                                placementAudit.representedBits == profile.expectedPlacementBits) },
                    new { name = "Disabled groups are not represented",
                        pass = disabledStillRepresented.Length == 0 },
                    new { name = "Optional disabled groups may be absent",
                        pass = profile.allowMissingDisabledGroups || missingDisabled.Length == 0 },
                };

                var pass = checks.All(item =>
                    (bool)item.GetType().GetProperty("pass").GetValue(item));
                var latest = parameters.parameters
                    .Where(parameter => parameter != null &&
                                        !string.IsNullOrEmpty(parameter.name))
                    .GroupBy(parameter => parameter.name, StringComparer.Ordinal)
                    .ToDictionary(
                        group => group.Key,
                        group => group.Select(parameter => parameter.networkSynced).ToArray(),
                        StringComparer.Ordinal);
                var changed = original.Keys
                    .Union(latest.Keys, StringComparer.Ordinal)
                    .Select(name =>
                    {
                        original.TryGetValue(name, out var before);
                        latest.TryGetValue(name, out var after);
                        return new
                        {
                            name,
                            before = before ?? Array.Empty<bool>(),
                            after = after ?? Array.Empty<bool>(),
                        };
                    })
                    .Where(item => !item.before.SequenceEqual(item.after))
                    .Select(item => new
                    {
                        item.name,
                        item.before,
                        item.after,
                    })
                    .OrderBy(item => item.name, StringComparer.Ordinal)
                    .ToArray();

                var directTargets =
                    (profile.forcePlacementDirect
                        ? placementAudit.entries
                            .Where(entry => entry.networkSynced)
                            .Select(entry => entry.name)
                        : Enumerable.Empty<string>())
                    .Concat(profile.forceGeometryDirect
                        ? geometryAudit.entries
                            .Where(entry => entry.networkSynced)
                            .Select(entry => entry.name)
                        : Enumerable.Empty<string>())
                    .Distinct(StringComparer.Ordinal)
                    .ToArray();
                menuOverridePlan = FullFTBudgetUtil.PlanDirectMenuOverrides(
                    avatarObject, directTargets);

                if (pass)
                {
                    FullFTBudgetUtil.ActivateDirectMenuOverrides(menuOverridePlan);
                }

                FullFTBudgetUtil.WriteReport("Docs/FullFT/pre-compression.json", new
                {
                    success = pass,
                    stage = "Max-200, after VRCFury injection and before Parameter Compressor",
                    scope = "Build clone only; source avatar was not modified",
                    avatar = avatarObject.name,
                    profile = profile.profileVersion,
                    vrcFuryVersion = actualVrcFuryVersion,
                    totalBits = parameters.CalcTotalCost(),
                    ft = ftAudit.ToReport(),
                    controls = controlAudit.ToReport(),
                    morphForge = new
                    {
                        forcePlacementDirect = profile.forcePlacementDirect,
                        forceGeometryDirect = profile.forceGeometryDirect,
                        placement = placementAudit.ToReport(),
                        geometry = geometryAudit.ToReport(),
                    },
                    duplicates,
                    disabled = profile.disableGroups.Select(group => new
                    {
                        group.id,
                        group.label,
                        group.remoteImpact,
                        present = group.parameters.Where(name => parameters.parameters.Any(
                            parameter => parameter.name == name)).ToArray(),
                        stillRepresented = group.parameters.Where(disabledStillRepresented.Contains).ToArray(),
                    }).ToArray(),
                    localOnly = localOnlyNames.Select(name => new
                    {
                        name,
                        present = parameters.parameters.Any(parameter =>
                            string.Equals(parameter.name, name, StringComparison.Ordinal)),
                        changedToLocal = ftDebugChanged,
                        represented = localStillRepresented.Contains(
                            name, StringComparer.Ordinal),
                    }).ToArray(),
                    missingDisabled,
                    changed,
                    menuOverride = menuOverridePlan.ToReport(),
                    unknownNetworkSynced = parameters.parameters
                        .Where(parameter => parameter.networkSynced)
                        .Where(parameter => !profile.IsProtected(parameter.name))
                        .Where(parameter => !disabledNames.Contains(parameter.name, StringComparer.Ordinal))
                        .Select(parameter => parameter.name)
                        .OrderBy(name => name, StringComparer.Ordinal)
                        .ToArray(),
                    checks,
                });

                if (!pass)
                {
                    Debug.LogError(
                        $"FullFT pre-compression validation failed. " +
                        $"VRCFury={actualVrcFuryVersion}/{profile.testedVrcFuryVersion}; " +
                        $"missing FT={string.Join(", ", ftAudit.missing)}; " +
                        $"FT={ftAudit.representedBits} bit/{ftAudit.representedCount}, " +
                        $"missing controls={string.Join(", ", controlAudit.missing)}; " +
                        $"geometry={geometryAudit.representedBits}, " +
                        $"placement={placementAudit.representedBits}; " +
                        $"duplicates={string.Join(", ", duplicates.Select(group => group.name))}.");
                    return false;
                }

                Debug.Log(
                    $"FullFT sync profile applied to build clone: FT={ftAudit.representedBits} bit, " +
                    $"VRCFT controls={controlAudit.representedCount}/{profile.expectedVrcftControlParameters}, " +
                    $"placement={placementAudit.representedBits}, " +
                    $"geometry={geometryAudit.representedBits}, " +
                    $"forced-direct controls={menuOverridePlan.changes.Length}, " +
                    $"total pre-compression={parameters.CalcTotalCost()}.");
                return true;
            }
            catch (Exception exception)
            {
                FullFTBudgetUtil.CancelDirectMenuOverrides(avatarObject);
                Debug.LogException(exception);
                FullFTBudgetUtil.WriteReport("Docs/FullFT/pre-compression-error.json", new
                {
                    success = false,
                    stage = "Max-200",
                    avatar = avatarObject ? avatarObject.name : "<missing>",
                    error = exception.ToString(),
                });
                return false;
            }
        }
    }
}
