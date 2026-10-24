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
public static class MotionCacheFix20261024 {
 const string Reports="Docs/MotionCacheFix20261024";
 const string BackupPath="Assets/MorphForge/Validation/MotionCacheFix20261024/Before";
 static string Save(string name,JObject r){Directory.CreateDirectory(Reports);File.WriteAllText(Reports+"/"+name+".json",r.ToString());return r.ToString();}
 static void Clean(){if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");var s=SceneManager.GetActiveScene();if(s.path!=Mf6AvatarBuild.ScenePath||s.isDirty)throw new Exception("Clean saved source required");for(int i=0;i<SceneManager.sceneCount;i++)if(SceneManager.GetSceneAt(i).isDirty)throw new Exception("User scene preserved");}
 public static string Backup(){
  Clean();if(AssetDatabase.IsValidFolder(BackupPath))throw new Exception("Backup already exists");CatalogVariant.EnsureFolder(BackupPath);
  var paths=new[]{Mf6MotionBuild.Root,Mf6NativeBuild.Root+"/Decoder.controller",Mf6AvatarBuild.AvatarPath,AvatarSetupWindow.PrefabPath};var rows=new JArray();
  for(int i=0;i<paths.Length;i++){string copy=BackupPath+"/Item"+i+(Path.HasExtension(paths[i])?Path.GetExtension(paths[i]):"");if(!AssetDatabase.CopyAsset(paths[i],copy))throw new Exception("Backup failed");rows.Add(new JObject{["source"]=paths[i],["copy"]=copy,["hash"]=AssetDatabase.GetAssetDependencyHash(paths[i]).ToString()});}
  var publicDecoder=AssetDatabase.GetDependencies(AvatarSetupWindow.PrefabPath,true).Where(p=>p.EndsWith("/Decoder.controller")).ToArray();for(int i=0;i<publicDecoder.Length;i++){string copy=BackupPath+"/PublicDecoder"+i+".controller";if(!AssetDatabase.CopyAsset(publicDecoder[i],copy))throw new Exception("Decoder backup failed");}
  var character=new JArray(AssetDatabase.GetDependencies(Mf6AvatarBuild.AvatarPath,true).Where(p=>p.StartsWith("Assets/Nemesis/")&&p.EndsWith(".prefab")).Select(p=>(JToken)new JObject{["path"]=p,["hash"]=AssetDatabase.GetAssetDependencyHash(p).ToString()}));
  if(!EditorSceneManager.SaveScene(SceneManager.GetActiveScene(),BackupPath+"/Source.unity",true))throw new Exception("Source snapshot failed");
  return Save("backup",new JObject{["success"]=true,["assets"]=rows,["characters"]=character});
 }
 public static string Build(){
  Clean();var original=JObject.Parse(File.ReadAllText(Reports+"/backup.json"));if(original["characters"].Any(r=>AssetDatabase.GetAssetDependencyHash((string)r["path"]).ToString()!=(string)r["hash"]))throw new Exception("Original character changed");
  var build=JObject.Parse(Mf6MotionBuild.Build());
  foreach(string path in new[]{Mf6AvatarBuild.AvatarPath,AvatarSetupWindow.PrefabPath}){
   var root=PrefabUtility.LoadPrefabContents(path);try{
    bool shared=path==AvatarSetupWindow.PrefabPath;var module=shared?root:root.transform.Find("MorphForgePlacement").gameObject;
    var catalog=JObject.Parse(File.ReadAllText(shared?AvatarSetupWindow.Root+"/CatalogSource/catalog.json":"Docs/MF6/avatar-catalog.json"));
    if(shared){var id=CatalogIdentity.Apply(module,catalog);var marker=module.GetComponent<MorphForgeAvatar.MorphForgeAvatarModule>();marker.version="1.3.1";marker.catalogIdentity=id;}else AvatarCapabilitiesBuild.Apply(root,catalog);
    PrefabUtility.SaveAsPrefabAsset(root,path);
   }finally{PrefabUtility.UnloadPrefabContents(root);}
  }
  AssetDatabase.SaveAssets();var scene=SceneManager.GetActiveScene();EditorSceneManager.MarkSceneDirty(scene);EditorSceneManager.SaveScene(scene);
  var fx=AssetDatabase.LoadAssetAtPath<AnimatorController>(Mf6MotionBuild.Root+"/Motion.controller");
  var receive=fx.layers.Where(l=>l.name.StartsWith("Motion receive ")).SelectMany(l=>l.stateMachine.states).Select(s=>s.state).ToArray();
  bool characters=original["characters"].All(r=>AssetDatabase.GetAssetDependencyHash((string)r["path"]).ToString()==(string)r["hash"]);
  var result=new JObject{["success"]=characters&&receive.Length==123&&receive.All(s=>s.writeDefaultValues),["receiveStates"]=receive.Length,["receiveStatesWriteDefaultsOn"]=receive.Count(s=>s.writeDefaultValues),["parameters"]=fx.parameters.Length,["cachedWordClips"]=AssetDatabase.LoadAllAssetsAtPath(AssetDatabase.GetAssetPath(fx)).OfType<AnimationClip>().Count(c=>c.name.StartsWith("Retain MF6_M")),["charactersUnchanged"]=characters,["motionBuild"]=build,["sourceHash"]=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString(),["receiverCapabilityVersion"]=9};
  return Save("build",result);
 }
 public static string Audit(){
  Clean();JObject Inspect(AnimatorController fx){
   var owners=new System.Collections.Generic.Dictionary<string,System.Collections.Generic.HashSet<string>>();int unsafeStates=0,receivers=0;
   foreach(var layer in fx.layers){var visited=new System.Collections.Generic.HashSet<Motion>();
    void Read(Motion motion){if(!motion||!visited.Add(motion))return;if(motion is BlendTree tree){foreach(var child in tree.children)Read(child.motion);}else if(motion is AnimationClip clip){foreach(var binding in AnimationUtility.GetCurveBindings(clip).Where(b=>b.type==typeof(Animator)&&b.propertyName.StartsWith("MF6_M")&&b.propertyName.Contains("_C"))){if(!owners.TryGetValue(binding.propertyName,out var set)){set=new System.Collections.Generic.HashSet<string>();owners[binding.propertyName]=set;}set.Add(layer.name);}}}
    foreach(var state in layer.stateMachine.states){if(layer.name.StartsWith("Motion receive ")){receivers++;if(state.state.motion is BlendTree&&!state.state.writeDefaultValues)unsafeStates++;}Read(state.state.motion);}
   }
   return new JObject{["parameters"]=fx.parameters.Length,["receiveStates"]=receivers,["directReceiveStatesWriteDefaultsOff"]=unsafeStates,["cachedWords"]=owners.Count,["wordsWrittenByMultipleLayers"]=owners.Count(p=>p.Value.Count>1)};
  }
  var before=Inspect(AssetDatabase.LoadAssetAtPath<AnimatorController>(BackupPath+"/Item0/Motion.controller"));var after=Inspect(AssetDatabase.LoadAssetAtPath<AnimatorController>(Mf6MotionBuild.Root+"/Motion.controller"));
  return Save("audit",new JObject{["success"]=(int)after["directReceiveStatesWriteDefaultsOff"]==0&&(int)after["wordsWrittenByMultipleLayers"]==0&&(int)before["parameters"]==(int)after["parameters"],["before"]=before,["after"]=after});
 }
}
