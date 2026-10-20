using System;
using System.IO;
using System.Linq;
using UnityEngine;
using UnityEditor;
using Newtonsoft.Json.Linq;
public static class Mf5GpuProbe
{
    static Color Read(Material m,RenderTexture rt,int output){m.SetFloat("_ProbeOut",output);Graphics.Blit(Texture2D.blackTexture,rt,m);var prev=RenderTexture.active;RenderTexture.active=rt;var t=new Texture2D(1,1,TextureFormat.RGBAFloat,false,true);t.ReadPixels(new Rect(0,0,1,1),0,0);t.Apply();var c=t.GetPixel(0,0);UnityEngine.Object.DestroyImmediate(t);RenderTexture.active=prev;return c;}
    static float[] Decode(int mode,int v,int slot)
    {
        Func<int,int,float> f=(shift,mask)=>(v>>shift)&mask;
        if(slot==3) {int style=v&4095;return new[]{(float)(style&63),(float)(style>>6),mode==7?0:f(12,63),mode==7?0:f(18,63)};}
        if(slot==2)return new[]{mode>=3&&mode<=6?mode-2:0,mode>=3&&mode<=6?.25f*Mathf.Pow(2,f(18,7)*5/7):1,mode>=3&&mode<=6?f(21,7):0,(float)mode};
        float metal=0,rough=.9f,alpha=1,emit=0;float[] color;
        if(mode==1)color=new[]{f(0,255)/255,f(8,255)/255,f(16,255)/255};
        else if(mode==2){color=new[]{f(0,31)/31,f(5,63)/63,f(11,31)/31};metal=f(16,3)/3;rough=f(18,3)/3;alpha=1-f(20,3)/3;emit=f(22,3);}
        else if(mode==7)color=new[]{f(12,15)/15,f(16,15)/15,f(20,15)/15};
        else {color=new[]{f(0,15)/15,f(4,15)/15,f(8,15)/15};metal=mode==3?.2f:0;rough=f(12,3)/3;alpha=1-f(14,3)/3;emit=f(16,3);}
        return slot==0?new[]{color[0],color[1],color[2],alpha}:new[]{metal,rough,emit,alpha<.999f?2f:0};
    }
    public static string Run()
    {
        if(EditorApplication.isPlaying)throw new Exception("Edit Mode required.");
        foreach(var path in new[]{"Assets/MorphForge/Shaders/ParticleBoxCatalog.shader","Assets/MorphForge/Shaders/ParticleBoxCatalogMetadataGrid.shader","Assets/MorphForge/Shaders/Mf5WireProbe.shader"})AssetDatabase.ImportAsset(path,ImportAssetOptions.ForceUpdate);
        var shader=Shader.Find("Hidden/MorphForge/MF5WireProbe");var errors=new[]{shader,Shader.Find("MorphForge/ParticleBoxCatalog"),Shader.Find("MorphForge/ParticleBoxCatalogMetadataGrid")}.SelectMany(s=>ShaderUtil.GetShaderMessages(s).Where(m=>m.severity==UnityEditor.Rendering.ShaderCompilerMessageSeverity.Error).Select(m=>m.message)).ToArray();if(errors.Length>0)throw new Exception(string.Join("\n",errors));
        var material=new Material(shader);var rt=new RenderTexture(1,1,0,RenderTextureFormat.ARGBFloat,RenderTextureReadWrite.Linear);rt.Create();int cases=0;float worst=0;var failures=new JArray();
        try {
            uint state=17;
            for(int mode=1;mode<=7;mode++)for(int j=0;j<24;j++){
                state=state*1664525+1013904223;int bits=j==0?0:j==1?0xFFFFFF:(int)(state&0xFFFFFF);
                material.SetVector("_ProbeData",new Vector4((bits&255)/255f,((bits>>8)&255)/255f,((bits>>16)&255)/255f,(255-mode)/255f));
                for(int slot=0;slot<3;slot++){
                    var actual=Read(material,rt,slot);var expected=Decode(mode,bits,slot);var values=new[]{actual.r,actual.g,actual.b,actual.a};float error=values.Zip(expected,(a,b)=>Mathf.Abs(a-b)).Max();worst=Mathf.Max(worst,error);cases++;if(error>.00003f)failures.Add(new JObject{["mode"]=mode,["bits"]=bits,["slot"]=slot,["actual"]=JArray.FromObject(values),["expected"]=JArray.FromObject(expected)});
                }
            }
            var report=new JObject{["success"]=failures.Count==0,["cases"]=cases,["worstAbsoluteError"]=worst,["failures"]=failures,["scope"]="Actual GPU float render/readback, independent CPU integer bitfield oracle. All runtime material modes, color/PBR/noise fields and alpha mode tags."};Directory.CreateDirectory("Docs/MF5");File.WriteAllText("Docs/MF5/gpu-probe.json",report.ToString());if(failures.Count>0)throw new Exception(report.ToString());return report.ToString();
        }finally{rt.Release();UnityEngine.Object.DestroyImmediate(rt);UnityEngine.Object.DestroyImmediate(material);}
    }
}
