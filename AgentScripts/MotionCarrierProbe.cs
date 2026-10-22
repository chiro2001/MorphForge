using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using UnityEngine;
using UnityEditor;
using UnityEditor.SceneManagement;
using MorphForge.Editor;
using Newtonsoft.Json.Linq;
public static class MotionCarrierProbe {
 public static string Clock(){
  var preview=EditorSceneManager.NewPreviewScene();var rows=new JArray();
  try{foreach(bool loop in new[]{false,true}){
   var root=new GameObject("Motion clock",typeof(Animator));UnityEngine.SceneManagement.SceneManager.MoveGameObjectToScene(root,preview);var probe=new GameObject("Probe").transform;probe.SetParent(root.transform,false);
   var fx=new UnityEditor.Animations.AnimatorController();fx.AddParameter(new AnimatorControllerParameter{name="Phase",type=AnimatorControllerParameterType.Float,defaultFloat=.5f});fx.AddParameter(new AnimatorControllerParameter{name="Rate",type=AnimatorControllerParameterType.Float,defaultFloat=0});fx.AddLayer("Clock");var sm=fx.layers[0].stateMachine;var state=sm.AddState("Clock");sm.defaultState=state;
   var clip=new AnimationClip();AnimationUtility.SetEditorCurve(clip,EditorCurveBinding.FloatCurve("Probe",typeof(Transform),"m_LocalPosition.x"),AnimationCurve.Linear(0,0,1,1));var settings=AnimationUtility.GetAnimationClipSettings(clip);settings.loopTime=loop;AnimationUtility.SetAnimationClipSettings(clip,settings);state.motion=clip;state.writeDefaultValues=true;state.cycleOffsetParameterActive=true;state.cycleOffsetParameter="Phase";state.speedParameterActive=true;state.speedParameter="Rate";
   var a=root.GetComponent<Animator>();a.runtimeAnimatorController=fx;a.Rebind();a.SetFloat("Phase",.5f);a.SetFloat("Rate",0);a.Update(0);float paused=probe.localPosition.x;a.SetFloat("Rate",1);a.Update(.2f);float moved=probe.localPosition.x;a.SetFloat("Rate",0);a.Update(.2f);float held=probe.localPosition.x;
   rows.Add(new JObject{["loop"]=loop,["initialPhase"]=paused,["afterPlay"]=moved,["afterPause"]=held,["offsetPass"]=Mathf.Abs(paused-.5f)<.001,["advancePass"]=Mathf.Abs(moved-.7f)<.001,["pausePass"]=Mathf.Abs(held-moved)<.001});
   UnityEngine.Object.DestroyImmediate(root);foreach(var obj in new UnityEngine.Object[]{state,sm,clip,fx})if(obj)UnityEngine.Object.DestroyImmediate(obj);
  }}finally{EditorSceneManager.ClosePreviewScene(preview);}
  var result=new JObject{["success"]=rows.All(r=>(bool)r["offsetPass"]&&(bool)r["advancePass"]&&(bool)r["pausePass"]),["rows"]=rows,["scope"]="Transient native Animator clock: local phase offset, speed and pause, no product asset changes."};Directory.CreateDirectory("Docs/TextAnimation");File.WriteAllText("Docs/TextAnimation/motion-clock.json",result.ToString());return result.ToString();
 }
 public static async System.Threading.Tasks.Task<string> Alpha(){
  var preview=EditorSceneManager.NewPreviewScene();var root=new GameObject("Motion alpha math",typeof(Animator));UnityEngine.SceneManagement.SceneManager.MoveGameObjectToScene(root,preview);var child=new GameObject("Particles",typeof(ParticleSystem));child.transform.SetParent(root.transform,false);var ps=child.GetComponent<ParticleSystem>();ps.Stop(true,ParticleSystemStopBehavior.StopEmittingAndClear);var main=ps.main;main.playOnAwake=false;main.startLifetime=1;main.startSpeed=0;main.startColor=Color.white;var emission=ps.emission;emission.enabled=false;child.GetComponent<ParticleSystemRenderer>().enabled=false;
  var fx=new UnityEditor.Animations.AnimatorController{name="Motion alpha math"};fx.AddParameter(new AnimatorControllerParameter{name="One",type=AnimatorControllerParameterType.Float,defaultFloat=1});fx.AddParameter("Group",AnimatorControllerParameterType.Float);fx.AddLayer("Math");var sm=fx.layers[0].stateMachine;var state=sm.AddState("Math");state.writeDefaultValues=true;sm.defaultState=state;
  var animator=root.GetComponent<Animator>();animator.runtimeAnimatorController=fx;animator.cullingMode=AnimatorCullingMode.AlwaysAnimate;var rows=new JArray();var owned=new List<UnityEngine.Object>{root,state,sm,fx};
  try {
   foreach(bool triangle in new[]{false,true}) {
    AnimationClip Clip(string name,float value){var c=new AnimationClip{name=name};owned.Add(c);foreach(string field in new[]{"minColor","maxColor"})AnimationUtility.SetEditorCurve(c,EditorCurveBinding.FloatCurve("Particles",typeof(ParticleSystem),"InitialModule.startColor."+field+".a"),AnimationCurve.Constant(0,1,value));return c;}
    var basis=Clip("Basis",triangle?2f/255:1);var delta=Clip("Group coefficient",(triangle?6f:-40f)/255);
    var tree=new UnityEditor.Animations.BlendTree{name="Alpha group",blendType=UnityEditor.Animations.BlendTreeType.Direct};owned.Add(tree);tree.AddChild(basis);tree.AddChild(delta);var children=tree.children;children[0].directBlendParameter="One";children[1].directBlendParameter="Group";tree.children=children;var so=new SerializedObject(tree);so.FindProperty("m_NormalizedBlendValues").boolValue=false;so.ApplyModifiedPropertiesWithoutUndo();state.motion=tree;animator.runtimeAnimatorController=null;animator.runtimeAnimatorController=fx;animator.Rebind();
    for(int group=0;group<4;group++){animator.SetFloat("Group",group);animator.SetFloat("One",1);animator.Update(0);await System.Threading.Tasks.Task.Delay(50);ps.Clear();ps.Emit(1);var particles=new ParticleSystem.Particle[1];ps.GetParticles(particles);int expected=triangle?2+6*group:255-40*group;int actual=particles[0].startColor.a;rows.Add(new JObject{["triangle"]=triangle,["group"]=group,["expectedAlpha"]=expected,["actualAlpha"]=actual,["pass"]=actual==expected});}
   }
  }finally{foreach(var obj in owned)if(obj)UnityEngine.Object.DestroyImmediate(obj);EditorSceneManager.ClosePreviewScene(preview);}
  var result=new JObject{["success"]=rows.All(r=>(bool)r["pass"]),["rows"]=rows,["scope"]="Transient native Animator -> ParticleSystem birth Color32; no product assets or Avatar parameters modified."};Directory.CreateDirectory("Docs/TextAnimation");File.WriteAllText("Docs/TextAnimation/motion-alpha.json",result.ToString());return result.ToString();
 }
 public static string Run(){
  if(EditorApplication.isPlaying)throw new Exception("Edit Mode diagnostic required");
  var scene=EditorSceneManager.NewPreviewScene();var result=new JArray();
  var go=new GameObject("Motion carrier diagnostic",typeof(ParticleSystem));UnityEngine.SceneManagement.SceneManager.MoveGameObjectToScene(go,scene);go.transform.position=new Vector3(3,2,5);go.transform.rotation=Quaternion.Euler(17,35,8);go.transform.localScale=Vector3.one*.8f;
  var ps=go.GetComponent<ParticleSystem>();ps.Stop(true,ParticleSystemStopBehavior.StopEmittingAndClear);var main=ps.main;main.playOnAwake=false;main.loop=false;main.startSpeed=0;main.startLifetime=1000;main.startSize3D=true;main.startSizeX=main.startSizeY=main.startSizeZ=1;main.startRotation3D=true;main.maxParticles=1;main.simulationSpace=ParticleSystemSimulationSpace.Custom;main.customSimulationSpace=go.transform;main.scalingMode=ParticleSystemScalingMode.Hierarchy;var emission=ps.emission;emission.enabled=false;var shape=ps.shape;shape.enabled=false;
  var r=go.GetComponent<ParticleSystemRenderer>();r.renderMode=ParticleSystemRenderMode.Mesh;r.alignment=ParticleSystemRenderSpace.Local;r.enableGPUInstancing=false;r.mesh=AssetDatabase.LoadAssetAtPath<Mesh>(Mf6NativeBuild.Root+"/Pool0/Carrier.asset");
  r.SetActiveVertexStreams(new List<ParticleSystemVertexStream>{ParticleSystemVertexStream.Position,ParticleSystemVertexStream.Normal,ParticleSystemVertexStream.Tangent,ParticleSystemVertexStream.Color,ParticleSystemVertexStream.UV,ParticleSystemVertexStream.UV2,ParticleSystemVertexStream.SizeXYZ,ParticleSystemVertexStream.Center,ParticleSystemVertexStream.InvStartLifetime,ParticleSystemVertexStream.Rotation3D});
  var material=new Material(Shader.Find("MorphForge/Diagnostics/MotionCarrierProbe"));r.sharedMaterial=material;material.SetVector("_ExpectedX",go.transform.right);material.SetVector("_ExpectedY",go.transform.up);material.SetVector("_ExpectedZ",go.transform.forward);
  var camera=new GameObject("Motion diagnostic camera").AddComponent<Camera>();UnityEngine.SceneManagement.SceneManager.MoveGameObjectToScene(camera.gameObject,scene);camera.scene=scene;camera.cameraType=CameraType.Preview;camera.enabled=false;camera.orthographic=true;camera.orthographicSize=1.4f;camera.transform.position=go.transform.position+Vector3.forward*5;camera.transform.LookAt(go.transform.position);camera.nearClipPlane=.01f;camera.farClipPlane=20;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=Color.clear;camera.allowHDR=true;
  var target=new RenderTexture(64,64,24,RenderTextureFormat.ARGBFloat,RenderTextureReadWrite.Linear);target.Create();camera.targetTexture=target;
  Color Sample(int mode){material.SetFloat("_Mode",mode);camera.Render();var old=RenderTexture.active;RenderTexture.active=target;var read=new Texture2D(1,1,TextureFormat.RGBAFloat,false,true);read.ReadPixels(new Rect(32,32,1,1),0,0);read.Apply();RenderTexture.active=old;var c=read.GetPixel(0,0);UnityEngine.Object.DestroyImmediate(read);return c;}
  try{
   foreach(var trial in new[]{new[]{.4f,.8f},new[]{511f/1048576,(1f+512*511)/1048576},new[]{1f/1048576,(511f+512)/1048576}}){
    ps.Clear();main.startRotationX=trial[0];main.startRotationY=0;main.startRotationZ=trial[1];ps.Emit(1);ps.Simulate(.01f,false,false,false);
    var raw=Sample(0);var frame=Sample(2);var particles=new ParticleSystem.Particle[1];ps.GetParticles(particles);var row=new JObject{["birthRadians"]=new JArray(trial),["rawStream"]=new JArray(raw.r,raw.g,raw.b,raw.a),["cpuRotationDegrees"]=particles[0].rotation3D.ToString("R"),["unrotatedBasisDot"]=new JArray(frame.r,frame.g,frame.b),["radiansPass"]=Mathf.Abs(raw.r-trial[0])<1e-6&&Mathf.Abs(raw.b-trial[1])<1e-6,["basisPass"]=Mathf.Min(frame.r,frame.g,frame.b)>.9999f};
    if(trial[0]<.01f){var decoded=Sample(1);row["decodedHalfCenters"]=new JArray(decoded.r,decoded.g,decoded.b);row["centerPass"]=Mathf.Abs(decoded.b-Mathf.Round(trial[0]*1048576))<.5f&&Mathf.Abs(decoded.r-(Mathf.Round(trial[1]*1048576)%512))<.5f&&Mathf.Abs(decoded.g-Mathf.Floor(Mathf.Round(trial[1]*1048576)/512))<.5f;}
    result.Add(row);
   }
   var report=new JObject{["success"]=result.All(x=>(bool)x["radiansPass"]&&(bool)x["basisPass"]&&(x["centerPass"]==null||(bool)x["centerPass"])),["trials"]=result,["scope"]="Edit Mode preview scene and real particle vertex streams/GPU readback; diagnostic only, no Avatar modification or runtime C# requirement."};Directory.CreateDirectory("Docs/TextAnimation");File.WriteAllText("Docs/TextAnimation/motion-carrier.json",report.ToString());return report.ToString();
  }finally{camera.targetTexture=null;target.Release();UnityEngine.Object.DestroyImmediate(target);UnityEngine.Object.DestroyImmediate(material);EditorSceneManager.ClosePreviewScene(scene);}
 }
}
