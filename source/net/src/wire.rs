//! Datagram framing and a small byte writer/reader.
//!
//! Every datagram starts with [`MAGIC`] and a kind byte. Hello, Welcome,
//! Reject and Bye run the session; Data carries renet's packets; Discover
//! and Announce find hosts on the LAN. All numbers are little-endian; a
//! datagram that does not parse is ignored.

/// Datagram prefix: "KH2D".
pub const MAGIC: [u8; 4] = *b"KH2D";
/// Bumped whenever a datagram or game message changes shape.
pub const PROTOCOL: u16 = 1;
/// Longest player name kept, in bytes of UTF-8.
pub const NAME_LIMIT: usize = 32;
/// Longest reason text kept, in bytes of UTF-8.
pub const REASON_LIMIT: usize = 200;

/// The build identity both sides must share (a hash of the executable).
pub type Build = [u8; 32];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Datagram {
    /// Joiner → host: ask to join with a session nonce.
    Hello {
        protocol: u16,
        build: Build,
        nonce: u64,
        name: String,
    },
    /// Host → joiner: accepted.
    Welcome { nonce: u64, name: String },
    /// Host → joiner: refused, with the reason to show.
    Reject { nonce: u64, reason: String },
    /// A renet packet of the session `nonce`.
    Data { nonce: u64, payload: Vec<u8> },
    /// Either side: the session ends.
    Bye { nonce: u64 },
    /// Joiner → anyone on the game port: who is hosting?
    Discover { protocol: u16 },
    /// Host → the asker: a host, its name, and whether it can take a player.
    Announce {
        protocol: u16,
        build: Build,
        name: String,
        open: bool,
    },
}

const HELLO: u8 = 1;
const WELCOME: u8 = 2;
const REJECT: u8 = 3;
const DATA: u8 = 4;
const BYE: u8 = 5;
const DISCOVER: u8 = 6;
const ANNOUNCE: u8 = 7;

impl Datagram {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.bytes(&MAGIC);
        match self {
            Datagram::Hello {
                protocol,
                build,
                nonce,
                name,
            } => {
                w.u8(HELLO);
                w.u16(*protocol);
                w.bytes(build);
                w.u64(*nonce);
                w.text(name, NAME_LIMIT);
            }
            Datagram::Welcome { nonce, name } => {
                w.u8(WELCOME);
                w.u64(*nonce);
                w.text(name, NAME_LIMIT);
            }
            Datagram::Reject { nonce, reason } => {
                w.u8(REJECT);
                w.u64(*nonce);
                w.text(reason, REASON_LIMIT);
            }
            Datagram::Data { nonce, payload } => {
                w.u8(DATA);
                w.u64(*nonce);
                w.bytes(payload);
            }
            Datagram::Bye { nonce } => {
                w.u8(BYE);
                w.u64(*nonce);
            }
            Datagram::Discover { protocol } => {
                w.u8(DISCOVER);
                w.u16(*protocol);
            }
            Datagram::Announce {
                protocol,
                build,
                name,
                open,
            } => {
                w.u8(ANNOUNCE);
                w.u16(*protocol);
                w.bytes(build);
                w.text(name, NAME_LIMIT);
                w.u8(u8::from(*open));
            }
        }
        w.finish()
    }

    pub fn decode(bytes: &[u8]) -> Option<Datagram> {
        let mut r = Reader::new(bytes);
        if r.take(4)? != MAGIC {
            return None;
        }
        let datagram = match r.u8()? {
            HELLO => Datagram::Hello {
                protocol: r.u16()?,
                build: r.array()?,
                nonce: r.u64()?,
                name: r.text(NAME_LIMIT)?,
            },
            WELCOME => Datagram::Welcome {
                nonce: r.u64()?,
                name: r.text(NAME_LIMIT)?,
            },
            REJECT => Datagram::Reject {
                nonce: r.u64()?,
                reason: r.text(REASON_LIMIT)?,
            },
            DATA => {
                let nonce = r.u64()?;
                return Some(Datagram::Data {
                    nonce,
                    payload: r.rest().to_vec(),
                });
            }
            BYE => Datagram::Bye { nonce: r.u64()? },
            DISCOVER => Datagram::Discover { protocol: r.u16()? },
            ANNOUNCE => Datagram::Announce {
                protocol: r.u16()?,
                build: r.array()?,
                name: r.text(NAME_LIMIT)?,
                open: r.u8()? != 0,
            },
            _ => return None,
        };
        r.is_empty().then_some(datagram)
    }
}

/// Cuts `text` to at most `limit` bytes on a character boundary.
pub fn clip(text: &str, limit: usize) -> &str {
    if text.len() <= limit {
        return text;
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Little-endian byte writer.
#[derive(Default)]
pub struct Writer(Vec<u8>);

impl Writer {
    pub fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    pub fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    /// The exact bits, so both sides hold the same value.
    pub fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    pub fn bytes(&mut self, v: &[u8]) {
        self.0.extend_from_slice(v);
    }
    /// A length byte, then at most `limit` (≤ 255) bytes of UTF-8.
    pub fn text(&mut self, v: &str, limit: usize) {
        let v = clip(v, limit.min(255));
        self.u8(v.len() as u8);
        self.bytes(v.as_bytes());
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn finish(self) -> Vec<u8> {
        self.0
    }
}

/// Bounds-checked little-endian reader: every getter returns None past the
/// end.
pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    pub fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n)?;
        let s = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(s)
    }
    pub fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }
    pub fn u8(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.array()?))
    }
    pub fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.array()?))
    }
    pub fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.array()?))
    }
    pub fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.array()?))
    }
    pub fn f32(&mut self) -> Option<f32> {
        Some(f32::from_bits(self.u32()?))
    }
    pub fn text(&mut self, limit: usize) -> Option<String> {
        let n = self.u8()? as usize;
        if n > limit {
            return None;
        }
        String::from_utf8(self.take(n)?.to_vec()).ok()
    }
    pub fn rest(&mut self) -> &'a [u8] {
        let s = &self.bytes[self.at..];
        self.at = self.bytes.len();
        s
    }
    pub fn is_empty(&self) -> bool {
        self.at >= self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<Datagram> {
        vec![
            Datagram::Hello {
                protocol: PROTOCOL,
                build: [7; 32],
                nonce: 0x0102_0304_0506_0708,
                name: "Riku".into(),
            },
            Datagram::Welcome {
                nonce: 9,
                name: "Kairi".into(),
            },
            Datagram::Reject {
                nonce: 9,
                reason: "match full".into(),
            },
            Datagram::Data {
                nonce: 3,
                payload: vec![1, 2, 3, 250],
            },
            Datagram::Data {
                nonce: 3,
                payload: Vec::new(),
            },
            Datagram::Bye { nonce: 11 },
            Datagram::Discover { protocol: PROTOCOL },
            Datagram::Announce {
                protocol: PROTOCOL,
                build: [1; 32],
                name: "Host".into(),
                open: true,
            },
        ]
    }

    #[test]
    fn datagrams_round_trip() {
        for d in all() {
            assert_eq!(Datagram::decode(&d.encode()), Some(d));
        }
    }

    #[test]
    fn truncated_or_foreign_datagrams_are_ignored() {
        for d in all() {
            let bytes = d.encode();
            // Data takes the rest of the datagram, so only its header can be cut.
            let shortest = if matches!(d, Datagram::Data { .. }) {
                13
            } else {
                bytes.len()
            };
            for n in 0..shortest {
                assert_eq!(Datagram::decode(&bytes[..n]), None, "{d:?} cut at {n}");
            }
        }
        assert_eq!(Datagram::decode(b"XXXX\x05\0\0\0\0\0\0\0\0"), None);
        let mut bytes = Datagram::Bye { nonce: 1 }.encode();
        bytes.push(0);
        assert_eq!(Datagram::decode(&bytes), None, "trailing bytes");
        let mut bytes = MAGIC.to_vec();
        bytes.push(99);
        assert_eq!(Datagram::decode(&bytes), None, "unknown kind");
    }

    #[test]
    fn names_are_clipped_on_a_character_boundary() {
        let long = "é".repeat(40);
        let d = Datagram::Welcome {
            nonce: 1,
            name: long,
        };
        let Some(Datagram::Welcome { name, .. }) = Datagram::decode(&d.encode()) else {
            panic!("welcome decodes");
        };
        assert_eq!(name, "é".repeat(16));
        assert_eq!(clip("abc", 2), "ab");
        assert_eq!(clip("aé", 2), "a");
    }

    #[test]
    fn reader_stops_at_the_end() {
        let mut w = Writer::default();
        w.u16(0xbeef);
        w.f32(-1.5);
        w.i32(-7);
        let bytes = w.finish();
        let mut r = Reader::new(&bytes);
        assert_eq!(r.u16(), Some(0xbeef));
        assert_eq!(r.f32(), Some(-1.5));
        assert_eq!(r.i32(), Some(-7));
        assert!(r.is_empty());
        assert_eq!(r.u8(), None);
    }
}
