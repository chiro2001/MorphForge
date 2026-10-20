using System;
using System.IO;
using System.Linq;
using System.Text;
using System.Security.Cryptography;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;
using nadena.dev.modular_avatar.core;
using VRC.SDK3.Avatars.Components;

namespace MorphForge.Editor {
    // Local defaults only: no synced bits and no per-frame behaviours/layers.
    public static class AvatarCapabilitiesBuild {
        static void Text(BinaryWriter w,string s){var b=Encoding.UTF8.GetBytes(s??"");w.Write((uint)b.Length);w.Write(b);}
        public static string Identity(JObject catalog){
            using(var stream=new MemoryStream())using(var w=new BinaryWriter(stream)){
                w.Write(Encoding.UTF8.GetBytes("MorphForgeCatalogIdentity/1"));
                w.Write((uint?)catalog["textureOptions"]?["cellPixels"]??32);w.Write((byte)(((bool?)catalog["textureOptions"]?["layeredOnly"]??false)?1:0));
                var materials=((JArray)catalog["materials"]).OfType<JObject>().OrderBy(m=>(uint)m["id"]).ToArray();w.Write((uint)materials.Length);
                foreach(var m in materials){
                    w.Write((uint)m["id"]);Text(w,(string)m["uid"]);Text(w,(string)m["family"]);
                    foreach(var c in (JArray)m["color"])w.Write((float)c);
                    foreach(string field in new[]{"metallic","roughness","opacity","emission","textureScale","frameTime"})w.Write((float? )m[field]??(field=="textureScale"?1f:0f));
                    w.Write((byte)((int?)m["textureRotation"]??0));w.Write((uint?)m["frames"]??0);Text(w,(string)m["renderMode"]);
                    var textures=(m["textures"] as JArray)??new JArray();w.Write((uint)textures.Count);foreach(var t in textures)Text(w,(string)t);
                    var p=m["procedural"] as JObject;w.Write((byte)(p!=null?1:0));if(p!=null){Text(w,(string)p["kind"]);w.Write((float)p["scale"]);w.Write((uint)p["seed"]);}
                }
                w.Flush();using(var hash=SHA256.Create())return BitConverter.ToString(hash.ComputeHash(stream.ToArray())).Replace("-","").ToLowerInvariant();
            }
        }
        public static string Apply(GameObject avatar,JObject catalog){
            var decoder=avatar.GetComponentsInChildren<ModularAvatarMergeAnimator>(true).Single(m=>m.animator&&AssetDatabase.GetAssetPath(m.animator)==Mf6NativeBuild.Root+"/Decoder.controller");
            var fx=(AnimatorController)decoder.animator;
            var parameters=decoder.GetComponent<ModularAvatarParameters>();
            foreach(var p in fx.parameters.Where(p=>p.name.StartsWith("MF_Info")||p.name.StartsWith("MF_Catalog")).ToArray())fx.RemoveParameter(fx.parameters.ToList().FindIndex(x=>x.name==p.name));
            parameters.parameters.RemoveAll(p=>p.nameOrPrefix.StartsWith("MF_Info")||p.nameOrPrefix.StartsWith("MF_Catalog"));
            Action<string,int> add=(name,value)=>{
                fx.AddParameter(new AnimatorControllerParameter{name=name,type=AnimatorControllerParameterType.Int,defaultInt=value});
                parameters.parameters.Add(new ParameterConfig{nameOrPrefix=name,syncType=ParameterSyncType.Int,localOnly=true,saved=false,hasExplicitDefaultValue=true,defaultValue=value,internalParameter=false});
            };
            var identity=Identity(catalog);add("MF_InfoVersion",7);add("MF_InfoFeatures",1|(((JArray)catalog["capabilities"]).Any(v=>(string)v=="transparent-face-mask/1")?2:0)|(((JArray)catalog["capabilities"]).Any(v=>(string)v=="virtual-light/1")?4:0)|(((bool?)catalog["textureOptions"]?["layeredOnly"]??false)?8:0));int build=(int)(DateTime.UtcNow.Ticks%16777215);for(int i=0;i<3;i++)add("MF_InfoBuild"+i,(build>>(8*i))&255);
            var bytes=Enumerable.Range(0,32).Select(i=>Convert.ToByte(identity.Substring(i*2,2),16)).Concat(new byte[]{0}).ToArray();
            for(int i=0;i<32;i++)add("MF_Catalog"+i.ToString("D2"),bytes[i]);
            EditorUtility.SetDirty(fx);EditorUtility.SetDirty(parameters);
            return identity;
        }
    }
}
