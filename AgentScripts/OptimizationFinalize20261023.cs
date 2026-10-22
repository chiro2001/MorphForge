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
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase.Editor.BuildPipeline;
public static class OptimizationFinalize20261023 {
 public static string Audit(){
  var scene=SceneManager.GetActiveScene();if(EditorApplication.isPlaying||scene.path!=Mf6AvatarBuild.ScenePath||scene.isDirty)throw new Exception("Clean saved source required");
  var before=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString();
  var baseline=JObject.Parse(File.ReadAllText("Docs/Optimization20261023/baseline-source.json"));
  bool characters=baseline["characterPrefabs"].All(r=>AssetDatabase.GetAssetDependencyHash((string)r["path"]).ToString()==(string)r["hash"]);
  var dependencies=AssetDatabase.GetDependencies(Mf6AvatarBuild.AvatarPath,true).Concat(AssetDatabase.GetDependencies(AvatarSetupWindow.PrefabPath,true)).Distinct().ToArray();
  var controllers=dependencies.Where(p=>p.EndsWith(".controller")).Select(AssetDatabase.LoadAssetAtPath<AnimatorController>).ToArray();
  int routes=controllers.SelectMany(c=>c.layers).Where(l=>l.name.StartsWith("MF6 route to native pool ")).Sum(l=>l.stateMachine.states.Count(s=>s.state.name=="Native point or segment"));
  int idle=controllers.SelectMany(c=>c.layers).Where(l=>l.name=="MF6 immutable data and phase output").Count(l=>l.stateMachine.states.Any(s=>s.state.name=="Prepare payload"));
  var preview=EditorSceneManager.NewPreviewScene();GameObject clone=null;int synced=0,ints=0,parameters=0;
  try{
   clone=UnityEngine.Object.Instantiate(AssetDatabase.LoadAssetAtPath<GameObject>(Mf6AvatarBuild.AvatarPath));SceneManager.MoveGameObjectToScene(clone,preview);clone.name="MorphForge_MF6_Avatar";
   if(!VRCBuildPipelineCallbacks.OnPreprocessAvatar(clone))throw new Exception("Native SDK merge failed");
   var descriptor=clone.GetComponent<VRCAvatarDescriptor>();synced=descriptor.expressionParameters.CalcTotalCost();
   ints=descriptor.expressionParameters.parameters.Count(p=>p.networkSynced&&p.valueType.ToString()=="Int"&&(p.name=="MF6_Ctl"||p.name.StartsWith("MF6_D")));
   parameters=((AnimatorController)descriptor.baseAnimationLayers.Single(l=>l.type==VRCAvatarDescriptor.AnimLayerType.FX).animatorController).parameters.Length;
  }finally{if(clone)UnityEngine.Object.DestroyImmediate(clone);EditorSceneManager.ClosePreviewScene(preview);}
  var report=new JObject{["success"]=characters&&routes==16&&idle==16&&synced==254&&ints==30&&AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString()==before,["characterPrefabsUnchanged"]=characters,["characterPrefabs"]=baseline["characterPrefabs"].Count(),["drawingRoutes"]=routes,["optimizedIdlePools"]=idle,["totalSyncedBits"]=synced,["directGeometryInts"]=ints,["geometryBits"]=ints*8,["mergedFxParameters"]=parameters,["sourcePrefabUnchanged"]=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString()==before,["sceneDirtyAfter"]=scene.isDirty,["scope"]="Read-only original dependency verification and SDK preprocess of a transient preview-scene clone. No product assets regenerated, saved or uploaded."};
  File.WriteAllText("Docs/Optimization20261023/final-audit.json",report.ToString());return report.ToString();
 }
}
