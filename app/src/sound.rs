//! Paper sounds.
//!
//! Recorded page turns are embedded at build time from `assets/sounds/` when
//! present (see build.rs and docs/SOUNDS.md). Otherwise sounds are synthesized:
//! filtered noise with a flutter envelope ≈ a page brushing past, plus a low
//! "slap" when it lands.

use rodio::{DeviceSinkBuilder, MixerDeviceSink, buffer::SamplesBuffer};
use std::num::NonZero;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/sounds.rs"));
}

const RATE: u32 = 44_100;

#[derive(Clone, Copy, Debug)]
pub enum Sfx {
    /// A complete page turn (click, key, wheel).
    Flip,
    /// The start of a hand-driven peel.
    Peel,
    /// A dragged page landing after release.
    Land,
}

struct Clip {
    rate: u32,
    data: Vec<f32>,
}

pub struct Sound {
    sink: Option<MixerDeviceSink>,
    flips: Vec<Clip>,
    peels: Vec<Clip>,
    lands: Vec<Clip>,
    n: usize,
}

/// Minimal WAV reader: PCM 16-bit, mono or stereo (downmixed).
fn decode_wav(bytes: &[u8]) -> Option<Clip> {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let (mut channels, mut rate, mut bits) = (1u16, RATE, 16u16);
    let mut i = 12;
    while i + 8 <= bytes.len() {
        let id = &bytes[i..i + 4];
        let len = u32::from_le_bytes(bytes[i + 4..i + 8].try_into().ok()?) as usize;
        let body = bytes.get(i + 8..(i + 8 + len).min(bytes.len()))?;
        if id == b"fmt " && body.len() >= 16 {
            channels = u16::from_le_bytes([body[2], body[3]]).max(1);
            rate = u32::from_le_bytes(body[4..8].try_into().ok()?);
            bits = u16::from_le_bytes([body[14], body[15]]);
        } else if id == b"data" {
            if bits != 16 {
                return None;
            }
            let frames = body.chunks_exact(2 * channels as usize);
            let data = frames
                .map(|f| {
                    f.as_chunks::<2>()
                        .0
                        .iter()
                        .map(|s| i16::from_le_bytes(*s) as f32 / 32768.0)
                        .sum::<f32>()
                        / channels as f32
                })
                .collect();
            return Some(Clip { rate, data });
        }
        i += 8 + len + (len & 1);
    }
    None
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0
    }
}

fn synth(seed: u64, dur: f32, flutter: (f32, f32), slap: bool, gain: f32) -> Vec<f32> {
    let n = (dur * RATE as f32) as usize;
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut out = Vec::with_capacity(n);
    let (mut hp_prev_in, mut hp_prev_out, mut lp) = (0.0f32, 0.0f32, 0.0f32);
    let mut lp2 = 0.0f32;
    let mut phase = 0.0f32;
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let x = t / dur;
        let w = rng.next();
        let hp = 0.97 * (hp_prev_out + w - hp_prev_in);
        hp_prev_in = w;
        hp_prev_out = hp;
        let cutoff = 0.25 + 0.35 * (1.0 - x);
        lp += cutoff * (hp - lp);
        let env = (x / 0.12).min(1.0).powf(1.5) * (1.0 - x).powf(1.8);
        let f = flutter.0 + (flutter.1 - flutter.0) * x;
        phase += f / RATE as f32;
        let flut = 0.55 + 0.45 * (phase * std::f32::consts::TAU).sin().abs().powf(0.6);
        let mut s = lp * env * flut;
        if slap && x > 0.78 {
            let k = (x - 0.78) / 0.22;
            let senv = (k / 0.06).min(1.0) * (-(k * 9.0)).exp();
            lp2 += 0.08 * (w - lp2);
            s += lp2 * senv * 2.6;
        }
        out.push(s * gain);
    }
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
    for v in out.iter_mut() {
        *v = *v / peak * gain;
    }
    out
}

fn bank(recorded: &[&[u8]], fallback: impl Fn(u64) -> Vec<f32>, n: u64) -> Vec<Clip> {
    let clips: Vec<Clip> = recorded.iter().filter_map(|b| decode_wav(b)).collect();
    if !clips.is_empty() {
        return clips;
    }
    (0..n)
        .map(|i| Clip {
            rate: RATE,
            data: fallback(i),
        })
        .collect()
}

impl Sound {
    pub fn new() -> Self {
        let sink = DeviceSinkBuilder::open_default_sink().ok().map(|mut s| {
            s.log_on_drop(false);
            s
        });
        Self {
            sink,
            flips: bank(
                embedded::FLIP,
                |i| synth(11 + i, 0.34 + i as f32 * 0.03, (46.0, 14.0), true, 0.55),
                4,
            ),
            peels: bank(embedded::PEEL, |i| synth(101 + i, 0.42, (18.0, 9.0), false, 0.22), 3),
            lands: bank(embedded::LAND, |i| synth(201 + i, 0.18, (30.0, 20.0), true, 0.4), 2),
            n: 0,
        }
    }

    pub fn play(&mut self, sfx: Sfx, volume: f32) {
        let Some(sink) = &self.sink else { return };
        self.n = self.n.wrapping_add(1);
        let bank = match sfx {
            Sfx::Flip => &self.flips,
            Sfx::Peel => &self.peels,
            Sfx::Land => &self.lands,
        };
        let clip = &bank[self.n % bank.len()];
        let data: Vec<f32> = clip.data.iter().map(|v| v * volume.clamp(0.0, 1.0)).collect();
        sink.mixer().add(SamplesBuffer::new(
            NonZero::new(1).unwrap(),
            NonZero::new(clip.rate.max(1)).unwrap(),
            data,
        ));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn synth_is_bounded() {
        let s = super::synth(1, 0.3, (40.0, 12.0), true, 0.5);
        assert!(s.iter().all(|v| v.abs() <= 0.5001));
        assert!(s.len() > 10_000);
    }

    #[test]
    fn embedded_sounds_decode() {
        for b in super::embedded::FLIP
            .iter()
            .chain(super::embedded::PEEL)
            .chain(super::embedded::LAND)
        {
            let c = super::decode_wav(b).expect("valid 16-bit wav");
            assert!(c.data.len() > 1000 && c.rate == 44_100);
        }
    }
}
