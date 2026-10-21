using System;
using System.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.SDK3.Avatars.Components;
public static class AvatarPluginCleanup {
 public static string Inspect(){return string.Join("\n",SceneManager.GetActiveScene().GetRootGameObjects().Select(g=>g.name+" / "+g.hideFlags+" / children="+g.transform.childCount));}
 public static string Restore(){
  if(EditorApplication.isPlaying)throw new Exception("Edit Mode required");
  var scene=SceneManager.GetActiveScene();
  if(scene.path!="Assets/MorphForge/Diagnostics/AvatarPlugin/Transparency.unity")throw new Exception("Unexpected scene; preserved");
  if(scene.GetRootGameObjects().Any(g=>g.name!="nadena.dev.ndmf__Activator"||(g.hideFlags&HideFlags.HideInHierarchy)==0))throw new Exception("Unexpected objects in diagnostic scene; preserved");
  var restored=EditorSceneManager.OpenScene("Assets/MorphForge/MF6/MorphForge_MF6.unity",OpenSceneMode.Single);
  int count=restored.GetRootGameObjects().Sum(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true).Length);
  return "Restored "+restored.path+"; avatars="+count+"; dirty="+restored.isDirty;
 }
 public static string CloseClean(){
  if(!Application.dataPath.Replace('\\','/').EndsWith("/avatar-plugin/CleanProject/Assets",StringComparison.OrdinalIgnoreCase))throw new Exception("Refusing to close any other project");
  EditorApplication.delayCall+=()=>EditorApplication.Exit(0);return "Closing isolated acceptance Editor";
 }
}
