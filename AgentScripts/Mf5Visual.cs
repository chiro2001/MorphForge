using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Threading.Tasks;
using UnityEngine;
using UnityEditor;
using UnityEditor.SceneManagement;
using Newtonsoft.Json.Linq;
public static class Mf5Visual
{
    static Texture2D Render(Camera c,RenderTexture rt){c.Render();var prev=RenderTexture.active;RenderTexture.active=rt;var t=new Texture2D(256,256,TextureFormat.RGBA32,false);t.ReadPixels(new Rect(0,0,256,256),0,0);t.Apply();RenderTexture.active=prev;return t;}
    public static async Task<string> Run()
    {
        if(EditorApplication.isPlaying)throw new Exception("Edit Mode required.");
        const string root="Docs/MF5/visual";Directory.CreateDirectory(root);
        var preview=EditorSceneManager.NewPreviewScene();var source=AssetDatabase.LoadAssetAtPath<Material>("Assets/MorphForge/Diagnostics/MF5Catalog/ParticleBoxCatalog.mat");
        var material=new Material(source);var target=new RenderTexture(256,256,24){antiAliasing=4};target.Create();Mesh mesh=null;var checks=new JArray();
        try{
            var cube=GameObject.CreatePrimitive(PrimitiveType.Cube);UnityEngine.SceneManagement.SceneManager.MoveGameObjectToScene(cube,preview);mesh=UnityEngine.Object.Instantiate(cube.GetComponent<MeshFilter>().sharedMesh);var vertices=mesh.vertices;var normals=mesh.normals;var uv=new List<Vector4>();
            for(int i=0;i<vertices.Length;i++){var v=vertices[i];var n=normals[i];int axis=Mathf.Abs(n.x)>.5f?0:Mathf.Abs(n.y)>.5f?1:2;uv.Add(new Vector4(axis==0?v.z+.5f:v.x+.5f,axis==1?v.z+.5f:v.y+.5f,axis,0));}
            mesh.SetUVs(0,uv);mesh.SetUVs(1,Enumerable.Repeat(Vector3.one*2,mesh.vertexCount).ToList());cube.GetComponent<MeshFilter>().sharedMesh=mesh;cube.GetComponent<MeshRenderer>().sharedMaterial=material;
            var go=new GameObject("MF5 production pixel camera");UnityEngine.SceneManagement.SceneManager.MoveGameObjectToScene(go,preview);var c=go.AddComponent<Camera>();c.scene=preview;c.cameraType=CameraType.Preview;c.enabled=false;c.targetTexture=target;c.orthographic=true;c.orthographicSize=.9f;c.transform.position=new Vector3(3,2,4);c.transform.LookAt(Vector3.zero);c.nearClipPlane=.01f;c.farClipPlane=30;c.clearFlags=CameraClearFlags.SolidColor;c.backgroundColor=new Color(.04f,.06f,.09f);
            foreach(var shaderName in new[]{"MorphForge/ParticleBoxCatalogMetadataGrid","MorphForge/ParticleBoxCatalog"}){
                material.shader=Shader.Find(shaderName);if(!material.shader)throw new Exception("Missing shader "+shaderName);
                var errors=ShaderUtil.GetShaderMessages(material.shader).Where(m=>m.severity==UnityEditor.Rendering.ShaderCompilerMessageSeverity.Error).ToArray();if(errors.Length>0)throw new Exception(string.Join("\n",errors.Select(e=>e.message)));
                foreach(int style in new[]{1,25,26,27,28,64,76,82,111}){
                    int bits=style|(17<<12);material.SetFloat("_WireEncoding",4);mesh.colors32=Enumerable.Repeat(new Color32((byte)bits,(byte)(bits>>8),(byte)(bits>>16),255),mesh.vertexCount).ToArray();var reference=Render(c,target);
                    material.SetFloat("_WireEncoding",5);var actual=Render(c,target);int diff=reference.GetPixels32().Zip(actual.GetPixels32(),(a,b)=>a.Equals(b)?0:1).Sum();checks.Add(new JObject{["shader"]=shaderName,["catalogStyle"]=style,["differentPixels"]=diff,["passed"]=diff==0});UnityEngine.Object.DestroyImmediate(reference);UnityEngine.Object.DestroyImmediate(actual);
                }
                for(int mode=1;mode<=7;mode++){
                    // Opaque RGB; silver PBR; semi-transparent water/cloud; wood/stone; tinted baked stone.
                    int bits=mode==1?0xAD79F1:mode==2?(24|(50<<5)|(28<<11)|(3<<16)|(1<<18)):mode==7?(65|(0xF73<<12)):(0xA76|(2<<12)|((mode<=4?1:0)<<14)|(3<<18)|(2<<21));
                    material.SetFloat("_WireEncoding",5);mesh.colors32=Enumerable.Repeat(new Color32((byte)bits,(byte)(bits>>8),(byte)(bits>>16),(byte)(255-mode)),mesh.vertexCount).ToArray();var actual=Render(c,target);var pixels=actual.GetPixels32();var background=pixels[0];int visible=pixels.Count(p=>!p.Equals(background));bool ok=visible>5000;checks.Add(new JObject{["shader"]=shaderName,["mode"]=mode,["visiblePixels"]=visible,["passed"]=ok});
                    if(shaderName.EndsWith("MetadataGrid"))File.WriteAllBytes(root+"/mode-"+mode+".png",actual.EncodeToPNG());
                    if(mode==1||mode==5||mode==6||mode==7){await Task.Delay(200);var later=Render(c,target);bool stable=pixels.SequenceEqual(later.GetPixels32());checks.Add(new JObject{["shader"]=shaderName,["mode"]=mode,["staticAcrossTime"]=stable,["passed"]=stable});UnityEngine.Object.DestroyImmediate(later);}
                    UnityEngine.Object.DestroyImmediate(actual);
                }
            }
            bool success=checks.All(t=>(bool)t["passed"]);var report=new JObject{["success"]=success,["checks"]=checks,["scope"]="Actual GPU camera pixels from both production shaders; catalog backward equivalence, every runtime surface visible, static colors/patterns stable. MSAA4. No OSC."};File.WriteAllText("Docs/MF5/visual.json",report.ToString());if(!success)throw new Exception(report.ToString());return report.ToString();
        }finally{if(mesh)UnityEngine.Object.DestroyImmediate(mesh);UnityEngine.Object.DestroyImmediate(material);target.Release();UnityEngine.Object.DestroyImmediate(target);EditorSceneManager.ClosePreviewScene(preview);}
    }
}
