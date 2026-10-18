Shader "MorphForge/ParticleBox"
{
    Properties { _CellSize("Cell size",Float)=0.25 }
    SubShader
    {
        Tags { "Queue"="Geometry" "RenderType"="Opaque" }
        Cull Back ZWrite On
        Pass
        {
            CGPROGRAM
            #pragma target 3.0
            #pragma vertex vert
            #pragma fragment frag
            #include "UnityCG.cginc"
            float _CellSize;
            struct appdata {float4 vertex:POSITION;float3 normal:NORMAL;fixed4 color:COLOR;UNITY_VERTEX_INPUT_INSTANCE_ID};
            struct v2f {float4 pos:SV_POSITION;float3 world:TEXCOORD0;float3 normal:TEXCOORD1;fixed4 data:COLOR;UNITY_VERTEX_OUTPUT_STEREO};
            v2f vert(appdata v)
            {
                UNITY_SETUP_INSTANCE_ID(v);
                v2f o;UNITY_INITIALIZE_OUTPUT(v2f,o);UNITY_INITIALIZE_VERTEX_OUTPUT_STEREO(o);
                o.pos=UnityObjectToClipPos(v.vertex);o.world=mul(unity_ObjectToWorld,v.vertex).xyz;
                o.normal=UnityObjectToWorldNormal(v.normal);o.data=v.color;return o;
            }
            fixed4 frag(v2f i):SV_Target
            {
                float4 codes=round(saturate(i.data)*63);
                float style=codes.r+64*codes.g;
                float3 color=0.25+0.65*frac(style*float3(.173,.397,.619));
                if(style==1)color=float3(.62,.67,.73);
                if(style==2)color=float3(.64,.31,.12);
                if(style==3)color=float3(.12,.55,.92);
                if(style==4)color=float3(.24,.78,.38);
                if(codes.b>0)color*=.45+.55*frac(codes.b*float3(.31,.53,.79));
                float3 n=normalize(i.normal),an=abs(n);
                float2 plane=an.y>an.x&&an.y>an.z?i.world.xz:(an.x>an.z?i.world.zy:i.world.xy);
                float2 uv=frac(plane/max(_CellSize,.0001));
                float edge=min(min(uv.x,1-uv.x),min(uv.y,1-uv.y));
                float checker=fmod(floor(uv.x*2)+floor(uv.y*2),2);
                float shading=.52+.48*abs(dot(n,normalize(float3(.4,.8,.6))));
                color*=shading*lerp(.84,1,checker)*lerp(.3,1,step(.025,edge));
                return float4(color,1); // A stores data, never opacity.
            }
            ENDCG
        }
    }
    Fallback Off
}
