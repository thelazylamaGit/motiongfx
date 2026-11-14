/// A value calculated from named signals or computed values, without
/// repeating their names: `computed!((a, b) => a * b)` is
/// `computed((&a, &b), move |(a, b)| a * b)`. The body can be an
/// expression or a block.
///
/// ```
/// use motiongfx_authoring::{computed, signal};
/// let progress = signal(0.0);
/// let radius = signal(80.0);
/// let position = computed!((progress, radius) => {
///     let angle = progress * std::f64::consts::TAU;
///     (radius * angle.cos(), radius * angle.sin())
/// });
/// # let _: motiongfx_authoring::Computed<motiongfx_authoring::World<()>, _> = position;
/// ```
#[macro_export]
macro_rules! computed {
    (($($name:ident),+ $(,)?) => $body:expr $(,)?) => {
        $crate::computed(($(&$name,)+), move |($($name,)+)| $body)
    };
}
