using System;
using System.Linq;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.SDK3.Avatars.Components;
using MorphForge.Editor;
public static class Mf4SceneSync
{
    public static string Run()
    {
        if(Application.isPlaying)throw new Exception("Edit Mode required.");
        var scene=SceneManager.GetSceneByPath(Mf4AvatarBuild.ScenePath);
        var avatar=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single().gameObject;avatar.name="MorphForge_MF4_Avatar";
        EditorSceneManager.SaveScene(scene);
        return avatar.name;
    }
}
