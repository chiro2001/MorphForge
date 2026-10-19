using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Threading.Tasks;
using UnityEngine;
using UnityEditor;
using UnityEditor.SceneManagement;
using MorphForge.Editor;
using Newtonsoft.Json.Linq;
public static class Mf4Visual
{
    const string MaterialPath="Assets/MorphForge/Diagnostics/MF4Catalog/ParticleBoxCatalog.mat";
    public static string RefreshCatalog(string sourceRoot)
    {
        var result=JObject.Parse(CatalogMaterialBuild.BuildAt(sourceRoot,"Assets/MorphForge/Diagnostics/MF4Catalog"));
        var m=AssetDatabase.LoadAssetAtPath<Material>(MaterialPath);m.SetFloat("_WireEncoding",4);EditorUtility.SetDirty(m);AssetDatabase.SaveAssetIfDirty(m);
        return result.ToString();
    }
    static Texture2D Render(Camera camera,RenderTexture target){camera.Render();var prev=RenderTexture.active;RenderTexture.active=target;var t=new Texture2D(256,256,TextureFormat.RGBA32,false);t.ReadPixels(new Rect(0,0,256,256),0,0);t.Apply();RenderTexture.active=prev;return t;}
    public static async Task<string> Run()
    {
        if(EditorApplication.isPlaying)throw new Exception("Edit Mode required.");
        var preview=EditorSceneManager.NewPreviewScene();var source=AssetDatabase.LoadAssetAtPath<Material>(MaterialPath);var material=new Material(source);
        var target=new RenderTexture(256,256,24){antiAliasing=4};target.Create();Mesh mesh=null;
        var checks=new JArray();Directory.CreateDirectory("Docs/MF4/visual");
        try {
            var cube=GameObject.CreatePrimitive(PrimitiveType.Cube);UnityEngine.SceneManagement.SceneManager.MoveGameObjectToScene(cube,preview);mesh=UnityEngine.Object.Instantiate(cube.GetComponent<MeshFilter>().sharedMesh);var vertices=mesh.vertices;var normals=mesh.normals;var uv=new List<Vector4>();
            for(int i=0;i<vertices.Length;i++){var v=vertices[i];var n=normals[i];int axis=Mathf.Abs(n.x)>.5f?0:Mathf.Abs(n.y)>.5f?1:2;uv.Add(new Vector4(axis==0?v.z+.5f:v.x+.5f,axis==1?v.z+.5f:v.y+.5f,axis,0));}
            mesh.SetUVs(0,uv);mesh.SetUVs(1,Enumerable.Repeat(Vector3.one*.25f,mesh.vertexCount).ToList());cube.GetComponent<MeshFilter>().sharedMesh=mesh;cube.GetComponent<MeshRenderer>().sharedMaterial=material;
            var go=new GameObject("MF4 pixel camera");UnityEngine.SceneManagement.SceneManager.MoveGameObjectToScene(go,preview);var c=go.AddComponent<Camera>();c.scene=preview;c.cameraType=CameraType.Preview;c.enabled=false;c.targetTexture=target;c.orthographic=true;c.orthographicSize=1.1f;c.transform.position=new Vector3(3,2,4);c.transform.LookAt(Vector3.zero);c.nearClipPlane=.01f;c.farClipPlane=30;c.clearFlags=CameraClearFlags.SolidColor;c.backgroundColor=new Color(.04f,.06f,.09f);
            var errors=ShaderUtil.GetShaderMessages(material.shader).Where(m=>m.severity==UnityEditor.Rendering.ShaderCompilerMessageSeverity.Error).Select(m=>m.message).ToArray();if(errors.Length>0)throw new Exception(string.Join("\n",errors));
            foreach(int style in new[]{1,25,26,27,28,64,76,78,82}) foreach(int tint in new[]{0,63})
            {
                int motion=0;material.SetFloat("_WireEncoding",0);mesh.colors=Enumerable.Repeat(new Color((style&63)/63f,(style>>6)/63f,tint/63f,motion/63f),mesh.vertexCount).ToArray();var reference=Render(c,target);
                int bits=style|(tint<<12)|(motion<<18);material.SetFloat("_WireEncoding",4);mesh.colors32=Enumerable.Repeat(new Color32((byte)bits,(byte)(bits>>8),(byte)(bits>>16),255),mesh.vertexCount).ToArray();var actual=Render(c,target);
                int difference=reference.GetPixels32().Zip(actual.GetPixels32(),(a,b)=>a.Equals(b)?0:1).Sum();checks.Add(new JObject{["style"]=style,["tint"]=tint,["differentPixels"]=difference,["passed"]=difference==0});
                if(style>=25&&style<=28&&tint==0){File.WriteAllBytes("Docs/MF4/visual/palette-"+style+".png",actual.EncodeToPNG());await Task.Delay(350);var later=Render(c,target);bool stable=actual.GetPixels32().SequenceEqual(later.GetPixels32());checks.Add(new JObject{["style"]=style,["staticAcrossTime"]=stable,["passed"]=stable});UnityEngine.Object.DestroyImmediate(later);}
                UnityEngine.Object.DestroyImmediate(reference);UnityEngine.Object.DestroyImmediate(actual);
            }
            bool success=checks.All(t=>(bool)t["passed"]);var report=new JObject{["success"]=success,["checks"]=checks,["scope"]="Actual GPU camera pixels: packed RGB24 vs MF3 reference, custom palette static across time. No OSC."};File.WriteAllText("Docs/MF4/visual.json",report.ToString());if(!success)throw new Exception(report.ToString());return report.ToString();
        }
        finally {if(mesh)UnityEngine.Object.DestroyImmediate(mesh);UnityEngine.Object.DestroyImmediate(material);target.Release();UnityEngine.Object.DestroyImmediate(target);EditorSceneManager.ClosePreviewScene(preview);}
    }
}
