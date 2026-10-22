Shader "MorphForgeAvatar/LocalPreview" {
 Properties { _MainTex("Received Avatar view",2D)="black"{} }
 SubShader {
  Tags {"Queue"="Overlay" "RenderType"="Transparent" "IgnoreProjector"="True"}
  Pass {
   Cull Off ZWrite Off ZTest Always Blend SrcAlpha OneMinusSrcAlpha
   CGPROGRAM
   #pragma vertex vert
   #pragma fragment frag
   #pragma multi_compile_instancing
   #include "UnityCG.cginc"
   sampler2D _MainTex; float _VRChatMirrorMode, _VRChatCameraMode;
   struct appdata {float4 vertex:POSITION;float2 uv:TEXCOORD0;UNITY_VERTEX_INPUT_INSTANCE_ID};
   struct v2f {float4 pos:SV_POSITION;float2 uv:TEXCOORD0;UNITY_VERTEX_OUTPUT_STEREO};
   v2f vert(appdata v){v2f o;UNITY_SETUP_INSTANCE_ID(v);UNITY_INITIALIZE_OUTPUT(v2f,o);UNITY_INITIALIZE_VERTEX_OUTPUT_STEREO(o);o.pos=UnityObjectToClipPos(v.vertex);o.uv=v.uv;return o;}
   fixed4 frag(v2f i):SV_Target {if(_VRChatMirrorMode>.5||_VRChatCameraMode>.5)discard;
    // Native preview camera has a distinct clip range; never sample its own RT.
    if(abs(_ProjectionParams.y-.01003125)<.0000005&&abs(_ProjectionParams.z-4093.125)<.01)discard;fixed4 c=tex2D(_MainTex,i.uv);c.a=1;return c;}
   ENDCG
  }
 }
}
