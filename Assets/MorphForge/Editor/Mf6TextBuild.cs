using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using UnityEngine;
using UnityEditor;
using UnityEditor.Animations;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase;
using nadena.dev.modular_avatar.core;
using MorphForgeAvatar.Editor;

namespace MorphForge.Editor {
    // Per-renderer retained properties: no shared GPU feedback buffer and no
    // per-character state machine. A page is active only while being written.
    public static class Mf6TextBuild {
        public const string Root="Assets/MorphForgeAvatar/Generated/Text";
        public const int Panels=4, Slots=128;
        static AnimatorController fx;
        static AnimationClip empty;
        struct Binding {public string path,property;public float value;}
        static string T(int i)=>"MF6_T"+i.ToString("D2");
        static Binding B(int slot,string property,float value)=>new Binding{path="Panel"+slot,property="material."+property,value=value};
        static AnimationClip Clip(string name,IEnumerable<Binding> bindings) {
            var c=new AnimationClip{name=name,frameRate=60};
            foreach(var b in bindings)AnimationUtility.SetEditorCurve(c,EditorCurveBinding.FloatCurve(b.path,typeof(MeshRenderer),b.property),AnimationCurve.Constant(0,.02f,b.value));
            AssetDatabase.AddObjectToAsset(c,fx);return c;
        }
        static BlendTree Linear(string name,string parameter,AnimationClip zero,AnimationClip max) {
            var t=new BlendTree{name=name,blendType=BlendTreeType.Simple1D,blendParameter=parameter,useAutomaticThresholds=false};AssetDatabase.AddObjectToAsset(t,fx);t.AddChild(zero,0);t.AddChild(max,255);return t;
        }
        static BlendTree Direct(string name,IEnumerable<Motion> motions) {
            var t=new BlendTree{name=name,blendType=BlendTreeType.Direct};AssetDatabase.AddObjectToAsset(t,fx);foreach(var m in motions)t.AddChild(m);
            var children=t.children;for(int i=0;i<children.Length;i++)children[i].directBlendParameter="MF6_TextOne";t.children=children;
            var so=new SerializedObject(t);so.FindProperty("m_NormalizedBlendValues").boolValue=false;so.ApplyModifiedPropertiesWithoutUndo();return t;
        }
        struct Term {public string property;public int field;public float multiplier,constant;}
        static Term Byte(string p,int field,float multiplier=1)=>new Term{property=p,field=field,multiplier=multiplier};
        static Term Fixed(string p,float value)=>new Term{property=p,field=-1,constant=value};
        static IEnumerable<Term> Word(string p,int low,float scale=1)=>new[]{Byte(p,low,scale),Byte(p,low+1,scale*256)};
        static string Cache(int slot,int field)=>"MF6_Text"+slot+"_B"+field.ToString("D3");
        static string Count(int slot)=>"MF6_Text"+slot+"_Count";
        static string Gate(int slot,int page)=>"MF6_Text"+slot+"_Page"+page;
        static Motion Map(int slot,string name,IEnumerable<Term> input) {
            var terms=input.ToArray();var motions=terms.Select(term=>(Motion)Clip(name+" "+term.property+" "+term.field,new[]{B(slot,term.property,term.field<0?term.constant:term.multiplier)})).ToArray();
            var result=Direct(name,motions);var children=result.children;
            // Verified unnormalized Direct weights can exceed one. Each cached
            // byte multiplies a constant coefficient, replacing a two-leaf 1D
            // lookup. This tree is continuously sampled with Write Defaults On.
            for(int i=0;i<children.Length;i++)children[i].directBlendParameter=terms[i].field<0?"MF6_TextOne":Cache(slot,terms[i].field);
            result.children=children;return result;
        }
        static Motion Output(int slot) {
            var config=new List<Term>();
            for(int a=0;a<3;a++){string axis="XYZ"[a].ToString();config.AddRange(Word("_Pos"+axis,256+a*2,1f/64));config.Add(Fixed("_Pos"+axis,-512));config.AddRange(Word("_Rot"+axis,262+a*2,360f/65536));}
            config.AddRange(Word("_Width",268,1f/64));config.AddRange(Word("_Height",270,1f/64));config.AddRange(Word("_FontSize",272,1f/64));config.AddRange(new[]{Byte("_Red",274),Byte("_Green",275),Byte("_Blue",276),Byte("_Opacity",277)});
            config.AddRange(new[]{Byte("_BackR",288),Byte("_BackG",289),Byte("_BackB",290),Byte("_BackA",291),Byte("_Font",292),Byte("_Align",293),Byte("_Vertical",294)});config.AddRange(Word("_Spacing",295,1f/64));config.AddRange(Word("_Gap",297,1f/64));config.Add(Byte("_MotionGroup",299));
            var baseline=config.Select(t=>t.property).Concat(new[]{"_Count"}).Concat(Enumerable.Range(0,Slots).Select(i=>"_G"+i.ToString("D3"))).Distinct().Select(p=>B(slot,p,0));
            var motions=new List<Motion>{Clip("Zero output baseline "+slot,baseline),Map(slot,"Config output "+slot,config)};var weights=new List<string>{"MF6_TextOne",Gate(slot,0)};
            for(int page=0;page<11;page++){var terms=new List<Term>();for(int i=0;i<12&&page*12+i<Slots;i++)terms.AddRange(Word("_G"+(page*12+i).ToString("D3"),(page*12+i)*2));motions.Add(Map(slot,"Glyph output "+slot+"/"+page,terms));weights.Add(Gate(slot,page));}
            var count=Clip("Count coefficient "+slot,new[]{B(slot,"_Count",1)});motions.Add(count);weights.Add(Count(slot));
            var result=Direct("Cached output "+slot,motions);var children=result.children;for(int i=0;i<children.Length;i++)children[i].directBlendParameter=weights[i];result.children=children;return result;
        }
        static void CopyCache(VRCAvatarParameterDriver driver,int slot,int source,int target)=>DriverCopy(driver,T(source),Cache(slot,target));
        static void UpdateCount(VRCAvatarParameterDriver driver,int slot,int field,bool snapshot=false){
            driver.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=Count(slot),source=T(field),type=VRC_AvatarParameterDriver.ChangeType.Copy,convertRange=snapshot,sourceMin=128,sourceMax=142,destMin=0,destMax=14});
            for(int page=0;page<11;page++)driver.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=Gate(slot,page),source=T(field),type=VRC_AvatarParameterDriver.ChangeType.Copy,convertRange=true,sourceMin=(snapshot?128:0)+page*12,sourceMax=(snapshot?128:0)+page*12+1,destMin=0,destMax=1});
        }
        static AnimatorState State(AnimatorStateMachine sm,string name,Motion motion){var s=sm.AddState(name);s.motion=motion;s.writeDefaultValues=true;return s;}
        static void Condition(AnimatorStateTransition t,string p,int value)=>t.AddCondition(AnimatorConditionMode.Equals,value,p);
        static AnimatorStateTransition Enter(AnimatorStateMachine sm,AnimatorState state) {var t=sm.AddAnyStateTransition(state);t.hasExitTime=false;t.duration=0;t.canTransitionToSelf=false;Condition(t,"MF6_Stage",2);Condition(t,"MF6_Format",0);return t;}
        static void Leave(AnimatorState state,AnimatorState idle){var t=state.AddTransition(idle);t.hasExitTime=false;t.duration=0;t.AddCondition(AnimatorConditionMode.NotEqual,2,"MF6_Stage");}
        static void DriverSet(VRCAvatarParameterDriver d,string n,float v)=>d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=n,type=VRC_AvatarParameterDriver.ChangeType.Set,value=v});
        static void DriverCopy(VRCAvatarParameterDriver d,string source,string dest)=>d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=dest,source=source,type=VRC_AvatarParameterDriver.ChangeType.Copy});
        static void SnapshotEpochs(AnimatorController decoder) {
            if(!decoder.parameters.Any(p=>p.name=="MF6_TextEpoch"))decoder.AddParameter("MF6_TextEpoch",AnimatorControllerParameterType.Int);
            var inbox=decoder.layers.Single(l=>l.name=="MF6 240bit atomic local snapshot").stateMachine;
            foreach(var state in inbox.states.Select(s=>s.state).Where(s=>s.name.StartsWith("Text snapshot "))) {
                int code=int.Parse(state.name.Substring("Text snapshot ".Length));var driver=state.behaviours.OfType<VRCAvatarParameterDriver>().Single();
                driver.parameters.RemoveAll(p=>p.name=="MF6_TextSlot"||p.name=="MF6_TextEpoch"||p.name=="MF6_Stage");DriverSet(driver,"MF6_TextSlot",code&3);DriverSet(driver,"MF6_TextEpoch",code>>2);DriverSet(driver,"MF6_Stage",1);
                foreach(var transition in state.transitions){transition.hasExitTime=false;transition.duration=0;transition.conditions=new[]{new AnimatorCondition{mode=AnimatorConditionMode.NotEqual,parameter="MF6_Stage",threshold=1}};EditorUtility.SetDirty(transition);}
            }
            EditorUtility.SetDirty(decoder);
        }
        public static void ExtendInbox(AnimatorController decoder) {
            if(decoder.parameters.Any(p=>p.name=="MF6_TextSnapshot")){SnapshotEpochs(decoder);return;}
            decoder.AddParameter("MF6_TextSnapshot",AnimatorControllerParameterType.Int);decoder.AddParameter("MF6_TextSlot",AnimatorControllerParameterType.Int);
            for(int i=0;i<29;i++)decoder.AddParameter(T(i),AnimatorControllerParameterType.Float);
            var sm=decoder.layers.Single(l=>l.name=="MF6 240bit atomic local snapshot").stateMachine;
            var idle=sm.defaultState;
            foreach(var state in sm.states.Select(s=>s.state).Where(s=>s.name.StartsWith("Capture"))) {
                var driver=state.behaviours.OfType<VRCAvatarParameterDriver>().Single();DriverSet(driver,"MF6_TextSnapshot",0);
            }
            // Region8 also uses format zero. Its routers must reject extension
            // and short snapshots even if stale region metadata looks valid.
            foreach(var layer in decoder.layers.Where(l=>l.name.StartsWith("MF6 route to native pool ")))
                foreach(var t in layer.stateMachine.anyStateTransitions.Where(t=>t.destinationState&&t.destinationState.name.StartsWith("Region"))) {
                    t.AddCondition(AnimatorConditionMode.Greater,167,"MF6_Q28");t.AddCondition(AnimatorConditionMode.Less,176,"MF6_Q28");
                }
            foreach(bool snapshot in new[]{false,true})for(int seq=0;seq<16;seq++) {
                var state=State(sm,(snapshot?"Text snapshot ":"Extension ")+seq,idle.motion);var driver=state.AddStateMachineBehaviour<VRCAvatarParameterDriver>();driver.localOnly=false;
                for(int i=0;i<29;i++){DriverCopy(driver,"MF6_D"+i.ToString("D2"),"MF6_Q"+i.ToString("D2"));DriverCopy(driver,"MF6_D"+i.ToString("D2"),T(i));}
                DriverSet(driver,"MF6_Format",0);DriverSet(driver,"MF6_TextSnapshot",snapshot?1:0);DriverSet(driver,"MF6_TextSlot",seq);DriverSet(driver,"MF6_Stage",1);
                if(!snapshot){DriverSet(driver,"MF6_Seq",seq);DriverSet(driver,"MF6_LastRx",seq);}
                var t=sm.AddAnyStateTransition(state);t.duration=0;t.hasExitTime=false;t.canTransitionToSelf=false;Condition(t,"MF6_Ctl",seq*16);Condition(t,"MF6_Stage",0);
                if(snapshot){t.AddCondition(AnimatorConditionMode.Greater,127,"MF6_D28");t.AddCondition(AnimatorConditionMode.Less,143,"MF6_D28");}
                else{Condition(t,"MF6_D28",254);t.AddCondition(AnimatorConditionMode.NotEqual,seq,"MF6_LastRx");}
                var leave=state.AddTransition(idle);leave.hasExitTime=true;leave.exitTime=1;leave.duration=0;
            }
            SnapshotEpochs(decoder);EditorUtility.SetDirty(decoder);
        }
        static Texture2D Font(string name,int width,int height) {
            string path=Root+"/"+name+".asset";var existing=AssetDatabase.LoadAssetAtPath<Texture2D>(path);if(existing)return existing;
            var t=new Texture2D(width,height,TextureFormat.R8,false,true){name=name,filterMode=FilterMode.Point,wrapMode=TextureWrapMode.Clamp};
            string font=name.StartsWith("jp")?"jp":"zh";string file=name.EndsWith("widths")?font+".widths.bin":font+".bin";
            t.LoadRawTextureData(File.ReadAllBytes("fonts/pixel16/"+file));t.Apply(false,true);AssetDatabase.CreateAsset(t,path);return t;
        }
        static Mesh Mesh() {
            var p=new List<Vector3>();var uv=new List<Vector2>();var index=new List<Vector2>();var triangles=new List<int>();
            foreach(int i in Enumerable.Range(0,Slots+1).Select(n=>n==0?Slots:n-1)) {
                int start=p.Count;foreach(var v in new[]{new Vector2(0,0),new Vector2(1,0),new Vector2(1,1),new Vector2(0,1)}){p.Add(new Vector3(v.x,v.y,0));uv.Add(v);index.Add(new Vector2(i,0));}
                triangles.AddRange(new[]{start,start+2,start+1,start,start+3,start+2});
            }
            var mesh=new Mesh{name="TextMesh"};mesh.SetVertices(p);mesh.SetUVs(0,uv);mesh.SetUVs(1,index);mesh.SetTriangles(triangles,0);mesh.bounds=new Bounds(Vector3.zero,Vector3.one*160);
            string path=Root+"/TextMesh.asset";var existing=AssetDatabase.LoadAssetAtPath<Mesh>(path);if(existing){EditorUtility.CopySerialized(mesh,existing);UnityEngine.Object.DestroyImmediate(mesh);return existing;}AssetDatabase.CreateAsset(mesh,path);return mesh;
        }
        public static void BuildAssets() {
            if(EditorApplication.isPlaying)throw new Exception("Edit Mode required");CatalogVariant.EnsureFolder(Root);
            var path=Root+"/Text.controller";fx=AssetDatabase.LoadAssetAtPath<AnimatorController>(path);
            if(!fx)fx=AnimatorController.CreateAnimatorControllerAtPath(path);
            else {fx.layers=new AnimatorControllerLayer[0];fx.parameters=new AnimatorControllerParameter[0];foreach(var a in AssetDatabase.LoadAllAssetsAtPath(path))if(a&&a!=fx)UnityEngine.Object.DestroyImmediate(a,true);}
            empty=new AnimationClip{name="Text buffer control tick",frameRate=60};AnimationUtility.SetEditorCurve(empty,EditorCurveBinding.FloatCurve("",typeof(Transform),"m_LocalScale.x"),AnimationCurve.Constant(0,.02f,1));AssetDatabase.AddObjectToAsset(empty,fx);
            foreach(string n in new[]{"MF6_Stage","MF6_Format","MF6_TextSnapshot","MF6_TextSlot","MF6_TextEpoch","MF6_Q00","MF6_Q01","MF6_Q02","MF6_Q14","MF6_Q25","MF6_Q28"})fx.AddParameter(n,AnimatorControllerParameterType.Int);
            for(int i=0;i<29;i++)fx.AddParameter(T(i),AnimatorControllerParameterType.Float);
            fx.AddParameter(new AnimatorControllerParameter{name="MF6_TextOne",type=AnimatorControllerParameterType.Float,defaultFloat=1});
            // Assign the local cache in one batch. Repeated AddParameter calls
            // make a thousands-entry controller quadratic to build/import.
            var cacheParameters=fx.parameters.ToList();
            for(int slot=0;slot<Panels;slot++){
                for(int field=0;field<300;field++)cacheParameters.Add(new AnimatorControllerParameter{name=Cache(slot,field),type=AnimatorControllerParameterType.Float});
                cacheParameters.Add(new AnimatorControllerParameter{name=Count(slot),type=AnimatorControllerParameterType.Float});
                cacheParameters.Add(new AnimatorControllerParameter{name="MF6_Text"+slot+"_Epoch",type=AnimatorControllerParameterType.Int});
                for(int page=0;page<11;page++)cacheParameters.Add(new AnimatorControllerParameter{name=Gate(slot,page),type=AnimatorControllerParameterType.Float});
            }
            fx.parameters=cacheParameters.ToArray();
            var root=new GameObject("TextDisplay");try {
                var mesh=Mesh();var zh=Font("zh",2048,1024);var jp=Font("jp",2048,1024);var zw=Font("zh-widths",256,256);var jw=Font("jp-widths",256,256);
                for(int slot=0;slot<Panels;slot++) {
                    var obj=new GameObject("Panel"+slot,typeof(MeshFilter),typeof(MeshRenderer));obj.transform.SetParent(root.transform,false);obj.GetComponent<MeshFilter>().sharedMesh=mesh;
                    var material=new Material(Shader.Find("MorphForgeAvatar/PixelText16")){name="Panel"+slot};material.SetTexture("_FontZH",zh);material.SetTexture("_FontJP",jp);material.SetTexture("_WidthsZH",zw);material.SetTexture("_WidthsJP",jw);
                    foreach(string property in new[]{"_PosX","_PosY","_PosZ","_RotX","_RotY","_RotZ","_Width","_Height","_FontSize","_Spacing","_Gap","_Align","_Vertical","_Font","_Count","_Red","_Green","_Blue","_Opacity","_BackR","_BackG","_BackB","_BackA","_MotionGroup"})material.SetFloat(property,0);
                    var materialPath=Root+"/Panel"+slot+".mat";var old=AssetDatabase.LoadAssetAtPath<Material>(materialPath);if(old){EditorUtility.CopySerialized(material,old);UnityEngine.Object.DestroyImmediate(material);material=old;EditorUtility.SetDirty(old);}else AssetDatabase.CreateAsset(material,materialPath);
                    obj.GetComponent<MeshRenderer>().sharedMaterial=material;obj.GetComponent<MeshRenderer>().shadowCastingMode=UnityEngine.Rendering.ShadowCastingMode.Off;
                    var output=Output(slot);
                    var outputSm=new AnimatorStateMachine{name="Text output "+slot};AssetDatabase.AddObjectToAsset(outputSm,fx);fx.AddLayer(new AnimatorControllerLayer{name=outputSm.name,stateMachine=outputSm,defaultWeight=1});outputSm.defaultState=State(outputSm,"Cached output",output);
                    var sm=new AnimatorStateMachine{name="Text buffer "+slot};AssetDatabase.AddObjectToAsset(sm,fx);fx.AddLayer(new AnimatorControllerLayer{name=sm.name,stateMachine=sm,defaultWeight=1});var idle=State(sm,"Wait for page",empty);idle.writeDefaultValues=false;sm.defaultState=idle;
                    AnimatorState Command(string name,int op,int page=-1) {
                        var state=State(sm,name,empty);state.writeDefaultValues=false;var t=Enter(sm,state);Condition(t,"MF6_TextSnapshot",0);Condition(t,"MF6_Q28",254);Condition(t,"MF6_Q00",op);Condition(t,"MF6_Q01",slot);if(page>=0)Condition(t,"MF6_Q02",page);Leave(state,idle);return state;
                    }
                    VRCAvatarParameterDriver Buffer(AnimatorState state){var d=state.AddStateMachineBehaviour<VRCAvatarParameterDriver>();d.localOnly=false;return d;}
                    var cfg=Buffer(Command("Configure",1));for(int i=2;i<=25;i++)CopyCache(cfg,slot,i,256+i-2);UpdateCount(cfg,slot,24);DriverCopy(cfg,"MF6_Q25","MF6_Text"+slot+"_Epoch");
                    var style=Buffer(Command("Style",3));for(int i=2;i<=13;i++)CopyCache(style,slot,i,288+i-2);DriverCopy(style,"MF6_Q14","MF6_Text"+slot+"_Epoch");
                    void Clear(VRCAvatarParameterDriver d){DriverSet(d,Count(slot),0);for(int page=0;page<11;page++)DriverSet(d,Gate(slot,page),0);}
                    var delete=Buffer(Command("Delete",4));Clear(delete);DriverCopy(delete,"MF6_Q02","MF6_Text"+slot+"_Epoch");
                    for(int page=0;page<11;page++) {var d=Buffer(Command("Page "+page,2,page));for(int i=0;i<12&&page*12+i<Slots;i++){CopyCache(d,slot,3+i*2,(page*12+i)*2);CopyCache(d,slot,4+i*2,(page*12+i)*2+1);}}
                    var clear=State(sm,"Clear all",empty);clear.writeDefaultValues=false;var ct=sm.AddAnyStateTransition(clear);ct.duration=0;ct.hasExitTime=false;ct.canTransitionToSelf=false;Condition(ct,"MF6_Stage",2);Condition(ct,"MF6_Format",15);Leave(clear,idle);Clear(Buffer(clear));
                    for(int epoch=0;epoch<4;epoch++) {
                        var snapshot=State(sm,"Latest 14 characters epoch "+epoch,empty);snapshot.writeDefaultValues=false;var enter=Enter(sm,snapshot);Condition(enter,"MF6_TextSnapshot",1);Condition(enter,"MF6_TextSlot",slot);Condition(enter,"MF6_TextEpoch",epoch);Condition(enter,"MF6_Text"+slot+"_Epoch",epoch);Leave(snapshot,idle);
                        var sd=Buffer(snapshot);for(int i=0;i<28;i++)CopyCache(sd,slot,i,i);UpdateCount(sd,slot,28,true);
                    }
                }
                var merge=root.AddComponent<ModularAvatarMergeAnimator>();merge.animator=fx;merge.layerType=VRCAvatarDescriptor.AnimLayerType.FX;merge.pathMode=MergeAnimatorPathMode.Relative;merge.matchAvatarWriteDefaults=false;
                PrefabUtility.SaveAsPrefabAsset(root,Root+"/TextDisplay.prefab");AssetDatabase.SaveAssets();
            }finally{UnityEngine.Object.DestroyImmediate(root);}
        }
        public static void Install(GameObject module) {
            var decoder=module.GetComponentsInChildren<ModularAvatarMergeAnimator>(true).Single(m=>m.gameObject.name=="MF6Decoder");ExtendInbox((AnimatorController)decoder.animator);
            var center=decoder.transform.parent;foreach(var old in center.Cast<Transform>().Where(t=>t.name=="TextDisplay").ToArray())UnityEngine.Object.DestroyImmediate(old.gameObject);
            var display=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(Root+"/TextDisplay.prefab"),center);display.name="TextDisplay";
        }
    }
}
