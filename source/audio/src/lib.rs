//! Sound output for the test room: a software mixer (safe) feeding the
//! Windows waveOut device (the only FFI, in `winmm`). Clips arrive already
//! decoded; voices are resampled linearly to the device rate.
use std::sync::{Arc, Mutex};

#[cfg(windows)]
mod winmm;

/// Device format: 16-bit stereo.
pub const RATE: u32 = 48_000;
const MAX_VOICES: usize = 32;

/// A decoded sound: interleaved 16-bit samples with an optional loop range
/// in frames (end inclusive).
#[derive(Debug, Clone)]
pub struct Clip {
    pub channels: u16,
    pub rate: u32,
    pub samples: Arc<[i16]>,
    pub loop_frames: Option<(u32, u32)>,
}

impl Clip {
    fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    fn frame(&self, index: usize) -> [f32; 2] {
        let c = self.channels as usize;
        let at = index * c;
        let left = self.samples[at] as f32;
        let right = if c > 1 {
            self.samples[at + 1] as f32
        } else {
            left
        };
        if c >= 4 {
            // Quad (front L/R, rear L/R) folded to stereo [inferred: the
            // exe hands 4 channels to XAudio2].
            return [
                left + self.samples[at + 2] as f32,
                right + self.samples[at + 3] as f32,
            ];
        }
        [left, right]
    }
}

struct Voice {
    clip: Clip,
    key: u32,
    position: f64,
    step: f64,
    gain: f32,
    /// Gain change per output frame, the target, and whether reaching it
    /// stops (or pauses) the voice.
    fade: Option<(f32, f32, FadeEnd)>,
    paused: bool,
}

/// What happens when a fade reaches its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FadeEnd {
    Hold,
    Stop,
    Pause,
}

/// A source that renders its own interleaved stereo at `RATE` (-1..1).
pub type Stream = Box<dyn FnMut(&mut [f32]) + Send>;

#[derive(Default)]
pub struct Mixer {
    voices: Vec<Voice>,
    stream: Option<Stream>,
    gains: Gains,
}

/// Output volumes: `music` for the voices of `music_keys`, `effects` for
/// every other voice and the stream.
struct Gains {
    music: f32,
    effects: f32,
    music_keys: Vec<u32>,
}

impl Default for Gains {
    fn default() -> Self {
        Self {
            music: 1.,
            effects: 1.,
            music_keys: Vec::new(),
        }
    }
}

impl Mixer {
    /// Starts `clip`; `key` identifies the request for `stop`. The oldest
    /// voice is dropped past the voice limit.
    pub fn play(&mut self, clip: Clip, key: u32, gain: f32, pitch: f32) {
        if clip.frames() == 0 {
            return;
        }
        if self.voices.len() >= MAX_VOICES {
            // The oldest one-shot goes first, then the oldest loop; the
            // music (and a voice's only copy of a bed) last.
            let music = &self.gains.music_keys;
            let oldest = self
                .voices
                .iter()
                .position(|v| v.clip.loop_frames.is_none() && !music.contains(&v.key))
                .or_else(|| self.voices.iter().position(|v| !music.contains(&v.key)))
                .unwrap_or(0);
            self.voices.remove(oldest);
        }
        let step = clip.rate as f64 / RATE as f64 * pitch as f64;
        self.voices.push(Voice {
            clip,
            key,
            position: 0.,
            step,
            gain,
            fade: None,
            paused: false,
        });
    }

    /// Ramps the voices of `key` linearly to `target` over `seconds`.
    pub fn fade(&mut self, key: u32, target: f32, seconds: f32, end: FadeEnd) {
        let frames = (seconds * RATE as f32).max(1.);
        for v in self.voices.iter_mut().filter(|v| v.key == key) {
            v.fade = Some(((target - v.gain) / frames, target, end));
        }
    }

    /// Resumes paused voices of `key`, ramping up to `target`.
    pub fn resume(&mut self, key: u32, target: f32, seconds: f32) {
        for v in self.voices.iter_mut().filter(|v| v.key == key) {
            v.paused = false;
        }
        self.fade(key, target, seconds, FadeEnd::Hold);
    }

    /// Holds (`true`) or releases every voice of `key` at its position,
    /// without a fade.
    pub fn hold(&mut self, key: u32, held: bool) {
        for v in self.voices.iter_mut().filter(|v| v.key == key) {
            v.paused = held;
        }
    }

    pub fn paused(&self, key: u32) -> bool {
        self.voices.iter().any(|v| v.key == key && v.paused)
    }

    /// Fades out every looping voice `keep` does not name, over `seconds`.
    /// Returns how many.
    pub fn stop_loops(&mut self, seconds: f32, keep: impl Fn(u32) -> bool) -> usize {
        let frames = (seconds * RATE as f32).max(1.);
        let mut n = 0;
        for v in self
            .voices
            .iter_mut()
            .filter(|v| v.clip.loop_frames.is_some() && !keep(v.key))
        {
            v.fade = Some((-v.gain / frames, 0., FadeEnd::Stop));
            n += 1;
        }
        n
    }

    pub fn stop(&mut self, key: u32) {
        self.voices.retain(|v| v.key != key);
    }

    pub fn playing(&self, key: u32) -> bool {
        self.voices.iter().any(|v| v.key == key)
    }

    pub fn voices(&self) -> usize {
        self.voices.len()
    }

    /// Adds (or with None removes) a stream mixed over the voices.
    pub fn set_stream(&mut self, stream: Option<Stream>) {
        self.stream = stream;
    }

    pub fn has_stream(&self) -> bool {
        self.stream.is_some()
    }

    /// Sets the output volumes (see `Gains`); they apply from the next
    /// buffer.
    pub fn set_gains(&mut self, music: f32, effects: f32, music_keys: &[u32]) {
        self.gains = Gains {
            music,
            effects,
            music_keys: music_keys.to_vec(),
        };
    }

    /// Mixes into interleaved stereo `out` at `RATE`.
    pub fn mix(&mut self, out: &mut [i16]) {
        let mut sum = vec![0f32; out.len()];
        let gains = &self.gains;
        if let Some(stream) = self.stream.as_mut() {
            let mut rendered = vec![0f32; out.len()];
            stream(&mut rendered);
            for (s, r) in sum.iter_mut().zip(rendered) {
                *s += r * 32767. * gains.effects;
            }
        }
        self.voices.retain_mut(|v| {
            if v.paused {
                return true;
            }
            let bus = if gains.music_keys.contains(&v.key) {
                gains.music
            } else {
                gains.effects
            };
            let frames = v.clip.frames();
            for pair in sum.as_chunks_mut::<2>().0 {
                let mut ended = None;
                if let Some((step, target, end)) = v.fade {
                    v.gain += step;
                    if (step >= 0. && v.gain >= target) || (step < 0. && v.gain <= target) {
                        v.gain = target;
                        v.fade = None;
                        ended = Some(end);
                    }
                }
                let mut index = v.position as usize;
                if let Some((start, end)) = v.clip.loop_frames {
                    let end = (end as usize).min(frames - 1);
                    if index > end && (start as usize) < end {
                        v.position -= (end + 1 - start as usize) as f64;
                        index = v.position as usize;
                    }
                }
                if index >= frames {
                    return false;
                }
                let t = (v.position - index as f64) as f32;
                let a = v.clip.frame(index);
                let b = if index + 1 < frames {
                    v.clip.frame(index + 1)
                } else {
                    a
                };
                pair[0] += (a[0] + (b[0] - a[0]) * t) * v.gain * bus;
                pair[1] += (a[1] + (b[1] - a[1]) * t) * v.gain * bus;
                v.position += v.step;
                match ended {
                    Some(FadeEnd::Stop) => return false,
                    Some(FadeEnd::Pause) => {
                        v.paused = true;
                        return true;
                    }
                    _ => {}
                }
            }
            true
        });
        for (o, s) in out.iter_mut().zip(sum) {
            *o = s.clamp(-32768., 32767.) as i16;
        }
    }
}

/// The open device and its shared mixer. Dropping it stops playback.
pub struct Output {
    pub mixer: Arc<Mutex<Mixer>>,
    #[cfg(windows)]
    _device: winmm::Device,
}

impl Output {
    pub fn open() -> std::io::Result<Self> {
        let mixer = Arc::new(Mutex::new(Mixer::default()));
        #[cfg(windows)]
        {
            let feed = mixer.clone();
            let device = winmm::Device::open(RATE, move |buffer| {
                if let Ok(mut m) = feed.lock() {
                    m.mix(buffer);
                } else {
                    buffer.fill(0);
                }
            })?;
            Ok(Self {
                mixer,
                _device: device,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = mixer;
            Err(std::io::Error::other("no audio output on this platform"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stream_is_mixed_over_the_voices() {
        let mut m = Mixer::default();
        m.set_stream(Some(Box::new(|out: &mut [f32]| out.fill(0.5))));
        let mut out = [0i16; 4];
        m.mix(&mut out);
        assert_eq!(out, [16383; 4]);
        m.set_stream(None);
        m.mix(&mut out);
        assert_eq!(out, [0; 4]);
    }

    fn clip(samples: &[i16], channels: u16, rate: u32) -> Clip {
        Clip {
            channels,
            rate,
            samples: samples.into(),
            loop_frames: None,
        }
    }

    #[test]
    fn past_the_limit_the_music_stays_and_loops_end_when_asked() {
        let mut mixer = Mixer::default();
        mixer.set_gains(1., 1., &[77]);
        let mut looped = clip(&[1, 2, 3, 4], 1, RATE);
        looped.loop_frames = Some((0, 3));
        mixer.play(looped.clone(), 77, 1., 1.);
        mixer.play(looped.clone(), 5, 1., 1.);
        for k in 0..MAX_VOICES as u32 {
            mixer.play(clip(&[1, 2], 1, RATE), 100 + k, 1., 1.);
        }
        assert_eq!(mixer.voices(), MAX_VOICES);
        assert!(mixer.playing(77));
        assert!(mixer.playing(5));
        assert_eq!(mixer.stop_loops(0.001, |key| key == 77), 1);
        let mut out = vec![0; 2048];
        mixer.mix(&mut out);
        assert!(mixer.playing(77));
        assert!(!mixer.playing(5));
    }

    #[test]
    fn mixes_mono_to_both_channels_and_ends() {
        let mut m = Mixer::default();
        m.play(clip(&[100, 200], 1, RATE), 7, 1., 1.);
        let mut out = [0i16; 8];
        m.mix(&mut out);
        assert_eq!(out, [100, 100, 200, 200, 0, 0, 0, 0]);
        assert_eq!(m.voices(), 0);
    }

    #[test]
    fn sums_clamps_resamples_and_stops() {
        let mut m = Mixer::default();
        m.play(clip(&[30000, 30000, 0, 0], 2, RATE), 1, 1., 1.);
        m.play(clip(&[30000, -100], 2, RATE), 2, 1., 1.);
        let mut out = [0i16; 2];
        m.mix(&mut out);
        assert_eq!(out, [32767, 29900]);
        // Half the device rate: each source frame spans two output frames.
        let mut m = Mixer::default();
        m.play(clip(&[0, 100], 1, RATE / 2), 3, 0.5, 1.);
        let mut out = [0i16; 6];
        m.mix(&mut out);
        assert_eq!(out, [0, 0, 25, 25, 50, 50]);
        assert!(m.playing(3));
        m.stop(3);
        assert!(!m.playing(3));
    }

    #[test]
    fn gains_scale_music_and_effects_apart() {
        let mut m = Mixer::default();
        m.play(clip(&[1000], 1, RATE), 1, 1., 1.);
        m.play(clip(&[100], 1, RATE), 2, 1., 1.);
        m.set_gains(0.5, 0., &[1]);
        let mut out = [0i16; 2];
        m.mix(&mut out);
        assert_eq!(out, [500, 500]);
        m.set_stream(Some(Box::new(|out: &mut [f32]| out.fill(0.5))));
        m.set_gains(1., 0.5, &[]);
        m.mix(&mut out);
        assert_eq!(out, [8191, 8191]);
    }

    #[test]
    fn loops_between_loop_frames() {
        let mut m = Mixer::default();
        let mut c = clip(&[1, 2, 3], 1, RATE);
        c.loop_frames = Some((1, 2));
        m.play(c, 9, 1., 1.);
        let mut out = [0i16; 12];
        m.mix(&mut out);
        let left: Vec<i16> = out.iter().step_by(2).copied().collect();
        assert_eq!(left, [1, 2, 3, 2, 3, 2]);
    }

    #[test]
    fn fades_pause_resume_and_stop() {
        let mut m = Mixer::default();
        m.play(clip(&[1000; 64], 1, RATE), 5, 1., 1.);
        // Two output frames to half gain, then pause.
        m.fade(5, 0.5, 2. / RATE as f32, FadeEnd::Pause);
        let mut out = [0i16; 6];
        m.mix(&mut out);
        assert_eq!(&out[..4], &[750, 750, 500, 500]);
        assert!(m.paused(5));
        m.resume(5, 1., 1. / RATE as f32);
        let mut out = [0i16; 2];
        m.mix(&mut out);
        assert_eq!(out, [1000, 1000]);
        m.fade(5, 0., 1. / RATE as f32, FadeEnd::Stop);
        m.mix(&mut out);
        assert_eq!(m.voices(), 0);
    }
}
