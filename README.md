<p align="center">
  <img src="https://raw.githubusercontent.com/noirbizarre/git-synchronizer/main/docs/images/logo.svg" alt="git-synchronizer" width="520">
</p>

<p align="center"><strong>Easily synchronize your local branches and worktrees</strong></p>

<p align="center">
  <a href="https://github.com/noirbizarre/git-synchronizer/actions/workflows/ci.yml">
    <img src="https://github.com/noirbizarre/git-synchronizer/actions/workflows/ci.yml/badge.svg" alt="CI">
  </a>
  <a href="https://codecov.io/gh/noirbizarre/git-synchronizer">
    <img src="https://codecov.io/gh/noirbizarre/git-synchronizer/graph/badge.svg" alt="Codecov">
  </a>
  <a href="https://crates.io/crates/git-synchronizer">
    <img src="https://img.shields.io/crates/v/git-synchronizer" alt="crates.io">
  </a>
  <img src="https://img.shields.io/github/v/release/noirbizarre/git-synchronizer" alt="Release">
  <img src="https://img.shields.io/github/license/noirbizarre/git-synchronizer" alt="License">
</p>

---

# git-synchronizer (git-sync)

`git-synchronizer` provides the `git sync` command: it detects branches merged
into your main branch(es) and offers to delete them -- both locally and on
configured remotes. It also handles orphaned worktree cleanup.

## Features

- Delete local and remote branches that have been merged
- Worktree cleanup: unified prompt for branches with worktrees and orphaned worktrees
- Respects locked worktrees: skips removal with an informational message
- Min-age guard (`--min-age`): never removes a worktree created too recently
- Glob pattern support for protected branches (e.g. `release/*`)
- Per-branch protection via git config (`branch.<name>.sync-protected`)
- Ignore branch patterns entirely (`sync.ignore`) -- never fetched, never analysed
- Multiple merge detection strategies (fast merge, rebase-aware via `git cherry`, tree SHA comparison, empty three-dot diff, patch-ID matching, simulated merge, squash-merge detection, and deleted-upstream detection)
- Tunable detection thoroughness with `--effort <1-3>` (speed vs accuracy)
- Parallel analysis (`--jobs`, defaults to the CPU count) with byte-identical results at any job count
- Automatic fast-forward of target branches before detection (with `--no-pull` to skip)
- Optional [worktrunk](https://worktrunk.dev) integration for worktree removal (triggers pre/post-remove hooks)
- Interactive setup wizard on first run
- JSON output (`--json`) for scripting and integration
- Configuration stored in git config (`[sync]` section)
- Safety-first: `--force-with-lease` for remote deletions

## Installation

Homebrew:

```sh
brew install noirbizarre/tap/git-sync
```

Arch Linux, from the AUR — prebuilt (`git-synchronizer-bin`), built from the
release source (`git-synchronizer`) or from `main` (`git-synchronizer-git`):

```sh
paru -S git-synchronizer-bin
```

From crates.io:

```sh
cargo install git-synchronizer
```

Or grab a prebuilt binary for your platform from the
[latest release](https://github.com/noirbizarre/git-synchronizer/releases/latest).

From a checkout:

```sh
cargo install --path .
```

The crate is named `git-synchronizer` but installs a binary called `git-sync`,
making it available as the `git sync` subcommand.

### Man pages and completions

Git rewrites `git sync --help` into `git help sync`, which runs `man git-sync`.
That only works once the man page is installed — otherwise git reports
*"No manual entry for git-sync"*. The Homebrew and AUR packages install the
pages and the shell completions for you; `cargo install` places the binary
alone, so with it they have to be installed separately.

From a checkout, `mise` does both, plus zsh completions:

```sh
mise run setup
```

Prebuilt release archives ship them under `man/` and `completions/`. Install by
hand with:

```sh
cp man/*.1 ~/.local/share/man/man1/
```

Make sure that directory is on your `MANPATH` (most distributions add
`~/.local/share/man` automatically). Then:

```sh
git sync --help   # full manual
man git-sync-config-set
```

Without a man page installed, `git sync -h` still prints the short help, and
`git-sync --help` (with a dash, bypassing git's dispatch) prints the long one.

Completions are generated for bash, zsh, fish, elvish and PowerShell; drop the
relevant file into your shell's completion directory.

## Usage

Run `git sync` with no arguments: it works interactively, showing what it found
and prompting before every destructive step.

```sh
# Interactive mode (prompts for confirmation at each step)
git sync

# Show what would be done, change nothing
git sync --dry-run

# Unattended: auto-confirm everything
git sync --yes

# Also delete branches whose upstream branch was deleted
git sync -y --delete-gone

# Also force-remove worktrees with uncommitted changes or unmerged commits
git sync -y --force

# Machine-readable output (implies --yes)
git sync --json
```

The rest of the flags tune scope (`--local-only`, `--no-worktrees`, …), merge
detection (`--effort`, `--jobs`) and worktree safety (`--min-age`). Rather than
repeat them
here, where they would drift, the complete reference is generated from the same
definition as the binary: run `git sync -h` for the summary, or `man git-sync`
for the full manual — see
[Man pages and completions](#man-pages-and-completions) above.

### JSON output

`--json` prints a single JSON document describing everything that was detected
and done. Human-readable logs are suppressed; stdout carries the document alone,
so it is safe to pipe. The document is pretty-printed on a terminal and compact
when piped or redirected.

```sh
git sync --json | jq '.summary'
git sync --json --dry-run | jq '.local.branches[] | select(.reason == "gone")'
git sync config list --json | jq '.protected'
```

Because prompts would hang a non-interactive caller, `--json` implies `--yes`:
deleted-upstream branches stay opt-in behind `--delete-gone`, forced removal of
dirty worktrees stays opt-in behind `--force`, and a repository
that has never been configured is an error rather than a setup wizard (run
`git sync` once interactively first).

| Field | Description |
| --- | --- |
| `version` | Schema version, currently `1` |
| `status` | `success` or `error` |
| `dry_run` | Whether `--dry-run` was in effect |
| `effort` | The effective merge-detection level (`1`-`3`) |
| `min_age` | The effective minimum worktree age, e.g. `"0s"` or `"2h"` |
| `jobs` | The effective number of concurrent git probes used during analysis |
| `fetch` | Phase 1: per-remote fetch/prune outcome |
| `pull` | Phase 2: per-branch fast-forward outcome |
| `local` | Phase 3: `merged`/`gone` candidates, plus per-branch and per-worktree outcomes |
| `remotes` | Phase 4: merged branches and deletion outcome per remote |
| `warnings` | Non-fatal messages surfaced during the run |
| `errors` | Failed operations, each with `action`, `target`, `kind` (`network`, `auth`, `other`) and `message` |
| `summary` | `local_branches_deleted`, `remote_branches_deleted`, `worktrees_removed`, `errors` |

Item statuses are `updated`, `deleted`, `removed`, `skipped`, `locked`,
`too_young`, `failed` or `dry_run`. A fatal error still yields a document (with
`status: "error"`) and a non-zero exit code.

### Configuration management

`git sync config` reads and writes the `[sync]` settings described in
[Configuration](#configuration) below:

```sh
# Display current configuration
git sync config list

# Re-run the interactive setup wizard
git sync config setup

# Set a configuration value directly
git sync config set worktrunk false

# Add a protected branch pattern
git sync config add-protected 'release/*'

# Ignore a single branch
git sync config ignore experiment
```

Each of these has a counterpart: `add-`/`remove-` pairs for the `protected`,
`ignore` and `remote` patterns, and `protect`/`unprotect`, `ignore`/`unignore`
for individual branches. `git sync config -h` lists them all, and each has its
own man page (`man git-sync-config-set`).

## Configuration

Configuration is read from the `[sync]` section of your git config at any
scope (local, global or system). Every `git sync config` subcommand **writes**
to the repository-local `.git/config`:

```ini
[sync]
    protected = main
    protected = master
    protected = release/*
    ignore = wip/*
    remote = origin
    worktrunk = true
    effort = 3
    minage = 2h
    jobs = 8
```

| Key | Type | Description |
|-----|------|-------------|
| `protected` | multi-value | Glob patterns for branches that should never be deleted |
| `ignore` | multi-value | Glob patterns for branches git-sync ignores entirely |
| `remote` | multi-value | Remotes to delete branches from (omit for all remotes) |
| `worktrunk` | bool | Enable/disable [worktrunk](https://worktrunk.dev) for worktree removal. When omitted, auto-detects (see below) |
| `effort` | `1`-`3` | How thorough merge detection should be. Defaults to `2`; `--effort` overrides it |
| `minage` | duration | Minimum age a worktree must have before it may be removed, e.g. `30s`, `2h`, `7d`. Defaults to `0s` (no guard); `--min-age` overrides it |
| `jobs` | integer >= 1 | How many read-only git probes analysis may run at once. Defaults to the CPU count; `--jobs` overrides it, `--verbose` forces `1` |

When `worktrunk` is unset, git-sync enables it only if the repository has a
`[worktrunk]` config section **and** `wt` is on `$PATH`; it then asks once per
run before using it, and enables it without asking under `--yes`. If either
condition is missing, worktrunk is not used.

Individual branches can also be protected via the standard `[branch]`
config namespace:

```ini
[branch "develop"]
    sync-protected = true
```

A per-branch protected branch is excluded from deletion candidates and also
serves as a merge target (branches merged into it are flagged for cleanup).

### Ignored branches

Ignored branches are invisible to git-sync: they are never fetched, never
become merge targets, never appear as deletion candidates, and their worktrees
are left alone. Use them for branches git-sync has no business touching, such
as long-lived spikes or vendor branches.

Patterns go in `sync.ignore`, and individual branches can carry the flag
directly:

```ini
[branch "experiment"]
    sync-ignored = true
```

**Ignoring takes precedence over protection.** A branch matching both
`sync.protected` and `sync.ignore` is ignored, which also means it is *not*
used as a merge target.

Exclusion at fetch time is implemented with negative refspecs
(`^refs/heads/wip/*`), which require **git 2.29 or later** and understand a
single `*` wildcard only. Richer glob patterns (`?`, character classes,
alternates) are still fetched, then filtered out by the same matcher used
everywhere else. Note that when at least one ignore pattern is expressible as a
refspec (a single `*` and no other metacharacters), the fetch uses an explicit
refspec and therefore bypasses a custom `remote.<name>.fetch` setting. With only
richer patterns, the default fetch — and your custom refspec — is left intact.

### First run

On first run (when no `[sync]` config section exists), an interactive
setup wizard runs automatically:

1. Auto-detects local branches and pre-selects well-known ones (`main`, `master`, `develop`, `development`)
2. Asks for additional protected patterns (e.g. `release/*`)
3. Asks for branch patterns to ignore entirely (e.g. `wip/*`)
4. Lists available remotes and asks which ones to operate on
5. If [worktrunk](https://worktrunk.dev) (`wt`) is detected on `$PATH`, asks whether to use it for worktree removal

## How it works

The cleanup runs in four sequential phases, each of which can be skipped via
CLI flags:

1. **Fetch & prune remotes** -- runs `git fetch --prune <remote>` for each
   configured remote (every remote when `sync.remote` is unset), pruning
   deleted remote-tracking branches. Skipped with `--no-fetch`.

2. **Pull / fast-forward target branches** -- fast-forwards each protected
   branch to its remote-tracking upstream so that merge detection operates on
   up-to-date refs. The strategy varies depending on the branch state:
   - *Current branch*: `git pull --ff-only` in the working directory.
   - *Checked out in another worktree*: `git pull --ff-only` run from that
     worktree directory (works with both plain git and worktrunk-managed
     worktrees).
   - *Not checked out*: `git fetch <remote> <ref>:<branch>` to update the
     local ref without any checkout.

   Branches without upstream tracking info are silently skipped. If a
   fast-forward fails (e.g. the branch has diverged), a warning is printed
   and the remaining branches are still processed.
   Skipped with `--no-pull`.

3. **Delete merged local branches & clean worktrees** -- identifies branches
   merged into any protected branch (both glob-pattern and per-branch
   protected) using several complementary strategies, applied from cheapest to
   most expensive and stopping as soon as one matches. How many of them run is
   controlled by `--effort` (or `sync.effort`), each level including the
   previous ones. The same levels drive remote-branch detection in step 4:

   **Effort 1 (fastest)**

   - *Standard detection*: `git branch --merged <target>` catches fast-forward
     and regular merges.

   **Effort 2 (default)**

   - *Rebase-aware detection*: `git cherry <target> <branch>` catches
     rebased branches by checking whether every commit has already been
     applied upstream.
   - *Tree SHA comparison*: compares `git rev-parse <ref>^{tree}` between
     the target and branch -- the cheapest content-equality check.
   - *Empty three-dot diff*: `git diff --quiet <target>...<branch>` catches
     branches whose own commits net out to no content change relative to their
     fork point (a commit and its revert, a pure history rewrite, a branch
     created but never meaningfully advanced).

   **Effort 3 (most thorough, noticeably slower)**

   - *Patch-ID matching*: compares `git patch-id --stable` fingerprints of the
     branch's commits against those recently applied on the target, catching
     branches re-applied under different SHAs (rebase + reword, partial
     cherry-pick, history rewrite).
   - *Simulated merge*: `git merge-tree --write-tree <target> <branch>` --
     if merging the branch would produce exactly the target's current tree,
     the branch adds nothing. Handles squash merges even after the target has
     advanced with unrelated changes.
   - *Squash-merge detection*: compares the patch-ID of the branch's combined
     diff against the target's recent commits, catching multi-commit branches
     collapsed into a single squash commit.

   Deleted-upstream detection (below) is independent of the effort level and
   always runs.

   Every one of these probes is a separate `git` process, and they are
   independent of one another, so git-sync runs up to `--jobs` of them at a
   time (defaulting to the CPU count). This is a wall-clock optimisation only:
   each detection pass collects its verdicts before acting on any of them, so
   the candidate list, the warnings and their order are identical at any job
   count. `--jobs 1` restores strictly serial execution, and `--verbose`
   forces it so the echoed commands stay in the order they ran.

   Only read-only inspection is parallelised. Fetching, pulling, deleting
   branches and removing worktrees always run one at a time: concurrent
   writers to the same repository corrupt its state.

   Per-branch protected branches also serve as merge targets, so branches
   merged into them are detected as candidates too.

   In addition, branches whose **upstream tracking branch no longer exists**
   are reported as a separate category. This is the footprint left by a merged
   pull request whose remote branch was deleted, and it is often the only
   remaining signal for branches squash-merged into a target that has since
   advanced far enough for the content-based strategies above to lose the
   trail. Because a deleted upstream does *not* prove the branch was merged
   (someone may simply have deleted an unmerged remote branch), these entries
   are **listed unchecked** in the multiselect and are never auto-selected by
   `--yes` unless you also pass `--delete-gone`. Detection requires up-to-date
   remote-tracking refs, so it only runs after a successful `fetch --prune`,
   or in `--dry-run` where a warning notes the results may be stale.

   All cleanup items are presented in a **single unified multiselect**:
   merged branches (with their worktree path shown when applicable), branches
   with a deleted upstream, and orphan worktrees (worktrees whose branch no
   longer exists locally). Merged branches default to selected;
   deleted-upstream branches and orphan worktrees default to unselected.

   Two cases are handled outside that multiselect. Worktrees that are dirty
   (uncommitted or untracked changes) or hold unmerged commits are collected
   into a **second multiselect** for forced removal, defaulting to unselected;
   anything left unselected there is skipped entirely, with neither the
   worktree removed nor the branch deleted. `--force` drives this prompt and
   nothing else: interactively it pre-selects every entry (each can still be
   unchecked), and under `--yes` it force-removes them all without prompting.
   `--yes` on its own — including via `--json` — skips them, so a
   non-interactive run never destroys uncommitted work. Separately, a selected branch whose
   commits are unreachable from any merge target is force-deleted
   automatically, with an informational line rather than a prompt.

   For selected branches that have worktrees, the worktree is removed first,
   then the branch is deleted with `git branch -D` (force-delete is safe here
   because the branch is already verified as merged into a protected target).
   Selected orphan worktrees are also removed in the same pass. When
   [worktrunk](https://worktrunk.dev) is enabled (via `--worktrunk` flag,
   `sync.worktrunk` config, or auto-detection), removal is delegated to
   `wt remove` so that pre/post-remove hooks are triggered. Otherwise falls
   back to `git worktree remove`.

   Locked worktrees (via `git worktree lock`) are automatically skipped with
   an informational message -- this also prevents their branch from being
   deleted, since git refuses to delete a branch checked out in any worktree.
   Worktrees created less than `--min-age` ago (or `sync.minage`) are skipped
   the same way. Age is measured from the creation time of the worktree's
   administrative directory (`.git/worktrees/<id>`), not from its branch tip,
   so a worktree you just created from an up-to-date default branch is
   protected even though that branch counts as merged. The guard is disabled
   by default (`0s`).
   Skipped with `--remote-only`. Worktree cleanup is skipped with
   `--no-worktrees`.

4. **Delete merged remote branches** -- for each configured remote, identifies
   merged remote-tracking branches. The user selects which to delete, and they
   are removed with `git push --delete --force-with-lease` for safety.

   Remote detection runs the **same strategies at the same effort levels** as
   step 3, so squash- and rebase-merged remote branches are reported too. The
   one deliberate difference: the content-based strategies compare against the
   **remote-tracking** counterparts of the protected branches (`origin/main`),
   not their local ones. A branch merged into a local `main` you have not
   pushed yet is still live on the remote and is not offered for deletion
   there. Which branches are protected or ignored is still read from your local
   configuration.
   Skipped with `--local-only`.

```mermaid
flowchart TD
    Start([git sync]) --> LoadConfig[Load configuration]
    LoadConfig --> FirstRun{First run?}
    FirstRun -- Yes --> Setup[Interactive setup wizard]
    Setup --> FetchCheck
    FirstRun -- No --> FetchCheck

    FetchCheck{--no-fetch?}
    FetchCheck -- No --> Fetch[Fetch & prune remotes]
    Fetch --> PullCheck
    FetchCheck -- Yes --> PullCheck

    PullCheck{--no-pull?}
    PullCheck -- No --> Pull[Fast-forward target branches]
    Pull --> PullCurrent[Current branch:\ngit pull --ff-only]
    Pull --> PullWT[In worktree:\ngit pull --ff-only from that worktree]
    Pull --> PullFetch[Not checked out:\ngit fetch remote ref:branch]
    PullCurrent --> LocalCheck
    PullWT --> LocalCheck
    PullFetch --> LocalCheck
    PullCheck -- Yes --> LocalCheck

    LocalCheck{--remote-only?}
    LocalCheck -- No --> FindLocal[Find merged local branches\n+ orphan worktrees]
    FindLocal --> Merged["Standard detection\ngit branch --merged\n(effort 1+)"]
    FindLocal --> Cherry["Rebase-aware detection\ngit cherry\n(effort 2+)"]
    FindLocal --> TreeSHA["Tree SHA comparison\n(effort 2+)"]
    FindLocal --> EmptyDiff["Empty-diff detection\ngit diff --quiet\n(effort 2+)"]
    FindLocal --> PatchID["Patch-ID matching\ngit patch-id\n(effort 3)"]
    FindLocal --> SimMerge["Simulated merge\ngit merge-tree --write-tree\n(effort 3)"]
    FindLocal --> Squash["Squash-merge detection\ncombined patch-id\n(effort 3)"]
    FindLocal --> GoneUpstream[Deleted-upstream detection\nrequires a fetch]
    FindLocal --> Orphans[Find orphan worktrees]
    Merged --> SelectLocal[Unified multiselect:\nbranches + worktrees]
    Cherry --> SelectLocal
    TreeSHA --> SelectLocal
    EmptyDiff --> SelectLocal
    PatchID --> SelectLocal
    SimMerge --> SelectLocal
    Squash --> SelectLocal
    GoneUpstream --> SelectLocal
    Orphans --> SelectLocal
    SelectLocal --> RemoveWT[Remove selected worktrees]
    RemoveWT --> UseWT{Worktrunk\nenabled?}
    UseWT -- Yes --> WTRemove[wt remove]
    UseWT -- No --> GitWTRemove[git worktree remove]
    WTRemove --> DeleteLocal[Delete local branches\ngit branch -D]
    GitWTRemove --> DeleteLocal
    DeleteLocal --> RemoteCheck
    LocalCheck -- Yes --> RemoteCheck

    RemoteCheck{--local-only?}
    RemoteCheck -- No --> FindRemote["For each remote:\nfind merged remote branches\n(same strategies, vs remote/target)"]
    FindRemote --> SelectRemote[User selects branches]
    SelectRemote --> DeleteRemote[Delete remote branches\ngit push --delete --force-with-lease]
    DeleteRemote --> Done
    RemoteCheck -- Yes --> Done

    Done([Done])
```

## Development

This project uses [mise](https://mise.jdx.dev/) for task management. Start by
installing the toolchain and the git hooks:

```sh
mise install            # Install the pinned toolchain and tools
prek install            # Install the pre-commit and commit-msg git hooks
```

The hooks are what enforce formatting, clippy and the commit convention
locally; without them the first feedback comes from CI.

```sh
mise run build          # Build the project
mise run build:release  # Build in release mode
mise run test           # Run tests with cargo-nextest
mise run lint           # Run clippy
mise run lint:actions   # Lint the GitHub Actions workflows
mise run fmt            # Format code
mise run fmt-check      # Check formatting without rewriting files
mise run check          # Run all checks (fmt-check + lint + test)
mise run cover          # Generate lcov coverage report
mise run cover:html     # Generate HTML coverage report
mise run changelog      # Preview the next version and changelog
mise run ship:validate  # Validate the gh-ship release setup
mise run man            # Collect the generated man pages and completions
mise run setup          # Install the binary locally
```

### Commits

[Conventional Commits](https://www.conventionalcommits.org/), enforced by
commitlint via the prek `commit-msg` hook and re-checked by the CI lint job.
The changelog and the next version number are derived from them, so the type
and scope matter.

### Releases

Orchestrated by [gh-ship](https://github.com/noirbizarre/gh-ship), with the
version and the changelog produced by [git-cliff](https://git-cliff.org)
(`cliff.toml`). gh-ship never versions and never writes changelogs; it drives
the lifecycle:

1. push to `main` → `gh ship prepare` opens or updates the **Release PR** on
   the `release/next` branch, carrying the `Cargo.toml` bump and the changelog;
2. review the changelog and merge that PR;
3. `gh ship release` tags the merge commit as `vX.Y.Z`, drafts the release,
   attaches the cross-compiled binaries, publishes the crate to crates.io, and
   only then makes the release public.

Maintainers do not tag by hand. `gh ship validate` runs in CI, so a workflow
that stops satisfying the contract fails on a pull request rather than
mid-release.

## License

MIT
