using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using UnityEditor;
using UnityEngine;
using Newtonsoft.Json.Linq;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
public static class NativePreviewProbe {
 public static Task<string> Run(){return Probe(true);}
 public static Task<string> RunLocal(){return Probe(false);}
 static async Task<string> Probe(bool includeRemote){
  if(!EditorApplication.isPlaying)throw new Exception("Play Mode required");var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single();var m=gm.Module as ModuleVrc3;if(m==null)throw new Exception("GestureManager not ready");var rows=new JArray();
  var root=m.Avatar.GetComponentsInChildren<Transform>(true).Single(t=>t.name=="LocalPreview");var camera=root.Find("CaptureRig/Camera").GetComponent<Camera>();var panel=root.Find("HeadRig/Panel");
  string[] names={"IsLocal","MF_Preview","MF_PreviewCorner","MF_PreviewSize","MF_PreviewZoom","MF_PreviewOblique"};var old=names.Select(n=>m.GetParam(n).FloatValue()).ToArray();
  void Set(string n,float v){m.GetParam(n).Set(m,v);}void Check(string name,bool pass,JToken data=null){rows.Add(new JObject{["name"]=name,["pass"]=pass,["data"]=data});}
  async Task Tick(){await Task.Delay(350);}
  try{
   Set("IsLocal",1);Set("MF_Preview",0);await Tick();Check("owner default off stops camera and screen",!camera.gameObject.activeInHierarchy&&!panel.gameObject.activeInHierarchy);
   Set("MF_Preview",1);await Tick();Check("owner can enable camera and screen",camera.gameObject.activeInHierarchy&&panel.gameObject.activeInHierarchy&&camera.targetTexture);
   var head=m.Avatar.GetComponent<Animator>().GetBoneTransform(HumanBodyBones.Head);Check("panel follows head without being head descendant",Vector3.Distance(panel.parent.position,head.position)<.02f&&!panel.IsChildOf(head),new JArray(panel.parent.position.x,panel.parent.position.y,panel.parent.position.z));
   var positions=new JArray();for(int i=0;i<6;i++){Set("MF_PreviewCorner",i);await Tick();var p=panel.localPosition;positions.Add(new JArray(p.x,p.y,p.z));}Check("six distinct edge and corner positions",positions.Select(p=>p.ToString()).Distinct().Count()==6,positions);
   Set("MF_PreviewSize",0);Set("MF_PreviewZoom",0);Set("MF_PreviewOblique",0);await Tick();var small=panel.localScale.x;var wide=camera.fieldOfView;var front=camera.transform.localPosition;
   Set("MF_PreviewSize",1);Set("MF_PreviewZoom",1);Set("MF_PreviewOblique",1);await Tick();Check("local size zoom and view controls",panel.localScale.x>small*2&&wide>camera.fieldOfView*2&&Vector3.Distance(front,camera.transform.localPosition)>20,new JObject{["small"]=small,["large"]=panel.localScale.x,["wideFov"]=wide,["zoomFov"]=camera.fieldOfView});
   Set("MF_PreviewZoom",0);Set("MF_PreviewOblique",0);await Tick();camera.Render();var rt=camera.targetTexture;var previous=RenderTexture.active;RenderTexture.active=rt;var image=new Texture2D(rt.width,rt.height,TextureFormat.RGB24,false);image.ReadPixels(new Rect(0,0,rt.width,rt.height),0,0);image.Apply();RenderTexture.active=previous;File.WriteAllBytes("Docs/TextAnimation/local-preview-received.png",image.EncodeToPNG());var previewPixels=image.GetPixels32();int varied=previewPixels.Count(c=>c.r>70||c.g>70||c.b>70);var panelRenderer=panel.GetComponent<MeshRenderer>();var rendererEnabled=panelRenderer.enabled;panelRenderer.enabled=false;camera.Render();RenderTexture.active=rt;image.ReadPixels(new Rect(0,0,rt.width,rt.height),0,0);image.Apply();RenderTexture.active=previous;panelRenderer.enabled=rendererEnabled;var withoutPanel=image.GetPixels32();int selfDifference=previewPixels.Select((c,i)=>Math.Abs(c.r-withoutPanel[i].r)+Math.Abs(c.g-withoutPanel[i].g)+Math.Abs(c.b-withoutPanel[i].b)).Count(v=>v>3);Check("capture camera excludes its own preview screen",selfDifference==0,new JValue(selfDifference));UnityEngine.Object.DestroyImmediate(image);Check("512x384 native render target contains scene output",rt.width==512&&rt.height==384&&varied>20,new JValue(varied));
   if(includeRemote){Set("IsLocal",0);Set("MF_Preview",1);await Tick();Check("remote observer cannot enable camera or screen",!camera.gameObject.activeInHierarchy&&!panel.gameObject.activeInHierarchy);}else{Check("owner identity remains fixed",m.GetParam("IsLocal").FloatValue()==1);}
   var result=new JObject{["success"]=rows.All(c=>(bool)c["pass"]),["checks"]=rows,["scope"]="Actual merged Animator/constraints/Camera/RenderTexture in Editor, owner-vs-remote IsLocal gate. No headset comfort, mirror/photo globals or VRChat runtime claim; prototype default off."};File.WriteAllText("Docs/TextAnimation/"+(includeRemote?"local-preview-play":"local-preview-owner-play")+".json",result.ToString());return result.ToString();
  }finally{for(int i=0;i<names.Length;i++)Set(names[i],old[i]);}
 }
}
