using System;
using System.Linq;
using System.Collections.Generic;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase;
using nadena.dev.modular_avatar.core;
using MorphForgeAvatar.Editor;
namespace MorphForge.Editor {
 public static class Mf6MotionBuild {
  public const string Root="Assets/MorphForgeAvatar/Generated/Motion";
  public static readonly float[] Durations={.125f,.25f,.5f,1,2,4,8,16,32,64};
  public static readonly float[] Speeds={.25f,.5f,1,2};
  static AnimatorController fx;static AnimationClip blank;static Dictionary<string,AnimatorControllerParameter> parameters;
  static Dictionary<string,Motion> retainedWords,receivePages;
  static string N(int g,string s)=>"MF6_M"+g+"_"+s;
  static string B(int g,int bank,string s)=>N(g,"B"+bank+"_"+s);
  static string C(int g,int bank,int i)=>B(g,bank,"C"+i.ToString("D2"));
  static string U(int g,int bank,string s)=>"_MF_M"+g+"B"+bank+"_"+s;
  static string Q(int i)=>"MF6_Q"+i.ToString("D2");static string T(int i)=>"MF6_T"+i.ToString("D2");
  public static readonly int[] CachedFields=new[]{2,3,4,6,8}.Concat(Enumerable.Range(0,4).SelectMany(k=>Enumerable.Range(0,9).Select(i=>16+k*18+i*2))).ToArray();
  static void Param(string name,bool integer=false,float value=0){if(!parameters.ContainsKey(name))parameters[name]=new AnimatorControllerParameter{name=name,type=integer?AnimatorControllerParameterType.Int:AnimatorControllerParameterType.Float,defaultInt=(int)value,defaultFloat=value};}
  static void Set(VRCAvatarParameterDriver d,string n,float value){d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=n,type=VRC_AvatarParameterDriver.ChangeType.Set,value=value});}
  static void Copy(VRCAvatarParameterDriver d,string source,string name,bool convert=false,float sourceMin=0,float sourceMax=255,float destMin=0,float destMax=1){d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=name,source=source,type=VRC_AvatarParameterDriver.ChangeType.Copy,convertRange=convert,sourceMin=sourceMin,sourceMax=sourceMax,destMin=destMin,destMax=destMax});}
  static AnimatorStateMachine Layer(string name){var sm=new AnimatorStateMachine{name=name};AssetDatabase.AddObjectToAsset(sm,fx);fx.AddLayer(new AnimatorControllerLayer{name=name,stateMachine=sm,defaultWeight=1});return sm;}
  static AnimatorState State(AnimatorStateMachine sm,string name,Motion motion=null,bool wd=false){var s=sm.AddState(name);s.motion=motion?motion:blank;s.writeDefaultValues=wd;return s;}
  static VRCAvatarParameterDriver Driver(AnimatorState state){var d=state.AddStateMachineBehaviour<VRCAvatarParameterDriver>();d.localOnly=false;return d;}
  static void Eq(AnimatorStateTransition t,string n,int v){Param(n,true);t.AddCondition(AnimatorConditionMode.Equals,v,n);}
  static void Ne(AnimatorStateTransition t,string n,int v){Param(n,true);t.AddCondition(AnimatorConditionMode.NotEqual,v,n);}
  static AnimatorStateTransition Any(AnimatorStateMachine sm,AnimatorState s){var t=sm.AddAnyStateTransition(s);t.duration=0;t.hasExitTime=false;t.canTransitionToSelf=false;return t;}
  static void Leave(AnimatorState s,AnimatorState idle,string n="MF6_Stage",int v=2){var t=s.AddTransition(idle);t.duration=0;t.hasExitTime=false;Ne(t,n,v);}
  static AnimatorStateTransition Enter(AnimatorStateMachine sm,AnimatorState s,int op,int group,int stage=2){var t=Any(sm,s);Eq(t,"MF6_Stage",stage);Eq(t,"MF6_Format",0);Eq(t,Q(28),254);Eq(t,Q(0),op);if(group>0)Eq(t,Q(1),group);return t;}
  static AnimationClip Clip(string name,string prop,float value,int first=0,int count=12){var c=new AnimationClip{name=name,frameRate=60};foreach(int i in Enumerable.Range(first,count)){string path=i<8?"Pool"+i+"/Store":"TextDisplay/Panel"+(i-8);AnimationUtility.SetEditorCurve(c,EditorCurveBinding.FloatCurve(path,i<8?typeof(ParticleSystemRenderer):typeof(MeshRenderer),"material."+prop),AnimationCurve.Constant(0,.02f,value));}AssetDatabase.AddObjectToAsset(c,fx);return c;}
  static BlendTree Direct(string name,IList<Motion> motions,IList<string> weights){var tree=new BlendTree{name=name,blendType=BlendTreeType.Direct};AssetDatabase.AddObjectToAsset(tree,fx);foreach(var m in motions)tree.AddChild(m);var children=tree.children;for(int i=0;i<children.Length;i++)children[i].directBlendParameter=weights[i];tree.children=children;var so=new SerializedObject(tree);so.FindProperty("m_NormalizedBlendValues").boolValue=false;so.ApplyModifiedPropertiesWithoutUndo();return tree;}
  static Motion HoldWords(IEnumerable<string> names){
   var keys=names.Distinct().ToArray();var clips=new List<Motion>();
   foreach(string name in keys){if(!retainedWords.TryGetValue(name,out var clip)){var c=new AnimationClip{name="Retain "+name,frameRate=60};AnimationUtility.SetEditorCurve(c,EditorCurveBinding.FloatCurve("",typeof(Animator),name),AnimationCurve.Constant(0,.02f,1));AssetDatabase.AddObjectToAsset(c,fx);clip=c;retainedWords[name]=clip;}clips.Add(clip);}
   return Direct("Retain motion words",clips,keys);
  }
  static IEnumerable<string> Words(int g)=>Enumerable.Range(0,2).SelectMany(b=>CachedFields.Where(i=>i>=4).Select(i=>C(g,b,i)));
  static Motion ReceivePage(int g,IEnumerable<WordCacheBuild.Input> input){
   var words=input.ToArray();string key=string.Join(";",words.Select(w=>w.target+"@"+w.firstByte));
   if(receivePages.TryGetValue(key,out var cached))return cached;
   var replaced=new HashSet<string>(words.Select(w=>w.target));
   // Every receive/idle state writes the complete cache with Write Defaults On.
   // One layer owns the feedback; the material output layer never rewrites it.
   var motion=Direct("Replace motion cache page",new Motion[]{WordCacheBuild.Receive("MF6_MOne",words),HoldWords(Words(g).Where(n=>!replaced.Contains(n)))},new[]{"MF6_MOne","MF6_MOne"});
   receivePages[key]=motion;return motion;
  }
  struct Term{public string prop,param;public float coefficient;public bool packed;}
  // Keep both transactional receive banks, but expose only the selected bank
  // to the shader's B0 view. Duplicated renderer bindings cost CPU even while
  // the output tree is inactive. Gate weights select the retained source bank.
  static IEnumerable<Term> Word(int g,int bank,string prop,int field,float unit,float offset=0){yield return new Term{prop=U(g,0,prop),param=C(g,bank,field),coefficient=unit,packed=true};if(offset!=0)yield return new Term{prop=U(g,0,prop),param="MF6_MOne",coefficient=offset};}
  static AnimationClip Factors(string name,IEnumerable<Term> terms,int first,int count){
   var c=new AnimationClip{name=name,frameRate=60};
   foreach(var property in terms.GroupBy(t=>t.prop))foreach(int i in Enumerable.Range(first,count))AnimationUtility.SetEditorCurve(c,EditorCurveBinding.FloatCurve(i<8?"Pool"+i+"/Store":"TextDisplay/Panel"+(i-8),i<8?typeof(ParticleSystemRenderer):typeof(MeshRenderer),"material."+property.Key),AnimationCurve.Constant(0,.02f,property.Sum(t=>t.coefficient)));
   AssetDatabase.AddObjectToAsset(c,fx);return c;
  }
  static Motion Output(int g,int first,int count){
   var all=new List<Term>();var bankTerms=new List<Term>[2];
   for(int b=0;b<2;b++){
    var ts=new List<Term>{new Term{prop=U(g,0,"Count"),param=C(g,b,2),coefficient=1},new Term{prop=U(g,0,"Interp"),param=C(g,b,3),coefficient=1}};
    for(int a=0;a<3;a++)ts.AddRange(Word(g,b,"Pivot"+"XYZ"[a],4+a*2,1f/64,-512));
    for(int k=0;k<4;k++){int start=16+k*18;ts.AddRange(Word(g,b,"K"+k+"T",start,1f/65535));for(int a=0;a<3;a++)ts.AddRange(Word(g,b,"K"+k+"P"+"XYZ"[a],start+2+a*2,1f/64,-512));for(int a=0;a<4;a++)ts.AddRange(Word(g,b,"K"+k+"Q"+"XYZW"[a],start+8+a*2,1f/32767,-32768f/32767));ts.AddRange(Word(g,b,"K"+k+"S",start+16,1f/4096));}
    bankTerms[b]=ts;all.AddRange(ts);
   }
   var controls=new[]{"Bank","Enabled","Base","Loop"}.Select(s=>new Term{prop="_MF_M"+g+"_"+s,param=s=="Bank"?"MF6_MOne":N(g,s),coefficient=s=="Bank"?0:1}).ToArray();all.AddRange(controls);
   var zero=new AnimationClip{name="Motion zero baseline "+g+"/"+first,frameRate=60};foreach(string p in all.Select(t=>t.prop).Distinct())foreach(int i in Enumerable.Range(first,count))AnimationUtility.SetEditorCurve(zero,EditorCurveBinding.FloatCurve(i<8?"Pool"+i+"/Store":"TextDisplay/Panel"+(i-8),i<8?typeof(ParticleSystemRenderer):typeof(MeshRenderer),"material."+p),AnimationCurve.Constant(0,.02f,0));AssetDatabase.AddObjectToAsset(zero,fx);
   var motions=new List<Motion>{zero};var weights=new List<string>{"MF6_MOne"};
   foreach(var t in controls){motions.Add(Clip(t.prop,t.prop,t.coefficient,first,count));weights.Add(t.param);}
   for(int b=0;b<2;b++){var terms=bankTerms[b].GroupBy(t=>t.param).ToArray();var clips=terms.Select(t=>(Motion)Factors("Motion factors "+g+"/"+b+"/"+first+"/"+t.Key,t,first,count)).ToArray();motions.Add(Direct("Motion bank "+g+"/"+b,clips,terms.Select(t=>t.Key).ToArray()));weights.Add(B(g,b,"Gate"));}
   var render=Direct("Motion cached output "+g,motions,weights);
   return render;
  }
  static void OutputLayer(int g,int first,int count,string name,int textSlot=-1){
   var output=Layer(name);var sleeping=State(output,"Inactive",Clip("Motion inactive "+g+"/"+first,"_MF_M"+g+"_Enabled",0,first,count));
   var rendering=State(output,"Cached",Output(g,first,count),true);output.defaultState=sleeping;
   var awake=Any(output,rendering);awake.AddCondition(AnimatorConditionMode.Greater,.5f,N(g,"Enabled"));
   var sleep=Any(output,sleeping);sleep.AddCondition(AnimatorConditionMode.Less,.5f,N(g,"Enabled"));
   if(textSlot>=0){
    string populated="MF6_Text"+textSlot+"_Count",group="MF6_Text"+textSlot+"_B299";Param(populated);Param(group);
    awake.AddCondition(AnimatorConditionMode.Greater,.5f,populated);awake.AddCondition(AnimatorConditionMode.Greater,g-.5f,group);awake.AddCondition(AnimatorConditionMode.Less,g+.5f,group);
    sleep=Any(output,sleeping);sleep.AddCondition(AnimatorConditionMode.Less,.5f,populated);
    sleep=Any(output,sleeping);sleep.AddCondition(AnimatorConditionMode.Less,g-.5f,group);
    sleep=Any(output,sleeping);sleep.AddCondition(AnimatorConditionMode.Greater,g+.5f,group);
   }
  }
  static void RateDecoder(){Param("MF6_MDecodedRate");var sm=Layer("Motion rate lookup");var idle=State(sm,"Wait");sm.defaultState=idle;for(int d=0;d<Durations.Length;d++)for(int s=0;s<Speeds.Length;s++){var state=State(sm,"Rate "+d+"/"+s);Set(Driver(state),"MF6_MDecodedRate",Speeds[s]/Durations[d]);var t=Enter(sm,state,16,0,1);Eq(t,Q(4),d);Eq(t,Q(15),s);Leave(state,idle,v:1);}}
  static void Buffers(int g){var sm=Layer("Motion receive "+g);var idle=State(sm,"Wait",HoldWords(Words(g)),true);sm.defaultState=idle;
   for(int b=0;b<2;b++)for(int v=0;v<4;v++){
    var state=State(sm,"Header "+b+"/"+v,ReceivePage(g,Enumerable.Range(0,3).Select(a=>WordCacheBuild.Pair(C(g,b,4+a*2),8+a*2))),true);var t=Enter(sm,state,16,g);Eq(t,Q(2),b);Eq(t,Q(3),v);Ne(t,B(g,b,"Version"),v);var d=Driver(state);Set(d,B(g,b,"Version"),v);Set(d,B(g,b,"Ready"),0);for(int k=0;k<4;k++)Set(d,B(g,b,"Key"+k),0);Copy(d,T(6),C(g,b,2));Copy(d,T(7),C(g,b,3));Copy(d,Q(5),B(g,b,"Loop"));Copy(d,Q(6),B(g,b,"Count"));Copy(d,"MF6_MDecodedRate",B(g,b,"Rate"));Leave(state,idle);
    for(int k=0;k<4;k++){int keyStart=16+k*18;state=State(sm,"Key "+b+"/"+v+"/"+k,ReceivePage(g,Enumerable.Range(0,9).Select(i=>WordCacheBuild.Pair(C(g,b,keyStart+i*2),4+i*2))),true);t=Enter(sm,state,17,g);Eq(t,Q(2),b);Eq(t,Q(3),k);Eq(t,Q(24),v);Eq(t,B(g,b,"Version"),v);d=Driver(state);Set(d,B(g,b,"Key"+k),1);Leave(state,idle);}
   }
   sm=Layer("Motion complete banks "+g);idle=State(sm,"Wait");sm.defaultState=idle;for(int b=0;b<2;b++)for(int count=1;count<=4;count++){var state=State(sm,"Ready "+b+"/"+count);var t=Any(sm,state);Eq(t,B(g,b,"Ready"),0);Eq(t,B(g,b,"Count"),count);for(int k=0;k<count;k++)Eq(t,B(g,b,"Key"+k),1);Set(Driver(state),B(g,b,"Ready"),1);var leave=state.AddTransition(idle);leave.duration=0;leave.hasExitTime=false;Eq(leave,B(g,b,"Ready"),1);}
  }
  static void Controls(int g){
   var sm=Layer("Motion validate control "+g);var idle=State(sm,"Wait");sm.defaultState=idle;var clear=State(sm,"New control");Set(Driver(clear),N(g,"Valid"),0);Enter(sm,clear,18,g,1);Leave(clear,idle,v:1);
   for(int b=0;b<2;b++)for(int v=0;v<4;v++){var s=State(sm,"Valid "+b+"/"+v);var t=Enter(sm,s,18,g);Eq(t,Q(2),b);Eq(t,Q(3),v);Eq(t,B(g,b,"Ready"),1);Eq(t,B(g,b,"Version"),v);var d=Driver(s);Set(d,N(g,"Valid"),1);Copy(d,B(g,b,"Loop"),N(g,"SelectedLoop"));Copy(d,B(g,b,"Rate"),N(g,"SelectedRate"));Leave(s,idle);}
   sm=Layer("Motion apply control "+g);idle=State(sm,"Wait");sm.defaultState=idle;
   for(int ticket=0;ticket<16;ticket++)for(int kind=0;kind<3;kind++){
    var s=State(sm,"Control "+ticket+"/"+kind);var t=Enter(sm,s,18,g);
    // Validation can finish on the last route frame. Apply on settled Stage0 too;
    // Stage1 is excluded so a prior Valid cannot authorize a new snapshot.
    t.conditions=t.conditions.Where(c=>c.parameter!="MF6_Stage").ToArray();Ne(t,"MF6_Stage",1);Eq(t,N(g,"Valid"),1);Eq(t,Q(4),ticket);Ne(t,N(g,"AppliedTicket"),ticket);Eq(t,Q(5),kind);
    var d=Driver(s);Set(d,N(g,"AppliedTicket"),ticket);Set(d,N(g,"PendingTicket"),ticket);Copy(d,T(2),N(g,"Bank"));Copy(d,N(g,"SelectedLoop"),N(g,"Loop"));Copy(d,N(g,"SelectedLoop"),N(g,"LoopInt"));Set(d,N(g,"Enabled"),kind==0?0:1);
    if(kind==0){Set(d,N(g,"Base"),0);Set(d,B(g,0,"Gate"),0);Set(d,B(g,1,"Gate"),0);}else{Copy(d,T(6),N(g,"Base"),true);Copy(d,T(2),B(g,0,"Gate"),true,0,1,1,0);Copy(d,T(2),B(g,1,"Gate"),true,0,1,0,1);}
    if(kind==1)Copy(d,N(g,"SelectedRate"),N(g,"Rate"));else Set(d,N(g,"Rate"),0);Leave(s,idle);
   }
  }
  static void Clocks(int g){
   var clips=new AnimationClip[2];for(int loop=0;loop<2;loop++){var c=Clip("Motion elapsed "+g+"/"+loop,"_MF_M"+g+"_Elapsed",0);foreach(var binding in AnimationUtility.GetCurveBindings(c))AnimationUtility.SetEditorCurve(c,binding,AnimationCurve.Linear(0,0,1,1));var settings=AnimationUtility.GetAnimationClipSettings(c);settings.loopTime=loop==1;AnimationUtility.SetAnimationClipSettings(c,settings);clips[loop]=c;}
   var sm=Layer("Motion local clock "+g);var idle=State(sm,"Off",Clip("Motion elapsed initial "+g,"_MF_M"+g+"_Elapsed",0),true);sm.defaultState=idle;var reset=Any(sm,idle);Eq(reset,N(g,"PendingTicket"),255);
   for(int ticket=0;ticket<16;ticket++)for(int loop=0;loop<2;loop++){var s=State(sm,"Clock "+ticket+"/"+loop,clips[loop],true);s.speedParameterActive=true;s.speedParameter=N(g,"Rate");Set(Driver(s),N(g,"ClockTicket"),ticket);var t=Any(sm,s);Eq(t,N(g,"PendingTicket"),ticket);Ne(t,N(g,"ClockTicket"),ticket);Eq(t,N(g,"LoopInt"),loop);}
  }
  static void ResetOnClear(){
   var sm=Layer("Motion clear session");var idle=State(sm,"Wait");sm.defaultState=idle;var reset=State(sm,"Clear all motion");var t=Any(sm,reset);Eq(t,"MF6_Stage",2);Eq(t,"MF6_Format",15);var d=Driver(reset);
   var session=Any(sm,reset);Eq(session,"MF6_Stage",2);Eq(session,"MF6_Format",0);Eq(session,Q(28),254);Eq(session,Q(0),19);
   for(int g=1;g<=3;g++){
    foreach(string key in new[]{"Enabled","Base","Rate","Valid","Bank","LoopInt"})Set(d,N(g,key),0);
    foreach(string key in new[]{"AppliedTicket","PendingTicket","ClockTicket"})Set(d,N(g,key),255);
    for(int b=0;b<2;b++){Set(d,B(g,b,"Version"),255);foreach(string key in new[]{"Ready","Count","Loop","Gate","Key0","Key1","Key2","Key3"})Set(d,B(g,b,key),0);}
   }Leave(reset,idle);
  }
  public static string Build(){
   if(EditorApplication.isPlaying)throw new Exception("Edit Mode required");CatalogVariant.EnsureFolder(Root);var path=Root+"/Motion.controller";fx=AssetDatabase.LoadAssetAtPath<AnimatorController>(path);if(!fx)fx=AnimatorController.CreateAnimatorControllerAtPath(path);else{fx.layers=new AnimatorControllerLayer[0];fx.parameters=new AnimatorControllerParameter[0];foreach(var a in AssetDatabase.LoadAllAssetsAtPath(path))if(a&&a!=fx)UnityEngine.Object.DestroyImmediate(a,true);}
   WordCacheBuild.Begin(fx);retainedWords=new Dictionary<string,Motion>();receivePages=new Dictionary<string,Motion>();
   parameters=new Dictionary<string,AnimatorControllerParameter>();Param("MF6_MOne",value:1);Param("MF6_Stage",true);Param("MF6_Format",true);for(int i=0;i<29;i++){Param(Q(i),true);Param(T(i));}
   blank=new AnimationClip{name="Motion receive tick",frameRate=60};AnimationUtility.SetEditorCurve(blank,EditorCurveBinding.FloatCurve("",typeof(Transform),"m_LocalScale.x"),AnimationCurve.Constant(0,.02f,1));AssetDatabase.AddObjectToAsset(blank,fx);
   RateDecoder();for(int g=1;g<=3;g++){
    foreach(string s in new[]{"Bank","Enabled","Base","Loop","Rate","SelectedRate"})Param(N(g,s));foreach(string s in new[]{"Valid","LoopInt","SelectedLoop"})Param(N(g,s),true);foreach(string s in new[]{"AppliedTicket","PendingTicket","ClockTicket"})Param(N(g,s),true,255);
    for(int b=0;b<2;b++){Param(B(g,b,"Version"),true,255);foreach(string s in new[]{"Ready","Count","Loop","Key0","Key1","Key2","Key3"})Param(B(g,b,s),true);Param(B(g,b,"Gate"));Param(B(g,b,"Rate"));foreach(int i in CachedFields)Param(C(g,b,i));}
    Buffers(g);Controls(g);
    // A WD-on state restores its entire property footprint even when a 1D
    // tree selects the tiny disabled clip. Sleep in an ordinary WD-off clip;
    // the complete direct output retains WD on whenever it is active.
    OutputLayer(g,0,8,"Motion output "+g);
    for(int slot=0;slot<4;slot++)OutputLayer(g,8+slot,1,"Motion text output "+g+"/"+slot,slot);
    Clocks(g);
   }
   ResetOnClear();fx.parameters=parameters.Values.ToArray();WordCacheBuild.AddInputs(fx);EditorUtility.SetDirty(fx);AssetDatabase.SaveAssets();return Newtonsoft.Json.JsonConvert.SerializeObject(new{success=true,layers=fx.layers.Length,states=fx.layers.Sum(l=>l.stateMachine.states.Length),parameters=fx.parameters.Length,syncedBits=0});
  }
  public static void Install(GameObject module){var decoder=module.GetComponentsInChildren<ModularAvatarMergeAnimator>(true).Single(m=>m.gameObject.name=="MF6Decoder");var center=decoder.transform.parent;var merge=center.GetComponent<ModularAvatarMergeAnimator>();if(!merge)merge=center.gameObject.AddComponent<ModularAvatarMergeAnimator>();merge.animator=AssetDatabase.LoadAssetAtPath<AnimatorController>(Root+"/Motion.controller");merge.layerType=VRCAvatarDescriptor.AnimLayerType.FX;merge.pathMode=MergeAnimatorPathMode.Relative;merge.matchAvatarWriteDefaults=false;EditorUtility.SetDirty(merge);}
 }
}
