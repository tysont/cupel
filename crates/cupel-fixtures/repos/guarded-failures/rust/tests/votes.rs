// ABOUTME: A test failing under a negated predicate (strong), and one panicking only in an `if let`, the
// ABOUTME: `expect` it spells out, which asserts nothing (ADR-221).
use votes::{has_vote, parse, Server};

#[test]
fn a_voter_has_a_vote() {
    let servers = [Server {
        id: "id1".to_owned(),
        voter: true,
    }];
    if !has_vote(&servers, "id1") {
        panic!("id1 has no vote");
    }
}

#[test]
fn parsing_an_id_succeeds() {
    if let Err(error) = parse("id2") {
        panic!("{error}");
    }
}
