using System;
using System.IO;
using System.IO.Compression;
using System.Linq;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;

namespace MorphForgeAvatar.Editor {
    public static class CatalogVariant {
        public static string Build(string source) {
            if (EditorApplication.isPlaying || EditorApplication.isCompiling) throw new InvalidOperationException("需要空闲 Edit Mode。");
            var jsonPath = Path.Combine(source, "catalog.json");
            var catalog = JObject.Parse(File.ReadAllText(jsonPath));
            if ((string)catalog["format"] != "MorphForgeMaterialCatalog/1") throw new InvalidOperationException("材质目录格式不支持。");
            // Receiver abilities come from the shipped template, not an older
            // Desktop material export that predates text/motion support.
            var templateCatalog = JObject.Parse(File.ReadAllText(AvatarSetupWindow.Root + "/CatalogSource/catalog.json"));
            catalog["capabilities"] = templateCatalog["capabilities"].DeepClone();
            EnsureFolder(AvatarSetupWindow.Root + "/Variants");
            var root = AssetDatabase.GenerateUniqueAssetPath(AvatarSetupWindow.Root + "/Variants/Catalog");
            if (string.IsNullOrEmpty(root)) throw new InvalidOperationException("无法创建独立材质目录。");
            try {
                EnsureFolder(root); EnsureFolder(root + "/Source/artifacts");
                File.WriteAllText(root + "/Source/catalog.json", catalog.ToString());
                foreach (var name in ((JArray)catalog["materials"]).OfType<JObject>().SelectMany(m => (m["textures"] as JArray) ?? new JArray()).Select(t => (string)t).Distinct()) {
                    const string prefix = "/artifacts/";
                    if (!name.StartsWith(prefix, StringComparison.Ordinal)) throw new InvalidOperationException("纹理必须来自目录 artifacts。");
                    var filename = name.Substring(prefix.Length);
                    if (filename != Path.GetFileName(filename) || !filename.EndsWith(".png", StringComparison.OrdinalIgnoreCase)) throw new InvalidOperationException("非法纹理文件名。");
                    File.Copy(Path.Combine(source, "artifacts", filename), root + "/Source/artifacts/" + filename, false);
                }
                AssetDatabase.Refresh();
                CatalogMaterialBuild.BuildAt(root + "/Source", root + "/Material");
                var template = AssetDatabase.LoadAssetAtPath<GameObject>(AvatarSetupWindow.PrefabPath);
                if (!template) throw new InvalidOperationException("基础 Prefab 缺失。");
                var clone = (GameObject)PrefabUtility.InstantiatePrefab(template);
                try {
                    PrefabUtility.UnpackPrefabInstance(clone, PrefabUnpackMode.Completely, InteractionMode.AutomatedAction);
                    var sourceMaterial = AssetDatabase.LoadAssetAtPath<Material>(root + "/Material/ParticleBoxCatalog.mat");
                    var surface = new Material(sourceMaterial) { name = "Surface", shader = Shader.Find("MorphForgeAvatar/ParticleBoxCatalogMetadataGrid"), renderQueue = 3000 };
                    surface.SetFloat("_WireEncoding", 6);
                    AssetDatabase.CreateAsset(surface, root + "/Material/Surface.mat");
                    foreach (var renderer in clone.GetComponentsInChildren<ParticleSystemRenderer>(true).Where(r => r.name == "Store"))
                        renderer.sharedMaterial = surface;
                    var decoder = clone.GetComponentsInChildren<Component>(true).Single(c => c && c.GetType().FullName == "nadena.dev.modular_avatar.core.ModularAvatarMergeAnimator" && c.gameObject.name == "MF6Decoder");
                    var so = new SerializedObject(decoder); var controllerProperty = so.FindProperty("animator");
                    var oldPath = AssetDatabase.GetAssetPath(controllerProperty.objectReferenceValue);
                    var controllerPath = root + "/Decoder.controller";
                    if (!AssetDatabase.CopyAsset(oldPath, controllerPath)) throw new InvalidOperationException("复制 Decoder 失败。");
                    controllerProperty.objectReferenceValue = AssetDatabase.LoadAssetAtPath<AnimatorController>(controllerPath); so.ApplyModifiedPropertiesWithoutUndo();
                    var identity = CatalogIdentity.Apply(clone, catalog);
                    var marker = clone.GetComponent<MorphForgeAvatarModule>(); marker.catalogIdentity = identity;
                    var path = root + "/MorphForge.prefab";
                    PrefabUtility.SaveAsPrefabAsset(clone, path); AssetDatabase.SaveAssets(); return path;
                } finally { UnityEngine.Object.DestroyImmediate(clone); }
            } catch { AssetDatabase.DeleteAsset(root); throw; }
        }
        public static void ExportBuiltin(string path) {
            var source = AvatarSetupWindow.Root + "/CatalogSource";
            // The shipped public source tree contains only original materials.
            // Export exact files, never recursively bundle unrelated user data.
            using (var file = File.Create(path)) using (var zip = new ZipArchive(file, ZipArchiveMode.Create)) {
                zip.CreateEntryFromFile(source + "/catalog.json", "catalog.json");
                var catalog = JObject.Parse(File.ReadAllText(source + "/catalog.json"));
                foreach (var name in ((JArray)catalog["materials"]).OfType<JObject>().SelectMany(m => (m["textures"] as JArray) ?? new JArray()).Select(t => (string)t).Distinct()) {
                    var filename = Path.GetFileName(name); zip.CreateEntryFromFile(source + "/artifacts/" + filename, "artifacts/" + filename);
                }
                zip.CreateEntryFromFile(source + "/LICENSE.txt", "LICENSE.txt");
            }
        }
        public static void EnsureFolder(string path) {
            string current = "Assets";
            foreach (var part in path.Split('/').Skip(1)) { var next = current + "/" + part; if (!AssetDatabase.IsValidFolder(next)) AssetDatabase.CreateFolder(current, part); current = next; }
        }
    }
}
