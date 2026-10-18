using System;
using System.Collections.Generic;
using System.Linq;
using UnityEngine;

namespace MorphForge.Editor
{
    /// <summary>
    /// Independent, recoverable policy for the FullFT joint-build test avatar.
    /// The policy is applied only to a VRChat build clone whose root name starts
    /// with avatarNamePrefix. It is never written back to the source avatar.
    /// </summary>
    [CreateAssetMenu(
        menuName = "MorphForge/FullFT Sync Profile",
        fileName = "FullFT_SyncProfile",
        order = 100)]
    public sealed class FullFTSyncProfile : ScriptableObject
    {
        public const int CurrentSchemaVersion = 2;
        public const string DefaultAssetPath = "Assets/MorphForge/FullFT/FullFT_SyncProfile.asset";
        public const string DefaultAvatarNamePrefix = "MorphForge_FullFT";
        public const string TestedVrcFuryVersion = "1.1430.0";

        public static readonly string[] VrcftControlParameters =
        {
            "EyeTrackingActive",
            "LipTrackingActive",
            "EyeDilationEnable",
            "VisemesEnable",
            "FacialExpressionsDisabled",
            "FaceTrackingEmulation",
            "FaceTrackingLimits",
            "MixExpressions",
        };

        [Serializable]
        public sealed class SyncDisableGroup
        {
            public string id;
            public string label;
            [TextArea] public string remoteImpact;
            public string[] parameters;
        }

        public int schemaVersion = CurrentSchemaVersion;
        public string profileVersion = "fullft-sync-v2";
        public bool enabled = true;
        public string avatarNamePrefix = DefaultAvatarNamePrefix;

        public string[] protectedParameterPrefixes =
        {
            "MF2_",
            "MF3_",
            "MF_",
        };

        public string[] protectedParameters = VrcftControlParameters.ToArray();
        public string[] localOnlyParameters = { "FT/Debug" };

        public SyncDisableGroup[] disableGroups = DefaultDisableGroups();

        public int expectedFtSyncedBits = 161;
        public int expectedFtSyncedParameters = 112;
        public int expectedVrcftControlParameters = 8;
        public int expectedGeometryBits = 56;
        public int expectedPlacementBits = 14;
        public int maxFinalBits = 256;

        public bool preserveUnknownSync = true;
        public bool allowMissingDisabledGroups = true;
        public bool requireExactFtBitCount = true;
        public bool requireExactFtManifest = true;
        public bool requireMorphForgeGeometry = true;
        public bool forcePlacementDirect = true;
        public bool forceGeometryDirect = true;
        public bool requireTestedVrcFuryVersion = true;
        public string testedVrcFuryVersion = TestedVrcFuryVersion;

        [TextArea]
        public string notes =
            "Source avatar is never modified. The Max-200 callback edits only the build clone. " +
            "Unknown body parameters keep their original networkSynced value. " +
            "FullFT and MorphForge identities are checked by exact manifest. " +
            "Protected menu controls are temporarily changed to Button only while " +
            "the tested VRCFury compressor runs, then restored before the build finishes.";

        public IEnumerable<string> DisabledParameterNames()
        {
            if (disableGroups == null) yield break;
            foreach (var group in disableGroups)
            {
                if (group?.parameters == null) continue;
                foreach (var parameter in group.parameters)
                    if (!string.IsNullOrWhiteSpace(parameter))
                        yield return parameter;
            }
        }

        public bool IsProtected(string parameterName)
        {
            if (string.IsNullOrEmpty(parameterName)) return false;
            if (FullFTSyncManifest.FtNames.Contains(parameterName)) return true;
            if (protectedParameters != null &&
                protectedParameters.Any(x => string.Equals(x, parameterName, StringComparison.Ordinal)))
                return true;
            return protectedParameterPrefixes != null &&
                   protectedParameterPrefixes.Any(x =>
                       !string.IsNullOrEmpty(x) &&
                       parameterName.StartsWith(x, StringComparison.Ordinal));
        }

        public void ResetToDefaults()
        {
            schemaVersion = CurrentSchemaVersion;
            profileVersion = "fullft-sync-v2";
            enabled = true;
            avatarNamePrefix = DefaultAvatarNamePrefix;
            protectedParameterPrefixes = new[] { "MF2_", "MF3_", "MF_" };
            protectedParameters = VrcftControlParameters.ToArray();
            localOnlyParameters = new[] { "FT/Debug" };
            disableGroups = DefaultDisableGroups();
            expectedFtSyncedBits = 161;
            expectedFtSyncedParameters = 112;
            expectedVrcftControlParameters = 8;
            expectedGeometryBits = 56;
            expectedPlacementBits = 14;
            maxFinalBits = 256;
            preserveUnknownSync = true;
            allowMissingDisabledGroups = true;
            requireExactFtBitCount = true;
            requireExactFtManifest = true;
            requireMorphForgeGeometry = true;
            forcePlacementDirect = true;
            forceGeometryDirect = true;
            requireTestedVrcFuryVersion = true;
            testedVrcFuryVersion = TestedVrcFuryVersion;
            notes =
                "Source avatar is never modified. The Max-200 callback edits only the build clone. " +
                "Unknown body parameters keep their original networkSynced value. " +
                "FT protection is the exact 112-parameter manifest; FT/Debug remains local. " +
                "FullFT and MorphForge identities are checked by exact manifest. " +
                "Protected menu controls are temporarily changed to Button only while " +
                "the tested VRCFury compressor runs, then restored before the build finishes.";
        }

        public static FullFTSyncProfile CreateDefault()
        {
            var profile = CreateInstance<FullFTSyncProfile>();
            profile.name = "FullFT_SyncProfile";
            profile.ResetToDefaults();
            return profile;
        }

        private static SyncDisableGroup[] DefaultDisableGroups()
        {
            return new[]
            {
                new SyncDisableGroup
                {
                    id = "foot-toe",
                    label = "Foot/toe tracking and animation",
                    remoteImpact =
                        "Remote viewers no longer receive toe axis values or toe control toggles; " +
                        "the remote toe PhysBone/animation state can fall back to its default.",
                    parameters = new[]
                    {
                        "AshiyubiLX",
                        "AshiyubiLY",
                        "AshiyubiRX",
                        "AshiyubiRY",
                        "ConstToggle",
                        "Toe",
                        "IsAshiyubiControllL",
                        "IsAshiyubiControllR",
                    },
                },
                new SyncDisableGroup
                {
                    id = "wand",
                    label = "Nemesis wand and projectile state",
                    remoteImpact =
                        "Remote viewers no longer receive wand take/reload/aim/fire/selection state. " +
                        "Non-local clients may render the default wand or omit transient effects.",
                    parameters = new[]
                    {
                        "Nem/AllowTakeWand",
                        "Nem/CloseFlag",
                        "Nem/Enhancement",
                        "Nem/Enhancemented",
                        "Nem/IsLightWand",
                        "Nem/NeedReload",
                        "Nem/Reload",
                        "Nem/Reticle",
                        "Nem/ReverceGrip",
                        "Nem/SelectParticle",
                        "Nem/ShotCount",
                        "Nem/ShotParticle",
                        "Nem/TakeWand",
                        "Nem/WandWorldFix",
                        "Nem/WavingReverce",
                        "Nem/WavingWand",
                    },
                },
                new SyncDisableGroup
                {
                    id = "sleep-supine",
                    label = "Sleep, crouch and Supine state",
                    remoteImpact =
                        "Remote viewers no longer receive sleep/supine, crouch pose, root-height or " +
                        "foot-anchor state. This group is optional and is normally absent from the " +
                        "minimal FullFT test avatar.",
                    parameters = new[]
                    {
                        "CrouchPose",
                        "RootHeight",
                        "SetRootHeight",
                        "VRCFootAnchor",
                        "VRCLockPose",
                        "VRCSupine",
                        "VRCSupinePoseAdjust",
                    },
                },
                new SyncDisableGroup
                {
                    id = "legacy-facial-selector",
                    label = "Legacy facial preset selector",
                    remoteImpact =
                        "Remote viewers no longer receive the locally selected legacy facial set. " +
                        "The source drivers are local-only, so remote clients otherwise stay at set 0. " +
                        "Current FaceLock/Fasial result state remains protected.",
                    parameters = new[]
                    {
                        "GestureNum",
                        "FasialSetNumGlobal",
                    },
                },
                new SyncDisableGroup
                {
                    id = "fullft-cohabitation-trim",
                    label = "Nonessential FullFT cohabitation controls",
                    remoteImpact =
                        "Remote viewers no longer receive the legacy manual FaceLock/Fasial preset " +
                        "selection, shield toggle, jump-collider toggle, face-petting gesture state, " +
                        "head-petting toggle, or tail-collider toggle. High-frequency FT/v2 tracking, " +
                        "the eight VRCFT controls, MF3 geometry, and placement remain direct.",
                    parameters = new[]
                    {
                        "FaceLock",
                        "Fasial",
                        "AllowShield",
                        "ColliderJump",
                        "NadeNadeFace",
                        "NadeNadeHeadToggle",
                        "TailColliderDisable",
                    },
                },
            };
        }
    }
}
