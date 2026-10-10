pub mod format;

pub fn summary(lines: &[String]) -> String {
    format::render(lines)
}
