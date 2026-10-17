// ABOUTME: The Cupel daemon behind `cupel serve` (design §8.10): the socket protocol (ADR-009), the server loop,
// ABOUTME: the client the CLI connects with, the file watcher, and the MCP server; the CLI supplies what they run.
//! The daemon's transport, independent of what it runs.
//!
//! [`serve`] listens on a Unix socket and runs each [`Body::Run`] request through a [`Handler`],
//! streaming what the run writes to standard error back as it is written; `cupel-cli`'s handler
//! runs the same code a cold process does, holding caches across requests (ADR-085). [`request`]
//! is the client side. [`Watch`] reports changed files to the handler under `serve --watch`.
//! The wire format is [`protocol`] (ADR-009). [`mcp`] serves the same runs as Model Context
//! Protocol tools over stdio (`serve --mcp`, ADR-121).

mod client;
mod lines;
pub mod mcp;
mod mcp_client;
pub mod protocol;
mod server;
mod watch;

pub use client::{ClientError, request};
pub use protocol::{
    Body, Build, Done, FrameError, Invocation, MAX_MESSAGE, PROTOCOL, Reply, Request, Status,
    read_message, read_message_within, write_message,
};
pub use server::{Daemon, Handler, ServeError, Served, serve};
pub use watch::Watch;
