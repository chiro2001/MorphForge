using System;
using System.Linq;
using System.IO;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.SDK3.Avatars.Components;
using MorphForge.Editor;
using Newtonsoft.Json.Linq;
public static class Mf5SingleAvatarScene
{
    static JObject Report(){var scenes=new JArray();int total=0;for(int i=0;i<SceneManager.sceneCount;i++){var s=SceneManager.GetSceneAt(i);var count=s.GetRootGameObjects().Sum(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true).Length);total+=count;scenes.Add(new JObject{["path"]=s.path,["avatars"]=count,["dirty"]=s.isDirty});}return new JObject{["success"]=total==1,["totalLoadedAvatars"]=total,["scenes"]=scenes};}
    public static string Audit()=>Report().ToString();
    public static string SelectOnly()
    {
        if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required.");
        for(int i=0;i<SceneManager.sceneCount;i++){var s=SceneManager.GetSceneAt(i);if(s.isDirty)throw new Exception("Unsaved scene preserved; cannot switch Single automatically: "+s.path);}
        var scene=EditorSceneManager.OpenScene(Mf5AvatarBuild.ScenePath,OpenSceneMode.Single);
        var count=scene.GetRootGameObjects().Sum(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true).Length);
        if(count!=1)throw new Exception("MF5 scene must contain exactly one Avatar, found "+count);
        var report=Report();Directory.CreateDirectory("Docs/MF5");File.WriteAllText("Docs/MF5/single-avatar-scene.json",report.ToString());return report.ToString();
    }
}
