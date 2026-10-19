// ABOUTME: The ways to install `cupel` and which of them work today (ADR-128): `describe` lists them, `doctor`'s
// ABOUTME: version finding names the live ones, and the README's install table is generated from them.
use cupel_core::{InstallRoute, Stability};

/// One way to install `cupel`.
struct Route {
    name: &'static str,
    command: &'static str,
    /// Operating systems, as Rust spells them.
    platforms: &'static [&'static str],
    /// What the route waits for before it works; `None` when it works today.
    pending: Option<&'static str>,
}

const UNIX: &[&str] = &["linux", "macos"];

/// Every route, the live ones first. A route goes live in the release that changes its `pending`
/// to `None`, after whatever it waited for has happened.
const ROUTES: &[Route] = &[
    Route {
        name: "Install script",
        command: "curl -fsSL https://raw.githubusercontent.com/tysont/cupel/main/dist/install.sh | sh",
        platforms: UNIX,
        pending: None,
    },
    Route {
        name: "cargo-binstall",
        command: "cargo binstall --git https://github.com/tysont/cupel cupel-cli",
        platforms: UNIX,
        pending: None,
    },
    Route {
        name: "Release archive",
        command: "gh release download --repo tysont/cupel --pattern SHA256SUMS --pattern 'cupel-*-<target>.tar.gz'",
        platforms: UNIX,
        pending: None,
    },
    Route {
        name: "From source",
        command: "cargo install --locked --git https://github.com/tysont/cupel cupel-cli",
        platforms: UNIX,
        pending: None,
    },
    Route {
        name: "Homebrew",
        command: "brew install tysont/cupel/cupel",
        platforms: UNIX,
        pending: Some(
            "once the release workflow pushes the formula to the `tysont/homebrew-cupel` tap",
        ),
    },
    Route {
        name: "npm",
        command: "npx cupel",
        platforms: UNIX,
        pending: Some("once the `cupel` package is published to npm"),
    },
    Route {
        name: "pipx",
        command: "pipx install cupel",
        platforms: UNIX,
        pending: Some("once Cupel's package is on PyPI; the `cupel` there is another project's"),
    },
    Route {
        name: "crates.io",
        command: "cargo install --locked cupel-cli",
        platforms: UNIX,
        pending: Some("once the crates are published to crates.io"),
    },
    Route {
        name: "cupel.dev",
        command: "curl -fsSL https://cupel.dev/install | sh",
        platforms: UNIX,
        pending: Some("once cupel.dev serves the install script"),
    },
];

/// Every route as `describe` shows it.
pub fn all() -> Vec<InstallRoute> {
    ROUTES
        .iter()
        .map(|route| InstallRoute {
            name: route.name.to_owned(),
            command: route.command.to_owned(),
            platforms: route.platforms.iter().map(|os| (*os).to_owned()).collect(),
            live: route.pending.is_none(),
            pending: route.pending.map(str::to_owned),
            stability: Stability::Experimental,
        })
        .collect()
}

/// The commands of the routes that work today.
pub fn live_commands() -> Vec<&'static str> {
    ROUTES
        .iter()
        .filter(|route| route.pending.is_none())
        .map(|route| route.command)
        .collect()
}
