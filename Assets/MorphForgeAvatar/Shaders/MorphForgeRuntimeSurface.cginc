#ifndef MORPHFORGE_RUNTIME_SURFACE
#define MORPHFORGE_RUNTIME_SURFACE
// MF5 mode is a birth-time alpha data byte, never visual opacity.
float MF5Mode(float4 data, float wire) { return wire>4.5 ? clamp(255.0-floor(saturate(data.a)*255.0+0.5),0.0,7.0) : 0.0; }
float3 MF5Bytes(float4 data) {return floor(saturate(data.rgb)*255.0+0.5);}
float MF5Bits(float4 data) {float3 b=MF5Bytes(data);return b.r+256.0*b.g+65536.0*b.b;}
float MF5Field(float v,float shift,float range) {return fmod(floor(v/exp2(shift)),range);}
float4 MF5Codes(float4 data,float wire,float mode)
{
    if(wire<3.5)return floor(saturate(data)*63.0+0.5);
    float3 b=MF5Bytes(data);float style=b.r+256.0*fmod(b.g,16.0);
    if(mode>6.5)return float4(fmod(style,64.0),floor(style/64.0),0,0);
    return float4(fmod(style,64.0),floor(style/64.0),floor(b.g/16.0)+16.0*fmod(b.b,4.0),floor(b.b/4.0));
}
void MF5Material(float4 data,float mode,inout float4 m1,inout float4 m2,out float kind,out float noiseScale,out float seed)
{
    kind=0;noiseScale=1;seed=0;float v=MF5Bits(data);
    if(mode<0.5)return;
    if(mode>6.5) {m1.rgb=float3(MF5Field(v,12,16),MF5Field(v,16,16),MF5Field(v,20,16))/15.0;return;}
    m2=float4(0,.9,0,0);m1=float4(MF5Bytes(data)/255.0,1.0);
    if(mode>1.5&&mode<2.5)
    {
        m1.rgb=float3(MF5Field(v,0,32)/31.0,MF5Field(v,5,64)/63.0,MF5Field(v,11,32)/31.0);
        m2.x=MF5Field(v,16,4)/3.0;m2.y=MF5Field(v,18,4)/3.0;m1.a=1.0-MF5Field(v,20,4)/3.0;m2.z=MF5Field(v,22,4);
    }
    else if(mode>2.5)
    {
        m1.rgb=float3(MF5Field(v,0,16),MF5Field(v,4,16),MF5Field(v,8,16))/15.0;
        m2.x=mode<3.5?.2:0.0;m2.y=MF5Field(v,12,4)/3.0;m1.a=1.0-MF5Field(v,14,4)/3.0;m2.z=MF5Field(v,16,4);
        noiseScale=.25*exp2(MF5Field(v,18,8)*5.0/7.0);seed=MF5Field(v,21,8);kind=mode-2.0;
    }
    m2.w=m1.a<.999?2.0:0.0;
}
float MF5Hash(float2 p,float seed) {return frac(sin(dot(p,float2(127.1,311.7))+seed*74.7)*43758.5453);}
float MF5Noise(float2 p,float seed)
{
    float2 i=floor(p),f=frac(p);f=f*f*(3.0-2.0*f);
    return lerp(lerp(MF5Hash(i,seed),MF5Hash(i+float2(1,0),seed),f.x),lerp(MF5Hash(i+float2(0,1),seed),MF5Hash(i+1,seed),f.x),f.y);
}
float MF5Pattern(float kind,float2 logical,float face,float scale,float seed,float time)
{
    float2 p=logical/max(scale,.01);float n=MF5Noise(p+face*17.0,seed);
    if(kind<.5)return 1.0;
    if(kind<1.5) {float ripple=sin(p.x*3.0+p.y*2.0+time*1.4+seed);float flow=MF5Noise(p*.7+float2(time*.14,time*.055),seed);return .78+.13*ripple+.2*flow;}
    if(kind<2.5) {float cloud=.6*MF5Noise(p*.55+time*.035,seed)+.4*MF5Noise(p*1.4-time*.02,seed+2.);return .72+.36*cloud;}
    if(kind<3.5) {float phase=(p.x+.15*sin(p.y*.7+seed))*7.0+seed;float attenuation=1.0-smoothstep(.1,3.14,length(float2(ddx(phase),ddy(phase))));return .82+.17*sin(phase)*attenuation+.07*n;}
    return .74+.35*(.65*n+.35*MF5Noise(p*3.,seed+3.));
}
#endif
