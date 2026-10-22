using System;
using System.IO;
using System.Linq;
using System.Text;
using System.Security.Cryptography;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;
namespace MorphForgeAvatar.Editor {
 public static class CatalogIdentity {
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

 public static string Apply(GameObject module, JObject catalog) {
  var decoder = module.GetComponentsInChildren<Component>(true).Single(c => c && c.GetType().FullName == "nadena.dev.modular_avatar.core.ModularAvatarMergeAnimator" && c.gameObject.name == "MF6Decoder");
  var controller = (AnimatorController)new SerializedObject(decoder).FindProperty("animator").objectReferenceValue;
  string identity = Identity(catalog);
  var config = decoder.GetComponents<Component>().Single(c => c && c.GetType().FullName == "nadena.dev.modular_avatar.core.ModularAvatarParameters");
  var so = new SerializedObject(config);var list = so.FindProperty("parameters");
  void Set(string name, int value) {
   var p = controller.parameters.FirstOrDefault(p => p.name == name);
   if(p == null) { controller.AddParameter(new AnimatorControllerParameter { name=name, type=AnimatorControllerParameterType.Int, defaultInt=value }); }
   else { var all=controller.parameters;foreach(var item in all)if(item.name==name)item.defaultInt=value;controller.parameters=all; }
   SerializedProperty found=null;
   for(int i=0;i<list.arraySize;i++){var item=list.GetArrayElementAtIndex(i);if(item.FindPropertyRelative("nameOrPrefix").stringValue==name){found=item;break;}}
   if(found==null){int i=list.arraySize;list.InsertArrayElementAtIndex(i);found=list.GetArrayElementAtIndex(i);found.FindPropertyRelative("nameOrPrefix").stringValue=name;found.FindPropertyRelative("syncType").enumValueIndex=1;}
   found.FindPropertyRelative("localOnly").boolValue=true;found.FindPropertyRelative("saved").boolValue=false;
   found.FindPropertyRelative("hasExplicitDefaultValue").boolValue=true;found.FindPropertyRelative("defaultValue").floatValue=value;
   found.FindPropertyRelative("internalParameter").boolValue=false;
  }
  Set("MF_InfoVersion",7);Set("MF_InfoFeatures",(((JArray)catalog["capabilities"]).Any(v=>(string)v=="mf6-motion4/1")?64:0)|32|16|1|2|4|(((bool?)catalog["textureOptions"]?["layeredOnly"]??false)?8:0));
  int build=(int)(DateTime.UtcNow.Ticks%16777215);for(int i=0;i<3;i++)Set("MF_InfoBuild"+i,(build>>(8*i))&255);
  for(int i=0;i<32;i++)Set("MF_Catalog"+i.ToString("D2"),Convert.ToByte(identity.Substring(i*2,2),16));
  Set(PluginBudgetGate.Marker,1);so.ApplyModifiedPropertiesWithoutUndo();EditorUtility.SetDirty(controller);
  return identity;
 }
 }
}
