//! Static borrowing support for native update target collections.
use crate::{BindingSource, NativeUpdate, UpdateTargets, World};
use motiongfx::prelude::ThreadSafe;
/// Optional model contract for safe array/tuple target borrowing. Prepare a
/// checked layout once, then borrow disjoint references without allocation.
/// This is model access, not a scheduler or an ECS storage requirement.
pub trait UpdateStorage: Sized + 'static {
    type Spec: Copy;
    type Layout<const N: usize>: ThreadSafe;
    type Batch<'a, const N: usize>;
    fn layout<const N: usize>(
        &self,
        specs: [Self::Spec; N],
    ) -> Self::Layout<N>;
    fn borrow<'a, const N: usize>(
        &'a mut self,
        layout: &Self::Layout<N>,
    ) -> Self::Batch<'a, N>;
}
/// A typed handle's selection and extraction within its model's borrowed batch.
pub trait UpdateHandle<M: UpdateStorage>: Copy + ThreadSafe {
    type Object;
    fn spec(self, slot: usize) -> M::Spec;
    fn take<'a, const N: usize>(
        batch: &mut M::Batch<'a, N>,
        slot: usize,
    ) -> &'a mut Self::Object;
}
macro_rules! tuple_targets {
    ($($name:ident:$slot:tt),+) => {
        impl<M:UpdateStorage,$($name: UpdateHandle<M>),+> UpdateTargets<M> for ($($name,)+) {
            type Objects<'a> = ($(&'a mut $name::Object,)+);
            #[track_caller]
            fn callback<S, F>(self, source: S, mut update: F, world: &World<M>) -> NativeUpdate<M>
            where S: BindingSource<World<M>>, F: for<'a> FnMut(Self::Objects<'a>, S::Value) + ThreadSafe {
                let layout = world.model().layout([$(self.$slot.spec($slot),)+]);
                NativeUpdate::new(move |world, bindings| {
                    let input = bindings.read_source(world, &source).expect("validated update source");
                    let mut batch = world.model_mut().borrow(&layout);
                    update(($($name::take(&mut batch, $slot),)+), input);
                })
            }
        }
    }
}
tuple_targets!(A:0, B:1);
tuple_targets!(A:0, B:1, C:2);
tuple_targets!(A:0, B:1, C:2, D:3);

impl<M: UpdateStorage, H: UpdateHandle<M>, const N: usize>
    UpdateTargets<M> for [H; N]
{
    type Objects<'a> = [&'a mut H::Object; N];
    #[track_caller]
    fn callback<S, F>(
        self,
        source: S,
        mut update: F,
        world: &World<M>,
    ) -> NativeUpdate<M>
    where
        S: BindingSource<World<M>>,
        F: for<'a> FnMut(Self::Objects<'a>, S::Value) + ThreadSafe,
    {
        let layout =
            world.model().layout(std::array::from_fn::<_, N, _>(
                |i| self[i].spec(i),
            ));
        NativeUpdate::new(move |world, bindings| {
            let input = bindings
                .read_source(world, &source)
                .expect("validated update source");
            let mut batch = world.model_mut().borrow(&layout);
            update(
                std::array::from_fn(|i| H::take(&mut batch, i)),
                input,
            );
        })
    }
}
