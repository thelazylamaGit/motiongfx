/// Optional syntax for scheduling ordinary typed animation descriptions.
///
/// `animate!(sequence, { ... })` takes a mutable `Sequence` reference. Animation
/// expressions end in `;` and are scheduled sequentially. Supported timing forms:
///
/// - `wait(duration);`
/// - `parallel { ... }` (mixed types and nested timing blocks)
/// - `chain { ... }` (a sequential branch inside a group)
/// - `parallel(changes);` (an iterator or array of same-type changes)
/// - `stagger(interval, changes);` (fixed spacing between successive starts)
///
/// Timing blocks optionally accept a trailing semicolon. Ordinary Rust loops,
/// bindings, conditions, defaults and helper calls stay outside macro invocations;
/// a loop or helper may itself contain `animate!`. The macro only generates calls
/// to the existing builder. It adds no playback evaluator or parser dependency.
#[macro_export]
macro_rules! animate {
    ($sequence:expr, { $($body:tt)* } $(,)?) => {{
        // Reborrow once, so expressions providing the context evaluate once
        // and the caller can continue using the original mutable reference.
        let __sequence = &mut *$sequence;
        $crate::animate!(@steps __sequence; $($body)*);
    }};

    (@steps $sequence:ident;) => {};

    (@steps $sequence:ident; ; $($rest:tt)*) => {
        $crate::animate!(@steps $sequence; $($rest)*);
    };

    (@steps $sequence:ident; wait($duration:expr); $($rest:tt)*) => {
        $sequence.wait($duration);
        $crate::animate!(@steps $sequence; $($rest)*);
    };

    (@steps $sequence:ident; parallel { $($body:tt)* } $($rest:tt)*) => {
        $sequence.parallel(|__group| {
            $crate::animate!(@steps __group; $($body)*);
        });
        $crate::animate!(@steps $sequence; $($rest)*);
    };

    (@steps $sequence:ident; chain { $($body:tt)* } $($rest:tt)*) => {
        $sequence.chain(|__group| {
            $crate::animate!(@steps __group; $($body)*);
        });
        $crate::animate!(@steps $sequence; $($rest)*);
    };

    (@steps $sequence:ident; parallel($changes:expr); $($rest:tt)*) => {
        $sequence.play_all($changes);
        $crate::animate!(@steps $sequence; $($rest)*);
    };

    (@steps $sequence:ident; stagger($interval:expr, $changes:expr); $($rest:tt)*) => {
        let __changes = $changes;
        $sequence.flow($interval, |__group| {
            for __change in __changes {
                __group.play(__change);
            }
        });
        $crate::animate!(@steps $sequence; $($rest)*);
    };

    (@steps $sequence:ident; $change:expr; $($rest:tt)*) => {
        // Evaluate before borrowing the sequence for scheduling. This also
        // guarantees the animation expression is evaluated exactly once.
        let __change = $change;
        $sequence.play(__change);
        $crate::animate!(@steps $sequence; $($rest)*);
    };
}
/// A computed relationship with explicitly named dependencies. Both single
/// expressions and multi-statement blocks are ordinary typed Rust closures.
#[macro_export]
macro_rules! computed {
    (($($name:ident),+ $(,)?) => $body:expr $(,)?) => {
        $crate::computed(($(&$name,)+), move |($($name,)+)| $body)
    };
}
