using System;
using System.IO;
using System.Linq;
using System.Collections;
using System.Reflection;
using System.Threading.Tasks;
using System.Net.Sockets;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEngine;
using UnityEngine.Animations;
using UnityEngine.SceneManagement;
using Unity.Profiling;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
using MorphForge.Editor;

public static class PerformanceAblation20261024 {
 const string Scene="Assets/MorphForge/Validation/WaterfallSdk20261024/Baseline.unity";
 const string Reports="Docs/LivePerformance20261024";
 public static async Task<string> Clear(){
  string scene=SceneManager.GetActiveScene().path;
  if(!EditorApplication.isPlaying||(scene!=Scene&&!scene.StartsWith("Assets/MorphForge/Validation/Performance20261024/")))throw new Exception("Owned scene required");
  var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single(g=>g.gameObject.scene.path==scene);
  var source=gm.GetComponent<MorphForgePlayReceiver>();if(!source.Running||source.ReceivePort!=9012)throw new Exception("Isolated port9012 required");
  var fixture=JObject.Parse(File.ReadAllText("E:/tmp/morphforge/avatar-motion-fixtures.json"));var packet=fixture["packets"][0]["packet"].Select(v=>(byte)(int)v).ToArray();
  using(var udp=new UdpClient())udp.Send(packet,packet.Length,"127.0.0.1",9012);await Task.Delay(600);return "Cleared owned native fixture";
 }
 public static async Task<string> Measure(string label){
  return await Run(label,false);
 }
 public static async Task<string> Baseline(string label){return await Run(label,true);}
 static async Task<string> Run(string label,bool baselineOnly){
  string scene=SceneManager.GetActiveScene().path;
  if(!EditorApplication.isPlaying||(scene!=Scene&&!scene.StartsWith("Assets/MorphForge/Validation/Performance20261024/")))throw new Exception("Owned SDK diagnostic Play Mode required");
  var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single(g=>g.gameObject.scene.path==scene);
  for(int i=0;i<100&&!(gm.Module is ModuleVrc3);i++)await Task.Delay(100);
  var m=gm.Module as ModuleVrc3;if(m==null)throw new Exception("GestureManager not ready");
  var flags=BindingFlags.Public|BindingFlags.NonPublic|BindingFlags.Instance;
  var layers=(IDictionary)m.GetType().GetField("_layers",flags).GetValue(m);
  var key=layers.Keys.Cast<object>().Single(k=>k.ToString()=="FX");
  var fx=(AnimatorControllerPlayable)layers[key].GetType().GetField("Playable",flags).GetValue(layers[key]);
  var names=Enumerable.Range(0,fx.GetLayerCount()).Select(fx.GetLayerName).ToArray();
  var weights=Enumerable.Range(0,names.Length).Select(fx.GetLayerWeight).ToArray();
  var renderers=m.Avatar.GetComponentsInChildren<Renderer>(true).Where(r=>r.sharedMaterials.Any(mat=>mat&&mat.shader&&mat.shader.name.StartsWith("MorphForge"))).ToArray();
  var enabled=renderers.Select(r=>r.enabled).ToArray();var rows=new JArray();
  bool Has(string value,string part)=>value.IndexOf(part,StringComparison.OrdinalIgnoreCase)>=0;
  var phases=baselineOnly?new[]{"baseline"}:new[]{"baseline","motion off","text off","geometry workers off","all MF data off","renderers off","baseline repeat"};
  try{foreach(string phase in phases){
   for(int i=0;i<names.Length;i++){
    bool motion=Has(names[i],"Motion "),text=Has(names[i],"Text ");
    bool geometry=Has(names[i],"MF6 immutable")||Has(names[i],"MF6 local inbox")||Has(names[i],"MF6 native lifecycle");
    bool all=motion||text||geometry||Has(names[i],"MF6 ");
    bool off=phase=="motion off"&&motion||phase=="text off"&&text||phase=="geometry workers off"&&geometry||phase=="all MF data off"&&all;
    fx.SetLayerWeight(i,off?0:weights[i]);
   }
   for(int i=0;i<renderers.Length;i++)renderers[i].enabled=phase=="renderers off"?false:enabled[i];
   await Task.Delay(900);
   using(var recorder=ProfilerRecorder.StartNew(ProfilerCategory.Animation,"Animators.Update",256)){
    await Task.Delay(2400);var values=Enumerable.Range(0,recorder.Count).Select(i=>recorder.GetSample(i).Value).Where(v=>v>0).OrderBy(v=>v).ToArray();
    rows.Add(new JObject{["phase"]=phase,["samples"]=values.Length,["animatorMeanMs"]=values.Length==0?0:values.Average()/1e6,["animatorMedianMs"]=values.Length==0?0:values[values.Length/2]/1e6,["animatorP95Ms"]=values.Length==0?0:values[(int)((values.Length-1)*.95)]/1e6,["cameraEnabled"]=m.Avatar.GetComponentsInChildren<Camera>(true).Any(c=>c.isActiveAndEnabled)});
   }
   Directory.CreateDirectory(Reports);File.WriteAllText(Reports+"/"+label+"-progress.json",new JObject{["rows"]=rows}.ToString());
  }}finally{for(int i=0;i<weights.Length;i++)fx.SetLayerWeight(i,weights[i]);for(int i=0;i<renderers.Length;i++)renderers[i].enabled=enabled[i];}
  var report=new JObject{["success"]=rows.All(r=>(int)r["samples"]>0),["label"]=label,["batchMode"]=Application.isBatchMode,["graphicsDevice"]=SystemInfo.graphicsDeviceType.ToString(),["targetFrameRate"]=Application.targetFrameRate,["vSyncCount"]=QualitySettings.vSyncCount,["layers"]=new JArray(names),["rows"]=rows,["scope"]="SDK-preprocessed native Avatar in owned Editor scene, paired layer-weight ablation. Editor animator CPU timing only; not VRChat/headset FPS."};
  File.WriteAllText(Reports+"/"+label+".json",report.ToString());return new JObject{["success"]=report["success"],["rows"]=rows}.ToString();
 }
}
