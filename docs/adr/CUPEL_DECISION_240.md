# CUPEL_DECISION_240: Leiningen is installed only from Cupel's own copy of its release, each file checked against a pinned sha256

**Date:** 2026-11-21 · **Status:** accepted · **Phase:** v0.3.1 follow-up · Amends [239](CUPEL_DECISION_239.md) (where the pinned Leiningen comes from)

## Context

ADR-239 pinned Leiningen 2.12.0. `make dev-setup` and CI's tools action downloaded the launcher and the standalone jar from Codeberg, and checked the jar against the sha256 in the launcher's `LEIN_CHECKSUM`.

On 2026-11-18, from about 06:30 UTC, codeberg.org dropped out of DNS (NXDOMAIN; reports said the registrar had put the domain on clientHold) for about 14 hours. CI on the v0.3.1 release commit failed in three jobs with `curl: (6) Could not resolve host: codeberg.org`. `make dev-setup` on a fresh machine would have failed the same way. The release waited until Codeberg was back.

The owner's ruling (2026-11-21): "We should just pin a version that we cache locally and update periodically instead of non-deterministic 'cache and try remote' behavior". The copy lives as an asset on a cupel GitHub release.

## Options

1. **Where the copy lives.**
   - (a) A GitHub Release on tysont/cupel, `tools-lein-<version>`, holding the launcher and the jar. The owner's choice.
   - (b) The files committed to the repository. About 14 MB of jar in git history at each update.
   - (c) A CI cache entry seeded once. GitHub evicts an entry unused for 7 days, and dev-setup would still need Codeberg.
2. **What is checked.**
   - (a) Each file's sha256, pinned in the source of both installers.
   - (b) The jar against the launcher's `LEIN_CHECKSUM`, as before. That trusts whatever launcher is downloaded.
3. **When the copy is unreachable.**
   - (a) The install fails. Nothing else is tried.
   - (b) Fall back to Codeberg. The owner ruled this out: one source, deterministic.

## Decision

1. **(a).** `scripts/mirror-lein.sh VERSION` (`make mirror-lein V=VERSION`):
   - downloads `bin/lein` at the tag and the standalone jar from Codeberg;
   - refuses unless the jar's sha256 is the launcher's `LEIN_CHECKSUM`;
   - creates the GitHub Release `tools-lein-VERSION` with both files unchanged, as a prerelease that is never the latest, so `install.sh`'s `releases/latest` still resolves to Cupel's own release;
   - says in the release notes that it is a copy, under the Eclipse Public License 1.0, with the source's address;
   - prints both files' sha256.

   It refuses a version whose release already exists. `tools-lein-2.12.0` was made with it on 2026-11-21. Its two files are byte-identical to the Codeberg download installed on 2026-11-17.
2. **(a).** `scripts/dev-setup.sh` pins `lein_launcher_sha256` and `lein_jar_sha256` beside `lein_version`. The tools action pins the same two values. Each file must match before it is installed.
3. **(a).** Neither installer names Codeberg. Updating Leiningen means running `make mirror-lein V=<new>`, then changing the version and both sums in `scripts/dev-setup.sh` and the tools action.

## Consequences

- A Codeberg outage no longer affects CI or `make dev-setup`. Both now depend only on GitHub, which hosts the repository and CI anyway.
- GitHub's release list shows `tools-lein-*` prereleases beside Cupel's own. `make release` and the install routes name `v`-prefixed tags or `latest`, so neither reaches them.
- Tests: `cupel-cli/tests/dev_setup.rs` `leiningen_installs_only_from_cupel_s_copy_with_both_files_pinned`, observed failing first: both installers download from the copy only, neither names codeberg.org, and both pin the same 64-hex sums.
- Verified for real: with the installed Leiningen moved aside, `make dev-setup` installed it from `tools-lein-2.12.0`. It ran `Leiningen 2.12.0 on Java 21.0.12.1`, and the result was identical to the earlier install.
