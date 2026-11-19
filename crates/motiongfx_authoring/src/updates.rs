//! Lets `scene.bind_mut` borrow several objects mutably at once: a tuple
//! or array of handles, checked when the `bind_mut` is added.
use crate::{
    BindMutTargets, BindMutWriter, BindingDependency, World,
};
use motiongfx::prelude::ThreadSafe;
/// A model whose objects `scene.bind_mut` can borrow several at a time.
///
/// [`layout`](Self::layout) checks a set of targets once, when the
/// `bind_mut` is added; [`borrow`](Self::borrow) then hands out the mutable references on
/// every sample without allocating.
pub trait BindMutStorage: Sized + 'static {
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
/// An object handle that can be one of several `scene.bind_mut` targets.
pub trait BindMutHandle<M: BindMutStorage>:
    Copy + ThreadSafe
{
    /// The object the callback receives.
    type Object;
    /// Describe this target, at position `slot` in the tuple or array.
    fn spec(self, slot: usize) -> M::Spec;
    /// The whole object, so bindings that read it run after the
    /// `bind_mut` that writes it.
    fn dependency(self) -> BindingDependency;
    /// Take this target's object out of a borrowed batch.
    fn take<'a, const N: usize>(
        batch: &mut M::Batch<'a, N>,
        slot: usize,
    ) -> &'a mut Self::Object;
}
macro_rules! tuple_targets {
    ($($name:ident:$slot:tt),+) => {
        impl<M: BindMutStorage, $($name: BindMutHandle<M>),+> BindMutTargets<M> for ($($name,)+) {
            type Objects<'a> = ($(&'a mut $name::Object,)+);
            fn dependencies(&self) -> Vec<BindingDependency> {
                vec![$(self.$slot.dependency(),)+]
            }
            #[track_caller]
            fn writer<V: 'static, F>(self, mut write: F, world: &World<M>) -> BindMutWriter<M, V>
            where F: for<'a> FnMut(Self::Objects<'a>, V) + ThreadSafe {
                let layout = world.model().layout([$(self.$slot.spec($slot),)+]);
                Box::new(move |world, value| {
                    let mut batch = world.model_mut().borrow(&layout);
                    write(($($name::take(&mut batch, $slot),)+), value);
                })
            }
        }
    }
}
tuple_targets!(A:0, B:1);
tuple_targets!(A:0, B:1, C:2);
tuple_targets!(A:0, B:1, C:2, D:3);

impl<M: BindMutStorage, H: BindMutHandle<M>, const N: usize>
    BindMutTargets<M> for [H; N]
{
    type Objects<'a> = [&'a mut H::Object; N];
    fn dependencies(&self) -> Vec<BindingDependency> {
        self.iter().map(|handle| handle.dependency()).collect()
    }
    #[track_caller]
    fn writer<V: 'static, F>(
        self,
        mut write: F,
        world: &World<M>,
    ) -> BindMutWriter<M, V>
    where
        F: for<'a> FnMut(Self::Objects<'a>, V) + ThreadSafe,
    {
        let layout =
            world.model().layout(std::array::from_fn::<_, N, _>(
                |i| self[i].spec(i),
            ));
        Box::new(move |world, value| {
            let mut batch = world.model_mut().borrow(&layout);
            write(
                std::array::from_fn(|i| H::take(&mut batch, i)),
                value,
            );
        })
    }
}
