using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using VRC.SDK3.Avatars.ScriptableObjects;
using nadena.dev.modular_avatar.core;

namespace MorphForgeAvatar.Editor {
    public static class MenuLocalization {
        public const string Root = AvatarSetupWindow.Root + "/Variants/LocalizedMenus";
        public static void Apply(GameObject module, string language) {
            if (!module || !module.GetComponent<MorphForgeAvatarModule>()) throw new ArgumentException("MorphForge module required");
            if (EditorApplication.isPlaying || EditorUtility.IsPersistent(module)) throw new InvalidOperationException("A scene instance in Edit Mode is required");
            if (Array.IndexOf(PluginLocalization.Codes, language) < 0) throw new ArgumentException("Unsupported menu language");
            var marker = module.GetComponent<MorphForgeAvatarModule>();
            Undo.RecordObject(marker, "Change MorphForge menu language");
            if (!Guid.TryParseExact(marker.menuInstanceId, "N", out _) ||
                UnityEngine.Object.FindObjectsOfType<MorphForgeAvatarModule>(true).Any(m => m != marker && m.menuInstanceId == marker.menuInstanceId))
                marker.menuInstanceId = Guid.NewGuid().ToString("N");
            var map = new Dictionary<VRCExpressionsMenu, VRCExpressionsMenu>();
            var destination = Root + "/" + marker.menuInstanceId + "/" + language;
            CatalogVariant.EnsureFolder(destination);
            VRCExpressionsMenu Clone(VRCExpressionsMenu menu) {
                if (!menu) return null;
                if (map.TryGetValue(menu, out var cached)) return cached;
                string sourcePath = AssetDatabase.GetAssetPath(menu);
                string id = AssetDatabase.AssetPathToGUID(sourcePath);
                // Keep canonical filenames across switches, preserving custom
                // controls in this instance instead of rereading a template.
                if (sourcePath.StartsWith(Root + "/", StringComparison.Ordinal)) {
                    id = Path.GetFileNameWithoutExtension(sourcePath);
                }
                // Unsaved custom menus receive a stable content hash for this
                // installation without touching another avatar's menu assets.
                if (string.IsNullOrEmpty(id)) id = Guid.NewGuid().ToString("N");
                string path = destination + "/" + id + ".asset";
                if (sourcePath == path) { map[menu] = menu; return menu; }
                var copy = AssetDatabase.LoadAssetAtPath<VRCExpressionsMenu>(path);
                if (!copy) { copy = UnityEngine.Object.Instantiate(menu); AssetDatabase.CreateAsset(copy, path); }
                else EditorUtility.CopySerialized(menu, copy);
                map[menu] = copy;
                copy.name = System.Text.RegularExpressions.Regex.Replace(menu.name, @" \((zh-CN|ja|en)\)$", "") + " (" + language + ")";
                foreach (var control in copy.controls) {
                    control.name = PluginLocalization.MenuText(control.name, language);
                    control.subMenu = Clone(control.subMenu);
                    if (control.labels != null) for (int i = 0; i < control.labels.Length; i++) {
                        var label = control.labels[i]; label.name = PluginLocalization.MenuText(label.name, language); control.labels[i] = label;
                    }
                }
                EditorUtility.SetDirty(copy); return copy;
            }
            foreach (var installer in module.GetComponentsInChildren<ModularAvatarMenuInstaller>(true)) {
                Undo.RecordObject(installer, "Change MorphForge menu language");
                installer.menuToAppend = Clone(installer.menuToAppend);
                installer.installTargetMenu = Clone(installer.installTargetMenu);
                EditorUtility.SetDirty(installer); PrefabUtility.RecordPrefabInstancePropertyModifications(installer);
            }
            marker.menuLanguage = language;
            EditorUtility.SetDirty(marker); PrefabUtility.RecordPrefabInstancePropertyModifications(marker);
            EditorSceneManager.MarkSceneDirty(module.scene);
            AssetDatabase.SaveAssets();
        }
    }
}
