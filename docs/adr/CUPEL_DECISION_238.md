# CUPEL_DECISION_238: Releases and publishing run on the owner's Mac through make targets; nothing publishes from GitHub Actions

**Date:** 2026-11-17 · **Status:** accepted; §4 (an npm publish needs a terminal) amended by [241](CUPEL_DECISION_241.md) · **Phase:** v0.3.0 follow-up, before batch v0.3.1 (the owner's direction change) · Supersedes [235](CUPEL_DECISION_235.md); amends [124](CUPEL_DECISION_124.md) (the release is built and published locally), [128](CUPEL_DECISION_128.md) §1–§2 (no publish jobs), and [137](CUPEL_DECISION_137.md) (the Windows archive is `x86_64-pc-windows-gnu`)

## Context

ADR-235 published from `.github/workflows/release.yml`. Its build jobs made the five archives on three runners, and its `release` job created the GitHub Release. npm and PyPI trusted the workflow through OIDC. The Homebrew tap was pushed with a deploy key held as a repository secret. The owner turned on each route after registering it.

v0.3.0 was tagged with every gate closed, so the release run published only the GitHub Release. The owner then ruled (2026-11-17, verbatim): "Direction change: nothing publishes from GitHub Actions. All release and publish steps run locally through make targets. ADR superseding ADR-235." The ruling also said:

- Remove the publish jobs and their GitHub-side setup.
- Build all five targets locally, with cargo-zigbuild for linux-musl and windows-gnu and natively for both macOS archs.
- Make each step a make target that is idempotent and has a dry run.
- Take secrets only from the environment at invocation, through `op run --env-file=release.env.tpl`, with nothing written to disk or to the repository.
- Keep the CI check jobs as they are.

Asked what becomes of release.yml's remaining `version`, `build`, and `release` jobs, the owner chose to delete the workflow. Its `release` job would otherwise race `make release` on every tag, and it would still publish from Actions.

## Decision

1. **No workflow publishes.**
   - `.github/workflows/release.yml` is deleted. ci.yml and pages.yml are unchanged.
   - On GitHub, the `npm` and `pypi` environments, the tap's deploy key, the `HOMEBREW_TAP_DEPLOY_KEY` secret, and any `*_TRUSTED_PUBLISHING` variable are removed.
   - The test `no_workflow_publishes` holds this. No workflow may name `gh release`, `npm publish`, `twine`, `gh-action-pypi-publish`, `cargo publish`, `homebrew/publish.sh`, or `homebrew-cupel`.
2. **The steps are make targets over `scripts/ship.sh`.**
   - Each target takes `V=X.Y.Z`.
   - With `DRY=1`, a target runs `ship.sh --dry-run`: every local step runs, and nothing is created, uploaded, or pushed.

   | Target | What it does | When it does nothing |
   | --- | --- | --- |
   | `make release` | Refuses a tree with changes, a missing tag, a tag that is not the checked-out commit, a tag GitHub does not have, a version other than the crate's, or a host other than macOS. Then it builds every target in `dist/targets.toml` with `make dist`: the Linux musl and Windows GNU targets through cargo-zigbuild, and both macOS targets with Apple's toolchain. It checks that the Linux binaries are static and that both macOS binaries report `cupel X.Y.Z`. It writes `SHA256SUMS` and creates the GitHub Release with `gh release create --verify-tag --generate-notes`, or uploads the missing files to a release that lacks some. | The release already holds every archive and `SHA256SUMS`. |
   | `make publish-npm` | Signs in to npm through the browser, where the owner confirms with their 2FA, publishes the tag's `dist/npm` as `cupel-cli`, and signs out (§4). | npm has that version. |
   | `make publish-pypi` | Builds the tag's `dist/pipx` (build 1.6.1, in a throwaway virtual environment), checks it with twine 6.1.0, and uploads it. | PyPI has that version. |
   | `make publish-homebrew` | Renders `Formula/cupel.rb` from the release's `SHA256SUMS` and pushes it through `dist/homebrew/publish.sh` over the SSH key of whoever runs it, committing as their git identity. | The tap's formula is already the rendered one. |
   | `make publish` | `publish-npm`, `publish-pypi`, `publish-homebrew`, in that order. | |
   | `make verify-release` | Checks every route a user installs from, against the tag's own `scripts/release.sh` and `dist/install.sh`. | |

   `make verify-release` checks, in order:
   - the release assets, their checksums, and each archive's layout
   - both macOS binaries
   - `install.sh` on the Mac, in Ubuntu 24.04 x86_64, and in Alpine 3.20 arm64
   - `cargo binstall`
   - npm (`npm view`, `npx`)
   - PyPI (its JSON API, `pipx run`)
   - the tap's last commit; `brew fetch` on the Mac, which downloads the macOS archive and checks the formula's digest; and `brew install` in Homebrew on Linux, in Docker. The Mac's own Homebrew installs nothing, so the check neither depends on it nor changes it. On the owner's Mac, a `brew install` first auto-updated Homebrew and then refused, because the Command Line Tools were out of date.

   Each check that runs `cupel` must print exactly `cupel X.Y.Z`. A package is published from the tag's copy of its files (`git archive vX.Y.Z dist`), never the working tree's, so a later checkout publishes an older release as it was tagged.
3. **Secrets come only from the environment.**
   - PyPI reads `PYPI_TOKEN`. A run without it, other than a dry run, stops before it touches the registry.
   - `release.env.tpl` is committed. It holds a 1Password secret reference to that token and no value, and `op run --env-file=release.env.tpl -- make publish V=X.Y.Z` resolves the reference into that command's environment alone.
   - twine gets `TWINE_PASSWORD` as an environment assignment on its own command line, never as an argument.
   - The tap needs no token: the owner's SSH key pushes it.
4. **npm is signed in to through the browser for each publish.** The owner ruled this on 2026-11-17, choosing among three ways:
   - **Browser sign-in (chosen).** `npm login --auth-type=web`, then `npm publish`, then `npm logout`, with the owner confirming in the browser each time. The owner's npm 2FA is a security key (a passkey in 1Password), with no one-time code.
   - **A granular token plus `--otp`.** Refused: npm answered the token's publish with `EOTP`, and a passkey gives no code to pass.
   - **A token that bypasses 2FA.** npm restricts these from July 2026 and plans to remove direct publishing from them in January 2027.

   The session's token is the one credential this step writes to disk:
   - It goes into a `--userconfig` file in the step's scratch directory (mode 0700), never `~/.npmrc`.
   - The exit trap revokes it with `npm logout` however the step ends, then removes the directory.
   - A publish therefore needs the owner at the browser, and `make publish` is not unattended.
   - The first version, 0.3.0, was published the same way by hand, from a clone of the tag, on 2026-11-17.
5. **PyPI uses an API token.** The first upload creates the project, so it needs a token scoped to the whole account. Once the project exists, the owner replaces that token with one scoped to `cupel-cli`.
6. **crates.io stays unpublished** ([146](CUPEL_DECISION_146.md)). `ship.sh publish crates` is refused with that reason.
7. **The Windows archive is `x86_64-pc-windows-gnu`.**
   - cargo-zigbuild cross-compiles Windows only for the GNU ABI.
   - The binary imports only Windows' own DLLs (the Universal CRT's `api-ms-win-crt-*`, kernel32, ntdll, userenv, ws2_32, bcryptprimitives) and needs no MinGW runtime.
   - `dist/targets.toml`, the cargo-binstall override, and both wrappers' target maps change from `x86_64-pc-windows-msvc` to `x86_64-pc-windows-gnu`.
   - Releases up to v0.3.0 keep their MSVC archive, and their own wrappers name it.

## Consequences

- A release needs the owner's Mac with `make dev-setup`'s tools: zig 0.16.0, cargo-zigbuild 0.23.4, and the Rust targets, which now include `x86_64-pc-windows-gnu`. It also needs `gh` signed in, 1Password's `op`, the owner at the browser for npm, and, for `make verify-release`, Docker, pipx, Node, and Homebrew.
- No CI run builds a release any more, and nothing checks the Linux binaries' `--version` before upload. `make verify-release` runs them in Docker after the release.
- CI's `windows` job still compiles and tests with MSVC, as the ruling kept it, while the release ships the GNU build. Until a Windows check runs the GNU binary, it is verified only as a PE32+ console executable with system imports, not run.
- npm packages published this way carry no provenance attestation. That needs an OIDC token from a CI provider.
- To stop a route: for PyPI, revoke its token on PyPI and delete it in 1Password. npm keeps no standing credential, and neither does the tap.
