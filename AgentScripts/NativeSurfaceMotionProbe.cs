using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using UnityEngine;
using UnityEditor;
using Newtonsoft.Json.Linq;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
public static class NativeSurfaceMotionProbe {
 public static async Task<string> Run(){
  if(!EditorApplication.isPlaying)throw new Exception("Play Mode required");var m=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single().Module as ModuleVrc3;
  var names=new[]{"MF6_M2_Base","MF6_M1_Enabled","MF6_M1_Bank","MF6_M1_B0_Gate","MF6_M1_B1_Gate","MF6_M1_Base","MF6_M1_Loop","MF6_M1_Rate"};var old=names.Select(n=>m.GetParam(n).FloatValue()).ToArray();void Set(string n,float v){m.GetParam(n).Set(m,v);}var rows=new JArray();
  async Task<JObject> Capture(Renderer renderer,Transform root,Vector3 origin,float halfHeight,int width,int height,string name,bool text){
   var layer=renderer.gameObject.layer;var go=new GameObject("MF surface motion GPU camera");var camera=go.AddComponent<Camera>();camera.enabled=false;camera.cullingMask=1<<31;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=Color.black;camera.orthographic=true;camera.orthographicSize=halfHeight*root.lossyScale.y;camera.aspect=(float)width/height;camera.nearClipPlane=.0001f;camera.farClipPlane=20;var center=root.TransformPoint(origin);camera.transform.position=center-root.forward*50*root.lossyScale.z;camera.transform.LookAt(center,root.up);var target=new RenderTexture(width,height,24,RenderTextureFormat.ARGB32);target.antiAliasing=4;target.Create();camera.targetTexture=target;
   try{renderer.gameObject.layer=31;await Task.Delay(180);camera.Render();var previous=RenderTexture.active;RenderTexture.active=target;var image=new Texture2D(width,height,TextureFormat.RGB24,false);image.ReadPixels(new Rect(0,0,width,height),0,0);image.Apply();RenderTexture.active=previous;File.WriteAllBytes("Docs/TextAnimation/"+name+".png",image.EncodeToPNG());var colors=image.GetPixels32();var pixels=Enumerable.Range(0,colors.Length).Where(i=>text?colors[i].r>140&&colors[i].g>140&&colors[i].b>140:colors[i].b>80&&colors[i].b>colors[i].r*1.3f).ToArray();var result=new JObject{["name"]=name,["ink"]=pixels.Length,["left"]=pixels.Length==0?0:pixels.Min(i=>i%width),["width"]=pixels.Length==0?0:pixels.Max(i=>i%width)-pixels.Min(i=>i%width)+1};UnityEngine.Object.DestroyImmediate(image);return result;}
   finally{renderer.gameObject.layer=layer;camera.targetTexture=null;target.Release();UnityEngine.Object.DestroyImmediate(target);UnityEngine.Object.DestroyImmediate(go);}
  }
  try{
   var ps=m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Single(p=>p.name=="Store"&&p.transform.parent.name=="Pool1");var renderer=ps.GetComponent<ParticleSystemRenderer>();var root=ps.main.customSimulationSpace;
   Set("MF6_M2_Base",0);var tri0=await Capture(renderer,root,new Vector3(25,-98,0)*.25f,4,512,512,"native-triangle-pose0",false);Set("MF6_M2_Base",.5f);var triHalf=await Capture(renderer,root,new Vector3(25,-98,0)*.25f,4,512,512,"native-triangle-poseHalf",false);rows.Add(tri0);rows.Add(triHalf);
   var panel=m.Avatar.GetComponentsInChildren<MeshRenderer>(true).Single(r=>r.name=="Panel0"&&r.transform.parent.name=="TextDisplay");Set("MF6_M1_Enabled",1);Set("MF6_M1_Bank",0);Set("MF6_M1_B0_Gate",1);Set("MF6_M1_B1_Gate",0);Set("MF6_M1_Loop",1);Set("MF6_M1_Rate",0);Set("MF6_M1_Base",0);
   var text0=await Capture(panel,panel.transform,new Vector3(0,-50,0)*.25f,3,1024,384,"native-text-pose0",true);Set("MF6_M1_Base",.25f);var textQuarter=await Capture(panel,panel.transform,new Vector3(0,-50,0)*.25f,3,1024,384,"native-text-poseQuarter",true);rows.Add(text0);rows.Add(textQuarter);
   bool triangle=(int)tri0["ink"]>1000&&(int)triHalf["ink"]>500&&Math.Abs((int)tri0["width"]-160)<5&&Math.Abs((int)triHalf["width"]-113)<6&&Math.Abs((int)triHalf["left"]-(int)tri0["left"]-64)<6;bool textPass=(int)text0["ink"]>100&&(int)textQuarter["ink"]<(int)text0["ink"]*.15;
   var result=new JObject{["success"]=triangle&&textPass,["triangleTranslationAndRotation"]=triangle,["textSharedRotation"]=textPass,["captures"]=rows,["scope"]="Actual received triangle/text definitions + production GPU shaders, isolated camera/layer. Only playback pose parameters are set for deterministic capture; UDP receiver/clock validated separately. All parameters restored."};File.WriteAllText("Docs/TextAnimation/native-motion-surfaces.json",result.ToString());return result.ToString();
  }finally{for(int i=0;i<names.Length;i++)Set(names[i],old[i]);}
 }
}
