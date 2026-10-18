using System;
using System.Collections;
using System.Linq;
using System.Reflection;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
namespace MorphForge.Editor
{
    // Editor test-scene component; never placed inside an Avatar.
    public sealed class MorphForgePlayReceiver:MonoBehaviour
    {
        public VRCAvatarDescriptor Avatar;
        public int ReceivePort=9012;
        public int SendPort=9013;
        public bool AutoStart=true;
        [NonSerialized] public bool Running;
        [NonSerialized] public string Error;
        [NonSerialized] public object Osc;
        void Call(string name)
        {
#if UNITY_EDITOR
            var driver=AppDomain.CurrentDomain.GetAssemblies().Select(a=>a.GetType("MorphForge.Editor.MorphForgePlayReceiverDriver")).FirstOrDefault(t=>t!=null);
            if(driver==null){Error="Editor receiver driver unavailable";return;}
            driver.GetMethod(name,BindingFlags.Public|BindingFlags.Static).Invoke(null,new object[]{this});
#endif
        }
        IEnumerator Start(){if(!AutoStart)yield break;yield return null;yield return null;TryStart();}
        public void TryStart(){Call("Start");}
        void OnDestroy(){Call("Stop");}
    }
}
