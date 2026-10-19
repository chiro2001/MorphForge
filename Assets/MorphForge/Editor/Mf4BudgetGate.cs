using System;
using System.Linq;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase.Editor.BuildPipeline;

namespace MorphForge.Editor
{
    // Independent profile. The FullFT gates deliberately do not match this root.
    public sealed class Mf4SyncOverride : IVRCSDKPreprocessAvatarCallback
    {
        public int callbackOrder => int.MaxValue - 200;
        public bool OnPreprocessAvatar(GameObject avatar)
        {
            if (!Mf4BudgetGate.Target(avatar)) return true;
            var parameters=avatar.GetComponent<VRCAvatarDescriptor>().expressionParameters;
            if(!parameters)throw new InvalidOperationException("MF4 parameters missing after MA/NDMF.");
            foreach(var p in parameters.parameters)p.networkSynced=Mf4BudgetGate.Keep(p.name);
            return true;
        }
    }
    public sealed class Mf4BudgetGate : IVRCSDKPreprocessAvatarCallback
    {
        public int callbackOrder => int.MaxValue - 50;
        public static bool Target(GameObject avatar)
        {
            // GestureManager/SDK may rename the build clone to Avatar(Clone).
            // Match the actual three-pool wire format, not a mutable root name.
            var stores=avatar.GetComponentsInChildren<ParticleSystem>(true).Where(p=>p.name=="Store").ToArray();
            return Enumerable.Range(0,3).All(l=>stores.Count(p=>p.transform.parent.name=="Lane"+l&&p.GetComponent<ParticleSystemRenderer>().sharedMaterial&&p.GetComponent<ParticleSystemRenderer>().sharedMaterial.HasProperty("_WireEncoding")&&p.GetComponent<ParticleSystemRenderer>().sharedMaterial.GetFloat("_WireEncoding")==4)==1);
        }
        public static readonly string[] Placement={"MF_World","MF_Handle","MF_Reset","MF_GrabL","MF_GrabR","MF_Level","MF_Scale"};
        public static string[] Geometry => Enumerable.Range(0,3).SelectMany(l=>Enumerable.Range(0,9).Select(i=>"MF4_L"+l+"_D"+i.ToString("D2")).Concat(new[]{"MF4_L"+l+"_Ctl"})).ToArray();
        public static bool Keep(string name) => Geometry.Contains(name)||Placement.Contains(name);
        public bool OnPreprocessAvatar(GameObject avatar)
        {
            if(!Target(avatar))return true;
            var p=avatar.GetComponent<VRCAvatarDescriptor>().expressionParameters;
            var sync=p.parameters.Where(v=>v.networkSynced).ToArray();
            var geometry=Geometry;
            bool pass=p.CalcTotalCost()==254&&sync.Length==37&&geometry.All(n=>sync.Count(v=>v.name==n&&v.valueType==VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionParameters.ValueType.Int)==1)
                &&Placement.All(n=>sync.Count(v=>v.name==n&&v.valueType==(n=="MF_Scale"?VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionParameters.ValueType.Float:VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionParameters.ValueType.Bool))==1)
                &&sync.All(v=>Keep(v.name));
            FullFTBudgetUtil.WriteReport("Docs/MF4/final-budget.json",new{success=pass,totalBits=p.CalcTotalCost(),geometryBits=240,placementBits=14,faceTrackingBits=0,bodySyncedBits=0,remainingBits=256-p.CalcTotalCost(),parameters=sync.Select(v=>new{v.name,type=v.valueType.ToString()}).ToArray()});
            if(!pass)throw new InvalidOperationException("MF4 final budget/manifest/direct-sync gate failed; see Docs/MF4/final-budget.json");
            return true;
        }
    }
}
