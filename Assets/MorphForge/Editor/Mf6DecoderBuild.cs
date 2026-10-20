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
    // Fixed layouts only: byte lookups + bounded routing, never enumerate payloads.
    public static class Mf6DecoderBuild
    {
        public const string Root=Mf6NativeBuild.Root;
        static AnimatorController fx;
        static AnimationClip empty;
        static HashSet<string> declared;
        static string N(string name)=>"MF6_"+name;
        static void Set(VRCAvatarParameterDriver d,string name,float value)=>d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=name,type=VRC_AvatarParameterDriver.ChangeType.Set,value=value});
        static void Copy(VRCAvatarParameterDriver d,string source,string name)=>d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=name,source=source,type=VRC_AvatarParameterDriver.ChangeType.Copy});
        static void Add(VRCAvatarParameterDriver d,string name,float value)=>d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=name,type=VRC_AvatarParameterDriver.ChangeType.Add,value=value});
        static VRCAvatarParameterDriver Driver(AnimatorState state){var d=state.AddStateMachineBehaviour<VRCAvatarParameterDriver>();d.localOnly=false;return d;}
        static AnimatorStateMachine Layer(string name){var sm=new AnimatorStateMachine{name=name};AssetDatabase.AddObjectToAsset(sm,fx);fx.AddLayer(new AnimatorControllerLayer{name=name,stateMachine=sm,defaultWeight=1});return sm;}
        static AnimatorState State(AnimatorStateMachine sm,string name,Motion motion=null){var s=sm.AddState(name);s.motion=motion??empty;s.writeDefaultValues=false;return s;}
        static AnimatorStateTransition To(AnimatorState a,AnimatorState b,bool exit=false){var t=a.AddTransition(b);t.duration=0;t.hasExitTime=exit;t.exitTime=1;t.canTransitionToSelf=false;return t;}
        static void Eq(AnimatorStateTransition t,string name,int value)=>t.AddCondition(AnimatorConditionMode.Equals,value,N(name));
        static void Greater(AnimatorStateTransition t,string name,int value)=>t.AddCondition(AnimatorConditionMode.Greater,value,N(name));
        static void IntParam(string name,int value=0){if(declared.Add(N(name)))fx.AddParameter(new AnimatorControllerParameter{name=N(name),type=AnimatorControllerParameterType.Int,defaultInt=value});}
        static void FloatParam(string name){if(declared.Add(N(name)))fx.AddParameter(N(name),AnimatorControllerParameterType.Float);}
        static void I(VRCAvatarParameterDriver d,string name,int value){IntParam(name);Set(d,N(name),value);}
        static void F(VRCAvatarParameterDriver d,string name,float value){FloatParam(name);Set(d,N(name),value);}
        static string Q(int i)=>N("Q"+i.ToString("D2"));
        static string D(int i)=>N("D"+i.ToString("D2"));
        static void ByteLookup(string name,string parameter,Action<VRCAvatarParameterDriver,int> decode)
        {
            var sm=Layer(name);var idle=State(sm,"Wait");sm.defaultState=idle;
            for(int value=0;value<256;value++){
                var state=State(sm,"Byte"+value);decode(Driver(state),value);
                var enter=sm.AddAnyStateTransition(state);enter.duration=0;enter.hasExitTime=false;enter.canTransitionToSelf=false;Eq(enter,"Stage",1);enter.AddCondition(AnimatorConditionMode.Equals,value,parameter);
                var leave=To(state,idle);leave.AddCondition(AnimatorConditionMode.NotEqual,1,N("Stage"));
            }
        }
        static void Inbox()
        {
            var sm=Layer("MF6 240bit atomic local snapshot");var idle=State(sm,"Wait");sm.defaultState=idle;
            for(int seq=0;seq<16;seq++)for(int format=0;format<16;format++){
                var state=State(sm,"Capture"+seq+"_"+format);var d=Driver(state);
                for(int i=0;i<29;i++)Copy(d,D(i),Q(i));I(d,"Seq",seq);I(d,"Format",format);I(d,"LastRx",seq);
                int lo=format==0?3:format==8?24:format==9?16:format==6||format==7||format==10?0:27;
                int hi=format==0?28:format==8?25:format==6||format==7||format==10?1:28;
                Copy(d,D(lo),N("MetaLo"));Copy(d,D(hi),N("MetaHi"));I(d,"Stage",1);
                var t=sm.AddAnyStateTransition(state);t.duration=0;t.hasExitTime=false;t.canTransitionToSelf=false;Eq(t,"Ctl",seq*16+format);if(format==0){t.AddCondition(AnimatorConditionMode.Greater,167,D(28));t.AddCondition(AnimatorConditionMode.Less,176,D(28));}Eq(t,"Stage",0);t.AddCondition(AnimatorConditionMode.NotEqual,seq,N("LastRx"));To(state,idle,true);
            }
        }
        static void Metadata()
        {
            ByteLookup("MF6 low byte",N("MetaLo"),(d,v)=>{
                I(d,"RegionShape",v&3);I(d,"RegionCount",((v>>2)&7)+1);F(d,"RegionAlpha",255-(v>>5));
                I(d,"Pool0",v&7);I(d,"Pool1",(v>>3)&7);I(d,"Pool2Low",(v>>6)&3);
                for(int i=0;i<8;i++)I(d,"Mask"+i,(v>>i)&1);
                for(int i=0;i<4;i++){I(d,"TriPool"+i,(v+i)&7);F(d,"TriX"+i,(v+i)&255);I(d,"TriCarry"+i,(v+i)>>8);}
            });
            ByteLookup("MF6 high byte",N("MetaHi"),(d,v)=>{
                I(d,"RegionStart",v&7);
                I(d,"Pool2High",v&1);I(d,"Pool3",(v>>1)&7);I(d,"CountRaw",((v>>1)&3)+1);I(d,"CountShared",((v>>4)&3)+1);
                F(d,"Alpha",255-((v>>3)&7));F(d,"VoxelAlpha",255-v);F(d,"TriZ",v&63);F(d,"TriAlpha",(v&64)!=0?2:1);
            });
            ByteLookup("MF6 second triangle ID low",Q(14),(d,v)=>{I(d,"RawTriPool1",v&7);});
            ByteLookup("MF6 second triangle ID high",Q(15),(d,v)=>{F(d,"RawTriZ1",v&63);F(d,"RawTriAlpha1",(v&64)!=0?2:1);});
        }
        static AnimatorStateTransition Route(AnimatorStateMachine sm,AnimatorState idle,string name,int format,int count,string countParameter,Action<VRCAvatarParameterDriver> build)
        {
            var state=State(sm,name);build(Driver(state));var enter=sm.AddAnyStateTransition(state);enter.duration=0;enter.hasExitTime=false;enter.canTransitionToSelf=false;Eq(enter,"Stage",2);Eq(enter,"Format",format);
            if(countParameter!=null)Greater(enter,countParameter,count);var leave=To(state,idle);leave.AddCondition(AnimatorConditionMode.NotEqual,2,N("Stage"));return enter;
        }
        static string P(int pool,int field)=>"MF6_P"+pool+"_D"+field.ToString("D2");
        static void Reset(VRCAvatarParameterDriver d,int pool){for(int i=0;i<Mf6NativeBuild.Fields;i++)Set(d,P(pool,i),0);}
        static void Commit(VRCAvatarParameterDriver d,int pool,int op)
        {
            // A sparse pool can be idle for exactly sixteen global frames.
            // Its local sequence must advance per dispatch, not mirror Seq.
            d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name="MF6_P"+pool+"_Ctl",source=N("NextSeq"+pool),type=VRC_AvatarParameterDriver.ChangeType.Copy,convertRange=true,sourceMin=0,sourceMax=15,destMin=op,destMax=120+op});
            Add(d,N("NextSeq"+pool),1);
        }
        static void PoolCondition(AnimatorStateTransition t,int record,int pool)
        {
            if(record==2){Eq(t,"Pool2Low",pool&3);Eq(t,"Pool2High",pool>>2);}
            else Eq(t,"Pool"+record,pool);
        }
        static void Box(VRCAvatarParameterDriver d,int pool,int offset,int color,string alpha,int alphaOffset=0,bool erase=false)
        {
            Reset(d,pool);for(int i=0;i<6;i++)Copy(d,Q(offset+i),P(pool,i));
            if(color>=0)for(int i=0;i<3;i++)Copy(d,Q(color+i),P(pool,6+i));
            if(alpha!=null)Copy(d,N(alpha),P(pool,9));else Set(d,P(pool,9),255);
            if(alphaOffset!=0)Add(d,P(pool,9),alphaOffset);Commit(d,pool,erase?3:1);
        }
        static void TriangleBase(VRCAvatarParameterDriver d,int pool,int idLow,string z,int carry,string alpha,int alphaOffset=0)
        {
            Reset(d,pool);Copy(d,Q(idLow),P(pool,0));Copy(d,N(z),P(pool,2));if(carry!=0)Add(d,P(pool,2),carry);Copy(d,N(alpha),P(pool,9));if(alphaOffset!=0)Add(d,P(pool,9),alphaOffset);
        }
        static void Routers()
        {
            for(int pool=0;pool<8;pool++){
                int p=pool;var sm=Layer("MF6 route to native pool "+p);var idle=State(sm,"Wait");sm.defaultState=idle;
                // Region packets have an absolute origin. Every record maps
                // to a distinct worker; no persistent decode cursor or ACK.
                for(int shape=0;shape<4;shape++)for(int i=0;i<8;i++){
                    int index=i,kind=shape;
                    var t=Route(sm,idle,"Region"+shape+"_"+i,0,i,"RegionCount",d=>{
                        Reset(d,p);for(int a=0;a<3;a++)Copy(d,Q(a),P(p,a));
                        if(kind<3){if(index!=0)Add(d,P(p,kind),index);}
                        else for(int a=0;a<3;a++){int delta=(index>>a)&1;if(delta!=0)Add(d,P(p,a),delta);}
                        for(int a=0;a<3;a++)Copy(d,Q(4+index*3+a),P(p,6+a));Copy(d,N("RegionAlpha"),P(p,9));Commit(d,p,1);
                    });Eq(t,"RegionShape",shape);Eq(t,"RegionStart",(p-i+8)%8);
                }
                foreach(int format in new[]{1,11,12,13,14})for(int i=0;i<3;i++){
                    int record=i,f=format;
                    var t=Route(sm,idle,"Box"+f+"_"+i,f,i,"CountRaw",d=>Box(d,p,record*9,record*9+6,"Alpha",f==1?0:-8*(f-10)));
                    PoolCondition(t,i,p);
                }
                for(int format=2;format<=4;format++)for(int i=0;i<4;i++){
                    int record=i,f=format;var t=Route(sm,idle,"Shared"+f+"_"+i,f,i,"CountShared",d=>Box(d,p,record*6,24,null,-(f-2)));PoolCondition(t,i,p);
                }
                var voxel=Route(sm,idle,"Unit voxel",5,0,null,d=>{Reset(d,p);for(int a=0;a<3;a++){Copy(d,Q(p*3+a),P(p,a));Copy(d,Q(24+a),P(p,6+a));}Copy(d,N("VoxelAlpha"),P(p,9));Commit(d,p,1);});Eq(voxel,"Mask"+p,1);
                for(int i=0;i<4;i++){
                    int record=i;var t=Route(sm,idle,"Remove box "+i,8,i,"CountShared",d=>Box(d,p,record*6,-1,null,0,true));PoolCondition(t,i,p);
                }
                for(int i=0;i<2;i++){
                    int record=i,offset=i*14;var t=Route(sm,idle,"Independent triangle "+i,6,i,"Q28",d=>{
                        TriangleBase(d,p,offset,record==0?"TriZ":"RawTriZ1",0,record==0?"TriAlpha":"RawTriAlpha1");
                        foreach(var pair in new[]{new[]{3,offset+2},new[]{10,offset+5},new[]{13,offset+8},new[]{6,offset+11}})for(int a=0;a<3;a++)Copy(d,Q(pair[1]+a),P(p,pair[0]+a));Commit(d,p,2);
                    });Eq(t,i==0?"TriPool0":"RawTriPool1",p);
                }
                foreach(int format in new[]{7,10})for(int i=0;i<(format==7?3:4);i++)for(int carry=0;carry<2;carry++){
                    int record=i,f=format,overflow=carry;var t=Route(sm,idle,"Strip"+f+"_"+i+"_"+carry,f,i,"Q26",d=>{
                        TriangleBase(d,p,0,"TriZ",overflow,"TriAlpha",f==10?(record%2==0?2:4):0);Copy(d,N("TriX"+record),P(p,0));
                        if(f==7){int[] vertices=record%2==0?new[]{record,record+1,record+2}:new[]{record+1,record,record+2};int[] targets={3,10,13};for(int vertex=0;vertex<3;vertex++)for(int a=0;a<3;a++)Copy(d,Q(2+vertices[vertex]*3+a),P(p,targets[vertex]+a));for(int a=0;a<3;a++)Copy(d,Q(17+record*3+a),P(p,6+a));}
                        else{int start=5+record*3/2;for(int a=0;a<3;a++)Copy(d,Q(start+a),P(p,3+a));for(int a=0;a<2;a++)Copy(d,Q(start+3+a),P(p,10+a));Copy(d,Q(2),P(p,12));Copy(d,Q(3),P(p,13));Copy(d,Q(4),P(p,14));for(int a=0;a<3;a++)Copy(d,Q(14+record*3+a),P(p,6+a));}
                        Commit(d,p,2);
                    });Eq(t,"TriPool"+i,p);Eq(t,"TriCarry"+i,carry);
                }
                var remove=Route(sm,idle,"Remove triangle",9,0,null,d=>{Reset(d,p);Copy(d,Q(p*2),P(p,0));Copy(d,Q(p*2+1),P(p,2));Set(d,P(p,9),2);Commit(d,p,4);});Eq(remove,"Mask"+p,1);
                Route(sm,idle,"Clear",15,0,null,d=>{Reset(d,p);for(int a=0;a<3;a++)Set(d,P(p,a),128);Commit(d,p,5);});
            }
        }
        static AnimationClip WaitClip(string name,float duration){var c=new AnimationClip{name=name,frameRate=60};AnimationUtility.SetEditorCurve(c,EditorCurveBinding.FloatCurve("",typeof(Transform),"m_LocalScale.x"),AnimationCurve.Constant(0,duration,1));AssetDatabase.AddObjectToAsset(c,fx);return c;}
        static void Clock()
        {
            var sm=Layer("MF6 decode route release clock");var idle=State(sm,"Idle");sm.defaultState=idle;var decode=State(sm,"Decode settle",WaitClip("DecodeSettle",.03f));var route=State(sm,"Route settle",WaitClip("RouteSettle",.03f));I(Driver(route),"Stage",2);var done=State(sm,"Release snapshot");I(Driver(done),"Stage",0);
            var t=sm.AddAnyStateTransition(decode);t.duration=0;t.hasExitTime=false;t.canTransitionToSelf=false;Eq(t,"Stage",1);To(decode,route,true);To(route,done,true);To(done,idle,true);
        }
        static void SequenceWrap()
        {
            for(int pool=0;pool<8;pool++){
                var sm=Layer("MF6 local sequence wrap "+pool);var idle=State(sm,"Wait");sm.defaultState=idle;var reset=State(sm,"Wrap");I(Driver(reset),"NextSeq"+pool,0);
                var t=sm.AddAnyStateTransition(reset);t.duration=0;t.hasExitTime=false;t.canTransitionToSelf=false;Greater(t,"NextSeq"+pool,15);To(reset,idle,true);
            }
        }
        public static string Build()
        {
            if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");
            if(!AssetDatabase.IsValidFolder(Root))AssetDatabase.CreateFolder("Assets/MorphForge","MF6");var path=Root+"/Decoder.controller";
            fx=AssetDatabase.LoadAssetAtPath<AnimatorController>(path);if(!fx)fx=AnimatorController.CreateAnimatorControllerAtPath(path);else{fx.layers=new AnimatorControllerLayer[0];fx.parameters=new AnimatorControllerParameter[0];foreach(var asset in AssetDatabase.LoadAllAssetsAtPath(path))if(asset&&asset!=fx)UnityEngine.Object.DestroyImmediate(asset,true);}
            declared=new HashSet<string>();empty=new AnimationClip{name="MF6 empty",frameRate=60};AssetDatabase.AddObjectToAsset(empty,fx);
            foreach(string name in new[]{"Ctl","Seq","Format","Stage","MetaLo","MetaHi"})IntParam(name,name=="Ctl"?255:0);IntParam("LastRx",255);for(int i=0;i<29;i++){IntParam("D"+i.ToString("D2"));IntParam("Q"+i.ToString("D2"));}
            for(int p=0;p<8;p++){fx.AddParameter("MF6_P"+p+"_Ctl",AnimatorControllerParameterType.Int);for(int i=0;i<Mf6NativeBuild.Fields;i++)fx.AddParameter("MF6_P"+p+"_D"+i.ToString("D2"),AnimatorControllerParameterType.Float);}
            for(int p=0;p<8;p++)IntParam("NextSeq"+p);
            Inbox();Metadata();Routers();Clock();SequenceWrap();var root=new GameObject("MF6 shared decoder");try{
                var merge=root.AddComponent<ModularAvatarMergeAnimator>();merge.animator=fx;merge.layerType=VRCAvatarDescriptor.AnimLayerType.FX;merge.pathMode=MergeAnimatorPathMode.Relative;merge.matchAvatarWriteDefaults=false;
                var parameters=root.AddComponent<ModularAvatarParameters>();foreach(string name in new[]{N("Ctl")}.Concat(Enumerable.Range(0,29).Select(i=>D(i))))parameters.parameters.Add(new ParameterConfig{nameOrPrefix=name,syncType=ParameterSyncType.Int,localOnly=false,saved=false,hasExplicitDefaultValue=true,defaultValue=name==N("Ctl")?255:0,internalParameter=false});
                PrefabUtility.SaveAsPrefabAsset(root,Root+"/Decoder.prefab");
            }finally{UnityEngine.Object.DestroyImmediate(root);}
            EditorUtility.SetDirty(fx);AssetDatabase.SaveAssets();return Newtonsoft.Json.JsonConvert.SerializeObject(new{success=true,layers=fx.layers.Length,states=fx.layers.Sum(l=>l.stateMachine.states.Length),parameters=fx.parameters.Length,syncedBits=240});
        }
    }
}
