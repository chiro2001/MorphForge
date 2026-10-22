using System;
using System.IO;
using System.Linq;
using System.Collections;
using System.Reflection;
using System.Threading.Tasks;
using System.Net.Sockets;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using UnityEngine.Animations;
using Unity.Profiling;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
using MorphForge.Editor;
using MorphForgeAvatar.Editor;

// Pipeline-only diagnostics. Every runtime mutation is confined to an owned
// copy of the source scene and restored; no packets are sent to VRChat.
public static class Optimization20261023 {
    const string Reports="Docs/Optimization20261023";
    const string TestScene="Assets/MorphForge/Validation/Optimization20261023/Baseline.unity";
    static string Save(string name,JObject data) {Directory.CreateDirectory(Reports);File.WriteAllText(Reports+"/"+name+".json",data.ToString());return data.ToString();}
    public static string Snapshot() {
        var scene=SceneManager.GetActiveScene();
        var paths=AssetDatabase.GetDependencies(Mf6AvatarBuild.AvatarPath,true);
        return Save("baseline-source",new JObject {
            ["scene"]=scene.path,["dirty"]=scene.isDirty,["playing"]=EditorApplication.isPlaying,
            ["prefabHash"]=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString(),
            ["publicHash"]=AssetDatabase.GetAssetDependencyHash(AvatarSetupWindow.PrefabPath).ToString(),
            ["characterPrefabs"]=new JArray(paths.Where(p=>p.StartsWith("Assets/Nemesis/")&&p.EndsWith(".prefab")).Select(p=>(JToken)new JObject{["path"]=p,["hash"]=AssetDatabase.GetAssetDependencyHash(p).ToString()}))
        });
    }
    public static string Prepare() {
        if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");
        for(int i=0;i<SceneManager.sceneCount;i++)if(SceneManager.GetSceneAt(i).isDirty)throw new Exception("User unsaved scene preserved");
        var scene=SceneManager.GetActiveScene();if(scene.path!=Mf6AvatarBuild.ScenePath)throw new Exception("Unexpected source scene");
        if(!File.Exists(Reports+"/baseline-source.json"))Snapshot();CatalogVariant.EnsureFolder(Path.GetDirectoryName(TestScene).Replace('\\','/'));
        if(!File.Exists(TestScene)&&!EditorSceneManager.SaveScene(scene,TestScene,true))throw new Exception("Could not copy source scene");
        EditorSceneManager.OpenScene(TestScene,OpenSceneMode.Single);
        return new JObject{["success"]=true,["scene"]=TestScene}.ToString();
    }
    public static string Rebuild() {
        if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");
        var scene=SceneManager.GetActiveScene();if(scene.path!=Mf6AvatarBuild.ScenePath||scene.isDirty)throw new Exception("Clean saved source required");
        var before=JObject.Parse(File.ReadAllText(Reports+"/baseline-source.json"));
        foreach(var row in before["characterPrefabs"])if(AssetDatabase.GetAssetDependencyHash((string)row["path"]).ToString()!=(string)row["hash"])throw new Exception("Original character dependency changed");
        const string backup="Assets/MorphForge/Validation/Optimization20261023/Original";
        if(!AssetDatabase.IsValidFolder(backup)) {
            CatalogVariant.EnsureFolder(backup);
            foreach(string path in new[]{Mf6TextBuild.Root,Mf6MotionBuild.Root})if(!AssetDatabase.CopyAsset(path,backup+"/"+Path.GetFileName(path)))throw new Exception("Generated asset backup failed");
        }
        Mf6TextBuild.BuildAssets();var motion=JObject.Parse(Mf6MotionBuild.Build());AssetDatabase.SaveAssets();
        if(scene.isDirty)EditorSceneManager.SaveScene(scene);
        return Save("rebuild",new JObject{["success"]=true,["motion"]=motion,["prefabHash"]=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString(),["charactersUnchanged"]=true});
    }
    public static string FinishRebuildScene() {
        if(EditorApplication.isPlaying)throw new Exception("Edit Mode required");
        var scene=SceneManager.GetActiveScene();if(scene.path!=Mf6AvatarBuild.ScenePath||!File.Exists(Reports+"/rebuild.json"))throw new Exception("Owned rebuild source required");
        var before=JObject.Parse(File.ReadAllText(Reports+"/baseline-source.json"));
        foreach(var row in before["characterPrefabs"])if(AssetDatabase.GetAssetDependencyHash((string)row["path"]).ToString()!=(string)row["hash"])throw new Exception("Original character dependency changed");
        if(scene.isDirty){string copy="Assets/MorphForge/Validation/Optimization20261023/AfterRebuild.unity";if(!EditorSceneManager.SaveScene(scene,copy,true)||!EditorSceneManager.SaveScene(scene))throw new Exception("Could not preserve rebuild scene");}
        return new JObject{["success"]=!scene.isDirty,["scene"]=scene.path}.ToString();
    }
    public static string InstallDrawings() {
        if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");
        var scene=SceneManager.GetActiveScene();if(scene.path!=Mf6AvatarBuild.ScenePath||scene.isDirty)throw new Exception("Clean saved source required");
        var rows=new JArray();
        foreach(string path in new[]{Mf6AvatarBuild.AvatarPath,AvatarSetupWindow.PrefabPath}) {
            var root=PrefabUtility.LoadPrefabContents(path);
            try {
                bool shared=path==AvatarSetupWindow.PrefabPath;var module=shared?root:root.transform.Find("MorphForgePlacement").gameObject;
                var decoder=module.GetComponentsInChildren<nadena.dev.modular_avatar.core.ModularAvatarMergeAnimator>(true).Single(m=>m.gameObject.name=="MF6Decoder");
                Mf6DecoderBuild.ExtendDrawings((UnityEditor.Animations.AnimatorController)decoder.animator);
                var catalogPath=shared?AvatarSetupWindow.Root+"/CatalogSource/catalog.json":"Docs/MF6/avatar-catalog.json";
                var catalog=JObject.Parse(File.ReadAllText(catalogPath));var caps=(JArray)catalog["capabilities"];foreach(string cap in new[]{"mf6-drawing/1","mf6-motion-session/1"})if(!caps.Any(c=>(string)c==cap))caps.Add(cap);
                File.WriteAllText(catalogPath,catalog.ToString());
                if(shared){var identity=CatalogIdentity.Apply(module,catalog);root.GetComponent<MorphForgeAvatar.MorphForgeAvatarModule>().catalogIdentity=identity;}else AvatarCapabilitiesBuild.Apply(root,catalog);
                PrefabUtility.SaveAsPrefabAsset(root,path);rows.Add(new JObject{["path"]=path,["capability"]="mf6-drawing/1"});
            }finally{PrefabUtility.UnloadPrefabContents(root);}
        }
        AssetDatabase.SaveAssets();EditorSceneManager.MarkSceneDirty(scene);EditorSceneManager.SaveScene(scene);
        return Save("drawing-install",new JObject{["success"]=true,["assets"]=rows,["additionalSyncedBits"]=0});
    }
    public static string OptimizeGeometry() {
        if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");
        var scene=SceneManager.GetActiveScene();if(scene.path!=Mf6AvatarBuild.ScenePath||scene.isDirty)throw new Exception("Clean saved source required");
        var paths=AssetDatabase.GetDependencies(Mf6AvatarBuild.AvatarPath,true).Concat(AssetDatabase.GetDependencies(AvatarSetupWindow.PrefabPath,true)).Where(p=>p.EndsWith(".controller")).Distinct();
        var rows=new JArray();foreach(string path in paths){var controller=AssetDatabase.LoadAssetAtPath<UnityEditor.Animations.AnimatorController>(path);if(controller&&Mf6NativeBuild.OptimizeIdle(controller)>0)rows.Add(path);}
        AssetDatabase.SaveAssets();if(scene.isDirty)EditorSceneManager.SaveScene(scene);
        return Save("idle-geometry",new JObject{["success"]=true,["changedControllers"]=rows,["additionalParameters"]=0,["storagePreserved"]=true});
    }
    static ModuleVrc3 Module() {return UnityEngine.Object.FindObjectsOfType<GestureManager>().Where(g=>g.gameObject.scene.path==TestScene).Select(g=>g.Module as ModuleVrc3).Single(m=>m!=null);}
    static float Value(Renderer r,string property){var block=new MaterialPropertyBlock();r.GetPropertyBlock(block);return block.HasFloat(property)?block.GetFloat(property):r.sharedMaterial.GetFloat(property);}
    public static async Task<string> RestartMotionSession() {
        if(!EditorApplication.isPlaying||SceneManager.GetActiveScene().path!=TestScene)throw new Exception("Owned diagnostic Play Mode required");
        var m=Module();var fixture=JObject.Parse(File.ReadAllText("E:/tmp/morphforge/avatar-motion-fixtures.json"));
        string Geometry()=>string.Join("|",m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Where(p=>p.name=="Store").SelectMany(ps=>{var values=new ParticleSystem.Particle[ps.main.maxParticles];int n=ps.GetParticles(values);return values.Take(n).Where(p=>p.startColor.a>0).Select(p=>ps.transform.parent.name+":"+p.position.ToString("R")+":"+p.startSize3D.ToString("R")+":"+p.startColor);}).OrderBy(v=>v));
        var renderer=m.Avatar.GetComponentsInChildren<ParticleSystemRenderer>(true).Single(r=>r.name=="Store"&&r.transform.parent.name=="Pool0");
        using(var udp=new UdpClient()){
            async Task Send(JToken packet){var bytes=packet.Select(v=>(byte)(int)v).ToArray();udp.Send(bytes,bytes.Length,"127.0.0.1",9012);await Task.Delay(450);}
            foreach(var p in fixture["packets"])await Send(p["packet"]);
            var before=Geometry();bool collidedBaseline=m.GetParam("MF6_M1_AppliedTicket").IntValue()==1&&m.GetParam("MF6_M1_B0_Version").IntValue()==0&&m.GetParam("MF6_M1_Rate").FloatValue()==0;
            foreach(var p in fixture["restart"])await Send(p["packet"]);
            float a=Value(renderer,"_MF_M1_Elapsed");await Task.Delay(550);float b=Value(renderer,"_MF_M1_Elapsed");bool restarted=b-a>.09&&m.GetParam("MF6_M1_AppliedTicket").IntValue()==1;
            var watch=System.Diagnostics.Stopwatch.StartNew();float phase=Value(renderer,"_MF_M1_Elapsed");foreach(var p in fixture["restartReplay"])await Send(p["packet"]);float actual=Value(renderer,"_MF_M1_Elapsed"),expected=Mathf.Repeat(phase+(float)watch.Elapsed.TotalSeconds*.25f,1);float error=Mathf.Abs(Mathf.DeltaAngle(actual*360,expected*360))/360;
            return Save("motion-session-restart",new JObject{["success"]=collidedBaseline&&restarted&&before==Geometry()&&error<.08,["sameInitialVersionAndTicket"]=collidedBaseline,["newSessionClockAdvances"]=restarted,["elapsed"]=new JArray(a,b),["geometryUnchanged"]=before==Geometry(),["replayPhaseError"]=error,["scope"]="Cold Desktop Session default version0/ticket1 against already-used native Avatar version0/ticket1. Motion-only reset preserves stored geometry; routine replay preserves automatic clock."});
        }
    }
    public static Task<string> Continuous()=>RunContinuous(false);
    public static Task<string> ContinuousPaced()=>RunContinuous(true);
    static async Task<string> RunContinuous(bool paced) {
        if(!EditorApplication.isPlaying||SceneManager.GetActiveScene().path!=TestScene)throw new Exception("Owned diagnostic Play Mode required");
        var m=Module();var fixture=JObject.Parse(File.ReadAllText("E:/tmp/morphforge/avatar-motion-fixtures.json"));
        var receiver=UnityEngine.Object.FindObjectsOfType<MorphForgePlayReceiver>().Single(r=>r.gameObject.scene.path==TestScene);if(!receiver.Running)receiver.TryStart();if(!receiver.Running)throw new Exception(receiver.Error);
        var system=m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Single(p=>p.name=="Store"&&p.transform.parent.name=="Pool0");var renderer=system.GetComponent<ParticleSystemRenderer>();var root=system.main.customSimulationSpace;
        int oldLayer=renderer.gameObject.layer;var camera=new GameObject("Continuous native playback proof").AddComponent<Camera>();camera.enabled=false;camera.cullingMask=1<<31;camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=Color.black;camera.orthographic=true;camera.orthographicSize=8*root.lossyScale.y;camera.aspect=1;camera.nearClipPlane=.001f;camera.farClipPlane=20;
        var center=root.TransformPoint(new Vector3(0,-86*.25f,0));camera.transform.position=center-root.forward*50*root.lossyScale.z;camera.transform.LookAt(center,root.up);
        var target=new RenderTexture(256,256,24,RenderTextureFormat.ARGB32);target.Create();camera.targetTexture=target;var image=new Texture2D(256,256,TextureFormat.RGB24,false);var rows=new JArray();
        using(var udp=new UdpClient())try {
            async Task Send(JToken packet){var bytes=packet.Select(v=>(byte)(int)v).ToArray();udp.Send(bytes,bytes.Length,"127.0.0.1",9012);await Task.Delay(450);}
            if(paced){
                var captured=JObject.Parse(File.ReadAllText("E:/tmp/morphforge/paced-motion-fixtures.json"));
                await Send(fixture["packets"][0]["packet"]);
                var clock=System.Diagnostics.Stopwatch.StartNew();
                foreach(var packet in captured["packets"]){int delay=(int)((double)packet["time"]*1000-clock.Elapsed.TotalMilliseconds);if(delay>0)await Task.Delay(delay);var bytes=packet["packet"].Select(v=>(byte)(int)v).ToArray();udp.Send(bytes,bytes.Length,"127.0.0.1",9012);}
                await Task.Delay(350);
            }else{
                foreach(var packet in fixture["packets"])await Send(packet["packet"]);
                await Send(fixture["controls"]["zero"]);await Send(fixture["controls"]["play"]);
            }
            renderer.gameObject.layer=31;var watch=System.Diagnostics.Stopwatch.StartNew();float previous=-1;int wraps=0;
            while(watch.Elapsed.TotalSeconds<8.6) {
                float elapsed=Value(renderer,"_MF_M1_Elapsed");if(previous>=0&&elapsed<previous-.5f)wraps++;previous=elapsed;
                camera.Render();var active=RenderTexture.active;RenderTexture.active=target;image.ReadPixels(new Rect(0,0,256,256),0,0);image.Apply();RenderTexture.active=active;
                var colors=image.GetPixels32();var pixels=Enumerable.Range(0,colors.Length).Where(i=>colors[i].r>50&&colors[i].r>colors[i].g*1.3f).ToArray();int width=pixels.Length==0?0:pixels.Max(i=>i%256)-pixels.Min(i=>i%256)+1;
                rows.Add(new JObject{["seconds"]=watch.Elapsed.TotalSeconds,["elapsed"]=elapsed,["width"]=width,["ink"]=pixels.Length});
                if(rows.Count%5==1)File.WriteAllBytes(Reports+"/continuous-"+rows.Count.ToString("D3")+".png",image.EncodeToPNG());
                await Task.Delay(120);
            }
            await Send(fixture["controls"]["pause"]);float pause=Value(renderer,"_MF_M1_Elapsed");await Task.Delay(400);bool paused=Mathf.Abs(Value(renderer,"_MF_M1_Elapsed")-pause)<.001;
            await Send(fixture["controls"]["resume"]);float resume=Value(renderer,"_MF_M1_Elapsed");await Task.Delay(350);bool resumed=Mathf.Abs(Value(renderer,"_MF_M1_Elapsed")-resume)>.03;
            await Send(fixture["controls"]["stop"]);bool stopped=Value(renderer,"_MF_M1_Enabled")<.01;
            int distinct=rows.Select(r=>(int)r["width"]).Distinct().Count();int max=rows.Max(r=>(int)r["width"]),min=rows.Min(r=>(int)r["width"]);
            return Save(paced?"continuous-paced":"continuous-play",new JObject{["success"]=wraps>=2&&distinct>=6&&max-min>20&&paused&&resumed&&stopped,["wraps"]=wraps,["distinctWidths"]=distinct,["minWidth"]=min,["maxWidth"]=max,["pause"]=paused,["resume"]=resumed,["stop"]=stopped,["samples"]=rows,["actualPacedWriter"]=paced,["scope"]="Rust production OSC packets -> native merged Animator -> freely advancing clock -> production particle shader GPU, sampled over two full cycles. No manual phase or material writes. Editor only."});
        } finally {renderer.gameObject.layer=oldLayer;camera.targetTexture=null;target.Release();UnityEngine.Object.DestroyImmediate(target);UnityEngine.Object.DestroyImmediate(image);UnityEngine.Object.DestroyImmediate(camera.gameObject);}
    }
    public static async Task<string> Benchmark(string label) {
        if(!EditorApplication.isPlaying||SceneManager.GetActiveScene().path!=TestScene)throw new Exception("Owned diagnostic Play Mode required");
        for(int i=0;i<150;i++){if(UnityEngine.Object.FindObjectsOfType<GestureManager>().Any(g=>g.Module is ModuleVrc3))break;await Task.Delay(100);}
        var m=Module();var flags=BindingFlags.Public|BindingFlags.NonPublic|BindingFlags.Instance;
        var layers=(IDictionary)m.GetType().GetField("_layers",flags).GetValue(m);var key=layers.Keys.Cast<object>().Single(k=>k.ToString()=="FX");
        var fx=(AnimatorControllerPlayable)layers[key].GetType().GetField("Playable",flags).GetValue(layers[key]);
        var names=Enumerable.Range(0,fx.GetLayerCount()).Select(fx.GetLayerName).ToArray();var weights=Enumerable.Range(0,names.Length).Select(fx.GetLayerWeight).ToArray();
        var text=m.Avatar.GetComponentsInChildren<MeshRenderer>(true).Where(r=>r.name.StartsWith("Panel")&&r.transform.parent.name=="TextDisplay").ToArray();var textEnabled=text.Select(r=>r.enabled).ToArray();
        var stores=m.Avatar.GetComponentsInChildren<ParticleSystem>(true).Where(p=>p.name=="Store"&&p.transform.parent.name.StartsWith("Pool")).ToArray();
        var rows=new JArray();
        try {foreach(string phase in new[]{"baseline","motion layers off","text layers off","text and motion off","text renderers off","baseline repeat"}) {
            for(int i=0;i<names.Length;i++){bool motion=names[i].IndexOf("Motion",StringComparison.OrdinalIgnoreCase)>=0;bool txt=names[i].IndexOf("Text",StringComparison.OrdinalIgnoreCase)>=0;bool disable=phase=="motion layers off"&&motion||phase=="text layers off"&&txt||phase=="text and motion off"&&(motion||txt);fx.SetLayerWeight(i,disable?0:weights[i]);}
            for(int i=0;i<text.Length;i++)text[i].enabled=phase=="text renderers off"?false:textEnabled[i];
            await Task.Delay(700);
            using(var recorder=ProfilerRecorder.StartNew(ProfilerCategory.Animation,"Animators.Update",256)) {
                await Task.Delay(2200);var values=Enumerable.Range(0,recorder.Count).Select(i=>recorder.GetSample(i).Value).Where(v=>v>0).OrderBy(v=>v).ToArray();
                rows.Add(new JObject{["phase"]=phase,["samples"]=values.Length,["animatorMeanMs"]=values.Length==0?0:values.Average()/1e6,["animatorP95Ms"]=values.Length==0?0:values[(int)((values.Length-1)*.95)]/1e6,
                    ["particleCounts"]=new JArray(stores.Select(p=>(JToken)new JObject{["pool"]=p.transform.parent.name,["alive"]=p.particleCount,["capacity"]=p.main.maxParticles})),
                    ["cameraActive"]=m.Avatar.GetComponentsInChildren<Camera>(true).Any(c=>c.isActiveAndEnabled),["visibleTextRenderers"]=text.Count(r=>r.isVisible&&r.enabled)});
                Save(label+"-performance-progress",new JObject{["rows"]=rows});
            }
        }} finally {for(int i=0;i<names.Length;i++)fx.SetLayerWeight(i,weights[i]);for(int i=0;i<text.Length;i++)text[i].enabled=textEnabled[i];}
        return Save(label+"-performance",new JObject{["success"]=rows.All(r=>(int)r["samples"]>0),["scene"]=TestScene,["layers"]=new JArray(names),["rows"]=rows,["scope"]="Paired whole-Avatar Editor Animator samples in a fresh empty owned scene. No VRChat FPS claim; no user display or source assets modified."});
    }
    public static string Restore() {
        if(EditorApplication.isPlaying)throw new Exception("Stop owned Play Mode first");
        var scene=SceneManager.GetActiveScene();if(scene.path!=TestScene)throw new Exception("Unexpected scene preserved");
        if(scene.isDirty){string snapshot=Path.GetDirectoryName(TestScene).Replace('\\','/')+"/AfterPlay-"+DateTime.UtcNow.ToString("HHmmss")+".unity";if(!EditorSceneManager.SaveScene(scene,snapshot,true))throw new Exception("Could not preserve diagnostic changes");}
        EditorSceneManager.OpenScene(Mf6AvatarBuild.ScenePath,OpenSceneMode.Single);
        return new JObject{["success"]=true,["scene"]=SceneManager.GetActiveScene().path}.ToString();
    }
}
