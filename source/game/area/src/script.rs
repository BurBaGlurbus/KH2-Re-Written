//! Area spawn scripts: FUN_1403a2170 runs the ARD's `map`, `btl` and `evt`
//! script groups for the location's three programs through the command
//! interpreter FUN_1403a24c0.
use data::ard::{self, Command};

/// One event entry declared by op 0x0c (table `DAT_142ae6ed0`, filled by
/// FUN_1403f5dc0 and the sub-ops).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EventEntry {
    pub signal: i32,
    pub spawn_id: i32,
    /// Type-22 event name (sub-op 0) and mode.
    pub event: Option<(u32, i16)>,
    /// Jump (sub-op 1): `{kind, s1, s2, s3, s4, s5}` as FUN_1403a4bd0 reads.
    pub jump: Option<[i16; 6]>,
    pub flag: Option<i16>,
    pub e0c: Option<i16>,
    pub party: Option<i16>,
    pub party_reset: Option<i16>,
    pub preload: Vec<u32>,
    pub mode: Option<i16>,
    pub opt: bool,
}

/// FUN_1403f60d0: the entry a signal picks. Its signal and arg both equal wins at
/// once; otherwise the last with the signal where either arg is negative.
pub fn find_entry(events: &[EventEntry], signal: i32, arg: i32) -> Option<usize> {
    let mut found = None;
    for (i, e) in events.iter().enumerate() {
        if e.signal != signal {
            continue;
        }
        if e.spawn_id == arg {
            return Some(i);
        }
        if arg < 0 || e.spawn_id < 0 {
            found = Some(i);
        }
    }
    found
}

/// A registered spawn set (list `DAT_142a10010`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registered {
    pub name: u32,
    /// Op 9: registered but not activated at play start (flag 2).
    pub deferred: bool,
}

/// The state FUN_1403a2170 leaves for one area.
#[derive(Clone, Debug, PartialEq)]
pub struct AreaSetup {
    /// Object capacity `DAT_142a0f7dc` (default 10000.0).
    pub capacity: f32,
    /// Background group mask `DAT_142a0f7a8` after show/hide bits.
    pub bg_mask: u64,
    pub show_bits: u64,
    pub hide_bits: u64,
    /// `DAT_142a0f7c0`.
    pub bob_mask: u64,
    pub map_heap: u32,
    pub heap2: u32,
    pub fx_heap: [u32; 2],
    pub defeat_memory: [u32; 2],
    pub radar: u32,
    /// Op 0x13 `DAT_142a0f7d0`.
    pub camera_mode: u32,
    /// Op 0x11 `DAT_142a0f7cc` (−1 none).
    pub map_exit: u32,
    pub battle_level: u32,
    /// Op 0x10: field / battle BGM, overriding arif.
    pub bgm: Option<[u16; 2]>,
    pub spawns: Vec<Registered>,
    /// Op 0x0a: raw ARD entry indexes started as world scripts.
    pub world_scripts: Vec<u32>,
    pub events: Vec<EventEntry>,
    pub party_set: Vec<u32>,
    pub party_reset: Vec<u32>,
    /// Op 0x19: progress flags to set.
    pub set_flags: Vec<u32>,
    pub missions: Vec<(u32, String)>,
    pub load_files: Vec<String>,
    /// `DAT_142a11400` bits from ops 0x14 / 0x17.
    pub flags_11400: u32,
}

impl Default for AreaSetup {
    fn default() -> Self {
        Self {
            capacity: 10000.,
            bg_mask: !0,
            show_bits: 0,
            hide_bits: 0,
            bob_mask: 0,
            map_heap: 0,
            heap2: 0,
            fx_heap: [0; 2],
            defeat_memory: [3, 4],
            radar: 0,
            camera_mode: 0,
            map_exit: 0xffff_ffff,
            battle_level: 0,
            bgm: None,
            spawns: Vec::new(),
            world_scripts: Vec::new(),
            events: Vec::new(),
            party_set: Vec::new(),
            party_reset: Vec::new(),
            set_flags: Vec::new(),
            missions: Vec::new(),
            load_files: Vec::new(),
            flags_11400: 0,
        }
    }
}

/// Inputs to one script run.
pub struct Location {
    pub world: u8,
    pub area: u8,
    pub entrance: u8,
    /// map, btl, evt.
    pub programs: [i16; 3],
}

/// Random state `DAT_140783ca0` (u32, ×0x10dcd per draw).
#[derive(Clone, Copy, Debug, Default)]
pub struct ScriptRandom(pub u32);

/// Lottery (op 0x1f, FUN_1403e3f60) and progress tests the interpreter
/// needs from the game.
pub trait Game {
    fn lottery(&mut self, _location: &Location) -> bool {
        false
    }
    fn flag_set(&self, flag: u32) -> bool;
}

fn string(words: &[u32]) -> String {
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// FUN_1403a2170 for an ARD BAR. `entry(type, name)` returns the data of
/// the first matching BAR entry.
pub fn run(
    location: &Location,
    scripts: [Option<&[u8]>; 3],
    rng: &mut ScriptRandom,
    game: &mut impl Game,
) -> Result<AreaSetup, ard::ArdError> {
    let mut s = AreaSetup::default();
    for (script, &program) in scripts.iter().zip(&location.programs) {
        let Some(script) = script else { continue };
        if let Some(commands) = ard::program(script, program)? {
            run_commands(&commands, location, &mut s, rng, game)?;
        }
    }
    if s.map_heap == 0 {
        s.map_heap = 0x280000;
    }
    if s.heap2 == 0 {
        s.heap2 = 0x100000;
    }
    if s.fx_heap[0] == 0 {
        s.fx_heap[0] = if location.world == 12 && location.area == 7 {
            0x100000
        } else {
            0x80000
        };
    }
    if s.fx_heap[1] == 0 {
        s.fx_heap[1] = 0x20000;
    }
    // `0x32 < *param_1` compares the map program as unsigned u16.
    if location.programs[0] as u16 > 0x32 {
        s.defeat_memory = [0, 0];
    }
    s.bg_mask = (s.show_bits | s.bg_mask) & !s.hide_bits;
    Ok(s)
}

/// The command loop: stops when a command returns false (ops 3 and 0x1f
/// when they fire).
fn run_commands(
    commands: &[Command],
    location: &Location,
    s: &mut AreaSetup,
    rng: &mut ScriptRandom,
    game: &mut impl Game,
) -> Result<bool, ard::ArdError> {
    for c in commands {
        if !command(c, location, s, rng, game)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn register(s: &mut AreaSetup, name: u32, deferred: bool) {
    s.spawns.push(Registered { name, deferred });
}

/// FUN_1403a24c0.
fn command(
    c: &Command,
    location: &Location,
    s: &mut AreaSetup,
    rng: &mut ScriptRandom,
    game: &mut impl Game,
) -> Result<bool, ard::ArdError> {
    let a = &c.args;
    let arg = |i: usize| {
        a.get(i)
            .copied()
            .ok_or(ard::ArdError("script operand absent"))
    };
    let u64_arg = || -> Result<u64, ard::ArdError> { Ok(arg(0)? as u64 | (arg(1)? as u64) << 32) };
    match c.op {
        0 => {
            for &name in a {
                register(s, name, false);
            }
        }
        1 => s.bg_mask = u64_arg()?,
        2 => {
            rng.0 = rng.0.wrapping_mul(0x10dcd);
            let argc = a.len() as u64;
            if argc == 0 {
                return Err(ard::ArdError("op 2 without names"));
            }
            // (argc − 1 + 2³²) / argc, as the exe computes it.
            let index = (rng.0 as u64 / 0x1_0000_0000_u64.div_ceil(argc)) as usize;
            register(s, arg(index)?, false);
        }
        3 => {
            rng.0 = rng.0.wrapping_mul(0x10dcd);
            if rng.0 / 0x28f5c29 < arg(0)? {
                register(s, arg(1)?, false);
                return Ok(false);
            }
        }
        4 => s.capacity = (arg(0)? as u16 as i16) as f32,
        5 => s.map_heap = arg(0)?,
        6 => s.defeat_memory[0] = arg(0)?,
        7 => s.defeat_memory[1] = arg(0)?,
        9 => register(s, arg(0)?, true),
        0xa => s.world_scripts.push(arg(0)?),
        0xb => s.bob_mask = u64_arg()?,
        0xc => s.events.push(event_entry(a)?),
        0xe => s.radar = arg(0)?,
        0xf => s.party_set.push(arg(0)?),
        0x10 => {
            let w = arg(0)?;
            s.bgm = Some([w as u16, (w >> 16) as u16]);
        }
        0x11 => s.map_exit = arg(0)?,
        0x12 => s.heap2 = arg(0)?,
        0x13 => s.camera_mode = arg(0)?,
        0x14 => s.flags_11400 |= 4,
        0x15 => s.missions.push((arg(0)?, string(&a[1..]))),
        0x16 => s.load_files.push(string(a)),
        0x17 => s.flags_11400 |= 0x10,
        0x18 => s.fx_heap = [arg(0)?, arg(1)?],
        // The loop bound is re-read from the high half of the word just
        // consumed (decompiled FUN_1403a24c0 ops 0x19/0x1a/0x1b), so only
        // the first word applies unless that half is non-zero; every TT
        // script passes one word.
        0x19 => {
            let mut i = 0;
            loop {
                let f = arg(i)?;
                if !game.flag_set(f) {
                    s.set_flags.push(f);
                }
                i += 1;
                if i as i32 >= (f >> 16) as i16 as i32 {
                    break;
                }
            }
        }
        0x1a | 0x1b => {
            let mut i = 0;
            loop {
                let w = arg(i)?;
                let bit = 1u64 << (w & 63);
                if c.op == 0x1a {
                    s.show_bits |= bit;
                } else {
                    s.hide_bits |= bit;
                }
                i += 1;
                if i as i32 >= (w >> 16) as i16 as i32 {
                    break;
                }
            }
        }
        0x1c => {
            if arg(0)? == location.entrance as u32 {
                return run_commands(&c.nested()?, location, s, rng, game);
            }
        }
        0x1d => s.party_reset.extend(a),
        0x1e => s.battle_level = arg(0)?,
        0x1f if game.lottery(location) => {
            register(s, arg(0)?, false);
            return Ok(false);
        }
        // 8 and 0xd are not handled by the exe.
        _ => {}
    }
    Ok(true)
}

/// Op 0x0c: `{i16 signal, i16 spawnId}` then sub-ops `{u16 sub, i16 v}`.
fn event_entry(a: &[u32]) -> Result<EventEntry, ard::ArdError> {
    let first = *a.first().ok_or(ard::ArdError("event entry key absent"))?;
    let mut e = EventEntry {
        signal: first as u16 as i16 as i32,
        spawn_id: (first >> 16) as u16 as i16 as i32,
        ..Default::default()
    };
    let half = |w: u32| [w as u16 as i16, (w >> 16) as u16 as i16];
    let mut i = 1;
    while i < a.len() {
        let [sub, v] = half(a[i]);
        match sub as u16 {
            0 => {
                e.event = Some((*a.get(i + 1).unwrap_or(&0), v));
                i += 2;
                continue;
            }
            1 => {
                let w: Vec<i16> = a[i + 1..(i + 4).min(a.len())]
                    .iter()
                    .flat_map(|&w| half(w))
                    .collect();
                if w.len() == 6 {
                    e.jump = Some(w.try_into().unwrap());
                }
                i += 4;
                continue;
            }
            2 => e.flag = Some(v),
            3 => e.e0c = Some(v),
            4 => e.party = Some(v),
            5 => e.party_reset = Some(v),
            6 => {
                let n = v.max(0) as usize;
                e.preload = a[i + 1..(i + 1 + n).min(a.len())].to_vec();
                i += n + 1;
                continue;
            }
            7 => e.mode = Some(v),
            8 => e.opt = true,
            _ => {}
        }
        i += 1;
    }
    Ok(e)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_signal_picks_the_equal_arg_first_then_the_last_open_one() {
        let e = |signal, spawn_id| EventEntry {
            signal,
            spawn_id,
            event: None,
            jump: None,
            flag: None,
            e0c: None,
            party: None,
            party_reset: None,
            preload: Vec::new(),
            mode: None,
            opt: false,
        };
        let list = [e(1, 58), e(0, -1), e(0, -1), e(1, 7), e(2, 3)];
        assert_eq!(find_entry(&list, 1, 7), Some(3));
        assert_eq!(find_entry(&list, 0, 0), Some(2));
        assert_eq!(find_entry(&list, 1, 9), None);
        assert_eq!(find_entry(&list, 1, -1), Some(3));
        assert_eq!(find_entry(&list, 5, 0), None);
    }

    use super::*;
    use data::ard::tag;

    struct NoGame;
    impl Game for NoGame {
        fn flag_set(&self, _: u32) -> bool {
            false
        }
    }

    type Block = (i16, Vec<(u16, Vec<u32>)>);

    fn script(blocks: &[Block]) -> Vec<u8> {
        let mut out = Vec::new();
        for (id, cmds) in blocks {
            let mut body = Vec::new();
            for (op, args) in cmds {
                body.extend(op.to_le_bytes());
                body.extend((args.len() as i16).to_le_bytes());
                for w in args {
                    body.extend(w.to_le_bytes());
                }
            }
            out.extend(id.to_le_bytes());
            out.extend(((body.len() + 4) as i16).to_le_bytes());
            out.extend(body);
        }
        out.extend((-1i16).to_le_bytes());
        out.extend([0, 0]);
        out
    }

    #[test]
    fn map_program_registers_spawns_and_mask() {
        let map = script(&[(
            4,
            vec![
                (0xe, vec![1]),
                (1, vec![0x27, 0]),
                (0, vec![tag("m_00"), tag("m_10")]),
                (0x1b, vec![1]),
                (0x10, vec![52 | 53 << 16]),
            ],
        )]);
        let evt = script(&[(
            18,
            vec![(
                0xc,
                vec![
                    1 | 7 << 16,
                    2 << 16,
                    tag("704"),
                    1,
                    1 | 2 << 16,
                    8 | 1 << 16,
                    0,
                ],
            )],
        )]);
        let loc = Location {
            world: 2,
            area: 8,
            entrance: 0,
            programs: [4, 0, 18],
        };
        let s = run(
            &loc,
            [Some(&map), None, Some(&evt)],
            &mut ScriptRandom::default(),
            &mut NoGame,
        )
        .unwrap();
        assert_eq!(s.radar, 1);
        assert_eq!(s.bg_mask, 0x25);
        assert_eq!(s.bgm, Some([52, 53]));
        assert_eq!(
            s.spawns.iter().map(|r| r.name).collect::<Vec<_>>(),
            vec![tag("m_00"), tag("m_10")]
        );
        let e = &s.events[0];
        assert_eq!((e.signal, e.spawn_id), (1, 7));
        assert_eq!(e.event, Some((tag("704"), 2)));
        assert_eq!(e.jump, Some([1, 2, 8, 1, 0, 0]));
        assert_eq!(s.defeat_memory, [3, 4]);
    }

    #[test]
    fn spawn_chance_stops_the_program() {
        let map = script(&[(0, vec![(3, vec![100, tag("a")]), (0, vec![tag("b")])])]);
        let loc = Location {
            world: 2,
            area: 1,
            entrance: 0,
            programs: [0, 0, 0],
        };
        let mut rng = ScriptRandom(1);
        let s = run(&loc, [Some(&map), None, None], &mut rng, &mut NoGame).unwrap();
        assert_eq!(rng.0, 0x10dcd);
        assert_eq!(s.spawns.len(), 1);
        assert_eq!(s.spawns[0].name, tag("a"));
    }

    #[test]
    fn if_entrance_runs_nested_commands() {
        let nested = [0u16.to_le_bytes(), 1i16.to_le_bytes()].concat();
        let mut words = vec![2u32, u32::from_le_bytes(nested.try_into().unwrap())];
        words.push(tag("e_50"));
        let map = script(&[(0, vec![(0x1c, words)])]);
        for (entrance, n) in [(2, 1), (0, 0)] {
            let loc = Location {
                world: 2,
                area: 8,
                entrance,
                programs: [0, 0, 0],
            };
            let s = run(
                &loc,
                [Some(&map), None, None],
                &mut ScriptRandom::default(),
                &mut NoGame,
            )
            .unwrap();
            assert_eq!(s.spawns.len(), n);
        }
    }
}
