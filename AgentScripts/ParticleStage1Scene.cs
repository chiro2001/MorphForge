using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;

public static class ParticleStage1Scene
{
    static float[] V(Vector3 p){return new[]{p.x,p.y,p.z};}
    static Transform Display()
    {
        var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().First(g=>g.name=="MorphForge Stage1 Gesture Manager");
        var m=gm.Module as ModuleVrc3;
        return m!=null?m.Avatar.transform.Find("MorphForgeStage1"):gm.settings.favourite.transform.Find("MorphForgeStage1");
    }
    public static string Inspect()
    {
        var root=Display();var ps=root.Find("Store").GetComponent<ParticleSystem>();
        var parts=new ParticleSystem.Particle[ps.main.maxParticles];int n=ps.GetParticles(parts);
        var sv=SceneView.lastActiveSceneView;
        var cameras=UnityEngine.Object.FindObjectsOfType<Camera>().Where(c=>c.cameraType==CameraType.Game);
        return Newtonsoft.Json.JsonConvert.SerializeObject(new{play=EditorApplication.isPlaying,scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene().path,
            rootPosition=V(root.position),rootScale=V(root.lossyScale),layer=ps.gameObject.layer,layerName=LayerMask.LayerToName(ps.gameObject.layer),visibleLayers=Tools.visibleLayers,
            sceneView=sv==null?null:new{sv.drawGizmos,particles=sv.sceneViewState.showParticleSystems,pivot=V(sv.pivot),size=sv.size,cullingMask=sv.camera.cullingMask,sv.in2DMode},
            hidden=SceneVisibilityManager.instance.IsHidden(ps.gameObject),count=n,
            particleWorldPositions=parts.Take(n).Select(p=>V(root.TransformPoint(p.position))).ToArray(),
            cameras=cameras.Select(c=>new{c.name,c.tag,c.enabled,c.cullingMask,c.orthographic,position=V(c.transform.position)}).ToArray(),
            exampleScenes=AssetDatabase.FindAssets("t:Scene").Select(AssetDatabase.GUIDToAssetPath).Where(p=>p.IndexOf("example",StringComparison.OrdinalIgnoreCase)>=0||p.IndexOf("sample",StringComparison.OrdinalIgnoreCase)>=0).ToArray()},Newtonsoft.Json.Formatting.Indented);
    }
    public static string Reveal()
    {
        var root=Display();var ps=root.Find("Store").GetComponent<ParticleSystem>();
        Tools.visibleLayers|=1<<ps.gameObject.layer;
        if(SceneVisibilityManager.instance.IsHidden(ps.gameObject))SceneVisibilityManager.instance.Show(ps.gameObject,true);
        var sv=SceneView.lastActiveSceneView??EditorWindow.GetWindow<SceneView>();
        sv.drawGizmos=true;sv.sceneViewState.showParticleSystems=true;sv.in2DMode=false;
        sv.LookAt(new Vector3(3,1.25f,0),Camera.main.transform.rotation,4.5f,false,true);
        Selection.activeGameObject=ps.gameObject;sv.Show();sv.Focus();sv.Repaint();
        return Inspect();
    }
    public static string InspectExample()
    {
        const string path="Packages/com.vrchat.avatars/Samples/Dynamics/Robot Avatar/Avatar Dynamics Robot Avatar PC.unity";
        var preview=EditorSceneManager.OpenScene(path,OpenSceneMode.Additive);
        try
        {
            var all=preview.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<Transform>(true)).ToArray();
            return Newtonsoft.Json.JsonConvert.SerializeObject(new{path,objects=all.Where(t=>t.GetComponent<Camera>()||t.GetComponent<Light>()||t.name.IndexOf("floor",StringComparison.OrdinalIgnoreCase)>=0||t.name.IndexOf("ground",StringComparison.OrdinalIgnoreCase)>=0).Select(t=>new{
                t.name,parent=t.parent?t.parent.name:"",position=V(t.position),scale=V(t.lossyScale),components=t.GetComponents<Component>().Where(c=>c).Select(c=>c.GetType().Name).ToArray(),
                camera=t.GetComponent<Camera>()?new{t.GetComponent<Camera>().fieldOfView,t.GetComponent<Camera>().orthographic,t.GetComponent<Camera>().nearClipPlane,t.GetComponent<Camera>().farClipPlane}:null}).ToArray()},Newtonsoft.Json.Formatting.Indented);
        }
        finally{EditorSceneManager.CloseScene(preview,true);}
    }
    public static string CleanupCaptureAssets()
    {
        foreach(var name in new[]{"observation-scene","observation-game"})
        {
            string path="Assets/Docs/Stage1/"+name+".png";
            if(AssetDatabase.LoadMainAssetAtPath(path))AssetDatabase.DeleteAsset(path);
        }
        foreach(var folder in new[]{"Assets/Docs/Stage1","Assets/Docs"})
            if(AssetDatabase.IsValidFolder(folder)&&!Directory.EnumerateFileSystemEntries(folder).Any())AssetDatabase.DeleteAsset(folder);
        return "Removed the two temporary capture assets; evidence is under project Docs/Stage1.";
    }
}
