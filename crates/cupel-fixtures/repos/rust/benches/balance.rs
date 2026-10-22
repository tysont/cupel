//! A bench: Cargo builds it as a crate of its own, and `bench_main!` writes its entry point, as
//! criterion's `criterion_main!` does, so nothing imports it and no function in it is `main`.

macro_rules! bench_main {
    ($($bench:ident),*) => {
        fn main() {
            $($bench();)*
        }
    };
}

fn balance_of_three() {
    std::hint::black_box(ledger::balance(&[1, 2, 3]));
}

bench_main!(balance_of_three);
