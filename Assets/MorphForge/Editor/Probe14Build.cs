using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
using nadena.dev.modular_avatar.core;
using BlackStartX.GestureManager;

namespace MorphForge.Editor
{
    public static class Probe14Build
    {
        public const string Root = "Assets/MorphForge/Probe14";
        public const string ScenePath = "Assets/MorphForge/Scenes/MorphForge_Probe14.unity";
        public const string ModuleName = "MorphForgeProbe14";
        public const string Prefix = "MFProbe_";
        public const int Coordinates = 72;

        [MenuItem("Tools/MorphForge/1. Build OSC and geometry probe")]
        public static void MenuBuild() { Debug.Log(Build()); }

        public static string Build()
        {
            if (EditorApplication.isPlaying || EditorApplication.isCompiling)
                throw new InvalidOperationException("Build requires idle Edit Mode.");
            if (AssetDatabase.LoadAssetAtPath<AnimatorController>(Root+"/ProbeFX.controller"))
                throw new InvalidOperationException("Probe assets already exist; inspect before rebuilding.");
            for (int i=0;i<UnityEngine.SceneManagement.SceneManager.sceneCount;i++)
                if (UnityEngine.SceneManagement.SceneManager.GetSceneAt(i).isDirty)
                    throw new InvalidOperationException("Save existing scene edits before creating the probe scene.");
            EnsureFolder(Root+"/Clips");
            var shader = Shader.Find("MorphForge/ProbeGeometry");
            if (!shader || !shader.isSupported) throw new InvalidOperationException("Probe Shader is unavailable.");
            if (ShaderUtil.GetShaderMessages(shader).Any(m=>m.severity.ToString()=="Error"))
                throw new InvalidOperationException("Probe Shader has compilation errors.");

            var signalMaterial = MaterialAsset(Root+"/Signal.mat",shader,0);
            var geometryMaterial = MaterialAsset(Root+"/Geometry.mat",shader,1);
            var carrier = AssetDatabase.LoadAssetAtPath<Mesh>(Root+"/Carrier.asset");
            if (!carrier) { carrier=new Mesh{name="Eight triangles - all CPU vertices zero"}; AssetDatabase.CreateAsset(carrier,Root+"/Carrier.asset"); }
            carrier.Clear();
            carrier.vertices=new Vector3[24];
            carrier.triangles=Enumerable.Range(0,24).ToArray();
            carrier.bounds=new Bounds(Vector3.zero,Vector3.one*4);
            EditorUtility.SetDirty(carrier);

            var controller=AnimatorController.CreateAnimatorControllerAtPath(Root+"/ProbeFX.controller");
            controller.layers=new AnimatorControllerLayer[0];
            var module=new GameObject(ModuleName);
            try
            {
                var signal=GameObject.CreatePrimitive(PrimitiveType.Cube);
                signal.name="Signal"; signal.transform.SetParent(module.transform,false);
                signal.transform.localPosition=new Vector3(-1.4f,0,0);
                signal.transform.localScale=Vector3.one*0.5f;
                UnityEngine.Object.DestroyImmediate(signal.GetComponent<Collider>());
                signal.GetComponent<Renderer>().sharedMaterial=signalMaterial;
                var data=new GameObject("Geometry",typeof(MeshFilter),typeof(MeshRenderer));
                data.transform.SetParent(module.transform,false);
                data.transform.localPosition=new Vector3(0.6f,0,0);
                data.GetComponent<MeshFilter>().sharedMesh=carrier;
                data.GetComponent<Renderer>().sharedMaterial=geometryMaterial;

                var merge=module.AddComponent<ModularAvatarMergeAnimator>();
                merge.animator=controller; merge.layerType=VRCAvatarDescriptor.AnimLayerType.FX;
                merge.pathMode=MergeAnimatorPathMode.Relative; merge.matchAvatarWriteDefaults=true;
                merge.deleteAttachedAnimator=false;
                var parameters=module.AddComponent<ModularAvatarParameters>();
                AddFloat(controller,parameters,"Control","Signal","_Control",0,1,0,1);
                AddFloat(controller,parameters,"Count","Geometry","_TriangleCount",0,1,0,8);
                for(int i=0;i<Coordinates;i++)
                    AddFloat(controller,parameters,"V"+i.ToString("D2"),"Geometry","_V"+i.ToString("D2"),-1,1,-1,1);
                AddVisible(controller,parameters);
                PrefabUtility.SaveAsPrefabAsset(module,Root+"/MorphForgeProbe14.prefab");
            }
            finally { UnityEngine.Object.DestroyImmediate(module); }
            EditorUtility.SetDirty(controller); AssetDatabase.SaveAssets();

            var scene=EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Single);
            var source=AssetDatabase.LoadAssetAtPath<GameObject>("Assets/Nemesis/Nemesis.prefab");
            var avatar=(GameObject)PrefabUtility.InstantiatePrefab(source,scene);
            avatar.name="MorphForge_ProbeAvatar";
            PrefabUtility.UnpackPrefabInstance(avatar,PrefabUnpackMode.Completely,InteractionMode.AutomatedAction);
            var pipeline=avatar.GetComponent<VRC.Core.PipelineManager>();
            if(pipeline) pipeline.blueprintId="";
            var installed=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(Root+"/MorphForgeProbe14.prefab"),avatar.transform);
            installed.name=ModuleName;
            installed.transform.localPosition=new Vector3(0.9f,0.55f,0.25f);
            installed.transform.localScale=Vector3.one*0.35f;
            var floor=GameObject.CreatePrimitive(PrimitiveType.Plane); floor.name="Probe Floor";
            floor.transform.localScale=Vector3.one*0.5f;
            var light=new GameObject("Probe Light").AddComponent<Light>(); light.type=LightType.Directional;
            light.intensity=1.1f; light.transform.rotation=Quaternion.Euler(45,-30,0);
            var camera=new GameObject("Probe Camera").AddComponent<Camera>(); camera.tag="MainCamera";
            camera.transform.position=new Vector3(2.0f,1.4f,2.5f); camera.transform.LookAt(new Vector3(.5f,.45f,0));
            camera.nearClipPlane=.01f; camera.fieldOfView=45; camera.clearFlags=CameraClearFlags.SolidColor;
            camera.backgroundColor=new Color(.07f,.09f,.13f);
            camera.gameObject.AddComponent<AudioListener>();
            var gmRoot=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>("Packages/vrchat.blackstartx.gesture-manager/GestureManager.prefab"),scene);
            gmRoot.name="MorphForge Gesture Manager"; gmRoot.tag="EditorOnly";
            var gm=gmRoot.GetComponent<GestureManager>(); gm.settings.favourite=avatar.GetComponent<VRCAvatarDescriptor>();
            gm.settings.loadStored=false; gm.settings.isRemote=false;
            PrefabUtility.SaveAsPrefabAsset(avatar,Root+"/ProbeAvatar.prefab");
            EditorSceneManager.SaveScene(scene,ScenePath); AssetDatabase.SaveAssets();
            Selection.activeGameObject=gmRoot;
            Directory.CreateDirectory("Docs/Probe14");
            var report=new BuildReport{scene=ScenePath,carrierVertices=24,carrierTriangles=8,parameters=75,coordinateParameters=72,networkSyncedProbeParameters=0,
                carrierCpuVerticesZero=carrier.vertices.All(v=>v==Vector3.zero),shader=shader.name};
            var json=JsonUtility.ToJson(report,true); File.WriteAllText("Docs/Probe14/build.json",json);
            return json;
        }

        [Serializable] class BuildReport
        {
            public string scene,shader;
            public int carrierVertices,carrierTriangles,parameters,coordinateParameters,networkSyncedProbeParameters;
            public bool carrierCpuVerticesZero;
        }

        static Material MaterialAsset(string path,Shader shader,float mode)
        {
            var m=AssetDatabase.LoadAssetAtPath<Material>(path);
            if(!m) {m=new Material(shader);AssetDatabase.CreateAsset(m,path);}
            m.shader=shader;m.SetFloat("_Mode",mode);m.SetFloat("_Visible",1);m.SetFloat("_TriangleCount",0);
            EditorUtility.SetDirty(m);return m;
        }

        static AnimationClip Clip(string name,string path,string property,float value)
        {
            var clip=new AnimationClip{name=name};
            AnimationUtility.SetEditorCurve(clip,EditorCurveBinding.FloatCurve(path,typeof(MeshRenderer),"material."+property),AnimationCurve.Constant(0,1,value));
            AssetDatabase.CreateAsset(clip,Root+"/Clips/"+name+".anim");return clip;
        }

        static void AddFloat(AnimatorController controller,ModularAvatarParameters parameters,string suffix,string path,string property,float inputMin,float inputMax,float outputMin,float outputMax)
        {
            string name=Prefix+suffix;
            controller.AddParameter(name,AnimatorControllerParameterType.Float);
            parameters.parameters.Add(new ParameterConfig{nameOrPrefix=name,syncType=ParameterSyncType.Float,localOnly=true,saved=false,hasExplicitDefaultValue=true,defaultValue=0,internalParameter=false});
            var a=Clip(suffix+"_Min",path,property,outputMin);
            var b=Clip(suffix+"_Max",path,property,outputMax);
            var tree=new BlendTree{name=suffix,blendType=BlendTreeType.Simple1D,blendParameter=name,useAutomaticThresholds=false};
            tree.AddChild(a,inputMin);tree.AddChild(b,inputMax);AssetDatabase.AddObjectToAsset(tree,controller);
            var machine=new AnimatorStateMachine{name=suffix};AssetDatabase.AddObjectToAsset(machine,controller);
            var state=machine.AddState("Drive");state.motion=tree;state.writeDefaultValues=false;
            controller.AddLayer(new AnimatorControllerLayer{name=suffix,stateMachine=machine,defaultWeight=1});
        }

        static void AddVisible(AnimatorController controller,ModularAvatarParameters parameters)
        {
            string name=Prefix+"Visible";controller.AddParameter(name,AnimatorControllerParameterType.Bool);
            parameters.parameters.Add(new ParameterConfig{nameOrPrefix=name,syncType=ParameterSyncType.Bool,localOnly=true,saved=false,hasExplicitDefaultValue=true,defaultValue=0,internalParameter=false});
            var off=Clip("Visible_Off","Signal","_Visible",0);
            var on=Clip("Visible_On","Signal","_Visible",1);
            foreach(var pair in new[]{Tuple.Create(off,0f),Tuple.Create(on,1f)})
                AnimationUtility.SetEditorCurve(pair.Item1,EditorCurveBinding.FloatCurve("Geometry",typeof(MeshRenderer),"material._Visible"),AnimationCurve.Constant(0,1,pair.Item2));
            var machine=new AnimatorStateMachine{name="Visibility"};AssetDatabase.AddObjectToAsset(machine,controller);
            var disabled=machine.AddState("Hidden");disabled.motion=off;disabled.writeDefaultValues=false;
            var enabled=machine.AddState("Shown");enabled.motion=on;enabled.writeDefaultValues=false;
            var t=disabled.AddTransition(enabled);t.AddCondition(AnimatorConditionMode.If,0,name);t.duration=0;t.hasExitTime=false;
            t=enabled.AddTransition(disabled);t.AddCondition(AnimatorConditionMode.IfNot,0,name);t.duration=0;t.hasExitTime=false;
            controller.AddLayer(new AnimatorControllerLayer{name="Visibility",stateMachine=machine,defaultWeight=1});
        }

        static void EnsureFolder(string path)
        {
            string parent="Assets";
            foreach(var part in path.Split('/').Skip(1)){string next=parent+"/"+part;if(!AssetDatabase.IsValidFolder(next))AssetDatabase.CreateFolder(parent,part);parent=next;}
        }
    }
}
