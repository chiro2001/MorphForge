using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using UnityEngine;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine.SceneManagement;
using Newtonsoft.Json.Linq;

// Shader-only GPU fixture, not a native particle/OSC acceptance test. Synthetic
// vertex streams isolate coplanar depth behavior without entering Avatar Play Mode.
public static class CoplanarDepthProbe
{
    static readonly string Root="Docs/Overlap0152";
    static float Packed(int x,int y,int z){int bits=x|(y<<8)|(z<<16);return bits<8388608?1+bits/8388608f:bits/4194304f;}
    static Mesh Streams(Mesh cube,Vector3 size,Vector3 center,Color32 color,bool triangle)
    {
        var mesh=UnityEngine.Object.Instantiate(cube);mesh.name="Synthetic particle vertex streams";
        var vertices=cube.vertices;var normals=cube.normals;var uv=cube.uv;var tangents=new Vector4[vertices.Length];
        var face=new List<Vector4>();var sizes=new List<Vector4>();var life=new List<Vector4>();
        var first=cube.triangles.Take(3).ToArray();var second=cube.triangles.Skip(3).Take(3).ToArray();int shared=first.First(v=>second.Contains(v));
        for(int i=0;i<vertices.Length;i++){
            var v=vertices[i];var n=normals[i];int axis=Mathf.Abs(n.x)>.5f?0:Mathf.Abs(n.y)>.5f?1:2;
            int corner=(v.x>0?1:0)|(v.y>0?2:0)|(v.z>0?4:0);float sign=axis==0?n.x:axis==1?n.y:n.z;
            int vertex=Array.IndexOf(first,i);if(vertex<0&&second.Contains(i))vertex=Array.IndexOf(first,shared);
            face.Add(new Vector4(uv[i].x,uv[i].y,corner+axis*8,vertex));
            sizes.Add(new Vector4(size.x,size.y,size.z,center.x));
            life.Add(new Vector4(center.y,center.z,triangle?1f/(524288+8*17):1e-6f,0));
            Vector3 t=axis==0?Vector3.forward:Vector3.right;tangents[i]=new Vector4(t.x,t.y,t.z,axis==2?sign:-sign);
            vertices[i]=center+Vector3.Scale(v,size);
        }
        mesh.vertices=vertices;mesh.tangents=tangents;mesh.SetUVs(0,face);mesh.SetUVs(1,sizes);mesh.SetUVs(2,life);
        mesh.colors32=Enumerable.Repeat(color,vertices.Length).ToArray();
        // The fixture uses a large mesh bounds solely for synthetic vertex
        // displacement. It does not change or validate native particle bounds.
        mesh.bounds=new Bounds(new Vector3(0,-28,0),Vector3.one*300);return mesh;
    }
    static Texture2D Render(Camera camera,RenderTexture target){camera.Render();var previous=RenderTexture.active;try{RenderTexture.active=target;var image=new Texture2D(target.width,target.height,TextureFormat.RGBA32,false);image.ReadPixels(new Rect(0,0,target.width,target.height),0,0);image.Apply();return image;}finally{RenderTexture.active=previous;}}
    static bool Red(Color32 p)=>p.r>p.g*2&&p.r>p.b*2&&p.r>35;
    static bool Green(Color32 p)=>p.g>p.r*2&&p.g>p.b*2&&p.g>35;
    public static string Run()
    {
        if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");
        foreach(string shaderPath in new[]{"Assets/MorphForge/Shaders/DepthBaseline0151.shader","Assets/MorphForge/Shaders/ParticleBoxCatalogMetadataGrid.shader"})AssetDatabase.ImportAsset(shaderPath,ImportAssetOptions.ForceUpdate|ImportAssetOptions.ForceSynchronousImport);
        var original=SceneManager.GetActiveScene();var before=Enumerable.Range(0,SceneManager.sceneCount).ToDictionary(i=>SceneManager.GetSceneAt(i).handle,i=>SceneManager.GetSceneAt(i).isDirty);
        var scene=EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Additive);SceneManager.SetActiveScene(scene);
        var resources=new List<UnityEngine.Object>();RenderTexture target=null;var rows=new JArray();
        try{
            Directory.CreateDirectory(Root);
            var cube=GameObject.CreatePrimitive(PrimitiveType.Cube);var cubeMesh=cube.GetComponent<MeshFilter>().sharedMesh;
            var boxMesh=Streams(cubeMesh,new Vector3(2,2,.5f),new Vector3(0,-28,-.25f),new Color32(0,220,0,254),false);resources.Add(boxMesh);
            var triangleMesh=Streams(cubeMesh,new Vector3(Packed(124,12,128),Packed(132,12,128),Packed(128,20,128)),new Vector3((17-128)*.25f,-128,-32),new Color32(230,0,0,1),true);resources.Add(triangleMesh);
            cube.GetComponent<MeshFilter>().sharedMesh=boxMesh;var box=cube.GetComponent<MeshRenderer>();
            var detail=new GameObject("Coplanar triangle",typeof(MeshFilter),typeof(MeshRenderer));detail.GetComponent<MeshFilter>().sharedMesh=triangleMesh;var triangle=detail.GetComponent<MeshRenderer>();
            var occluder=GameObject.CreatePrimitive(PrimitiveType.Cube);occluder.name="Closer box must still occlude";var nearer=Streams(cubeMesh,new Vector3(2,2,.5f),new Vector3(0,-28,0),new Color32(0,220,0,254),false);resources.Add(nearer);occluder.GetComponent<MeshFilter>().sharedMesh=nearer;var cover=occluder.GetComponent<MeshRenderer>();cover.enabled=false;
            var camera=new GameObject("Depth fixture camera").AddComponent<Camera>();camera.enabled=false;camera.scene=scene;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=Color.black;camera.orthographic=false;camera.fieldOfView=40;camera.nearClipPlane=.01f;camera.farClipPlane=1000;
            target=new RenderTexture(256,256,24,RenderTextureFormat.ARGB32){antiAliasing=1};target.Create();camera.targetTexture=target;
            foreach(string shaderName in new[]{"Hidden/MorphForge/DepthBaseline0151","MorphForge/ParticleBoxCatalogMetadataGrid"}){
                var shader=Shader.Find(shaderName);if(!shader)throw new Exception("Missing shader "+shaderName);
                var material=new Material(shader);resources.Add(material);material.SetFloat("_WireEncoding",6);material.SetFloat("_VoxelGrid",0);material.SetFloat("_CellSize",.25f);box.sharedMaterial=material;triangle.sharedMaterial=material;cover.sharedMaterial=material;
                foreach(float yaw in new[]{-65f,-40f,-15f,0f,15f,40f,65f})foreach(float distance in new[]{4f,40f})foreach(bool detailFirst in new[]{false,true}){
                    camera.transform.position=new Vector3(0,-28,0)+Quaternion.Euler(7,yaw,0)*Vector3.forward*distance;camera.transform.LookAt(new Vector3(0,-28,0));
                    box.sortingOrder=1;triangle.sortingOrder=detailFirst?0:2;
                    box.enabled=false;triangle.enabled=true;var reference=Render(camera,target);var red=reference.GetPixels32();UnityEngine.Object.DestroyImmediate(reference);
                    box.enabled=true;triangle.enabled=false;reference=Render(camera,target);var green=reference.GetPixels32();UnityEngine.Object.DestroyImmediate(reference);
                    triangle.enabled=true;var actual=Render(camera,target);var pixels=actual.GetPixels32();int interior=0,wrong=0;
                    for(int y=2;y<254;y++)for(int x=2;x<254;x++){
                        int i=y*256+x;bool inside=true;foreach(int offset in new[]{0,1,-1,256,-256,257,-257,255,-255})if(!Red(red[i+offset])||!Green(green[i+offset])){inside=false;break;}
                        if(inside){interior++;if(!Red(pixels[i]))wrong++;}
                    }
                    if(distance==4&&yaw==15&&!detailFirst)File.WriteAllBytes(Root+(shaderName.StartsWith("Hidden")?"/baseline.png":"/fixed.png"),actual.EncodeToPNG());UnityEngine.Object.DestroyImmediate(actual);
                    rows.Add(new JObject{["shader"]=shaderName,["yaw"]=yaw,["distance"]=distance,["detailFirst"]=detailFirst,["testedPixels"]=interior,["wrongPixels"]=wrong,["passed"]=interior>20&&wrong==0});
                }
                camera.transform.position=new Vector3(0,-28,4);camera.transform.LookAt(new Vector3(0,-28,0));box.enabled=false;triangle.enabled=true;cover.enabled=true;var hidden=Render(camera,target);int leaked=hidden.GetPixels32().Count(Red);UnityEngine.Object.DestroyImmediate(hidden);cover.enabled=false;
                rows.Add(new JObject{["shader"]=shaderName,["kind"]="closer-occluder",["leakedRedPixels"]=leaked,["passed"]=leaked==0});
                var errors=ShaderUtil.GetShaderMessages(shader).Where(m=>m.severity==UnityEditor.Rendering.ShaderCompilerMessageSeverity.Error).Select(m=>m.message).ToArray();if(errors.Length>0)throw new Exception(string.Join("\n",errors));
            }
            var fixedRows=rows.Where(r=>((string)r["shader"]).StartsWith("MorphForge/")).ToArray();var failedBaseline=rows.Count(r=>((string)r["shader"]).StartsWith("Hidden/")&&!(bool)r["passed"]);
            var report=new JObject{["success"]=fixedRows.All(r=>(bool)r["passed"])&&failedBaseline>0,["baselineFailedCases"]=failedBaseline,["fixedCases"]=fixedRows.Length,["checks"]=rows,["scope"]="GPU synthetic particle vertex streams in a temporary additive scene; old production shader negative control, fixed production shader, both draw orders, perspective views and a closer occluder. Not native particle birth, OSC, or VRChat multiplayer validation."};
            File.WriteAllText(Root+"/unity-depth.json",report.ToString());return new JObject{["success"]=report["success"],["baselineFailedCases"]=failedBaseline,["fixedCases"]=fixedRows.Length,["fixedFailures"]=new JArray(fixedRows.Where(r=>!(bool)r["passed"]))}.ToString();
        }finally{
            if(target){target.Release();UnityEngine.Object.DestroyImmediate(target);}foreach(var resource in resources)if(resource)UnityEngine.Object.DestroyImmediate(resource);
            SceneManager.SetActiveScene(original);EditorSceneManager.CloseScene(scene,true);
            foreach(var item in before){var existing=Enumerable.Range(0,SceneManager.sceneCount).Select(SceneManager.GetSceneAt).FirstOrDefault(s=>s.handle==item.Key);if(existing.isLoaded&&existing.isDirty!=item.Value)throw new Exception("Original scene dirty state changed");}
        }
    }
}
