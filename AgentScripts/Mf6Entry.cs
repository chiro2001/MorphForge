using System;
using System.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine.SceneManagement;
using VRC.SDK3.Avatars.Components;
public static class Mf6Entry
{
    public static string FixBoundsScale()
    {
        if(EditorApplication.isPlaying)throw new Exception("Edit Mode required");
        for(int pool=0;pool<8;pool++){
            var path=MorphForge.Editor.Mf6NativeBuild.Root+"/Pool"+pool+"/Display.prefab";var root=PrefabUtility.LoadPrefabContents(path);
            try{var system=root.transform.Find("BoundsPump").GetComponent<UnityEngine.ParticleSystem>();var main=system.main;main.scalingMode=UnityEngine.ParticleSystemScalingMode.Hierarchy;PrefabUtility.SaveAsPrefabAsset(root,path);}finally{PrefabUtility.UnloadPrefabContents(root);}
        }
        AssetDatabase.SaveAssets();return "Eight bounds emitters now use hierarchy scaling; controllers unchanged";
    }
    public static string Prepare()=>MorphForge.Editor.Mf6AvatarBuild.Prepare();
    public static string PrepareOwned()=>PrepareOwnedAt(null);
    public static string PrepareBuiltinOwned()=>PrepareOwnedAt("Docs/MF6/BuiltinSource");
    static string PrepareOwnedAt(string sourceRoot)
    {
        if(EditorApplication.isPlaying)throw new Exception("Edit Mode required");
        var scene=SceneManager.GetActiveScene();
        if(scene.path==MorphForge.Editor.Mf6AvatarBuild.ScenePath&&scene.isDirty){
            if(scene.GetRootGameObjects().Sum(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true).Length)!=1)throw new Exception("Generated MF6 scene must contain exactly one Avatar");
            // This scene was generated in this task. Preserve its current state
            // before rebuilding; legacy/user scenes are never auto-saved here.
            EditorSceneManager.SaveScene(scene,"Assets/MorphForge/MF6/BeforeRebuild.unity",true);EditorSceneManager.SaveScene(scene);
        }
        return MorphForge.Editor.Mf6AvatarBuild.Prepare(sourceRoot);
    }
}
