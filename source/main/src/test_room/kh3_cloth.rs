//! KH3's secondary motion on one Sora: every frame the KBD component runs KineDriver and then Bonamik on the
//! component-space pose, after the anim graph (0x143ABFAD0, 0x143B5B990). The clip's `Bonamik_*` notify states drive
//! the solver as StrawHatPro's `kh-anim/pose.rs` reads them (MIT; natives 0x140562500 EnableGroundCollision,
//! 0x140562780 SpeedRate, 0x140562220 CharaWind, 0x140562650 ScaleGroupParams, 0x1405623A0 Enable, 0x140562840 Weight,
//! 0x140548A20 ResetAndPreRoll).

use data::kh3::anim::BonePose;
use data::kh3::skeletal::Bone as MeshBone;
use data::kh3::sora::{Clip, Notify, SoraData};
use kh3_rig::bonamik::notify;
use kh3_rig::glam::{Quat, Vec3};
use kh3_rig::json::Json;
use kh3_rig::kine::KineDriver;
use kh3_rig::{Bonamik, BonamikHost, BonamikOptions, Bone, Jitter, Skeleton, Trs};

fn trs(p: &BonePose) -> Trs {
    Trs {
        translation: Vec3::from_array(p.translation),
        rotation: Quat::from_array(p.rotation),
        scale: Vec3::from_array(p.scale),
    }
}

fn pose(t: &Trs) -> BonePose {
    BonePose {
        rotation: t.rotation.to_array(),
        translation: t.translation.to_array(),
        scale: t.scale.to_array(),
    }
}

/// A notify state that is running: its clip, its index, its properties.
struct Running {
    clip: usize,
    index: usize,
    notify: Notify,
    properties: Json,
}

pub(crate) struct Secondary {
    kine: Option<KineDriver>,
    cloth: Option<Bonamik>,
    /// The Kingdom Key's keychain: a second solver on the weapon's skeleton (`w_so010_Bonamik`).
    keychain: Option<Bonamik>,
    keychain_reference: Vec<Trs>,
    keychain_pose: Vec<Trs>,
    /// The character wind of Sora's solver reaches the attached weapon's (0x143b53410).
    wind_mirrored: bool,
    /// A notify's wind or rate on the keychain, from Sora's clip.
    armed: bool,
    world: Trs,
    pose: Vec<Trs>,
    local: Vec<Trs>,
    running: Vec<Running>,
    /// SpeedRate's saved rates.
    speed: Vec<((usize, usize), f32)>,
    /// The last clip and its time, for the instant notifies.
    last: (usize, f32),
}

fn properties(n: &Notify) -> Json {
    Json::parse(&data::kh3::rig::plain(&n.props).to_string()).unwrap_or(Json::Null)
}

fn groups(p: &Json) -> Vec<String> {
    p.get("m_Groups")
        .items()
        .iter()
        .filter_map(|g| g.as_str().map(str::to_owned))
        .collect()
}

fn skeleton(bones: &[MeshBone]) -> Skeleton {
    Skeleton {
        bones: bones
            .iter()
            .map(|b| Bone {
                name: b.name.clone(),
                parent: usize::try_from(b.parent).ok(),
                reference: Trs {
                    translation: Vec3::from_array(b.translation),
                    rotation: Quat::from_array(b.rotation),
                    scale: Vec3::from_array(b.scale),
                },
            })
            .collect(),
    }
}

fn solver(text: &str, skeleton: &Skeleton) -> Result<Bonamik, String> {
    Json::parse(text).and_then(|j| {
        Bonamik::new(
            &j,
            skeleton,
            BonamikOptions {
                // The thread-local stream Integrate draws from (0x143B123B0): its position cannot be known.
                jitter: Jitter::Seeded(kh3_rig::bonamik::DEFAULT_SEED),
                ..BonamikOptions::default()
            },
        )
    })
}

impl Secondary {
    pub(crate) fn new(data: &SoraData) -> Self {
        let kine = Json::parse(&data.rig.kine).and_then(|j| KineDriver::from_json(&j));
        let cloth = solver(&data.rig.bonamik, &skeleton(&data.mesh.bones));
        let blade = skeleton(&data.keyblade.bones);
        let keychain = solver(&data.rig.keychain, &blade);
        if let Err(e) = &keychain {
            eprintln!("KH3's keychain: {e}");
        }
        if let Err(e) = &kine {
            eprintln!("KH3's KineDriver: {e}");
        }
        if let Err(e) = &cloth {
            eprintln!("KH3's Bonamik: {e}");
        }
        Secondary {
            kine: kine.ok(),
            cloth: cloth.ok(),
            keychain: keychain.ok(),
            keychain_reference: kh3_rig::bonamik::reference_pose(&blade),
            keychain_pose: Vec::new(),
            wind_mirrored: false,
            armed: true,
            world: Trs::IDENTITY,
            pose: Vec::new(),
            local: Vec::new(),
            running: Vec::new(),
            speed: Vec::new(),
            last: (0, 0.),
        }
    }

    fn notify_state(&mut self, key: (usize, usize), n: &Notify, p: &Json, begin: bool) {
        let Some(solver) = &mut self.cloth else {
            return;
        };
        let names = groups(p);
        match n.class.as_str() {
            "TresAnimNotifyState_Bonamik_EnableGroundCollision" => {
                let enable = p.get("m_Enable").as_bool().unwrap_or(false);
                solver.set_ground_collision(&names, if begin { enable } else { !enable });
            }
            "TresAnimNotifyState_Bonamik_SpeedRate" => {
                // ctor 0x140559D00: m_ApplyToChildren true: the attached weapon's solver too.
                let children = p.get("m_ApplyToChildren").as_bool().unwrap_or(true);
                let chain = self.keychain.as_mut().filter(|_| children);
                if begin {
                    self.speed.retain(|(k, _)| *k != key);
                    self.speed.push((key, solver.speed_rate()));
                    let rate = p.get("m_BeginSpeedRate").as_f32().unwrap_or(1.);
                    let blend = p.get("m_BeginBlendTime").as_f32().unwrap_or(0.);
                    solver.set_speed_rate(rate, blend);
                    if let Some(chain) = chain {
                        chain.set_speed_rate(rate, blend);
                    }
                } else {
                    let saved = self.speed.iter().find(|(k, _)| *k == key).map(|(_, r)| *r);
                    let rate = if p
                        .get("m_RestoreOriginalSpeedRate")
                        .as_bool()
                        .unwrap_or(true)
                    {
                        saved.unwrap_or(1.)
                    } else {
                        p.get("m_EndSpeedRate").as_f32().unwrap_or(1.)
                    };
                    let blend = p.get("m_EndBlendTime").as_f32().unwrap_or(0.);
                    solver.set_speed_rate(rate, blend);
                    if let Some(chain) = chain {
                        chain.set_speed_rate(rate, blend);
                    }
                    self.speed.retain(|(k, _)| *k != key);
                }
            }
            "TresAnimNotifyState_Bonamik_ChangeGroupParams" => {
                if let Ok(rows) = notify::group_params_from_json(p.get("m_GroupParams")) {
                    if begin {
                        solver.set_group_param(&rows);
                    } else {
                        solver.restore_group_param(&rows);
                    }
                }
            }
            "TresAnimNotifyState_Bonamik_CharaWind" => {
                // begin / end call the KBD component with "children" set (0x140562220 -> 0x143b53410): the weapon's core
                // resolves the group names itself.
                if begin {
                    if let Ok(wind) = notify::chara_wind_from_json(p) {
                        solver.set_chara_wind(&wind);
                        if let Some(chain) = &mut self.keychain {
                            chain.set_chara_wind(&wind);
                        }
                    }
                } else {
                    solver.clear_chara_wind();
                    if let Some(chain) = &mut self.keychain {
                        chain.clear_chara_wind();
                    }
                }
            }
            "TresAnimNotifyState_Bonamik_ScaleGroupParams" => {
                // ctor 0x140559C90: every m_Change* true, every scale 1.
                let on = |flag: &str| p.get(flag).as_bool().unwrap_or(true);
                let value = |key: &str| {
                    if begin {
                        p.get(key).as_f32().unwrap_or(1.)
                    } else {
                        1.
                    }
                };
                solver.set_group_scales(
                    &names,
                    on("m_ChangeInnerCone").then(|| value("m_InnerConeScale")),
                    on("m_ChangeOuterCone").then(|| value("m_OuterConeScale")),
                    on("m_ChangeLocalForce").then(|| value("m_LocalForceScale")),
                    on("m_ChangeWindScale").then(|| value("m_GroupWindScale")),
                    on("m_ChangeMassScale").then(|| value("m_GroupMassScale")),
                );
            }
            "TresAnimNotifyState_Bonamik_Enable" => {
                let on = p.get("m_On").as_bool().unwrap_or(false);
                let blend = p
                    .get(if begin {
                        "m_BeginBlendTime"
                    } else {
                        "m_EndBlendTime"
                    })
                    .as_f32()
                    .unwrap_or(0.);
                solver.turn_groups(&names, if begin { on } else { !on }, blend);
            }
            "TresAnimNotifyState_Bonamik_Weight" => {
                // ctor 0x140559D40: begin weight 0, begin time 0.5, end weight 1, end time 0.
                let (weight, time) = if begin {
                    (
                        p.get("m_BeginBonamikWeight").as_f32().unwrap_or(0.),
                        p.get("m_BeginTime").as_f32().unwrap_or(0.5),
                    )
                } else {
                    (
                        p.get("m_EndBonamikWeight").as_f32().unwrap_or(1.),
                        p.get("m_EndTime").as_f32().unwrap_or(0.),
                    )
                };
                solver.set_group_weight(&names, weight, time);
            }
            _ => {}
        }
    }

    /// The stock order: instant notifies, ends of the states no longer listed, begins of the new ones
    /// (0x143F2DC50).
    fn notifies(&mut self, clip: Option<&Clip>, time: f32) {
        let id = clip.map_or(0, |c| std::ptr::from_ref(c) as usize);
        let from = if self.last.0 == id { self.last.1 } else { -1. };
        self.last = (id, time);
        let list = clip.map_or(&[][..], |c| c.notifies.as_slice());
        for n in list {
            if n.class == "TresAnimNotify_Bonamik_ResetAndPreRoll"
                && n.start > from
                && n.start <= time
                && let Some(solver) = &mut self.cloth
            {
                // ctor 0x14052BD40: m_Reset true, m_ResetPose false, m_PreRoll 0.
                let p = properties(n);
                if p.get("m_Reset").as_bool().unwrap_or(true) {
                    solver.request_reset(p.get("m_ResetPose").as_bool().unwrap_or(false));
                    solver.force_evaluation();
                }
                let roll = p.get("m_PreRoll").as_f32().unwrap_or(0.) as i32;
                if roll > 0 {
                    solver.set_pre_roll(roll);
                }
            }
        }
        let listed: Vec<usize> = list
            .iter()
            .enumerate()
            .filter(|(_, n)| {
                n.state
                    && n.class.starts_with("TresAnimNotifyState_Bonamik_")
                    && n.start <= time
                    && n.end > time
            })
            .map(|(i, _)| i)
            .collect();
        let mut ended = Vec::new();
        self.running.retain_mut(|r| {
            let keep = r.clip == id && listed.contains(&r.index);
            if !keep {
                ended.push(Running {
                    clip: r.clip,
                    index: r.index,
                    notify: r.notify.clone(),
                    properties: std::mem::replace(&mut r.properties, Json::Null),
                });
            }
            keep
        });
        for r in ended {
            self.notify_state((r.clip, r.index), &r.notify, &r.properties, false);
        }
        for i in listed {
            if self.running.iter().any(|r| r.clip == id && r.index == i) {
                continue;
            }
            let n = list[i].clone();
            let p = properties(&n);
            self.notify_state((id, i), &n, &p, true);
            self.running.push(Running {
                clip: id,
                index: i,
                notify: n,
                properties: p,
            });
        }
        // The wind's scale from its second tick: the clip's float curve `Wind` (0x140562220).
        if self
            .running
            .iter()
            .any(|r| r.notify.class == "TresAnimNotifyState_Bonamik_CharaWind")
            && from >= 0.
            && let Some(scale) = clip.and_then(|c| c.seq.curve("Wind", time))
            && let Some(solver) = &mut self.cloth
        {
            solver.set_chara_wind_scale(scale);
        }
    }

    /// One frame: `component` is rewritten. `at`, `yaw`: the actor in KH3's world, `floor`: its feet on the ground.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn step(
        &mut self,
        clip: Option<&Clip>,
        time: f32,
        dt: f32,
        local: &[BonePose],
        component: &mut [BonePose],
        at: Vec3,
        yaw: f32,
        floor: bool,
    ) {
        self.notifies(clip, time);
        self.pose.clear();
        self.pose.extend(component.iter().map(trs));
        self.local.clear();
        self.local.extend(local.iter().map(trs));
        if let Some(kine) = &mut self.kine {
            kine.evaluate(&mut self.pose, &self.local);
        }
        if let Some(cloth) = &mut self.cloth {
            self.world = Trs {
                translation: at,
                rotation: Quat::from_rotation_z(-yaw),
                scale: Vec3::ONE,
            };
            let host = BonamikHost {
                ground: floor.then_some((at, Vec3::Z)),
                ..BonamikHost::default()
            };
            cloth.frame(dt.max(1e-4), &mut self.pose, &self.world, &host);
        }
        for (out, t) in component.iter_mut().zip(&self.pose) {
            *out = pose(t);
        }
    }

    /// The keychain, after `step`: all its motion comes through the weapon's transform; the weapon's pose is its
    /// reference pose. `at`: the weapon's root in Sora's component space. Its pose goes to `out`.
    pub(crate) fn keychain(&mut self, dt: f32, at: &BonePose, armed: bool, out: &mut [BonePose]) {
        let Some(chain) = &mut self.keychain else {
            return;
        };
        // Whether the game resets when the weapon is shown was not determined: as after a teleport.
        if armed && !self.armed {
            chain.request_reset(false);
        }
        self.armed = armed;
        match self.cloth.as_ref().and_then(Bonamik::chara_wind_all_groups) {
            Some(wind) => {
                chain.set_chara_wind_all_groups(&wind);
                self.wind_mirrored = true;
            }
            None if self.wind_mirrored => {
                chain.clear_chara_wind_all_groups();
                self.wind_mirrored = false;
            }
            None => {}
        }
        self.keychain_pose.clear();
        self.keychain_pose
            .extend_from_slice(&self.keychain_reference);
        let world = trs(at).then(&self.world);
        chain.frame(
            dt.max(1e-4),
            &mut self.keychain_pose,
            &world,
            &BonamikHost::default(),
        );
        for (o, t) in out.iter_mut().zip(&self.keychain_pose) {
            *o = pose(t);
        }
    }
}
