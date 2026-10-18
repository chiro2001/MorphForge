using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using UnityEngine;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
public static class PlacementGeometryCheck
{
    static Transform T(Transform r,string n){return r.GetComponentsInChildren<Transform>(true).First(t=>t.name==n);}
    static float[] V(Vector3 p){return new[]{p.x,p.y,p.z};}
    public static async Task<string> Check()
    {
        var m=UnityEngine.Object.FindObjectsOfType<GestureManager>().First(g=>g.name=="MorphForge Stage3 Gesture Manager").Module as ModuleVrc3;
        var ps=T(m.Avatar.transform,"Store").GetComponent<ParticleSystem>();var rend=ps.GetComponent<ParticleSystemRenderer>();var anchor=T(m.Avatar.transform,"AnchorPose");var content=T(m.Avatar.transform,"ContentScale");
        var p=new ParticleSystem.Particle[64];int count=ps.GetParticles(p);if(count!=1)throw new Exception("Requires one OSC-born cuboid");
        var main=ps.main;var oldMode=main.scalingMode;var oldScale=m.GetParam("MF_Scale").FloatValue();int layer=ps.gameObject.layer;
        var camGo=new GameObject("Placement scale validation camera",typeof(Camera));var cam=camGo.GetComponent<Camera>();
        cam.cullingMask=1<<31;cam.clearFlags=CameraClearFlags.SolidColor;cam.backgroundColor=new Color(.035f,.05f,.08f);cam.orthographic=true;cam.nearClipPlane=.001f;cam.farClipPlane=500;
        var rt=RenderTexture.GetTemporary(512,512,24);var tex=new Texture2D(512,512,TextureFormat.RGB24,false);var oldRt=RenderTexture.active;
        var records=new System.Collections.Generic.List<object>();Color32[] baseline=null;
        try
        {
            ps.gameObject.layer=31;cam.targetTexture=rt;
            foreach(var mode in new[]{ParticleSystemScalingMode.Hierarchy,ParticleSystemScalingMode.Local})
            foreach(float s in new[]{1f,.25f,.05f})
            {
                main.scalingMode=mode;m.GetParam("MF_Scale").Set(m,s);await Task.Delay(200);
                var sim=main.customSimulationSpace;var center=sim.TransformPoint(p[0].position);var dir=new Vector3(1.2f,.75f,2).normalized;
                cam.transform.position=center+dir*4*s;cam.transform.rotation=Quaternion.LookRotation(-dir,Vector3.up);cam.orthographicSize=1.1f*s;
                cam.Render();RenderTexture.active=rt;tex.ReadPixels(new Rect(0,0,512,512),0,0);tex.Apply();var pixels=tex.GetPixels32();
                if(s==1)baseline=pixels;
                double error=0;if(baseline!=null){long sum=0;for(int i=0;i<pixels.Length;i++)sum+=Math.Abs(pixels[i].r-baseline[i].r)+Math.Abs(pixels[i].g-baseline[i].g)+Math.Abs(pixels[i].b-baseline[i].b);error=sum/(double)(pixels.Length*3);}
                var mesh=new Mesh();rend.BakeMesh(mesh,cam,true);
                string file="scale-"+mode+"-"+s.ToString(System.Globalization.CultureInfo.InvariantCulture)+".png";Directory.CreateDirectory("Docs/Stage3");File.WriteAllBytes("Docs/Stage3/"+file,tex.EncodeToPNG());
                records.Add(new{mode=mode.ToString(),scale=s,actualContentScale=V(content.lossyScale),nativeBounds=V(rend.bounds.size),meshBounds=V(mesh.bounds.size),meshCenter=V(mesh.bounds.center),imageMeanAbsoluteError=error,file});UnityEngine.Object.DestroyImmediate(mesh);
            }
        }
        finally{ps.gameObject.layer=layer;main.scalingMode=oldMode;m.GetParam("MF_Scale").Set(m,oldScale);RenderTexture.active=oldRt;cam.targetTexture=null;RenderTexture.ReleaseTemporary(rt);UnityEngine.Object.DestroyImmediate(camGo);UnityEngine.Object.DestroyImmediate(tex);}
        bool success=records.Where(x=>(string)x.GetType().GetProperty("mode").GetValue(x)=="Hierarchy").All(x=>(double)x.GetType().GetProperty("imageMeanAbsoluteError").GetValue(x)<.1);
        string result=Newtonsoft.Json.JsonConvert.SerializeObject(new{success,scope="Native OSC-created cuboid; camera position and orthographic scale follow actual content scale; Local mode is a negative control",records},Newtonsoft.Json.Formatting.Indented);File.WriteAllText("Docs/Stage3/scale-validation.json",result);return result;
    }
}
