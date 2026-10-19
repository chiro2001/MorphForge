using System;
using System.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine.SceneManagement;
using MorphForge.Editor;
using Newtonsoft.Json.Linq;
public static class Mf4Finish
{
    public static string Run()
    {
        if(EditorApplication.isPlaying)throw new Exception("Edit Mode required.");
        var scene=SceneManager.GetSceneByPath(Mf4AvatarBuild.ScenePath);
        if(scene.IsValid()&&scene.isLoaded){if(scene.isDirty)EditorSceneManager.SaveScene(scene);EditorSceneManager.CloseScene(scene,true);}
        var original=SceneManager.GetSceneByPath("Assets/MorphForge/Diagnostics/FinalFullFT/MorphForge_FinalFullFT_Diagnostic.unity");
        if(original.IsValid()&&original.isLoaded)SceneManager.SetActiveScene(original);
        return new JObject{["success"]=true,["mf4SceneSavedOnDisk"]=true,["playStopped"]=!EditorApplication.isPlaying,["originalScenes"]=new JArray(Enumerable.Range(0,SceneManager.sceneCount).Select(i=>{var s=SceneManager.GetSceneAt(i);return new JObject{["path"]=s.path,["dirty"]=s.isDirty};}))}.ToString();
    }
}
