using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using System.Collections.Generic;
using UnityEditor;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
using VRC.SDK3.Dynamics.Constraint.Components;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
public static class Stage4Entry
{
    static float[] V(Vector3 v){return new[]{v.x,v.y,v.z};}
    static ModuleVrc3 Module(){return UnityEngine.Object.FindObjectsOfType<GestureManager>().Select(g=>g.Module as ModuleVrc3).First(m=>m!=null&&m.Avatar.GetComponentsInChildren<Transform>(true).Any(t=>t.name=="MorphForgeGeometry"));}
    static Transform Named(Transform root,string name){return root.GetComponentsInChildren<Transform>(true).First(t=>t.name==name);}
    public static string ResumeOwnedUpgrade(){return MorphForge.Editor.PlacementStage4Build.UpgradeCurrent(true);}
    public static string Upgrade(){return MorphForge.Editor.PlacementStage4Build.UpgradeCurrent();}
    public static string Receiver(){var c=UnityEngine.Object.FindObjectOfType<MorphForge.Editor.MorphForgePlayReceiver>();return Newtonsoft.Json.JsonConvert.SerializeObject(new{exists=!!c,c?.Running,c?.Error,c?.ReceivePort,c?.SendPort});}
    public static string Capture()
    {
        var m=Module();var root=m.Avatar.transform;var anchor=Named(root,"AnchorPose");var level=Named(root,"ContentLeveling");var scale=Named(root,"ContentScale");var ps=Named(root,"Store").GetComponent<ParticleSystem>();var parts=new ParticleSystem.Particle[ps.main.maxParticles];int count=ps.GetParticles(parts);
        var c=anchor.GetComponent<VRCParentConstraint>();return Newtonsoft.Json.JsonConvert.SerializeObject(new{frame=Time.frameCount,avatar=V(root.position),anchor=V(anchor.position),anchorEuler=V(anchor.eulerAngles),contentEuler=V(level.eulerAngles),contentUp=V(level.up),contentForward=V(level.forward),contentScale=V(scale.lossyScale),
            world=m.GetParam("MF_World").BoolValue(),leveled=m.GetParam("MF_Level").BoolValue(),grab=m.GetParam("MF_Grab").IntValue(),grabL=m.GetParam("MF_GrabL").BoolValue(),grabR=m.GetParam("MF_GrabR").BoolValue(),frozen=c.FreezeToWorld,
            count,capacity=ps.main.maxParticles,received=m.GetParam("MF3_RxCount").FloatValue(),completed=m.GetParam("MF3_DoneCount").FloatValue(),busy=m.GetParam("MF3_Busy").BoolValue(),inbox=m.GetParam("MF3_InboxReady").BoolValue(),brush=Enumerable.Range(0,4).Select(i=>m.GetParam("MF3_B"+i).FloatValue()).ToArray(),
            particles=parts.Take(count).Select(p=>new{position=V(p.position),size=V(p.startSize3D),worldPosition=V(ps.main.customSimulationSpace.TransformPoint(p.position)),codes=new[]{Mathf.RoundToInt(p.startColor.r*63f/255),Mathf.RoundToInt(p.startColor.g*63f/255),Mathf.RoundToInt(p.startColor.b*63f/255),Mathf.RoundToInt(p.startColor.a*63f/255)}}).ToArray()},Newtonsoft.Json.Formatting.Indented);
    }
    public static string Validate()
    {
        if(EditorApplication.isPlaying)throw new InvalidOperationException("Edit Mode required");var avatar=AssetDatabase.LoadAssetAtPath<GameObject>("Assets/MorphForge/Stage4/Avatar.prefab");var clone=UnityEngine.Object.Instantiate(avatar);clone.hideFlags=HideFlags.HideAndDontSave;
        try{
            nadena.dev.ndmf.AvatarProcessor.ProcessAvatar(clone);var d=clone.GetComponent<VRCAvatarDescriptor>();var p=d.expressionParameters.parameters;var sync=p.Where(x=>x.networkSynced).ToArray();int bits=sync.Sum(x=>x.valueType.ToString()=="Bool"?1:8);var mf3=sync.Where(x=>x.name.StartsWith("MF3_")).ToArray();var place=sync.Where(x=>x.name.StartsWith("MF_")).ToArray();
            var missing=clone.GetComponentsInChildren<Transform>(true).Sum(t=>GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(t.gameObject));var scripts=clone.GetComponentsInChildren<MonoBehaviour>(true).Where(b=>b&&b.GetType().Namespace!=null&&b.GetType().Namespace.StartsWith("MorphForge")).Select(b=>b.GetType().FullName).ToArray();
            var ps=Named(clone.transform,"Store").GetComponent<ParticleSystem>();var level=Named(clone.transform,"ContentLeveling").GetComponent<VRCRotationConstraint>();
            var report=new{success=missing==0&&scripts.Length==0&&mf3.Length==7&&place.Sum(x=>x.valueType.ToString()=="Bool"?1:8)==14&&bits<=256&&ps.main.maxParticles==16384&&level,
                missingScripts=missing,totalSyncedBits=bits,scope="Basic Nemesis + MF3; NOT Full_FT integrated budget",geometryBits=mf3.Length*8,placementBits=place.Sum(x=>x.valueType.ToString()=="Bool"?1:8),capacity=ps.main.maxParticles,
                parameters=p.Where(x=>x.name.StartsWith("MF")).Select(x=>new{x.name,type=x.valueType.ToString(),x.networkSynced}).ToArray(),scripts,
                parentConstraints=clone.GetComponentsInChildren<VRCParentConstraint>(true).Length,rotationConstraints=clone.GetComponentsInChildren<VRCRotationConstraint>(true).Length,aimConstraints=clone.GetComponentsInChildren<VRCAimConstraint>(true).Length,positionConstraints=clone.GetComponentsInChildren<VRCPositionConstraint>(true).Length};
            string json=Newtonsoft.Json.JsonConvert.SerializeObject(report,Newtonsoft.Json.Formatting.Indented);Directory.CreateDirectory("Docs/Stage4");File.WriteAllText("Docs/Stage4/export.json",json);return json;
        }finally{UnityEngine.Object.DestroyImmediate(clone);}
    }
    public static async Task<string> CheckLevel()
    {
        var m=Module();var root=m.Avatar.transform;var anchor=Named(root,"AnchorPose");var content=Named(root,"ContentLeveling");var hand=Named(root,"RightHand");var old=hand.localRotation;var rootRot=root.rotation;
        var checks=new List<object>();var samples=new List<object>();void Check(string name,bool pass){checks.Add(new{name,pass});}
        async Task Set(string name,float v){m.GetParam(name).Set(m,v);await Task.Delay(230);}
        try{
            await Set("MF_Reset",1);await Set("MF_Reset",0);await Set("MF_World",1);await Set("MF_Grab",2);await Set("MF_Level",1);
            foreach(var e in new[]{new Vector3(25,37,15),new Vector3(-40,73,-20),new Vector3(80,110,35),new Vector3(90,20,0),new Vector3(89.427f,20,0)})
            {
                hand.localRotation=old*Quaternion.Euler(e);await Task.Delay(250);var projected=Vector3.ProjectOnPlane(anchor.forward,Vector3.up);float headingError=projected.sqrMagnitude>.0001f?Vector3.Angle(projected,content.forward):0;
                samples.Add(new{handEuler=V(e),anchorEuler=V(anchor.eulerAngles),contentEuler=V(content.eulerAngles),up=V(content.up),headingError});
                Check("Level keeps world up at "+e,Vector3.Dot(content.up,Vector3.up)>.999f&&!float.IsNaN(content.eulerAngles.x));
            }
            foreach(var desiredEuler in new[]{new Vector3(89.427f,20,0),new Vector3(90,20,0),new Vector3(90,-35,89)})
            {
                var desired=Quaternion.Euler(desiredEuler);hand.rotation=desired*Quaternion.Inverse(anchor.rotation)*hand.rotation;await Task.Delay(250);
                Check("Actual anchor rotation stimulus "+desiredEuler,Quaternion.Angle(anchor.rotation,desired)<.3f);
                Check("World up at actual near-vertical anchor "+desiredEuler,Vector3.Dot(content.up,Vector3.up)>.999f&&!float.IsNaN(content.eulerAngles.y));
            }
            var p=anchor.position;await Set("MF_Level",0);Check("Full rotation returns when leveling disabled",Quaternion.Angle(content.rotation,anchor.rotation)<.2f&&Vector3.Distance(anchor.position,p)<.001f);
            await Set("MF_Level",1);await Set("MF_Grab",0);var worldForward=content.forward;var worldPosition=content.position;root.rotation=Quaternion.Euler(0,63,0);await Task.Delay(250);
            Check("World fixed retains its world heading after avatar turns",Vector3.Angle(content.forward,worldForward)<.2f&&Vector3.Distance(content.position,worldPosition)<.002f);
            Check("Grab encoder released both network bits",!m.GetParam("MF_GrabL").BoolValue()&&!m.GetParam("MF_GrabR").BoolValue());
            await Set("IsLocal",0);await Set("MF_GrabL",1);Check("Remote bit decoder selects left",m.GetParam("MF_Grab").IntValue()==1);await Set("MF_GrabL",0);await Set("MF_GrabR",1);Check("Remote bit decoder selects right",m.GetParam("MF_Grab").IntValue()==2);await Set("MF_GrabR",0);
        }finally{hand.localRotation=old;root.rotation=rootRot;m.GetParam("IsLocal").Set(m,1);m.GetParam("MF_Grab").Set(m,0);m.GetParam("MF_Reset").Set(m,1);await Task.Delay(180);m.GetParam("MF_Reset").Set(m,0);}
        var result=new{success=checks.All(c=>(bool)c.GetType().GetProperty("pass").GetValue(c)),checks,samples,scope="Editor native constraints, actual bone-proxy rotation stimuli; no VR hardware"};string json=Newtonsoft.Json.JsonConvert.SerializeObject(result,Newtonsoft.Json.Formatting.Indented);File.WriteAllText("Docs/Stage4/level-validation.json",json);return json;
    }
}
