Shader "MorphForgeAvatar/PixelText16" {
    Properties {
        // BEGIN GENERATED MOTION PROPERTIES
        [HideInInspector] _MF_M1_Bank("_MF_M1_Bank",Float)=0
        [HideInInspector] _MF_M1_Enabled("_MF_M1_Enabled",Float)=0
        [HideInInspector] _MF_M1_Base("_MF_M1_Base",Float)=0
        [HideInInspector] _MF_M1_Loop("_MF_M1_Loop",Float)=0
        [HideInInspector] _MF_M1_Elapsed("_MF_M1_Elapsed",Float)=0
        [HideInInspector] _MF_M1B0_Count("_MF_M1B0_Count",Float)=0
        [HideInInspector] _MF_M1B0_Interp("_MF_M1B0_Interp",Float)=0
        [HideInInspector] _MF_M1B0_PivotX("_MF_M1B0_PivotX",Float)=0
        [HideInInspector] _MF_M1B0_PivotY("_MF_M1B0_PivotY",Float)=0
        [HideInInspector] _MF_M1B0_PivotZ("_MF_M1B0_PivotZ",Float)=0
        [HideInInspector] _MF_M1B0_K0T("_MF_M1B0_K0T",Float)=0
        [HideInInspector] _MF_M1B0_K0PX("_MF_M1B0_K0PX",Float)=0
        [HideInInspector] _MF_M1B0_K0PY("_MF_M1B0_K0PY",Float)=0
        [HideInInspector] _MF_M1B0_K0PZ("_MF_M1B0_K0PZ",Float)=0
        [HideInInspector] _MF_M1B0_K0QX("_MF_M1B0_K0QX",Float)=0
        [HideInInspector] _MF_M1B0_K0QY("_MF_M1B0_K0QY",Float)=0
        [HideInInspector] _MF_M1B0_K0QZ("_MF_M1B0_K0QZ",Float)=0
        [HideInInspector] _MF_M1B0_K0QW("_MF_M1B0_K0QW",Float)=0
        [HideInInspector] _MF_M1B0_K0S("_MF_M1B0_K0S",Float)=0
        [HideInInspector] _MF_M1B0_K1T("_MF_M1B0_K1T",Float)=0
        [HideInInspector] _MF_M1B0_K1PX("_MF_M1B0_K1PX",Float)=0
        [HideInInspector] _MF_M1B0_K1PY("_MF_M1B0_K1PY",Float)=0
        [HideInInspector] _MF_M1B0_K1PZ("_MF_M1B0_K1PZ",Float)=0
        [HideInInspector] _MF_M1B0_K1QX("_MF_M1B0_K1QX",Float)=0
        [HideInInspector] _MF_M1B0_K1QY("_MF_M1B0_K1QY",Float)=0
        [HideInInspector] _MF_M1B0_K1QZ("_MF_M1B0_K1QZ",Float)=0
        [HideInInspector] _MF_M1B0_K1QW("_MF_M1B0_K1QW",Float)=0
        [HideInInspector] _MF_M1B0_K1S("_MF_M1B0_K1S",Float)=0
        [HideInInspector] _MF_M1B0_K2T("_MF_M1B0_K2T",Float)=0
        [HideInInspector] _MF_M1B0_K2PX("_MF_M1B0_K2PX",Float)=0
        [HideInInspector] _MF_M1B0_K2PY("_MF_M1B0_K2PY",Float)=0
        [HideInInspector] _MF_M1B0_K2PZ("_MF_M1B0_K2PZ",Float)=0
        [HideInInspector] _MF_M1B0_K2QX("_MF_M1B0_K2QX",Float)=0
        [HideInInspector] _MF_M1B0_K2QY("_MF_M1B0_K2QY",Float)=0
        [HideInInspector] _MF_M1B0_K2QZ("_MF_M1B0_K2QZ",Float)=0
        [HideInInspector] _MF_M1B0_K2QW("_MF_M1B0_K2QW",Float)=0
        [HideInInspector] _MF_M1B0_K2S("_MF_M1B0_K2S",Float)=0
        [HideInInspector] _MF_M1B0_K3T("_MF_M1B0_K3T",Float)=0
        [HideInInspector] _MF_M1B0_K3PX("_MF_M1B0_K3PX",Float)=0
        [HideInInspector] _MF_M1B0_K3PY("_MF_M1B0_K3PY",Float)=0
        [HideInInspector] _MF_M1B0_K3PZ("_MF_M1B0_K3PZ",Float)=0
        [HideInInspector] _MF_M1B0_K3QX("_MF_M1B0_K3QX",Float)=0
        [HideInInspector] _MF_M1B0_K3QY("_MF_M1B0_K3QY",Float)=0
        [HideInInspector] _MF_M1B0_K3QZ("_MF_M1B0_K3QZ",Float)=0
        [HideInInspector] _MF_M1B0_K3QW("_MF_M1B0_K3QW",Float)=0
        [HideInInspector] _MF_M1B0_K3S("_MF_M1B0_K3S",Float)=0
        [HideInInspector] _MF_M1B1_Count("_MF_M1B1_Count",Float)=0
        [HideInInspector] _MF_M1B1_Interp("_MF_M1B1_Interp",Float)=0
        [HideInInspector] _MF_M1B1_PivotX("_MF_M1B1_PivotX",Float)=0
        [HideInInspector] _MF_M1B1_PivotY("_MF_M1B1_PivotY",Float)=0
        [HideInInspector] _MF_M1B1_PivotZ("_MF_M1B1_PivotZ",Float)=0
        [HideInInspector] _MF_M1B1_K0T("_MF_M1B1_K0T",Float)=0
        [HideInInspector] _MF_M1B1_K0PX("_MF_M1B1_K0PX",Float)=0
        [HideInInspector] _MF_M1B1_K0PY("_MF_M1B1_K0PY",Float)=0
        [HideInInspector] _MF_M1B1_K0PZ("_MF_M1B1_K0PZ",Float)=0
        [HideInInspector] _MF_M1B1_K0QX("_MF_M1B1_K0QX",Float)=0
        [HideInInspector] _MF_M1B1_K0QY("_MF_M1B1_K0QY",Float)=0
        [HideInInspector] _MF_M1B1_K0QZ("_MF_M1B1_K0QZ",Float)=0
        [HideInInspector] _MF_M1B1_K0QW("_MF_M1B1_K0QW",Float)=0
        [HideInInspector] _MF_M1B1_K0S("_MF_M1B1_K0S",Float)=0
        [HideInInspector] _MF_M1B1_K1T("_MF_M1B1_K1T",Float)=0
        [HideInInspector] _MF_M1B1_K1PX("_MF_M1B1_K1PX",Float)=0
        [HideInInspector] _MF_M1B1_K1PY("_MF_M1B1_K1PY",Float)=0
        [HideInInspector] _MF_M1B1_K1PZ("_MF_M1B1_K1PZ",Float)=0
        [HideInInspector] _MF_M1B1_K1QX("_MF_M1B1_K1QX",Float)=0
        [HideInInspector] _MF_M1B1_K1QY("_MF_M1B1_K1QY",Float)=0
        [HideInInspector] _MF_M1B1_K1QZ("_MF_M1B1_K1QZ",Float)=0
        [HideInInspector] _MF_M1B1_K1QW("_MF_M1B1_K1QW",Float)=0
        [HideInInspector] _MF_M1B1_K1S("_MF_M1B1_K1S",Float)=0
        [HideInInspector] _MF_M1B1_K2T("_MF_M1B1_K2T",Float)=0
        [HideInInspector] _MF_M1B1_K2PX("_MF_M1B1_K2PX",Float)=0
        [HideInInspector] _MF_M1B1_K2PY("_MF_M1B1_K2PY",Float)=0
        [HideInInspector] _MF_M1B1_K2PZ("_MF_M1B1_K2PZ",Float)=0
        [HideInInspector] _MF_M1B1_K2QX("_MF_M1B1_K2QX",Float)=0
        [HideInInspector] _MF_M1B1_K2QY("_MF_M1B1_K2QY",Float)=0
        [HideInInspector] _MF_M1B1_K2QZ("_MF_M1B1_K2QZ",Float)=0
        [HideInInspector] _MF_M1B1_K2QW("_MF_M1B1_K2QW",Float)=0
        [HideInInspector] _MF_M1B1_K2S("_MF_M1B1_K2S",Float)=0
        [HideInInspector] _MF_M1B1_K3T("_MF_M1B1_K3T",Float)=0
        [HideInInspector] _MF_M1B1_K3PX("_MF_M1B1_K3PX",Float)=0
        [HideInInspector] _MF_M1B1_K3PY("_MF_M1B1_K3PY",Float)=0
        [HideInInspector] _MF_M1B1_K3PZ("_MF_M1B1_K3PZ",Float)=0
        [HideInInspector] _MF_M1B1_K3QX("_MF_M1B1_K3QX",Float)=0
        [HideInInspector] _MF_M1B1_K3QY("_MF_M1B1_K3QY",Float)=0
        [HideInInspector] _MF_M1B1_K3QZ("_MF_M1B1_K3QZ",Float)=0
        [HideInInspector] _MF_M1B1_K3QW("_MF_M1B1_K3QW",Float)=0
        [HideInInspector] _MF_M1B1_K3S("_MF_M1B1_K3S",Float)=0
        [HideInInspector] _MF_M2_Bank("_MF_M2_Bank",Float)=0
        [HideInInspector] _MF_M2_Enabled("_MF_M2_Enabled",Float)=0
        [HideInInspector] _MF_M2_Base("_MF_M2_Base",Float)=0
        [HideInInspector] _MF_M2_Loop("_MF_M2_Loop",Float)=0
        [HideInInspector] _MF_M2_Elapsed("_MF_M2_Elapsed",Float)=0
        [HideInInspector] _MF_M2B0_Count("_MF_M2B0_Count",Float)=0
        [HideInInspector] _MF_M2B0_Interp("_MF_M2B0_Interp",Float)=0
        [HideInInspector] _MF_M2B0_PivotX("_MF_M2B0_PivotX",Float)=0
        [HideInInspector] _MF_M2B0_PivotY("_MF_M2B0_PivotY",Float)=0
        [HideInInspector] _MF_M2B0_PivotZ("_MF_M2B0_PivotZ",Float)=0
        [HideInInspector] _MF_M2B0_K0T("_MF_M2B0_K0T",Float)=0
        [HideInInspector] _MF_M2B0_K0PX("_MF_M2B0_K0PX",Float)=0
        [HideInInspector] _MF_M2B0_K0PY("_MF_M2B0_K0PY",Float)=0
        [HideInInspector] _MF_M2B0_K0PZ("_MF_M2B0_K0PZ",Float)=0
        [HideInInspector] _MF_M2B0_K0QX("_MF_M2B0_K0QX",Float)=0
        [HideInInspector] _MF_M2B0_K0QY("_MF_M2B0_K0QY",Float)=0
        [HideInInspector] _MF_M2B0_K0QZ("_MF_M2B0_K0QZ",Float)=0
        [HideInInspector] _MF_M2B0_K0QW("_MF_M2B0_K0QW",Float)=0
        [HideInInspector] _MF_M2B0_K0S("_MF_M2B0_K0S",Float)=0
        [HideInInspector] _MF_M2B0_K1T("_MF_M2B0_K1T",Float)=0
        [HideInInspector] _MF_M2B0_K1PX("_MF_M2B0_K1PX",Float)=0
        [HideInInspector] _MF_M2B0_K1PY("_MF_M2B0_K1PY",Float)=0
        [HideInInspector] _MF_M2B0_K1PZ("_MF_M2B0_K1PZ",Float)=0
        [HideInInspector] _MF_M2B0_K1QX("_MF_M2B0_K1QX",Float)=0
        [HideInInspector] _MF_M2B0_K1QY("_MF_M2B0_K1QY",Float)=0
        [HideInInspector] _MF_M2B0_K1QZ("_MF_M2B0_K1QZ",Float)=0
        [HideInInspector] _MF_M2B0_K1QW("_MF_M2B0_K1QW",Float)=0
        [HideInInspector] _MF_M2B0_K1S("_MF_M2B0_K1S",Float)=0
        [HideInInspector] _MF_M2B0_K2T("_MF_M2B0_K2T",Float)=0
        [HideInInspector] _MF_M2B0_K2PX("_MF_M2B0_K2PX",Float)=0
        [HideInInspector] _MF_M2B0_K2PY("_MF_M2B0_K2PY",Float)=0
        [HideInInspector] _MF_M2B0_K2PZ("_MF_M2B0_K2PZ",Float)=0
        [HideInInspector] _MF_M2B0_K2QX("_MF_M2B0_K2QX",Float)=0
        [HideInInspector] _MF_M2B0_K2QY("_MF_M2B0_K2QY",Float)=0
        [HideInInspector] _MF_M2B0_K2QZ("_MF_M2B0_K2QZ",Float)=0
        [HideInInspector] _MF_M2B0_K2QW("_MF_M2B0_K2QW",Float)=0
        [HideInInspector] _MF_M2B0_K2S("_MF_M2B0_K2S",Float)=0
        [HideInInspector] _MF_M2B0_K3T("_MF_M2B0_K3T",Float)=0
        [HideInInspector] _MF_M2B0_K3PX("_MF_M2B0_K3PX",Float)=0
        [HideInInspector] _MF_M2B0_K3PY("_MF_M2B0_K3PY",Float)=0
        [HideInInspector] _MF_M2B0_K3PZ("_MF_M2B0_K3PZ",Float)=0
        [HideInInspector] _MF_M2B0_K3QX("_MF_M2B0_K3QX",Float)=0
        [HideInInspector] _MF_M2B0_K3QY("_MF_M2B0_K3QY",Float)=0
        [HideInInspector] _MF_M2B0_K3QZ("_MF_M2B0_K3QZ",Float)=0
        [HideInInspector] _MF_M2B0_K3QW("_MF_M2B0_K3QW",Float)=0
        [HideInInspector] _MF_M2B0_K3S("_MF_M2B0_K3S",Float)=0
        [HideInInspector] _MF_M2B1_Count("_MF_M2B1_Count",Float)=0
        [HideInInspector] _MF_M2B1_Interp("_MF_M2B1_Interp",Float)=0
        [HideInInspector] _MF_M2B1_PivotX("_MF_M2B1_PivotX",Float)=0
        [HideInInspector] _MF_M2B1_PivotY("_MF_M2B1_PivotY",Float)=0
        [HideInInspector] _MF_M2B1_PivotZ("_MF_M2B1_PivotZ",Float)=0
        [HideInInspector] _MF_M2B1_K0T("_MF_M2B1_K0T",Float)=0
        [HideInInspector] _MF_M2B1_K0PX("_MF_M2B1_K0PX",Float)=0
        [HideInInspector] _MF_M2B1_K0PY("_MF_M2B1_K0PY",Float)=0
        [HideInInspector] _MF_M2B1_K0PZ("_MF_M2B1_K0PZ",Float)=0
        [HideInInspector] _MF_M2B1_K0QX("_MF_M2B1_K0QX",Float)=0
        [HideInInspector] _MF_M2B1_K0QY("_MF_M2B1_K0QY",Float)=0
        [HideInInspector] _MF_M2B1_K0QZ("_MF_M2B1_K0QZ",Float)=0
        [HideInInspector] _MF_M2B1_K0QW("_MF_M2B1_K0QW",Float)=0
        [HideInInspector] _MF_M2B1_K0S("_MF_M2B1_K0S",Float)=0
        [HideInInspector] _MF_M2B1_K1T("_MF_M2B1_K1T",Float)=0
        [HideInInspector] _MF_M2B1_K1PX("_MF_M2B1_K1PX",Float)=0
        [HideInInspector] _MF_M2B1_K1PY("_MF_M2B1_K1PY",Float)=0
        [HideInInspector] _MF_M2B1_K1PZ("_MF_M2B1_K1PZ",Float)=0
        [HideInInspector] _MF_M2B1_K1QX("_MF_M2B1_K1QX",Float)=0
        [HideInInspector] _MF_M2B1_K1QY("_MF_M2B1_K1QY",Float)=0
        [HideInInspector] _MF_M2B1_K1QZ("_MF_M2B1_K1QZ",Float)=0
        [HideInInspector] _MF_M2B1_K1QW("_MF_M2B1_K1QW",Float)=0
        [HideInInspector] _MF_M2B1_K1S("_MF_M2B1_K1S",Float)=0
        [HideInInspector] _MF_M2B1_K2T("_MF_M2B1_K2T",Float)=0
        [HideInInspector] _MF_M2B1_K2PX("_MF_M2B1_K2PX",Float)=0
        [HideInInspector] _MF_M2B1_K2PY("_MF_M2B1_K2PY",Float)=0
        [HideInInspector] _MF_M2B1_K2PZ("_MF_M2B1_K2PZ",Float)=0
        [HideInInspector] _MF_M2B1_K2QX("_MF_M2B1_K2QX",Float)=0
        [HideInInspector] _MF_M2B1_K2QY("_MF_M2B1_K2QY",Float)=0
        [HideInInspector] _MF_M2B1_K2QZ("_MF_M2B1_K2QZ",Float)=0
        [HideInInspector] _MF_M2B1_K2QW("_MF_M2B1_K2QW",Float)=0
        [HideInInspector] _MF_M2B1_K2S("_MF_M2B1_K2S",Float)=0
        [HideInInspector] _MF_M2B1_K3T("_MF_M2B1_K3T",Float)=0
        [HideInInspector] _MF_M2B1_K3PX("_MF_M2B1_K3PX",Float)=0
        [HideInInspector] _MF_M2B1_K3PY("_MF_M2B1_K3PY",Float)=0
        [HideInInspector] _MF_M2B1_K3PZ("_MF_M2B1_K3PZ",Float)=0
        [HideInInspector] _MF_M2B1_K3QX("_MF_M2B1_K3QX",Float)=0
        [HideInInspector] _MF_M2B1_K3QY("_MF_M2B1_K3QY",Float)=0
        [HideInInspector] _MF_M2B1_K3QZ("_MF_M2B1_K3QZ",Float)=0
        [HideInInspector] _MF_M2B1_K3QW("_MF_M2B1_K3QW",Float)=0
        [HideInInspector] _MF_M2B1_K3S("_MF_M2B1_K3S",Float)=0
        [HideInInspector] _MF_M3_Bank("_MF_M3_Bank",Float)=0
        [HideInInspector] _MF_M3_Enabled("_MF_M3_Enabled",Float)=0
        [HideInInspector] _MF_M3_Base("_MF_M3_Base",Float)=0
        [HideInInspector] _MF_M3_Loop("_MF_M3_Loop",Float)=0
        [HideInInspector] _MF_M3_Elapsed("_MF_M3_Elapsed",Float)=0
        [HideInInspector] _MF_M3B0_Count("_MF_M3B0_Count",Float)=0
        [HideInInspector] _MF_M3B0_Interp("_MF_M3B0_Interp",Float)=0
        [HideInInspector] _MF_M3B0_PivotX("_MF_M3B0_PivotX",Float)=0
        [HideInInspector] _MF_M3B0_PivotY("_MF_M3B0_PivotY",Float)=0
        [HideInInspector] _MF_M3B0_PivotZ("_MF_M3B0_PivotZ",Float)=0
        [HideInInspector] _MF_M3B0_K0T("_MF_M3B0_K0T",Float)=0
        [HideInInspector] _MF_M3B0_K0PX("_MF_M3B0_K0PX",Float)=0
        [HideInInspector] _MF_M3B0_K0PY("_MF_M3B0_K0PY",Float)=0
        [HideInInspector] _MF_M3B0_K0PZ("_MF_M3B0_K0PZ",Float)=0
        [HideInInspector] _MF_M3B0_K0QX("_MF_M3B0_K0QX",Float)=0
        [HideInInspector] _MF_M3B0_K0QY("_MF_M3B0_K0QY",Float)=0
        [HideInInspector] _MF_M3B0_K0QZ("_MF_M3B0_K0QZ",Float)=0
        [HideInInspector] _MF_M3B0_K0QW("_MF_M3B0_K0QW",Float)=0
        [HideInInspector] _MF_M3B0_K0S("_MF_M3B0_K0S",Float)=0
        [HideInInspector] _MF_M3B0_K1T("_MF_M3B0_K1T",Float)=0
        [HideInInspector] _MF_M3B0_K1PX("_MF_M3B0_K1PX",Float)=0
        [HideInInspector] _MF_M3B0_K1PY("_MF_M3B0_K1PY",Float)=0
        [HideInInspector] _MF_M3B0_K1PZ("_MF_M3B0_K1PZ",Float)=0
        [HideInInspector] _MF_M3B0_K1QX("_MF_M3B0_K1QX",Float)=0
        [HideInInspector] _MF_M3B0_K1QY("_MF_M3B0_K1QY",Float)=0
        [HideInInspector] _MF_M3B0_K1QZ("_MF_M3B0_K1QZ",Float)=0
        [HideInInspector] _MF_M3B0_K1QW("_MF_M3B0_K1QW",Float)=0
        [HideInInspector] _MF_M3B0_K1S("_MF_M3B0_K1S",Float)=0
        [HideInInspector] _MF_M3B0_K2T("_MF_M3B0_K2T",Float)=0
        [HideInInspector] _MF_M3B0_K2PX("_MF_M3B0_K2PX",Float)=0
        [HideInInspector] _MF_M3B0_K2PY("_MF_M3B0_K2PY",Float)=0
        [HideInInspector] _MF_M3B0_K2PZ("_MF_M3B0_K2PZ",Float)=0
        [HideInInspector] _MF_M3B0_K2QX("_MF_M3B0_K2QX",Float)=0
        [HideInInspector] _MF_M3B0_K2QY("_MF_M3B0_K2QY",Float)=0
        [HideInInspector] _MF_M3B0_K2QZ("_MF_M3B0_K2QZ",Float)=0
        [HideInInspector] _MF_M3B0_K2QW("_MF_M3B0_K2QW",Float)=0
        [HideInInspector] _MF_M3B0_K2S("_MF_M3B0_K2S",Float)=0
        [HideInInspector] _MF_M3B0_K3T("_MF_M3B0_K3T",Float)=0
        [HideInInspector] _MF_M3B0_K3PX("_MF_M3B0_K3PX",Float)=0
        [HideInInspector] _MF_M3B0_K3PY("_MF_M3B0_K3PY",Float)=0
        [HideInInspector] _MF_M3B0_K3PZ("_MF_M3B0_K3PZ",Float)=0
        [HideInInspector] _MF_M3B0_K3QX("_MF_M3B0_K3QX",Float)=0
        [HideInInspector] _MF_M3B0_K3QY("_MF_M3B0_K3QY",Float)=0
        [HideInInspector] _MF_M3B0_K3QZ("_MF_M3B0_K3QZ",Float)=0
        [HideInInspector] _MF_M3B0_K3QW("_MF_M3B0_K3QW",Float)=0
        [HideInInspector] _MF_M3B0_K3S("_MF_M3B0_K3S",Float)=0
        [HideInInspector] _MF_M3B1_Count("_MF_M3B1_Count",Float)=0
        [HideInInspector] _MF_M3B1_Interp("_MF_M3B1_Interp",Float)=0
        [HideInInspector] _MF_M3B1_PivotX("_MF_M3B1_PivotX",Float)=0
        [HideInInspector] _MF_M3B1_PivotY("_MF_M3B1_PivotY",Float)=0
        [HideInInspector] _MF_M3B1_PivotZ("_MF_M3B1_PivotZ",Float)=0
        [HideInInspector] _MF_M3B1_K0T("_MF_M3B1_K0T",Float)=0
        [HideInInspector] _MF_M3B1_K0PX("_MF_M3B1_K0PX",Float)=0
        [HideInInspector] _MF_M3B1_K0PY("_MF_M3B1_K0PY",Float)=0
        [HideInInspector] _MF_M3B1_K0PZ("_MF_M3B1_K0PZ",Float)=0
        [HideInInspector] _MF_M3B1_K0QX("_MF_M3B1_K0QX",Float)=0
        [HideInInspector] _MF_M3B1_K0QY("_MF_M3B1_K0QY",Float)=0
        [HideInInspector] _MF_M3B1_K0QZ("_MF_M3B1_K0QZ",Float)=0
        [HideInInspector] _MF_M3B1_K0QW("_MF_M3B1_K0QW",Float)=0
        [HideInInspector] _MF_M3B1_K0S("_MF_M3B1_K0S",Float)=0
        [HideInInspector] _MF_M3B1_K1T("_MF_M3B1_K1T",Float)=0
        [HideInInspector] _MF_M3B1_K1PX("_MF_M3B1_K1PX",Float)=0
        [HideInInspector] _MF_M3B1_K1PY("_MF_M3B1_K1PY",Float)=0
        [HideInInspector] _MF_M3B1_K1PZ("_MF_M3B1_K1PZ",Float)=0
        [HideInInspector] _MF_M3B1_K1QX("_MF_M3B1_K1QX",Float)=0
        [HideInInspector] _MF_M3B1_K1QY("_MF_M3B1_K1QY",Float)=0
        [HideInInspector] _MF_M3B1_K1QZ("_MF_M3B1_K1QZ",Float)=0
        [HideInInspector] _MF_M3B1_K1QW("_MF_M3B1_K1QW",Float)=0
        [HideInInspector] _MF_M3B1_K1S("_MF_M3B1_K1S",Float)=0
        [HideInInspector] _MF_M3B1_K2T("_MF_M3B1_K2T",Float)=0
        [HideInInspector] _MF_M3B1_K2PX("_MF_M3B1_K2PX",Float)=0
        [HideInInspector] _MF_M3B1_K2PY("_MF_M3B1_K2PY",Float)=0
        [HideInInspector] _MF_M3B1_K2PZ("_MF_M3B1_K2PZ",Float)=0
        [HideInInspector] _MF_M3B1_K2QX("_MF_M3B1_K2QX",Float)=0
        [HideInInspector] _MF_M3B1_K2QY("_MF_M3B1_K2QY",Float)=0
        [HideInInspector] _MF_M3B1_K2QZ("_MF_M3B1_K2QZ",Float)=0
        [HideInInspector] _MF_M3B1_K2QW("_MF_M3B1_K2QW",Float)=0
        [HideInInspector] _MF_M3B1_K2S("_MF_M3B1_K2S",Float)=0
        [HideInInspector] _MF_M3B1_K3T("_MF_M3B1_K3T",Float)=0
        [HideInInspector] _MF_M3B1_K3PX("_MF_M3B1_K3PX",Float)=0
        [HideInInspector] _MF_M3B1_K3PY("_MF_M3B1_K3PY",Float)=0
        [HideInInspector] _MF_M3B1_K3PZ("_MF_M3B1_K3PZ",Float)=0
        [HideInInspector] _MF_M3B1_K3QX("_MF_M3B1_K3QX",Float)=0
        [HideInInspector] _MF_M3B1_K3QY("_MF_M3B1_K3QY",Float)=0
        [HideInInspector] _MF_M3B1_K3QZ("_MF_M3B1_K3QZ",Float)=0
        [HideInInspector] _MF_M3B1_K3QW("_MF_M3B1_K3QW",Float)=0
        [HideInInspector] _MF_M3B1_K3S("_MF_M3B1_K3S",Float)=0
        // END GENERATED MOTION PROPERTIES
        _MotionGroup("Motion group",Float)=0
        _FontZH("Chinese binary atlas",2D)="black" {}
        _FontJP("Japanese binary atlas",2D)="black" {}
        _WidthsZH("Chinese widths",2D)="white" {}
        _WidthsJP("Japanese widths",2D)="white" {}
        _PosX("X",Float)=0
        _PosY("Y",Float)=24
        _PosZ("Z",Float)=0
        _RotX("Rotation X",Float)=0
        _RotY("Rotation Y",Float)=0
        _RotZ("Rotation Z",Float)=0
        _Width("Width",Float)=128
        _Height("Height",Float)=32
        _FontSize("Font size",Float)=8
        _Spacing("Letter spacing",Float)=0
        _Gap("Line gap",Float)=2
        _Align("Alignment",Float)=0
        _Vertical("Vertical alignment",Float)=0
        _Font("Font",Float)=0
        _Count("Character slots",Float)=0
        _Red("Red",Float)=255
        _Green("Green",Float)=255
        _Blue("Blue",Float)=255
        _Opacity("Opacity",Float)=255
        _BackR("Background R",Float)=0
        _BackG("Background G",Float)=0
        _BackB("Background B",Float)=0
        _BackA("Background A",Float)=0
        [HideInInspector] _G000("Glyph 0",Float)=0
        [HideInInspector] _G001("Glyph 1",Float)=0
        [HideInInspector] _G002("Glyph 2",Float)=0
        [HideInInspector] _G003("Glyph 3",Float)=0
        [HideInInspector] _G004("Glyph 4",Float)=0
        [HideInInspector] _G005("Glyph 5",Float)=0
        [HideInInspector] _G006("Glyph 6",Float)=0
        [HideInInspector] _G007("Glyph 7",Float)=0
        [HideInInspector] _G008("Glyph 8",Float)=0
        [HideInInspector] _G009("Glyph 9",Float)=0
        [HideInInspector] _G010("Glyph 10",Float)=0
        [HideInInspector] _G011("Glyph 11",Float)=0
        [HideInInspector] _G012("Glyph 12",Float)=0
        [HideInInspector] _G013("Glyph 13",Float)=0
        [HideInInspector] _G014("Glyph 14",Float)=0
        [HideInInspector] _G015("Glyph 15",Float)=0
        [HideInInspector] _G016("Glyph 16",Float)=0
        [HideInInspector] _G017("Glyph 17",Float)=0
        [HideInInspector] _G018("Glyph 18",Float)=0
        [HideInInspector] _G019("Glyph 19",Float)=0
        [HideInInspector] _G020("Glyph 20",Float)=0
        [HideInInspector] _G021("Glyph 21",Float)=0
        [HideInInspector] _G022("Glyph 22",Float)=0
        [HideInInspector] _G023("Glyph 23",Float)=0
        [HideInInspector] _G024("Glyph 24",Float)=0
        [HideInInspector] _G025("Glyph 25",Float)=0
        [HideInInspector] _G026("Glyph 26",Float)=0
        [HideInInspector] _G027("Glyph 27",Float)=0
        [HideInInspector] _G028("Glyph 28",Float)=0
        [HideInInspector] _G029("Glyph 29",Float)=0
        [HideInInspector] _G030("Glyph 30",Float)=0
        [HideInInspector] _G031("Glyph 31",Float)=0
        [HideInInspector] _G032("Glyph 32",Float)=0
        [HideInInspector] _G033("Glyph 33",Float)=0
        [HideInInspector] _G034("Glyph 34",Float)=0
        [HideInInspector] _G035("Glyph 35",Float)=0
        [HideInInspector] _G036("Glyph 36",Float)=0
        [HideInInspector] _G037("Glyph 37",Float)=0
        [HideInInspector] _G038("Glyph 38",Float)=0
        [HideInInspector] _G039("Glyph 39",Float)=0
        [HideInInspector] _G040("Glyph 40",Float)=0
        [HideInInspector] _G041("Glyph 41",Float)=0
        [HideInInspector] _G042("Glyph 42",Float)=0
        [HideInInspector] _G043("Glyph 43",Float)=0
        [HideInInspector] _G044("Glyph 44",Float)=0
        [HideInInspector] _G045("Glyph 45",Float)=0
        [HideInInspector] _G046("Glyph 46",Float)=0
        [HideInInspector] _G047("Glyph 47",Float)=0
        [HideInInspector] _G048("Glyph 48",Float)=0
        [HideInInspector] _G049("Glyph 49",Float)=0
        [HideInInspector] _G050("Glyph 50",Float)=0
        [HideInInspector] _G051("Glyph 51",Float)=0
        [HideInInspector] _G052("Glyph 52",Float)=0
        [HideInInspector] _G053("Glyph 53",Float)=0
        [HideInInspector] _G054("Glyph 54",Float)=0
        [HideInInspector] _G055("Glyph 55",Float)=0
        [HideInInspector] _G056("Glyph 56",Float)=0
        [HideInInspector] _G057("Glyph 57",Float)=0
        [HideInInspector] _G058("Glyph 58",Float)=0
        [HideInInspector] _G059("Glyph 59",Float)=0
        [HideInInspector] _G060("Glyph 60",Float)=0
        [HideInInspector] _G061("Glyph 61",Float)=0
        [HideInInspector] _G062("Glyph 62",Float)=0
        [HideInInspector] _G063("Glyph 63",Float)=0
        [HideInInspector] _G064("Glyph 64",Float)=0
        [HideInInspector] _G065("Glyph 65",Float)=0
        [HideInInspector] _G066("Glyph 66",Float)=0
        [HideInInspector] _G067("Glyph 67",Float)=0
        [HideInInspector] _G068("Glyph 68",Float)=0
        [HideInInspector] _G069("Glyph 69",Float)=0
        [HideInInspector] _G070("Glyph 70",Float)=0
        [HideInInspector] _G071("Glyph 71",Float)=0
        [HideInInspector] _G072("Glyph 72",Float)=0
        [HideInInspector] _G073("Glyph 73",Float)=0
        [HideInInspector] _G074("Glyph 74",Float)=0
        [HideInInspector] _G075("Glyph 75",Float)=0
        [HideInInspector] _G076("Glyph 76",Float)=0
        [HideInInspector] _G077("Glyph 77",Float)=0
        [HideInInspector] _G078("Glyph 78",Float)=0
        [HideInInspector] _G079("Glyph 79",Float)=0
        [HideInInspector] _G080("Glyph 80",Float)=0
        [HideInInspector] _G081("Glyph 81",Float)=0
        [HideInInspector] _G082("Glyph 82",Float)=0
        [HideInInspector] _G083("Glyph 83",Float)=0
        [HideInInspector] _G084("Glyph 84",Float)=0
        [HideInInspector] _G085("Glyph 85",Float)=0
        [HideInInspector] _G086("Glyph 86",Float)=0
        [HideInInspector] _G087("Glyph 87",Float)=0
        [HideInInspector] _G088("Glyph 88",Float)=0
        [HideInInspector] _G089("Glyph 89",Float)=0
        [HideInInspector] _G090("Glyph 90",Float)=0
        [HideInInspector] _G091("Glyph 91",Float)=0
        [HideInInspector] _G092("Glyph 92",Float)=0
        [HideInInspector] _G093("Glyph 93",Float)=0
        [HideInInspector] _G094("Glyph 94",Float)=0
        [HideInInspector] _G095("Glyph 95",Float)=0
        [HideInInspector] _G096("Glyph 96",Float)=0
        [HideInInspector] _G097("Glyph 97",Float)=0
        [HideInInspector] _G098("Glyph 98",Float)=0
        [HideInInspector] _G099("Glyph 99",Float)=0
        [HideInInspector] _G100("Glyph 100",Float)=0
        [HideInInspector] _G101("Glyph 101",Float)=0
        [HideInInspector] _G102("Glyph 102",Float)=0
        [HideInInspector] _G103("Glyph 103",Float)=0
        [HideInInspector] _G104("Glyph 104",Float)=0
        [HideInInspector] _G105("Glyph 105",Float)=0
        [HideInInspector] _G106("Glyph 106",Float)=0
        [HideInInspector] _G107("Glyph 107",Float)=0
        [HideInInspector] _G108("Glyph 108",Float)=0
        [HideInInspector] _G109("Glyph 109",Float)=0
        [HideInInspector] _G110("Glyph 110",Float)=0
        [HideInInspector] _G111("Glyph 111",Float)=0
        [HideInInspector] _G112("Glyph 112",Float)=0
        [HideInInspector] _G113("Glyph 113",Float)=0
        [HideInInspector] _G114("Glyph 114",Float)=0
        [HideInInspector] _G115("Glyph 115",Float)=0
        [HideInInspector] _G116("Glyph 116",Float)=0
        [HideInInspector] _G117("Glyph 117",Float)=0
        [HideInInspector] _G118("Glyph 118",Float)=0
        [HideInInspector] _G119("Glyph 119",Float)=0
        [HideInInspector] _G120("Glyph 120",Float)=0
        [HideInInspector] _G121("Glyph 121",Float)=0
        [HideInInspector] _G122("Glyph 122",Float)=0
        [HideInInspector] _G123("Glyph 123",Float)=0
        [HideInInspector] _G124("Glyph 124",Float)=0
        [HideInInspector] _G125("Glyph 125",Float)=0
        [HideInInspector] _G126("Glyph 126",Float)=0
        [HideInInspector] _G127("Glyph 127",Float)=0
    }
    SubShader {
        Tags {"Queue"="Transparent" "RenderType"="Transparent" "DisableBatching"="True"}
        Cull Off ZWrite Off Blend SrcAlpha OneMinusSrcAlpha
        Pass {
        CGPROGRAM
        #pragma vertex vert
        #pragma fragment frag
        #pragma target 4.0
        #include "UnityCG.cginc"
        #include "MorphForgeMotion.cginc"
        sampler2D _FontZH,_FontJP,_WidthsZH,_WidthsJP;
        float _MotionGroup;
        float _PosX,_PosY,_PosZ,_RotX,_RotY,_RotZ,_Width,_Height,_FontSize,_Spacing,_Gap,_Align,_Vertical,_Font,_Count;
        float _Red,_Green,_Blue,_Opacity,_BackR,_BackG,_BackB,_BackA;
        float _G000,_G001,_G002,_G003,_G004,_G005,_G006,_G007,_G008,_G009,_G010,_G011,_G012,_G013,_G014,_G015;
        float _G016,_G017,_G018,_G019,_G020,_G021,_G022,_G023,_G024,_G025,_G026,_G027,_G028,_G029,_G030,_G031;
        float _G032,_G033,_G034,_G035,_G036,_G037,_G038,_G039,_G040,_G041,_G042,_G043,_G044,_G045,_G046,_G047;
        float _G048,_G049,_G050,_G051,_G052,_G053,_G054,_G055,_G056,_G057,_G058,_G059,_G060,_G061,_G062,_G063;
        float _G064,_G065,_G066,_G067,_G068,_G069,_G070,_G071,_G072,_G073,_G074,_G075,_G076,_G077,_G078,_G079;
        float _G080,_G081,_G082,_G083,_G084,_G085,_G086,_G087,_G088,_G089,_G090,_G091,_G092,_G093,_G094,_G095;
        float _G096,_G097,_G098,_G099,_G100,_G101,_G102,_G103,_G104,_G105,_G106,_G107,_G108,_G109,_G110,_G111;
        float _G112,_G113,_G114,_G115,_G116,_G117,_G118,_G119,_G120,_G121,_G122,_G123,_G124,_G125,_G126,_G127;
        struct appdata {float4 vertex:POSITION;float2 uv:TEXCOORD0;float2 index:TEXCOORD1;UNITY_VERTEX_INPUT_INSTANCE_ID};
        struct v2f {float4 pos:SV_POSITION;float2 uv:TEXCOORD0;float3 glyph:TEXCOORD1;float2 panel:TEXCOORD2;UNITY_VERTEX_OUTPUT_STEREO};
        float width(float code) {float2 uv=(float2(fmod(code,256),floor(code/256))+.5)/256;return floor(lerp(tex2Dlod(_WidthsZH,float4(uv,0,0)).r,tex2Dlod(_WidthsJP,float4(uv,0,0)).r,step(.5,_Font))*255+.5);}
        float3 rotateXYZ(float3 p) {
            float3 s,c;sincos(float3(_RotX,_RotY,_RotZ)*0.01745329252,s,c);
            // Same intrinsic XYZ quaternion as Rust/Three: Rz is applied first.
            p=float3(c.z*p.x-s.z*p.y,s.z*p.x+c.z*p.y,p.z);
            p=float3(c.y*p.x+s.y*p.z,p.y,-s.y*p.x+c.y*p.z);
            return float3(p.x,c.x*p.y-s.x*p.z,s.x*p.y+c.x*p.z);
        }
        v2f vert(appdata v) {
            v2f o;UNITY_SETUP_INSTANCE_ID(v);UNITY_INITIALIZE_OUTPUT(v2f,o);UNITY_INITIALIZE_VERTEX_OUTPUT_STEREO(o);
            float glyphs[128]={_G000,_G001,_G002,_G003,_G004,_G005,_G006,_G007,_G008,_G009,_G010,_G011,_G012,_G013,_G014,_G015,_G016,_G017,_G018,_G019,_G020,_G021,_G022,_G023,_G024,_G025,_G026,_G027,_G028,_G029,_G030,_G031,_G032,_G033,_G034,_G035,_G036,_G037,_G038,_G039,_G040,_G041,_G042,_G043,_G044,_G045,_G046,_G047,_G048,_G049,_G050,_G051,_G052,_G053,_G054,_G055,_G056,_G057,_G058,_G059,_G060,_G061,_G062,_G063,_G064,_G065,_G066,_G067,_G068,_G069,_G070,_G071,_G072,_G073,_G074,_G075,_G076,_G077,_G078,_G079,_G080,_G081,_G082,_G083,_G084,_G085,_G086,_G087,_G088,_G089,_G090,_G091,_G092,_G093,_G094,_G095,_G096,_G097,_G098,_G099,_G100,_G101,_G102,_G103,_G104,_G105,_G106,_G107,_G108,_G109,_G110,_G111,_G112,_G113,_G114,_G115,_G116,_G117,_G118,_G119,_G120,_G121,_G122,_G123,_G124,_G125,_G126,_G127};
            int index=(int)v.index.x;int count=clamp((int)floor(_Count+.5),0,128);
            float x=0,row=0,lineWidth=0,totalRows=1;
            float code=0,pixels=16;
            // All vertices compute the same bounded line layout. Newlines are
            // inserted by the Rust layout, including word/overflow decisions.
            [loop]for(int i=0;i<count;i++) {
                float g=floor(glyphs[i]+.5);
                if(i<index){if(g==10){x=0;row++;}else x+=width(g)*_FontSize/16+_Spacing;}
                if(i==index){code=g;pixels=width(g);}
                if(g==10)totalRows++;
            }
            int start=index;[loop]for(int j=index-1;j>=0;j--){if(floor(glyphs[j]+.5)==10)break;start=j;}
            [loop]for(int k=start;k<count;k++){float g=floor(glyphs[k]+.5);if(g==10)break;lineWidth+=width(g)*_FontSize/16+_Spacing;}
            lineWidth=max(0,lineWidth-_Spacing);
            float shift=_Align<.5?0:(_Align<1.5?(_Width-lineWidth)*.5:_Width-lineWidth);
            float visibleHeight=totalRows*(_FontSize+_Gap)-_Gap;
            float top=_Vertical<.5?_Height:(_Vertical<1.5?(_Height+visibleHeight)*.5:visibleHeight);
            float2 p=float2(x+shift+v.uv.x*pixels*_FontSize/16-_Width*.5,top-row*(_FontSize+_Gap)-_FontSize+v.uv.y*_FontSize);
            if(index>=128){p=float2((.5-v.uv.x)*_Width,v.uv.y*_Height);code=-1;}
            float3 origin=float3(_PosX,_PosY-128,_PosZ)*.25;
            float3 localCamera=mul(unity_WorldToObject,float4(_WorldSpaceCameraPos,1)).xyz;
            MFMotionPose motion=MFGetMotion(_MotionGroup,.25);float3 effectiveOrigin=MFApplyPose(origin,motion);float side=dot(localCamera-effectiveOrigin,MFRotateVector(rotateXYZ(float3(0,0,-1)),motion.rotation))>=0?1:-1;
            float3 local=MFApplyPose(origin+rotateXYZ(float3(p.x*side,p.y,0)*.25),motion);
            o.pos=UnityObjectToClipPos(float4(local,1));o.panel=p;o.uv=v.uv;o.glyph=float3(code,pixels,index>=count&&index<128?0:1);
            if(code==10 || (index<128 && code<.5))o.glyph.z=0;
            return o;
        }
        float4 frag(v2f i):SV_Target {
            clip(i.glyph.z-.5);clip(_Count-.5);
            clip(_Width*.5-abs(i.panel.x));clip(i.panel.y);clip(_Height-i.panel.y);
            if(i.glyph.x<0){float3 bg=float3(_BackR,_BackG,_BackB)/255;
            #ifndef UNITY_COLORSPACE_GAMMA
            bg=GammaToLinearSpace(bg);
            #endif
            return float4(bg,_BackA*_Opacity/65025);}

            float col=min(i.glyph.y-1,floor(i.uv.x*i.glyph.y));float row=min(15,floor((1-i.uv.y)*16));
            float address=i.glyph.x*32+row*2+floor(col/8);float2 uv=(float2(fmod(address,2048),floor(address/2048))+.5)/float2(2048,1024);
            float packed=floor(lerp(tex2D(_FontZH,uv).r,tex2D(_FontJP,uv).r,step(.5,_Font))*255+.5);
            float bit=fmod(floor(packed/exp2(7-fmod(col,8))),2);clip(bit-.5);
            float3 color=float3(_Red,_Green,_Blue)/255;
            #ifndef UNITY_COLORSPACE_GAMMA
            color=GammaToLinearSpace(color);
            #endif
            return float4(color,_Opacity/255);
        }
        ENDCG
        }
    }
}
