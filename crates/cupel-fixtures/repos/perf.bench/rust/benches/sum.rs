use criterion::{criterion_group, criterion_main, Criterion};

fn sum(c: &mut Criterion) {
    let values: Vec<u64> = (0..100).collect();
    c.bench_function("sum", |b| b.iter(|| units::sum(std::hint::black_box(&values))));
}

criterion_group!(benches, sum);
criterion_main!(benches);
