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
    /// Runs after VRCFury's Max-100 Parameter Compressor and evaluates the actual
    /// final Expression Parameters plus the generated compressor carrier.
    /// </summary>
    public sealed class FullFTBudgetGate : IVRCSDKPreprocessAvatarCallback
    {
        public int callbackOrder => int.MaxValue - 50;

        public bool OnPreprocessAvatar(GameObject avatarObject)
        {
            var profile = FullFTBudgetUtil.LoadProfile();
            if (!FullFTBudgetUtil.IsFullFTTarget(avatarObject, profile)) return true;

            FullFTBudgetUtil.MenuDirectOverridePlan menuOverridePlan = null;
            try
            {
                if (!profile.enabled)
                    throw new InvalidOperationException("FullFT sync profile is disabled.");

                var descriptor = avatarObject.GetComponent<VRCAvatarDescriptor>();
                var parameters = descriptor ? descriptor.expressionParameters : null;
                if (parameters == null)
                    throw new InvalidOperationException("Final FullFT parameters are missing.");

                var compressed = FullFTBudgetUtil.CollectCompressedParameterNames(avatarObject);
                menuOverridePlan = FullFTBudgetUtil.TakeDirectMenuOverrides(avatarObject);
                if ((profile.forcePlacementDirect || profile.forceGeometryDirect) &&
                    menuOverridePlan == null)
                    throw new InvalidOperationException(
                        "Forced-direct menu override state is missing at Max-50. " +
                        "The Max-200 callback did not run or was interrupted.");

                var finalCost = parameters.CalcTotalCost();
                var maxBits = FinalBitLimit();
                var ftAudit = FullFTBudgetUtil.AuditManifest(
                    parameters,
                    FullFTSyncManifest.FtParameters,
                    compressed,
                    "FT/");
                var controlAudit = FullFTBudgetUtil.AuditManifest(
                    parameters,
                    FullFTSyncManifest.VrcftControls(
                        profile.protectedParameters ?? Array.Empty<string>()),
                    compressed,
                    null);
                var geometryAudit = FullFTBudgetUtil.AuditManifest(
                    parameters,
                    FullFTSyncManifest.GeometryParameters,
                    compressed,
                    "MF3_");
                var placementAudit = FullFTBudgetUtil.AuditManifest(
                    parameters,
                    FullFTSyncManifest.PlacementParameters,
                    compressed,
                    "MF_");
                var duplicates = FullFTBudgetUtil.DuplicateParameterGroups(parameters);
                var disabledNames = profile.DisabledParameterNames()
                    .Distinct(StringComparer.Ordinal).ToArray();
                var disabledStillRepresented = disabledNames.Where(name =>
                {
                    var parameter = parameters.parameters.FirstOrDefault(item =>
                        string.Equals(item.name, name, StringComparison.Ordinal));
                    return parameter != null &&
                           FullFTBudgetUtil.IsRepresented(parameter, compressed);
                }).ToArray();
                var localOnlyNames = profile.localOnlyParameters
                    .Distinct(StringComparer.Ordinal).ToArray();
                var localStillRepresented = localOnlyNames.Where(name =>
                {
                    var parameter = parameters.parameters.FirstOrDefault(item =>
                        string.Equals(item.name, name, StringComparison.Ordinal));
                    return parameter != null &&
                           FullFTBudgetUtil.IsRepresented(parameter, compressed);
                }).ToArray();
                var ftCountOk = profile.expectedFtSyncedParameters == ftAudit.representedCount;
                var ftBitsOk = !profile.requireExactFtBitCount ||
                               ftAudit.representedBits == profile.expectedFtSyncedBits;
                var ftDirectOk = ftAudit.allDirect;
                var geometryOk = !profile.requireMorphForgeGeometry ||
                                 (geometryAudit.representedBits == profile.expectedGeometryBits &&
                                  (!profile.forceGeometryDirect || geometryAudit.allDirect));
                var placementOk = !profile.requireMorphForgeGeometry ||
                                  (placementAudit.representedBits == profile.expectedPlacementBits &&
                                   (!profile.forcePlacementDirect || placementAudit.allDirect));

                var checks = new List<object>
                {
                    new { name = "Final bits within profile limit", pass = finalCost <= maxBits },
                    new { name = "No duplicate expression parameter names", pass = duplicates.Length == 0 },
                    new { name = "FT exact manifest names and types",
                        pass = !profile.requireExactFtManifest || ftAudit.exact },
                    new { name = "All VRCFT controls represented",
                        pass = controlAudit.exact &&
                               controlAudit.representedCount ==
                               profile.expectedVrcftControlParameters },
                    new { name = "FT represented count is 112", pass = ftCountOk },
                    new { name = "FT represented bits are 161", pass = ftBitsOk },
                    new { name = "FT is fully direct (161 physical bits)", pass = ftDirectOk },
                    new { name = "MorphForge geometry is 56 bits and direct when required",
                        pass = geometryOk },
                    new { name = "MorphForge placement is 14 bits and direct when required",
                        pass = placementOk },
                    new { name = "Disabled groups are not represented", pass = disabledStillRepresented.Length == 0 },
                    new { name = "Local-only FullFT parameters are not represented",
                        pass = localStillRepresented.Length == 0 },
                };

                var compressorStructure = new
                {
                    parameters = parameters.parameters
                        .Where(parameter => parameter.name.StartsWith("SyncData", StringComparison.Ordinal) ||
                                            parameter.name.StartsWith("SyncIndex", StringComparison.Ordinal))
                        .Select(parameter => new
                        {
                            name = parameter.name,
                            type = parameter.valueType.ToString(),
                            bits = FullFTBudgetUtil.Bits(parameter),
                            networkSynced = parameter.networkSynced,
                        })
                        .OrderBy(item => item.name, StringComparer.Ordinal)
                        .ToArray(),
                    compressedParameters = compressed.OrderBy(name => name, StringComparer.Ordinal).ToArray(),
                };

                var menuRestoreError = string.Empty;
                try
                {
                    if (menuOverridePlan != null) menuOverridePlan.Restore();
                }
                catch (Exception exception)
                {
                    menuRestoreError = exception.ToString();
                }
                checks.Add(new
                {
                    name = "Temporary menu control types restored",
                    pass = menuOverridePlan == null ||
                           (menuOverridePlan.restored &&
                            string.IsNullOrEmpty(menuRestoreError)),
                });
                var pass = checks.All(item =>
                    (bool)item.GetType().GetProperty("pass").GetValue(item));

                FullFTBudgetUtil.WriteReport("Docs/FullFT/final-budget.json", new
                {
                    success = pass,
                    stage = "Max-50, after VRCFury Parameter Compressor",
                    scope = "Actual VRChat SDK preprocess result on a build clone; not an upload",
                    avatar = avatarObject.name,
                    profile = profile.profileVersion,
                    finalBits = finalCost,
                    maxBits,
                    remainingBits = maxBits - finalCost,
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
                    menuOverride = new
                    {
                        restored = menuOverridePlan == null || menuOverridePlan.restored,
                        restoreError = menuRestoreError,
                        plan = menuOverridePlan?.ToReport(),
                    },
                    disabled = profile.disableGroups.Select(group => new
                    {
                        group.id,
                        group.label,
                        group.remoteImpact,
                        present = group.parameters.Where(name => parameters.parameters.Any(
                            parameter => parameter.name == name)).ToArray(),
                        stillRepresented = group.parameters.Where(name => disabledStillRepresented.Contains(
                            name, StringComparer.Ordinal)).ToArray(),
                    }).ToArray(),
                    localOnly = localOnlyNames.Select(name => new
                    {
                        name,
                        present = parameters.parameters.Any(parameter =>
                            string.Equals(parameter.name, name, StringComparison.Ordinal)),
                        stillRepresented = localStillRepresented.Contains(
                            name, StringComparer.Ordinal),
                    }).ToArray(),
                    compressorStructure,
                    parameters = FullFTBudgetUtil.Snapshot(parameters, compressed),
                    checks,
                });

                if (!pass)
                {
                    Debug.LogError(
                        $"FullFT final budget failed: {finalCost}/{maxBits} bit; " +
                        $"FT {ftAudit.representedBits}/{ftAudit.representedCount}; " +
                        $"controls {controlAudit.representedCount}/{profile.expectedVrcftControlParameters}; " +
                        $"geometry {geometryAudit.representedBits} direct={geometryAudit.allDirect}; " +
                        $"placement {placementAudit.representedBits} direct={placementAudit.allDirect}; " +
                        $"duplicates={string.Join(", ", duplicates.Select(group => group.name))}; " +
                        $"menu restore={menuRestoreError}; " +
                        $"disabled still represented: {string.Join(", ", disabledStillRepresented)}.");
                    return false;
                }

                Debug.Log(
                    $"FullFT joint budget passed: {finalCost}/{maxBits} bit, " +
                    $"FT={ftAudit.representedBits} ({ftAudit.representedCount}), " +
                    $"controls={controlAudit.representedCount}/{profile.expectedVrcftControlParameters}, " +
                    $"geometry={geometryAudit.representedBits}, " +
                    $"placement={placementAudit.representedBits}.");
                return true;
            }
            catch (Exception exception)
            {
                try
                {
                    if (menuOverridePlan != null) menuOverridePlan.Restore();
                }
                catch
                {
                    // Preserve the original exception in the build report.
                }
                Debug.LogException(exception);
                FullFTBudgetUtil.WriteReport("Docs/FullFT/final-budget-error.json", new
                {
                    success = false,
                    stage = "Max-50",
                    avatar = avatarObject ? avatarObject.name : "<missing>",
                    error = exception.ToString(),
                });
                return false;
            }
        }

        private static int FinalBitLimit()
        {
            var dexProtectPresent = AppDomain.CurrentDomain.GetAssemblies()
                .Any(assembly => string.Equals(
                    assembly.GetName().Name,
                    "DexProtectEditor",
                    StringComparison.Ordinal));
            return dexProtectPresent ? 237 : 256;
        }
    }
}
