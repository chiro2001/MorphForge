Shader "MorphForge/ParticleBoxLocalUV"
{
    Properties { _VoxelGrid("Debug voxel grid",Float)=0 _CellSize("Logical cell size",Float)=0.25 }
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
            float _VoxelGrid;
            // Native streams: UV.xy + UV2.xy => TEXCOORD0; SizeXYZ => TEXCOORD1.
            struct appdata {float4 vertex:POSITION;fixed4 color:COLOR;float4 uvFace:TEXCOORD0;float3 size:TEXCOORD1;UNITY_VERTEX_INPUT_INSTANCE_ID};
            struct v2f {float4 pos:SV_POSITION;float2 tiles:TEXCOORD0;float shade:TEXCOORD1;fixed4 data:COLOR;UNITY_VERTEX_OUTPUT_STEREO};
            v2f vert(appdata v)
            {
                UNITY_SETUP_INSTANCE_ID(v);
                v2f o;UNITY_INITIALIZE_OUTPUT(v2f,o);UNITY_INITIALIZE_VERTEX_OUTPUT_STEREO(o);
                o.pos=UnityObjectToClipPos(v.vertex);o.data=v.color;
                float axis=v.uvFace.z;
                float2 extent=axis<.5?v.size.zy:(axis<1.5?v.size.xz:v.size.xy);
                o.tiles=v.uvFace.xy*extent/max(_CellSize,.0001);
                float3 faceNormal=axis<.5?float3(1,0,0):(axis<1.5?float3(0,1,0):float3(0,0,1));
                o.shade=.52+.48*dot(faceNormal,normalize(float3(.4,.8,.6)));
                return o;
            }
            fixed4 frag(v2f i):SV_Target
            {
                float4 codes=round(saturate(i.data)*63);
                float style=codes.r+64*codes.g;
                float3 color=.25+.65*frac(style*float3(.173,.397,.619));
                if(style==1)color=float3(.62,.67,.73);
                if(style==2)color=float3(.64,.31,.12);
                if(style==3)color=float3(.12,.55,.92);
                if(style==4)color=float3(.24,.78,.38);
                if(codes.b>0)color*=.45+.55*frac(codes.b*float3(.31,.53,.79));
                float2 uv=frac(i.tiles);
                float edge=min(min(uv.x,1-uv.x),min(uv.y,1-uv.y));
                float checker=fmod(floor(uv.x*2)+floor(uv.y*2),2);
                color*=i.shade; if(_VoxelGrid>.5)color*=lerp(.55,1,smoothstep(0,fwidth(edge)*1.2,edge));
                return float4(color,1);
            }
            ENDCG
        }
    }
    Fallback Off
}
