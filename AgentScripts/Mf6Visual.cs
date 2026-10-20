using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Threading.Tasks;
using UnityEngine;
using UnityEditor;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
using Newtonsoft.Json.Linq;
public static class Mf6Visual
{
    public static string Stream()
    {
        var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single(g=>(g.gameObject.scene.path=="Assets/MorphForge/MF6/NativeProbe.unity"||g.gameObject.scene.path=="Assets/MorphForge/MF6/MorphForge_MF6.unity"));var m=gm.Module as ModuleVrc3;var ps=m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Single(p=>p.name=="Store"&&p.transform.parent.name=="Pool0");var renderer=ps.GetComponent<ParticleSystemRenderer>();
        var shader=Shader.Find("Hidden/MorphForge/MF6StreamProbe");if(!shader)throw new Exception("Probe shader unavailable");var material=new Material(shader);var previousMaterial=renderer.sharedMaterial;int layer=ps.gameObject.layer;renderer.sharedMaterial=material;renderer.localBounds=new Bounds(Vector3.zero,Vector3.one*10000);ps.gameObject.layer=31;
        var camera=new GameObject("MF6 stream probe").AddComponent<Camera>();camera.enabled=false;camera.cullingMask=1<<31;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=Color.clear;camera.transform.position=Vector3.back*20;camera.transform.LookAt(Vector3.zero);camera.nearClipPlane=.001f;camera.farClipPlane=1000;
        var rt=new RenderTexture(4,4,24,RenderTextureFormat.ARGBFloat,RenderTextureReadWrite.Linear);rt.Create();camera.targetTexture=rt;var samples=new JArray();
        try{
            for(int mode=0;mode<11;mode++){material.SetFloat("_ProbeMode",mode);camera.Render();var prev=RenderTexture.active;RenderTexture.active=rt;var image=new Texture2D(4,4,TextureFormat.RGBAFloat,false,true);image.ReadPixels(new Rect(0,0,4,4),0,0);image.Apply();var value=image.GetPixel(2,2);samples.Add(new JObject{["mode"]=mode,["value"]=new JArray(value.r,value.g,value.b,value.a)});UnityEngine.Object.DestroyImmediate(image);RenderTexture.active=prev;}
            var streams=new List<ParticleSystemVertexStream>();renderer.GetActiveVertexStreams(streams);
            var report=new JObject{["success"]=true,["samples"]=samples,["rendererEnabled"]=renderer.enabled,["shader"]=previousMaterial.shader.name,["wireEncoding"]=previousMaterial.GetFloat("_WireEncoding"),["streams"]=string.Join(",",streams),["nativeCount"]=ps.particleCount};File.WriteAllText("Docs/MF6/stream-probe.json",report.ToString());return report.ToString();
        }finally{renderer.sharedMaterial=previousMaterial;renderer.ResetLocalBounds();ps.gameObject.layer=layer;UnityEngine.Object.DestroyImmediate(material);UnityEngine.Object.DestroyImmediate(camera.gameObject);rt.Release();UnityEngine.Object.DestroyImmediate(rt);}
    }
    public static Task<string> Run()=>RunCase(false,false);
    public static Task<string> RunSingle()=>RunCase(true,false);
    public static Task<string> RunTransformed()=>RunCase(false,true);
    static async Task<string> RunCase(bool single,bool transformed)
    {
        if(!Application.isPlaying)throw new Exception("Play Mode required");
        var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single(g=>(g.gameObject.scene.path=="Assets/MorphForge/MF6/NativeProbe.unity"||g.gameObject.scene.path=="Assets/MorphForge/MF6/MorphForge_MF6.unity"));var m=gm.Module as ModuleVrc3;
        var ps=m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Single(p=>p.name=="Store"&&p.transform.parent.name=="Pool0");
        var nativeRenderer=ps.GetComponent<ParticleSystemRenderer>();
        int seq=(m.GetParam("MF6_P0_LastRx").IntValue()+1)%16;
        m.GetParam("MF6_P0_Ctl").Set(m,seq*8+5);await Task.Delay(450);seq=(seq+1)%16;
        var d=new float[20];d[0]=17;d[2]=0;d[3]=100;d[4]=120;d[5]=128;d[10]=156;d[11]=120;d[12]=128;d[13]=128;d[14]=170;d[15]=128;d[6]=255;d[7]=180;d[8]=40;d[9]=single?1:2;
        for(int i=0;i<20;i++)m.GetParam("MF6_P0_D"+i.ToString("D2")).Set(m,d[i]);m.GetParam("MF6_P0_Ctl").Set(m,seq*8+2);await Task.Delay(450);
        var reference=ps.main.customSimulationSpace;var oldRotation=reference.localRotation;var oldScale=reference.localScale;
        if(transformed){reference.localRotation=oldRotation*Quaternion.Euler(20,40,10);reference.localScale=oldScale*.3f;await Task.Delay(100);}
        int oldLayer=ps.gameObject.layer;ps.gameObject.layer=31;
        var camera=new GameObject("MF6 native visual probe").AddComponent<Camera>();camera.enabled=false;camera.cullingMask=1<<31;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=new Color(.04f,.06f,.09f);camera.orthographic=true;camera.nearClipPlane=.00001f;camera.farClipPlane=1000;
        var target=new RenderTexture(512,512,24){antiAliasing=4};target.Create();camera.targetTexture=target;
        var checks=new JArray();Directory.CreateDirectory("Docs/MF6/visual");
        try{
            camera.cullingMask=0;camera.Render();var beforeBlank=RenderTexture.active;RenderTexture.active=target;var blank=new Texture2D(1,1,TextureFormat.RGBA32,false);blank.ReadPixels(new Rect(0,0,1,1),0,0);blank.Apply();Color32 background=blank.GetPixels32()[0];UnityEngine.Object.DestroyImmediate(blank);RenderTexture.active=beforeBlank;camera.cullingMask=1<<31;
            Vector3 center=ps.transform.TransformPoint(new Vector3(0,4,0));float span=Mathf.Abs(ps.transform.lossyScale.x)*16;camera.orthographicSize=span*.7f;
            foreach(var view in new[]{new{name="front",direction=Vector3.forward},new{name="back",direction=Vector3.back},new{name="hero",direction=new Vector3(1,.7f,2)},new{name="close",direction=Vector3.forward}}){
                camera.orthographicSize=view.name=="close"?span*.06f:span*.7f;camera.transform.position=center+ps.transform.TransformDirection(view.direction).normalized*span*(view.name=="close"?.05f:3);camera.transform.LookAt(center,ps.transform.TransformDirection(Vector3.up));camera.Render();var previous=RenderTexture.active;RenderTexture.active=target;var image=new Texture2D(512,512,TextureFormat.RGBA32,false);image.ReadPixels(new Rect(0,0,512,512),0,0);image.Apply();RenderTexture.active=previous;
                var pixels=image.GetPixels32();int visible=pixels.Count(p=>!p.Equals(background));File.WriteAllBytes("Docs/MF6/visual/"+view.name+".png",image.EncodeToPNG());UnityEngine.Object.DestroyImmediate(image);checks.Add(new JObject{["view"]=view.name,["visiblePixels"]=visible,["passed"]=single&&view.name=="back"?visible==0:visible>1000});
            }
            var renderer=ps.GetComponent<ParticleSystemRenderer>();var bounds=renderer.bounds;
            var report=new JObject{["success"]=checks.All(c=>(bool)c["passed"]),["diagnosticBoundsOverride"]=false,["checks"]=checks,["nativeCount"]=ps.particleCount,["lossyScale"]=new JArray(ps.transform.lossyScale.x,ps.transform.lossyScale.y,ps.transform.lossyScale.z),["localToWorld"]=ps.transform.localToWorldMatrix.ToString(),["rendererBounds"]=JObject.FromObject(new{center=new[]{bounds.center.x,bounds.center.y,bounds.center.z},size=new[]{bounds.size.x,bounds.size.y,bounds.size.z}}),["scope"]="Actual production particle renderer after native Animator birth; native invisible bounds carrier, no transient bounds override. No MeshRenderer stand-in or particle injection."};report["singleSided"]=single;report["transformed"]=transformed;File.WriteAllText("Docs/MF6/"+(single?"native-single-visual":transformed?"native-transformed-visual":"native-visual")+".json",report.ToString());return report.ToString();
        }finally{reference.localRotation=oldRotation;reference.localScale=oldScale;nativeRenderer.ResetLocalBounds();ps.gameObject.layer=oldLayer;camera.targetTexture=null;UnityEngine.Object.DestroyImmediate(camera.gameObject);target.Release();UnityEngine.Object.DestroyImmediate(target);}
    }
}
