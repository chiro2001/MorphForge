using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;
using MorphForgeAvatar;
using MorphForgeAvatar.Editor;
using Newtonsoft.Json.Linq;
public static class PluginCatalogCheck {
 public static string Run(){
  if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");
  string zip="E:/tmp/morphforge/avatar-plugin/reports/exported-catalog.zip";Directory.CreateDirectory(Path.GetDirectoryName(zip));CatalogVariant.ExportBuiltin(zip);
  var source=JObject.Parse(File.ReadAllText(AvatarSetupWindow.Root+"/CatalogSource/catalog.json"));
  string sourceRoot="E:/tmp/morphforge/avatar-plugin/variant-source";Directory.CreateDirectory(sourceRoot+"/artifacts");
  foreach(var file in Directory.GetFiles(AvatarSetupWindow.Root+"/CatalogSource/artifacts","*.png"))File.Copy(file,sourceRoot+"/artifacts/"+Path.GetFileName(file),true);
  source["materials"][0]["opacity"]=.37;source["materials"][0]["renderMode"]="transparent";
  File.WriteAllText(sourceRoot+"/catalog.json",source.ToString());
  string original=AssetDatabase.LoadAssetAtPath<GameObject>(AvatarSetupWindow.PrefabPath).GetComponent<MorphForgeAvatarModule>().catalogIdentity;
  string path=CatalogVariant.Build(sourceRoot);
  var prefab=AssetDatabase.LoadAssetAtPath<GameObject>(path);
  var marker=prefab.GetComponent<MorphForgeAvatarModule>();var stores=prefab.GetComponentsInChildren<ParticleSystemRenderer>(true).Where(r=>r.name=="Store").ToArray();
  var decoder=prefab.GetComponentsInChildren<Component>(true).Single(c=>c&&c.GetType().FullName=="nadena.dev.modular_avatar.core.ModularAvatarMergeAnimator"&&c.name=="MF6Decoder");
  var controller=(AnimatorController)new SerializedObject(decoder).FindProperty("animator").objectReferenceValue;
  var hash=string.Concat(Enumerable.Range(0,32).Select(i=>controller.parameters.Single(p=>p.name=="MF_Catalog"+i.ToString("D2")).defaultInt.ToString("x2")));
  var metadata=(Texture2D)stores[0].sharedMaterial.GetTexture("_CatalogMeta");int pixel=(int)source["materials"][0]["id"]*4+1;
  float opacity=metadata.GetPixel(pixel%metadata.width,pixel/metadata.width).a;
  bool baseUnchanged=AssetDatabase.LoadAssetAtPath<GameObject>(AvatarSetupWindow.PrefabPath).GetComponent<MorphForgeAvatarModule>().catalogIdentity==original;
  bool valid=baseUnchanged&&hash!=original&&Math.Abs(opacity-.37)<.0001&&stores.Length==8&&marker.catalogIdentity==CatalogIdentity.Identity(source)&&marker.catalogIdentity==hash&&stores.All(r=>AssetDatabase.GetAssetPath(r.sharedMaterial).StartsWith(Path.GetDirectoryName(path).Replace('\\','/'),StringComparison.Ordinal))&&new FileInfo(zip).Length>1000;
  int missing=prefab.GetComponentsInChildren<Transform>(true).Sum(t=>GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(t.gameObject));valid&=missing==0;
  var report=new JObject{["success"]=valid,["variant"]=path,["catalogIdentity"]=hash,["stores"]=stores.Length,["changedOpacity"]=opacity,["baseUnchanged"]=baseUnchanged,["missingScripts"]=missing,["exportBytes"]=new FileInfo(zip).Length};
  File.WriteAllText("E:/tmp/morphforge/avatar-plugin/reports/catalog-variant.json",report.ToString());return report.ToString();
 }
}
