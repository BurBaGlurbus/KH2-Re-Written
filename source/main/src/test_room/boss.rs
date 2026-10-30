//! The host side of the boss framework: what a boss's object script asks of
//! the engine beyond an ordinary enemy. Sora played by the boss's own motions
//! (`obj_motion_capture`, FUN_1403c6cc0: a slot of the captor's set holds the
//! victim's body and Keyblade clips), the motions it queues behind them, and
//! the bosses whose sets hold such slots.
use super::*;

/// The bosses whose scripts capture Sora, and the slots of their motion sets
/// they capture him into: Sephiroth (N_HB630.mset 252 AQ90 the Block pose,
/// 253 AQ90 the Block counter, 254 AQ91 the grab).
/// [U] M_EX770 (objentry 306) captures Sora into 252, 253 (`rc_catch`,
/// whose body motion holds range 25 = 28, the partner's reaction command),
/// 255 and 256, but its motion slot 202 has a clock the room's enemy motion
/// does not admit, so it is not fielded here.
pub const CAPTURES: &[(u32, &[usize])] = &[(enemy_battle::SEPHIROTH, &[252, 253, 254])];

/// The id Sora's kit keeps a boss's captured motion under: below the
/// summons' (0x7800..) and above the limits' (0x4000..).
pub fn capture_clip(object: u32, slot: i32) -> Option<u16> {
    let (index, (_, slots)) = CAPTURES
        .iter()
        .enumerate()
        .find(|(_, (o, _))| *o == object)?;
    let slot = usize::try_from(slot).ok().filter(|s| slots.contains(s))?;
    Some(0x7000 | (index as u16) << 8 | (slot - 240) as u16)
}

/// Sora's captured motions of every boss in [`CAPTURES`] for his kit: (clip
/// id, body motion, Keyblade motion).
pub fn capture_motions(
    source: &data::probe_source::ProbeSource,
) -> Vec<(
    u16,
    data::motion::SelectedMotion,
    Option<data::motion::SelectedMotion>,
)> {
    let mut out = Vec::new();
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
    for &(object, slots) in CAPTURES {
        let Some(entry) = objects.iter().find(|o| o.object_id == object) else {
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
        for &slot in slots {
            let Some(clip) = capture_clip(object, slot as i32) else {
                continue;
            };
            let Some(anb) = table
                .entries
                .get(slot)
                .filter(|e| e.offset > 0 && e.size > 0)
                .and_then(|e| bytes.get(e.offset as usize..e.offset as usize + e.size as usize))
            else {
                continue;
            };
            match summon::capture_parts(anb, clip) {
                Some((body, weapon)) => out.push((clip, body, weapon)),
                None => eprintln!("{path}: slot {slot} holds no motion for Sora; left out"),
            }
        }
    }
    out
}

/// The lengths of a boss's captured motions in Sora's kit, which its script
/// reads through `obj_motion_capture` (how long the capture lasts).
pub fn capture_lengths(clips: &player::Clips, object: u32) -> HashMap<usize, u32> {
    CAPTURES
        .iter()
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
        // FUN_1403aabe0's signals go to the mission's trigger 10.
        game.mission_signals.append(&mut a.signals);
        if !captures.is_empty() {
            // [I] The partner (+0x2b0) the search offers range 25's command
            // for is the object holding Sora's capture: its trigger 11 takes
            // the command (M_EX770's handles 27 and 28).
            game.reaction.captor = Some(actor.id as u32);
        }
        play_on_sora(game, object, &captures, &pushed);
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
/// (BAR type 2) and the overlay events it names, run by the mission
/// manager's per-frame task FUN_1403a35c0. The mission script (`ms_boss`)
/// is not run: a boss's death signal (code 4) is what its trigger 10 turns
/// into `trap_mission_dead_boss` (FUN_1404334d0), translated here.
pub struct Mission {
    /// +0x04: flag bits (bit 0 forces battle style 2, FUN_1403ab690).
    pub flags: u16,
    /// +0x0d: the bonus the clear grants; +0x14 the clear sound.
    pub bonus: u8,
    pub clear_sound: u32,
    /// The kind-2 sub-record's overlay ("re_deadboss-0a") and 00battle.bin's
    /// third event ("re_deadboss2-0a", FUN_1403f4b10(2)).
    boss_event: Option<Overlay>,
    clear_event: Option<Overlay>,
    /// `trap_mission_lock` (DAT_142a10500 bit 0): the death signal was taken.
    pub locked: bool,
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
            flags: u16_at(4)?,
            bonus: *rec.get(0xd)?,
            clear_sound: u32_at(0x14)?,
            boss_event,
            clear_event,
            locked: false,
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
    gallery: Res<enemy_gallery::EnemyGallery>,
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
    let signals = std::mem::take(&mut game.mission_signals);
    let tick = game.tick;
    for (code, arg) in signals {
        // ms_boss trigger 10: code 4 (a boss died) locks the mission and
        // runs `trap_mission_dead_boss(-1)` (FUN_1404334d0).
        let start = game
            .mission
            .as_mut()
            .is_some_and(|m| code == 4 && !std::mem::replace(&mut m.locked, true));
        game.event(serde_json::json!({"event":"mission_signal","tick":tick,"code":code,"arg":arg,"clear":start}));
        if start {
            dead_boss(&mut game);
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
