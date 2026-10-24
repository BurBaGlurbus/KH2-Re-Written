//! Reading installed files by path name. A HED record's key is the MD5
//! digest of the file's path (for example `map/tt08.map`), so a name the
//! game builds at run time can be looked up directly instead of carrying an
//! ordinal and digest in a private selection file.
use crate::{original_payload, probe_source::CheckedIndex, probe_source::ProbeSource};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// The six KH2 indexes, in package order.
pub const INDEXES: [&str; 6] = [
    "Image/dt/kh2_first.hed",
    "Image/dt/kh2_second.hed",
    "Image/dt/kh2_third.hed",
    "Image/dt/kh2_fourth.hed",
    "Image/dt/kh2_fifth.hed",
    "Image/dt/kh2_sixth.hed",
];

/// MD5 (RFC 1321) of `data`.
pub fn md5(data: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let k: [u32; 64] = std::array::from_fn(|i| ((i as f64 + 1.).sin().abs() * 4294967296.) as u32);
    let mut state = [0x67452301u32, 0xefcdab89, 0x98badcfe, 0x10325476];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((data.len() as u64).wrapping_mul(8)).to_le_bytes());
    for block in message.as_chunks::<64>().0 {
        let m: [u32; 16] = std::array::from_fn(|i| {
            u32::from_le_bytes(block[i * 4..i * 4 + 4].try_into().unwrap())
        });
        let [mut a, mut b, mut c, mut d] = state;
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let rotated = a
                .wrapping_add(f)
                .wrapping_add(k[i])
                .wrapping_add(m[g])
                .rotate_left(S[i]);
            (a, d, c, b) = (d, c, b, b.wrapping_add(rotated));
        }
        for (s, v) in state.iter_mut().zip([a, b, c, d]) {
            *s = s.wrapping_add(v);
        }
    }
    let mut out = [0; 16];
    for (i, s) in state.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&s.to_le_bytes());
    }
    out
}

/// The six checked indexes of one bound source, opened once.
pub struct Library {
    indexes: Vec<(&'static str, CheckedIndex)>,
}

/// Where a name was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub index: &'static str,
    pub ordinal: usize,
    pub name_hash: [u8; 16],
}

impl Library {
    pub fn open(source: &ProbeSource) -> Result<Self> {
        let indexes = INDEXES
            .iter()
            .map(|&name| Ok((name, source.open_index(name)?)))
            .collect::<Result<_>>()?;
        Ok(Self { indexes })
    }

    /// Every index holding `name`, in package order.
    pub fn find_all(&self, name: &str) -> Vec<Found> {
        let hash = md5(name.as_bytes());
        self.indexes
            .iter()
            .filter_map(|(index, checked)| {
                let ordinal = checked
                    .inventory
                    .entries
                    .iter()
                    .position(|e| e.name_hash == hash)?;
                Some(Found {
                    index,
                    ordinal,
                    name_hash: hash,
                })
            })
            .collect()
    }

    /// The copy of `name` the program reads: the last package holding it
    /// (later packages carry the patched copies; repeated names checked so
    /// far are byte-identical) [inference].
    pub fn find(&self, name: &str) -> Option<Found> {
        self.find_all(name).pop()
    }

    /// The original (pre-remaster) bytes of `name`.
    pub fn read(&mut self, name: &str) -> Result<Vec<u8>> {
        let found = self
            .find(name)
            .ok_or_else(|| format!("{name} is not in the installed packages"))?;
        self.read_found(&found)
    }

    /// `read` for an entry that may exceed the 32 MiB inspection limit, up
    /// to `limit` bytes.
    pub fn read_limited(&mut self, name: &str, limit: u32) -> Result<Vec<u8>> {
        let found = self
            .find(name)
            .ok_or_else(|| format!("{name} is not in the installed packages"))?;
        self.read_found_limited(&found, limit)
    }

    pub fn read_found(&mut self, found: &Found) -> Result<Vec<u8>> {
        self.read_found_limited(found, crate::original_payload::MAX_ORIGINAL_BYTES)
    }

    pub fn read_found_limited(&mut self, found: &Found, limit: u32) -> Result<Vec<u8>> {
        let (_, checked) = self
            .indexes
            .iter_mut()
            .find(|(i, _)| *i == found.index)
            .ok_or("index not open")?;
        let entry = checked
            .inventory
            .entries
            .get(found.ordinal)
            .ok_or("entry outside index")?;
        if entry.name_hash != found.name_hash {
            return Err("entry identity mismatch".into());
        }
        Ok(original_payload::read_original_limited(&mut checked.reader, entry, limit)?.bytes)
    }

    /// The remastered assets stored after `name`'s original bytes (the PC
    /// textures that replace its PS2 images), in their declared order.
    pub fn read_replacements(
        &mut self,
        name: &str,
    ) -> Result<Vec<crate::replacement_payload::ReplacementSample>> {
        let found = self
            .find(name)
            .ok_or_else(|| format!("{name} is not in the installed packages"))?;
        let (_, checked) = self
            .indexes
            .iter_mut()
            .find(|(i, _)| *i == found.index)
            .ok_or("index not open")?;
        let entry = checked
            .inventory
            .entries
            .get(found.ordinal)
            .ok_or("entry outside index")?;
        Ok(crate::replacement_payload::read_all_replacements(
            &mut checked.reader,
            entry,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::md5;

    fn hex(d: [u8; 16]) -> String {
        d.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn rfc1321_vectors() {
        for (input, digest) in [
            ("", "d41d8cd98f00b204e9800998ecf8427e"),
            ("a", "0cc175b9c0f1b6a831c399e269772661"),
            ("abc", "900150983cd24fb0d6963f7d28e17f72"),
            ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
            (
                "abcdefghijklmnopqrstuvwxyz",
                "c3fcd3d76192e4007dfb496cca67e13b",
            ),
            (
                "12345678901234567890123456789012345678901234567890123456789012345678901234567890",
                "57edf4a22be3c955ac49da2e2107b67a",
            ),
        ] {
            assert_eq!(hex(md5(input.as_bytes())), digest, "{input:?}");
        }
    }
}
