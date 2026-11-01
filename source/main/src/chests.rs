//! Field treasure chests (objentry type 10, YS::TREASURE_BOX: init
//! FUN_140402200, open FUN_1404025c0). A chest has no per-tick code of its own
//! and no script: it stands on a one-pose motion and the reaction search
//! offers its command 32 until it is opened.
use crate::npcs::{Anim, build_anim};
use bevy::prelude::*;
use data::model_inspection::InspectionModel;
use data::objentry::ObjectEntry;
use data::path_lookup::Library;
use data::probe_source::ProbeSource;
use data::{journal_table::Treasure, motion, object_collision};
use std::sync::Arc;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// The chest's motions (an RTN set: the id is the slot): closed, opened, opening.
const SLOTS: [usize; 3] = [151, 152, 153];

/// One object's model, poses and shapes.
pub struct ChestKind {
    pub model: InspectionModel,
    pub closed: Vec<Vec3>,
    pub opened: Vec<Vec3>,
    pub opening: Anim,
    /// The opening motion's frame triggers: sound 8 and effect 1.
    pub sounds: Vec<(u32, u32)>,
    pub effects: Vec<(u32, u16)>,
    /// The REACTION entry (command 32). Its centre is carried in `position`.
    pub reaction: object_collision::Entry,
    /// The first kind-1 shape: radius, height, centre height.
    pub body: [f32; 3],
    /// The object's own effects (`obj/F_EX030.a.us`).
    pub pax: Option<Arc<data::object_effect::Pax>>,
    /// The `.a.us` holds an event `00`: a big chest (FUN_140402460 +0xbf8).
    pub big: bool,
    /// That event's script (`re_f_ex040-00`).
    pub event: Option<Vec<u8>>,
}

impl ChestKind {
    pub fn load(source: &ProbeSource, library: &mut Library, object: &ObjectEntry) -> Result<Self> {
        let model_name = String::from_utf8_lossy(&object.model_name).into_owned();
        let found = library
            .find(&format!("obj/{model_name}.mdlx"))
            .ok_or_else(|| format!("obj/{model_name}.mdlx is not installed"))?;
        let mut models = data::model_inspection::read_model_at(
            source,
            found.index,
            found.ordinal,
            found.name_hash,
        )?;
        if models.is_empty() {
            return Err("chest model has no mesh".into());
        }
        let mut model = models.remove(0);
        model.merge_parts_by_texture();
        let set_name = String::from_utf8_lossy(&object.animation_name).into_owned();
        let set = library
            .find(&format!(
                "obj/{}",
                if set_name.ends_with(".mset") {
                    set_name.clone()
                } else {
                    format!("{set_name}.mset")
                }
            ))
            .ok_or_else(|| format!("{set_name} motion set is not installed"))?;
        let mut read = motion::read_slots_each(
            source,
            set.index,
            set.ordinal,
            set.name_hash,
            SLOTS.as_slice(),
        )?
        .into_iter();
        let mut next = || -> Result<Arc<motion::SelectedMotion>> {
            read.next()
                .flatten()
                .map(Arc::new)
                .ok_or_else(|| "chest set lacks a motion".into())
        };
        let (closed, opened, opening) = (next()?, next()?, next()?);
        let pose = |m: &Arc<motion::SelectedMotion>| -> Result<Vec<Vec3>> {
            Ok(build_anim(&model, m, None)?
                .frames
                .into_iter()
                .next()
                .unwrap_or_default())
        };
        let closed_pose = pose(&closed)?;
        let opened_pose = pose(&opened)?;
        let sounds = crate::test_room::sound_triggers(&opening, -1);
        let effects = opening
            .triggers
            .iter()
            .flat_map(|t| &t.records.frames)
            .filter(|f| f.trigger == 1)
            .filter_map(|f| {
                Some((
                    f.frame.max(0) as u32,
                    u16::try_from(*f.parameters.first()?).ok()?,
                ))
            })
            .collect();
        let opening = build_anim(&model, &opening, None)?;
        let archive = library.read_found(&found)?;
        let table = data::bar::read_bar(&archive)?;
        let shapes = table
            .entries
            .iter()
            .find(|e| e.entry_type == 23 && e.size > 0)
            .map(|e| {
                object_collision::read(
                    &archive[e.offset as usize..e.offset as usize + e.size as usize],
                )
            })
            .transpose()?
            .unwrap_or_default();
        let reaction = shapes
            .iter()
            .find(|e| e.kind == object_collision::REACTION)
            .copied()
            .ok_or("chest has no REACTION shape")?;
        let body = shapes
            .iter()
            .find(|e| e.kind == 1)
            .map_or([50., 30., 30.], |e| {
                [e.radius as f32, e.height as f32, e.position_height as f32]
            });
        let effect_file = format!("obj/{model_name}.a.us");
        let pax = data::object_effect::read_file_pax(library, &effect_file).map(Arc::new);
        let event = library.read(&effect_file).ok().and_then(|b| {
            let table = data::bar::read_bar(&b).ok()?;
            let e = table.entries.iter().find(|e| e.entry_type == 22)?;
            Some(
                b.get(e.offset as usize..e.offset as usize + e.size as usize)?
                    .to_vec(),
            )
        });
        let big = event.is_some();
        Ok(ChestKind {
            model,
            closed: closed_pose,
            opened: opened_pose,
            opening,
            sounds,
            effects,
            reaction,
            body,
            pax,
            big,
            event,
        })
    }
}

/// What the chest is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChestState {
    /// Not read yet: the save decides when the area starts.
    Unread,
    Closed,
    /// The opening motion, at this tick.
    Opening(u32),
    Opened,
}

pub struct Chest {
    pub kind: Arc<ChestKind>,
    pub object: u32,
    /// Scene position and yaw (facing (sin yaw, cos yaw)).
    pub position: Vec3,
    pub yaw: f32,
    /// FUN_1403a19c0: its `trsr` row: the item and the opened bit (0: none).
    pub item: Option<u16>,
    pub bit: u16,
    pub state: ChestState,
}

impl Chest {
    /// The vertex positions to show.
    pub fn pose(&self) -> &[Vec3] {
        match self.state {
            ChestState::Unread | ChestState::Closed => &self.kind.closed,
            ChestState::Opened => &self.kind.opened,
            ChestState::Opening(t) => {
                let frames = &self.kind.opening.frames;
                frames
                    .get(t as usize)
                    .or(frames.last())
                    .map_or(&self.kind.closed, |f| f)
            }
        }
    }
}

/// FUN_1403a19c0: the first `trsr` row of type 0 for this place and sub index.
pub fn row(treasures: &[Treasure], world: u8, area: u8, sub: u8) -> Option<&Treasure> {
    treasures
        .iter()
        .find(|t| t.kind == 0 && t.game_world == world && t.room == area && t.sub == sub)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chest_finds_its_row_by_place_and_sub_index() {
        let t = |kind, world, room, sub, item| Treasure {
            item,
            kind,
            game_world: world,
            room,
            sub,
            flag: 0,
            opened: 1,
        };
        let rows = [t(1, 2, 9, 0, 1), t(0, 2, 9, 0, 2), t(0, 2, 9, 1, 3)];
        assert_eq!(row(&rows, 2, 9, 0).map(|r| r.item), Some(2));
        assert_eq!(row(&rows, 2, 9, 1).map(|r| r.item), Some(3));
        assert!(row(&rows, 2, 8, 0).is_none());
    }
}
