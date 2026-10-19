using System;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEngine;
using MorphForge.Editor;

/// <summary>Rebuild the product catalog while proving its IDs, data and references survive.</summary>
public static class CatalogProductionMigration
{
    static readonly string[] Prefabs = {
        Stage4FinishBuild.GeometryPrefabPath, Stage4FinishBuild.PlacementPrefabPath,
        Stage4FinishBuild.Stage4AvatarPath, Stage4FinishBuild.FullFTAvatarPath
    };

    public static string Run()
    {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling)
            throw new InvalidOperationException("Requires idle Edit Mode.");
        var path = CatalogMaterialBuild.MetadataAssetPath;
        var before = AssetDatabase.LoadAssetAtPath<Texture2D>(path);
        if (!before) throw new InvalidOperationException("Missing product metadata.");
        var beforeGuid = AssetDatabase.AssetPathToGUID(path);
        var beforeHash = Hash(before.GetRawTextureData());
        var beforeSize = new[] { before.width, before.height };
        var materialGuid = AssetDatabase.AssetPathToGUID(CatalogMaterialBuild.MaterialAssetPath);
        var prefabHashes = Prefabs.ToDictionary(p => p, p => Hash(File.ReadAllBytes(p)));
        var build = JObject.Parse(CatalogMaterialBuild.Build());
        var metadata = AssetDatabase.LoadAssetAtPath<Texture2D>(path);
        var material = AssetDatabase.LoadAssetAtPath<Material>(CatalogMaterialBuild.MaterialAssetPath);
        if (metadata.width != 256 || metadata.height != 64 || metadata.mipmapCount != 1 ||
            metadata.format != TextureFormat.RGBAFloat || metadata.isDataSRGB ||
            metadata.filterMode != FilterMode.Point || metadata.wrapMode != TextureWrapMode.Clamp ||
            beforeGuid != AssetDatabase.AssetPathToGUID(path) || beforeHash != Hash(metadata.GetRawTextureData()) ||
            materialGuid != AssetDatabase.AssetPathToGUID(CatalogMaterialBuild.MaterialAssetPath) ||
            material.GetFloat("_MetaWidth") != 256 || material.GetFloat("_MetaHeight") != 64 ||
            ShaderUtil.ShaderHasError(material.shader))
            throw new InvalidOperationException("Catalog migration invariants failed.");
        var prefabs = Prefabs.Select(p => {
            var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(p);
            var stores = prefab.GetComponentsInChildren<ParticleSystem>(true).Where(ps => ps.name == "Store").ToArray();
            var allBound = stores.Length == 1 && stores.All(ps =>
                ps.GetComponent<ParticleSystemRenderer>().sharedMaterial == material && ps.main.maxParticles == 16384);
            return new { path = p, stores = stores.Length, allBound, capacity = stores.Select(ps => ps.main.maxParticles).ToArray(),
                unchanged = prefabHashes[p] == Hash(File.ReadAllBytes(p)) };
        }).ToArray();
        if (prefabs.Any(p => !p.allBound || !p.unchanged))
            throw new InvalidOperationException("Product prefab catalog references/capacity changed or are invalid: " + JsonConvert.SerializeObject(prefabs));
        var report = new { success = true, generatedAtUtc = DateTime.UtcNow.ToString("O"), build,
            beforeSize, afterSize = new[] { metadata.width, metadata.height }, metadataGuid = beforeGuid,
            materialGuid, dataSha256 = beforeHash, allDataPreserved = true, guidsPreserved = true,
            shader = material.shader.name, shaderErrors = ShaderUtil.GetShaderMessages(material.shader)
                .Where(m => m.severity.ToString() == "Error").Select(m => m.message).ToArray(), prefabs };
        var json = JsonConvert.SerializeObject(report, Formatting.Indented);
        Directory.CreateDirectory("Docs/Security");
        File.WriteAllText("Docs/Security/catalog-production-migration.json", json + Environment.NewLine);
        return json;
    }

    static string Hash(byte[] bytes)
    {
        using (var sha = SHA256.Create())
            return BitConverter.ToString(sha.ComputeHash(bytes)).Replace("-", "").ToLowerInvariant();
    }
}
