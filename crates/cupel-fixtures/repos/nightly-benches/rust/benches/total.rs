// ABOUTME: A libtest benchmark, which needs the unstable `test` crate and so a nightly toolchain; on stable the
// ABOUTME: feature gate below is rustc's E0554.
#![feature(test)]

extern crate test;

use test::Bencher;

#[bench]
fn totals_a_thousand_values(bencher: &mut Bencher) {
    let values: Vec<u64> = (0..1000).collect();
    bencher.iter(|| meter::total(&values));
}
