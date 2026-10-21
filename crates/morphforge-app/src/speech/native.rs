//! Native Windows capture. Never changes the default system audio device.
use super::Frame;
use anyhow::{bail, Result};
#[cfg(windows)]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde_json::{json, Value};
#[cfg(windows)]
use std::sync::atomic::Ordering;
use std::{
    sync::{atomic::AtomicU8, mpsc::SyncSender, Arc},
    time::Instant,
};
#[cfg(windows)]
pub type Capture = cpal::Stream;
#[cfg(not(windows))]
pub struct Capture;
pub fn devices() -> Result<Value> {
    #[cfg(windows)]
    {
        let host = cpal::default_host();
        let mut devices = vec![];
        for d in host.input_devices()? {
            if let Ok(name) = d.name() {
                devices.push(json!({"name":name}));
            }
        }
        return Ok(json!({"native":true,"devices":devices}));
    }
    #[cfg(not(windows))]
    {
        Ok(json!({"native":false,"devices":[],"browserCapture":true}))
    }
}
pub fn capture(
    name: Option<&str>,
    tx: SyncSender<Frame>,
    stop: Arc<AtomicU8>,
    start: Instant,
) -> Result<Capture> {
    #[cfg(not(windows))]
    {
        let _ = (name, tx, stop, start);
        bail!("此平台使用浏览器麦克风输入");
    }
    #[cfg(windows)]
    {
        let host = cpal::default_host();
        let device = if let Some(name) = name {
            host.input_devices()?
                .find(|d| d.name().ok().as_deref() == Some(name))
                .ok_or_else(|| anyhow::anyhow!("选择的输入设备不存在"))?
        } else {
            host.default_input_device()
                .ok_or_else(|| anyhow::anyhow!("没有输入设备"))?
        };
        let mut ranges = device.supported_input_configs()?.collect::<Vec<_>>();
        ranges.sort_by_key(|r| {
            let distance = if 16000 < r.min_sample_rate().0 {
                r.min_sample_rate().0 - 16000
            } else {
                16000u32.saturating_sub(r.max_sample_rate().0)
            };
            (distance, r.channels())
        });
        let range = ranges
            .first()
            .ok_or_else(|| anyhow::anyhow!("输入设备没有可用采样格式"))?;
        let rate = 16000u32.clamp(range.min_sample_rate().0, range.max_sample_rate().0);
        let format = range.sample_format();
        let cfg = range
            .clone()
            .with_sample_rate(cpal::SampleRate(rate))
            .config();
        let channels = cfg.channels as usize;
        let resampler = Resampler::new(rate, channels, tx, stop);
        let stream = match format {
            cpal::SampleFormat::I8 => build::<i8>(&device, &cfg, resampler, start)?,
            cpal::SampleFormat::I16 => build::<i16>(&device, &cfg, resampler, start)?,
            cpal::SampleFormat::I32 => build::<i32>(&device, &cfg, resampler, start)?,
            cpal::SampleFormat::I64 => build::<i64>(&device, &cfg, resampler, start)?,
            cpal::SampleFormat::U8 => build::<u8>(&device, &cfg, resampler, start)?,
            cpal::SampleFormat::U16 => build::<u16>(&device, &cfg, resampler, start)?,
            cpal::SampleFormat::U32 => build::<u32>(&device, &cfg, resampler, start)?,
            cpal::SampleFormat::U64 => build::<u64>(&device, &cfg, resampler, start)?,
            cpal::SampleFormat::F32 => build::<f32>(&device, &cfg, resampler, start)?,
            cpal::SampleFormat::F64 => build::<f64>(&device, &cfg, resampler, start)?,
            _ => bail!("输入设备采样格式尚不支持：{format:?}"),
        };
        stream.play()?;
        Ok(stream)
    }
}
#[cfg(windows)]
fn build<T>(
    device: &cpal::Device,
    cfg: &cpal::StreamConfig,
    mut resampler: Resampler,
    start: Instant,
) -> Result<cpal::Stream>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    use cpal::Sample;
    let stop = resampler.stop.clone();
    Ok(device.build_input_stream(
        cfg,
        move |data: &[T], _| resampler.push(data.iter().map(|x| f32::from_sample(*x)), start),
        move |_: cpal::StreamError| {
            stop.store(4, Ordering::SeqCst);
        },
        None,
    )?)
}
#[cfg(windows)]
struct Resampler {
    channels: usize,
    ratio: f64,
    index: u64,
    next: f64,
    previous: f32,
    pending: Vec<f32>,
    tx: SyncSender<Frame>,
    stop: Arc<AtomicU8>,
}
#[cfg(windows)]
impl Resampler {
    fn new(rate: u32, channels: usize, tx: SyncSender<Frame>, stop: Arc<AtomicU8>) -> Self {
        Self {
            channels,
            ratio: rate as f64 / 16000.,
            index: 0,
            next: 0.,
            previous: 0.,
            pending: Vec::with_capacity(512),
            tx,
            stop,
        }
    }
    fn push(&mut self, data: impl Iterator<Item = f32>, start: Instant) {
        if self.stop.load(Ordering::SeqCst) > 0 {
            return;
        }
        let data = data.collect::<Vec<_>>();
        for frame in data.chunks_exact(self.channels) {
            let value = frame.iter().sum::<f32>() / self.channels as f32;
            let value = if value.is_finite() {
                value.clamp(-1., 1.)
            } else {
                0.
            };
            while self.next <= self.index as f64 {
                let fraction = (self.next - (self.index as f64 - 1.)).clamp(0., 1.) as f32;
                self.pending
                    .push(self.previous + (value - self.previous) * fraction);
                self.next += self.ratio;
                if self.pending.len() == 512 {
                    let samples = std::mem::replace(&mut self.pending, Vec::with_capacity(512));
                    if self
                        .tx
                        .try_send(Frame {
                            samples,
                            captured_ms: start.elapsed().as_secs_f64() * 1000.,
                        })
                        .is_err()
                    {
                        self.stop.store(3, Ordering::SeqCst);
                        return;
                    }
                }
            }
            self.previous = value;
            self.index += 1;
        }
    }
}
