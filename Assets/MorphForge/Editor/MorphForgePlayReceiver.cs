using System;
using System.Linq;
using System.Reflection;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
namespace MorphForge.Editor
{
    public static class MorphForgePlayReceiverDriver
    {
        const BindingFlags Flags=BindingFlags.Public|BindingFlags.NonPublic|BindingFlags.Instance;
        public static void Start(MorphForgePlayReceiver receiver)
        {
            try{
                if(receiver.ReceivePort<1||receiver.ReceivePort>65535||receiver.SendPort<1||receiver.SendPort>65535)throw new InvalidOperationException("Invalid OSC port");
                var managers=UnityEngine.Object.FindObjectsOfType<GestureManager>();var gm=managers.FirstOrDefault(g=>g.gameObject.scene==receiver.gameObject.scene);
                if(!gm)throw new InvalidOperationException("Scene has no Gesture Manager");
                if(!receiver.Avatar)receiver.Avatar=receiver.gameObject.scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single();
                if(gm.Module==null)gm.SetModule(new ModuleVrc3(receiver.Avatar));
                var module=gm.Module as ModuleVrc3;if(module==null)throw new InvalidOperationException("Gesture Manager does not use VRC3 module");
                receiver.Osc=typeof(ModuleVrc3).GetField("OscModule",Flags).GetValue(module);
                if(!(bool)receiver.Osc.GetType().GetProperty("Enabled",Flags).GetValue(receiver.Osc))receiver.Osc.GetType().GetMethod("Start",Flags).Invoke(receiver.Osc,new object[]{receiver.ReceivePort,"127.0.0.1",receiver.SendPort});
                receiver.Running=true;receiver.Error=null;Debug.Log("MorphForge native OSC ready: "+receiver.ReceivePort+" → "+receiver.SendPort);
            }catch(Exception e){receiver.Running=false;receiver.Error=e.GetBaseException().Message;Debug.LogError("MorphForge OSC receiver: "+receiver.Error);}
        }
        public static void Stop(MorphForgePlayReceiver receiver){if(receiver.Osc==null)return;try{receiver.Osc.GetType().GetMethod("Stop",Flags).Invoke(receiver.Osc,new object[]{true});}catch{}receiver.Running=false;receiver.Osc=null;}
    }
}
