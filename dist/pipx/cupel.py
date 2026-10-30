# ABOUTME: The `cupel` console script of the Python package (ADR-125): downloads this platform's release binary on
# ABOUTME: first run, checks it against the release's SHA256SUMS, caches it, and runs it with the caller's arguments.
"""Verification suite that grades what a coding agent produced. Runs the cupel release binary for this platform."""

from __future__ import annotations

import hashlib
import io
import os
import platform
import subprocess
import sys
import tarfile
import tempfile
import urllib.error
import urllib.request
import zipfile

# The release this package runs.
__version__ = "0.2.1"

RELEASES = "https://github.com/tysont/cupel/releases"

# Each release target's triple and archive format, by `<os>-<arch>` in Rust's spelling, as
# dist/targets.toml lists them.
TARGETS: dict[str, list[str]] = {
    "linux-x86_64": ["x86_64-unknown-linux-musl", "tar.gz"],
    "linux-aarch64": ["aarch64-unknown-linux-musl", "tar.gz"],
    "macos-x86_64": ["x86_64-apple-darwin", "tar.gz"],
    "macos-aarch64": ["aarch64-apple-darwin", "tar.gz"],
    "windows-x86_64": ["x86_64-pc-windows-msvc", "zip"],
}

OS = {"Linux": "linux", "Darwin": "macos", "Windows": "windows"}
ARCH = {"x86_64": "x86_64", "AMD64": "x86_64", "aarch64": "aarch64", "arm64": "aarch64", "ARM64": "aarch64"}


class InstallError(Exception):
    """The release binary could not be downloaded or checked."""


def target() -> list[str]:
    system, machine = platform.system(), platform.machine()
    found = TARGETS.get(f"{OS.get(system)}-{ARCH.get(machine)}")
    if found is None:
        raise InstallError(f"no release for {system} {machine}; see {RELEASES}")
    return found


def cache_dir() -> str:
    """Where downloaded binaries are kept: CUPEL_CACHE_DIR, else the user's cache directory."""
    if os.environ.get("CUPEL_CACHE_DIR"):
        return os.environ["CUPEL_CACHE_DIR"]
    if os.name == "nt":
        return os.path.join(os.environ.get("LOCALAPPDATA", os.path.expanduser("~")), "cupel")
    return os.path.join(os.environ.get("XDG_CACHE_HOME", os.path.expanduser("~/.cache")), "cupel")


def download(url: str) -> bytes:
    try:
        with urllib.request.urlopen(url) as response:
            body: bytes = response.read()
            return body
    except urllib.error.HTTPError as error:
        raise InstallError(f"could not download {url}: HTTP {error.code}") from error
    except urllib.error.URLError as error:
        raise InstallError(f"could not download {url}: {error.reason}") from error


def listed_digest(sums: str, archive: str) -> str:
    """The digest SHA256SUMS gives `archive`, in hex."""
    for line in sums.splitlines():
        fields = line.split()
        if len(fields) == 2 and fields[1].lstrip("*") == archive:
            return fields[0]
    raise InstallError(f"SHA256SUMS lists no {archive}")


def extract(data: bytes, archive_format: str, into: str) -> None:
    if archive_format == "zip":
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            archive.extractall(into)
        return
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        if hasattr(tarfile, "data_filter"):
            archive.extractall(into, filter="data")
        else:
            archive.extractall(into)


def install(path: str, triple: str, archive_format: str) -> None:
    """Downloads the release archive for `triple`, checks it, and moves its binary to `path`."""
    base = f"{os.environ.get('CUPEL_DOWNLOAD_URL') or RELEASES}/download/v{__version__}"
    archive = f"cupel-{__version__}-{triple}.{archive_format}"
    listed = listed_digest(download(f"{base}/SHA256SUMS").decode("utf-8"), archive)
    print(f"cupel: downloading {base}/{archive}", file=sys.stderr, flush=True)
    data = download(f"{base}/{archive}")
    actual = hashlib.sha256(data).hexdigest()
    if actual != listed:
        raise InstallError(f"checksum mismatch for {archive}: SHA256SUMS has {listed}, the download is {actual}")
    with tempfile.TemporaryDirectory(prefix=".download-", dir=os.path.dirname(path)) as staging:
        extract(data, archive_format, staging)
        os.replace(os.path.join(staging, f"cupel-{__version__}-{triple}", os.path.basename(path)), path)


def binary() -> str:
    """The cached release binary, downloaded first if it is not there."""
    triple, archive_format = target()
    name = "cupel.exe" if os.name == "nt" else "cupel"
    path = os.path.join(cache_dir(), __version__, triple, name)
    if not os.path.exists(path):
        os.makedirs(os.path.dirname(path), exist_ok=True)
        install(path, triple, archive_format)
    return path


def main() -> None:
    try:
        path = binary()
    except (InstallError, OSError) as error:
        # Exit 3, Cupel's environment code: the binary could not run, so nothing was graded.
        print(f"cupel: {error}", file=sys.stderr)
        sys.exit(3)
    if os.name == "nt":
        sys.exit(subprocess.call([path, *sys.argv[1:]]))
    os.execv(path, [path, *sys.argv[1:]])


if __name__ == "__main__":
    main()
