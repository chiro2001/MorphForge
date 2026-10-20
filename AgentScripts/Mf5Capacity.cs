using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Threading.Tasks;
using UnityEngine;
using UnityEditor;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
using MorphForge.Editor;
using Newtonsoft.Json.Linq;
public static class Mf5Capacity
{
    const string Root="Docs/MF5";
    static void Str(Stream s,string v){var b=Encoding.UTF8.GetBytes(v);s.Write(b,0,b.Length);s.WriteByte(0);while(s.Position%4!=0)s.WriteByte(0);}
    static void Int(Stream s,int v){var b=BitConverter.GetBytes(IPAddress.HostToNetworkOrder(v));s.Write(b,0,4);}
    static void Message(Stream bundle,string name,int v){using(var msg=new MemoryStream()){Str(msg,"/avatar/parameters/"+name);Str(msg,",i");Int(msg,v);var b=msg.ToArray();Int(bundle,b.Length);bundle.Write(b,0,b.Length);}}
    static void Lane(Stream b,int l,int seq,int op,int[] d){for(int i=0;i<9;i++)Message(b,"MF5_L"+l+"_D"+i.ToString("D2"),d[i]);Message(b,"MF5_L"+l+"_Ctl",seq*32+op);}
    static MemoryStream Bundle(){var b=new MemoryStream();Str(b,"#bundle");b.Write(new byte[]{0,0,0,0,0,0,0,1},0,8);return b;}
    static int[] Pos(int index)=>new[]{32+index%128,128,32+index/128};
    public static string Start(){if(!Application.isPlaying)throw new Exception("Play required");Directory.CreateDirectory(Root);File.WriteAllText(Root+"/capacity-progress.json","{\"status\":\"starting\"}");EditorApplication.CallbackFunction start=null;start=()=>{EditorApplication.update-=start;Run();};EditorApplication.update+=start;return "Native full-capacity OSC test started; 0.25 s per batch";}
    static async void Run(){try{
        for(int i=0;i<200;i++){if(UnityEngine.Object.FindObjectsOfType<GestureManager>().Any(g=>g.gameObject.scene.path==Mf5AvatarBuild.ScenePath&&g.Module is ModuleVrc3))break;await Task.Delay(100);}
        var m=UnityEngine.Object.FindObjectsOfType<GestureManager>().Where(g=>g.gameObject.scene.path==Mf5AvatarBuild.ScenePath).Select(g=>g.Module as ModuleVrc3).Single(x=>x!=null);
        var receiver=UnityEngine.Object.FindObjectsOfType<MorphForgePlayReceiver>().Single(r=>r.gameObject.scene.path==Mf5AvatarBuild.ScenePath);if(!receiver.Running)receiver.TryStart();if(!receiver.Running)throw new Exception(receiver.Error);
        var seq=new[]{1,1,1};var watch=System.Diagnostics.Stopwatch.StartNew();int frames=0;
        using(var udp=new UdpClient()){
            void Send(MemoryStream b){var bytes=b.ToArray();udp.Send(bytes,bytes.Length,"127.0.0.1",9012);}
            async Task Clear(){using(var b=Bundle()){for(int l=0;l<3;l++){Lane(b,l,seq[l],22,new int[9]);seq[l]=(seq[l]+1)%8;}Send(b);}await Task.Delay(500);}
            await Clear();
            for(int first=0;first<16384;first+=6){
                using(var b=Bundle()){
                    for(int l=0;l<3;l++){int a=first+l*2;if(a>=16384)continue;var p=Pos(a);int op=18;var d=new[]{p[0],p[1],p[2],0,0,0,241,121,173};if(a+1<16384)Array.Copy(Pos(a+1),0,d,3,3);else op=2;Lane(b,l,seq[l],op,d);seq[l]=(seq[l]+1)%8;}Send(b);
                }frames++;await Task.Delay(250);
                if(frames%64==0)File.WriteAllText(Root+"/capacity-progress.json",new JObject{["status"]="running",["sent"]=Math.Min(16384,first+6),["frames"]=frames,["seconds"]=watch.Elapsed.TotalSeconds}.ToString());
            }
            await Task.Delay(500);var errors=new JArray();var all=new HashSet<int>();var counts=new JArray();
            void Inspect(){
                errors.Clear();all.Clear();counts.Clear();
                for(int slot=0;slot<6;slot++){
                    int lane=slot/2;string name=slot%2==0?"Store":"Store2";var ps=m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Single(p=>p.name==name&&p.transform.parent.name=="Lane"+lane);var particles=new ParticleSystem.Particle[ps.main.maxParticles];int n=ps.GetParticles(particles);counts.Add(n);
                    for(int i=0;i<n;i++){var p=particles[i];var pos=p.position/.25f+Vector3.one*128-Vector3.one*.5f;int x=Mathf.RoundToInt(pos.x)-32,z=Mathf.RoundToInt(pos.z)-32,index=z*128+x;var c=p.startColor;
                        if(x<0||x>=128||z<0||z>=128||Mathf.Abs(pos.y-128)>.001f||index>=16384||index%6!=slot||!all.Add(index)||c.r!=241||c.g!=121||c.b!=173||c.a!=254)errors.Add("Corrupt/duplicate pool "+slot+" position="+pos+" color="+c);
                    }
                }
            }
            Inspect();int firstPassCount=all.Count;var missing=Enumerable.Range(0,16384).Where(i=>!all.Contains(i)).ToArray();var firstMissing=new JArray(missing);int repairBatches=0;
            // Test oracle only: Avatar/OSC production has no remote ACK. A full
            // idempotent replay can repair omissions without knowing these IDs.
            for(int retry=0;retry<3&&missing.Length>0&&errors.Count==0;retry++){
                var queues=Enumerable.Range(0,3).Select(l=>new Queue<int>(missing.Where(i=>i%6/2==l))).ToArray();
                while(queues.Any(q=>q.Count>0)){
                    using(var b=Bundle()){
                        for(int l=0;l<3;l++){if(queues[l].Count==0)continue;int index=queues[l].Dequeue();var p=Pos(index);Lane(b,l,seq[l],index%2==0?2:10,new[]{p[0],p[1],p[2],0,0,0,241,121,173});seq[l]=(seq[l]+1)%8;}Send(b);
                    }repairBatches++;await Task.Delay(500);
                }await Task.Delay(500);Inspect();missing=Enumerable.Range(0,16384).Where(i=>!all.Contains(i)).ToArray();
            }
            bool success=all.Count==16384&&errors.Count==0;var report=new JObject{["success"]=success,["boxes"]=all.Count,["pools"]=counts,["frames"]=frames,["firstPassCount"]=firstPassCount,["firstPassMissing"]=firstMissing,["repairBatches"]=repairBatches,["repairInterval"]=.5,["interval"]=.25,["seconds"]=watch.Elapsed.TotalSeconds,["errors"]=errors,["scope"]="16384 actual OSC-drawn unit voxels via native Animator into six independent production particle pools; no particle prefill. Local missing-ID oracle and slower repair if needed; not remote ACK or multiplayer guarantee."};File.WriteAllText(Root+"/capacity.json",report.ToString());await Clear();File.WriteAllText(Root+"/capacity-progress.json",new JObject{["status"]=success?"complete":"failed",["boxes"]=all.Count}.ToString());
        }
    }catch(Exception e){File.WriteAllText(Root+"/capacity-progress.json",new JObject{["status"]="failed",["error"]=e.ToString()}.ToString());Debug.LogError(e);}}
}
