using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEngine;
using Newtonsoft.Json;
using VRC.SDK3.Avatars.Components;
using VRC.SDK3.Dynamics.Contact.Components;
using nadena.dev.modular_avatar.core;
using MorphForge.Editor;
public static class StreamWorkbenchEntry
{
    public static async Task<string> RunInteraction()
    {
        if(!EditorApplication.isPlaying)throw new Exception("Play Mode required");
        var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single(g=>g.gameObject.scene.path==Mf6AvatarBuild.ScenePath);
        var module=gm.Module as ModuleVrc3;if(module==null)throw new Exception("GestureManager not ready");
        var placement=module.Avatar.transform.Find(PlacementStage4Build.ModuleName);
        var receivers=module.Avatar.GetComponentsInChildren<VRCContactReceiver>(true).Where(c=>c.parameter=="MF_NearL"||c.parameter=="MF_NearR").ToArray();
        var oldEnabled=receivers.Select(c=>c.enabled).ToArray();
        var names=new[]{"MF_Handle","MF_Reset","GestureLeft","GestureRight","MF_NearL","MF_NearR"};
        var old=names.Select(n=>module.GetParam(n).FloatValue()).ToArray();
        var checks=new JArray();
        void Set(string n,float v){module.GetParam(n).Set(module,v);}
        try{
            foreach(var c in receivers)c.enabled=false;
            Set("MF_Handle",1);Set("MF_Reset",0);Set("GestureLeft",0);Set("GestureRight",0);Set("MF_NearL",0);Set("MF_NearR",0);await Task.Delay(300);
            foreach(var side in new[]{new{gesture="GestureLeft",near="MF_NearL",value=1},new{gesture="GestureRight",near="MF_NearR",value=2}}){
                Set(side.near,1);Set(side.gesture,1);await Task.Delay(450);
                var visual=module.Avatar.GetComponentsInChildren<Transform>(true).Single(t=>t.name=="HandleVisual");
                checks.Add(new JObject{["hand"]=side.gesture,["held"]=module.GetParam("MF_Grab").IntValue(),["handleVisible"]=visual.gameObject.activeInHierarchy,["passed"]=module.GetParam("MF_Grab").IntValue()==side.value&&visual.gameObject.activeInHierarchy});
                Set(side.gesture,0);Set(side.near,0);await Task.Delay(450);
                checks.Add(new JObject{["release"]=side.gesture,["passed"]=module.GetParam("MF_Grab").IntValue()==0&&visual.gameObject.activeInHierarchy});
            }
            var senders=module.Avatar.GetComponentsInChildren<VRCContactSender>(true).Where(c=>c.collisionTags.Any(t=>t=="MF6_HandL"||t=="MF6_HandR")).ToArray();
            checks.Add(new JObject{["explicitHandSenders"]=senders.Length,["passed"]=senders.Length==2&&receivers.Length==2&&receivers.All(r=>r.allowSelf&&!r.allowOthers&&r.radius>=.16f&&senders.Any(c=>c.collisionTags.Intersect(r.collisionTags).Any()))});
            var result=new JObject{["success"]=checks.All(c=>(bool)c["passed"]),["checks"]=checks,["scope"]="Actual GestureManager merged Avatar Animator; diagnostic proximity params and real gesture variables. Native sender/receiver configuration checked; not a physical VR hardware or multiplayer test."};
            Directory.CreateDirectory("Docs/StreamV16");File.WriteAllText("Docs/StreamV16/placement-play.json",result.ToString());return result.ToString();
        }finally{for(int i=0;i<names.Length;i++)Set(names[i],old[i]);for(int i=0;i<receivers.Length;i++)receivers[i].enabled=oldEnabled[i];}
    }
    public static string Inspect()
    {
        var s=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        var a=s.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single();
        var module=a.transform.Find(PlacementStage4Build.ModuleName);
        var contacts=module.GetComponentsInChildren<VRCContactReceiver>(true).Select(c=>new{c.name,c.parameter,c.radius,c.allowSelf,c.localOnly,tags=c.collisionTags,active=c.gameObject.activeSelf}).ToArray();
        var senders=module.GetComponentsInChildren<VRCContactSender>(true).Select(c=>new{c.name,c.radius,tags=c.collisionTags}).ToArray();
        var r=new{scene=s.path,dirty=s.isDirty,play=EditorApplication.isPlaying,module=module.name,controller=AssetDatabase.GetAssetPath(module.GetComponent<ModularAvatarMergeAnimator>().animator),contacts,senders};
        return JsonConvert.SerializeObject(r,Formatting.Indented);
    }
}
