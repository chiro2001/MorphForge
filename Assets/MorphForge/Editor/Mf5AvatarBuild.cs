using System;
using System.IO;
using System.Linq;
using MorphForge;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase.Editor.BuildPipeline;
using BlackStartX.GestureManager;
using nadena.dev.modular_avatar.core;

namespace MorphForge.Editor
{
    public static class Mf5AvatarBuild
    {
        public const string Root="Assets/MorphForge/MF5";
        public const string AvatarPath=Root+"/Avatar.prefab";
        public const string ScenePath=Root+"/MorphForge_MF5.unity";
        public const string AvatarId="avtr_df2ff222-b6ff-47fb-80be-e060868596a2";
        static string Hash(string path) { using(var h=System.Security.Cryptography.SHA256.Create())return BitConverter.ToString(h.ComputeHash(File.ReadAllBytes(path))).Replace("-","").ToLowerInvariant(); }
        public static string Prepare(string sourceRoot)
        {
            if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required.");
            for(int i=0;i<3;i++)Mf3DisplayBuild.BuildRuntimeLane(i);
            var catalog=JObject.Parse(CatalogMaterialBuild.BuildAt(sourceRoot,"Assets/MorphForge/Diagnostics/MF5Catalog"));
            var material=AssetDatabase.LoadAssetAtPath<Material>("Assets/MorphForge/Diagnostics/MF5Catalog/ParticleBoxCatalog.mat");
            material.SetFloat("_WireEncoding",5);EditorUtility.SetDirty(material);AssetDatabase.SaveAssetIfDirty(material);
            // Original FT assets and dirty scenes stay untouched. Remove the FT
            // plugin injection objects only from this independent product copy.
            var contents=PrefabUtility.LoadPrefabContents(FullFTBuild.TestAvatarPath);
            try
            {
                contents.name="MorphForge_MF5_Avatar";
                contents.GetComponent<VRC.Core.PipelineManager>().blueprintId=AvatarId;
                foreach(var t in contents.transform.Cast<Transform>().ToArray())
                    if(t.name=="VRCFT_UE_Nemesis_VRCFur"||t.name=="MixExpressions"||t.name=="MorphForgePlacement")UnityEngine.Object.DestroyImmediate(t.gameObject);
                var placement=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(FullFTBuild.PlacementPath),contents.transform);
                placement.name="MorphForgePlacement";
                var center=placement.GetComponentsInChildren<Transform>(true).Single(t=>t.name=="ProtocolCenter");
                foreach(var child in center.Cast<Transform>().ToArray())if(child.GetComponent<ModularAvatarMergeAnimator>())UnityEngine.Object.DestroyImmediate(child.gameObject);
                for(int i=0;i<3;i++)
                {
                    var lane=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(Root+"/Lane"+i+"/Display.prefab"),center);
                    lane.name="Lane"+i;
                    foreach(var store in lane.GetComponentsInChildren<ParticleSystem>(true).Where(p=>p.name=="Store"||p.name=="Store2")) {
                    var renderer=store.GetComponent<ParticleSystemRenderer>();renderer.sharedMaterial=material;
                    renderer.sortMode=ParticleSystemSortMode.Distance;
                    var settings=new SerializedObject(renderer);settings.FindProperty("m_ApplyActiveColorSpace").boolValue=false;settings.ApplyModifiedPropertiesWithoutUndo();
                    }
                }
                ConfigureScale(placement);
                FullFTBuild.SanitizePickupParticleForces(contents);
                if(contents.GetComponentsInChildren<Transform>(true).Sum(t=>GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(t.gameObject))!=0)throw new Exception("Missing script on MF5 avatar.");
                PrefabUtility.SaveAsPrefabAsset(contents,AvatarPath);
            }
            finally { PrefabUtility.UnloadPrefabContents(contents); }
            AssetDatabase.SaveAssets();
            var source=AssetDatabase.LoadAssetAtPath<GameObject>(AvatarPath);
            var clone=UnityEngine.Object.Instantiate(source);clone.name="MorphForge_MF5_Build";clone.hideFlags=HideFlags.HideAndDontSave;
            JObject budget;
            try {
                if(!VRCBuildPipelineCallbacks.OnPreprocessAvatar(clone))throw new Exception("MF5 joint preprocess failed.");
                budget=JObject.Parse(File.ReadAllText("Docs/MF5/final-budget.json"));
                var descriptor=clone.GetComponent<VRCAvatarDescriptor>();
                var fx=descriptor.baseAnimationLayers.First(l=>l.type==VRCAvatarDescriptor.AnimLayerType.FX).animatorController;
                budget["fxLayers"]=((UnityEditor.Animations.AnimatorController)fx).layers.Length;
                budget["fxParameters"]=((UnityEditor.Animations.AnimatorController)fx).parameters.Length;
            } finally { UnityEngine.Object.DestroyImmediate(clone); }
            var previous=SceneManager.GetActiveScene();
            if(SceneManager.GetSceneByPath(ScenePath).IsValid())throw new Exception("MF5 scene already open; inspect before replacing.");
            var scene=EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Additive);
            try
            {
                SceneManager.SetActiveScene(scene);
                var avatar=(GameObject)PrefabUtility.InstantiatePrefab(source,scene);avatar.name="MorphForge_MF5_Avatar";
                var gm=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>("Packages/vrchat.blackstartx.gesture-manager/GestureManager.prefab"),scene);
                gm.tag="EditorOnly";gm.name="MF5 Gesture Manager";var manager=gm.GetComponent<GestureManager>();manager.settings.favourite=avatar.GetComponent<VRCAvatarDescriptor>();manager.settings.loadStored=false;manager.settings.isRemote=false;
                var receiver=gm.AddComponent<MorphForgePlayReceiver>();receiver.ReceivePort=9012;receiver.SendPort=9013;
                var camera=new GameObject("MF5 Camera").AddComponent<Camera>();camera.tag="MainCamera";camera.transform.position=new Vector3(3,2.5f,4);camera.transform.LookAt(new Vector3(0,1,0));camera.nearClipPlane=.01f;camera.backgroundColor=new Color(.04f,.06f,.09f);camera.clearFlags=CameraClearFlags.SolidColor;camera.gameObject.AddComponent<AudioListener>();
                EditorSceneManager.SaveScene(scene,ScenePath);Selection.activeGameObject=avatar;
            }
            catch { EditorSceneManager.CloseScene(scene,true);SceneManager.SetActiveScene(previous);throw; }
            var report=new JObject{["success"]=true,["catalog"]=catalog,["budget"]=budget,["avatarId"]=AvatarId,["scene"]=ScenePath,["sourceSceneDirtyPreserved"]=previous.isDirty,["prefabSha256"]=Hash(AvatarPath),["activeCapacity"]=16384,["poolCapacity"]=16386,["lanes"]=3};
            Directory.CreateDirectory("Docs/MF5");
            var targetCatalog=JObject.Parse(File.ReadAllText(Path.Combine(sourceRoot,"catalog.json")));
            var capabilities=(JArray)targetCatalog["capabilities"]??new JArray();
            if(!capabilities.Any(v=>(string)v=="mf5-runtime-materials/1"))capabilities.Add("mf5-runtime-materials/1");
            if(!capabilities.Any(v=>(string)v=="mf5-dual-pool/1"))capabilities.Add("mf5-dual-pool/1");
            if(!capabilities.Any(v=>(string)v=="scale-reference-0.05/1"))capabilities.Add("scale-reference-0.05/1");
            targetCatalog["capabilities"]=capabilities;
            targetCatalog["targetAvatarId"]=AvatarId;
            targetCatalog["targetProtocol"]="mf5";
            File.WriteAllText("Docs/MF5/avatar-catalog.json",targetCatalog.ToString());
            File.WriteAllText("Docs/MF5/prepared.json",report.ToString());return report.ToString();
        }
        static void ConfigureScale(GameObject placement)
        {
            var source=AssetDatabase.LoadAssetAtPath<UnityEditor.Animations.AnimatorController>(PlacementStage4Build.Root+"/Placement.controller");
            var path=Root+"/Placement.controller";
            if(!AssetDatabase.LoadAssetAtPath<UnityEditor.Animations.AnimatorController>(path)) {
                if(!AssetDatabase.CopyAsset(PlacementStage4Build.Root+"/Placement.controller",path))throw new Exception("Cannot duplicate placement controller.");
            }
            var copy=AssetDatabase.LoadAssetAtPath<UnityEditor.Animations.AnimatorController>(path);
            var scaling=copy.layers.Single(l=>l.name=="Placement: content scale");
            var sm=scaling.stateMachine;var localState=sm.states.Single().state;
            var tree=new UnityEditor.Animations.BlendTree{name="MF5 logarithmic scale",blendType=UnityEditor.Animations.BlendTreeType.Simple1D,blendParameter="MF_Scale",useAutomaticThresholds=false};AssetDatabase.AddObjectToAsset(tree,copy);
            for(int i=0;i<=24;i++) {
                float wire=-1f+i/12f;float physical=.0005f*Mathf.Pow(1000f,i/24f);
                var clip=new AnimationClip{name="Scale"+i,frameRate=60};
                foreach(char axis in "xyz") {
                    AnimationUtility.SetEditorCurve(clip,EditorCurveBinding.FloatCurve(PlacementStage4Build.AnchorPath+"/ContentLeveling/ContentScale",typeof(Transform),"m_LocalScale."+axis),AnimationCurve.Constant(0,.03f,physical));
                    float handle=Mathf.Clamp(Mathf.Sqrt(physical/.05f),.2f,2f);
                    AnimationUtility.SetEditorCurve(clip,EditorCurveBinding.FloatCurve(PlacementStage4Build.AnchorPath+"/HandleVisual",typeof(Transform),"m_LocalScale."+axis),AnimationCurve.Constant(0,.03f,handle));
                }
                AssetDatabase.AddObjectToAsset(clip,copy);tree.AddChild(clip,wire);
            }
            localState.motion=tree;var layers=copy.layers;for(int i=0;i<layers.Length;i++)if(layers[i].name==scaling.name)layers[i].stateMachine=sm;copy.layers=layers;
            var parameters=copy.parameters;for(int i=0;i<parameters.Length;i++)if(parameters[i].name=="MF_Scale")parameters[i].defaultFloat=1f/3f;copy.parameters=parameters;
            placement.GetComponent<ModularAvatarMergeAnimator>().animator=copy;
            var settings=placement.GetComponent<ModularAvatarParameters>();for(int i=0;i<settings.parameters.Count;i++)if(settings.parameters[i].nameOrPrefix=="MF_Scale"){var p=settings.parameters[i];p.defaultValue=1f/3f;settings.parameters[i]=p;}
            var scale=placement.GetComponentsInChildren<Transform>(true).Single(t=>t.name=="ContentScale");scale.localScale=Vector3.one*.05f;
            ConfigureScaleMenu(placement,copy);
            EditorUtility.SetDirty(copy);AssetDatabase.SaveAssets();
        }
        static T SaveMenu<T>(T value,string path)where T:UnityEngine.Object {
            var existing=AssetDatabase.LoadAssetAtPath<T>(path);if(existing){EditorUtility.CopySerialized(value,existing);UnityEngine.Object.DestroyImmediate(value);EditorUtility.SetDirty(existing);return existing;}
            AssetDatabase.CreateAsset(value,path);return value;
        }
        static void ConfigureScaleMenu(GameObject placement,UnityEditor.Animations.AnimatorController fx)
        {
            const string dummy="MF5_ScaleDummy",preset="MF5_ScalePreset";
            var config=placement.GetComponent<ModularAvatarParameters>();
            foreach(var n in new[]{dummy,preset})if(!config.parameters.Any(p=>p.nameOrPrefix==n))config.parameters.Add(new ParameterConfig{nameOrPrefix=n,syncType=n==dummy?ParameterSyncType.Float:ParameterSyncType.Int,localOnly=true,saved=false,internalParameter=false,hasExplicitDefaultValue=true,defaultValue=0});
            if(!fx.parameters.Any(p=>p.name==dummy))fx.AddParameter(dummy,AnimatorControllerParameterType.Float);
            if(!fx.parameters.Any(p=>p.name==preset))fx.AddParameter(preset,AnimatorControllerParameterType.Int);
            fx.layers=fx.layers.Where(l=>l.name!="MF5 scale presets").ToArray();
            var sm=new UnityEditor.Animations.AnimatorStateMachine{name="MF5 scale presets"};AssetDatabase.AddObjectToAsset(sm,fx);
            fx.AddLayer(new UnityEditor.Animations.AnimatorControllerLayer{name=sm.name,stateMachine=sm,defaultWeight=1});
            var clip=new AnimationClip{name="MF5ScalePresetWait",frameRate=60};AssetDatabase.AddObjectToAsset(clip,fx);
            var idle=sm.AddState("Idle");idle.motion=clip;idle.writeDefaultValues=false;sm.defaultState=idle;
            var values=new[]{-1f,-1f/3f,1f/3f,1f};
            for(int i=0;i<values.Length;i++){
                var state=sm.AddState("Preset"+i);state.motion=clip;state.writeDefaultValues=false;
                var driver=state.AddStateMachineBehaviour<VRC.SDK3.Avatars.Components.VRCAvatarParameterDriver>();driver.localOnly=true;
                driver.parameters.Add(new VRC.SDKBase.VRC_AvatarParameterDriver.Parameter{name="MF_Scale",type=VRC.SDKBase.VRC_AvatarParameterDriver.ChangeType.Set,value=values[i]});
                driver.parameters.Add(new VRC.SDKBase.VRC_AvatarParameterDriver.Parameter{name=preset,type=VRC.SDKBase.VRC_AvatarParameterDriver.ChangeType.Set,value=0});
                var enter=sm.AddAnyStateTransition(state);enter.duration=0;enter.hasExitTime=false;enter.canTransitionToSelf=false;enter.AddCondition(UnityEditor.Animations.AnimatorConditionMode.Equals,i+1,preset);enter.AddCondition(UnityEditor.Animations.AnimatorConditionMode.If,0,"IsLocal");
                var leave=state.AddTransition(idle);leave.duration=0;leave.hasExitTime=false;leave.AddCondition(UnityEditor.Animations.AnimatorConditionMode.Equals,0,preset);
            }
            var sizeMenu=new VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu{name="MF5 Scale"};
            var joystick=new VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu.Control{name="连续缩放（上大下小）",type=VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu.Control.ControlType.TwoAxisPuppet,subParameters=new[]{new VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu.Control.Parameter{name=dummy},new VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu.Control.Parameter{name="MF_Scale"}}};sizeMenu.controls.Add(joystick);
            string[] labels={"1%","10%","100%（标准）","1000%"};
            for(int i=0;i<labels.Length;i++)sizeMenu.controls.Add(new VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu.Control{name=labels[i],type=VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu.Control.ControlType.Button,parameter=new VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu.Control.Parameter{name=preset},value=i+1});
            sizeMenu=SaveMenu(sizeMenu,Root+"/ScaleMenu.asset");
            var menu=UnityEngine.Object.Instantiate(AssetDatabase.LoadAssetAtPath<VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu>(PlacementStage4Build.Root+"/PlacementMenu.asset"));
            var index=menu.controls.FindIndex(c=>c.subParameters!=null&&c.subParameters.Any(p=>p.name=="MF_Scale"));if(index<0)throw new Exception("Scale menu entry missing.");
            menu.controls[index]=new VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu.Control{name="缩放比例",type=VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu.Control.ControlType.SubMenu,subMenu=sizeMenu};menu=SaveMenu(menu,Root+"/PlacementMenu.asset");
            var root=new VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu{name="MorphForge"};root.controls.Add(new VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu.Control{name="MorphForge",type=VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu.Control.ControlType.SubMenu,subMenu=menu});root=SaveMenu(root,Root+"/RootMenu.asset");placement.GetComponent<ModularAvatarMenuInstaller>().menuToAppend=root;
        }
    }
}
