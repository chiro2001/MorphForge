using System;
using System.Linq;
using System.Collections.Generic;
using UnityEngine;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;

public static class ParticleStage1VisualProbe
{
    public static string Inspect()
    {
        var g=UnityEngine.Object.FindObjectsOfType<GestureManager>().First(x=>x.name=="MorphForge Stage1 Gesture Manager");
        var m=g.Module as ModuleVrc3;
        var ps=m.Avatar.transform.Find("MorphForgeStage1/Store").GetComponent<ParticleSystem>();
        var r=ps.GetComponent<ParticleSystemRenderer>();
        var streams=new List<ParticleSystemVertexStream>();r.GetActiveVertexStreams(streams);
        var baked=new Mesh();
        try
        {
            r.BakeMesh(baked,Camera.main,true);
            return Newtonsoft.Json.JsonConvert.SerializeObject(new {alignment=r.alignment.ToString(),renderMode=r.renderMode.ToString(),mesh=r.mesh.name,
                shader=r.sharedMaterial.shader.name,material=r.sharedMaterial.name,streams=streams.Select(x=>x.ToString()).ToArray(),
                bakedVertices=baked.vertices.Take(30).Select(v=>new[]{v.x,v.y,v.z}).ToArray(),
                bakedColors=baked.colors32.Take(30).Select(c=>new[]{(int)c.r,(int)c.g,(int)c.b,(int)c.a}).ToArray()},Newtonsoft.Json.Formatting.Indented);
        }
        finally{UnityEngine.Object.DestroyImmediate(baked);}
    }
}
