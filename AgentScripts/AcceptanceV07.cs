using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using UnityEngine;
using UnityEditor;
using UnityEditor.SceneManagement;
using Newtonsoft.Json.Linq;
using MorphForge.Editor;

public static class AcceptanceV07
{
    const string Output = "Assets/MorphForge/Diagnostics/AcceptanceV07";
    static Color32 Background = new Color32(16,25,35,255);
    public static string Run(string sourceRoot)
    {
        if (EditorApplication.isPlaying) throw new Exception("Edit Mode required.");
        foreach(var path in new[]{"Assets/MorphForge/Shaders/ParticleBoxCatalog.shader","Assets/MorphForge/Shaders/ParticleBoxCatalogMetadataGrid.shader"})
            AssetDatabase.ImportAsset(path,ImportAssetOptions.ForceUpdate);
        var productionGuid = AssetDatabase.AssetPathToGUID(CatalogMaterialBuild.MaterialAssetPath);
        var build = JObject.Parse(CatalogMaterialBuild.BuildAt(sourceRoot,Output));
        var material=AssetDatabase.LoadAssetAtPath<Material>(Output+"/ParticleBoxCatalog.mat");
        var array=AssetDatabase.LoadAssetAtPath<Texture2DArray>(Output+"/CatalogLayers.asset");
        if(!material || !array || array.mipmapCount<2) throw new Exception("Missing generated mipmapped layer asset.");
        var errors=ShaderUtil.GetShaderMessages(material.shader).Where(m=>m.severity==UnityEditor.Rendering.ShaderCompilerMessageSeverity.Error).Select(m=>m.message).ToArray();
        if(errors.Length!=0) throw new Exception(string.Join("\n",errors));
        var doc=JObject.Parse(File.ReadAllText(Path.Combine(sourceRoot,"catalog.json")));
        var materials=(JArray)doc["materials"];
        int stone=materials.First(m=>(string)m["uid"]=="mf-pixel:stone")["id"].Value<int>();
        int water=materials.First(m=>(string)m["uid"]=="mf-pixel:water")["id"].Value<int>();
        var preview=EditorSceneManager.NewPreviewScene();
        var directory=Path.Combine(Directory.GetParent(Application.dataPath).FullName,"Docs","quality-v7","unity");
        Directory.CreateDirectory(directory);
        var target=new RenderTexture(256,256,24,RenderTextureFormat.ARGB32){antiAliasing=4};target.Create();
        GameObject cube=null;Mesh mesh=null;
        try
        {
            cube=GameObject.CreatePrimitive(PrimitiveType.Cube);UnityEngine.SceneManagement.SceneManager.MoveGameObjectToScene(cube,preview);
            mesh=UnityEngine.Object.Instantiate(cube.GetComponent<MeshFilter>().sharedMesh);
            var vertices=mesh.vertices;var normals=mesh.normals;var uv=new List<Vector4>();var sizes=new List<Vector3>();
            for(int i=0;i<vertices.Length;i++){
                var v=vertices[i];var n=normals[i];int axis=Mathf.Abs(n.x)>.5f?0:(Mathf.Abs(n.y)>.5f?1:2);
                uv.Add(new Vector4(axis==0?v.z+.5f:v.x+.5f,axis==1?v.z+.5f:v.y+.5f,axis,0));sizes.Add(Vector3.one*.25f);
            }
            mesh.SetUVs(0,uv);mesh.SetUVs(1,sizes);cube.GetComponent<MeshFilter>().sharedMesh=mesh;cube.GetComponent<MeshRenderer>().sharedMaterial=material;
            var cameraObject=new GameObject("Acceptance camera");UnityEngine.SceneManagement.SceneManager.MoveGameObjectToScene(cameraObject,preview);
            var camera=cameraObject.AddComponent<Camera>();camera.scene=preview;camera.cameraType=CameraType.Preview;camera.enabled=false;camera.useOcclusionCulling=false;camera.targetTexture=target;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=Background;
            camera.orthographic=true;camera.orthographicSize=1.1f;camera.transform.position=new Vector3(3,2,4);camera.transform.LookAt(Vector3.zero);camera.nearClipPlane=.01f;camera.farClipPlane=30;
            SetCode(mesh,stone,0);var clean=Render(camera,target);File.WriteAllBytes(Path.Combine(directory,"pixel-stone.png"),clean.EncodeToPNG());
            var lightObject=new GameObject("Unrelated world light");UnityEngine.SceneManagement.SceneManager.MoveGameObjectToScene(lightObject,preview);var light=lightObject.AddComponent<Light>();light.type=LightType.Directional;light.intensity=12;light.color=Color.red;
            var red=Render(camera,target);light.color=Color.green;light.intensity=.1f;var green=Render(camera,target);
            var fixedLight=red.GetPixels32().SequenceEqual(green.GetPixels32());
            material.SetFloat("_VoxelGrid",1);var grid=Render(camera,target);material.SetFloat("_VoxelGrid",0);
            var optionalGrid=!clean.GetPixels32().SequenceEqual(grid.GetPixels32());
            SetCode(mesh,water,0);var open=Render(camera,target);File.WriteAllBytes(Path.Combine(directory,"water-surface.png"),open.EncodeToPNG());
            SetCode(mesh,water,63);var hidden=Render(camera,target);var pixels=hidden.GetPixels32();var center=pixels[128*256+128];
            bool hiddenWorks=Mathf.Abs(center.r-pixels[0].r)<3&&Mathf.Abs(center.g-pixels[0].g)<3&&Mathf.Abs(center.b-pixels[0].b)<3&&!open.GetPixels32().SequenceEqual(pixels);
            var originalShader=material.shader;
            var metadataGridShader=Shader.Find("MorphForge/ParticleBoxCatalogMetadataGrid");
            material.shader=metadataGridShader;SetCode(mesh,stone,0);var metadataGridPixels=Render(camera,target);
            bool metadataGridMatches=metadataGridPixels.GetPixels32().SequenceEqual(clean.GetPixels32());
            var metadataGridErrors=ShaderUtil.GetShaderMessages(metadataGridShader).Count(m=>m.severity==UnityEditor.Rendering.ShaderCompilerMessageSeverity.Error);
            UnityEngine.Object.DestroyImmediate(metadataGridPixels);material.shader=originalShader;
            errors=ShaderUtil.GetShaderMessages(material.shader).Where(m=>m.severity==UnityEditor.Rendering.ShaderCompilerMessageSeverity.Error).Select(m=>m.message).ToArray();
            var report=new JObject { ["success"]=errors.Length==0&&fixedLight&&optionalGrid&&hiddenWorks&&metadataGridMatches&&metadataGridErrors==0,["build"]=build,["shaderErrors"]=new JArray(errors),["metadataGridMatches"]=metadataGridMatches,["metadataGridShaderErrors"]=metadataGridErrors,["layerCount"]=array.depth,["mipmapCount"]=array.mipmapCount,["virtualLightIgnoresWorldLights"]=fixedLight,["gridIsOptional"]=optionalGrid,["transparentFaceMaskWorks"]=hiddenWorks,["productionMaterialGuidUnchanged"]=productionGuid==AssetDatabase.AssetPathToGUID(CatalogMaterialBuild.MaterialAssetPath),["output"]=Output,["scope"]="Isolated persisted material/array assets and actual camera pixels. No Avatar upload or OSC." };
            foreach(var t in new[]{clean,red,green,grid,open,hidden}) UnityEngine.Object.DestroyImmediate(t);
            File.WriteAllText(Path.Combine(directory,"validation.json"),report.ToString());
            if(!(bool)report["success"]) throw new Exception(report.ToString());
            return report.ToString();
        }
        finally { target.Release();UnityEngine.Object.DestroyImmediate(target);if(mesh)UnityEngine.Object.DestroyImmediate(mesh);EditorSceneManager.ClosePreviewScene(preview); }
    }
    static void SetCode(Mesh mesh,int style,int motion)
    {
        var color=new Color((style&63)/63f,((style>>6)&63)/63f,0,motion/63f);mesh.colors=Enumerable.Repeat(color,mesh.vertexCount).ToArray();
    }
    static Texture2D Render(Camera camera,RenderTexture target)
    {
        camera.Render();var previous=RenderTexture.active;RenderTexture.active=target;
        var image=new Texture2D(target.width,target.height,TextureFormat.RGBA32,false);image.ReadPixels(new Rect(0,0,target.width,target.height),0,0);image.Apply();RenderTexture.active=previous;return image;
    }
}
