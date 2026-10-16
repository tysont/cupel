// ABOUTME: Tests for the daemon protocol's framing (ADR-009): every message round-trips as one JSON line,
// ABOUTME: and malformed input (not JSON, wrong shape, cut short, too long) is an error, never a message.
use std::collections::BTreeMap;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};

use cupel_serve::{
    Body, Build, Done, FrameError, Invocation, PROTOCOL, Reply, Request, Status, read_message,
    read_message_within, write_message,
};

/// Version 0.1.0 of an executable that is not there, so its fingerprint is empty.
fn build() -> Build {
    Build::of("0.1.0", Path::new("/no/such/cupel"))
}

fn invocation() -> Invocation {
    Invocation {
        args: vec![
            "check".to_owned(),
            "--paths".to_owned(),
            "a b\nc".to_owned(),
        ],
        current_dir: Some(PathBuf::from("/repo")),
        stdout_is_terminal: true,
        variables: BTreeMap::from([("CUPEL_DEPTH".to_owned(), "quick".to_owned())]),
        no_color: true,
    }
}

fn requests() -> Vec<Request> {
    vec![
        Request::new(
            build(),
            Body::Run {
                path: Some("/bin:/usr/bin".to_owned()),
                invocation: invocation(),
            },
        ),
        Request::new(build(), Body::Status),
        Request::new(build(), Body::Stop),
    ]
}

fn replies() -> Vec<Reply> {
    vec![
        Reply::Stderr {
            text: "{\"ev\":\"plan\"}\n".to_owned(),
        },
        Reply::Done(Done {
            stdout: "{\n  \"status\": \"pass\"\n}\n".to_owned(),
            stderr: "cupel: ünïcode\n".to_owned(),
            code: 5,
        }),
        Reply::Status(Status {
            version: "0.1.0".to_owned(),
            build: build(),
            pid: 42,
            root: PathBuf::from("/repo"),
            socket: PathBuf::from("/repo/.cupel/serve.sock"),
            watching: true,
            served: 3,
            reparsed: 7,
        }),
        Reply::Stopping,
        Reply::Refused {
            reason: "another build".to_owned(),
        },
        Reply::Error {
            message: "not JSON".to_owned(),
        },
    ]
}

/// Every message in `bytes`, read one after another to the end.
fn read_all<T: serde::de::DeserializeOwned>(bytes: Vec<u8>) -> Vec<T> {
    let mut input = BufReader::new(Cursor::new(bytes));
    std::iter::from_fn(|| read_message::<T>(&mut input).unwrap()).collect()
}

/// `messages` written one after another to one buffer.
fn written<T: serde::Serialize>(messages: &[T]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for message in messages {
        write_message(&mut bytes, message).unwrap();
    }
    bytes
}

#[test]
fn every_message_is_one_line_and_reads_back_as_itself() {
    let requests = requests();
    let replies = replies();
    let request_bytes = written(&requests);
    let reply_bytes = written(&replies);
    let lines = |bytes: &[u8]| String::from_utf8_lossy(bytes).matches('\n').count();
    assert_eq!(
        (lines(&request_bytes), lines(&reply_bytes)),
        (requests.len(), replies.len()),
        "newlines inside strings are escaped"
    );
    assert_eq!(
        (read_all(request_bytes), read_all(reply_bytes)),
        (requests, replies)
    );
}

#[test]
fn messages_carry_their_type_and_the_protocol_and_build_on_every_request() {
    let line = String::from_utf8(written(&[Request::new(build(), Body::Status)])).unwrap();
    assert_eq!(
        line,
        "{\"protocol\":1,\"build\":{\"version\":\"0.1.0\",\"binary\":\"\"},\"type\":\"status\"}\n"
    );
    // Hand-written, in another member order, with a member this version does not know.
    let text = "{\"type\":\"stop\",\"build\":{\"version\":\"9\",\"binary\":\"\"},\"protocol\":1,\"later\":true}\n";
    let request: Request = read_message(&mut BufReader::new(text.as_bytes()))
        .unwrap()
        .unwrap();
    assert_eq!((request.protocol, request.body), (PROTOCOL, Body::Stop));
    let done = String::from_utf8(written(&[Reply::Stopping])).unwrap();
    assert_eq!(done, "{\"type\":\"stopping\"}\n");
}

#[test]
fn the_end_of_input_before_any_byte_is_no_message() {
    let mut input = BufReader::new(Cursor::new(Vec::new()));
    assert!(read_message::<Request>(&mut input).unwrap().is_none());
}

/// What reading one `Request` from `text` fails with.
fn refused(text: &str) -> FrameError {
    read_message::<Request>(&mut BufReader::new(text.as_bytes())).unwrap_err()
}

/// The kind of error reading one `Request` from `text` fails with.
fn refusal_kind(text: &str) -> &'static str {
    match refused(text) {
        FrameError::Malformed(_) => "malformed",
        FrameError::Truncated => "truncated",
        FrameError::TooLong { .. } => "too long",
        FrameError::Io(_) => "io",
    }
}

#[test]
fn malformed_input_is_an_error_naming_what_is_wrong() {
    let cases = [
        ("not json\n", "malformed"),
        ("[1, 2]\n", "malformed"),
        (
            "{\"type\":\"dance\",\"protocol\":1,\"build\":{\"version\":\"\",\"binary\":\"\"}}\n",
            "malformed",
        ),
        ("{\"type\":\"status\",\"protocol\":1}\n", "malformed"),
        (
            "{\"type\":\"run\",\"protocol\":1,\"build\":{\"version\":\"\",\"binary\":\"\"}}\n",
            "malformed",
        ),
        ("{\"type\":\"status\"", "truncated"),
    ];
    let kinds: Vec<(&str, &str)> = cases
        .iter()
        .map(|(text, _)| (*text, refusal_kind(text)))
        .collect();
    assert_eq!(kinds, cases.to_vec());
    let message = refused("not json\n").to_string();
    assert!(
        message.starts_with("the message is not valid: "),
        "{message}"
    );
}

#[test]
fn a_line_longer_than_the_limit_is_refused_before_it_is_read_whole() {
    let line = format!("{{\"type\":\"stderr\",\"text\":\"{}\"}}\n", "x".repeat(100));
    let mut input = BufReader::new(Cursor::new(line.clone().into_bytes()));
    let error = read_message_within::<Reply>(&mut input, 64).unwrap_err();
    assert!(
        matches!(error, FrameError::TooLong { limit: 64 }),
        "{error}"
    );
    assert_eq!(
        error.to_string(),
        "the message is longer than the limit of 64 bytes"
    );
    let mut input = BufReader::new(Cursor::new(line.clone().into_bytes()));
    assert!(read_message_within::<Reply>(&mut input, line.len()).is_ok());
}
