<!-- ABOUTME: How the owner releases and publishes Cupel from their Mac (ADR-238): the one-time setup of tokens in 1Password, -->
<!-- ABOUTME: the per-release make targets, verifying a release on every route, and how to stop or undo one. -->
# Publishing

Nothing publishes from GitHub Actions ([ADR-238](../adr/CUPEL_DECISION_238.md)). The owner releases and publishes from their Mac through make targets over `scripts/ship.sh`.

- Each target takes `V=X.Y.Z` and does nothing when that version is already released or published, so it is safe to run again.
- `DRY=1` runs every local step and creates, uploads, and pushes nothing.

| Target | Publishes | Credential |
| --- | --- | --- |
| `make release` | the GitHub Release: five archives and `SHA256SUMS` | your `gh` login |
| `make publish-npm` | npm `cupel-cli`, from the tag's `dist/npm` | the owner's browser sign-in, confirmed with their 2FA, each publish |
| `make publish-pypi` | PyPI `cupel-cli`, from the tag's `dist/pipx` | `PYPI_TOKEN`, from 1Password |
| `make publish-homebrew` | `Formula/cupel.rb` on `tysont/homebrew-cupel`'s `main` | your SSH key and git identity |
| `make publish` | the three above, in that order | |
| `make verify-release` | nothing; checks every route | |

crates.io stays unpublished while another project holds `cupel-core` there ([ADR-146](../adr/CUPEL_DECISION_146.md)).

PyPI's token comes only from the environment of the command that runs, through 1Password's `op run`; `release.env.tpl` holds its 1Password reference, never its value:

```sh
PYPI_TOKEN=op://Cupel/prod-cupel-pypi-api-token/credential
```

npm keeps no token. Each `make publish-npm` signs in through the browser, where you confirm with your 2FA (a passkey works), and signs out when it ends. The session's token is held only in a config file in that step's private scratch directory, never `~/.npmrc`, and `npm logout` revokes it however the step ends. Nothing else writes a credential to disk or into the repository.

## One-time setup

Some steps need the owner's own registry or 1Password login, which no agent can stand in for; they are marked **owner**. The agent runs the rest.

| Step | Who |
| --- | --- |
| 1. Tools | agent: `make dev-setup`; owner: sign in to `gh`, `op`, and Docker |
| 2. npm: 2FA | **owner** (npm login) |
| 3. PyPI: 2FA, the token | **owner** (PyPI login) |
| 4. PyPI's token in 1Password | **owner** |
| 5. Homebrew: nothing to set up | — |

### 1. Tools

- **Build tools:** `make dev-setup` installs zig 0.16.0 and cargo-zigbuild 0.23.4. It also adds the Rust targets: `x86_64-apple-darwin` (or `aarch64-apple-darwin` on an Intel Mac), both `*-unknown-linux-musl`, and `x86_64-pc-windows-gnu`.
- **Sign-ins:**
  - `gh auth login`, an account that can create releases on `tysont/cupel`.
  - The 1Password CLI, `op`, signed in. With the desktop app, turn on Settings → Developer → "Integrate with 1Password CLI".
- **For `make verify-release`:** Docker, pipx, Homebrew, and Node with npm.
- **SSH:** `ssh -T git@github.com` must answer as an account that can push to `tysont/homebrew-cupel`.

### 2. npm (owner)

1. Use the npm account that owns `cupel-cli`, `sinuous-dev`.
2. Turn on 2FA for it: avatar → **Account** → **Two-Factor Authentication**, **Authorization and writes**. A security key or passkey is enough, because `make publish-npm` confirms in the browser.

No token is created for npm.

### 3. PyPI (owner)

1. Turn on 2FA at https://pypi.org/manage/account/two-factor/.
2. If a pending trusted publisher for `cupel-cli` was added at https://pypi.org/manage/account/publishing/, remove it; nothing uses it.
3. For the first upload, which creates the project, create an API token at https://pypi.org/manage/account/token/ with **Scope: Entire account (all projects)**, named `cupel-cli first upload`.
4. After the first upload:
   1. Create a token with **Scope: Project: cupel-cli**, named `cupel-cli upload`.
   2. Replace the first token with it in 1Password.
   3. Delete the account-wide token on PyPI.

### 4. PyPI's token in 1Password (owner)

Make an **API Credential** item `prod-cupel-pypi-api-token` in the vault `Cupel`, with the PyPI API token (`pypi-…`) in its **credential** field. Keeping it under another name or in another vault means changing `release.env.tpl`'s reference to match. `op read op://Cupel/prod-cupel-pypi-api-token/credential >/dev/null && echo ok` checks the reference resolves without printing it.

### 5. Homebrew

Nothing to set up. `make publish-homebrew` clones the tap over SSH with your key, commits as your git identity, and pushes. The tap's `main` is created by the first publish when the tap is empty.

## Each release

1. Set the version, gate, push `main`, and tag:
   ```sh
   make version V=X.Y.Z          # then commit, gate, and push main
   git tag -a vX.Y.Z -m "Cupel X.Y.Z" && git push origin vX.Y.Z
   ```
2. From a clean checkout of the tag, build the archives and create the GitHub Release:
   ```sh
   git switch --detach vX.Y.Z
   make release V=X.Y.Z DRY=1    # builds all five archives and SHA256SUMS under target/release-X.Y.Z; creates nothing
   make release V=X.Y.Z
   ```
3. Publish to the registries, with the tokens from 1Password:
   ```sh
   op run --env-file=release.env.tpl -- make publish V=X.Y.Z DRY=1
   op run --env-file=release.env.tpl -- make publish V=X.Y.Z
   ```
   `make publish-npm` prints `signing in to npm`, opens a browser sign-in, and asks for your 2FA again when it publishes; stay at the browser until it ends with `+ cupel-cli@X.Y.Z`. To rerun one registry alone, run its target: `make publish-npm V=X.Y.Z`, `op run --env-file=release.env.tpl -- make publish-pypi V=X.Y.Z`, or `make publish-homebrew V=X.Y.Z`; only PyPI's needs `op run`.
4. Verify: `make verify-release V=X.Y.Z` (below).

A step that fails can be run again once its cause is fixed: a finished step does nothing the second time.

## Verify

`make verify-release V=X.Y.Z` checks the release as a user receives it, and ends with `verify=ok`. Wherever a check runs `cupel --version`, it must print exactly `cupel X.Y.Z`. It checks:

- **The GitHub Release:** the assets download, `SHA256SUMS` checks every archive, each archive holds exactly the binary, `LICENSE` and `README.md` (the tag's `scripts/release.sh verify`), and both macOS binaries run.
- **`install.sh`:** the tag's copy, on the Mac, in `ubuntu:24.04` x86_64, and in `alpine:3.20` arm64.
- **`cargo binstall`:** `cargo binstall --git https://github.com/tysont/cupel cupel-cli` in `rust:1.92-bookworm`.
- **npm:** `npm view cupel-cli@X.Y.Z version` and `npx -y cupel-cli@X.Y.Z --version`.
- **PyPI:** `https://pypi.org/pypi/cupel-cli/X.Y.Z/json` and `pipx run --spec cupel-cli==X.Y.Z cupel --version`.
- **Homebrew:** the tap's last commit (`cupel X.Y.Z`); `brew fetch --formula tysont/cupel/cupel` on the Mac, which downloads the macOS archive and checks the formula's digest; and `brew install tysont/cupel/cupel` in `homebrew/brew` (Linux) in Docker. Your Mac's Homebrew installs nothing, so the check neither depends on it nor changes it.

## Stop or undo

- **Stop a route:** PyPI, revoke the token at https://pypi.org/manage/account/token/ and delete its 1Password item; `make publish-pypi` then stops before it touches PyPI. npm and the tap keep no standing credential: they publish only while you sign in or push with your own SSH key. If `make publish-npm` reports `npm logout failed`, revoke the session at https://www.npmjs.com/settings/~/tokens.
- **A bad GitHub Release:** `gh release delete vX.Y.Z --repo tysont/cupel --yes` deletes the release and keeps the tag. Fix the cause, then `make release V=X.Y.Z` again.
- **A bad npm version:** `npm deprecate cupel-cli@X.Y.Z "<reason>"`. `npm unpublish cupel-cli@X.Y.Z` works only within 72 hours, and the version number can never be reused.
- **A bad PyPI version:** at https://pypi.org/manage/project/cupel-cli/releases/, open the release → Options → **Yank**.
- **A bad formula:** revert the tap's last commit:
  ```sh
  git clone git@github.com:tysont/homebrew-cupel.git "$TMPDIR/tap" && cd "$TMPDIR/tap"
  git revert --no-edit HEAD && git push origin main
  ```
