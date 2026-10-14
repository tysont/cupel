# CUPEL_DECISION_038: Unregistered-language roots, language-only config validation, and `doctor`'s tool probes

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P1.9

## Context

Three P1.9 questions are open from earlier ADRs:

1. ADR-021 recognizes Maven, Gradle, and `deps.edn` roots as `java` and `clojure` and puts them in `[roots]`, but those languages are not registered until P6. Once the CLI validates configuration against the language registry (ADR-017 §7), a generated `cupel.toml` naming them would be rejected.
2. `Config::validate_registered(checks, langs)` rejects every `[checks.*]` table and gate key while no check is registered (P2.1), so it cannot be called yet as written.
3. ADR-022 left `doctor`'s toolchain requirements empty until the table existed.

## Options

For (1): register `java` and `clojure` as languages without producers (a language whose `describe` promises nothing, and P6 then changes its meaning); drop the roots silently; or keep detecting them, leave them out of `cupel.toml`, and say so with an `info` finding.

For (2): call `validate_registered` and reject check tables until P2.1 (breaks every configuration that sets a check option, as design §6.7's example does); or split out the language half.

For (3): require every configured tool (exit 3 when, say, `pnpm` is missing, although no check runs tests until P3); or warn.

## Decision

1. Java and Clojure roots are still detected (`cupel_lang::project::unregistered_roots`). `init` leaves them out of `[roots]` and reports each as an `info` finding `init.unsupported-root` (check `init`, subject `<lang>:<path>`, action `review`), titled "unsupported language root: java at ./svc — supported in a later release", with evidence `{lang, path, build_files}`. The finding appears in a preview and after `--write`.
2. `Config::validate_languages(&[LangId])` rejects unknown language IDs in `[lang]`, `[roots]`, and per-check `lang` overrides; `validate_registered` calls it and adds the check and gate validation. The CLI calls `validate_languages` with the registered languages on every configuration load; an unknown language is a configuration error like any other (exit 2; `init` ignores it, `doctor` reports it). P2.1 switches the call to `validate_registered` once checks register.
3. `doctor` probes the program of every command a language would use: the configured `[lang.<id>]` `test`/`coverage`/`bench`, falling back for unset keys to the command `init` would choose for the detected roots. Each `&&`-separated part's first word is a program, probed with `<program> --version` unless `[probes]` in the toolchain table says otherwise. A missing program is `doctor.tool-missing` (subject the program, `warn`, `install_tool`), `needed_for` naming the key and, for a table default, the selected producer, such as ``[lang.rust] test (tool.cargo-test)``; a configured command names only its key, since the table did not choose it. One finding per program lists every use. They are not `required` until P3's checks run these commands, so they never make `doctor` exit 3 today.

## Consequences

- Generated configurations always pass validation; P6 makes Java and Clojure roots appear in `[roots]` by registering the languages, with no `init` change.
- `doctor` on a repository without `cupel.toml` already reports missing toolchains for what it detects.
- P3 decides which missing programs become `required` (the configured checks' runners) and replaces the presence probe with the selected producer's `availability`.
