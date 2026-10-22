using System;
using System.IO;
using System.Text;
using System.Linq;
using System.Net.Sockets;
using System.Threading.Tasks;
using UnityEditor;
using MorphForge.Editor;
using UnityEngine;
using Newtonsoft.Json.Linq;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
public static class NativeOnceProbe {
 static void Int(Stream s,int v){var b=BitConverter.GetBytes(v);if(BitConverter.IsLittleEndian)Array.Reverse(b);s.Write(b,0,4);}
 static void Str(Stream s,string v){var b=Encoding.UTF8.GetBytes(v);s.Write(b,0,b.Length);s.WriteByte(0);while(s.Position%4!=0)s.WriteByte(0);}
 static byte[] Packet(byte[] data,int seq){using(var s=new MemoryStream()){Str(s,"#bundle");for(int i=0;i<7;i++)s.WriteByte(0);s.WriteByte(1);for(int i=0;i<30;i++)using(var m=new MemoryStream()){Str(m,"/avatar/parameters/"+(i<29?"MF6_D"+i.ToString("D2"):"MF6_Ctl"));Str(m,",i");Int(m,i<29?data[i]:seq*16);var b=m.ToArray();Int(s,b.Length);s.Write(b,0,b.Length);}return s.ToArray();}}
 static void Word(byte[] d,int i,int value){d[i]=(byte)value;d[i+1]=(byte)(value>>8);}
 static byte[] Message(int op){var d=new byte[29];d[0]=(byte)op;d[1]=1;d[2]=1;d[28]=254;return d;}
 static float Value(Renderer r,string property){var b=new MaterialPropertyBlock();r.GetPropertyBlock(b);return b.HasFloat(property)?b.GetFloat(property):r.sharedMaterial.GetFloat(property);}
 public static async Task<string> Run(){
  if(!EditorApplication.isPlaying)throw new Exception("Play Mode required");var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single();var m=gm.Module as ModuleVrc3;if(m==null||gm.GetComponent<MorphForgePlayReceiver>().ReceivePort!=9012)throw new Exception("Isolated native receiver required");int seq=(m.GetParam("MF6_Seq").IntValue()+1)%16;
  using(var udp=new UdpClient()){
   async Task Send(byte[] d){var b=Packet(d,seq);seq=(seq+1)%16;udp.Send(b,b.Length,"127.0.0.1",9012);await Task.Delay(450);}
   var h=Message(16);h[3]=1;h[4]=1;h[5]=0;h[6]=2;h[7]=1;h[15]=2;for(int a=0;a<3;a++)Word(h,8+a*2,32768+(a==1?42*64:0));await Send(h);
   for(int i=0;i<2;i++){var k=Message(17);k[3]=(byte)i;k[24]=1;Word(k,4,i==0?0:65535);for(int a=0;a<3;a++)Word(k,6+a*2,32768);float angle=i*Mathf.PI/4;var q=new[]{0f,-Mathf.Sin(angle),0f,Mathf.Cos(angle)};for(int a=0;a<4;a++)Word(k,12+a*2,Mathf.RoundToInt(32768+32767*q[a]));Word(k,20,4096);await Send(k);}
   var control=Message(18);control[3]=1;control[4]=8;control[5]=1;control[6]=128;await Send(control);
  }
  var ps=m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Single(p=>p.name=="Store"&&p.transform.parent.name=="Pool0");var r=ps.GetComponent<ParticleSystemRenderer>();float first=Value(r,"_MF_M1_Elapsed");await Task.Delay(350);float later=Value(r,"_MF_M1_Elapsed");bool clock=Mathf.Abs(first-1)<.001&&Mathf.Abs(later-1)<.001&&Value(r,"_MF_M1_Loop")<.01&&Mathf.Abs(Value(r,"_MF_M1_Base")-128f/255)<.001&&Mathf.Abs(Value(r,"_MF_M1_Bank")-1)<.01;
  int layer=r.gameObject.layer;var root=ps.main.customSimulationSpace;var camera=new GameObject("MF once GPU diagnostic").AddComponent<Camera>();camera.enabled=false;camera.cullingMask=1<<31;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=Color.black;camera.orthographic=true;camera.orthographicSize=8*root.lossyScale.y;camera.aspect=1;camera.nearClipPlane=.001f;camera.farClipPlane=20;var center=root.TransformPoint(new Vector3(0,-86*.25f,0));camera.transform.position=center-root.forward*50*root.lossyScale.z;camera.transform.LookAt(center,root.up);var target=new RenderTexture(512,512,24,RenderTextureFormat.ARGB32);target.antiAliasing=4;target.Create();camera.targetTexture=target;int width=0,ink=0;
  try{r.gameObject.layer=31;camera.Render();var old=RenderTexture.active;RenderTexture.active=target;var image=new Texture2D(512,512,TextureFormat.RGB24,false);image.ReadPixels(new Rect(0,0,512,512),0,0);image.Apply();RenderTexture.active=old;File.WriteAllBytes("Docs/TextAnimation/native-motion-once-end.png",image.EncodeToPNG());var colors=image.GetPixels32();var pixels=Enumerable.Range(0,colors.Length).Where(i=>colors[i].r>50&&colors[i].r>colors[i].g*1.3f).ToArray();ink=pixels.Length;width=ink==0?0:pixels.Max(i=>i%512)-pixels.Min(i=>i%512)+1;UnityEngine.Object.DestroyImmediate(image);}
  finally{r.gameObject.layer=layer;camera.targetTexture=null;target.Release();UnityEngine.Object.DestroyImmediate(target);UnityEngine.Object.DestroyImmediate(camera.gameObject);}
  var result=new JObject{["success"]=clock&&ink>100&&width>=60&&width<=70,["onceStartsAtHalfPhaseAndHoldsEnd"]=clock,["elapsed"]=new JArray(first,later),["endPoseWidth"]=width,["endPoseInkPixels"]=ink,["scope"]="Actual UDP native once clock from nonzero phase + production particle shader 90-degree end pose. No generated resources or real VRChat changed."};File.WriteAllText("Docs/TextAnimation/native-motion-once.json",result.ToString());return result.ToString();
 }
}
