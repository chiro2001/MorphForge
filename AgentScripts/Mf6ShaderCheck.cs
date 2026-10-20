using System;using System.Linq;using System.IO;using UnityEditor;using UnityEngine;using Newtonsoft.Json.Linq;
public static class Mf6ShaderCheck{
 public static string Run(){
  var shader=Shader.Find("MorphForge/ParticleBoxCatalogMetadataGrid");
  var messages=new JArray(ShaderUtil.GetShaderMessages(shader).Select(m=>new JObject{["severity"]=m.severity.ToString(),["message"]=m.message,["line"]=m.line,["platform"]=m.platform.ToString()}));
  var systems=UnityEngine.Object.FindObjectsOfType<ParticleSystem>(true).Where(p=>p.name=="Store").Take(8).ToArray();
  var result=new JObject{["success"]=messages.All(m=>(string)m["severity"]!="Error"),["shader"]=shader.name,["supported"]=shader.isSupported,["messages"]=messages,["stores"]=new JArray(systems.Select(p=>new JObject{["material"]=p.GetComponent<ParticleSystemRenderer>().sharedMaterial.name,["shader"]=p.GetComponent<ParticleSystemRenderer>().sharedMaterial.shader.name,["count"]=p.particleCount,["wire"]=p.GetComponent<ParticleSystemRenderer>().sharedMaterial.GetFloat("_WireEncoding")}))};
  File.WriteAllText("Docs/MF6/shader-check.json",result.ToString());return result.ToString();
 }
}
