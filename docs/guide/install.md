<!-- ABOUTME: Every way to install Cupel: release archives, source, and the generated table of install routes. -->
<!-- ABOUTME: Part of Cupel's guide, linked from the README; `make readme` rewrites its generated sections and crates/cupel-cli/tests/readme.rs checks them. -->
# Install

## From a release archive

Each [GitHub Release](https://github.com/tysont/cupel/releases) carries one archive per platform, `cupel-<version>-<target>.tar.gz`, for `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` (static binaries), `aarch64-apple-darwin`, and `x86_64-apple-darwin`, `cupel-<version>-x86_64-pc-windows-msvc.zip` for Windows, and a `SHA256SUMS` file. Download your platform's archive and `SHA256SUMS`, verify the checksum, and put `cupel` on your PATH:

```sh
sha256sum --ignore-missing -c SHA256SUMS     # on macOS: shasum -a 256 --ignore-missing -c SHA256SUMS
tar -xzf cupel-0.2.9-x86_64-unknown-linux-musl.tar.gz
install cupel-0.2.9-x86_64-unknown-linux-musl/cupel ~/.local/bin/
```

On Windows, extract the `.zip` and put `cupel.exe` on your PATH; `cupel serve` listens on a named pipe there ([ADR-137](../adr/CUPEL_DECISION_137.md)). Commands Cupel runs through `sh -c` (configured coverage and oracle commands, `--install`) need `sh` on PATH, as Git for Windows provides, and a timeout stops the command but not the processes it started. WSL runs the Linux archive without these limits.

## From source

Build from source with a Rust toolchain. The pinned version is in `rust-toolchain.toml`.

```sh
git clone https://github.com/tysont/cupel
cd cupel
make install            # cargo install --path crates/cupel-cli
```

The binary has no runtime dependencies except `git`, which Cupel calls for `--changed` and `--base` scopes.

Every other route installs a release binary from GitHub Releases, checked against the release's `SHA256SUMS`; `<target>` is one of the targets above. The installers are in `dist/` and are tested in CI against a fake release. The table is the binary's own: `cupel describe` lists the same routes as `install_routes`, and `doctor`'s `doctor.version-unsatisfied` finding names the ones available now.

<!-- generated:install-routes -->
| Route | Command | Platforms | Available |
| --- | --- | --- | --- |
| Install script | `curl -fsSL https://raw.githubusercontent.com/tysont/cupel/main/dist/install.sh \| sh` | Linux, macOS | now |
| cargo-binstall | `cargo binstall --git https://github.com/tysont/cupel cupel-cli` | Linux, macOS | now |
| Release archive | `gh release download --repo tysont/cupel --pattern SHA256SUMS --pattern 'cupel-*-<target>.tar.gz'` | Linux, macOS | now |
| From source | `cargo install --locked --git https://github.com/tysont/cupel cupel-cli` | Linux, macOS | now |
| Homebrew | `brew install tysont/cupel/cupel` | Linux, macOS | once the release workflow pushes the formula to the `tysont/homebrew-cupel` tap |
| npm | `npx cupel-cli` | Linux, macOS | once the `cupel-cli` package is published to npm |
| pipx | `pipx install cupel-cli` | Linux, macOS | once the `cupel-cli` package is published to PyPI |
| crates.io | `cargo install --locked cupel-cli` | Linux, macOS | once the library crates `cupel-cli` depends on are published to crates.io; `cupel-core` there is another project's |
| cupel.dev | `curl -fsSL https://cupel.dev/install \| sh` | Linux, macOS | once cupel.dev serves the install script |
<!-- end:install-routes -->

- **Install script** (POSIX `sh`): installs Linux x86_64/aarch64 and macOS arm64/x86_64 binaries into `~/.local/bin`, or `CUPEL_INSTALL_DIR`, and prints the line to add to your `PATH` when that directory is not on it. `CUPEL_VERSION=0.1.0` installs that release instead of the latest. It needs `curl` or `wget`, `tar`, and `sha256sum` or `shasum`.
- **npm and pipx**: the `cupel-cli` package's `cupel` command downloads the binary of the package's own version on first run into `CUPEL_CACHE_DIR`, else `~/.cache/cupel` (`%LOCALAPPDATA%\cupel` on Windows), then runs it with your arguments, stdio, and exit code. When the binary cannot be downloaded or its checksum differs, it prints why and exits 3. npm needs Node 18 or newer; pipx, Python 3.9 or newer.
- **Homebrew**: the formula, rendered per release by `dist/homebrew/render.sh` from `SHA256SUMS`, covers macOS and Linux.
