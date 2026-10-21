Shader "MorphForge/Diagnostics/MotionCarrierProbe" {
 Properties { _Mode("Mode",Float)=0 _ExpectedX("Expected X",Vector)=(1,0,0,0) _ExpectedY("Expected Y",Vector)=(0,1,0,0) _ExpectedZ("Expected Z",Vector)=(0,0,1,0) }
 SubShader { Tags {"RenderType"="Opaque"} Cull Off Pass {
 CGPROGRAM
 #pragma vertex vert
 #pragma fragment frag
 #pragma target 4.0
 #include "UnityCG.cginc"
 float _Mode;float4 _ExpectedX,_ExpectedY,_ExpectedZ;
 struct appdata {float4 vertex:POSITION;float3 normal:NORMAL;float4 tangent:TANGENT;float4 uv:TEXCOORD0;float4 size:TEXCOORD1;float4 center:TEXCOORD2;float2 rotationYZ:TEXCOORD3;};
 struct v2f {float4 pos:SV_POSITION;float3 value:TEXCOORD0;};
 v2f vert(appdata v) {
  v2f o;o.pos=UnityObjectToClipPos(v.vertex);float3 rotation=float3(v.center.w,v.rotationYZ);
  if(_Mode<.5){o.value=rotation;return o;}
  if(_Mode<1.5){float3 bits=floor(rotation*1048576+.5);o.value=float3(fmod(bits.z,512),floor(bits.z/512),bits.x);return o;}
  float code=floor(v.uv.z+.5),axis=floor(code/8);code=fmod(code,8);float3 corner=float3(fmod(code,2),fmod(floor(code/2),2),floor(code/4))-.5;
  float sign=axis<.5?corner.x*2:(axis<1.5?corner.y*2:corner.z*2);
  float3 n=normalize(v.normal),t=normalize(v.tangent.xyz),b=normalize(cross(n,t)*v.tangent.w),x,y,z;
  if(axis<.5){x=n*sign;y=b;z=t;}else if(axis<1.5){x=t;y=n*sign;z=b;}else{x=t;y=b;z=n*sign;}
  float sx,cx,sz,cz;sincos(rotation.x,sx,cx);sincos(rotation.z,sz,cz);
  float3 ax=x*cz-y*sz,ay=x*(cx*sz)+y*(cx*cz)-z*sx,az=x*(sx*sz)+y*(sx*cz)+z*cx;
  o.value=float3(dot(ax,_ExpectedX.xyz),dot(ay,_ExpectedY.xyz),dot(az,_ExpectedZ.xyz));return o;
 }
 float4 frag(v2f i):SV_Target{return float4(i.value,1);}
 ENDCG
 } }
}
