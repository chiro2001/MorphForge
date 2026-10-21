using System;
using System.Linq;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase.Editor.BuildPipeline;
namespace MorphForge.Editor
{
    public sealed class Mf6SyncOverride:IVRCSDKPreprocessAvatarCallback
    {
        public int callbackOrder=>int.MaxValue-200;
        public bool OnPreprocessAvatar(GameObject avatar){if(!Mf6BudgetGate.Target(avatar))return true;var p=avatar.GetComponent<VRCAvatarDescriptor>().expressionParameters;if(!p)throw new Exception("MF6 parameters missing");foreach(var value in p.parameters)value.networkSynced=Mf6BudgetGate.Keep(value.name);return true;}
    }
    public sealed class Mf6BudgetGate:IVRCSDKPreprocessAvatarCallback
    {
        public int callbackOrder=>int.MaxValue-50;
        public static string[] Geometry=>Enumerable.Range(0,29).Select(i=>"MF6_D"+i.ToString("D2")).Concat(new[]{"MF6_Ctl"}).ToArray();
        public static bool Keep(string name)=>Geometry.Contains(name)||Mf5BudgetGate.Placement.Contains(name);
        public static bool Target(GameObject avatar){if(avatar.GetComponentsInChildren<Component>(true).Any(c=>c&&c.GetType().FullName=="MorphForgeAvatar.MorphForgeAvatarModule") || (avatar.GetComponent<VRCAvatarDescriptor>()?.expressionParameters?.parameters.Any(p=>p.name=="MF_Plugin")??false))return false;var stores=avatar.GetComponentsInChildren<ParticleSystem>(true).Where(p=>p.name=="Store").ToArray();return Enumerable.Range(0,8).All(i=>stores.Count(p=>p.transform.parent.name=="Pool"+i&&p.GetComponent<ParticleSystemRenderer>().sharedMaterial&&p.GetComponent<ParticleSystemRenderer>().sharedMaterial.HasProperty("_WireEncoding")&&p.GetComponent<ParticleSystemRenderer>().sharedMaterial.GetFloat("_WireEncoding")==6)==1);}
        public bool OnPreprocessAvatar(GameObject avatar){if(!Target(avatar))return true;var p=avatar.GetComponent<VRCAvatarDescriptor>().expressionParameters;var sync=p.parameters.Where(v=>v.networkSynced).ToArray();var compressed=FullFTBudgetUtil.CollectCompressedParameterNames(avatar);
            bool success=p.CalcTotalCost()==254&&sync.Length==37&&Geometry.All(n=>sync.Count(v=>v.name==n&&v.valueType==VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionParameters.ValueType.Int)==1)&&sync.All(v=>Keep(v.name))&&!Geometry.Any(n=>compressed.Contains(n));
            FullFTBudgetUtil.WriteReport("Docs/MF6/final-budget.json",new{success,totalBits=p.CalcTotalCost(),geometryBits=240,placementBits=14,remainingBits=256-p.CalcTotalCost(),parameters=sync.Select(v=>new{v.name,type=v.valueType.ToString()}).ToArray(),compressedGeometry=Geometry.Where(n=>compressed.Contains(n)).ToArray()});
            if(!success)throw new Exception("MF6 budget/direct-sync gate failed");return true;}
    }
}
