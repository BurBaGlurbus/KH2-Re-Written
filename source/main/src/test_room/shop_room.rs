//! The shop's host side: FUN_1403aabe0's signal opens the mode (FUN_1401518e0,
//! FUN_14029b600) over the paused field, as the camp does, and the screen's
//! trades go back to the stock and the save when it closes.
use super::*;
use std::collections::BTreeMap;
use visual::pause_menu::shop::{self, Shop};

/// `save+0x49e0` goods seen, `save+0x49f0` the notices shown.
const SEEN: usize = 0x49e0;
const NOTICES: usize = 0x49f0;

/// The art, loaded at the first shop.
pub struct ShopArt {
    pub menu: menu_screen::Menu2ld,
    pub sheets: [Option<usize>; 2],
}

/// What a shop reads from 03system and sys.bar.
pub struct ShopSource {
    pub table: data::shop_table::Table,
    pub prices: BTreeMap<u16, u16>,
    pub texts: shop::Texts,
    /// Each row's name, by signal.
    pub names: HashMap<i16, Vec<u8>>,
}

impl ShopSource {
    pub const MESSAGES: [u32; 30] = [
        0x8acc, 0x8acd, 0xbbb5, 0x8ace, 0x8ad1, 0x8ad2, 0xbbb6, 0x8ad3, 0x8ad6, 0xc88f, 0x8ad4,
        0x8ad7, 0x8ad8, 0x8ad9, 0x8adb, 0x8ad0, 0x8acf, 0x9fcf, 0xc401, 0xc402, 0x8ad5, 0xc648,
        0xc649, 0xc64a, 0xc64b, 0xc651, 0xc652, 0xc653, 0xc654, 0xc64c,
    ];

    /// `m` in `MESSAGES`' order.
    pub fn texts(m: Vec<Vec<u8>>) -> shop::Texts {
        let mut m = m.into_iter();
        let mut next = || m.next().unwrap_or_default();
        shop::Texts {
            rows: [next(), next(), next(), next()],
            helps: [next(), next(), next(), next()],
            munny: next(),
            none: next(),
            stock: next(),
            how_many: [next(), next()],
            sure: [next(), next()],
            no: next(),
            yes: next(),
            ok: next(),
            notice: [next(), next()],
            total: next(),
            stats: std::array::from_fn(|_| next()),
        }
    }
}

/// `KH2RW_TOUR_SHOP=<tick>:<signal>[:<munny>]`: a shop opens then, in a tour.
fn tour_request(tick: u64) -> Option<(i16, Option<u32>)> {
    let v = std::env::var("KH2RW_TOUR_SHOP").ok()?;
    let mut parts = v.split(':');
    if parts.next()?.parse::<u64>().ok()? != tick {
        return None;
    }
    let signal = i16::from_str_radix(parts.next()?.trim_start_matches("0x"), 16).ok()?;
    Some((signal, parts.next().and_then(|m| m.parse().ok())))
}

fn load_art(
    c: &mut CampRes,
    frame: &mut Ui2dFrame,
    images: &mut Assets<Image>,
) -> Result<(), String> {
    if c.shop_art.is_some() {
        return Ok(());
    }
    let source = c.picture_source.clone().ok_or("no selected source")?;
    let library = data::path_lookup::Library::open(&source).map_err(|e| e.to_string())?;
    let found = library
        .find("menu/us/itemshop.2ld")
        .ok_or("itemshop.2ld is not in the installed packages")?;
    let mut menu = menu_screen::load(&source, found.index, found.ordinal, found.name_hash)
        .map_err(|e| e.to_string())?;
    let mut sheets = [None, None];
    for (k, slot) in sheets.iter_mut().enumerate() {
        let scale = menu.texture_for(k, k).map(|(_, s)| s);
        if let (Some(t), Some(scale)) = (menu.replacements.get_mut(k).and_then(Option::take), scale)
        {
            *slot = Some(frame.add_sheet(images, t.rgba, t.size, scale));
        }
    }
    c.shop_art = Some(ShopArt { menu, sheets });
    Ok(())
}

/// The party list (FUN_1402fc300): Sora, then the members, each with the name
/// of its face (`menu/shopface/<model>.bin`, by FUN_1403e3180's object).
fn members(c: &CampRes, game: &Game) -> Vec<(shop::Member, String)> {
    let mut out = vec![(
        shop::Member {
            equipment: c.equipment.clone(),
            sora: true,
        },
        "p_ex100".to_owned(),
    )];
    for m in &game.party {
        let s = &m.save;
        let list = |v: &[u16]| v.iter().copied().filter(|&i| i != 0).collect::<Vec<_>>();
        let equipment = pause_menu::items::Equipment {
            character: m.kind.character,
            weapon: s.weapon,
            armor: list(&s.armor),
            accessories: list(&s.accessories),
            items: list(&s.items),
            ..Default::default()
        };
        out.push((
            shop::Member {
                equipment,
                sora: false,
            },
            m.kind.name.to_lowercase(),
        ));
    }
    out
}

/// FUN_1401518e0 and FUN_1402a0440: the signal raised, the screen opens.
pub fn open(
    camp: Option<ResMut<CampRes>>,
    mut game: ResMut<Game>,
    area: Option<ResMut<AreaRes>>,
    mut audio: ResMut<Audio>,
    mut frame: ResMut<Ui2dFrame>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut c) = camp else { return };
    let c = &mut *c;
    if let Some((signal, munny)) = tour_request(game.tick) {
        game.shop_wanted = Some(signal);
        if let (Some(m), Some(rt)) = (munny, game.enemy.as_mut()) {
            rt.munny = m;
        }
    }
    let Some(signal) = game.shop_wanted else {
        return;
    };
    // FUN_1401515c0: refused while an overlay is up.
    if c.menu_up() {
        return;
    }
    game.shop_wanted = None;
    if let Err(e) = load_art(c, &mut frame, &mut images) {
        game.event(serde_json::json!({"event":"shop_refused","signal":signal,"error":e}));
        return;
    }
    let Some(src) = c.source.shop.as_ref() else {
        return;
    };
    let table = &c.source.items.table;
    let kind = |id: u16| table.iter().find(|r| r.id == id).map(|r| r.kind);
    let Some(row) = src.table.row(signal) else {
        return;
    };
    // FUN_1402fba40's first step: the visited flag.
    let mut area = area;
    let goods = match area.as_deref_mut() {
        Some(a) => {
            let p = a.source.progress_mut();
            if row.visited >= 0 && !p.test(row.visited as u32) {
                p.set(row.visited as u32);
            }
            src.table.goods(signal, |f| p.test(f as u32), kind)
        }
        None => src.table.goods(signal, |_| false, kind),
    };
    let Some(goods) = goods else { return };
    let party = members(c, &game);
    let mut seen = [0u8; 0x10];
    for (k, b) in seen.iter_mut().enumerate() {
        *b = game.save.base.u8_at(SEEN + k);
    }
    let stock = game.items.as_ref().map_or(&c.stock, |i| &i.stock).clone();
    let data = shop::Data {
        name: src.names.get(&signal).cloned().unwrap_or_default(),
        texts: src.texts.clone(),
        table: table.clone(),
        stats: c.source.equip_stats.clone(),
        prices: src.prices.clone(),
        goods: goods.items.clone(),
        buy_sell: goods.buy_sell,
        synthesize: goods.synthesize,
        own_goods: goods.own_goods,
        stock,
        munny: game.enemy.as_ref().map_or(0, |rt| rt.munny),
        seen,
        watch: src.table.watch.clone(),
        notices: game.save.base.u8_at(NOTICES),
        members: party.iter().map(|m| m.0.clone()).collect(),
        picture_sequence: c.source.picture_sequence.clone(),
        model: (row.model, [row.x, row.y], row.height),
    };
    let Some(art) = c.shop_art.as_ref() else {
        return;
    };
    let (screen, sounds) = Shop::open(&art.menu.layout, data);
    c.shop = Some(screen);
    // FUN_14029baf0: each member's face.
    c.shop_faces.clear();
    if let Some(source) = c.picture_source.clone() {
        for (_, name) in &party {
            let path = format!("menu/shopface/{name}.bin");
            let sheet = match menu_screen::load_imgd(&source, PICTURE_INDEX, &path) {
                Ok(Some(t)) => Some(frame.add_sheet(&mut images, t.rgba, t.size, t.texel_scale)),
                _ => None,
            };
            c.shop_faces.push(sheet);
        }
    }
    let tick = game.tick;
    game.event(
        serde_json::json!({"event":"shop_open","tick":tick,"signal":signal,
        "goods":goods.items,"buy_sell":goods.buy_sell,"synthesize":goods.synthesize}),
    );
    play(&mut audio, &mut game, &sounds);
}

fn play(audio: &mut Audio, game: &mut Game, sounds: &[u32]) {
    let tick = game.tick;
    for &n in sounds {
        game.event(serde_json::json!({"event":"sound","tick":tick,"id":n,"cause":"shop"}));
        let Audio { output, banks, .. } = audio;
        if let (Some(output), Ok(Some(r))) = (output.as_ref(), banks.request(n))
            && let Ok(mut mixer) = output.mixer.lock()
        {
            for layer in r.layers {
                mixer.play(layer, n, r.volume, 1.);
            }
        }
    }
}

/// The screen's frame while it's up (FUN_14029dd40, FUN_14029bfe0). Returns
/// true while it holds the pad.
pub fn tick(
    c: &mut CampRes,
    game: &mut Game,
    audio: &mut Audio,
    frame: &mut Ui2dFrame,
    images: &mut Assets<Image>,
) -> bool {
    let (Some(screen), Some(art)) = (c.shop.as_mut(), c.shop_art.as_ref()) else {
        return false;
    };
    screen.pictures.loaded = c
        .picture_sheets
        .iter()
        .filter(|(_, s)| s.is_some())
        .map(|(&id, _)| id)
        .collect();
    let sounds = screen.frame(&art.menu.layout, c.pad.pressed, c.pad.repeat);
    game.tick += 1;
    play(audio, game, &sounds);
    let wanted = screen.pictures.wanted.take();
    if let (Some(id), Some(source)) = (wanted, c.picture_source.clone())
        && !c.picture_sheets.contains_key(&id)
    {
        let path = format!("itempic/item-{id:03}.imd");
        let sheet = match menu_screen::load_imgd(&source, PICTURE_INDEX, &path) {
            Ok(Some(t)) => Some(frame.add_sheet(images, t.rgba, t.size, t.texel_scale)),
            _ => None,
        };
        c.picture_sheets.insert(id, sheet);
    }
    if screen.done() {
        let Some(screen) = c.shop.take() else {
            return false;
        };
        close(c, game, screen.data);
    }
    true
}

/// The stock, munny and marks go back (FUN_1403c3fe0, FUN_1403c4a40 wrote them
/// as they happened).
fn close(c: &mut CampRes, game: &mut Game, data: shop::Data) {
    match game.items.as_mut() {
        Some(i) => i.stock = data.stock,
        None => c.stock = data.stock,
    }
    if let Some(rt) = game.enemy.as_mut() {
        rt.munny = data.munny;
    }
    for (k, &b) in data.seen.iter().enumerate() {
        game.save.base.set_u8(SEEN + k, b);
    }
    game.save.base.set_u8(NOTICES, data.notices);
    let (tick, munny) = (game.tick, data.munny);
    game.event(serde_json::json!({"event":"shop_close","tick":tick,"munny":munny}));
}

/// A SHOP_POINT's model at its placement, looping its motion 0. Its class
/// (FUN_14041a490) isn't traced past the constructor: [I] it stands and plays.
const MOOGLE: u16 = 0x487;

/// A placement and its model's load.
type PointLoad = (
    Vec3,
    f32,
    std::thread::JoinHandle<Result<journal_model::Loaded, String>>,
);

#[derive(Default, Resource)]
pub struct ShopPoints {
    key: Option<(u8, u8, usize)>,
    loading: Vec<PointLoad>,
    shown: Vec<ShownPoint>,
}

struct ShownPoint {
    root: Entity,
    loaded: journal_model::Loaded,
    indices: Vec<Vec<u32>>,
    time: f32,
    tick: u64,
}

#[derive(Component)]
pub struct ShopPointPart {
    point: usize,
    ordinal: usize,
}

#[allow(clippy::too_many_arguments)]
pub fn shop_points(
    mut commands: Commands,
    mut points: ResMut<ShopPoints>,
    game: Res<Game>,
    camp: Option<Res<CampRes>>,
    area: Option<Res<AreaRes>>,
    world: Option<Res<SceneRoot>>,
    parts: Query<(&ShopPointPart, &Mesh3d)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<FogMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(area), Some(camp), Some(world)) = (area, camp, world) else {
        return;
    };
    let a = &area.current;
    let key = (a.world, a.area, a.shop_points.len());
    if points.key != Some(key) {
        points.key = Some(key);
        for p in std::mem::take(&mut points.shown) {
            commands.entity(p.root).despawn();
        }
        points.loading.clear();
        if let (Some(source), Some(j)) = (&camp.picture_source, &camp.journal_source) {
            for &(_, at, yaw) in &a.shop_points {
                let (source, objects) = (source.clone(), j.objects.clone());
                // The SHOP_POINT's sub-object (0x487, objentry field +0x46) is
                // the Moogle.
                let thread = std::thread::spawn(move || {
                    journal_model::load(&source, &objects, MOOGLE, 0).map_err(|e| e.to_string())
                });
                points.loading.push((at, yaw, thread));
            }
        }
    }
    let mut waiting = Vec::new();
    for (at, yaw, thread) in std::mem::take(&mut points.loading) {
        if !thread.is_finished() {
            waiting.push((at, yaw, thread));
            continue;
        }
        let loaded = match thread.join() {
            Ok(Ok(l)) => l,
            Ok(Err(e)) => {
                eprintln!("Shop point rejected: {e}");
                continue;
            }
            Err(_) => continue,
        };
        let Some(pose) = journal_model::positions(&loaded, 0.) else {
            continue;
        };
        let root = commands
            .spawn((
                RoomPart,
                ChildOf(world.0),
                Transform::from_translation(at).with_rotation(Quat::from_rotation_y(yaw)),
                Visibility::default(),
            ))
            .id();
        let point = points.shown.len();
        let pose: Vec<Vec3> = pose.into_iter().collect();
        for (ordinal, entity) in spawn_model(
            &mut commands,
            &loaded.model,
            &pose,
            root,
            &mut meshes,
            &mut materials,
            &mut images,
        ) {
            if let Some(entity) = entity {
                commands
                    .entity(entity)
                    .insert(ShopPointPart { point, ordinal });
            }
        }
        let indices = loaded
            .model
            .parts
            .iter()
            .map(|p| p.indices.clone())
            .collect();
        points.shown.push(ShownPoint {
            root,
            loaded,
            indices,
            time: 0.,
            tick: game.tick,
        });
    }
    points.loading = waiting;
    for p in &mut points.shown {
        for _ in p.tick..game.tick {
            p.time = journal_model::clock(p.time, p.loaded.total, p.loaded.back);
        }
        p.tick = game.tick;
    }
    let poses: Vec<_> = points
        .shown
        .iter()
        .map(|p| journal_model::positions(&p.loaded, p.time))
        .collect();
    for (part, handle) in &parts {
        let (Some(p), Some(Some(pose))) = (points.shown.get(part.point), poses.get(part.point))
        else {
            continue;
        };
        if let Some(indices) = p.indices.get(part.ordinal) {
            visual::room::set_positions(
                &mut meshes,
                &handle.0,
                indices.iter().map(|&i| pose[i as usize].to_array()),
            );
        }
    }
}

pub fn present(c: &CampRes, frame: &mut Ui2dFrame, font: &field_command_menu::Font) {
    let (Some(screen), Some(art)) = (c.shop.as_ref(), c.shop_art.as_ref()) else {
        return;
    };
    for (sheet, quads) in screen.draw(&art.menu.layout, font) {
        let slot = match sheet {
            shop::Sheet::Image(k) => art.sheets.get(k).copied().flatten(),
            shop::Sheet::Font => Some(c.font_sheet),
            shop::Sheet::Icon => Some(c.icon_sheet),
            shop::Sheet::Model => c.journal_sheets.model,
            shop::Sheet::Picture(id) => c.picture_sheets.get(&id).copied().flatten(),
            shop::Sheet::Face(i) => c.shop_faces.get(i).copied().flatten(),
        };
        if let Some(slot) = slot {
            frame.draws.push((slot, quads));
        }
    }
}
