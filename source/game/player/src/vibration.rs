//! Controller vibration: frame trigger 31 of a motion starts a vibration wave,
//! and a per-frame callback turns it into the pad's two motor bytes.
//!
//! The trigger is `FUN_14040dd70` case 0x1f → `FUN_1403eefc0(id)`: a binary
//! search of the id (a byte) in the `vibr` table of the battle archive's `vbrt`
//! block (entries of two bytes, id then wave number) picks the n-th type 47
//! entry of the block, and `FUN_14019ae60` starts it: the playing state is 1,
//! the time 0 and the length the largest key time of both tracks. The callback
//! `FUN_14019b1d0` (also `FUN_14019b000`) runs once a frame: state 1 starts at
//! time 0, state 2 advances the time by 0.5 (`DAT_140717484 x DAT_140623a4c`),
//! a time at or past the length stops it (the time goes back to length - 1),
//! and each of the two tracks writes its motor byte (`pad+0x48`, `pad+0x49`).
//! A track holds keys of two i32 (time, value) after the header; the byte is
//! the value of the first key at or after the time, scaled by how far the time
//! is through the segment from the key before it (so each segment ramps from
//! zero up to its end key's value). The pad update clears both bytes each frame.
//!
//! The PC layer (`FUN_14039bbe0`) turns the bytes into `XInputSetState`
//! (`FUN_140132500`): the left (strong) motor takes byte 1 (as 0x0101 x byte)
//! and the right (weak) motor is fully on when byte 0 is above 0x7f; the
//! whole thing is skipped when the config's vibration bit
//! (`FUN_1402e53f0`) is off.

/// A wave's two tracks of `(time, value)` keys and its length.
#[derive(Clone, Debug, PartialEq)]
pub struct Wave {
    tracks: [Vec<(i32, i32)>; 2],
    length: i32,
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}

fn i32_at(bytes: &[u8], at: usize) -> Option<i32> {
    u32_at(bytes, at).map(|v| v as i32)
}

impl Wave {
    /// Reads a type 47 entry: counts at +8 and +0x10, key offsets at +0xc and
    /// +0x14.
    pub fn read(bytes: &[u8]) -> Option<Wave> {
        let mut tracks = [Vec::new(), Vec::new()];
        let mut length = 0;
        for (track, at) in tracks.iter_mut().zip([8usize, 0x10]) {
            let count = u32_at(bytes, at)? as usize;
            let base = u32_at(bytes, at + 4)? as usize;
            if count > 4096 {
                return None;
            }
            for i in 0..count {
                let key = (
                    i32_at(bytes, base + i * 8)?,
                    i32_at(bytes, base + i * 8 + 4)?,
                );
                length = length.max(key.0);
                track.push(key);
            }
        }
        Some(Wave { tracks, length })
    }

    /// The motor bytes at `time`: a track without two keys, or whose first key
    /// is later than `time`, or whose keys all end before it writes nothing.
    fn bytes(&self, time: f32) -> [Option<u8>; 2] {
        let mut out = [None; 2];
        for (byte, keys) in out.iter_mut().zip(&self.tracks) {
            if keys.len() < 2 || keys[0].0 as f32 > time {
                continue;
            }
            for pair in keys.windows(2) {
                let (before, after) = (pair[0], pair[1]);
                if (after.0 as f32) < time {
                    continue;
                }
                let span = (after.0 - before.0) as f32;
                let fraction = if span == 0. {
                    1.
                } else {
                    (time - before.0 as f32) / span
                };
                *byte = Some((after.1 as f32 * fraction) as i32 as u8);
                break;
            }
        }
        out
    }
}

/// The `vibr` table and the waves it names.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Library {
    /// `(id, wave number)` in id order, as stored.
    table: Vec<(u8, u8)>,
    waves: Vec<Wave>,
}

impl Library {
    /// `table` is the `vibr` entry (u32 version, u32 count, then two-byte
    /// rows); `waves` the block's type 47 entries in order.
    pub fn read(table: &[u8], waves: &[&[u8]]) -> Option<Library> {
        let count = u32_at(table, 4)? as usize;
        let rows = table.get(8..8 + count.checked_mul(2)?)?;
        Some(Library {
            table: rows
                .as_chunks::<2>()
                .0
                .iter()
                .map(|r| (r[0], r[1]))
                .collect(),
            waves: waves
                .iter()
                .map(|w| Wave::read(w))
                .collect::<Option<Vec<_>>>()?,
        })
    }

    /// `FUN_1403eefc0`'s lookup: the id's wave, if the table and the block
    /// have it.
    pub fn find(&self, id: u8) -> Option<&Wave> {
        let row = self.table.binary_search_by_key(&id, |&(key, _)| key).ok()?;
        self.waves.get(self.table[row].1 as usize)
    }
}

/// The one running wave (`DAT_140ac0ec0`..`ef0`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Playback {
    wave: Option<Wave>,
    /// 0 stopped, 1 just started, 2 running (`DAT_140ac0ee0`).
    state: u8,
    time: f32,
}

impl Playback {
    /// `FUN_14019ae60`: starts (or restarts) a wave.
    pub fn start(&mut self, wave: &Wave) {
        self.wave = Some(wave.clone());
        self.state = 1;
        self.time = 0.;
    }

    pub fn playing(&self) -> bool {
        self.state != 0
    }

    /// `FUN_14019b1d0` for one frame: the motor bytes this frame (the pad
    /// update clears them otherwise).
    pub fn tick(&mut self) -> [u8; 2] {
        let Some(wave) = &self.wave else {
            return [0; 2];
        };
        match self.state {
            1 => {
                self.time = 0.;
                self.state = 2;
            }
            2 => self.time += 0.5,
            _ => return [0; 2],
        }
        if wave.length as f32 <= self.time {
            self.state = 0;
            self.time = (wave.length - 1) as f32;
        }
        wave.bytes(self.time).map(|b| b.unwrap_or(0))
    }
}

/// `FUN_14039bbe0`: the two XInput motors, as fractions of full speed: the
/// strong (left) one follows byte 1, the weak (right) one is on above 0x7f
/// in byte 0.
pub fn motors(bytes: [u8; 2]) -> (f32, f32) {
    (
        bytes[1] as f32 / 255.,
        if bytes[0] > 0x7f { 1. } else { 0. },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic wave: track 0 holds the given keys, track 1 none.
    fn wave(keys: &[(i32, i32)], second: &[(i32, i32)]) -> Vec<u8> {
        let mut b = b"VIBD".to_vec();
        b.extend(0x100u32.to_le_bytes());
        let first_at = 0x18u32;
        let second_at = first_at + keys.len() as u32 * 8;
        for (count, at) in [(keys.len(), first_at), (second.len(), second_at)] {
            b.extend((count as u32).to_le_bytes());
            b.extend(at.to_le_bytes());
        }
        for &(t, v) in keys.iter().chain(second) {
            b.extend(t.to_le_bytes());
            b.extend(v.to_le_bytes());
        }
        b
    }

    #[test]
    fn a_wave_ramps_each_segment_up_to_its_end_value_and_ends_at_its_length() {
        let bytes = wave(&[(0, 255), (12, 255)], &[]);
        let wave = Wave::read(&bytes).unwrap();
        assert_eq!(wave.length, 12);
        let mut play = Playback::default();
        play.start(&wave);
        let mut seen = Vec::new();
        for _ in 0..30 {
            seen.push(play.tick()[0]);
        }
        // The first frame is time 0, then half a unit a frame.
        assert_eq!(seen[0], 0);
        assert_eq!(seen[1], 10); // 0.5 / 12 of 255
        assert_eq!(seen[12], 127); // time 6: 127.5 truncated
        assert_eq!(seen[13], 138); // time 6.5, past the weak motor's 0x7f
        // Time 12 reaches the length: it stops, the time goes back to 11.
        assert_eq!(seen[24], 233);
        assert!(!play.playing());
        assert_eq!(&seen[25..], &[0; 5]);
    }

    #[test]
    fn the_motors_follow_the_bytes() {
        assert_eq!(motors([0, 0]), (0., 0.));
        assert_eq!(motors([0x7f, 255]), (1., 0.));
        assert_eq!(motors([0x80, 0]), (0., 1.));
    }

    #[test]
    fn the_table_is_searched_by_id() {
        let mut table = vec![1, 0, 0, 0, 3, 0, 0, 0];
        table.extend([1, 0, 4, 1, 9, 1]);
        let a = wave(&[(0, 10), (4, 10)], &[]);
        let b = wave(&[(0, 20), (4, 20)], &[]);
        let library = Library::read(&table, &[&a, &b]).unwrap();
        assert_eq!(library.find(4), Wave::read(&b).as_ref());
        assert_eq!(library.find(1), Wave::read(&a).as_ref());
        assert!(library.find(2).is_none());
    }
}
