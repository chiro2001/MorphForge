//! MF5's self-contained per-Box surface data. No runtime palette registry.
use crate::materials::Material;
use serde::{Deserialize, Serialize};

pub const CAPABILITY: &str = "mf5-runtime-materials/1";
pub const SCALE_REFERENCE: f32 = 0.05;
pub const SCALE_MIN: f32 = 0.0005;
pub const SCALE_MAX: f32 = 0.5;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Surface {
    pub mode: u8,
    pub bytes: [u8; 3],
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Decoded {
    pub color: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub opacity: f32,
    pub emission: f32,
    pub procedural_kind: u8,
    pub noise_scale: f32,
    pub seed: u8,
}
pub fn rgb(bits: u32) -> [u8; 3] {
    [bits as u8, (bits >> 8) as u8, (bits >> 16) as u8]
}
pub fn bits(bytes: [u8; 3]) -> u32 {
    bytes[0] as u32 | ((bytes[1] as u32) << 8) | ((bytes[2] as u32) << 16)
}
pub fn quant(v: f32, max: u32) -> u32 {
    (v.clamp(0., 1.) * max as f32).round() as u32
}
pub fn linear(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
pub fn srgb(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1. / 2.4) - 0.055
    }
}
pub fn tinted(color: [f32; 3], tint: u8) -> [f32; 3] {
    if tint == 0 {
        return color;
    }
    let factors = [0.31, 0.53, 0.79];
    std::array::from_fn(|i| {
        srgb(linear(color[i]) * (0.45 + 0.55 * (tint as f32 * factors[i]).fract())).clamp(0., 1.)
    })
}
impl Surface {
    pub fn catalog(style: u32, tint: u8, motion: u8) -> Self {
        Self {
            mode: 0,
            bytes: rgb(style | ((tint as u32) << 12) | ((motion as u32) << 18)),
        }
    }
    pub fn textured_tint(style: u32, color: [f32; 3]) -> Self {
        let c = quant(color[0], 15) | (quant(color[1], 15) << 4) | (quant(color[2], 15) << 8);
        Self {
            mode: 7,
            bytes: rgb(style | (c << 12)),
        }
    }
    pub fn runtime(m: &Material, tint: u8) -> Option<Self> {
        if !m.textures.is_empty() || m.render_mode == "cutout" {
            return None;
        }
        let c = tinted(m.color, tint);
        if let Some(p) = &m.procedural {
            let mode = match p.kind.as_str() {
                "water" => 3,
                "cloud" => 4,
                "wood" => 5,
                "stone" => 6,
                _ => return None,
            };
            let color = quant(c[0], 15) | (quant(c[1], 15) << 4) | (quant(c[2], 15) << 8);
            let scale = quant(((p.scale / 0.25).log2() / 5.).clamp(0., 1.), 7);
            let value = color
                | (quant(m.roughness, 3) << 12)
                | (quant(1. - m.opacity, 3) << 14)
                | (quant(m.emission / 3., 3) << 16)
                | (scale << 18)
                | (((p.seed % 8) as u32) << 21);
            return Some(Self {
                mode,
                bytes: rgb(value),
            });
        }
        if m.opacity >= 1.
            && m.metallic == 0.
            && m.emission == 0.
            && (m.roughness - 0.9).abs() < 0.0001
        {
            return Some(Self {
                mode: 1,
                bytes: c.map(|x| quant(x, 255) as u8),
            });
        }
        let color = quant(c[0], 31) | (quant(c[1], 63) << 5) | (quant(c[2], 31) << 11);
        Some(Self {
            mode: 2,
            bytes: rgb(color
                | (quant(m.metallic, 3) << 16)
                | (quant(m.roughness, 3) << 18)
                | (quant(1. - m.opacity, 3) << 20)
                | (quant(m.emission / 3., 3) << 22)),
        })
    }
    pub fn alpha(self) -> u8 {
        255 - self.mode
    }
    pub fn decode(self) -> Option<Decoded> {
        let v = bits(self.bytes);
        let mut d = Decoded {
            color: [0.; 3],
            metallic: 0.,
            roughness: 0.9,
            opacity: 1.,
            emission: 0.,
            procedural_kind: 0,
            noise_scale: 1.,
            seed: 0,
        };
        match self.mode {
            1 => d.color = self.bytes.map(|b| b as f32 / 255.),
            2 => {
                d.color = [
                    (v & 31) as f32 / 31.,
                    ((v >> 5) & 63) as f32 / 63.,
                    ((v >> 11) & 31) as f32 / 31.,
                ];
                d.metallic = ((v >> 16) & 3) as f32 / 3.;
                d.roughness = ((v >> 18) & 3) as f32 / 3.;
                d.opacity = 1. - ((v >> 20) & 3) as f32 / 3.;
                d.emission = ((v >> 22) & 3) as f32;
            }
            3..=6 => {
                d.color = [
                    (v & 15) as f32 / 15.,
                    ((v >> 4) & 15) as f32 / 15.,
                    ((v >> 8) & 15) as f32 / 15.,
                ];
                d.roughness = ((v >> 12) & 3) as f32 / 3.;
                d.opacity = 1. - ((v >> 14) & 3) as f32 / 3.;
                d.emission = ((v >> 16) & 3) as f32;
                d.noise_scale = 0.25 * 2f32.powf(((v >> 18) & 7) as f32 * 5. / 7.);
                d.seed = ((v >> 21) & 7) as u8;
                d.procedural_kind = self.mode - 2;
                d.metallic = if self.mode == 3 { 0.2 } else { 0. };
            }
            _ => return None,
        }
        Some(d)
    }
}
/// Signed float uses the full synchronized range for a 1000:1 physical range.
pub fn scale_wire(physical: f32) -> f32 {
    2. * ((physical.clamp(SCALE_MIN, SCALE_MAX) / SCALE_MIN).ln() / (SCALE_MAX / SCALE_MIN).ln())
        - 1.
}
pub fn scale_physical(wire: f32) -> f32 {
    SCALE_MIN * (SCALE_MAX / SCALE_MIN).powf((wire.clamp(-1., 1.) + 1.) * 0.5)
}
