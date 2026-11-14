/// A computed relationship with explicitly named dependencies. Both single
/// expressions and multi-statement blocks are ordinary typed Rust closures.
#[macro_export]
macro_rules! computed {
    (($($name:ident),+ $(,)?) => $body:expr $(,)?) => {
        $crate::computed(($(&$name,)+), move |($($name,)+)| $body)
    };
}
