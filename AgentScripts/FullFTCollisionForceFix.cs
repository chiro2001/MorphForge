using System;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using MorphForge.Editor;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEngine;

public static class FullFTCollisionForceFix
{
    const string Suffix = "/LusterRailGun/LusterRailGun_Toggle/Shot/Coin/Light";

    public static string Run()
    {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling)
            throw new InvalidOperationException("Requires idle Edit Mode.");
        var path = FullFTBuild.TestAvatarPath;
        var beforeHash = HashFile(path);
        var baseHash = HashFile(FullFTBuild.BaseAvatarPath);
        var contents = PrefabUtility.LoadPrefabContents(path);
        object report;
        try
        {
            var systems = contents.GetComponentsInChildren<ParticleSystem>(true);
            var target = systems.Single(ps => AnimationUtility.CalculateTransformPath(ps.transform, contents.transform).EndsWith(Suffix, StringComparison.Ordinal));
            var targetPath = AnimationUtility.CalculateTransformPath(target.transform, contents.transform);
            var beforeForce = target.collision.colliderForce;
            var beforeMask = target.collision.collidesWith.value;
            var beforeEnabled = target.collision.enabled;
            var settings = systems.Select(SettingsWithoutForce).ToArray();
            var cleanup = JObject.FromObject(FullFTBuild.SanitizePickupParticleForces(contents));
            if ((int)cleanup["changed"] != 1 || beforeForce == 0 || target.collision.colliderForce != 0 ||
                target.collision.collidesWith.value != beforeMask || target.collision.enabled != beforeEnabled ||
                !settings.SequenceEqual(systems.Select(SettingsWithoutForce)))
                throw new InvalidOperationException("Unexpected changes during pickup-force repair.");
            PrefabUtility.SaveAsPrefabAsset(contents, path);
            report = new { success = true, scope = "Only final FullFT prefab particle colliderForce changed; commercial Nemesis source remains untouched.",
                generatedAtUtc = DateTime.UtcNow.ToString("O"), nemesisSourceModified = false, fullFtSourceModified = true,
                prefabPath = path, prefabSha256Before = beforeHash, prefabSha256After = HashFile(path),
                targetPath, colliderForceBefore = beforeForce, colliderForceAfter = target.collision.colliderForce,
                pickupForceDisabled = true, disabledPickupLayers = false, collisionMask = beforeMask,
                collisionEnabled = beforeEnabled, otherParticleSettingsPreserved = true, cleanup,
                nemesisSourceSha256Before = baseHash, nemesisSourceSha256After = HashFile(FullFTBuild.BaseAvatarPath) };
        }
        finally { PrefabUtility.UnloadPrefabContents(contents); }
        if (baseHash != HashFile(FullFTBuild.BaseAvatarPath))
            throw new InvalidOperationException("Commercial base prefab changed unexpectedly.");
        var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(path);
        var stored = prefab.GetComponentsInChildren<ParticleSystem>(true).Single(ps =>
            AnimationUtility.CalculateTransformPath(ps.transform, prefab.transform).EndsWith(Suffix, StringComparison.Ordinal));
        if (stored.collision.colliderForce != 0)
            throw new InvalidOperationException("Collision-force override did not persist in the final prefab.");
        var json = JsonConvert.SerializeObject(report, Formatting.Indented);
        Directory.CreateDirectory("Docs/Security");
        File.WriteAllText("Docs/Security/fullft-collision-force-fix.json", json + Environment.NewLine);
        return json;
    }

    static string SettingsWithoutForce(ParticleSystem system)
    {
        var tree = JObject.Parse(EditorJsonUtility.ToJson(system));
        foreach (var property in tree.Descendants().OfType<JProperty>().Where(p => p.Name == "colliderForce").ToArray())
            property.Remove();
        return tree.ToString(Formatting.None);
    }
    static string HashFile(string path)
    {
        using (var sha = SHA256.Create())
            return BitConverter.ToString(sha.ComputeHash(File.ReadAllBytes(path))).Replace("-", "").ToLowerInvariant();
    }
}
