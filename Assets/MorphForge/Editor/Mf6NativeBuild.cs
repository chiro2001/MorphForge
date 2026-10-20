using System;
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
    // Local workers consume immutable normalized records from the shared MF6
    // decoder. They add no synced expression parameters. No runtime C# needed.
    public static class Mf6NativeBuild
    {
        public const string Root="Assets/MorphForge/MF6";
        public const int Fields=20;
        static AnimatorController fx;
        static string prefix,path;
        struct Binding {public string path,property;public Type type;public float min,max;}
        static Binding B(string p,Type t,string field,float a,float b)=>new Binding{path=p,type=t,property=field,min=a,max=b};
        static Binding[] bindings;
        static int clipId;
        static Dictionary<Motion,(bool emit,bool erase)> flows;
        static void Folder(string p){string current="Assets";foreach(var part in p.Split('/').Skip(1)){var next=current+"/"+part;if(!AssetDatabase.IsValidFolder(next))AssetDatabase.CreateFolder(current,part);current=next;}}
        static T Save<T>(T value,string p)where T:UnityEngine.Object{var old=AssetDatabase.LoadAssetAtPath<T>(p);if(old){EditorUtility.CopySerialized(value,old);UnityEngine.Object.DestroyImmediate(value);EditorUtility.SetDirty(old);return old;}AssetDatabase.CreateAsset(value,p);return value;}
        static AnimationClip Clip(string name,IEnumerable<Binding> values,bool maximum=false,float duration=.001f)
        {
            var c=new AnimationClip{name=name,frameRate=60};foreach(var b in values)AnimationUtility.SetEditorCurve(c,EditorCurveBinding.FloatCurve(b.path,b.type,b.property),AnimationCurve.Constant(0,duration,maximum?b.max:b.min));
            AssetDatabase.AddObjectToAsset(c,fx);clipId++;return c;
        }
        static AnimationClip Basis(string name,Binding[] values,bool maximum)
        {
            var all=bindings.Select(v=>B(v.path,v.type,v.property,0,0)).ToArray();
            foreach(var b in values){int index=Array.FindIndex(all,v=>v.path==b.path&&v.type==b.type&&v.property==b.property);if(index<0)throw new Exception("Unknown binding "+b.property);float value=maximum?b.max:b.min;all[index]=B(b.path,b.type,b.property,value,value);}
            return Clip(name,all);
        }
        static BlendTree Linear(string name,string parameter,Binding[] values,float maximum=255)
        {
            var t=new BlendTree{name=name,blendType=BlendTreeType.Simple1D,blendParameter=prefix+parameter,useAutomaticThresholds=false};
            AssetDatabase.AddObjectToAsset(t,fx);t.AddChild(Basis(name+"Min",values,false),0);t.AddChild(Basis(name+"Max",values,true),maximum);return t;
        }
        static BlendTree Direct(string name,List<Motion> motions)
        {
            var tree=new BlendTree{name=name,blendType=BlendTreeType.Direct};AssetDatabase.AddObjectToAsset(tree,fx);
            foreach(var m in motions)tree.AddChild(m);var children=tree.children;for(int i=0;i<children.Length;i++)children[i].directBlendParameter=prefix+"One";tree.children=children;
            var so=new SerializedObject(tree);so.FindProperty("m_NormalizedBlendValues").boolValue=false;so.ApplyModifiedPropertiesWithoutUndo();return tree;
        }
        static AnimatorStateMachine Layer(string name){var sm=new AnimatorStateMachine{name=name};AssetDatabase.AddObjectToAsset(sm,fx);fx.AddLayer(new AnimatorControllerLayer{name=name,stateMachine=sm,defaultWeight=1});return sm;}
        static AnimatorState State(AnimatorStateMachine sm,string name,Motion motion){var s=sm.AddState(name);s.motion=motion;s.writeDefaultValues=false;if(flows!=null&&flows.TryGetValue(motion,out var phase)){var d=Driver(s);Set(d,"Emit",phase.emit?1:0);Set(d,"EraseEnable",phase.erase?1:0);}return s;}
        static AnimatorStateTransition To(AnimatorState a,AnimatorState b,bool exit=true){var t=a.AddTransition(b);t.duration=0;t.hasExitTime=exit;t.exitTime=1;t.canTransitionToSelf=false;return t;}
        static VRCAvatarParameterDriver Driver(AnimatorState s){var d=s.AddStateMachineBehaviour<VRCAvatarParameterDriver>();d.localOnly=false;return d;}
        static void Set(VRCAvatarParameterDriver d,string n,float v){d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=prefix+n,type=VRC_AvatarParameterDriver.ChangeType.Set,value=v});}
        static void Copy(VRCAvatarParameterDriver d,string a,string b){d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=prefix+b,source=prefix+a,type=VRC_AvatarParameterDriver.ChangeType.Copy});}
        static Motion Flow(string name,bool pump,bool erase,float duration=.001f)
        {
            // Phase booleans and payload W fields go through the same parameter
            // driver timing. Driving Trigger directly from an earlier layer can
            // erase at the preceding command's center while W is still pending.
            var clock=Clip(name+" clock",new[]{B("Store",typeof(ParticleSystemRenderer),"m_Enabled",1,1)},false,duration);
            flows.Add(clock,(pump,erase));return clock;
        }
        static void Data()
        {
            var list=new List<Binding>();
            foreach(char a in "xyz"){
                list.Add(B("Minimum",typeof(Transform),"m_LocalPosition."+a,0,0));list.Add(B("Minimum/Center",typeof(Transform),"m_LocalPosition."+a,0,0));list.Add(B("Minimum/Center/Eraser",typeof(Transform),"m_LocalScale."+a,0,0));
                string size=a=='x'?"startSize":a=='y'?"startSizeY":"startSizeZ";foreach(string scalar in new[]{"scalar","minScalar"})list.Add(B("Store",typeof(ParticleSystem),"InitialModule."+size+"."+scalar,0,0));
            }
            foreach(char c in "rgba")foreach(string field in new[]{"minColor","maxColor"})list.Add(B("Store",typeof(ParticleSystem),"InitialModule.startColor."+field+"."+c,0,0));bindings=list.ToArray();
            foreach(string field in new[]{"scalar","minScalar"})list.Add(B("Store",typeof(ParticleSystem),"InitialModule.startLifetime."+field,0,0));bindings=list.ToArray();
            var modes=new List<Motion>();
            foreach(bool triangle in new[]{false,true}){
                var motions=new List<Motion>();
                for(int a=0;a<3;a++){
                    string axis="xyz"[a].ToString(),size=a==0?"startSize":a==1?"startSizeY":"startSizeZ";
                    if(triangle&&a==1)motions.Add(Basis("TriIdHeight",new[]{B("Minimum",typeof(Transform),"m_LocalPosition.y",-128,-128)},false));
                    else motions.Add(Linear((triangle?"Tri":"Box")+"Position"+axis,"W"+a.ToString("D2"),new[]{B("Minimum",typeof(Transform),"m_LocalPosition."+axis,-32,31.75f)}));
                    var sizes=new[]{B("Store",typeof(ParticleSystem),"InitialModule."+size+".scalar",triangle?1:.25f,triangle?1:64),B("Store",typeof(ParticleSystem),"InitialModule."+size+".minScalar",triangle?1:.25f,triangle?1:64)};
                    if(!triangle){motions.Add(Linear("BoxSize"+axis,"W"+(a+3).ToString("D2"),sizes.Concat(new[]{B("Minimum/Center",typeof(Transform),"m_LocalPosition."+axis,.125f,32)}).ToArray()));}
                    else{
                        int start=a==0?3:a==1?10:13;
                        var selector=new BlendTree{name="Exact float24 exponent "+axis,blendType=BlendTreeType.Simple1D,blendParameter=prefix+"W"+(start+2).ToString("D2"),useAutomaticThresholds=false};AssetDatabase.AddObjectToAsset(selector,fx);
                        foreach(bool upper in new[]{false,true}){
                            var terms=new List<Motion>();terms.Add(Basis("TriSizeBase"+axis+upper,sizes.Select(v=>B(v.path,v.type,v.property,upper?0:1,upper?0:1)).ToArray(),false));
                            for(int component=0;component<3;component++){
                                float weight=(component==0?1:component==1?256:65536)/(upper?4194304f:8388608f);
                                terms.Add(Linear("TriVertex"+a+"Part"+component+upper,"W"+(start+component).ToString("D2"),sizes.Select(v=>B(v.path,v.type,v.property,0,255*weight)).ToArray()));
                            }
                            selector.AddChild(Direct("Tri packed "+axis+upper,terms),upper?128:127);
                        }
                        motions.Add(selector);
                    }
                }
                var life=new[]{B("Store",typeof(ParticleSystem),"InitialModule.startLifetime.scalar",triangle?524288:1000000,triangle?524288:1000000),B("Store",typeof(ParticleSystem),"InitialModule.startLifetime.minScalar",triangle?524288:1000000,triangle?524288:1000000)};
                motions.Add(Basis((triangle?"Tri":"Box")+"LifetimeBase",life,false));
                if(triangle){motions.Add(Linear("Tri ID low lifetime","W00",life.Select(v=>B(v.path,v.type,v.property,0,255*8)).ToArray()));motions.Add(Linear("Tri ID high lifetime","W02",life.Select(v=>B(v.path,v.type,v.property,0,255*2048)).ToArray()));}
                foreach(int a in Enumerable.Range(0,4)){
                    string c="rgba"[a].ToString();int field=a<3?a+6:9;
                    motions.Add(Linear((triangle?"Tri":"Box")+"Color"+c,"W"+field.ToString("D2"),new[]{B("Store",typeof(ParticleSystem),"InitialModule.startColor.minColor."+c,0,1),B("Store",typeof(ParticleSystem),"InitialModule.startColor.maxColor."+c,0,1)}));
                }
                motions.Add(Linear((triangle?"Tri":"Box")+"Eraser","Clear", "xyz".Select(a=>B("Minimum/Center/Eraser",typeof(Transform),"m_LocalScale."+a,.025f,1024)).ToArray(),1));
                modes.Add(Direct(triangle?"Triangle data":"Box data",motions));
            }
            var sentinel=new List<Binding>();
            foreach(char axis in "xyz"){
                sentinel.Add(B("Minimum",typeof(Transform),"m_LocalPosition."+axis,0,0));
                sentinel.Add(B("Minimum/Center/Eraser",typeof(Transform),"m_LocalScale."+axis,.025f,.025f));
                string size=axis=='x'?"startSize":axis=='y'?"startSizeY":"startSizeZ";foreach(string scalar in new[]{"scalar","minScalar"})sentinel.Add(B("Store",typeof(ParticleSystem),"InitialModule."+size+"."+scalar,.25f,.25f));
            }
            foreach(string scalar in new[]{"scalar","minScalar"})sentinel.Add(B("Store",typeof(ParticleSystem),"InitialModule.startLifetime."+scalar,1000000,1000000));
            var select=new BlendTree{name="Geometry type",blendType=BlendTreeType.Simple1D,blendParameter=prefix+"Kind",useAutomaticThresholds=false};AssetDatabase.AddObjectToAsset(select,fx);select.AddChild(modes[0],0);select.AddChild(modes[1],1);select.AddChild(Basis("Invisible bounds carrier",sentinel.ToArray(),false),2);
            var outputLayer=Layer("MF6 immutable data and phase output");
            AnimatorState Phase(string name,bool regular,bool bounds,bool erase){
                // Only the phase leaf binds booleans. Geometry leaves bind zero
                // data for additive direct blending; they must not bind false
                // to the phase booleans and compete with the active emitter.
                var values=bindings.Select(v=>B(v.path,v.type,v.property,0,0)).Concat(new[]{
                    B("Minimum/Center/Pump",typeof(GameObject),"m_IsActive",regular?1:0,regular?1:0),
                    B("BoundsPump",typeof(GameObject),"m_IsActive",bounds?1:0,bounds?1:0),
                    B("Store",typeof(ParticleSystem),"TriggerModule.enabled",erase?1:0,erase?1:0)});
                var phase=Clip("Phase "+name,values);var output=Direct("Atomic "+name,new List<Motion>{select,phase});
                var state=State(outputLayer,name,output);state.writeDefaultValues=true;return state;
            }
            var wait=Phase("Wait",false,false,false);outputLayer.defaultState=wait;
            var erasing=Phase("Erase",false,false,true);var regularState=Phase("Regular birth",true,false,false);var boundsState=Phase("Bounds birth",false,true,false);
            AnimatorStateTransition Enter(AnimatorState state){var t=outputLayer.AddAnyStateTransition(state);t.duration=0;t.hasExitTime=false;t.canTransitionToSelf=false;return t;}
            var idleTransition=Enter(wait);idleTransition.AddCondition(AnimatorConditionMode.Less,.5f,prefix+"Emit");idleTransition.AddCondition(AnimatorConditionMode.Less,.5f,prefix+"EraseEnable");
            var eraseTransition=Enter(erasing);eraseTransition.AddCondition(AnimatorConditionMode.Greater,.5f,prefix+"EraseEnable");
            var emitTransition=Enter(regularState);emitTransition.AddCondition(AnimatorConditionMode.Greater,.5f,prefix+"Emit");emitTransition.AddCondition(AnimatorConditionMode.Less,1.5f,prefix+"Kind");
            var boundsTransition=Enter(boundsState);boundsTransition.AddCondition(AnimatorConditionMode.Greater,.5f,prefix+"Emit");boundsTransition.AddCondition(AnimatorConditionMode.Greater,1.5f,prefix+"Kind");

        }
        static void Lifecycle()
        {
            var sm=Layer("MF6 local inbox");var empty=Clip("Inbox",new Binding[0]);var wait=State(sm,"Wait",empty);sm.defaultState=wait;
            for(int seq=0;seq<16;seq++)for(int op=1;op<=5;op++){
                var capture=State(sm,"Capture"+seq+"_"+op,empty);var d=Driver(capture);
                for(int i=0;i<Fields;i++)Copy(d,"D"+i.ToString("D2"),"Q"+i.ToString("D2"));
                Set(d,"QKind",op==2||op==4?1:0);Set(d,"QClear",op==5?1:0);Set(d,"QOp",op);Set(d,"LastRx",seq);Set(d,"Ready",1);
                var t=sm.AddAnyStateTransition(capture);t.duration=0;t.hasExitTime=false;t.canTransitionToSelf=false;t.AddCondition(AnimatorConditionMode.Equals,seq*8+op,prefix+"Ctl");t.AddCondition(AnimatorConditionMode.NotEqual,seq,prefix+"LastRx");t.AddCondition(AnimatorConditionMode.IfNot,0,prefix+"Ready");To(capture,wait);
            }
            sm=Layer("MF6 native lifecycle");var idle=State(sm,"Idle",Flow("Idle",false,false));
            var latch=State(sm,"Latch",Flow("Latch",false,false));var driver=Driver(latch);for(int i=0;i<Fields;i++)Copy(driver,"Q"+i.ToString("D2"),"W"+i.ToString("D2"));Copy(driver,"QKind","Kind");Copy(driver,"QClear","Clear");Copy(driver,"QOp","Op");Set(driver,"Ready",0);Set(driver,"Busy",1);
            var settle=State(sm,"Settle",Flow("Settle",false,false,.03f));var erase=State(sm,"Erase",Flow("Erase",false,true));var release=State(sm,"Release",Flow("Release",false,false));var fire=State(sm,"Birth",Flow("Birth",true,false,.03f));var done=State(sm,"Done",Flow("Done",false,false));
            var finish=Driver(done);Set(finish,"Busy",0);finish.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=prefix+"DoneCount",type=VRC_AvatarParameterDriver.ChangeType.Add,value=1});
            var carrier=State(sm,"Prepare bounds carrier",Flow("CarrierPrepare",false,false));var carrierDriver=Driver(carrier);Set(carrierDriver,"Kind",2);Set(carrierDriver,"Clear",0);Set(carrierDriver,"Busy",1);
            var carrierSettle=State(sm,"Bounds carrier settle",Flow("CarrierSettle",false,false,.03f));To(carrier,carrierSettle);To(carrierSettle,fire);sm.defaultState=carrier;
            To(latch,settle);To(settle,erase);To(erase,release);foreach(int op in new[]{1,2}){var t=To(release,fire);t.AddCondition(AnimatorConditionMode.Equals,op,prefix+"Op");}
            var rebuild=To(release,carrier);rebuild.AddCondition(AnimatorConditionMode.Equals,5,prefix+"Op");
            var skip=To(release,done);skip.AddCondition(AnimatorConditionMode.Greater,2,prefix+"Op");skip.AddCondition(AnimatorConditionMode.NotEqual,5,prefix+"Op");To(fire,done);To(done,idle);
            var enter=sm.AddAnyStateTransition(latch);enter.duration=0;enter.hasExitTime=false;enter.canTransitionToSelf=false;enter.AddCondition(AnimatorConditionMode.If,0,prefix+"Ready");enter.AddCondition(AnimatorConditionMode.IfNot,0,prefix+"Busy");
        }
        public static string BuildWorker(int pool,string materialSource=null)
        {
            if(EditorApplication.isPlaying||EditorApplication.isCompiling||pool<0||pool>7)throw new Exception("Idle Edit Mode and pool0..7 required");
            Folder(Root);path=Root+"/Pool"+pool;Folder(path);prefix="MF6_P"+pool+"_";clipId=0;flows=new Dictionary<Motion,(bool,bool)>();
            fx=AssetDatabase.LoadAssetAtPath<AnimatorController>(path+"/FX.controller");if(!fx)fx=AnimatorController.CreateAnimatorControllerAtPath(path+"/FX.controller");
            else{fx.layers=new AnimatorControllerLayer[0];fx.parameters=new AnimatorControllerParameter[0];foreach(var item in AssetDatabase.LoadAllAssetsAtPath(path+"/FX.controller"))if(item&&item!=fx)UnityEngine.Object.DestroyImmediate(item,true);}
            fx.AddParameter(prefix+"Ctl",AnimatorControllerParameterType.Int);foreach(string n in new[]{"QOp","Op","LastRx"})fx.AddParameter(new AnimatorControllerParameter{name=prefix+n,type=AnimatorControllerParameterType.Int,defaultInt=n=="LastRx"?255:0});
            foreach(string n in new[]{"Ready","Busy"})fx.AddParameter(new AnimatorControllerParameter{name=prefix+n,type=AnimatorControllerParameterType.Bool,defaultBool=n=="Busy"});
            foreach(string n in new[]{"QKind","Kind","QClear","Clear","DoneCount","Emit","EraseEnable"})fx.AddParameter(prefix+n,AnimatorControllerParameterType.Float);
            fx.AddParameter(new AnimatorControllerParameter{name=prefix+"One",type=AnimatorControllerParameterType.Float,defaultFloat=1});
            for(int i=0;i<Fields;i++)foreach(string n in new[]{"D","Q","W"})fx.AddParameter(prefix+n+i.ToString("D2"),AnimatorControllerParameterType.Float);
            // Latch payload and phase first, then sample both in one final
            // output. Direct trees retain WD on and bind every driven field.
            Lifecycle();Data();
            var module=PrefabUtility.LoadPrefabContents(ParticleStage2Build.Root+"/Display.prefab");
            try{
                module.name="Pool"+pool;var store=module.transform.Find("Store").GetComponent<ParticleSystem>();var main=store.main;main.maxParticles=2056;main.startLifetime=1000000;main.scalingMode=ParticleSystemScalingMode.Hierarchy;main.customSimulationSpace=module.transform;
                var trigger=store.trigger;trigger.radiusScale=0;
                var pump=module.transform.Find("Minimum/Center/Pump").GetComponent<ParticleSystem>().main;pump.startLifetime=.5f;
                // Eight tiny hidden corners keep shader-displaced geometry in
                // the native renderer bounds, including close-up/rotated views.
                // This emitter and the regular pump are never active together.
                var shapeMesh=new Mesh{name="MF6 bounds corners"};shapeMesh.vertices=Enumerable.Range(0,8).Select(i=>new Vector3((i&1)==0?-40:40,(i&2)==0?-40:40,(i&4)==0?-40:40)).ToArray();shapeMesh.triangles=new[]{0,2,1,1,2,3,4,5,6,5,7,6,0,1,4,1,5,4,2,6,3,3,6,7,0,4,2,2,4,6,1,3,5,3,7,5};shapeMesh.RecalculateNormals();shapeMesh.RecalculateBounds();shapeMesh=Save(shapeMesh,path+"/BoundsCorners.asset");
                var boundsObject=new GameObject("BoundsPump",typeof(ParticleSystem));boundsObject.transform.SetParent(module.transform,false);var boundsSystem=boundsObject.GetComponent<ParticleSystem>();boundsSystem.Stop(true,ParticleSystemStopBehavior.StopEmittingAndClear);
                var bm=boundsSystem.main;bm.playOnAwake=true;bm.loop=false;bm.duration=.1f;bm.startLifetime=.5f;bm.startSpeed=0;bm.maxParticles=8;bm.cullingMode=ParticleSystemCullingMode.AlwaysSimulate;bm.scalingMode=ParticleSystemScalingMode.Hierarchy;
                var be=boundsSystem.emission;be.enabled=true;be.rateOverTime=0;be.rateOverDistance=0;be.SetBursts(new[]{new ParticleSystem.Burst(0,8)});
                var bs=boundsSystem.shape;bs.enabled=true;bs.shapeType=ParticleSystemShapeType.Mesh;bs.mesh=shapeMesh;bs.meshShapeType=ParticleSystemMeshShapeType.Vertex;bs.meshSpawnMode=ParticleSystemShapeMultiModeValue.BurstSpread;bs.meshSpawnSpread=1;
                boundsSystem.GetComponent<ParticleSystemRenderer>().enabled=false;var sub=boundsSystem.subEmitters;sub.enabled=true;sub.AddSubEmitter(store,ParticleSystemSubEmitterType.Birth,ParticleSystemSubEmitterProperties.InheritNothing);boundsObject.SetActive(false);
                var renderer=store.GetComponent<ParticleSystemRenderer>();
                var temporary=GameObject.CreatePrimitive(PrimitiveType.Cube);var mesh=UnityEngine.Object.Instantiate(temporary.GetComponent<MeshFilter>().sharedMesh);UnityEngine.Object.DestroyImmediate(temporary);mesh.name="MF6 cube / triangle carrier";var uv2=new Vector2[mesh.vertexCount];var uv=new Vector2[mesh.vertexCount];var tangents=new Vector4[mesh.vertexCount];var vertices=mesh.vertices;var normals=mesh.normals;var indices=mesh.triangles;var first=indices.Take(3).ToArray();var next=indices.Skip(3).Take(3).ToArray();int shared=first.First(v=>next.Contains(v));
                for(int i=0;i<uv2.Length;i++){
                    var v=vertices[i];var n=normals[i];int axis=Mathf.Abs(n.x)>.5f?0:Mathf.Abs(n.y)>.5f?1:2;int cube=(v.x>0?1:0)|(v.y>0?2:0)|(v.z>0?4:0);int corner=Array.IndexOf(first,i);if(corner<0&&next.Contains(i))corner=Array.IndexOf(first,shared);uv2[i]=new Vector2(cube+axis*8,corner);
                    uv[i]=axis==0?new Vector2(v.z+.5f,v.y+.5f):axis==1?new Vector2(v.x+.5f,v.z+.5f):new Vector2(v.x+.5f,v.y+.5f);
                    var tangent=axis==0?Vector3.forward:Vector3.right;float sign=axis==0?n.x:axis==1?n.y:n.z;tangents[i]=new Vector4(tangent.x,tangent.y,tangent.z,axis==2?sign:-sign);
                }
                mesh.uv=uv;mesh.uv2=uv2;mesh.tangents=tangents;mesh.RecalculateBounds();renderer.mesh=Save(mesh,path+"/Carrier.asset");
                renderer.SetActiveVertexStreams(new List<ParticleSystemVertexStream>{ParticleSystemVertexStream.Position,ParticleSystemVertexStream.Normal,ParticleSystemVertexStream.Tangent,ParticleSystemVertexStream.Color,ParticleSystemVertexStream.UV,ParticleSystemVertexStream.UV2,ParticleSystemVertexStream.SizeXYZ,ParticleSystemVertexStream.Center,ParticleSystemVertexStream.InvStartLifetime});
                var material=UnityEngine.Object.Instantiate(AssetDatabase.LoadAssetAtPath<Material>(materialSource??"Assets/MorphForge/Diagnostics/MF5Catalog/ParticleBoxCatalog.mat"));material.name="MF6 catalogue and triangles";material.shader=Shader.Find("MorphForge/ParticleBoxCatalogMetadataGrid");material.SetFloat("_WireEncoding",6);renderer.sharedMaterial=Save(material,path+"/Surface.mat");
                module.GetComponent<ModularAvatarMergeAnimator>().animator=fx;module.GetComponent<ModularAvatarParameters>().parameters.Clear();
                PrefabUtility.SaveAsPrefabAsset(module,path+"/Display.prefab");
            }finally{PrefabUtility.UnloadPrefabContents(module);}
            EditorUtility.SetDirty(fx);AssetDatabase.SaveAssets();return Newtonsoft.Json.JsonConvert.SerializeObject(new{success=true,pool,layers=fx.layers.Length,parameters=fx.parameters.Length,clips=clipId,prefab=path+"/Display.prefab",syncedBits=0});
        }
    }
}
