using System;
using System.IO;
using System.Linq;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using MorphForge.Editor;
using MorphForgeAvatar.Editor;
using nadena.dev.modular_avatar.core;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase.Editor.BuildPipeline;
using BlackStartX.GestureManager;

// Offline ablation: remove components before SDK processing, never user assets.
public static class StaticAblation20261024 {
 const string Folder="Assets/MorphForge/Validation/Performance20261024";
 const string Reports="Docs/LivePerformance20261024";
 public static string RecoverSource(){
  if(!Application.isBatchMode||Application.dataPath.Replace('\\','/')!="D:/Projects/VRChatProjects2/MorphForge/Assets"||EditorApplication.isPlaying)throw new Exception("Owned recovery Editor required");
  var scene=SceneManager.GetActiveScene();
  if(scene.path==Mf6AvatarBuild.ScenePath&&!scene.isDirty)return "Source already clean";
  if(scene.path!=""&&!scene.path.StartsWith(Folder+"/"))throw new Exception("Unexpected scene preserved: "+scene.path);
  if(scene.isDirty){CatalogVariant.EnsureFolder(Folder);if(!EditorSceneManager.SaveScene(scene,Folder+"/RecoveryBootstrap.unity",true))throw new Exception("Recovery snapshot failed");}
  EditorSceneManager.OpenScene(Mf6AvatarBuild.ScenePath,OpenSceneMode.Single);return "Restored saved source in owned batch recovery Editor";
 }
 public static string FinishRecovery(){
  if(!Application.isBatchMode||SceneManager.GetActiveScene().path!=Mf6AvatarBuild.ScenePath||SceneManager.GetActiveScene().isDirty||EditorApplication.isPlaying)throw new Exception("Clean owned recovery Editor required");
  var report=new JObject{["pid"]=System.Diagnostics.Process.GetCurrentProcess().Id,["cleanSource"]=true,["utc"]=DateTime.UtcNow.ToString("O")};
  File.WriteAllText(Reports+"/recovery-exit.json",report.ToString());
  EditorApplication.delayCall+=()=>EditorApplication.Exit(0);return report.ToString();
 }
 public static string Prepare(string variant){
  if(!new[]{"full","previous-motion","previous-full","no-module","no-motion","no-text","no-geometry","no-preview"}.Contains(variant))throw new Exception("Unknown variant");
  if(variant=="no-geometry")throw new Exception("This offline variant triggers the existing VRCFury parameter-budget dialog; use renderer/simulation controls or a separately budgeted fixture instead");
  if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");
  var source=SceneManager.GetActiveScene();if(source.path!=Mf6AvatarBuild.ScenePath||source.isDirty)throw new Exception("Clean source required");
  for(int i=0;i<SceneManager.sceneCount;i++)if(SceneManager.GetSceneAt(i).isDirty)throw new Exception("User scene preserved");
  var hash=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString();
  CatalogVariant.EnsureFolder(Folder);Directory.CreateDirectory(Reports);
  string path=Folder+"/"+variant+".unity";
  if(!EditorSceneManager.SaveScene(source,path,true))throw new Exception("Scene copy failed");
  var scene=EditorSceneManager.OpenScene(path,OpenSceneMode.Single);
  var avatar=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single();
  PrefabUtility.UnpackPrefabInstance(avatar.gameObject,PrefabUnpackMode.Completely,InteractionMode.AutomatedAction);
  var module=avatar.transform.Find("MorphForgePlacement");if(!module)throw new Exception("Module missing");
  if(variant=="previous-full"){
   var map=JObject.Parse(File.ReadAllText(Reports+"/component-backups.json"));
   foreach(var merge in module.GetComponentsInChildren<ModularAvatarMergeAnimator>(true)){string original=AssetDatabase.GetAssetPath(merge.animator);if(map.TryGetValue(original,out var backup)){merge.animator=AssetDatabase.LoadAssetAtPath<AnimatorController>((string)backup);if(!merge.animator)throw new Exception("Previous component backup missing");}}
  }
  else if(variant=="previous-motion"){
   var motion=module.GetComponentsInChildren<ModularAvatarMergeAnimator>(true).Single(m=>AssetDatabase.GetAssetPath(m.animator).EndsWith("/Motion.controller"));
   motion.animator=AssetDatabase.LoadAssetAtPath<AnimatorController>(Folder+"/Before/Motion/Motion.controller");if(!motion.animator)throw new Exception("Previous motion backup missing");
  }
  else if(variant=="no-module")UnityEngine.Object.DestroyImmediate(module.gameObject);
  else if(variant=="no-preview")UnityEngine.Object.DestroyImmediate(module.Find("LocalPreview").gameObject);
  else if(variant=="no-text"){
   var display=module.GetComponentsInChildren<Transform>(true).Single(t=>t.name=="TextDisplay");UnityEngine.Object.DestroyImmediate(display.gameObject);
  }else if(variant=="no-motion"){
   var motion=module.GetComponentsInChildren<ModularAvatarMergeAnimator>(true).Single(m=>AssetDatabase.GetAssetPath(m.animator).EndsWith("/Motion.controller"));UnityEngine.Object.DestroyImmediate(motion);
  }else if(variant=="no-geometry"){
   var pools=module.GetComponentsInChildren<Transform>(true).Where(t=>t.name.Length==5&&t.name.StartsWith("Pool")&&char.IsDigit(t.name[4])).ToArray();foreach(var pool in pools)UnityEngine.Object.DestroyImmediate(pool.gameObject);
  }
  if(!VRCBuildPipelineCallbacks.OnPreprocessAvatar(avatar.gameObject))throw new Exception("SDK preprocessing failed");
  var gm=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<GestureManager>(true)).Single();gm.settings.favourite=avatar;gm.settings.loadStored=false;gm.settings.isRemote=false;
  var receiver=gm.GetComponent<MorphForgePlayReceiver>();receiver.Avatar=avatar;
  var fx=avatar.baseAnimationLayers.Single(l=>l.type==VRCAvatarDescriptor.AnimLayerType.FX).animatorController as AnimatorController;
  EditorSceneManager.MarkSceneDirty(scene);EditorSceneManager.SaveScene(scene);
  var result=new JObject{["success"]=hash==AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString(),["variant"]=variant,["scene"]=path,["layers"]=fx.layers.Length,["parameters"]=fx.parameters.Length,["curveBindings"]=fx.animationClips.Distinct().Sum(c=>AnimationUtility.GetCurveBindings(c).Length),["renderers"]=avatar.GetComponentsInChildren<Renderer>(true).Length,["particles"]=avatar.GetComponentsInChildren<ParticleSystem>(true).Length,["syncedBits"]=avatar.expressionParameters.CalcTotalCost(),["sourceHash"]=hash,["scope"]="Owned offline scene; component removed before actual SDK preprocessing. No online avatar changed."};
  File.WriteAllText(Reports+"/static-"+variant+"-structure.json",result.ToString());return result.ToString();
 }
 public static string Restore(){
  if(EditorApplication.isPlaying)throw new Exception("Stop owned Play Mode first");
  var scene=SceneManager.GetActiveScene();if(!scene.path.StartsWith(Folder+"/"))throw new Exception("Not an owned ablation scene");
  if(scene.isDirty&&!EditorSceneManager.SaveScene(scene,Folder+"/AfterPlay-"+Path.GetFileName(scene.path),true))throw new Exception("Snapshot failed");
  EditorSceneManager.OpenScene(Mf6AvatarBuild.ScenePath,OpenSceneMode.Single);return "Restored clean MF6 source";
 }
}
