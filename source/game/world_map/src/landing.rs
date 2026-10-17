//! Landing on a world (#87): the reaction a symbol offers the cursor, the
//! rules of the landing menu dk::WM_LANDING (FUN_140390d00, FUN_1403919e0,
//! FUN_140390ff0, FUN_1403928a0, FUN_1403931f0) and the values its info
//! panel dk::WM_LANDING_INFO shows (FUN_140395970). The drawing is
//! `visual::world_map_menu`.
use area::world_state::{GridCell, LandingGrid, Target};

/// The 'cmd' executes of the world map objects' reactions (their shapes'
/// argument names a 'cmd' entry, one per world: 0xa0 Twilight Town, 0xa1
/// Hollow Bastion, ...): landing on a world and a Gummi route
/// (FUN_140394fc0). The entries have no direction gate (+0x10 / +0x18 are
/// −1) and priority 100.
pub const LAND_EXEC: u16 = 0x9f;
pub const ROUTE_EXEC: u16 = 0x206;

/// Pad mode 3 (DAT_140749600[3] = 0x1405c3880, set by FUN_14039b580(3)
/// while the menu runs): R1 → 0x100, L1 → 0x200, Triangle → 0x400, on top
/// of the base table's decide 4, cancel 8 and directions 0x10..0x80.
pub mod pad {
    pub const DECIDE: u64 = 0x4;
    pub const CANCEL: u64 = 0x8;
    pub const UP: u64 = 0x10;
    pub const DOWN: u64 = 0x20;
    pub const LEFT: u64 = 0x40;
    pub const RIGHT: u64 = 0x80;
    pub const R1: u64 = 0x100;
    pub const L1: u64 = 0x200;
    pub const TRIANGLE: u64 = 0x400;
    /// The raw-to-logical pairs of mode 3 (0x1405c3880).
    pub const MODE: &[(u64, u64)] = &[(0x800, 0x100), (0x400, 0x200), (0x1000, 0x400)];
}

/// Menu sounds (thunk_FUN_1401de0c0 numbers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    Cursor = 1,
    Decide = 2,
    Open = 3,
    Cancel = 4,
    Error = 5,
}

/// +0x2838: the menu's mode. ROWS when the world has no touched world
/// point (two rows: land / cancel), POINTS for the world-point grid,
/// EPISODES for the episode list and BACK for the grid after it.
pub const ROWS: u8 = 0;
pub const POINTS: u8 = 1;
pub const EPISODES: u8 = 2;
pub const BACK: u8 = 3;

/// Layout scenes per mode (0x1405c0f00): {open, close, idle}.
pub const SCENES: [[usize; 3]; 4] = [[0, 2, 1], [3, 5, 4], [6, 9, 8], [10, 5, 4]];

/// 'wm' messages: the first row of the two-row menu per world
/// (0x1405c10b0) and its second row.
pub const LAND_ROW: [u32; 19] = [
    0x4871, 0x4872, 0x4873, 0x4874, 0x4875, 0x4876, 0x4877, 0x4878, 0x4879, 0x487a, 0x487b, 0x487c,
    0x487d, 0x487e, 0x487f, 0x4880, 0x4881, 0x4882, 0x4883,
];
pub const CANCEL_ROW: u32 = 0x4884;
/// The blocked world point's notice and the episode panel heading.
pub const BLOCKED_NOTICE: u32 = 0x4c0d;
pub const EPISODE_HEADING: u32 = 0x4c16;
/// Rows the episode panel shows at once (+0x344c).
pub const EPISODE_ROWS: usize = 3;

/// What the menu ended with (+0x2834 and the target at +0x3a50).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Land(Target),
    Cancel,
}

/// One row of the episode list: its title and help messages and its NEW
/// mark (FUN_1403f0b20).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EpisodeRow {
    pub title: u16,
    pub help: u16,
    pub new: bool,
}

/// The menu's presentation changes for the drawing side to follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// The layout plays `scene` now, then `next` (FUN_1402ec550).
    Scene { scene: usize, next: usize },
    /// The blocked notice opens (group 0x5b then 0x5c) or closes (0x5d).
    Notice(bool),
    /// The menu closes (FUN_140392430): the mode's close scene plays.
    Close,
    /// The episode rows show from this episode on (FUN_1402facd0 and the
    /// row texts of FUN_1403928a0).
    EpisodeTop(usize),
}

/// FUN_1402f9440's results the menu handles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pick {
    None,
    Moved,
    Row(usize),
    Cancel,
    /// −4: logical bit 8 (R1 in mode 3).
    Bit8,
}

/// A Tz list in grid mode (FUN_1402f8a70): 2 columns, wrapping (list
/// +0x24 = 1), no centring (+0x26 = 0), no scroll (the grid never holds
/// more cells than the list's 8 rows), keys UP / DOWN / LEFT / RIGHT.
/// FUN_1402f8240's moves for that case.
fn grid_move(cursor: usize, n: usize, pressed: u64, repeat: u64) -> Option<usize> {
    const COLS: usize = 2;
    if n == 0 {
        return None;
    }
    let (row, col) = (cursor / COLS, cursor % COLS);
    let rows = n.div_ceil(COLS);
    if repeat & pad::UP != 0 {
        if (row == 0 && pressed & pad::UP == 0) || (rows == 1 && pressed & pad::UP != 0) {
            return None;
        }
        if row > 0 {
            return Some(cursor - COLS);
        }
        return Some(((rows - 1) * COLS + col).min(n - 1));
    }
    if repeat & pad::DOWN != 0 {
        if (row + 1 >= rows && pressed & pad::DOWN == 0) || (rows == 1 && pressed & pad::DOWN != 0)
        {
            return None;
        }
        if row + 1 < rows {
            return Some((cursor + COLS).min(n - 1));
        }
        return Some(col);
    }
    // The row the cursor is in: a short last row wraps within itself.
    let row_len = if !n.is_multiple_of(COLS) && n - n % COLS <= cursor {
        n % COLS
    } else {
        COLS
    };
    if repeat & pad::LEFT != 0 {
        return Some(if col > 0 { cursor } else { cursor + row_len } - 1);
    }
    if repeat & pad::RIGHT != 0 {
        return Some(if col + 1 < row_len {
            cursor + 1
        } else {
            cursor + 1 - row_len
        });
    }
    None
}

/// FUN_1402f9440 for a plain list (mask 0x73) of `n` rows that wraps on a
/// fresh press: UP / DOWN move, then decide, cancel, bit 8.
fn list_input(cursor: &mut usize, n: usize, pressed: u64, repeat: u64) -> Pick {
    if n == 0 {
        return Pick::None;
    }
    let before = *cursor;
    if repeat & pad::UP != 0 {
        if *cursor > 0 {
            *cursor -= 1;
        } else if pressed & pad::UP != 0 {
            *cursor = n - 1;
        }
    } else if repeat & pad::DOWN != 0 {
        if *cursor + 1 < n {
            *cursor += 1;
        } else if pressed & pad::DOWN != 0 {
            *cursor = 0;
        }
    }
    if *cursor != before {
        return Pick::Moved;
    }
    decision(*cursor, pressed)
}

fn decision(cursor: usize, pressed: u64) -> Pick {
    if pressed & pad::DECIDE != 0 {
        Pick::Row(cursor)
    } else if pressed & pad::CANCEL != 0 {
        Pick::Cancel
    } else if pressed & pad::R1 != 0 {
        Pick::Bit8
    } else {
        Pick::None
    }
}

/// dk::WM_LANDING's state.
#[derive(Clone, Debug, PartialEq)]
pub struct Landing {
    pub world: u8,
    pub mode: u8,
    /// +0x2850: the grid's cells up to the highest one used (+0x31d0).
    pub cells: Vec<Option<GridCell>>,
    /// The grid's (or the two rows') cursor.
    pub cursor: usize,
    /// FUN_1403e4190's target for the two-row menu.
    pub default_target: Target,
    pub episodes: Vec<EpisodeRow>,
    /// The first episode shown (the episode list's cursor + scroll).
    pub episode_top: usize,
    /// +0x20 bit 0: the blocked notice is up.
    pub notice: bool,
    pub outcome: Option<Outcome>,
    /// Set when the menu closed; the drawing side plays the close scene.
    pub closing: bool,
}

impl Landing {
    /// FUN_140390d00 with FUN_1403919e0 / FUN_140390ff0: the grid mode
    /// when any world point of the world is touched; the grid cursor starts
    /// on the first selectable cell (else the last).
    pub fn new(
        world: u8,
        grid: LandingGrid,
        default_target: Target,
        episodes: Vec<EpisodeRow>,
    ) -> Self {
        let mode = if grid.placed != 0 { POINTS } else { ROWS };
        let cells = if mode == POINTS {
            grid.cells
        } else {
            Vec::new()
        };
        let cursor = cells
            .iter()
            .position(Option::is_some)
            .unwrap_or(cells.len().saturating_sub(1));
        Self {
            world,
            mode,
            cells,
            cursor,
            default_target,
            episodes,
            episode_top: 0,
            notice: false,
            outcome: None,
            closing: false,
        }
    }

    /// The scenes a new menu plays (FUN_1402ec3b0 then FUN_1402ec520).
    pub fn opening(&self) -> Change {
        let [open, _, idle] = SCENES[self.mode as usize];
        Change::Scene {
            scene: open,
            next: idle,
        }
    }

    /// The mode's idle scene: input runs only while the layout plays it.
    pub fn idle_scene(&self) -> usize {
        SCENES[self.mode as usize][2]
    }

    /// The scene the menu closes with.
    pub fn close_scene(&self) -> usize {
        SCENES[self.mode as usize][1]
    }

    fn selectable(&self, i: usize) -> bool {
        self.cells.get(i).is_some_and(Option::is_some)
    }

    fn close(&mut self, outcome: Outcome, changes: &mut Vec<Change>) {
        self.outcome = Some(outcome);
        self.closing = true;
        changes.push(Change::Close);
    }

    /// One frame of input (`pressed` / `repeat` in pad mode 3) while the
    /// layout plays the idle scene. Returns the sounds and changes.
    pub fn input(&mut self, pressed: u64, repeat: u64) -> (Vec<Sound>, Vec<Change>) {
        let mut sounds = Vec::new();
        let mut changes = Vec::new();
        if self.closing {
            return (sounds, changes);
        }
        match self.mode {
            ROWS => self.rows_input(pressed, repeat, &mut sounds, &mut changes),
            EPISODES => self.episodes_input(pressed, repeat, &mut sounds, &mut changes),
            _ => self.grid_input(pressed, repeat, &mut sounds, &mut changes),
        }
        (sounds, changes)
    }

    /// FUN_1403931f0's two-row branch: row 0 lands at the default target,
    /// row 1 and cancel close.
    fn rows_input(&mut self, p: u64, r: u64, s: &mut Vec<Sound>, c: &mut Vec<Change>) {
        match list_input(&mut self.cursor, 2, p, r) {
            Pick::Moved => s.push(Sound::Cursor),
            Pick::Cancel => {
                s.push(Sound::Cancel);
                self.close(Outcome::Cancel, c);
            }
            Pick::Row(row) => {
                s.push(Sound::Decide);
                let outcome = if row == 0 {
                    Outcome::Land(self.default_target)
                } else {
                    Outcome::Cancel
                };
                self.close(outcome, c);
            }
            Pick::None | Pick::Bit8 => {}
        }
    }

    /// FUN_1403928a0's episode branch: L1 or cancel goes back to the grid
    /// (scene 10, then 4); with four or more episodes UP / DOWN scroll the
    /// three rows.
    fn episodes_input(&mut self, p: u64, r: u64, s: &mut Vec<Sound>, c: &mut Vec<Change>) {
        if p & (pad::L1 | pad::CANCEL) != 0 {
            s.push(Sound::Decide);
            self.mode = BACK;
            c.push(Change::Scene { scene: 10, next: 4 });
            return;
        }
        let n = self.episodes.len();
        if n <= EPISODE_ROWS {
            return;
        }
        // The list's positions are the first rows shown: 0..=n − 3.
        let mut top = self.episode_top;
        if let Pick::Moved = list_input(&mut top, n - 2, p, r) {
            s.push(Sound::Cursor);
            self.episode_top = top;
            c.push(Change::EpisodeTop(top));
        }
    }

    /// FUN_1403928a0's grid branch.
    fn grid_input(&mut self, p: u64, r: u64, s: &mut Vec<Sound>, c: &mut Vec<Change>) {
        if !self.notice {
            if p & pad::R1 != 0 {
                s.push(Sound::Decide);
                self.mode = EPISODES;
                c.push(Change::Scene { scene: 6, next: 8 });
                return;
            }
            // Triangle opens the world's journal page (FUN_14038e050 →
            // FUN_1402e4bc0) [U: not ported].
            if p & pad::TRIANGLE != 0 {
                return;
            }
        }
        let mut proceed = true;
        if self.notice && p & 0xfc != 0 {
            self.notice = false;
            c.push(Change::Notice(false));
            s.push(Sound::Cancel);
            proceed = p & (pad::DECIDE | pad::CANCEL) == 0;
        }
        if self.notice {
            return;
        }
        let before = self.cursor;
        let n = self.cells.len();
        let pick = match grid_move(self.cursor, n, p, r) {
            Some(to) => {
                self.cursor = to;
                Pick::Moved
            }
            None => decision(self.cursor, p),
        };
        if self.cursor != before {
            // Step over cells without a touched point, toward the move
            // (or the other way round when it wrapped).
            let span = (n.saturating_sub(1) & !1) as isize;
            let (from, to) = (before as isize, self.cursor as isize);
            let forward = if to < from {
                from - to >= span
            } else {
                to - from < span
            };
            let mut at = self.cursor;
            for _ in 0..n {
                if self.selectable(at) {
                    break;
                }
                at = if forward {
                    (at + 1) % n
                } else {
                    (at + n - 1) % n
                };
            }
            self.cursor = at;
            if at != before {
                s.push(Sound::Cursor);
            }
        }
        if !proceed {
            return;
        }
        match pick {
            Pick::Cancel => {
                s.push(Sound::Cancel);
                self.close(Outcome::Cancel, c);
            }
            Pick::Row(_) => match self.cells.get(self.cursor).copied().flatten() {
                Some(cell) if cell.blocked.is_some() => {
                    s.push(Sound::Error);
                    self.notice = true;
                    c.push(Change::Notice(true));
                }
                Some(cell) => {
                    s.push(Sound::Decide);
                    self.close(Outcome::Land(cell.target), c);
                }
                None => {}
            },
            Pick::None | Pick::Moved | Pick::Bit8 => {}
        }
    }
}

/// The world name the info panel shows (FUN_140395970): sys 0x8211 + world
/// (FUN_140153280, table 0x1405b1610), or 0x8ae9 for Hollow Bastion once
/// story flag 0x1077 is set.
pub fn world_name(world: u8, flag_1077: bool) -> u32 {
    if world == 4 && flag_1077 {
        0x8ae9
    } else {
        0x8211 + world as u32
    }
}

/// FUN_140157430: a world's play time (save +0x244c, 60 ticks a second)
/// as hours, minutes and seconds.
pub fn play_time(ticks: u32) -> [u32; 3] {
    let h = ticks / 216_000;
    let rest = ticks % 216_000;
    [h, rest / 3600, rest % 3600 / 60]
}

/// FUN_140392620: `value`'s last `digits` decimal digits, most significant
/// first, and how many digits it has (at least 1).
pub fn digits(value: u32, digits: usize) -> (Vec<u32>, usize) {
    let mut out = vec![0; digits];
    let (mut v, mut used) = (value, 0);
    for d in out.iter_mut().rev() {
        if v != 0 {
            *d = v % 10;
            used += 1;
            v /= 10;
        }
    }
    (out, used.max(1))
}

/// FUN_1403c9c00 for a shape on an object's root (bone 0x4000): the shape
/// position in the object's model space moved to it, then raised by the
/// height offset (engine Y down). Model space faces −Z at yaw 0: the root
/// turns a model point by −R(yaw) in the ground plane, as the models are
/// drawn [I: from the drawing convention and the arrival points, which sit
/// in front of each symbol inside its reaction sphere].
pub fn shape_centre(position: [f32; 3], yaw: f32, local: [f32; 3], height: f32) -> [f32; 3] {
    let (s, c) = yaw.sin_cos();
    [
        position[0] - (local[0] * c + local[2] * s),
        position[1] + local[1] - height,
        position[2] - (-local[0] * s + local[2] * c),
    ]
}

/// FUN_1403c94c0 for two spheres (shape 3): their centres are closer than
/// the sum of the radii.
pub fn spheres_touch(a: [f32; 3], ra: f32, b: [f32; 3], rb: f32) -> bool {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < (ra + rb) * (ra + rb)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(point: u8, blocked: bool) -> Option<GridCell> {
        Some(GridCell {
            point,
            picture: point,
            blocked: blocked.then_some(0x4000),
            target: Target::at(2, point, 0x63),
        })
    }

    fn grid(cells: Vec<Option<GridCell>>) -> LandingGrid {
        let placed = cells.iter().flatten().count();
        LandingGrid { cells, placed }
    }

    #[test]
    fn grid_moves_wrap_rows_and_columns() {
        // Five cells: rows {0,1}, {2,3}, {4}.
        assert_eq!(grid_move(0, 5, 0, pad::RIGHT), Some(1));
        assert_eq!(grid_move(1, 5, 0, pad::RIGHT), Some(0));
        assert_eq!(grid_move(4, 5, 0, pad::RIGHT), Some(4));
        assert_eq!(grid_move(0, 5, 0, pad::LEFT), Some(1));
        assert_eq!(grid_move(3, 5, 0, pad::DOWN), Some(4));
        // The bottom row wraps up only on a fresh press.
        assert_eq!(grid_move(4, 5, 0, pad::DOWN), None);
        assert_eq!(grid_move(4, 5, pad::DOWN, pad::DOWN), Some(0));
        assert_eq!(grid_move(1, 5, pad::UP, pad::UP), Some(4));
        assert_eq!(grid_move(0, 2, pad::UP, pad::UP), None);
    }

    #[test]
    fn the_grid_cursor_skips_empty_cells_and_lands() {
        let mut l = Landing::new(
            2,
            grid(vec![cell(1, false), None, cell(3, false), cell(4, true)]),
            Target::NONE,
            Vec::new(),
        );
        assert_eq!((l.mode, l.cursor), (POINTS, 0));
        let (s, _) = l.input(pad::RIGHT, pad::RIGHT);
        assert_eq!((l.cursor, s), (2, vec![Sound::Cursor]));
        let (s, c) = l.input(pad::DECIDE, 0);
        assert_eq!(s, vec![Sound::Decide]);
        assert_eq!(c, vec![Change::Close]);
        assert_eq!(l.outcome, Some(Outcome::Land(Target::at(2, 3, 0x63))));
    }

    #[test]
    fn a_blocked_cell_shows_the_notice_until_a_press() {
        let mut l = Landing::new(2, grid(vec![cell(4, true)]), Target::NONE, Vec::new());
        let (s, c) = l.input(pad::DECIDE, 0);
        assert_eq!((s, c), (vec![Sound::Error], vec![Change::Notice(true)]));
        // Decide only closes the notice.
        let (s, c) = l.input(pad::DECIDE, 0);
        assert_eq!((s, c), (vec![Sound::Cancel], vec![Change::Notice(false)]));
        assert_eq!(l.outcome, None);
    }

    #[test]
    fn two_rows_without_world_points() {
        let target = Target::at(5, 1, 0);
        let mut l = Landing::new(5, LandingGrid::default(), target, Vec::new());
        assert_eq!(l.mode, ROWS);
        assert_eq!(l.opening(), Change::Scene { scene: 0, next: 1 });
        l.input(pad::DECIDE, 0);
        assert_eq!(l.outcome, Some(Outcome::Land(target)));
        let mut l = Landing::new(5, LandingGrid::default(), target, Vec::new());
        l.input(pad::DOWN, pad::DOWN);
        l.input(pad::DECIDE, 0);
        assert_eq!(l.outcome, Some(Outcome::Cancel));
    }

    #[test]
    fn episodes_scroll_by_their_first_row() {
        let row = EpisodeRow {
            title: 1,
            help: 2,
            new: false,
        };
        let mut l = Landing::new(2, grid(vec![cell(1, false)]), Target::NONE, vec![row; 5]);
        l.input(pad::R1, 0);
        assert_eq!(l.mode, EPISODES);
        l.input(pad::DOWN, pad::DOWN);
        l.input(0, pad::DOWN);
        l.input(0, pad::DOWN);
        assert_eq!(l.episode_top, 2);
        l.input(pad::L1, 0);
        assert_eq!(l.mode, BACK);
        assert_eq!(l.idle_scene(), 4);
    }

    #[test]
    fn info_values() {
        assert_eq!(world_name(4, true), 0x8ae9);
        assert_eq!(world_name(2, true), 0x8213);
        assert_eq!(play_time(216_000 + 3600 * 5 + 61), [1, 5, 1]);
        assert_eq!(digits(7, 2), (vec![0, 7], 1));
        assert_eq!(digits(0, 3), (vec![0, 0, 0], 1));
    }

    #[test]
    fn reaction_spheres() {
        // TT's reaction sphere: 1500 to the front of a symbol at yaw π.
        let pi = std::f32::consts::PI;
        let c = shape_centre([1000., -600., 0.], pi, [0., 0., -1500.], 0.);
        assert!((c[0] - 1000.).abs() < 1e-3 && (c[2] - -1500.).abs() < 1e-3);
        let turned = shape_centre([0.; 3], std::f32::consts::FRAC_PI_2, [0., 0., 100.], 0.);
        assert!((turned[0] - -100.).abs() < 1e-3 && turned[2].abs() < 1e-3);
        assert!(spheres_touch(c, 1500., [1000., -600., -2900.], 45.));
        assert!(!spheres_touch(c, 1500., [1000., -600., 100.], 45.));
    }
}
