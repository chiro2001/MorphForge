using System;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEngine;
using UnityEngine.SceneManagement;

public static class HedgehogTrialBootstrap {
    const string Project="D:/Projects/VRChatProjects2/MorphForgeHedgehogDemo_1_4_0";
    static void Guard(){
        if(!Application.dataPath.Replace('\\','/').Equals(Project+"/Assets",StringComparison.OrdinalIgnoreCase))throw new Exception("Dedicated MorphForge hedgehog trial project required");
        if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Ready Edit Mode required");
    }
    static string Hash(string path){using(var f=File.OpenRead(path))using(var h=SHA256.Create())return BitConverter.ToString(h.ComputeHash(f)).Replace("-","").ToLowerInvariant();}
    public static string Inspect(){
        Guard();return new JObject{["unityVersion"]=Application.unityVersion,["compiling"]=EditorApplication.isCompiling,["sceneDirty"]=SceneManager.GetActiveScene().isDirty,["authorPrefabPresent"]=File.Exists("Assets/Saltroche/256Hedgehog/256Hedgehog.prefab"),["modulePresent"]=File.Exists("Assets/MorphForgeAvatar/Generated/MorphForge.prefab")}.ToString();
    }
    public static string Import(){
        Guard();if(Enumerable.Range(0,SceneManager.sceneCount).Select(SceneManager.GetSceneAt).Any(s=>s.isDirty))throw new Exception("Unsaved scene preserved");
        string author="SourcePackages/256Hedgehog.unitypackage",module="SourcePackages/MorphForgeAvatar-1.4.0.unitypackage";
        string a=Hash(author),m=Hash(module);
        if(a!="9b31eb126014f85584a776b39d5bc33bb3d6755c7d8c43055fc979f4b1d12afb"||m!="7e60412f16cbc3e9858e7c85c7d02ec9c16d15af94f6c37805190b59c5a39264")throw new Exception("Original source packages differ");
        Directory.CreateDirectory("TrialReview");
        var state=new JObject{["authorPackageSha256"]=a,["modulePackageSha256"]=m,["defaultLanguage"]="ja",["method"]="AssetDatabase.ImportPackage",["requestedUtc"]=DateTime.UtcNow.ToString("O")};
        if(!AssetDatabase.IsValidFolder("Assets/Saltroche/256Hedgehog"))AssetDatabase.ImportPackage(author,false);
        if(!AssetDatabase.IsValidFolder("Assets/MorphForgeAvatar"))AssetDatabase.ImportPackage(module,false);
        File.WriteAllText("TrialReview/import.json",state.ToString());return "Original CC0 hedgehog and validated MorphForge package import requested; wait for compilation";
    }
}
