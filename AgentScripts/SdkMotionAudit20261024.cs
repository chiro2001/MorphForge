using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Threading.Tasks;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using MorphForge.Editor;
using MorphForgeAvatar.Editor;
using VRC.Core;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase.Editor.BuildPipeline;
using VRC.SDKBase.Editor.Api;
using BlackStartX.GestureManager;

public static class SdkMotionAudit20261024 {
 const string Folder="Assets/MorphForge/Validation/WaterfallSdk20261024";
 const string ScenePath=Folder+"/Baseline.unity";
 const string Reports="Docs/WaterfallSdk20261024";
 static string Save(string name,JObject report){Directory.CreateDirectory(Reports);File.WriteAllText(Reports+"/"+name+".json",report.ToString());return report.ToString();}
 public static async Task<string> Remote(){
  if(APIUser.CurrentUser==null&&ApiCredentials.Load())APIUser.InitialFetchCurrentUser(_=>{},null);
  for(int i=0;i<60&&APIUser.CurrentUser==null;i++)await Task.Delay(250);
  if(APIUser.CurrentUser==null)throw new Exception("Existing SDK login unavailable");
  var avatar=await VRCApi.GetAvatar(Mf6AvatarBuild.AvatarId,forceRefresh:true);
  if(avatar.AuthorId!=APIUser.CurrentUser.id)throw new Exception("Expected owned Avatar");
  var url=avatar.GetLatestAssetUrlForCurrentPlatform();string file=null,version=null;
  if(!string.IsNullOrEmpty(url)){int start=url.IndexOf("/file_",StringComparison.Ordinal);if(start>=0){var parts=url.Substring(start+1).Split('/');file=parts[0];version=parts.Length>1?parts[1]:null;}}
  return Save("online",new JObject{["avatarId"]=avatar.ID,["avatarVersion"]=avatar.Version,["fileId"]=file,["fileVersion"]=version,["pendingUpload"]=avatar.PendingUpload,["releaseStatus"]=avatar.ReleaseStatus,["timeUtc"]=DateTime.UtcNow.ToString("O")});
 }
 static JObject Controller(AnimatorController fx){
  var motions=new HashSet<Motion>();var states=new List<AnimatorState>();
  void Motion(Motion m){if(!m||!motions.Add(m))return;if(m is BlendTree t)foreach(var c in t.children)Motion(c.motion);}
  void Machine(AnimatorStateMachine m){foreach(var s in m.states){states.Add(s.state);Motion(s.state.motion);}foreach(var sub in m.stateMachines)Machine(sub.stateMachine);}
  foreach(var layer in fx.layers)Machine(layer.stateMachine);
  var clocks=new JArray();foreach(var s in states.Where(s=>s.name.StartsWith("Clock "))){if(!(s.motion is AnimationClip clip))continue;var binding=AnimationUtility.GetCurveBindings(clip).FirstOrDefault(b=>b.propertyName.EndsWith("_Elapsed"));if(string.IsNullOrEmpty(binding.propertyName))continue;var curve=AnimationUtility.GetEditorCurve(clip,binding);var settings=AnimationUtility.GetAnimationClipSettings(clip);clocks.Add(new JObject{["state"]=s.name,["clip"]=clip.name,["loop"]=settings.loopTime,["loopBlend"]=settings.loopBlend,["wrapMode"]=clip.wrapMode.ToString(),["speedParameter"]=s.speedParameter,["speedParameterActive"]=s.speedParameterActive,["path"]=binding.path,["property"]=binding.propertyName,["keys"]=new JArray(curve.keys.Select(k=>(JToken)new JObject{["time"]=k.time,["value"]=k.value,["inTangent"]=k.inTangent,["outTangent"]=k.outTangent}))});}
  var trees=new JArray(motions.OfType<BlendTree>().Where(t=>t.name.Contains("Latch")||t.name.Contains("Retain")||t.name.Contains("Motion bank")).Take(30).Select(t=>{var so=new SerializedObject(t);return (JToken)new JObject{["name"]=t.name,["normalized"]=so.FindProperty("m_NormalizedBlendValues").boolValue,["children"]=t.children.Length};}));
  var receive=fx.layers.Where(l=>l.name.Contains("Motion receive ")).SelectMany(l=>l.stateMachine.states).Select(s=>s.state).ToArray();
  return new JObject{["path"]=AssetDatabase.GetAssetPath(fx),["parameters"]=fx.parameters.Length,["cacheParameters"]=fx.parameters.Count(p=>p.name.StartsWith("MF6_M")&&p.name.Contains("_C")),["receiveStates"]=receive.Length,["receiveStatesWriteDefaultsOn"]=receive.Count(s=>s.writeDefaultValues),["layers"]=fx.layers.Length,["clocks"]=clocks,["trees"]=trees};
 }
 public static string Prepare(){
  if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");
  var source=SceneManager.GetActiveScene();if(source.path!=Mf6AvatarBuild.ScenePath||source.isDirty)throw new Exception("Clean source required");
  for(int i=0;i<SceneManager.sceneCount;i++)if(SceneManager.GetSceneAt(i).isDirty)throw new Exception("User scene preserved");
  var sourceHash=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString();
  var before=Controller(AssetDatabase.LoadAssetAtPath<AnimatorController>(Mf6MotionBuild.Root+"/Motion.controller"));
  CatalogVariant.EnsureFolder(Folder);
  if(!EditorSceneManager.SaveScene(source,ScenePath,true))throw new Exception("Scene copy failed");
  var scene=EditorSceneManager.OpenScene(ScenePath,OpenSceneMode.Single);
  var avatar=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single().gameObject;
  if(PrefabUtility.IsPartOfPrefabInstance(avatar))PrefabUtility.UnpackPrefabInstance(avatar,PrefabUnpackMode.Completely,InteractionMode.AutomatedAction);
  if(!VRCBuildPipelineCallbacks.OnPreprocessAvatar(avatar))throw new Exception("SDK preprocessing failed");
  var descriptor=avatar.GetComponent<VRCAvatarDescriptor>();
  var fx=(AnimatorController)descriptor.baseAnimationLayers.Single(l=>l.type==VRCAvatarDescriptor.AnimLayerType.FX).animatorController;
  if(!EditorUtility.IsPersistent(fx))throw new Exception("SDK generated FX is transient; preserve the owned scene for inspection");
  var gm=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<GestureManager>(true)).Single();gm.settings.favourite=descriptor;gm.settings.loadStored=false;gm.settings.isRemote=false;gm.GetComponent<MorphForgePlayReceiver>().Avatar=descriptor;
  EditorSceneManager.MarkSceneDirty(scene);EditorSceneManager.SaveScene(scene);
  var report=new JObject{["success"]=sourceHash==AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString(),["sourceHash"]=sourceHash,["source"]=before,["built"]=Controller(fx),["scene"]=scene.path,["syncedBits"]=descriptor.expressionParameters.CalcTotalCost(),["scope"]="Actual SDK preprocessing of an owned scene copy; no source assets or online Avatar modified."};
  return Save("prepare",report);
 }
 public static string Restore(){
  if(EditorApplication.isPlaying||SceneManager.GetActiveScene().path!=ScenePath)throw new Exception("Stop owned SDK diagnostic first");
  var scene=SceneManager.GetActiveScene();if(scene.isDirty&&!EditorSceneManager.SaveScene(scene,Folder+"/AfterPlay.unity",true))throw new Exception("Snapshot failed");
  EditorSceneManager.OpenScene(Mf6AvatarBuild.ScenePath,OpenSceneMode.Single);var report=JObject.Parse(File.ReadAllText(Reports+"/prepare.json"));bool same=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString()==(string)report["sourceHash"];
  return Save("restore",new JObject{["success"]=same&&!SceneManager.GetActiveScene().isDirty,["sourceAssetsUnchanged"]=same,["sourceSceneClean"]=!SceneManager.GetActiveScene().isDirty});
 }
}
