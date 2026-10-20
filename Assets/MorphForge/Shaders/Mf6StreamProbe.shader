Shader "Hidden/MorphForge/MF6StreamProbe"
{
    Properties { _ProbeMode("Mode",Float)=0 }
    SubShader { Pass {
        Cull Off ZWrite Off ZTest Always
        CGPROGRAM
        #pragma target 3.5
        #pragma vertex vert
        #pragma fragment frag
        #include "UnityCG.cginc"
        float _ProbeMode;
        struct appdata {float4 vertex:POSITION;float3 normal:NORMAL;float4 tangent:TANGENT;float4 color:COLOR;float4 uv:TEXCOORD0;float4 size:TEXCOORD1;float4 center:TEXCOORD2;};
        struct v2f {float4 position:SV_POSITION;float3 native:TEXCOORD0;float4 size:TEXCOORD1;float4 color:TEXCOORD2;float4 uv:TEXCOORD3;float4 center:TEXCOORD4;float3 normal:TEXCOORD5;float4 tangent:TEXCOORD6;};
        v2f vert(appdata v){v2f o;o.position=float4(v.uv.xy*2-1,.5,1);o.native=v.vertex.xyz;o.size=v.size;o.color=v.color;o.uv=v.uv;o.center=v.center;o.normal=v.normal;o.tangent=v.tangent;return o;}
        float4 frag(v2f i):SV_Target{
            if(_ProbeMode<.5)return i.size;
            if(_ProbeMode<1.5)return float4(unity_ObjectToWorld._m03_m13_m23,1);
            if(_ProbeMode<2.5)return float4(unity_ObjectToWorld._m00_m10_m20,1);
            if(_ProbeMode<3.5)return float4(unity_ObjectToWorld._m01_m11_m21,1);
            if(_ProbeMode<4.5)return float4(unity_ObjectToWorld._m02_m12_m22,1);
            if(_ProbeMode<5.5)return i.color*255;
            if(_ProbeMode<6.5)return float4(i.native,1);
            if(_ProbeMode<7.5)return i.uv;
            if(_ProbeMode<8.5)return i.center;
            if(_ProbeMode<9.5)return float4(i.normal,1);
            return i.tangent;
        }
        ENDCG
    }} Fallback Off
}
