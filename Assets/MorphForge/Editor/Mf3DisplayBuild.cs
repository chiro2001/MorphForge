using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase;
using nadena.dev.modular_avatar.core;
namespace MorphForge.Editor
{
    public static class Mf3DisplayBuild
    {
        public const string Root="Assets/MorphForge/Stage4/Geometry";
        public const string ModuleName="MorphForgeGeometry";
        public const string Prefix="MF3_";
        public const int DataCount=6;
        public const int Capacity=16384;
        static string BuildRoot=Root, BuildPrefix=Prefix;
        static bool wide;
        static int BuildCapacity=Capacity;
        static int Count => wide ? 9 : 6;
        static AnimatorController controller;
        static int clips;
        public static string Build() => BuildCore(false,0);
        public static string BuildWideLane(int lane) => BuildCore(true,lane);
        static string BuildCore(bool useWide,int lane)
        {
            wide=useWide;
            BuildRoot=wide?"Assets/MorphForge/MF4/Lane"+lane:Root;
            BuildPrefix=wide?"MF4_L"+lane+"_":Prefix;
            BuildCapacity=wide?5462:Capacity;
            if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new InvalidOperationException("Requires idle Edit Mode");
            Folder(BuildRoot);Folder(BuildRoot+"/Clips");clips=0;
            controller=AssetDatabase.LoadAssetAtPath<AnimatorController>(BuildRoot+"/FX.controller");
            if(!controller)controller=AnimatorController.CreateAnimatorControllerAtPath(BuildRoot+"/FX.controller");
            else {controller.layers=new AnimatorControllerLayer[0];controller.parameters=new AnimatorControllerParameter[0];foreach(var item in AssetDatabase.LoadAllAssetsAtPath(BuildRoot+"/FX.controller"))if(item&&item!=controller)UnityEngine.Object.DestroyImmediate(item,true);}
            controller.AddParameter(BuildPrefix+"Ctl",AnimatorControllerParameterType.Int);
            for(int i=0;i<Count;i++){controller.AddParameter(BuildPrefix+"D"+i.ToString("D2"),AnimatorControllerParameterType.Int);controller.AddParameter(BuildPrefix+"Q"+i.ToString("D2"),AnimatorControllerParameterType.Float);}
            for(int i=0;i<10;i++)controller.AddParameter(new AnimatorControllerParameter{name=BuildPrefix+"W"+i.ToString("D2"),type=AnimatorControllerParameterType.Float,defaultFloat=i<3?128:(i==6?1:0)});
            for(int i=0;i<4;i++)controller.AddParameter(new AnimatorControllerParameter{name=BuildPrefix+"B"+i,type=AnimatorControllerParameterType.Float,defaultFloat=i==0?1:0});
            foreach(string name in new[]{"LastSeq","LastRxSeq"})controller.AddParameter(new AnimatorControllerParameter{name=BuildPrefix+name,type=AnimatorControllerParameterType.Int,defaultInt=255});
            foreach(string name in new[]{"QSeq","QOp","Op"})controller.AddParameter(BuildPrefix+name,AnimatorControllerParameterType.Int);
            foreach(string name in new[]{"QEraseMode","EraseMode","RxCount","DoneCount"})controller.AddParameter(BuildPrefix+name,AnimatorControllerParameterType.Float);
            foreach(string name in new[]{"InboxReady","Busy"})controller.AddParameter(BuildPrefix+name,AnimatorControllerParameterType.Bool);
            controller.AddParameter(new AnimatorControllerParameter{name=BuildPrefix+"One",type=AnimatorControllerParameterType.Float,defaultFloat=1});
            BuildDataDrive();BuildFlow();EditorUtility.SetDirty(controller);AssetDatabase.SaveAssets();
            var module=PrefabUtility.LoadPrefabContents(ParticleStage2Build.Root+"/Display.prefab");
            try{
                module.name=ModuleName;var store=module.transform.Find("Store").GetComponent<ParticleSystem>();var main=store.main;main.maxParticles=BuildCapacity;main.scalingMode=ParticleSystemScalingMode.Hierarchy;
                // A 25ms pump can die before Birth dispatch during a long frame.
                var pumpMain=module.transform.Find("Minimum/Center/Pump").GetComponent<ParticleSystem>().main;pumpMain.startLifetime=.5f;
                var merge=module.GetComponent<ModularAvatarMergeAnimator>();merge.animator=controller;var parameters=module.GetComponent<ModularAvatarParameters>();parameters.parameters.Clear();
                foreach(string name in new[]{BuildPrefix+"Ctl"}.Concat(Enumerable.Range(0,Count).Select(i=>BuildPrefix+"D"+i.ToString("D2"))))parameters.parameters.Add(new ParameterConfig{nameOrPrefix=name,syncType=ParameterSyncType.Int,localOnly=false,saved=false,hasExplicitDefaultValue=true,defaultValue=0,internalParameter=false});
                PrefabUtility.SaveAsPrefabAsset(module,BuildRoot+"/Display.prefab");
            }finally{PrefabUtility.UnloadPrefabContents(module);}
            AssetDatabase.ImportAsset(BuildRoot+"/Display.prefab",ImportAssetOptions.ForceUpdate);var asset=AssetDatabase.LoadAssetAtPath<GameObject>(BuildRoot+"/Display.prefab");
            var saved=asset.GetComponent<ModularAvatarMergeAnimator>();var so=new SerializedObject(saved);so.FindProperty("animator").objectReferenceValue=controller;so.ApplyModifiedPropertiesWithoutUndo();EditorUtility.SetDirty(saved);PrefabUtility.SavePrefabAsset(asset);AssetDatabase.SaveAssets();
            Directory.CreateDirectory("Docs/Stage4");var result=Newtonsoft.Json.JsonConvert.SerializeObject(new{protocol=wide?"MF4/80/lane":"MF3/56",syncedBits=wide?80:56,dataInts=Count,controlBits=new{opcode=4,sequence=4},capacity=BuildCapacity,layers=controller.layers.Length,captureStates=wide?80:96,clips,brush="4 local fields, applied from immutable W payload",prefab=BuildRoot+"/Display.prefab"},Newtonsoft.Json.Formatting.Indented);File.WriteAllText(wide?"Docs/Stage4/mf4-lane"+lane+"-build.json":"Docs/Stage4/mf3-build.json",result);return result;
        }
        struct Binding{public string path,property;public Type type;public float min,max;}
        static Binding Bind(string path,Type type,string property,float min,float max){return new Binding{path=path,type=type,property=property,min=min,max=max};}
        static T Asset<T>(T obj,string path) where T:UnityEngine.Object
        {
            var existing=AssetDatabase.LoadAssetAtPath<T>(path);
            if(existing){EditorUtility.CopySerialized(obj,existing);EditorUtility.SetDirty(existing);UnityEngine.Object.DestroyImmediate(obj);return existing;}
            AssetDatabase.CreateAsset(obj,path);return obj;
        }
        static void Folder(string path){string p="Assets";foreach(var n in path.Split('/').Skip(1)){string next=p+"/"+n;if(!AssetDatabase.IsValidFolder(next))AssetDatabase.CreateFolder(p,n);p=next;}}
        static AnimationClip Clip(string name,Binding[] bindings,bool max,float duration=.1f)
        {
            var clip=new AnimationClip{name=name,frameRate=60};
            foreach(var b in bindings)AnimationUtility.SetEditorCurve(clip,EditorCurveBinding.FloatCurve(b.path,b.type,b.property),AnimationCurve.Constant(0,duration,max?b.max:b.min));
            clip=Asset(clip,BuildRoot+"/Clips/"+name+".anim");clips++;return clip;
        }
        static AnimatorStateMachine Layer(string name)
        {
            var sm=new AnimatorStateMachine{name=name};AssetDatabase.AddObjectToAsset(sm,controller);
            controller.AddLayer(new AnimatorControllerLayer{name=name,stateMachine=sm,defaultWeight=1});return sm;
        }
        static BlendTree Tree(string name,string parameter,float min,float max,Binding[] bindings)
        {
            var a=Clip(name+"_Min",bindings,false);var b=Clip(name+"_Max",bindings,true);
            var bt=new BlendTree{name=name,blendType=BlendTreeType.Simple1D,blendParameter=parameter,useAutomaticThresholds=false};
            bt.AddChild(a,min);bt.AddChild(b,max);AssetDatabase.AddObjectToAsset(bt,controller);return bt;
        }
        static AnimatorState State(AnimatorStateMachine sm,string name,Motion motion)
        {
            var s=sm.AddState(name);if(!AssetDatabase.Contains(s))AssetDatabase.AddObjectToAsset(s,controller);
            s.motion=motion;s.writeDefaultValues=false;return s;
        }
        static void Linear(string name,string parameter,float min,float max,Binding[] bindings)
        {var sm=Layer(name);State(sm,"Drive",Tree(name,parameter,min,max,bindings));}
        static Binding[] dataBindings;
        static AnimationClip Basis(string name,Binding[] changed,bool maximum)
        {
            var all=dataBindings.Select(b=>Bind(b.path,b.type,b.property,0,0)).ToArray();
            foreach(var c in changed)
            {
                int i=Array.FindIndex(all,b=>b.path==c.path&&b.type==c.type&&b.property==c.property);
                if(i<0)throw new InvalidOperationException("Unknown data binding "+c.property);
                float v=maximum?c.max:c.min;all[i]=Bind(c.path,c.type,c.property,v,v);
            }
            return Clip(name,all,false);
        }
        static BlendTree BasisTree(string name,string parameter,float min,float max,Binding[] bindings)
        {
            var bt=new BlendTree{name=name,blendType=BlendTreeType.Simple1D,blendParameter=parameter,useAutomaticThresholds=false};
            bt.AddChild(Basis(name+"_Min",bindings,false),min);bt.AddChild(Basis(name+"_Max",bindings,true),max);
            AssetDatabase.AddObjectToAsset(bt,controller);return bt;
        }
        static void BuildDataDrive()
        {
            var bindings=new List<Binding>();
            for(int a=0;a<3;a++)
            {
                string axis="xyz"[a].ToString(),size=a==0?"startSize":a==1?"startSizeY":"startSizeZ";
                bindings.Add(Bind("Minimum",typeof(Transform),"m_LocalPosition."+axis,0,0));
                bindings.Add(Bind("Minimum/Center",typeof(Transform),"m_LocalPosition."+axis,0,0));
                bindings.Add(Bind("Minimum/Center/Eraser",typeof(Transform),"m_LocalScale."+axis,0,0));
                bindings.Add(Bind("Store",typeof(ParticleSystem),"InitialModule."+size+".scalar",0,0));
                bindings.Add(Bind("Store",typeof(ParticleSystem),"InitialModule."+size+".minScalar",0,0));
            }
            foreach(char c in "rgba")foreach(string kind in new[]{"minColor","maxColor"})bindings.Add(Bind("Store",typeof(ParticleSystem),"InitialModule.startColor."+kind+"."+c,0,0));
            dataBindings=bindings.ToArray();
            var direct=new BlendTree{name="Complete vector output",blendType=BlendTreeType.Direct};AssetDatabase.AddObjectToAsset(direct,controller);
            var children=new List<Motion>();
            for(int a=0;a<3;a++)
            {
                string axis="xyz"[a].ToString(),size=a==0?"startSize":a==1?"startSizeY":"startSizeZ";
                children.Add(BasisTree("Position"+axis,BuildPrefix+"W"+a.ToString("D2"),0,255,new[]{Bind("Minimum",typeof(Transform),"m_LocalPosition."+axis,-32,31.75f)}));
                children.Add(BasisTree("Size"+axis,BuildPrefix+"W"+(a+3).ToString("D2"),0,255,new[]{
                    Bind("Minimum/Center",typeof(Transform),"m_LocalPosition."+axis,.125f,32),
                    Bind("Store",typeof(ParticleSystem),"InitialModule."+size+".scalar",.25f,64),
                    Bind("Store",typeof(ParticleSystem),"InitialModule."+size+".minScalar",.25f,64)}));
                var eraser=Bind("Minimum/Center/Eraser",typeof(Transform),"m_LocalScale."+axis,.25f,64);
                var select=new BlendTree{name="Eraser select "+axis,blendType=BlendTreeType.Simple1D,blendParameter=BuildPrefix+"EraseMode",useAutomaticThresholds=false};
                select.AddChild(Basis("EraserPoint"+axis,new[]{Bind(eraser.path,eraser.type,eraser.property,.025f,.025f)},false),0);
                select.AddChild(BasisTree("EraserRegion"+axis,BuildPrefix+"W"+(a+3).ToString("D2"),0,255,new[]{eraser}),1);
                select.AddChild(Basis("EraserClear"+axis,new[]{Bind(eraser.path,eraser.type,eraser.property,128,128)},false),2);
                AssetDatabase.AddObjectToAsset(select,controller);children.Add(select);
            }
            for(int a=0;a<(wide?3:4);a++)
            {
                string c="rgba"[a].ToString();children.Add(BasisTree("BirthColor"+c,BuildPrefix+"W"+(a+6).ToString("D2"),0,wide?255:63,new[]{
                    Bind("Store",typeof(ParticleSystem),"InitialModule.startColor.minColor."+c,0,1),
                    Bind("Store",typeof(ParticleSystem),"InitialModule.startColor.maxColor."+c,0,1)}));
            }
            if(wide)children.Add(Basis("PackedAlpha",new[]{
                Bind("Store",typeof(ParticleSystem),"InitialModule.startColor.minColor.a",1,1),
                Bind("Store",typeof(ParticleSystem),"InitialModule.startColor.maxColor.a",1,1)},false));
            foreach(var child in children)direct.AddChild(child);
            var arr=direct.children;for(int i=0;i<arr.Length;i++)arr[i].directBlendParameter=BuildPrefix+"One";direct.children=arr;
            var serialized=new SerializedObject(direct);var normalized=serialized.FindProperty("m_NormalizedBlendValues");if(normalized!=null){normalized.boolValue=false;serialized.ApplyModifiedPropertiesWithoutUndo();}
            var sm=Layer("Shared complete data output");var state=State(sm,"Drive",direct);state.writeDefaultValues=true;
        }
        static AnimatorStateTransition Transition(AnimatorState a,AnimatorState b,bool exit=false)
        {var t=a.AddTransition(b);t.duration=0;t.hasExitTime=exit;t.exitTime=1;t.canTransitionToSelf=false;return t;}
        static void EraserLayer(string axis,int field)
        {
            var sm=Layer("EraseSize"+axis);var binding=Bind("Minimum/Center/Eraser",typeof(Transform),"m_LocalScale."+axis,.25f,64);
            var point=State(sm,"Point",Clip("EraserPoint"+axis,new[]{Bind(binding.path,binding.type,binding.property,.025f,.025f)},false));
            var region=State(sm,"Region",Tree("EraserRegion"+axis,BuildPrefix+"W"+field.ToString("D2"),0,255,new[]{binding}));
            var clear=State(sm,"Clear",Clip("EraserClear"+axis,new[]{Bind(binding.path,binding.type,binding.property,128,128)},false));
            var t=Transition(point,region);t.AddCondition(AnimatorConditionMode.Equals,2,BuildPrefix+"Op");
            t=Transition(point,clear);t.AddCondition(AnimatorConditionMode.Equals,4,BuildPrefix+"Op");
            t=Transition(region,point);t.AddCondition(AnimatorConditionMode.NotEqual,2,BuildPrefix+"Op");
            t=Transition(clear,point);t.AddCondition(AnimatorConditionMode.NotEqual,4,BuildPrefix+"Op");
        }
        static AnimationClip FlowClip(string name,bool pump,bool trigger,float length)
        {return Clip(name,new[]{Bind("Minimum/Center/Pump",typeof(GameObject),"m_IsActive",pump?1:0,pump?1:0),Bind("Store",typeof(ParticleSystem),"TriggerModule.enabled",trigger?1:0,trigger?1:0)},false,length);}
        static VRCAvatarParameterDriver Driver(AnimatorState state)
        {
            var d=state.AddStateMachineBehaviour<VRCAvatarParameterDriver>();
            if(!d)throw new InvalidOperationException("Cannot attach native Parameter Driver to "+state.name);
            d.localOnly=false;return d;
        }
        static void Set(VRCAvatarParameterDriver d,string n,float v){d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=BuildPrefix+n,type=VRC_AvatarParameterDriver.ChangeType.Set,value=v});}
        static void Copy(VRCAvatarParameterDriver d,string source,string dest){d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=BuildPrefix+dest,source=BuildPrefix+source,type=VRC_AvatarParameterDriver.ChangeType.Copy});}
        static void Increment(VRCAvatarParameterDriver d,string name)
        {d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=BuildPrefix+name,type=VRC_AvatarParameterDriver.ChangeType.Add,value=1});}
        static void BuildFlow()
        {
            var inbox=Layer("MF3 single commit inbox");var empty=Clip("InboxEmpty",new Binding[0],false,.001f);var wait=State(inbox,"Wait",empty);inbox.defaultState=wait;
            for(int seq=0;seq<16;seq++)for(int op=1;op<=(wide?5:6);op++)
            {
                var capture=State(inbox,"Capture_"+op+"_"+seq,empty);var d=Driver(capture);
                for(int i=0;i<Count;i++)if(op==2&&i>=3&&i<6)Set(d,"Q"+i.ToString("D2"),0);else Copy(d,"D"+i.ToString("D2"),"Q"+i.ToString("D2"));
                Set(d,"QSeq",seq);Set(d,"QOp",op);Set(d,"QEraseMode",op==3?1:op==5?2:0);Set(d,"LastRxSeq",seq);Set(d,"InboxReady",1);Increment(d,"RxCount");
                var cap=inbox.AddAnyStateTransition(capture);cap.duration=0;cap.hasExitTime=false;cap.canTransitionToSelf=false;cap.AddCondition(AnimatorConditionMode.Equals,(seq<<4)|op,BuildPrefix+"Ctl");cap.AddCondition(AnimatorConditionMode.NotEqual,seq,BuildPrefix+"LastRxSeq");cap.AddCondition(AnimatorConditionMode.IfNot,0,BuildPrefix+"InboxReady");Transition(capture,wait,true);
            }
            var sm=Layer("MF3 native particle execution");var idle=State(sm,"Idle",FlowClip("Idle",false,false,.001f));sm.defaultState=idle;
            var latch=State(sm,"Latch",FlowClip("Latch",false,false,.001f));var driver=Driver(latch);for(int i=0;i<Count;i++)Copy(driver,"Q"+i.ToString("D2"),"W"+i.ToString("D2"));
            if(!wide)for(int i=0;i<4;i++)Copy(driver,"B"+i,"W"+(i+6).ToString("D2"));
            Copy(driver,"QSeq","LastSeq");Copy(driver,"QOp","Op");Copy(driver,"QEraseMode","EraseMode");Set(driver,"Busy",1);Set(driver,"InboxReady",0);
            var brush=State(sm,"Apply shared brush",FlowClip("Brush",false,false,.001f));var brushDriver=Driver(brush);for(int i=0;i<4;i++){Copy(brushDriver,"W"+i.ToString("D2"),"B"+i);Copy(brushDriver,"W"+i.ToString("D2"),"W"+(i+6).ToString("D2"));}
            var settle=State(sm,"Settle",FlowClip("Settle",false,false,.03f));var t=Transition(latch,brush,true);t.AddCondition(AnimatorConditionMode.Equals,6,BuildPrefix+"Op");t=Transition(latch,settle,true);t.AddCondition(AnimatorConditionMode.NotEqual,6,BuildPrefix+"Op");
            var erase=State(sm,"Erase",FlowClip("Erase",false,true,.001f));Transition(settle,erase,true);var release=State(sm,"Release",FlowClip("Release",false,false,.001f));var fire=State(sm,"Fire",FlowClip("Fire",true,false,.03f));
            var done=State(sm,"Done",FlowClip("Done",false,false,.001f));var finish=Driver(done);Set(finish,"Busy",0);Increment(finish,"DoneCount");Transition(brush,done,true);Transition(erase,release,true);
            foreach(int op in new[]{1,2}){t=Transition(release,fire,true);t.AddCondition(AnimatorConditionMode.Equals,op,BuildPrefix+"Op");}
            t=Transition(release,done,true);t.AddCondition(AnimatorConditionMode.Greater,2,BuildPrefix+"Op");Transition(fire,done,true);Transition(done,idle,true);
            var execute=sm.AddAnyStateTransition(latch);execute.duration=0;execute.hasExitTime=false;execute.canTransitionToSelf=false;execute.AddCondition(AnimatorConditionMode.If,0,BuildPrefix+"InboxReady");execute.AddCondition(AnimatorConditionMode.IfNot,0,BuildPrefix+"Busy");
        }
    }
}
