//! A test that fails on every other run: state outside the process flips its outcome between
//! identical runs.

#[test]
fn settles_on_every_other_run() {
    let marker = std::path::Path::new(".settled");
    if marker.exists() {
        std::fs::remove_file(marker).unwrap();
        panic!("settled twice in a row");
    }
    std::fs::write(marker, "").unwrap();
}
