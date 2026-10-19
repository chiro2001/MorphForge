using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Net;
using System.Net.Sockets;
using System.Text;
using System.Threading.Tasks;
using UnityEngine;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
using MorphForge.Editor;
using Newtonsoft.Json.Linq;
public static class Mf4NativeSmoke
{
    static ModuleVrc3 Module() => UnityEngine.Object.FindObjectsOfType<GestureManager>().Where(g=>g.gameObject.scene.path==Mf4AvatarBuild.ScenePath).Select(g=>g.Module as ModuleVrc3).Single(m=>m!=null);
    static ParticleSystem Store(ModuleVrc3 m,int lane) => m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Single(p=>p.name=="Store"&&p.transform.parent.name=="Lane"+lane);
    static void String(Stream s,string v){var b=Encoding.UTF8.GetBytes(v);s.Write(b,0,b.Length);s.WriteByte(0);while(s.Position%4!=0)s.WriteByte(0);}
    static void Int(Stream s,int value){var b=BitConverter.GetBytes(IPAddress.HostToNetworkOrder(value));s.Write(b,0,4);}
    static byte[] Packet(int lane,int seq,int op,int[] data)
    {
        using(var stream=new MemoryStream())
        {
            String(stream,"#bundle");stream.Write(new byte[]{0,0,0,0,0,0,0,1},0,8);
            for(int i=0;i<10;i++)using(var msg=new MemoryStream())
            {String(msg,"/avatar/parameters/MF4_L"+lane+"_"+(i==9?"Ctl":"D"+i.ToString("D2")));String(msg,",i");Int(msg,i==9?seq*16+op:data[i]);var b=msg.ToArray();Int(stream,b.Length);stream.Write(b,0,b.Length);}
            return stream.ToArray();
        }
    }
    static int[] Data(int index,int style,int tint,int motion)
    {
        int bits=style|(tint<<12)|(motion<<18);
        return new[]{80+index%24*4,128+index/24*4,100, index%3,1,0,bits&255,(bits>>8)&255,(bits>>16)&255};
    }
    public static string Inspect()
    {
        var m=Module();return new JObject{["receiver"]=new JArray(UnityEngine.Object.FindObjectsOfType<MorphForgePlayReceiver>().Select(r=>new JObject{["scene"]=r.gameObject.scene.path,["running"]=r.Running,["error"]=r.Error})),["lanes"]=new JArray(Enumerable.Range(0,3).Select(l=>new JObject{["lane"]=l,["count"]=Store(m,l).particleCount,["rx"]=m.GetParam("MF4_L"+l+"_RxCount").FloatValue(),["done"]=m.GetParam("MF4_L"+l+"_DoneCount").FloatValue(),["busy"]=m.GetParam("MF4_L"+l+"_Busy").BoolValue()}))}.ToString();
    }
    public static async Task<string> Run()
    {
        if(!Application.isPlaying)throw new Exception("Play Mode required.");
        var m=Module();
        // Only transient Play objects are disabled; user Edit Mode scenes remain intact.
        foreach(var gm in UnityEngine.Object.FindObjectsOfType<GestureManager>().Where(g=>g.gameObject.scene.path!=Mf4AvatarBuild.ScenePath))
        {foreach(var r in gm.gameObject.scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<MorphForgePlayReceiver>(true)))MorphForgePlayReceiverDriver.Stop(r);gm.gameObject.SetActive(false);}
        var receiver=UnityEngine.Object.FindObjectsOfType<MorphForgePlayReceiver>().Single(r=>r.gameObject.scene.path==Mf4AvatarBuild.ScenePath);MorphForgePlayReceiverDriver.Stop(receiver);receiver.TryStart();
        if(!receiver.Running)throw new Exception(receiver.Error);
        var seq=new[]{1,1,1};var expected=new Dictionary<int,int[]>();var checks=new JArray();
        using(var udp=new UdpClient())
        {
            async Task Send(int op,IEnumerable<int> indices,int generation=0)
            {
                foreach(int index in indices)
                {
                    int lane=index%3;int style=generation==0?25+index%4:(index%2==0?4095:64+index%19);int tint=(index*7+generation)%64,motion=(index*11+generation)%64;
                    var d=Data(index,style,tint,motion);var packet=Packet(lane,seq[lane],op,d);seq[lane]=(seq[lane]+1)%16;udp.Send(packet,packet.Length,"127.0.0.1",9012);
                    if(op<=2)expected[index]=d;else if(op==4)expected.Remove(index);
                }
                await Task.Delay(250);
            }
            async Task Clear(){for(int l=0;l<3;l++){var p=Packet(l,seq[l],5,new int[9]);seq[l]=(seq[l]+1)%16;udp.Send(p,p.Length,"127.0.0.1",9012);}expected.Clear();await Task.Delay(500);}
            async Task Verify(string phase)
            {
                await Task.Delay(350);var errors=new JArray();int total=0;
                for(int lane=0;lane<3;lane++)
                {
                    var store=Store(m,lane);var particles=new ParticleSystem.Particle[store.main.maxParticles];int count=store.GetParticles(particles);total+=count;
                    var remaining=new HashSet<int>(expected.Keys.Where(i=>i%3==lane));
                    for(int i=0;i<count;i++)
                    {
                        var p=particles[i];var min=p.position/.25f+Vector3.one*128-p.startSize3D/.5f;
                        int ix=Mathf.RoundToInt((min.x-80)/4),iy=Mathf.RoundToInt((min.y-128)/4),index=ix+iy*24;
                        if(!remaining.Remove(index)||!expected.ContainsKey(index)){errors.Add("Unexpected/duplicate particle lane="+lane+" min="+min);continue;}
                        var d=expected[index];var ideal=new Vector3(d[0],d[1],d[2]);var size=new Vector3(d[3]+1,d[4]+1,d[5]+1)*.25f;var c=p.startColor;
                        if(Vector3.Distance(min,ideal)>.001||Vector3.Distance(p.startSize3D,size)>.001||c.r!=d[6]||c.g!=d[7]||c.b!=d[8]||c.a!=255)errors.Add("Bad data index="+index+" actual="+c+" expected="+d[6]+","+d[7]+","+d[8]+",255 min="+min);
                    }
                    foreach(int missing in remaining)errors.Add("Missing "+missing);
                }
                var check=new JObject{["phase"]=phase,["expected"]=expected.Count,["actual"]=total,["passed"]=errors.Count==0,["errors"]=errors};checks.Add(check);
                if(errors.Count>0)throw new Exception(check.ToString());
            }
            try
            {
                await Clear();await Verify("empty");
                for(int i=0;i<48;i+=3)await Send(1,Enumerable.Range(i,3));await Verify("48 mixed-size packed-color boxes after 16 sequence steps");
                for(int i=0;i<48;i+=3)await Send(1,Enumerable.Range(i,3),1);await Verify("appearance replacements and byte 255");
                for(int i=0;i<48;i+=6)await Send(4,Enumerable.Range(i,3));await Verify("erase half across three pools");
                for(int i=0;i<48;i+=6)await Send(1,Enumerable.Range(i,3),2);await Verify("redraw erased boxes");
                await Task.Delay(2000);await Verify("retained immutable colors while idle");
                await Clear();await Verify("broadcast clear");
                var report=new JObject{["success"]=true,["intervalSeconds"]=.25,["checks"]=checks,["scope"]="Real UDP 9012 into GestureManager, native animator/ParameterDriver/particle systems; no runtime script creates particles. Not a multiplayer network test."};Directory.CreateDirectory("Docs/MF4");File.WriteAllText("Docs/MF4/native-smoke.json",report.ToString());return report.ToString();
            }
            catch(Exception e){Directory.CreateDirectory("Docs/MF4");File.WriteAllText("Docs/MF4/native-smoke-failed.json",new JObject{["success"]=false,["checks"]=checks,["error"]=e.Message,["state"]=JObject.Parse(Inspect())}.ToString());throw;}
        }
    }
}
