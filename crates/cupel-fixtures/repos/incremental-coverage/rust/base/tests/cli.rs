//! The ledger's binary, spawned as a user runs it: attributed to its own files and data
//! (`[lang.rust.attribution.binary]`), not to the library lines the binary executes.

#[test]
fn the_binary_scales_each_pair_of_its_input() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ledger"))
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/pairs.txt"))
        .output()
        .expect("the binary runs");
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "6\n-8\n");
}
