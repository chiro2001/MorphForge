Shader "MorphForge/AnchorHandle"
{
    Properties { _Color("Gold",Color)=(1,0.57,0.06,1) _Opacity("Opacity",Range(0,1))=.38 _Glow("Glow",Range(0,4))=1.5 }
    SubShader
    {
        Tags { "Queue"="Transparent" "RenderType"="Transparent" }
        Blend SrcAlpha OneMinusSrcAlpha
        ZWrite Off Cull Back
        Pass
        {
            CGPROGRAM
            #pragma vertex vert
            #pragma fragment frag
            #include "UnityCG.cginc"
            float4 _Color;float _Opacity,_Glow;
            struct appdata{float4 vertex:POSITION;float3 normal:NORMAL;UNITY_VERTEX_INPUT_INSTANCE_ID};
            struct v2f{float4 pos:SV_POSITION;float3 normal:TEXCOORD0;float3 view:TEXCOORD1;UNITY_VERTEX_OUTPUT_STEREO};
            v2f vert(appdata v)
            {
                UNITY_SETUP_INSTANCE_ID(v);v2f o;UNITY_INITIALIZE_OUTPUT(v2f,o);UNITY_INITIALIZE_VERTEX_OUTPUT_STEREO(o);
                o.pos=UnityObjectToClipPos(v.vertex);float3 world=mul(unity_ObjectToWorld,v.vertex).xyz;
                o.normal=UnityObjectToWorldNormal(v.normal);o.view=_WorldSpaceCameraPos-world;return o;
            }
            float4 frag(v2f i):SV_Target
            {
                float rim=pow(1-saturate(dot(normalize(i.normal),normalize(i.view))),2);
                return float4(_Color.rgb*(.6+rim*_Glow),saturate(_Opacity*(.45+rim)));
            }
            ENDCG
        }
    }
    Fallback Off
}
