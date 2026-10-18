using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using UnityEditor;
using UnityEngine;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;

public static class ParticleStage2PoseCheck
{
    static ModuleVrc3 Module()
    {return UnityEngine.Object.FindObjectsOfType<GestureManager>().First(g=>g.name=="MorphForge Stage2 Gesture Manager").Module as ModuleVrc3;}
    public static string Stats()
    {
        var m=Module();return Newtonsoft.Json.JsonConvert.SerializeObject(new{frame=Time.frameCount,realtime=Time.realtimeSinceStartupAsDouble,
            received=m.GetParam("MF2_RxCount").FloatValue(),completed=m.GetParam("MF2_DoneCount").FloatValue(),
            busy=m.GetParam("MF2_Busy").BoolValue(),inboxReady=m.GetParam("MF2_InboxReady").BoolValue()});
    }
    public static string Diagnose()
    {
        var m=Module();var root=m.Avatar.transform.Find("MorphForgeStage2");
        return Newtonsoft.Json.JsonConvert.SerializeObject(new{parameters=m.Params.Keys.Where(k=>k.StartsWith("MF2_")).ToArray(),
            hierarchy=root.GetComponentsInChildren<Transform>(true).Select(t=>t.name).ToArray(),
            controllers=m.Avatar.GetComponentsInChildren<Animator>(true).Select(a=>new{a.name,parameters=a.parameters.Select(p=>p.name).Where(n=>n.StartsWith("MF2_")).ToArray()}).ToArray()},Newtonsoft.Json.Formatting.Indented);
    }
    static Color32[] Render(Transform root,ParticleSystem ps,string name)
    {
        var p=new ParticleSystem.Particle[ps.main.maxParticles];int count=ps.GetParticles(p);
        if(count==0)throw new InvalidOperationException("Texture check needs visible OSC-created boxes.");
        var bounds=new Bounds(p[0].position,p[0].startSize3D);
        foreach(var item in p.Take(count))bounds.Encapsulate(new Bounds(item.position,item.startSize3D));
        float span=Mathf.Max(.5f,Mathf.Max(bounds.size.x,Mathf.Max(bounds.size.y,bounds.size.z)));
        var go=new GameObject("Texture co-moving observation camera",typeof(Camera));var cam=go.GetComponent<Camera>();
        var rt=RenderTexture.GetTemporary(512,512,24,RenderTextureFormat.ARGB32);var tex=new Texture2D(512,512,TextureFormat.RGB24,false);
        int oldLayer=ps.gameObject.layer;var oldRt=RenderTexture.active;
        try
        {
            ps.gameObject.layer=31;cam.cullingMask=1<<31;cam.clearFlags=CameraClearFlags.SolidColor;cam.backgroundColor=new Color(.035f,.05f,.08f);
            go.transform.SetParent(root,false);var direction=new Vector3(1.2f,.75f,2).normalized;
            go.transform.localPosition=bounds.center+direction*span*4;go.transform.localRotation=Quaternion.LookRotation(-direction,Vector3.up);
            cam.orthographic=true;cam.orthographicSize=span*.8f*Mathf.Abs(root.lossyScale.x);cam.nearClipPlane=.001f;cam.farClipPlane=500;
            cam.targetTexture=rt;cam.Render();RenderTexture.active=rt;tex.ReadPixels(new Rect(0,0,512,512),0,0);tex.Apply();
            File.WriteAllBytes("Docs/Stage2/"+name+".png",tex.EncodeToPNG());return tex.GetPixels32();
        }
        finally
        {ps.gameObject.layer=oldLayer;RenderTexture.active=oldRt;cam.targetTexture=null;RenderTexture.ReleaseTemporary(rt);UnityEngine.Object.DestroyImmediate(tex);UnityEngine.Object.DestroyImmediate(go);}
    }
    static double Error(Color32[] a,Color32[] b)
    {
        long sum=0;for(int i=0;i<a.Length;i++)sum+=Math.Abs(a[i].r-b[i].r)+Math.Abs(a[i].g-b[i].g)+Math.Abs(a[i].b-b[i].b);
        return sum/(double)(a.Length*3);
    }
    public static async Task<string> Check()
    {
        var m=Module();var avatar=m.Avatar.transform;var root=avatar.Find("MorphForgeStage2");var ps=root.Find("Store").GetComponent<ParticleSystem>();
        var renderer=ps.GetComponent<ParticleSystemRenderer>();var material=renderer.sharedMaterial;
        var position=avatar.position;var rotation=avatar.rotation;var scale=avatar.localScale;
        var labels=new[]{"base","translated","yaw37","tilted","scale-half","scale18","combined"};
        var offsets=new[]{Vector3.zero,new Vector3(.137f,.219f,.071f),Vector3.zero,Vector3.zero,Vector3.zero,Vector3.zero,new Vector3(3.173f,2.219f,-1.071f)};
        var rotations=new[]{Quaternion.identity,Quaternion.identity,Quaternion.Euler(0,37,0),Quaternion.Euler(23,71,-17),Quaternion.identity,Quaternion.identity,Quaternion.Euler(-19,123,31)};
        var scales=new[]{1f,1f,1f,1f,.5f,1.8f,1.3f};
        var errors=new double[labels.Length];Color32[] baseline=null;Material legacy=null;double legacyError=0;
        try
        {
            for(int k=0;k<labels.Length;k++)
            {
                avatar.position=position+offsets[k];avatar.rotation=rotation*rotations[k];avatar.localScale=scale*scales[k];
                await Task.Delay(120);
                var image=Render(root,ps,"texture-"+labels[k]);
                if(k==0)baseline=image;errors[k]=Error(baseline,image);
            }
            legacy=new Material(Shader.Find("MorphForge/ParticleBox"));legacy.SetFloat("_CellSize",.25f);renderer.sharedMaterial=legacy;
            avatar.position=position;avatar.rotation=rotation;avatar.localScale=scale;await Task.Delay(120);
            var oldBase=Render(root,ps,"legacy-world-base");
            avatar.position=position+offsets[1];await Task.Delay(120);
            var oldShift=Render(root,ps,"legacy-world-translated");legacyError=Error(oldBase,oldShift);
            var report=new{success=errors.All(e=>e<.05)&&legacyError>.2,scope="Native OSC-born particles; co-moving camera, pose-only Editor stimuli",labels,
                meanAbsoluteErrors=errors,allowedMeanAbsoluteError=.05,legacyTranslationError=legacyError,
                invariant="UV repetition follows face UV and native birth size; no world-space position or normal chooses texture projection"};
            string json=Newtonsoft.Json.JsonConvert.SerializeObject(report,Newtonsoft.Json.Formatting.Indented);File.WriteAllText("Docs/Stage2/texture-validation.json",json);return json;
        }
        finally
        {
            renderer.sharedMaterial=material;avatar.position=position;avatar.rotation=rotation;avatar.localScale=scale;
            if(legacy)UnityEngine.Object.DestroyImmediate(legacy);
        }
    }
}
