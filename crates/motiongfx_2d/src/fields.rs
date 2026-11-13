//! Public field paths for compatibility and advanced integrations.
//! Definitions live beside their owning objects; this module only re-exports.
pub use crate::camera::fields as camera;
pub use crate::camera::{camera_position, camera_position_axes};
pub use crate::chevron::fields as chevron;
pub use crate::circle::fields as circle;
pub use crate::circle::{circle_position, circle_position_axes};
pub use crate::line::fields as line;
pub use crate::line::{line_end, line_end_axes};
pub use crate::line::{line_start, line_start_axes};
pub use crate::path::fields as path;
pub use crate::path::{path_position, path_position_axes};
pub use crate::rectangle::fields as rectangle;
pub use crate::rectangle::{
    rectangle_position, rectangle_position_axes,
};
pub use crate::text::fields as text;
pub use crate::text::{text_position, text_position_axes};
pub use crate::text_selection::fields as text_selection;
pub use crate::triangle::fields as triangle;
