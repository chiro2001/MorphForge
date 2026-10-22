using System;
using System.Linq;
using System.IO;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
using MorphForge.Editor;
using MorphForgeAvatar.Editor;
using Newtonsoft.Json.Linq;
public static class PublicModuleValidation {
 public const string ScenePath="Assets/MorphForge/Validation/PublicModule12.unity";
 public static string Prepare(){
  var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();if(EditorApplication.isPlaying||scene.isDirty||scene.path!=Mf6AvatarBuild.ScenePath)throw new Exception("Clean source MF6 scene required");
  CatalogVariant.EnsureFolder("Assets/MorphForge/Validation");if(!EditorSceneManager.SaveScene(scene,ScenePath,true))throw new Exception("Could not copy source validation scene");
  scene=EditorSceneManager.OpenScene(ScenePath);var avatar=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single();
  if(PrefabUtility.IsPartOfPrefabInstance(avatar.gameObject))PrefabUtility.UnpackPrefabInstance(avatar.gameObject,PrefabUnpackMode.Completely,InteractionMode.AutomatedAction);
  var old=avatar.transform.Find(PlacementStage4Build.ModuleName);if(!old)throw new Exception("Source module missing");UnityEngine.Object.DestroyImmediate(old.gameObject);
  var module=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(AvatarSetupWindow.PrefabPath),avatar.transform);module.transform.localPosition=Vector3.zero;module.transform.localRotation=Quaternion.identity;module.transform.localScale=Vector3.one;
  if(avatar.GetComponentsInChildren<VRCAvatarDescriptor>(true).Length!=1)throw new Exception("Single Avatar required");
  EditorSceneManager.MarkSceneDirty(scene);EditorSceneManager.SaveScene(scene);
  return new JObject{["success"]=true,["scene"]=ScenePath,["module"]=PrefabUtility.GetCorrespondingObjectFromSource(module).name,["avatarCount"]=1,["publicMarkerVersion"]=module.GetComponent<MorphForgeAvatar.MorphForgeAvatarModule>().version}.ToString();
 }
 public static string VariantCapabilities(){
  if(EditorApplication.isPlaying)throw new Exception("Edit Mode required");const string input="E:/tmp/morphforge/avatar-plugin/legacy-variant-fixture";Directory.CreateDirectory(input+"/artifacts");var source=AvatarSetupWindow.Root+"/CatalogSource";var catalog=JObject.Parse(File.ReadAllText(source+"/catalog.json"));catalog["capabilities"]=new JArray("mf6-unified/1","mf5-runtime-materials/1");File.WriteAllText(input+"/catalog.json",catalog.ToString());foreach(var file in Directory.GetFiles(source+"/artifacts","*.png"))File.Copy(file,input+"/artifacts/"+Path.GetFileName(file),true);
  var path=CatalogVariant.Build(input);var prefab=AssetDatabase.LoadAssetAtPath<GameObject>(path);var root=Path.GetDirectoryName(path).Replace('\\','/');var actual=JObject.Parse(File.ReadAllText(root+"/Source/catalog.json"));var controller=prefab.GetComponentsInChildren<nadena.dev.modular_avatar.core.ModularAvatarMergeAnimator>(true).Single(c=>c.name=="MF6Decoder").animator as UnityEditor.Animations.AnimatorController;int flags=controller.parameters.Single(p=>p.name=="MF_InfoFeatures").defaultInt;bool success=flags==119&&((JArray)actual["capabilities"]).Any(v=>(string)v=="mf6-motion4/1")&&prefab.transform.Find("LocalPreview");var result=new JObject{["success"]=success,["receiverFlags"]=flags,["legacyMaterialExportGainsTemplateAbilities"]=success,["scope"]="Private test variant built from original public materials with old two-capability input; template native text/motion capabilities restored. User variants/source assets not changed; variant is excluded from public package."};File.WriteAllText("Docs/TextAnimation/variant-capabilities.json",result.ToString());return result.ToString();
 }
 public static string Isolate(){
  var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();if(EditorApplication.isPlaying||scene.path!=ScenePath)throw new Exception("Owned validation scene in Edit Mode required");
  // Reopen this task's saved disposable scene, never the source scene.
  scene=EditorSceneManager.OpenScene(ScenePath);var avatar=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single();var module=avatar.GetComponentInChildren<MorphForgeAvatar.MorphForgeAvatarModule>(true);
  var parameters=ScriptableObject.CreateInstance<VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionParameters>();parameters.parameters=new VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionParameters.Parameter[0];var parameterPath="Assets/MorphForge/Validation/TestBaseParameters.asset";if(AssetDatabase.LoadMainAssetAtPath(parameterPath))AssetDatabase.DeleteAsset(parameterPath);AssetDatabase.CreateAsset(parameters,parameterPath);avatar.expressionParameters=parameters;
  var menu=ScriptableObject.CreateInstance<VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu>();var menuPath="Assets/MorphForge/Validation/TestBaseMenu.asset";if(AssetDatabase.LoadMainAssetAtPath(menuPath))AssetDatabase.DeleteAsset(menuPath);AssetDatabase.CreateAsset(menu,menuPath);avatar.expressionsMenu=menu;
  int local=0,removed=0;foreach(var config in avatar.GetComponentsInChildren<nadena.dev.modular_avatar.core.ModularAvatarParameters>(true).Where(c=>!c.transform.IsChildOf(module.transform))){for(int i=0;i<config.parameters.Count;i++){var p=config.parameters[i];p.localOnly=true;config.parameters[i]=p;local++;}EditorUtility.SetDirty(config);}
  foreach(var c in avatar.GetComponentsInChildren<MonoBehaviour>(true).Where(c=>c&&!c.transform.IsChildOf(module.transform)&&(c.GetType().FullName.StartsWith("VF.")||c.GetType().FullName.Contains("VRCFury"))).ToArray()){UnityEngine.Object.DestroyImmediate(c);removed++;}
  EditorUtility.SetDirty(avatar);EditorSceneManager.MarkSceneDirty(scene);EditorSceneManager.SaveScene(scene);AssetDatabase.SaveAssets();return new JObject{["success"]=true,["isolatedBaseParameters"]=true,["otherAuthoringParametersLocalized"]=local,["otherFuryComponentsRemoved"]=removed,["sourceAvatarModified"]=false}.ToString();
 }
 public static string Restore(){if(EditorApplication.isPlaying)throw new Exception("Edit Mode required");var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();if(scene.isDirty||scene.path!=ScenePath)throw new Exception("Saved owned validation scene required");EditorSceneManager.OpenScene(Mf6AvatarBuild.ScenePath);return "Restored original saved MF6 scene";}
}
