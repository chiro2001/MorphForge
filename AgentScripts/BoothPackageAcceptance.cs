using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.SDK3.Avatars.Components;
using VRC.SDK3.Avatars.ScriptableObjects;
using VRC.SDKBase.Editor.BuildPipeline;
using nadena.dev.modular_avatar.core;
using MorphForgeAvatar;
using MorphForgeAvatar.Editor;
using Activator = System.Activator;

public static class BoothPackageAcceptance {
    const string Reports="E:/tmp/morphforge/booth-20261025/clean-import";
    const string Folder="Assets/BoothAcceptance20261025";
    static readonly JArray Checks=new JArray();
    static readonly JArray Builds=new JArray();
    static void Check(string name,bool pass) {
        Checks.Add(new JObject{["name"]=name,["passed"]=pass});
        File.WriteAllText(Reports+"/progress.json",Checks.ToString());
        if(!pass)throw new Exception(name);
    }
    static string Sha(byte[] data){using(var h=SHA256.Create())return BitConverter.ToString(h.ComputeHash(data)).Replace("-","").ToLowerInvariant();}
    static bool EmissionOnly(Renderer renderer){
        if(!(renderer is ParticleSystemRenderer)||renderer.enabled||(renderer.name!="Pump"&&renderer.name!="BoundsPump"))return false;
        var system=renderer.GetComponent<ParticleSystem>();if(!system)return false;
        var sub=system.subEmitters;
        return sub.enabled&&sub.subEmittersCount>0&&Enumerable.Range(0,sub.subEmittersCount).All(i=>sub.GetSubEmitterSystem(i));
    }
    static bool MatchFile(JObject item){
        string path=(string)item["path"];if(!File.Exists(path))return false;
        byte[] data=File.ReadAllBytes(path);
        if((bool)item["normalizeText"])data=Encoding.UTF8.GetBytes(Encoding.UTF8.GetString(data).Replace("\r\n","\n"));
        return Sha(data)==(string)item["sha256"]&&AssetDatabase.AssetPathToGUID(path)==(string)item["guid"];
    }
    static GameObject Rig(string name,int extras){
        var root=new GameObject(name,typeof(Animator),typeof(VRCAvatarDescriptor));
        var bones=new Dictionary<string,Transform>();
        Transform Bone(string n,string parent,Vector3 p){var t=new GameObject(n).transform;t.SetParent(parent==null?root.transform:bones[parent],false);t.localPosition=p;bones[n]=t;return t;}
        Bone("Hips",null,new Vector3(0,1,0));Bone("Spine","Hips",new Vector3(0,.15f,0));Bone("Chest","Spine",new Vector3(0,.15f,0));Bone("Neck","Chest",new Vector3(0,.2f,0));Bone("Head","Neck",new Vector3(0,.15f,0));
        foreach(string side in new[]{"Left","Right"}){float s=side=="Left"?1:-1;Bone(side+"Shoulder","Chest",new Vector3(s*.1f,.1f,0));Bone(side+"UpperArm",side+"Shoulder",new Vector3(s*.1f,0,0));Bone(side+"LowerArm",side+"UpperArm",new Vector3(s*.25f,0,0));Bone(side+"Hand",side+"LowerArm",new Vector3(s*.25f,0,0));Bone(side+"UpperLeg","Hips",new Vector3(s*.1f,-.1f,0));Bone(side+"LowerLeg",side+"UpperLeg",new Vector3(0,-.4f,.01f));Bone(side+"Foot",side+"LowerLeg",new Vector3(0,-.4f,-.01f));Bone(side+"Toes",side+"Foot",new Vector3(0,0,.15f));}
        var human=AvatarBuilder.BuildHumanAvatar(root,new HumanDescription{human=bones.Select(k=>new HumanBone{boneName=k.Key,humanName=k.Key,limit=new HumanLimit{useDefaultValues=true}}).ToArray(),skeleton=root.GetComponentsInChildren<Transform>().Select(t=>new SkeletonBone{name=t.name,position=t.localPosition,rotation=t.localRotation,scale=t.localScale}).ToArray(),upperArmTwist=.5f,lowerArmTwist=.5f,upperLegTwist=.5f,lowerLegTwist=.5f,armStretch=.05f,legStretch=.05f});
        if(!human.isHuman||!human.isValid)throw new Exception("Synthetic humanoid invalid");
        AssetDatabase.CreateAsset(human,AssetDatabase.GenerateUniqueAssetPath(Folder+"/"+name+"-Rig.asset"));root.GetComponent<Animator>().avatar=human;
        var d=root.GetComponent<VRCAvatarDescriptor>();d.customExpressions=true;d.customizeAnimationLayers=true;
        d.baseAnimationLayers=new[]{VRCAvatarDescriptor.AnimLayerType.Base,VRCAvatarDescriptor.AnimLayerType.Additive,VRCAvatarDescriptor.AnimLayerType.Gesture,VRCAvatarDescriptor.AnimLayerType.Action,VRCAvatarDescriptor.AnimLayerType.FX}.Select(t=>new VRCAvatarDescriptor.CustomAnimLayer{type=t,isDefault=true}).ToArray();
        d.specialAnimationLayers=new[]{VRCAvatarDescriptor.AnimLayerType.Sitting,VRCAvatarDescriptor.AnimLayerType.TPose,VRCAvatarDescriptor.AnimLayerType.IKPose}.Select(t=>new VRCAvatarDescriptor.CustomAnimLayer{type=t,isDefault=true}).ToArray();
        d.expressionParameters=ScriptableObject.CreateInstance<VRCExpressionParameters>();d.expressionsMenu=ScriptableObject.CreateInstance<VRCExpressionsMenu>();
        d.expressionParameters.parameters=Enumerable.Range(0,extras).Select(i=>new VRCExpressionParameters.Parameter{name="Clothes"+i,valueType=VRCExpressionParameters.ValueType.Bool,networkSynced=true}).ToArray();
        foreach(var p in d.expressionParameters.parameters)d.expressionsMenu.controls.Add(new VRCExpressionsMenu.Control{name=p.name,type=VRCExpressionsMenu.Control.ControlType.Toggle,parameter=new VRCExpressionsMenu.Control.Parameter{name=p.name},value=1});
        AssetDatabase.CreateAsset(d.expressionParameters,AssetDatabase.GenerateUniqueAssetPath(Folder+"/"+name+"-Parameters.asset"));AssetDatabase.CreateAsset(d.expressionsMenu,AssetDatabase.GenerateUniqueAssetPath(Folder+"/"+name+"-Menu.asset"));
        var fx=AnimatorController.CreateAnimatorControllerAtPath(AssetDatabase.GenerateUniqueAssetPath(Folder+"/"+name+"-FX.controller"));fx.layers[0].stateMachine.AddState("Idle").writeDefaultValues=false;
        var layers=d.baseAnimationLayers;for(int i=0;i<layers.Length;i++)if(layers[i].type==VRCAvatarDescriptor.AnimLayerType.FX){layers[i].isDefault=false;layers[i].animatorController=fx;}d.baseAnimationLayers=layers;
        return root;
    }
    static HashSet<VRCExpressionsMenu> Menus(GameObject module){var set=new HashSet<VRCExpressionsMenu>();void Add(VRCExpressionsMenu m){if(!m||!set.Add(m))return;foreach(var c in m.controls)Add(c.subMenu);}foreach(var i in module.GetComponentsInChildren<ModularAvatarMenuInstaller>(true)){Add(i.menuToAppend);Add(i.installTargetMenu);}return set;}
    static IEnumerable<string> Labels(VRCExpressionsMenu m,HashSet<VRCExpressionsMenu> seen=null){if(!m)return Enumerable.Empty<string>();seen=seen??new HashSet<VRCExpressionsMenu>();if(!seen.Add(m))return Enumerable.Empty<string>();return m.controls.SelectMany(c=>new[]{c.name}.Concat(Labels(c.subMenu,seen)));}
    static void FuryFixture(GameObject root){
        // Only the synthetic fixture chooses Disabled to avoid VRCFury's
        // unattended WD modal. No global preference or shipped asset changes.
        var type=AvatarSetupWindow.FindType("VF.Model.Feature.FixWriteDefaults");var content=Activator.CreateInstance(type,true);
        var mode=type.GetField("mode");mode.SetValue(content,Enum.Parse(mode.FieldType,"Disabled"));
        var component=root.AddComponent(AvatarSetupWindow.FindType("VF.Model.VRCFury"));component.GetType().GetField("content").SetValue(component,content);
    }
    static void Build(GameObject avatar,string label,int extras){
        foreach(var instance in avatar.GetComponentsInChildren<Transform>(true).Select(t=>t.gameObject).Where(PrefabUtility.IsAnyPrefabInstanceRoot).ToArray())if(PrefabUtility.IsPartOfPrefabInstance(instance))PrefabUtility.UnpackPrefabInstance(instance,PrefabUnpackMode.Completely,InteractionMode.AutomatedAction);
        bool built=true;string error=null;
        try{foreach(var c in TypeCache.GetTypesDerivedFrom<IVRCSDKPreprocessAvatarCallback>().Where(t=>!t.IsAbstract&&t.GetConstructor(Type.EmptyTypes)!=null).Select(t=>(IVRCSDKPreprocessAvatarCallback)Activator.CreateInstance(t)).OrderBy(c=>c.callbackOrder))if(!c.OnPreprocessAvatar(avatar)){built=false;error=c.GetType().FullName;break;}}
        catch(Exception e){built=false;error=e.GetBaseException().Message;}
        var d=avatar.GetComponent<VRCAvatarDescriptor>();
        Builds.Add(new JObject{["name"]=label,["success"]=built,["error"]=error,["bits"]=d.expressionParameters?d.expressionParameters.CalcTotalCost():-1});
        File.WriteAllText(Reports+"/build-progress.json",Builds.ToString());
        Check(label+" actual SDK preprocessing",built);
        Check(label+" final parameter budget",d.expressionParameters.CalcTotalCost()<=(extras>0?256:253)&&(extras>0||d.expressionParameters.CalcTotalCost()==253));
        Check(label+" 30 geometry Ints remain directly synced",PluginBudgetGate.Geometry.All(n=>d.expressionParameters.parameters.Count(p=>p.name==n&&p.networkSynced&&p.valueType==VRCExpressionParameters.ValueType.Int)==1));
        string language=label.StartsWith("fury")?"ja":label;
        Check(label+" merged owner-preview menu is localized",Labels(d.expressionsMenu).Contains(PluginLocalization.Translate("本人预览",language)));
        if(extras>0)Check(label+" existing clothing parameters retained",Enumerable.Range(0,extras).All(i=>d.expressionParameters.parameters.Any(p=>p.name=="Clothes"+i)));
    }
    static void NegativeBudgetChecks(){
        var root=new GameObject("negative-budget",typeof(VRCAvatarDescriptor));var parameters=ScriptableObject.CreateInstance<VRCExpressionParameters>();root.GetComponent<VRCAvatarDescriptor>().expressionParameters=parameters;
        try{
            var values=PluginBudgetGate.Geometry.Select(n=>new VRCExpressionParameters.Parameter{name=n,valueType=VRCExpressionParameters.ValueType.Int,networkSynced=true}).ToList();
            values.Add(new VRCExpressionParameters.Parameter{name=PluginBudgetGate.Marker,valueType=VRCExpressionParameters.ValueType.Int,networkSynced=false});
            for(int i=0;i<3;i++)values.Add(new VRCExpressionParameters.Parameter{name="Face"+i,valueType=VRCExpressionParameters.ValueType.Float,networkSynced=true});
            parameters.parameters=values.ToArray();bool rejected=false;try{new PluginBudgetGate().OnPreprocessAvatar(root);}catch(InvalidOperationException){rejected=true;}
            Check("over-budget face tracking rejected without disabling sync",rejected&&parameters.CalcTotalCost()==264&&values.Where(v=>v.name.StartsWith("Face")).All(v=>v.networkSynced));
            parameters.parameters=values.Where(v=>!v.name.StartsWith("Face")).ToArray();parameters.parameters[0].networkSynced=false;rejected=false;try{new PluginBudgetGate().OnPreprocessAvatar(root);}catch(InvalidOperationException){rejected=true;}
            Check("compressed geometry rejected despite fitting budget",rejected&&parameters.CalcTotalCost()<256);
        }finally{UnityEngine.Object.DestroyImmediate(root);UnityEngine.Object.DestroyImmediate(parameters);}
    }
    static void RestoreOriginal(){
        var setup=JArray.Parse(File.ReadAllText(Reports+"/original-scenes.json")).OfType<JObject>().Where(s=>!string.IsNullOrEmpty((string)s["path"])&&!((string)s["path"]).StartsWith("Packages/")).Select(s=>new SceneSetup{path=(string)s["path"],isLoaded=(bool)s["isLoaded"],isActive=(bool)s["isActive"]}).ToArray();
        if(setup.Length>0)EditorSceneManager.RestoreSceneManagerSetup(setup);else EditorSceneManager.NewScene(NewSceneSetup.DefaultGameObjects,NewSceneMode.Single);
    }
    public static string InspectRenderers(){
        var prefab=AssetDatabase.LoadAssetAtPath<GameObject>(AvatarSetupWindow.PrefabPath);
        var rows=new JArray();
        foreach(var r in prefab.GetComponentsInChildren<Renderer>(true)){
            var values=new JArray(r.sharedMaterials.Select((m,i)=>new JObject{["slot"]=i,["missingMaterial"]=!m,["material"]=m?m.name:null,["path"]=m?AssetDatabase.GetAssetPath(m):null,["shader"]=m&&m.shader?m.shader.name:null}));
            if(values.OfType<JObject>().Any(v=>(bool)v["missingMaterial"]||v["shader"].Type==JTokenType.Null)){
                string path=r.name;for(var t=r.transform.parent;t;t=t.parent)path=t.name+"/"+path;
                rows.Add(new JObject{["path"]=path,["type"]=r.GetType().Name,["rendererEnabled"]=r.enabled,["activeSelf"]=r.gameObject.activeSelf,["materials"]=values});
            }
        }
        File.WriteAllText(Reports+"/renderer-diagnostics.json",rows.ToString());return rows.ToString();
    }
    public static string VerifyDependencyClosure(){
        if(!Application.dataPath.Replace('\\','/').EndsWith("/avatar-plugin/CleanProject/Assets",StringComparison.OrdinalIgnoreCase))throw new Exception("Dedicated CleanProject required");
        var manifest=JArray.Parse(File.ReadAllText(Reports+"/package-files.json")).OfType<JObject>().ToArray();
        var dependencies=manifest.SelectMany(m=>AssetDatabase.GetDependencies((string)m["path"],true)).Distinct().ToArray();
        var external=dependencies.Where(p=>p.StartsWith("Assets/")&&!p.StartsWith(AvatarSetupWindow.Root+"/")).ToArray();
        var detail=new JObject{["dependencyCount"]=dependencies.Length,["externalProjectDependencies"]=new JArray(external)};
        File.WriteAllText(Reports+"/dependency-closure.json",detail.ToString());
        if(external.Length>0)throw new Exception("Package depends on project assets outside plugin: "+string.Join(", ",external));
        var result=JObject.Parse(File.ReadAllText(Reports+"/acceptance.json"));
        if(!(bool)result["success"])throw new Exception("Successful installation and SDK checks required");
        var checks=(JArray)result["checks"];string name="all package dependencies stay inside plugin or separately installed packages";
        if(!checks.OfType<JObject>().Any(c=>(string)c["name"]==name))checks.Add(new JObject{["name"]=name,["passed"]=true});
        result["checkCount"]=checks.Count;result["dependencyClosure"]=detail;
        File.WriteAllText(Reports+"/acceptance.json",result.ToString());File.WriteAllText(Reports+"/progress.json",checks.ToString());
        return new JObject{["success"]=true,["checkCount"]=checks.Count,["dependencyClosure"]=detail}.ToString();
    }
    public static string Run(){
        if(!Application.dataPath.Replace('\\','/').EndsWith("/avatar-plugin/CleanProject/Assets",StringComparison.OrdinalIgnoreCase)||EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Dedicated ready CleanProject required");
        if(Enumerable.Range(0,SceneManager.sceneCount).Select(SceneManager.GetSceneAt).Any(s=>s.isDirty))throw new Exception("Unsaved scene before acceptance");
        Directory.CreateDirectory(Reports);Checks.Clear();Builds.Clear();var manifest=JArray.Parse(File.ReadAllText(Reports+"/package-files.json")).OfType<JObject>().ToArray();
        Check("original buyer package import recorded",JObject.Parse(File.ReadAllText(Reports+"/import-request.json"))["packageSha256"].ToString()=="7e60412f16cbc3e9858e7c85c7d02ec9c16d15af94f6c37805190b59c5a39264");
        Check("old plugin absent before import",(bool)JObject.Parse(File.ReadAllText(Reports+"/removed.json"))["pluginFolderAbsent"]);
        var actual=Directory.GetFiles(AvatarSetupWindow.Root,"*",SearchOption.AllDirectories).Where(f=>!f.EndsWith(".meta")).Select(f=>f.Replace('\\','/')).OrderBy(f=>f).ToArray();
        Check("imported assets exactly match original 162-file inventory",actual.SequenceEqual(manifest.Select(x=>(string)x["path"]).OrderBy(x=>x)));
        Check("all 162 imported files and GUIDs match buyer package",manifest.Length==162&&manifest.All(MatchFile));
        var prefab=AssetDatabase.LoadAssetAtPath<GameObject>(AvatarSetupWindow.PrefabPath);
        Check("public prefab is version 1.4.0",prefab&&prefab.GetComponent<MorphForgeAvatarModule>().version=="1.4.0");
        Check("prefab has no missing scripts",prefab.GetComponentsInChildren<Transform>(true).All(t=>GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(t.gameObject)==0));
        var renderers=prefab.GetComponentsInChildren<Renderer>(true);
        Check("16 emission-only pumps have rendering disabled and valid sub-emitters",renderers.Count(EmissionOnly)==16);
        Check("all display renderer materials and shaders resolve",renderers.Where(r=>!EmissionOnly(r)).All(r=>r.sharedMaterials.Length>0&&r.sharedMaterials.All(m=>m&&m.shader)));
        var shaders=AssetDatabase.FindAssets("t:Shader",new[]{AvatarSetupWindow.Root}).Select(g=>AssetDatabase.LoadAssetAtPath<Shader>(AssetDatabase.GUIDToAssetPath(g))).ToArray();
        Check("imported shaders have no compiler errors",shaders.Length>0&&shaders.All(s=>s&&ShaderUtil.GetShaderMessages(s).All(m=>m.severity.ToString()!="Error")));
        Check("required Modular Avatar dependency resolves",AvatarSetupWindow.HasMA);Check("optional VRCFury fixture dependency resolves",AvatarSetupWindow.HasFury);
        var catalog=JObject.Parse(File.ReadAllText(AvatarSetupWindow.Root+"/CatalogSource/catalog.json"));Check("public catalog has 544 materials",((JArray)catalog["materials"]).Count==544);
        CatalogVariant.EnsureFolder(Folder);EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);
        try{
            var modules=new List<GameObject>();
            foreach(string language in new[]{"zh-CN","ja","en"}){
                var avatar=Rig(language,0);var module=AvatarSetupWindow.Install(avatar.GetComponent<VRCAvatarDescriptor>(),false,language:language);modules.Add(module);
                var menus=Menus(module);var marker=module.GetComponent<MorphForgeAvatarModule>();
                Check(language+" installation records language",marker.menuLanguage==language);
                Check(language+" all menus are independent persistent assets",menus.Count>0&&menus.All(m=>AssetDatabase.GetAssetPath(m).StartsWith(MenuLocalization.Root+"/"+marker.menuInstanceId+"/"+language+"/")));
                Check(language+" localized owner-preview label",menus.SelectMany(m=>m.controls).Any(c=>c.name==PluginLocalization.Translate("本人预览",language)));
                Check(language+" menu capacity",menus.All(m=>m.controls.Count<=8));
                bool duplicate=false;try{AvatarSetupWindow.Install(avatar.GetComponent<VRCAvatarDescriptor>(),false);}catch(InvalidOperationException){duplicate=true;}Check(language+" duplicate installation rejected",duplicate);
            }
            Check("three avatars never share localized menu assets",!Menus(modules[0]).Intersect(Menus(modules[1])).Any()&&!Menus(modules[0]).Intersect(Menus(modules[2])).Any()&&!Menus(modules[1]).Intersect(Menus(modules[2])).Any());
            var other=string.Join("|",Menus(modules[1]).SelectMany(m=>m.controls).Select(c=>c.name));
            var custom=modules[0].GetComponent<ModularAvatarMenuInstaller>().menuToAppend;custom.controls.Add(new VRCExpressionsMenu.Control{name="User custom / 自定义",type=VRCExpressionsMenu.Control.ControlType.Toggle,parameter=new VRCExpressionsMenu.Control.Parameter{name="MF_World"},value=1});EditorUtility.SetDirty(custom);
            foreach(string language in new[]{"en","ja","zh-CN"})MenuLocalization.Apply(modules[0],language);
            Check("language switching preserves custom control",Menus(modules[0]).SelectMany(m=>m.controls).Any(c=>c.name=="User custom / 自定义"));
            Check("language switching leaves other avatar unchanged",other==string.Join("|",Menus(modules[1]).SelectMany(m=>m.controls).Select(c=>c.name)));
            var fury=Rig("fury-ja",8);AvatarSetupWindow.Install(fury.GetComponent<VRCAvatarDescriptor>(),true,language:"ja");FuryFixture(fury);
            EditorSceneManager.SaveScene(SceneManager.GetActiveScene(),Folder+"/Installed.unity");EditorSceneManager.OpenScene(Folder+"/Installed.unity",OpenSceneMode.Single);
            Check("language persists after scene reload",UnityEngine.Object.FindObjectsOfType<MorphForgeAvatarModule>().Select(m=>m.menuLanguage).OrderBy(x=>x).SequenceEqual(new[]{"zh-CN","ja","en","ja"}.OrderBy(x=>x)));
            foreach(var avatar in UnityEngine.Object.FindObjectsOfType<VRCAvatarDescriptor>())Build(avatar.gameObject,avatar.gameObject.name,avatar.gameObject.name.StartsWith("fury")?8:0);
            EditorSceneManager.SaveScene(SceneManager.GetActiveScene(),Folder+"/AfterSdk.unity",true);
            NegativeBudgetChecks();
            Check("all original public assets remain unchanged after install and SDK",manifest.All(MatchFile));
        }finally{RestoreOriginal();}
        var result=new JObject{["success"]=true,["unityVersion"]=Application.unityVersion,["packageSha256"]="7e60412f16cbc3e9858e7c85c7d02ec9c16d15af94f6c37805190b59c5a39264",["importedFileCount"]=manifest.Length,["checks"]=Checks,["checkCount"]=Checks.Count,["builds"]=Builds,["fixtureScene"]=Folder+"/Installed.unity",["scope"]="Original buyer package imported after AssetDatabase removal of previous plugin; synthetic humanoids, three languages, actual SDK preprocessing with MA/NDMF and optional VRCFury. No upload or real VRChat session."};
        File.WriteAllText(Reports+"/acceptance.json",result.ToString());
        VerifyDependencyClosure();
        return new JObject{["success"]=true,["checkCount"]=JObject.Parse(File.ReadAllText(Reports+"/acceptance.json"))["checkCount"],["builds"]=Builds}.ToString();
    }
}
