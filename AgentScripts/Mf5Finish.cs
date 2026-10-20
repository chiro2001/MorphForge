using System;
using System.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine.SceneManagement;
using MorphForge.Editor;
using Newtonsoft.Json.Linq;
public static class Mf5Finish
{
    public static string Run()
    {
        if(EditorApplication.isPlaying)throw new Exception("Edit Mode required.");
        var scene=SceneManager.GetSceneByPath(Mf5AvatarBuild.ScenePath);
        if(scene.IsValid()&&scene.isLoaded){if(scene.isDirty)EditorSceneManager.SaveScene(scene);EditorSceneManager.CloseScene(scene,true);}
        var original=SceneManager.GetSceneByPath("Assets/MorphForge/Diagnostics/Recovery20261020/UnsavedFullFT.unity");
        if(original.IsValid()&&original.isLoaded)SceneManager.SetActiveScene(original);
        return new JObject{["success"]=true,["mf5SceneSavedOnDisk"]=true,["playStopped"]=!EditorApplication.isPlaying,["originalScenes"]=new JArray(Enumerable.Range(0,SceneManager.sceneCount).Select(i=>{var s=SceneManager.GetSceneAt(i);return new JObject{["path"]=s.path,["dirty"]=s.isDirty};}))}.ToString();
    }
}
