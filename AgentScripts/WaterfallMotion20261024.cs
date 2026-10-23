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
 static Vector3 ExpectedCenter(JObject values,int group,int bank){
  string p="_MF_M"+group+"B"+bank+"_";float F(string suffix)=>(float)values[p+suffix];
  Vector3 V(string suffix)=>new Vector3(F(suffix+"X"),F(suffix+"Y"),F(suffix+"Z"));
  Quaternion Q(int k){string n="K"+k+"Q";var q=new Quaternion(F(n+"X"),F(n+"Y"),F(n+"Z"),F(n+"W"));float length=Mathf.Sqrt(q.x*q.x+q.y*q.y+q.z*q.z+q.w*q.w);return new Quaternion(q.x/length,q.y/length,q.z/length,q.w/length);}
  int count=Mathf.RoundToInt(F("Count")),a=0;float phase=128f/255f;for(int k=1;k<count;k++)if(phase>=F("K"+k+"T"))a=k;int z=Math.Min(a+1,count-1);
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
  if(!EditorApplication.isPlaying||SceneManager.GetActiveScene().path!=Scene)throw new Exception("Owned Play Mode required");
  var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single(g=>g.gameObject.scene.path==Scene);
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
     float actual=Value(renderers[0],p.Name),wanted=(float)p.Value;if(Mathf.Abs(actual-wanted)>.025f)errors.Add(new JObject{["property"]=p.Name,["expected"]=wanted,["actual"]=actual});
    }
    var gpu=new JArray();foreach(string view in new[]{"front","top"}){
     var center=root.TransformPoint(new Vector3(0,(45-128)*cell,8*cell));camera.transform.position=center+(view=="front"?-root.forward:root.up)*100*cell*root.lossyScale.y;camera.transform.LookAt(center,view=="front"?root.up:root.forward);camera.Render();var old=RenderTexture.active;RenderTexture.active=rt;image.ReadPixels(new Rect(0,0,384,384),0,0);image.Apply();RenderTexture.active=old;var pixels=image.GetPixels32();
     var colors=new JArray();for(int g=0;g<3;g++){var ids=Enumerable.Range(0,pixels.Length).Where(i=>{var c=pixels[i];float[] a={c.r,c.g,c.b};return a[g]>25&&a[g]>a[(g+1)%3]*1.4f&&a[g]>a[(g+2)%3]*1.4f;}).ToArray();var expectedCenter=ExpectedCenter(expected,g+1,index%2);var projected=camera.WorldToScreenPoint(root.TransformPoint((expectedCenter-new Vector3(0,128,0))*cell));float x=ids.Length==0?-1:(float)ids.Average(i=>i%384),y=ids.Length==0?-1:(float)ids.Average(i=>i/384);float error=Vector2.Distance(new Vector2(x,y),new Vector2(projected.x,projected.y));colors.Add(new JObject{["group"]=g+1,["pixels"]=ids.Length,["x"]=x,["y"]=y,["expectedX"]=projected.x,["expectedY"]=projected.y,["pixelError"]=error,["pass"]=ids.Length>50&&error<3});}
     File.WriteAllBytes(Reports+"/stage"+index+"-"+view+".png",image.EncodeToPNG());gpu.Add(new JObject{["view"]=view,["colors"]=colors});
    }
    var caches=new JObject();for(int g=1;g<=3;g++){var bank=module.GetParam("MF6_M"+g+"_Bank").FloatValue();var prefix="MF6_M"+g+"_B"+Mathf.RoundToInt(bank)+"_";var values=new JObject();foreach(int i in new[]{16,18,20,22,24,26,28,30,32,34,36,38,40,42,44,46,48,50})values["C"+i.ToString("D2")]=module.GetParam(prefix+"C"+i.ToString("D2")).FloatValue();caches["group"+g]=values;}
    rows.Add(new JObject{["name"]=stage["name"],["uniformMismatches"]=errors,["gpu"]=gpu,["cacheWords"]=caches});index++;
    Save("transition-progress",new JObject{["stages"]=rows});
   }
   return Save("transition",new JObject{["success"]=rows.All(r=>!r["uniformMismatches"].Any()&&r["gpu"].All(v=>v["colors"].All(c=>(bool)c["pass"]))),["stages"]=rows,["scope"]="Production paced packets to isolated native Avatar; replaces windmill rotation/X motion with Y-/Z+ waterfall movement across banks and reused banks. GPU centroids compared with authored transforms from front and top. No material or AAP writes."});
  }finally{for(int i=0;i<renderers.Length;i++)renderers[i].gameObject.layer=layers[i];camera.targetTexture=null;rt.Release();UnityEngine.Object.DestroyImmediate(image);UnityEngine.Object.DestroyImmediate(rt);UnityEngine.Object.DestroyImmediate(camera.gameObject);}
 }
 public static string Restore(){
  if(EditorApplication.isPlaying||SceneManager.GetActiveScene().path!=Scene)throw new Exception("Stop owned Play Mode first");
  var current=SceneManager.GetActiveScene();if(current.isDirty&&!EditorSceneManager.SaveScene(current,Path.GetDirectoryName(Scene).Replace('\\','/')+"/AfterPlay.unity",true))throw new Exception("Could not preserve diagnostic changes");
  EditorSceneManager.OpenScene(Mf6AvatarBuild.ScenePath,OpenSceneMode.Single);var baseline=JObject.Parse(File.ReadAllText(Reports+"/prepare.json"));bool preserved=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString()==(string)baseline["prefabHash"]&&AssetDatabase.GetAssetDependencyHash(AvatarSetupWindow.PrefabPath).ToString()==(string)baseline["publicHash"];return Save("restore",new JObject{["success"]=preserved&&!SceneManager.GetActiveScene().isDirty,["scene"]=SceneManager.GetActiveScene().path,["sourceAssetsUnchanged"]=preserved,["sourceSceneClean"]=!SceneManager.GetActiveScene().isDirty});
 }
}
