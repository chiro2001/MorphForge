using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using System.Net.Sockets;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using MorphForge.Editor;
using MorphForgeAvatar.Editor;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;

public static class WaterfallMotion20261024 {
 const string Scene="Assets/MorphForge/Validation/WaterfallMotion20261024/Baseline.unity";
 const string Reports="Docs/WaterfallMotion20261024";
 static string Save(string file,JObject value){Directory.CreateDirectory(Reports);File.WriteAllText(Reports+"/"+file+".json",value.ToString());return value.ToString();}
 static float Value(Renderer r,string prop){var b=new MaterialPropertyBlock();r.GetPropertyBlock(b);return b.HasFloat(prop)?b.GetFloat(prop):r.sharedMaterial.GetFloat(prop);}
 static Vector3 ExpectedCenter(JObject values,int group,int bank,float phase=128f/255f){
  string p="_MF_M"+group+"B"+bank+"_";float F(string suffix)=>(float)values[p+suffix];
  Vector3 V(string suffix)=>new Vector3(F(suffix+"X"),F(suffix+"Y"),F(suffix+"Z"));
  Quaternion Q(int k){string n="K"+k+"Q";var q=new Quaternion(F(n+"X"),F(n+"Y"),F(n+"Z"),F(n+"W"));float length=Mathf.Sqrt(q.x*q.x+q.y*q.y+q.z*q.z+q.w*q.w);return new Quaternion(q.x/length,q.y/length,q.z/length,q.w/length);}
  int count=Mathf.RoundToInt(F("Count")),a=0;for(int k=1;k<count;k++)if(phase>=F("K"+k+"T"))a=k;int z=Math.Min(a+1,count-1);
  float t=a==z?0:Mathf.Clamp01((phase-F("K"+a+"T"))/(F("K"+z+"T")-F("K"+a+"T")));
  Vector3 pivot=V("Pivot"),translation=Vector3.Lerp(V("K"+a+"P"),V("K"+z+"P"),t);float scale=Mathf.Lerp(F("K"+a+"S"),F("K"+z+"S"),t);
  return pivot+translation+Quaternion.Slerp(Q(a),Q(z),t)*((new Vector3(-21+(group-1)*20,55,group==3?24:0)-pivot)*scale);
 }
 public static string Prepare(){
  if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");
  for(int i=0;i<SceneManager.sceneCount;i++)if(SceneManager.GetSceneAt(i).isDirty)throw new Exception("User unsaved scene preserved");
  var source=SceneManager.GetActiveScene();if(source.path!=Mf6AvatarBuild.ScenePath)throw new Exception("Unexpected source scene preserved");
  CatalogVariant.EnsureFolder(Path.GetDirectoryName(Scene).Replace('\\','/'));
  if(!File.Exists(Scene)&&!EditorSceneManager.SaveScene(source,Scene,true))throw new Exception("Could not create diagnostic scene copy");
  var report=new JObject{["success"]=true,["source"]=source.path,["prefabHash"]=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString(),["publicHash"]=AssetDatabase.GetAssetDependencyHash(AvatarSetupWindow.PrefabPath).ToString()};
  EditorSceneManager.OpenScene(Scene,OpenSceneMode.Single);return Save("prepare",report);
 }
 public static async Task<string> Run(){
  string activeScene=SceneManager.GetActiveScene().path;bool previous=activeScene=="Assets/MorphForge/Validation/Performance20261024/previous-motion.unity"||activeScene=="Assets/MorphForge/Validation/Performance20261024/previous-full.unity";bool sdk=activeScene=="Assets/MorphForge/Validation/WaterfallSdk20261024/Baseline.unity"||previous;
  if(!EditorApplication.isPlaying||(activeScene!=Scene&&!sdk))throw new Exception("Owned Play Mode required");
  string reportPrefix=previous?"previous-":sdk?"sdk-":"";
  var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single(g=>g.gameObject.scene.path==activeScene);
  for(int i=0;i<150&&!(gm.Module is ModuleVrc3);i++)await Task.Delay(100);
  var module=gm.Module as ModuleVrc3;if(module==null)throw new Exception("GestureManager not ready");
  var receiver=gm.GetComponent<MorphForgePlayReceiver>();if(receiver.ReceivePort!=9012)throw new Exception("Isolated port9012 required");if(!receiver.Running)receiver.TryStart();if(!receiver.Running)throw new Exception(receiver.Error);
  var stores=module.Avatar.GetComponentsInChildren<ParticleSystem>(true).Where(p=>p.name=="Store"&&p.transform.parent.name.StartsWith("Pool")).ToArray();
  var renderers=stores.Select(p=>p.GetComponent<ParticleSystemRenderer>()).ToArray();var root=stores[0].main.customSimulationSpace;float cell=renderers[0].sharedMaterial.GetFloat("_CellSize");
  var layers=renderers.Select(r=>r.gameObject.layer).ToArray();var camera=new GameObject("Waterfall axis diagnostic camera").AddComponent<Camera>();camera.enabled=false;camera.cullingMask=1<<31;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=Color.black;camera.orthographic=true;camera.orthographicSize=35*cell*root.lossyScale.y;camera.aspect=1;camera.nearClipPlane=.001f;camera.farClipPlane=100;
  var rt=new RenderTexture(384,384,24,RenderTextureFormat.ARGB32);rt.Create();camera.targetTexture=rt;var image=new Texture2D(384,384,TextureFormat.RGB24,false);var rows=new JArray();
  using(var udp=new UdpClient())try{
   foreach(var r in renderers)r.gameObject.layer=31;
   var fixture=JObject.Parse(File.ReadAllText("E:/tmp/morphforge/motion-transition-fixtures.json"));int index=0;
   foreach(var stage in fixture["stages"]){
    var clock=System.Diagnostics.Stopwatch.StartNew();foreach(var packet in stage["packets"]){int delay=(int)((double)packet["time"]*1000-clock.Elapsed.TotalMilliseconds);if(delay>0)await Task.Delay(delay);var bytes=packet["packet"].Select(v=>(byte)(int)v).ToArray();udp.Send(bytes,bytes.Length,"127.0.0.1",9012);}await Task.Delay(500);
    var errors=new JArray();var expected=(JObject)stage["expectedUniforms"];foreach(var p in expected.Properties()){
     // Transactional bank identity remains in the receiver; the renderer now
     // consumes a compact B0 view of whichever complete bank was selected.
     if(!previous&&p.Name.EndsWith("_Bank")){float selected=module.GetParam(p.Name.Replace("_MF_","MF6_")).FloatValue();if(Mathf.Abs(selected-(float)p.Value)>.01f)errors.Add(new JObject{["property"]=p.Name+" receiver selection",["expected"]=p.Value,["actual"]=selected});}
     string property=previous?p.Name:System.Text.RegularExpressions.Regex.Replace(p.Name,@"B[01]_","B0_");
     float actual=Value(renderers[0],property),wanted=!previous&&p.Name.EndsWith("_Bank")?0:(float)p.Value;if(Mathf.Abs(actual-wanted)>.025f)errors.Add(new JObject{["property"]=property,["sourceProperty"]=p.Name,["expected"]=wanted,["actual"]=actual});
    }
    var gpu=new JArray();foreach(string view in new[]{"front","top"}){
     var center=root.TransformPoint(new Vector3(0,(45-128)*cell,8*cell));camera.transform.position=center+(view=="front"?-root.forward:root.up)*100*cell*root.lossyScale.y;camera.transform.LookAt(center,view=="front"?root.up:root.forward);camera.Render();var old=RenderTexture.active;RenderTexture.active=rt;image.ReadPixels(new Rect(0,0,384,384),0,0);image.Apply();RenderTexture.active=old;var pixels=image.GetPixels32();
     var colors=new JArray();for(int g=0;g<3;g++){var ids=Enumerable.Range(0,pixels.Length).Where(i=>{var c=pixels[i];float[] a={c.r,c.g,c.b};return a[g]>25&&a[g]>a[(g+1)%3]*1.4f&&a[g]>a[(g+2)%3]*1.4f;}).ToArray();var expectedCenter=ExpectedCenter(expected,g+1,index%2);var projected=camera.WorldToScreenPoint(root.TransformPoint((expectedCenter-new Vector3(0,128,0))*cell));float x=ids.Length==0?-1:(float)ids.Average(i=>i%384),y=ids.Length==0?-1:(float)ids.Average(i=>i/384);float error=Vector2.Distance(new Vector2(x,y),new Vector2(projected.x,projected.y));colors.Add(new JObject{["group"]=g+1,["pixels"]=ids.Length,["x"]=x,["y"]=y,["expectedX"]=projected.x,["expectedY"]=projected.y,["pixelError"]=error,["pass"]=ids.Length>50&&error<3});}
     File.WriteAllBytes(Reports+"/"+reportPrefix+"stage"+index+"-"+view+".png",image.EncodeToPNG());gpu.Add(new JObject{["view"]=view,["colors"]=colors});
    }
    var caches=new JObject();for(int g=1;g<=3;g++){var bank=module.GetParam("MF6_M"+g+"_Bank").FloatValue();var prefix="MF6_M"+g+"_B"+Mathf.RoundToInt(bank)+"_";var values=new JObject{["selectedBank"]=bank,["gate0"]=module.GetParam("MF6_M"+g+"_B0_Gate").FloatValue(),["gate1"]=module.GetParam("MF6_M"+g+"_B1_Gate").FloatValue()};foreach(int i in new[]{16,18,20,22,24,26,28,30,32,34,36,38,40,42,44,46,48,50})values["C"+i.ToString("D2")]=module.GetParam(prefix+"C"+i.ToString("D2")).FloatValue();caches["group"+g]=values;}
    rows.Add(new JObject{["name"]=stage["name"],["uniformMismatches"]=errors,["gpu"]=gpu,["cacheWords"]=caches});index++;
    Save(reportPrefix+"transition-progress",new JObject{["stages"]=rows});
   }
   return Save(reportPrefix+"transition",new JObject{["success"]=rows.All(r=>!r["uniformMismatches"].Any()&&r["gpu"].All(v=>v["colors"].All(c=>(bool)c["pass"]))),["sdkPreprocessed"]=sdk,["stages"]=rows,["scope"]="Production paced packets to isolated native Avatar; replaces windmill rotation/X motion with Y-/Z+ waterfall movement across banks and reused banks. GPU centroids compared with authored transforms from front and top. No material or AAP writes."});
  }finally{for(int i=0;i<renderers.Length;i++)renderers[i].gameObject.layer=layers[i];camera.targetTexture=null;rt.Release();UnityEngine.Object.DestroyImmediate(image);UnityEngine.Object.DestroyImmediate(rt);UnityEngine.Object.DestroyImmediate(camera.gameObject);}
 }
 public static string Restore(){
  if(EditorApplication.isPlaying||SceneManager.GetActiveScene().path!=Scene)throw new Exception("Stop owned Play Mode first");
  var current=SceneManager.GetActiveScene();if(current.isDirty&&!EditorSceneManager.SaveScene(current,Path.GetDirectoryName(Scene).Replace('\\','/')+"/AfterPlay.unity",true))throw new Exception("Could not preserve diagnostic changes");
  EditorSceneManager.OpenScene(Mf6AvatarBuild.ScenePath,OpenSceneMode.Single);var baseline=JObject.Parse(File.ReadAllText(Reports+"/prepare.json"));bool preserved=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString()==(string)baseline["prefabHash"]&&AssetDatabase.GetAssetDependencyHash(AvatarSetupWindow.PrefabPath).ToString()==(string)baseline["publicHash"];return Save("restore",new JObject{["success"]=preserved&&!SceneManager.GetActiveScene().isDirty,["scene"]=SceneManager.GetActiveScene().path,["sourceAssetsUnchanged"]=preserved,["sourceSceneClean"]=!SceneManager.GetActiveScene().isDirty});
 }
 public static async Task<string> Continuous(){
  string active=SceneManager.GetActiveScene().path;if(!EditorApplication.isPlaying||active!="Assets/MorphForge/Validation/WaterfallSdk20261024/Baseline.unity")throw new Exception("Owned SDK Play Mode required");
  var module=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single(g=>g.gameObject.scene.path==active).Module as ModuleVrc3;
  var receiver=UnityEngine.Object.FindObjectsOfType<MorphForgePlayReceiver>().Single(r=>r.gameObject.scene.path==active);if(!receiver.Running||receiver.ReceivePort!=9012)throw new Exception("Isolated receiver required");
  var stores=module.Avatar.GetComponentsInChildren<ParticleSystem>(true).Where(p=>p.name=="Store"&&p.transform.parent.name.StartsWith("Pool")).ToArray();var renderers=stores.Select(p=>p.GetComponent<ParticleSystemRenderer>()).ToArray();var root=stores[0].main.customSimulationSpace;float cell=renderers[0].sharedMaterial.GetFloat("_CellSize");
  var fixture=JObject.Parse(File.ReadAllText("E:/tmp/morphforge/motion-transition-fixtures.json"));var expected=(JObject)fixture["stages"].Last["expectedUniforms"];
  using(var udp=new UdpClient()){var clock=System.Diagnostics.Stopwatch.StartNew();foreach(var p in fixture["playback"]){int delay=(int)((double)p["time"]*1000-clock.Elapsed.TotalMilliseconds);if(delay>0)await Task.Delay(delay);var data=p["packet"].Select(v=>(byte)(int)v).ToArray();udp.Send(data,data.Length,"127.0.0.1",9012);}}
  await Task.Delay(350);
  var layers=renderers.Select(r=>r.gameObject.layer).ToArray();var camera=new GameObject("SDK waterfall continuous camera").AddComponent<Camera>();camera.enabled=false;camera.cullingMask=1<<31;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=Color.black;camera.orthographic=true;camera.orthographicSize=35*cell*root.lossyScale.y;camera.aspect=1;camera.nearClipPlane=.001f;camera.farClipPlane=100;
  var rt=new RenderTexture(192,192,24,RenderTextureFormat.ARGB32);rt.Create();camera.targetTexture=rt;var image=new Texture2D(192,192,TextureFormat.RGB24,false);var rows=new JArray();var previous=new[]{-1f,-1f,-1f};var wraps=new int[3];int backwards=0;
  try{foreach(var r in renderers)r.gameObject.layer=31;var watch=System.Diagnostics.Stopwatch.StartNew();while(watch.Elapsed.TotalSeconds<6.2){
   var phases=Enumerable.Range(1,3).Select(g=>Mathf.Repeat(Value(renderers[0],"_MF_M"+g+"_Base")+Value(renderers[0],"_MF_M"+g+"_Elapsed"),1)).ToArray();for(int i=0;i<3;i++){if(previous[i]>=0){float delta=phases[i]-previous[i];if(delta<-.5f)wraps[i]++;else if(delta<-.005f)backwards++;}previous[i]=phases[i];}
   var samples=new JArray();foreach(string view in new[]{"front","top"}){int channel=view=="front"?0:2,group=channel==0?1:3;float phase=Mathf.Repeat(Value(renderers[0],"_MF_M"+group+"_Base")+Value(renderers[0],"_MF_M"+group+"_Elapsed"),1);
    var center=root.TransformPoint(new Vector3(0,(45-128)*cell,24*cell));camera.transform.position=center+(view=="front"?-root.forward:root.up)*100*cell*root.lossyScale.y;camera.transform.LookAt(center,view=="front"?root.up:root.forward);camera.Render();var old=RenderTexture.active;RenderTexture.active=rt;image.ReadPixels(new Rect(0,0,192,192),0,0);image.Apply();RenderTexture.active=old;var pixels=image.GetPixels32();var ids=Enumerable.Range(0,pixels.Length).Where(i=>{var c=pixels[i];float[] v={c.r,c.g,c.b};return v[channel]>25&&v[channel]>v[(channel+1)%3]*1.4f&&v[channel]>v[(channel+2)%3]*1.4f;}).ToArray();
    var wanted=camera.WorldToScreenPoint(root.TransformPoint((ExpectedCenter(expected,group,1,phase)-new Vector3(0,128,0))*cell));float x=ids.Length==0?-1:(float)ids.Average(i=>i%192),y=ids.Length==0?-1:(float)ids.Average(i=>i/192);float error=Vector2.Distance(new Vector2(x,y),new Vector2(wanted.x,wanted.y));samples.Add(new JObject{["view"]=view,["group"]=group,["phase"]=phase,["pixels"]=ids.Length,["pixelError"]=error,["x"]=x,["y"]=y,["expectedX"]=wanted.x,["expectedY"]=wanted.y,["pass"]=ids.Length>30&&error<3});
   }
   rows.Add(new JObject{["seconds"]=watch.Elapsed.TotalSeconds,["phases"]=new JArray(phases),["gpu"]=samples});await Task.Delay(100);
  }
  return Save("sdk-continuous",new JObject{["success"]=wraps.All(w=>w>=2)&&backwards==0&&rows.All(r=>r["gpu"].All(g=>(bool)g["pass"])),["wraps"]=new JArray(wraps),["backwardsSteps"]=backwards,["samples"]=rows,["scope"]="SDK-processed Avatar, production resume packets, freely advancing native clocks through at least two complete sawtooth cycles. GPU Y-/Z+ translation checked against current phase; no manual phase/material/cache writes."});
  }finally{for(int i=0;i<renderers.Length;i++)renderers[i].gameObject.layer=layers[i];camera.targetTexture=null;rt.Release();UnityEngine.Object.DestroyImmediate(image);UnityEngine.Object.DestroyImmediate(rt);UnityEngine.Object.DestroyImmediate(camera.gameObject);}
 }
}
