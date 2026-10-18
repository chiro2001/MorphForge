using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
using VRC.SDK3.Avatars.ScriptableObjects;
using VRC.SDK3.Dynamics.Constraint.Components;
using VRC.SDK3.Dynamics.Contact.Components;
using VRC.Dynamics;
using VRC.SDKBase;
using nadena.dev.modular_avatar.core;
using BlackStartX.GestureManager;

namespace MorphForge.Editor
{
    public static class PlacementBuild
    {
        public const string Root="Assets/MorphForge/Stage3";
        public const string ScenePath="Assets/MorphForge/Scenes/MorphForge_Stage3.unity";
        public const string ModuleName="MorphForgePlacement";
        public const string AnchorPath="WorldFrame/AnchorPose";
        static AnimatorController fx;
        static T Asset<T>(T value,string path) where T:UnityEngine.Object
        {
            var existing=AssetDatabase.LoadAssetAtPath<T>(path);
            if(existing){EditorUtility.CopySerialized(value,existing);EditorUtility.SetDirty(existing);UnityEngine.Object.DestroyImmediate(value);return existing;}
            AssetDatabase.CreateAsset(value,path);return value;
        }
        static void Folder(string path)
        {string p="Assets";foreach(var part in path.Split('/').Skip(1)){string next=p+"/"+part;if(!AssetDatabase.IsValidFolder(next))AssetDatabase.CreateFolder(p,part);p=next;}}
        static GameObject Child(string name,Transform parent)
        {var go=new GameObject(name);go.transform.SetParent(parent,false);return go;}
        static AnimatorStateMachine Layer(string name)
        {
            var sm=new AnimatorStateMachine{name=name};AssetDatabase.AddObjectToAsset(sm,fx);
            fx.AddLayer(new AnimatorControllerLayer{name=name,stateMachine=sm,defaultWeight=1});return sm;
        }
        static AnimationClip Clip(string name,float duration=.03f)
        {return new AnimationClip{name=name,frameRate=60};}
        static void Curve(AnimationClip clip,string path,Type type,string property,float value,float duration=.03f)
        {AnimationUtility.SetEditorCurve(clip,EditorCurveBinding.FloatCurve(path,type,property),AnimationCurve.Constant(0,duration,value));}
        static AnimatorState State(AnimatorStateMachine sm,string name,AnimationClip clip)
        {
            clip=Asset(clip,Root+"/Clips/"+clip.name+".anim");var state=sm.AddState(name);state.motion=clip;state.writeDefaultValues=false;return state;
        }
        static AnimatorStateTransition To(AnimatorState from,AnimatorState to,bool exit=false)
        {var t=from.AddTransition(to);t.duration=0;t.hasExitTime=exit;t.exitTime=1;t.canTransitionToSelf=false;return t;}
        static void Equals(AnimatorStateTransition t,string name,int value){t.AddCondition(AnimatorConditionMode.Equals,value,name);}
        static void Bool(AnimatorStateTransition t,string name,bool value){t.AddCondition(value?AnimatorConditionMode.If:AnimatorConditionMode.IfNot,0,name);}
        static void Driver(AnimatorState state,int grab)
        {
            var d=state.AddStateMachineBehaviour<VRCAvatarParameterDriver>();d.localOnly=true;
            d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name="MF_Grab",type=VRC_AvatarParameterDriver.ChangeType.Set,value=grab});
        }
        static AnimationClip Pose(string name,bool freeze,int source,bool reset=false,float duration=.03f,bool guide=false)
        {
            var c=Clip(name);var type=typeof(VRCParentConstraint);
            foreach(var field in new[]{"Locked","AffectsPositionX","AffectsPositionY","AffectsPositionZ","AffectsRotationX","AffectsRotationY","AffectsRotationZ"})Curve(c,AnchorPath,type,field,1,duration);
            Curve(c,AnchorPath,type,"IsActive",reset?0:1,duration);
            Curve(c,"ResetGuide",type,"IsActive",reset||guide?1:0,duration);
            Curve(c,AnchorPath,type,"GlobalWeight",1,duration);
            Curve(c,AnchorPath,type,"FreezeToWorld",freeze?1:0,duration);
            Curve(c,AnchorPath,type,"RebakeOffsetsWhenUnfrozen",reset?0:1,duration);
            for(int i=0;i<3;i++)
            {
                Curve(c,AnchorPath,type,"Sources.source"+i+".Weight",source==i?1:0,duration);
            }
            return c;
        }
        static void BuildFx()
        {
            fx=AssetDatabase.LoadAssetAtPath<AnimatorController>(Root+"/Placement.controller");
            if(!fx)fx=AnimatorController.CreateAnimatorControllerAtPath(Root+"/Placement.controller");
            else
            {
                fx.layers=new AnimatorControllerLayer[0];fx.parameters=new AnimatorControllerParameter[0];
                foreach(var item in AssetDatabase.LoadAllAssetsAtPath(Root+"/Placement.controller"))if(item&&item!=fx)UnityEngine.Object.DestroyImmediate(item,true);
            }
            foreach(string name in new[]{"MF_World","MF_Handle","MF_Reset","MF_NearL","MF_NearR","IsLocal"})
                fx.AddParameter(new AnimatorControllerParameter{name=name,type=AnimatorControllerParameterType.Bool,defaultBool=name=="MF_World"||name=="MF_Handle"});
            fx.AddParameter(new AnimatorControllerParameter{name="MF_Scale",type=AnimatorControllerParameterType.Float,defaultFloat=1});
            foreach(string name in new[]{"MF_Grab","GestureLeft","GestureRight"})fx.AddParameter(name,AnimatorControllerParameterType.Int);

            var pose=Layer("Placement: world / avatar / hands");
            var init=State(pose,"Initialize at avatar dock",Pose("Init",false,0,true,.15f));pose.defaultState=init;
            var world=State(pose,"World fixed",Pose("World",true,0));
            var follow=State(pose,"Follow avatar",Pose("Follow",false,0));
            var armL=State(pose,"Arm left",Pose("ArmLeft",true,1));var left=State(pose,"Held left",Pose("Left",false,1));
            var armR=State(pose,"Arm right",Pose("ArmRight",true,2));var right=State(pose,"Held right",Pose("Right",false,2));
            var release=State(pose,"Capture release pose",Pose("Release",true,0));
            var reset=State(pose,"Reset to avatar dock",Pose("Reset",false,0,true,.1f));
            Driver(reset,0);
            var armReset=State(pose,"Rearm after reset",Pose("RearmReset",false,0,false,.08f,true));
            var captureReset=State(pose,"Capture reset pose",Pose("CaptureReset",true,0,false,.04f,true));
            var any=pose.AddAnyStateTransition(reset);any.duration=0;any.hasExitTime=false;any.canTransitionToSelf=false;Bool(any,"MF_Reset",true);
            foreach(var s in new[]{init,reset})
            {
                var t=To(s,armReset,true);Bool(t,"MF_Reset",false);
            }
            To(armReset,captureReset,true);
            var afterReset=To(captureReset,world,true);Bool(afterReset,"MF_World",true);
            afterReset=To(captureReset,follow,true);Bool(afterReset,"MF_World",false);
            var sw=To(world,follow);Bool(sw,"MF_World",false);Equals(sw,"MF_Grab",0);
            sw=To(follow,world);Bool(sw,"MF_World",true);Equals(sw,"MF_Grab",0);
            foreach(var s in new[]{world,follow})
            {
                var t=To(s,armL);Equals(t,"MF_Grab",1);Bool(t,"MF_Reset",false);
                t=To(s,armR);Equals(t,"MF_Grab",2);Bool(t,"MF_Reset",false);
            }
            To(armL,left,true);To(armR,right,true);
            foreach(var s in new[]{left,right}){var t=To(s,release);Equals(t,"MF_Grab",0);}
            sw=To(left,armR);Equals(sw,"MF_Grab",2);sw=To(right,armL);Equals(sw,"MF_Grab",1);
            sw=To(release,world,true);Bool(sw,"MF_World",true);sw=To(release,follow,true);Bool(sw,"MF_World",false);

            var detect=Layer("Placement: owner contact and fist detection");
            AnimationClip blank(string name){var c=Clip(name);Curve(c,"HandReferences",typeof(Transform),"m_LocalScale.x",1);return c;}
            var idle=State(detect,"Idle",blank("GripIdle"));detect.defaultState=idle;
            var hL=State(detect,"Grip left",blank("GripL"));Driver(hL,1);
            var hR=State(detect,"Grip right",blank("GripR"));Driver(hR,2);
            var done=State(detect,"Wait for open hands",blank("GripRelease"));Driver(done,0);
            foreach(var item in new[]{new{state=hL,near="MF_NearL",gesture="GestureLeft",value=1},new{state=hR,near="MF_NearR",gesture="GestureRight",value=2}})
            {
                var t=To(idle,item.state);Bool(t,"IsLocal",true);Bool(t,"MF_Handle",true);Bool(t,"MF_Reset",false);Bool(t,item.near,true);Equals(t,item.gesture,1);Equals(t,"MF_Grab",0);
                t=To(item.state,idle);t.AddCondition(AnimatorConditionMode.NotEqual,item.value,"MF_Grab");
                t=To(item.state,done);t.AddCondition(AnimatorConditionMode.NotEqual,1,item.gesture);
                t=To(item.state,done);Bool(t,"MF_Reset",true);
                t=To(item.state,done);Bool(t,"MF_Handle",false);
            }
            sw=To(done,idle);sw.AddCondition(AnimatorConditionMode.NotEqual,1,"GestureLeft");sw.AddCondition(AnimatorConditionMode.NotEqual,1,"GestureRight");

            var scaling=Layer("Placement: content scale");
            var small=Clip("ScaleMin");var large=Clip("ScaleMax");
            foreach(char a in "xyz")
            {Curve(small,AnchorPath+"/ContentScale",typeof(Transform),"m_LocalScale."+a,.05f);Curve(large,AnchorPath+"/ContentScale",typeof(Transform),"m_LocalScale."+a,1);}
            small=Asset(small,Root+"/Clips/ScaleMin.anim");large=Asset(large,Root+"/Clips/ScaleMax.anim");
            var tree=new BlendTree{name="Uniform scale",blendType=BlendTreeType.Simple1D,blendParameter="MF_Scale",useAutomaticThresholds=false};
            tree.AddChild(small,.05f);tree.AddChild(large,1);AssetDatabase.AddObjectToAsset(tree,fx);
            var drive=scaling.AddState("Scale");drive.motion=tree;drive.writeDefaultValues=false;
            var visibility=Layer("Placement: handle visibility");
            var on=Clip("HandleOn");var off=Clip("HandleOff");
            foreach(var path in new[]{AnchorPath+"/HandleVisual",AnchorPath+"/Sensors"}){Curve(on,path,typeof(GameObject),"m_IsActive",1);Curve(off,path,typeof(GameObject),"m_IsActive",0);}
            var shown=State(visibility,"Shown",on);var hidden=State(visibility,"Hidden",off);visibility.defaultState=shown;
            sw=To(shown,hidden);Bool(sw,"MF_Handle",false);sw=To(hidden,shown);Bool(sw,"MF_Handle",true);
            EditorUtility.SetDirty(fx);AssetDatabase.SaveAssets();
        }
        static void AddParameter(ModularAvatarParameters config,string name,ParameterSyncType type,float value,bool saved)
        {config.parameters.Add(new ParameterConfig{nameOrPrefix=name,syncType=type,defaultValue=value,hasExplicitDefaultValue=true,saved=saved,localOnly=false,internalParameter=false});}
        static void SaveControllerRefs(GameObject asset)
        {
            foreach(var merge in asset.GetComponentsInChildren<ModularAvatarMergeAnimator>(true))
            {
                var controller=AssetDatabase.LoadAssetAtPath<AnimatorController>(merge.name==ModuleName||merge.transform==asset.transform&&asset.name=="Placement"?Root+"/Placement.controller":ParticleStage2Build.Root+"/FX.controller");
                if(merge.gameObject.name==ModuleName||merge.animator==fx)controller=fx;
                var so=new SerializedObject(merge);so.FindProperty("animator").objectReferenceValue=controller;so.ApplyModifiedPropertiesWithoutUndo();EditorUtility.SetDirty(merge);
            }
        }
        public static string Build()
        {
            if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new InvalidOperationException("Requires idle Edit Mode.");
            var old=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
            for(int i=UnityEngine.SceneManagement.SceneManager.sceneCount-1;i>=0;i--)
            {var scene=UnityEngine.SceneManagement.SceneManager.GetSceneAt(i);if(scene.path==ScenePath){if(scene.isDirty)throw new InvalidOperationException("Save Stage3 edits before rebuilding.");EditorSceneManager.CloseScene(scene,true);}}
            var target=EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Additive);UnityEngine.SceneManagement.SceneManager.SetActiveScene(target);
            Folder(Root);Folder(Root+"/Clips");BuildFx();
            var source=AssetDatabase.LoadAssetAtPath<GameObject>(ParticleStage2Build.Root+"/Avatar.prefab");
            if(!source)throw new InvalidOperationException("Build Stage2 first.");
            var avatar=(GameObject)PrefabUtility.InstantiatePrefab(source,target);PrefabUtility.UnpackPrefabInstance(avatar,PrefabUnpackMode.Completely,InteractionMode.AutomatedAction);avatar.name="MorphForge_Stage3_Avatar";
            var previous=avatar.transform.Find(ParticleStage2Build.ModuleName);if(previous)UnityEngine.Object.DestroyImmediate(previous.gameObject);
            var module=Child(ModuleName,avatar.transform);
            var hands=Child("HandReferences",module.transform);
            var l=Child("LeftHand",hands.transform);var r=Child("RightHand",hands.transform);
            foreach(var item in new[]{new{go=l,bone=HumanBodyBones.LeftHand},new{go=r,bone=HumanBodyBones.RightHand}})
            {var proxy=item.go.AddComponent<ModularAvatarBoneProxy>();proxy.boneReference=item.bone;proxy.attachmentMode=BoneProxyAttachmentMode.AsChildAtRoot;proxy.matchScale=false;}
            var animator=avatar.GetComponent<Animator>();var lh=animator.GetBoneTransform(HumanBodyBones.LeftHand);var rh=animator.GetBoneTransform(HumanBodyBones.RightHand);
            float height=Mathf.Max(.2f,avatar.transform.InverseTransformPoint((lh.position+rh.position)*.5f).y);
            var dock=Child("AvatarDock",module.transform);dock.transform.localPosition=new Vector3(0,height,Mathf.Clamp(height*.6f,.18f,.65f));
            var frame=Child("WorldFrame",module.transform);frame.AddComponent<ModularAvatarWorldFixedObject>();frame.AddComponent<ModularAvatarWorldScaleObject>();
            var anchor=Child("AnchorPose",frame.transform);anchor.transform.position=dock.transform.position;anchor.transform.rotation=dock.transform.rotation;
            var constraint=anchor.AddComponent<VRCParentConstraint>();constraint.IsActive=true;constraint.Locked=true;constraint.GlobalWeight=1;constraint.RebakeOffsetsWhenUnfrozen=true;
            constraint.AffectsPositionX=constraint.AffectsPositionY=constraint.AffectsPositionZ=true;
            constraint.AffectsRotationX=constraint.AffectsRotationY=constraint.AffectsRotationZ=true;
            foreach(var t in new[]{dock.transform,l.transform,r.transform})constraint.Sources.Add(new VRCConstraintSource{SourceTransform=t,Weight=t==dock.transform?1:0});
            // A separate native constraint resets the pose without animating the main
            // constraint's rebaked offsets (Write Defaults would continually erase them).
            var guide=Child("ResetGuide",module.transform).AddComponent<VRCParentConstraint>();
            guide.TargetTransform=anchor.transform;guide.IsActive=false;guide.Locked=true;guide.GlobalWeight=1;
            guide.AffectsPositionX=guide.AffectsPositionY=guide.AffectsPositionZ=true;
            guide.AffectsRotationX=guide.AffectsRotationY=guide.AffectsRotationZ=true;
            guide.Sources.Add(new VRCConstraintSource{SourceTransform=dock.transform,Weight=1});
            var visible=Child("HandleVisual",anchor.transform);
            var sphere=GameObject.CreatePrimitive(PrimitiveType.Sphere);sphere.name="Gold bottom-center handle";sphere.transform.SetParent(visible.transform,false);sphere.transform.localScale=Vector3.one*.09f;UnityEngine.Object.DestroyImmediate(sphere.GetComponent<Collider>());
            var material=new Material(Shader.Find("MorphForge/AnchorHandle")){name="Gold translucent placement handle"};material=Asset(material,Root+"/Handle.mat");sphere.GetComponent<Renderer>().sharedMaterial=material;
            var sensors=Child("Sensors",anchor.transform);
            foreach(var side in new[]{new{name="Left",tag="HandL",parameter="MF_NearL"},new{name="Right",tag="HandR",parameter="MF_NearR"}})
            {
                var sensor=Child(side.name,sensors.transform).AddComponent<VRCContactReceiver>();sensor.rootTransform=anchor.transform; sensor.shapeType=ContactBase.ShapeType.Sphere;sensor.radius=.13f;
                sensor.allowSelf=true;sensor.allowOthers=false;sensor.localOnly=true;sensor.receiverType=ContactReceiver.ReceiverType.Constant;sensor.parameter=side.parameter;sensor.collisionValue=1;sensor.collisionTags=new List<string>{side.tag};
            }
            var scale=Child("ContentScale",anchor.transform);var center=Child("ProtocolCenter",scale.transform);center.transform.localPosition=new Vector3(0,32,0);
            var display=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(ParticleStage2Build.Root+"/Display.prefab"),center.transform);display.name=ParticleStage2Build.ModuleName;
            var store=display.transform.Find("Store").GetComponent<ParticleSystem>();var main=store.main;main.scalingMode=ParticleSystemScalingMode.Hierarchy;
            PrefabUtility.RecordPrefabInstancePropertyModifications(store);
            var merge=module.AddComponent<ModularAvatarMergeAnimator>();merge.animator=fx;merge.layerType=VRCAvatarDescriptor.AnimLayerType.FX;merge.pathMode=MergeAnimatorPathMode.Relative;merge.matchAvatarWriteDefaults=false;merge.layerPriority=100;
            var parameters=module.AddComponent<ModularAvatarParameters>();AddParameter(parameters,"MF_World",ParameterSyncType.Bool,1,true);AddParameter(parameters,"MF_Handle",ParameterSyncType.Bool,1,true);
            AddParameter(parameters,"MF_Scale",ParameterSyncType.Float,1,true);AddParameter(parameters,"MF_Reset",ParameterSyncType.Bool,0,false);AddParameter(parameters,"MF_Grab",ParameterSyncType.Int,0,false);
            var menu=new VRCExpressionsMenu{name="MorphForge placement"};
            VRCExpressionsMenu.Control Control(string name,VRCExpressionsMenu.Control.ControlType type,string parameter,float value=1)
            {return new VRCExpressionsMenu.Control{name=name,type=type,parameter=new VRCExpressionsMenu.Control.Parameter{name=parameter},value=value};}
            menu.controls.Add(Control("固定在世界",VRCExpressionsMenu.Control.ControlType.Toggle,"MF_World"));
            menu.controls.Add(Control("显示定位球",VRCExpressionsMenu.Control.ControlType.Toggle,"MF_Handle"));
            var size=Control("体素缩放",VRCExpressionsMenu.Control.ControlType.RadialPuppet,"");size.subParameters=new[]{new VRCExpressionsMenu.Control.Parameter{name="MF_Scale"}};menu.controls.Add(size);
            menu.controls.Add(Control("移到面前",VRCExpressionsMenu.Control.ControlType.Button,"MF_Reset"));
            menu.controls.Add(Control("左手跟随（菜单）",VRCExpressionsMenu.Control.ControlType.Toggle,"MF_Grab",1));
            menu.controls.Add(Control("右手跟随（菜单）",VRCExpressionsMenu.Control.ControlType.Toggle,"MF_Grab",2));
            menu=Asset(menu,Root+"/PlacementMenu.asset");
            var install=new VRCExpressionsMenu{name="MorphForge"};install.controls.Add(new VRCExpressionsMenu.Control{name="MorphForge",type=VRCExpressionsMenu.Control.ControlType.SubMenu,subMenu=menu});install=Asset(install,Root+"/RootMenu.asset");module.AddComponent<ModularAvatarMenuInstaller>().menuToAppend=install;
            PrefabUtility.SaveAsPrefabAsset(module,Root+"/Placement.prefab");PrefabUtility.SaveAsPrefabAsset(avatar,Root+"/Avatar.prefab");AssetDatabase.SaveAssets();
            foreach(var path in new[]{Root+"/Placement.prefab",Root+"/Avatar.prefab"})
            {AssetDatabase.ImportAsset(path,ImportAssetOptions.ForceUpdate);var asset=AssetDatabase.LoadAssetAtPath<GameObject>(path);SaveControllerRefs(asset);PrefabUtility.SavePrefabAsset(asset);}
            foreach(var m in module.GetComponentsInChildren<ModularAvatarMergeAnimator>(true)){m.animator=m.gameObject==module?fx:AssetDatabase.LoadAssetAtPath<AnimatorController>(ParticleStage2Build.Root+"/FX.controller");EditorUtility.SetDirty(m);PrefabUtility.RecordPrefabInstancePropertyModifications(m);}
            var camera=new GameObject("Main Camera",typeof(Camera),typeof(AudioListener)).GetComponent<Camera>();camera.tag="MainCamera";camera.transform.position=new Vector3(3,2.2f,6);camera.transform.LookAt(new Vector3(0,height+.4f,.3f));camera.nearClipPlane=.01f;camera.farClipPlane=500;camera.backgroundColor=new Color(.04f,.055f,.08f);camera.clearFlags=CameraClearFlags.SolidColor;
            var light=new GameObject("Directional Light").AddComponent<Light>();light.type=LightType.Directional;light.intensity=1;light.transform.rotation=Quaternion.Euler(50,-30,0);
            var ground=GameObject.CreatePrimitive(PrimitiveType.Plane);ground.name="Observation floor";ground.tag="EditorOnly";ground.transform.localScale=Vector3.one*2;ground.transform.position=new Vector3(0,-.01f,0);UnityEngine.Object.DestroyImmediate(ground.GetComponent<Collider>());ground.GetComponent<Renderer>().sharedMaterial=AssetDatabase.LoadAssetAtPath<Material>(ParticleStage2Build.Root+"/ObservationFloor.mat");
            var gmRoot=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>("Packages/vrchat.blackstartx.gesture-manager/GestureManager.prefab"),target);gmRoot.name="MorphForge Stage3 Gesture Manager";gmRoot.tag="EditorOnly";
            var gm=gmRoot.GetComponent<GestureManager>();gm.settings.favourite=avatar.GetComponent<VRCAvatarDescriptor>();gm.settings.loadStored=false;gm.settings.isRemote=false;
            EditorSceneManager.MarkSceneDirty(target);EditorSceneManager.SaveScene(target,ScenePath);AssetDatabase.SaveAssets();Selection.activeGameObject=avatar;
            Directory.CreateDirectory("Docs/Stage3");string report=Newtonsoft.Json.JsonConvert.SerializeObject(new{scene=ScenePath,avatar=Root+"/Avatar.prefab",module=Root+"/Placement.prefab",transport="MF2 unchanged",placementParameters=new[]{"MF_World","MF_Handle","MF_Scale","MF_Reset","MF_Grab"},addedBits=19,defaultWorldFixed=true,scaleRange=new[]{.05f,1},bottomCenterProtocolOffset=new[]{0,32,0},dock=new[]{dock.transform.localPosition.x,dock.transform.localPosition.y,dock.transform.localPosition.z}},Newtonsoft.Json.Formatting.Indented);
            File.WriteAllText("Docs/Stage3/build.json",report);return report;
        }
    }
}
