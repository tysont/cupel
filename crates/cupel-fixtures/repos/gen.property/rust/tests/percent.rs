use units::percent::clamp_percent;

#[test]
fn clamps_above_a_hundred() {
    assert_eq!(clamp_percent(250), 100);
}
