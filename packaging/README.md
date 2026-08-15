# Packaging

Templates for the distribution channels that are not driven by `cargo publish`.
Both are rendered and pushed by workflows that run on `release: published` —
after gh-ship has undrafted the release, so the URLs they bake in already
resolve.

| Path | Channel | Workflow |
| --- | --- | --- |
| `aur/git-synchronizer/` | AUR, built from the release source tarball | `.github/workflows/aur.yml` |
| `aur/git-synchronizer-bin/` | AUR, prebuilt binary (x86_64, aarch64) | `.github/workflows/aur.yml` |
| `aur/git-synchronizer-git/` | AUR, tracks `main` | `.github/workflows/aur.yml` |
| `homebrew/git-sync.rb` | `noirbizarre/homebrew-tap` | `.github/workflows/homebrew.yml` |

## The placeholder contract

The templates are not valid as they stand: the workflows substitute
`@VERSION@` and the `@SHA256*@` placeholders from the published release assets.

| Placeholder | Filled from |
| --- | --- |
| `@VERSION@` | the tag with its `v` stripped |
| `@SHA256@` | `git-synchronizer-<version>.tar.gz` |
| `@SHA256_X86_64@`, `@SHA256_AARCH64@` | `git-sync-<arch>-unknown-linux-gnu.tar.gz` |
| `@SHA256_DARWIN_*@`, `@SHA256_LINUX_*@` | `git-sync-<target>.tar.gz` (macOS, Linux musl) |

Checksums are always computed from the downloaded asset itself, never read from
the `.sha256` files published beside it: a mismatch between the two must not be
able to reach users.

`git-synchronizer-git` carries no placeholder — `makepkg` derives its `pkgver`
from the checkout, and its source is a git URL, so there is nothing to pin.

Nothing else in these templates may hardcode a version: adding an asset means
adding both a placeholder and the substitution that fills it, and
`homebrew.yml` fails if any placeholder survives rendering.

## Renaming or removing a release asset

The templates address assets by name, so `publish-release.yml` and these files
change together:

- `git-sync-<target>.tar.gz` comes from `taiki-e/upload-rust-binary-action`
  (`archive: $bin-$target` by default) and carries **no leading directory** —
  `git-sync`, `man/`, `completions/`, `LICENSE` and `README.md` sit at its root.
  `git-synchronizer-bin` and the formula both rely on that layout.
- `git-synchronizer-<version>.tar.gz` is produced by the `source` job, with a
  `git-synchronizer-<version>/` prefix so a PKGBUILD can `cd "$pkgname-$pkgver"`.

## One-off bootstrap

None of this works until the following exist. All of it is manual, once.

### AUR

The AUR creates a repository on the first push, so each pkgbase has to be
imported by hand before the workflow can update it:

```sh
git clone ssh://aur@aur.archlinux.org/git-synchronizer-bin.git
cd git-synchronizer-bin
# render the PKGBUILD as aur.yml does, then:
makepkg --printsrcinfo > .SRCINFO
git add PKGBUILD .SRCINFO && git commit -m "Initial import" && git push
```

Repeat for `git-synchronizer` and `git-synchronizer-git`.

Then create an `aur` environment on the repository holding a single secret,
`AUR_SSH_PRIVATE_KEY`: the private half of an SSH key registered on the AUR
account that maintains the three pkgbases. It is kept out of the `release`
environment on purpose — a key that can push to the AUR has no business sitting
next to the GitHub App credentials.

### Homebrew

Create the public repository `noirbizarre/homebrew-tap` (the `homebrew-`
prefix is what makes `brew install noirbizarre/tap/git-sync` work). An empty
repository is enough; the workflow creates `Formula/` on the first push.

Then create a `homebrew` environment holding `TAP_TOKEN`, a fine-grained token
with `contents: write` on that repository and nothing else.

## Re-running a failed publish

Both workflows are idempotent — they compare the staged index and exit early
when nothing changed — so a failed leg can simply be replayed:

```sh
gh workflow run aur.yml -f tag=vX.Y.Z
gh workflow run homebrew.yml -f tag=vX.Y.Z
```

## Testing a change

`aur.yml` builds every non-VCS package before pushing it, so a broken PKGBUILD
fails the workflow rather than reaching users. To check one locally, substitute
the placeholders against an already published release and run:

```sh
cd packaging/aur/git-synchronizer-bin
makepkg -si --noconfirm
namcap PKGBUILD ./*.pkg.tar.zst
```

For the formula, render it and hand it to brew directly:

```sh
brew install --formula ./packaging/homebrew/git-sync.rb
brew test git-sync
brew audit --strict --online git-sync
```
