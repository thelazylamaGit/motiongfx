//! [`Background`]: a model's clear colour, set with
//! [`Scene::set_background`].
use crate::{
    CompiledScene, Input, Interp, Property, Scene, World, WorldObject,
};
use motiongfx::prelude::ThreadSafe;

/// A model with a background: the colour renderers clear each frame with
/// before drawing. Implementing it gives scenes with this model
/// [`Scene::set_background`] and [`CompiledScene::background`].
///
/// The model picks the colour type, so a Vello-based model and a
/// Bevy-based one each use their own.
pub trait Background: 'static {
    /// The colour type, such as `peniko::Color`.
    type Color: Clone + PartialEq + ThreadSafe;
    /// The background colour.
    fn background(&self) -> &Self::Color;
    /// The background colour, mutably.
    fn background_mut(&mut self) -> &mut Self::Color;
    /// Blend from `start` to `end` by `t` (0 to 1), for animating it.
    fn blend(
        start: &Self::Color,
        end: &Self::Color,
        t: f32,
    ) -> Self::Color;
}

/// Identifies a scene's background colour.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct BackgroundId;

/// The background as a property, to animate with `.to(..)`.
pub type BackgroundProperty<M> = Property<
    BackgroundId,
    <M as Background>::Color,
    <M as Background>::Color,
>;

// `path!` takes a plain type path, not `<M as Background>::Color`.
type ColorOf<M> = <M as Background>::Color;

fn property<M: Background>() -> BackgroundProperty<M> {
    Property {
        id: BackgroundId,
        path: motiongfx::path!(ColorOf::<M>),
        interp: Interp::Smooth(M::blend),
    }
}

impl<M: Background> WorldObject<BackgroundId, M> for M::Color {
    fn get(world: &World<M>, _: BackgroundId) -> Option<&Self> {
        Some(world.model().background())
    }
    fn get_mut(
        world: &mut World<M>,
        _: BackgroundId,
    ) -> Option<&mut Self> {
        Some(world.model_mut().background_mut())
    }
}

impl<M: Background> Scene<M> {
    /// Set the background colour: fixed, or following a signal or
    /// computed value. The last call wins. Returns the background as a
    /// property, so it can also be animated with `.to(..)`.
    pub fn set_background(
        &mut self,
        color: impl Into<Input<M, M::Color>>,
    ) -> BackgroundProperty<M> {
        // Both arms prepare under the same key, so a later call's (possibly
        // absent) binding replaces this one instead of conflicting with it.
        match color.into() {
            Input::Fixed(value) => {
                *self.model_mut().background_mut() = value;
                self.prepare_with::<BackgroundId>(|_| {});
            }
            color => {
                self.prepare_with::<BackgroundId>(move |scene| {
                    scene.bind(
                        property::<M>(),
                        color,
                        std::convert::identity,
                    );
                })
            }
        }
        property::<M>()
    }
}

impl<M: Background> CompiledScene<M> {
    /// The background colour as last sampled.
    pub fn background(&self) -> &M::Color {
        self.model().background()
    }
}
