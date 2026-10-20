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
        static bool wide, runtime;
        static int BuildCapacity=Capacity;
        static int Count => wide ? 9 : 6;
        static AnimatorController controller;
        static int clips;
        public static string Build() => BuildCore(3,0);
        public static string BuildWideLane(int lane) => BuildCore(4,lane);
        public static string BuildRuntimeLane(int lane) => BuildCore(5,lane);
        static string BuildCore(int version,int lane)
        {
            wide=version>=4;runtime=version>=5;
            BuildRoot=runtime?"Assets/MorphForge/MF5/Lane"+lane:wide?"Assets/MorphForge/MF4/Lane"+lane:Root;
            BuildPrefix=runtime?"MF5_L"+lane+"_":wide?"MF4_L"+lane+"_":Prefix;
            BuildCapacity=wide?5462:Capacity;
            if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new InvalidOperationException("Requires idle Edit Mode");
            Folder(BuildRoot);Folder(BuildRoot+"/Clips");clips=0;
            controller=AssetDatabase.LoadAssetAtPath<AnimatorController>(BuildRoot+"/FX.controller");
            if(!controller)controller=AnimatorController.CreateAnimatorControllerAtPath(BuildRoot+"/FX.controller");
            else {controller.layers=new AnimatorControllerLayer[0];controller.parameters=new AnimatorControllerParameter[0];foreach(var item in AssetDatabase.LoadAllAssetsAtPath(BuildRoot+"/FX.controller"))if(item&&item!=controller)UnityEngine.Object.DestroyImmediate(item,true);}
            controller.AddParameter(BuildPrefix+"Ctl",AnimatorControllerParameterType.Int);
            for(int i=0;i<Count;i++){controller.AddParameter(BuildPrefix+"D"+i.ToString("D2"),AnimatorControllerParameterType.Int);controller.AddParameter(BuildPrefix+"Q"+i.ToString("D2"),AnimatorControllerParameterType.Float);}
            for(int i=0;i<10;i++)controller.AddParameter(new AnimatorControllerParameter{name=BuildPrefix+"W"+i.ToString("D2"),type=AnimatorControllerParameterType.Float,defaultFloat=i<3?128:(runtime&&i==9?255:(i==6?1:0))});
            for(int i=0;i<4;i++)controller.AddParameter(new AnimatorControllerParameter{name=BuildPrefix+"B"+i,type=AnimatorControllerParameterType.Float,defaultFloat=i==0?1:0});
            foreach(string name in new[]{"LastSeq","LastRxSeq"})controller.AddParameter(new AnimatorControllerParameter{name=BuildPrefix+name,type=AnimatorControllerParameterType.Int,defaultInt=255});
            foreach(string name in new[]{"QSeq","QOp","Op"})controller.AddParameter(BuildPrefix+name,AnimatorControllerParameterType.Int);
            foreach(string name in new[]{"QEraseMode","EraseMode","RxCount","DoneCount"})controller.AddParameter(BuildPrefix+name,AnimatorControllerParameterType.Float);
            foreach(string name in new[]{"InboxReady","Busy"})controller.AddParameter(BuildPrefix+name,AnimatorControllerParameterType.Bool);
            controller.AddParameter(new AnimatorControllerParameter{name=BuildPrefix+"One",type=AnimatorControllerParameterType.Float,defaultFloat=1});
            if(runtime)foreach(var n in new[]{"QAlpha","QPair","Pair","QPool","Pool"})controller.AddParameter(BuildPrefix+n,AnimatorControllerParameterType.Float);
            BuildDataDrive();BuildFlow();EditorUtility.SetDirty(controller);AssetDatabase.SaveAssets();
            var module=PrefabUtility.LoadPrefabContents(ParticleStage2Build.Root+"/Display.prefab");
            try{
                module.name=ModuleName;var store=module.transform.Find("Store").GetComponent<ParticleSystem>();var main=store.main;main.maxParticles=BuildCapacity;main.scalingMode=ParticleSystemScalingMode.Hierarchy;
                // A 25ms pump can die before Birth dispatch during a long frame.
                var pumpMain=module.transform.Find("Minimum/Center/Pump").GetComponent<ParticleSystem>().main;pumpMain.startLifetime=.5f;
                if(runtime) {
                    var firstMain=store.main;firstMain.maxParticles=2731;
                    var second=UnityEngine.Object.Instantiate(module.transform.Find("Minimum").gameObject,module.transform);second.name="Minimum2";second.SetActive(true);
                    var secondStore=UnityEngine.Object.Instantiate(store.gameObject,module.transform);secondStore.name="Store2";var pool=secondStore.GetComponent<ParticleSystem>();var main2=pool.main;main2.maxParticles=2731;main2.customSimulationSpace=module.transform;
                    var trigger2=pool.trigger;trigger2.SetCollider(0,second.transform.Find("Center/Eraser").GetComponent<BoxCollider>());
                    var emitters=second.transform.Find("Center/Pump").GetComponent<ParticleSystem>().subEmitters;
                    while(emitters.subEmittersCount>0)emitters.RemoveSubEmitter(0);
                    emitters.AddSubEmitter(pool,ParticleSystemSubEmitterType.Birth,ParticleSystemSubEmitterProperties.InheritNothing);
                }
                var merge=module.GetComponent<ModularAvatarMergeAnimator>();merge.animator=controller;var parameters=module.GetComponent<ModularAvatarParameters>();parameters.parameters.Clear();
                foreach(string name in new[]{BuildPrefix+"Ctl"}.Concat(Enumerable.Range(0,Count).Select(i=>BuildPrefix+"D"+i.ToString("D2"))))parameters.parameters.Add(new ParameterConfig{nameOrPrefix=name,syncType=ParameterSyncType.Int,localOnly=false,saved=false,hasExplicitDefaultValue=true,defaultValue=0,internalParameter=false});
                PrefabUtility.SaveAsPrefabAsset(module,BuildRoot+"/Display.prefab");
            }finally{PrefabUtility.UnloadPrefabContents(module);}
            AssetDatabase.ImportAsset(BuildRoot+"/Display.prefab",ImportAssetOptions.ForceUpdate);var asset=AssetDatabase.LoadAssetAtPath<GameObject>(BuildRoot+"/Display.prefab");
            var saved=asset.GetComponent<ModularAvatarMergeAnimator>();var so=new SerializedObject(saved);so.FindProperty("animator").objectReferenceValue=controller;so.ApplyModifiedPropertiesWithoutUndo();EditorUtility.SetDirty(saved);PrefabUtility.SavePrefabAsset(asset);AssetDatabase.SaveAssets();
            Directory.CreateDirectory("Docs/Stage4");var result=Newtonsoft.Json.JsonConvert.SerializeObject(new{protocol=runtime?"MF5/80/lane":wide?"MF4/80/lane":"MF3/56",syncedBits=wide?80:56,dataInts=Count,controlBits=new{opcode=runtime?5:4,sequence=runtime?3:4},capacity=BuildCapacity,layers=controller.layers.Length,captureStates=runtime?192:wide?80:96,clips,brush="4 local fields, applied from immutable W payload",prefab=BuildRoot+"/Display.prefab"},Newtonsoft.Json.Formatting.Indented);File.WriteAllText(runtime?"Docs/Stage4/mf5-lane"+lane+"-build.json":wide?"Docs/Stage4/mf4-lane"+lane+"-build.json":"Docs/Stage4/mf3-build.json",result);return result;
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
            if(runtime) {
                bindings.AddRange(bindings.Where(b=>b.path=="Store").Select(b=>Bind("Store2",b.type,b.property,b.min,b.max)).ToArray());
                foreach(char a in "xyz") {
                    bindings.Add(Bind("Minimum2",typeof(Transform),"m_LocalPosition."+a,0,0));
                    bindings.Add(Bind("Minimum2/Center",typeof(Transform),"m_LocalPosition."+a,0,0));
                    bindings.Add(Bind("Minimum2/Center/Eraser",typeof(Transform),"m_LocalScale."+a,0,0));
                }
            }
            dataBindings=bindings.ToArray();
            var direct=new BlendTree{name="Complete vector output",blendType=BlendTreeType.Direct};AssetDatabase.AddObjectToAsset(direct,controller);
            var children=new List<Motion>();
            for(int a=0;a<3;a++)
            {
                string axis="xyz"[a].ToString(),size=a==0?"startSize":a==1?"startSizeY":"startSizeZ";
                children.Add(BasisTree("Position"+axis,BuildPrefix+"W"+a.ToString("D2"),0,255,new[]{Bind("Minimum",typeof(Transform),"m_LocalPosition."+axis,-32,31.75f)}));
                var sizeFields=new[]{Bind("Minimum/Center",typeof(Transform),"m_LocalPosition."+axis,.125f,32),Bind("Store",typeof(ParticleSystem),"InitialModule."+size+".scalar",.25f,64),Bind("Store",typeof(ParticleSystem),"InitialModule."+size+".minScalar",.25f,64)};
                if(runtime)sizeFields=sizeFields.Concat(sizeFields.Select(b=>Bind(b.path=="Store"?"Store2":"Minimum2/Center",b.type,b.property,b.min,b.max))).ToArray();
                Motion sizeMotion=BasisTree("Size"+axis,BuildPrefix+"W"+(a+3).ToString("D2"),0,255,sizeFields);
                if(runtime) {
                    var choose=new BlendTree{name="Pair unit size "+axis,blendType=BlendTreeType.Simple1D,blendParameter=BuildPrefix+"Pair",useAutomaticThresholds=false};
                    choose.AddChild(sizeMotion,0);choose.AddChild(Basis("PairSize"+axis,sizeFields.Select(b=>Bind(b.path,b.type,b.property,b.min,b.min)).ToArray(),false),1);AssetDatabase.AddObjectToAsset(choose,controller);sizeMotion=choose;
                    var pos=new BlendTree{name="Second pool position "+axis,blendType=BlendTreeType.Simple1D,blendParameter=BuildPrefix+"Pair",useAutomaticThresholds=false};
                    pos.AddChild(BasisTree("SecondSinglePosition"+axis,BuildPrefix+"W"+a.ToString("D2"),0,255,new[]{Bind("Minimum2",typeof(Transform),"m_LocalPosition."+axis,-32,31.75f)}),0);
                    pos.AddChild(BasisTree("SecondPairPosition"+axis,BuildPrefix+"W"+(a+3).ToString("D2"),0,255,new[]{Bind("Minimum2",typeof(Transform),"m_LocalPosition."+axis,-32,31.75f)}),1);AssetDatabase.AddObjectToAsset(pos,controller);children.Add(pos);
                }
                children.Add(sizeMotion);
                var eraser=Bind("Minimum/Center/Eraser",typeof(Transform),"m_LocalScale."+axis,.25f,64);
                var select=new BlendTree{name="Eraser select "+axis,blendType=BlendTreeType.Simple1D,blendParameter=BuildPrefix+"EraseMode",useAutomaticThresholds=false};
                var eraseFields=runtime?new[]{eraser,Bind("Minimum2/Center/Eraser",eraser.type,eraser.property,eraser.min,eraser.max)}:new[]{eraser};
                select.AddChild(Basis("EraserPoint"+axis,eraseFields.Select(b=>Bind(b.path,b.type,b.property,.025f,.025f)).ToArray(),false),0);
                select.AddChild(BasisTree("EraserRegion"+axis,BuildPrefix+"W"+(a+3).ToString("D2"),0,255,eraseFields),1);
                select.AddChild(Basis("EraserClear"+axis,eraseFields.Select(b=>Bind(b.path,b.type,b.property,128,128)).ToArray(),false),2);
                AssetDatabase.AddObjectToAsset(select,controller);children.Add(select);
            }
            for(int a=0;a<(wide?3:4);a++)
            {
                string c="rgba"[a].ToString();var colors=new[]{Bind("Store",typeof(ParticleSystem),"InitialModule.startColor.minColor."+c,0,1),Bind("Store",typeof(ParticleSystem),"InitialModule.startColor.maxColor."+c,0,1)};
                if(runtime)colors=colors.Concat(colors.Select(b=>Bind("Store2",b.type,b.property,b.min,b.max))).ToArray();
                children.Add(BasisTree("BirthColor"+c,BuildPrefix+"W"+(a+6).ToString("D2"),0,wide?255:63,colors));
            }
            if(runtime) {
                children.Add(BasisTree("PackedAlpha",BuildPrefix+"W09",0,255,new[]{"Store","Store2"}.SelectMany(path=>new[]{Bind(path,typeof(ParticleSystem),"InitialModule.startColor.minColor.a",0,1),Bind(path,typeof(ParticleSystem),"InitialModule.startColor.maxColor.a",0,1)}).ToArray()));

            }
            else if(wide)children.Add(Basis("PackedAlpha",new[]{
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
        static Motion FlowClip(string name,bool pump,bool trigger,float length)
        {
            if(!runtime)return Clip(name,new[]{Bind("Minimum/Center/Pump",typeof(GameObject),"m_IsActive",pump?1:0,pump?1:0),Bind("Store",typeof(ParticleSystem),"TriggerModule.enabled",trigger?1:0,trigger?1:0)},false,length);
            var tree=new BlendTree{name=name+" pool select",blendType=BlendTreeType.Simple1D,blendParameter=BuildPrefix+"Pool",useAutomaticThresholds=false};
            for(int pool=0;pool<3;pool++) {
                bool first=pool!=1,second=pool!=0;
                var clip=Clip(name+"_Pool"+pool,new[]{Bind("Minimum/Center/Pump",typeof(GameObject),"m_IsActive",pump&&first?1:0,pump&&first?1:0),Bind("Minimum2/Center/Pump",typeof(GameObject),"m_IsActive",pump&&second?1:0,pump&&second?1:0),Bind("Store",typeof(ParticleSystem),"TriggerModule.enabled",trigger&&first?1:0,trigger&&first?1:0),Bind("Store2",typeof(ParticleSystem),"TriggerModule.enabled",trigger&&second?1:0,trigger&&second?1:0)},false,length);tree.AddChild(clip,pool);
            }AssetDatabase.AddObjectToAsset(tree,controller);return tree;
        }
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
            for(int seq=0;seq<(runtime?8:16);seq++)for(int op=1;op<=(runtime?24:wide?5:6);op++)
            {
                var capture=State(inbox,"Capture_"+op+"_"+seq,empty);var d=Driver(capture);
                for(int i=0;i<Count;i++)if(!runtime&&op==2&&i>=3&&i<6)Set(d,"Q"+i.ToString("D2"),0);else Copy(d,"D"+i.ToString("D2"),"Q"+i.ToString("D2"));
                int canonical=runtime?(op<=19?1:op<=21?4:op==22?5:3):op;
                Set(d,"QSeq",seq);Set(d,"QOp",canonical);
                if(runtime){int mode=op<=8?op-1:op<=16?op-9:op<=19?op-17:0;Set(d,"QAlpha",255-mode);Set(d,"QPair",op>=17&&op<=19?1:0);Set(d,"QPool",op>=17&&op<=19||op==22?2:op>=9&&op<=16||op==21||op==24?1:0);}
                Set(d,"QEraseMode",canonical==3?1:canonical==5?2:0);Set(d,"LastRxSeq",seq);Set(d,"InboxReady",1);Increment(d,"RxCount");
                var cap=inbox.AddAnyStateTransition(capture);cap.duration=0;cap.hasExitTime=false;cap.canTransitionToSelf=false;cap.AddCondition(AnimatorConditionMode.Equals,(seq<<(runtime?5:4))|op,BuildPrefix+"Ctl");cap.AddCondition(AnimatorConditionMode.NotEqual,seq,BuildPrefix+"LastRxSeq");cap.AddCondition(AnimatorConditionMode.IfNot,0,BuildPrefix+"InboxReady");Transition(capture,wait,true);
            }
            var sm=Layer("MF3 native particle execution");var idle=State(sm,"Idle",FlowClip("Idle",false,false,.001f));sm.defaultState=idle;
            var latch=State(sm,"Latch",FlowClip("Latch",false,false,.001f));var driver=Driver(latch);for(int i=0;i<Count;i++)Copy(driver,"Q"+i.ToString("D2"),"W"+i.ToString("D2"));
            if(runtime){Copy(driver,"QAlpha","W09");Copy(driver,"QPair","Pair");Copy(driver,"QPool","Pool");}
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
