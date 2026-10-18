//! Event (cutscene) scripts: ARD BAR entries of type 22. A script is a run of
//! `[u16 size][u16 opcode][payload]` records ended by a zero size. Record
//! layouts follow the opcodes the PC event player reads; opcodes this crate
//! does not decode are kept as raw payloads.
use std::io;

/// One camera channel key: interpolation code (top 3 bits of the flag word),
/// key time (low 29 bits, sign-extended), value and Hermite tangents.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraKey {
    pub interpolation: u8,
    pub time: i32,
    pub value: f32,
    pub tangent_in: f32,
    pub tangent_out: f32,
}

/// Opcode 0x13: eight key channels — eye x/y/z, target x/y/z, roll, fov.
#[derive(Clone, Debug, PartialEq)]
pub struct CameraData {
    pub id: i16,
    pub channels: [Vec<CameraKey>; 8],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Voice {
    pub frame: i16,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Load {
    Actor {
        object: u16,
        actor: i16,
        name: String,
    },
    Motion {
        object: u16,
        actor: i16,
        delete: i16,
        name: String,
    },
    Audio(String),
    Effect {
        id: i16,
        name: String,
    },
    Other {
        opcode: u16,
        payload: Vec<u8>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// 0x00
    Project {
        memory: u32,
        version: u8,
        camera_type: u8,
        name: String,
    },
    /// 0x01
    Actor {
        object: u16,
        actor: i16,
        name: String,
    },
    /// 0x02
    ActorPosition {
        sub: i16,
        field_02: i16,
        position: [f32; 3],
        rotation: [f32; 3],
        scale: f32,
        actor: i16,
        frame: i16,
    },
    /// 0x03
    Map {
        world: String,
        place: i16,
    },
    /// 0x06
    Camera {
        camera: i16,
        start: i16,
        end: i16,
        field_06: i16,
    },
    /// 0x08
    EndFrame {
        frame: i16,
        field_02: i16,
    },
    /// 0x09
    Effect {
        start: i16,
        loop_frame: i16,
        effect: i16,
        pax: i16,
        pax_entry: i16,
        end_type: i16,
        fade: i16,
    },
    /// 0x0A
    AttachEffect {
        start: i16,
        end: i16,
        attach: i16,
        actor: i16,
        bone: i16,
        pax_entry: i16,
        kind: i16,
    },
    /// 0x0D
    Part {
        start: i16,
        end: i16,
        actor: i16,
        parts: Vec<i16>,
    },
    /// 0x0F
    Setup,
    /// 0x16 BgGrupe (FUN_1402daa70): shows or hides map background group
    /// `group` (a bit of the area's group mask) at `frame`.
    BgGroup {
        frame: i16,
        group: u8,
        show: bool,
    },
    /// 0x10
    Start {
        fade_in: i16,
        field_02: i16,
    },
    /// 0x12
    Fade {
        frame: i16,
        duration: i16,
        kind: i16,
    },
    /// 0x13
    CameraData(CameraData),
    /// 0x15
    Subtitle {
        frame: i16,
        slot: i16,
        message: u16,
        hide: i16,
    },
    /// 0x17
    Blur {
        frame: i16,
        switch: u8,
        alpha: u8,
        rotation: f32,
        x: i16,
        y: i16,
        rotation_frames: i16,
        field_0e: i16,
    },
    /// 0x19
    TextureAnimation {
        frame: i16,
        actor: i16,
        number: i16,
        flag: u8,
        field_07: u8,
    },
    /// 0x1A
    ActorLeave {
        frame: i16,
        actor: i16,
    },
    /// 0x22
    WideMask {
        frame: i16,
        flag: i16,
    },
    /// 0x23
    Voices(Vec<Voice>),
    /// 0x24
    Loads {
        start: i16,
        end: i16,
        field_06: i16,
        loads: Vec<Load>,
    },
    /// 0x2A
    SoundData {
        put: i16,
        bank: i16,
        wave: i16,
    },
    /// 0x2B
    PlaySound {
        put: i16,
        kind: i16,
        number: i32,
        frame: i16,
        field_0a: i16,
    },
    /// 0x2C
    Animation {
        start: i16,
        end: i16,
        motion_start: i16,
        loop_start: i16,
        loop_end: i16,
        actor: i16,
        blend: i16,
        name: String,
    },
    /// 0x2E
    PlayBgm {
        frame: i16,
        volume_continue: i16,
        volume_start: u8,
        volume_end: u8,
        fade: u8,
        bank: u8,
    },
    /// 0x2F
    ReadBgm {
        put: i16,
        bgm: i16,
    },
    /// 0x30
    SetBgm {
        frame: i16,
        bank: i16,
        bgm: i16,
    },
    /// 0x3F
    StopEffect {
        frame: i16,
        put: i16,
    },
    Other {
        opcode: u16,
        payload: Vec<u8>,
    },
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> io::Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&e| e <= self.bytes.len())
            .ok_or_else(|| invalid("truncated event record"))?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }
    fn i16(&mut self) -> io::Result<i16> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> io::Result<i32> {
        Ok(self.u32()? as i32)
    }
    fn f32(&mut self) -> io::Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn cstr(&mut self) -> io::Result<String> {
        let rest = &self.bytes[self.at.min(self.bytes.len())..];
        let len = rest
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| invalid("unterminated event string"))?;
        let s = rest[..len].iter().map(|&b| b as char).collect();
        self.at += len + 1;
        Ok(s)
    }
}

fn camera_key(c: &mut Cursor) -> io::Result<CameraKey> {
    let flag = c.u32()?;
    Ok(CameraKey {
        interpolation: (flag >> 29) as u8,
        time: (((flag & 0x1fff_ffff) ^ 0x1000_0000) as i32) - 0x1000_0000,
        value: c.f32()?,
        tangent_in: c.f32()?,
        tangent_out: c.f32()?,
    })
}

fn load(opcode: u16, payload: &[u8]) -> io::Result<Load> {
    let mut c = Cursor {
        bytes: payload,
        at: 0,
    };
    Ok(match opcode {
        0x38 => Load::Actor {
            object: c.u16()?,
            actor: c.i16()?,
            name: c.cstr()?,
        },
        0x25 => Load::Motion {
            object: c.u16()?,
            actor: c.i16()?,
            delete: c.i16()?,
            name: c.cstr()?,
        },
        0x26 => Load::Audio(c.cstr()?),
        0x39 => Load::Effect {
            id: c.i16()?,
            name: c.cstr()?,
        },
        _ => Load::Other {
            opcode,
            payload: payload.to_vec(),
        },
    })
}

/// `record` is the payload after the 4-byte header; `size` the record size.
fn command(opcode: u16, record: &[u8]) -> io::Result<Command> {
    let mut c = Cursor {
        bytes: record,
        at: 0,
    };
    Ok(match opcode {
        0x00 => Command::Project {
            memory: c.u32()?,
            version: c.u8()?,
            camera_type: c.u8()?,
            name: c.cstr()?,
        },
        0x01 => Command::Actor {
            object: c.u16()?,
            actor: c.i16()?,
            name: c.cstr()?,
        },
        0x02 => Command::ActorPosition {
            sub: c.i16()?,
            field_02: c.i16()?,
            position: [c.f32()?, c.f32()?, c.f32()?],
            rotation: [c.f32()?, c.f32()?, c.f32()?],
            scale: c.f32()?,
            actor: c.i16()?,
            frame: c.i16()?,
        },
        0x03 => {
            let place = c.i16()?;
            Command::Map {
                world: c.cstr()?,
                place,
            }
        }
        0x06 => Command::Camera {
            camera: c.i16()?,
            start: c.i16()?,
            end: c.i16()?,
            field_06: c.i16()?,
        },
        0x08 => Command::EndFrame {
            frame: c.i16()?,
            field_02: c.i16()?,
        },
        0x09 => Command::Effect {
            start: c.i16()?,
            loop_frame: c.i16()?,
            effect: c.i16()?,
            pax: c.i16()?,
            pax_entry: c.i16()?,
            end_type: c.i16()?,
            fade: c.i16()?,
        },
        0x0a => Command::AttachEffect {
            start: c.i16()?,
            end: c.i16()?,
            attach: c.i16()?,
            actor: c.i16()?,
            bone: c.i16()?,
            pax_entry: c.i16()?,
            kind: c.i16()?,
        },
        0x0d => {
            let (start, end, actor) = (c.i16()?, c.i16()?, c.i16()?);
            let parts = record[6..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| i16::from_le_bytes(*b))
                .collect();
            Command::Part {
                start,
                end,
                actor,
                parts,
            }
        }
        0x0f => Command::Setup,
        0x10 => Command::Start {
            fade_in: c.i16()?,
            field_02: c.i16()?,
        },
        0x12 => Command::Fade {
            frame: c.i16()?,
            duration: c.i16()?,
            kind: c.i16()?,
        },
        0x13 => {
            let id = c.i16()?;
            let mut headers = [(0usize, 0usize); 8];
            for h in &mut headers {
                let index = c.i16()?;
                let count = c.i16()?;
                if index < 0 || count < 0 {
                    return Err(invalid("negative camera channel range"));
                }
                *h = (index as usize, count as usize);
            }
            // Keys start 4-aligned in the record (record start is 4-aligned).
            c.at = (c.at + 4).div_ceil(4) * 4 - 4;
            let total = headers.iter().map(|(i, n)| i + n).max().unwrap_or(0);
            let keys = (0..total)
                .map(|_| camera_key(&mut c))
                .collect::<io::Result<Vec<_>>>()?;
            Command::CameraData(CameraData {
                id,
                channels: headers.map(|(i, n)| keys[i..i + n].to_vec()),
            })
        }
        0x15 => Command::Subtitle {
            frame: c.i16()?,
            slot: c.i16()?,
            message: c.u16()?,
            hide: c.i16()?,
        },
        0x16 => Command::BgGroup {
            frame: c.i16()?,
            group: c.u8()?,
            show: c.u8()? != 0,
        },
        0x17 => Command::Blur {
            frame: c.i16()?,
            switch: c.u8()?,
            alpha: c.u8()?,
            rotation: c.f32()?,
            x: c.i16()?,
            y: c.i16()?,
            rotation_frames: c.i16()?,
            field_0e: c.i16()?,
        },
        0x19 => Command::TextureAnimation {
            frame: c.i16()?,
            actor: c.i16()?,
            number: c.i16()?,
            flag: c.u8()?,
            field_07: c.u8()?,
        },
        0x1a => Command::ActorLeave {
            frame: c.i16()?,
            actor: c.i16()?,
        },
        0x22 => Command::WideMask {
            frame: c.i16()?,
            flag: c.i16()?,
        },
        0x23 => {
            let count = c.u16()? as usize;
            c.u16()?;
            let mut voices = Vec::with_capacity(count);
            for i in 0..count {
                c.at = 4 + 0x20 * i;
                c.take(4)?;
                let frame = c.i16()?;
                c.u8()?;
                voices.push(Voice {
                    frame,
                    name: c.cstr()?,
                });
            }
            Command::Voices(voices)
        }
        0x24 => {
            let count = c.i16()?;
            let (start, end, field_06) = (c.i16()?, c.i16()?, c.i16()?);
            let mut loads = Vec::new();
            for _ in 0..count.max(0) {
                let at = c.at;
                let opcode = c.u16()?;
                let len = c.u16()? as usize;
                if len < 4 {
                    return Err(invalid("short event load record"));
                }
                c.at = at;
                let item = c.take(len)?;
                loads.push(load(opcode, &item[4..])?);
            }
            Command::Loads {
                start,
                end,
                field_06,
                loads,
            }
        }
        0x2a => Command::SoundData {
            put: c.i16()?,
            bank: c.i16()?,
            wave: c.i16()?,
        },
        0x2b => Command::PlaySound {
            put: c.i16()?,
            kind: c.i16()?,
            number: c.i32()?,
            frame: c.i16()?,
            field_0a: c.i16()?,
        },
        0x2c => Command::Animation {
            start: c.i16()?,
            end: c.i16()?,
            motion_start: c.i16()?,
            loop_start: c.i16()?,
            loop_end: c.i16()?,
            actor: c.i16()?,
            blend: c.i16()?,
            name: c.cstr()?,
        },
        0x2e => Command::PlayBgm {
            frame: c.i16()?,
            volume_continue: c.i16()?,
            volume_start: c.u8()?,
            volume_end: c.u8()?,
            fade: c.u8()?,
            bank: c.u8()?,
        },
        0x2f => Command::ReadBgm {
            put: c.i16()?,
            bgm: c.i16()?,
        },
        0x30 => Command::SetBgm {
            frame: c.i16()?,
            bank: c.i16()?,
            bgm: c.i16()?,
        },
        0x3f => Command::StopEffect {
            frame: c.i16()?,
            put: c.i16()?,
        },
        _ => Command::Other {
            opcode,
            payload: record.to_vec(),
        },
    })
}

/// Decodes one event script.
pub fn read(bytes: &[u8]) -> io::Result<Vec<Command>> {
    let mut at = 0;
    let mut out = Vec::new();
    while at + 2 <= bytes.len() {
        let size = u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
        if size == 0 {
            return Ok(out);
        }
        if at + 4 > bytes.len() {
            break;
        }
        let opcode = u16::from_le_bytes([bytes[at + 2], bytes[at + 3]]);
        let record = bytes
            .get(at + 4..at + size)
            .filter(|_| size >= 4)
            .ok_or_else(|| invalid("event record exceeds script"))?;
        out.push(command(opcode, record)?);
        at += size;
    }
    Err(invalid("event script has no terminator"))
}

/// The type-22 entry named `name` in an ARD archive.
pub fn find_in_ard<'a>(ard: &'a [u8], name: &str) -> io::Result<&'a [u8]> {
    let table = crate::bar::read_bar(ard)?;
    let mut key = [0u8; 4];
    for (k, b) in key.iter_mut().zip(name.bytes()) {
        *k = b;
    }
    let entry = table
        .entries
        .iter()
        .find(|e| e.entry_type == 22 && e.name_bytes == key && e.offset > 0)
        .ok_or_else(|| invalid("event entry absent from ARD"))?;
    Ok(&ard[entry.offset as usize..entry.offset as usize + entry.size as usize])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(opcode: u16, payload: &[u8]) -> Vec<u8> {
        let mut out = ((payload.len() + 4) as u16).to_le_bytes().to_vec();
        out.extend(opcode.to_le_bytes());
        out.extend(payload);
        out
    }

    #[test]
    fn decodes_a_background_group_toggle() {
        let mut script = record(0x16, &[0x00, 0x00, 0x01, 0x01]);
        script.extend(record(0x16, &[0x2c, 0x01, 0x07, 0x00]));
        script.extend([0, 0]);
        assert_eq!(
            read(&script).unwrap(),
            vec![
                Command::BgGroup {
                    frame: 0,
                    group: 1,
                    show: true
                },
                Command::BgGroup {
                    frame: 300,
                    group: 7,
                    show: false
                }
            ]
        );
    }

    #[test]
    fn decodes_records_and_terminator() {
        let mut script = record(0x0f, &[]);
        script.extend(record(0x08, &[0x45, 0x04, 0x12, 0x00]));
        let mut anim = Vec::new();
        for v in [0i16, 152, 0, -1, -1, 3, 0] {
            anim.extend(v.to_le_bytes());
        }
        anim.extend(b"anm_tt/X\0\0");
        script.extend(record(0x2c, &anim));
        script.extend([0, 0]);
        let commands = read(&script).unwrap();
        assert_eq!(commands[0], Command::Setup);
        assert_eq!(
            commands[1],
            Command::EndFrame {
                frame: 1093,
                field_02: 18
            }
        );
        let Command::Animation {
            end, actor, name, ..
        } = &commands[2]
        else {
            panic!()
        };
        assert_eq!((*end, *actor, name.as_str()), (152, 3, "anm_tt/X"));
        assert!(read(&script[..script.len() - 2]).is_err());
    }

    #[test]
    fn camera_keys_split_flag_word() {
        let mut p = 7i16.to_le_bytes().to_vec();
        for ch in 0..8i16 {
            p.extend((ch.min(1)).to_le_bytes());
            p.extend(1i16.to_le_bytes());
        }
        p.extend([0, 0]); // align
        for (flag, value) in [(2u32 << 29, 1.5f32), ((1u32 << 29) | 0x1fff_fffc, -2.0)] {
            p.extend(flag.to_le_bytes());
            p.extend(value.to_le_bytes());
            p.extend(0f32.to_le_bytes());
            p.extend(0f32.to_le_bytes());
        }
        let mut script = record(0x13, &p);
        script.extend([0, 0]);
        let Command::CameraData(cam) = &read(&script).unwrap()[0] else {
            panic!()
        };
        assert_eq!(cam.id, 7);
        assert_eq!(cam.channels[0][0].interpolation, 2);
        assert_eq!(cam.channels[1][0].time, -4);
        assert_eq!(cam.channels[1][0].interpolation, 1);
    }
}
