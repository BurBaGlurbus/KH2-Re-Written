//! The debug menu's Outfits and Crown pages: every Sora, Roxas, Donald and
//! Goofy object the install has, worn live, and the crown on Sora's model. A
//! development aid; the choice of outfit is not a game rule.
//!
//! The list comes from the object table (`00objentry.bin`) and the files
//! installed, not from a fixed list. A world costume's Drive forms come from
//! its `memt` row, as FUN_140405190 picks them. Entering a room wears the
//! place's bodies (`enter_place`), so a menu's choice lasts until then.
//!
//! The crown is a Final Mix costume rule (FUN_1403a9e20): the number of Proofs
//! held picks a TEXA animation of the model (`CROWN_ANIMATIONS`), and
//! FUN_1403cd8a0 / FUN_1401a87a0 start it. It paints a 64x64 patch of the
//! crown's own texture page, whose last image is empty. Only models with those
//! animations have a crown: base Sora, his forms and his scene bodies, not the
//! world costumes or Roxas.
//!
//! A gauge's face is the object's own (FUN_14017e980 → FUN_1403b57b0: the type
//! 0x18 image named `face` in the object's archive), so a form or a costume
//! changes the HUD's icon with the object (`hud_faces`).
use super::*;
use data::model_inspection_textures::{
    ImageIdentity, InspectionTextures, TextureMode, TexturePlay,
};
use data::objentry::ObjectEntry;

/// DAT_1405c4c98: Proof of Connection, Nonexistence and Peace.
pub const PROOFS: [u16; 3] = [0x251, 0x252, 0x253];
/// DAT_1405c4ca0: the TEXA animation number for 0..=3 Proofs held.
pub const CROWN_ANIMATIONS: [u16; 4] = [0xa4, 0x9a, 0x9b, 0x9c];
pub const CROWN_NAMES: [&str; 4] = ["No crown", "Copper crown", "Silver crown", "Gold crown"];

/// Sections of the Outfits page, in the order they are listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Sora,
    Halloween,
    Christmas,
    Space,
    Timeless,
    Animal,
    Scene,
    Event,
    Roxas,
    Donald,
    Goofy,
}

impl Group {
    /// The party member wearing this section's bodies (objentry +0x4c).
    pub fn member(self) -> Option<u8> {
        match self {
            Self::Donald => Some(2),
            Self::Goofy => Some(3),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Sora => "Sora and Drive forms",
            Self::Halloween => "Halloween Town",
            Self::Christmas => "Christmas Town",
            Self::Space => "Space Paranoids",
            Self::Timeless => "Timeless River",
            Self::Animal => "Pride Lands and Atlantica",
            Self::Scene => "Rides and scene bodies",
            Self::Event => "Cutscene models",
            Self::Roxas => "Roxas",
            Self::Donald => "Donald",
            Self::Goofy => "Goofy",
        }
    }
}

/// One row of the Outfits page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outfit {
    pub object_id: u32,
    pub model: String,
    pub label: String,
    pub group: Group,
}

/// The Drive form a model name's suffix names.
fn form_of(suffix: &str) -> Option<&'static str> {
    Some(match suffix {
        "" => "Sora",
        "BTLF" => "Valor Form",
        "MAGF" => "Wisdom Form",
        "KH1F" => "Limit Form",
        "TRIF" => "Master Form",
        "ULTF" => "Final Form",
        "HTLF" => "Antiform",
        _ => return None,
    })
}

/// A `P_EX100` or `H_EX500` family model: its world costume's section and its
/// name, or `None` for a suffix outside the family's pattern.
fn family(rest: &str) -> Option<(Group, String)> {
    let (group, rest) = match rest.split_once('_') {
        Some(("NM", rest)) => (Group::Halloween, rest),
        Some(("XM", rest)) => (Group::Christmas, rest),
        Some(("TR", rest)) => (Group::Space, rest),
        Some(("WI", rest)) => (Group::Timeless, rest),
        None if rest == "NM" => (Group::Halloween, ""),
        None if rest == "XM" => (Group::Christmas, ""),
        None if rest == "TR" => (Group::Space, ""),
        None if rest == "WI" => (Group::Timeless, ""),
        _ => (Group::Sora, rest),
    };
    let form = form_of(rest)?;
    Some((
        group,
        match group {
            Group::Sora => form.to_owned(),
            other => format!("{form}, {}", other.label()),
        },
    ))
}

/// A Donald or Goofy body: its section and name. Character ids 2 and 3 name the
/// bodies that fight beside Sora, and the rest are the same members' scene
/// bodies.
fn party_body(object: &ObjectEntry, model: &str) -> Option<(Group, String)> {
    let (group, name) = match object.character {
        2 => (Group::Donald, "Donald"),
        3 => (Group::Goofy, "Goofy"),
        _ => match model {
            "P_EX020_ANGRY_NPC" | "P_EX020_WM" => (Group::Donald, "Donald"),
            "P_EX030_WM" => (Group::Goofy, "Goofy"),
            _ => return None,
        },
    };
    let place = match model {
        "P_EX020" | "P_EX030" => None,
        "P_EX020_NM" | "P_EX030_NM" => Some("Halloween Town"),
        "P_EX020_XM" | "P_EX030_XM" => Some("Christmas Town"),
        "P_EX020_TR" | "P_EX030_TR" => Some("Space Paranoids"),
        "P_WI020" | "P_WI030" => Some("Timeless River"),
        "P_LK020" | "P_LK030" => Some("Pride Lands"),
        "P_EX020_WM" | "P_EX030_WM" => Some("world map"),
        "P_EX020_ANGRY_NPC" => Some("angry"),
        "N_LM010_PLY" | "N_LM020_PLY" => Some("Atlantica"),
        // Anything else the table adds.
        _ => return Some((group, model.to_owned())),
    };
    Some((
        group,
        place.map_or_else(|| name.to_owned(), |p| format!("{name}, {p}")),
    ))
}

/// The section and the friendly name of a model the page lists, or `None` for
/// a model that is not a body of Sora or Roxas. A model with no known name
/// shows its raw name; its model code is always in the value column.
fn describe(object: &ObjectEntry, model: &str) -> Option<(Group, String)> {
    if let Some(body) = party_body(object, model) {
        return Some(body);
    }
    if let Some(rest) = model.strip_prefix("P_EX100") {
        let rest = rest.strip_prefix('_').unwrap_or(rest);
        if let Some((group, name)) = family(rest) {
            return Some((group, name));
        }
        let name = match rest {
            "NPC" => "Sora, as an NPC",
            "MEMO" => "Sora, Journal pose",
            "WM" => "Sora, world map",
            "LAST" => "Sora, last battle",
            "AL_CARPET" => "Sora, Magic Carpet",
            "TR_LIGHTCYCLE" => "Sora, Light Cycle",
            "SIDECAR" => "Sora, sidecar",
            "HTLF_BTL" => "Antiform, as an enemy",
            _ => model,
        };
        return Some((Group::Scene, name.to_owned()));
    }
    if let Some(rest) = model.strip_prefix("H_EX500") {
        let rest = rest.strip_prefix('_').unwrap_or(rest);
        return Some((Group::Event, family(rest)?.1));
    }
    let named = |group, name: &str| Some((group, name.to_owned()));
    match model {
        "P_LK100" => named(Group::Animal, "Sora, Lion"),
        "P_LM100" => named(Group::Animal, "Sora, Mermaid"),
        "PLAYER" | "ACTOR_SORA" | "TR_PLAYER" | "AL14_PLAYER" | "PO06_PLAYER" | "PO07_PLAYER"
        | "PO08_PLAYER" | "WM_EX100" => named(Group::Scene, model),
        "ACTOR_SORA_H" | "H_EX501" => named(Group::Event, model),
        "P_EX120" => named(Group::Sora, "Sora, first game's outfit"),
        "P_EX120_NPC_RED" => named(Group::Sora, "First game's outfit, red"),
        "P_EX120_NPC_GREEN" => named(Group::Sora, "First game's outfit, green"),
        "P_EX120_NPC_BLUE" => named(Group::Sora, "First game's outfit, blue"),
        "P_EX130" => named(Group::Roxas, "Roxas, Organization coat"),
        "P_EX110" => named(Group::Roxas, "Roxas"),
        "P_EX110_BTLF" => named(Group::Roxas, "Roxas, two Keyblades"),
        "P_EX110_NPC_PAJAMAS" => named(Group::Roxas, "Roxas, pajamas"),
        "H_EX510" => named(Group::Roxas, "Roxas, cutscene"),
        "H_EX510_PAJAMAS" => named(Group::Roxas, "Roxas, pajamas, cutscene"),
        _ if model.starts_with("P_EX1") => named(Group::Roxas, model),
        // Any other object the table gives Sora's or Roxas's character id.
        _ if object.character == 1 && !model.starts_with("M_") => named(Group::Scene, model),
        _ if object.character == 14 => named(Group::Roxas, model),
        _ => None,
    }
}

/// Every installed body of Sora and Roxas, by section, in table order. Rows
/// that `installed` denies are left out.
pub fn catalog(objects: &[ObjectEntry], installed: impl Fn(&str) -> bool) -> Vec<Outfit> {
    let mut list: Vec<Outfit> = objects
        .iter()
        .filter_map(|object| {
            let model = String::from_utf8_lossy(&object.model_name).into_owned();
            let (group, label) = describe(object, &model)?;
            installed(&format!("obj/{model}.mdlx")).then_some(Outfit {
                object_id: object.object_id,
                model,
                label,
                group,
            })
        })
        .collect();
    list.sort_by_key(|o| o.group);
    list
}

/// The motion sets an outfit can play, best first: the object's own, a set
/// named after the model, then its family's for a body with none of its own
/// (they share a skeleton; the kit's evaluation rejects any that does not).
pub fn motion_sets(object: &ObjectEntry, model: &str) -> Vec<String> {
    let own = String::from_utf8_lossy(&object.animation_name).into_owned();
    let family = if model.starts_with("P_EX110") || model.starts_with("H_EX510") {
        "P_EX110.mset"
    } else if model.starts_with("P_EX120") {
        "P_EX120.mset"
    } else {
        "P_EX100.mset"
    };
    [own, format!("{model}.mset"), family.to_owned()]
        .into_iter()
        .filter(|name| !name.is_empty())
        .collect()
}

/// FUN_1403a9e20's count: the Proofs in `stock`.
pub fn proofs_held(stock: impl Fn(u16) -> u8) -> usize {
    PROOFS.iter().filter(|&&id| stock(id) > 0).count()
}

/// The crown patch of a model: the TEXA chunk whose id map holds the crown
/// animations.
pub fn crown_patch(set: &InspectionTextures) -> Option<&TexturePlay> {
    set.play.iter().find(|p| {
        CROWN_ANIMATIONS
            .iter()
            .all(|&n| p.patch.animation_for(n).is_some())
    })
}

/// `play`'s page with the image of `number` on it, as `(width, height, rgba)`.
/// The remaster keeps a TEXA chunk's images as one vertical strip at a whole
/// multiple of the patch; the original page is scaled to match it, so the
/// model's UVs still address it.
pub fn texa_page(
    set: &InspectionTextures,
    play: &TexturePlay,
    number: u16,
) -> Option<(u32, u32, Vec<u8>)> {
    let patch = &play.patch;
    let image = patch.image_at(patch.animation_for(number)?, 0.)?;
    texa_image_page(set, play, image)
}

/// `play`'s page with its image `image` on it.
pub fn texa_image_page(
    set: &InspectionTextures,
    play: &TexturePlay,
    image: u16,
) -> Option<(u32, u32, Vec<u8>)> {
    let patch = &play.patch;
    let page = set
        .bind(patch.environment, TextureMode::OriginalIndexed)
        .ok()?
        .image_index?;
    let page = &set.images[page].levels[0];
    let count = patch.images.len() as u32;
    let strip = set.images.iter().find_map(|i| {
        if !matches!(i.identity, ImageIdentity::Replacement { .. }) {
            return None;
        }
        let level = &i.levels[0];
        let k = level.width / patch.width.max(1);
        (k > 1 && level.width == patch.width * k && level.height == patch.height * k * count)
            .then_some((k, level))
    });
    if let Some((k, strip)) = strip {
        let (width, height) = (page.width * k, page.height * k);
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let from = (((y / k) * page.width + x / k) * 4) as usize;
                let to = ((y * width + x) * 4) as usize;
                if let (Some(s), Some(d)) = (
                    page.pixels_rgba.get(from..from + 4),
                    rgba.get_mut(to..to + 4),
                ) {
                    d.copy_from_slice(s);
                }
            }
        }
        let (fw, fh) = (patch.width * k, patch.height * k);
        for y in 0..fh {
            let from = (((u32::from(image) * fh + y) * fw) * 4) as usize;
            let to = (((patch.y * k + y) * width + patch.x * k) * 4) as usize;
            let row = (fw * 4) as usize;
            if let (Some(s), Some(d)) = (
                strip.pixels_rgba.get(from..from + row),
                rgba.get_mut(to..to + row),
            ) {
                d.copy_from_slice(s);
            }
        }
        return Some((width, height, rgba));
    }
    let mut rgba = page.pixels_rgba.clone();
    patch.paint(&mut rgba, page.width, image, &play.clut_rgba);
    Some((page.width, page.height, rgba))
}

/// The crown image a part wears, once it has been set.
#[derive(Component)]
pub struct CrownPart(u16);

/// Dresses Sora's crown parts in the image of the current animation number:
/// the debug menu's choice, else the Proofs held.
#[allow(clippy::too_many_arguments)]
pub fn crown(
    mut commands: Commands,
    game: Res<Game>,
    camp: Option<Res<CampRes>>,
    parts: Query<(Entity, &SoraPart, Option<&CrownPart>)>,
    mut materials: ResMut<Assets<FogMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut cache: Local<HashMap<(usize, u16), Handle<FogMaterial>>>,
) {
    let Some(d) = game.drive.as_ref() else { return };
    let Some((actor, ..)) = d.models.get(&d.shown) else {
        return;
    };
    let Some(set) = actor.textures.as_ref() else {
        return;
    };
    let Some(play) = crown_patch(set) else { return };
    let held = crown_level(&game, camp.as_deref());
    let number = CROWN_ANIMATIONS[held];
    let key = (Arc::as_ptr(set) as usize, number);
    for (entity, part, worn) in &parts {
        let on_page = actor
            .texture_routes
            .first()
            .and_then(|r| r.parts.get(part.0))
            .is_some_and(|r| r.environment_ordinal == play.patch.environment);
        if !on_page || worn.is_some_and(|w| w.0 == number) {
            continue;
        }
        let material = cache.entry(key).or_insert_with(|| {
            let (width, height, rgba) = texa_page(set, play, number).unwrap_or((1, 1, vec![0; 4]));
            let texture = visual::room::static_rgba_image(width, height, rgba, &mut images);
            materials.add_fogged(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(texture),
                unlit: true,
                cull_mode: None,
                // The crown's patch has holes, and its last image is empty.
                alpha_mode: AlphaMode::Mask(0.5),
                ..default()
            })
        });
        commands
            .entity(entity)
            .insert((MeshMaterial3d(material.clone()), CrownPart(number)));
    }
}

/// The crown shown now, 0..=3: the menu's choice, else FUN_1403a9e20's count.
pub fn crown_level(game: &Game, camp: Option<&CampRes>) -> usize {
    if let Some(choice) = game.cheats.crown {
        return usize::from(choice).min(3);
    }
    let stock = game
        .items
        .as_ref()
        .map(|items| &items.stock)
        .or(camp.map(|c| &c.stock));
    proofs_held(|id| stock.and_then(|s| s.get(&id)).copied().unwrap_or(0))
}

/// The faces on the HUD: the body each gauge shows, and a body's face still
/// loading, for Sora and his two members.
#[derive(Default)]
pub struct FaceLoads {
    shown: [Option<String>; 3],
    loading: [Option<FaceLoad>; 3],
}

type FaceLoad = (String, std::thread::JoinHandle<Option<hud::Sheet>>);

/// Reads the `face` sheet of `model`'s own archive, if it has one.
fn load_face(source: &ProbeSource, model: &str) -> Option<hud::Sheet> {
    let library = data::path_lookup::Library::open(source).ok()?;
    let found = library.find(&format!("obj/{model}.a.us"))?;
    hud::load(source, found.index, found.ordinal, found.name_hash, b"face").ok()
}

/// Puts a loaded face on Sora's gauge (0) or a member's (1, 2).
fn install_face(hud: &mut Hud, slot: usize, face: hud::Sheet) {
    if slot == 0 {
        if let Some(sheet) = hud.sheets.get_mut(1) {
            *sheet = face;
            let sequences: Vec<_> = hud.sheets.iter().map(|s| &s.sequence).collect();
            hud.gauge.face_changed(&sequences);
        }
    } else {
        let member = slot - 1;
        match hud.friend_faces[member] {
            Some(at) => hud.sheets[at] = face,
            None => {
                hud.friend_faces[member] = Some(hud.sheets.len());
                hud.sheets.push(face);
            }
        }
        // A gauge holds its face's sheet from when it is made: made again.
        hud.friends[member] = None;
    }
    hud.updated = true;
}

/// Keeps each gauge's face on the body its owner wears now: a Drive form, a
/// costume or a body from the Outfits page. A body with no `face` of its own
/// keeps the last one. Loaded on a thread, like the kits.
pub fn hud_faces(mut hud: Option<ResMut<Hud>>, game: Res<Game>, mut faces: Local<FaceLoads>) {
    let (Some(hud), Some(d)) = (hud.as_mut(), game.drive.as_ref()) else {
        return;
    };
    let faces = &mut *faces;
    let wants = [
        d.shown_model(),
        game.party.first().map(|m| m.kind.name.to_owned()),
        game.party.get(1).map(|m| m.kind.name.to_owned()),
    ];
    for (slot, want) in wants.into_iter().enumerate() {
        let Some(want) = want else { continue };
        // The room started on these.
        let shown = faces.shown[slot].get_or_insert_with(|| {
            if slot == 0 {
                d.src.base_model().to_owned()
            } else {
                want.clone()
            }
        });
        if faces.loading[slot]
            .as_ref()
            .is_some_and(|(_, handle)| handle.is_finished())
            && let Some((model, handle)) = faces.loading[slot].take()
        {
            let face = handle.join().ok().flatten();
            // Another body may have been chosen while this one loaded.
            if model == want {
                *shown = model;
                if let Some(face) = face {
                    install_face(hud, slot, face);
                }
            }
        }
        if *shown != want && faces.loading[slot].is_none() {
            let source = d.source();
            let model = want.clone();
            faces.loading[slot] =
                Some((want, std::thread::spawn(move || load_face(&source, &model))));
        }
    }
}

/// Whether the model on screen can wear a crown.
pub fn wears_crown(game: &Game) -> bool {
    game.drive
        .as_ref()
        .and_then(|d| d.models.get(&d.shown))
        .and_then(|(actor, ..)| actor.textures.as_ref())
        .is_some_and(|set| crown_patch(set).is_some())
}

/// The bodies of the place the room entered (`AreaSource::members`,
/// FUN_140405190 and FUN_1403c32e0's objects): Sora's, and each friend's where
/// that character is in the room. The party that fights is fixed when the room
/// starts, so a member who is not here stays out.
pub fn enter_place(game: &mut Game, members: &crate::area::Members) {
    drive_room::wear_place(game, members.player);
    let Some(src) = game.party_reload.clone() else {
        return;
    };
    for &object in &members.friends {
        let Some(entry) = src
            .objects
            .iter()
            .find(|o| o.object_id == u32::from(object))
        else {
            continue;
        };
        let Ok(character) = u8::try_from(entry.character) else {
            continue;
        };
        let model = String::from_utf8_lossy(&entry.model_name).into_owned();
        let worn = game
            .party
            .iter()
            .any(|m| m.kind.member.character == character && m.kind.name == model);
        if !worn {
            party_room::wear(game, character, &model);
        }
    }
}

/// Wears `outfit` on its owner: Sora, or the party member of its section.
pub fn wear(game: &mut Game, outfit: &Outfit) {
    match outfit.group.member() {
        Some(character) => party_room::wear(game, character, &outfit.model),
        None => drive_room::set_outfit(game, Some(outfit.object_id)),
    }
}

/// Ticks each outfit gets in the unattended `--outfit-tour`: it loads, then
/// stands, then runs.
pub const TOUR_STEP: u64 = 240;
/// Where a step's two captures fall, and its run.
pub const TOUR_STAND: u64 = 190;
pub const TOUR_RUN: u64 = 230;
const TOUR_START: u64 = 60;
/// The crowns after the outfits, on base Sora.
const TOUR_CROWNS: usize = 4;

/// The stick of the outfit tour: a short run late in each step, back and
/// forth so Sora stays in the room.
pub fn tour_stick(tick: u64) -> Vec2 {
    let Some(t) = tick.checked_sub(TOUR_START) else {
        return Vec2::ZERO;
    };
    if t % TOUR_STEP < TOUR_STAND + 4 {
        return Vec2::ZERO;
    }
    let way = if (t / TOUR_STEP).is_multiple_of(2) {
        1.
    } else {
        -1.
    };
    Vec2::new(0., way)
}

/// A step of the unattended `--place-tour`: long enough for a warp's fades and
/// its kit, or a Drive's motion.
pub const PLACE_STEP: u64 = 480;
/// The worlds it visits: Halloween and Christmas Town, Space Paranoids,
/// Timeless River, Pride Lands, Atlantica and Twilight Town.
const PLACE_WORLDS: [u8; 6] = [14, 17, 13, 10, 11, 2];

/// One room of each different body the tour's worlds put Sora in.
fn tour_places(area: &AreaRes) -> Vec<(u8, u8)> {
    let mut seen = Vec::new();
    let mut places = Vec::new();
    for world in PLACE_WORLDS {
        for &room in area.source.rooms(world) {
            let player = area.source.members(world, room, &[]).player;
            if !seen.contains(&player) {
                seen.push(player);
                places.push((world, room));
            }
        }
    }
    places
}

/// The unattended `--place-tour`: warps to each place, drives into Valor where
/// the game has forms, reverts, and goes on; last, a warp in a form.
fn place_tour(game: &mut Game, area: &mut AreaRes, step: usize) {
    let places = tour_places(area);
    let (at, part) = (step / 3, step % 3);
    let tick = game.tick;
    let Some(&(world, room)) = places.get(at) else {
        // Last, a warp in a form: the form stays, on the new place's body.
        match step - places.len() * 3 {
            0 => {
                game.sora.player.drive.refill();
                drive_room::start_change(game, player::forms::VALOR);
            }
            1 => {
                if let Some(&place) = places.first() {
                    game.event(serde_json::json!({"event":"place_tour_warp","tick":tick,
                        "world":place.0,"room":place.1,"form":true}));
                    debug_menu::warp_to(area, (place.0, place.1, 0));
                }
            }
            2 => drive_room::start_change(game, 0),
            _ => {}
        }
        return;
    };
    let forms = !matches!(world, 10 | 11);
    match part {
        0 => {
            let object = area.source.members(world, room, &[]).player;
            game.event(serde_json::json!({"event":"place_tour_warp","tick":tick,
                "world":world,"room":room,"object":object}));
            if area.transition.is_none() && area.pending.is_none() {
                debug_menu::warp_to(area, (world, room, 0));
            }
        }
        1 if forms => {
            game.sora.player.drive.refill();
            drive_room::start_change(game, player::forms::VALOR);
        }
        2 if forms => drive_room::start_change(game, 0),
        _ => {}
    }
}

/// The unattended tours: each outfit in turn, then each crown on base Sora, then
/// the debug menu's page this returns; or, with `--place-tour`, the places.
pub fn tour(game: &mut Game, area: Option<&mut AreaRes>) -> Option<usize> {
    let t = game.tick.checked_sub(TOUR_START)?;
    let step = game.outfit_tour?;
    let length = if game.place_tour {
        PLACE_STEP
    } else {
        TOUR_STEP
    };
    // Frames can land on a tick twice or skip one.
    if t < step as u64 * length {
        return None;
    }
    game.outfit_tour = Some(step + 1);
    if game.place_tour {
        if let Some(area) = area {
            place_tour(game, area, step);
        }
        return None;
    }
    let outfits = game.drive.as_ref().map(|d| d.src.outfits.clone())?;
    let tick = game.tick;
    if let Some(outfit) = outfits.get(step) {
        game.event(
            serde_json::json!({"event":"outfit_tour_row","tick":tick,"step":step,
            "model":outfit.model,"label":outfit.label}),
        );
        wear(game, outfit);
    } else if let Some(level) = step.checked_sub(outfits.len()).filter(|&l| l < TOUR_CROWNS) {
        if level == 0 {
            drive_room::set_outfit(game, None);
        }
        game.cheats.crown = Some(level as u8);
        game.event(serde_json::json!({"event":"outfit_tour_crown","tick":tick,"level":level}));
    } else if step == outfits.len() + TOUR_CROWNS {
        // A real Drive: the gauge's face follows the form.
        game.sora.player.drive.refill();
        drive_room::start_change(game, player::forms::VALOR);
    } else {
        return step.checked_sub(outfits.len() + TOUR_CROWNS + 1);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(id: u32, model: &str, character: u16) -> ObjectEntry {
        ObjectEntry {
            object_id: id,
            object_type: 0,
            sub_type: 0,
            draw_priority: 0,
            weapon_joint: 0,
            model_name: model.as_bytes().to_vec(),
            animation_name: format!("{model}.mset").into_bytes(),
            neo_moveset: 0,
            flags: 0,
            character,
            form: 0,
            weight_bits: 0,
            spawn_limit: 0,
        }
    }

    #[test]
    fn the_catalog_keeps_installed_bodies_of_sora_and_roxas_by_section() {
        let objects = [
            object(1, "P_EX110", 14),
            object(2, "P_EX100_NM_BTLF", 1),
            object(3, "P_EX100", 1),
            object(4, "P_EX020_NM", 2),
            object(8, "M_EX020", 1001),
            object(5, "P_EX100_XM", 1),
            object(6, "M_EX950_CARD", 1),
            object(7, "P_EX100_UNKNOWN", 1),
        ];
        let list = catalog(&objects, |path| !path.contains("XM"));
        let rows: Vec<(&str, &str, Group)> = list
            .iter()
            .map(|o| (o.model.as_str(), o.label.as_str(), o.group))
            .collect();
        assert_eq!(
            rows,
            [
                ("P_EX100", "Sora", Group::Sora),
                (
                    "P_EX100_NM_BTLF",
                    "Valor Form, Halloween Town",
                    Group::Halloween
                ),
                ("P_EX100_UNKNOWN", "P_EX100_UNKNOWN", Group::Scene),
                ("P_EX110", "Roxas", Group::Roxas),
                ("P_EX020_NM", "Donald, Halloween Town", Group::Donald),
            ]
        );
    }

    #[test]
    fn a_body_with_no_set_of_its_own_uses_its_family() {
        let mut npc = object(1, "P_EX110_NPC_PAJAMAS", 0);
        npc.animation_name.clear();
        assert_eq!(
            motion_sets(&npc, "P_EX110_NPC_PAJAMAS"),
            ["P_EX110_NPC_PAJAMAS.mset", "P_EX110.mset"]
        );
    }

    #[test]
    fn the_crown_follows_the_number_of_proofs() {
        assert_eq!(proofs_held(|_| 0), 0);
        assert_eq!(proofs_held(|id| u8::from(id == PROOFS[1])), 1);
        assert_eq!(proofs_held(|id| u8::from(id != PROOFS[0]) * 2), 2);
        assert_eq!(proofs_held(|_| 1), 3);
    }
}
