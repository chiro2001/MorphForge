using UnityEngine;
using VRC.SDKBase;

namespace MorphForgeAvatar {
    // Editor-only install marker. The uploaded Avatar uses native components,
    // Animator and shaders; no custom MonoBehaviour executes in VRChat.
    [AddComponentMenu("MorphForge/Avatar Module")]
    [DisallowMultipleComponent]
    public sealed class MorphForgeAvatarModule : MonoBehaviour, IEditorOnly {
        [HideInInspector] public string version = "1.0.0";
        [HideInInspector] public string catalogIdentity;
        [HideInInspector] public bool vrcFuryBridge;
    }
}
