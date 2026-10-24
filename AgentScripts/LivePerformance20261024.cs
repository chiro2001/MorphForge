using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.Core;
using VRC.SDKBase.Editor.Api;
using VRC.SDK3.Avatars.Components;
using nadena.dev.modular_avatar.core;
using MorphForge.Editor;
using System.Collections.Generic;

// Reuse the SDK's existing authenticated transport; never export credentials.
public static class LivePerformance20261024 {
 const string Reports="Docs/LivePerformance20261024";
 public static async Task<string> Snapshot(){
  if(Application.dataPath.Replace('\\','/')!="D:/Projects/VRChatProjects2/MorphForge/Assets")throw new Exception("Wrong project");
  var user=await VRCApi.Get<JObject>("auth/user",forceRefresh:true);
  var scenes=new JArray(Enumerable.Range(0,SceneManager.sceneCount).Select(SceneManager.GetSceneAt).Select(s=>(JToken)new JObject{["path"]=s.path,["dirty"]=s.isDirty}));
  var report=new JObject{["success"]=true,["currentAvatar"]=(string)user["currentAvatar"],["sdkIdentityMatches"]=APIUser.CurrentUser!=null&&(string)user["id"]==APIUser.CurrentUser.id,["scenes"]=scenes,["playing"]=EditorApplication.isPlaying,["utc"]=DateTime.UtcNow.ToString("O")};
  Directory.CreateDirectory(Reports);if(!File.Exists(Reports+"/initial-sdk.json"))File.WriteAllText(Reports+"/initial-sdk.json",report.ToString());return report.ToString();
 }
 public static async Task<string> OwnAvatars(){
  var avatars=await VRCApi.Get<JArray>("avatars",new Dictionary<string,string>{{"user","me"},{"releaseStatus","all"},{"n","100"}},forceRefresh:true);
  var user=await VRCApi.Get<JObject>("auth/user",forceRefresh:true);
  var rows=new JArray(avatars.Where(a=>(string)a["authorId"]==(string)user["id"]).Select(a=>(JToken)new JObject{["id"]=a["id"],["name"]=a["name"],["version"]=a["version"],["releaseStatus"]=a["releaseStatus"]}));
  File.WriteAllText(Reports+"/own-avatars.json",rows.ToString());return rows.ToString();
 }
 public static async Task<string> Select(string id){
  if(!File.Exists(Reports+"/initial-sdk.json"))throw new Exception("Initial state required");
  var allowed=JArray.Parse(File.ReadAllText(Reports+"/own-avatars.json"));
  if(!allowed.Any(a=>(string)a["id"]==id))throw new Exception("Only existing avatars owned by the logged in SDK user may be selected");
  var selected=await VRCApi.Put<JObject>("avatars/"+id+"/select",forceRefresh:true);
  if((string)selected["currentAvatar"]!=id)throw new Exception("API did not confirm requested selection");
  return new JObject{["success"]=true,["currentAvatar"]=id,["utc"]=DateTime.UtcNow.ToString("O")}.ToString();
 }
 public static string InspectSource(){
  var scene=SceneManager.GetActiveScene();
  if(EditorApplication.isPlaying||scene.path!=Mf6AvatarBuild.ScenePath||scene.isDirty)throw new Exception("Clean MF6 source required");
  var avatar=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single();
  JArray V(Vector3 v)=>new JArray(v.x,v.y,v.z);
  var camera=avatar.GetComponentsInChildren<Camera>(true).Single(c=>c.targetTexture);
  var menus=new JArray(avatar.GetComponentsInChildren<ModularAvatarMenuInstaller>(true).Where(m=>m.menuToAppend).Select(m=>(JToken)new JObject{["object"]=m.name,["path"]=AssetDatabase.GetAssetPath(m.menuToAppend),["target"]=AssetDatabase.GetAssetPath(m.installTargetMenu),["controls"]=new JArray(m.menuToAppend.controls.Select(c=>(JToken)new JObject{["name"]=c.name,["subMenu"]=AssetDatabase.GetAssetPath(c.subMenu),["subControls"]=c.subMenu?new JArray(c.subMenu.controls.Select(x=>x.name)):null}))}));
  var result=new JObject{["scene"]=scene.path,["cameraLocalPosition"]=V(camera.transform.localPosition),["cameraForward"]=V(camera.transform.forward),["menus"]=menus,["characterPrefabs"]=new JArray(AssetDatabase.GetDependencies(Mf6AvatarBuild.AvatarPath,true).Where(p=>p.StartsWith("Assets/Nemesis/")&&p.EndsWith(".prefab")).Select(p=>(JToken)new JObject{["path"]=p,["hash"]=AssetDatabase.GetAssetDependencyHash(p).ToString()}))};
  Directory.CreateDirectory(Reports);File.WriteAllText(Reports+"/source-inspection.json",result.ToString());return result.ToString();
 }
}
