//! Another Sora in the room: a second [`Fighter`] with a whole Sora's state of
//! his own (kit, magic, items, Drive forms, abilities, equipment), steered
//! by a pad of his own. For his tick his state is swapped into the room's
//! (`swap`), so the room's own code moves him, casts his spells, uses his
//! items and changes his forms; then it is swapped back. An extension puts
//! him in the room and runs him. The room's camera, HUD and menus stay on the
//! room's own Sora.

use super::*;

/// The room's starting kit, which a rival starts from.
pub struct RivalKit {
    pub params: Params,
    pub clips: Clips,
    pub poses: HashMap<(Action, bool), usize>,
    pub clip_poses: Arc<Vec<ClipPoses>>,
    pub target_shape: Option<object_collision::Entry>,
    /// HIT spheres (bone, offset, radius, high).
    pub body: Vec<(usize, Vec3, f32, bool)>,
    pub target_radius: f32,
}

/// A command the other player chose in their menu (the host runs it).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Command {
    #[default]
    None,
    /// A spell by magic type.
    Cast(u8),
    /// An item slot.
    Item(u8),
    /// A Drive form, or 0 to revert.
    Drive(u8),
}

/// One tick of a rival's buttons: the stick already as a world XZ direction
/// (magnitude 0..=1), the buttons pressed this tick and a menu command.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RivalPad {
    pub direction: [f32; 2],
    pub jump: bool,
    pub jump_held: bool,
    pub attack: bool,
    pub guard: bool,
    pub guard_held: bool,
    /// Two buttons beyond the KH2 pad's (`extension::ExtraButtons`).
    pub shotlock_held: bool,
    pub flow: bool,
    pub command: Command,
}

impl RivalPad {
    /// The buttons of a room Sora's input, for the host to replay.
    pub fn of(input: &Input) -> RivalPad {
        RivalPad {
            direction: input.direction,
            jump: input.jump,
            jump_held: !input.jump_up,
            attack: input.attack,
            guard: input.guard,
            guard_held: input.guard_held,
            shotlock_held: false,
            flow: false,
            command: Command::None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// A test script.
    Script,
    /// Another player's input, or what another instance sends.
    Remote,
}

/// What a Sora has of his own besides the [`Fighter`]: the fields of the
/// room's `Game` his tick reads and changes, swapped in for it.
pub struct Own {
    pub params: Params,
    pub clips: Clips,
    pub poses: HashMap<(Action, bool), usize>,
    pub clip_poses: Arc<Vec<ClipPoses>>,
    pub player_target_shape: Option<object_collision::Entry>,
    pub sora_attack: HashMap<u8, Vec<(usize, Vec3, f32)>>,
    pub magic: Option<magic_room::MagicRt>,
    pub items: Option<item_room::ItemRt>,
    pub drive: Option<drive_room::DriveRt>,
    pub ability_slots: Vec<u16>,
    pub equipment: pause_menu::items::Equipment,
    pub gear: player::equipment_stats::Totals,
    pub selected_target: Option<[f32; 3]>,
    pub target_locked: bool,
    pub cure_target: usize,
    pub weapon_pax: Option<Arc<object_effect::Pax>>,
    pub weapon2_pax: Option<Arc<object_effect::Pax>>,
    /// The battle runtime's spheres for him (they follow his kit).
    pub blades: [character_kit::Blade; 2],
    pub body: Vec<(usize, Vec3, f32, bool)>,
    /// The spell effects' feedback under his own keys.
    pub fx_live: HashMap<u64, magic::FxLive>,
    /// His swing trails (weapon_trail.rs): live, and each slot's blade length.
    pub trail_live: [bool; 2],
    pub trail_length: [Option<f32>; 2],
    /// Where his motions' sound and dust triggers were last scanned.
    pub scans: Scans,
}

/// A Sora's frame-trigger scans (swings, voices, cloth, footsteps, dust):
/// the action start and the frame already scanned.
#[derive(Default)]
pub struct Scans {
    pub sound_frame: Option<(u64, u32)>,
    pub voice: Option<(u64, Action, u32)>,
    pub move_se: Option<(u64, Action, u32)>,
    pub step: Option<(u64, Action, u32)>,
    pub dust: Option<(u64, Action, u32)>,
}

pub struct Rival {
    /// His place among the room's Soras.
    pub slot: u8,
    pub fighter: Fighter,
    /// Tick the current action started (attack instance ids, blends).
    pub started: u64,
    pub control: Control,
    pub pad: RivalPad,
    pub own: Box<Own>,
    /// The map collision's last floor normal and material for this body
    /// (the map keeps the room Sora's own).
    pub floor: Option<([f32; 3], u8)>,
    /// What the presenter showed (crossfades between motions).
    pub shown: Shown,
}

#[derive(Default)]
pub struct Shown {
    pub key: Option<(usize, usize)>,
    pub target: Vec<Vec3>,
    pub display: Vec<Vec3>,
    pub action: Option<(Action, u64)>,
    pub blend_from: Option<(Vec<Vec3>, u64)>,
    pub spawned: bool,
    /// The model's parts' vertex indices, and each Keyblade's.
    pub part_indices: Vec<Vec<u32>>,
    pub weapon_indices: [Vec<Vec<u32>>; 2],
}

/// The second slot's id.
pub const RIVAL_SLOT: u8 = 1;
/// A rival's spell and item effect keys carry his slot (+1) in these bits,
/// so they never meet the room Sora's.
const KEY_SLOT_SHIFT: u32 = 56;

/// The effect key of `key` for a Sora in `slot` (the room Sora's own keys
/// stay as they are).
pub fn tag_key(key: u64, slot: u8) -> u64 {
    key ^ (u64::from(slot) + 1) << KEY_SLOT_SHIFT
}

/// A new rival's own state: the room's starting kit, a fresh magic, item
/// and Drive state with the room Sora's levels, stock and form records, his
/// abilities and equipment.
pub fn own_from(game: &Game, kit: &RivalKit, slot: u8) -> Own {
    let blades = game
        .enemy
        .as_ref()
        .map(|rt| rt.blade.clone())
        .unwrap_or_default();
    let mut magic = game
        .magic
        .as_ref()
        .map(|m| magic_room::MagicRt::new(m.src.clone(), m.levels));
    if let Some(m) = magic.as_mut() {
        // Spell serials of their own, so volume keys never meet the room
        // Sora's.
        m.serial = (u64::from(slot) + 1) << 40;
    }
    let items = game.items.as_ref().map(|i| {
        item_room::ItemRt::new(
            i.src.clone(),
            i.slots.clone(),
            i.originals.clone(),
            i.stock.clone(),
        )
    });
    Own {
        params: kit.params,
        clips: kit.clips.clone(),
        poses: kit.poses.clone(),
        clip_poses: Arc::clone(&kit.clip_poses),
        player_target_shape: kit.target_shape,
        sora_attack: game.sora_attack.clone(),
        magic,
        items,
        drive: game.drive.as_ref().map(|d| {
            let mut d = d.sibling();
            d.offset_bullet_keys(slot);
            d
        }),
        ability_slots: game.ability_slots.clone(),
        equipment: game.equipment.clone(),
        gear: game.gear,
        selected_target: None,
        target_locked: false,
        cure_target: 0,
        weapon_pax: game.weapon_pax.clone(),
        weapon2_pax: game.weapon2_pax.clone(),
        blades,
        body: kit.body.clone(),
        fx_live: HashMap::new(),
        trail_live: [false; 2],
        trail_length: [None; 2],
        scans: Scans::default(),
    }
}

/// Exchanges a rival's state with the room's: called in pairs around his
/// tick.
pub fn swap(game: &mut Game, rival: &mut Rival) {
    use std::mem::swap as x;
    x(&mut game.sora, &mut rival.fighter);
    x(&mut game.action_started, &mut rival.started);
    let o = &mut *rival.own;
    x(&mut game.params, &mut o.params);
    x(&mut game.clips, &mut o.clips);
    x(&mut game.poses, &mut o.poses);
    x(&mut game.clip_poses, &mut o.clip_poses);
    x(&mut game.player_target_shape, &mut o.player_target_shape);
    x(&mut game.sora_attack, &mut o.sora_attack);
    x(&mut game.magic, &mut o.magic);
    x(&mut game.items, &mut o.items);
    x(&mut game.drive, &mut o.drive);
    x(&mut game.ability_slots, &mut o.ability_slots);
    x(&mut game.equipment, &mut o.equipment);
    x(&mut game.gear, &mut o.gear);
    x(&mut game.selected_target, &mut o.selected_target);
    x(&mut game.target_locked, &mut o.target_locked);
    x(&mut game.cure_target, &mut o.cure_target);
    x(&mut game.weapon_pax, &mut o.weapon_pax);
    x(&mut game.weapon2_pax, &mut o.weapon2_pax);
    x(&mut game.fx_live, &mut o.fx_live);
    x(&mut game.trail_live, &mut o.trail_live);
    x(&mut game.trail_length, &mut o.trail_length);
    x(&mut game.sound_frame, &mut o.scans.sound_frame);
    x(&mut game.move_se_scan, &mut o.scans.move_se);
    x(&mut game.step_scan, &mut o.scans.step);
    x(&mut game.dust_scan, &mut o.scans.dust);
    if let Some(v) = game.voices.as_mut() {
        x(&mut v.scan, &mut o.scans.voice);
    }
    if let Some(rt) = game.enemy.as_mut() {
        x(&mut rt.blade, &mut o.blades);
        x(&mut rt.sora_body, &mut o.body);
    }
}

impl Rival {
    pub fn new(
        game: &Game,
        slot: u8,
        player: Player,
        vitals: Vitals,
        control: Control,
    ) -> Option<Rival> {
        let kit = game.rival_kit.as_ref()?;
        Some(Rival {
            slot,
            fighter: Fighter {
                player,
                vitals,
                ext: (hooks().new_fighter)(),
            },
            started: 0,
            control,
            pad: RivalPad::default(),
            own: Box::new(own_from(game, kit, slot)),
            floor: None,
            shown: Shown {
                part_indices: game.part_indices.clone(),
                weapon_indices: [game.weapon_indices.clone(), Vec::new()],
                ..Shown::default()
            },
        })
    }
}

/// A Sora's TARGET centre: the kit's TARGET shape on its bone this frame.
pub fn target_centre(
    shape: Option<&object_collision::Entry>,
    poses: &HashMap<(Action, bool), usize>,
    clip_poses: &[ClipPoses],
    clips: &Clips,
    player: &Player,
) -> [f32; 3] {
    let Some(shape) = shape else {
        return player.position;
    };
    let matrix = usize::try_from(shape.bone)
        .ok()
        .filter(|&b| b != 0x4000)
        .and_then(|bone| {
            let clip = clip_poses.get(*poses.get(&(player.action, player.armed))?)?;
            let frame = player.motion_frame(clips) as usize;
            clip.body
                .get(frame.min(clip.body.len().saturating_sub(1)))?
                .get(&bone)
                .copied()
        })
        .unwrap_or(Mat4::IDENTITY);
    let offset = Vec3::from_array(shape.position.map(|value| value as f32));
    let local = matrix.transform_point3(offset) + Vec3::Y * shape.position_height as f32;
    let centre = Vec3::from_array(player.position) + Quat::from_rotation_y(player.yaw) * local;
    if centre.is_finite() {
        centre.to_array()
    } else {
        player.position
    }
}

/// A rival's TARGET centre with his own kit.
pub fn rival_centre(r: &Rival) -> [f32; 3] {
    let o = &r.own;
    target_centre(
        o.player_target_shape.as_ref(),
        &o.poses,
        &o.clip_poses,
        &o.clips,
        &r.fighter.player,
    )
}
