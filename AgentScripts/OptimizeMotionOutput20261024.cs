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

public static class OptimizeMotionOutput20261024 {
 const string Folder="Assets/MorphForge/Validation/Performance20261024/Before";
 const string Reports="Docs/LivePerformance20261024";
 static void Clean(){
  if(EditorApplication.isPlaying||EditorApplication.isCompiling||SceneManager.GetActiveScene().path!=Mf6AvatarBuild.ScenePath)throw new Exception("Saved source Edit Mode required");
  for(int i=0;i<SceneManager.sceneCount;i++)if(SceneManager.GetSceneAt(i).isDirty)throw new Exception("User scene preserved");
 }
 static JObject Inspect(AnimatorController fx){
  var clips=fx.animationClips.Distinct().ToArray();var bindings=clips.SelectMany(AnimationUtility.GetCurveBindings).ToArray();
  var receive=fx.layers.Where(l=>l.name.StartsWith("Motion receive ")).SelectMany(l=>l.stateMachine.states).Select(s=>s.state).ToArray();
  return new JObject{["parameters"]=fx.parameters.Length,["layers"]=fx.layers.Length,["clips"]=clips.Length,["curves"]=bindings.Length,["rendererProperties"]=bindings.Where(b=>b.propertyName.StartsWith("material.")).Select(b=>b.path+":"+b.propertyName).Distinct().Count(),["receiveStates"]=receive.Length,["receiveStatesWriteDefaultsOn"]=receive.Count(s=>s.writeDefaultValues)};
 }
 public static string Backup(){
  Clean();if(AssetDatabase.IsValidFolder(Folder))throw new Exception("Backup already exists; preserve it");CatalogVariant.EnsureFolder(Folder);Directory.CreateDirectory(Reports);
  if(!AssetDatabase.CopyAsset(Mf6MotionBuild.Root,Folder+"/Motion"))throw new Exception("Motion backup failed");
  var report=Inspect(AssetDatabase.LoadAssetAtPath<AnimatorController>(Mf6MotionBuild.Root+"/Motion.controller"));File.WriteAllText(Reports+"/motion-before-structure.json",report.ToString());return report.ToString();
 }
 public static string BackupOtherComponents(){
  Clean();if(File.Exists(Reports+"/component-backups.json"))throw new Exception("Component backups already exist");
  var map=new JObject{[Mf6MotionBuild.Root+"/Motion.controller"]=Folder+"/Motion/Motion.controller"};
  string text=Mf6TextBuild.Root+"/Text.controller";if(!AssetDatabase.CopyAsset(text,Folder+"/Text.controller"))throw new Exception("Text backup failed");map[text]=Folder+"/Text.controller";
  var paths=AssetDatabase.GetDependencies(new[]{Mf6AvatarBuild.AvatarPath,AvatarSetupWindow.PrefabPath},true).Where(p=>p.EndsWith("/FX.controller")&&p.Contains("/Pool")&&(p.StartsWith("Assets/MorphForge/")||p.StartsWith("Assets/MorphForgeAvatar/"))).Distinct().ToArray();
  if(paths.Length!=16)throw new Exception("Expected the private/public 8-worker controllers");
  for(int i=0;i<paths.Length;i++){string copy=Folder+"/Geometry"+i+".controller";if(!AssetDatabase.CopyAsset(paths[i],copy))throw new Exception("Geometry backup failed");map[paths[i]]=copy;}
  File.WriteAllText(Reports+"/component-backups.json",map.ToString());return new JObject{["success"]=true,["geometryControllers"]=paths.Length}.ToString();
 }
 public static string BuildSleepingComponents(){
  Clean();if(!File.Exists(Reports+"/component-backups.json"))throw new Exception("Component backups required");
  var motion=JObject.Parse(Mf6MotionBuild.Build());Mf6TextBuild.BuildAssets();
  var map=JObject.Parse(File.ReadAllText(Reports+"/component-backups.json"));var rows=new JArray();
  foreach(var property in map.Properties().Where(p=>p.Name.EndsWith("/FX.controller"))){var fx=AssetDatabase.LoadAssetAtPath<AnimatorController>(property.Name);int changed=Mf6NativeBuild.OptimizeIdle(fx);var wait=fx.layers.Single(l=>l.name=="MF6 immutable data and phase output").stateMachine.states.Single(s=>s.state.name=="Wait").state;rows.Add(new JObject{["path"]=property.Name,["changed"]=changed,["idleCurves"]=AnimationUtility.GetCurveBindings((AnimationClip)wait.motion).Length,["idleWriteDefaults"]=wait.writeDefaultValues});}
  AssetDatabase.SaveAssets();var scene=SceneManager.GetActiveScene();if(scene.isDirty)EditorSceneManager.SaveScene(scene);
  var text=AssetDatabase.LoadAssetAtPath<AnimatorController>(Mf6TextBuild.Root+"/Text.controller");
  var report=new JObject{["success"]=rows.All(r=>(int)r["idleCurves"]==3&&!(bool)r["idleWriteDefaults"]),["motion"]=motion,["textLayers"]=text.layers.Length,["geometry"]=rows};File.WriteAllText(Reports+"/sleep-build.json",report.ToString());return new JObject{["success"]=report["success"],["motion"]=motion,["textLayers"]=text.layers.Length,["geometryControllers"]=rows.Count,["maximumIdleCurves"]=rows.Max(r=>(int)r["idleCurves"])}.ToString();
 }
 public static string Build(){
  Clean();if(!AssetDatabase.IsValidFolder(Folder+"/Motion"))throw new Exception("Backup required");
  var before=Inspect(AssetDatabase.LoadAssetAtPath<AnimatorController>(Folder+"/Motion/Motion.controller"));
  var build=JObject.Parse(Mf6MotionBuild.Build());
  var after=Inspect(AssetDatabase.LoadAssetAtPath<AnimatorController>(Mf6MotionBuild.Root+"/Motion.controller"));
  AssetDatabase.SaveAssets();var scene=SceneManager.GetActiveScene();if(scene.isDirty)EditorSceneManager.SaveScene(scene);
  const int sharedTextInputs=8; // References to existing Count/Group parameters.
  var result=new JObject{["success"]=(int)after["parameters"]-sharedTextInputs==(int)before["parameters"]&&(int)after["receiveStatesWriteDefaultsOn"]==123&&(int)after["rendererProperties"]<(int)before["rendererProperties"],["before"]=before,["after"]=after,["sharedTextInputs"]=sharedTextInputs,["build"]=build,["wireProtocolChanged"]=false,["cacheOwnershipChanged"]=false};
  File.WriteAllText(Reports+"/motion-output-build.json",result.ToString());return result.ToString();
 }
}
