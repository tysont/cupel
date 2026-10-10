pub mod format;

use shapes::square;

pub fn summary(lines: &[String]) -> String {
    format::render(lines)
}

pub fn floor_area(side: i64) -> i64 {
    square(side)
}
