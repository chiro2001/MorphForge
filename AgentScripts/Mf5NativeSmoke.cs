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
public static class Mf5NativeSmoke
{
    static string Root=>Path.Combine(Directory.GetParent(Application.dataPath).FullName,"Docs/MF5");
    static ModuleVrc3 Module()=>UnityEngine.Object.FindObjectsOfType<GestureManager>().Where(g=>g.gameObject.scene.path==Mf5AvatarBuild.ScenePath).Select(g=>g.Module as ModuleVrc3).Single(m=>m!=null);
    static ParticleSystem Store(ModuleVrc3 m,int l)=>m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Single(p=>p.name==(l%2==0?"Store":"Store2")&&p.transform.parent.name=="Lane"+(l/2));
    static void Str(Stream s,string v){var b=Encoding.UTF8.GetBytes(v);s.Write(b,0,b.Length);s.WriteByte(0);while(s.Position%4!=0)s.WriteByte(0);}
    static void Int(Stream s,int v){var b=BitConverter.GetBytes(IPAddress.HostToNetworkOrder(v));s.Write(b,0,4);}
    static byte[] Header(){using(var s=new MemoryStream()){Str(s,"#bundle");s.Write(new byte[]{0,0,0,0,0,0,0,1},0,8);return s.ToArray();}}
    static byte[] Packet(int lane,int seq,int op,int[] d){using(var s=new MemoryStream()){s.Write(Header(),0,16);for(int i=0;i<10;i++)using(var m=new MemoryStream()){Str(m,"/avatar/parameters/MF5_L"+lane+"_"+(i==9?"Ctl":"D"+i.ToString("D2")));Str(m,",i");Int(m,i==9?seq*32+op:d[i]);var b=m.ToArray();Int(s,b.Length);s.Write(b,0,b.Length);}return s.ToArray();}}
    static int[] Min(int index)=>new[]{80+index%20*4,128+index/20*4,100};
    static string Key(int l,int[] min,int[] size)=>l+":"+string.Join(",",min)+"/"+string.Join(",",size);
    sealed class Expected{public int[] Min,Size,Bytes;public int Mode;}
    public static string Start(){if(!Application.isPlaying)throw new Exception("Play Mode required.");Directory.CreateDirectory(Root);File.WriteAllText(Path.Combine(Root,"native-progress.json"),"{\"status\":\"starting\"}");EditorApplication.CallbackFunction start=null;start=()=>{EditorApplication.update-=start;RunDetached();};EditorApplication.update+=start;return "MF5 UDP smoke scheduled on next update";}
    static async void RunDetached(){try{
        for(int i=0;i<200;i++){var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().FirstOrDefault(g=>g.gameObject.scene.path==Mf5AvatarBuild.ScenePath);if(gm&&gm.Module is ModuleVrc3)break;await Task.Delay(100);}
        await Run();
    }catch(Exception e){var path=Path.Combine(Root,"native-progress.json");if(!File.Exists(path)||!File.ReadAllText(path).Contains("failed"))File.WriteAllText(path,new JObject{["status"]="failed",["error"]=e.ToString()}.ToString());Debug.LogError("MF5 native smoke: "+e);}}
    public static async Task<string> Run()
    {
        var m=Module();var receiver=UnityEngine.Object.FindObjectsOfType<MorphForgePlayReceiver>().Single(r=>r.gameObject.scene.path==Mf5AvatarBuild.ScenePath);if(!receiver.Running){receiver.TryStart();if(!receiver.Running)throw new Exception(receiver.Error);}
        var seq=new[]{1,1,1};var expected=new Dictionary<string,Expected>();var checks=new JArray();int batches=0;
        using(var udp=new UdpClient())
        {
            async Task Send(int op,int start,int mode,int bits)
            {
                using(var bundle=new MemoryStream()) {
                    bundle.Write(Header(),0,16);
                    for(int l=0;l<3;l++) {
                        bool paired=op>=17&&op<=19;bool secondary=op>=9&&op<=16||op==21||op==24;int slot=l*2+(secondary?1:0);int index=start+l;var min=Min(index);var size=paired?new[]{1,1,1}:new[]{index%3+1,2,1};var d=new[]{min[0],min[1],min[2],size[0]-1,size[1]-1,size[2]-1,bits&255,(bits>>8)&255,(bits>>16)&255};
                        if(paired){var second=Min(start+3+l);Array.Copy(second,0,d,3,3);expected[Key(l*2+1,second,new[]{1,1,1})]=new Expected{Min=second,Size=new[]{1,1,1},Mode=mode,Bytes=d.Skip(6).ToArray()};}
                        if(op==20||op==21){size=new[]{1,1,1};d[3]=d[4]=d[5]=0;expected.Remove(Key(slot,min,size));}
                        else expected[Key(slot,min,size)]=new Expected{Min=min,Size=size,Mode=mode,Bytes=d.Skip(6).ToArray()};
                        var p=Packet(l,seq[l],op,d);seq[l]=(seq[l]+1)%8;bundle.Write(p,16,p.Length-16);
                    }
                    var bytes=bundle.ToArray();udp.Send(bytes,bytes.Length,"127.0.0.1",9012);batches++;
                }await Task.Delay(250);
            }
            async Task Clear(){using(var bundle=new MemoryStream()){bundle.Write(Header(),0,16);for(int l=0;l<3;l++){var p=Packet(l,seq[l],22,new int[9]);seq[l]=(seq[l]+1)%8;bundle.Write(p,16,p.Length-16);}var bytes=bundle.ToArray();udp.Send(bytes,bytes.Length,"127.0.0.1",9012);}expected.Clear();await Task.Delay(500);}
            async Task Verify(string phase)
            {
                await Task.Delay(300);var errors=new JArray();int total=0;var remaining=new HashSet<string>(expected.Keys);
                for(int l=0;l<6;l++) {
                    var ps=Store(m,l);var particles=new ParticleSystem.Particle[ps.main.maxParticles];int count=ps.GetParticles(particles);total+=count;
                    for(int i=0;i<count;i++) {
                        var p=particles[i];var dims=p.startSize3D/.25f;var min=p.position/.25f+Vector3.one*128-dims*.5f;var mi=new[]{Mathf.RoundToInt(min.x),Mathf.RoundToInt(min.y),Mathf.RoundToInt(min.z)};var si=new[]{Mathf.RoundToInt(dims.x),Mathf.RoundToInt(dims.y),Mathf.RoundToInt(dims.z)};string key=Key(l,mi,si);
                        if(!remaining.Remove(key)||!expected.ContainsKey(key)){errors.Add("Unexpected/duplicate "+key+" color="+p.startColor);continue;}
                        var e=expected[key];var c=p.startColor;
                        if(Vector3.Distance(min,new Vector3(mi[0],mi[1],mi[2]))>.001f||Vector3.Distance(dims,new Vector3(si[0],si[1],si[2]))>.001f||c.r!=e.Bytes[0]||c.g!=e.Bytes[1]||c.b!=e.Bytes[2]||c.a!=255-e.Mode)errors.Add("Corrupt "+key+" data="+c+" expected="+string.Join(",",e.Bytes)+" alpha="+(255-e.Mode));
                    }
                }
                foreach(var key in remaining)errors.Add("Missing "+key);
                var check=new JObject{["phase"]=phase,["passed"]=errors.Count==0,["expected"]=expected.Count,["actual"]=total,["errors"]=errors};checks.Add(check);File.WriteAllText(Path.Combine(Root,"native-progress.json"),new JObject{["status"]=errors.Count==0?"running":"failed",["checks"]=checks,["batches"]=batches}.ToString());if(errors.Count>0)throw new Exception(check.ToString());
            }
            async Task Scale(float wire,float physical,string phase)
            {
                using(var b=new MemoryStream()){b.Write(Header(),0,16);using(var msg=new MemoryStream()){Str(msg,"/avatar/parameters/MF_Scale");Str(msg,",f");var bytes=BitConverter.GetBytes(wire);if(BitConverter.IsLittleEndian)Array.Reverse(bytes);msg.Write(bytes,0,4);var p=msg.ToArray();Int(b,p.Length);b.Write(p,0,p.Length);}var packet=b.ToArray();udp.Send(packet,packet.Length,"127.0.0.1",9012);}await Task.Delay(350);
                var transform=m.Avatar.GetComponentsInChildren<Transform>(true).Single(t=>t.name=="ContentScale");float value=transform.localScale.x;bool ok=Mathf.Abs(value-physical)<Mathf.Max(.0000001f,physical*.001f);checks.Add(new JObject{["phase"]=phase,["passed"]=ok,["actualPhysical"]=value,["expectedPhysical"]=physical});if(!ok)throw new Exception("Scale mismatch "+phase+" actual="+value+" expected="+physical);
            }
            try {
                await Clear();await Verify("clear");
                for(int mode=0;mode<=7;mode++){await Send(mode+1,mode*3,mode,mode==0?64:mode==1?0xE178AD:mode==7?64|(0xF73<<12):0x63AD9F);await Verify("primary surface-mode-"+mode);}
                for(int mode=0;mode<=7;mode++){await Send(mode+9,24+mode*3,mode,mode==0?64:mode==1?0x47E17C:mode==7?64|(0xF73<<12):0x63AD9F);await Verify("secondary surface-mode-"+mode);}
                for(int frame=0;frame<16;frame++)await Send(18,60+frame*6,1,0xF817C1);await Verify("96 RGB unit voxels / two sequence wraps / independent dual pools");
                await Send(17,162,0,64);await Verify("catalog pair");await Send(19,168,2,0xC7479F);await Verify("PBR pair");
                await Send(2,174,1,0xFFFFFF);await Verify("single draw after pair retains secondary pool");
                await Send(21,63,0,0);await Verify("precise erase in secondary pools");
                await Send(18,60,1,0xF817C1);await Verify("paired repaint retains count and restores erased voxels");
                await Scale(-1,.0005f,"1% minimum");await Scale(-1f/3f,.005f,"10%");await Scale(1f/3f,.05f,"100% equals old 5%");await Scale(1,.5f,"1000% maximum");await Scale(1f/3f,.05f,"restore standard size");
                var presets=new[]{.0005f,.005f,.05f,.5f};m.GetParam("IsLocal").Set(m,1);
                for(int i=0;i<4;i++){m.GetParam("MF5_ScalePreset").Set(m,i+1);await Task.Delay(350);float actual=m.Avatar.GetComponentsInChildren<Transform>(true).Single(t=>t.name=="ContentScale").localScale.x;bool ok=Mathf.Abs(actual-presets[i])<Mathf.Max(.0000001f,presets[i]*.001f);checks.Add(new JObject{["phase"]="Avatar menu preset "+i,["passed"]=ok,["actualPhysical"]=actual});if(!ok)throw new Exception("Avatar scale preset failed "+i);}
                await Scale(1f/3f,.05f,"restore after menu presets");await Task.Delay(1000);await Verify("retained data after rescale and idle");await Clear();await Verify("final clear");
                var report=new JObject{["success"]=true,["checks"]=checks,["batches"]=batches,["interval"]=.25,["scope"]="Real production-format OSC bundles on 9012 -> GestureManager -> native Animator and particles. Includes dual-pump birth, all surface tags, color retention, deletes, scale and Avatar menu drivers; no particle prefill or paid model calls."};File.WriteAllText(Path.Combine(Root,"native-smoke.json"),report.ToString());File.WriteAllText(Path.Combine(Root,"native-progress.json"),"{\"status\":\"complete\"}");return report.ToString();
            }catch(Exception e){File.WriteAllText(Path.Combine(Root,"native-progress.json"),new JObject{["status"]="failed",["error"]=e.Message,["checks"]=checks,["batches"]=batches}.ToString());throw;}
        }
    }
}
