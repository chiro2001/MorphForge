using System;
using System.Collections.Generic;
using System.Linq;
using VRC.SDK3.Avatars.ScriptableObjects;

namespace MorphForge.Editor
{
    /// <summary>
    /// Exact expression-parameter identities taken from the current FullFT
    /// source avatar. Prefix checks alone cannot detect a Modular Avatar rename
    /// or an accidental replacement with another FT-like parameter.
    /// </summary>
    internal static class FullFTSyncManifest
    {
        internal sealed class ExpectedParameter
        {
            public readonly string name;
            public readonly VRCExpressionParameters.ValueType valueType;

            public ExpectedParameter(string name, VRCExpressionParameters.ValueType valueType)
            {
                this.name = name;
                this.valueType = valueType;
            }

            public int Bits => valueType == VRCExpressionParameters.ValueType.Bool ? 1 : 8;
        }

        internal const int FtParameterCount = 112;
        internal const int FtBits = 161;
        internal const int FtBoolCount = 105;
        internal const int FtFloatCount = 7;

        internal static readonly ExpectedParameter[] FtParameters =
        {
            new ExpectedParameter("FT/AutoToggle", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/EyeSync", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/BrowExpressionLeft1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/BrowExpressionLeft2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/BrowExpressionLeft4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/BrowExpressionLeftNegative", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/BrowExpressionRight1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/BrowExpressionRight2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/BrowExpressionRight4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/BrowExpressionRightNegative", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/CheekPuffSuckLeft1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/CheekPuffSuckLeft2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/CheekPuffSuckLeft4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/CheekPuffSuckLeftNegative", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/CheekPuffSuckRight1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/CheekPuffSuckRight2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/CheekPuffSuckRight4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/CheekPuffSuckRightNegative", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/EyeLeftX", VRCExpressionParameters.ValueType.Float),
            new ExpectedParameter("FT/v2/EyeLidLeft", VRCExpressionParameters.ValueType.Float),
            new ExpectedParameter("FT/v2/EyeLidRight", VRCExpressionParameters.ValueType.Float),
            new ExpectedParameter("FT/v2/EyeRightX", VRCExpressionParameters.ValueType.Float),
            new ExpectedParameter("FT/v2/EyeSquintLeft1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/EyeSquintLeft2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/EyeSquintLeft4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/EyeSquintRight1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/EyeSquintRight2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/EyeSquintRight4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/EyeY", VRCExpressionParameters.ValueType.Float),
            new ExpectedParameter("FT/v2/JawForward1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/JawForward2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/JawForward4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/JawOpen", VRCExpressionParameters.ValueType.Float),
            new ExpectedParameter("FT/v2/JawX1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/JawX2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/JawX4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/JawXNegative", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/LipFunnel1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/LipFunnel2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/LipFunnel4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/LipPucker1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/LipPucker2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/LipPucker4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/LipSuckLower1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/LipSuckLower2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/LipSuckLower4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/LipSuckUpper1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/LipSuckUpper2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/LipSuckUpper4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthClosed", VRCExpressionParameters.ValueType.Float),
            new ExpectedParameter("FT/v2/MouthLowerDown1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthLowerDown2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthLowerDown4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthPress1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthPress2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthPress4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthRaiserLower1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthRaiserLower2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthRaiserLower4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthRaiserUpper1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthRaiserUpper2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthRaiserUpper4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthStretchLeft1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthStretchLeft2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthStretchLeft4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthStretchRight1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthStretchRight2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthStretchRight4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthTightenerLeft1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthTightenerLeft2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthTightenerLeft4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthTightenerRight1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthTightenerRight2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthTightenerRight4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthUpperUpLeft1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthUpperUpLeft2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthUpperUpLeft4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthUpperUpRight1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthUpperUpRight2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthUpperUpRight4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthX1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthX2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthX4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/MouthXNegative", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/NoseSneer1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/NoseSneer2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/NoseSneer4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/PupilDilation1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/PupilDilation2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/PupilDilation4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/SmileFrownLeft1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/SmileFrownLeft2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/SmileFrownLeft4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/SmileFrownLeftNegative", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/SmileFrownRight1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/SmileFrownRight2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/SmileFrownRight4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/SmileFrownRightNegative", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/TongueOut1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/TongueOut2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/TongueOut4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/TongueRoll1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/TongueRoll2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/TongueRoll4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/TongueX1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/TongueX2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/TongueX4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/TongueXNegative", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/TongueY1", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/TongueY2", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/TongueY4", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("FT/v2/TongueYNegative", VRCExpressionParameters.ValueType.Bool),
        };

        internal static readonly ExpectedParameter[] PlacementParameters =
        {
            new ExpectedParameter("MF_World", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("MF_Handle", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("MF_Scale", VRCExpressionParameters.ValueType.Float),
            new ExpectedParameter("MF_Reset", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("MF_GrabL", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("MF_GrabR", VRCExpressionParameters.ValueType.Bool),
            new ExpectedParameter("MF_Level", VRCExpressionParameters.ValueType.Bool),
        };

        internal static readonly ExpectedParameter[] GeometryParameters =
        {
            new ExpectedParameter("MF3_Ctl", VRCExpressionParameters.ValueType.Int),
            new ExpectedParameter("MF3_D00", VRCExpressionParameters.ValueType.Int),
            new ExpectedParameter("MF3_D01", VRCExpressionParameters.ValueType.Int),
            new ExpectedParameter("MF3_D02", VRCExpressionParameters.ValueType.Int),
            new ExpectedParameter("MF3_D03", VRCExpressionParameters.ValueType.Int),
            new ExpectedParameter("MF3_D04", VRCExpressionParameters.ValueType.Int),
            new ExpectedParameter("MF3_D05", VRCExpressionParameters.ValueType.Int),
        };

        internal static readonly HashSet<string> FtNames = new HashSet<string>(
            FtParameters.Select(parameter => parameter.name), StringComparer.Ordinal);

        internal static readonly HashSet<string> PlacementNames = new HashSet<string>(
            PlacementParameters.Select(parameter => parameter.name), StringComparer.Ordinal);

        internal static readonly HashSet<string> GeometryNames = new HashSet<string>(
            GeometryParameters.Select(parameter => parameter.name), StringComparer.Ordinal);

        internal static ExpectedParameter[] VrcftControls(IEnumerable<string> names)
        {
            return names.Select(name => new ExpectedParameter(
                name, VRCExpressionParameters.ValueType.Bool)).ToArray();
        }

        internal static void Validate()
        {
            ValidateSet(
                "FullFT face tracking", FtParameters, FtParameterCount, FtBits,
                FtBoolCount, FtFloatCount);
            ValidateSet("MorphForge placement", PlacementParameters, 7, 14, 6, 1);
            ValidateSet("MorphForge geometry", GeometryParameters, 7, 56, 0, 0);
        }

        private static void ValidateSet(
            string label,
            ExpectedParameter[] parameters,
            int expectedCount,
            int expectedBits,
            int expectedBoolCount,
            int expectedFloatCount)
        {
            var count = parameters?.Length ?? 0;
            var bits = parameters?.Sum(parameter => parameter.Bits) ?? 0;
            var boolCount = parameters?.Count(parameter =>
                parameter.valueType == VRCExpressionParameters.ValueType.Bool) ?? 0;
            var floatCount = parameters?.Count(parameter =>
                parameter.valueType == VRCExpressionParameters.ValueType.Float) ?? 0;
            var uniqueCount = parameters?
                .Select(parameter => parameter.name)
                .Distinct(StringComparer.Ordinal)
                .Count() ?? 0;
            if (count != expectedCount || bits != expectedBits ||
                boolCount != expectedBoolCount || floatCount != expectedFloatCount ||
                uniqueCount != expectedCount)
            {
                throw new InvalidOperationException(
                    $"{label} parameter manifest is inconsistent: " +
                    $"count={count}/{expectedCount}, bits={bits}/{expectedBits}, " +
                    $"bool={boolCount}/{expectedBoolCount}, " +
                    $"float={floatCount}/{expectedFloatCount}, unique={uniqueCount}/{expectedCount}.");
            }
        }
    }
}
