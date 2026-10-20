using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Net.Sockets;
using System.Threading.Tasks;
using System.Diagnostics;
using UnityEngine;
using UnityEditor;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
using MorphForge.Editor;
using Newtonsoft.Json.Linq;
public static class Mf6NativeSmoke
{
    sealed class Expected{public int Pool,Id=-1,Alpha;public int[] Min,Size,Color;public int[][] Vertices;}
    static string Root=>"Docs/MF6";
    static int[] V(JToken value)=>value.Select(v=>(int)v).ToArray();
    static string BoxKey(int pool,int[] min,int[] size)=>pool+":B:"+string.Join(",",min)+"/"+string.Join(",",size);
    static string TriKey(int id)=>(id%8)+":T:"+id;
    static Expected Box(JToken r,int shape=0){var min=V(r["geometry"]["min"]);var size=V(r["geometry"]["extent"]).Select(n=>n+1).ToArray();return new Expected{Pool=(int)r["pool"],Min=min,Size=size,Color=V(r["surface"]["bytes"]),Alpha=255-(int)r["surface"]["mode"]-shape*8};}
    static void Apply(Dictionary<string,Expected> expected,JToken frame)
    {
        string format=(string)frame["format"];
        if(format=="clear"){expected.Clear();return;}
        if(format=="boxes"||format=="shared_boxes"||format=="voxels")foreach(var record in frame["records"]){var value=Box(record,(int?)frame["shape"]??0);expected[BoxKey(value.Pool,value.Min,value.Size)]=value;}
        else if(format=="region"){
            var r=frame["region"];var origin=V(r["origin"]);int shape=(int)r["shape"],first=(int)r["first_pool"],i=0;
            foreach(var surface in r["surfaces"]){var min=(int[])origin.Clone();if(shape<3)min[shape]+=i;else for(int a=0;a<3;a++)min[a]+=(i>>a)&1;
                var value=new Expected{Pool=(first+i)%8,Min=min,Size=new[]{1,1,1},Color=V(surface["bytes"]),Alpha=255-(int)surface["mode"]};expected[BoxKey(value.Pool,min,value.Size)]=value;i++;}
        }
        else if(format=="remove_boxes")foreach(var record in frame["records"]){var min=V(record["geometry"]["min"]);var size=V(record["geometry"]["extent"]).Select(n=>n+1).ToArray();expected.Remove(BoxKey((int)record["pool"],min,size));}
        else if(format=="triangles")foreach(var record in frame["records"]){int id=(int)record["id"];expected[TriKey(id)]=new Expected{Id=id,Pool=id%8,Vertices=record["vertices"].Select(V).ToArray(),Color=V(record["color"]),Alpha=(bool)record["double_sided"]?2:1};}
        else if(format=="remove_triangles")foreach(var id in frame["ids"])expected.Remove(TriKey((int)id));
        else if(format=="triangle_strip"||format=="local_triangle_strip"){
            var strip=frame["strip"];int start=(int)strip["first_id"];var vertices=strip["vertices"].Select(V).ToArray();var colors=strip["colors"].Select(V).ToArray();bool doubleSided=(bool)strip["double_sided"];
            for(int i=0;i<colors.Length;i++){int id=start+i;var points=i%2==0?new[]{vertices[i],vertices[i+1],vertices[i+2]}:new[]{vertices[i+1],vertices[i],vertices[i+2]};expected[TriKey(id)]=new Expected{Id=id,Pool=id%8,Vertices=points,Color=colors[i],Alpha=(doubleSided?2:1)+(format=="local_triangle_strip"?(i%2==0?2:4):0)};}
        }else throw new Exception("Unknown fixture format "+format);
    }
    static int Packed(float value)=>Mathf.RoundToInt(value<2?(value-1)*8388608f:value*4194304f);
    static int[] XYZ(int bits)=>new[]{bits&255,(bits>>8)&255,(bits>>16)&255};
    static int[] Local(int bits)=>new[]{bits&15,(bits>>4)&15,(bits>>8)&15};
    static int[][] Vertices(ParticleSystem.Particle p)
    {
        int x=Packed(p.startSize3D.x),y=Packed(p.startSize3D.y),z=Packed(p.startSize3D.z),alpha=p.startColor.a;
        if(alpha<=2)return new[]{XYZ(x),XYZ(y),XYZ(z)};
        int[] origin={y>>16,z&255,(z>>8)&255};int[][] points;
        if(alpha<=4)points=new[]{Local(x),Local(x>>12),Local(y)};
        else points=new[]{Local((x>>16)|((y&15)<<8)),Local(x>>4),Local(y>>4)};
        return points.Select(v=>v.Select((n,a)=>n+origin[a]).ToArray()).ToArray();
    }
    static ModuleVrc3 Module()=>UnityEngine.Object.FindObjectsOfType<GestureManager>().Where(g=>g.gameObject.scene.path==Mf6AvatarBuild.ScenePath).Select(g=>g.Module as ModuleVrc3).Single(m=>m!=null);
    public static string InspectTriangles(){var m=Module();var values=new JArray();foreach(var ps in m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Where(p=>p.name=="Store")){var particles=new ParticleSystem.Particle[ps.main.maxParticles];int count=ps.GetParticles(particles);foreach(var p in particles.Take(count).Where(p=>p.startColor.a>0&&p.startColor.a<=6)){int id=Mathf.RoundToInt((p.position.x+32)*4)+256*Mathf.RoundToInt((p.position.z+32)*4);values.Add(new JObject{["id"]=id,["lifetime"]=p.startLifetime,["expectedLifetime"]=524288+id*8,["decodedLifetimeId"]=Mathf.RoundToInt((p.startLifetime-524288)/8),["pool"]=ps.transform.parent.name});}}return values.ToString();}
    public static string InspectBounds(){var m=Module();var ps=m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Single(p=>p.name=="Store"&&p.transform.parent.name=="Pool0");var particles=new ParticleSystem.Particle[ps.main.maxParticles];int count=ps.GetParticles(particles);return new JObject{["count"]=count,["corners"]=new JArray(particles.Take(count).Where(p=>p.startColor.a==0).Select(p=>new JArray(p.position.x,p.position.y,p.position.z))),["storeScale"]=ps.transform.lossyScale.ToString(),["shapeScale"]=ps.transform.parent.Find("BoundsPump").lossyScale.ToString()}.ToString();}
    static JObject Verify(ModuleVrc3 module,Dictionary<string,Expected> expected,string phase)
    {
        var missing=new HashSet<string>(expected.Keys);var errors=new JArray();var pools=new JArray();int total=0;
        void Error(string message){if(errors.Count<50)errors.Add(message);}
        for(int pool=0;pool<8;pool++){
            var ps=module.Avatar.GetComponentsInChildren<ParticleSystem>(true).Single(p=>p.name=="Store"&&p.transform.parent.name=="Pool"+pool);var particles=new ParticleSystem.Particle[ps.main.maxParticles];int count=ps.GetParticles(particles);int live=particles.Take(count).Count(p=>p.startColor.a!=0);pools.Add(live);total+=live;
            var boundsParticles=particles.Take(count).Where(p=>p.startColor.a==0).ToArray();var corners=new HashSet<string>(boundsParticles.Select(p=>string.Join(",",new[]{Mathf.RoundToInt(p.position.x),Mathf.RoundToInt(p.position.y),Mathf.RoundToInt(p.position.z)})));
            if(boundsParticles.Length!=8||corners.Count!=8||boundsParticles.Any(p=>Mathf.Abs(Mathf.Abs(p.position.x)-40)>.01f||Mathf.Abs(Mathf.Abs(p.position.y)-40)>.01f||Mathf.Abs(Mathf.Abs(p.position.z)-40)>.01f))Error("Bounds corners missing/duplicated pool="+pool+" count="+boundsParticles.Length+" unique="+corners.Count);
            var renderBounds=ps.GetComponent<ParticleSystemRenderer>().bounds;
            for(int corner=0;corner<8;corner++){var point=ps.main.customSimulationSpace.TransformPoint(new Vector3((corner&1)==0?-32:32,(corner&2)==0?-32:32,(corner&4)==0?-32:32));if(!renderBounds.Contains(point))Error("Renderer bounds exclude legal geometry pool="+pool+" corner="+corner);}
            for(int i=0;i<count;i++){
                var p=particles[i];if(p.startColor.a==0)continue;string key;int[] min=null,size=null;bool tri=p.startColor.a<=6;
                if(tri){int id=Mathf.RoundToInt((p.position.x+32)*4)+256*Mathf.RoundToInt((p.position.z+32)*4);key=TriKey(id);if(Mathf.Abs(p.position.y+128)>.001f)Error("Triangle storage Y changed "+key);}
                else{var dims=p.startSize3D/.25f;var point=p.position/.25f+Vector3.one*128-dims*.5f;min=new[]{Mathf.RoundToInt(point.x),Mathf.RoundToInt(point.y),Mathf.RoundToInt(point.z)};size=new[]{Mathf.RoundToInt(dims.x),Mathf.RoundToInt(dims.y),Mathf.RoundToInt(dims.z)};key=BoxKey(pool,min,size);}
                if(!missing.Remove(key)||!expected.ContainsKey(key)){Error("Unexpected/duplicate "+key);continue;}var e=expected[key];var color=p.startColor;
                if(color.r!=e.Color[0]||color.g!=e.Color[1]||color.b!=e.Color[2]||color.a!=e.Alpha)Error("Appearance mismatch "+key+" actual="+color+" expected="+string.Join(",",e.Color)+"/"+e.Alpha);
                if(tri){var vertices=Vertices(p);if(!vertices.SelectMany(v=>v).SequenceEqual(e.Vertices.SelectMany(v=>v)))Error("Vertices mismatch "+key+" actual="+string.Join("/",vertices.Select(v=>string.Join(",",v)))+" expected="+string.Join("/",e.Vertices.Select(v=>string.Join(",",v))));if(Mathf.RoundToInt((p.startLifetime-524288)/8)!=e.Id)Error("Lifetime ID mismatch "+key+" actual="+p.startLifetime);}
            }
        }
        foreach(var key in missing.Take(30))Error("Missing "+key);
        var report=new JObject{["phase"]=phase,["passed"]=errors.Count==0&&missing.Count==0&&total==expected.Count,["expected"]=expected.Count,["actual"]=total,["missingCount"]=missing.Count,["pools"]=pools,["errors"]=errors};
        if(!(bool)report["passed"]){report["stage"]=module.GetParam("MF6_Stage").IntValue();report["sequence"]=module.GetParam("MF6_Seq").IntValue();report["format"]=module.GetParam("MF6_Format").IntValue();report["workers"]=new JArray(Enumerable.Range(0,8).Select(pool=>new JObject{["pool"]=pool,["receivedSeq"]=module.GetParam("MF6_P"+pool+"_LastRx").IntValue(),["done"]=module.GetParam("MF6_P"+pool+"_DoneCount").FloatValue(),["busy"]=module.GetParam("MF6_P"+pool+"_Busy").BoolValue(),["ready"]=module.GetParam("MF6_P"+pool+"_Ready").BoolValue()}));}
        return report;
    }
    public static string Start(bool capacity=false)=>Schedule(capacity,"");
    public static string StartRandom()=>Schedule(false,"random");
    public static string StartRegions()=>Schedule(false,"regions");
    static string Schedule(bool capacity,string variant)
    {
        if(!Application.isPlaying)throw new Exception("Play Mode required");Directory.CreateDirectory(Root);string progress=Root+"/unified-progress.json";
        if(File.Exists(progress)&&(string)JObject.Parse(File.ReadAllText(progress))["status"]=="running")throw new Exception("MF6 validation already running");File.WriteAllText(progress,new JObject{["status"]="starting",["capacity"]=capacity}.ToString());
        EditorApplication.CallbackFunction callback=null;callback=()=>{EditorApplication.update-=callback;Run(capacity,variant);};EditorApplication.update+=callback;return "MF6 native unified fixture scheduled";
    }
    static async void Run(bool capacity,string variant)
    {
        var checks=new JArray();var watch=Stopwatch.StartNew();int frames=0;
        try{
            for(int i=0;i<300;i++){var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().FirstOrDefault(g=>g.gameObject.scene.path==Mf6AvatarBuild.ScenePath);if(gm&&gm.Module is ModuleVrc3)break;await Task.Delay(100);}
            var module=Module();var receiver=UnityEngine.Object.FindObjectsOfType<MorphForgePlayReceiver>().Single(r=>r.gameObject.scene.path==Mf6AvatarBuild.ScenePath);if(!receiver.Running)receiver.TryStart();if(!receiver.Running)throw new Exception(receiver.Error);
            var fixture=JObject.Parse(File.ReadAllText(Root+(capacity?"/capacity-fixtures.json":variant=="random"?"/random-fixtures.json":variant=="regions"?"/region-fixtures.json":"/fixtures.json")));var expected=new Dictionary<string,Expected>();int interval=(int)fixture["intervalMs"];int previous=module.GetParam("MF6_LastRx").IntValue();int sequenceOffset=previous>=0&&previous<16?previous:0;
            using(var udp=new UdpClient())foreach(var item in fixture["cases"]){
                var bytes=item["packet"].Select(v=>(byte)(int)v).ToArray();int control=bytes[bytes.Length-1];bytes[bytes.Length-1]=(byte)(((((control>>4)+sequenceOffset)%16)<<4)|(control&15));udp.Send(bytes,bytes.Length,"127.0.0.1",9012);Apply(expected,item["frame"]);frames++;await Task.Delay(interval);
                if((bool)item["verify"]){await Task.Delay(100);var check=Verify(module,expected,(string)item["name"]);checks.Add(check);File.WriteAllText(Root+"/unified-progress.json",new JObject{["status"]=(bool)check["passed"]?"running":"failed",["capacity"]=capacity,["frames"]=frames,["expected"]=expected.Count,["elapsedSeconds"]=watch.Elapsed.TotalSeconds,["latestCheck"]=check}.ToString());if(!(bool)check["passed"])throw new Exception(check.ToString());}
            }
            var report=new JObject{["success"]=true,["capacity"]=capacity,["checks"]=checks,["frames"]=frames,["intervalMs"]=interval,["elapsedSeconds"]=watch.Elapsed.TotalSeconds,["scope"]="Production Rust codec OSC bundles -> real UDP9012 -> GestureManager -> shared 240bit decoder -> eight independent native pools. No SetParticles, Emit, or scripted geometry injection."};
            File.WriteAllText(Root+(capacity?"/capacity.json":variant=="random"?"/random-smoke.json":variant=="regions"?"/region-smoke.json":"/unified-smoke.json"),report.ToString());File.WriteAllText(Root+"/unified-progress.json",new JObject{["status"]="complete",["capacity"]=capacity,["variant"]=variant,["frames"]=frames}.ToString());
        }catch(Exception e){File.WriteAllText(Root+"/unified-progress.json",new JObject{["status"]="failed",["capacity"]=capacity,["frames"]=frames,["error"]=e.ToString(),["checks"]=checks}.ToString());UnityEngine.Debug.LogError("MF6 native fixture: "+e);}
    }
}
