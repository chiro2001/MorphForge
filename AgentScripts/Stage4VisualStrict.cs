using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEngine;

/// <summary>
/// Strict image-level wrapper around Stage4FinishBuild.ValidateVisual.
/// Lives outside Assets so it is never included in the Avatar. It does not
/// inject particles into the product path and never enables OSC.
/// </summary>
public static class Stage4VisualStrict
{
    const string Evidence = "Docs/Materials/native";
    const string ReportPath = "Docs/Materials/native-strict-validation.json";

    public static string Run()
    {
        if (!EditorApplication.isPlaying)
            throw new InvalidOperationException("Stage4VisualStrict requires Editor Play Mode after the long test ends.");

        var raw = MorphForge.Editor.Stage4FinishBuild.ValidateVisual();
        var result = JObject.Parse(raw);
        var checks = new List<object>();
        Action<string, bool, object> check = (name, pass, detail) =>
        {
            checks.Add(new { name, pass, detail });
        };

        check("base visual report completed", result["completed"]?.Value<bool>() == true, null);
        check("base visual report success", result["success"]?.Value<bool>() == true, null);
        var baseChecks = result["checks"] as JArray;
        if (baseChecks == null || baseChecks.Count == 0)
            throw new InvalidOperationException("Base visual report has no checks.");
        void AddBaseChecks(JToken item)
        {
            if (item is JArray array)
            {
                foreach (var child in array) AddBaseChecks(child);
                return;
            }
            if (item.Type != JTokenType.Object) return;
            check(
                "base: " + (string)item["name"],
                item["pass"]?.Value<bool>() == true,
                item);
        }
        foreach (var item in baseChecks) AddBaseChecks(item);
        var particleStats = result["stats"]?["particleStats"] as JObject;
        foreach (var systemName in new[] { "animation", "sixFace", "styles", "scale" })
        {
            var system = particleStats?[systemName];
            var streams = system?["streams"] as JArray;
            var streamNames = streams == null
                ? Array.Empty<string>()
                : streams.Select(value => (string)value).ToArray();
            var particleCount = system?["count"]?.Value<int>() ?? 0;
            var requiredStreams = new[] { "Normal", "UV", "UV2", "SizeXYZ" };
            check(
                "native fixture system " + systemName,
                system != null &&
                particleCount > 0 &&
                (string)system["shader"] == "MorphForge/ParticleBoxCatalog" &&
                requiredStreams.All(streamNames.Contains),
                system);
        }

        var animation = LoadPng("fixture25-frames.png", 640, 480, checks);
        var animationExpected = new[] { "blue", "red", "purple" };
        // Camera at +Z looking toward the origin has screen-right = world -X.
        // The fixture places frame 0/1/2 at world -1/0/+1, therefore the
        // saved image order right-to-left is blue/red/purple.
        var animationX = new[] { 587, 320, 53 };
        for (var index = 0; index < animationExpected.Length; index++)
        {
            var coverage = ClassCoverage(
                animation,
                640,
                480,
                animationX[index],
                240,
                34,
                animationExpected[index]);
            check(
                "decoded fixture25 " + index + " " + animationExpected[index],
                coverage.foregroundFraction >= .45f && coverage.expectedCoverage >= .72f,
                coverage);
        }

        var faceNames = new[] { "px", "nx", "py", "ny", "pz", "nz" };
        var faceExpected = new[] { "red", "green", "blue", "yellow", "cyan", "magenta" };
        for (var index = 0; index < faceNames.Length; index++)
        {
            var image = LoadPng("fixture26-" + faceNames[index] + ".png", 384, 384, checks);
            var coverage = ClassCoverage(
                image,
                384,
                384,
                192,
                192,
                42,
                faceExpected[index]);
            check(
                "decoded fixture26 " + faceNames[index] + " " + faceExpected[index],
                coverage.foregroundFraction >= .55f && coverage.expectedCoverage >= .82f,
                coverage);
        }

        LoadPng("catalog-styles.png", 800, 480, checks);
        var scaleOne = LoadPng("scale-1.png", 512, 512, checks);
        var scaleHalf = LoadPng("scale-half.png", 512, 512, checks);
        var scaleDifference = CroppedPixelDifference(scaleOne, scaleHalf, 512, 512, 256, 256, 108);
        check(
            "decoded scale 0.5 per-pixel difference",
            scaleDifference.meanAbsoluteError < .06 &&
            scaleDifference.changedFraction < .30,
            scaleDifference);
        var transitionsOne = LuminanceTransitions(scaleOne, 512, 512, 256, 156, 356);
        var transitionsHalf = LuminanceTransitions(scaleHalf, 512, 512, 256, 156, 356);
        check(
            "decoded scale 0.5 keeps checker transition count",
            transitionsOne > 0 &&
            Math.Abs(transitionsOne - transitionsHalf) <= 1,
            new { transitionsOne, transitionsHalf, tolerance = 1 });
        var rigidBefore = LoadPng("rigid-root-before.png", 512, 512, checks);
        var rigidAfter = LoadPng("rigid-root-after.png", 512, 512, checks);
        var rigidDifference = CroppedPixelDifference(
            rigidBefore,
            rigidAfter,
            512,
            512,
            256,
            256,
            180);
        check(
            "decoded static texture unchanged after rigid root transform",
            rigidDifference.meanAbsoluteError < .01f &&
            rigidDifference.changedFraction < .03f,
            rigidDifference);

        var report = new
        {
            success = checks.All(item => BoolProperty(item, "pass")),
            completed = true,
            scope = "Strict checks of PNGs generated from the native Material fixture ParticleSystem; no OSC and no product particle injection.",
            oscUsed = false,
            productParticlesInjected = false,
            testRig = "temporary non-product ParticleSystem",
            checks,
            sourceReport = raw
        };
        WriteJson(ReportPath, report);
        var json = Newtonsoft.Json.JsonConvert.SerializeObject(report, Newtonsoft.Json.Formatting.Indented);
        if (!(bool)report.GetType().GetProperty("success").GetValue(report))
            throw new InvalidOperationException("STAGE4_VISUAL_STRICT_FAILED\n" + json);
        return json;
    }

    static bool BoolProperty(object value, string property)
    {
        return value.GetType().GetProperty(property)?.GetValue(value) is bool flag && flag;
    }

    static Color32[] LoadPng(string relative, int width, int height, List<object> checks)
    {
        var full = ProjectPath(Evidence + "/" + relative);
        if (!File.Exists(full))
        {
            checks.Add(new { name = "PNG exists " + relative, pass = false, detail = "missing" });
            return new Color32[width * height];
        }
        var bytes = File.ReadAllBytes(full);
        var header = bytes.Length >= 24 &&
                     bytes[0] == 0x89 && bytes[1] == 0x50 && bytes[2] == 0x4e && bytes[3] == 0x47;
        if (!header)
        {
            checks.Add(new { name = "PNG header " + relative, pass = false, detail = "not PNG" });
            return new Color32[width * height];
        }
        var texture = new Texture2D(2, 2, TextureFormat.RGBA32, false, false);
        try
        {
            if (!texture.LoadImage(bytes))
                throw new InvalidOperationException("Cannot decode evidence PNG " + relative);
            var sizeMatches = texture.width == width && texture.height == height;
            checks.Add(new
            {
                name = "PNG dimensions " + relative,
                pass = sizeMatches,
                detail = new { expected = new[] { width, height }, actual = new[] { texture.width, texture.height }, bytes = bytes.Length }
            });
            return sizeMatches ? texture.GetPixels32() : new Color32[width * height];
        }
        finally
        {
            UnityEngine.Object.DestroyImmediate(texture);
        }
    }

    static CoverageResult ClassCoverage(
        Color32[] pixels,
        int width,
        int height,
        int x,
        int y,
        int radius,
        string expected)
    {
        var foreground = 0;
        var expectedPixels = 0;
        var total = 0;
        var classified = new Dictionary<string, int>(StringComparer.Ordinal);
        for (var py = Mathf.Max(0, y - radius); py <= Mathf.Min(height - 1, y + radius); py++)
        for (var px = Mathf.Max(0, x - radius); px <= Mathf.Min(width - 1, x + radius); px++)
        {
            total++;
            var color = pixels[py * width + px];
            if (Mathf.Max(color.r, Mathf.Max(color.g, color.b)) <= 3) continue;
            foreground++;
            var kind = ColorClass(color);
            classified.TryGetValue(kind, out var count);
            classified[kind] = count + 1;
            if (kind == expected) expectedPixels++;
        }
        var coverage = foreground == 0 ? 0f : expectedPixels / (float)foreground;
        return new CoverageResult
        {
            x = x,
            y = y,
            radius = radius,
            expected = expected,
            foreground = foreground,
            total = total,
            foregroundFraction = total == 0 ? 0f : foreground / (float)total,
            expectedPixels = expectedPixels,
            expectedCoverage = coverage,
            classified = classified
        };
    }

    static string ColorClass(Color32 color)
    {
        var r = color.r / 255f;
        var g = color.g / 255f;
        var b = color.b / 255f;
        var maximum = Mathf.Max(r, Mathf.Max(g, b));
        if (maximum < .02f) return "black";

        // Hue-ratio tests are intentionally insensitive to the shader's
        // lighting multiplier while separating the six fixture colors.
        if (r > g * 1.20f && b > g * 1.20f)
        {
            var balance = Mathf.Abs(r - b) / Mathf.Max(.001f, Mathf.Max(r, b));
            if (balance < .38f) return maximum < .40f ? "purple" : "magenta";
            return r > b ? "red" : "blue";
        }
        if (r > b * 1.35f && g > b * 1.35f)
            return Mathf.Abs(r - g) / Mathf.Max(.001f, Mathf.Max(r, g)) < .38f ? "yellow" : (r > g ? "red" : "green");
        if (g > r * 1.35f && b > r * 1.35f)
            return "cyan";
        if (r > g * 1.25f && r > b * 1.25f) return "red";
        if (g > r * 1.25f && g > b * 1.25f) return "green";
        if (b > r * 1.25f && b > g * 1.25f) return "blue";
        if (r > g * 1.15f && b > g * 1.15f) return "magenta";
        return "unknown";
    }

    static DifferenceResult CroppedPixelDifference(
        Color32[] first,
        Color32[] second,
        int width,
        int height,
        int x,
        int y,
        int radius)
    {
        long error = 0;
        var total = 0;
        var changed = 0;
        for (var py = Mathf.Max(0, y - radius); py <= Mathf.Min(height - 1, y + radius); py++)
        for (var px = Mathf.Max(0, x - radius); px <= Mathf.Min(width - 1, x + radius); px++)
        {
            var a = first[py * width + px];
            var b = second[py * width + px];
            var delta = Mathf.Abs(a.r - b.r) + Mathf.Abs(a.g - b.g) + Mathf.Abs(a.b - b.b);
            error += delta;
            if (delta > 18) changed++;
            total++;
        }
        return new DifferenceResult
        {
            meanAbsoluteError = total == 0 ? 0f : error / (total * 3f * 255f),
            changedFraction = total == 0 ? 0f : changed / (float)total,
            total = total
        };
    }

    static int LuminanceTransitions(
        Color32[] pixels,
        int width,
        int height,
        int y,
        int startX,
        int endX)
    {
        var transitions = 0;
        var previous = -1f;
        for (var x = Mathf.Max(1, startX); x < Mathf.Min(width, endX); x++)
        {
            var color = pixels[y * width + x];
            var luminance = .2126f * color.r + .7152f * color.g + .0722f * color.b;
            if (previous >= 0f && Mathf.Abs(luminance - previous) > 1.5f) transitions++;
            previous = luminance;
        }
        return transitions;
    }

    static string ProjectPath(string relative)
    {
        return Path.GetFullPath(Path.Combine(Application.dataPath, "..", relative));
    }

    static void WriteJson(string relative, object value)
    {
        var path = ProjectPath(relative);
        Directory.CreateDirectory(Path.GetDirectoryName(path));
        File.WriteAllText(path, Newtonsoft.Json.JsonConvert.SerializeObject(value, Newtonsoft.Json.Formatting.Indented) + Environment.NewLine);
    }

    sealed class CoverageResult
    {
        public int x;
        public int y;
        public int radius;
        public string expected;
        public int foreground;
        public int total;
        public float foregroundFraction;
        public int expectedPixels;
        public float expectedCoverage;
        public Dictionary<string, int> classified;
    }

    sealed class DifferenceResult
    {
        public float meanAbsoluteError;
        public float changedFraction;
        public int total;
    }
}
