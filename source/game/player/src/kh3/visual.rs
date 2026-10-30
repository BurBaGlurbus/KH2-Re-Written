//! Which of KH3's animations Sora shows: the rows of `p_ex001_AnimSet` by the
//! layer's state, and a clock for the presenter. Row names and which state
//! plays which are from StrawHatPro's reading of KH3 (`crates/kh-core/src/
//! states/*`, MIT); the clips themselves are the player's own.

use super::{Jump, Kh3, Knock, Mode};
use crate::{Action, Player};

/// What the presenter plays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Visual {
    pub row: &'static str,
    /// Bumps on every new play, also of the same row (a chain's next swing).
    pub serial: u32,
    /// Seconds into the row; a looping row wraps at its own length.
    pub time: f32,
    /// A blend space's axes.
    pub at: [f32; 2],
    /// The blend in, seconds; below zero the asset's `m_InterpolationTime`.
    pub blend: f32,
    pub looped: bool,
}

impl Default for Visual {
    fn default() -> Self {
        Visual {
            row: "IdleB",
            serial: 0,
            time: 0.,
            at: [0.; 2],
            blend: -1.,
            looped: true,
        }
    }
}

const TICK: f32 = 1. / 60.;

/// A choice of row this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Choice {
    pub row: &'static str,
    pub looped: bool,
    pub blend: f32,
    pub at: [f32; 2],
}

const fn once(row: &'static str) -> Choice {
    Choice {
        row,
        looped: false,
        blend: -1.,
        at: [0.; 2],
    }
}

const fn looping(row: &'static str) -> Choice {
    Choice {
        row,
        looped: true,
        blend: -1.,
        at: [0.; 2],
    }
}

impl Visual {
    /// One tick: a new row starts at 0 (with a new serial), the same one runs on.
    pub fn advance(&mut self, c: Choice) {
        if c.row != self.row {
            self.row = c.row;
            self.serial = self.serial.wrapping_add(1);
            self.time = 0.;
            self.blend = c.blend;
        } else {
            self.time += TICK;
        }
        self.looped = c.looped;
        self.at = c.at;
    }

    /// Plays a row from its start, even the same one again.
    pub fn restart(&mut self, row: &'static str, blend: f32, looped: bool) {
        self.row = row;
        self.serial = self.serial.wrapping_add(1);
        self.time = 0.;
        self.blend = blend;
        self.looped = looped;
    }
}

/// KH3's attack ids (the selector's, `combo::id`) to their rows
/// (`tools/sora_combat_sheet.js` in the reference).
pub fn attack_row(id: u8) -> Option<&'static str> {
    Some(match id {
        0x01 => "1301",
        0x02 => "1302",
        0x03 => "1303",
        0x04 => "1304",
        0x05 => "1305",
        0x06 => "1306",
        0x07 => "1307",
        0x08 | 0x17 => "1308",
        0x09 => "M_1359",
        0x0a => "M_1360",
        0x0b => "M_1361",
        0x0c => "M_1362",
        0x0d | 0x0e => "1318",
        0x0f => "1310",
        0x10 => "1311",
        0x11 => "1312",
        0x12 => "1313",
        0x13 => "1314",
        0x14 => "1315",
        0x15 => "1316",
        0x16 => "1317",
        0x18 => "M_1363",
        0x19 => "M_1364",
        0x1a => "M_1365",
        0x1b => "M_1366",
        0x1c => "1319",
        0x1d => "1320",
        0x1f => "1309",
        0x20 => "M_1367",
        0x21 => "RevengeSlash",
        0x22 => "ChaseAttack",
        0x23 => "RevengeImpact",
        0x24 => "RevengeDive",
        0x27 => "BlowCounter",
        0x28 => "HookUpper",
        0x29 => "BackRaid",
        0x2a => "BackRush",
        0x2b => "WarpSlash",
        0x2f => "M_1143",
        0x31 => "M_1144",
        0x32 => "M_1147",
        0x33 => "M_1146",
        0x34 => "M_1145",
        0x35 => "DiveFallAttack",
        0x36 => "1181",
        0x37 => "1182",
        0x38 => "1183",
        0x39 => "1184",
        _ => return None,
    })
}

/// The row for this tick's state.
pub fn choose(k: &Kh3, player: &Player) -> Choice {
    if let Some(a) = &k.attack {
        return once(a.row);
    }
    if let Some(c) = &k.cast {
        return Choice {
            blend: 0.,
            ..once(c.row)
        };
    }
    if k.aim.is_some() {
        return looping("IdleLockon");
    }
    let rising = k.velocity[1] > 0.;
    match k.mode {
        Mode::Ground => match player.action {
            Action::Walk => looping("WalkLoopB"),
            Action::Run => Choice {
                at: [k.turning.clamp(-10., 10.), 0.],
                ..looping("RunLoopB")
            },
            _ => looping("IdleB"),
        },
        Mode::Takeoff { jump, .. } => once(match jump {
            Jump::High => "JumpHighStart",
            Jump::Double => "JumpDoubleFlightStart",
            Jump::Super => "JumpSuperStart",
            _ => "JumpNormalStart",
        }),
        Mode::Air { jump, .. } => {
            if !rising {
                looping("JumpNormalDownLoop")
            } else {
                looping(match jump {
                    Jump::High => "JumpHighRiseLoop",
                    Jump::Double => "JumpDoubleFlightRiseLoop",
                    Jump::Super => "JumpSuperRiseLoop",
                    _ => "JumpNormalRiseLoop",
                })
            }
        }
        Mode::Land { .. } => once("LightLand"),
        Mode::Roll { .. } => once("DodgeRoll"),
        Mode::Slide { .. } => once("AirSlide"),
        Mode::Glide { .. } => Choice {
            at: [k.turning.clamp(-180., 180.), 0.],
            ..looping("GlideLoop")
        },
        Mode::Superslide { .. } => Choice {
            blend: 0.,
            ..looping("SuperSlideLoop")
        },
        Mode::SlideEnd { .. } => Choice {
            blend: 0.05,
            ..once("SlideEnd")
        },
        Mode::WallRun { up: true, .. } => looping("WallRunLoopF"),
        Mode::WallRun { side, .. } => looping(if side >= 0. {
            "WallRunLoopR"
        } else {
            "WallRunLoopL"
        }),
        Mode::WallKick { .. } => once("WallKickF"),
        Mode::Pole { .. } => looping("PoleTurnLoop"),
        Mode::Airstep { .. } => once("AthleticSlide"),
        Mode::HopBack { .. } => once("AthleticSlideEnd"),
        Mode::Volley { tick, ragnarok, .. } => {
            // Charge, fire and end, by the clips' lengths (shoot_flow.rs).
            let (charge, fire, charge_row, fire_row) = if ragnarok {
                (2.333, 1.0, "M_1481", "M_1482")
            } else {
                (2.5, 0.7, "M_1473", "M_1474")
            };
            let t = tick as f32 * TICK;
            once(if t < charge {
                charge_row
            } else if t < charge + fire {
                fire_row
            } else {
                "ShootFlowEnd"
            })
        }
        Mode::Syphon { .. } => once("M_1483"),
        Mode::Flow { .. } => once("1181"),
        Mode::Launch { .. } => once("1319"),
        Mode::Chase { .. } => once("ChaseAttack"),
        Mode::Risk { warp: false, .. } => once("UnderSlide"),
        Mode::Risk { .. } => once("ShootFlowStart"),
        Mode::Recover { .. } => Choice {
            blend: 0.,
            ..once("AirRecovery")
        },
        Mode::Down { back, .. } => once(if back { "0216" } else { "0213" }),
        Mode::Hurt {
            knock,
            air,
            back,
            high,
            ..
        } => hurt_row(knock, air, back, high, rising),
        Mode::Kh2 | Mode::Attack | Mode::Cast => kh2_row(player),
    }
}

fn hurt_row(knock: Knock, air: bool, back: bool, _high: bool, rising: bool) -> Choice {
    match knock {
        Knock::Small => once(match (air, back) {
            (false, false) => "0201",
            (false, true) => "0202",
            (true, false) => "0207",
            (true, true) => "0208",
        }),
        Knock::Blow | Knock::SideSpin | Knock::Parabola => {
            looping(if back { "0215" } else { "0212" })
        }
        Knock::Beat => looping("M_0230"),
        Knock::Toss | Knock::Launch | Knock::VerticalRoll => {
            looping(if rising { "M_0228" } else { "M_0237" })
        }
    }
}

/// While the KH2 controller has him (casts, items, guards, a death).
fn kh2_row(player: &Player) -> Choice {
    match player.action {
        Action::Guard => once("Guard"),
        Action::GuardRecoil => once("GuardHit"),
        Action::Cast(_) => once("MagicFire"),
        Action::AirCast(_) => once("MagicAirFire"),
        Action::Item => once("ItemUse"),
        Action::Hurt(_) => once("0201"),
        Action::Fall | Action::Jump | Action::HighJump => looping("JumpNormalDownLoop"),
        _ if player.dead() => once("0213"),
        _ => looping("IdleB"),
    }
}
