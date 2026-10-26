//! The test room's debug menu (F3, or both stick buttons on a controller;
//! F2 opens it on Help): four tabs, Sora, Enemies, Warp and Help, whose rows
//! give items, restore and level Sora, change Drive form, control the enemy,
//! preview enemy animations and warp between areas. A development aid, not
//! part of the original game. It is drawn on the HUD canvas with the system
//! font, in a blue panel styled after the game's menus, and plays the se000
//! menu sounds the pause menu uses. The field is paused while it is open.
//!
//! Rows are grouped under section headings, every row has a one-line
//! description at the bottom, and the footer lists the keys and buttons the
//! page takes. Lists with many rows (items, enemies, motions, rooms) take
//! typed letters as a search: only rows containing them stay. Tabs change
//! with Tab / Shift+Tab, LB / RB, or Left / Right on a row with no value to
//! change; the enemy animation preview has its own tabs and keeps playing
//! behind the menu.
use super::*;
use bevy::input::{
    ButtonState,
    keyboard::{Key, KeyboardInput},
};
use pause_menu::{SOUND_CLOSE, SOUND_CURSOR, SOUND_DECIDE, SOUND_INVALID, SOUND_OPEN};
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

/// One white texel, for the panel, the bars and the arrows.
pub const WHITE: [u8; 4] = [255; 4];

/// The HUD canvas in HUD units.
const CANVAS: [f32; 2] = [512., 416.];
/// The panel, and its title row, tab strip, list, description and key hint
/// lines (HUD units).
const BOX: [f32; 4] = [64., 20., 448., 394.];
const MARGIN: f32 = 14.;
const TITLE_Y: f32 = BOX[1] + 4.;
const TITLE_HEIGHT: f32 = 22.;
const STRIP_Y: f32 = BOX[1] + 30.;
const BODY_Y: f32 = BOX[1] + 54.;
const DESCRIPTION_Y: f32 = BOX[3] - 48.;
const HINTS_Y: f32 = BOX[3] - 24.;
/// Lines in the list area (header lines and rows together).
const LINES: usize = 15;
const ROW_HEIGHT: f32 = 17.;
const TEXT_SIZE: f32 = 13.;
const TITLE_SIZE: f32 = 16.;
const SMALL_SIZE: f32 = 11.;
/// Rows sit this far right of their section heading.
const INDENT: f32 = 6.;
/// Room right of the value column for the arrows.
const ARROW_ROOM: f32 = 12.;
const TAB_GAP: f32 = 16.;
// The list fits between the tab strip and the description, and the panel
// on the canvas.
const _: () = assert!(
    BODY_Y >= STRIP_Y + ROW_HEIGHT + 4.
        && BODY_Y + LINES as f32 * ROW_HEIGHT <= DESCRIPTION_Y - 5.
        && DESCRIPTION_Y + ROW_HEIGHT <= HINTS_Y
        && HINTS_Y + ROW_HEIGHT <= BOX[3]
        && BOX[2] <= CANVAS[0]
        && BOX[3] <= CANVAS[1]
);

/// Panel colours. The game's menus are blue panels with a lighter edge and
/// a bright cursor bar; these values were picked by eye to match that look
/// (they are not read from the game's files).
const PANEL_TOP: [f32; 4] = [0.10, 0.17, 0.38, 0.94];
const PANEL_BOTTOM: [f32; 4] = [0.02, 0.04, 0.13, 0.94];
const BAND_TOP: [f32; 4] = [0.30, 0.45, 0.85, 0.35];
const BAND_BOTTOM: [f32; 4] = [0.10, 0.18, 0.45, 0.0];
const BORDER: [f32; 4] = [0.55, 0.72, 1.0, 0.85];
const RULE: [f32; 4] = [0.45, 0.62, 1.0, 0.55];
const CURSOR_LEFT: [f32; 4] = [0.25, 0.50, 1.0, 0.60];
const CURSOR_RIGHT: [f32; 4] = [0.15, 0.32, 0.90, 0.0];
const CURSOR_EDGE: [f32; 4] = [0.88, 0.95, 1.0, 0.95];
const TAB_TOP: [f32; 4] = [0.40, 0.62, 1.0, 0.75];
const TAB_BOTTOM: [f32; 4] = [0.12, 0.26, 0.72, 0.75];
/// Text colours multiply the system font's grey glyphs with 0x80 = 1.0, as
/// the game's colour tables do. Rows, the cursor's row and unusable rows
/// take the command menu's row and cursor text colours (0x140717d78,
/// 0x140717da0: 0xb4, 0xe6, 0x64 and 0x96 per channel); the tinted lines
/// are picked by eye.
const TEXT: [f32; 4] = [1.406, 1.406, 1.406, 1.];
const TEXT_CHOSEN: [f32; 4] = [1.797, 1.797, 1.797, 1.];
const TEXT_DIM: [f32; 4] = [0.781, 0.781, 0.781, 1.];
const TEXT_DIM_CHOSEN: [f32; 4] = [1.172, 1.172, 1.172, 1.];
const VALUE: [f32; 4] = [1.15, 1.30, 1.60, 1.];
const SECTION: [f32; 4] = [0.80, 1.10, 1.70, 1.];
const TAB_DIM: [f32; 4] = [0.95, 1.05, 1.30, 1.];
const INFO: [f32; 4] = [1.05, 1.30, 1.75, 1.];
const SEARCH: [f32; 4] = [1.80, 1.60, 0.85, 1.];
const DESCRIPTION: [f32; 4] = [1.50, 1.56, 1.70, 1.];
const HINT: [f32; 4] = [0.95, 1.05, 1.25, 1.];
/// The arrows and the scroll bar's thumb (plain, untextured quads).
const MARK: [f32; 4] = [0.85, 0.92, 1.0, 1.];

/// Motion, in seconds: opening and closing, a new page or tab sliding in,
/// and the time constant with which the cursor and the tab bar follow.
/// Chosen for a quick feel; the game's own menu timings were not measured.
const OPEN_SECONDS: f32 = 0.12;
const CLOSE_SECONDS: f32 = 0.08;
const SLIDE_SECONDS: f32 = 0.12;
const FOLLOW_SECONDS: f32 = 0.035;
/// How far (HUD units) the panel slides in from the left, and a page from
/// the side it comes from.
const OPEN_DISTANCE: f32 = 16.;
const SLIDE_DISTANCE: f32 = 18.;

/// Held arrows repeat after 18 ticks, then every 4 (ticks, so the repeat
/// is as fast at any frame rate).
const REPEAT_DELAY: u32 = 18;
const REPEAT_EVERY: u32 = 4;
/// Spell items hold a level; other stock counts stop at 99.
const MAX_SPELL: u8 = 3;
const MAX_STOCK: u8 = 99;
/// Frames a result line replaces the description.
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

/// Where `at` sits in `all`.
fn place<T: Copy + PartialEq>(all: &[T], at: T) -> usize {
    all.iter().position(|&t| t == at).unwrap_or(0)
}

/// The tab `by` places along `all` from `at`, wrapping around.
fn shifted<T: Copy + PartialEq>(all: &[T], at: T, by: i32) -> T {
    let index = place(all, at) as i32;
    all[(index + by).rem_euclid(all.len() as i32) as usize]
}

/// The menu's own tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum HomeTab {
    /// Restore, level, cheats, items, Drive forms and growth.
    #[default]
    Sora,
    /// The room's battle enemy, summoning and the summoned enemies.
    Enemies,
    /// The worlds with installed rooms.
    Warp,
    /// Controls, pages and hotkeys.
    Help,
}

impl HomeTab {
    const ALL: [Self; 4] = [Self::Sora, Self::Enemies, Self::Warp, Self::Help];

    fn label(self) -> &'static str {
        match self {
            Self::Sora => "Sora",
            Self::Enemies => "Enemies",
            Self::Warp => "Warp",
            Self::Help => "Help",
        }
    }
}

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    /// One of the menu's own tabs.
    Home(HomeTab),
    Items,
    Forms,
    Growth,
    Roster(enemy_catalog::Category),
    /// The summoned enemies.
    Actors,
    /// The animation preview of the selected summoned enemy.
    Preview(PreviewTab),
    /// One world's rooms.
    Rooms(u8),
}

impl Default for Page {
    fn default() -> Self {
        Self::Home(HomeTab::default())
    }
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
                | Self::Home(HomeTab::Warp)
                | Self::Rooms(_)
        )
    }

    /// Lines above the rows: two about the playing motion on the preview,
    /// and the search line on searchable pages.
    fn header_lines(self) -> usize {
        let preview = if matches!(self, Self::Preview(_)) {
            2
        } else {
            0
        };
        preview + usize::from(self.searchable())
    }

    fn visible_rows(self) -> usize {
        LINES - self.header_lines()
    }

    /// The tab labels of the strip, and the open one's place.
    fn tabs(self) -> Option<(Vec<&'static str>, usize)> {
        match self {
            Self::Home(tab) => Some((
                HomeTab::ALL.iter().map(|t| t.label()).collect(),
                place(&HomeTab::ALL, tab),
            )),
            Self::Preview(tab) => Some((
                PreviewTab::ALL.iter().map(|t| t.label()).collect(),
                place(&PreviewTab::ALL, tab),
            )),
            _ => None,
        }
    }

    /// The page `by` tabs along, on a page with tabs.
    fn next_tab(self, by: i32) -> Option<Self> {
        match self {
            Self::Home(tab) => Some(Self::Home(shifted(&HomeTab::ALL, tab, by))),
            Self::Preview(tab) => Some(Self::Preview(shifted(&PreviewTab::ALL, tab, by))),
            _ => None,
        }
    }

    /// Whether `self` and `other` share a tab strip.
    fn same_strip(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Home(_), Self::Home(_)) | (Self::Preview(_), Self::Preview(_))
        )
    }

    /// The page Back returns to; none on the menu's own tabs (Back closes).
    fn parent(self) -> Option<Self> {
        match self {
            Self::Home(_) => None,
            Self::Items | Self::Forms | Self::Growth => Some(Self::Home(HomeTab::Sora)),
            Self::Roster(_) | Self::Actors => Some(Self::Home(HomeTab::Enemies)),
            Self::Preview(_) => Some(Self::Actors),
            Self::Rooms(_) => Some(Self::Home(HomeTab::Warp)),
        }
    }

    fn title(self) -> String {
        match self {
            Self::Home(_) => "Debug Menu".into(),
            Self::Items => "Items".into(),
            Self::Forms => "Drive Form".into(),
            Self::Growth => "Growth Abilities".into(),
            Self::Roster(category) => format!("Summon {}", category.label()),
            Self::Actors => "Summoned Enemies".into(),
            Self::Preview(_) => "Enemy Preview".into(),
            Self::Rooms(world) => area::world_name(world).into(),
        }
    }

    /// Where the page sits, for pages with no tab strip.
    fn path(self) -> String {
        match self {
            Self::Home(_) | Self::Preview(_) => String::new(),
            Self::Items | Self::Forms | Self::Growth => format!("Sora / {}", self.title()),
            Self::Roster(_) => "Enemies / Summon".into(),
            Self::Actors => "Enemies / Summoned".into(),
            Self::Rooms(world) => format!("Warp / {}", area::world_name(world)),
        }
    }

    /// The keys and buttons the page takes.
    fn hints(self) -> &'static str {
        match self {
            Self::Home(HomeTab::Help) => "Up/Down scroll   Tab, LB/RB tabs   Esc/B close",
            Self::Home(HomeTab::Warp) => {
                "Enter/A open   type to search   Tab, LB/RB tabs   Esc/B close"
            }
            Self::Home(_) => "Enter/A select   Tab, LB/RB tabs   Esc/B close   F2 help",
            Self::Items => "Left/Right change   type to search   Esc/B back",
            Self::Forms => "Enter/A change form   Esc/B back",
            Self::Growth => "Left/Right change level   Esc/B back",
            Self::Roster(_) => "Enter/A summon   type to search   Esc/B back",
            Self::Actors => "Enter/A preview   type to search   Esc/B back",
            Self::Preview(PreviewTab::Motions | PreviewTab::Actions) => {
                "Enter/A play   type to search   Tab, LB/RB tabs   Esc/B back"
            }
            Self::Preview(PreviewTab::Playback) => {
                "Enter/A use   Left/Right change   Tab, LB/RB tabs   F3 hide"
            }
            Self::Preview(_) => "Enter/A use   Tab, LB/RB tabs   Esc/B back   F3 hide",
            Self::Rooms(_) => "Enter/A warp   type to search   Esc/B back",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Page(Page),
    /// A section heading (the cursor passes over it).
    Section,
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
    TogglePerfOverlay,
    /// The duel's test script: a second Sora that fights the room's.
    SpawnRival,
    RemoveRival,
    ToggleNetOverlay,
    /// The duel's damage rule (host): Left/Right step it.
    DuelDamage,
    /// Steps the link simulator through its presets.
    CycleNetSimulator,
    RemoveActor,
    ClearActors,
    Warp(u8, u8),
    /// The current room becomes its world's launcher start.
    SaveStart,
}

impl Action {
    /// Whether Left and Right change this row's value.
    fn adjustable(self) -> bool {
        matches!(
            self,
            Self::Item(_)
                | Self::Growth(_)
                | Self::PlaybackSpeed
                | Self::GoToFrame
                | Self::DuelDamage
        )
    }

    /// Whether the row opens another page.
    fn opens_page(self) -> bool {
        matches!(self, Self::Page(_) | Self::SelectActor(_))
    }

    /// The row's line in the description bar.
    fn describe(self) -> &'static str {
        use enemy_catalog::Category;
        match self {
            Self::Section | Self::Info => "",
            Self::Page(page) => match page {
                Page::Items => "Change item stock and spell levels.",
                Page::Forms => "Change into a Drive form, or revert.",
                Page::Growth => "Change the levels of Sora's growth abilities.",
                Page::Roster(Category::Heartless) => "Pick a Heartless to summon in front of Sora.",
                Page::Roster(Category::Nobody) => "Pick a Nobody to summon in front of Sora.",
                Page::Roster(Category::Unresolved) => {
                    "Models that look like enemies but have no known name."
                }
                Page::Actors => "The enemies summoned so far. Pick one to preview it.",
                Page::Preview(_) => "Play, step and inspect the selected enemy's motions.",
                Page::Rooms(_) => "Show this world's installed rooms.",
                Page::Home(_) => "",
            },
            Self::Restore => "Fill HP, MP and the Drive gauge. A form's timer keeps running.",
            Self::KnockOut => "Drop Sora's HP to 0: the KO motion and the game over.",
            Self::LevelUp => "Give Sora the EXP for the next level.",
            Self::BonusLevel => "Apply the next bonus level Sora has not taken.",
            Self::ToggleInvincible => "Sora takes no damage.",
            Self::ToggleScan => "Show enemy HP gauges. Needs Scan in an ability slot.",
            Self::DefeatBattleTarget => "Defeat the room's own battle enemy.",
            Self::RespawnBattleTarget => "Bring the room's battle enemy back.",
            Self::ToggleFreezeBattleTarget => "The room's battle enemy stands still.",
            Self::FillItems => "Every item to 99 and every spell to level 3.",
            Self::Item(_) => "Left/Right change the count. Spells hold a level, 0 to 3.",
            Self::Form(0) => "Leave the current form.",
            Self::Form(_) => "Change into this form with a full gauge.",
            Self::Growth(_) => "Left/Right change the level. Equip it in the pause menu.",
            Self::Summon(_) => "Summon this enemy in front of Sora.",
            Self::SelectActor(_) => "Open this enemy's animation preview.",
            Self::PlayMotion(_) => "Play this motion. The menu stays open.",
            Self::EnableBattle => "Load the enemy's own AI and let it fight.",
            Self::DefeatSummoned => "Defeat this enemy.",
            Self::RespawnSummoned => "Bring this enemy back.",
            Self::PlayPause => "Pause or resume the motion.",
            Self::StepFrame(_) => "Move one whole frame and pause.",
            Self::GoToFrame => "Left/Right move the paused frame.",
            Self::JumpToPercent(_) => "Pause at this point of the motion.",
            Self::Replay => "Play the motion again from its first frame.",
            Self::ToggleLoop => "Repeat the motion when it ends.",
            Self::PlaybackSpeed => "Left/Right step the speed from 0.1x to 4x.",
            Self::ToggleRootMotion => "Move the enemy along the motion's root.",
            Self::ToggleSkeleton => "Draw a line from each bone to its parent.",
            Self::ToggleHideModel => "Hide the model and keep the skeleton.",
            Self::ToggleCollisionOverlay => "Show the room's collision. F1 does the same.",
            Self::TogglePerfOverlay => "FPS, frame times, GPU and entities at the top left.",
            Self::SpawnRival => "A second Sora, run by a test script, to try the duel alone.",
            Self::RemoveRival => "Remove the scripted second Sora.",
            Self::ToggleNetOverlay => "Ping, loss, data rates and warnings at the top right.",
            Self::DuelDamage => {
                "Left/Right: what every Sora-against-Sora hit's damage is multiplied by."
            }
            Self::CycleNetSimulator => {
                "Adds latency and loss to what this game sends. Set both games for a round trip."
            }
            Self::RemoveActor => "Remove this enemy from the room.",
            Self::ClearActors => "Remove every summoned enemy.",
            Self::Warp(..) => "Warp to this room.",
            Self::SaveStart => "The launcher starts this world in this room next time.",
        }
    }
}

struct Row {
    label: Vec<u8>,
    value: Vec<u8>,
    action: Action,
    enabled: bool,
}

impl Row {
    /// Whether the cursor can rest on the row.
    fn selectable(&self) -> bool {
        self.action != Action::Section
    }
}

/// How a row is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Look {
    Normal,
    Disabled,
    Section,
}

#[derive(Clone, Debug, PartialEq)]
struct Line {
    label: Vec<u8>,
    value: Vec<u8>,
    look: Look,
    /// An arrow on the right: the row opens a page.
    opens: bool,
    /// Arrows either side of the value when selected: Left / Right change it.
    adjustable: bool,
}

/// What the HUD draws while the menu is open or closing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    title: Vec<u8>,
    /// Where the page sits, on pages with no tabs.
    path: Vec<u8>,
    /// "4 / 31" on lists longer than the page.
    position: Vec<u8>,
    tabs: Vec<Vec<u8>>,
    tab_open: usize,
    /// The tab bar's place (it slides between tabs).
    tab_at: f32,
    /// Lines about the playing motion, above the rows.
    info: Vec<Vec<u8>>,
    /// The search line, on pages that take typed letters.
    search: Option<Vec<u8>>,
    lines: Vec<Line>,
    selected: Option<usize>,
    /// The cursor bar's place among the shown rows (it slides).
    cursor_at: f32,
    /// The scroll bar's thumb, as fractions of the list.
    scrollbar: Option<[f32; 2]>,
    description: Vec<u8>,
    /// The description line is a result notice.
    notice: bool,
    hints: Vec<u8>,
    /// 0 hidden .. 1 open (eased).
    appear: f32,
    /// A page sliding in: its offset as a fraction of the slide distance.
    slide: f32,
}

impl View {
    fn visible(&self) -> bool {
        self.appear > 0.
    }
}

#[derive(Resource, Default)]
pub struct DebugMenu {
    pub open: bool,
    page: Page,
    cursor: usize,
    top: usize,
    /// The last of the menu's own tabs and its cursor, for reopening.
    home: (HomeTab, usize),
    /// The arrow being held and for how many ticks.
    held: Option<(Dir, u32)>,
    /// The current world's saved launcher start (read on opening Warp).
    saved: Option<(u8, u8)>,
    notice: Option<(String, u32)>,
    /// Letters typed on a searchable page; only rows containing them stay.
    search: String,
    /// Motion: the panel's opening (0..1), the page slide (1..0) and the
    /// side it comes from, the cursor bar's row and the tab bar's tab.
    appear: f32,
    slide: f32,
    slide_from: f32,
    cursor_at: f32,
    tab_at: f32,
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
        self.go_to(
            Page::Preview(PreviewTab::ALL[tab % PreviewTab::ALL.len()]),
            0,
        );
        self.search = search.into();
    }

    /// Opens the menu's own page number `n` for the unattended tour: the
    /// Sora, Enemies, Warp and Help tabs, the end of Help, Items, the
    /// preview's Enemy tab, an Items search, Drive Form, Growth Abilities
    /// and the Heartless list.
    pub fn show_tour_page(&mut self, n: usize) {
        self.open = true;
        let (page, cursor, search) = match n {
            0 => (Page::Home(HomeTab::Sora), 0, ""),
            1 => (Page::Home(HomeTab::Enemies), 0, ""),
            2 => (Page::Home(HomeTab::Warp), 0, ""),
            3 => (Page::Home(HomeTab::Help), 0, ""),
            4 => (Page::Home(HomeTab::Help), usize::MAX, ""),
            5 => (Page::Items, 3, ""),
            6 => (Page::Preview(PreviewTab::Enemy), 0, ""),
            7 => (Page::Items, 0, "potion"),
            8 => (Page::Forms, 0, ""),
            9 => (Page::Growth, 0, ""),
            _ => (Page::Roster(enemy_catalog::Category::Heartless), 0, ""),
        };
        self.go_to(page, cursor);
        self.search = search.into();
    }

    /// Whether the menu is on screen (open, or still closing).
    pub fn visible(&self) -> bool {
        self.view.visible()
    }

    /// Opens `page` with the cursor on row `cursor` and no search.
    fn go_to(&mut self, page: Page, cursor: usize) {
        if let Some((_, open)) = page.tabs()
            && !page.same_strip(self.page)
        {
            self.tab_at = open as f32;
        }
        self.page = page;
        self.cursor = cursor;
        self.cursor_at = cursor as f32;
        self.top = 0;
        self.search.clear();
    }

    /// `go_to`, with the page sliding in from the right (`from` 1, deeper or
    /// the next tab) or the left (-1, back or the previous tab).
    fn slide_to(&mut self, page: Page, cursor: usize, from: f32) {
        self.go_to(page, cursor);
        self.slide = 1.;
        self.slide_from = from;
    }

    /// Where the menu reopens: the preview where it was left, otherwise the
    /// last of the menu's own tabs.
    fn reopen(&mut self, keep_preview: bool) {
        if keep_preview && matches!(self.page, Page::Preview(_)) {
            self.top = 0;
        } else {
            let (tab, cursor) = self.home;
            self.go_to(Page::Home(tab), cursor);
        }
        self.slide = 0.;
    }

    /// Advances the motion by `dt` seconds.
    fn animate(&mut self, dt: f32) {
        let (target, seconds) = if self.open {
            (1., OPEN_SECONDS)
        } else {
            (0., CLOSE_SECONDS)
        };
        self.appear = toward(self.appear, target, dt / seconds);
        self.slide = (self.slide - dt / SLIDE_SECONDS).max(0.);
        let k = 1. - (-dt / FOLLOW_SECONDS).exp();
        // A jump of more than a page (wrapping round) is not travelled.
        let cursor = self.cursor as f32;
        self.cursor_at = if (cursor - self.cursor_at).abs() > self.page.visible_rows() as f32 {
            cursor
        } else {
            follow(self.cursor_at, cursor, k)
        };
        if let Some((_, open)) = self.page.tabs() {
            self.tab_at = follow(self.tab_at, open as f32, k);
        }
    }

    /// The motion at rest (unattended runs capture still pages).
    fn settle_motion(&mut self) {
        self.appear = if self.open { 1. } else { 0. };
        self.slide = 0.;
        self.cursor_at = self.cursor as f32;
        if let Some((_, open)) = self.page.tabs() {
            self.tab_at = open as f32;
        }
    }
}

/// `at` moved toward `target` by at most `step`.
fn toward(at: f32, target: f32, step: f32) -> f32 {
    if at < target {
        (at + step).min(target)
    } else {
        (at - step).max(target)
    }
}

/// `at` moved the fraction `k` of the way to `target`; close enough lands.
fn follow(at: f32, target: f32, k: f32) -> f32 {
    let next = at + (target - at) * k;
    if (target - next).abs() < 0.01 {
        target
    } else {
        next
    }
}

/// Fast at first, gentle at the end.
fn ease(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
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
    /// F2, or the pad's Select / View button while the menu is open.
    help: bool,
    confirm: bool,
    back: bool,
    /// Backspace alone (it also counts as `back`).
    erase: bool,
    /// Tabs to move this frame: Tab or RB forward, Shift+Tab or LB back.
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
    ticks: u64,
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
        help: keys.just_pressed(KeyCode::F2),
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
        (Some(d), Some((h, held_for))) if d == h => {
            // One repeat when any of this frame's ticks reaches a repeat
            // point (several ticks in one slow frame still step once).
            let repeats = (held_for + 1..=held_for + ticks as u32)
                .any(|t| t >= REPEAT_DELAY && (t - REPEAT_DELAY).is_multiple_of(REPEAT_EVERY));
            if repeats {
                input.step = Some(d);
            }
            Some((d, held_for + ticks as u32))
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

/// The Help tab: what each key does and what each page is for. The field
/// hotkeys are the test room's (`hud_tick`, `finish`, `skate::toggle`), the
/// game controls its keyboard map (`visual::input`).
const HELP: &[(&str, &[(&str, &str)])] = &[
    (
        "Menu controls",
        &[
            ("Move the cursor", "Arrows / D-pad"),
            ("Select", "Enter, Space / A"),
            ("Back", "Esc, Backspace / B"),
            ("Change tab", "Tab, Left, Right / LB, RB"),
            ("Change a value", "Left, Right / D-pad"),
            ("Jump through a list", "PgUp, PgDn, Home, End"),
            ("Search a long list", "Type letters"),
            ("Help", "F2 / Select"),
            ("Open or close", "F3 / both sticks"),
        ],
    ),
    (
        "Pages",
        &[
            ("Sora", "Restore, level, cheats, items"),
            ("Enemies", "Battle enemy, summon, preview"),
            ("Enemy Preview", "Play and inspect motions"),
            ("Warp", "Any installed room"),
            ("Help", "This page"),
        ],
    ),
    (
        "Field hotkeys",
        &[
            ("Collision overlay on or off", "F1"),
            ("Debug menu help", "F2"),
            ("Debug menu", "F3"),
            ("Test blow hit on Sora, no damage", "F4"),
            ("HP -2 / +2, in battle F6 refills", "F5, F6"),
            ("MP -10 / +10, outside battle", "F7, F8"),
            ("Drive +10, outside battle", "F9"),
            ("Refill the Drive gauge", "F11"),
            ("Stand-in party for Drive forms", "F12"),
            ("Skateboard on or off", "J"),
            ("Leave the test room", "Esc"),
        ],
    ),
    (
        "Game controls",
        &[
            ("Move", "WASD"),
            ("Walk", "hold Left Alt"),
            ("Jump", "Space, Backspace"),
            ("Attack, decide", "Enter, left click"),
            ("Choose a command", "Up, Down, wheel"),
            ("Guard", "Right click, Right Shift"),
            ("Reaction, talk", "F, Right Ctrl"),
            ("Lock on", "R"),
            ("Shortcuts", "hold Left Shift"),
            ("First-person view", "N"),
            ("Turn the camera", "Mouse"),
            ("Recentre the camera", "Middle click, Right Alt"),
            ("Pause menu", "M"),
        ],
    ),
];

fn section(label: &str) -> Row {
    Row {
        label: text(label),
        value: Vec::new(),
        action: Action::Section,
        enabled: false,
    }
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
        Page::Home(HomeTab::Sora) => {
            let level = battle.map(|b| {
                let level = game.sora.vitals.level;
                (level, (level as usize) < b.sora_levels.len())
            });
            let drive = game.drive.as_ref();
            let form = drive
                .filter(|d| d.active)
                .and_then(|d| FORMS.iter().find(|&&(f, _)| f == d.form))
                .map_or("None", |&(_, name)| name);
            vec![
                section("Status"),
                row("Restore HP, MP and Drive", Action::Restore, true),
                Row {
                    value: level.map_or_else(Vec::new, |(l, _)| text(&format!("Lv {l}"))),
                    ..row("Level up", Action::LevelUp, level.is_some_and(|l| l.1))
                },
                Row {
                    value: next_bonus(game).map_or_else(Vec::new, |id| text(&format!("No. {id}"))),
                    ..row(
                        "Bonus level",
                        Action::BonusLevel,
                        next_bonus(game).is_some(),
                    )
                },
                row("Knock Sora out", Action::KnockOut, battle.is_some()),
                section("Cheats"),
                Row {
                    value: on_off(game.cheats.invincible),
                    ..row("Invincible", Action::ToggleInvincible, battle.is_some())
                },
                Row {
                    value: on_off(game.sora.player.abilities.has(enemy_target::SCAN_BIT)),
                    ..row(
                        "Scan",
                        Action::ToggleScan,
                        game.ability_slots
                            .iter()
                            .any(|s| s & 0x7fff == enemy_target::SCAN_ITEM),
                    )
                },
                section("Display"),
                Row {
                    value: on_off(game.perf_overlay),
                    ..row("Performance overlay", Action::TogglePerfOverlay, true)
                },
                Row {
                    value: on_off(game.net_overlay),
                    ..row("Network overlay", Action::ToggleNetOverlay, game.duel)
                },
                Row {
                    value: text(
                        game.net_simulated
                            .and_then(|p| duel_net::SIMULATED.get(p))
                            .map_or("Off", |s| s.0),
                    ),
                    ..row("Simulated network", Action::CycleNetSimulator, game.duel)
                },
                section("Duel"),
                Row {
                    value: text(&format!("x{}", game.duel_rules.damage)),
                    ..row(
                        "Damage",
                        Action::DuelDamage,
                        (game.duel || !game.rivals.is_empty()) && !game.net_client,
                    )
                },
                row(
                    "Scripted rival Sora",
                    Action::SpawnRival,
                    game.rivals.is_empty() && game.rival_kit.is_some(),
                ),
                row(
                    "Remove the rival",
                    Action::RemoveRival,
                    game.rivals
                        .iter()
                        .any(|r| r.control == rival::Control::Script),
                ),
                section("Items and forms"),
                row(
                    "Items",
                    Action::Page(Page::Items),
                    game.items.is_some() || camp.is_some(),
                ),
                Row {
                    value: if drive.is_some() {
                        text(form)
                    } else {
                        Vec::new()
                    },
                    ..row("Drive form", Action::Page(Page::Forms), drive.is_some())
                },
                row("Growth abilities", Action::Page(Page::Growth), true),
            ]
        }
        Page::Home(HomeTab::Enemies) => {
            use enemy_catalog::Category;
            let mut list = vec![
                section("Battle enemy"),
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
                section("Summon"),
            ];
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
            let selected = gallery.selected_actor();
            list.extend([
                section("Summoned"),
                Row {
                    value: text(&format!(
                        "{} / {}",
                        gallery.actors.len(),
                        enemy_gallery::MAX_ACTORS
                    )),
                    ..row(
                        "Summoned enemies",
                        Action::Page(Page::Actors),
                        !gallery.actors.is_empty(),
                    )
                },
                Row {
                    value: selected.map_or_else(|| text("None"), |a| enemy_name(gallery, a)),
                    ..row(
                        "Animation preview",
                        Action::Page(Page::Preview(PreviewTab::Playback)),
                        selected.is_some(),
                    )
                },
                row(
                    "Remove every summoned enemy",
                    Action::ClearActors,
                    !gallery.actors.is_empty(),
                ),
            ]);
            list
        }
        Page::Home(HomeTab::Warp) => {
            let Some(area) = area else {
                return vec![
                    section("Warp"),
                    row("Warping needs a game room loaded", Action::Info, false),
                ];
            };
            let busy = area.transition.is_some() || area.pending.is_some();
            let (world, here) = (area.current.world, area.current.area);
            let code = ::area::progress::WORLDS[world as usize];
            let saved = crate::launcher::field_start(code).map(|_| {
                saved.map_or_else(|| text("Default"), |(n, _)| text(&format!("{code}{n:02}")))
            });
            let mut list = vec![
                section("This world"),
                Row {
                    value: saved.clone().unwrap_or_default(),
                    ..row(
                        &format!("Start here next launch: {code}{here:02}"),
                        Action::SaveStart,
                        saved.is_some(),
                    )
                },
                section("Worlds"),
            ];
            list.extend(
                (0..::area::progress::WORLDS.len() as u8)
                    .filter(|&w| !area.source.rooms(w).is_empty())
                    .map(|w| Row {
                        value: if w == world {
                            text("Here")
                        } else {
                            text(&format!("{} rooms", area.source.rooms(w).len()))
                        },
                        ..row(area::world_name(w), Action::Page(Page::Rooms(w)), !busy)
                    }),
            );
            list
        }
        Page::Home(HomeTab::Help) => HELP
            .iter()
            .flat_map(|&(heading, entries)| {
                std::iter::once(section(heading)).chain(entries.iter().map(|&(what, keys)| Row {
                    value: text(keys),
                    ..row(what, Action::Info, true)
                }))
            })
            .collect(),
        Page::Items => {
            let mut list = vec![
                row("Fill every item", Action::FillItems, true),
                section("Stock"),
            ];
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
            let current = drive.filter(|d| d.active).map(|d| d.form);
            let mut list = vec![section("Forms")];
            list.extend(FORMS.iter().map(|&(form, name)| Row {
                value: if current == Some(form) {
                    text("Active")
                } else {
                    Vec::new()
                },
                ..row(name, Action::Form(form), idle)
            }));
            list.push(row(
                "Revert",
                Action::Form(0),
                drive.is_some_and(|d| d.active && d.change.is_none()),
            ));
            list
        }
        Page::Growth => {
            let mut list = vec![section("Growth abilities")];
            list.extend(
                game.ability_source
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
                    }),
            );
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
                label.extend(text(&format!(" No. {}", actor.id)));
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
                        section("Play"),
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
                        section("Jump"),
                        row("Jump to start", Action::JumpToPercent(0), playable),
                        row("Jump to middle", Action::JumpToPercent(50), playable),
                        row("Jump to end", Action::JumpToPercent(100), playable),
                        row("Replay from start", Action::Replay, playable),
                        section("Settings"),
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
                    section("Overlays"),
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
                    section("Motion"),
                    Row {
                        value: on_off(actor.apply_root),
                        ..row("Root motion", Action::ToggleRootMotion, controllable)
                    },
                ],
                PreviewTab::Enemy => vec![
                    section("Battle"),
                    Row {
                        value: text(&actor.battle.as_ref().map_or_else(
                            || "Preview only".into(),
                            |b| format!("HP {} {}", b.brain.hp(), b.brain.description()),
                        )),
                        ..row(
                            "Enable AI and combat",
                            Action::EnableBattle,
                            actor.battle.is_none()
                                && super::enemy_battle::supported(
                                    &gallery.catalog.entries[actor.entry],
                                ),
                        )
                    },
                    row(
                        "Defeat this enemy",
                        Action::DefeatSummoned,
                        actor.battle.is_some(),
                    ),
                    row(
                        "Respawn this enemy",
                        Action::RespawnSummoned,
                        actor.battle.is_some(),
                    ),
                    section("Enemy"),
                    row("Remove this enemy", Action::RemoveActor, true),
                    row("Choose another enemy", Action::Page(Page::Actors), true),
                ],
            }
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

/// `rows`, narrowed to the ones containing the typed search (section
/// headings go while a search is typed).
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
            row.selectable()
                && menu
                    .search
                    .split_whitespace()
                    .all(|word| words.contains(word))
        });
    }
    list
}

/// The nearest row the cursor can rest on: `cursor` itself, the next one
/// down, or failing that the next one up.
fn settle(list: &[Row], cursor: usize) -> usize {
    let cursor = cursor.min(list.len().saturating_sub(1));
    (cursor..list.len())
        .chain((0..cursor).rev())
        .find(|&i| list[i].selectable())
        .unwrap_or(cursor)
}

/// The row `by` selectable rows along from `cursor`, wrapping round.
fn step_cursor(list: &[Row], cursor: usize, by: i32) -> usize {
    let n = list.len() as i32;
    let mut at = cursor as i32;
    for _ in 0..n {
        at = (at + by.signum()).rem_euclid(n);
        if list[at as usize].selectable() {
            return at as usize;
        }
    }
    cursor
}

/// Whether row action `action` leads to `page` (the cursor lands on it
/// when Back returns from `page`).
fn leads_to(action: Action, page: Page, selected: Option<u64>) -> bool {
    match (action, page) {
        (Action::SelectActor(id), Page::Preview(_)) => Some(id) == selected,
        (Action::Page(Page::Preview(_)), Page::Preview(_)) => true,
        (Action::Page(to), page) => to == page,
        _ => false,
    }
}

/// What a newly shown page needs: Warp reads the world's saved start, and
/// the current world's rooms open on the current room.
fn arrive(menu: &mut DebugMenu, area: Option<&AreaRes>) {
    let Some(area) = area else { return };
    match menu.page {
        Page::Home(HomeTab::Warp) => {
            let code = ::area::progress::WORLDS[area.current.world as usize];
            menu.saved = crate::launcher::saved_start(&crate::launcher::data_dir(), code);
        }
        Page::Rooms(world) if world == area.current.world && menu.cursor == 0 => {
            let rooms = area.source.rooms(world);
            menu.cursor = rooms
                .iter()
                .position(|&room| room == area.current.area)
                .unwrap_or(0);
            menu.cursor_at = menu.cursor as f32;
        }
        _ => {}
    }
}

/// Runs a row's action; returns a line for the description bar.
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
        Action::Info | Action::Section => None,
        Action::ToggleScan => {
            let enabled = !game.sora.player.abilities.has(enemy_target::SCAN_BIT);
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
            menu.slide_to(page, 0, 1.);
            arrive(menu, area.as_deref());
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
        Action::Restore => {
            if game.enemy.is_some() {
                let vitals = &mut game.sora.vitals;
                vitals.hp = vitals.target.max_hp;
                vitals.ko_ticks = 0;
            } else if let Some(hud) = hud {
                hud.stats.hp = hud.stats.max_hp;
            }
            let mp = &mut game.sora.player.mp;
            (mp.cur, mp.remaining, mp.total) = (mp.max, 0., 0.);
            game.sora.player.mp_charging = false;
            // In a form the gauge is the form's timer; leave it running.
            if game.drive.as_ref().is_none_or(|d| !d.active) {
                game.sora.player.drive.refill();
            }
            Some("HP, MP and Drive restored".into())
        }
        Action::KnockOut => {
            // Sora's HP reaches 0: the DEAD motion and the game over.
            let Game {
                sora, clips, enemy, ..
            } = game;
            enemy.as_ref()?;
            sora.vitals.hp = 0;
            sora.player.die(clips);
            menu.open = false;
            None
        }
        Action::LevelUp => {
            let tick = game.tick;
            let Game {
                enemy, sora, log, ..
            } = game;
            let battle = enemy.as_mut()?;
            let next = battle.sora_levels.get(sora.vitals.level as usize - 1)?.exp;
            let need = next.saturating_sub(battle.exp_total).max(1);
            prize_room::grant_exp(battle, &mut sora.vitals, need, log, tick);
            Some(format!("Level {}", sora.vitals.level))
        }
        Action::BonusLevel => {
            // A battle's bonus level (FUN_1403edca0): the next one Sora has
            // a row in.
            let id = next_bonus(game)?;
            let bonus_table = game.drive.as_ref()?.src.growth.bons.clone();
            let Game { enemy, sora, .. } = game;
            super::level_growth::apply_bonus(enemy.as_mut()?, sora, &bonus_table, id).ok()?;
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
            game.sora.player.drive.refill();
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
            Some("Growth level changed. Equip it in Abilities".into())
        }
        Action::Summon(entry) => {
            let from = Vec3::from_array(game.sora.player.position);
            let yaw = game.sora.player.yaw;
            let at = from + Quat::from_rotation_y(yaw) * Vec3::new(0., 0., 350.);
            gallery.queue_summon(entry, at, yaw + std::f32::consts::PI);
            menu.open = false;
            Some("Loading selected enemy".into())
        }
        Action::SelectActor(id) => {
            gallery.select(id);
            menu.slide_to(Page::Preview(PreviewTab::Playback), 0, 1.);
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
        Action::TogglePerfOverlay => {
            game.perf_overlay = !game.perf_overlay;
            None
        }
        Action::DuelDamage => {
            let (lo, hi) = duel_loadout::DAMAGE_RANGE;
            let d = &mut game.duel_rules.damage;
            *d = (*d + step as f32 * duel_loadout::DAMAGE_STEP).clamp(lo, hi);
            Some(format!("Duel damage x{}", *d))
        }
        Action::ToggleNetOverlay => {
            game.net_overlay = !game.net_overlay;
            None
        }
        Action::CycleNetSimulator => {
            let next = game
                .net_simulated
                .map_or(1, |p| (p + 1) % duel_net::SIMULATED.len());
            game.net_simulated = Some(next);
            Some(format!(
                "Simulated network: {}",
                duel_net::SIMULATED[next].0
            ))
        }
        Action::SpawnRival => {
            rival::spawn_scripted(game);
            menu.open = false;
            Some("A scripted rival Sora joined".into())
        }
        Action::RemoveRival => {
            game.rivals.retain(|r| r.control != rival::Control::Script);
            Some("The rival left".into())
        }
        Action::RemoveActor => {
            gallery.remove_selected();
            menu.slide_to(Page::Actors, 0, -1.);
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

/// The row to start on when a tab opens: on the preview, the motion now
/// playing.
fn tab_row(
    page: Page,
    menu: &DebugMenu,
    game: &Game,
    area: Option<&AreaRes>,
    camp: Option<&CampRes>,
    gallery: &enemy_gallery::EnemyGallery,
) -> usize {
    let list = rows(page, menu.saved, game, area, camp, gallery);
    let playing = gallery.selected_actor().and_then(|a| a.motion_choice);
    let at = match (page, playing) {
        (Page::Preview(_), Some(choice)) => list
            .iter()
            .position(|row| row.action == Action::PlayMotion(choice))
            .unwrap_or(0),
        _ => 0,
    };
    settle(&list, at)
}

/// Plays se000 menu sounds through the mixer, as the camp does.
fn play_sounds(sounds: &[u32], audio: Option<&mut Audio>, game: &mut Game) {
    let Some(audio) = audio else { return };
    for &n in sounds {
        game.event(
            serde_json::json!({"event":"sound","tick":game.tick,"id":n,"cause":"debug_menu"}),
        );
        let Audio { output, banks, .. } = &mut *audio;
        if let (Some(output), Ok(Some(r))) = (output.as_ref(), banks.request(n))
            && let Ok(mut mixer) = output.mixer.lock()
        {
            for layer in r.layers {
                mixer.play(layer, n, r.volume, 1.);
            }
        }
    }
}

/// Opens, drives and closes the menu (Update, so presses are not missed
/// between fixed ticks).
#[allow(clippy::too_many_arguments)]
pub fn tick(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut key_events: MessageReader<KeyboardInput>,
    time: Res<Time>,
    real: Res<Time<Real>>,
    clock: Res<visual::smoothing::TickClock>,
    pads: Query<(&Gamepad, Option<&Name>)>,
    mut menu: ResMut<DebugMenu>,
    mut game: ResMut<Game>,
    mut area: Option<ResMut<AreaRes>>,
    mut camp: Option<ResMut<CampRes>>,
    mut hud: Option<ResMut<Hud>>,
    mut audio: Option<ResMut<Audio>>,
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
            menu.settle_motion();
            menu.view = build_view(&mut menu, &game, area.as_deref(), camp.as_deref(), &gallery);
            if let Some(h) = hud.as_deref_mut() {
                h.updated = true;
            }
        }
        return;
    }
    let menu = &mut *menu;
    let input = read_input(&keys, &pads, &mut menu.held, clock.ticks_this_frame());
    let was_open = menu.open;
    let mut sounds = Vec::new();
    // Esc backs out of the menu; the room must not also take it as exit.
    if was_open {
        keys.clear_just_pressed(KeyCode::Escape);
    }
    if !menu.open {
        let blocked = camp.as_ref().is_some_and(|c| c.menu_up()) || game.talking;
        if (input.toggle || input.help) && !blocked {
            menu.open = true;
            sounds.push(SOUND_OPEN);
            if input.help {
                menu.go_to(Page::Home(HomeTab::Help), 0);
            } else {
                menu.reopen(gallery.selected_actor().is_some());
            }
            arrive(menu, area.as_deref());
        }
    } else if input.toggle {
        menu.open = false;
        sounds.push(SOUND_CLOSE);
    } else {
        // The pad's Select / View button opens Help while the menu is up.
        let help = input.help
            || pads
                .iter()
                .any(|(p, _)| p.just_pressed(GamepadButton::Select));
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
        menu.cursor = settle(&list, menu.cursor);
        let before = menu.cursor;
        if count > 0 {
            if keys.just_pressed(KeyCode::PageDown) {
                menu.cursor = settle(&list, (menu.cursor + visible).min(count - 1));
            } else if keys.just_pressed(KeyCode::PageUp) {
                menu.cursor = settle(&list, menu.cursor.saturating_sub(visible));
            } else if keys.just_pressed(KeyCode::Home) {
                menu.cursor = settle(&list, 0);
            } else if keys.just_pressed(KeyCode::End) {
                menu.cursor = settle(&list, count - 1);
            }
        }
        let mut run = None;
        let mut tab = input.tab;
        match input.step {
            Some(Dir::Up) if count > 0 => menu.cursor = step_cursor(&list, menu.cursor, -1),
            Some(Dir::Down) if count > 0 => menu.cursor = step_cursor(&list, menu.cursor, 1),
            Some(d @ (Dir::Left | Dir::Right)) => {
                let direction = if d == Dir::Left { -1 } else { 1 };
                match list.get(menu.cursor).filter(|r| r.action.adjustable()) {
                    Some(row) if row.enabled => run = Some((row.action, direction)),
                    Some(_) => sounds.push(SOUND_INVALID),
                    // Rows with no value to change leave Left and Right free
                    // for the tabs.
                    None => tab = direction,
                }
            }
            _ => {}
        }
        if menu.cursor != before {
            sounds.push(SOUND_CURSOR);
        }
        let selected = list.get(menu.cursor);
        if help && menu.page != Page::Home(HomeTab::Help) {
            menu.slide_to(Page::Home(HomeTab::Help), 0, 1.);
            sounds.push(SOUND_CURSOR);
        } else if tab != 0
            && let Some(next) = menu.page.next_tab(tab)
        {
            let cursor = tab_row(
                next,
                menu,
                &game,
                area.as_deref(),
                camp.as_deref(),
                &gallery,
            );
            menu.slide_to(next, cursor, tab.signum() as f32);
            arrive(menu, area.as_deref());
            sounds.push(SOUND_CURSOR);
        } else if input.confirm
            && let Some(row) = selected
        {
            if row.enabled && !matches!(row.action, Action::Info | Action::Section) {
                run = Some((row.action, 1));
                sounds.push(SOUND_DECIDE);
            } else if !row.enabled {
                sounds.push(SOUND_INVALID);
            }
        } else if back {
            sounds.push(SOUND_CLOSE);
            match menu.page.parent() {
                None => menu.open = false,
                Some(up) => {
                    let from = menu.page;
                    let chosen = gallery.selected_actor().map(|a| a.id);
                    let list = rows(
                        up,
                        menu.saved,
                        &game,
                        area.as_deref(),
                        camp.as_deref(),
                        &gallery,
                    );
                    let cursor = list
                        .iter()
                        .position(|r| leads_to(r.action, from, chosen))
                        .unwrap_or_else(|| settle(&list, 0));
                    menu.slide_to(up, cursor, -1.);
                    arrive(menu, area.as_deref());
                }
            }
        }
        if let Some((action, step)) = run {
            if input.step.is_some() {
                sounds.push(SOUND_CURSOR);
            }
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
    if let Page::Home(tab) = menu.page {
        menu.home = (tab, menu.cursor);
    }
    play_sounds(&sounds, audio.as_deref_mut(), &mut game);
    // The preview keeps playing behind its own tabs (the field is paused
    // while the menu is open, so its clock does not).
    if menu.open && matches!(menu.page, Page::Preview(_)) {
        enemy_gallery::advance(&mut gallery, time.delta_secs() * TICKS_PER_SECOND);
    }
    if let Some((_, frames)) = menu.notice.as_mut() {
        // Counted in ticks, so the notice lasts as long at any frame rate.
        *frames = frames.saturating_sub(clock.ticks_this_frame() as u32);
        if *frames == 0 {
            menu.notice = None;
        }
    }
    if !was_open && !menu.open && !menu.view.visible() {
        return;
    }
    menu.animate(real.delta_secs().min(0.1));
    let view = build_view(menu, &game, area.as_deref(), camp.as_deref(), &gallery);
    if view != menu.view {
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
    if !menu.open && menu.appear <= 0. {
        return View::default();
    }
    let list = listed(menu, game, area, camp, gallery);
    let visible = menu.page.visible_rows();
    let settled = settle(&list, menu.cursor);
    // A page opening on a heading starts on the row below it, not sliding.
    if menu.cursor_at == menu.cursor as f32 {
        menu.cursor_at = settled as f32;
    }
    menu.cursor = settled;
    menu.top = scroll(menu.cursor, menu.top, visible);
    // A heading just above the cursor's row comes into view with it.
    if menu.top == menu.cursor && menu.top > 0 && !list[menu.top - 1].selectable() {
        menu.top -= 1;
    }
    let selected = list.get(menu.cursor);
    let (description, notice) = match &menu.notice {
        Some((notice, _)) => (notice.clone(), true),
        None if menu.page == Page::Home(HomeTab::Help) => (
            "Menu controls: the keyboard key, then the controller button after /.".into(),
            false,
        ),
        None => (
            selected.map_or("", |r| r.action.describe()).to_string(),
            false,
        ),
    };
    let (tabs, tab_open) = menu.page.tabs().unwrap_or_default();
    let info = match menu.page {
        Page::Preview(_) => preview_info(gallery, gallery.selected_actor()),
        _ => Vec::new(),
    };
    let search = menu.page.searchable().then(|| {
        text(&if menu.search.is_empty() {
            "Search: type to filter".to_string()
        } else {
            format!("Search: {}   - {} found", menu.search, list.len())
        })
    });
    let long = list.len() > visible;
    let scrollbar = long.then(|| {
        let total = list.len() as f32;
        [
            menu.top as f32 / total,
            ((menu.top + visible) as f32 / total).min(1.),
        ]
    });
    let position = if long {
        text(&format!("{} / {}", menu.cursor + 1, list.len()))
    } else {
        Vec::new()
    };
    let shown = list.len().saturating_sub(menu.top).min(visible);
    View {
        title: text(&menu.page.title()),
        path: text(&menu.page.path()),
        position,
        tabs: tabs.into_iter().map(text).collect(),
        tab_open,
        tab_at: menu.tab_at,
        info,
        search,
        lines: list
            .iter()
            .skip(menu.top)
            .take(visible)
            .map(|r| Line {
                label: r.label.clone(),
                value: r.value.clone(),
                look: if !r.selectable() {
                    Look::Section
                } else if r.enabled {
                    Look::Normal
                } else {
                    Look::Disabled
                },
                opens: r.action.opens_page() && r.enabled,
                adjustable: r.action.adjustable() && r.enabled,
            })
            .collect(),
        selected: (shown > 0).then(|| menu.cursor - menu.top),
        cursor_at: (menu.cursor_at - menu.top as f32).clamp(0., shown.saturating_sub(1) as f32),
        scrollbar,
        description: text(&description),
        notice,
        hints: text(menu.page.hints()),
        appear: ease(menu.appear),
        slide: menu.slide_from * (1. - ease(1. - menu.slide)),
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

fn quad(corners: [[f32; 2]; 4], colors: [[f32; 4]; 4]) -> sprite_sequence::Quad {
    sprite_sequence::Quad {
        corners,
        uv: [0, 0, 1, 1],
        colors,
        blend: 0,
    }
}

fn rect(x0: f32, y0: f32, x1: f32, y1: f32, rgba: [f32; 4]) -> sprite_sequence::Quad {
    quad([[x0, y0], [x1, y0], [x0, y1], [x1, y1]], [rgba; 4])
}

/// A rectangle shaded from `top` to `bottom`.
fn shade_down(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    top: [f32; 4],
    bottom: [f32; 4],
) -> sprite_sequence::Quad {
    quad(
        [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
        [top, top, bottom, bottom],
    )
}

/// A rectangle shaded from `left` to `right`.
fn shade_across(
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    left: [f32; 4],
    right: [f32; 4],
) -> sprite_sequence::Quad {
    quad(
        [[x0, y0], [x1, y0], [x0, y1], [x1, y1]],
        [left, right, left, right],
    )
}

/// A small arrow with its flat side at `x`, centred on `y`, pointing right
/// or left (two corners meet at the tip).
fn arrow(x: f32, y: f32, right: bool, rgba: [f32; 4]) -> sprite_sequence::Quad {
    let tip = if right { x + 4. } else { x - 4. };
    quad([[x, y - 4.], [tip, y], [x, y + 4.], [tip, y]], [rgba; 4])
}

/// `rgba` with its alpha scaled by `alpha`.
fn fade(rgba: [f32; 4], alpha: f32) -> [f32; 4] {
    [rgba[0], rgba[1], rgba[2], rgba[3] * alpha]
}

/// The menu's draws: the panel, bars and arrows on `white`, the text on the
/// system font sheet `font_sheet`.
pub fn draws(view: &View, white: usize, font_sheet: usize, widths: &[u8]) -> Vec<Draw> {
    let font = field_command_menu::Font { widths, pad: 1 };
    let width = |text: &[u8], size: f32| field_command_menu::text_width(text, &font, size);
    // The size that fits `text` in `room`, at most `size`.
    let fit = |text: &[u8], size: f32, room: f32| {
        size.min(size * room.max(1.) / width(text, size).max(1.))
    };
    // The top of a glyph cell centred in a line `height` tall.
    let line = |y: f32, height: f32, size: f32| y + (height - 24. * size / 18.) / 2.;
    let appear = view.appear;
    let dx = -OPEN_DISTANCE * (1. - appear);
    let [x0, y0, x1, y1] = BOX;
    let (x0, x1) = (x0 + dx, x1 + dx);
    let inner = x1 - x0 - MARGIN * 2.;
    let panel = |c: [f32; 4]| fade(c, appear);
    // The list part slides and fades as a page comes in.
    let cx = dx + view.slide * SLIDE_DISTANCE;
    let ink = |c: [f32; 4]| fade(c, appear * (1. - view.slide.abs() * 0.75));
    let mut shapes = Vec::new();
    let mut glyphs = Vec::new();
    let say = |glyphs: &mut Vec<_>, s: &[u8], at: [f32; 2], rgba: [f32; 4], size: f32| {
        glyphs.extend(field_command_menu::text_quads_sized(
            s, at, rgba, &font, size,
        ));
    };
    // The panel: a light edge round a blue body, a lighter wash behind the
    // title and rules under the tab strip and over the description.
    shapes.push(rect(x0, y0, x1, y1, panel(BORDER)));
    shapes.push(shade_down(
        x0 + 1.,
        y0 + 1.,
        x1 - 1.,
        y1 - 1.,
        panel(PANEL_TOP),
        panel(PANEL_BOTTOM),
    ));
    shapes.push(shade_down(
        x0 + 1.,
        y0 + 1.,
        x1 - 1.,
        STRIP_Y,
        panel(BAND_TOP),
        panel(BAND_BOTTOM),
    ));
    for y in [STRIP_Y + ROW_HEIGHT + 3., DESCRIPTION_Y - 4.] {
        shapes.push(shade_across(
            x0 + MARGIN,
            y,
            x1 - MARGIN,
            y + 1.,
            panel(RULE),
            panel(fade(RULE, 0.)),
        ));
    }
    say(
        &mut glyphs,
        &view.title,
        [x0 + MARGIN, line(TITLE_Y, TITLE_HEIGHT, TITLE_SIZE)],
        panel(TEXT_CHOSEN),
        fit(&view.title, TITLE_SIZE, inner * 0.7),
    );
    if !view.position.is_empty() {
        say(
            &mut glyphs,
            &view.position,
            [
                x1 - MARGIN - width(&view.position, SMALL_SIZE),
                line(TITLE_Y, TITLE_HEIGHT, SMALL_SIZE),
            ],
            panel(HINT),
            SMALL_SIZE,
        );
    }
    // The tab strip: the open tab on a bar that slides between tabs; pages
    // with no tabs show where they sit instead.
    if !view.tabs.is_empty() {
        let mut spans = Vec::new();
        let mut x = x0 + MARGIN + 6.;
        for label in &view.tabs {
            let w = width(label, TEXT_SIZE);
            spans.push((x, w));
            x += w + TAB_GAP;
        }
        let at = view.tab_at.clamp(0., (spans.len() - 1) as f32);
        let (from, to) = (
            at.floor() as usize,
            (at.ceil() as usize).min(spans.len() - 1),
        );
        let f = at - from as f32;
        let bar_x = spans[from].0 + (spans[to].0 - spans[from].0) * f;
        let bar_w = spans[from].1 + (spans[to].1 - spans[from].1) * f;
        shapes.push(shade_down(
            bar_x - 6.,
            STRIP_Y,
            bar_x + bar_w + 6.,
            STRIP_Y + ROW_HEIGHT,
            panel(TAB_TOP),
            panel(TAB_BOTTOM),
        ));
        shapes.push(rect(
            bar_x - 6.,
            STRIP_Y + ROW_HEIGHT - 1.,
            bar_x + bar_w + 6.,
            STRIP_Y + ROW_HEIGHT,
            panel(CURSOR_EDGE),
        ));
        for (k, (label, &(x, _))) in view.tabs.iter().zip(&spans).enumerate() {
            let colour = if k == view.tab_open {
                TEXT_CHOSEN
            } else {
                TAB_DIM
            };
            say(
                &mut glyphs,
                label,
                [x, line(STRIP_Y, ROW_HEIGHT, TEXT_SIZE)],
                panel(colour),
                TEXT_SIZE,
            );
        }
    } else if !view.path.is_empty() {
        say(
            &mut glyphs,
            &view.path,
            [x0 + MARGIN, line(STRIP_Y, ROW_HEIGHT, SMALL_SIZE)],
            panel(HINT),
            fit(&view.path, SMALL_SIZE, inner),
        );
    }
    // Lines above the rows: the playing motion and the search.
    let mut top = BODY_Y;
    for info in &view.info {
        say(
            &mut glyphs,
            info,
            [x0 + MARGIN + cx - dx, line(top, ROW_HEIGHT, TEXT_SIZE - 1.)],
            ink(INFO),
            fit(info, TEXT_SIZE - 1., inner),
        );
        top += ROW_HEIGHT;
    }
    if let Some(search) = &view.search {
        say(
            &mut glyphs,
            search,
            [x0 + MARGIN + cx - dx, line(top, ROW_HEIGHT, TEXT_SIZE - 1.)],
            ink(SEARCH),
            fit(search, TEXT_SIZE - 1., inner),
        );
        top += ROW_HEIGHT;
    }
    // The cursor bar: bright at its left edge, fading to the right.
    let (left, right) = (x0 + 5. + cx - dx, x1 - 12. + cx - dx);
    if view.selected.is_some() {
        let y = top + view.cursor_at * ROW_HEIGHT;
        shapes.push(shade_across(
            left,
            y,
            right,
            y + ROW_HEIGHT,
            ink(CURSOR_LEFT),
            ink(CURSOR_RIGHT),
        ));
        shapes.push(shade_across(
            left,
            y,
            right,
            y + 1.,
            ink(fade(CURSOR_EDGE, 0.6)),
            ink(fade(CURSOR_EDGE, 0.)),
        ));
        shapes.push(rect(left, y, left + 2., y + ROW_HEIGHT, ink(CURSOR_EDGE)));
    }
    let label_x = x0 + MARGIN + INDENT + cx - dx;
    let value_right = x1 - MARGIN - ARROW_ROOM + cx - dx;
    for (i, row) in view.lines.iter().enumerate() {
        let y = top + i as f32 * ROW_HEIGHT;
        let mid = y + ROW_HEIGHT / 2.;
        if row.look == Look::Section {
            let size = TEXT_SIZE - 1.;
            let x = x0 + MARGIN + cx - dx;
            say(
                &mut glyphs,
                &row.label,
                [x, line(y, ROW_HEIGHT, size)],
                ink(SECTION),
                size,
            );
            let end = x + width(&row.label, size) + 6.;
            shapes.push(shade_across(
                end,
                mid,
                value_right,
                mid + 1.,
                ink(fade(SECTION, 0.5)),
                ink(fade(SECTION, 0.)),
            ));
            continue;
        }
        let chosen = view.selected == Some(i);
        let (label_colour, value_colour) = match (row.look, chosen) {
            (Look::Disabled, false) => (TEXT_DIM, TEXT_DIM),
            (Look::Disabled, true) => (TEXT_DIM_CHOSEN, TEXT_DIM_CHOSEN),
            (_, false) => (TEXT, VALUE),
            (_, true) => (TEXT_CHOSEN, TEXT_CHOSEN),
        };
        let value_w = width(&row.value, TEXT_SIZE);
        let arrows = chosen && row.adjustable;
        if !row.value.is_empty() {
            say(
                &mut glyphs,
                &row.value,
                [value_right - value_w, line(y, ROW_HEIGHT, TEXT_SIZE)],
                ink(value_colour),
                TEXT_SIZE,
            );
        }
        if arrows {
            shapes.push(arrow(value_right - value_w - 4., mid, false, ink(MARK)));
            shapes.push(arrow(value_right + 5., mid, true, ink(MARK)));
        } else if row.opens {
            shapes.push(arrow(value_right + 5., mid, true, ink(fade(MARK, 0.75))));
        }
        let room = value_right - value_w - if arrows { 10. } else { 0. } - 8. - label_x;
        say(
            &mut glyphs,
            &row.label,
            [label_x, line(y, ROW_HEIGHT, TEXT_SIZE)],
            ink(label_colour),
            fit(&row.label, TEXT_SIZE, room),
        );
    }
    // The scroll bar beside a list longer than the page.
    if let Some([from, to]) = view.scrollbar {
        let track =
            (LINES - view.info.len() - usize::from(view.search.is_some())) as f32 * ROW_HEIGHT;
        let x = x1 - 8.;
        shapes.push(rect(x, top, x + 3., top + track, panel(fade(BORDER, 0.2))));
        shapes.push(rect(
            x,
            top + track * from,
            x + 3.,
            top + track * to,
            panel(BORDER),
        ));
    }
    say(
        &mut glyphs,
        &view.description,
        [x0 + MARGIN, line(DESCRIPTION_Y, ROW_HEIGHT, TEXT_SIZE - 1.)],
        panel(if view.notice { SEARCH } else { DESCRIPTION }),
        fit(&view.description, TEXT_SIZE - 1., inner),
    );
    say(
        &mut glyphs,
        &view.hints,
        [x0 + MARGIN, line(HINTS_Y, ROW_HEIGHT, SMALL_SIZE)],
        panel(HINT),
        fit(&view.hints, SMALL_SIZE, inner),
    );
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

    fn test_row(action: Action) -> Row {
        Row {
            label: text("Row"),
            value: Vec::new(),
            action,
            enabled: true,
        }
    }

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
    fn tabs_wrap_in_both_directions() {
        let preview = |t| Page::Preview(t);
        assert_eq!(
            preview(PreviewTab::Playback).next_tab(1),
            Some(preview(PreviewTab::Motions))
        );
        assert_eq!(
            preview(PreviewTab::Playback).next_tab(-1),
            Some(preview(PreviewTab::Enemy))
        );
        assert_eq!(
            preview(PreviewTab::View).next_tab(5),
            Some(preview(PreviewTab::View))
        );
        assert_eq!(
            Page::Home(HomeTab::Help).next_tab(1),
            Some(Page::Home(HomeTab::Sora))
        );
        assert_eq!(
            Page::Home(HomeTab::Sora).next_tab(-1),
            Some(Page::Home(HomeTab::Help))
        );
        assert_eq!(Page::Items.next_tab(1), None);
    }

    #[test]
    fn back_climbs_to_the_page_that_opened_it() {
        assert_eq!(Page::Home(HomeTab::Warp).parent(), None);
        assert_eq!(Page::Items.parent(), Some(Page::Home(HomeTab::Sora)));
        assert_eq!(Page::Rooms(2).parent(), Some(Page::Home(HomeTab::Warp)));
        assert_eq!(Page::Preview(PreviewTab::View).parent(), Some(Page::Actors));
        assert_eq!(Page::Actors.parent(), Some(Page::Home(HomeTab::Enemies)));
        assert!(leads_to(Action::Page(Page::Rooms(2)), Page::Rooms(2), None));
        assert!(!leads_to(
            Action::Page(Page::Rooms(3)),
            Page::Rooms(2),
            None
        ));
        assert!(leads_to(
            Action::SelectActor(4),
            Page::Preview(PreviewTab::Motions),
            Some(4)
        ));
        assert!(!leads_to(
            Action::SelectActor(5),
            Page::Preview(PreviewTab::Motions),
            Some(4)
        ));
    }

    #[test]
    fn header_lines_leave_room_for_the_rows() {
        assert_eq!(Page::Home(HomeTab::Sora).visible_rows(), LINES);
        assert_eq!(Page::Home(HomeTab::Warp).visible_rows(), LINES - 1);
        assert_eq!(Page::Items.visible_rows(), LINES - 1);
        assert_eq!(
            Page::Preview(PreviewTab::Playback).visible_rows(),
            LINES - 2
        );
        assert_eq!(Page::Preview(PreviewTab::Motions).visible_rows(), LINES - 3);
    }

    #[test]
    fn the_cursor_passes_over_section_headings() {
        let list = [
            test_row(Action::Section),
            test_row(Action::Restore),
            test_row(Action::Section),
            test_row(Action::LevelUp),
            test_row(Action::KnockOut),
        ];
        assert_eq!(settle(&list, 0), 1);
        assert_eq!(settle(&list, 2), 3);
        assert_eq!(step_cursor(&list, 1, 1), 3);
        assert_eq!(step_cursor(&list, 3, -1), 1);
        // Wrapping skips the heading at the top.
        assert_eq!(step_cursor(&list, 4, 1), 1);
        assert_eq!(step_cursor(&list, 1, -1), 4);
        // A list of headings only leaves the cursor where it is.
        let headings = [test_row(Action::Section), test_row(Action::Section)];
        assert_eq!(step_cursor(&headings, 0, 1), 0);
        assert_eq!(settle(&[], 3), 0);
    }

    #[test]
    fn every_page_row_has_a_description() {
        use enemy_catalog::Category;
        for action in [
            Action::Page(Page::Items),
            Action::Page(Page::Forms),
            Action::Page(Page::Growth),
            Action::Page(Page::Roster(Category::Heartless)),
            Action::Page(Page::Roster(Category::Nobody)),
            Action::Page(Page::Roster(Category::Unresolved)),
            Action::Page(Page::Actors),
            Action::Page(Page::Preview(PreviewTab::Playback)),
            Action::Page(Page::Rooms(2)),
            Action::Restore,
            Action::ToggleScan,
            Action::Warp(2, 1),
            Action::PlaybackSpeed,
        ] {
            assert!(!action.describe().is_empty(), "{action:?}");
        }
    }

    /// Whether the menu font can show every character of `s`.
    fn printable(s: &str) -> bool {
        s.chars()
            .all(|c| c == ' ' || text(&c.to_string())[0] != 0x01)
    }

    #[test]
    fn every_line_uses_characters_the_font_has() {
        let pages = [
            Page::Home(HomeTab::Sora),
            Page::Home(HomeTab::Help),
            Page::Home(HomeTab::Warp),
            Page::Items,
            Page::Forms,
            Page::Growth,
            Page::Actors,
            Page::Roster(enemy_catalog::Category::Unresolved),
            Page::Preview(PreviewTab::Playback),
            Page::Preview(PreviewTab::Motions),
            Page::Rooms(4),
        ];
        for page in pages {
            for s in [page.title(), page.path(), page.hints().to_string()] {
                assert!(printable(&s), "{s}");
            }
        }
        for action in [
            Action::Page(Page::Actors),
            Action::Page(Page::Growth),
            Action::Item(1),
            Action::Growth(0),
            Action::KnockOut,
            Action::Form(0),
            Action::SaveStart,
        ] {
            assert!(printable(action.describe()), "{action:?}");
        }
    }

    #[test]
    fn help_lists_every_field_hotkey() {
        let keys: Vec<&str> = HELP
            .iter()
            .find(|(heading, _)| *heading == "Field hotkeys")
            .map(|(_, rows)| rows.iter().map(|&(_, keys)| keys).collect())
            .unwrap_or_default();
        for key in [
            "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F11", "F12", "J", "Esc",
        ] {
            assert!(
                keys.iter().any(|k| k.split(", ").any(|part| part == key)),
                "{key} missing from Help"
            );
        }
        // Every help line uses only characters the menu font has.
        for (heading, rows) in HELP {
            for s in std::iter::once(*heading).chain(rows.iter().flat_map(|&(a, b)| [a, b])) {
                assert!(printable(s), "{s}");
            }
        }
    }

    #[test]
    fn motion_settles_quickly() {
        let mut menu = DebugMenu {
            open: true,
            cursor: 6,
            ..DebugMenu::default()
        };
        menu.slide = 1.;
        // Two frames at 60 fps are not yet open; after the opening time
        // it is.
        menu.animate(1. / 60.);
        menu.animate(1. / 60.);
        assert!(menu.appear > 0. && menu.appear < 1.);
        assert!(menu.cursor_at > 0. && menu.cursor_at < 6.);
        for _ in 0..12 {
            menu.animate(1. / 60.);
        }
        assert_eq!(menu.appear, 1.);
        assert_eq!(menu.slide, 0.);
        assert_eq!(menu.cursor_at, 6.);
        menu.open = false;
        for _ in 0..5 {
            menu.animate(1. / 60.);
        }
        assert_eq!(menu.appear, 0.);
        assert_eq!(ease(0.), 0.);
        assert_eq!(ease(1.), 1.);
        assert!(ease(0.5) > 0.5);
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
        let line = |label: &str, value: &str, look| Line {
            label: text(label),
            value: text(value),
            look,
            opens: true,
            adjustable: true,
        };
        let view = View {
            title: text("Enemy Preview"),
            position: text("3 / 40"),
            tabs: vec![text("Playback"), text("Motions"), text("Actions")],
            tab_open: 1,
            tab_at: 0.5,
            info: vec![text("Shadow - Idle"), text("Frame 1.0 / 48")],
            search: Some(text("Search: type to filter")),
            lines: vec![
                line("Play", "", Look::Section),
                line("Items", "12", Look::Normal),
                line("Level up", "Lv 3", Look::Disabled),
            ],
            selected: Some(1),
            cursor_at: 1.,
            scrollbar: Some([0., 0.5]),
            description: text("Change item stock and spell levels."),
            hints: text("Enter/A select   Esc/B back"),
            appear: 1.,
            ..View::default()
        };
        let widths = [10u8; 224];
        let d = draws(&view, 0, 1, &widths);
        assert_eq!(d.len(), 2);
        for draw in &d {
            let Draw::Quads { quads, .. } = draw else {
                panic!("quad draws")
            };
            for q in quads {
                for [x, y] in q.corners {
                    assert!((BOX[0]..=BOX[2]).contains(&x) && (BOX[1]..=BOX[3]).contains(&y));
                }
            }
        }
        // A hidden view draws nothing visible.
        assert!(!View::default().visible());
    }
}
