//! Drawing between simulation ticks. The game advances on the fixed 60 Hz
//! tick and the presenters place everything from the newest tick, so above
//! 60 frames per second the extra frames would repeat the same picture.
//! With smoothing on, the transforms of the scene camera and of everything
//! under the scene's root are shown between the last two ticks, by the
//! fixed clock's overstep: the picture runs at most one tick behind the
//! simulation and moves on every frame. Nothing here feeds back into the
//! simulation: presenters keep writing the newest tick's transforms and
//! the blend is kept apart from them.
//!
//! Not blended: skinned vertices (poses still change once per tick) and
//! effect meshes built in scene space (they show the newest tick).
use bevy::prelude::*;
use bevy::transform::TransformSystems;

/// Whether to blend (the Graphics page's Motion Smoothing).
#[derive(Resource)]
pub struct Smoothing {
    pub on: bool,
}

/// Fixed-tick runs, for presentation that must step once per tick rather
/// than once per frame.
#[derive(Resource, Default)]
pub struct TickClock {
    runs: u64,
    frame_start: u64,
}

impl TickClock {
    /// Fixed ticks run this frame (from `Update` on; the ticks run between
    /// `PreUpdate` and `Update`).
    pub fn ticks_this_frame(&self) -> u64 {
        self.runs - self.frame_start
    }
}

/// Smooth this entity and everything under it.
#[derive(Component)]
pub struct SmoothRoot;

/// Smooth this entity only (a camera outside the scene's root).
#[derive(Component)]
pub struct Smoothed;

/// The newest tick's transform, written by the presenters, and the one
/// before it.
#[derive(Component)]
struct TickPose {
    prev: Transform,
    curr: Transform,
}

/// A step longer than this between two ticks is a cut (a warp, an area
/// change, a camera switch), shown at once instead of blended.
const CUT_DISTANCE: f32 = 250.;
const CUT_ANGLE: f32 = 1.0;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SmoothingSet;

pub struct SmoothingPlugin;

impl Plugin for SmoothingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Smoothing { on: true })
            .init_resource::<TickClock>()
            .add_systems(FixedFirst, |mut clock: ResMut<TickClock>| clock.runs += 1)
            .add_systems(First, |mut clock: ResMut<TickClock>| {
                clock.frame_start = clock.runs;
            })
            .add_systems(
                PostUpdate,
                (track, blend)
                    .chain()
                    .in_set(SmoothingSet)
                    .before(TransformSystems::Propagate),
            );
    }
}

/// Starts tracking new entities: those marked, and children of tracked
/// entities or of a root (one level per frame).
#[allow(clippy::type_complexity)]
fn track(
    mut commands: Commands,
    marked: Query<(Entity, &Transform), (With<Smoothed>, Without<TickPose>)>,
    children: Query<(Entity, &Transform, &ChildOf), Without<TickPose>>,
    parents: Query<(), Or<(With<TickPose>, With<SmoothRoot>)>>,
) {
    let start = |t: &Transform| TickPose { prev: *t, curr: *t };
    for (e, t) in &marked {
        commands.entity(e).insert(start(t));
    }
    for (e, t, child_of) in &children {
        if parents.contains(child_of.parent()) {
            commands.entity(e).insert(start(t));
        }
    }
}

fn is_cut(a: &Transform, b: &Transform) -> bool {
    a.translation.distance(b.translation) > CUT_DISTANCE
        || a.rotation.angle_between(b.rotation) > CUT_ANGLE
        || a.scale != b.scale && (a.scale - b.scale).abs().max_element() > 0.5
}

/// One entity's step: `written` is the presenters' transform this frame (or
/// None when they left the shown one in place). Returns what to show.
fn step(pose: &mut TickPose, written: Option<Transform>, ticks: u64, alpha: f32) -> Transform {
    let newest = written.unwrap_or(pose.curr);
    match ticks {
        0 => pose.curr = newest,
        // One tick on: blend from the last tick's transform.
        1 => {
            pose.prev = pose.curr;
            pose.curr = newest;
        }
        // Several ticks since the last frame (a slow frame): nothing to
        // blend from.
        _ => {
            pose.prev = newest;
            pose.curr = newest;
        }
    }
    if alpha >= 1. || pose.prev == pose.curr || is_cut(&pose.prev, &pose.curr) {
        return pose.curr;
    }
    Transform {
        translation: pose.prev.translation.lerp(pose.curr.translation, alpha),
        rotation: pose.prev.rotation.slerp(pose.curr.rotation, alpha),
        scale: pose.prev.scale.lerp(pose.curr.scale, alpha),
    }
}

fn blend(
    smoothing: Res<Smoothing>,
    clock: Res<TickClock>,
    fixed: Res<Time<Fixed>>,
    virt: Res<Time<Virtual>>,
    mut poses: Query<(Mut<Transform>, &mut TickPose)>,
) {
    // A paused clock (a capture holds the tick) shows the tick itself.
    let alpha = if smoothing.on && !virt.is_paused() {
        fixed.overstep_fraction()
    } else {
        1.
    };
    let ticks = clock.ticks_this_frame();
    poses.par_iter_mut().for_each(|(mut transform, mut pose)| {
        // The blend's own writes are not changes: only a presenter's are.
        let written = transform.is_changed().then_some(*transform);
        let shown = step(&mut pose, written, ticks, alpha);
        if *transform != shown {
            *transform = shown;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32) -> Transform {
        Transform::from_xyz(x, 0., 0.)
    }

    #[test]
    fn blends_between_the_last_two_ticks() {
        let mut p = TickPose {
            prev: at(0.),
            curr: at(0.),
        };
        // Tick 1 places it at 10; a quarter of a tick later it shows 2.5.
        assert_eq!(step(&mut p, Some(at(10.)), 1, 0.25), at(2.5));
        // A frame with no new tick: the presenter writes 10 again.
        assert_eq!(step(&mut p, Some(at(10.)), 0, 0.75), at(7.5));
        // The presenter did not write: the shown blend stays apart.
        assert_eq!(step(&mut p, None, 0, 0.9).translation.x, 9.);
        // Tick 2 at 20.
        assert_eq!(step(&mut p, Some(at(20.)), 1, 0.5), at(15.));
    }

    #[test]
    fn full_alpha_slow_frames_and_cuts_show_the_newest_tick() {
        let mut p = TickPose {
            prev: at(0.),
            curr: at(0.),
        };
        assert_eq!(step(&mut p, Some(at(10.)), 1, 1.), at(10.));
        assert_eq!(step(&mut p, Some(at(30.)), 2, 0.5), at(30.));
        assert_eq!(step(&mut p, Some(at(5000.)), 1, 0.5), at(5000.));
        let turned = Transform::from_rotation(Quat::from_rotation_y(3.));
        assert_eq!(step(&mut p, Some(turned), 1, 0.5), turned);
    }

    #[test]
    fn presenter_writes_reach_the_blend_and_its_own_do_not() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, SmoothingPlugin));
        let root = app
            .world_mut()
            .spawn((SmoothRoot, Transform::default()))
            .id();
        let e = app.world_mut().spawn((at(0.), ChildOf(root))).id();
        for _ in 0..3 {
            app.update();
        }
        assert!(
            app.world().get::<TickPose>(e).is_some(),
            "children are tracked"
        );
        // A paused clock shows the newest written transform as it is.
        app.world_mut().resource_mut::<Time<Virtual>>().pause();
        app.world_mut()
            .get_mut::<Transform>(e)
            .unwrap()
            .translation
            .x = 4.;
        app.update();
        assert_eq!(app.world().get::<Transform>(e).unwrap().translation.x, 4.);
        assert_eq!(app.world().get::<TickPose>(e).unwrap().curr, at(4.));
        // Frames without a presenter's write keep it.
        app.update();
        app.update();
        assert_eq!(app.world().get::<TickPose>(e).unwrap().curr, at(4.));
    }
}
