using System;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Net;
using System.Net.Sockets;
using UnityEditor;
using UnityEngine;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;

namespace MorphForge.Editor
{
    // Editor-only observations and reference rendering. No parameter writes here.
    public static class Probe14Harness
    {
        const BindingFlags InstanceFlags=BindingFlags.Instance|BindingFlags.Public|BindingFlags.NonPublic;

        [Serializable] public class Input
        {
            public string shape;
            public int triangle_count;
            public float[] coordinates;
        }

        [Serializable] public class Snapshot
        {
            public string label,environment,avatar,shader,input,actualImage,referenceImage;
            public bool visible,carrierCpuVerticesZero,carrierIndicesUnchanged;
            public int parameterCount,triangleCount,vertexCount,foregroundPixels;
            public float control,materialControl,materialTriangleCount,coordinateParameterMaxError,referenceMeanAbsoluteError,referenceMaxError;
            public float foregroundR,foregroundG,foregroundB;
            public float[] coordinates,materialCoordinates;
        }

        static GestureManager Manager()
        {
            var manager=UnityEngine.Object.FindObjectsOfType<GestureManager>().FirstOrDefault(g=>g.name=="MorphForge Gesture Manager");
            if(!manager)throw new InvalidOperationException("MorphForge Gesture Manager is not loaded.");
            return manager;
        }

        static ModuleVrc3 Module()
        {
            if(!EditorApplication.isPlaying)throw new InvalidOperationException("Enter Play Mode for the probe.");
            var manager=Manager();
            if(manager.Module==null)
                manager.SetModule(new ModuleVrc3(manager.settings.favourite.GetComponent<VRC.SDK3.Avatars.Components.VRCAvatarDescriptor>()));
            var module=manager.Module as ModuleVrc3;
            if(module==null || !module.Params.ContainsKey("MFProbe_V71"))
                throw new InvalidOperationException("The actual MA/NDMF FX graph did not expose probe parameters.");
            return module;
        }

        static object Osc(ModuleVrc3 module)
        {
            return typeof(ModuleVrc3).GetField("OscModule",InstanceFlags).GetValue(module);
        }

        [MenuItem("Tools/MorphForge/2. Start Gesture Manager OSC on 9010")]
        public static void MenuStart() { Debug.Log(StartOsc()); }

        public static string StartOsc()
        {
            var module=Module();
            var osc=Osc(module);
            bool enabled=(bool)osc.GetType().GetProperty("Enabled",InstanceFlags).GetValue(osc);
            if(!enabled)
            {
                // Preflight before GM's method, which otherwise opens a modal port-conflict dialog.
                using(var available=new UdpClient(new IPEndPoint(IPAddress.Any,9010))) { }
                osc.GetType().GetMethod("Start",InstanceFlags).Invoke(osc,new object[]{9010,"127.0.0.1",9011});
            }
            enabled=(bool)osc.GetType().GetProperty("Enabled",InstanceFlags).GetValue(osc);
            if(!enabled)throw new InvalidOperationException("Gesture Manager OSC did not start.");
            return "Gesture Manager native OSC receiver on UDP 9010; "+module.Params.Keys.Count(n=>n.StartsWith(Probe14Build.Prefix))+" probe parameters; Editor emulation only.";
        }

        [MenuItem("Tools/MorphForge/3. Stop probe OSC")]
        public static void StopOsc()
        {
            var manager=Manager();
            var module=manager.Module as ModuleVrc3;
            if(module==null)return;
            var osc=Osc(module);
            osc.GetType().GetMethod("Stop",InstanceFlags).Invoke(osc,new object[]{true});
        }

        public static string Capture(string label,string inputPath="")
        {
            if(label.Any(c=>!(char.IsLetterOrDigit(c)||c=='_'||c=='-')))throw new ArgumentException("Invalid capture label.");
            var module=Module();
            var root=module.Avatar.transform.Find(Probe14Build.ModuleName);
            if(!root)throw new InvalidOperationException("Processed Avatar lost the probe module.");
            var target=root.Find(string.IsNullOrEmpty(inputPath)?"Signal":"Geometry");
            var renderer=target.GetComponent<MeshRenderer>();
            var carrier=root.Find("Geometry").GetComponent<MeshFilter>().sharedMesh;
            var input=string.IsNullOrEmpty(inputPath)?null:JsonUtility.FromJson<Input>(File.ReadAllText(inputPath));
            var coordinates=Enumerable.Range(0,72).Select(i=>module.GetParam("MFProbe_V"+i.ToString("D2")).FloatValue()).ToArray();
            var materialCoordinates=Enumerable.Range(0,72).Select(i=>MaterialValue(root.Find("Geometry").GetComponent<MeshRenderer>(),"_V"+i.ToString("D2"))).ToArray();
            Directory.CreateDirectory("Docs/Probe14");
            string actualPath="Docs/Probe14/"+label+".png";
            var actual=Render(renderer,actualPath);
            var background=actual[0];
            var foreground=actual.Where(c=>Math.Abs(c.r-background.r)+Math.Abs(c.g-background.g)+Math.Abs(c.b-background.b)>15).ToArray();
            var report=new Snapshot {
                label=label,environment="Unity Play Mode + Gesture Manager native OSC; not VRChat client",
                avatar=module.Avatar.name,shader=renderer.sharedMaterial.shader.name,input=inputPath,actualImage=actualPath,
                visible=module.GetParam("MFProbe_Visible").BoolValue(),
                control=module.GetParam("MFProbe_Control").FloatValue(),
                parameterCount=module.Params.Keys.Count(n=>n.StartsWith(Probe14Build.Prefix)),
                materialControl=MaterialValue(root.Find("Signal").GetComponent<MeshRenderer>(),"_Control"),
                materialTriangleCount=MaterialValue(root.Find("Geometry").GetComponent<MeshRenderer>(),"_TriangleCount"),
                coordinates=coordinates,materialCoordinates=materialCoordinates,
                carrierCpuVerticesZero=carrier.vertices.All(v=>v==Vector3.zero),
                carrierIndicesUnchanged=carrier.triangles.SequenceEqual(Enumerable.Range(0,24)),
                triangleCount=input==null?0:input.triangle_count,vertexCount=carrier.vertexCount,
                foregroundPixels=foreground.Length,
                foregroundR=foreground.Length==0?0:(float)foreground.Average(c=>(double)c.r),
                foregroundG=foreground.Length==0?0:(float)foreground.Average(c=>(double)c.g),
                foregroundB=foreground.Length==0?0:(float)foreground.Average(c=>(double)c.b)
            };
            if(input!=null)
            {
                if(input.coordinates==null||input.coordinates.Length!=72)throw new InvalidOperationException("Expected 72 external coordinate scalars.");
                report.coordinateParameterMaxError=coordinates.Select((v,i)=>Math.Abs(v-input.coordinates[i])).Max();
                string referencePath="Docs/Probe14/"+label+"-reference.png";
                var reference=Reference(renderer,input,referencePath);
                long sum=0;int max=0;
                for(int i=0;i<actual.Length;i++)
                {
                    int dr=Math.Abs(actual[i].r-reference[i].r),dg=Math.Abs(actual[i].g-reference[i].g),db=Math.Abs(actual[i].b-reference[i].b);
                    sum+=dr+dg+db;max=Math.Max(max,Math.Max(dr,Math.Max(dg,db)));
                }
                report.referenceImage=referencePath;
                report.referenceMeanAbsoluteError=(float)(sum/(double)(actual.Length*3));
                report.referenceMaxError=max;
            }
            string json=JsonUtility.ToJson(report,true);
            File.WriteAllText("Docs/Probe14/"+label+".json",json);
            return json;
        }

        static float MaterialValue(Renderer renderer,string property)
        {
            var block=new MaterialPropertyBlock();renderer.GetPropertyBlock(block);
            if(block.isEmpty)renderer.GetPropertyBlock(block,0);
            return block.isEmpty?renderer.sharedMaterial.GetFloat(property):block.GetFloat(property);
        }

        static Color32[] Reference(MeshRenderer actual,Input input,string path)
        {
            var referenceObject=new GameObject("CPU reference - Editor test only",typeof(MeshFilter),typeof(MeshRenderer));
            var mesh=new Mesh();
            var material=new Material(actual.sharedMaterial);
            bool enabled=actual.enabled;
            try
            {
                referenceObject.transform.SetParent(actual.transform.parent,false);
                referenceObject.transform.localPosition=actual.transform.localPosition;
                referenceObject.transform.localRotation=actual.transform.localRotation;
                referenceObject.transform.localScale=actual.transform.localScale;
                mesh.vertices=Enumerable.Range(0,24).Select(i=>new Vector3(input.coordinates[i*3],input.coordinates[i*3+1],input.coordinates[i*3+2])).ToArray();
                mesh.triangles=Enumerable.Range(0,24).ToArray();mesh.bounds=new Bounds(Vector3.zero,Vector3.one*4);
                referenceObject.GetComponent<MeshFilter>().sharedMesh=mesh;
                material.SetFloat("_Mode",2);material.SetFloat("_Visible",1);material.SetFloat("_TriangleCount",input.triangle_count);
                var renderer=referenceObject.GetComponent<MeshRenderer>();renderer.sharedMaterial=material;
                actual.enabled=false;
                return Render(renderer,path);
            }
            finally
            {
                actual.enabled=enabled;
                UnityEngine.Object.DestroyImmediate(referenceObject);
                UnityEngine.Object.DestroyImmediate(mesh);
                UnityEngine.Object.DestroyImmediate(material);
            }
        }

        static Color32[] Render(MeshRenderer renderer,string path)
        {
            var cameraObject=new GameObject("Probe capture camera - Editor only",typeof(Camera));
            var camera=cameraObject.GetComponent<Camera>();
            int oldLayer=renderer.gameObject.layer;
            var oldActive=RenderTexture.active;
            var texture=new Texture2D(512,512,TextureFormat.RGB24,false);
            var target=RenderTexture.GetTemporary(512,512,24,RenderTextureFormat.ARGB32);
            try
            {
                renderer.gameObject.layer=31;camera.cullingMask=1<<31;
                camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=new Color(.035f,.05f,.08f);
                camera.orthographic=true;camera.orthographicSize=1.1f*renderer.transform.lossyScale.x;
                camera.nearClipPlane=.001f;camera.farClipPlane=100;
                camera.transform.position=renderer.transform.TransformPoint(new Vector3(2.3f,1.7f,3f));
                camera.transform.LookAt(renderer.transform.TransformPoint(new Vector3(0,.1f,0)));
                camera.targetTexture=target;camera.Render();RenderTexture.active=target;
                texture.ReadPixels(new Rect(0,0,512,512),0,0);texture.Apply();
                File.WriteAllBytes(path,texture.EncodeToPNG());
                return texture.GetPixels32();
            }
            finally
            {
                renderer.gameObject.layer=oldLayer;camera.targetTexture=null;RenderTexture.active=oldActive;
                RenderTexture.ReleaseTemporary(target);UnityEngine.Object.DestroyImmediate(texture);UnityEngine.Object.DestroyImmediate(cameraObject);
            }
        }
    }
}
