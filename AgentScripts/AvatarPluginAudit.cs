using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEngine;
using Newtonsoft.Json.Linq;
using MorphForge.Editor;
using VRC.SDK3.Avatars.Components;
public static class AvatarPluginAudit {
 public static string Describe(){
  var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();var avatars=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).ToArray();
  var module=avatars.Length==1?avatars[0].transform.Find(PlacementStage4Build.ModuleName):null;
  var renderers=module?module.GetComponentsInChildren<ParticleSystemRenderer>(true):Array.Empty<ParticleSystemRenderer>();
  var data=new JObject{["scene"]=scene.path,["dirty"]=scene.isDirty,["avatars"]=avatars.Length,["colorSpace"]=PlayerSettings.colorSpace.ToString(),["renderers"]=new JArray(renderers.Select(r=>new JObject{["name"]=r.name,["mesh"]=r.mesh?AssetDatabase.GetAssetPath(r.mesh):null,["subMeshes"]=r.mesh?r.mesh.subMeshCount:0,["sort"]=r.sortMode.ToString(),["materialCount"]=r.sharedMaterials.Length,["materials"]=new JArray(r.sharedMaterials.Select(m=>m?new JObject{["path"]=AssetDatabase.GetAssetPath(m),["shader"]=m.shader.name,["queue"]=m.renderQueue,["passes"]=m.passCount}:null))}))};
  Directory.CreateDirectory("Docs/AvatarPlugin");File.WriteAllText("Docs/AvatarPlugin/audit.json",data.ToString());return data.ToString();
 }
}
