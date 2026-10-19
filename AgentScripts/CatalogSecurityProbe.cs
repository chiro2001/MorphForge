using System;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using Newtonsoft.Json;
using UnityEditor;
using UnityEngine;

/// <summary>Isolated data-layout probes; never changes the shared catalog or Avatar.</summary>
public static class CatalogSecurityProbe
{
    const string Root = "Assets/MorphForge/Diagnostics/CatalogSecurity";
    const string Source = "Assets/MorphForge/MaterialsCatalog/ParticleBoxCatalog.mat";
    const string GridShader = "Assets/MorphForge/Shaders/ParticleBoxCatalogMetadataGrid.shader";

    public static string Build()
    {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling)
            throw new InvalidOperationException("Requires idle Edit Mode.");
        AssetDatabase.ImportAsset(GridShader, ImportAssetOptions.ForceSynchronousImport);
        var original = AssetDatabase.LoadAssetAtPath<Material>(Source);
        if (!original) throw new InvalidOperationException("Missing original catalog material.");
        var metadata = original.GetTexture("_CatalogMeta") as Texture2D;
        if (!metadata || metadata.format != TextureFormat.RGBAFloat ||
            metadata.width != 16384 || metadata.height != 1 || metadata.mipmapCount != 1)
            throw new InvalidOperationException("Unexpected source metadata format/layout.");
        var originalBytes = metadata.GetRawTextureData();
        if (originalBytes.Length != 16384 * 16)
            throw new InvalidOperationException("Unexpected source byte count.");
        var sourceHash = FileHash(Source);
        var metadataPath = AssetDatabase.GetAssetPath(metadata);
        var metadataHash = FileHash(metadataPath);
        EnsureFolder(Root);
        var narrowBytes = originalBytes.Take(4096 * 16).ToArray();
        var narrow = Texture("MetadataNarrow", 4096, 1, narrowBytes);
        var grid = Texture("MetadataGrid", 256, 64, originalBytes);
        var shader = AssetDatabase.LoadAssetAtPath<Shader>(GridShader);
        if (!shader || ShaderUtil.ShaderHasError(shader))
            throw new InvalidOperationException("Grid shader has compilation errors.");
        var narrowMaterial = Material("CatalogNarrow", original, original.shader, narrow);
        var gridMaterial = Material("CatalogGrid", original, shader, grid);
        AssetDatabase.SaveAssets();
        if (sourceHash != FileHash(Source) || metadataHash != FileHash(metadataPath))
            throw new InvalidOperationException("Shared source asset changed unexpectedly.");
        var report = new
        {
            success = true,
            generatedAtUtc = DateTime.UtcNow.ToString("O"),
            source = new { material = Source, metadata = metadataPath, sha256 = Hash(originalBytes), sourceHash, metadataHash },
            narrow = new { material = AssetDatabase.GetAssetPath(narrowMaterial), size = new[] { 4096, 1 }, styles = 1024,
                diagnosticOnly = true, sha256 = Hash(narrow.GetRawTextureData()), prefixMatches = narrow.GetRawTextureData().SequenceEqual(narrowBytes) },
            grid = new { material = AssetDatabase.GetAssetPath(gridMaterial), size = new[] { 256, 64 }, styles = 4096,
                sha256 = Hash(grid.GetRawTextureData()), allBytesMatch = grid.GetRawTextureData().SequenceEqual(originalBytes) },
            sharedAssetsUnchanged = true,
            shaderErrors = ShaderUtil.GetShaderMessages(shader).Where(m => m.severity.ToString() == "Error").Select(m => m.message).ToArray()
        };
        var json = JsonConvert.SerializeObject(report, Formatting.Indented);
        Directory.CreateDirectory("Docs/Security");
        File.WriteAllText("Docs/Security/catalog-layout-probes.json", json + Environment.NewLine);
        return json;
    }

    static Texture2D Texture(string name, int width, int height, byte[] data)
    {
        var path = Root + "/" + name + ".asset";
        if (AssetDatabase.LoadAssetAtPath<UnityEngine.Object>(path))
            throw new InvalidOperationException("Probe already exists; refusing overwrite: " + path);
        var texture = new Texture2D(width, height, TextureFormat.RGBAFloat, false, true)
        {
            name = name, filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp, anisoLevel = 0
        };
        texture.LoadRawTextureData(data);
        texture.Apply(false, false);
        AssetDatabase.CreateAsset(texture, path);
        return texture;
    }

    static Material Material(string name, Material source, Shader shader, Texture2D metadata)
    {
        var path = Root + "/" + name + ".mat";
        if (AssetDatabase.LoadAssetAtPath<UnityEngine.Object>(path))
            throw new InvalidOperationException("Probe already exists; refusing overwrite: " + path);
        var material = new Material(source) { name = name, shader = shader };
        material.SetTexture("_CatalogMeta", metadata);
        material.SetFloat("_MetaWidth", metadata.width);
        if (material.HasProperty("_MetaHeight")) material.SetFloat("_MetaHeight", metadata.height);
        AssetDatabase.CreateAsset(material, path);
        return material;
    }

    static void EnsureFolder(string path)
    {
        var parent = "Assets";
        foreach (var name in path.Split('/').Skip(1))
        {
            var next = parent + "/" + name;
            if (!AssetDatabase.IsValidFolder(next)) AssetDatabase.CreateFolder(parent, name);
            parent = next;
        }
    }

    static string FileHash(string path) => Hash(File.ReadAllBytes(path));
    static string Hash(byte[] bytes)
    {
        using (var sha = SHA256.Create())
            return BitConverter.ToString(sha.ComputeHash(bytes)).Replace("-", "").ToLowerInvariant();
    }
}
