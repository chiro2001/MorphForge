using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using UnityEngine;
using UnityEditor;
using UnityEditor.Animations;
using VRC.SDK3.Avatars.Components;
using VRC.SDK3.Avatars.ScriptableObjects;
using VRC.SDKBase.Editor.BuildPipeline;
using Newtonsoft.Json.Linq;
using MorphForgeAvatar;
using MorphForgeAvatar.Editor;
public static class PluginAcceptance {
 public static GameObject Rig(){
  var root=new GameObject("MorphForge clean fixture",typeof(Animator),typeof(VRCAvatarDescriptor));
  var bones=new Dictionary<string,Transform>();
  Transform Bone(string name,string parent,Vector3 position){var t=new GameObject(name).transform;t.SetParent(parent==null?root.transform:bones[parent],false);t.localPosition=position;bones[name]=t;return t;}
  Bone("Hips",null,new Vector3(0,1,0));Bone("Spine","Hips",new Vector3(0,.15f,0));Bone("Chest","Spine",new Vector3(0,.15f,0));Bone("Neck","Chest",new Vector3(0,.2f,0));Bone("Head","Neck",new Vector3(0,.15f,0));
  foreach(var side in new[]{"Left","Right"}){float sign=side=="Left"?1:-1;Bone(side+"Shoulder","Chest",new Vector3(sign*.1f,.1f,0));Bone(side+"UpperArm",side+"Shoulder",new Vector3(sign*.1f,0,0));Bone(side+"LowerArm",side+"UpperArm",new Vector3(sign*.25f,0,0));Bone(side+"Hand",side+"LowerArm",new Vector3(sign*.25f,0,0));Bone(side+"UpperLeg","Hips",new Vector3(sign*.1f,-.1f,0));Bone(side+"LowerLeg",side+"UpperLeg",new Vector3(0,-.4f,.01f));Bone(side+"Foot",side+"LowerLeg",new Vector3(0,-.4f,-.01f));Bone(side+"Toes",side+"Foot",new Vector3(0,0,.15f));}
  var description=new HumanDescription{human=bones.Select(k=>new HumanBone{boneName=k.Key,humanName=k.Key,limit=new HumanLimit{useDefaultValues=true}}).ToArray(),skeleton=root.GetComponentsInChildren<Transform>().Select(t=>new SkeletonBone{name=t.name,position=t.localPosition,rotation=t.localRotation,scale=t.localScale}).ToArray(),upperArmTwist=.5f,lowerArmTwist=.5f,upperLegTwist=.5f,lowerLegTwist=.5f,armStretch=.05f,legStretch=.05f,feetSpacing=0,hasTranslationDoF=false};
  var human=AvatarBuilder.BuildHumanAvatar(root,description);if(!human.isValid||!human.isHuman)throw new Exception("Synthetic humanoid invalid");root.GetComponent<Animator>().avatar=human;
  return root;
 }
 static JArray GateChecks() {
  var checks=new JArray();var root=new GameObject("budget negative fixtures",typeof(VRCAvatarDescriptor));
  var p=ScriptableObject.CreateInstance<VRCExpressionParameters>();root.GetComponent<VRCAvatarDescriptor>().expressionParameters=p;
  try {
   var geometry=PluginBudgetGate.Geometry.Select(name=>new VRCExpressionParameters.Parameter{name=name,valueType=VRCExpressionParameters.ValueType.Int,networkSynced=true}).ToList();
   geometry.Add(new VRCExpressionParameters.Parameter{name=PluginBudgetGate.Marker,valueType=VRCExpressionParameters.ValueType.Int,networkSynced=false});
   for(int i=0;i<3;i++)geometry.Add(new VRCExpressionParameters.Parameter{name="Face"+i,valueType=VRCExpressionParameters.ValueType.Float,networkSynced=true});
   p.parameters=geometry.ToArray();bool rejected=false;try{new PluginBudgetGate().OnPreprocessAvatar(root);}catch(InvalidOperationException){rejected=true;}
   checks.Add(new JObject{["name"]="uncompressible over-budget rejected without changing sync flags",["pass"]=rejected&&p.CalcTotalCost()==264&&geometry.Where(v=>v.name.StartsWith("Face")).All(v=>v.networkSynced)});
   p.parameters=geometry.Where(v=>!v.name.StartsWith("Face")).ToArray();p.parameters[0].networkSynced=false;rejected=false;try{new PluginBudgetGate().OnPreprocessAvatar(root);}catch(InvalidOperationException){rejected=true;}
   checks.Add(new JObject{["name"]="compressed geometry rejected even when budget fits",["pass"]=rejected&&p.CalcTotalCost()<256});
   return checks;
  } finally { UnityEngine.Object.DestroyImmediate(root);UnityEngine.Object.DestroyImmediate(p); }
 }
 public static string Import(){
  AssetDatabase.ImportPackage("E:/tmp/morphforge/avatar-plugin/dist/MorphForgeAvatar-1.0.0.unitypackage",false);return "Imported via AssetDatabase.ImportPackage";
 }
 public static string Run(){
  var checks=GateChecks();var reports=new JArray();bool hasFury=AvatarSetupWindow.HasFury;
  foreach(var test in new[]{new{name="standalone",extras=0,fury=false},new{name="menu-compression",extras=8,fury=true},new{name="uncompressible-rejected",extras=0,fury=false}}){
   if(test.fury&&!hasFury)continue;if(test.name=="uncompressible-rejected")continue;
   var avatar=Rig();avatar.hideFlags=HideFlags.HideAndDontSave;
   var before=new HashSet<int>(Resources.FindObjectsOfTypeAll<GameObject>().Select(o=>o.GetInstanceID()));
   try{
    var d=avatar.GetComponent<VRCAvatarDescriptor>();d.customExpressions=true;d.customizeAnimationLayers=true;d.baseAnimationLayers=new[]{VRCAvatarDescriptor.AnimLayerType.Base,VRCAvatarDescriptor.AnimLayerType.Additive,VRCAvatarDescriptor.AnimLayerType.Gesture,VRCAvatarDescriptor.AnimLayerType.Action,VRCAvatarDescriptor.AnimLayerType.FX}.Select(type=>new VRCAvatarDescriptor.CustomAnimLayer{type=type,isDefault=true}).ToArray();d.specialAnimationLayers=new[]{VRCAvatarDescriptor.AnimLayerType.Sitting,VRCAvatarDescriptor.AnimLayerType.TPose,VRCAvatarDescriptor.AnimLayerType.IKPose}.Select(type=>new VRCAvatarDescriptor.CustomAnimLayer{type=type,isDefault=true}).ToArray();
    var parameters=ScriptableObject.CreateInstance<VRCExpressionParameters>();
    var menu=ScriptableObject.CreateInstance<VRCExpressionsMenu>();var values=new List<VRCExpressionParameters.Parameter>();
    for(int i=0;i<test.extras;i++){var name="Clothes"+i;values.Add(new VRCExpressionParameters.Parameter{name=name,valueType=VRCExpressionParameters.ValueType.Bool,networkSynced=true});menu.controls.Add(new VRCExpressionsMenu.Control{name=name,type=VRCExpressionsMenu.Control.ControlType.Toggle,parameter=new VRCExpressionsMenu.Control.Parameter{name=name},value=1});}
    if(test.name=="uncompressible-rejected")for(int i=0;i<4;i++)values.Add(new VRCExpressionParameters.Parameter{name="Face"+i,valueType=VRCExpressionParameters.ValueType.Float,networkSynced=true});
    parameters.parameters=values.ToArray();d.expressionParameters=parameters;d.expressionsMenu=menu;
    var fixtureFx=new AnimatorController{name="Clean fixture FX"};fixtureFx.AddLayer("Fixture");fixtureFx.layers[0].stateMachine.AddState("Idle").writeDefaultValues=false;
    var layers=d.baseAnimationLayers;for(int i=0;i<layers.Length;i++)if(layers[i].type==VRCAvatarDescriptor.AnimLayerType.FX){layers[i].isDefault=false;layers[i].animatorController=fixtureFx;}d.baseAnimationLayers=layers;
    var module=AvatarSetupWindow.Install(d,test.fury);
    if(test.fury){
     // Fixture-only choice: keep native/SDK write-default values. This avoids
     // VRCFury asking a modal question during unattended acceptance; it does
     // not add any policy to the shipped plugin or modify global preferences.
     var featureType=AvatarSetupWindow.FindType("VF.Model.Feature.FixWriteDefaults");
     var content=Activator.CreateInstance(featureType,true);
     var mode=featureType.GetField("mode");mode.SetValue(content,Enum.Parse(mode.FieldType,"Disabled"));
     var component=avatar.AddComponent(AvatarSetupWindow.FindType("VF.Model.VRCFury"));
     component.GetType().GetField("content").SetValue(component,content);
    }
    bool duplicate=false;try{AvatarSetupWindow.Install(d,false);}catch(InvalidOperationException){duplicate=true;}
    checks.Add(new JObject{["name"]=test.name+" duplicate install blocked",["pass"]=duplicate});
    if(PrefabUtility.IsPartOfPrefabInstance(module))PrefabUtility.UnpackPrefabInstance(module,PrefabUnpackMode.Completely,InteractionMode.AutomatedAction);
    bool built=false;string error=null;
    try{
      var callbacks=TypeCache.GetTypesDerivedFrom<IVRCSDKPreprocessAvatarCallback>().Where(t=>!t.IsAbstract && t.GetConstructor(Type.EmptyTypes)!=null).Select(t=>(IVRCSDKPreprocessAvatarCallback)Activator.CreateInstance(t)).OrderBy(c=>c.callbackOrder).ToArray();
      built=true;foreach(var callback in callbacks){if(!callback.OnPreprocessAvatar(avatar)){built=false;error="Callback returned false: "+callback.GetType().FullName;break;}}
    }catch(Exception e){error=e.GetBaseException().Message;}
    JObject report=null;if(File.Exists("Logs/MorphForge/avatar-budget.json"))report=JObject.Parse(File.ReadAllText("Logs/MorphForge/avatar-budget.json"));
    bool reject=test.name=="uncompressible-rejected";
    bool pass=reject?!built:built&&report!=null&&(bool)report["success"];
    if(!reject&&test.extras>0)pass&=Enumerable.Range(0,test.extras).All(i=>d.expressionParameters.parameters.Any(p=>p.name=="Clothes"+i));
    checks.Add(new JObject{["name"]=test.name+" SDK preprocessing",["pass"]=pass});
    reports.Add(new JObject{["name"]=test.name,["built"]=built,["error"]=error,["budget"]=report});
   }finally{foreach(var go in Resources.FindObjectsOfTypeAll<GameObject>().Where(o=>o&&!EditorUtility.IsPersistent(o)&&!before.Contains(o.GetInstanceID())).ToArray())if(go)UnityEngine.Object.DestroyImmediate(go);if(avatar)UnityEngine.Object.DestroyImmediate(avatar);}
  }
  var result=new JObject{["success"]=checks.OfType<JObject>().All(c=>(bool)c["pass"]),["vrcFury"]=hasFury,["checks"]=checks,["reports"]=reports,["scope"]="Synthetic humanoid fixture, real MA/NDMF and SDK preprocessing callbacks. No commercial Avatar asset or real VRChat connection."};Directory.CreateDirectory("E:/tmp/morphforge/avatar-plugin/reports");File.WriteAllText("E:/tmp/morphforge/avatar-plugin/reports/acceptance-"+(hasFury?"fury":"ma")+".json",result.ToString());return new JObject{["success"]=result["success"],["vrcFury"]=hasFury,["checks"]=checks,["budgets"]=new JArray(reports.OfType<JObject>().Select(r=>new JObject{["name"]=r["name"],["built"]=r["built"],["error"]=r["error"],["bits"]=r["budget"]?["totalBits"]}))}.ToString();
 }
}
