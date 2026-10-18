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
    public static class PlacementStage4Build
    {
        public const string Root="Assets/MorphForge/Stage4/Placement";
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
            foreach(string name in new[]{"MF_GrabL","MF_GrabR","MF_Level"})fx.AddParameter(new AnimatorControllerParameter{name=name,type=AnimatorControllerParameterType.Bool,defaultBool=name=="MF_Level"});
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
            {Curve(small,AnchorPath+"/ContentLeveling/ContentScale",typeof(Transform),"m_LocalScale."+a,.05f);Curve(large,AnchorPath+"/ContentLeveling/ContentScale",typeof(Transform),"m_LocalScale."+a,1);}
            small=Asset(small,Root+"/Clips/ScaleMin.anim");large=Asset(large,Root+"/Clips/ScaleMax.anim");
            var tree=new BlendTree{name="Uniform scale",blendType=BlendTreeType.Simple1D,blendParameter="MF_Scale",useAutomaticThresholds=false};
            tree.AddChild(small,.05f);tree.AddChild(large,1);AssetDatabase.AddObjectToAsset(tree,fx);
            var drive=scaling.AddState("Scale");drive.motion=tree;drive.writeDefaultValues=false;
            var visibility=Layer("Placement: handle visibility");
            var on=Clip("HandleOn");var off=Clip("HandleOff");
            foreach(var path in new[]{AnchorPath+"/HandleVisual",AnchorPath+"/Sensors"}){Curve(on,path,typeof(GameObject),"m_IsActive",1);Curve(off,path,typeof(GameObject),"m_IsActive",0);}
            var shown=State(visibility,"Shown",on);var hidden=State(visibility,"Hidden",off);visibility.defaultState=shown;
            sw=To(shown,hidden);Bool(sw,"MF_Handle",false);sw=To(hidden,shown);Bool(sw,"MF_Handle",true);
            var level=Layer("Placement: world horizontal content");
            var levelOn=Clip("LevelOn");var levelOff=Clip("LevelOff");
            Curve(levelOn,AnchorPath+"/ContentLeveling",typeof(VRCRotationConstraint),"GlobalWeight",1);
            Curve(levelOff,AnchorPath+"/ContentLeveling",typeof(VRCRotationConstraint),"GlobalWeight",0);
            foreach(var clip in new[]{levelOn,levelOff})foreach(char axis in "xyzw")Curve(clip,AnchorPath+"/ContentLeveling",typeof(Transform),"m_LocalRotation."+axis,axis=='w'?1:0);
            var leveled=State(level,"Horizontal",levelOn);var tilted=State(level,"Full rotation",levelOff);level.defaultState=leveled;
            var lt=To(leveled,tilted);Bool(lt,"MF_Level",false);lt=To(tilted,leveled);Bool(lt,"MF_Level",true);
            var bridge=Layer("Placement: local hand / two synced bits");
            var start=State(bridge,"Wait",blank("BridgeWait"));bridge.defaultState=start;
            foreach(bool local in new[]{true,false})for(int grab=0;grab<3;grab++)
            {
                var state=State(bridge,(local?"Owner":"Remote")+grab,blank((local?"Owner":"Remote")+grab));
                var driver=state.AddStateMachineBehaviour<VRCAvatarParameterDriver>();driver.localOnly=local;
                void SetValue(string n,float v){driver.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=n,type=VRC_AvatarParameterDriver.ChangeType.Set,value=v});}
                if(local){SetValue("MF_GrabL",grab==1?1:0);SetValue("MF_GrabR",grab==2?1:0);}else SetValue("MF_Grab",grab);
                var t=bridge.AddAnyStateTransition(state);t.duration=0;t.hasExitTime=false;t.canTransitionToSelf=false;Bool(t,"IsLocal",local);
                if(local)Equals(t,"MF_Grab",grab);
                else if(grab==0){Bool(t,"MF_GrabL",false);Bool(t,"MF_GrabR",false);}
                else if(grab==1){Bool(t,"MF_GrabL",true);Bool(t,"MF_GrabR",false);}
                else Bool(t,"MF_GrabR",true);
            }
            EditorUtility.SetDirty(fx);AssetDatabase.SaveAssets();
        }
        static void AddParameter(ModularAvatarParameters config,string name,ParameterSyncType type,float value,bool saved)
        {config.parameters.Add(new ParameterConfig{nameOrPrefix=name,syncType=type,defaultValue=value,hasExplicitDefaultValue=true,saved=saved,localOnly=false,internalParameter=false});}
        static void SaveControllerRefs(GameObject asset)
        {
            foreach(var merge in asset.GetComponentsInChildren<ModularAvatarMergeAnimator>(true))
            {
                var controller=AssetDatabase.LoadAssetAtPath<AnimatorController>(merge.name==ModuleName||merge.transform==asset.transform&&asset.name=="Placement"?Root+"/Placement.controller":Mf3DisplayBuild.Root+"/FX.controller");
                if(merge.gameObject.name==ModuleName||merge.animator==fx)controller=fx;
                var so=new SerializedObject(merge);so.FindProperty("animator").objectReferenceValue=controller;so.ApplyModifiedPropertiesWithoutUndo();EditorUtility.SetDirty(merge);
            }
        }
        static Transform Named(Transform root,string name){return root.GetComponentsInChildren<Transform>(true).First(t=>t.name==name);}
        static void Projected(Transform target,Transform source)
        {
            var c=target.GetComponent<VRCPositionConstraint>();if(!c)c=target.gameObject.AddComponent<VRCPositionConstraint>();c.Sources.Clear();c.Sources.Add(new VRCConstraintSource{SourceTransform=source,Weight=1});
            c.IsActive=true;c.Locked=true;c.GlobalWeight=1;c.SolveInLocalSpace=false;c.PositionAtRest=Vector3.zero;c.PositionOffset=Vector3.zero;c.AffectsPositionX=c.AffectsPositionZ=true;c.AffectsPositionY=false;c.FreezeToWorld=false;c.RebakeOffsetsWhenUnfrozen=false;
        }
        public static string UpgradeCurrent(bool resumeOwnedPartial=false)
        {
            if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new InvalidOperationException("Requires Edit Mode");
            var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
            if(scene.path!=ScenePath||(scene.isDirty&&!resumeOwnedPartial))throw new InvalidOperationException("The saved single-Avatar Stage3 scene must be active; existing unsaved edits are protected");
            var avatars=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).ToArray();if(avatars.Length!=1)throw new InvalidOperationException("Requires exactly one Avatar");
            var avatar=avatars[0].gameObject;var module=avatar.transform.Find(ModuleName);if(!module)throw new InvalidOperationException("Missing placement module");
            Folder("Assets/MorphForge/Stage4");Folder(Root);Folder(Root+"/Clips");
            const string backup="Assets/MorphForge/Stage4/BeforeUpgrade.unity";if(!AssetDatabase.LoadAssetAtPath<SceneAsset>(backup))EditorSceneManager.SaveScene(scene,backup,true);
            Mf3DisplayBuild.Build();BuildFx();Undo.RegisterFullObjectHierarchyUndo(avatar,"Upgrade MorphForge MF3");
            var anchor=Named(module,"AnchorPose");var frame=Named(module,"WorldFrame");
            var level=anchor.Find("ContentLeveling");if(!level)level=Child("ContentLeveling",anchor).transform;
            var scale=anchor.Find("ContentScale");if(scale)scale.SetParent(level,false);
            var probe=anchor.Find("ForwardProbe");if(!probe)probe=Child("ForwardProbe",anchor).transform;probe.localPosition=new Vector3(.01f,0,1);
            var origin=frame.Find("LevelYawBase");if(!origin)origin=Child("LevelYawBase",frame).transform;origin.localPosition=Vector3.zero;Projected(origin,anchor);
            var forward=frame.Find("LevelYawForward");if(!forward)forward=Child("LevelYawForward",frame).transform;forward.localPosition=Vector3.zero;Projected(forward,probe);
            var aimTransform=origin.Find("LevelYawAim");if(!aimTransform)aimTransform=Child("LevelYawAim",origin).transform;
            var aim=aimTransform.GetComponent<VRCAimConstraint>();if(!aim)aim=aimTransform.gameObject.AddComponent<VRCAimConstraint>();aim.Sources.Clear();aim.Sources.Add(new VRCConstraintSource{SourceTransform=forward,Weight=1});aim.IsActive=true;aim.Locked=true;aim.GlobalWeight=1;aim.SolveInLocalSpace=false;aim.AffectsRotationX=aim.AffectsRotationY=aim.AffectsRotationZ=true;aim.AimAxis=Vector3.forward;aim.UpAxis=Vector3.up;aim.WorldUp=VRCConstraintBase.WorldUpType.Vector;aim.WorldUpVector=Vector3.up;aim.FreezeToWorld=false;aim.RebakeOffsetsWhenUnfrozen=false;aim.RotationAtRest=Vector3.zero;aim.RotationOffset=new Vector3(0,-Mathf.Atan(.01f)*Mathf.Rad2Deg,0);
            var rotation=level.GetComponent<VRCRotationConstraint>();if(!rotation)rotation=level.gameObject.AddComponent<VRCRotationConstraint>();rotation.Sources.Clear();rotation.Sources.Add(new VRCConstraintSource{SourceTransform=aimTransform,Weight=1});rotation.IsActive=true;rotation.Locked=true;rotation.GlobalWeight=1;rotation.SolveInLocalSpace=false;rotation.AffectsRotationX=rotation.AffectsRotationY=rotation.AffectsRotationZ=true;rotation.FreezeToWorld=false;rotation.RebakeOffsetsWhenUnfrozen=false;rotation.RotationAtRest=Vector3.zero;rotation.RotationOffset=Vector3.zero;
            var center=Named(module,"ProtocolCenter");foreach(var child in center.Cast<Transform>().ToArray())if(child.GetComponent<ModularAvatarMergeAnimator>())UnityEngine.Object.DestroyImmediate(child.gameObject);
            var display=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(Mf3DisplayBuild.Root+"/Display.prefab"),center);display.name=Mf3DisplayBuild.ModuleName;
            var merge=module.GetComponent<ModularAvatarMergeAnimator>();merge.animator=fx;EditorUtility.SetDirty(merge);
            var parameters=module.GetComponent<ModularAvatarParameters>();parameters.parameters.Clear();AddParameter(parameters,"MF_World",ParameterSyncType.Bool,1,true);AddParameter(parameters,"MF_Handle",ParameterSyncType.Bool,1,true);AddParameter(parameters,"MF_Scale",ParameterSyncType.Float,1,true);AddParameter(parameters,"MF_Reset",ParameterSyncType.Bool,0,false);AddParameter(parameters,"MF_GrabL",ParameterSyncType.Bool,0,false);AddParameter(parameters,"MF_GrabR",ParameterSyncType.Bool,0,false);AddParameter(parameters,"MF_Level",ParameterSyncType.Bool,1,true);
            parameters.parameters.Add(new ParameterConfig{nameOrPrefix="MF_Grab",syncType=ParameterSyncType.Int,localOnly=true,saved=false,hasExplicitDefaultValue=true,defaultValue=0,internalParameter=false});EditorUtility.SetDirty(parameters);
            var menu=UnityEngine.Object.Instantiate(AssetDatabase.LoadAssetAtPath<VRCExpressionsMenu>("Assets/MorphForge/Stage3/PlacementMenu.asset"));menu.name="MorphForge placement";
            if(!menu.controls.Any(c=>c.parameter?.name=="MF_Level"))menu.controls.Add(new VRCExpressionsMenu.Control{name="保持水平",type=VRCExpressionsMenu.Control.ControlType.Toggle,parameter=new VRCExpressionsMenu.Control.Parameter{name="MF_Level"},value=1});menu=Asset(menu,Root+"/PlacementMenu.asset");
            var install=new VRCExpressionsMenu{name="MorphForge"};install.controls.Add(new VRCExpressionsMenu.Control{name="MorphForge",type=VRCExpressionsMenu.Control.ControlType.SubMenu,subMenu=menu});install=Asset(install,Root+"/RootMenu.asset");module.GetComponent<ModularAvatarMenuInstaller>().menuToAppend=install;
            PrefabUtility.SaveAsPrefabAsset(module.gameObject,Root+"/Placement.prefab");PrefabUtility.SaveAsPrefabAsset(avatar,"Assets/MorphForge/Stage4/Avatar.prefab");AssetDatabase.SaveAssets();
            foreach(string path in new[]{Root+"/Placement.prefab","Assets/MorphForge/Stage4/Avatar.prefab"}){AssetDatabase.ImportAsset(path,ImportAssetOptions.ForceUpdate);var asset=AssetDatabase.LoadAssetAtPath<GameObject>(path);SaveControllerRefs(asset);PrefabUtility.SavePrefabAsset(asset);}
            var receiver=scene.GetRootGameObjects().Select(g=>g.GetComponent<MorphForgePlayReceiver>()).FirstOrDefault(c=>c);if(!receiver){var go=scene.GetRootGameObjects().FirstOrDefault(g=>g.name=="MorphForge Test Receiver")??new GameObject("MorphForge Test Receiver");go.tag="EditorOnly";receiver=go.AddComponent<MorphForgePlayReceiver>();if(!receiver)throw new InvalidOperationException("Cannot attach runtime-assembly Editor-only receiver");}receiver.Avatar=avatars[0];receiver.ReceivePort=9012;receiver.SendPort=9013;receiver.AutoStart=true;
            foreach(var m in module.GetComponentsInChildren<ModularAvatarMergeAnimator>(true)){m.animator=m.transform==module?fx:AssetDatabase.LoadAssetAtPath<AnimatorController>(Mf3DisplayBuild.Root+"/FX.controller");EditorUtility.SetDirty(m);PrefabUtility.RecordPrefabInstancePropertyModifications(m);}
            EditorSceneManager.MarkSceneDirty(scene);EditorSceneManager.SaveScene(scene);AssetDatabase.SaveAssets();Selection.activeGameObject=avatar;
            Directory.CreateDirectory("Docs/Stage4");var result=Newtonsoft.Json.JsonConvert.SerializeObject(new{success=true,scene=scene.path,avatar=avatar.name,sceneAvatarCount=avatars.Length,backup,geometryBits=56,placementBits=14,localGrabBits=0,level="Anchor forward projected to world horizontal; world yaw retained",receiverPort=receiver.ReceivePort},Newtonsoft.Json.Formatting.Indented);File.WriteAllText("Docs/Stage4/upgrade.json",result);return result;
        }
    }
}
