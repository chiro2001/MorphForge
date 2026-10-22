using System;
using System.Linq;
using UnityEngine;
using UnityEditor;
using MorphForge.Editor;
using MorphForgeAvatar.Editor;
public static class LocalPreviewEntry {
 public static string Build(){
  var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();if(EditorApplication.isPlaying||scene.isDirty)throw new Exception("Clean Edit Mode required");
  var report=LocalPreviewBuild.Build();foreach(var path in new[]{Mf6AvatarBuild.AvatarPath,AvatarSetupWindow.PrefabPath}){var root=PrefabUtility.LoadPrefabContents(path);try{var module=path==Mf6AvatarBuild.AvatarPath?root.transform.Find(PlacementStage4Build.ModuleName).gameObject:root;LocalPreviewBuild.Install(module);PrefabUtility.SaveAsPrefabAsset(root,path);}finally{PrefabUtility.UnloadPrefabContents(root);}}
  AssetDatabase.SaveAssets();return report;
 }
}
