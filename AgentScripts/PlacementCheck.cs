using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Threading.Tasks;
using UnityEditor;
using UnityEngine;
using VRC.SDK3.Dynamics.Constraint.Components;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
public static class PlacementCheck
{
    static ModuleVrc3 M(){return UnityEngine.Object.FindObjectsOfType<GestureManager>().First(g=>g.name=="MorphForge Stage3 Gesture Manager").Module as ModuleVrc3;}
    static Transform T(Transform root,string name){return root.GetComponentsInChildren<Transform>(true).First(t=>t.name==name);}
    static float[] V(Vector3 p){return new[]{p.x,p.y,p.z};}
    public static async Task<string> Check()
    {
        var m=M();if(m==null)throw new Exception("Start OSC first");var avatar=m.Avatar.transform;var anchor=T(avatar,"AnchorPose");var c=anchor.GetComponent<VRCParentConstraint>();
        var dock=T(avatar,"AvatarDock");var left=T(avatar,"LeftHand");var right=T(avatar,"RightHand");var scale=T(avatar,"ContentScale");var ps=T(avatar,"Store").GetComponent<ParticleSystem>();
        var origin=avatar.position;var rotation=avatar.rotation;var leftLocal=left.localPosition;var rightLocal=right.localPosition;
        var checks=new List<object>();var states=new List<object>();
        void Check(string name,bool pass,object detail=null){checks.Add(new{name,pass,detail});}
        void Snap(string label){var guide=T(avatar,"ResetGuide").GetComponent<VRCParentConstraint>();states.Add(new{label,avatar=V(avatar.position),dock=V(dock.position),anchor=V(anchor.position),left=V(left.position),right=V(right.position),c.IsActive,c.FreezeToWorld,c.RebakeOffsetsWhenUnfrozen,guideActive=guide.IsActive,guideTarget=guide.TargetTransform?guide.TargetTransform.name:"null",sources=Enumerable.Range(0,c.Sources.Count).Select(i=>new{weight=c.Sources[i].Weight,offset=V(c.Sources[i].ParentPositionOffset)}).ToArray(),grab=m.GetParam("MF_Grab").IntValue(),near=m.GetParam("MF_NearL").BoolValue(),scale=V(scale.localScale)});}
        async Task Set(string p,float v){m.GetParam(p).Set(m,v);await Task.Delay(250);}
        try
        {
            await Set("MF_Grab",0);await Set("MF_Reset",1);await Set("MF_Reset",0);await Set("MF_World",1);await Set("MF_Scale",1);Snap("initial");
            Check("Default world mode freezes native constraint",c.FreezeToWorld);
            Check("Reset locates anchor at dock",Vector3.Distance(anchor.position,dock.position)<.003f);
            var fixedPosition=anchor.position;avatar.position=origin+new Vector3(.7f,0,.3f);await Task.Delay(250);Snap("avatar moved in world mode");
            Check("Avatar stimulus applied",Vector3.Distance(avatar.position,origin)>.5f);
            Check("World anchor remains in place",Vector3.Distance(anchor.position,fixedPosition)<.003f);
            var frozenRotation=anchor.rotation;avatar.rotation=Quaternion.Euler(0,37,0);await Task.Delay(200);
            Check("World anchor ignores actual avatar rotation",Quaternion.Angle(avatar.rotation,rotation)>30&&Quaternion.Angle(anchor.rotation,frozenRotation)<.1f&&Vector3.Distance(anchor.position,fixedPosition)<.003f);
            await Set("MF_World",0);var followStart=anchor.position;var rootStart=avatar.position;avatar.position+=new Vector3(.3f,.1f,-.2f);await Task.Delay(250);Snap("follow moved");
            Check("Follow mode unfreezes",!c.FreezeToWorld);
            Check("Follow tracks actual avatar movement",Vector3.Distance(anchor.position-followStart,avatar.position-rootStart)<.004f);
            var relative=avatar.InverseTransformPoint(anchor.position);var relativeRotation=Quaternion.Inverse(avatar.rotation)*anchor.rotation;
            avatar.rotation=Quaternion.Euler(0,79,0);await Task.Delay(200);
            Check("Follow tracks avatar rotation and preserved offset",Vector3.Distance(anchor.position,avatar.TransformPoint(relative))<.004f&&Quaternion.Angle(anchor.rotation,avatar.rotation*relativeRotation)<.1f);
            await Set("MF_World",1);var beforeGrab=anchor.position;await Set("MF_Grab",1);Snap("left held");
            Check("Left grab keeps pose without snapping",Vector3.Distance(anchor.position,beforeGrab)<.005f);
            Check("Left source selected",c.Sources[1].Weight>.99f&&!c.FreezeToWorld);
            var handStart=left.position;var heldStart=anchor.position;left.localPosition+=new Vector3(.2f,.15f,.1f);await Task.Delay(250);Snap("left moved");
            Check("Hand stimulus applied",Vector3.Distance(left.position,handStart)>.1f);
            Check("Held anchor follows left movement",Vector3.Distance(anchor.position-heldStart,left.position-handStart)<.005f);
            await Set("MF_Grab",0);var released=anchor.position;left.localPosition=leftLocal;avatar.position+=new Vector3(.1f,0,.2f);await Task.Delay(250);Snap("left release");
            Check("Release retains dropped world pose",Vector3.Distance(anchor.position,released)<.005f&&c.FreezeToWorld);
            await Set("MF_Grab",2);var rightStart=right.position;heldStart=anchor.position;right.localPosition+=new Vector3(-.12f,.1f,.15f);await Task.Delay(250);Snap("right moved");
            Check("Held anchor follows right movement",Vector3.Distance(right.position,rightStart)>.1f&&Vector3.Distance(anchor.position-heldStart,right.position-rightStart)<.005f);
            await Set("MF_Reset",1);Snap("reset held after grab");await Set("MF_Reset",0);Snap("reset after grab");Check("Reset during manual grab releases and returns to dock",m.GetParam("MF_Grab").IntValue()==0&&Vector3.Distance(anchor.position,dock.position)<.004f);
            await Set("MF_Scale",.25f);Snap("quarter scale");Check("MF_Scale drives all content axes",Vector3.Distance(scale.localScale,Vector3.one*.25f)<.002f);
            await Set("MF_Handle",0);Check("Hiding handle disables visual and sensors",!T(avatar,"HandleVisual").gameObject.activeSelf&&!T(avatar,"Sensors").gameObject.activeSelf);
            await Set("MF_Handle",1);Check("Showing handle restores visual and sensors",T(avatar,"HandleVisual").gameObject.activeSelf&&T(avatar,"Sensors").gameObject.activeSelf);
        }
        finally
        {
            avatar.position=origin;avatar.rotation=rotation;left.localPosition=leftLocal;right.localPosition=rightLocal;
            m.GetParam("MF_Grab").Set(m,0);m.GetParam("MF_Scale").Set(m,1);m.GetParam("MF_World").Set(m,true);m.GetParam("MF_Handle").Set(m,true);m.GetParam("MF_Reset").Set(m,true);await Task.Delay(180);m.GetParam("MF_Reset").Set(m,false);
        }
        bool success=checks.All(x=>(bool)x.GetType().GetProperty("pass").GetValue(x));
        var report=new{success,scope="Editor native constraint evaluation; root and bone proxy stimuli; not hardware pickup or multiplayer",checks,states};
        var json=Newtonsoft.Json.JsonConvert.SerializeObject(report,Newtonsoft.Json.Formatting.Indented);Directory.CreateDirectory("Docs/Stage3");File.WriteAllText("Docs/Stage3/placement-validation.json",json);return json;
    }
}
