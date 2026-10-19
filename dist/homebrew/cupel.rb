# ABOUTME: Template of the Homebrew formula for the tysont/homebrew-cupel tap (ADR-125): render.sh fills in the
# ABOUTME: release version and each macOS and Linux archive's SHA-256 from the release's SHA256SUMS.
class Cupel < Formula
  desc "Verification suite that grades what a coding agent produced"
  homepage "https://github.com/tysont/cupel"
  version "@VERSION@"
  license "BSD-3-Clause"

  on_macos do
    on_arm do
      url "https://github.com/tysont/cupel/releases/download/v@VERSION@/cupel-@VERSION@-aarch64-apple-darwin.tar.gz"
      sha256 "@SHA256:aarch64-apple-darwin@"
    end
    on_intel do
      url "https://github.com/tysont/cupel/releases/download/v@VERSION@/cupel-@VERSION@-x86_64-apple-darwin.tar.gz"
      sha256 "@SHA256:x86_64-apple-darwin@"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/tysont/cupel/releases/download/v@VERSION@/cupel-@VERSION@-aarch64-unknown-linux-musl.tar.gz"
      sha256 "@SHA256:aarch64-unknown-linux-musl@"
    end
    on_intel do
      url "https://github.com/tysont/cupel/releases/download/v@VERSION@/cupel-@VERSION@-x86_64-unknown-linux-musl.tar.gz"
      sha256 "@SHA256:x86_64-unknown-linux-musl@"
    end
  end

  def install
    bin.install "cupel"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/cupel --version")
  end
end
