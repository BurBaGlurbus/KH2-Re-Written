//! Sound banks as the exe registers them (sound-01): a request id selects
//! bank `id / 1000` and sound `id % 1000` in that bank's SCD. Banks come
//! from `se/seNNN.win32.scd` entries or from an object archive's
//! `se/obj/seNNN` replacement asset.
use audio::Clip;
use data::{original_payload, probe_source::ProbeSource, replacement_payload, scd};
use std::collections::HashMap;

/// Reads a bank: the entry's original payload, or its named replacement.
pub fn load_bank(
    source: &ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
    replacement_name: Option<&str>,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut checked = source.open_index(index)?;
    let entry = checked
        .inventory
        .entries
        .get(ordinal)
        .filter(|e| e.name_hash == name_hash)
        .ok_or("sound bank entry mismatch")?;
    let bytes = match replacement_name {
        None => original_payload::read_original(&mut checked.reader, entry)?.bytes,
        Some(name) => {
            replacement_payload::read_all_replacements(&mut checked.reader, entry)?
                .into_iter()
                .find(|r| {
                    let n = &r.metadata.name_bytes;
                    let end = n.iter().position(|&b| b == 0).unwrap_or(n.len());
                    &n[..end] == name.as_bytes()
                })
                .ok_or("sound bank replacement absent")?
                .bytes
        }
    };
    checked.verify_unchanged()?;
    scd::read(&bytes)?;
    Ok(bytes)
}

/// The sound bank an object archive carries: its `se/obj/seNNN` replacement
/// asset, which the exe loads together with the object (FUN_14039e900 →
/// FUN_1401dcef0). `None` when the archive has none.
pub fn object_bank(
    source: &ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
) -> Option<Vec<u8>> {
    object_asset(source, index, ordinal, name_hash, b"se/obj/se")
}

/// The voice bank an object archive carries: the replacement asset its
/// type-34 record names (`voice/battle/us/<name>`), which the object's
/// voice object YS::VOICE_EE plays its lines from (FUN_1403da940).
pub fn object_voice_bank(
    source: &ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
) -> Option<Vec<u8>> {
    object_asset(source, index, ordinal, name_hash, b"voice/")
}

fn object_asset(
    source: &ProbeSource,
    index: &str,
    ordinal: usize,
    name_hash: [u8; 16],
    prefix: &[u8],
) -> Option<Vec<u8>> {
    let mut checked = source.open_index(index).ok()?;
    let entry = checked
        .inventory
        .entries
        .get(ordinal)
        .filter(|e| e.name_hash == name_hash)?;
    let bank = replacement_payload::read_all_replacements(&mut checked.reader, entry)
        .ok()?
        .into_iter()
        .find(|r| r.metadata.name_bytes.starts_with(prefix))?
        .bytes;
    checked.verify_unchanged().ok()?;
    scd::read(&bank).ok()?;
    Some(bank)
}

/// One resolved request: its layers, the sound's volume and loop flag.
pub struct Request {
    pub layers: Vec<Clip>,
    pub volume: f32,
    pub looped: bool,
}

#[derive(Default)]
pub struct Banks {
    banks: HashMap<u16, Vec<u8>>,
    decoded: HashMap<(u16, usize), Option<Clip>>,
    /// When each decoded clip was last asked for (a counter of requests).
    used: HashMap<(u16, usize), u64>,
    requests: u64,
}

impl Banks {
    /// Registers a bank under the number in its own table header.
    pub fn insert(&mut self, bytes: Vec<u8>) -> Result<u16, Box<dyn std::error::Error>> {
        let number = scd::read(&bytes)?.number;
        self.banks.insert(number, bytes);
        Ok(number)
    }

    /// Past `budget` bytes the clips asked for longest ago are dropped (a
    /// playing voice keeps its own reference; the next request decodes
    /// again). Returns how many.
    pub fn trim_clips(&mut self, budget: usize) -> usize {
        let bytes = |c: &Option<Clip>| c.as_ref().map_or(0, |c| c.samples.len() * size_of::<i16>());
        let mut total: usize = self.decoded.values().map(bytes).sum();
        if total <= budget {
            return 0;
        }
        let mut order: Vec<((u16, usize), u64)> = self
            .decoded
            .keys()
            .map(|k| (*k, self.used.get(k).copied().unwrap_or(0)))
            .collect();
        order.sort_by_key(|o| o.1);
        let mut dropped = 0;
        for (key, _) in order {
            if total <= budget {
                break;
            }
            if let Some(clip) = self.decoded.remove(&key) {
                total -= bytes(&clip);
                dropped += 1;
            }
            self.used.remove(&key);
        }
        dropped
    }

    /// Banks held and their bytes, sounds decoded and their bytes.
    pub fn census(&self) -> (usize, usize, usize, usize) {
        let bank_bytes = self.banks.values().map(Vec::len).sum();
        let clip_bytes = self
            .decoded
            .values()
            .flatten()
            .map(|c| c.samples.len() * size_of::<i16>())
            .sum();
        (self.banks.len(), bank_bytes, self.decoded.len(), clip_bytes)
    }

    pub fn numbers(&self) -> Vec<u16> {
        let mut n: Vec<_> = self.banks.keys().copied().collect();
        n.sort_unstable();
        n
    }

    /// Resolves `id`; None when its bank is not loaded or the sound is
    /// outside it (the engine drops such requests).
    pub fn request(&mut self, id: u32) -> Result<Option<Request>, Box<dyn std::error::Error>> {
        let (number, no) = ((id / 1000) as u16, (id % 1000) as usize);
        let Some(bytes) = self.banks.get(&number) else {
            return Ok(None);
        };
        let bank = scd::read(bytes)?;
        let Some(sound) = bank.sound(no)? else {
            return Ok(None);
        };
        let mut layers = Vec::new();
        self.requests += 1;
        for &audio in &sound.audio {
            self.used.insert((number, audio as usize), self.requests);
            let decoded = match self.decoded.entry((number, audio as usize)) {
                std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(bank.decode(audio as usize)?.map(|pcm| Clip {
                        channels: pcm.channels,
                        rate: pcm.rate,
                        loop_frames: Some((pcm.loop_start, pcm.loop_end)),
                        samples: pcm.samples.into(),
                    }))
                }
            };
            if let Some(clip) = decoded {
                let mut clip = clip.clone();
                if !sound.looped {
                    clip.loop_frames = None;
                }
                layers.push(clip);
            }
        }
        Ok(Some(Request {
            layers,
            volume: sound.volume,
            looped: sound.looped,
        }))
    }
}

/// A decoded music clip with its level and decode statistics.
pub struct Music {
    pub clip: audio::Clip,
    pub volume: f32,
    pub stats: serde_json::Value,
}

/// Decodes a music SCD (sound 0, codec 6) with level statistics.
pub fn decode_music(bytes: Vec<u8>) -> Result<Music, String> {
    let started = std::time::Instant::now();
    let bank = data::scd::read(&bytes).map_err(|e| e.to_string())?;
    let sound = bank
        .sound(0)
        .map_err(|e| e.to_string())?
        .ok_or("music sound absent")?;
    let audio = *sound.audio.first().ok_or("music audio absent")?;
    let pcm = bank
        .decode(audio as usize)
        .map_err(|e| e.to_string())?
        .ok_or("empty music stream")?;
    let ch = pcm.channels as usize;
    let mut peak = vec![0i32; ch];
    let mut energy = vec![0f64; ch];
    for (i, &v) in pcm.samples.iter().enumerate() {
        peak[i % ch] = peak[i % ch].max((v as i32).abs());
        energy[i % ch] += (v as f64) * (v as f64);
    }
    let frames = pcm.frames();
    let rms: Vec<f64> = energy
        .iter()
        .map(|e| (e / frames.max(1) as f64).sqrt())
        .collect();
    let stats = serde_json::json!({"channels":ch,"rate":pcm.rate,"frames":frames,
        "loop":[pcm.loop_start,pcm.loop_end],"looped":sound.looped,"volume":sound.volume,
        "peak":peak,"rms":rms,"decode_ms":started.elapsed().as_millis() as u64});
    Ok(Music {
        clip: audio::Clip {
            channels: pcm.channels,
            rate: pcm.rate,
            loop_frames: sound.looped.then_some((pcm.loop_start, pcm.loop_end)),
            samples: pcm.samples.into(),
        },
        volume: sound.volume,
        stats,
    })
}

/// Where a background music decode leaves its result.
pub type MusicSlot = std::sync::Arc<std::sync::Mutex<Option<Result<Music, String>>>>;

/// Decodes `bytes` on its own thread (the exe streams; a whole-track
/// decode takes about a second here).
pub fn decode_music_in_background(bytes: Vec<u8>) -> MusicSlot {
    let slot = MusicSlot::default();
    let out = slot.clone();
    std::thread::spawn(move || {
        let decoded = decode_music(bytes);
        if let Ok(mut r) = out.lock() {
            *r = Some(decoded);
        }
    });
    slot
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    fn clip(frames: usize) -> Option<Clip> {
        Some(Clip {
            channels: 1,
            rate: 44_100,
            samples: vec![0i16; frames].into(),
            loop_frames: None,
        })
    }

    #[test]
    fn past_the_budget_the_clips_asked_for_longest_ago_go() {
        let mut banks = Banks::default();
        for (k, used) in [(0usize, 3u64), (1, 1), (2, 2)] {
            banks.decoded.insert((7, k), clip(1000));
            banks.used.insert((7, k), used);
        }
        assert_eq!(banks.trim_clips(1 << 20), 0);
        assert_eq!(banks.trim_clips(2000), 2);
        assert!(banks.decoded.contains_key(&(7, 0)));
        assert_eq!(banks.census().2, 1);
    }
}
