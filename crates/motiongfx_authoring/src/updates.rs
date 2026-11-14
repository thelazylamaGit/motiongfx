//! Lets `scene.update` borrow several objects mutably at once: a tuple or
//! array of handles, checked when the update is added.
use crate::{BindingSource, NativeUpdate, UpdateTargets, World};
use motiongfx::prelude::ThreadSafe;
/// A model whose objects `scene.update` can borrow several at a time.
///
/// [`layout`](Self::layout) checks a set of targets once, when the update is
/// added; [`borrow`](Self::borrow) then hands out the mutable references on
/// every sample without allocating.
pub trait UpdateStorage: Sized + 'static {
    /// Which object one target refers to.
    type Spec: Copy;
    /// A checked set of `N` targets.
    type Layout<const N: usize>: ThreadSafe;
    /// The borrowed objects for one call.
    type Batch<'a, const N: usize>;
    /// Check `specs` (they exist, and none is borrowed twice).
    ///
    /// # Panics
    ///
    /// Implementations panic if a target doesn't exist or appears twice.
    fn layout<const N: usize>(
        &self,
        specs: [Self::Spec; N],
    ) -> Self::Layout<N>;
    /// Borrow the objects of a checked layout.
    fn borrow<'a, const N: usize>(
        &'a mut self,
        layout: &Self::Layout<N>,
    ) -> Self::Batch<'a, N>;
}
/// An object handle that can be one of several `scene.update` targets.
pub trait UpdateHandle<M: UpdateStorage>: Copy + ThreadSafe {
    /// The object the callback receives.
    type Object;
    /// Describe this target, at position `slot` in the tuple or array.
    fn spec(self, slot: usize) -> M::Spec;
    /// Take this target's object out of a borrowed batch.
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
