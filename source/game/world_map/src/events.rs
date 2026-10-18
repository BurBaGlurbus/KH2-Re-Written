//! The world map's event player (#89): FUN_14038de90 / FUN_14038dc60 play
//! the queued world events (each distinct world-map event id once, in
//! priority order) through FUN_14038fae0, which runs the event's script
//! from `libretto-wm.bar` (FUN_1403ec4d0 / FUN_1403ec420) a command per
//! step: the common commands of FUN_1403ec260 and the world map's own of
//! FUN_14038f270.

/// One script command: {u16 op, u16 n, n words}.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// 1: a message window (FUN_140393760: WINDOW_BALLOON, or
    /// WINDOW_TUTORIAL after op 0x107), held until it closes.
    Message(u32),
    /// 2: wait (FUN_140150080), in frames.
    Wait(f32),
    /// 3: FUN_1403aabe0(n, 0) [U].
    Signal(u32),
    /// 4: the script ends.
    Stop,
    /// 5..8: the screen fade (FUN_140156ce0 out / FUN_140156c90 in), black
    /// or white, the float halved into the fade's n.
    Fade {
        out: bool,
        white: bool,
        n: u32,
    },
    /// 0x100: preload objects (FUN_1403e0460(id | 0x40000000)).
    Preload(Vec<u32>),
    /// 0x101: the event's archive is named, `libretto/<lang>/wm<event>.bar`
    /// (format 0x1405c0798, "fm" for languages 0 and 7, else "us"), and its
    /// load starts (thunk_FUN_14039e000); FUN_14039d460(0) drops the load
    /// cache's unused files.
    Archive,
    /// 0x102: a 3D object widget in slot `.0` for object `.1`
    /// (worldmap::ObjWidget, FUN_140393920); the first hides the map.
    Widget(u32, u32),
    /// 0x103: the slot plays the first archive entry (FUN_140535440) and
    /// queues the rest (FUN_140535420); 0x111 queues them all.
    WidgetMotion(u32, Vec<u32>),
    WidgetQueue(u32, Vec<u32>),
    /// 0x104: the event camera to `target` from `target + offset` over
    /// `frames`, then a wait of as many frames (FUN_1403943f0).
    Camera {
        target: [f32; 3],
        offset: [f32; 3],
        frames: f32,
    },
    /// 0x105: the objects' looks refresh (FUN_1403940b0, 240 frames,
    /// FUN_140394050, sound 16008, 90 frames).
    Refresh,
    /// 0x106: the script waits for the loads (FUN_14039e340) and keeps the
    /// archive (FUN_14039cc40 → +0xa0).
    WaitLoads,
    /// 0x107: later messages use WINDOW_TUTORIAL.
    TutorialWindows,
    /// 0x108: the voice in this archive entry plays after the last stops
    /// (thunk_FUN_1401df2f0, thunk_FUN_1401dfbb0 at volume 0x3fff): an
    /// "ORIGIN" stub naming the voice file.
    Voice(u32),
    /// 0x109: a widget's position (+0x318, w = 1); 0x10a: its rotation
    /// (+0x308, radians).
    WidgetPosition(u32, [f32; 3]),
    WidgetRotation(u32, [f32; 3]),
    /// 0x10b / 0x10c: the widgets' view translation and rotation
    /// (FUN_140393ad0 / FUN_140393b10 rebuild its matrix with
    /// FUN_1403b7410).
    WidgetViewPosition([f32; 3]),
    WidgetViewRotation([f32; 3]),
    /// 0x10d: a widget goes (FUN_14038f1f0); the last brings the map back.
    RemoveWidget(u32),
    /// 0x10e: jump to (world, area); 0x10f: to a spawn of a world
    /// (FUN_14039fbc0), both through FUN_140152990.
    Jump {
        world: u8,
        area: u8,
    },
    JumpSpawn {
        world: u8,
        spawn: u32,
    },
    /// 0x110: the cursor (`DAT_142a105d0`) rides Hollow Bastion's symbol
    /// (FUN_1403b4110(0x3a3), FUN_1403d9fe0 with flags 0x4002), turned by π
    /// on it (FUN_1403b65f0), and plays motion 0x9c (FUN_1403c86a0).
    ShipToHollowBastion,
    /// 0x112: every widget goes (FUN_14038f150); 0x113: looks refresh now.
    ClearWidgets,
    RefreshNow,
    Other(u16, Vec<u32>),
}

fn f(w: u32) -> f32 {
    f32::from_bits(w)
}

/// FUN_1403fc730's walk over a script until op 0 with no words.
pub fn read_script(bytes: &[u8]) -> Vec<Op> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        let op = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
        let n = u16::from_le_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if op == 0 && n == 0 {
            break;
        }
        let a: Vec<u32> = (0..n)
            .filter_map(|k| bytes.get(i + 4 + 4 * k..i + 8 + 4 * k))
            .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
            .collect();
        let w = |k: usize| a.get(k).copied().unwrap_or(0);
        let v3 = |k: usize| [f(w(k)), f(w(k + 1)), f(w(k + 2))];
        out.push(match op {
            1 => Op::Message(w(0)),
            2 => Op::Wait(f(w(0))),
            3 => Op::Signal(w(0)),
            4 => Op::Stop,
            5..=8 => Op::Fade {
                out: op == 5 || op == 7,
                white: op >= 7,
                // DAT_1405b18f4 (1.0) × DAT_140623b18 (2.0).
                n: (f(w(0)) / 2.) as u32,
            },
            0x100 => Op::Preload(a.clone()),
            0x101 => Op::Archive,
            0x102 => Op::Widget(w(0), w(1)),
            0x103 => Op::WidgetMotion(w(0), a.get(1..).unwrap_or(&[]).to_vec()),
            0x111 => Op::WidgetQueue(w(0), a.get(1..).unwrap_or(&[]).to_vec()),
            0x104 => Op::Camera {
                target: v3(0),
                offset: v3(3),
                frames: f(w(6)),
            },
            0x105 => Op::Refresh,
            0x106 => Op::WaitLoads,
            0x107 => Op::TutorialWindows,
            0x108 => Op::Voice(w(0)),
            0x109 => Op::WidgetPosition(w(0), v3(1)),
            0x10a => Op::WidgetRotation(w(0), v3(1)),
            0x10b => Op::WidgetViewPosition(v3(0)),
            0x10c => Op::WidgetViewRotation(v3(0)),
            0x10d => Op::RemoveWidget(w(0)),
            0x10e => Op::Jump {
                world: w(0) as u8,
                area: w(1) as u8,
            },
            0x10f => Op::JumpSpawn {
                world: w(0) as u8,
                spawn: w(1),
            },
            0x110 => Op::ShipToHollowBastion,
            0x112 => Op::ClearWidgets,
            0x113 => Op::RefreshNow,
            _ => Op::Other(op, a.clone()),
        });
        i += 4 + 4 * n;
    }
    out
}

/// The sound op 0x105 plays (FUN_1401de0c0(0x3e88): bank 16, sound 8).
pub const REFRESH_SOUND: u32 = 0x3e88;
/// Op 0x105's waits (DAT_1405b4b20, DAT_140623be0).
pub const REFRESH_WAITS: [f32; 2] = [240., 90.];

/// Op 0x101's archive: `sprintf("libretto/%s/wm%d.bar", lang, event)`.
pub fn archive_name(event: u32, language: &str) -> String {
    format!("libretto/{language}/wm{event}.bar")
}

/// An archive entry that holds an "ORIGIN" stub (BAR types 0x1f, 0x22 and
/// 0x30, FUN_1403a64f0): the file the PC build loads in its place, named at
/// +0x10 (`voice/wm/<lang>/<name>`).
pub fn origin_path(stub: &[u8]) -> Option<String> {
    if stub.get(..7)? != b"ORIGIN\0" {
        return None;
    }
    let name = stub.get(0x10..)?;
    let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
    std::str::from_utf8(&name[..end]).ok().map(str::to_owned)
}

/// The installed voice file of an ORIGIN voice path: its base name in the
/// language's event voices [I: FUN_14012db50's remap is not traced; every
/// world map voice stub resolves this way in the selected install].
pub fn voice_file(origin: &str, language: &str) -> Option<String> {
    let name = origin.rsplit('/').next().filter(|n| !n.is_empty())?;
    Some(format!("voice/{language}/event/{name}.win32.scd"))
}

/// What the player asks of the program this tick.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Open a message window (`tutorial`: WINDOW_TUTORIAL) and report when
    /// it closed.
    Message {
        id: u32,
        tutorial: bool,
    },
    Fade {
        out: bool,
        white: bool,
        n: u32,
    },
    Camera {
        target: [f32; 3],
        offset: [f32; 3],
        frames: f32,
    },
    /// FUN_1403940b0: the objects' pre-refresh (symbols whose record flag
    /// bit 0 is set, FUN_1403900d0).
    PreRefresh,
    /// FUN_140394050: the objects re-read their look and the symbol slots
    /// apply.
    Refresh,
    Sound(u32),
    /// A widget op for the scene (widgets::Scene::apply).
    Widgets(Op),
    /// Op 0x108: the voice of this archive entry.
    Voice(u32),
    /// Op 0x110.
    ShipToHollowBastion,
    Jump {
        world: u8,
        area: u8,
    },
    /// Op 0x10f: the jump by program id (FUN_14039fbc0 resolves it).
    JumpProgram {
        world: u8,
        program: u16,
    },
    /// Logged, not carried out here.
    Unported(Op),
}

/// One event's run (FUN_14038fae0's object and its libretto slot).
#[derive(Clone, Debug, PartialEq)]
pub struct EventPlayer {
    pub id: u32,
    script: Vec<Op>,
    pc: usize,
    wait: f32,
    /// The refresh's second half is pending after its first wait.
    refresh_stage: u8,
    waiting_message: bool,
    tutorial: bool,
    pub done: bool,
}

impl EventPlayer {
    pub fn new(id: u32, script: Vec<Op>) -> Self {
        Self {
            id,
            script,
            pc: 0,
            wait: 0.,
            refresh_stage: 0,
            waiting_message: false,
            tutorial: false,
            done: false,
        }
    }

    /// The message window the player waits on closed.
    pub fn message_closed(&mut self) {
        self.waiting_message = false;
    }

    /// One tick: commands run until one waits.
    pub fn tick(&mut self) -> Vec<Action> {
        let mut out = Vec::new();
        if self.done || self.waiting_message {
            return out;
        }
        if self.wait > 0. {
            self.wait -= 1.;
            if self.wait > 0. {
                return out;
            }
        }
        if self.refresh_stage == 1 {
            self.refresh_stage = 2;
            out.push(Action::Refresh);
            out.push(Action::Sound(REFRESH_SOUND));
            self.wait = REFRESH_WAITS[1];
            return out;
        }
        self.refresh_stage = 0;
        loop {
            let Some(op) = self.script.get(self.pc).cloned() else {
                self.done = true;
                return out;
            };
            self.pc += 1;
            match op {
                Op::Stop => {
                    self.done = true;
                    return out;
                }
                Op::Wait(frames) => {
                    self.wait = frames;
                    if self.wait > 0. {
                        return out;
                    }
                }
                Op::Message(id) => {
                    out.push(Action::Message {
                        id,
                        tutorial: self.tutorial,
                    });
                    self.waiting_message = true;
                    return out;
                }
                Op::Fade { out: o, white, n } => out.push(Action::Fade { out: o, white, n }),
                Op::Camera {
                    target,
                    offset,
                    frames,
                } => {
                    out.push(Action::Camera {
                        target,
                        offset,
                        frames,
                    });
                    self.wait = frames;
                    if frames > 0. {
                        return out;
                    }
                }
                Op::Refresh => {
                    out.push(Action::PreRefresh);
                    self.refresh_stage = 1;
                    self.wait = REFRESH_WAITS[0];
                    return out;
                }
                Op::RefreshNow => out.push(Action::Refresh),
                Op::TutorialWindows => self.tutorial = true,
                Op::Jump { world, area } => out.push(Action::Jump { world, area }),
                Op::JumpSpawn { world, spawn } => out.push(Action::JumpProgram {
                    world,
                    program: spawn as u16,
                }),
                Op::Voice(entry) => out.push(Action::Voice(entry)),
                Op::ShipToHollowBastion => out.push(Action::ShipToHollowBastion),
                op @ (Op::Widget(..)
                | Op::WidgetMotion(..)
                | Op::WidgetQueue(..)
                | Op::WidgetPosition(..)
                | Op::WidgetRotation(..)
                | Op::WidgetViewPosition(_)
                | Op::WidgetViewRotation(_)
                | Op::RemoveWidget(_)
                | Op::ClearWidgets) => out.push(Action::Widgets(op)),
                // The program reads every archive before the map starts.
                Op::Preload(_) | Op::Archive | Op::WaitLoads => {}
                other => out.push(Action::Unported(other)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(code: u16, words: &[u32]) -> Vec<u8> {
        let mut b = code.to_le_bytes().to_vec();
        b.extend((words.len() as u16).to_le_bytes());
        for w in words {
            b.extend(w.to_le_bytes());
        }
        b
    }

    #[test]
    fn a_world_opening_refreshes_then_moves_the_camera() {
        let mut bytes = op(0x105, &[]);
        bytes.extend(op(
            0x104,
            &[
                5000f32.to_bits(),
                (-400f32).to_bits(),
                0,
                0,
                (-2000f32).to_bits(),
                (-3000f32).to_bits(),
                90f32.to_bits(),
            ],
        ));
        bytes.extend(op(2, &[60f32.to_bits()]));
        bytes.extend(op(0, &[]));
        let script = read_script(&bytes);
        assert_eq!(script.len(), 3);
        let mut p = EventPlayer::new(0x1f8, script);
        assert_eq!(p.tick(), vec![Action::PreRefresh]);
        let mut ticks = 1;
        let mut seen = Vec::new();
        while !p.done && ticks < 1000 {
            for a in p.tick() {
                seen.push((ticks, a));
            }
            ticks += 1;
        }
        assert_eq!(seen[0], (240, Action::Refresh));
        assert_eq!(seen[1], (240, Action::Sound(REFRESH_SOUND)));
        assert!(matches!(seen[2], (330, Action::Camera { frames: 90., .. })));
        // 90 camera frames then the 60-frame wait.
        assert_eq!(ticks, 481);
    }

    #[test]
    fn widget_ops_and_voices_reach_the_program() {
        let mut bytes = op(0x101, &[]);
        bytes.extend(op(0x102, &[1, 0x895]));
        bytes.extend(op(0x103, &[1, 5, 6]));
        bytes.extend(op(0x111, &[1, 7]));
        bytes.extend(op(0x108, &[4]));
        let script = read_script(&bytes);
        assert_eq!(script[2], Op::WidgetMotion(1, vec![5, 6]));
        assert_eq!(script[3], Op::WidgetQueue(1, vec![7]));
        let mut p = EventPlayer::new(0x1f6, script);
        assert_eq!(
            p.tick(),
            vec![
                Action::Widgets(Op::Widget(1, 0x895)),
                Action::Widgets(Op::WidgetMotion(1, vec![5, 6])),
                Action::Widgets(Op::WidgetQueue(1, vec![7])),
                Action::Voice(4),
            ]
        );
        assert_eq!(archive_name(0x1f6, "us"), "libretto/us/wm502.bar");
        let mut stub = b"ORIGIN\0\x04".to_vec();
        stub.extend([0; 8]);
        stub.extend(b"voice/wm/us/wm00000ab\0\0");
        let path = origin_path(&stub).unwrap();
        assert_eq!(path, "voice/wm/us/wm00000ab");
        assert_eq!(
            voice_file(&path, "us").unwrap(),
            "voice/us/event/wm00000ab.win32.scd"
        );
        assert!(origin_path(b"BAR\x01").is_none());
    }

    #[test]
    fn messages_hold_the_script() {
        let mut bytes = op(1, &[0x1823]);
        bytes.extend(op(5, &[12f32.to_bits()]));
        let mut p = EventPlayer::new(1, read_script(&bytes));
        assert_eq!(
            p.tick(),
            vec![Action::Message {
                id: 0x1823,
                tutorial: false
            }]
        );
        assert!(p.tick().is_empty());
        p.message_closed();
        assert_eq!(
            p.tick(),
            vec![Action::Fade {
                out: true,
                white: false,
                n: 6
            }]
        );
        assert!(p.done || p.tick().is_empty());
    }
}
