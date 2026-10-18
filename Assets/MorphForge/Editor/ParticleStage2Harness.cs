using System;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Collections.Generic;
using System.Net;
using System.Net.Sockets;
using UnityEditor;
using UnityEngine;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;

namespace MorphForge.Editor
{
    // Observations/reference rendering only. Does not write Animator input or call Emit.
    public static class ParticleStage2Harness
    {
        const BindingFlags Flags=BindingFlags.Instance|BindingFlags.Public|BindingFlags.NonPublic;
        static readonly List<object> Trace=new List<object>();
        static string traceKey="";
        static double startTime;
        [Serializable] public class ExpectedBox{public int[] min,size;public int styleId=1,tintId,motionPhase;}
        [Serializable] public class ExpectedScene{public ExpectedBox[] boxes;}

        static GestureManager Manager(){return UnityEngine.Object.FindObjectsOfType<GestureManager>().First(g=>g.name=="MorphForge Stage2 Gesture Manager");}
        static ModuleVrc3 Module()
        {
            if(!EditorApplication.isPlaying)throw new InvalidOperationException("Enter Stage2 Play Mode.");
            var g=Manager();if(g.Module==null)g.SetModule(new ModuleVrc3(g.settings.favourite.GetComponent<VRC.SDK3.Avatars.Components.VRCAvatarDescriptor>()));
            var m=g.Module as ModuleVrc3;
            if(m==null||!m.Params.ContainsKey("MF2_D09"))throw new InvalidOperationException("Processed Stage2 FX is not available.");return m;
        }
        static object Osc(ModuleVrc3 m){return typeof(ModuleVrc3).GetField("OscModule",Flags).GetValue(m);}
        static Transform Root(ModuleVrc3 m){return m.Avatar.transform.Find(ParticleStage2Build.ModuleName);}
        static ParticleSystem Store(ModuleVrc3 m){return Root(m).Find("Store").GetComponent<ParticleSystem>();}
        public static string StartOsc()
        {
            // Test-only isolation. Edit Mode restores every other scene's active state.
            if(!EditorApplication.isPlaying)throw new InvalidOperationException("Enter Stage2 Play Mode first.");
            for(int i=0;i<UnityEngine.SceneManagement.SceneManager.sceneCount;i++)
            {
                var scene=UnityEngine.SceneManagement.SceneManager.GetSceneAt(i);
                if(scene.path==ParticleStage2Build.ScenePath)continue;
                foreach(var go in scene.GetRootGameObjects())go.SetActive(false);
            }
            var m=Module();var osc=Osc(m);bool enabled=(bool)osc.GetType().GetProperty("Enabled",Flags).GetValue(osc);
            if(!enabled){using(var p=new UdpClient(new IPEndPoint(IPAddress.Any,9012))){}osc.GetType().GetMethod("Start",Flags).Invoke(osc,new object[]{9012,"127.0.0.1",9013});}
            Trace.Clear();traceKey="";startTime=EditorApplication.timeSinceStartup;EditorApplication.update-=Observe;EditorApplication.update+=Observe;
            return Newtonsoft.Json.JsonConvert.SerializeObject(new{port=9012,initialParticleCount=Store(m).particleCount,sourceTypes=Enumerable.Range(0,10).Select(i=>m.GetParam("MF2_D"+i.ToString("D2")).TypeText).ToArray(),scope="Native GM OSC, actual Avatar FX; Editor only"});
        }
        public static string StopOsc()
        {
            EditorApplication.update-=Observe;
            var m=Manager().Module as ModuleVrc3;if(m!=null){var o=Osc(m);o.GetType().GetMethod("Stop",Flags).Invoke(o,new object[]{true});}
            Directory.CreateDirectory("Docs/Stage2");File.WriteAllText("Docs/Stage2/trace.json",Newtonsoft.Json.JsonConvert.SerializeObject(Trace,Newtonsoft.Json.Formatting.Indented));
            return "Stopped Stage2 OSC; read-only trace saved.";
        }
        static void Observe()
        {
            if(!EditorApplication.isPlaying){EditorApplication.update-=Observe;return;}
            try
            {
                var m=Module();var root=Root(m);var store=Store(m);
                int ctl=m.GetParam("MF2_Ctl").IntValue();bool busy=m.GetParam("MF2_Busy").BoolValue();
                bool pump=root.Find("Minimum/Center/Pump").gameObject.activeInHierarchy;bool trigger=store.trigger.enabled;
                string key=ctl+"/"+busy+"/"+pump+"/"+trigger+"/"+store.particleCount+"/"+m.GetParam("MF2_DoneCount").FloatValue()+"/"+m.GetParam("MF2_InboxReady").BoolValue();
                if(key==traceKey)return;traceKey=key;
                Trace.Add(new{time=EditorApplication.timeSinceStartup-startTime,ctl,busy,pump,trigger,count=store.particleCount,locator=V(root.Find("Minimum").localPosition),received=m.GetParam("MF2_RxCount").FloatValue(),completed=m.GetParam("MF2_DoneCount").FloatValue(),inboxReady=m.GetParam("MF2_InboxReady").BoolValue(),op=m.GetParam("MF2_Op").IntValue(),lastSeq=m.GetParam("MF2_LastSeq").IntValue()});
            }catch(Exception){}
        }
        static float[] V(Vector3 v){return new[]{v.x,v.y,v.z};}
        static object Particle(ParticleSystem.Particle p)
        {var c=p.startColor;return new{position=V(p.position),size=V(p.startSize3D),color=new[]{(int)c.r,(int)c.g,(int)c.b,(int)c.a},codes=new[]{Mathf.RoundToInt(c.r*63f/255),Mathf.RoundToInt(c.g*63f/255),Mathf.RoundToInt(c.b*63f/255),Mathf.RoundToInt(c.a*63f/255)},p.remainingLifetime,p.startLifetime};}
        public static string Capture(string label,string expectedPath="",bool images=true)
        {
            if(label.Any(c=>!(char.IsLetterOrDigit(c)||c=='_'||c=='-')))throw new ArgumentException("Invalid label");
            var m=Module();var root=Root(m);var store=Store(m);
            var particles=new ParticleSystem.Particle[store.main.maxParticles];int n=store.GetParticles(particles);particles=particles.Take(n).ToArray();
            var expected=string.IsNullOrEmpty(expectedPath)?null:JsonUtility.FromJson<ExpectedScene>(File.ReadAllText(expectedPath));
            Directory.CreateDirectory("Docs/Stage2");
            string actualPath=images?"Docs/Stage2/"+label+".png":"",referencePath="";
            int foreground=0;double error=0;int maxError=0;
            if(images)
            {
                Bounds bounds=BoundsFor(particles,root);
                var renderer=store.GetComponent<ParticleSystemRenderer>();var actual=Render(renderer,bounds,actualPath);
                var bg=actual[0];foreground=actual.Count(c=>Math.Abs(c.r-bg.r)+Math.Abs(c.g-bg.g)+Math.Abs(c.b-bg.b)>20);
                if(expected!=null)
                {
                    referencePath="Docs/Stage2/"+label+"-reference.png";
                    var reference=Reference(root,renderer,expected,bounds,referencePath);
                    long sum=0;for(int i=0;i<actual.Length;i++)
                    {int a=Math.Abs(actual[i].r-reference[i].r),b=Math.Abs(actual[i].g-reference[i].g),c=Math.Abs(actual[i].b-reference[i].b);sum+=a+b+c;maxError=Math.Max(maxError,Math.Max(a,Math.Max(b,c)));}
                    error=sum/(double)(actual.Length*3);
                }
            }
            var result=new{label,environment="Unity Play Mode + native Gesture Manager OSC, not VRChat client",count=n,
                sourceControl=m.GetParam("MF2_Ctl").IntValue(),source=Enumerable.Range(0,10).Select(i=>m.GetParam("MF2_D"+i.ToString("D2")).IntValue()).ToArray(),
                received=m.GetParam("MF2_RxCount").FloatValue(),completed=m.GetParam("MF2_DoneCount").FloatValue(),
                inboxReady=m.GetParam("MF2_InboxReady").BoolValue(),lastRxSeq=m.GetParam("MF2_LastRxSeq").IntValue(),
                work=Enumerable.Range(0,10).Select(i=>m.GetParam("MF2_W"+i.ToString("D2")).FloatValue()).ToArray(),
                op=m.GetParam("MF2_Op").IntValue(),lastSeq=m.GetParam("MF2_LastSeq").IntValue(),busy=m.GetParam("MF2_Busy").BoolValue(),
                pumpActive=root.Find("Minimum/Center/Pump").gameObject.activeInHierarchy,triggerEnabled=store.trigger.enabled,
                storePlaying=store.isPlaying,storePaused=store.isPaused,particles=particles.Select(Particle).ToArray(),
                nativeStartSize=V(new Vector3(store.main.startSizeX.constant,store.main.startSizeY.constant,store.main.startSizeZ.constant)),
                locator=V(root.Find("Minimum").localPosition),centerOffset=V(root.Find("Minimum/Center").localPosition),
                eraseScale=V(root.Find("Minimum/Center/Eraser").localScale),expectedPath,actualImage=actualPath,referenceImage=referencePath,
                foregroundPixels=foreground,referenceMeanAbsoluteError=error,referenceMaxError=maxError,traceEvents=Trace.Count};
            string json=Newtonsoft.Json.JsonConvert.SerializeObject(result,Newtonsoft.Json.Formatting.Indented);
            File.WriteAllText("Docs/Stage2/"+label+".json",json);return json;
        }
        static Bounds BoundsFor(ParticleSystem.Particle[] p,Transform root)
        {
            if(p.Length==0)return new Bounds(root.position,Vector3.one);
            var b=new Bounds(root.TransformPoint(p[0].position),Vector3.zero);
            foreach(var a in p)
            {
                var c=root.TransformPoint(a.position);var s=Vector3.Scale(a.startSize3D,root.lossyScale);
                b.Encapsulate(new Bounds(c,s));
            }return b;
        }
        public static string CaptureFaces(string label,string expectedPath)
        {
            if(label.Any(c=>!(char.IsLetterOrDigit(c)||c=='_'||c=='-')))throw new ArgumentException("Invalid label");
            var m=Module();var root=Root(m);var store=Store(m);var renderer=store.GetComponent<ParticleSystemRenderer>();
            var particles=new ParticleSystem.Particle[store.main.maxParticles];int n=store.GetParticles(particles);
            var bounds=BoundsFor(particles.Take(n).ToArray(),root);
            var expected=JsonUtility.FromJson<ExpectedScene>(File.ReadAllText(expectedPath));
            var names=new[]{"px","nx","py","ny","pz","nz"};
            var directions=new[]{Vector3.right,Vector3.left,Vector3.up,Vector3.down,Vector3.forward,Vector3.back};
            var views=new List<object>();
            for(int k=0;k<6;k++)
            {
                string path="Docs/Stage2/"+label+"-"+names[k]+".png",referencePath="Docs/Stage2/"+label+"-"+names[k]+"-reference.png";
                var actual=Render(renderer,bounds,path,directions[k]);
                var reference=Reference(root,renderer,expected,bounds,referencePath,directions[k]);
                var bg=actual[0];long error=0;
                for(int i=0;i<actual.Length;i++)error+=Math.Abs(actual[i].r-reference[i].r)+Math.Abs(actual[i].g-reference[i].g)+Math.Abs(actual[i].b-reference[i].b);
                views.Add(new{face=names[k],actualImage=path,referenceImage=referencePath,
                    foregroundPixels=actual.Count(c=>Math.Abs(c.r-bg.r)+Math.Abs(c.g-bg.g)+Math.Abs(c.b-bg.b)>20),referenceMeanAbsoluteError=error/(double)(actual.Length*3)});
            }
            string json=Newtonsoft.Json.JsonConvert.SerializeObject(new{label,views},Newtonsoft.Json.Formatting.Indented);
            File.WriteAllText("Docs/Stage2/"+label+".json",json);return json;
        }
        static Color32[] Reference(Transform root,ParticleSystemRenderer actual,ExpectedScene scene,Bounds bounds,string path,Vector3? direction=null)
        {
            var go=new GameObject("Stage2 independent CPU reference - test only",typeof(MeshFilter),typeof(MeshRenderer));
            var mesh=new Mesh();bool was=actual.enabled;
            try
            {
                go.transform.SetParent(root,false);var cube=actual.mesh;var cv=cube.vertices;var cn=cube.normals;var ct=cube.triangles;
                var verts=new List<Vector3>();var normals=new List<Vector3>();var colors=new List<Color32>();var tris=new List<int>();var uv0=new List<Vector4>();var uv1=new List<Vector3>();var cubeUV=cube.uv;var faceUV=cube.uv2;
                foreach(var b in scene.boxes??new ExpectedBox[0])
                {
                    var size=new Vector3(b.size[0],b.size[1],b.size[2])*.25f;
                    var center=new Vector3(b.min[0]+b.size[0]*.5f-128,b.min[1]+b.size[1]*.5f-128,b.min[2]+b.size[2]*.5f-128)*.25f;
                    int offset=verts.Count;var color=(Color32)new Color((b.styleId&63)/63f,(b.styleId>>6)/63f,b.tintId/63f,b.motionPhase/63f);
                    for(int i=0;i<cv.Length;i++){verts.Add(center+Vector3.Scale(cv[i],size));normals.Add(cn[i]);colors.Add(color);uv0.Add(new Vector4(cubeUV[i].x,cubeUV[i].y,faceUV[i].x,faceUV[i].y));uv1.Add(size);}
                    tris.AddRange(ct.Select(i=>i+offset));
                }
                mesh.SetVertices(verts);mesh.SetNormals(normals);mesh.SetColors(colors);mesh.SetUVs(0,uv0);mesh.SetUVs(1,uv1);mesh.SetTriangles(tris,0);mesh.RecalculateBounds();
                go.GetComponent<MeshFilter>().sharedMesh=mesh;var renderer=go.GetComponent<MeshRenderer>();renderer.sharedMaterial=actual.sharedMaterial;
                actual.enabled=false;return Render(renderer,bounds,path,direction);
            }
            finally{actual.enabled=was;UnityEngine.Object.DestroyImmediate(go);UnityEngine.Object.DestroyImmediate(mesh);}
        }
        static Color32[] Render(Renderer renderer,Bounds bounds,string path,Vector3? direction=null)
        {
            var go=new GameObject("Stage2 snapshot camera - test only",typeof(Camera));var cam=go.GetComponent<Camera>();
            var tex=new Texture2D(512,512,TextureFormat.RGB24,false);var rt=RenderTexture.GetTemporary(512,512,24,RenderTextureFormat.ARGB32);
            int layer=renderer.gameObject.layer;var old=RenderTexture.active;
            try
            {
                renderer.gameObject.layer=31;cam.cullingMask=1<<31;cam.clearFlags=CameraClearFlags.SolidColor;cam.backgroundColor=new Color(.035f,.05f,.08f);
                float span=Mathf.Max(.5f,Mathf.Max(bounds.size.x,Mathf.Max(bounds.size.y,bounds.size.z)));
                cam.orthographic=true;cam.orthographicSize=span*.8f;cam.nearClipPlane=.001f;cam.farClipPlane=500;
                var view=direction??new Vector3(1.2f,.75f,2);
                cam.transform.position=bounds.center+view.normalized*span*4;cam.transform.LookAt(bounds.center,Mathf.Abs(view.normalized.y)>.9f?Vector3.forward:Vector3.up);
                cam.targetTexture=rt;cam.Render();RenderTexture.active=rt;tex.ReadPixels(new Rect(0,0,512,512),0,0);tex.Apply();File.WriteAllBytes(path,tex.EncodeToPNG());return tex.GetPixels32();
            }
            finally{renderer.gameObject.layer=layer;cam.targetTexture=null;RenderTexture.active=old;RenderTexture.ReleaseTemporary(rt);UnityEngine.Object.DestroyImmediate(tex);UnityEngine.Object.DestroyImmediate(go);}
        }
    }
}
