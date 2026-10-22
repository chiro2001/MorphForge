using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using UnityEngine;
using UnityEditor;
using UnityEditor.SceneManagement;
using MorphForge.Editor;
using Newtonsoft.Json.Linq;
public static class MotionStageEntry {
 public static string AdvertiseValidatedSource(){
  var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();if(EditorApplication.isPlaying||scene.isDirty||scene.path!=Mf6AvatarBuild.ScenePath)throw new Exception("Clean saved source MF6 scene required");
  foreach(var file in new[]{"native-motion-play.json","native-motion-dynamics.json","native-motion-visual.json","native-motion-once.json","native-motion-surfaces.json","local-preview-owner-play.json"})if((bool?)JObject.Parse(File.ReadAllText("Docs/TextAnimation/"+file))["success"]!=true)throw new Exception("Receiver verification missing: "+file);
  var path="Docs/MF6/avatar-catalog.json";var catalog=JObject.Parse(File.ReadAllText(path));var caps=(JArray)catalog["capabilities"];foreach(var capability in new[]{"mf6-motion4/1","mf6-text-panels/1","mf6-text-latest14/1","local-placement-controls/1","local-avatar-preview-prototype/1"})if(!caps.Any(c=>(string)c==capability))caps.Add(capability);
  var root=PrefabUtility.LoadPrefabContents(Mf6AvatarBuild.AvatarPath);try{AvatarCapabilitiesBuild.Apply(root,catalog);PrefabUtility.SaveAsPrefabAsset(root,Mf6AvatarBuild.AvatarPath);}finally{PrefabUtility.UnloadPrefabContents(root);}File.WriteAllText(path,catalog.ToString());AssetDatabase.SaveAssets();
  var report=new JObject{["success"]=true,["nativeMotionAdvertised"]=true,["additionalSyncBits"]=0,["bodyParametersChanged"]=false,["note"]="Source development Avatar receiver now advertises validated features; other body parameters and upload are unchanged."};File.WriteAllText("Docs/TextAnimation/source-capabilities.json",report.ToString());return report.ToString();
 }
 public static string BuildPlayer(){
  var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();if(EditorApplication.isPlaying||scene.isDirty)throw new Exception("Clean Edit Mode required");
  var result=Mf6MotionBuild.Build();foreach(var path in new[]{Mf6AvatarBuild.AvatarPath,MorphForgeAvatar.Editor.AvatarSetupWindow.PrefabPath}){var root=PrefabUtility.LoadPrefabContents(path);try{var module=path==Mf6AvatarBuild.AvatarPath?root.transform.Find(PlacementStage4Build.ModuleName).gameObject:root;Mf6MotionBuild.Install(module);PrefabUtility.SaveAsPrefabAsset(root,path);}finally{PrefabUtility.UnloadPrefabContents(root);}}
  AssetDatabase.SaveAssets();return result;
 }
 public static async Task<string> Prepare(){
  var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();if(EditorApplication.isPlaying||scene.isDirty||scene.path!=Mf6AvatarBuild.ScenePath)throw new Exception("Clean MF6 scene in Edit Mode required");
  Directory.CreateDirectory("Docs/TextAnimation");var rows=new JArray();
  for(int pool=0;pool<8;pool++){rows.Add(JObject.Parse(Mf6NativeBuild.BuildWorker(pool)));File.WriteAllText("Docs/TextAnimation/motion-prepare.json",new JObject{["complete"]=false,["workers"]=rows}.ToString());await Task.Delay(50);}
  var decoder=JObject.Parse(Mf6DecoderBuild.Build());
  var contents=PrefabUtility.LoadPrefabContents(Mf6AvatarBuild.AvatarPath);
  try{var catalog=JObject.Parse(File.ReadAllText("Docs/MF6/avatar-catalog.json"));AvatarCapabilitiesBuild.Apply(contents,catalog);PrefabUtility.SaveAsPrefabAsset(contents,Mf6AvatarBuild.AvatarPath);}finally{PrefabUtility.UnloadPrefabContents(contents);}
  AssetDatabase.SaveAssets();EditorSceneManager.MarkSceneDirty(scene);EditorSceneManager.SaveScene(scene);
  var result=new JObject{["complete"]=true,["workers"]=rows,["decoder"]=decoder,["motionPlaybackAdvertised"]=false,["scope"]="Experimental group-tag geometry receiver only. Clock/definition player not yet installed; no Avatar upload."};File.WriteAllText("Docs/TextAnimation/motion-prepare.json",result.ToString());return result.ToString();
 }
}
