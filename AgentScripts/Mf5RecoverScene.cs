using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using Newtonsoft.Json.Linq;
public static class Mf5RecoverScene
{
    const string Original="Assets/MorphForge/Diagnostics/FinalFullFT/MorphForge_FinalFullFT_Diagnostic.unity";
    const string Recovered="Assets/MorphForge/Diagnostics/Recovery20261020/UnsavedFullFT.unity";
    public static string Run()
    {
        if(EditorApplication.isPlaying)throw new Exception("Edit Mode required.");
        string folder="Assets/MorphForge/Diagnostics/Recovery20261020";if(!AssetDatabase.IsValidFolder(folder))AssetDatabase.CreateFolder("Assets/MorphForge/Diagnostics","Recovery20261020");
        if(!AssetDatabase.LoadAssetAtPath<SceneAsset>(Recovered)) {
            string backup=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),"tmp/morphforge/mf5-editor-recovery/scenes/0.backup");
            FileUtil.CopyFileOrDirectory(backup,Recovered);AssetDatabase.ImportAsset(Recovered,ImportAssetOptions.ForceUpdate);
        }
        var scene=SceneManager.GetSceneByPath(Recovered);if(!scene.IsValid()||!scene.isLoaded)scene=EditorSceneManager.OpenScene(Recovered,OpenSceneMode.Additive);
        var proof=JObject.Parse(File.ReadAllText("Docs/MF5/test-isolation.json"));int restored=0;
        var objects=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<Transform>(true)).Select(t=>t.gameObject).ToArray();
        foreach(var record in proof["records"].Where(r=>(string)r["scene"]==Original)) {
            var id=((string)record["globalId"]).Split('-');
            var go=objects.SingleOrDefault(g=>{var p=GlobalObjectId.GetGlobalObjectIdSlow(g).ToString().Split('-');return p.Length==id.Length&&p[3]==id[3]&&p[4]==id[4];});
            if(!go)throw new Exception("Recovered scene object does not match archived identity "+record["globalId"]);
            go.SetActive((bool)record["active"]);restored++;
        }
        EditorSceneManager.SaveScene(scene);SceneManager.SetActiveScene(scene);
        var original=SceneManager.GetSceneByPath(Original);if(original.IsValid()&&original.isLoaded&&!original.isDirty)EditorSceneManager.CloseScene(original,true);
        // Existing SDK/user scene assets are not overwritten. Restore unaffected
        // original objects after this Editor restart using stable GlobalObjectIds.
        foreach(var record in proof["records"].Where(r=>(string)r["scene"]!=Original)) {
            if(GlobalObjectId.TryParse((string)record["globalId"],out var id)){var go=GlobalObjectId.GlobalObjectIdentifierToObjectSlow(id) as GameObject;if(go&&go.scene.IsValid())go.SetActive((bool)record["active"]);}
        }
        proof["suspended"]=false;File.WriteAllText("Docs/MF5/test-isolation.json",proof.ToString());
        var report=new JObject{["success"]=true,["originalAssetOverwritten"]=false,["recoveredScene"]=Recovered,["sourceBackup"]="mf5-editor-recovery/scenes/0.backup",["restoredTemporaryToggles"]=restored,["timeUtc"]=DateTime.UtcNow.ToString("O")};File.WriteAllText("Docs/MF5/recovery.json",report.ToString());return report.ToString();
    }
}
