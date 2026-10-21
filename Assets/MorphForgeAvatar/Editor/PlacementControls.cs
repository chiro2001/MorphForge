using System;
using System.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;
using nadena.dev.modular_avatar.core;
using VRC.SDK3.Avatars.ScriptableObjects;

namespace MorphForgeAvatar.Editor {
    /// <summary>Build-time migration. Runtime uses only native Animator/constraints.</summary>
    public static class PlacementControls {
        public const string AllowGrab = "MF_AllowGrab";
        public static void Apply(GameObject module) {
            var merge = module.GetComponent<ModularAvatarMergeAnimator>();
            var fx = merge ? merge.animator as AnimatorController : null;
            if (!fx) throw new InvalidOperationException("Placement controller missing");
            if (!fx.parameters.Any(p => p.name == AllowGrab))
                fx.AddParameter(new AnimatorControllerParameter {name=AllowGrab,type=AnimatorControllerParameterType.Bool,defaultBool=true});
            var parameters = module.GetComponent<ModularAvatarParameters>();
            if (!parameters.parameters.Any(p => p.nameOrPrefix == AllowGrab))
                parameters.parameters.Add(new ParameterConfig {nameOrPrefix=AllowGrab,syncType=ParameterSyncType.Bool,localOnly=true,saved=true,hasExplicitDefaultValue=true,defaultValue=1});
            var detect = fx.layers.Single(l => l.name == "Placement: owner contact and fist detection").stateMachine;
            foreach (var state in detect.states.Select(s=>s.state)) foreach(var t in state.transitions) {
                var conditions=t.conditions;
                for(int i=0;i<conditions.Length;i++) if(conditions[i].parameter=="MF_Handle") conditions[i].parameter=AllowGrab;
                t.conditions=conditions; EditorUtility.SetDirty(t);
            }
            // Visibility has no authority over contacts or the held pose.
            foreach(var clip in fx.animationClips.Distinct()) {
                bool changed=false;
                foreach(var binding in AnimationUtility.GetCurveBindings(clip))
                    if(binding.path.EndsWith("/Sensors",StringComparison.Ordinal) && binding.propertyName=="m_IsActive") {
                        AnimationUtility.SetEditorCurve(clip,binding,null); changed=true;
                    }
                if(changed)EditorUtility.SetDirty(clip);
            }
            var sensors=module.GetComponentsInChildren<Transform>(true).Single(t=>t.name=="Sensors");
            sensors.gameObject.SetActive(true);
            var dock=module.transform.Find("AvatarDock");
            if(!dock)throw new InvalidOperationException("Avatar dock missing");
            dock.localRotation=Quaternion.Euler(0,180,0);
            var installer=module.GetComponent<ModularAvatarMenuInstaller>();
            foreach(var submenu in installer.menuToAppend.controls.Where(c=>c.subMenu).Select(c=>c.subMenu)) {
                if(!submenu.controls.Any(c=>c.parameter?.name==AllowGrab)) {
                    if(submenu.controls.Count>=8)throw new InvalidOperationException("Placement menu needs a submenu before adding grab toggle");
                    submenu.controls.Add(new VRCExpressionsMenu.Control {name="允许本人抓握",type=VRCExpressionsMenu.Control.ControlType.Toggle,parameter=new VRCExpressionsMenu.Control.Parameter {name=AllowGrab},value=1});
                    EditorUtility.SetDirty(submenu);
                }
            }
            EditorUtility.SetDirty(fx);EditorUtility.SetDirty(parameters);EditorUtility.SetDirty(dock);EditorUtility.SetDirty(sensors.gameObject);
        }
    }
}
