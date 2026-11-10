//! Minimal 5x7 vector lettering for the explainer's short ASCII labels.
//! Kept out of the lesson: a production application should use a proper
//! text shaping/font stack. These labels need no platform font files.

use vello::Scene;
use vello::kurbo::{Affine, BezPath, Point, Rect, Shape};
use vello::peniko::{Color, Fill};

/// Each byte is a row of five pixels, most significant visible bit first.
fn glyph(c: char) -> [u8; 7] {
    match c {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [31, 4, 4, 4, 4, 4, 31],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' | '0' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 27, 17],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '=' => [0, 0, 31, 0, 31, 0, 0],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        '(' => [2, 4, 8, 8, 8, 4, 2],
        ')' => [8, 4, 2, 2, 2, 4, 8],
        ',' => [0, 0, 0, 0, 0, 4, 8],
        '.' => [0, 0, 0, 0, 0, 0, 4],
        _ => [0; 7],
    }
}

pub fn draw(
    scene: &mut Scene,
    xf: Affine,
    origin: Point,
    text: &str,
    ink: Color,
    size: f64,
) {
    // Append every pixel rectangle to one path: one Vello fill per label.
    let mut path = BezPath::new();
    for (index, c) in text.chars().enumerate() {
        for (row, bits) in glyph(c).into_iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) != 0 {
                    let x =
                        origin.x + (index * 6 + column) as f64 * size;
                    let y = origin.y + row as f64 * size;
                    path.extend(
                        Rect::new(x, y, x + size, y + size)
                            .path_elements(0.1),
                    );
                }
            }
        }
    }
    scene.fill(Fill::NonZero, xf, ink, None, &path);
}
