using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Threading.Tasks;
using UnityEngine;
using VRC.SDK3.Dynamics.Contact.Components;
using VRC.SDK3.Dynamics.Constraint.Components;
using VRC.Dynamics;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
public static class PlacementGripCheck
{
    public static async Task<string> Check()
    {
        var m=UnityEngine.Object.FindObjectsOfType<GestureManager>().First(g=>g.name=="MorphForge Stage3 Gesture Manager").Module as ModuleVrc3;
        var root=m.Avatar.transform;var anchor=root.GetComponentsInChildren<Transform>(true).First(t=>t.name=="AnchorPose");var checks=new List<object>();
        void Check(string name,bool pass){checks.Add(new{name,pass});}
        async Task Set(string p,float v){m.GetParam(p).Set(m,v);await Task.Delay(220);}
        GameObject senderObject=null;
        try
        {
            await Set("MF_Handle",1);await Set("MF_World",1);await Set("GestureLeft",0);await Set("GestureRight",0);await Set("MF_Grab",0);
            foreach(var side in new[]{new{tag="HandL",near="MF_NearL",gesture="GestureLeft",grab=1},new{tag="HandR",near="MF_NearR",gesture="GestureRight",grab=2}})
            {
                senderObject=new GameObject("Temporary native self contact stimulus");senderObject.transform.SetParent(root,false);senderObject.transform.position=anchor.position;
                var sender=senderObject.AddComponent<VRCContactSender>();sender.shapeType=ContactBase.ShapeType.Sphere;sender.radius=.03f;sender.collisionTags=new List<string>{side.tag};
                await Task.Delay(250);Check(side.tag+" native Contact reaches Near",m.GetParam(side.near).BoolValue());
                Check(side.tag+" open hand alone does not grab",m.GetParam("MF_Grab").IntValue()==0);
                await Set(side.gesture,1);Check(side.tag+" contact + fist drives synced Grab",m.GetParam("MF_Grab").IntValue()==side.grab);
                senderObject.transform.position+=Vector3.one*4;await Task.Delay(200);Check(side.tag+" leaving contact while closed retains hold",m.GetParam("MF_Grab").IntValue()==side.grab);
                await Set(side.gesture,0);Check(side.tag+" opening hand releases",m.GetParam("MF_Grab").IntValue()==0&&anchor.GetComponent<VRCParentConstraint>().FreezeToWorld);
                UnityEngine.Object.DestroyImmediate(senderObject);senderObject=null;await Task.Delay(150);
            }
            await Set("IsLocal",0);await Set("MF_Grab",2);Check("Remote synced Grab selects right source without contacts",anchor.GetComponent<VRCParentConstraint>().Sources[2].Weight>.99f);
            await Set("MF_Grab",0);Check("Remote synced release freezes",anchor.GetComponent<VRCParentConstraint>().FreezeToWorld);
        }
        finally
        {
            if(senderObject)UnityEngine.Object.DestroyImmediate(senderObject);m.GetParam("IsLocal").Set(m,1);m.GetParam("GestureLeft").Set(m,0);m.GetParam("GestureRight").Set(m,0);m.GetParam("MF_Grab").Set(m,0);m.GetParam("MF_Reset").Set(m,1);await Task.Delay(180);m.GetParam("MF_Reset").Set(m,0);
        }
        bool success=checks.All(x=>(bool)x.GetType().GetProperty("pass").GetValue(x));
        var json=Newtonsoft.Json.JsonConvert.SerializeObject(new{success,scope="Editor native temporary ContactSender + gesture parameter stimuli; not VR hardware or multiplayer",checks},Newtonsoft.Json.Formatting.Indented);Directory.CreateDirectory("Docs/Stage3");File.WriteAllText("Docs/Stage3/grip-validation.json",json);return json;
    }
}
