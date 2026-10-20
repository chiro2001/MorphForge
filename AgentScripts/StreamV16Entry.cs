using System;using System.IO;using System.Linq;using UnityEditor;using UnityEngine;using Newtonsoft.Json.Linq;using MorphForge.Editor;
public static class StreamV16Entry {
 public static string SchedulePrepare(){
  Directory.CreateDirectory("Docs/StreamV16");File.WriteAllText("Docs/StreamV16/build-status.json",new JObject{["status"]="scheduled"}.ToString());
  EditorApplication.CallbackFunction callback=null;callback=()=>{EditorApplication.update-=callback;try{File.WriteAllText("Docs/StreamV16/build-status.json",new JObject{["status"]="running"}.ToString());var value=Prepare();File.WriteAllText("Docs/StreamV16/build-status.json",new JObject{["status"]="complete",["result"]=JObject.Parse(value)}.ToString());}catch(Exception e){File.WriteAllText("Docs/StreamV16/build-status.json",new JObject{["status"]="failed",["error"]=e.ToString()}.ToString());Debug.LogException(e);}};
  EditorApplication.update+=callback;return "Prepare scheduled; poll Docs/StreamV16/build-status.json";
 }
 public static async System.Threading.Tasks.Task<string> ProveDefaults(){
  for(int i=0;i<300;i++){var ready=UnityEngine.Object.FindObjectsOfType<BlackStartX.GestureManager.GestureManager>().FirstOrDefault(g=>g.gameObject.scene.path==Mf6AvatarBuild.ScenePath);if(ready&&ready.Module is BlackStartX.GestureManager.Editor.Modules.Vrc3.ModuleVrc3)break;await System.Threading.Tasks.Task.Delay(100);}
  await System.Threading.Tasks.Task.Delay(300);
  var gm=UnityEngine.Object.FindObjectsOfType<BlackStartX.GestureManager.GestureManager>().Single(g=>g.gameObject.scene.path==Mf6AvatarBuild.ScenePath);
  var module=(BlackStartX.GestureManager.Editor.Modules.Vrc3.ModuleVrc3)gm.Module;
  var bytes=Enumerable.Range(0,32).Select(i=>(byte)module.GetParam("MF_Catalog"+i.ToString("D2")).IntValue()).ToArray();var hash=BitConverter.ToString(bytes).Replace("-","").ToLowerInvariant();
  var expected=Identity();var result=new JObject{["success"]=hash==expected&&module.GetParam("MF_InfoVersion").IntValue()==7,["actualHash"]=hash,["expectedHash"]=expected,["features"]=module.GetParam("MF_InfoFeatures").IntValue(),["scope"]="Actual GestureManager parameters from the preprocessed Avatar; local defaults, no OSC execution ACK."};
  File.WriteAllText("Docs/StreamV16/capability-defaults.json",result.ToString());return result.ToString();
 }
 public static string Prepare(){
  var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
  if(scene.path==Mf6AvatarBuild.ScenePath&&scene.isDirty){
   if(scene.GetRootGameObjects().Sum(g=>g.GetComponentsInChildren<VRC.SDK3.Avatars.Components.VRCAvatarDescriptor>(true).Length)!=1)throw new Exception("Expected one Avatar in this task's generated scene");
   // This task opened the scene clean, generated it and ran Play Mode. Preserve
   // its current state as a separate Unity snapshot before the owned rebuild.
   UnityEditor.SceneManagement.EditorSceneManager.SaveScene(scene,"Assets/MorphForge/MF6/BeforeStreamV16.unity",true);
   UnityEditor.SceneManagement.EditorSceneManager.SaveScene(scene);
  }

  var result=JObject.Parse(Mf6AvatarBuild.Prepare("Docs/MF6/MinecraftSource"));
  var catalog=JObject.Parse(File.ReadAllText("Docs/MF6/avatar-catalog.json"));
  var budget=JObject.Parse(File.ReadAllText("Docs/MF6/final-budget.json"));
  var material=AssetDatabase.LoadAssetAtPath<Material>("Assets/MorphForge/Diagnostics/MF6Catalog/ParticleBoxCatalog.mat");
  var metrics=new JObject{["success"]=true,["styles"]=((JArray)catalog["materials"]).Count,["identityHash"]=AvatarCapabilitiesBuild.Identity(catalog),["catalogIdentityMatches"]=(string)catalog["identityHash"]==AvatarCapabilitiesBuild.Identity(catalog),["syncedBits"]=budget["totalBits"],["layeredOnly"]=catalog["textureOptions"]?["layeredOnly"],["cellPixels"]=material.GetFloat("_CellPixels"),["usesFaceMap"]=material.GetFloat("_UseFaceMap"),["avatarId"]=Mf6AvatarBuild.AvatarId};
  var meta=(Texture2D)material.GetTexture("_CatalogMeta");bool masks=((JArray)catalog["capabilities"]).Any(c=>(string)c=="transparent-face-mask/1");
  bool maskConsistent=((JArray)catalog["materials"]).OfType<JObject>().Where(m=>(string)m["renderMode"]=="transparent").All(m=>{int texel=(int)m["id"]*4+2;float flags=meta.GetPixel(texel%meta.width,texel/meta.width).a;return (((int)flags&32)!=0)==masks;});metrics["surfaceMasksConsistent"]=maskConsistent;
  if(!maskConsistent)throw new Exception("Catalog advertises transparent masks that were not baked");
  if(!(bool)metrics["catalogIdentityMatches"]||(int)metrics["syncedBits"]!=254)throw new Exception("Identity/budget gate failed");
  Directory.CreateDirectory("Docs/StreamV16");File.WriteAllText("Docs/StreamV16/prepared.json",result.ToString());File.WriteAllText("Docs/StreamV16/catalog.json",catalog.ToString());File.WriteAllText("Docs/StreamV16/metrics.json",metrics.ToString());return metrics.ToString();
 }
 public static string Identity()=>AvatarCapabilitiesBuild.Identity(JObject.Parse(File.ReadAllText("Docs/MF6/avatar-catalog.json")));
 public static string Status()=>File.Exists("Docs/MF6/unified-progress.json")?File.ReadAllText("Docs/MF6/unified-progress.json"):"No smoke status";
}
