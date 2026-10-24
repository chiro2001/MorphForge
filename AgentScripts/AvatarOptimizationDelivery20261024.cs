using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using MorphForge.Editor;
using MorphForgeAvatar.Editor;
using VRC.SDK3.Avatars.Components;
using VRC.SDK3.Avatars.ScriptableObjects;

public static class AvatarOptimizationDelivery20261024 {
 const string Reports="Docs/LivePerformance20261024";
 public static string RestoreAfterSynthetic(){
  var scene=SceneManager.GetActiveScene();if(EditorApplication.isPlaying||scene.path!=Mf6AvatarBuild.ScenePath)throw new Exception("Unexpected scene preserved: "+scene.path);
  var allowed=new[]{"MF5 Gesture Manager","MF5 Camera","MorphForge_MF6_Avatar","nadena.dev.ndmf__Activator"};
  if(scene.GetRootGameObjects().Any(g=>!allowed.Contains(g.name)))throw new Exception("Unexpected source roots preserved");
  if(scene.isDirty){const string copy="Assets/MorphForge/Validation/Performance20261024/AfterSynthetic.unity";if(!EditorSceneManager.SaveScene(scene,copy,true))throw new Exception("Synthetic snapshot failed");}
  EditorSceneManager.OpenScene(Mf6AvatarBuild.ScenePath,OpenSceneMode.Single);return "Restored source after synthetic fixture cleanup; scene snapshot preserved";
 }
 public static string FinalizeSource(){
  var scene=SceneManager.GetActiveScene();if(EditorApplication.isPlaying||scene.path!=Mf6AvatarBuild.ScenePath||scene.isDirty)throw new Exception("Clean source required");
  var before=JObject.Parse(File.ReadAllText(Reports+"/source-inspection.json"));
  var root=PrefabUtility.LoadPrefabContents(AvatarSetupWindow.PrefabPath);
  try{root.GetComponent<MorphForgeAvatar.MorphForgeAvatarModule>().version="1.3.2";PrefabUtility.SaveAsPrefabAsset(root,AvatarSetupWindow.PrefabPath);}finally{PrefabUtility.UnloadPrefabContents(root);}
  AssetDatabase.SaveAssets();AssetDatabase.Refresh(ImportAssetOptions.ForceSynchronousImport);
  bool characters=before["characterPrefabs"].All(p=>AssetDatabase.GetAssetDependencyHash((string)p["path"]).ToString()==(string)p["hash"]);
  if(scene.isDirty)EditorSceneManager.SaveScene(scene);
  var result=new JObject{["success"]=characters,["charactersUnchanged"]=characters,["privatePrefabHash"]=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString(),["publicPrefabHash"]=AssetDatabase.GetAssetDependencyHash(AvatarSetupWindow.PrefabPath).ToString(),["version"]="1.3.2",["sourceClean"]=!scene.isDirty};
  Directory.CreateDirectory(Reports);File.WriteAllText(Reports+"/source-final.json",result.ToString());return result.ToString();
 }
 public static string AuditBuilt(){
  var scene=SceneManager.GetActiveScene();if(scene.path!="Assets/MorphForge/Validation/WaterfallSdk20261024/Baseline.unity")throw new Exception("Owned SDK scene required");
  var avatar=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single();
  var fx=(AnimatorController)avatar.baseAnimationLayers.Single(l=>l.type==VRCAvatarDescriptor.AnimLayerType.FX).animatorController;
  var paths=new JArray();var menus=new HashSet<VRCExpressionsMenu>();var oversized=new JArray();
  void Menu(VRCExpressionsMenu menu,string path,HashSet<VRCExpressionsMenu> ancestors){
   if(!menu||ancestors.Contains(menu))return;var next=new HashSet<VRCExpressionsMenu>(ancestors){menu};menus.Add(menu);
   if(menu.controls.Count>8)oversized.Add(path);
   foreach(var control in menu.controls){string p=path+"/"+control.name;if(control.name=="本人预览")paths.Add(p);if(control.subMenu)Menu(control.subMenu,p,next);}
  }
  Menu(avatar.expressionsMenu,"root",new HashSet<VRCExpressionsMenu>());
  bool previewNested=paths.Count==1&&((string)paths[0]).Contains("/MorphForge/本人预览");
  var output=fx.layers.Where(l=>l.name.StartsWith("Motion output ")||l.name.StartsWith("Motion text output ")||l.name.StartsWith("Text output ")).ToArray();
  var wrong=new JArray(output.SelectMany(l=>l.stateMachine.states.Select(s=>new{layer=l.name,state=s.state})).Where(x=>(x.state.motion is BlendTree)!=x.state.writeDefaultValues).Select(x=>x.layer+":"+x.state.name));
  var receives=fx.layers.Where(l=>l.name.StartsWith("Motion receive ")).SelectMany(l=>l.stateMachine.states).Select(s=>s.state).ToArray();
  var camera=avatar.GetComponentsInChildren<Camera>(true).Single(c=>c.targetTexture);
  bool front=camera.transform.localPosition.z>0&&Mathf.Abs(camera.transform.localPosition.y)<.001f&&Vector3.Dot(camera.transform.localRotation*Vector3.forward,Vector3.back)>.999f;
  int bits=avatar.expressionParameters.CalcTotalCost();
  var result=new JObject{["success"]=previewNested&&oversized.Count==0&&wrong.Count==0&&receives.Length==123&&receives.All(s=>s.writeDefaultValues)&&front&&bits==254&&fx.parameters.Length<=2048,["previewPaths"]=paths,["oversizedMenus"]=oversized,["outputLayers"]=output.Length,["incorrectOutputStates"]=wrong,["completeCacheStates"]=receives.Length,["frontCamera"]=front,["cameraDefaultOff"]=!camera.gameObject.activeSelf,["fxLayers"]=fx.layers.Length,["fxParameters"]=fx.parameters.Length,["syncedBits"]=bits};
  File.WriteAllText(Reports+"/built-audit.json",result.ToString());return result.ToString();
 }
}
