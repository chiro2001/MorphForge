Shader "MorphForge/ProbeGeometry"
{
    Properties
    {
        _Mode ("0 Signal / 1 OSC Vertices / 2 CPU Reference", Float) = 0
        _Control ("Signal control", Range(0,1)) = 0
        _Visible ("Visible", Float) = 1
        _TriangleCount ("Triangle count", Float) = 0
        _V00 ("Coordinate 00", Float) = 0
        _V01 ("Coordinate 01", Float) = 0
        _V02 ("Coordinate 02", Float) = 0
        _V03 ("Coordinate 03", Float) = 0
        _V04 ("Coordinate 04", Float) = 0
        _V05 ("Coordinate 05", Float) = 0
        _V06 ("Coordinate 06", Float) = 0
        _V07 ("Coordinate 07", Float) = 0
        _V08 ("Coordinate 08", Float) = 0
        _V09 ("Coordinate 09", Float) = 0
        _V10 ("Coordinate 10", Float) = 0
        _V11 ("Coordinate 11", Float) = 0
        _V12 ("Coordinate 12", Float) = 0
        _V13 ("Coordinate 13", Float) = 0
        _V14 ("Coordinate 14", Float) = 0
        _V15 ("Coordinate 15", Float) = 0
        _V16 ("Coordinate 16", Float) = 0
        _V17 ("Coordinate 17", Float) = 0
        _V18 ("Coordinate 18", Float) = 0
        _V19 ("Coordinate 19", Float) = 0
        _V20 ("Coordinate 20", Float) = 0
        _V21 ("Coordinate 21", Float) = 0
        _V22 ("Coordinate 22", Float) = 0
        _V23 ("Coordinate 23", Float) = 0
        _V24 ("Coordinate 24", Float) = 0
        _V25 ("Coordinate 25", Float) = 0
        _V26 ("Coordinate 26", Float) = 0
        _V27 ("Coordinate 27", Float) = 0
        _V28 ("Coordinate 28", Float) = 0
        _V29 ("Coordinate 29", Float) = 0
        _V30 ("Coordinate 30", Float) = 0
        _V31 ("Coordinate 31", Float) = 0
        _V32 ("Coordinate 32", Float) = 0
        _V33 ("Coordinate 33", Float) = 0
        _V34 ("Coordinate 34", Float) = 0
        _V35 ("Coordinate 35", Float) = 0
        _V36 ("Coordinate 36", Float) = 0
        _V37 ("Coordinate 37", Float) = 0
        _V38 ("Coordinate 38", Float) = 0
        _V39 ("Coordinate 39", Float) = 0
        _V40 ("Coordinate 40", Float) = 0
        _V41 ("Coordinate 41", Float) = 0
        _V42 ("Coordinate 42", Float) = 0
        _V43 ("Coordinate 43", Float) = 0
        _V44 ("Coordinate 44", Float) = 0
        _V45 ("Coordinate 45", Float) = 0
        _V46 ("Coordinate 46", Float) = 0
        _V47 ("Coordinate 47", Float) = 0
        _V48 ("Coordinate 48", Float) = 0
        _V49 ("Coordinate 49", Float) = 0
        _V50 ("Coordinate 50", Float) = 0
        _V51 ("Coordinate 51", Float) = 0
        _V52 ("Coordinate 52", Float) = 0
        _V53 ("Coordinate 53", Float) = 0
        _V54 ("Coordinate 54", Float) = 0
        _V55 ("Coordinate 55", Float) = 0
        _V56 ("Coordinate 56", Float) = 0
        _V57 ("Coordinate 57", Float) = 0
        _V58 ("Coordinate 58", Float) = 0
        _V59 ("Coordinate 59", Float) = 0
        _V60 ("Coordinate 60", Float) = 0
        _V61 ("Coordinate 61", Float) = 0
        _V62 ("Coordinate 62", Float) = 0
        _V63 ("Coordinate 63", Float) = 0
        _V64 ("Coordinate 64", Float) = 0
        _V65 ("Coordinate 65", Float) = 0
        _V66 ("Coordinate 66", Float) = 0
        _V67 ("Coordinate 67", Float) = 0
        _V68 ("Coordinate 68", Float) = 0
        _V69 ("Coordinate 69", Float) = 0
        _V70 ("Coordinate 70", Float) = 0
        _V71 ("Coordinate 71", Float) = 0
    }
    SubShader
    {
        Tags { "RenderType"="Opaque" "Queue"="Geometry" }
        Cull Off
        ZWrite On
        Pass
        {
            CGPROGRAM
            #pragma target 4.0
            #pragma vertex vert
            #pragma fragment frag
            #include "UnityCG.cginc"
            float _Mode, _Control, _Visible, _TriangleCount;
            float _V00; float _V01; float _V02; float _V03; float _V04; float _V05; float _V06; float _V07;
            float _V08; float _V09; float _V10; float _V11; float _V12; float _V13; float _V14; float _V15;
            float _V16; float _V17; float _V18; float _V19; float _V20; float _V21; float _V22; float _V23;
            float _V24; float _V25; float _V26; float _V27; float _V28; float _V29; float _V30; float _V31;
            float _V32; float _V33; float _V34; float _V35; float _V36; float _V37; float _V38; float _V39;
            float _V40; float _V41; float _V42; float _V43; float _V44; float _V45; float _V46; float _V47;
            float _V48; float _V49; float _V50; float _V51; float _V52; float _V53; float _V54; float _V55;
            float _V56; float _V57; float _V58; float _V59; float _V60; float _V61; float _V62; float _V63;
            float _V64; float _V65; float _V66; float _V67; float _V68; float _V69; float _V70; float _V71;
            struct appdata
            {
                float4 vertex : POSITION;
                uint vertexID : SV_VertexID;
                UNITY_VERTEX_INPUT_INSTANCE_ID
            };
            struct v2f
            {
                float4 position : SV_POSITION;
                float3 world : TEXCOORD0;
                nointerpolation float triIndex : TEXCOORD1;
                UNITY_VERTEX_OUTPUT_STEREO
            };
            float coordinate(uint index)
            {
                if (index == 0u) return _V00;
                if (index == 1u) return _V01;
                if (index == 2u) return _V02;
                if (index == 3u) return _V03;
                if (index == 4u) return _V04;
                if (index == 5u) return _V05;
                if (index == 6u) return _V06;
                if (index == 7u) return _V07;
                if (index == 8u) return _V08;
                if (index == 9u) return _V09;
                if (index == 10u) return _V10;
                if (index == 11u) return _V11;
                if (index == 12u) return _V12;
                if (index == 13u) return _V13;
                if (index == 14u) return _V14;
                if (index == 15u) return _V15;
                if (index == 16u) return _V16;
                if (index == 17u) return _V17;
                if (index == 18u) return _V18;
                if (index == 19u) return _V19;
                if (index == 20u) return _V20;
                if (index == 21u) return _V21;
                if (index == 22u) return _V22;
                if (index == 23u) return _V23;
                if (index == 24u) return _V24;
                if (index == 25u) return _V25;
                if (index == 26u) return _V26;
                if (index == 27u) return _V27;
                if (index == 28u) return _V28;
                if (index == 29u) return _V29;
                if (index == 30u) return _V30;
                if (index == 31u) return _V31;
                if (index == 32u) return _V32;
                if (index == 33u) return _V33;
                if (index == 34u) return _V34;
                if (index == 35u) return _V35;
                if (index == 36u) return _V36;
                if (index == 37u) return _V37;
                if (index == 38u) return _V38;
                if (index == 39u) return _V39;
                if (index == 40u) return _V40;
                if (index == 41u) return _V41;
                if (index == 42u) return _V42;
                if (index == 43u) return _V43;
                if (index == 44u) return _V44;
                if (index == 45u) return _V45;
                if (index == 46u) return _V46;
                if (index == 47u) return _V47;
                if (index == 48u) return _V48;
                if (index == 49u) return _V49;
                if (index == 50u) return _V50;
                if (index == 51u) return _V51;
                if (index == 52u) return _V52;
                if (index == 53u) return _V53;
                if (index == 54u) return _V54;
                if (index == 55u) return _V55;
                if (index == 56u) return _V56;
                if (index == 57u) return _V57;
                if (index == 58u) return _V58;
                if (index == 59u) return _V59;
                if (index == 60u) return _V60;
                if (index == 61u) return _V61;
                if (index == 62u) return _V62;
                if (index == 63u) return _V63;
                if (index == 64u) return _V64;
                if (index == 65u) return _V65;
                if (index == 66u) return _V66;
                if (index == 67u) return _V67;
                if (index == 68u) return _V68;
                if (index == 69u) return _V69;
                if (index == 70u) return _V70;
                if (index == 71u) return _V71;
                return 0;
            }
            v2f vert(appdata input)
            {
                UNITY_SETUP_INSTANCE_ID(input);
                v2f output;
                UNITY_INITIALIZE_OUTPUT(v2f, output);
                UNITY_INITIALIZE_VERTEX_OUTPUT_STEREO(output);
                float3 local = input.vertex.xyz;
                uint triIndex = input.vertexID / 3u;
                if (_Mode < 0.5)
                    local *= lerp(0.35, 1.25, saturate(_Control));
                else
                {
                    if (_Mode < 1.5)
                    {
                        uint index = input.vertexID * 3u;
                        local = float3(coordinate(index), coordinate(index+1u), coordinate(index+2u));
                    }
                    if (triIndex >= (uint)clamp(round(_TriangleCount), 0, 8)) local = 0;
                }
                output.position = UnityObjectToClipPos(float4(local,1));
                output.world = mul(unity_ObjectToWorld, float4(local,1)).xyz;
                output.triIndex = triIndex;
                return output;
            }
            fixed4 frag(v2f input) : SV_Target
            {
                clip(_Visible - 0.5);
                if (_Mode < 0.5)
                    return float4(lerp(float3(0.08,0.25,0.95),float3(0.95,0.15,0.08),saturate(_Control)),1);
                float3 normal = normalize(cross(ddx(input.world),ddy(input.world)));
                float light = 0.45 + 0.55 * abs(dot(normal,normalize(float3(0.4,0.7,0.6))));
                float3 color = 0.35 + 0.55 * frac(float3(0.19,0.43,0.71) * (input.triIndex+1));
                return float4(color*light,1);
            }
            ENDCG
        }
    }
    Fallback Off
}
