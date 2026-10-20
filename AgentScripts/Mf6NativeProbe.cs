using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.SDK3.Avatars.Components;
using MorphForge.Editor;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
using Newtonsoft.Json.Linq;
using System.Collections.Generic;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Threading.Tasks;
public static class Mf6NativeProbe
{
    public const string ScenePath="Assets/MorphForge/MF6/NativeProbe.unity";
    public static string Build()
    {
        if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");
        for(int i=0;i<SceneManager.sceneCount;i++)if(SceneManager.GetSceneAt(i).isDirty)throw new Exception("Unsaved scene preserved");
        Mf6NativeBuild.BuildWorker(0);
        EditorSceneManager.OpenScene(Mf5AvatarBuild.ScenePath,OpenSceneMode.Single);
        var scene=SceneManager.GetActiveScene();var avatar=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single();avatar.gameObject.name="MorphForge_MF6_Probe";
        var center=avatar.GetComponentsInChildren<Transform>(true).Single(t=>t.name=="ProtocolCenter");foreach(var child in center.Cast<Transform>().ToArray())UnityEngine.Object.DestroyImmediate(child.gameObject);
        var worker=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(Mf6NativeBuild.Root+"/Pool0/Display.prefab"),center);worker.name="Pool0";
        EditorSceneManager.SaveScene(scene,ScenePath);Directory.CreateDirectory("Docs/MF6");
        return new JObject{["success"]=true,["scene"]=ScenePath,["avatarCount"]=scene.GetRootGameObjects().Sum(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true).Length)}.ToString();
    }
    public static string Inspect()
    {
        if(!Application.isPlaying)throw new Exception("Play Mode required");var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single(g=>g.gameObject.scene.path==ScenePath);var module=gm.Module as ModuleVrc3;if(module==null)throw new Exception("GestureManager not ready");
        var ps=module.Avatar.GetComponentsInChildren<ParticleSystem>(true).Single(p=>p.name=="Store");var particles=new ParticleSystem.Particle[ps.main.maxParticles];int count=ps.GetParticles(particles);
        var result=new JObject{["success"]=true,["count"]=count,["particles"]=new JArray(particles.Take(count).Select(p=>new JObject{["position"]=new JArray(p.position.x,p.position.y,p.position.z),["size"]=new JArray(p.startSize3D.x,p.startSize3D.y,p.startSize3D.z),["color"]=new JArray(p.startColor.r,p.startColor.g,p.startColor.b,p.startColor.a)}))};
        foreach(string name in new[]{"Kind","Clear","DoneCount"})result[name]=module.GetParam("MF6_P0_"+name).FloatValue();
        foreach(string name in new[]{"Busy","Ready"})result[name]=module.GetParam("MF6_P0_"+name).BoolValue();result["Op"]=module.GetParam("MF6_P0_Op").IntValue();
        Directory.CreateDirectory("Docs/MF6");File.WriteAllText("Docs/MF6/native-probe.json",result.ToString());return result.ToString();
    }
    public static string Start()
    {
        if(!Application.isPlaying)throw new Exception("Play Mode required");Directory.CreateDirectory("Docs/MF6");File.WriteAllText("Docs/MF6/probe-progress.json","{\"status\":\"starting\"}");
        EditorApplication.CallbackFunction callback=null;callback=()=>{EditorApplication.update-=callback;Run();};EditorApplication.update+=callback;return "MF6 production worker OSC probe scheduled";
    }
    static void Str(Stream s,string text){var bytes=Encoding.UTF8.GetBytes(text);s.Write(bytes,0,bytes.Length);s.WriteByte(0);while(s.Position%4!=0)s.WriteByte(0);}
    static void Int(Stream s,int value){var bytes=BitConverter.GetBytes(IPAddress.HostToNetworkOrder(value));s.Write(bytes,0,4);}
    static byte[] Packet(float[] values,int control)
    {
        using(var bundle=new MemoryStream()){
            Str(bundle,"#bundle");bundle.Write(new byte[]{0,0,0,0,0,0,0,1},0,8);
            for(int i=0;i<=values.Length;i++)using(var message=new MemoryStream()){
                Str(message,"/avatar/parameters/MF6_P0_"+(i==values.Length?"Ctl":"D"+i.ToString("D2")));Str(message,i==values.Length?",i":",f");
                if(i==values.Length)Int(message,control);else{var bytes=BitConverter.GetBytes(values[i]);if(BitConverter.IsLittleEndian)Array.Reverse(bytes);message.Write(bytes,0,4);}
                var data=message.ToArray();Int(bundle,data.Length);bundle.Write(data,0,data.Length);
            }return bundle.ToArray();
        }
    }
    static float Decode(float value)=>1+(value<2?(value-1)*8388608f:value*4194304f);
    static async void Run()
    {
        var checks=new JArray();
        try{
            GestureManager gm=null;for(int attempt=0;attempt<200;attempt++){gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().FirstOrDefault(g=>g.gameObject.scene.path==ScenePath);if(gm&&gm.Module is ModuleVrc3)break;await Task.Delay(100);}
            var module=gm.Module as ModuleVrc3;if(module==null)throw new Exception("GestureManager unavailable");
            var receiver=gm.GetComponent<MorphForgePlayReceiver>();if(!receiver.Running)receiver.TryStart();if(!receiver.Running)throw new Exception(receiver.Error);
            var ps=module.Avatar.GetComponentsInChildren<ParticleSystem>(true).Single(p=>p.name=="Store");
            var expected=new Dictionary<int,float[]>();int sequence=1;
            using(var udp=new UdpClient()){
                async Task Send(float[] data,int op){var packet=Packet(data,sequence*8+op);udp.Send(packet,packet.Length,"127.0.0.1",9012);sequence=(sequence+1)%16;await Task.Delay(300);}
                async Task Verify(string name){await Task.Delay(100);var values=new ParticleSystem.Particle[2048];int count=ps.GetParticles(values);var errors=new JArray();var missing=new HashSet<int>(expected.Keys);
                    for(int i=0;i<count;i++){var p=values[i];int id=Mathf.RoundToInt((p.position.x+32)*4);if(!missing.Remove(id)||!expected.ContainsKey(id)){errors.Add("Unexpected/duplicate ID "+id);continue;}var e=expected[id];var actual=new[]{Decode(p.startSize3D.x),Decode(p.startSize3D.y),Decode(p.startSize3D.z)};for(int a=0;a<3;a++)if(actual[a]!=e[a])errors.Add("Packed vertex mismatch ID="+id+" component="+a+" actual="+actual[a]+" expected="+e[a]);if(p.startColor.r!=e[3]||p.startColor.g!=e[4]||p.startColor.b!=e[5]||p.startColor.a!=2)errors.Add("Color mismatch ID="+id+" color="+p.startColor);if(Mathf.Abs(p.position.y+128)>.001f)errors.Add("ID storage area changed");}
                    foreach(int id in missing)errors.Add("Missing "+id);var check=new JObject{["phase"]=name,["passed"]=errors.Count==0,["expected"]=expected.Count,["actual"]=count,["errors"]=errors};checks.Add(check);File.WriteAllText("Docs/MF6/probe-progress.json",new JObject{["status"]=errors.Count==0?"running":"failed",["checks"]=checks}.ToString());if(errors.Count!=0)throw new Exception(check.ToString());}
                await Send(new float[20],5);await Verify("initial clear");
                for(int id=0;id<20;id++){
                    var d=new float[20];d[0]=id;d[2]=0;d[9]=2;
                    int[] positions=id==0?new[]{0,0,0,255,255,255,0,255,0}:id==1?new[]{255,255,255,0,0,0,255,0,255}:new[]{120+id,128,120,128+id,128,120,124+id,140,120};
                    for(int a=0;a<3;a++){d[3+a]=positions[a];d[10+a]=positions[3+a];d[13+a]=positions[6+a];}d[6]=(id*37)%256;d[7]=(id*97)%256;d[8]=(id*53)%256;
                    expected[id]=new[]{1+positions[0]+256f*positions[1]+65536f*positions[2],1+positions[3]+256f*positions[4]+65536f*positions[5],1+positions[6]+256f*positions[7]+65536f*positions[8],d[6],d[7],d[8]};await Send(d,2);
                }
                await Verify("20 native births including full 24-bit maxima and sequence wrap");
                var rewrite=new float[20];rewrite[0]=7;rewrite[2]=0;rewrite[9]=2;rewrite[3]=120;rewrite[4]=128;rewrite[5]=120;rewrite[10]=130;rewrite[11]=128;rewrite[12]=120;rewrite[13]=125;rewrite[14]=145;rewrite[15]=120;rewrite[6]=255;rewrite[7]=200;rewrite[8]=40;
                expected[7]=new[]{1+120+256f*128+65536f*120,1+130+256f*128+65536f*120,1+125+256f*145+65536f*120,255,200,40};await Send(rewrite,2);await Verify("ID rewrite has no duplicate");
                await Send(rewrite,4);expected.Remove(7);await Verify("ID erase without visible geometry lookup");
                await Task.Delay(1000);await Verify("retained after idle");
                var report=new JObject{["success"]=true,["checks"]=checks,["scope"]="Real UDP to GestureManager, native Animator birth and Trigger erase, no SetParticles/Emit/C# geometry injection",["interval"]=.3};File.WriteAllText("Docs/MF6/native-worker-smoke.json",report.ToString());File.WriteAllText("Docs/MF6/probe-progress.json","{\"status\":\"complete\"}");
            }
        }catch(Exception e){File.WriteAllText("Docs/MF6/probe-progress.json",new JObject{["status"]="failed",["error"]=e.ToString(),["checks"]=checks}.ToString());Debug.LogError(e);}
    }
}
