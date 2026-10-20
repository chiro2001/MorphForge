// Validation fixture only. No product material references this shader.
Shader "Hidden/MorphForge/MF5WireProbe" {
 Properties {_ProbeData("data",Vector)=(0,0,0,1) _ProbeOut("out",Float)=0}
 SubShader {Pass {ZTest Always Cull Off ZWrite Off
 CGPROGRAM
 #pragma vertex vert_img
 #pragma fragment frag
 #pragma target 3.5
 #include "UnityCG.cginc"
 #include "MorphForgeRuntimeSurface.cginc"
 float4 _ProbeData;float _ProbeOut;
 float4 frag(v2f_img i):SV_Target {
  float mode=MF5Mode(_ProbeData,5);float4 color=float4(0,0,0,1),surface=float4(0,.9,0,0);float kind,scale,seed;
  MF5Material(_ProbeData,mode,color,surface,kind,scale,seed);
  if(_ProbeOut<.5)return color;
  if(_ProbeOut<1.5)return surface;
  if(_ProbeOut<2.5)return float4(kind,scale,seed,mode);
  return MF5Codes(_ProbeData,5,mode);
 }
 ENDCG
 }}
}
