# Homebrew formula template.
#
# `@VERSION@` and the `@SHA256_*@` placeholders are substituted by
# .github/workflows/homebrew.yml from the published release assets, and the
# result is pushed to noirbizarre/homebrew-tap as Formula/git-sync.rb.
#
# The formula is named after the binary (`git-sync`), not the crate, because
# that is what `brew install noirbizarre/tap/git-sync` has to spell.
class GitSync < Formula
  desc "Easily synchronize your local branches and worktrees"
  homepage "https://github.com/noirbizarre/git-synchronizer"
  version "@VERSION@"
  license "MIT"

  # Prebuilt binaries from the GitHub release rather than a source build: the
  # archives already carry the man pages and completions, and installing takes
  # no Rust toolchain.
  on_macos do
    on_arm do
      url "https://github.com/noirbizarre/git-synchronizer/releases/download/v#{version}/git-sync-aarch64-apple-darwin.tar.gz"
      sha256 "@SHA256_DARWIN_ARM64@"
    end
    on_intel do
      url "https://github.com/noirbizarre/git-synchronizer/releases/download/v#{version}/git-sync-x86_64-apple-darwin.tar.gz"
      sha256 "@SHA256_DARWIN_X86_64@"
    end
  end

  # musl rather than gnu: the binaries are statically linked, so they run on
  # any distribution Homebrew supports regardless of its glibc.
  on_linux do
    on_arm do
      url "https://github.com/noirbizarre/git-synchronizer/releases/download/v#{version}/git-sync-aarch64-unknown-linux-musl.tar.gz"
      sha256 "@SHA256_LINUX_ARM64@"
    end
    on_intel do
      url "https://github.com/noirbizarre/git-synchronizer/releases/download/v#{version}/git-sync-x86_64-unknown-linux-musl.tar.gz"
      sha256 "@SHA256_LINUX_X86_64@"
    end
  end

  depends_on "git"

  def install
    bin.install "git-sync"
    # Git rewrites `git sync --help` into `git help sync`, which runs
    # `man git-sync`: the pages are what makes that work.
    man1.install Dir["man/*.1"]
    bash_completion.install "completions/git-sync.bash" => "git-sync"
    zsh_completion.install "completions/_git-sync"
    fish_completion.install "completions/git-sync.fish"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/git-sync --version")
    assert_match "worktree", shell_output("#{bin}/git-sync --help")
  end
end
