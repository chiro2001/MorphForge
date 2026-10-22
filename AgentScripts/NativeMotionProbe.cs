using System;
using System.IO;
using System.Linq;
using System.Net.Sockets;
using System.Threading.Tasks;
using UnityEngine;
using UnityEditor;
using Newtonsoft.Json.Linq;
using MorphForge.Editor;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
public static class NativeMotionProbe {
 static float Value(Renderer r,string property){var b=new MaterialPropertyBlock();r.GetPropertyBlock(b);return b.isEmpty?r.sharedMaterial.GetFloat(property):b.GetFloat(property);}
 public static async Task<string> Performance(){
  var m=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single().Module as ModuleVrc3;var f=System.Reflection.BindingFlags.Instance|System.Reflection.BindingFlags.Public|System.Reflection.BindingFlags.NonPublic;var d=(System.Collections.IDictionary)m.GetType().GetField("_layers",f).GetValue(m);var k=d.Keys.Cast<object>().Single(x=>x.ToString()=="FX");var p=(UnityEngine.Animations.AnimatorControllerPlayable)d[k].GetType().GetField("Playable",f).GetValue(d[k]);
  var all=new System.Collections.Generic.List<Unity.Profiling.LowLevel.Unsafe.ProfilerRecorderHandle>();Unity.Profiling.LowLevel.Unsafe.ProfilerRecorderHandle.GetAvailable(all);var descriptors=all.Select(h=>Unity.Profiling.LowLevel.Unsafe.ProfilerRecorderHandle.GetDescription(h)).Where(h=>h.Name.Contains("Animator")&&h.Name.Contains("Update")).ToArray();if(descriptors.Length==0)throw new Exception("No Animator CPU marker");var descriptor=descriptors.FirstOrDefault(h=>h.Name=="Animators.Update");if(string.IsNullOrEmpty(descriptor.Name))descriptor=descriptors[0];
  var layers=Enumerable.Range(0,p.GetLayerCount()).Where(i=>p.GetLayerName(i).StartsWith("Motion output")||p.GetLayerName(i).StartsWith("Motion local clock")).ToArray();var weights=layers.Select(i=>p.GetLayerWeight(i)).ToArray();var names=Enumerable.Range(1,3).SelectMany(g=>new[]{"MF6_M"+g+"_Enabled","MF6_M"+g+"_B0_Gate","MF6_M"+g+"_B1_Gate"}.Concat(Enumerable.Range(0,96).Select(i=>"MF6_M"+g+"_B0_C"+i.ToString("D2")))).ToArray();var values=names.Select(n=>m.GetParam(n).FloatValue()).ToArray();var template=Enumerable.Range(0,96).Select(i=>m.GetParam("MF6_M1_B0_C"+i.ToString("D2")).FloatValue()).ToArray();var rows=new JArray();
  try{foreach(int mode in new[]{0,1,2,2,1,0}){foreach(int i in layers)p.SetLayerWeight(i,mode==0?0:1);for(int g=1;g<=3;g++){m.GetParam("MF6_M"+g+"_Enabled").Set(m,mode==2?1:0);m.GetParam("MF6_M"+g+"_B0_Gate").Set(m,mode==2?1:0);m.GetParam("MF6_M"+g+"_B1_Gate").Set(m,0);if(mode==2)for(int i=0;i<96;i++)m.GetParam("MF6_M"+g+"_B0_C"+i.ToString("D2")).Set(m,template[i]);}await Task.Delay(250);using(var recorder=Unity.Profiling.ProfilerRecorder.StartNew(descriptor.Category,descriptor.Name,128)){await Task.Delay(1400);var samples=Enumerable.Range(0,recorder.Count).Select(i=>recorder.GetSample(i).Value).ToArray();rows.Add(new JObject{["phase"]=mode==0?"motion output layers weight0":mode==1?"motion idle gates0":"3 active motion banks",["samples"]=samples.Length,["meanRaw"]=samples.Length>0?samples.Average():0,["maxRaw"]=samples.Length>0?samples.Max():0});}}}
  finally{for(int i=0;i<layers.Length;i++)p.SetLayerWeight(layers[i],weights[i]);for(int i=0;i<names.Length;i++)m.GetParam(names[i]).Set(m,values[i]);}
  var path=Mf6MotionBuild.Root+"/Motion.controller";var assets=AssetDatabase.LoadAllAssetsAtPath(path);var result=new JObject{["available"]=true,["marker"]=descriptor.Name,["unit"]=descriptor.UnitType.ToString(),["rows"]=rows,["controllerYamlBytes"]=new FileInfo(path).Length,["runtimeAssetBytesEstimate"]=assets.Sum(a=>UnityEngine.Profiling.Profiler.GetRuntimeMemorySizeLong(a)),["scope"]="Whole Avatar Editor Animator sampling, forward/reverse layer-off, idle and three populated motion banks. Includes base avatar/text/other native layers; not VRChat FPS or causal isolated CPU guarantee. All test parameters/weights restored."};File.WriteAllText("Docs/TextAnimation/native-motion-performance.json",result.ToString());return result.ToString();
 }
 public static async Task<string> Suite(){
  var rows=new JArray();foreach(var entry in new[]{"receiver","dynamics","visual","recovery"}){string data=entry=="receiver"?await Run():entry=="dynamics"?await Dynamics():entry=="visual"?await Visual():await Recovery();var report=JObject.Parse(data);rows.Add(new JObject{["name"]=entry,["success"]=report["success"]});File.WriteAllText("Docs/TextAnimation/native-motion-suite.json",new JObject{["complete"]=false,["checks"]=rows}.ToString());}
  var result=new JObject{["complete"]=true,["success"]=rows.All(r=>(bool)r["success"]),["checks"]=rows,["scene"]=UnityEngine.SceneManagement.SceneManager.GetActiveScene().path};File.WriteAllText("Docs/TextAnimation/native-motion-suite.json",result.ToString());return result.ToString();
 }
 public static async Task<string> Recovery(){
  var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single();var m=gm.Module as ModuleVrc3;var fixture=JObject.Parse(File.ReadAllText("E:/tmp/morphforge/avatar-motion-fixtures.json"));using(var udp=new UdpClient()){var data=fixture["packets"][0]["packet"].Select(v=>(byte)(int)v).ToArray();udp.Send(data,data.Length,"127.0.0.1",9012);}await Task.Delay(450);
  var reset=Enumerable.Range(1,3).All(g=>m.GetParam("MF6_M"+g+"_AppliedTicket").IntValue()==255&&m.GetParam("MF6_M"+g+"_B0_Version").IntValue()==255&&m.GetParam("MF6_M"+g+"_B1_Version").IntValue()==255&&m.GetParam("MF6_M"+g+"_Enabled").FloatValue()==0);
  var rebuilt=JObject.Parse(await Run());var result=new JObject{["success"]=reset&&(bool)rebuilt["success"],["clearedOldMotionSession"]=reset,["rebuildSameGenerationAndTicket"]=rebuilt,["scope"]="Native Clear resets motion versions and tickets; same app-start definition can be received again without switching Avatar."};File.WriteAllText("Docs/TextAnimation/native-motion-recovery.json",result.ToString());return result.ToString();
 }
 public static async Task<string> Visual(){
  if(!EditorApplication.isPlaying)throw new Exception("Play Mode required");var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single();var m=gm.Module as ModuleVrc3;
  var ps=m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Single(p=>p.name=="Store"&&p.transform.parent.name=="Pool0");var root=ps.main.customSimulationSpace;var r=ps.GetComponent<ParticleSystemRenderer>();int layer=r.gameObject.layer;
  var fixture=JObject.Parse(File.ReadAllText("E:/tmp/morphforge/avatar-motion-fixtures.json"));var camera=new GameObject("MF diagnostic motion camera").AddComponent<Camera>();camera.enabled=false;camera.cullingMask=1<<31;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=Color.black;camera.orthographic=true;camera.orthographicSize=8*root.lossyScale.y;camera.aspect=1;camera.nearClipPlane=.001f;camera.farClipPlane=20;
  var center=root.TransformPoint(new Vector3(0,-86*.25f,0));camera.transform.position=center-root.forward*50*root.lossyScale.z;camera.transform.LookAt(center,root.up);
  var target=new RenderTexture(512,512,24,RenderTextureFormat.ARGB32);target.antiAliasing=4;target.Create();camera.targetTexture=target;var rows=new JArray();
  using(var udp=new UdpClient())try{
   r.gameObject.layer=31;
   async Task<JObject> Capture(string name){var data=fixture["controls"][name].Select(v=>(byte)(int)v).ToArray();udp.Send(data,data.Length,"127.0.0.1",9012);await Task.Delay(450);camera.Render();var old=RenderTexture.active;RenderTexture.active=target;var image=new Texture2D(512,512,TextureFormat.RGB24,false);image.ReadPixels(new Rect(0,0,512,512),0,0);image.Apply();RenderTexture.active=old;File.WriteAllBytes("Docs/TextAnimation/native-motion-"+name+".png",image.EncodeToPNG());var colors=image.GetPixels32();var pixels=Enumerable.Range(0,colors.Length).Where(i=>colors[i].r>50&&colors[i].r>colors[i].g*1.3f).ToArray();var row=new JObject{["name"]=name,["inkPixels"]=pixels.Length,["width"]=pixels.Length==0?0:pixels.Max(i=>i%512)-pixels.Min(i=>i%512)+1,["height"]=pixels.Length==0?0:pixels.Max(i=>i/512)-pixels.Min(i=>i/512)+1};UnityEngine.Object.DestroyImmediate(image);return row;}
   var zero=await Capture("zero");var quarter=await Capture("quarter");var stop=await Capture("stop");rows.Add(zero);rows.Add(quarter);rows.Add(stop);
   var result=new JObject{["success"]=(int)zero["inkPixels"]>100&&(int)quarter["inkPixels"]>50&&(int)zero["width"]>2*(int)quarter["width"]&&Math.Abs((int)stop["width"]-(int)zero["width"])<4,["captures"]=rows,["scope"]="Actual received Box particle + production shader GPU at zero, quarter-turn and stopped static pose; isolated diagnostic camera/layer restored."};File.WriteAllText("Docs/TextAnimation/native-motion-visual.json",result.ToString());return result.ToString();
  }finally{r.gameObject.layer=layer;camera.targetTexture=null;target.Release();UnityEngine.Object.DestroyImmediate(target);UnityEngine.Object.DestroyImmediate(camera.gameObject);}
 }
 public static async Task<string> Dynamics(){
  if(!EditorApplication.isPlaying)throw new Exception("Play Mode required");var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single();var m=gm.Module as ModuleVrc3;if(m==null)throw new Exception("GestureManager not ready");
  var fixture=JObject.Parse(File.ReadAllText("E:/tmp/morphforge/avatar-motion-fixtures.json"));var rows=new JArray();
  var r=m.Avatar.GetComponentsInChildren<ParticleSystemRenderer>(true).Single(x=>x.name=="Store"&&x.transform.parent.name=="Pool0");
  float P(string n)=>m.GetParam("MF6_M1_"+n).FloatValue();
  void Check(string name,bool pass,JToken data=null){rows.Add(new JObject{["name"]=name,["pass"]=pass,["data"]=data});}
  using(var udp=new UdpClient()){
   async Task Send(JToken data){var bytes=data.Select(v=>(byte)(int)v).ToArray();udp.Send(bytes,bytes.Length,"127.0.0.1",9012);await Task.Delay(450);}
   async Task Control(string name){await Send(fixture["controls"][name]);}
   await Control("zero");Check("zero paused phase",Mathf.Abs(Value(r,"_MF_M1_Base"))<.001&&Mathf.Abs(P("Rate"))<.001);
   await Control("play");var a=Value(r,"_MF_M1_Elapsed");await Task.Delay(500);var b=Value(r,"_MF_M1_Elapsed");Check("native clock advances at quarter cycles per second",b-a>.09&&b-a<.18,new JArray(a,b));
   var oldBase=P("Base");await Control("duplicate");var c=Value(r,"_MF_M1_Elapsed");Check("duplicate ticket cannot seek or restart",Mathf.Abs(P("Base")-oldBase)<.001&&c>b,new JArray(b,c,P("Base")));
   await Control("pause");a=Value(r,"_MF_M1_Elapsed");await Task.Delay(350);b=Value(r,"_MF_M1_Elapsed");Check("pause holds at requested half phase",Mathf.Abs(P("Base")-128f/255)<.001&&Mathf.Abs(a-b)<.001,new JArray(a,b));
   await Control("resume");a=Value(r,"_MF_M1_Elapsed");await Task.Delay(350);b=Value(r,"_MF_M1_Elapsed");Check("resume advances from preserved phase",Mathf.Abs(P("Base")-128f/255)<.001&&b-a>.06,new JArray(a,b));
   await Control("stop");Check("stop returns to static pose",P("Enabled")==0&&P("Rate")==0&&Value(r,"_MF_M1_Enabled")<.01);
   var bank=fixture["bank1"].ToArray();foreach(var item in bank.Take(bank.Length-1))await Send(item);
   await Control("bank1");Check("incomplete alternate bank cannot commit",P("B1_Ready")==0&&P("AppliedTicket")==7&&P("Bank")==0);
   await Send(bank.Last());await Control("bank1");Check("completed alternate bank switches atomically",P("B1_Ready")==1&&P("AppliedTicket")==8&&P("Bank")==1&&Mathf.Abs(P("Base")-64f/255)<.001);
  }
  var result=new JObject{["success"]=rows.All(c=>(bool)c["pass"]),["checks"]=rows,["scope"]="Actual UDP9012 native Animator clock, ticket idempotency and missing-key bank gating. No remote network guarantee."};File.WriteAllText("Docs/TextAnimation/native-motion-dynamics.json",result.ToString());return result.ToString();
 }
 public static async Task<string> Run(){
  if(!EditorApplication.isPlaying)throw new Exception("Play Mode required");var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single();var deadline=DateTime.UtcNow.AddSeconds(15);while(gm.Module==null&&DateTime.UtcNow<deadline)await Task.Delay(100);var m=gm.Module as ModuleVrc3;if(m==null)throw new Exception("GestureManager not ready");var receiver=gm.GetComponent<MorphForgePlayReceiver>();if(!receiver.Running||receiver.ReceivePort!=9012)throw new Exception("Isolated OSC9012 required");
  var fixture=JObject.Parse(File.ReadAllText("E:/tmp/morphforge/avatar-motion-fixtures.json"));var checks=new JArray();void Check(string name,bool pass,JToken data=null){checks.Add(new JObject{["name"]=name,["pass"]=pass,["data"]=data});}
  using(var udp=new UdpClient())foreach(var item in fixture["packets"]){var bytes=item["packet"].Select(v=>(byte)(int)v).ToArray();udp.Send(bytes,bytes.Length,"127.0.0.1",9012);await Task.Delay(400);}
  await Task.Delay(500);
  var store=m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Single(p=>p.name=="Store"&&p.transform.parent.name=="Pool0");var renderer=store.GetComponent<ParticleSystemRenderer>();
  for(int g=1;g<=2;g++){
   var fields=new JObject();foreach(string key in new[]{"B0_Version","B0_Ready","AppliedTicket","ClockTicket","Enabled","Base","Rate"})fields[key]=m.GetParam("MF6_M"+g+"_"+key).FloatValue();
   Check("group "+g+" complete paused definition",(int)fields["B0_Ready"]==1&&(int)fields["AppliedTicket"]==1&&(int)fields["ClockTicket"]==1&&Mathf.Abs((float)fields["Base"]-64f/255)<.001f&&Mathf.Abs((float)fields["Rate"])<.001f,fields);
   Check("group "+g+" renderer definition and clock",Mathf.Abs(Value(renderer,"_MF_M"+g+"_Enabled")-1)<.01f&&Mathf.Abs(Value(renderer,"_MF_M"+g+"_Base")-64f/255)<.001f&&Value(renderer,"_MF_M"+g+"B0_Count")>1,new JObject{["enabled"]=Value(renderer,"_MF_M"+g+"_Enabled"),["base"]=Value(renderer,"_MF_M"+g+"_Base"),["count"]=Value(renderer,"_MF_M"+g+"B0_Count"),["elapsed"]=Value(renderer,"_MF_M"+g+"_Elapsed")});
  }
  var particles=new ParticleSystem.Particle[store.main.maxParticles];var count=store.GetParticles(particles);var box=particles.Take(count).FirstOrDefault(p=>p.startColor.a==214);Check("animated box birth tag",box.startColor.a==214,new JValue(count));
  var r=box.rotation3D*Mathf.Deg2Rad;int z=Mathf.RoundToInt(r.x*1048576),xy=Mathf.RoundToInt(r.z*1048576);Check("animated box original center persists",z==256&&(xy&511)==256&&(xy>>9)==84,new JArray(xy&511,xy>>9,z));
  var triStore=m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Single(p=>p.name=="Store"&&p.transform.parent.name=="Pool1");count=triStore.GetParticles(particles);Check("animated triangle birth tag",particles.Take(count).Any(p=>p.startColor.a==14),new JArray(particles.Take(count).Select(p=>(int)p.startColor.a)));
  var panel=m.Avatar.GetComponentsInChildren<MeshRenderer>(true).Single(rn=>rn.name=="Panel0"&&rn.transform.parent.name=="TextDisplay");Check("text group receives shared pose",Mathf.Abs(Value(panel,"_MotionGroup")-1)<.01f&&Value(panel,"_MF_M1B0_Count")>1,new JObject{["group"]=Value(panel,"_MotionGroup"),["keys"]=Value(panel,"_MF_M1B0_Count")});
  var result=new JObject{["success"]=checks.All(c=>(bool)c["pass"]),["checks"]=checks,["scope"]="Actual UDP9012 -> merged native Avatar receiver/caches/particle birth streams. Fixed paused pose; dynamic and GPU pose tests are separate."};Directory.CreateDirectory("Docs/TextAnimation");File.WriteAllText("Docs/TextAnimation/native-motion-play.json",result.ToString());return result.ToString();
 }
}
