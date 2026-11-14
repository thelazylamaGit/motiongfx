//! Safe typed access to 2D callback targets.
use crate::*;
use motiongfx::prelude::ThreadSafe;
use motiongfx_authoring::{
    NativeUpdate, UpdateHandle, UpdateStorage, UpdateTargets,
};
#[doc(hidden)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectSpec {
    kind: u8,
    index: usize,
    slot: usize,
}

// Each type has separate storage. The array slots retain the caller's order;
// sorted specs let split_at_mut obtain disjoint references within each Vec.
pub struct Layout<const N: usize>([ObjectSpec; N]);
impl<const N: usize> Layout<N> {
    #[track_caller]
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
                world.triangles.len(),
                world.chevrons.len(),
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
            triangles: select(&self.0, 6, &mut world.triangles),
            chevrons: select(&self.0, 7, &mut world.chevrons),
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
    triangles: [Option<&'a mut Triangle>; N],
    chevrons: [Option<&'a mut Chevron>; N],
}
macro_rules! update_handle {
    ($handle:ty, $object:ty, $field:ident, $kind:literal, $index:expr) => {
        impl UpdateHandle<Graphics> for $handle {
            type Object = $object;
            fn spec(self, slot: usize) -> ObjectSpec {
                ObjectSpec {
                    kind: $kind,
                    index: ($index)(self),
                    slot,
                }
            }
            fn take<'a, const N: usize>(
                batch: &mut <Graphics as UpdateStorage>::Batch<'a, N>,
                slot: usize,
            ) -> &'a mut Self::Object {
                batch.$field[slot]
                    .take()
                    .expect("typed update target")
            }
        }
        impl UpdateTargets<Graphics> for $handle {
            type Objects<'a> = &'a mut $object;
            #[track_caller]
            fn callback<S, F>(
                self,
                source: S,
                mut update: F,
                world: &World<Graphics>,
            ) -> NativeUpdate<Graphics>
            where
                S: BindingSource<World<Graphics>>,
                F: for<'a> FnMut(Self::Objects<'a>, S::Value)
                    + ThreadSafe,
            {
                let layout =
                    Layout::new([self.spec(0)], world.model());
                NativeUpdate::new(move |world, bindings| {
                    let input = bindings
                        .read_source(world, &source)
                        .expect("validated update source");
                    let mut batch = layout.borrow(world.model_mut());
                    update(Self::take(&mut batch, 0), input);
                })
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
update_handle!(
    TriangleHandle,
    Triangle,
    triangles,
    6,
    |h: TriangleHandle| h.id.0
);
update_handle!(
    ChevronHandle,
    Chevron,
    chevrons,
    7,
    |h: ChevronHandle| h.id.0
);

impl<S: Copy + ThreadSafe, E: Copy + ThreadSafe>
    UpdateHandle<Graphics> for LineTipHandle<S, E>
{
    type Object = Line;
    fn spec(self, slot: usize) -> ObjectSpec {
        self.line.spec(slot)
    }
    fn take<'a, const N: usize>(
        batch: &mut <Graphics as UpdateStorage>::Batch<'a, N>,
        slot: usize,
    ) -> &'a mut Line {
        LineHandle::take(batch, slot)
    }
}
impl<S: Copy + ThreadSafe, E: Copy + ThreadSafe>
    UpdateTargets<Graphics> for LineTipHandle<S, E>
{
    type Objects<'a> = &'a mut Line;
    #[track_caller]
    fn callback<I, F>(
        self,
        source: I,
        update: F,
        world: &World<Graphics>,
    ) -> NativeUpdate<Graphics>
    where
        I: BindingSource<World<Graphics>>,
        F: for<'a> FnMut(&'a mut Line, I::Value) + ThreadSafe,
    {
        self.line.callback(source, update, world)
    }
}
update_handle!(PathHandle, Path, paths, 3, |h: PathHandle| h.id.0);
update_handle!(TextHandle, Text, texts, 4, |h: TextHandle| h.id.0);
update_handle!(CameraHandle, Camera, cameras, 5, |_: CameraHandle| 0);

impl UpdateStorage for Graphics {
    type Spec = ObjectSpec;
    type Layout<const N: usize> = Layout<N>;
    type Batch<'a, const N: usize> = ObjectBatch<'a, N>;
    #[track_caller]
    fn layout<const N: usize>(
        &self,
        specs: [ObjectSpec; N],
    ) -> Layout<N> {
        Layout::new(specs, self)
    }
    fn borrow<'a, const N: usize>(
        &'a mut self,
        layout: &Layout<N>,
    ) -> ObjectBatch<'a, N> {
        layout.borrow(self)
    }
}
