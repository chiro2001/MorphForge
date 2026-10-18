using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEngine;

public static class ParticleStageInvestigate
{
    public static string Inspect()
    {
        var scene=UnityEditor.SceneManagement.EditorSceneManager.NewPreviewScene();
        var root=new GameObject("MorphForge temporary particle investigation");
        UnityEngine.SceneManagement.SceneManager.MoveGameObjectToScene(root,scene);
        try
        {
            var store=new GameObject("Store").AddComponent<ParticleSystem>();store.transform.SetParent(root.transform,false);
            var pump=new GameObject("Pump").AddComponent<ParticleSystem>();pump.transform.SetParent(root.transform,false);
            var sm=store.main;sm.playOnAwake=false;sm.startLifetime=100;sm.startSpeed=0;sm.maxParticles=64;
            var se=store.emission;se.enabled=true;se.rateOverTime=0;se.SetBursts(new[]{new ParticleSystem.Burst(0,1)});
            var pm=pump.main;pm.playOnAwake=false;pm.loop=false;pm.duration=.1f;pm.startLifetime=.05f;pm.startSpeed=0;
            var pe=pump.emission;pe.rateOverTime=0;pe.SetBursts(new[]{new ParticleSystem.Burst(0,1)});
            var subs=pump.subEmitters;subs.enabled=true;subs.AddSubEmitter(store,ParticleSystemSubEmitterType.Birth,ParticleSystemSubEmitterProperties.InheritNothing);
            pump.Play(false);pump.Simulate(.03f,true,false,true);
            store.Simulate(.01f,false,false,true);
            var countAfterFirst=store.particleCount;
            pump.gameObject.SetActive(false);
            var countAfterPumpDisable=store.particleCount;
            pump.gameObject.SetActive(true);
            pump.Play(false);pump.Simulate(.03f,true,false,true);
            store.Simulate(.01f,false,false,true);
            var countAfterSecond=store.particleCount;
            var bindings=AnimationUtility.GetAnimatableBindings(store.gameObject,root).Select(b=>new {b.path,type=b.type.Name,b.propertyName}).Where(b=>b.type=="ParticleSystem"||b.type=="ParticleSystemRenderer").ToArray();
            var report=new {
                environment="Editor native API mechanism investigation, not OSC validation",
                siblingSubEmitterReference=subs.GetSubEmitterSystem(0)==store,
                storeParent=store.transform.parent.name,pumpParent=pump.transform.parent.name,
                countAfterFirst,countAfterPumpDisable,countAfterSecond,bindings
            };
            Directory.CreateDirectory("Docs/Stage1");
            var json=Newtonsoft.Json.JsonConvert.SerializeObject(report,Newtonsoft.Json.Formatting.Indented);
            File.WriteAllText("Docs/Stage1/investigation.json",json);
            return Newtonsoft.Json.JsonConvert.SerializeObject(new {report.siblingSubEmitterReference,report.storeParent,report.pumpParent,countAfterFirst,countAfterPumpDisable,countAfterSecond,
                bindings=bindings.Where(b=>b.path=="Store"&&(b.propertyName.Contains("startSize")||b.propertyName.Contains("startColor")||b.propertyName=="TriggerModule.enabled")).ToArray()},Newtonsoft.Json.Formatting.Indented);
        }
        finally {UnityEngine.Object.DestroyImmediate(root);UnityEditor.SceneManagement.EditorSceneManager.ClosePreviewScene(scene);}
    }
}
