using System;
using System.Linq;
using System.Collections.Generic;
using System.Threading.Tasks;
using UnityEngine;
public static class PumpProbe
{
 public static string SimulatedFrames(){
  var source=UnityEngine.Object.FindObjectsOfType<ParticleSystem>().First(p=>p.name=="Store"&&p.transform.parent.name=="MorphForgeGeometry").transform.parent.gameObject;
  var cases=new List<object>();foreach(float lifetime in new[]{.025f,.1f,.5f})foreach(float step in new[]{.016f,.033f,.1f,.3f}){
   var rig=UnityEngine.Object.Instantiate(source);rig.name="MF pump test fixture";
   try{var store=rig.GetComponentsInChildren<ParticleSystem>(true).First(p=>p.name=="Store");var pump=rig.GetComponentsInChildren<ParticleSystem>(true).First(p=>p.name=="Pump");foreach(var r in rig.GetComponentsInChildren<Renderer>(true))r.enabled=false;var trigger=store.trigger;trigger.enabled=false;store.Stop(true,ParticleSystemStopBehavior.StopEmittingAndClear);var main=pump.main;main.startLifetime=lifetime;main.maxParticles=1;pump.gameObject.SetActive(true);pump.Simulate(step,true,true,false);cases.Add(new{lifetime,step,storeCount=store.particleCount,pumpCount=pump.particleCount});}
   finally{UnityEngine.Object.DestroyImmediate(rig);}
  }
  return Newtonsoft.Json.JsonConvert.SerializeObject(new{scope="Isolated native ParticleSystem pump simulation; does not modify live display",cases});
 }
 public static async Task<string> Reactivation(){
  var source=UnityEngine.Object.FindObjectsOfType<ParticleSystem>().First(p=>p.name=="Store"&&p.transform.parent.name=="MorphForgeGeometry").transform.parent.gameObject;var cases=new List<object>();
  foreach(float lifetime in new[]{.025f,.1f,.5f}){var rig=UnityEngine.Object.Instantiate(source);rig.name="MF reactivation fixture";try{var store=rig.GetComponentsInChildren<ParticleSystem>(true).First(p=>p.name=="Store");var pump=rig.GetComponentsInChildren<ParticleSystem>(true).First(p=>p.name=="Pump");foreach(var r in rig.GetComponentsInChildren<Renderer>(true))r.enabled=false;var trigger=store.trigger;trigger.enabled=false;store.Stop(true,ParticleSystemStopBehavior.StopEmittingAndClear);pump.gameObject.SetActive(false);var main=pump.main;main.startLifetime=lifetime;var counts=new List<int>();for(int n=0;n<8;n++){pump.gameObject.SetActive(true);await Task.Delay(65);pump.gameObject.SetActive(false);await Task.Delay(185);counts.Add(store.particleCount);}cases.Add(new{lifetime,counts});}finally{UnityEngine.Object.DestroyImmediate(rig);}}
  return Newtonsoft.Json.JsonConvert.SerializeObject(new{scope="Isolated native GameObject activation at .25s; no live display changes",cases});
 }
}
