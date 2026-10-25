using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Reflection;
using System.Threading.Tasks;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.Rendering;
using UnityEngine.SceneManagement;
using VRC.Core;
using VRC.SDK3.Avatars.Components;
using VRC.SDK3.Avatars.ScriptableObjects;
using VRC.SDKBase.Editor.BuildPipeline;
using MorphForgeAvatar;
using MorphForgeAvatar.Editor;
using nadena.dev.modular_avatar.core;
using Object=UnityEngine.Object;
using Activator=System.Activator;

public static class HedgehogTrialBuilder {
    public const string Project="D:/Projects/VRChatProjects2/MorphForgeHedgehogDemo_1_4_0";
    public const string Folder="Assets/MorphForgeHedgehogTrial";
    public const string ScenePath=Folder+"/MorphForgeHedgehogTrial.unity";
    public const string PrefabPath=Folder+"/MorphForgeHedgehogTrial.prefab";
    public const string Source="Assets/Saltroche/256Hedgehog/256Hedgehog.prefab";
    public const string GuideParam="MFT_Guide";
    static string ReportPath=>"TrialReview/validation.json";
    static TaskCompletionSource<string> buildCompletion;
    static void Guard(){
        if(!Application.dataPath.Replace('\\','/').Equals(Project+"/Assets",StringComparison.OrdinalIgnoreCase))throw new Exception("Dedicated MorphForge trial project required");
        if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Ready Edit Mode required");
    }
    static JObject State()=>File.Exists(ReportPath)?JObject.Parse(File.ReadAllText(ReportPath)):new JObject();
    static void Save(JObject state){Directory.CreateDirectory("TrialReview");state["updatedUtc"]=DateTime.UtcNow.ToString("O");File.WriteAllText(ReportPath,state.ToString());}
    static void Phase(string phase){var s=State();s["phase"]=phase;Save(s);}
    static GameObject Target(){
        Guard();if(SceneManager.GetActiveScene().path!=ScenePath)throw new Exception("Open the trial scene");
        return SceneManager.GetActiveScene().GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single().gameObject;
    }
    static void Check(JObject state,string name,bool pass){
        ((JArray)state["checks"]).Add(new JObject{["name"]=name,["passed"]=pass});Save(state);
        if(!pass)throw new Exception(name);
    }
    static VRCExpressionsMenu CloneMenu(VRCExpressionsMenu source,string name,Dictionary<VRCExpressionsMenu,VRCExpressionsMenu> map=null){
        if(!source)return null;map=map??new Dictionary<VRCExpressionsMenu,VRCExpressionsMenu>();if(map.TryGetValue(source,out var found))return found;
        var copy=Object.Instantiate(source);map[source]=copy;AssetDatabase.CreateAsset(copy,AssetDatabase.GenerateUniqueAssetPath(Folder+"/"+name+".asset"));
        foreach(var c in copy.controls)if(c.subMenu)c.subMenu=CloneMenu(c.subMenu,name+"-Sub",map);EditorUtility.SetDirty(copy);return copy;
    }
    static void PreserveWriteDefaults(GameObject avatar){
        var feature=AvatarSetupWindow.FindType("VF.Model.Feature.FixWriteDefaults");
        if(feature==null)throw new Exception("VRCFury fixture dependency missing");
        var value=Activator.CreateInstance(feature,true);var mode=feature.GetField("mode");mode.SetValue(value,Enum.Parse(mode.FieldType,"Disabled"));
        var component=avatar.AddComponent(AvatarSetupWindow.FindType("VF.Model.VRCFury"));component.GetType().GetField("content").SetValue(component,value);
    }
    static void Guide(GameObject avatar,VRCAvatarDescriptor descriptor){
        CatalogVariant.EnsureFolder(Folder+"/Guide");
        string texturePath=Folder+"/Guide/Instructions.png";File.Copy("SourcePackages/Guide.ja.png",texturePath,true);AssetDatabase.ImportAsset(texturePath,ImportAssetOptions.ForceSynchronousImport);
        var importer=(TextureImporter)AssetImporter.GetAtPath(texturePath);importer.alphaIsTransparency=true;importer.mipmapEnabled=false;importer.maxTextureSize=1024;importer.SaveAndReimport();
        string shaderPath=Folder+"/Guide/Guide.shader";
        File.WriteAllText(shaderPath,@"Shader ""MorphForge/TrialGuide"" { Properties { _MainTex(""Instructions"",2D)=""white""{} } SubShader { Tags { ""Queue""=""Transparent"" ""RenderType""=""Transparent"" } Cull Off ZWrite Off Blend SrcAlpha OneMinusSrcAlpha Pass { CGPROGRAM
#pragma vertex vert
#pragma fragment frag
#pragma target 3.0
#include ""UnityCG.cginc""
sampler2D _MainTex; struct v2f { float4 pos:SV_POSITION; float2 uv:TEXCOORD0; }; v2f vert(appdata_base v){v2f o;o.pos=UnityObjectToClipPos(v.vertex);o.uv=v.texcoord;return o;} fixed4 frag(v2f i,fixed facing:VFACE):SV_Target{if(facing<0)i.uv.x=1-i.uv.x;return tex2D(_MainTex,i.uv);}
ENDCG } } }");
        AssetDatabase.ImportAsset(shaderPath,ImportAssetOptions.ForceSynchronousImport);var material=new Material(AssetDatabase.LoadAssetAtPath<Shader>(shaderPath));material.mainTexture=AssetDatabase.LoadAssetAtPath<Texture2D>(texturePath);AssetDatabase.CreateAsset(material,Folder+"/Guide/Guide.mat");
        var root=new GameObject("TrialGuide");root.transform.SetParent(avatar.transform,false);var panel=GameObject.CreatePrimitive(PrimitiveType.Quad);panel.name="Panel";panel.transform.SetParent(root.transform,false);Object.DestroyImmediate(panel.GetComponent<Collider>());
        var body=avatar.GetComponentsInChildren<SkinnedMeshRenderer>(true);var bounds=body[0].bounds;foreach(var r in body.Skip(1))bounds.Encapsulate(r.bounds);float h=Mathf.Max(.3f,bounds.size.y);
        panel.transform.localPosition=new Vector3(h*1.1f,h*.8f,h*.7f);panel.transform.localRotation=Quaternion.Euler(0,180,0);panel.transform.localScale=Vector3.one*h*1.35f;
        var renderer=panel.GetComponent<MeshRenderer>();renderer.sharedMaterial=material;renderer.shadowCastingMode=ShadowCastingMode.Off;renderer.receiveShadows=false;
        var fx=AnimatorController.CreateAnimatorControllerAtPath(Folder+"/Guide/Guide.controller");fx.AddParameter(new AnimatorControllerParameter{name=GuideParam,type=AnimatorControllerParameterType.Bool,defaultBool=true});
        var states=new List<AnimatorState>();foreach(bool show in new[]{true,false}){
            var clip=new AnimationClip{name=show?"Show":"Hide"};AnimationUtility.SetEditorCurve(clip,EditorCurveBinding.FloatCurve("Panel",typeof(GameObject),"m_IsActive"),AnimationCurve.Constant(0,1f/60,show?1:0));AssetDatabase.CreateAsset(clip,Folder+"/Guide/"+clip.name+".anim");
            var state=fx.layers[0].stateMachine.AddState(clip.name);state.motion=clip;state.writeDefaultValues=false;states.Add(state);
        }
        fx.layers[0].stateMachine.defaultState=states[0];for(int i=0;i<2;i++){var t=states[i].AddTransition(states[1-i]);t.hasExitTime=false;t.duration=0;t.AddCondition(i==0?AnimatorConditionMode.IfNot:AnimatorConditionMode.If,0,GuideParam);}
        var merge=root.AddComponent<ModularAvatarMergeAnimator>();merge.animator=fx;merge.layerType=VRCAvatarDescriptor.AnimLayerType.FX;merge.pathMode=MergeAnimatorPathMode.Relative;
        var parameters=descriptor.expressionParameters.parameters.ToList();parameters.Add(new VRCExpressionParameters.Parameter{name=GuideParam,valueType=VRCExpressionParameters.ValueType.Bool,networkSynced=false,saved=true,defaultValue=1});descriptor.expressionParameters.parameters=parameters.ToArray();EditorUtility.SetDirty(descriptor.expressionParameters);
        descriptor.expressionsMenu.controls.Add(new VRCExpressionsMenu.Control{name="ガイド表示 / Guide",type=VRCExpressionsMenu.Control.ControlType.Toggle,parameter=new VRCExpressionsMenu.Control.Parameter{name=GuideParam},value=1});EditorUtility.SetDirty(descriptor.expressionsMenu);
    }
    static void Environment(){
        RenderSettings.ambientMode=AmbientMode.Flat;RenderSettings.ambientLight=new Color(.6f,.65f,.72f);
        foreach(var x in new[]{(new Vector3(40,-35,0),1.3f),(new Vector3(30,145,0),.8f)}){var light=new GameObject("Trial Directional Light").AddComponent<Light>();light.type=LightType.Directional;light.intensity=x.Item2;light.transform.rotation=Quaternion.Euler(x.Item1);}
        var camera=new GameObject("Trial Camera").AddComponent<Camera>();camera.tag="MainCamera";camera.fieldOfView=35;camera.nearClipPlane=.01f;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=new Color(.045f,.075f,.11f);
    }
    public static string Prepare(){
        Guard();if(Enumerable.Range(0,SceneManager.sceneCount).Select(SceneManager.GetSceneAt).Any(s=>s.isDirty))throw new Exception("Unsaved user scene preserved");
        if(File.Exists(ScenePath)){EditorSceneManager.OpenScene(ScenePath);return "Existing trial preserved; use validation/status instead of reinstalling";}
        if(!File.Exists("SourcePackages/Guide.ja.png"))throw new Exception("Instruction texture missing");
        EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);CatalogVariant.EnsureFolder(Folder);PlayerSettings.colorSpace=ColorSpace.Linear;
        var source=AssetDatabase.LoadAssetAtPath<GameObject>(Source);if(!source)throw new Exception("Author prefab not imported");
        var avatar=(GameObject)PrefabUtility.InstantiatePrefab(source);avatar.name="MorphForge 1.4.0 - 256Hedgehog Trial";
        var d=avatar.GetComponent<VRCAvatarDescriptor>();var pipeline=avatar.GetComponent<PipelineManager>();var animator=avatar.GetComponent<Animator>();
        if(!d||!pipeline||!animator||!animator.avatar||!animator.avatar.isHuman||!animator.avatar.isValid)throw new Exception("Original humanoid or SDK descriptor invalid");
        if(!string.IsNullOrEmpty(pipeline.blueprintId))throw new Exception("Author source has an existing upload ID; preserve it");
        int originalBits=d.expressionParameters.CalcTotalCost();var parameters=Object.Instantiate(d.expressionParameters);AssetDatabase.CreateAsset(parameters,Folder+"/Parameters.asset");d.expressionParameters=parameters;d.expressionsMenu=CloneMenu(d.expressionsMenu,"RootMenu");
        PluginLocalization.SelectedLanguage="ja";var module=AvatarSetupWindow.Install(d,true,language:"ja");PreserveWriteDefaults(avatar);Guide(avatar,d);Environment();
        PrefabUtility.SaveAsPrefabAssetAndConnect(avatar,PrefabPath,InteractionMode.AutomatedAction);EditorSceneManager.SaveScene(avatar.scene,ScenePath);AssetDatabase.SaveAssets();Selection.activeGameObject=avatar;AvatarSetupWindow.ShowWindow();
        var state=new JObject{["status"]="prepared",["phase"]="scene-created",["name"]=avatar.name,["scene"]=ScenePath,["prefab"]=PrefabPath,["menuLanguage"]="ja",["baselineBits"]=originalBits,["parameterProfile"]="original",["sourceBodyHumanScale"]=animator.humanScale,["viewHeight"]=d.ViewPosition.y,["avatarIdAssigned"]=false,["checks"]=new JArray()};Save(state);
        File.WriteAllText("TrialReview/upload-metadata.json",new JObject{["name"]=avatar.name,["description"]="MorphForge 1.4.0 PC/PCVR trial. Requires MorphForge Desktop 0.27.0+ and OSC: connect, detect avatar, publish a work. Japanese menus; installer supports Chinese/Japanese/English. Very Poor rank expected. Base: 256Hedgehog 1.2.0 by messier_num / Saltroche, CC0. Source: https://booth.pm/en/items/2064264",["intendedReleaseStatus"]="public",["uploaded"]=false}.ToString());
        return state.ToString();
    }
    static JObject Preprocess(GameObject source){
        var clone=Object.Instantiate(source);clone.name=source.name+"-SDK-probe";
        try{
            foreach(var instance in clone.GetComponentsInChildren<Transform>(true).Select(t=>t.gameObject).Where(PrefabUtility.IsAnyPrefabInstanceRoot).ToArray())if(PrefabUtility.IsPartOfPrefabInstance(instance))PrefabUtility.UnpackPrefabInstance(instance,PrefabUnpackMode.Completely,InteractionMode.AutomatedAction);
            bool pass=true;string error=null;try{foreach(var c in TypeCache.GetTypesDerivedFrom<IVRCSDKPreprocessAvatarCallback>().Where(t=>!t.IsAbstract&&t.GetConstructor(Type.EmptyTypes)!=null).Select(t=>(IVRCSDKPreprocessAvatarCallback)Activator.CreateInstance(t)).OrderBy(c=>c.callbackOrder))if(!c.OnPreprocessAvatar(clone)){pass=false;error=c.GetType().FullName;break;}}catch(Exception e){pass=false;error=e.GetBaseException().Message;}
            var d=clone.GetComponent<VRCAvatarDescriptor>();var values=d.expressionParameters?.parameters??Array.Empty<VRCExpressionParameters.Parameter>();int bits=d.expressionParameters?d.expressionParameters.CalcTotalCost():-1;
            bool direct=PluginBudgetGate.Geometry.All(n=>values.Count(p=>p.name==n&&p.networkSynced&&p.valueType==VRCExpressionParameters.ValueType.Int)==1);
            return new JObject{["success"]=pass&&bits<=256&&direct,["callbackSuccess"]=pass,["error"]=error,["bits"]=bits,["geometryDirect"]=direct,["parameters"]=new JArray(values.Select(p=>new JObject{["name"]=p.name,["networkSynced"]=p.networkSynced,["type"]=p.valueType.ToString()}))};
        }finally{Object.DestroyImmediate(clone);}
    }
    static void SimplifyLegacyMenus(GameObject avatar,JObject state){
        var d=avatar.GetComponent<VRCAvatarDescriptor>();
        string[] allowed={"Color",GuideParam};state["removedLegacyParameters"]=new JArray(d.expressionParameters.parameters.Where(p=>!allowed.Contains(p.name)).Select(p=>p.name));
        d.expressionParameters.parameters=d.expressionParameters.parameters.Where(p=>allowed.Contains(p.name)).ToArray();
        d.expressionsMenu.controls=d.expressionsMenu.controls.Where(c=>c.parameter!=null&&allowed.Contains(c.parameter.name)).ToList();
        EditorUtility.SetDirty(d.expressionParameters);EditorUtility.SetDirty(d.expressionsMenu);AssetDatabase.SaveAssets();
        state["parameterProfile"]="color-and-native-gestures";state["legacyMenuNote"]="Trial preserves Color, native hand-gesture facial expressions and SDK visemes. Legacy Emote menu and unused legacy face parameters omitted to preserve the 240-bit direct geometry channel.";Save(state);
        PrefabUtility.ApplyPrefabInstance(avatar,InteractionMode.AutomatedAction);EditorSceneManager.SaveScene(avatar.scene,ScenePath);
    }
    static void Capture(GameObject avatar){
        var camera=Camera.main;if(!camera)throw new Exception("Trial camera missing");var guide=avatar.transform.Find("TrialGuide");
        var rt=new RenderTexture(1200,1200,24);camera.targetTexture=rt;
        try{foreach(bool showGuide in new[]{false,true}){
            guide.gameObject.SetActive(showGuide);
            var visible=avatar.GetComponentsInChildren<Renderer>(false).Where(r=>r.enabled&&(r is SkinnedMeshRenderer||r.transform.IsChildOf(guide))).ToArray();
            var selected=showGuide?visible:avatar.GetComponentsInChildren<SkinnedMeshRenderer>(false).Cast<Renderer>().ToArray();
            var bounds=selected[0].bounds;foreach(var r in selected.Skip(1))bounds.Encapsulate(r.bounds);float span=Mathf.Max(bounds.size.x,bounds.size.y);
            camera.transform.position=bounds.center+new Vector3(-.38f,.2f,1).normalized*span*2.0f;camera.transform.LookAt(bounds.center);camera.Render();
            var previous=RenderTexture.active;RenderTexture.active=rt;var image=new Texture2D(rt.width,rt.height,TextureFormat.RGB24,false);image.ReadPixels(new Rect(0,0,rt.width,rt.height),0,0);image.Apply();File.WriteAllBytes("TrialReview/"+(showGuide?"hedgehog-guide":"hedgehog-thumbnail")+".png",image.EncodeToPNG());Object.DestroyImmediate(image);RenderTexture.active=previous;
        }}finally{guide.gameObject.SetActive(true);camera.targetTexture=null;Object.DestroyImmediate(rt);EditorSceneManager.SaveScene(avatar.scene,ScenePath);}
    }
    public static string BeginValidation(){
        var avatar=Target();if(avatar.scene.isDirty)throw new Exception("Saved trial scene required");var state=State();if((string)state["status"]=="running")return state.ToString();
        state["status"]="running";state["phase"]="queued";state["checks"]=new JArray();Save(state);
        EditorApplication.CallbackFunction next=null;next=()=>{EditorApplication.update-=next;Validate();};EditorApplication.update+=next;EditorApplication.QueuePlayerLoopUpdate();return state.ToString();
    }
    static async void Validate(){
        GameObject buildTarget=null;
        try{
            var avatar=Target();Phase("original-parameter-preprocess");var first=Preprocess(avatar);var state=State();state["originalProfileProbe"]=first;Save(state);
            if(!(bool)first["success"]){
                string error=(string)first["error"]??"";
                bool budgetFailure=new[]{"parameter","budget","予算","パラメーター","预算","参数","out of space"}.Any(word=>error.IndexOf(word,StringComparison.OrdinalIgnoreCase)>=0)||((bool)first["callbackSuccess"]&&(int)first["bits"]>256);
                if(!budgetFailure)throw new Exception("Original trial failed for a reason other than parameter budget: "+first["error"]);
                SimplifyLegacyMenus(avatar,state);Phase("trial-parameter-preprocess");var second=Preprocess(avatar);state=State();state["finalProfileProbe"]=second;Save(state);if(!(bool)second["success"])throw new Exception("Simplified trial failed SDK preprocessing: "+second["error"]);
            }else{state["finalProfileProbe"]=first;Save(state);}
            state=State();var final=state["finalProfileProbe"];Check(state,"SDK preprocessing and parameter budget passed",(bool)final["success"]);Check(state,"240-bit geometry remains directly synced",(bool)final["geometryDirect"]);
            var d=avatar.GetComponent<VRCAvatarDescriptor>();Check(state,"source avatar remains unassigned to any online ID",string.IsNullOrEmpty(avatar.GetComponent<PipelineManager>().blueprintId));Check(state,"original humanoid rig remains valid",avatar.GetComponent<Animator>().avatar.isHuman);Check(state,"source avatar has no missing scripts",avatar.GetComponentsInChildren<Transform>(true).All(t=>GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(t.gameObject)==0));Check(state,"installed module uses Japanese menus",avatar.GetComponentInChildren<MorphForgeAvatarModule>(true).menuLanguage=="ja");
            Capture(avatar);Phase("native-sdk-local-build");string hash=AssetDatabase.GetAssetDependencyHash(PrefabPath).ToString();
            buildTarget=Object.Instantiate(avatar);buildTarget.name=avatar.name+"-local-build";buildTarget.GetComponent<PipelineManager>().blueprintId="avtr_"+Guid.NewGuid().ToString();
            string bundle=await Export(buildTarget);Object.DestroyImmediate(buildTarget);buildTarget=null;
            state=State();state["bundleBytes"]=new FileInfo(bundle).Length;Check(state,"native SDK produced a local avatar bundle",File.Exists(bundle)&&new FileInfo(bundle).Length>0);Check(state,"saved trial prefab unchanged by local SDK build",AssetDatabase.GetAssetDependencyHash(PrefabPath).ToString()==hash);
            state["status"]="passed";state["phase"]="ready-for-upload-review";state["uploaded"]=false;state["bundleFile"]=Path.GetFileName(bundle);Save(state);
            EditorSceneManager.SaveScene(avatar.scene,ScenePath);Selection.activeGameObject=avatar;SealReview();
        }catch(Exception e){var s=State();s["status"]="failed";s["error"]=e.ToString();Save(s);Debug.LogError("MorphForge trial validation failed: "+e.Message);}
        finally{if(buildTarget)Object.DestroyImmediate(buildTarget);}
    }
    static async Task<string> Export(GameObject target){
        var type=AppDomain.CurrentDomain.GetAssemblies().Select(a=>a.GetType("VRC.SDK3.Builder.VRCAvatarBuilder")).First(t=>t!=null);var builder=Activator.CreateInstance(type);buildCompletion=new TaskCompletionSource<string>();
        foreach(var pair in new[]{("OnBuildSuccess","NativeSuccess"),("OnBuildError","NativeError")}){var e=type.GetEvent(pair.Item1);e.AddEventHandler(builder,Delegate.CreateDelegate(e.EventHandlerType,typeof(HedgehogTrialBuilder).GetMethod(pair.Item2,BindingFlags.NonPublic|BindingFlags.Static)));}
        try{
            var accepted=type.GetMethod("ExportAvatarBlueprint").Invoke(builder,new object[]{target});if(accepted is bool b&&!b)throw new Exception("SDK refused local trial build");
            if(await Task.WhenAny(buildCompletion.Task,Task.Delay(TimeSpan.FromMinutes(8)))!=buildCompletion.Task)throw new TimeoutException("SDK local trial build timed out");
            string path=await buildCompletion.Task,destination=Path.GetFullPath("TrialReview/MorphForge-Hedgehog-Trial-local.vrca");File.Copy(path,destination,true);return destination;
        }finally{type.GetMethod("ClearEventHandlers").Invoke(builder,null);}
    }
    static void NativeSuccess(object sender,(string,string) payload){var file=new[]{payload.Item1,payload.Item2}.FirstOrDefault(p=>!string.IsNullOrEmpty(p)&&File.Exists(p));if(file==null)buildCompletion.TrySetException(new Exception("SDK returned no local bundle"));else buildCompletion.TrySetResult(file);}
    static void NativeError(object sender,string error)=>buildCompletion.TrySetException(new Exception(error));
    public static string SealReview(){
        var avatar=Target();var state=State();if((string)state["status"]!="passed"||avatar.scene.isDirty)throw new Exception("Completed native validation and a saved scene required");
        if(!string.IsNullOrEmpty(avatar.GetComponent<PipelineManager>().blueprintId))throw new Exception("Trial already has an online ID; preserve existing upload state");
        state["validatedPrefabHash"]=AssetDatabase.GetAssetDependencyHash(PrefabPath).ToString();state["validatedSceneHash"]=AssetDatabase.GetAssetDependencyHash(ScenePath).ToString();
        state["menuLanguage"]=avatar.GetComponentInChildren<MorphForgeAvatarModule>(true).menuLanguage;
        var metadata=JObject.Parse(File.ReadAllText("TrialReview/upload-metadata.json"));
        if((string)state["parameterProfile"]=="color-and-native-gestures"&&!metadata["description"].ToString().Contains("Legacy Emote menu omitted"))metadata["description"]=metadata["description"].ToString()+". Legacy Emote menu omitted; palette, native visemes and hand gestures retained.";
        File.WriteAllText("TrialReview/upload-metadata.json",metadata.ToString());Save(state);return state.ToString();
    }
    public static string InspectReview(){
        var avatar=Target();var state=State();var descriptor=avatar.GetComponent<VRCAvatarDescriptor>();
        var author=AssetDatabase.LoadAssetAtPath<GameObject>(Source);var module=avatar.GetComponentInChildren<MorphForgeAvatarModule>(true);
        var renderers=avatar.GetComponentsInChildren<Renderer>(true);var displayRenderers=renderers.Where(r=>!EmissionOnly(r)).ToArray();
        var materials=displayRenderers.SelectMany(r=>r.sharedMaterials).Distinct().ToArray();
        var menus=new HashSet<VRCExpressionsMenu>();var labels=new List<string>();
        Action<VRCExpressionsMenu> visit=null;visit=menu=>{if(!menu||!menus.Add(menu))return;foreach(var c in menu.controls){labels.Add(c.name);visit(c.subMenu);}};
        foreach(var installer in module.GetComponentsInChildren<ModularAvatarMenuInstaller>(true))visit(installer.menuToAppend);
        var guide=descriptor.expressionParameters.parameters.Single(p=>p.name==GuideParam);
        var checks=new JObject{
            ["nativeSdkValidationPassed"]=(string)state["status"]=="passed",
            ["savedPrefabMatchesReview"]=(string)state["validatedPrefabHash"]==AssetDatabase.GetAssetDependencyHash(PrefabPath).ToString(),
            ["savedSceneMatchesReview"]=!avatar.scene.isDirty&&(string)state["validatedSceneHash"]==AssetDatabase.GetAssetDependencyHash(ScenePath).ToString(),
            ["authorSourceHasNoOnlineId"]=string.IsNullOrEmpty(author.GetComponent<PipelineManager>().blueprintId),
            ["trialHasNoOnlineId"]=string.IsNullOrEmpty(avatar.GetComponent<PipelineManager>().blueprintId),
            ["threeInstallerLanguagesAvailable"]=PluginLocalization.Codes.SequenceEqual(new[]{"zh-CN","ja","en"}),
            ["installerAndInstalledMenusUseJapanese"]=PluginLocalization.SelectedLanguage=="ja"&&module.menuLanguage=="ja",
            ["actualModuleMenusContainJapanesePlacementAndPreview"]=labels.Contains("ワールドに固定")&&labels.Contains("本人プレビュー"),
            ["setupMenusFitEightControls"]=menus.All(m=>m.controls.Count<=8),
            ["fourOriginalPaletteControlsRetained"]=descriptor.expressionsMenu.controls.Count(c=>c.parameter!=null&&c.parameter.name=="Color")==4,
            ["guideUsesLocalBool"]=guide.valueType==VRCExpressionParameters.ValueType.Bool&&!guide.networkSynced,
            ["sixteenEmissionOnlyPumpsHaveRenderingDisabled"]=renderers.Count(EmissionOnly)==16,
            ["allDisplayMaterialsAndShadersResolve"]=displayRenderers.All(r=>r.sharedMaterials.Length>0)&&materials.All(m=>m&&m.shader&&m.shader.isSupported&&m.shader.name!="Hidden/InternalErrorShader"),
            ["nativeAnimationControllersRetained"]=descriptor.baseAnimationLayers.Select(l=>l.animatorController).SequenceEqual(author.GetComponent<VRCAvatarDescriptor>().baseAnimationLayers.Select(l=>l.animatorController)),
            ["nativeLipSyncRetained"]=descriptor.lipSync==author.GetComponent<VRCAvatarDescriptor>().lipSync
        };
        var report=new JObject{["checkedUtc"]=DateTime.UtcNow.ToString("O"),["allPassed"]=checks.Properties().All(p=>(bool)p.Value),["checks"]=checks,["installedModuleMenuLabels"]=new JArray(labels),["materialCount"]=materials.Length,["materials"]=new JArray(materials.Select(m=>m?new JObject{["name"]=m.name,["asset"]=AssetDatabase.GetAssetPath(m),["shader"]=m.shader?m.shader.name:null,["supported"]=m.shader&&m.shader.isSupported}:null)),["unresolvedDisplayRendererSlots"]=new JArray(displayRenderers.Where(r=>r.sharedMaterials.Any(m=>!m||!m.shader||m.shader.name=="Hidden/InternalErrorShader")).Select(r=>new JObject{["path"]=AnimationUtility.CalculateTransformPath(r.transform,avatar.transform),["type"]=r.GetType().Name,["enabled"]=r.enabled,["active"]=r.gameObject.activeInHierarchy,["slots"]=new JArray(r.sharedMaterials.Select(m=>m?m.name:null))})),["uploaded"]=false};
        File.WriteAllText("TrialReview/readiness.json",report.ToString());return report.ToString();
    }
    static bool EmissionOnly(Renderer renderer){
        if(!(renderer is ParticleSystemRenderer)||renderer.enabled||(renderer.name!="Pump"&&renderer.name!="BoundsPump"))return false;
        var system=renderer.GetComponent<ParticleSystem>();if(!system)return false;var sub=system.subEmitters;
        return sub.enabled&&sub.subEmittersCount>0&&Enumerable.Range(0,sub.subEmittersCount).All(i=>sub.GetSubEmitterSystem(i));
    }
    public static string Status()=>State().ToString();
}
