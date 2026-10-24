using System;
using System.IO;
using System.Linq;
using System.Net.Sockets;
using System.Threading.Tasks;
using UnityEditor;
using UnityEngine;
using Newtonsoft.Json.Linq;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
using MorphForge.Editor;
public static class NativeDrawingProbe {
    public static string RefreshShader(){AssetDatabase.ImportAsset("Assets/MorphForgeAvatar/Shaders/ParticleBoxCatalogMetadataGrid.shader",ImportAssetOptions.ForceSynchronousImport|ImportAssetOptions.ForceUpdate);return "Native drawing shader refreshed";}
    public static string Inspect(){
        var m=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single().Module as ModuleVrc3;
        return new JObject{["stores"]=new JArray(m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Where(p=>p.name=="Store").Select(p=>{var r=p.GetComponent<ParticleSystemRenderer>();var values=new ParticleSystem.Particle[p.main.maxParticles];int n=p.GetParticles(values);return (JToken)new JObject{["pool"]=p.transform.parent.name,["shader"]=r.sharedMaterial.shader.name,["path"]=AssetDatabase.GetAssetPath(r.sharedMaterial.shader),["supported"]=r.sharedMaterial.shader.isSupported,["wire"]=r.sharedMaterial.GetFloat("_WireEncoding"),["cell"]=r.sharedMaterial.GetFloat("_CellSize"),["bounds"]=r.bounds.ToString(),["scale"]=p.main.customSimulationSpace.lossyScale.ToString("R"),["errors"]=new JArray(ShaderUtil.GetShaderMessages(r.sharedMaterial.shader).Select(e=>e.message)),["particles"]=new JArray(values.Take(n).Where(v=>v.startColor.a>0).Select(v=>(JToken)new JObject{["color"]=v.startColor.ToString(),["position"]=v.position.ToString("R"),["size"]=v.startSize3D.ToString("R"),["life"]=v.startLifetime}))};}))}.ToString();
    }
    public static async Task<string> Run() {
        string scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene().path;
        if(scene!="Assets/MorphForge/Validation/Optimization20261023/Baseline.unity"&&scene!="Assets/MorphForge/Validation/WaterfallSdk20261024/Baseline.unity")throw new Exception("Owned native drawing scene required");
        if(!EditorApplication.isPlaying)throw new Exception("Owned Play Mode required");
        var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single(g=>g.gameObject.scene.path==scene);var m=gm.Module as ModuleVrc3;if(m==null)throw new Exception("Native Avatar not ready");
        var receiver=gm.GetComponent<MorphForgePlayReceiver>();if(!receiver.Running)receiver.TryStart();if(!receiver.Running||receiver.ReceivePort!=9012)throw new Exception("Isolated receiver required");
        var data=JObject.Parse(File.ReadAllText("E:/tmp/morphforge/native-drawing-fixtures.json"));
        var stores=m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Where(p=>p.name=="Store"&&p.transform.parent.name.StartsWith("Pool")).ToArray();var renderers=stores.Select(p=>p.GetComponent<ParticleSystemRenderer>()).ToArray();var layers=renderers.Select(r=>r.gameObject.layer).ToArray();var root=stores[0].main.customSimulationSpace;
        var camera=new GameObject("Native point and line GPU proof").AddComponent<Camera>();camera.enabled=false;camera.cullingMask=1<<31;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=Color.black;camera.orthographic=true;camera.orthographicSize=8*root.lossyScale.y;camera.aspect=1;camera.nearClipPlane=.0001f;camera.farClipPlane=20;
        var center=root.TransformPoint(new Vector3(0,-86*.25f,0));camera.transform.position=center-root.forward*50*root.lossyScale.z;camera.transform.LookAt(center,root.up);
        var target=new RenderTexture(256,256,24,RenderTextureFormat.ARGB32);target.Create();camera.targetTexture=target;var image=new Texture2D(256,256,TextureFormat.RGB24,false);var rows=new JArray();
        using(var udp=new UdpClient())try {
            async Task Send(JToken packet){var bytes=packet.Select(v=>(byte)(int)v).ToArray();udp.Send(bytes,bytes.Length,"127.0.0.1",9012);await Task.Delay(450);}
            async Task Phase(string name){foreach(var row in data[name])await Send(row["packet"]);}
            JObject Capture(string name){camera.Render();var active=RenderTexture.active;RenderTexture.active=target;image.ReadPixels(new Rect(0,0,256,256),0,0);image.Apply();RenderTexture.active=active;File.WriteAllBytes("Docs/Optimization20261023/drawing-"+name+".png",image.EncodeToPNG());var pixels=image.GetPixels32();
                JObject Bounds(Func<Color32,bool> test){var indices=Enumerable.Range(0,pixels.Length).Where(i=>test(pixels[i])).ToArray();return new JObject{["ink"]=indices.Length,["width"]=indices.Length==0?0:indices.Max(i=>i%256)-indices.Min(i=>i%256)+1,["height"]=indices.Length==0?0:indices.Max(i=>i/256)-indices.Min(i=>i/256)+1};}
                return new JObject{["phase"]=name,["point"]=Bounds(c=>c.r>60&&c.r>c.g*1.5f&&c.r>c.b*1.5f),["line"]=Bounds(c=>c.b>60&&c.b>c.r*1.5f),["cube"]=Bounds(c=>c.g>60&&c.g>c.r*1.5f&&c.g>c.b*1.5f)};}
            int Alive()=>stores.Sum(p=>{var values=new ParticleSystem.Particle[p.main.maxParticles];int n=p.GetParticles(values);return values.Take(n).Count(v=>v.startColor.a>0);});
            await Send(data["clear"]);for(int i=0;i<renderers.Length;i++)renderers[i].gameObject.layer=31;
            await Phase("thin");var thin=Capture("thin");rows.Add(thin);bool count=Alive()==3;
            await Phase("thick");var thick=Capture("thick");rows.Add(thick);bool replaced=Alive()==3;
            await Phase("replay");var replay=Capture("replay");rows.Add(replay);bool idempotent=Alive()==3;
            camera.transform.position=center+root.right*50*root.lossyScale.x;camera.transform.LookAt(center,root.up);var endOn=Capture("end-on");rows.Add(endOn);bool endVisible=(int)endOn["line"]["width"]>10&&(int)endOn["line"]["height"]>10;
            camera.transform.position=center-root.forward*50*root.lossyScale.z;camera.transform.LookAt(center,root.up);
            await Phase("delete");var deleted=Capture("delete");rows.Add(deleted);bool removed=Alive()==2&&(int)deleted["point"]["ink"]==0&&(int)deleted["line"]["ink"]>20;
            bool sizes=(int)thin["point"]["width"]>5&&(int)thick["point"]["width"]>(int)thin["point"]["width"]*1.7&&(int)thin["line"]["height"]>1&&(int)thick["line"]["height"]>(int)thin["line"]["height"]*2.3&&(int)thin["cube"]["ink"]>20;
            var report=new JObject{["success"]=count&&replaced&&idempotent&&removed&&sizes&&endVisible,["initialCount"]=count,["replacementCount"]=replaced,["replayCount"]=idempotent,["removedOnlyPoint"]=removed,["diameterAndThickness"]=sizes,["endOnVisible"]=endVisible,["captures"]=rows,["scope"]="Production Rust shared planner -> one OSC command per point/segment -> native Animator particle storage -> production GPU shader. No scripted Emit, SetParticles or phase/material injection."};File.WriteAllText("Docs/Optimization20261023/native-drawing.json",report.ToString());return report.ToString();
        }finally{for(int i=0;i<renderers.Length;i++)renderers[i].gameObject.layer=layers[i];camera.targetTexture=null;target.Release();UnityEngine.Object.DestroyImmediate(target);UnityEngine.Object.DestroyImmediate(image);UnityEngine.Object.DestroyImmediate(camera.gameObject);}
    }
}
