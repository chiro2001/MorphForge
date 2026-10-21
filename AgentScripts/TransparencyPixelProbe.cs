using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using Newtonsoft.Json.Linq;
public static class TransparencyPixelProbe {
 public static string Run(){
  if(EditorApplication.isPlaying)throw new Exception("Idle Edit Mode required");
  var scene=EditorSceneManager.NewPreviewScene();var original=AssetDatabase.LoadAssetAtPath<Material>("Assets/MorphForge/MF6/Pool0/Surface.mat");
  var material=new Material(original);material.SetFloat("_WireEncoding",5);material.SetFloat("_UseLayeredTextures",0);material.SetFloat("_VoxelGrid",0);
  var meta=new Texture2D(8,1,TextureFormat.RGBAFloat,false,true){filterMode=FilterMode.Point};material.SetTexture("_CatalogMeta",meta);material.SetFloat("_MetaWidth",8);material.SetFloat("_MetaHeight",1);
  var target=new RenderTexture(256,256,24,RenderTextureFormat.ARGBFloat,RenderTextureReadWrite.Linear);target.Create();Mesh mesh=null;var results=new JArray();
  try{
   var cube=GameObject.CreatePrimitive(PrimitiveType.Cube);UnityEngine.SceneManagement.SceneManager.MoveGameObjectToScene(cube,scene);mesh=UnityEngine.Object.Instantiate(cube.GetComponent<MeshFilter>().sharedMesh);
   var vertices=mesh.vertices;var normals=mesh.normals;var uv=new List<Vector4>();for(int i=0;i<vertices.Length;i++){var v=vertices[i];var n=normals[i];int axis=Mathf.Abs(n.x)>.5?0:Mathf.Abs(n.y)>.5?1:2;uv.Add(new Vector4(axis==0?v.z+.5f:v.x+.5f,axis==1?v.z+.5f:v.y+.5f,axis,0));}mesh.SetUVs(0,uv);mesh.SetUVs(1,Enumerable.Repeat(Vector3.one,mesh.vertexCount).ToList());mesh.colors32=Enumerable.Repeat(new Color32(1,0,0,255),mesh.vertexCount).ToArray();cube.GetComponent<MeshFilter>().sharedMesh=mesh;cube.GetComponent<MeshRenderer>().sharedMaterial=material;
   var camera=new GameObject("Alpha parity probe camera").AddComponent<Camera>();UnityEngine.SceneManagement.SceneManager.MoveGameObjectToScene(camera.gameObject,scene);camera.scene=scene;camera.cameraType=CameraType.Preview;camera.enabled=false;camera.allowHDR=false;camera.targetTexture=target;camera.orthographic=true;camera.orthographicSize=.8f;camera.transform.position=new Vector3(0,0,3);camera.transform.LookAt(Vector3.zero);camera.nearClipPlane=.01f;camera.farClipPlane=20;camera.clearFlags=CameraClearFlags.SolidColor;
   foreach(float opacity in new[]{0f,.1f,1f/3,.5f,2f/3,.9f,1f}){
    meta.SetPixel(4,0,new Color(0,0,1,.1f));meta.SetPixel(5,0,new Color(.5f,.5f,.5f,opacity));meta.SetPixel(6,0,new Color(0,1,0,opacity<1?2:0));meta.SetPixel(7,0,new Color(0,0,2,0));meta.Apply(false,false);
    var samples=new JArray();foreach(float background in new[]{0f,1f}){
     camera.backgroundColor=new Color(background,background,background,1);camera.Render();var prev=RenderTexture.active;RenderTexture.active=target;var read=new Texture2D(1,1,TextureFormat.RGBAFloat,false,true);read.ReadPixels(new Rect(128,128,1,1),0,0);read.Apply();RenderTexture.active=prev;var color=read.GetPixel(0,0);samples.Add(new JArray(color.r,color.g,color.b,color.a));UnityEngine.Object.DestroyImmediate(read);
    }
    results.Add(new JObject{["opacity"]=opacity,["samplesLinear"]=samples,["measuredAlpha"]=1f-((float)samples[1][0]-(float)samples[0][0])});
   }
   var report=new JObject{["colorSpace"]=PlayerSettings.colorSpace.ToString(),["results"]=results,["scope"]="Production material and Shader, actual GPU, diagnostic standard cube and precise catalog alpha. Two backgrounds isolate the blend factor. Not a native particle execution test."};Directory.CreateDirectory("Docs/AvatarPlugin");File.WriteAllText("Docs/AvatarPlugin/alpha-baseline.json",report.ToString());return report.ToString();
  }finally{if(mesh)UnityEngine.Object.DestroyImmediate(mesh);UnityEngine.Object.DestroyImmediate(material);UnityEngine.Object.DestroyImmediate(meta);target.Release();UnityEngine.Object.DestroyImmediate(target);EditorSceneManager.ClosePreviewScene(scene);}
 }
}
