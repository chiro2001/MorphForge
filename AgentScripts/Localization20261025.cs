using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.SDK3.Avatars.Components;
using VRC.SDK3.Avatars.ScriptableObjects;
using VRC.SDKBase.Editor.BuildPipeline;
using nadena.dev.modular_avatar.core;
using MorphForge.Editor;
using MorphForgeAvatar;
using MorphForgeAvatar.Editor;

public static class Localization20261025 {
 const string Folder="Assets/MorphForge/Validation/Localization20261025";
 const string Reports="E:/tmp/morphforge/localization-20261025";
 static GameObject Rig(){
  var root=new GameObject("Localization fixture",typeof(Animator),typeof(VRCAvatarDescriptor));var bones=new Dictionary<string,Transform>();
  Transform Bone(string name,string parent,Vector3 p){var t=new GameObject(name).transform;t.SetParent(parent==null?root.transform:bones[parent],false);t.localPosition=p;bones[name]=t;return t;}
  Bone("Hips",null,new Vector3(0,1,0));Bone("Spine","Hips",new Vector3(0,.15f,0));Bone("Chest","Spine",new Vector3(0,.15f,0));Bone("Neck","Chest",new Vector3(0,.2f,0));Bone("Head","Neck",new Vector3(0,.15f,0));
  foreach(var side in new[]{"Left","Right"}){float s=side=="Left"?1:-1;Bone(side+"Shoulder","Chest",new Vector3(s*.1f,.1f,0));Bone(side+"UpperArm",side+"Shoulder",new Vector3(s*.1f,0,0));Bone(side+"LowerArm",side+"UpperArm",new Vector3(s*.25f,0,0));Bone(side+"Hand",side+"LowerArm",new Vector3(s*.25f,0,0));Bone(side+"UpperLeg","Hips",new Vector3(s*.1f,-.1f,0));Bone(side+"LowerLeg",side+"UpperLeg",new Vector3(0,-.4f,.01f));Bone(side+"Foot",side+"LowerLeg",new Vector3(0,-.4f,-.01f));Bone(side+"Toes",side+"Foot",new Vector3(0,0,.15f));}
  var human=AvatarBuilder.BuildHumanAvatar(root,new HumanDescription{human=bones.Select(k=>new HumanBone{boneName=k.Key,humanName=k.Key,limit=new HumanLimit{useDefaultValues=true}}).ToArray(),skeleton=root.GetComponentsInChildren<Transform>().Select(t=>new SkeletonBone{name=t.name,position=t.localPosition,rotation=t.localRotation,scale=t.localScale}).ToArray(),upperArmTwist=.5f,lowerArmTwist=.5f,upperLegTwist=.5f,lowerLegTwist=.5f,armStretch=.05f,legStretch=.05f});
  AssetDatabase.CreateAsset(human,AssetDatabase.GenerateUniqueAssetPath(Folder+"/Rig.asset"));root.GetComponent<Animator>().avatar=human;
  var d=root.GetComponent<VRCAvatarDescriptor>();d.customExpressions=true;d.customizeAnimationLayers=true;
  d.baseAnimationLayers=new[]{VRCAvatarDescriptor.AnimLayerType.Base,VRCAvatarDescriptor.AnimLayerType.Additive,VRCAvatarDescriptor.AnimLayerType.Gesture,VRCAvatarDescriptor.AnimLayerType.Action,VRCAvatarDescriptor.AnimLayerType.FX}.Select(type=>new VRCAvatarDescriptor.CustomAnimLayer{type=type,isDefault=true}).ToArray();
  d.specialAnimationLayers=new[]{VRCAvatarDescriptor.AnimLayerType.Sitting,VRCAvatarDescriptor.AnimLayerType.TPose,VRCAvatarDescriptor.AnimLayerType.IKPose}.Select(type=>new VRCAvatarDescriptor.CustomAnimLayer{type=type,isDefault=true}).ToArray();
  d.expressionParameters=ScriptableObject.CreateInstance<VRCExpressionParameters>();d.expressionParameters.parameters=new VRCExpressionParameters.Parameter[0];d.expressionsMenu=ScriptableObject.CreateInstance<VRCExpressionsMenu>();AssetDatabase.CreateAsset(d.expressionParameters,AssetDatabase.GenerateUniqueAssetPath(Folder+"/Parameters.asset"));AssetDatabase.CreateAsset(d.expressionsMenu,AssetDatabase.GenerateUniqueAssetPath(Folder+"/Menu.asset"));return root;
 }
 static HashSet<VRCExpressionsMenu> Menus(GameObject module){var seen=new HashSet<VRCExpressionsMenu>();void Add(VRCExpressionsMenu m){if(!m||!seen.Add(m))return;foreach(var c in m.controls)Add(c.subMenu);}foreach(var i in module.GetComponentsInChildren<ModularAvatarMenuInstaller>(true)){Add(i.menuToAppend);Add(i.installTargetMenu);}return seen;}
 static IEnumerable<string> Paths(VRCExpressionsMenu menu,string path="root",HashSet<VRCExpressionsMenu> ancestors=null){if(!menu)return Enumerable.Empty<string>();ancestors=ancestors==null?new HashSet<VRCExpressionsMenu>():new HashSet<VRCExpressionsMenu>(ancestors);if(!ancestors.Add(menu))return Enumerable.Empty<string>();return menu.controls.SelectMany(c=>new[]{path+"/"+c.name}.Concat(Paths(c.subMenu,path+"/"+c.name,ancestors)));}
 public static string Run(){
  var source=SceneManager.GetActiveScene();if(EditorApplication.isPlaying||source.path!=Mf6AvatarBuild.ScenePath||Enumerable.Range(0,SceneManager.sceneCount).Select(SceneManager.GetSceneAt).Any(s=>s.isDirty))throw new Exception("Clean saved MorphForge source required");
  string preference=PluginLocalization.SelectedLanguage;string sourceHash=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString(),publicHash=AssetDatabase.GetAssetDependencyHash(AvatarSetupWindow.PrefabPath).ToString();
  var checks=new JArray();var reports=new JArray();void Check(string name,bool pass){checks.Add(new JObject{["name"]=name,["passed"]=pass});Directory.CreateDirectory(Reports);File.WriteAllText(Reports+"/unity-progress.json",checks.ToString());if(!pass)throw new Exception(name);}
  CatalogVariant.EnsureFolder(Folder);Directory.CreateDirectory(Reports);var testScene=EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);
  try{
   var roots=new List<GameObject>();var modules=new List<GameObject>();
   foreach(string lang in PluginLocalization.Codes){
    PluginLocalization.SelectedLanguage=lang;var avatar=Rig();avatar.name="Localization-"+lang;roots.Add(avatar);
    var module=AvatarSetupWindow.Install(avatar.GetComponent<VRCAvatarDescriptor>(),false,language:lang);modules.Add(module);
    var menus=Menus(module);var preview=module.transform.Find("LocalPreview").GetComponent<ModularAvatarMenuInstaller>();
    Check(lang+" UI text translated",PluginLocalization.Text("一键安装到所选 Avatar")==PluginLocalization.Translate("一键安装到所选 Avatar",lang));
    Check(lang+" preview label translated",menus.SelectMany(m=>m.controls).Any(c=>c.name==PluginLocalization.Translate("本人预览",lang)));
    if(lang=="en")Check("all built-in English menu labels translated",menus.SelectMany(m=>m.controls).All(c=>!System.Text.RegularExpressions.Regex.IsMatch(c.name,"[\\u3400-\\u9fff]")));
    Check(lang+" all menu assets isolated",menus.All(m=>AssetDatabase.GetAssetPath(m).StartsWith(MenuLocalization.Root+"/"+module.GetComponent<MorphForgeAvatarModule>().menuInstanceId+"/"+lang+"/")));
    Check(lang+" target submenu remapped",menus.Contains(preview.installTargetMenu)&&preview.installTargetMenu.controls.Any(c=>c.parameter?.name=="MF_World"));
    Check(lang+" menu capacity",menus.All(m=>m.controls.Count<=8));
    reports.Add(new JObject{["language"]=lang,["labels"]=new JArray(menus.SelectMany(m=>m.controls).Select(c=>c.name)),["menus"]=menus.Count});
   }
   Check("avatars do not share localized menu assets",!Menus(modules[0]).Intersect(Menus(modules[1])).Any()&&!Menus(modules[1]).Intersect(Menus(modules[2])).Any());
   var secondBefore=string.Join("|",Menus(modules[1]).SelectMany(m=>m.controls).Select(c=>c.name));
   var customMenu=modules[0].GetComponent<ModularAvatarMenuInstaller>().menuToAppend;customMenu.controls.Add(new VRCExpressionsMenu.Control{name="用户自定义 / Custom",type=VRCExpressionsMenu.Control.ControlType.Toggle,parameter=new VRCExpressionsMenu.Control.Parameter{name="MF_World"},value=1});EditorUtility.SetDirty(customMenu);AssetDatabase.SaveAssets();
   foreach(string lang in new[]{"en","ja","zh-CN"})MenuLocalization.Apply(modules[0],lang);
   Check("switching preserves custom controls",Menus(modules[0]).SelectMany(m=>m.controls).Any(c=>c.name=="用户自定义 / Custom"));
   Check("switching leaves other avatar language unchanged",secondBefore==string.Join("|",Menus(modules[1]).SelectMany(m=>m.controls).Select(c=>c.name)));
   EditorSceneManager.SaveScene(testScene,Folder+"/Installed.unity");EditorSceneManager.OpenScene(Folder+"/Installed.unity",OpenSceneMode.Single);
   Check("menu language persists after scene reload",UnityEngine.Object.FindObjectsOfType<MorphForgeAvatarModule>().Select(m=>m.menuLanguage).OrderBy(x=>x).SequenceEqual(PluginLocalization.Codes.OrderBy(x=>x)));
   foreach(var avatar in UnityEngine.Object.FindObjectsOfType<VRCAvatarDescriptor>()){
    string lang=avatar.GetComponentInChildren<MorphForgeAvatarModule>(true).menuLanguage;
    foreach(var instance in avatar.GetComponentsInChildren<Transform>(true).Select(t=>t.gameObject).Where(PrefabUtility.IsAnyPrefabInstanceRoot).ToArray())if(PrefabUtility.IsPartOfPrefabInstance(instance))PrefabUtility.UnpackPrefabInstance(instance,PrefabUnpackMode.Completely,InteractionMode.AutomatedAction);
    bool built=true;foreach(var callback in TypeCache.GetTypesDerivedFrom<IVRCSDKPreprocessAvatarCallback>().Where(t=>!t.IsAbstract&&t.GetConstructor(Type.EmptyTypes)!=null).Select(t=>(IVRCSDKPreprocessAvatarCallback)System.Activator.CreateInstance(t)).OrderBy(c=>c.callbackOrder)){if(!callback.OnPreprocessAvatar(avatar.gameObject)){built=false;break;}}
    Check(lang+" actual SDK preprocessing",built);
    Check(lang+" merged SDK menu label",Paths(avatar.expressionsMenu).Any(p=>p.EndsWith("/MorphForge/"+PluginLocalization.Translate("本人预览",lang))));
    Check(lang+" protocol budget unchanged",avatar.expressionParameters.CalcTotalCost()==253);
   }
   Check("shared source prefab unchanged",sourceHash==AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString()&&publicHash==AssetDatabase.GetAssetDependencyHash(AvatarSetupWindow.PrefabPath).ToString());
  }finally{
   PluginLocalization.SelectedLanguage=preference;
   if(SceneManager.GetActiveScene().path.StartsWith(Folder+"/"))EditorSceneManager.SaveScene(SceneManager.GetActiveScene(),Folder+"/AfterSdk.unity",true);
   EditorSceneManager.OpenScene(Mf6AvatarBuild.ScenePath,OpenSceneMode.Single);
  }
  var result=new JObject{["success"]=true,["checks"]=checks,["reports"]=reports,["sourceClean"]=!SceneManager.GetActiveScene().isDirty,["scope"]="Three synthetic avatars, persisted Unity menu assets and actual SDK preprocessing. No private avatar changes or upload."};File.WriteAllText(Reports+"/unity-validation.json",result.ToString());return result.ToString();
 }
 public static string FinalizePackage(){
  if(EditorApplication.isPlaying||SceneManager.GetActiveScene().path!=Mf6AvatarBuild.ScenePath||SceneManager.GetActiveScene().isDirty)throw new Exception("Clean source required");
  var root=PrefabUtility.LoadPrefabContents(AvatarSetupWindow.PrefabPath);try{var marker=root.GetComponent<MorphForgeAvatarModule>();marker.version="1.4.0";marker.menuLanguage="zh-CN";marker.menuInstanceId=null;PrefabUtility.SaveAsPrefabAsset(root,AvatarSetupWindow.PrefabPath);}finally{PrefabUtility.UnloadPrefabContents(root);}AssetDatabase.SaveAssets();return "Public plugin version 1.4.0; source menu templates unchanged";
 }
}
