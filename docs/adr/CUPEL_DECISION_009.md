# CUPEL_DECISION_009: The daemon protocol is JSON Lines over a Unix stream socket, one request per connection

**Date:** 2026-10-15 · **Status:** accepted · **Phase:** P4.2

## Context

`cupel serve` (design §6.5, §8.10) answers CLI invocations over `.cupel/serve.sock`, and the output must be byte-identical to a cold run (design §9.5). The client sends an invocation (arguments, working directory, whether stdout is a terminal, the `CUPEL_*` variables) and gets back what a cold process would have written: stdout, stderr, the exit code, and, while the run is going, the `--events stderr` lines in order (design §7.4: "`serve` exposes the same stream over the socket"). Plan §2.5 reserves this ADR for the framing: length-prefixed JSON, JSON Lines, or something else. `serve --mcp` (P7.7) will speak MCP's own framing and is not decided here.

## Options

**Length-prefixed JSON** (a 4-byte big-endian length, then that many bytes of UTF-8 JSON). The reader knows each message's size before reading it, so an oversized message is refused before any allocation, and a message may hold any bytes. Nothing in Cupel's messages needs the latter: every payload is text that `serde_json` escapes. A prefix makes the socket opaque to `nc -U` and `socat`, and every reader needs the same four-byte dance.

**JSON Lines** (one JSON object per line, `\n`-terminated). `serde_json::to_string` never emits a raw newline (newlines inside strings are escaped as `\n`), so a line is always exactly one message. It is the encoding `--events` already uses, so an event line travels as a string inside a message without re-encoding rules of its own, and a person can talk to the daemon with `nc -U .cupel/serve.sock` while debugging. A reader caps a line's length while reading it (`BufRead::take`), which bounds memory as well as a prefix does.

**A binary encoding** (MessagePack, bincode, protobuf). Smaller and faster to parse, but envelopes are already JSON text when they reach the protocol and parsing them is not on the warm path (the cost is the run). It would add a dependency and a schema to keep in step for no measurable gain.

**Connections.** One request per connection, or a persistent connection carrying many. A CLI invocation is one request, and the client process ends after it; a connection per request keeps the server stateless per connection, makes the client's hang-up an unambiguous "stop working on this" signal, and costs one `connect` (microseconds on a Unix socket).

## Decision

1. **Transport:** a Unix stream socket, by default `<root>/.cupel/serve.sock`, created with mode `0600` so only the user who started the daemon can connect.
2. **Framing:** JSON Lines. Each message is one compact JSON object followed by `\n`, at most **64 MiB** including the newline. A message is an object with a `"type"` member naming its kind; unknown members are ignored, so either side may add optional members within a protocol version.
3. **Exchange:** one request per connection. The client connects, writes one request line, and reads reply lines until a final one; it keeps its end open until then, because the daemon treats end-of-file from the client before the final reply as a hang-up and cancels the request's run (ADR-085 §7).
4. **Requests** (client → daemon). Every request carries `"protocol": 1` and `"build"`: the client's `cupel` version and a fingerprint of its executable (ADR-085 §4).
   - `{"type":"run","protocol":1,"build":…,"path":"…","invocation":{"args":[…],"current_dir":"…","stdout_is_terminal":false,"variables":{…},"no_color":false}}` runs one invocation. `path` is the client's `PATH`.
   - `{"type":"status",…}` asks what the daemon is.
   - `{"type":"stop",…}` asks it to exit.
5. **Replies** (daemon → client). Zero or more `{"type":"stderr","text":"…"}` lines, in the order the run wrote them, then exactly one final line:
   - `{"type":"done","stdout":"…","stderr":"…","code":N}`: the invocation's output, exactly what a cold process would write (its `stderr` is what the run had not yet streamed).
   - `{"type":"status","version":…,"build":…,"pid":N,"root":"…","socket":"…","watching":bool,"served":N,"reparsed":N}`.
   - `{"type":"stopping"}`.
   - `{"type":"refused","reason":"…"}`: the daemon will not run this request (another build, another protocol, another `PATH`; ADR-085 §4–5). The reason is a sentence for a person.
   - `{"type":"error","message":"…"}`: the request was malformed (not JSON, not an object, an unknown `type`, a missing member, a line over the limit, or end-of-file before a newline).
6. `status` and `stop` are answered whatever the client's build, so a newer `cupel` can see and stop an older daemon; only `run` is refused across builds.

## Consequences

- The protocol is debuggable with standard tools: `printf '{"type":"status","protocol":1,"build":{"version":"","binary":""}}\n' | nc -U .cupel/serve.sock` prints a status line.
- Text is the only payload; a future binary payload (a SARIF file is still text) would need a new message type with base64 or a protocol version 2.
- Streaming is per line, so the client can forward each `stderr` line as it arrives; `--events stderr` reaches the client in emission order (`cupel-serve` tests, ADR-085 §6).
- One request per connection means no multiplexing and no per-connection state; concurrency is concurrent connections (ADR-085 §8).
- The limit of 64 MiB bounds a malicious or broken peer's memory; an envelope larger than that (a survey of a very large tree without `--limit`) cannot cross the socket and the client runs locally instead (ADR-085 §3).
