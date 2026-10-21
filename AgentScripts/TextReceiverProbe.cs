using System;
using System.IO;
using System.Linq;
using System.Net.Sockets;
using System.Threading.Tasks;
using UnityEngine;
using UnityEditor;
using Newtonsoft.Json.Linq;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
using MorphForge.Editor;
public static class TextReceiverProbe {
    static float Value(Renderer r,string name){var block=new MaterialPropertyBlock();r.GetPropertyBlock(block);return block.isEmpty?r.sharedMaterial.GetFloat(name):block.GetFloat(name);}
    public static async Task<string> DirectMath() {
        var root=new GameObject("MFDirectMathProbe",typeof(Animator));var probe=new GameObject("Probe").transform;probe.SetParent(root.transform,false);
        var fx=new UnityEditor.Animations.AnimatorController{name="MFDirectMathProbe"};fx.AddParameter("Weight",AnimatorControllerParameterType.Float);fx.AddLayer("Probe");
        var clip=new AnimationClip{name="MFDirectMathProbe"};AnimationUtility.SetEditorCurve(clip,EditorCurveBinding.FloatCurve("Probe",typeof(Transform),"m_LocalPosition.x"),AnimationCurve.Constant(0,1,1));
        var tree=new UnityEditor.Animations.BlendTree{name="MFDirectMathProbe",blendType=UnityEditor.Animations.BlendTreeType.Direct};tree.AddChild(clip);var children=tree.children;children[0].directBlendParameter="Weight";tree.children=children;var so=new SerializedObject(tree);so.FindProperty("m_NormalizedBlendValues").boolValue=false;so.ApplyModifiedPropertiesWithoutUndo();
        var sm=fx.layers[0].stateMachine;var state=sm.AddState("Probe");state.writeDefaultValues=true;state.motion=tree;sm.defaultState=state;var animator=root.GetComponent<Animator>();animator.cullingMode=AnimatorCullingMode.AlwaysAnimate;animator.runtimeAnimatorController=fx;animator.Rebind();var rows=new JArray();
        try{foreach(float weight in new[]{0,.5f,1,2,65,255}){animator.SetFloat("Weight",weight);animator.Update(0);await Task.Delay(60);rows.Add(new JObject{["weight"]=weight,["actual"]=probe.localPosition.x,["pass"]=Mathf.Abs(probe.localPosition.x-weight)<.001f});}}
        finally{UnityEngine.Object.DestroyImmediate(root);foreach(var obj in new UnityEngine.Object[]{state,sm,tree,clip,fx})if(obj)UnityEngine.Object.DestroyImmediate(obj);}
        var result=new JObject{["success"]=rows.All(r=>(bool)r["pass"]),["rows"]=rows,["scope"]="Transient animation-math fixture only. No product asset or actual Avatar parameter modified."};File.WriteAllText("Docs/TextAnimation/direct-math.json",result.ToString());return result.ToString();
    }
    public static async Task<string> Performance() {
        var all=new System.Collections.Generic.List<Unity.Profiling.LowLevel.Unsafe.ProfilerRecorderHandle>();Unity.Profiling.LowLevel.Unsafe.ProfilerRecorderHandle.GetAvailable(all);
        var descriptors=all.Select(h=>Unity.Profiling.LowLevel.Unsafe.ProfilerRecorderHandle.GetDescription(h)).Where(d=>d.Name.Contains("Animator")&&d.Name.Contains("Update")).ToArray();
        if(descriptors.Length==0)return new JObject{["available"]=false,["note"]="Animator profiler marker unavailable; no CPU estimate invented"}.ToString();
        var descriptor=descriptors.FirstOrDefault(d=>d.Name=="Animator.Update");if(string.IsNullOrEmpty(descriptor.Name))descriptor=descriptors[0];
        var m=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single().Module as ModuleVrc3;if(m==null)throw new Exception("Play Mode required");
        var names=new System.Collections.Generic.List<string>{"MF6_Ctl","MF6_D28"};for(int slot=0;slot<4;slot++){names.Add("MF6_Text"+slot+"_Count");for(int page=0;page<11;page++)names.Add("MF6_Text"+slot+"_Page"+page);for(int field=0;field<256;field++)names.Add("MF6_Text"+slot+"_B"+field.ToString("D3"));}
        var values=names.Select(n=>m.GetParam(n).FloatValue()).ToArray();var rows=new JArray();void Set(string n,float v){m.GetParam(n).Set(m,v);}
        try {
            Set("MF6_D28",0);Set("MF6_Ctl",0);await Task.Delay(200);
            foreach(int mode in new[]{0,1,2,2,1,0}) {
                for(int i=2;i<names.Count;i++)Set(names[i],values[i]);
                if(mode!=1)for(int slot=0;slot<4;slot++){
                    Set("MF6_Text"+slot+"_Count",mode==2?128:0);for(int page=0;page<11;page++)Set("MF6_Text"+slot+"_Page"+page,mode==2?1:0);
                    if(mode==2)for(int field=0;field<256;field++)Set("MF6_Text"+slot+"_B"+field.ToString("D3"),field%2==0?45:78);
                }
                await Task.Delay(250);
                using(var recorder=Unity.Profiling.ProfilerRecorder.StartNew(descriptor.Category,descriptor.Name,128)) {
                    await Task.Delay(1800);var samples=Enumerable.Range(0,recorder.Count).Select(i=>recorder.GetSample(i).Value).ToArray();
                    rows.Add(new JObject{["phase"]=mode==0?"disabled":mode==1?"received":"512 CJK slots",["samples"]=samples.Length,["meanRaw"]=samples.Length>0?samples.Average():0,["maxRaw"]=samples.Length>0?samples.Max():0});
                }
            }
        }finally{for(int i=0;i<names.Count;i++)Set(names[i],values[i]);}
        var result=new JObject{["available"]=true,["marker"]=descriptor.Name,["unit"]=descriptor.UnitType.ToString(),["rows"]=rows,["scope"]="Editor-only whole Avatar Animator, disabled/received/4x128 populated CJK slots, forward and reverse phases. Includes Avatar baseline and Editor noise; not a VRChat CPU benchmark."};Directory.CreateDirectory("Docs/TextAnimation");File.WriteAllText("Docs/TextAnimation/text-performance.json",result.ToString());return result.ToString();
    }
    public static string Visual() {
        if(!EditorApplication.isPlaying)throw new Exception("Play Mode required");
        var m=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single().Module as ModuleVrc3;if(m==null)throw new Exception("GestureManager not ready");
        var result=new JArray();Directory.CreateDirectory("Docs/TextAnimation");
        foreach(int slot in new[]{0,1}) {
            var panel=m.Avatar.GetComponentsInChildren<MeshRenderer>(true).Single(r=>r.name=="Panel"+slot&&r.transform.parent.name=="TextDisplay");
            int layer=panel.gameObject.layer;var go=new GameObject("Temporary text capture camera");var camera=go.AddComponent<Camera>();camera.enabled=false;camera.allowHDR=false;camera.orthographic=true;camera.cullingMask=1<<31;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=new Color(.025f,.035f,.05f,1);camera.nearClipPlane=.0001f;camera.farClipPlane=10000;
            var target=new RenderTexture(1024,384,24,RenderTextureFormat.ARGB32);target.antiAliasing=4;target.Create();camera.targetTexture=target;var images=new System.Collections.Generic.List<Color32[]>();
            try {
                panel.gameObject.layer=31;var t=panel.transform;
                var origin=new Vector3(Value(panel,"_PosX"),(Value(panel,"_PosY")-128+Value(panel,"_Height")*.5f),Value(panel,"_PosZ"))*.25f;
                camera.orthographicSize=Mathf.Max(Value(panel,"_Height")*.25f*.65f*t.lossyScale.y,.0001f);camera.aspect=1024f/384;
                foreach(bool back in new[]{false,true}) {
                    camera.transform.position=t.TransformPoint(origin+new Vector3(0,0,back?100:-100));camera.transform.LookAt(t.TransformPoint(origin),t.up);camera.Render();
                    var previous=RenderTexture.active;RenderTexture.active=target;var image=new Texture2D(1024,384,TextureFormat.RGB24,false);image.ReadPixels(new Rect(0,0,1024,384),0,0);image.Apply();RenderTexture.active=previous;
                    File.WriteAllBytes("Docs/TextAnimation/text-"+slot+(back?"-back":"-front")+".png",image.EncodeToPNG());images.Add(image.GetPixels32());UnityEngine.Object.DestroyImmediate(image);
                }
                int ink=images[0].Count(c=>c.r>180&&c.g>180&&c.b>180),difference=images[0].Select((c,i)=>Math.Abs(c.r-images[1][i].r)+Math.Abs(c.g-images[1][i].g)+Math.Abs(c.b-images[1][i].b)).Count(v=>v>30);
                result.Add(new JObject{["slot"]=slot,["inkPixels"]=ink,["differentPixels"]=difference,["pass"]=ink>100&&difference<Math.Max(10,ink*.05f)});
            }finally{panel.gameObject.layer=layer;camera.targetTexture=null;target.Release();UnityEngine.Object.DestroyImmediate(target);UnityEngine.Object.DestroyImmediate(go);}
        }
        var report=new JObject{["success"]=result.All(r=>(bool)r["pass"]),["checks"]=result,["scope"]="Actual received text materials and binary font GPU. Camera/layer isolation is diagnostic only and restored; no duplicate Avatar or user scene changes."};File.WriteAllText("Docs/TextAnimation/text-visual.json",report.ToString());return report.ToString();
    }
    public static async Task<string> Run() {
        if(!EditorApplication.isPlaying)throw new Exception("Play Mode required");
        var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single(g=>g.gameObject.scene.path==Mf6AvatarBuild.ScenePath);
        var deadline=DateTime.UtcNow.AddSeconds(15);while(gm.Module==null&&DateTime.UtcNow<deadline)await Task.Delay(100);
        var m=gm.Module as ModuleVrc3;if(m==null)throw new Exception("GestureManager not ready");
        var receiver=gm.GetComponent<MorphForgePlayReceiver>();if(!receiver.Running||receiver.ReceivePort!=9012)throw new Exception("Expected isolated OSC port9012");
        var data=JObject.Parse(File.ReadAllText("E:/tmp/morphforge/text-fixtures.json"));var checks=new JArray();
        void Check(string name,bool pass,JToken detail=null){checks.Add(new JObject{["name"]=name,["pass"]=pass,["detail"]=detail});}
        Renderer Panel(int slot)=>m.Avatar.GetComponentsInChildren<Renderer>(true).Single(r=>r.name=="Panel"+slot&&r.transform.parent.name=="TextDisplay");
        using(var udp=new UdpClient()) {
            foreach(var frame in data["frames"]){var bytes=frame["packet"].Select(v=>(byte)(int)v).ToArray();udp.Send(bytes,bytes.Length,"127.0.0.1",9012);await Task.Delay(350);}
            await Task.Delay(300);
            foreach(var row in data["checks"]) {
                var panel=Panel((int)row["slot"]);var chars=row["characters"].Select(v=>(int)v).ToArray();
                var actual=chars.Select((c,i)=>Value(panel,"_G"+i.ToString("D3"))).ToArray();
                Check("retained pages panel "+row["slot"],actual.Select((v,i)=>Mathf.Abs(v-chars[i])<.6f).All(v=>v),new JArray(actual));
                Check("published character count panel "+row["slot"],Mathf.Abs(Value(panel,"_Count")-chars.Length)<.6f,new JValue(Value(panel,"_Count")));
            }
            var before=Enumerable.Range(0,128).Select(i=>Value(Panel(0),"_G"+i.ToString("D3"))).ToArray();
            await Task.Delay(1000);
            Check("idle retains text without continuous OSC",before.Select((v,i)=>Mathf.Abs(v-Value(Panel(0),"_G"+i.ToString("D3")))<.6f).All(v=>v));
            var bytesSnap=data["snapshot"].Select(v=>(byte)(int)v).ToArray();udp.Send(bytesSnap,bytesSnap.Length,"127.0.0.1",9012);await Task.Delay(600);
            var snapshot=data["snapshotCharacters"].Select(v=>(int)v).Take(14).ToArray();
            Check("latest14 snapshot decodes without normal sequence",snapshot.Select((v,i)=>Mathf.Abs(v-Value(Panel(0),"_G"+i.ToString("D3")))<.6f).All(v=>v),new JArray(snapshot.Select((v,i)=>Value(Panel(0),"_G"+i.ToString("D3")))));
            Check("snapshot visible count",Mathf.Abs(Value(Panel(0),"_Count")-snapshot.Length)<.6f,new JValue(Value(Panel(0),"_Count")));
            var live=m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Where(p=>p.name=="Store").Sum(ps=>{var particles=new ParticleSystem.Particle[ps.main.maxParticles];var n=ps.GetParticles(particles);return particles.Take(n).Count(p=>p.startColor.a>0);});
            Check("extension and snapshot never route to voxel pools",live==0,new JValue(live));
            var delete=data["deletePacket"].Select(v=>(byte)(int)v).ToArray();udp.Send(delete,delete.Length,"127.0.0.1",9012);await Task.Delay(350);
            Check("delete hides text and advances generation",Value(Panel(0),"_Count")<.5f&&m.GetParam("MF6_Text0_Epoch").IntValue()==1,new JObject{["visibleCount"]=Value(Panel(0),"_Count"),["cachedCount"]=m.GetParam("MF6_Text0_Count").FloatValue(),["epoch"]=m.GetParam("MF6_Text0_Epoch").IntValue()});
            udp.Send(bytesSnap,bytesSnap.Length,"127.0.0.1",9012);await Task.Delay(350);Check("late old subtitle cannot resurrect deleted panel",Value(Panel(0),"_Count")<.5f);
            var fresh=data["freshSnapshot"].Select(v=>(byte)(int)v).ToArray();udp.Send(fresh,fresh.Length,"127.0.0.1",9012);await Task.Delay(350);Check("current-generation subtitle still displays",Mathf.Abs(Value(Panel(0),"_Count")-snapshot.Length)<.6f);
            var zero=data["zeroConfig"].Select(v=>(byte)(int)v).ToArray();udp.Send(zero,zero.Length,"127.0.0.1",9012);await Task.Delay(350);Check("zero opacity and black channels overwrite prior nonzero values",Value(Panel(0),"_Opacity")<.5f&&Value(Panel(0),"_Red")<.5f&&Value(Panel(0),"_Green")<.5f&&Value(Panel(0),"_Blue")<.5f);
            var restore=data["restoredConfig"].Select(v=>(byte)(int)v).ToArray();udp.Send(restore,restore.Length,"127.0.0.1",9012);await Task.Delay(350);
            var zeroWords=data["zeroSnapshot"].Select(v=>(byte)(int)v).ToArray();udp.Send(zeroWords,zeroWords.Length,"127.0.0.1",9012);await Task.Delay(350);Check("zero low-byte words replace previous glyph bytes",Mathf.Abs(Value(Panel(0),"_G001")-19968)<.5f&&Mathf.Abs(Value(Panel(0),"_G002")-256)<.5f);
            udp.Send(fresh,fresh.Length,"127.0.0.1",9012);await Task.Delay(350);Check("same-control snapshot updates again",Mathf.Abs(Value(Panel(0),"_Count")-snapshot.Length)<.6f&&Mathf.Abs(Value(Panel(0),"_G001")-snapshot[1])<.5f,new JObject{["count"]=Value(Panel(0),"_Count"),["glyph1"]=Value(Panel(0),"_G001"),["inputCount"]=m.GetParam("MF6_D28").IntValue(),["latchedCount"]=m.GetParam("MF6_Q28").IntValue()});


        }
        var fx=m.Avatar.GetComponent<Animator>().runtimeAnimatorController;
        var assets=UnityEditor.AssetDatabase.LoadAllAssetsAtPath(Mf6TextBuild.Root+"/Text.controller");
        var memory=assets.Sum(a=>UnityEngine.Profiling.Profiler.GetRuntimeMemorySizeLong(a));
        checks.Add(new JObject{["name"]="bounded text controller metadata",["pass"]=true,["detail"]=new JObject{["controllerAssetBytes"]=new FileInfo(Mf6TextBuild.Root+"/Text.controller").Length,["runtimeAssetsBytesEstimate"]=memory,["clips"]=assets.OfType<AnimationClip>().Count(),["layers"]=((UnityEditor.Animations.AnimatorController)AssetDatabase.LoadAssetAtPath<UnityEditor.Animations.AnimatorController>(Mf6TextBuild.Root+"/Text.controller")).layers.Length}});
        var result=new JObject{["success"]=checks.All(c=>(bool)c["pass"]),["checks"]=checks,["scope"]="Actual UDP9012 -> GestureManager merged native Animator -> retained Renderer MaterialPropertyBlock. No VRChat remote delivery claim."};
        Directory.CreateDirectory("Docs/TextAnimation");File.WriteAllText("Docs/TextAnimation/text-receiver-play.json",result.ToString());return result.ToString();
    }
}
