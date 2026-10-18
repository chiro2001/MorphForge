using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase;
using nadena.dev.modular_avatar.core;
using BlackStartX.GestureManager;

namespace MorphForge.Editor
{
    public static class ParticleStage2Build
    {
        public const string Root="Assets/MorphForge/Stage2";
        public const string ScenePath="Assets/MorphForge/Scenes/MorphForge_Stage2.unity";
        public const string ModuleName="MorphForgeStage2";
        public const string Prefix="MF2_";
        public const int DataCount=10;
        static AnimatorController controller;
        static int clips;

        [MenuItem("Tools/MorphForge/Stage 2/Build particle display")]
        public static void MenuBuild(){Debug.Log(Build());}

        public static string Build()
        {
            if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new InvalidOperationException("Requires idle Edit Mode.");
            // Keep unrelated open scenes, including unsaved user changes, intact.
            var previous=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
            string blueprint="";
            if(previous.path==ParticleStage1Build.ScenePath||previous.path==ScenePath)
                blueprint=previous.GetRootGameObjects().Select(g=>g.GetComponent<VRC.Core.PipelineManager>()).Where(p=>p).Select(p=>p.blueprintId).FirstOrDefault()??"";
            if(string.IsNullOrEmpty(blueprint))
                for(int i=0;i<UnityEngine.SceneManagement.SceneManager.sceneCount;i++)
                {
                    var original=UnityEngine.SceneManagement.SceneManager.GetSceneAt(i);
                    if(original.path==ParticleStage1Build.ScenePath)
                        blueprint=original.GetRootGameObjects().Select(g=>g.GetComponent<VRC.Core.PipelineManager>()).Where(p=>p).Select(p=>p.blueprintId).FirstOrDefault()??"";
                }
            for(int i=UnityEngine.SceneManagement.SceneManager.sceneCount-1;i>=0;i--)
            {
                var existing=UnityEngine.SceneManagement.SceneManager.GetSceneAt(i);
                if(existing.path!=ScenePath)continue;
                if(existing.isDirty)throw new InvalidOperationException("Save edits to the existing Stage2 scene before rebuilding.");
                EditorSceneManager.CloseScene(existing,true);
            }
            var scene=EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Additive);
            UnityEngine.SceneManagement.SceneManager.SetActiveScene(scene);
            Folder(Root);Folder(Root+"/Clips");clips=0;
            var shader=Shader.Find("MorphForge/ParticleBoxLocalUV");
            if(!shader||!shader.isSupported)throw new InvalidOperationException("ParticleBox shader unavailable.");
            var material=new Material(shader){name="MorphForge box cells"};material.SetFloat("_CellSize",.25f);material=Asset(material,Root+"/Box.mat");
            var cubeTemp=GameObject.CreatePrimitive(PrimitiveType.Cube);
            var mesh=UnityEngine.Object.Instantiate(cubeTemp.GetComponent<MeshFilter>().sharedMesh);mesh.name="Preuploaded cube, 12 triangles";
            UnityEngine.Object.DestroyImmediate(cubeTemp);
            var faceUV=new Vector2[mesh.vertexCount];var tileUV=new Vector2[mesh.vertexCount];
            var vertices=mesh.vertices;var normals=mesh.normals;
            for(int i=0;i<vertices.Length;i++)
            {
                var v=vertices[i];var n=normals[i];int axis=Mathf.Abs(n.x)>.5f?0:Mathf.Abs(n.y)>.5f?1:2;
                tileUV[i]=axis==0?new Vector2(v.z+.5f,v.y+.5f):axis==1?new Vector2(v.x+.5f,v.z+.5f):new Vector2(v.x+.5f,v.y+.5f);
                faceUV[i]=new Vector2(axis,0);
            }
            mesh.uv=tileUV;mesh.uv2=faceUV;mesh=Asset(mesh,Root+"/Cube.asset");
            controller=AssetDatabase.LoadAssetAtPath<AnimatorController>(Root+"/FX.controller");
            if(!controller)controller=AnimatorController.CreateAnimatorControllerAtPath(Root+"/FX.controller");
            else
            {
                controller.layers=new AnimatorControllerLayer[0];controller.parameters=new AnimatorControllerParameter[0];
                foreach(var item in AssetDatabase.LoadAllAssetsAtPath(Root+"/FX.controller"))
                    if(item&&item!=controller)UnityEngine.Object.DestroyImmediate(item,true);
            }
            controller.layers=new AnimatorControllerLayer[0];
            controller.AddParameter(Prefix+"Ctl",AnimatorControllerParameterType.Int);
            for(int i=0;i<DataCount;i++)
            {
                controller.AddParameter(Prefix+"D"+i.ToString("D2"),AnimatorControllerParameterType.Int);
                var p=new AnimatorControllerParameter{name=Prefix+"W"+i.ToString("D2"),type=AnimatorControllerParameterType.Float,defaultFloat=i<3?128:(i==6?1:0)};
                controller.AddParameter(p);
            }
            controller.AddParameter(new AnimatorControllerParameter{name=Prefix+"LastSeq",type=AnimatorControllerParameterType.Int,defaultInt=255});
            controller.AddParameter(new AnimatorControllerParameter{name=Prefix+"LastRxSeq",type=AnimatorControllerParameterType.Int,defaultInt=255});
            controller.AddParameter(Prefix+"QSeq",AnimatorControllerParameterType.Int);
            controller.AddParameter(Prefix+"QOp",AnimatorControllerParameterType.Int);
            controller.AddParameter(Prefix+"QEraseMode",AnimatorControllerParameterType.Float);
            controller.AddParameter(Prefix+"InboxReady",AnimatorControllerParameterType.Bool);
            controller.AddParameter(Prefix+"RxCount",AnimatorControllerParameterType.Float);
            controller.AddParameter(Prefix+"DoneCount",AnimatorControllerParameterType.Float);
            for(int i=0;i<DataCount;i++)controller.AddParameter(Prefix+"Q"+i.ToString("D2"),AnimatorControllerParameterType.Float);
            controller.AddParameter(Prefix+"Op",AnimatorControllerParameterType.Int);
            controller.AddParameter(Prefix+"Busy",AnimatorControllerParameterType.Bool);
            controller.AddParameter(new AnimatorControllerParameter{name=Prefix+"One",type=AnimatorControllerParameterType.Float,defaultFloat=1});
            controller.AddParameter(Prefix+"EraseMode",AnimatorControllerParameterType.Float);

            var module=new GameObject(ModuleName);
            try
            {
                var min=new GameObject("Minimum");min.transform.SetParent(module.transform,false);
                var center=new GameObject("Center");center.transform.SetParent(min.transform,false);
                var eraser=new GameObject("Eraser",typeof(BoxCollider));eraser.transform.SetParent(center.transform,false);
                eraser.transform.localScale=Vector3.one*.025f;
                var collider=eraser.GetComponent<BoxCollider>();collider.isTrigger=true;
                var storeObject=new GameObject("Store",typeof(ParticleSystem));storeObject.transform.SetParent(module.transform,false);
                var store=storeObject.GetComponent<ParticleSystem>();store.Stop(true,ParticleSystemStopBehavior.StopEmittingAndClear);
                var main=store.main;main.playOnAwake=false;main.loop=false;main.duration=.1f;main.startLifetime=float.PositiveInfinity;main.startSpeed=0;
                main.startSize3D=true;main.startSizeX=.25f;main.startSizeY=.25f;main.startSizeZ=.25f;
                main.startColor=new Color(1f/63,0,0,0);main.maxParticles=64;main.simulationSpace=ParticleSystemSimulationSpace.Custom;main.customSimulationSpace=module.transform;
                main.scalingMode=ParticleSystemScalingMode.Local;main.ringBufferMode=ParticleSystemRingBufferMode.Disabled;
                main.cullingMode=ParticleSystemCullingMode.AlwaysSimulate;
                var em=store.emission;em.enabled=true;em.rateOverTime=0;em.rateOverDistance=0;em.SetBursts(new[]{new ParticleSystem.Burst(0,1)});
                var shape=store.shape;shape.enabled=false;
                var trigger=store.trigger;trigger.enabled=false;trigger.inside=ParticleSystemOverlapAction.Kill;
                trigger.outside=ParticleSystemOverlapAction.Ignore;trigger.enter=ParticleSystemOverlapAction.Ignore;trigger.exit=ParticleSystemOverlapAction.Ignore;
                trigger.radiusScale=.0001f;trigger.colliderQueryMode=ParticleSystemColliderQueryMode.Disabled;trigger.SetCollider(0,collider);
                var renderer=store.GetComponent<ParticleSystemRenderer>();renderer.renderMode=ParticleSystemRenderMode.Mesh;renderer.mesh=mesh;renderer.sharedMaterial=material;
                renderer.alignment=ParticleSystemRenderSpace.Local;
                // Unity 2022 exposes this Inspector option only through SerializedObject.
                var rendererSettings=new SerializedObject(renderer);
                var activeColorSpace=rendererSettings.FindProperty("m_ApplyActiveColorSpace");
                if(activeColorSpace==null)throw new InvalidOperationException("Particle metadata needs Apply Active Color Space disabled.");
                activeColorSpace.boolValue=false;rendererSettings.ApplyModifiedPropertiesWithoutUndo();
                renderer.enableGPUInstancing=false;renderer.shadowCastingMode=UnityEngine.Rendering.ShadowCastingMode.Off;renderer.receiveShadows=false;
                renderer.SetActiveVertexStreams(new List<ParticleSystemVertexStream>{ParticleSystemVertexStream.Position,ParticleSystemVertexStream.Normal,ParticleSystemVertexStream.Color,ParticleSystemVertexStream.UV,ParticleSystemVertexStream.UV2,ParticleSystemVertexStream.SizeXYZ});

                var pumpObject=new GameObject("Pump",typeof(ParticleSystem));pumpObject.transform.SetParent(center.transform,false);
                var pump=pumpObject.GetComponent<ParticleSystem>();pump.Stop(true,ParticleSystemStopBehavior.StopEmittingAndClear);
                var pm=pump.main;pm.playOnAwake=true;pm.loop=false;pm.duration=.1f;pm.startLifetime=.025f;pm.startSpeed=0;pm.maxParticles=1;
                pm.cullingMode=ParticleSystemCullingMode.AlwaysSimulate;
                var pe=pump.emission;pe.enabled=true;pe.rateOverTime=0;pe.SetBursts(new[]{new ParticleSystem.Burst(0,1)});
                var ps=pump.shape;ps.enabled=false;
                pump.GetComponent<ParticleSystemRenderer>().enabled=false;
                var sub=pump.subEmitters;sub.enabled=true;sub.AddSubEmitter(store,ParticleSystemSubEmitterType.Birth,ParticleSystemSubEmitterProperties.InheritNothing);
                pumpObject.SetActive(false);

                BuildDataDrive();
                BuildFlow();
                var merge=module.AddComponent<ModularAvatarMergeAnimator>();merge.animator=controller;merge.layerType=VRCAvatarDescriptor.AnimLayerType.FX;
                merge.pathMode=MergeAnimatorPathMode.Relative;merge.matchAvatarWriteDefaults=false;
                var mp=module.AddComponent<ModularAvatarParameters>();
                foreach(string name in new[]{Prefix+"Ctl"}.Concat(Enumerable.Range(0,DataCount).Select(i=>Prefix+"D"+i.ToString("D2"))))
                    mp.parameters.Add(new ParameterConfig{nameOrPrefix=name,syncType=ParameterSyncType.Int,localOnly=false,saved=false,hasExplicitDefaultValue=true,defaultValue=0,internalParameter=false});
                PrefabUtility.SaveAsPrefabAsset(module,Root+"/Display.prefab");
            }
            finally{UnityEngine.Object.DestroyImmediate(module);}
            EditorUtility.SetDirty(controller);AssetDatabase.SaveAssets();
            var source=AssetDatabase.LoadAssetAtPath<GameObject>("Assets/Nemesis/Nemesis.prefab");
            var avatar=(GameObject)PrefabUtility.InstantiatePrefab(source,scene);avatar.name="MorphForge_Stage2_Avatar";
            PrefabUtility.UnpackPrefabInstance(avatar,PrefabUnpackMode.Completely,InteractionMode.AutomatedAction);
            var pipeline=avatar.GetComponent<VRC.Core.PipelineManager>();if(pipeline)pipeline.blueprintId=blueprint;
            var installed=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(Root+"/Display.prefab"),avatar.transform);
            installed.name=ModuleName;installed.transform.localPosition=new Vector3(1,1,0);
            PrefabUtility.SaveAsPrefabAsset(avatar,Root+"/Avatar.prefab");
            var camera=new GameObject("Stage2 Camera").AddComponent<Camera>();camera.tag="MainCamera";
            camera.transform.position=new Vector3(6,4,8);camera.transform.LookAt(new Vector3(1,1,0));camera.nearClipPlane=.01f;
            camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=new Color(.035f,.05f,.08f);camera.gameObject.AddComponent<AudioListener>();
            var gmRoot=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>("Packages/vrchat.blackstartx.gesture-manager/GestureManager.prefab"),scene);
            gmRoot.name="MorphForge Stage2 Gesture Manager";gmRoot.tag="EditorOnly";
            var gm=gmRoot.GetComponent<GestureManager>();gm.settings.favourite=avatar.GetComponent<VRCAvatarDescriptor>();gm.settings.loadStored=false;gm.settings.isRemote=false;
            EditorSceneManager.SaveScene(scene,ScenePath);ConfigureObservation();AssetDatabase.SaveAssets();
            // Explicitly persist controller references after prefab overwrite/import.
            foreach(var path in new[]{Root+"/Display.prefab",Root+"/Avatar.prefab"})
            {
                AssetDatabase.ImportAsset(path,ImportAssetOptions.ForceUpdate);
                var saved=AssetDatabase.LoadAssetAtPath<GameObject>(path);
                foreach(var m in saved.GetComponentsInChildren<ModularAvatarMergeAnimator>(true))
                {
                    var serialized=new SerializedObject(m);serialized.FindProperty("animator").objectReferenceValue=controller;
                    serialized.ApplyModifiedPropertiesWithoutUndo();EditorUtility.SetDirty(m);
                }
                PrefabUtility.SavePrefabAsset(saved);
            }
            foreach(var m in avatar.GetComponentsInChildren<ModularAvatarMergeAnimator>(true))
            {m.animator=controller;EditorUtility.SetDirty(m);PrefabUtility.RecordPrefabInstancePropertyModifications(m);}
            EditorSceneManager.MarkSceneDirty(scene);EditorSceneManager.SaveScene(scene);AssetDatabase.SaveAssets();Selection.activeGameObject=gmRoot;
            Directory.CreateDirectory("Docs/Stage2");
            string json=Newtonsoft.Json.JsonConvert.SerializeObject(new {scene=ScenePath,controllerLayers=controller.layers.Length,clips,
                protocol="MF2/single-commit",sequenceBits=5,inboxCapacity=1,syncedIntParameters=11,addedSyncedBits=88,capacity=64,customAvatarRuntimeScripts=0,
                emission="Animator toggles sibling pump; native Birth sub-emitter emits into persistent Store",scope="Editor prototype, not VRChat validation"},Newtonsoft.Json.Formatting.Indented);
            File.WriteAllText("Docs/Stage2/build.json",json);return json;
        }

        public static string ConfigureObservation()
        {
            if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new InvalidOperationException("Observation setup requires Edit Mode.");
            var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
            if(scene.path!=ScenePath)throw new InvalidOperationException("Only the owned Stage2 scene may be configured.");
            const string example="Packages/com.vrchat.avatars/Samples/Dynamics/Robot Avatar/Avatar Dynamics Robot Avatar PC.unity";
            var old=scene.GetRootGameObjects().FirstOrDefault(g=>g.name=="Stage2 Observation");
            if(old)UnityEngine.Object.DestroyImmediate(old);
            var environment=new GameObject("Stage2 Observation");environment.tag="EditorOnly";
            var camera=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<Camera>(true)).FirstOrDefault(c=>c.CompareTag("MainCamera"));
            if(!camera)camera=new GameObject("Main Camera",typeof(Camera),typeof(AudioListener)).GetComponent<Camera>();
            var sample=EditorSceneManager.OpenScene(example,OpenSceneMode.Additive);
            try
            {
                var sampleCamera=sample.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<Camera>()).First();camera.CopyFrom(sampleCamera);
                var sampleLight=sample.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<Light>()).First();
                var lighting=UnityEngine.Object.Instantiate(sampleLight.gameObject);lighting.name="Directional Light";
                UnityEngine.SceneManagement.SceneManager.MoveGameObjectToScene(lighting,scene);lighting.transform.SetParent(environment.transform,true);
            }
            finally{EditorSceneManager.CloseScene(sample,true);}
            camera.name="Main Camera";camera.tag="MainCamera";camera.cullingMask=-1;camera.targetTexture=null;
            camera.nearClipPlane=.03f;camera.farClipPlane=200;camera.orthographic=false;camera.fieldOfView=60;
            camera.transform.position=new Vector3(6,3.5f,8);camera.transform.LookAt(new Vector3(3,1.25f,0));
            camera.clearFlags=CameraClearFlags.SolidColor;camera.backgroundColor=new Color(.06f,.08f,.11f);
            var floorMaterial=new Material(Shader.Find("Standard")){name="Observation ground",color=new Color(.23f,.25f,.28f)};
            floorMaterial.SetFloat("_Glossiness",0);floorMaterial=Asset(floorMaterial,Root+"/ObservationFloor.mat");
            var ground=GameObject.CreatePrimitive(PrimitiveType.Plane);ground.name="Ground (Y=0, 20m)";ground.transform.SetParent(environment.transform,false);
            ground.transform.position=new Vector3(3,-.01f,0);ground.transform.localScale=new Vector3(2,1,2);ground.GetComponent<Renderer>().sharedMaterial=floorMaterial;
            UnityEngine.Object.DestroyImmediate(ground.GetComponent<Collider>());
            for(int axis=0;axis<3;axis++)
            {
                var material=new Material(Shader.Find("Unlit/Color")){name="Observation "+"XYZ"[axis]};material.color=new[]{Color.red,Color.green,Color.blue}[axis];
                material=Asset(material,Root+"/ObservationAxis"+"XYZ"[axis]+".mat");
                foreach(bool display in new[]{false,true})
                {
                    var origin=display?new Vector3(1,1,0):Vector3.zero;
                    var bar=GameObject.CreatePrimitive(PrimitiveType.Cube);bar.name=(display?"Display origin (1,1,0) / ":"World origin / ")+"XYZ"[axis]+" axis 1m";
                    bar.transform.SetParent(environment.transform,false);var vector=axis==0?Vector3.right:axis==1?Vector3.up:Vector3.forward;
                    bar.transform.position=origin+vector*.5f;bar.transform.localScale=Vector3.one*.018f+vector*.982f;
                    bar.GetComponent<Renderer>().sharedMaterial=material;UnityEngine.Object.DestroyImmediate(bar.GetComponent<Collider>());
                }
            }
            Tools.visibleLayers|=1;
            var sv=SceneView.lastActiveSceneView??EditorWindow.GetWindow<SceneView>();sv.drawGizmos=true;sv.sceneViewState.showParticleSystems=true;sv.in2DMode=false;
            sv.LookAt(new Vector3(3,1.25f,0),camera.transform.rotation,5,false,true);sv.Repaint();
            EditorSceneManager.MarkSceneDirty(scene);EditorSceneManager.SaveScene(scene);AssetDatabase.SaveAssets();
            var report=Newtonsoft.Json.JsonConvert.SerializeObject(new{scene=ScenePath,referenceScene=example,mainCamera="Main Camera",fieldOfView=60,
                directionalLight=true,groundY=-.01f,displayOrigin=new[]{1,1,0},displayStoreLayer=0,cameraMask=camera.cullingMask,sceneGizmos=sv.drawGizmos},Newtonsoft.Json.Formatting.Indented);
            Directory.CreateDirectory("Docs/Stage2");File.WriteAllText("Docs/Stage2/observation-setup.json",report);return report;
        }

        struct Binding{public string path,property;public Type type;public float min,max;}
        static Binding Bind(string path,Type type,string property,float min,float max){return new Binding{path=path,type=type,property=property,min=min,max=max};}
        static T Asset<T>(T obj,string path) where T:UnityEngine.Object
        {
            var existing=AssetDatabase.LoadAssetAtPath<T>(path);
            if(existing){EditorUtility.CopySerialized(obj,existing);EditorUtility.SetDirty(existing);UnityEngine.Object.DestroyImmediate(obj);return existing;}
            AssetDatabase.CreateAsset(obj,path);return obj;
        }
        static void Folder(string path){string p="Assets";foreach(var n in path.Split('/').Skip(1)){string next=p+"/"+n;if(!AssetDatabase.IsValidFolder(next))AssetDatabase.CreateFolder(p,n);p=next;}}
        static AnimationClip Clip(string name,Binding[] bindings,bool max,float duration=.1f)
        {
            var clip=new AnimationClip{name=name,frameRate=60};
            foreach(var b in bindings)AnimationUtility.SetEditorCurve(clip,EditorCurveBinding.FloatCurve(b.path,b.type,b.property),AnimationCurve.Constant(0,duration,max?b.max:b.min));
            clip=Asset(clip,Root+"/Clips/"+name+".anim");clips++;return clip;
        }
        static AnimatorStateMachine Layer(string name)
        {
            var sm=new AnimatorStateMachine{name=name};AssetDatabase.AddObjectToAsset(sm,controller);
            controller.AddLayer(new AnimatorControllerLayer{name=name,stateMachine=sm,defaultWeight=1});return sm;
        }
        static BlendTree Tree(string name,string parameter,float min,float max,Binding[] bindings)
        {
            var a=Clip(name+"_Min",bindings,false);var b=Clip(name+"_Max",bindings,true);
            var bt=new BlendTree{name=name,blendType=BlendTreeType.Simple1D,blendParameter=parameter,useAutomaticThresholds=false};
            bt.AddChild(a,min);bt.AddChild(b,max);AssetDatabase.AddObjectToAsset(bt,controller);return bt;
        }
        static AnimatorState State(AnimatorStateMachine sm,string name,Motion motion)
        {
            var s=sm.AddState(name);if(!AssetDatabase.Contains(s))AssetDatabase.AddObjectToAsset(s,controller);
            s.motion=motion;s.writeDefaultValues=false;return s;
        }
        static void Linear(string name,string parameter,float min,float max,Binding[] bindings)
        {var sm=Layer(name);State(sm,"Drive",Tree(name,parameter,min,max,bindings));}
        static Binding[] dataBindings;
        static AnimationClip Basis(string name,Binding[] changed,bool maximum)
        {
            var all=dataBindings.Select(b=>Bind(b.path,b.type,b.property,0,0)).ToArray();
            foreach(var c in changed)
            {
                int i=Array.FindIndex(all,b=>b.path==c.path&&b.type==c.type&&b.property==c.property);
                if(i<0)throw new InvalidOperationException("Unknown data binding "+c.property);
                float v=maximum?c.max:c.min;all[i]=Bind(c.path,c.type,c.property,v,v);
            }
            return Clip(name,all,false);
        }
        static BlendTree BasisTree(string name,string parameter,float min,float max,Binding[] bindings)
        {
            var bt=new BlendTree{name=name,blendType=BlendTreeType.Simple1D,blendParameter=parameter,useAutomaticThresholds=false};
            bt.AddChild(Basis(name+"_Min",bindings,false),min);bt.AddChild(Basis(name+"_Max",bindings,true),max);
            AssetDatabase.AddObjectToAsset(bt,controller);return bt;
        }
        static void BuildDataDrive()
        {
            var bindings=new List<Binding>();
            for(int a=0;a<3;a++)
            {
                string axis="xyz"[a].ToString(),size=a==0?"startSize":a==1?"startSizeY":"startSizeZ";
                bindings.Add(Bind("Minimum",typeof(Transform),"m_LocalPosition."+axis,0,0));
                bindings.Add(Bind("Minimum/Center",typeof(Transform),"m_LocalPosition."+axis,0,0));
                bindings.Add(Bind("Minimum/Center/Eraser",typeof(Transform),"m_LocalScale."+axis,0,0));
                bindings.Add(Bind("Store",typeof(ParticleSystem),"InitialModule."+size+".scalar",0,0));
                bindings.Add(Bind("Store",typeof(ParticleSystem),"InitialModule."+size+".minScalar",0,0));
            }
            foreach(char c in "rgba")foreach(string kind in new[]{"minColor","maxColor"})bindings.Add(Bind("Store",typeof(ParticleSystem),"InitialModule.startColor."+kind+"."+c,0,0));
            dataBindings=bindings.ToArray();
            var direct=new BlendTree{name="Complete vector output",blendType=BlendTreeType.Direct};AssetDatabase.AddObjectToAsset(direct,controller);
            var children=new List<Motion>();
            for(int a=0;a<3;a++)
            {
                string axis="xyz"[a].ToString(),size=a==0?"startSize":a==1?"startSizeY":"startSizeZ";
                children.Add(BasisTree("Position"+axis,Prefix+"W"+a.ToString("D2"),0,255,new[]{Bind("Minimum",typeof(Transform),"m_LocalPosition."+axis,-32,31.75f)}));
                children.Add(BasisTree("Size"+axis,Prefix+"W"+(a+3).ToString("D2"),0,255,new[]{
                    Bind("Minimum/Center",typeof(Transform),"m_LocalPosition."+axis,.125f,32),
                    Bind("Store",typeof(ParticleSystem),"InitialModule."+size+".scalar",.25f,64),
                    Bind("Store",typeof(ParticleSystem),"InitialModule."+size+".minScalar",.25f,64)}));
                var eraser=Bind("Minimum/Center/Eraser",typeof(Transform),"m_LocalScale."+axis,.25f,64);
                var select=new BlendTree{name="Eraser select "+axis,blendType=BlendTreeType.Simple1D,blendParameter=Prefix+"EraseMode",useAutomaticThresholds=false};
                select.AddChild(Basis("EraserPoint"+axis,new[]{Bind(eraser.path,eraser.type,eraser.property,.025f,.025f)},false),0);
                select.AddChild(BasisTree("EraserRegion"+axis,Prefix+"W"+(a+3).ToString("D2"),0,255,new[]{eraser}),1);
                select.AddChild(Basis("EraserClear"+axis,new[]{Bind(eraser.path,eraser.type,eraser.property,128,128)},false),2);
                AssetDatabase.AddObjectToAsset(select,controller);children.Add(select);
            }
            for(int a=0;a<4;a++)
            {
                string c="rgba"[a].ToString();children.Add(BasisTree("BirthColor"+c,Prefix+"W"+(a+6).ToString("D2"),0,63,new[]{
                    Bind("Store",typeof(ParticleSystem),"InitialModule.startColor.minColor."+c,0,1),
                    Bind("Store",typeof(ParticleSystem),"InitialModule.startColor.maxColor."+c,0,1)}));
            }
            foreach(var child in children)direct.AddChild(child);
            var arr=direct.children;for(int i=0;i<arr.Length;i++)arr[i].directBlendParameter=Prefix+"One";direct.children=arr;
            var serialized=new SerializedObject(direct);var normalized=serialized.FindProperty("m_NormalizedBlendValues");if(normalized!=null){normalized.boolValue=false;serialized.ApplyModifiedPropertiesWithoutUndo();}
            var sm=Layer("Shared complete data output");var state=State(sm,"Drive",direct);state.writeDefaultValues=true;
        }
        static AnimatorStateTransition Transition(AnimatorState a,AnimatorState b,bool exit=false)
        {var t=a.AddTransition(b);t.duration=0;t.hasExitTime=exit;t.exitTime=1;t.canTransitionToSelf=false;return t;}
        static void EraserLayer(string axis,int field)
        {
            var sm=Layer("EraseSize"+axis);var binding=Bind("Minimum/Center/Eraser",typeof(Transform),"m_LocalScale."+axis,.25f,64);
            var point=State(sm,"Point",Clip("EraserPoint"+axis,new[]{Bind(binding.path,binding.type,binding.property,.025f,.025f)},false));
            var region=State(sm,"Region",Tree("EraserRegion"+axis,Prefix+"W"+field.ToString("D2"),0,255,new[]{binding}));
            var clear=State(sm,"Clear",Clip("EraserClear"+axis,new[]{Bind(binding.path,binding.type,binding.property,128,128)},false));
            var t=Transition(point,region);t.AddCondition(AnimatorConditionMode.Equals,2,Prefix+"Op");
            t=Transition(point,clear);t.AddCondition(AnimatorConditionMode.Equals,4,Prefix+"Op");
            t=Transition(region,point);t.AddCondition(AnimatorConditionMode.NotEqual,2,Prefix+"Op");
            t=Transition(clear,point);t.AddCondition(AnimatorConditionMode.NotEqual,4,Prefix+"Op");
        }
        static AnimationClip FlowClip(string name,bool pump,bool trigger,float length)
        {return Clip(name,new[]{Bind("Minimum/Center/Pump",typeof(GameObject),"m_IsActive",pump?1:0,pump?1:0),Bind("Store",typeof(ParticleSystem),"TriggerModule.enabled",trigger?1:0,trigger?1:0)},false,length);}
        static VRCAvatarParameterDriver Driver(AnimatorState state)
        {
            var d=state.AddStateMachineBehaviour<VRCAvatarParameterDriver>();
            if(!d)throw new InvalidOperationException("Cannot attach native Parameter Driver to "+state.name);
            d.localOnly=false;return d;
        }
        static void Set(VRCAvatarParameterDriver d,string n,float v){d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=Prefix+n,type=VRC_AvatarParameterDriver.ChangeType.Set,value=v});}
        static void Copy(VRCAvatarParameterDriver d,string source,string dest){d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=Prefix+dest,source=Prefix+source,type=VRC_AvatarParameterDriver.ChangeType.Copy});}
        static void Increment(VRCAvatarParameterDriver d,string name)
        {d.parameters.Add(new VRC_AvatarParameterDriver.Parameter{name=Prefix+name,type=VRC_AvatarParameterDriver.ChangeType.Add,value=1});}
        static void BuildFlow()
        {
            // Local inbox is separate from the immutable work buffer. Network has one commit.
            var inbox=Layer("Single commit input inbox");
            var empty=Clip("InboxEmpty",new Binding[0],false,.001f);
            var wait=State(inbox,"Wait",empty);inbox.defaultState=wait;
            for(int seq=0;seq<32;seq++)for(int op=1;op<=4;op++)
            {
                var capture=State(inbox,"Capture_"+op+"_"+seq,empty);var d=Driver(capture);
                for(int i=0;i<DataCount;i++)Copy(d,"D"+i.ToString("D2"),"Q"+i.ToString("D2"));
                Set(d,"QSeq",seq);Set(d,"QOp",op);Set(d,"QEraseMode",op==2?1:op==4?2:0);Set(d,"LastRxSeq",seq);Set(d,"InboxReady",1);Increment(d,"RxCount");
                var t=inbox.AddAnyStateTransition(capture);t.duration=0;t.hasExitTime=false;t.canTransitionToSelf=false;
                t.AddCondition(AnimatorConditionMode.Equals,(seq<<3)|op,Prefix+"Ctl");
                t.AddCondition(AnimatorConditionMode.NotEqual,seq,Prefix+"LastRxSeq");
                t.AddCondition(AnimatorConditionMode.IfNot,0,Prefix+"InboxReady");
                Transition(capture,wait,true);
            }
            var sm=Layer("Frame driven native particle lifecycle");
            var idle=State(sm,"Idle",FlowClip("Idle",false,false,.001f));sm.defaultState=idle;
            var latch=State(sm,"Latch",FlowClip("Latch",false,false,.001f));var driver=Driver(latch);
            for(int i=0;i<DataCount;i++)Copy(driver,"Q"+i.ToString("D2"),"W"+i.ToString("D2"));
            Copy(driver,"QSeq","LastSeq");Copy(driver,"QOp","Op");Copy(driver,"QEraseMode","EraseMode");Set(driver,"Busy",1);Set(driver,"InboxReady",0);
            // Parameter Driver values reach the data blend tree on a later evaluation.
            // Allow the animated collider transform to cross a normal 20ms physics update.
            var settle=State(sm,"Settle",FlowClip("Settle",false,false,.03f));Transition(latch,settle,true);
            var erase=State(sm,"Erase",FlowClip("Erase",false,true,.001f));Transition(settle,erase,true);
            var release=State(sm,"Release",FlowClip("Release",false,false,.001f));
            var fire=State(sm,"Fire",FlowClip("Fire",true,false,.001f));
            var done=State(sm,"Done",FlowClip("Done",false,false,.001f));var finish=Driver(done);Set(finish,"Busy",0);Increment(finish,"DoneCount");
            Transition(erase,release,true);
            var branch=Transition(release,fire,true);branch.AddCondition(AnimatorConditionMode.Equals,1,Prefix+"Op");
            branch=Transition(release,done,true);branch.AddCondition(AnimatorConditionMode.NotEqual,1,Prefix+"Op");
            Transition(fire,done,true);Transition(done,idle,true);
            var execute=sm.AddAnyStateTransition(latch);execute.duration=0;execute.hasExitTime=false;execute.canTransitionToSelf=false;
            execute.AddCondition(AnimatorConditionMode.If,0,Prefix+"InboxReady");execute.AddCondition(AnimatorConditionMode.IfNot,0,Prefix+"Busy");
        }
    }
}
