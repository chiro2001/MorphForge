using System;
using System.IO;
using System.Linq;
using System.Net.Sockets;
using System.Threading.Tasks;
using UnityEditor;
using MorphForge.Editor;
using UnityEngine;
using Newtonsoft.Json.Linq;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
public static class TextGapProbe {
    static float Value(Renderer r,string property){var block=new MaterialPropertyBlock();r.GetPropertyBlock(block);return block.HasFloat(property)?block.GetFloat(property):r.sharedMaterial.GetFloat(property);}
    public static async Task<string> Run(){
        if(!EditorApplication.isPlaying)throw new Exception("Owned Play Mode required");
        var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single(g=>g.gameObject.scene.path=="Assets/MorphForge/Validation/Optimization20261023/Baseline.unity");
        var receiver=gm.GetComponent<MorphForgePlayReceiver>();if(!receiver.Running||receiver.ReceivePort!=9012)throw new Exception("Isolated receiver required");
        var module=gm.Module as ModuleVrc3;var panel=module.Avatar.GetComponentsInChildren<MeshRenderer>(true).Single(r=>r.name=="Panel0"&&r.transform.parent.name=="TextDisplay");
        var fixture=JObject.Parse(File.ReadAllText("E:/tmp/morphforge/text-fixtures.json"));
        using(var udp=new UdpClient()){foreach(var token in fixture["gapFrames"]){var bytes=token.Select(v=>(byte)(int)v).ToArray();udp.Send(bytes,bytes.Length,"127.0.0.1",9012);await Task.Delay(300);}}
        await Task.Delay(500);
        int layer=panel.gameObject.layer;var camera=new GameObject("Text gap stability proof").AddComponent<Camera>();camera.enabled=false;camera.allowHDR=false;camera.orthographic=true;camera.aspect=2;camera.cullingMask=1<<31;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=Color.clear;camera.nearClipPlane=.0001f;camera.farClipPlane=100;
        var target=new RenderTexture(1024,512,24,RenderTextureFormat.ARGB32);target.Create();camera.targetTexture=target;var image=new Texture2D(1024,512,TextureFormat.RGBA32,false);var rows=new JArray();
        var origin=new Vector3(Value(panel,"_PosX"),Value(panel,"_PosY")-128,Value(panel,"_PosZ"))*.25f;var t=panel.transform;var layout=fixture["gapLayout"];
        try{panel.gameObject.layer=31;
            foreach(float zoom in new[]{1f,2.5f}){
                camera.orthographicSize=Value(panel,"_Height")*.25f*.75f*t.lossyScale.y*zoom;
                int ink=0,leaked=0,blank=0;string first=null;int changed=0;
                for(int frame=0;frame<45;frame++){
                    var jitter=new Vector3((frame%3-1)*.015f,0,0);var center=origin+new Vector3(0,Value(panel,"_Height")*.125f,0)+jitter;
                    camera.transform.position=t.TransformPoint(center+new Vector3(0,0,-100));camera.transform.LookAt(t.TransformPoint(center),t.up);
                    var allowed=new bool[1024*512];foreach(var glyph in layout["glyphs"]){if((int)glyph["glyph"]==32)continue;
                        float x=(float)glyph["x"]-Value(panel,"_Width")*.5f,y=(float)glyph["y"],w=(float)glyph["width"],h=(float)glyph["height"];
                        var p=camera.WorldToScreenPoint(t.TransformPoint(origin+new Vector3(x,y,0)*.25f));var q=camera.WorldToScreenPoint(t.TransformPoint(origin+new Vector3(x+w,y+h,0)*.25f));
                        for(int yy=Math.Max(0,(int)Math.Floor(Math.Min(p.y,q.y))-2);yy<Math.Min(512,(int)Math.Ceiling(Math.Max(p.y,q.y))+2);yy++)for(int xx=Math.Max(0,(int)Math.Floor(Math.Min(p.x,q.x))-2);xx<Math.Min(1024,(int)Math.Ceiling(Math.Max(p.x,q.x))+2);xx++)allowed[yy*1024+xx]=true;
                    }
                    camera.Render();var active=RenderTexture.active;RenderTexture.active=target;image.ReadPixels(new Rect(0,0,1024,512),0,0);image.Apply();RenderTexture.active=active;var pixels=image.GetPixels32();
                    int frameInk=0,frameLeak=0;for(int i=0;i<pixels.Length;i++){if(pixels[i].a>8){frameInk++;if(!allowed[i])frameLeak++;}else if(!allowed[i])blank++;}
                    ink+=frameInk;leaked+=frameLeak;if(frame%3==0){var signature=Convert.ToBase64String(System.Security.Cryptography.SHA256.Create().ComputeHash(image.GetRawTextureData()));if(first==null)first=signature;else if(signature!=first)changed++;}
                    if(frame==0)File.WriteAllBytes("Docs/Optimization20261023/text-gap-"+zoom+".png",image.EncodeToPNG());await Task.Delay(20);
                }
                rows.Add(new JObject{["zoom"]=zoom,["frames"]=45,["inkPixels"]=ink,["blankSamples"]=blank,["unexpectedOpaquePixels"]=leaked,["sameViewChangedFrames"]=changed,["pass"]=ink>100&&blank>100000&&leaked==0&&changed==0});
            }
        }finally{panel.gameObject.layer=layer;camera.targetTexture=null;target.Release();UnityEngine.Object.DestroyImmediate(target);UnityEngine.Object.DestroyImmediate(image);UnityEngine.Object.DestroyImmediate(camera.gameObject);}
        var result=new JObject{["success"]=rows.All(r=>(bool)r["pass"]),["captures"]=rows,["scope"]="Production UDP text config and pages, mixed Chinese/English with 8-cell letter spacing. No material writes or phase injection. 90 GPU frames, near/far and subpixel camera changes after sender stops."};File.WriteAllText("Docs/Optimization20261023/text-gap.json",result.ToString());return result.ToString();
    }
}
