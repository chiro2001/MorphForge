using System;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;

// Bootstrap deliberately has no MorphForge references: it must run with the
// old plugin removed and before the buyer's original package is imported.
public static class BoothCleanImport {
    public const string Plugin = "Assets/MorphForgeAvatar";
    public const string Reports = "E:/tmp/morphforge/booth-20261025/clean-import";
    public const string Package = "D:/Projects/VRChatProjects2/MorphForge/Releases/MorphForge-0.27.0/BOOTH-Publish/Download/MorphForge_Ver.0.27.0/Unity/MorphForgeAvatar-1.4.0.unitypackage";
    public const string PackageHash = "7e60412f16cbc3e9858e7c85c7d02ec9c16d15af94f6c37805190b59c5a39264";
    static void Guard() {
        if (!Application.dataPath.Replace('\\','/').EndsWith("/avatar-plugin/CleanProject/Assets", StringComparison.OrdinalIgnoreCase)) throw new Exception("Dedicated CleanProject required");
        if (EditorApplication.isPlaying || EditorApplication.isCompiling) throw new Exception("Ready Edit Mode required");
    }
    public static string Inspect() {
        Guard();
        var scenes = new JArray(Enumerable.Range(0,SceneManager.sceneCount).Select(i=>{
            var s=SceneManager.GetSceneAt(i);return new JObject{["path"]=s.path,["dirty"]=s.isDirty,["roots"]=new JArray(s.GetRootGameObjects().Select(g=>g.name))};
        }));
        var scripts=AssetDatabase.FindAssets("t:MonoScript",new[]{"Assets"}).Select(AssetDatabase.GUIDToAssetPath).ToArray();
        var result=new JObject{["unityVersion"]=Application.unityVersion,["scenes"]=scenes,["pluginPresent"]=AssetDatabase.IsValidFolder(Plugin),["scriptsOutsidePlugin"]=new JArray(scripts.Where(p=>!p.StartsWith(Plugin+"/"))),["pluginFileCount"]=Directory.Exists(Plugin)?Directory.GetFiles(Plugin,"*",SearchOption.AllDirectories).Count(f=>!f.EndsWith(".meta")):0};
        Directory.CreateDirectory(Reports);File.WriteAllText(Reports+"/before.json",result.ToString());return result.ToString();
    }
    public static string RemoveOldPlugin() {
        Guard();
        if (Enumerable.Range(0,SceneManager.sceneCount).Select(SceneManager.GetSceneAt).Any(s=>s.isDirty))throw new Exception("Unsaved scene detected; do not discard user work");
        if (File.Exists(Reports+"/removed.json"))throw new Exception("Removal already recorded; inspect the previous run before retrying");
        var scenes=new JArray(EditorSceneManager.GetSceneManagerSetup().Select(s=>new JObject{["path"]=s.path,["isLoaded"]=s.isLoaded,["isActive"]=s.isActive}));
        File.WriteAllText(Reports+"/original-scenes.json",scenes.ToString());
        EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);
        bool existed=AssetDatabase.IsValidFolder(Plugin);
        if (existed) {
            AssetDatabase.ExportPackage(Plugin,Reports+"/old-plugin-backup.unitypackage",ExportPackageOptions.Recurse);
            if (!AssetDatabase.DeleteAsset(Plugin))throw new Exception("Failed to delete old plugin with AssetDatabase");
        }
        var remaining=AssetDatabase.FindAssets("t:MonoScript",new[]{"Assets"}).Select(AssetDatabase.GUIDToAssetPath).Where(p=>File.ReadAllText(p).Contains("namespace MorphForgeAvatar")).ToArray();
        if(remaining.Length>0)throw new Exception("MorphForge code outside public plugin remains: "+string.Join(",",remaining));
        var result=new JObject{["success"]=true,["oldPluginExisted"]=existed,["removedViaAssetDatabase"]=existed,["pluginFolderAbsent"]=!AssetDatabase.IsValidFolder(Plugin),["remainingMorphForgeScripts"]=new JArray(remaining),["backupCreated"]=existed};
        File.WriteAllText(Reports+"/removed.json",result.ToString());AssetDatabase.Refresh();return result.ToString();
    }
    public static string ImportOriginal() {
        Guard();
        if(AssetDatabase.IsValidFolder(Plugin))throw new Exception("Plugin folder must be absent before first import");
        var removed=JObject.Parse(File.ReadAllText(Reports+"/removed.json"));
        if(!(bool)removed["pluginFolderAbsent"])throw new Exception("Clean removal record required");
        string digest;using(var stream=File.OpenRead(Package))using(var algorithm=SHA256.Create())digest=BitConverter.ToString(algorithm.ComputeHash(stream)).Replace("-","").ToLowerInvariant();
        if(digest!=PackageHash)throw new Exception("Buyer package hash differs");
        File.WriteAllText(Reports+"/import-request.json",new JObject{["packageSha256"]=digest,["oldPluginAbsentBeforeImport"]=true,["method"]="AssetDatabase.ImportPackage"}.ToString());
        AssetDatabase.ImportPackage(Package,false);return "Original buyer package import requested; wait for import and compilation";
    }
}
