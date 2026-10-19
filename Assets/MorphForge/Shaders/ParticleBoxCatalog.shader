Shader "MorphForge/ParticleBoxCatalog"
{
    Properties
    {
        _VoxelGrid("Debug voxel grid", Float) = 0
        _UseLayeredTextures("Mipmapped texture layers", Float) = 0
        _CatalogLayers("Catalog texture layers", 2DArray) = "" {}
        _CellSize("Logical cell size", Float) = 0.25
        _CellPixels("Catalog cell pixels", Float) = 32
        _AtlasSize("Atlas size", Vector) = (1, 1, 1, 1)
        _MetaWidth("Metadata width", Float) = 256
        _MetaHeight("Metadata height", Float) = 64
        _CatalogAtlas("Catalog atlas", 2D) = "white" {}
        _CatalogMeta("Catalog metadata", 2D) = "black" {}
    }

    SubShader
    {
        // A material has one render-queue slot. Transparent is selected so
        // water/glass are not drawn too early; opaque particles share the late
        // queue and keep ZWrite on in their pass. This is a single-pool
        // approximation; see docs/materials/unity-catalog.md.
        Tags { "Queue"="Transparent" "RenderType"="Transparent" }

        CGINCLUDE
        #include "UnityCG.cginc"
        #include "Lighting.cginc"

        UNITY_DECLARE_TEX2DARRAY(_CatalogLayers);
        float _UseLayeredTextures;
        float _VoxelGrid;
        sampler2D _CatalogAtlas;
        sampler2D _CatalogMeta;
        float4 _AtlasSize;
        float _CellSize;
        float _CellPixels;
        float _MetaWidth;
        float _MetaHeight;

        #define CATALOG_HAS_TEXTURE  1.0
        #define CATALOG_TRANSPARENT  2.0
        #define CATALOG_CUTOUT       4.0
        #define CATALOG_WATER        8.0
        #define CATALOG_CLOUD        16.0

        struct appdata
        {
            float4 vertex : POSITION;
            float3 normal : NORMAL;
            fixed4 color : COLOR;
            float4 uvFace : TEXCOORD0; // xy=face-local tile UV, z=axis, w=unused legacy sign
            float3 size : TEXCOORD1;   // particle birth SizeXYZ
            UNITY_VERTEX_INPUT_INSTANCE_ID
        };

        struct v2f
        {
            float4 pos : SV_POSITION;
            float3 world : TEXCOORD0;
            float3 local : TEXCOORD1;
            float3 normal : TEXCOORD2;
            fixed4 data : COLOR;
            float4 uvFace : TEXCOORD3;
            float3 size : TEXCOORD4;
            UNITY_VERTEX_OUTPUT_STEREO
        };

        v2f vert(appdata v)
        {
            v2f o;
            UNITY_SETUP_INSTANCE_ID(v);
            UNITY_INITIALIZE_OUTPUT(v2f, o);
            UNITY_INITIALIZE_VERTEX_OUTPUT_STEREO(o);
            o.pos = UnityObjectToClipPos(v.vertex);
            o.world = mul(unity_ObjectToWorld, v.vertex).xyz;
            o.local = v.vertex.xyz;
            o.normal = v.normal;
            o.data = v.color;
            o.uvFace = v.uvFace;
            o.size = max(v.size, 1e-6);
            return o;
        }

        float4 MetaTexel(float styleId, float slot)
        {
            float index = styleId * 4.0 + slot;
            float width = max(_MetaWidth, 1.0);
            float2 pixel = float2(fmod(index, width), floor(index / width));
            float2 uv = (pixel + 0.5) / float2(width, max(_MetaHeight, 1.0));
            return tex2Dlod(_CatalogMeta, float4(uv, 0.0, 0.0));
        }

        float AxisOf(float4 faceData)
        {
            return floor(faceData.z + 0.5);
        }

        float3 AxisVector(float axis)
        {
            return axis < 0.5 ? float3(1, 0, 0) : (axis < 1.5 ? float3(0, 1, 0) : float3(0, 0, 1));
        }

        float FaceIndex(float axis, float3 normal, float3 local)
        {
            float3 axisVector = AxisVector(axis);
            float projected = dot(normal, axisVector);
            float vertexAxis = axis < 0.5 ? local.x : (axis < 1.5 ? local.y : local.z);
            float side = abs(projected) > 1e-4 ? sign(projected) : sign(vertexAxis);
            if (side == 0.0) side = 1.0;
            if (axis < 0.5) return side > 0.0 ? 0.0 : 1.0;  // +X / -X
            if (axis < 1.5) return side > 0.0 ? 2.0 : 3.0;  // +Y / -Y
            return side > 0.0 ? 4.0 : 5.0;                  // +Z / -Z
        }

        float2 FaceUV(float axis, float3 local, float3 size, float4 faceData)
        {
            // This deliberately mirrors ParticleBoxLocalUV: no world-space projection.
            float2 extent = axis < 0.5 ? size.zy : (axis < 1.5 ? size.xz : size.xy);
            return frac(faceData.xy * extent / max(_CellSize, 0.0001));
        }

        fixed4 SampleCatalog(v2f i, out float3 baseColor, out float opacity, out float flags)
        {
            float4 codes = floor(saturate(i.data) * 63.0 + 0.5);
            float styleId = codes.r + 64.0 * codes.g;
            float tint = codes.b;
            float motion = codes.a / 64.0;
            float4 m0 = MetaTexel(styleId, 0.0); // offsetXY, frames, frameTime
            float4 m1 = MetaTexel(styleId, 1.0); // sRGB color, opacity
            float4 m2 = MetaTexel(styleId, 2.0); // metallic, roughness, emission, flags
            flags = m2.w;
            float3 sRGBColor = m1.rgb;
            if (styleId < 0.5)
            {
                sRGBColor = float3(0.62, 0.67, 0.73);
                m1.a = 1.0;
            }
            // Manifest colors are sRGB-normalized values. This is the only
            // color-space conversion applied to metadata colors.
            baseColor = GammaToLinearSpace(sRGBColor);
            opacity = saturate(m1.a);

            if (tint > 0.0)
            {
                float3 tintFactor = 0.45 + 0.55 * frac(tint * float3(0.31, 0.53, 0.79));
                baseColor *= tintFactor;
            }

            float axis = AxisOf(i.uvFace);
            float3 axisVector = AxisVector(axis);
            float normalLength = length(i.normal);
            float3 n = normalLength > 1e-4 ? i.normal / normalLength : axisVector;
            if (abs(dot(n, axisVector)) < 1e-4)
            {
                float vertexAxis = axis < 0.5 ? i.local.x : (axis < 1.5 ? i.local.y : i.local.z);
                n = vertexAxis >= 0.0 ? axisVector : -axisVector;
            }

            float face = FaceIndex(axis, i.normal, i.local);
            if (fmod(floor(flags / 32.0), 2.0) >= 1.0)
            {
                clip(0.5 - fmod(floor(codes.a / exp2(face)), 2.0));
                motion = 0.0; // transparent surfaces use these six bits for interface masks
            }
            float2 tile = FaceUV(axis, i.local, i.size, i.uvFace);
            float4 texel = float4(1, 1, 1, 1);
            bool hasTexture = fmod(flags, 2.0) >= 1.0;
            if (hasTexture)
            {
                float frames = max(1.0, m0.z);
                float frameTime = max(0.001, m0.w);
                float phaseTicks = motion * frames;
                float frame = fmod(floor((_Time.y / frameTime + phaseTicks) + 0.0001), frames);
                float cell = max(1.0, _CellPixels);
                float2 inset = float2(0.001, 0.001);
                float2 atlasPixels = m0.xy + float2(face * cell, frame * cell) + clamp(tile, inset, 1.0 - inset) * cell;
                float2 atlasUV = atlasPixels / max(_AtlasSize.xy, float2(1, 1));
                if (_UseLayeredTextures > 0.5 && fmod(floor(flags / 64.0), 2.0) >= 1.0)
                {
                    float baseLayer = MetaTexel(styleId, 3.0).y;
                    float2 extent = axis < .5 ? i.size.zy : (axis < 1.5 ? i.size.xz : i.size.xy);
                    float2 logical = i.uvFace.xy * extent / max(_CellSize, .0001);
                    texel = _CatalogLayers.SampleGrad(sampler_CatalogLayers, float3(tile, baseLayer + frame * 6.0 + face), ddx(logical), ddy(logical));
                }
                else texel = tex2D(_CatalogAtlas, atlasUV);
                #if defined(UNITY_COLORSPACE_GAMMA)
                texel.rgb = GammaToLinearSpace(texel.rgb);
                #endif
            }

            if (hasTexture)
            {
                baseColor *= texel.rgb;
            }
            if (_VoxelGrid > .5)
            {
                float2 edge = min(tile, 1.0 - tile);
                float2 smoothing = max(fwidth(i.uvFace.xy), .0001);
                float2 lines = smoothstep(0.0, smoothing * 1.2, edge);
                baseColor *= lerp(.55, 1.0, min(lines.x, lines.y));
            }

            // Fixed virtual sun + hemisphere lighting in artwork-local coordinates.
            n = normalize(n);
            float3 lightDirection = normalize(float3(.4,.8,.6));
            float3 viewDirection = normalize(mul((float3x3)unity_WorldToObject, _WorldSpaceCameraPos.xyz - i.world));
            float3 halfDirection = normalize(lightDirection + viewDirection);
            float nDotL = saturate(dot(n, lightDirection));
            float metallic = saturate(m2.x);
            float roughness = saturate(m2.y);
            float specularPower = exp2(2.0 + 10.0 * (1.0 - roughness));
            float specular = pow(saturate(dot(n, halfDirection)), specularPower) * lerp(0.04, 1.0, metallic);
            float3 diffuse = baseColor * (1.0 - 0.55 * metallic) * (0.28 + 0.72 * nDotL);
            float3 ambient = lerp(float3(.12,.13,.16),float3(.35,.42,.52),n.y*.5+.5)*baseColor;
            float3 surface = ambient + diffuse + baseColor * specular * lerp(0.18, 1.0, metallic);
            surface += baseColor * max(0.0, m2.z);

            if (fmod(flags, 32.0) >= 16.0)
            {
                float3 fraction = frac(i.local / max(_CellSize,.0001));
                surface *= .94 + .06 * sin(fraction.x*9.+fraction.y*7.+fraction.z*8.+_Time.y*.13);
            }

            if (fmod(flags, 16.0) >= 8.0)
            {
                float wave = sin((i.local.x + i.local.z) * 9.0 + _Time.y * (0.5 + motion * 0.8));
                surface += baseColor * (0.035 + 0.035 * wave) * (1.0 - roughness);
            }

            if (fmod(flags, 8.0) >= 4.0)
            {
                if (!hasTexture) clip(sin(tile.x * 12.0) * sin(tile.y * 12.0) + 0.2);
                clip(texel.a - 0.5);
            }

            float alpha = hasTexture ? saturate(texel.a * opacity) : opacity;
            #if defined(UNITY_COLORSPACE_GAMMA)
            surface = LinearToGammaSpace(surface);
            #endif
            return fixed4(surface, alpha);
        }
        ENDCG

        // Opaque and alpha-cutout pool. Transparent styles are discarded here.
        Pass
        {
            Tags { "LightMode"="ForwardBase" }
            Cull Back
            ZWrite On
            Blend Off

            CGPROGRAM
            #pragma target 3.5
            #pragma vertex vert
            #pragma fragment frag

            fixed4 frag(v2f i) : SV_Target
            {
                float3 color;
                float opacity;
                float flags;
                fixed4 result = SampleCatalog(i, color, opacity, flags);
                if (fmod(flags, 4.0) >= 2.0)
                    discard;
                return fixed4(result.rgb, 1.0);
            }
            ENDCG
        }

        // Transparent pool. It is intentionally a second pass of the same
        // particle system; per-particle perfect sorting is not guaranteed.
        Pass
        {
            Tags { "Queue"="Transparent" "RenderType"="Transparent" }
            Cull Back
            ZWrite Off
            Blend SrcAlpha OneMinusSrcAlpha

            CGPROGRAM
            #pragma target 3.5
            #pragma vertex vert
            #pragma fragment frag

            fixed4 frag(v2f i) : SV_Target
            {
                float3 color;
                float opacity;
                float flags;
                fixed4 result = SampleCatalog(i, color, opacity, flags);
                if (fmod(flags, 4.0) < 2.0)
                    discard;
                return fixed4(result.rgb, result.a);
            }
            ENDCG
        }
    }
    Fallback Off
}
