//! The test room's debug menu (F3, or both stick buttons on a controller):
//! give items, restore and level Sora, change Drive form, control the enemy,
//! preview enemy animations and warp between areas. A development aid, not
//! part of the original game: a plain box and list drawn on the HUD canvas.
//! The field is paused while it is open.
//!
//! Lists with many rows (items, enemies, motions, rooms) take typed letters
//! as a search: only rows containing them stay. The enemy animation preview
//! has tabs (Tab / Shift+Tab, L1 / R1, or Left / Right on a row with no
//! value to change) and keeps playing behind the menu.
use super::*;
use bevy::input::{
    ButtonState,
    keyboard::{Key, KeyboardInput},
};
use player::forms;
use std::sync::OnceLock;

/// Effects the menu leaves switched on.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cheats {
    /// Sora takes no damage.
    pub invincible: bool,
    /// The enemy stands still.
    pub enemy_frozen: bool,
}

/// One white texel, for the box and the highlight bar.
pub const WHITE: [u8; 4] = [255; 4];

/// Lines shown at once (header lines and rows together), and the layout in
/// HUD units (512 × 416).
const LINES: usize = 15;
const ROW_HEIGHT: f32 = 19.;
const TEXT_SIZE: f32 = 14.;
const BOX: [f32; 4] = [96., 36., 416., 400.];
const MARGIN: f32 = 14.;
/// Held arrows repeat after 18 frames, then every 4.
const REPEAT_DELAY: u32 = 18;
const REPEAT_EVERY: u32 = 4;
/// Spell items hold a level; other stock counts stop at 99.
const MAX_SPELL: u8 = 3;
const MAX_STOCK: u8 = 99;
/// Frames a result line stays in the footer.
const NOTICE_FRAMES: u32 = 120;
/// Longest search the menu keeps.
const MAX_SEARCH: usize = 24;
/// A second of preview playback is this many field ticks.
const TICKS_PER_SECOND: f32 = 60.;

const FORMS: [(u8, &str); 6] = [
    (forms::VALOR, "Valor Form"),
    (forms::WISDOM, "Wisdom Form"),
    (forms::LIMIT, "Limit Form"),
    (forms::MASTER, "Master Form"),
    (forms::FINAL, "Final Form"),
    (forms::ANTI, "Antiform"),
];

/// The tabs of the enemy animation preview.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PreviewTab {
    /// Play, pause, step, jump to a frame, loop and speed.
    #[default]
    Playback,
    /// Every motion in the enemy's motion set, by name.
    Motions,
    /// The enemy script's named actions and the motions they request.
    Actions,
    /// Skeleton, model and collision overlays.
    View,
    /// AI and combat, defeat, respawn and removal.
    Enemy,
}

impl PreviewTab {
    const ALL: [Self; 5] = [
        Self::Playback,
        Self::Motions,
        Self::Actions,
        Self::View,
        Self::Enemy,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Playback => "Playback",
            Self::Motions => "Motions",
            Self::Actions => "Actions",
            Self::View => "View",
            Self::Enemy => "Enemy",
        }
    }

    /// The tab `by` places along, wrapping around.
    fn shifted(self, by: i32) -> Self {
        let at = Self::ALL.iter().position(|&t| t == self).unwrap_or(0) as i32;
        Self::ALL[(at + by).rem_euclid(Self::ALL.len() as i32) as usize]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Page {
    #[default]
    Main,
    Items,
    Forms,
    Growth,
    Enemies,
    Roster(enemy_catalog::Category),
    /// The summoned enemies.
    Actors,
    /// The animation preview of the selected summoned enemy.
    Preview(PreviewTab),
    /// The worlds with installed rooms.
    Warp,
    /// One world's rooms.
    Rooms(u8),
}

impl Page {
    /// Pages whose rows can be narrowed by typing.
    fn searchable(self) -> bool {
        matches!(
            self,
            Self::Items
                | Self::Roster(_)
                | Self::Actors
                | Self::Preview(PreviewTab::Motions | PreviewTab::Actions)
                | Self::Warp
                | Self::Rooms(_)
        )
    }

    /// Lines above the rows: for the preview the tab strip and two lines
    /// about the playing motion, and the search line on searchable pages.
    fn header_lines(self) -> usize {
        let preview = if matches!(self, Self::Preview(_)) {
            3
        } else {
            0
        };
        preview + usize::from(self.searchable())
    }

    fn visible_rows(self) -> usize {
        LINES - self.header_lines()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Page(Page),
    /// A row that only shows a value.
    Info,
    Restore,
    KnockOut,
    LevelUp,
    BonusLevel,
    ToggleInvincible,
    ToggleScan,
    DefeatBattleTarget,
    RespawnBattleTarget,
    ToggleFreezeBattleTarget,
    FillItems,
    Item(u16),
    Form(u8),
    Growth(usize),
    Summon(usize),
    SelectActor(u64),
    /// Plays the motion at this index in the enemy's motion list.
    PlayMotion(usize),
    EnableBattle,
    DefeatSummoned,
    RespawnSummoned,
    PlayPause,
    /// One whole frame forward (`true`) or back.
    StepFrame(bool),
    /// Left and Right move the paused frame.
    GoToFrame,
    /// Pauses this many percent of the way through the motion.
    JumpToPercent(u8),
    Replay,
    ToggleLoop,
    PlaybackSpeed,
    ToggleRootMotion,
    ToggleSkeleton,
    ToggleHideModel,
    ToggleCollisionOverlay,
    RemoveActor,
    ClearActors,
    Warp(u8, u8),
    /// The current room becomes its world's launcher start.
    SaveStart,
    Close,
}

impl Action {
    /// Whether Left and Right change this row's value.
    fn adjustable(self) -> bool {
        matches!(
            self,
            Self::Item(_) | Self::Growth(_) | Self::PlaybackSpeed | Self::GoToFrame
        )
    }
}

struct Row {
    label: Vec<u8>,
    value: Vec<u8>,
    action: Action,
    enabled: bool,
}

/// What the HUD draws while the menu is open.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    heading: Vec<u8>,
    /// The preview's tabs, and whether each is the open one.
    tabs: Vec<(Vec<u8>, bool)>,
    /// Lines about the playing motion, above the rows.
    info: Vec<Vec<u8>>,
    /// The search line, on pages that take typed letters.
    search: Option<Vec<u8>>,
    /// Label, value (drawn on the right) and whether the row can be used.
    rows: Vec<(Vec<u8>, Vec<u8>, bool)>,
    selected: Option<usize>,
    footer: Vec<u8>,
}

#[derive(Resource, Default)]
pub struct DebugMenu {
    pub open: bool,
    page: Page,
    cursor: usize,
    top: usize,
    /// The main page's cursor, restored when a sub-page closes.
    main_cursor: usize,
    /// The arrow being held and for how many frames.
    held: Option<(Dir, u32)>,
    /// The current world's saved launcher start (read on opening Warp).
    saved: Option<(u8, u8)>,
    notice: Option<(String, u32)>,
    /// Letters typed on a searchable page; only rows containing them stay.
    search: String,
    pub view: View,
}

impl DebugMenu {
    pub fn show_enemy_actors(&mut self) {
        self.open = true;
        self.go_to(Page::Actors, 0);
    }

    /// Opens the animation preview on tab number `tab` with `search`
    /// typed (for the unattended tour).
    pub fn show_preview(&mut self, tab: usize, search: &str) {
        self.open = true;
        self.go_to(Page::Preview(PreviewTab::ALL[tab % 5]), 0);
        self.search = search.into();
    }

    /// Opens `page` with the cursor on row `cursor` and no search.
    fn go_to(&mut self, page: Page, cursor: usize) {
        self.page = page;
        self.cursor = cursor;
        self.top = 0;
        self.search.clear();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Default)]
struct Input {
    toggle: bool,
    confirm: bool,
    back: bool,
    /// Backspace alone (it also counts as `back`).
    erase: bool,
    /// Tabs to move this frame: Tab or R1 forward, Shift+Tab or L1 back.
    tab: i32,
    /// This frame's arrow step (a press or a repeat).
    step: Option<Dir>,
}

fn text(s: &str) -> Vec<u8> {
    pause_menu::encode_text(s)
}

/// The system font's codes back into lower case ASCII, for searching.
fn plain(bytes: &[u8]) -> String {
    static TABLE: OnceLock<[char; 256]> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut table = ['\u{fffd}'; 256];
        for c in (0x20u8..0x7f).map(char::from) {
            let code = pause_menu::encode_text(&c.to_string())[0] as usize;
            // Characters the font lacks encode as a space.
            if (code != 0x01 || c == ' ') && table[code] == '\u{fffd}' {
                table[code] = c.to_ascii_lowercase();
            }
        }
        table
    });
    bytes.iter().map(|&b| table[b as usize]).collect()
}

/// Whether typing `c` adds to the search (letters the menu font can show).
fn searchable_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "-.:/'!?%,".contains(c)
}

fn read_input(
    keys: &ButtonInput<KeyCode>,
    pads: &Query<(&Gamepad, Option<&Name>)>,
    held: &mut Option<(Dir, u32)>,
) -> Input {
    let pad_pressed = |b: GamepadButton| pads.iter().any(|(p, _)| p.pressed(b));
    let pad_just = |b: GamepadButton| pads.iter().any(|(p, _)| p.just_pressed(b));
    let sticks = pad_pressed(GamepadButton::LeftThumb)
        && pad_pressed(GamepadButton::RightThumb)
        && (pad_just(GamepadButton::LeftThumb) || pad_just(GamepadButton::RightThumb));
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let tab = if pad_just(GamepadButton::RightTrigger) {
        1
    } else if pad_just(GamepadButton::LeftTrigger) {
        -1
    } else if keys.just_pressed(KeyCode::Tab) {
        if shift { -1 } else { 1 }
    } else {
        0
    };
    let mut input = Input {
        toggle: keys.just_pressed(KeyCode::F3) || sticks,
        confirm: keys.just_pressed(KeyCode::Enter)
            || keys.just_pressed(KeyCode::NumpadEnter)
            || keys.just_pressed(KeyCode::Space)
            || pad_just(GamepadButton::South),
        back: keys.just_pressed(KeyCode::Escape)
            || keys.just_pressed(KeyCode::Backspace)
            || pad_just(GamepadButton::East),
        erase: keys.just_pressed(KeyCode::Backspace),
        tab,
        step: None,
    };
    let down = |d: Dir| {
        let (key, button) = match d {
            Dir::Up => (KeyCode::ArrowUp, GamepadButton::DPadUp),
            Dir::Down => (KeyCode::ArrowDown, GamepadButton::DPadDown),
            Dir::Left => (KeyCode::ArrowLeft, GamepadButton::DPadLeft),
            Dir::Right => (KeyCode::ArrowRight, GamepadButton::DPadRight),
        };
        keys.pressed(key) || pad_pressed(button)
    };
    let current = [Dir::Up, Dir::Down, Dir::Left, Dir::Right]
        .into_iter()
        .find(|&d| down(d));
    *held = match (current, *held) {
        (Some(d), Some((h, frames))) if d == h => {
            let frames = frames + 1;
            if frames >= REPEAT_DELAY && (frames - REPEAT_DELAY).is_multiple_of(REPEAT_EVERY) {
                input.step = Some(d);
            }
            Some((d, frames))
        }
        (Some(d), _) => {
            input.step = Some(d);
            Some((d, 0))
        }
        (None, _) => None,
    };
    input
}

/// The items the Items page lists: what the camp's All tab shows, plus the
/// spell items, with their names (empty names are skipped).
fn item_list(camp: Option<&CampRes>, game: &Game) -> Vec<(u16, Vec<u8>)> {
    if let Some(c) = camp {
        return c
            .source
            .items
            .table
            .iter()
            .filter(|r| matches!(r.kind, 0..=17 | 20) || ELEMENT_ITEMS.contains(&r.id))
            .filter(|r| r.name.iter().any(|&b| b >= 0x20))
            .map(|r| (r.id, r.name.clone()))
            .collect();
    }
    game.items
        .as_ref()
        .map(|it| {
            it.stock
                .keys()
                .map(|&id| (id, text(&format!("Item {id}"))))
                .collect()
        })
        .unwrap_or_default()
}

fn stock_of(game: &Game, camp: Option<&CampRes>, id: u16) -> u8 {
    game.items
        .as_ref()
        .map(|it| &it.stock)
        .or(camp.map(|c| &c.stock))
        .and_then(|s| s.get(&id))
        .copied()
        .unwrap_or(0)
}

fn cap_of(id: u16) -> u8 {
    if ELEMENT_ITEMS.contains(&id) {
        MAX_SPELL
    } else {
        MAX_STOCK
    }
}

/// Sets a stock count in the battle items and the camp (the camp shows the
/// battle's stock while one exists), and a spell's level.
fn set_stock(game: &mut Game, camp: Option<&mut CampRes>, id: u16, count: u8) {
    let count = count.min(cap_of(id));
    let set = |stock: &mut std::collections::BTreeMap<u16, u8>| {
        if count == 0 {
            stock.remove(&id);
        } else {
            stock.insert(id, count);
        }
    };
    if let Some(items) = game.items.as_mut() {
        set(&mut items.stock);
    }
    if let Some(camp) = camp {
        set(&mut camp.stock);
    }
    if let Some(element) = ELEMENT_ITEMS.iter().position(|&e| e == id)
        && let Some(magic) = game.magic.as_mut()
    {
        magic.levels[element] = count;
    }
}

/// The next bonus level Sora has a row in and has not taken.
fn next_bonus(game: &Game) -> Option<u8> {
    let battle = game.enemy.as_ref()?;
    let bonus_table = &game.drive.as_ref()?.src.growth.bons;
    let count = u32::from_le_bytes(bonus_table.get(4..8)?.try_into().ok()?) as usize;
    (0..count)
        .filter_map(|k| bonus_table.get(8 + k * 16..8 + k * 16 + 16))
        .filter(|record| record[1] == 1 && !battle.bonus_taken.contains(&record[0]))
        .map(|record| record[0])
        .min()
}

fn on_off(on: bool) -> Vec<u8> {
    text(if on { "On" } else { "Off" })
}

/// A catalog enemy's name as the game prints it. An enemy with no known
/// name is "Unidentified enemy"; its model code stays in the value column.
fn entry_name(entry: &enemy_catalog::Entry) -> Vec<u8> {
    match &entry.name_bytes {
        Some(bytes) => bytes.clone(),
        None if entry.category == enemy_catalog::Category::Unresolved => text("Unidentified enemy"),
        None => text(&entry.name),
    }
}

fn enemy_name(
    gallery: &enemy_gallery::EnemyGallery,
    actor: &enemy_gallery::GalleryActor,
) -> Vec<u8> {
    entry_name(&gallery.catalog.entries[actor.entry])
}

/// A motion's name for the menu. The motion set names its entries with bare
/// codes ("02A"), so a motion is named by the script actions that request
/// it ("appear, idle").
fn motion_title(kind: &enemy_catalog::Kind, index: usize) -> String {
    let names = kind.action_names_for_slot(kind.motions[index].slot);
    match names.as_slice() {
        [] => "Unnamed motion".into(),
        [one] => one.clone(),
        [first, second] => format!("{first}, {second}"),
        [first, second, rest @ ..] => format!("{first}, {second} and {} more", rest.len()),
    }
}

/// The motion set's own code for a motion, with its slot number.
fn motion_code(kind: &enemy_catalog::Kind, index: usize) -> String {
    let motion = &kind.motions[index];
    match motion.name.trim() {
        "" => format!("Slot {}", motion.slot),
        code => format!("{code}, slot {}", motion.slot),
    }
}

/// "12.5 / 48": the frame now showing and the motion's last frame.
fn frame_text(actor: &enemy_gallery::GalleryActor) -> String {
    match actor.timeline() {
        Some(timeline) => format!("{:.1} / {:.0}", actor.frame, timeline.end),
        None => "No motion".into(),
    }
}

/// The two lines above a preview's rows: the enemy and motion, then the
/// frame, speed and play state.
fn preview_info(
    gallery: &enemy_gallery::EnemyGallery,
    actor: Option<&enemy_gallery::GalleryActor>,
) -> Vec<Vec<u8>> {
    let Some(actor) = actor else {
        return vec![text("No enemy selected"), Vec::new()];
    };
    let motion = if actor.battle.is_some() {
        "AI is in control".to_string()
    } else if let Some(choice) = actor.motion_choice {
        format!(
            "{}, {}",
            motion_title(&actor.kind, choice),
            motion_code(&actor.kind, choice)
        )
    } else {
        "Bind pose".into()
    };
    let mut first = enemy_name(gallery, actor);
    first.extend(text(&format!(" - {motion}")));
    let state = if actor.battle.is_some() {
        "AI"
    } else if actor.paused {
        "Paused"
    } else if actor.looping {
        "Playing, looping"
    } else {
        "Playing"
    };
    let second = match actor.timeline() {
        Some(timeline) => format!(
            "Frame {}   {:.0} fps   {:.2}x   {state}",
            frame_text(actor),
            timeline.fps,
            actor.rate
        ),
        None => "Pick a motion on the Motions tab".into(),
    };
    vec![first, text(&second)]
}

fn rows(
    page: Page,
    saved: Option<(u8, u8)>,
    game: &Game,
    area: Option<&AreaRes>,
    camp: Option<&CampRes>,
    gallery: &enemy_gallery::EnemyGallery,
) -> Vec<Row> {
    let row = |label: &str, action: Action, enabled: bool| Row {
        label: text(label),
        value: Vec::new(),
        action,
        enabled,
    };
    let battle = game.enemy.as_ref();
    match page {
        Page::Main => {
            let level = battle.map(|b| (b.level, (b.level as usize) < b.sora_levels.len()));
            let busy = area.is_some_and(|a| a.transition.is_some() || a.pending.is_some());
            let mut list = vec![
                row(
                    "Items",
                    Action::Page(Page::Items),
                    game.items.is_some() || camp.is_some(),
                ),
                row("Restore HP, MP and Drive", Action::Restore, true),
                row("Knock Sora out", Action::KnockOut, battle.is_some()),
                Row {
                    value: level.map_or_else(Vec::new, |(l, _)| text(&format!("Lv {l}"))),
                    ..row("Level up", Action::LevelUp, level.is_some_and(|l| l.1))
                },
                row(
                    "Drive form",
                    Action::Page(Page::Forms),
                    game.drive.is_some(),
                ),
                Row {
                    value: next_bonus(game).map_or_else(Vec::new, |id| text(&format!("No. {id}"))),
                    ..row(
                        "Bonus level",
                        Action::BonusLevel,
                        next_bonus(game).is_some(),
                    )
                },
                row("Growth levels", Action::Page(Page::Growth), true),
                row("Summon enemies", Action::Page(Page::Enemies), true),
                Row {
                    value: on_off(game.player.abilities.has(enemy_target::SCAN_BIT)),
                    ..row(
                        "Scan",
                        Action::ToggleScan,
                        game.ability_slots
                            .iter()
                            .any(|s| s & 0x7fff == enemy_target::SCAN_ITEM),
                    )
                },
                Row {
                    value: on_off(game.cheats.invincible),
                    ..row("Invincible", Action::ToggleInvincible, battle.is_some())
                },
                row(
                    "Defeat battle target",
                    Action::DefeatBattleTarget,
                    battle.is_some_and(|b| b.enemy.alive()),
                ),
                row(
                    "Respawn battle target",
                    Action::RespawnBattleTarget,
                    battle.is_some(),
                ),
                Row {
                    value: on_off(game.cheats.enemy_frozen),
                    ..row(
                        "Freeze battle target",
                        Action::ToggleFreezeBattleTarget,
                        battle.is_some(),
                    )
                },
                row("Warp", Action::Page(Page::Warp), area.is_some() && !busy),
            ];
            list.push(row("Close", Action::Close, true));
            list
        }
        Page::Items => {
            let mut list = vec![row("Fill every item", Action::FillItems, true)];
            for (id, name) in item_list(camp, game) {
                list.push(Row {
                    label: name,
                    value: text(&stock_of(game, camp, id).to_string()),
                    action: Action::Item(id),
                    enabled: true,
                });
            }
            list
        }
        Page::Forms => {
            let drive = game.drive.as_ref();
            let idle = drive.is_some_and(|d| !d.active && d.change.is_none());
            let mut list: Vec<Row> = FORMS
                .iter()
                .map(|&(form, name)| row(name, Action::Form(form), idle))
                .collect();
            list.push(row(
                "Revert",
                Action::Form(0),
                drive.is_some_and(|d| d.active && d.change.is_none()),
            ));
            list
        }
        Page::Growth => game
            .ability_source
            .inventory
            .growth
            .iter()
            .enumerate()
            .filter_map(|(family, tiers)| {
                let slot = game
                    .ability_slots
                    .iter()
                    .find(|s| tiers.contains(&(**s & 0x7fff)))?;
                let id = *slot & 0x7fff;
                let label = camp
                    .and_then(|c| c.source.items.table.iter().find(|r| r.id == id))
                    .map(|r| r.name.clone())
                    .unwrap_or_else(|| text(&format!("Growth {}", family + 1)));
                Some(Row {
                    label,
                    value: on_off(*slot & 0x8000 != 0),
                    action: Action::Growth(family),
                    enabled: true,
                })
            })
            .collect(),
        Page::Enemies => {
            use enemy_catalog::Category;
            let mut list = Vec::new();
            if let Some(actor) = gallery.selected_actor() {
                list.push(Row {
                    value: enemy_name(gallery, actor),
                    ..row(
                        "Animation preview",
                        Action::Page(Page::Preview(PreviewTab::Playback)),
                        true,
                    )
                });
            }
            list.extend(
                [Category::Heartless, Category::Nobody, Category::Unresolved]
                    .into_iter()
                    .map(|category| {
                        let count = gallery
                            .catalog
                            .entries
                            .iter()
                            .filter(|e| e.category == category)
                            .count();
                        Row {
                            value: text(&count.to_string()),
                            ..row(
                                category.label(),
                                Action::Page(Page::Roster(category)),
                                count > 0,
                            )
                        }
                    }),
            );
            list.push(Row {
                value: text(&gallery.actors.len().to_string()),
                ..row(
                    "Summoned enemies",
                    Action::Page(Page::Actors),
                    !gallery.actors.is_empty(),
                )
            });
            list.push(row(
                "Remove every summoned enemy",
                Action::ClearActors,
                !gallery.actors.is_empty(),
            ));
            list
        }
        Page::Roster(category) => gallery
            .catalog
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.category == category)
            .map(|(index, e)| Row {
                label: entry_name(e),
                value: text(&if !e.model_available {
                    "No model".to_string()
                } else if !e.motions_available {
                    "No motions".to_string()
                } else if e.name_bytes.is_none() {
                    e.model_name.replace('_', " ")
                } else {
                    String::new()
                }),
                action: Action::Summon(index),
                enabled: e.model_available,
            })
            .collect(),
        Page::Actors => gallery
            .actors
            .iter()
            .map(|actor| {
                let mut label = enemy_name(gallery, actor);
                label.extend(text(&format!(" ({})", actor.id)));
                Row {
                    label,
                    value: text(&match actor.motion_choice {
                        Some(choice) => motion_title(&actor.kind, choice),
                        None => "Bind pose".into(),
                    }),
                    action: Action::SelectActor(actor.id),
                    enabled: true,
                }
            })
            .collect(),
        Page::Preview(tab) => {
            let Some(actor) = gallery.selected_actor() else {
                return vec![row("Summon an enemy first", Action::Info, false)];
            };
            let controllable = actor.battle.is_none();
            let playable = controllable && actor.timeline().is_some();
            match tab {
                PreviewTab::Playback => {
                    let length = actor.timeline().map_or_else(String::new, |t| {
                        format!("{:.0} frames, {:.2} s", t.length(), t.seconds())
                    });
                    vec![
                        Row {
                            value: text(if actor.paused { "Paused" } else { "Playing" }),
                            ..row("Play / pause", Action::PlayPause, playable)
                        },
                        row("Step forward one frame", Action::StepFrame(true), playable),
                        row("Step back one frame", Action::StepFrame(false), playable),
                        Row {
                            value: text(&frame_text(actor)),
                            ..row("Go to frame", Action::GoToFrame, playable)
                        },
                        row("Jump to start", Action::JumpToPercent(0), playable),
                        row("Jump to middle", Action::JumpToPercent(50), playable),
                        row("Jump to end", Action::JumpToPercent(100), playable),
                        row("Replay from start", Action::Replay, playable),
                        Row {
                            value: on_off(actor.looping),
                            ..row("Loop", Action::ToggleLoop, playable)
                        },
                        Row {
                            value: text(&format!("{:.2}x", actor.rate)),
                            ..row("Playback speed", Action::PlaybackSpeed, playable)
                        },
                        Row {
                            value: text(&length),
                            ..row("Length", Action::Info, false)
                        },
                    ]
                }
                PreviewTab::Motions => actor
                    .kind
                    .motions
                    .iter()
                    .enumerate()
                    .map(|(index, motion)| Row {
                        label: text(&motion_title(&actor.kind, index)),
                        value: text(&if motion.status.is_some() {
                            "Unavailable".to_string()
                        } else if actor.motion_choice == Some(index) {
                            "Playing".to_string()
                        } else {
                            motion_code(&actor.kind, index)
                        }),
                        action: Action::PlayMotion(index),
                        enabled: controllable && motion.status.is_none(),
                    })
                    .collect(),
                PreviewTab::Actions => actor
                    .kind
                    .action_choices()
                    .into_iter()
                    .map(|(label, index)| Row {
                        label: text(&label),
                        value: text(if actor.motion_choice == Some(index) {
                            "Playing"
                        } else {
                            ""
                        }),
                        action: Action::PlayMotion(index),
                        enabled: controllable && actor.kind.motions[index].status.is_none(),
                    })
                    .collect(),
                PreviewTab::View => vec![
                    Row {
                        value: on_off(gallery.overlays.skeleton),
                        ..row("Skeleton", Action::ToggleSkeleton, true)
                    },
                    Row {
                        value: on_off(gallery.overlays.hide_model),
                        ..row("Hide model", Action::ToggleHideModel, true)
                    },
                    Row {
                        value: on_off(game.show_collision),
                        ..row("Collision overlay", Action::ToggleCollisionOverlay, true)
                    },
                    Row {
                        value: on_off(actor.apply_root),
                        ..row("Root motion", Action::ToggleRootMotion, controllable)
                    },
                ],
                PreviewTab::Enemy => vec![
                    row(
                        "Enable AI and combat",
                        Action::EnableBattle,
                        actor.battle.is_none()
                            && super::enemy_battle::supported(
                                &gallery.catalog.entries[actor.entry],
                            ),
                    ),
                    Row {
                        value: text(&actor.battle.as_ref().map_or_else(
                            || "Preview only".into(),
                            |b| format!("HP {} {}", b.brain.hp(), b.brain.description()),
                        )),
                        ..row(
                            "Defeat this enemy",
                            Action::DefeatSummoned,
                            actor.battle.is_some(),
                        )
                    },
                    row(
                        "Respawn this enemy",
                        Action::RespawnSummoned,
                        actor.battle.is_some(),
                    ),
                    row("Remove this enemy", Action::RemoveActor, true),
                    row("Choose another enemy", Action::Page(Page::Actors), true),
                ],
            }
        }
        Page::Warp => {
            let Some(area) = area else {
                return Vec::new();
            };
            let (world, here) = (area.current.world, area.current.area);
            let code = ::area::progress::WORLDS[world as usize];
            let saved = crate::launcher::field_start(code).map(|_| {
                saved.map_or_else(|| text("Default"), |(n, _)| text(&format!("{code}{n:02}")))
            });
            let mut list = vec![Row {
                value: saved.clone().unwrap_or_default(),
                ..row(
                    &format!("Start here next launch ({code}{here:02})"),
                    Action::SaveStart,
                    saved.is_some(),
                )
            }];
            list.extend(
                (0..::area::progress::WORLDS.len() as u8)
                    .filter(|&w| !area.source.rooms(w).is_empty())
                    .map(|w| Row {
                        value: if w == world {
                            text("Here")
                        } else {
                            text(&format!("{} rooms", area.source.rooms(w).len()))
                        },
                        ..row(area::world_name(w), Action::Page(Page::Rooms(w)), true)
                    }),
            );
            list
        }
        Page::Rooms(world) => {
            let Some(area) = area else {
                return Vec::new();
            };
            let code = ::area::progress::WORLDS[world as usize];
            let current = (area.current.world, area.current.area);
            area.source
                .rooms(world)
                .iter()
                .map(|&room| Row {
                    label: area
                        .source
                        .place_name(world, room)
                        .cloned()
                        .unwrap_or_else(|| text(&format!("Room {room:02}"))),
                    value: if (world, room) == current {
                        text("Here")
                    } else {
                        text(&format!("{code}{room:02}"))
                    },
                    action: Action::Warp(world, room),
                    enabled: (world, room) != current,
                })
                .collect()
        }
    }
}

/// `rows`, narrowed to the ones containing the typed search.
fn listed(
    menu: &DebugMenu,
    game: &Game,
    area: Option<&AreaRes>,
    camp: Option<&CampRes>,
    gallery: &enemy_gallery::EnemyGallery,
) -> Vec<Row> {
    let mut list = rows(menu.page, menu.saved, game, area, camp, gallery);
    if menu.page.searchable() && !menu.search.is_empty() {
        list.retain(|row| {
            let words = format!("{} {}", plain(&row.label), plain(&row.value));
            menu.search
                .split_whitespace()
                .all(|word| words.contains(word))
        });
    }
    list
}

fn heading(page: Page) -> String {
    match page {
        Page::Main => "Debug".into(),
        Page::Items => "Debug - Items".into(),
        Page::Forms => "Debug - Drive form".into(),
        Page::Growth => "Debug - Growth levels".into(),
        Page::Enemies => "Debug - Summon enemies".into(),
        Page::Roster(category) => format!("Summon - {}", category.label()),
        Page::Actors => "Debug - Summoned enemies".into(),
        Page::Preview(_) => "Enemy animation preview".into(),
        Page::Warp => "Debug - Warp".into(),
        Page::Rooms(world) => format!("Debug - {}", area::world_name(world)),
    }
}

fn footer(page: Page) -> &'static str {
    match page {
        Page::Main => "Enter select   F3 close",
        Page::Items => "Left/Right change   type to search   Esc back",
        Page::Growth => "Left/Right change   Esc back",
        Page::Roster(_) => "Enter summon   type to search   Esc back",
        Page::Actors => "Enter preview   type to search   Esc back",
        Page::Preview(PreviewTab::Motions) => "Enter play   type to search   Tab next tab",
        Page::Preview(PreviewTab::Actions) => "Enter play (script candidates)   Tab next tab",
        Page::Preview(_) => "Tab next tab   Left/Right change   F3 hide",
        Page::Rooms(_) => "Enter warp   type to search   Esc back",
        Page::Warp => "Enter select   type to search   Esc back",
        _ => "Enter select   Esc back",
    }
}

/// Runs a row's action; returns a line for the footer.
#[allow(clippy::too_many_arguments)]
fn act(
    action: Action,
    step: i32,
    menu: &mut DebugMenu,
    game: &mut Game,
    area: Option<&mut AreaRes>,
    mut camp: Option<&mut CampRes>,
    hud: Option<&mut Hud>,
    gallery: &mut enemy_gallery::EnemyGallery,
) -> Option<String> {
    match action {
        Action::Info => None,
        Action::ToggleScan => {
            let enabled = !game.player.abilities.has(enemy_target::SCAN_BIT);
            Some(match enemy_target::set_scan(game, camp, enabled) {
                Ok(()) => format!("Scan {}", if enabled { "on" } else { "off" }),
                Err(e) => e,
            })
        }
        Action::EnableBattle => {
            gallery.queue_battle();
            Some("Loading source AI and combat".into())
        }
        Action::DefeatSummoned => {
            gallery.defeat_selected();
            None
        }
        Action::RespawnSummoned => {
            gallery.respawn_selected();
            None
        }
        Action::Page(page) => {
            if menu.page == Page::Main {
                menu.main_cursor = menu.cursor;
            }
            menu.go_to(page, 0);
            match (page, area.as_deref()) {
                (Page::Warp, Some(area)) => {
                    let code = ::area::progress::WORLDS[area.current.world as usize];
                    menu.saved = crate::launcher::saved_start(&crate::launcher::data_dir(), code);
                }
                // A world's rooms open on the current room.
                (Page::Rooms(world), Some(area)) if world == area.current.world => {
                    let rooms = area.source.rooms(world);
                    menu.cursor = rooms
                        .iter()
                        .position(|&room| room == area.current.area)
                        .unwrap_or(0);
                }
                _ => {}
            }
            None
        }
        Action::SaveStart => {
            let area = area?;
            let code = ::area::progress::WORLDS[area.current.world as usize];
            let (room, entrance) = (area.current.area, area.current.state.entrance);
            match crate::launcher::save_start(code, room, entrance) {
                Ok(_) => {
                    menu.saved = Some((room, entrance));
                    Some(format!("{code}{room:02} is this world's start"))
                }
                Err(e) => Some(format!("Start not saved: {e}")),
            }
        }
        Action::Close => {
            menu.open = false;
            None
        }
        Action::Restore => {
            if let Some(battle) = game.enemy.as_mut() {
                battle.sora_hp = battle.sora_target.max_hp;
                battle.ko_ticks = 0;
            } else if let Some(hud) = hud {
                hud.stats.hp = hud.stats.max_hp;
            }
            let mp = &mut game.player.mp;
            (mp.cur, mp.remaining, mp.total) = (mp.max, 0., 0.);
            game.player.mp_charging = false;
            // In a form the gauge is the form's timer; leave it running.
            if game.drive.as_ref().is_none_or(|d| !d.active) {
                game.player.drive.refill();
            }
            Some("HP, MP and Drive restored".into())
        }
        Action::KnockOut => {
            // Sora's HP reaches 0: the DEAD motion and the game over.
            let Game {
                player,
                clips,
                enemy,
                ..
            } = game;
            enemy.as_mut()?.sora_hp = 0;
            player.die(clips);
            menu.open = false;
            None
        }
        Action::LevelUp => {
            let tick = game.tick;
            let Game { enemy, log, .. } = game;
            let battle = enemy.as_mut()?;
            let next = battle.sora_levels.get(battle.level as usize - 1)?.exp;
            let need = next.saturating_sub(battle.exp_total).max(1);
            prize_room::grant_exp(battle, need, log, tick);
            Some(format!("Level {}", battle.level))
        }
        Action::BonusLevel => {
            // A battle's bonus level (FUN_1403edca0): the next one Sora has
            // a row in.
            let id = next_bonus(game)?;
            let bonus_table = game.drive.as_ref()?.src.growth.bons.clone();
            let Game { enemy, player, .. } = game;
            super::level_growth::apply_bonus(enemy.as_mut()?, player, &bonus_table, id).ok()?;
            Some(format!("Bonus level {id}"))
        }
        Action::ToggleInvincible => {
            game.cheats.invincible = !game.cheats.invincible;
            None
        }
        Action::ToggleFreezeBattleTarget => {
            game.cheats.enemy_frozen = !game.cheats.enemy_frozen;
            None
        }
        Action::DefeatBattleTarget => {
            game.enemy.as_mut()?.enemy.defeat();
            Some("Enemy defeated".into())
        }
        Action::RespawnBattleTarget => {
            game.enemy.as_mut()?.enemy.respawn();
            Some("Enemy respawned".into())
        }
        Action::FillItems => {
            for (id, _) in item_list(camp.as_deref(), game) {
                set_stock(game, camp.as_deref_mut(), id, cap_of(id));
            }
            // Empty battle slots take their item back.
            if let Some(items) = game.items.as_mut() {
                for (slot, &auto) in items.slots.iter_mut().zip(&items.originals) {
                    if *slot == 0 {
                        *slot = auto;
                    }
                }
            }
            Some("Every item filled".into())
        }
        Action::Item(id) => {
            let count = stock_of(game, camp.as_deref(), id) as i32 + step;
            set_stock(game, camp, id, count.clamp(0, cap_of(id) as i32) as u8);
            None
        }
        Action::Form(form) => {
            let drive = game.drive.as_ref()?;
            let (active, changing) = (drive.active, drive.change.is_some());
            if changing {
                return None;
            }
            if form == 0 {
                if active {
                    drive_room::start_change(game, 0);
                    menu.open = false;
                }
                return None;
            }
            if active {
                return None;
            }
            // A full gauge gives the form its full time.
            game.player.drive.refill();
            drive_room::start_change(game, form);
            menu.open = false;
            None
        }
        Action::Growth(family) => {
            game.ability_source
                .inventory
                .change_growth(&mut game.ability_slots, family, step)?;
            if let Some(camp) = camp {
                camp.ability_slots.clone_from(&game.ability_slots);
            }
            drive_room::rebuild_abilities(game, None);
            Some("Growth level changed; equip it in Abilities".into())
        }
        Action::Summon(entry) => {
            let from = Vec3::from_array(game.player.position);
            let yaw = game.player.yaw;
            let at = from + Quat::from_rotation_y(yaw) * Vec3::new(0., 0., 350.);
            gallery.queue_summon(entry, at, yaw + std::f32::consts::PI);
            menu.open = false;
            Some("Loading selected enemy".into())
        }
        Action::SelectActor(id) => {
            gallery.select(id);
            menu.go_to(Page::Preview(PreviewTab::Playback), 0);
            None
        }
        // The menu stays open: the motion plays behind it.
        Action::PlayMotion(index) => {
            gallery.queue_motion(index);
            None
        }
        Action::PlayPause => {
            gallery.toggle_paused();
            None
        }
        Action::StepFrame(forward) => {
            gallery.step_frame(forward);
            None
        }
        Action::GoToFrame => {
            gallery.scrub(step as f32);
            None
        }
        Action::JumpToPercent(percent) => {
            gallery.go_to_fraction(f32::from(percent) / 100.);
            None
        }
        Action::Replay => {
            gallery.restart();
            None
        }
        Action::ToggleLoop => {
            gallery.toggle_loop();
            None
        }
        Action::PlaybackSpeed => {
            gallery.change_speed(step);
            None
        }
        Action::ToggleRootMotion => {
            gallery.toggle_root();
            None
        }
        Action::ToggleSkeleton => {
            let overlays = &mut gallery.overlays;
            overlays.skeleton = !overlays.skeleton;
            // A hidden model needs the skeleton to show anything.
            overlays.hide_model &= overlays.skeleton;
            None
        }
        Action::ToggleHideModel => {
            let overlays = &mut gallery.overlays;
            overlays.hide_model = !overlays.hide_model;
            overlays.skeleton |= overlays.hide_model;
            None
        }
        Action::ToggleCollisionOverlay => {
            game.show_collision = !game.show_collision;
            None
        }
        Action::RemoveActor => {
            gallery.remove_selected();
            menu.go_to(Page::Actors, 0);
            None
        }
        Action::ClearActors => {
            gallery.clear();
            Some("Summons cleared".into())
        }
        Action::Warp(world, room) => {
            let area = area?;
            if area.transition.is_some() || area.pending.is_some() {
                return None;
            }
            area.transition = Some(Transition {
                stage: Stage::Closing,
                iris: None,
                fade: Fade::default(),
                target: (world, room, 0),
                job: start_area_job(&area.source, (world, room, 0)),
            });
            menu.open = false;
            None
        }
    }
}

/// The row to start on when a preview tab opens: the motion now playing.
fn playing_row(
    page: Page,
    menu: &DebugMenu,
    game: &Game,
    area: Option<&AreaRes>,
    camp: Option<&CampRes>,
    gallery: &enemy_gallery::EnemyGallery,
) -> usize {
    let Some(choice) = gallery.selected_actor().and_then(|a| a.motion_choice) else {
        return 0;
    };
    rows(page, menu.saved, game, area, camp, gallery)
        .iter()
        .position(|row| row.action == Action::PlayMotion(choice))
        .unwrap_or(0)
}

/// Opens, drives and closes the menu (Update, so presses are not missed
/// between fixed ticks).
#[allow(clippy::too_many_arguments)]
pub fn tick(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut key_events: MessageReader<KeyboardInput>,
    time: Res<Time>,
    pads: Query<(&Gamepad, Option<&Name>)>,
    mut menu: ResMut<DebugMenu>,
    mut game: ResMut<Game>,
    mut area: Option<ResMut<AreaRes>>,
    mut camp: Option<ResMut<CampRes>>,
    mut hud: Option<ResMut<Hud>>,
    mut gallery: ResMut<enemy_gallery::EnemyGallery>,
    mut gallery_notice: Local<String>,
) {
    let typed: Vec<char> = key_events
        .read()
        .filter(|event| event.state == ButtonState::Pressed)
        .filter_map(|event| match &event.logical_key {
            Key::Character(characters) => Some(characters.chars().collect::<Vec<_>>()),
            _ => None,
        })
        .flatten()
        .collect();
    if *gallery_notice != gallery.status {
        *gallery_notice = gallery.status.clone();
        eprintln!("Enemy preview: {}", gallery.status);
        game.event(
            serde_json::json!({"event":"enemy_preview_status","message":gallery.status,
            "actors":gallery.actors.len(),"selected":gallery.selected}),
        );
        if let Some(h) = hud.as_deref_mut() {
            h.updated = true;
        }
    }
    if game.autoplay.is_some() {
        if game.enemy_tour {
            enemy_tour::show_page(game.captures_taken, &mut menu, &mut gallery);
        }
        if game.enemy_tour && (menu.open || menu.view != View::default()) {
            menu.view = if menu.open {
                build_view(&mut menu, &game, area.as_deref(), camp.as_deref(), &gallery)
            } else {
                View::default()
            };
            if let Some(h) = hud.as_deref_mut() {
                h.updated = true;
            }
        }
        return;
    }
    let menu = &mut *menu;
    let input = read_input(&keys, &pads, &mut menu.held);
    let was_open = menu.open;
    // Esc backs out of the menu; the room must not also take it as exit.
    if was_open {
        keys.clear_just_pressed(KeyCode::Escape);
    }
    if !menu.open {
        let blocked = camp.as_ref().is_some_and(|c| c.menu_up()) || game.talking;
        if !input.toggle || blocked {
            return;
        }
        menu.open = true;
        // The preview reopens where it was left; everything else starts at
        // the main page.
        if matches!(menu.page, Page::Preview(_)) && gallery.selected_actor().is_some() {
            menu.top = 0;
        } else {
            menu.go_to(Page::Main, menu.main_cursor);
        }
    } else if input.toggle {
        menu.open = false;
    } else {
        // Typed letters narrow the list; Backspace and Esc take them back
        // before they leave the page.
        let mut back = input.back;
        if menu.page.searchable() {
            let before = menu.search.clone();
            for c in typed.iter().copied().filter(|&c| searchable_char(c)) {
                if menu.search.len() < MAX_SEARCH {
                    menu.search.push(c.to_ascii_lowercase());
                }
            }
            if back && !menu.search.is_empty() {
                if input.erase {
                    menu.search.pop();
                } else {
                    menu.search.clear();
                }
                back = false;
            }
            if menu.search != before {
                menu.cursor = 0;
                menu.top = 0;
            }
        }
        let list = listed(menu, &game, area.as_deref(), camp.as_deref(), &gallery);
        let count = list.len();
        let visible = menu.page.visible_rows();
        menu.cursor = menu.cursor.min(count.saturating_sub(1));
        if count > 0 {
            if keys.just_pressed(KeyCode::PageDown) {
                menu.cursor = (menu.cursor + visible).min(count - 1);
            } else if keys.just_pressed(KeyCode::PageUp) {
                menu.cursor = menu.cursor.saturating_sub(visible);
            } else if keys.just_pressed(KeyCode::Home) {
                menu.cursor = 0;
            } else if keys.just_pressed(KeyCode::End) {
                menu.cursor = count - 1;
            }
        }
        let selected = list.get(menu.cursor);
        let mut run = None;
        let mut tab = input.tab;
        match input.step {
            Some(Dir::Up) if count > 0 => menu.cursor = (menu.cursor + count - 1) % count,
            Some(Dir::Down) if count > 0 => menu.cursor = (menu.cursor + 1) % count,
            Some(d @ (Dir::Left | Dir::Right)) => {
                let direction = if d == Dir::Left { -1 } else { 1 };
                if let Some(row) = selected.filter(|r| r.action.adjustable()) {
                    run = Some((row.action, direction));
                } else {
                    // Rows with no value to change leave Left and Right free
                    // for the preview's tabs.
                    tab = direction;
                }
            }
            _ => {}
        }
        if let Page::Preview(current) = menu.page
            && tab != 0
        {
            let next = Page::Preview(current.shifted(tab));
            let cursor = playing_row(
                next,
                menu,
                &game,
                area.as_deref(),
                camp.as_deref(),
                &gallery,
            );
            menu.go_to(next, cursor);
        } else if input.confirm
            && let Some(row) = selected.filter(|r| r.enabled)
        {
            run = Some((row.action, 1));
        } else if back {
            match menu.page {
                Page::Main => menu.open = false,
                // Back to the world list, on the world just left.
                Page::Rooms(world) => {
                    let cursor = rows(
                        Page::Warp,
                        menu.saved,
                        &game,
                        area.as_deref(),
                        None,
                        &gallery,
                    )
                    .iter()
                    .position(|r| r.action == Action::Page(Page::Rooms(world)))
                    .unwrap_or(0);
                    menu.go_to(Page::Warp, cursor);
                }
                Page::Roster(_) | Page::Actors => menu.go_to(Page::Enemies, 0),
                Page::Preview(_) => menu.go_to(Page::Actors, 0),
                _ => menu.go_to(Page::Main, menu.main_cursor),
            }
        }
        if let Some((action, step)) = run {
            let notice = act(
                action,
                step,
                menu,
                &mut game,
                area.as_deref_mut(),
                camp.as_deref_mut(),
                hud.as_deref_mut(),
                &mut gallery,
            );
            if let Some(n) = notice {
                eprintln!("Debug menu: {n}");
                menu.notice = Some((n, NOTICE_FRAMES));
            }
        }
    }
    // The preview keeps playing behind its own tabs (the field is paused
    // while the menu is open, so its clock does not).
    if menu.open && matches!(menu.page, Page::Preview(_)) {
        enemy_gallery::advance(&mut gallery, time.delta_secs() * TICKS_PER_SECOND);
    }
    if let Some((_, frames)) = menu.notice.as_mut() {
        *frames = frames.saturating_sub(1);
        if *frames == 0 {
            menu.notice = None;
        }
    }
    let view = if menu.open {
        build_view(menu, &game, area.as_deref(), camp.as_deref(), &gallery)
    } else {
        View::default()
    };
    if view != menu.view || was_open != menu.open {
        menu.view = view;
        if let Some(h) = hud.as_deref_mut() {
            h.updated = true;
        }
    }
}

fn build_view(
    menu: &mut DebugMenu,
    game: &Game,
    area: Option<&AreaRes>,
    camp: Option<&CampRes>,
    gallery: &enemy_gallery::EnemyGallery,
) -> View {
    let list = listed(menu, game, area, camp, gallery);
    let visible = menu.page.visible_rows();
    menu.cursor = menu.cursor.min(list.len().saturating_sub(1));
    menu.top = scroll(menu.cursor, menu.top, visible);
    let footer = match &menu.notice {
        Some((notice, _)) => notice.clone(),
        None if list.len() > visible => {
            format!("{}   {}/{}", footer(menu.page), menu.cursor + 1, list.len())
        }
        None => footer(menu.page).to_string(),
    };
    let (tabs, info) = match menu.page {
        Page::Preview(open) => (
            PreviewTab::ALL
                .iter()
                .map(|&tab| (text(tab.label()), tab == open))
                .collect(),
            preview_info(gallery, gallery.selected_actor()),
        ),
        _ => (Vec::new(), Vec::new()),
    };
    let search = menu.page.searchable().then(|| {
        text(&if menu.search.is_empty() {
            "Search: type to filter".to_string()
        } else {
            format!("Search: {}   ({} found)", menu.search, list.len())
        })
    });
    View {
        heading: text(&heading(menu.page)),
        tabs,
        info,
        search,
        rows: list
            .into_iter()
            .skip(menu.top)
            .take(visible)
            .map(|r| (r.label, r.value, r.enabled))
            .collect(),
        selected: Some(menu.cursor - menu.top),
        footer: text(&footer),
    }
}

/// The first row shown, moved just enough to keep the cursor in view.
fn scroll(cursor: usize, top: usize, visible: usize) -> usize {
    if cursor < top {
        cursor
    } else if cursor >= top + visible {
        cursor + 1 - visible
    } else {
        top
    }
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32, rgba: [f32; 4]) -> sprite_sequence::Quad {
    sprite_sequence::Quad {
        corners: [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
        uv: [0, 0, 1, 1],
        colors: [rgba; 4],
        blend: 0,
    }
}

/// The menu's draws: the box and highlight bar on `white`, the text on the
/// system font sheet `font_sheet`.
pub fn draws(view: &View, white: usize, font_sheet: usize, widths: &[u8]) -> Vec<Draw> {
    let font = field_command_menu::Font { widths, pad: 1 };
    let [x0, y0, x1, y1] = BOX;
    let fit = |text: &[u8], size: f32, width: f32| {
        size.min(size * width.max(1.) / field_command_menu::text_width(text, &font, size).max(1.))
    };
    let mut shapes = vec![rect(x0, y0, x1, y1, [0.04, 0.04, 0.06, 0.85])];
    let mut glyphs = Vec::new();
    let cell = 24. * TEXT_SIZE / 18.;
    let line = |y: f32| y + (ROW_HEIGHT - cell) / 2.;
    let inner_width = x1 - x0 - MARGIN * 2.;
    glyphs.extend(field_command_menu::text_quads_sized(
        &view.heading,
        [x0 + MARGIN, line(y0 + 6.)],
        [1.; 4],
        &font,
        fit(&view.heading, TEXT_SIZE + 2., inner_width),
    ));
    let mut top = y0 + 34.;
    if !view.tabs.is_empty() {
        // The tab strip: the open tab on a bar, the others dim.
        let mut x = x0 + MARGIN;
        for (label, open) in &view.tabs {
            let width = field_command_menu::text_width(label, &font, TEXT_SIZE - 1.);
            if *open {
                shapes.push(rect(
                    x - 4.,
                    top,
                    x + width + 4.,
                    top + ROW_HEIGHT,
                    [1., 1., 1., 0.3],
                ));
            }
            glyphs.extend(field_command_menu::text_quads_sized(
                label,
                [x, line(top)],
                if *open { [1.; 4] } else { [0.6, 0.6, 0.6, 1.] },
                &font,
                TEXT_SIZE - 1.,
            ));
            x += width + 14.;
        }
        top += ROW_HEIGHT;
    }
    for info in &view.info {
        glyphs.extend(field_command_menu::text_quads_sized(
            info,
            [x0 + MARGIN, line(top)],
            [0.7, 0.85, 1., 1.],
            &font,
            fit(info, TEXT_SIZE - 1., inner_width),
        ));
        top += ROW_HEIGHT;
    }
    if let Some(search) = &view.search {
        glyphs.extend(field_command_menu::text_quads_sized(
            search,
            [x0 + MARGIN, line(top)],
            [1., 0.9, 0.5, 1.],
            &font,
            fit(search, TEXT_SIZE - 1., inner_width),
        ));
        top += ROW_HEIGHT;
    }
    for (i, (label, value, enabled)) in view.rows.iter().enumerate() {
        let y = top + i as f32 * ROW_HEIGHT;
        if view.selected == Some(i) {
            shapes.push(rect(
                x0 + 6.,
                y,
                x1 - 6.,
                y + ROW_HEIGHT,
                [1., 1., 1., 0.16],
            ));
        }
        let colour = if *enabled {
            [1.; 4]
        } else {
            [0.5, 0.5, 0.5, 1.]
        };
        let value_width = field_command_menu::text_width(value, &font, TEXT_SIZE);
        glyphs.extend(field_command_menu::text_quads_sized(
            label,
            [x0 + MARGIN, line(y)],
            colour,
            &font,
            fit(label, TEXT_SIZE, inner_width - value_width - 8.),
        ));
        if !value.is_empty() {
            glyphs.extend(field_command_menu::text_quads_sized(
                value,
                [x1 - MARGIN - value_width, line(y)],
                colour,
                &font,
                TEXT_SIZE,
            ));
        }
    }
    glyphs.extend(field_command_menu::text_quads_sized(
        &view.footer,
        [x0 + MARGIN, line(y1 - ROW_HEIGHT - 6.)],
        [0.7, 0.7, 0.7, 1.],
        &font,
        fit(&view.footer, TEXT_SIZE - 2., inner_width),
    ));
    vec![
        Draw::Quads {
            quads: shapes,
            z: 0,
            sheet: white,
        },
        Draw::Quads {
            quads: glyphs,
            z: 0,
            sheet: font_sheet,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scroll_keeps_the_cursor_in_view() {
        assert_eq!(scroll(20, 0, 15), 20 + 1 - 15);
        assert_eq!(scroll(3, 10, 15), 3);
        assert_eq!(scroll(12, 5, 15), 5);
        // A page with header lines shows fewer rows.
        assert_eq!(scroll(12, 0, 11), 2);
    }

    #[test]
    fn spell_items_cap_at_their_level() {
        assert_eq!(cap_of(ELEMENT_ITEMS[0]), MAX_SPELL);
        assert_eq!(cap_of(1), MAX_STOCK);
    }

    #[test]
    fn preview_tabs_wrap_in_both_directions() {
        assert_eq!(PreviewTab::Playback.shifted(1), PreviewTab::Motions);
        assert_eq!(PreviewTab::Playback.shifted(-1), PreviewTab::Enemy);
        assert_eq!(PreviewTab::Enemy.shifted(1), PreviewTab::Playback);
        assert_eq!(PreviewTab::View.shifted(5), PreviewTab::View);
    }

    #[test]
    fn header_lines_leave_room_for_the_rows() {
        assert_eq!(Page::Main.visible_rows(), LINES);
        assert_eq!(Page::Items.visible_rows(), LINES - 1);
        assert_eq!(
            Page::Preview(PreviewTab::Playback).visible_rows(),
            LINES - 3
        );
        assert_eq!(Page::Preview(PreviewTab::Motions).visible_rows(), LINES - 4);
    }

    #[test]
    fn search_reads_back_what_the_menu_font_shows() {
        assert_eq!(plain(&text("Shadow 12")), "shadow 12");
        assert_eq!(plain(&text("Idle: slot 3.")), "idle: slot 3.");
        assert_eq!(plain(&text("N_HIT")), "n hit");
        assert!(searchable_char('a') && searchable_char('7') && searchable_char('-'));
        assert!(!searchable_char('_') && !searchable_char('#'));
    }

    #[test]
    fn draws_put_text_inside_the_box() {
        let view = View {
            heading: text("Debug"),
            tabs: vec![(text("Playback"), true), (text("Motions"), false)],
            info: vec![text("Shadow - Idle"), text("Frame 1.0 / 48")],
            search: Some(text("Search: type to filter")),
            rows: vec![(text("Items"), text("12"), true)],
            selected: Some(0),
            footer: text("F3 close"),
        };
        let widths = [10u8; 224];
        let d = draws(&view, 0, 1, &widths);
        assert_eq!(d.len(), 2);
        let Draw::Quads { quads, .. } = &d[1] else {
            panic!("text draw")
        };
        for q in quads {
            for [x, y] in q.corners {
                assert!((BOX[0]..=BOX[2]).contains(&x) && (BOX[1]..=BOX[3]).contains(&y));
            }
        }
    }
}
