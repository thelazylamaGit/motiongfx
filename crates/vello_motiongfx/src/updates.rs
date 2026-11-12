//! Direct updates: timeline -> bindings -> callbacks in registration order.
//! Typed targets are borrowed safely, without RefCell, unsafe, or per-sample
//! allocations. Target selection is sorted/checked once during registration.

use crate::*;
use motiongfx_authoring::{BindingPlan, BindingSource};

type RunUpdate =
    dyn FnMut(&mut Graphics, &BindingPlan<Graphics>) + Send + Sync;
#[doc(hidden)]
pub struct NativeUpdate(Box<RunUpdate>);
impl NativeUpdate {
    pub(crate) fn run(
        &mut self,
        world: &mut Graphics,
        bindings: &BindingPlan<Graphics>,
    ) {
        (self.0)(world, bindings);
    }
}

/// Targets passed directly to an update: a handle, a homogeneous handle
/// array, or a tuple of two to four handles. Callback values are mutable
/// references to existing objects, in the same order as the targets.
pub trait UpdateTargets: Sized {
    type Objects<'a>;
    #[doc(hidden)]
    fn callback<S, F>(
        self,
        source: S,
        update: F,
        world: &Graphics,
    ) -> NativeUpdate
    where
        S: BindingSource<Graphics>,
        F: for<'a> FnMut(Self::Objects<'a>, S::Value) + ThreadSafe;
}

#[doc(hidden)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectSpec {
    kind: u8,
    index: usize,
    slot: usize,
}

// Each type has separate storage. The array slots retain the caller's order;
// sorted specs let split_at_mut obtain disjoint references within each Vec.
struct Layout<const N: usize>([ObjectSpec; N]);
impl<const N: usize> Layout<N> {
    fn new(mut specs: [ObjectSpec; N], world: &Graphics) -> Self {
        specs.sort_unstable();
        for (i, spec) in specs.iter().enumerate() {
            let lengths = [
                world.rectangles.len(),
                world.circles.len(),
                world.lines.len(),
                world.paths.len(),
                world.texts.len(),
                1,
            ];
            assert!(
                spec.index < lengths[spec.kind as usize],
                "update target does not exist"
            );
            assert!(
                i == 0
                    || (spec.kind, spec.index)
                        != (specs[i - 1].kind, specs[i - 1].index),
                "an update cannot borrow the same object twice"
            );
        }
        Self(specs)
    }
    fn borrow<'a>(
        &self,
        world: &'a mut Graphics,
    ) -> ObjectBatch<'a, N> {
        ObjectBatch {
            rectangles: select(&self.0, 0, &mut world.rectangles),
            circles: select(&self.0, 1, &mut world.circles),
            lines: select(&self.0, 2, &mut world.lines),
            paths: select(&self.0, 3, &mut world.paths),
            texts: select(&self.0, 4, &mut world.texts),
            cameras: select(
                &self.0,
                5,
                std::slice::from_mut(&mut world.camera),
            ),
        }
    }
}
fn select<'a, T, const N: usize>(
    specs: &[ObjectSpec; N],
    kind: u8,
    mut remaining: &'a mut [T],
) -> [Option<&'a mut T>; N] {
    let mut selected = std::array::from_fn(|_| None);
    let mut consumed = 0;
    for spec in specs.iter().filter(|s| s.kind == kind) {
        let (_, tail) = remaining.split_at_mut(spec.index - consumed);
        let (object, tail) =
            tail.split_first_mut().expect("validated update target");
        selected[spec.slot] = Some(object);
        remaining = tail;
        consumed = spec.index + 1;
    }
    selected
}
#[doc(hidden)]
pub struct ObjectBatch<'a, const N: usize> {
    rectangles: [Option<&'a mut Rectangle>; N],
    circles: [Option<&'a mut Circle>; N],
    lines: [Option<&'a mut Line>; N],
    paths: [Option<&'a mut Path>; N],
    texts: [Option<&'a mut Text>; N],
    cameras: [Option<&'a mut Camera>; N],
}
#[doc(hidden)]
pub trait UpdateHandle: Copy + ThreadSafe {
    type Object;
    fn spec(self, slot: usize) -> ObjectSpec;
    fn take<'a, const N: usize>(
        batch: &mut ObjectBatch<'a, N>,
        slot: usize,
    ) -> &'a mut Self::Object;
}

macro_rules! update_handle {
    ($handle:ty, $object:ty, $field:ident, $kind:literal, $index:expr) => {
        impl UpdateHandle for $handle {
            type Object = $object;
            fn spec(self, slot: usize) -> ObjectSpec {
                ObjectSpec {
                    kind: $kind,
                    index: ($index)(self),
                    slot,
                }
            }
            fn take<'a, const N: usize>(
                batch: &mut ObjectBatch<'a, N>,
                slot: usize,
            ) -> &'a mut Self::Object {
                batch.$field[slot]
                    .take()
                    .expect("typed update target")
            }
        }
        impl UpdateTargets for $handle {
            type Objects<'a> = &'a mut $object;
            fn callback<S, F>(
                self,
                source: S,
                mut update: F,
                world: &Graphics,
            ) -> NativeUpdate
            where
                S: BindingSource<Graphics>,
                F: for<'a> FnMut(Self::Objects<'a>, S::Value)
                    + ThreadSafe,
            {
                let layout = Layout::new([self.spec(0)], world);
                NativeUpdate(Box::new(move |world, bindings| {
                    let input = bindings
                        .read_source(world, &source)
                        .expect("validated update source");
                    let mut batch = layout.borrow(world);
                    update(Self::take(&mut batch, 0), input);
                }))
            }
        }
    };
}
update_handle!(
    RectangleHandle,
    Rectangle,
    rectangles,
    0,
    |h: RectangleHandle| h.id.0
);
update_handle!(
    CircleHandle,
    Circle,
    circles,
    1,
    |h: CircleHandle| h.id.0
);
update_handle!(LineHandle, Line, lines, 2, |h: LineHandle| h.id.0);
update_handle!(PathHandle, Path, paths, 3, |h: PathHandle| h.id.0);
update_handle!(TextHandle, Text, texts, 4, |h: TextHandle| h.id.0);
update_handle!(CameraHandle, Camera, cameras, 5, |_: CameraHandle| 0);

macro_rules! tuple_targets {
    ($($name:ident:$slot:tt),+) => {
        impl<$($name: UpdateHandle),+> UpdateTargets for ($($name,)+) {
            type Objects<'a> = ($(&'a mut $name::Object,)+);
            fn callback<S, F>(self, source: S, mut update: F, world: &Graphics) -> NativeUpdate
            where S: BindingSource<Graphics>, F: for<'a> FnMut(Self::Objects<'a>, S::Value) + ThreadSafe {
                let layout = Layout::new([$(self.$slot.spec($slot),)+], world);
                NativeUpdate(Box::new(move |world, bindings| {
                    let input = bindings.read_source(world, &source).expect("validated update source");
                    let mut batch = layout.borrow(world);
                    update(($($name::take(&mut batch, $slot),)+), input);
                }))
            }
        }
    }
}
tuple_targets!(A:0, B:1);
tuple_targets!(A:0, B:1, C:2);
tuple_targets!(A:0, B:1, C:2, D:3);

impl<H: UpdateHandle, const N: usize> UpdateTargets for [H; N] {
    type Objects<'a> = [&'a mut H::Object; N];
    fn callback<S, F>(
        self,
        source: S,
        mut update: F,
        world: &Graphics,
    ) -> NativeUpdate
    where
        S: BindingSource<Graphics>,
        F: for<'a> FnMut(Self::Objects<'a>, S::Value) + ThreadSafe,
    {
        let layout = Layout::<N>::new(
            std::array::from_fn(|i| self[i].spec(i)),
            world,
        );
        NativeUpdate(Box::new(move |world, bindings| {
            let input = bindings
                .read_source(world, &source)
                .expect("validated update source");
            let mut batch = layout.borrow(world);
            update(
                std::array::from_fn(|i| H::take(&mut batch, i)),
                input,
            );
        }))
    }
}

impl Scene {
    /// Register a direct callback. It runs on EVERY sample after bindings,
    /// in registration order. There is no caching or dependency sorting.
    /// Targets are mutable stored objects; source values are read before
    /// borrowing targets. Repeated targets panic during construction.
    /// For seeking, assign state from sampled inputs rather than accumulate dt.
    ///
    /// ```
    /// use motiongfx::prelude::{ease, ms};
    /// use vello_motiongfx::{Circle, Point, Scene};
    /// let mut scene = Scene::new();
    /// let x = scene.signal(0.0);
    /// let dot = scene.add(Circle::new(10.0));
    /// scene.update(dot, x, |circle, x| {
    ///     circle.position = Point::new(x, 30.0);
    /// });
    /// let mut animation = scene.animate(|sequence| {
    ///     sequence.play(x.to(100.0).ease(ease::linear));
    /// });
    /// animation.sample(ms(500));
    /// assert_eq!(animation.circle(dot).position, Point::new(50.0, 30.0));
    /// ```
    ///
    /// Targets retain their concrete Rust type:
    /// ```compile_fail
    /// use vello_motiongfx::{Circle, Point, Scene};
    /// let mut scene = Scene::new();
    /// let x = scene.signal(0.0);
    /// let dot = scene.add(Circle::new(10.0));
    /// scene.update(dot, x, |circle, _| {
    ///     circle.end = Point::ZERO; // A circle has no line endpoint.
    /// });
    /// ```
    pub fn update<T, S, F>(
        &mut self,
        targets: T,
        source: S,
        update: F,
    ) where
        T: UpdateTargets,
        S: BindingSource<Graphics>,
        F: for<'a> FnMut(T::Objects<'a>, S::Value) + ThreadSafe,
    {
        self.bindings
            .validate_source(&self.graphics, &source)
            .expect("invalid update source");
        self.updates.push(targets.callback(
            source,
            update,
            &self.graphics,
        ));
    }
}
