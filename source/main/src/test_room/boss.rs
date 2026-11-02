//! The host side of the boss framework: what a boss's object script asks of
//! the engine beyond an ordinary enemy. Sora played by the boss's own motions
//! (`obj_motion_capture`, FUN_1403c6cc0: a slot of the captor's set holds the
//! victim's body and Keyblade clips), the motions it queues behind them, and
//! the bosses whose sets hold such slots.
use super::*;

/// The first slot of a motion set that can hold a captured motion.
const CAPTURE_SLOT: usize = 240;
/// The name a captured motion's first entry has when it is for Sora's skeleton.
/// [I] Others are for the player of another story point: 0140 in B_EX100's set
/// (Roxas), 0200 in B_LK120's (lion Sora), 0080 in one slot of B_CA010's; their
/// skeletons are not the kit's, so they are left out.
const CAPTURE_NAME: &[u8; 4] = b"0010";
/// The objects whose captured motions fit Sora's kit's ids.
const CAPTURE_OBJECTS: usize = 8;

/// The objects of this room whose motion sets capture Sora, and the slots that
/// hold him (Sephiroth: N_HB630.mset 252 AQ90 the Block pose, 253 AQ90 the
/// Block counter, 254 AQ91 the grab). Read once, for his kit.
static CAPTURES: std::sync::OnceLock<Vec<(u32, Vec<usize>)>> = std::sync::OnceLock::new();

static CHEST_OBJECTS: std::sync::OnceLock<Vec<(bool, u32)>> = std::sync::OnceLock::new();

/// A motion in Sora's kit: clip id, body motion, Keyblade motion.
use super::character_kit::KitMotion;
/// The chests' captures: clip id and the slot's archive.
static CHEST_CLIPS: std::sync::OnceLock<Vec<(u16, Vec<u8>)>> = std::sync::OnceLock::new();

/// The generic chest object whose capture motion Sora's kit holds.
pub fn chest_capture(big: bool) -> Option<u32> {
    CHEST_OBJECTS
        .get()?
        .iter()
        .find(|(b, _)| *b == big)
        .map(|(_, id)| *id)
}

/// The generic small and big chests (`F_EX030`, `F_EX040`), by big: their
/// capture motions stand for every world's copy, whose slot 252 holds the same
/// clip for Sora.
pub fn chest_objects(objects: &[objentry::ObjectEntry]) -> Vec<(bool, u32)> {
    [(false, &b"F_EX030"[..]), (true, &b"F_EX040"[..])]
        .into_iter()
        .filter_map(|(big, name)| {
            let id = objects
                .iter()
                .find(|o| o.object_type == 10 && o.model_name == name)?
                .object_id;
            Some((big, id))
        })
        .collect()
}

/// The id Sora's kit keeps a boss's captured motion under: below the
/// summons' (0x7800..) and above the limits' (0x4000..).
pub fn capture_clip(object: u32, slot: i32) -> Option<u16> {
    let (index, (_, slots)) = CAPTURES
        .get()?
        .iter()
        .enumerate()
        .find(|(_, (o, _))| *o == object)?;
    let slot = usize::try_from(slot).ok().filter(|s| slots.contains(s))?;
    Some(clip_id(index, slot))
}

fn clip_id(index: usize, slot: usize) -> u16 {
    0x7000 | (index as u16) << 8 | (slot - CAPTURE_SLOT) as u16
}

/// Sora's captured motions for his kit, from Sephiroth's motion set and those
/// of `objects`: (clip id, body motion, Keyblade motion).
pub fn capture_motions(
    source: &data::probe_source::ProbeSource,
    objects: impl Iterator<Item = u32>,
) -> Vec<(
    u16,
    data::motion::SelectedMotion,
    Option<data::motion::SelectedMotion>,
)> {
    let mut out = Vec::new();
    let mut captures = Vec::new();
    // Tinker Bell's revive of Sora captures him too.
    let mut wanted = vec![enemy_battle::SEPHIROTH, u32::from(summon::TINKER_BELL)];
    for object in objects {
        if !wanted.contains(&object) {
            wanted.push(object);
        }
    }
    let Ok(mut library) = data::path_lookup::Library::open(source) else {
        return out;
    };
    let Some(objects) = library
        .read("00objentry.bin")
        .ok()
        .and_then(|b| objentry::read(&b).ok())
    else {
        return out;
    };
    // The chests' capture motions: any area's chest may be opened later.
    let chests = chest_objects(&objects);
    let _ = CHEST_OBJECTS.set(chests.clone());
    let mut chest_clips = Vec::new();
    for &(_, id) in &chests {
        if !wanted.contains(&id) {
            wanted.insert(1, id);
        }
    }
    for object in wanted {
        if captures.len() == CAPTURE_OBJECTS {
            eprintln!(
                "Sora's kit holds the captures of {CAPTURE_OBJECTS} objects; the rest left out"
            );
            break;
        }
        // Objentry types 0 and 1: the party, whose sets hold no captures.
        let Some(entry) = objects
            .iter()
            .find(|o| o.object_id == object && o.object_type > 1)
        else {
            continue;
        };
        let name: Vec<u8> = entry
            .animation_name
            .iter()
            .copied()
            .take_while(|&b| b != 0)
            .collect();
        let name = String::from_utf8_lossy(&name);
        let path = if name.ends_with(".mset") {
            format!("obj/{name}")
        } else {
            format!("obj/{name}.mset")
        };
        let Ok(bytes) = library.read(&path) else {
            continue;
        };
        let Ok(table) = data::bar::read_bar(&bytes) else {
            continue;
        };
        let mut slots = Vec::new();
        let is_chest = chests.iter().any(|&(_, id)| id == object);
        for slot in CAPTURE_SLOT..table.entries.len() {
            let clip = clip_id(captures.len(), slot);
            let Some(anb) = Some(&table.entries[slot])
                .filter(|e| e.offset > 0 && e.size > 0)
                .and_then(|e| bytes.get(e.offset as usize..e.offset as usize + e.size as usize))
                .filter(|anb| {
                    data::bar::read_bar(anb).is_ok_and(|b| {
                        b.entries
                            .first()
                            .is_some_and(|e| e.name_bytes == *CAPTURE_NAME)
                    })
                })
            else {
                continue;
            };
            if let Some((body, weapon)) = summon::capture_parts(anb, clip) {
                if is_chest {
                    chest_clips.push((clip, anb.to_vec()));
                }
                out.push((clip, body, weapon));
                slots.push(slot);
            }
        }
        if !slots.is_empty() {
            captures.push((object, slots));
        }
    }
    let _ = CAPTURES.set(captures);
    let _ = CHEST_CLIPS.set(chest_clips);
    out
}

/// The chests' capture motions for a kit worn later (an outfit or a place's
/// body, drive_room.rs), under the same clip ids.
pub fn chest_clips() -> Vec<KitMotion> {
    CHEST_CLIPS
        .get()
        .into_iter()
        .flatten()
        .filter_map(|(clip, anb)| {
            let (body, weapon) = summon::capture_parts(anb, *clip)?;
            Some((*clip, body, weapon))
        })
        .collect()
}

/// The lengths of a boss's captured motions in Sora's kit, which its script
/// reads through `obj_motion_capture` (how long the capture lasts).
pub fn capture_lengths(clips: &player::Clips, object: u32) -> HashMap<usize, u32> {
    CAPTURES
        .get()
        .into_iter()
        .flatten()
        .filter(|(o, _)| *o == object)
        .flat_map(|(_, slots)| slots.iter())
        .filter_map(|&slot| {
            let clip = capture_clip(object, slot as i32)?;
            Some((slot, clips.scripted_clip(clip)?.1.end))
        })
        .collect()
}

/// What a boss's script did to Sora this tick: the captures start his
/// motion (the queue behind it is cleared), then the motions it pushed
/// behind them (`sysobj_motion_push`, FUN_1403c8310) queue up.
pub fn play_on_sora(
    game: &mut Game,
    object: u32,
    captures: &[(i32, f32)],
    pushed: &[(usize, f32)],
) {
    for &(slot, blend) in captures {
        let started = capture_clip(object, slot)
            .is_some_and(|clip| game.sora.player.capture_motion(clip, blend, &game.clips));
        let tick = game.tick;
        game.event(serde_json::json!({"event":"boss_capture","tick":tick,
            "object":object,"slot":slot,"blend":blend,"played":started}));
    }
    for &(motion, blend) in pushed {
        if let Ok(m) = u16::try_from(motion) {
            game.sora.player.queue_motion(m, blend);
        }
    }
}

/// What `camera_watch` watches (FUN_140430b80 builds a group-0 reference
/// of the object): a battle actor, or Sora.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Watch {
    Actor(u64),
    Sora,
}

/// Each script actor's requests of this tick, in the order the engine
/// carries them out after the objects: Sora's captures and queued motions,
/// then the presentation calls (`camera_watch` is camera mode 3,
/// `func_screen_whiteout` / `func_screen_whitein` the global screen fade
/// FUN_140156ce0 / FUN_140156c90 with `(int)(x / 2.0)`).
pub fn requests(game: &mut Game, actors: &mut [enemy_gallery::GalleryActor]) {
    for actor in actors.iter_mut() {
        let Some(battle) = actor.battle.as_mut() else {
            continue;
        };
        let object = battle.assets.object_id;
        let enemy_battle::Brain::Script(a) = &mut battle.brain else {
            continue;
        };
        let captures = std::mem::take(&mut a.sora_captures);
        let pushed = std::mem::take(&mut a.sora_pushed);
        let presentations = std::mem::take(&mut a.presentations);
        let hp = std::mem::take(&mut a.sora_hp);
        let mpdrive = std::mem::take(&mut a.sora_mpdrive);
        let falls = std::mem::take(&mut a.sora_falls);
        let jumps = std::mem::take(&mut a.sora_jumps);
        // FUN_1403aabe0's signals go to the mission's trigger 10.
        game.mission_signals.append(&mut a.signals);
        if !captures.is_empty() {
            // The partner (+0x2b0) the search offers range 25's command for
            // is the object holding Sora's capture: FUN_1403c6cc0 →
            // FUN_1403c65e0 writes it from the captor. Its trigger 11 takes
            // the command (M_EX770's handles 27 and 28).
            game.reaction.captor = Some(actor.id as u32);
        }
        play_on_sora(game, object, &captures, &pushed);
        // After the capture, which starts on the ground: the jump puts him in
        // the air under it.
        for (direction, height, speed) in jumps {
            game.sora.player.script_jump(direction, height, speed);
            let tick = game.tick;
            game.event(serde_json::json!({"event":"boss_sora_jump","tick":tick,
                "direction":direction,"height":height,"speed":speed}));
        }
        for delta in hp {
            sora_hp_change(game, delta);
        }
        // `obj_fly_to_jump` on Sora (FUN_1403b8150): he leaves the held
        // flight into a fall from where he is (the flight's velocity is
        // zero), with the fall / land motions the thread set. His own fall
        // and land are motions 4 and 5; other pairs are not his.
        for (fall, land) in falls {
            let tick = game.tick;
            let played = (fall, land) == (4, 5) && {
                let Game { sora, clips, .. } = game;
                sora.player.play_motion(4, 0., clips)
            };
            if played {
                game.action_started = tick;
            }
            game.event(serde_json::json!({"event":"boss_sora_fall","tick":tick,
                "fall":fall,"land":land,"played":played}));
        }
        for f in mpdrive {
            // FUN_1403d4a20: `f` times Sora's PRTY +0x1c.
            game.sora
                .player
                .mp
                .start_charge(f * player::magic::CHARGE_LENGTH);
            let tick = game.tick;
            game.event(serde_json::json!({"event":"boss_mpdrive","tick":tick,"factor":f}));
        }
        for (name, target, words) in presentations {
            let tick = game.tick;
            match name {
                "camera_watch" => {
                    game.camera_watch = match target {
                        Some(enemy::world::PLAYER_OBJECT) => Some(Watch::Sora),
                        Some(_) => Some(Watch::Actor(actor.id)),
                        None => None,
                    };
                }
                "func_screen_whiteout" => {
                    let n = (f32::from_bits(words[0]) / 2.0) as i32;
                    game.screen_fade.fade_out(n.max(0) as u32, true);
                }
                "func_screen_whitein" => {
                    let n = (f32::from_bits(words[0]) / 2.0) as i32;
                    game.screen_fade.fade_in(n.max(0) as u32, true);
                }
                _ => {}
            }
            game.event(serde_json::json!({"event":"boss_presentation","tick":tick,
                "name":name,"object":object,"target":target,"word":words[0]}));
        }
    }
    // The vacuums pull Sora (type 0, the leader) and the party members (by
    // their objentry type and weight) on their next move.
    let mut pull = [0f32; 3];
    let mut party = vec![[0f32; 3]; game.party.len()];
    for actor in actors.iter() {
        if let Some(enemy_battle::Brain::Script(a)) = actor.battle.as_ref().map(|b| &b.brain) {
            let p = enemy::world::vacuum_pull(
                a.vacuums(),
                game.sora.player.position,
                0,
                game.sora_weight,
                1.,
            );
            for k in 0..3 {
                pull[k] += p[k];
            }
            for (m, out) in game.party.iter().zip(party.iter_mut()) {
                let p = enemy::world::vacuum_pull(
                    a.vacuums(),
                    m.follower.position,
                    u32::from(m.kind.object_type),
                    m.kind.weight,
                    1.,
                );
                for k in 0..3 {
                    out[k] += p[k];
                }
            }
        }
    }
    game.sora.player.push = pull;
    game.party_push = party;
    // The lights' scene tint (FUN_1403f7040), over every actor's pool.
    let mut tint = [0x80u32; 3];
    for actor in actors.iter() {
        if let Some(enemy_battle::Brain::Script(a)) = actor.battle.as_ref().map(|b| &b.brain) {
            let t = enemy::world::scene_tint(a.lights());
            for k in 0..3 {
                tint[k] = (tint[k] * u32::from(t[k])) >> 7;
            }
        }
    }
    // The Drive change's grey and the Limit Form limit's colour are filters of
    // the same list.
    if let Some(d) = game.drive.as_ref() {
        for filter in d.scene_filters() {
            for k in 0..3 {
                tint[k] = (tint[k] * filter[k]) >> 7;
            }
        }
    }
    game.scene_tint = tint.map(|v| v.min(255) as u8);
    game.screen_fade.update(1.0);
}

/// FUN_1401626f0: the map chunks' and the actors' colour is the scene tint
/// (0x80 = 1.0; its light table and FUN_14016f2b0's list) and the fog colour
/// is the area record's times the tint (bytes, `>> 7`). It reaches only the
/// map's effects, never an object's. [U] The map effects' colour (its loop
/// over the effect groups), FUN_1401714d0, and a tinted neutral fog turning
/// fog on are not carried out.
pub fn map_tint(
    game: Res<Game>,
    mut materials: ResMut<Assets<FogMaterial>>,
    mut last: Local<Option<[u8; 3]>>,
) {
    if *last == Some(game.scene_tint) {
        return;
    }
    *last = Some(game.scene_tint);
    let [r, g, b] = game.scene_tint.map(|c| f32::from(c) / 128.);
    let textured: Vec<_> = materials
        .iter()
        .filter(|(_, m)| m.base.base_color_texture.is_some())
        .map(|(id, _)| id)
        .collect();
    for id in textured {
        if let Some(mut m) = materials.get_mut(id) {
            let alpha = m.base.base_color.alpha();
            m.base.base_color = Color::srgba(r, g, b, alpha);
        }
    }
    if let Some(fog) = visual::area_fog::tinted_colour(game.scene_tint) {
        let ids: Vec<_> = materials
            .iter()
            .filter(|(_, m)| m.extension.colour.w > 0. && m.extension.colour != fog)
            .map(|(id, _)| id)
            .collect();
        for id in ids {
            if let Some(mut m) = materials.get_mut(id) {
                m.extension.colour = fog;
            }
        }
    }
}

/// Camera mode 3's anchor (FUN_1403bdbe0): the watched actor's kind-7
/// shape of group 0, else its first TARGET shape; Sora's target point. The
/// watch ends when its object is gone. [U] What else ends mode 3
/// (FUN_1401656f0's other callers) is not traced; the lock button does
/// here.
pub fn watch_anchor(game: &Game, gallery: &enemy_gallery::EnemyGallery) -> Option<[f32; 3]> {
    match game.camera_watch? {
        Watch::Sora => {
            let p = game.sora.player.position;
            Some([p[0], p[1] + SORA_OBJ[2], p[2]])
        }
        Watch::Actor(id) => {
            let battle = gallery
                .actors
                .iter()
                .find(|a| a.id == id)?
                .battle
                .as_ref()?;
            let shape = battle
                .pose
                .camera
                .iter()
                .find(|s| s.group == 0)
                .or_else(|| battle.pose.targets.first())?;
            Some(shape.centre.to_array())
        }
    }
}

/// Sora's HP change (vt+0xe8, FUN_1403a85f0 -> FUN_1403d5e50 ->
/// FUN_1403d2eb0) a script asked for: a loss feeds the Drive gauge (Damage
/// Drive) and, without an MP charge, MP Rage; then HP moves within
/// [0, max] and 0 is his death.
pub fn sora_hp_change(game: &mut Game, delta: i32) {
    let tick = game.tick;
    if game.enemy.is_none() {
        return;
    }
    let max = game.sora.vitals.target.max_hp;
    if delta < 0 {
        let loss = -delta;
        game.sora
            .player
            .drive
            .damaged(loss, max, game.sora.player.abilities.damage_drive);
        if !game.sora.player.mp.charging() && game.sora.player.abilities.mp_rage > 0. {
            let gain = (loss as f32 / max as f32 * game.sora.player.abilities.mp_rage) as i32;
            if gain > 0 {
                game.sora.player.mp.add(gain);
            }
        }
    }
    let vitals = &mut game.sora.vitals;
    vitals.hp = (vitals.hp + delta).clamp(0, max);
    let hp = vitals.hp;
    if hp == 0 && game.sora.player.die(&game.clips).is_some() {
        game.action_started = tick;
    }
    game.event(serde_json::json!({"event":"boss_sora_hp","tick":tick,"delta":delta,"hp":hp}));
}

/// The spawner a battle actor's script makes objects with
/// (`sysobj_appear`; Sephiroth's carrier, objentry 2322 B_EX360).
pub fn spawner(source: &Arc<data::probe_source::ProbeSource>) -> Option<summon::Spawner> {
    let mut library = data::path_lookup::Library::open(source).ok()?;
    let objects = objentry::read(&library.read("00objentry.bin").ok()?).ok()?;
    Some(summon::Spawner::new(source.clone(), &objects))
}

/// A mission an area's battle program registered (op 0x15, FUN_1403a3fa0 ->
/// FUN_1403a2c30 loads `msn/<language>/<name>.bar`): its 48-byte record
/// (BAR type 2), its script (type 3: `ms_boss` for most, with its own rule for
/// others, AL03_MS103 counting two Lords) and the overlay events it names, run
/// by the mission manager's per-frame task FUN_1403a35c0.
pub struct Mission {
    script: Option<Box<enemy_script::ScriptBrain>>,
    /// +0x04: flag bits (bit 0 forces battle style 2, FUN_1403ab690).
    pub flags: u16,
    /// +0x0d: the bonus the clear grants; +0x14 the clear sound.
    pub bonus: u8,
    pub clear_sound: u32,
    /// The kind-2 sub-record's overlay ("re_deadboss-0a") and 00battle.bin's
    /// third event ("re_deadboss2-0a", FUN_1403f4b10(2)).
    boss_event: Option<Overlay>,
    clear_event: Option<Overlay>,
    /// FUN_1403a35c0's state after the clear started: 3 the boss overlay
    /// runs, 4 the clear overlay runs, 5 done (signal 0x10).
    pub state: u8,
}

/// An overlay event (camera type 2) as the clear runs it: its timeline
/// (end frame, camera cuts, fades), game-speed commands (opcode 0x20:
/// frame, speed) and `SeqBlur` switches; the event frame it is at and the
/// base its camera is placed on.
pub struct Overlay {
    timeline: ::event::timeline::Timeline,
    speeds: Vec<(f32, f32)>,
    blur: Vec<(i32, u8)>,
    /// `SeqCameraCollision` (opcode 0x46, FUN_1402d34a0): the camera is
    /// kept out of the map.
    collision: bool,
    frame: Option<f32>,
    /// FUN_140406a90's base: the anchor's scene yaw, taken once as the
    /// overlay starts (states 3 / 4 copy the object's matrix), and its
    /// position, followed every frame (states 7 / 8).
    anchor: Option<(f32, Vec3)>,
}

/// A running overlay's camera in the scene frame, for `camera_control`.
#[derive(Clone, Copy, Debug)]
pub struct OverlayView {
    pub eye: Vec3,
    pub target: Vec3,
    pub roll: f32,
    pub fov: f32,
    /// Opcode 0x46: kept out of the map's camera collision.
    pub collision: bool,
}

/// What an overlay's base follows (FUN_140406bc0's state).
#[derive(Clone, Copy)]
enum Anchor {
    /// States 3 then 7: the object FUN_1403a35c0 named (0x236, Sora): its
    /// matrix, then its position (FUN_1403b5b40).
    Sora,
    /// States 4 then 8: the dead boss (DAT_142ae7d30) matrix, then the
    /// position of his death effect (DAT_142ae7d38, FUN_1402be970).
    DeadBoss,
}

impl Overlay {
    /// An overlay based on a fixed place (a big chest's event, FUN_1403db2f0).
    pub fn at(bytes: &[u8], yaw: f32, place: Vec3) -> Option<Self> {
        let mut o = Self::read(bytes)?;
        o.anchor = Some((yaw, place));
        o.frame = Some(0.);
        Some(o)
    }

    /// The view at event frame `frame`, None once it has ended.
    pub fn view_at(&mut self, frame: f32) -> Option<OverlayView> {
        self.frame = Some(frame);
        self.camera()
    }

    fn read(bytes: &[u8]) -> Option<Self> {
        let commands = data::event_script::read(bytes).ok()?;
        let mut speeds = Vec::new();
        let collision = commands
            .iter()
            .any(|c| matches!(c, data::event_script::Command::Other { opcode: 0x46, .. }));
        for c in &commands {
            // GameSpeed: u16 frame, u16, f32 speed (DAT_140717424).
            if let data::event_script::Command::Other {
                opcode: 0x20,
                payload,
            } = c
                && payload.len() >= 8
            {
                speeds.push((
                    f32::from(u16::from_le_bytes([payload[0], payload[1]])),
                    f32::from_le_bytes(payload[4..8].try_into().ok()?),
                ));
            }
        }
        Some(Overlay {
            timeline: ::event::timeline::Timeline::new(&commands),
            speeds,
            blur: crate::cutscene::blur::Blur::switches(&commands),
            collision,
            frame: None,
            anchor: None,
        })
    }

    fn end(&self) -> f32 {
        self.timeline.end_frame as f32
    }

    fn running(&self) -> bool {
        self.frame.is_some_and(|f| f < self.end())
    }

    /// The speed the latest passed command set, if any.
    fn speed(&self) -> Option<f32> {
        let f = self.frame?;
        self.speeds
            .iter()
            .filter(|(at, _)| *at <= f)
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map(|s| s.1)
    }

    /// The `SeqBlur` weight (previous picture over the scene) at its frame.
    fn blur(&self) -> Option<f32> {
        let f = self.frame.filter(|_| self.running())?;
        let a = self
            .blur
            .iter()
            .rev()
            .find(|(at, _)| *at as f32 <= f)
            .map_or(0, |s| s.1);
        (a != 0).then_some(f32::from(a) / 128.)
    }

    /// The overlay's camera in the scene (FUN_1402d37b0): the event camera
    /// at its frame (FUN_1402d3d40, which negates y and z: in the room's
    /// scene frame the channels read as they are) times the base matrix
    /// (FUN_14013f900 with DAT_140b64f90): the anchor's engine yaw (pi -
    /// scene yaw) turns it and its position moves it. [I] The roll is
    /// negated for the room's right-handed view.
    fn camera(&self) -> Option<OverlayView> {
        let f = self.frame.filter(|_| self.running())?;
        let (yaw, at) = self.anchor?;
        let cut = self.timeline.cut_at(f)?;
        let data = self.timeline.camera(cut.camera)?;
        let seconds =
            (f - cut.start as f32).min((cut.end - cut.start) as f32) / crate::cutscene::EVENT_FPS;
        let v = ::event::event_camera::view(data, seconds);
        let turn = Quat::from_rotation_y(yaw - std::f32::consts::PI);
        Some(OverlayView {
            eye: at + turn * Vec3::from_array(v.eye),
            target: at + turn * Vec3::from_array(v.target),
            roll: -v.roll.to_radians(),
            fov: v.fov.to_radians(),
            collision: self.collision,
        })
    }
}

impl Mission {
    /// Reads the area's mission BAR. `None` when it is absent.
    pub fn load(source: &data::probe_source::ProbeSource, name: &str) -> Option<Mission> {
        let mut library = data::path_lookup::Library::open(source).ok()?;
        let bar_bytes = library.read(&format!("msn/us/{name}.bar")).ok()?;
        let table = data::bar::read_bar(&bar_bytes).ok()?;
        let entry = |k: usize| {
            let e = table.entries.get(k)?;
            bar_bytes.get(e.offset as usize..e.offset as usize + e.size as usize)
        };
        let record_index = table.entries.iter().position(|e| e.entry_type == 2)?;
        let rec = entry(record_index)?;
        let script = table
            .entries
            .iter()
            .position(|e| e.entry_type == 3)
            .and_then(entry)
            .map(|payload| enemy_script::ScriptBrain::mission(payload.to_vec()))
            .transpose()
            .map_err(|e| eprintln!("{name}: {e}"))
            .ok()?
            .map(Box::new);
        let u16_at = |o: usize| Some(u16::from_le_bytes(rec.get(o..o + 2)?.try_into().ok()?));
        let u32_at = |o: usize| Some(u32::from_le_bytes(rec.get(o..o + 4)?.try_into().ok()?));
        // Sub-records from +0x1c: {u16 kind, u16 count, count * 4 bytes}
        // up to kind 0; kind 2 names the boss overlay's entry (FUN_1403a31a0).
        let mut boss_entry = None;
        let mut at = 0x1c;
        while let (Some(kind), Some(count)) = (u16_at(at), u16_at(at + 2)) {
            if kind == 0 {
                break;
            }
            if kind == 2 {
                boss_entry = rec.get(at + 4).map(|&b| b as usize);
            }
            at += 4 + 4 * count as usize;
        }
        let boss_event = boss_entry.and_then(entry).and_then(Overlay::read);
        let clear_event = library.read("00battle.bin").ok().and_then(|b| {
            let t = data::bar::read_bar(&b).ok()?;
            let e = t.entries.iter().filter(|e| e.entry_type == 22).nth(2)?;
            Overlay::read(b.get(e.offset as usize..e.offset as usize + e.size as usize)?)
        });
        Some(Mission {
            script,
            flags: u16_at(4)?,
            bonus: *rec.get(0xd)?,
            clear_sound: u32_at(0x14)?,
            boss_event,
            clear_event,
            state: 0,
        })
    }

    /// The fade the running overlays draw (at their own event frames).
    pub fn fade(&self) -> Option<[f32; 4]> {
        let mut out = None;
        for o in [&self.boss_event, &self.clear_event].into_iter().flatten() {
            let Some(f) = o.frame else {
                continue;
            };
            for layer in 0..3 {
                if let Some(c) =
                    crate::cutscene::screen::layer_colour(&o.timeline.fades, layer, 0, f)
                {
                    let v = if c.white { 1. } else { 0. };
                    out = Some([v, v, v, c.alpha]);
                }
            }
        }
        out
    }
}

/// The mission's per-frame task (FUN_1403a35c0), run every fixed step even
/// while the field is slowed: it loads the area's mission once, applies its
/// record (battle style 2), takes a boss's death signal, runs the clear's
/// overlays and sets the game speed they ask for (DAT_140717424), which
/// gates the field's steps.
pub fn mission_tick(
    mut game: ResMut<Game>,
    area: Option<Res<AreaRes>>,
    mut gallery: ResMut<enemy_gallery::EnemyGallery>,
) {
    if game.mission.is_none() && !game.mission_tried {
        game.mission_tried = true;
        if let Some((id, name)) = area
            .as_ref()
            .and_then(|a| a.current.state.setup.missions.first().cloned())
        {
            match Mission::load(&gallery.source, &name) {
                Some(m) => {
                    if m.flags & 1 != 0 {
                        game.battle_style = 2;
                    }
                    game.event(serde_json::json!({"event":"mission_start","id":id,"name":name,
                        "flags":m.flags,"bonus":m.bonus,"clear_sound":m.clear_sound,
                        "boss_overlay":m.boss_event.is_some(),"clear_overlay":m.clear_event.is_some()}));
                    game.mission = Some(m);
                }
                None => {
                    game.event(serde_json::json!({"event":"mission_missing","id":id,"name":name}))
                }
            }
        }
    }
    medal_tick(&mut game, &mut gallery);
    reserved_signals(&mut game, &mut gallery);
    let tick = game.tick;
    // `KH2RW_TOUR_SIGNAL=<tick>:<code>[:<arg>]`: FUN_1403aabe0 is called then.
    if let Ok(v) = std::env::var("KH2RW_TOUR_SIGNAL") {
        let n: Vec<i32> = v.split(':').filter_map(|p| p.trim().parse().ok()).collect();
        if n.first() == Some(&(tick as i32)) && n.len() >= 2 {
            game.mission_signals
                .push((n[1], n.get(2).copied().unwrap_or(0)));
        }
    }
    let signals = std::mem::take(&mut game.mission_signals);
    for &(code, arg) in &signals {
        game.event(serde_json::json!({"event":"mission_signal","tick":tick,"code":code,"arg":arg}));
    }
    // FUN_1403aabe0: the mission's trigger 10 first, then the area's entries.
    if story_events::enabled() {
        game.story_signals.extend(&signals);
    }
    // The mission's script: the signals reach its trigger 10, then its main
    // thread and the threads it started run, with the fighters' objects in its
    // list. Its calls come after.
    if let Some(mut script) = game.mission.as_mut().and_then(|m| m.script.take()) {
        let mut rng = game
            .enemy
            .as_mut()
            .map_or_else(Default::default, |e| std::mem::take(&mut e.rng));
        let player = game.sora.player.position;
        for actor in &mut gallery.actors {
            if let Some(other) = enemy_battle::fighter_brain(actor) {
                other.lend_objects(&mut script);
            }
        }
        for (code, arg) in signals {
            script.mission_signal(&game.world, player, &mut rng, code, arg);
        }
        let calls = script.mission_tick(&game.world, player, &mut rng);
        for actor in &mut gallery.actors {
            if let Some(other) = enemy_battle::fighter_brain(actor) {
                other.take_back_objects(&mut script);
            }
        }
        if let Some(e) = game.enemy.as_mut() {
            e.rng = rng;
        }
        if let Some(fault) = script.fault.take() {
            game.event(serde_json::json!({"event":"mission_fault","tick":tick,"fault":fault}));
        }
        // The mission's own 1:41 calls raise signals too; they reach its
        // trigger 10 on the next step.
        game.mission_signals.append(&mut script.signals);
        if let Some(m) = game.mission.as_mut() {
            m.script = Some(script);
        }
        for call in calls {
            game.event(serde_json::json!({"event":"mission_call","tick":tick,
                "call":format!("{call:?}")}));
            match call {
                // [I] FUN_1404334c0 and FUN_1404334f0 have overlays and jumps
                // of their own, not carried out: the fight ends as for a
                // boss's death.
                enemy::world::MissionCall::DeadBoss(_) | enemy::world::MissionCall::Complete(_) => {
                    dead_boss(&mut game)
                }
                enemy::world::MissionCall::Exit(_) => game.mission_cleared = true,
                // [U] thunk_FUN_1403a3800's sound, game-over overlays and jump.
                enemy::world::MissionCall::Failed => game.mission_failed = true,
            }
        }
    }
    // FUN_1403a35c0's states 3 and 4. Each overlay's frame advances at the
    // event rate (30 per second, half a frame per tick) times the game
    // speed (frame time).
    let speed = game.game_speed;
    let step = crate::cutscene::EVENT_FPS / crate::cutscene::TICK_HZ as f32 * speed;
    let sora = (
        game.sora.player.yaw,
        Vec3::from_array(game.sora.player.position),
    );
    let boss = gallery
        .actors
        .iter()
        .find(|a| {
            a.battle.as_ref().is_some_and(|b| {
                matches!(b.brain, enemy_battle::Brain::Script(_)) && b.brain.hp() <= 0
            })
        })
        .and_then(|a| {
            let b = a.battle.as_ref()?;
            // [I] The death effect rides bone 161, the bone of his TARGET
            // shape; its centre stands in for the effect's position.
            let at = b.pose.targets.first().map(|s| s.centre)?;
            Some((a.yaw, at))
        });
    let mut done = false;
    let mut camera = None;
    let mut blur = None;
    if let Some(m) = game.mission.as_mut() {
        for (o, anchor) in [
            (&mut m.boss_event, Anchor::DeadBoss),
            (&mut m.clear_event, Anchor::Sora),
        ] {
            let Some(o) = o.as_mut() else {
                continue;
            };
            let end = o.end();
            if let Some(f) = o.frame.as_mut()
                && *f < end
            {
                *f += step;
                let (yaw, at) = match anchor {
                    Anchor::Sora => Some(sora),
                    Anchor::DeadBoss => boss,
                }
                .map_or((None, None), |(y, p)| (Some(y), Some(p)));
                // The matrix is taken once; the position follows.
                o.anchor = match (o.anchor, yaw, at) {
                    (Some((y, _)), _, Some(p)) => Some((y, p)),
                    (None, Some(y), Some(p)) => Some((y, p)),
                    (old, ..) => old,
                };
            }
            // The later overlay's camera and blur win (the clear one draws
            // over the boss one's last frames).
            if let Some(c) = o.camera() {
                camera = Some(c);
            }
            if let Some(w) = o.blur() {
                blur = Some((w, o.frame.unwrap_or(0.).floor() as i32));
            }
        }
        if m.state == 3 {
            // The clear overlay starts once the boss one has four frames
            // (DAT_140623b90) or fewer left.
            let left = m
                .boss_event
                .as_ref()
                .filter(|o| o.running())
                .map_or(0., |o| o.end() - o.frame.unwrap_or(0.));
            if left <= 4. {
                if let Some(o) = m.clear_event.as_mut() {
                    o.frame = Some(0.);
                }
                m.state = 4;
            }
        } else if m.state == 4
            && !m.boss_event.as_ref().is_some_and(Overlay::running)
            && !m.clear_event.as_ref().is_some_and(Overlay::running)
        {
            // FUN_1403a3540(0x10, -1): the area's signal-0x10 entry (event
            // 602 and its jump) takes over.
            m.state = 5;
            done = true;
        }
        let speeds = [&m.boss_event, &m.clear_event]
            .into_iter()
            .flatten()
            .filter(|o| o.running())
            .filter_map(Overlay::speed)
            .next_back();
        game.game_speed = speeds.unwrap_or(1.);
    }
    if let Some(view) = camera
        && tick.is_multiple_of(15)
    {
        let frames: Vec<_> = game
            .mission
            .iter()
            .flat_map(|m| [m.boss_event.as_ref(), m.clear_event.as_ref()])
            .map(|o| o.and_then(|o| o.frame.zip(o.anchor)))
            .collect();
        game.event(serde_json::json!({"event":"overlay_camera","tick":tick,
            "eye":view.eye.to_array(),"target":view.target.to_array(),"roll":view.roll,
            "fov":view.fov,"blur":blur.map(|b| b.0),"overlays":format!("{frames:?}")}));
    }
    game.overlay_camera = camera;
    game.overlay_blur = blur;
    if done {
        game.mission_cleared = true;
        game.event(serde_json::json!({"event":"mission_clear","tick":tick}));
    }
    // A slowed field steps once its accumulated speed reaches a whole step.
    game.speed_carry += game.game_speed;
    game.field_step = game.speed_carry >= 1.;
    if game.field_step {
        game.speed_carry -= 1.;
    }
}

/// FUN_1403aaf30, every frame: a signal reserved on an object goes out when its
/// HP is down to its floor (kind 1). FUN_1403aabe0 then drops every reservation
/// of that code. [U] Kind 0, on an HP rate (FUN_1403ab0c0), is not ported.
fn reserved_signals(game: &mut Game, gallery: &mut enemy_gallery::EnemyGallery) {
    let store = enemy_script::fighters_teamwork();
    let Ok(mut team) = store.lock() else {
        return;
    };
    if team.reserved.is_empty() {
        return;
    }
    let mut fired = Vec::new();
    for r in &team.reserved {
        let down = gallery
            .actors
            .iter_mut()
            .filter_map(enemy_battle::fighter_brain)
            .find(|b| b.object_id() == r.object)
            .is_some_and(|b| b.hp_at_floor());
        if down {
            fired.push((r.code, r.argument));
        }
    }
    team.reserved
        .retain(|r| !fired.iter().any(|&(code, _)| code == r.code));
    game.mission_signals.extend(fired);
}

/// FUN_14040b9e0's reach: a flying medal arrives within this of its object.
const MEDAL_REACH: f32 = 100.;
/// FUN_14040b9e0: its speed toward the object (DAT_140623bc8) and the share of
/// its velocity it keeps (DAT_1406239f8).
const MEDAL_SPEED: f32 = 20.;
const MEDAL_KEEP: f32 = 0.1;

/// The medals of the Grim Reaper's fight. A free one falls as a prize does and
/// Sora collects it when it has landed; a flying one homes on its object
/// (FUN_14040b9e0) and adds to its count there. [I] Sora's collecting them by
/// the ordinary prizes' rule: FUN_14040b8b0 hands them to the player's vt+0x160,
/// which is not traced.
pub fn medal_tick(game: &mut Game, gallery: &mut enemy_gallery::EnemyGallery) {
    let store = enemy_script::fighters_teamwork();
    let Ok(mut team) = store.lock() else {
        return;
    };
    if team.medals.prizes.is_empty() {
        return;
    }
    let sora = game.sora.player.position;
    let draw = game.sora.player.abilities.draw;
    let medals = std::mem::take(&mut team.medals.prizes);
    let mut kept = Vec::with_capacity(medals.len());
    for mut m in medals {
        let Some(target) = m.target else {
            let [x, y, z] = m.orb.position;
            let floor = game.world.floor(x, z, y + 30.).unwrap_or(0.);
            m.orb.tick(floor);
            if m.orb.collected_by(sora, draw) && !game.sora.player.dead() {
                team.medals.player += 1;
            } else {
                kept.push(m);
            }
            continue;
        };
        m.orb.age = m.orb.age.saturating_add(1);
        let brain = gallery
            .actors
            .iter_mut()
            .filter_map(enemy_battle::fighter_brain)
            .find(|b| b.object_id() == target);
        let Some(brain) = brain else {
            continue;
        };
        let to = Vec3::from_array(brain.position()) - Vec3::from_array(m.orb.position);
        if to.length() < MEDAL_REACH {
            brain.add_medal(1);
            continue;
        }
        let v = to.normalize_or_zero() * MEDAL_SPEED;
        let velocity = v.lerp(Vec3::from_array(m.orb.velocity), MEDAL_KEEP);
        m.orb.velocity = velocity.to_array();
        m.orb.position = (Vec3::from_array(m.orb.position) + velocity).to_array();
        kept.push(m);
    }
    team.medals.prizes = kept;
}

/// FUN_1404334d0 (`trap_mission_dead_boss`): the bonus, the clear sound, the
/// won state (DAT_142a11400 bit 1: commands locked, Sora unhurt) and the boss
/// overlay ("re_deadboss-0a"). [U] The HUD reset, the pause definitions and
/// the overlays' opcodes 0x41/0x46/0x51 are not carried out.
fn dead_boss(game: &mut Game) {
    let Some(m) = game.mission.as_mut() else {
        return;
    };
    let (bonus, sound) = (m.bonus, m.clear_sound);
    if let Some(o) = m.boss_event.as_mut() {
        o.frame = Some(0.);
    }
    m.state = 3;
    game.battle_won = true;
    if sound != 0 {
        // FUN_1401de0c0: bank 0 sound `sound`.
        game.sound_queue.push((sound, "mission_clear"));
    }
    if bonus != 0
        && let Some(bons) = game.drive.as_ref().map(|d| d.src.growth.bons.clone())
    {
        let Game { enemy, sora, .. } = game;
        if let Some(rt) = enemy.as_mut() {
            let _ = level_growth::apply_bonus(rt, sora, &bons, bonus);
        }
    }
    let tick = game.tick;
    game.event(
        serde_json::json!({"event":"mission_dead_boss","tick":tick,"bonus":bonus,"sound":sound}),
    );
}
