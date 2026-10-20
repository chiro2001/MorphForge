using System;using System.IO;using System.Linq;using System.Threading.Tasks;using UnityEngine;using UnityEditor;using Newtonsoft.Json.Linq;
using BlackStartX.GestureManager;using BlackStartX.GestureManager.Editor.Modules.Vrc3;using MorphForge.Editor;
public static class Mf6CatalogVisual {
 static readonly Vector3[] Faces={Vector3.right,Vector3.left,Vector3.up,Vector3.down,Vector3.forward,Vector3.back};
 static readonly Color[] Colors={Color.red,Color.cyan,Color.green,Color.magenta,Color.blue,Color.yellow};
 static Quaternion Rotation(int code){var basis=new[]{Quaternion.identity,Quaternion.Euler(0,0,90),Quaternion.Euler(90,0,0),Quaternion.Euler(-90,0,0),Quaternion.Euler(0,0,180),Quaternion.Euler(0,0,-90)};return Quaternion.Euler(0,-90*(code%4),0)*basis[code/4];}
 static int Face(Vector3 n)=>Mathf.Abs(n.x)>.5f?(n.x>0?0:1):Mathf.Abs(n.y)>.5f?(n.y>0?2:3):(n.z>0?4:5);
 static void Metadata(Texture2D meta,int rotation,float cells){meta.SetPixel(4,0,new Color(0,0,1,.1f));meta.SetPixel(5,0,Color.white);meta.SetPixel(6,0,new Color(0,1,1,65));meta.SetPixel(7,0,new Color(0,0,cells+1,rotation));meta.Apply(false,false);}
 static Texture2DArray Texture(bool stripes){var t=new Texture2DArray(16,16,6,TextureFormat.RGBA32,false,false){filterMode=FilterMode.Point,wrapMode=TextureWrapMode.Repeat};for(int face=0;face<6;face++){var colors=new Color[256];for(int y=0;y<16;y++)for(int x=0;x<16;x++)colors[y*16+x]=stripes?(x<8?Color.white:Color.black):Colors[face];t.SetPixels(colors,face);}t.Apply(false,false);return t;}
 static Texture2D Render(Camera camera,RenderTexture target){camera.Render();var active=RenderTexture.active;RenderTexture.active=target;var image=new Texture2D(target.width,target.height,TextureFormat.RGBA32,false);image.ReadPixels(new Rect(0,0,target.width,target.height),0,0);image.Apply();RenderTexture.active=active;return image;}
 static void View(Camera camera,ParticleSystem store,Vector3 normal){var center=store.transform.TransformPoint(Vector3.zero);float size=2*Mathf.Abs(store.transform.lossyScale.x);camera.orthographicSize=size*.6f;camera.transform.position=center+store.transform.TransformDirection(normal)*size*3;camera.transform.LookAt(center,store.transform.TransformDirection(Mathf.Abs(normal.y)>.5f?Vector3.forward:Vector3.up));}
 public static Task<string> Run()=>RunInternal(false);
 public static Task<string> RunBanks()=>RunInternal(true);
 public static Task<string> RunLeaves()=>RunInternal(false,true);
 static async Task<string> RunInternal(bool banked,bool leaves=false){
  if(!Application.isPlaying)throw new Exception("Play Mode required");
  var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single(g=>g.gameObject.scene.path==Mf6AvatarBuild.ScenePath);var module=gm.Module as ModuleVrc3;if(module==null)throw new Exception("GestureManager not ready");
  var store=module.Avatar.GetComponentsInChildren<ParticleSystem>(true).Single(p=>p.name=="Store"&&p.transform.parent.name=="Pool0");
  int seq=(module.GetParam("MF6_P0_LastRx").IntValue()+1)%16;module.GetParam("MF6_P0_Ctl").Set(module,seq*8+5);await Task.Delay(350);seq=(seq+1)%16;
  var data=new float[20];for(int i=0;i<3;i++){data[i]=124;data[i+3]=7;}data[6]=1;data[9]=255;for(int i=0;i<20;i++)module.GetParam("MF6_P0_D"+i.ToString("D2")).Set(module,data[i]);module.GetParam("MF6_P0_Ctl").Set(module,seq*8+1);await Task.Delay(350);
  var renderer=store.GetComponent<ParticleSystemRenderer>();var originalMaterial=renderer.sharedMaterial;int layer=store.gameObject.layer;var reference=store.main.customSimulationSpace;var oldRotation=reference.localRotation;var oldScale=reference.localScale;
  var material=new Material(originalMaterial);var meta=new Texture2D(8,1,TextureFormat.RGBAFloat,false,true){filterMode=FilterMode.Point};var faces=Texture(false);var stripes=Texture(true);
  material.SetTexture("_CatalogMeta",meta);material.SetFloat("_MetaWidth",8);material.SetFloat("_MetaHeight",1);material.SetTexture("_CatalogLayers",faces);material.SetFloat("_UseLayeredTextures",1);material.SetFloat("_VoxelGrid",0);
  var bankResources=new System.Collections.Generic.List<UnityEngine.Object>();
  if(banked){
   var palette=new[]{Color.magenta,Color.white,Color.yellow,Color.black,Color.green,Color.cyan,Color.red,Color.blue};
   for(int bank=0;bank<4;bank++){var array=new Texture2DArray(16,16,2,TextureFormat.RGBA32,false,false){filterMode=FilterMode.Point,wrapMode=TextureWrapMode.Repeat};for(int i=0;i<2;i++)array.SetPixels(Enumerable.Repeat(palette[bank*2+i],256).ToArray(),i);array.Apply(false,false);bankResources.Add(array);material.SetTexture(bank==0?"_CatalogLayers":"_CatalogLayers"+bank,array);}
   var mapping=new Texture2D(8,1,TextureFormat.RGBA32,false,true){filterMode=FilterMode.Point,wrapMode=TextureWrapMode.Clamp};var values=new Color32[8];var addresses=new[]{6,5,4,0,7,2};for(int i=0;i<6;i++)values[i]=new Color32((byte)addresses[i],0,0,255);mapping.SetPixels32(values);mapping.Apply(false,false);bankResources.Add(mapping);
   material.SetTexture("_CatalogFaceMap",mapping);material.SetFloat("_UseFaceMap",1);material.SetFloat("_LayerBankSize",2);material.SetVector("_FaceMapSize",new Vector4(8,1,0,0));
  }
  renderer.sharedMaterial=material;store.gameObject.layer=31;var camera=new GameObject("MF6 metadata GPU acceptance").AddComponent<Camera>();camera.enabled=false;camera.cullingMask=1<<31;camera.orthographic=true;camera.nearClipPlane=.00001f;camera.farClipPlane=100;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=new Color(.02f,.03f,.04f);
  var target=new RenderTexture(256,256,24,RenderTextureFormat.ARGB32){antiAliasing=4};target.Create();camera.targetTexture=target;var checks=new JArray();Directory.CreateDirectory("Docs/MF6/visual");
  try{
   if(leaves){
    material.SetFloat("_UseFaceMap",0);
    for(int face=0;face<6;face++)faces.SetPixels(Enumerable.Repeat(face==5?new Color(.04f,1,.05f,1):new Color(1,1,1,0),256).ToArray(),face);
    faces.Apply(false,false);Metadata(meta,0,4);
    foreach(bool cutout in new[]{false,true}){
     meta.SetPixel(6,0,new Color(0,1,1,cutout?69:65));meta.Apply(false,false);
     View(camera,store,Vector3.forward);var image=Render(camera,target);var pixel=image.GetPixel(128,128);
     bool green=pixel.g>.3f&&pixel.g>pixel.r*2&&pixel.g>pixel.b*2;
     checks.Add(new JObject{["cutout"]=cutout,["pixel"]=new JArray(pixel.r,pixel.g,pixel.b),["passed"]=green==cutout});
     File.WriteAllBytes("Docs/MF6/visual/leaves-"+cutout+".png",image.EncodeToPNG());UnityEngine.Object.DestroyImmediate(image);
    }
    var proof=new JObject{["success"]=checks.All(c=>(bool)c["passed"]),["checks"]=checks,["scope"]="Production MF6 Shader and native Animator-born box: fully transparent near face, solid green far face. Cutout must show far backface; opaque styles keep normal culling."};
    File.WriteAllText("Docs/StreamV16/leaves-gpu.json",proof.ToString());return proof.ToString();
   }
   foreach(bool transformed in new[]{false,true}){
    reference.localRotation=transformed?oldRotation*Quaternion.Euler(20,35,10):oldRotation;reference.localScale=oldScale*(transformed?.4f:1);await Task.Delay(100);
    for(int rotation=0;rotation<24;rotation++){
     Metadata(meta,rotation,4);
     for(int view=0;view<6;view++){
      View(camera,store,Faces[view]);var image=Render(camera,target);var pixel=image.GetPixel(128,128);int expected=Face(Quaternion.Inverse(Rotation(rotation))*Faces[view]);Color c=Colors[expected];bool pass=(pixel.r>.3f)==(c.r>.5f)&&(pixel.g>.3f)==(c.g>.5f)&&(pixel.b>.3f)==(c.b>.5f);
      checks.Add(new JObject{["kind"]="face",["rotation"]=rotation,["view"]=view,["transformed"]=transformed,["expectedFace"]=expected,["pixel"]=new JArray(pixel.r,pixel.g,pixel.b),["passed"]=pass});
      if(rotation==0&&view==4)File.WriteAllBytes("Docs/MF6/visual/catalog-"+(transformed?"transformed":"base")+".png",image.EncodeToPNG());UnityEngine.Object.DestroyImmediate(image);
     }
    }
   }
   reference.localRotation=oldRotation;reference.localScale=oldScale;await Task.Delay(100);material.SetTexture("_CatalogLayers",stripes);material.SetFloat("_UseFaceMap",0);material.SetFloat("_LayerBankSize",2048);
   foreach(float scale in new[]{0f,1f,4f,8f}){
    Metadata(meta,0,scale);View(camera,store,Vector3.forward);var image=Render(camera,target);var pixels=image.GetPixels();int transitions=0;bool previous=pixels[128*256+23].r>.5f;for(int x=24;x<233;x++){bool next=pixels[128*256+x].r>.5f;if(next!=previous)transitions++;previous=next;}
    int expected=scale==1?15:scale==4?3:1;checks.Add(new JObject{["kind"]="repeat",["cellsPerTile"]=scale,["transitions"]=transitions,["expected"]=expected,["passed"]=transitions==expected});File.WriteAllBytes("Docs/MF6/visual/repeat-"+scale+".png",image.EncodeToPNG());UnityEngine.Object.DestroyImmediate(image);
   }
   var result=new JObject{["success"]=checks.All(c=>(bool)c["passed"]),["checks"]=checks,["scope"]="Production MF6 shader and native Animator-born Box; diagnostic six-face texture array and metadata, 24 cube orientations, anchor rotation/scale, tile repeat. No MeshRenderer stand-in or scripted particle injection."};File.WriteAllText(banked?"Docs/MF6/catalog-banks-gpu.json":"Docs/MF6/catalog-gpu.json",result.ToString());return new JObject{["success"]=result["success"],["checks"]=checks.Count,["failed"]=new JArray(checks.Where(c=>!(bool)c["passed"]))}.ToString();
  }finally{reference.localRotation=oldRotation;reference.localScale=oldScale;renderer.sharedMaterial=originalMaterial;store.gameObject.layer=layer;camera.targetTexture=null;UnityEngine.Object.DestroyImmediate(camera.gameObject);target.Release();UnityEngine.Object.DestroyImmediate(target);foreach(var resource in bankResources)UnityEngine.Object.DestroyImmediate(resource);foreach(var resource in new UnityEngine.Object[]{material,meta,faces,stripes})UnityEngine.Object.DestroyImmediate(resource);}
 }
}
