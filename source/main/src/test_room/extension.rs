//! What the room offers a crate built on it: other drivers for its Soras,
//! extra Soras, extra state, and places in its schedule. A room with no
//! extension runs the single-player game; none of this is reached then but
//! the defaults below.
//!
//! Plain function pointers, because every hook takes the room's `Game` and a
//! hook kept inside it could not. The table is set once, before the room is
//! built.

use super::*;
use std::any::Any;
use std::error::Error;
use std::sync::OnceLock;
use visual::pause_menu::{DuelPages, Entry};

/// State an extension keeps on the room or on one of its Soras. A room with
/// no extension holds `()`.
pub type Extension = Box<dyn Any + Send + Sync>;

pub fn no_extension() -> Extension {
    Box::new(())
}

/// Two buttons beyond the KH2 pad's, for a driver that uses them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExtraButtons {
    /// Held.
    pub shotlock: bool,
    /// Pressed this tick.
    pub flow: bool,
}

/// What a driver wants of the room Sora's tick, in place of the pad's. The
/// default leaves the pad alone.
#[derive(Clone, Copy, Debug, Default)]
pub struct SoraDrive {
    /// Takes the stick and Attack, and leaves Jump and Guard unpressed.
    pub bench: Option<rival::RivalPad>,
    /// The bench's command is the only one this tick (none of the menu's).
    pub commands_only: bool,
    /// Takes the stick and every button, and may add a spell.
    pub script: Option<rival::RivalPad>,
    pub buttons: ExtraButtons,
}

/// The follow camera this tick, for a camera drawn over it.
#[derive(Clone, Copy, Debug)]
pub struct CameraFrame {
    /// The right stick: turn (right +) and height (down +).
    pub turn: f32,
    pub height: f32,
    pub eye: Vec3,
    pub look: Vec3,
    pub fov: f32,
}

/// A top-level camp entry: one of the camp's own (by id: its label and help
/// are the game's) or the screen's.
#[derive(Clone, Debug)]
pub enum CampItem {
    Own(u8),
    New(Entry),
}

/// What a screen asks the camp for.
#[derive(Clone, Debug)]
pub struct CampRequest {
    pub entries: Vec<CampItem>,
    /// Opens on the Config screen (the pages), not the top level.
    pub on_config: bool,
    pub pages: DuelPages,
}

/// A row an extension adds to the debug menu's Display page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DebugRow {
    Section(&'static str),
    Row {
        label: &'static str,
        value: Option<String>,
        /// Handed back to `debug_action`.
        id: u8,
        enabled: bool,
        /// Left and Right step it.
        steps: bool,
        help: &'static str,
    },
}

/// What a debug row did.
#[derive(Clone, Debug, Default)]
pub struct DebugOutcome {
    pub message: Option<String>,
    pub close: bool,
}

/// The room's hooks. `NONE` is the single-player game.
#[derive(Clone, Copy)]
pub struct RoomHooks {
    /// A new Fighter's extension state.
    pub new_fighter: fn() -> Extension,
    /// Neither button nor stick reaches the room Sora this tick.
    pub frozen: fn(&Game) -> bool,
    /// Who drives the room Sora this tick.
    pub drive: fn(&mut Game, u64) -> SoraDrive,
    /// Another controller runs this Sora, so the unattended run's own spells
    /// and items stay out of it.
    pub layer_on: fn(&Game) -> bool,
    /// The room Sora casts.
    pub cast: fn(&mut Game, u8, u64),
    /// A replica's input this tick, its command chosen from the menus.
    pub replica_input: fn(&mut Game, &Input, rival::Command, ExtraButtons, bool, u64),
    /// Runs the room Sora's tick in place of `Player::tick`.
    pub sora_step: fn(&mut Game, &Input, ExtraButtons, u64) -> Option<player::Step>,
    /// After the spells' tick, before the shots join the hit volumes.
    pub after_magic: fn(&mut Game),
    /// A camera over the follow camera, from the fixed tick.
    pub camera_tick: fn(&mut Game, &CameraFrame) -> Option<boss::OverlayView>,
    /// A camera over that one, each drawn frame.
    pub camera_view: fn(&Game, f32) -> Option<boss::OverlayView>,
    /// Something else draws the room's Soras this frame.
    pub draws_soras: fn(&Game) -> bool,
    /// A Sora model's turn.
    pub sora_turn: fn(&fighter::Fighter) -> Quat,
    /// A bullet its own layer steers.
    pub steers_bullet: fn(u64) -> bool,
    /// A bullet's effect style.
    pub bullet_style: fn(u64) -> drive_fx::Style,
    /// A looping voice the room's cleanup leaves alone.
    pub keeps_loop: fn(u32) -> bool,
    /// The census line's extension part.
    pub census: fn(&Game) -> serde_json::Value,
    pub debug_rows: fn(&Game) -> Vec<DebugRow>,
    /// A debug row was chosen, or stepped.
    pub debug_action: fn(&mut Game, u8, i32) -> DebugOutcome,
}

impl RoomHooks {
    pub const NONE: RoomHooks = RoomHooks {
        new_fighter: no_extension,
        frozen: |_| false,
        drive: |_, _| SoraDrive {
            bench: None,
            commands_only: false,
            script: None,
            buttons: ExtraButtons {
                shotlock: false,
                flow: false,
            },
        },
        layer_on: |_| false,
        cast: |game, spell, tick| {
            magic_room::request(game, spell, tick);
        },
        replica_input: |_, _, _, _, _, _| {},
        sora_step: |_, _, _, _| None,
        after_magic: |_| {},
        camera_tick: |_, _| None,
        camera_view: |_, _| None,
        draws_soras: |_| false,
        sora_turn: |f| Quat::from_rotation_y(f.player.yaw),
        steers_bullet: |_| false,
        bullet_style: |_| drive_fx::Style::default(),
        keeps_loop: |_| false,
        census: |_| serde_json::Value::Null,
        debug_rows: |_| Vec::new(),
        debug_action: |_, _, _| DebugOutcome {
            message: None,
            close: false,
        },
    };
}

static HOOKS: OnceLock<RoomHooks> = OnceLock::new();

/// The room's hooks.
pub fn hooks() -> &'static RoomHooks {
    HOOKS.get().unwrap_or(&RoomHooks::NONE)
}

/// Sets the hooks, once per process, before any room is built.
pub fn install(hooks: RoomHooks) {
    let _ = HOOKS.set(hooks);
}

/// A crate built on the room: what it does while the room starts.
pub trait RoomExtension {
    fn hooks(&self) -> RoomHooks;
    /// The window's title in place of the room's.
    fn window_title(&self) -> Option<String> {
        None
    }
    /// Takes its own options out of the room's, and returns the rest.
    fn take_options(&mut self, options: &[PathBuf]) -> Result<Vec<PathBuf>, Box<dyn Error>>;
    /// An unattended run's captures and last tick, when it sets them.
    fn schedule(&self) -> Option<(Vec<u64>, u64)> {
        None
    }
    /// The room's `Game` is ready; the camera comes next.
    fn setup_game(&mut self, game: &mut Game, args: &[PathBuf]) -> Result<(), Box<dyn Error>>;
    /// Adds its resources and systems (`RoomSet` names the room's places).
    fn build_app(&mut self, app: &mut App, args: &[PathBuf]);
}

/// The room's systems an extension can order its own around.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RoomSet {
    ReadPad,
    CampTick,
    MissionTick,
    ReactionCommand,
    Simulate,
    SummonTick,
    EffectsTick,
    DriveFxTick,
    HudTick,
    PlaySounds,
    MusicTick,
    KeepWeaponPose,
    Present,
    ThrownKeyblade,
    PerfPresent,
    Finish,
}
