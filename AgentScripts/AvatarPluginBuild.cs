using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
using nadena.dev.modular_avatar.core;
using MorphForgeAvatar;
using MorphForgeAvatar.Editor;
using Newtonsoft.Json.Linq;

public static class AvatarPluginBuild {
 const string Root=AvatarSetupWindow.Root;
 static void Folder(string path)=>CatalogVariant.EnsureFolder(path);
 static IEnumerable<UnityEngine.Object> References(UnityEngine.Object value) {
  if(!value)yield break;var so=new SerializedObject(value);var p=so.GetIterator();
  while(p.Next(true))if(p.propertyType==SerializedPropertyType.ObjectReference&&p.objectReferenceValue&&
   p.name!="m_CorrespondingSourceObject"&&p.name!="m_PrefabInstance"&&p.name!="m_PrefabAsset")yield return p.objectReferenceValue;
 }
 static string Key(UnityEngine.Object value){AssetDatabase.TryGetGUIDAndLocalFileIdentifier(value,out string guid,out long id);return guid+":"+id;}
 static void Remap(UnityEngine.Object value,Dictionary<string,UnityEngine.Object> map){
  var so=new SerializedObject(value);var p=so.GetIterator();bool changed=false;
  while(p.Next(true))if(p.propertyType==SerializedPropertyType.ObjectReference&&p.objectReferenceValue&&map.TryGetValue(Key(p.objectReferenceValue),out var replacement)){p.objectReferenceValue=replacement;changed=true;}
  if(changed){so.ApplyModifiedPropertiesWithoutUndo();EditorUtility.SetDirty(value);}
 }
 public static string Build(){
  if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");
  Folder(Root+"/Generated");
  var source="E:/tmp/morphforge/avatar-plugin/public-source";Folder(Root+"/CatalogSource/artifacts");
  foreach(var file in Directory.GetFiles(source))File.Copy(file,Root+"/CatalogSource/"+Path.GetFileName(file),true);
  foreach(var file in Directory.GetFiles(source+"/artifacts"))File.Copy(file,Root+"/CatalogSource/artifacts/"+Path.GetFileName(file),true);
  AssetDatabase.Refresh();
  CatalogMaterialBuild.Build(Root+"/CatalogSource");
  var baseMaterial=AssetDatabase.LoadAssetAtPath<Material>(Root+"/Generated/Catalog/ParticleBoxCatalog.mat");
  baseMaterial.shader=Shader.Find("MorphForgeAvatar/ParticleBoxCatalogMetadataGrid");baseMaterial.SetFloat("_WireEncoding",6);EditorUtility.SetDirty(baseMaterial);
  var avatar=UnityEngine.Object.Instantiate(AssetDatabase.LoadAssetAtPath<GameObject>("Assets/MorphForge/MF6/Avatar.prefab"));
  avatar.hideFlags=HideFlags.HideAndDontSave;
  try{
   if(PrefabUtility.IsPartOfPrefabInstance(avatar))PrefabUtility.UnpackPrefabInstance(avatar,PrefabUnpackMode.Completely,InteractionMode.AutomatedAction);
   var module=avatar.transform.Find("MorphForgePlacement").gameObject;module.name="MorphForge";
   var stores=module.GetComponentsInChildren<ParticleSystemRenderer>(true).Where(r=>r.name=="Store").ToArray();
   foreach(var r in stores){r.sharedMaterial=baseMaterial;r.sortMode=ParticleSystemSortMode.Distance;}
   var dependencies=module.GetComponentsInChildren<Component>(true).Where(c=>c).SelectMany(References).Select(AssetDatabase.GetAssetPath).Where(p=>p.StartsWith("Assets/",StringComparison.Ordinal)&&!p.StartsWith(Root+"/",StringComparison.Ordinal)).Distinct().ToArray();
   dependencies=AssetDatabase.GetDependencies(dependencies,true).Where(p=>p.StartsWith("Assets/",StringComparison.Ordinal)&&!p.StartsWith(Root+"/",StringComparison.Ordinal)).Distinct().ToArray();
   if(dependencies.Any(p=>p.IndexOf("Nemesis",StringComparison.OrdinalIgnoreCase)>=0||p.EndsWith(".prefab")||p.EndsWith(".unity")||p.EndsWith(".cs")))throw new Exception("Unsafe dependency: "+string.Join("\n",dependencies));
   var map=new Dictionary<string,UnityEngine.Object>();var copied=new List<string>();
   foreach(var old in dependencies){
    string dest=Root+"/Generated/ResourcesV12/"+old.Substring("Assets/".Length);Folder(Path.GetDirectoryName(dest).Replace('\\','/'));
    if(AssetDatabase.LoadMainAssetAtPath(dest))throw new Exception("Refusing to overwrite generated asset: "+dest);
    if(!AssetDatabase.CopyAsset(old,dest))throw new Exception("Copy failed: "+old);
    var newObjects=AssetDatabase.LoadAllAssetsAtPath(dest).ToDictionary(o=>{AssetDatabase.TryGetGUIDAndLocalFileIdentifier(o,out string g,out long id);return id;});
    foreach(var value in AssetDatabase.LoadAllAssetsAtPath(old)){AssetDatabase.TryGetGUIDAndLocalFileIdentifier(value,out string g,out long id);if(!newObjects.TryGetValue(id,out var next))throw new Exception("Subasset mapping failed: "+old+" / "+id);map[Key(value)]=next;}
    copied.Add(dest);
   }
   foreach(var asset in copied.SelectMany(AssetDatabase.LoadAllAssetsAtPath))Remap(asset,map);
   foreach(var c in module.GetComponentsInChildren<Component>(true).Where(c=>c))Remap(c,map);
   var config=module.GetComponent<ModularAvatarParameters>();var handleIndex=config.parameters.FindIndex(p=>p.nameOrPrefix=="MF_Handle");var handle=config.parameters[handleIndex];handle.localOnly=true;config.parameters[handleIndex]=handle;EditorUtility.SetDirty(config);
   var placement=(AnimatorController)module.GetComponent<ModularAvatarMergeAnimator>().animator;
   var machine=new AnimatorStateMachine{name="MorphForge owner helper"};AssetDatabase.AddObjectToAsset(machine,placement);
   var hidden=new AnimationClip{name="Remote helper hidden"};AnimationUtility.SetEditorCurve(hidden,EditorCurveBinding.FloatCurve("WorldFrame/AnchorPose/HandleVisual",typeof(GameObject),"m_IsActive"),AnimationCurve.Constant(0,.01f,0));AssetDatabase.AddObjectToAsset(hidden,placement);
   var shown=machine.AddState("Owner");shown.writeDefaultValues=false;var remote=machine.AddState("Remote");remote.motion=hidden;remote.writeDefaultValues=false;machine.defaultState=shown;
   var t=shown.AddTransition(remote);t.hasExitTime=false;t.duration=0;t.AddCondition(AnimatorConditionMode.IfNot,0,"IsLocal");
   t=remote.AddTransition(shown);t.hasExitTime=false;t.duration=0;t.AddCondition(AnimatorConditionMode.If,0,"IsLocal");
   placement.AddLayer(new AnimatorControllerLayer{name="MorphForge owner helper",stateMachine=machine,defaultWeight=1});EditorUtility.SetDirty(placement);
   var catalog=JObject.Parse(File.ReadAllText(Root+"/CatalogSource/catalog.json"));var capabilities=(JArray)catalog["capabilities"];
   foreach(var capability in new[]{"mf6-region8/1","mf6-text-panels/1","mf6-text-latest14/1","local-placement-controls/1","mf6-motion4/1","local-avatar-preview-prototype/1"})if(!capabilities.Any(c=>(string)c==capability))capabilities.Add(capability);
   if(!module.GetComponentsInChildren<ModularAvatarMergeAnimator>(true).Any(m=>m.animator&&AssetDatabase.GetAssetPath(m.animator).EndsWith("/Motion.controller"))||!module.transform.Find("LocalPreview"))throw new Exception("Missing native motion or preview assets; refusing capability advertisement");
   File.WriteAllText(Root+"/CatalogSource/catalog.json",catalog.ToString());var identity=CatalogIdentity.Apply(module,catalog);
   var marker=module.AddComponent<MorphForgeAvatarModule>();marker.catalogIdentity=identity;
   module.transform.SetParent(null,false);module.hideFlags=HideFlags.None;
   try{PrefabUtility.SaveAsPrefabAsset(module,AvatarSetupWindow.PrefabPath);}finally{UnityEngine.Object.DestroyImmediate(module);}
   AssetDatabase.SaveAssets();
   var closure=AssetDatabase.GetDependencies(AvatarSetupWindow.PrefabPath,true);
   var outside=closure.Where(p=>p.StartsWith("Assets/",StringComparison.Ordinal)&&!p.StartsWith(Root+"/",StringComparison.Ordinal)).ToArray();if(outside.Length>0)throw new Exception("Package escaped root: "+string.Join("\n",outside));
   var report=new JObject{["success"]=true,["prefab"]=AvatarSetupWindow.PrefabPath,["catalogIdentity"]=identity,["materials"]=((JArray)catalog["materials"]).Count,["stores"]=stores.Length,["copiedAssets"]=copied.Count,["dependencyCount"]=closure.Length,["externalAssets"]=new JArray(outside),["dependencyPaths"]=new JArray(closure)};
   Directory.CreateDirectory("Docs/AvatarPlugin");File.WriteAllText("Docs/AvatarPlugin/package-build.json",report.ToString());return report.ToString();
  }finally{UnityEngine.Object.DestroyImmediate(avatar);}
 }
 public static string Export(){
  if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");
  AssetDatabase.Refresh();
  var publicCatalog=JObject.Parse(File.ReadAllText(Root+"/CatalogSource/catalog.json"));
  foreach(string field in new[]{"targetAvatarId","sourceProject","sourceRevision"})publicCatalog.Remove(field);
  File.WriteAllText(Root+"/CatalogSource/catalog.json",publicCatalog.ToString());
  var surface=AssetDatabase.LoadAssetAtPath<Material>(Root+"/Generated/Catalog/ParticleBoxCatalog.mat");
  var saved=new SerializedObject(surface);var floats=saved.FindProperty("m_SavedProperties.m_Floats");
  for(int i=floats.arraySize-1;i>=0;i--)if(floats.GetArrayElementAtIndex(i).FindPropertyRelative("first").stringValue=="_SurfacePass")floats.DeleteArrayElementAtIndex(i);
  saved.ApplyModifiedPropertiesWithoutUndo();AssetDatabase.SaveAssets();AssetDatabase.Refresh();
  // Never export user-created/private catalog variants.
  var active=new HashSet<string>(AssetDatabase.GetDependencies(AvatarSetupWindow.PrefabPath,true));
  var files=AssetDatabase.FindAssets("",new[]{Root}).Select(AssetDatabase.GUIDToAssetPath).Where(p=>!AssetDatabase.IsValidFolder(p)&&!p.StartsWith(Root+"/Variants/",StringComparison.Ordinal)&&(!p.StartsWith(Root+"/Generated/",StringComparison.Ordinal)||active.Contains(p))).ToArray();
  var dependencies=AssetDatabase.GetDependencies(files,true).Where(p=>p.StartsWith("Assets/")&&!p.StartsWith(Root+"/")).ToArray();
  if(dependencies.Length!=0)throw new Exception("Unsafe export dependencies: "+string.Join("\n",dependencies));
  Directory.CreateDirectory("E:/tmp/morphforge/avatar-plugin/dist");string path="E:/tmp/morphforge/avatar-plugin/dist/MorphForgeAvatar-1.2.0.unitypackage";
  AssetDatabase.ExportPackage(files,path,ExportPackageOptions.Default);
  CatalogVariant.ExportBuiltin("E:/tmp/morphforge/avatar-plugin/dist/MorphForge-public-catalog.zip");
  return new JObject{["success"]=true,["path"]=path,["bytes"]=new FileInfo(path).Length,["files"]=files.Length}.ToString();
 }
}
