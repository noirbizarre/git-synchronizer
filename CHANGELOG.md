# Changelog

All notable changes to this project will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [v0.3.0](https://github.com/noirbizarre/git-synchronizer/compare/v0.2.0..v0.3.0) - 2026-08-15

### 💫 Features

- **artwork** Add the logo, icon and social preview ([#55](https://github.com/noirbizarre/git-synchronizer/issues/55)) - ([1645f1a](https://github.com/noirbizarre/git-synchronizer/commit/1645f1a753fbe028fabe2bcfc38302b0b8352d2e))
- **branches** Apply advanced merge detection to remote branches ([#59](https://github.com/noirbizarre/git-synchronizer/issues/59)) - ([dbeb892](https://github.com/noirbizarre/git-synchronizer/commit/dbeb892cbf8bdde021295a58b99cdb9271940195))
- **branches** Add effort levels for merge detection ([#58](https://github.com/noirbizarre/git-synchronizer/issues/58)) - ([77b40cf](https://github.com/noirbizarre/git-synchronizer/commit/77b40cf4b5dab86798acea495060c32acacf643b))
- **ci** Publish a source tarball and an Intel macOS binary - ([8579497](https://github.com/noirbizarre/git-synchronizer/commit/85794979dab58e3933d5d919519d42c82f388713))
- **cli**  🚨 **breaking** Decouple --force from --yes for worktree force-removal ([#63](https://github.com/noirbizarre/git-synchronizer/issues/63)) - ([19aeb31](https://github.com/noirbizarre/git-synchronizer/commit/19aeb31079f889d1c124955b09b0dce976e072c4))
- **cli** Add --json machine-readable output ([#57](https://github.com/noirbizarre/git-synchronizer/issues/57)) - ([e351e4a](https://github.com/noirbizarre/git-synchronizer/commit/e351e4ae477f6a8df22e5d7d0deb18a738275867))
- **docs** Generate and ship man pages and shell completions ([#60](https://github.com/noirbizarre/git-synchronizer/issues/60)) - ([a95efc4](https://github.com/noirbizarre/git-synchronizer/commit/a95efc46f5546724c6300acaa78e2f92b13ee33a))
- **packaging** Publish a Homebrew formula - ([bae2296](https://github.com/noirbizarre/git-synchronizer/commit/bae2296ee7d9c5d353d980e0d406caf87e15912b))
- **packaging** Publish on the AUR - ([953316b](https://github.com/noirbizarre/git-synchronizer/commit/953316b58d7f8937e64ca35619ccdd53c3725614))
- **worktrees** Add a min-age guard for worktree removal (`--min-age`) ([#62](https://github.com/noirbizarre/git-synchronizer/issues/62)) - ([ffb7c24](https://github.com/noirbizarre/git-synchronizer/commit/ffb7c24973b2784c797dd16d0b8ce1de8984242b))

### 📚 Documentation

- **readme** Defer the CLI reference to --help and man pages ([#64](https://github.com/noirbizarre/git-synchronizer/issues/64)) - ([5cfe1e7](https://github.com/noirbizarre/git-synchronizer/commit/5cfe1e73cbdcd3a4bc894e14010b6312a434bda4))

## [v0.2.0](https://github.com/noirbizarre/git-synchronizer/compare/v0.1.1..v0.2.0) - 2026-08-15

### 💫 Features

- **branches** Ignore branches matching configured patterns ([#53](https://github.com/noirbizarre/git-synchronizer/issues/53)) - ([2bf4fbc](https://github.com/noirbizarre/git-synchronizer/commit/2bf4fbce1b027cdd0fba197992e2929be318742d))
- **branches** Detect branches whose upstream was deleted - ([8a0b3d1](https://github.com/noirbizarre/git-synchronizer/commit/8a0b3d1a509d68cd193893cdc239fa9afc68843b))
- **branches** Detect squash-merged branches via combined patch-id ([#43](https://github.com/noirbizarre/git-synchronizer/issues/43)) - ([a83bc0a](https://github.com/noirbizarre/git-synchronizer/commit/a83bc0ad2ab5f9b3898a27609edf631c3888cecf))
- **branches** Detect branches whose simulated merge adds nothing ([#41](https://github.com/noirbizarre/git-synchronizer/issues/41)) - ([ce92247](https://github.com/noirbizarre/git-synchronizer/commit/ce92247b47a267665771661c2862fa164e32a20a))
- **branches** Add patch-id match for merged branch detection ([#40](https://github.com/noirbizarre/git-synchronizer/issues/40)) - ([725fdfa](https://github.com/noirbizarre/git-synchronizer/commit/725fdfaf65800eab68621869e5fcc66759604909))
- **branches** Add tree SHA comparison for merged branch detection ([#21](https://github.com/noirbizarre/git-synchronizer/issues/21)) - ([3e9b570](https://github.com/noirbizarre/git-synchronizer/commit/3e9b570ef16b6e00b771dafb293720abcdf728fa))
- **branches** Add empty diff detection for squash-merged branches ([#20](https://github.com/noirbizarre/git-synchronizer/issues/20)) - ([454ee95](https://github.com/noirbizarre/git-synchronizer/commit/454ee95f77bf6eeabbf20639a8572fac1f8c5164))
- **cleaner** Fast-forward target branches before merge detection ([#23](https://github.com/noirbizarre/git-synchronizer/issues/23)) - ([8dd9386](https://github.com/noirbizarre/git-synchronizer/commit/8dd9386cc00d3bec0ed9f41416b3be55c00f1283))
- **ui** Show spinners during slow operations ([#48](https://github.com/noirbizarre/git-synchronizer/issues/48)) - ([b15df03](https://github.com/noirbizarre/git-synchronizer/commit/b15df038676125763c65e8c9a2dba9171ace41d4))
- **ui** Add reverse-selection shortcut to multi-select prompts ([#47](https://github.com/noirbizarre/git-synchronizer/issues/47)) - ([ec50ef5](https://github.com/noirbizarre/git-synchronizer/commit/ec50ef5ddc0e5cb18892896f0528cfc2c61e47f4))
- **worktrees** Skip locked worktrees during cleanup ([#22](https://github.com/noirbizarre/git-synchronizer/issues/22)) - ([32f7ab0](https://github.com/noirbizarre/git-synchronizer/commit/32f7ab011f381d6efc5f47f59754ec3a71f0f921))
- Add worktree force-removal prompts with dirty/unmerged detection ([#39](https://github.com/noirbizarre/git-synchronizer/issues/39)) - ([7353707](https://github.com/noirbizarre/git-synchronizer/commit/73537073dc1fbec50aa60b367fef8661522420ed))

### 🐛 Bug Fixes

- **changelog** Remove duplicate header - ([7fccf54](https://github.com/noirbizarre/git-synchronizer/commit/7fccf54d9c414767eec97d222175bee738626d19))
- **ci** Move codecov status config under coverage top-level key ([#35](https://github.com/noirbizarre/git-synchronizer/issues/35)) - ([f98ecc0](https://github.com/noirbizarre/git-synchronizer/commit/f98ecc0081c7d68aae811495a5f7499a68d79f55))
- **ci** Use GitHub App token for release-plz to fix PR recreation ([#16](https://github.com/noirbizarre/git-synchronizer/issues/16)) - ([0bce1f8](https://github.com/noirbizarre/git-synchronizer/commit/0bce1f831f0a4708c05e94a4746cd83d5a465ffa))
- **cleaner** Honour --dry-run when worktrunk handled the worktree - ([fe93761](https://github.com/noirbizarre/git-synchronizer/commit/fe93761ea19d389a6fc7a767a0773c3155248e54))
- **cleaner** Delete branches worktrunk leaves behind on worktree removal ([#46](https://github.com/noirbizarre/git-synchronizer/issues/46)) - ([2f46528](https://github.com/noirbizarre/git-synchronizer/commit/2f465285cff5377582fb536ea28f37c0dbe73802))
- **cleaner** Skip force-removal prompt for merged-but-unreachable branches ([#44](https://github.com/noirbizarre/git-synchronizer/issues/44)) - ([79c3d50](https://github.com/noirbizarre/git-synchronizer/commit/79c3d50e855d7df060125f39b3beba0a43682b4b))
- **cli** Show clean error when run outside a git repository ([#36](https://github.com/noirbizarre/git-synchronizer/issues/36)) - ([aeb8fba](https://github.com/noirbizarre/git-synchronizer/commit/aeb8fbae8e9186bf41e66490c2147621486d4d8e))
- **errors** Stop swallowing git failures as empty results - ([c912daf](https://github.com/noirbizarre/git-synchronizer/commit/c912daf5d7964493c8057033ed63d0afcd3bc3b0))
- **git** Compare against the merge base in empty-diff detection - ([0c2c663](https://github.com/noirbizarre/git-synchronizer/commit/0c2c66305fc7d8fe3624d048358009f415e8727c))
- **git** Handle network failures gracefully ([#42](https://github.com/noirbizarre/git-synchronizer/issues/42)) - ([3587d93](https://github.com/noirbizarre/git-synchronizer/commit/3587d93dd9929a3b54a5fc2423bbe8a5f55cc02b))
- **ui** Correct summary() pluralization to avoid 'worktreees' - ([4c4c8f0](https://github.com/noirbizarre/git-synchronizer/commit/4c4c8f0253e37cb30cecb143e48bdad90882d1d4))
- Correct worktrunk detection, config set and a bogus test - ([331cc41](https://github.com/noirbizarre/git-synchronizer/commit/331cc41f618a0cc929758d6bc1bc08d6c7a0095a))

### ⚡ Performance

- **branches** Use HashSet for O(1) candidate membership checks - ([e7894fc](https://github.com/noirbizarre/git-synchronizer/commit/e7894fcb51b5e6c84cf38ec424661240eaec1f42))

### 🔨 Refactor

- **cleaner** Simplify deleted-upstream messaging - ([ad896b1](https://github.com/noirbizarre/git-synchronizer/commit/ad896b19602ca43cbe6c047d0c4f4f30222a8fcd))
- **cleaner** Unify branch and worktree deletion into a single multiselect ([#37](https://github.com/noirbizarre/git-synchronizer/issues/37)) - ([05b7f5d](https://github.com/noirbizarre/git-synchronizer/commit/05b7f5d719541884895da96b9a48f930876e568e))
- **cleaner** Add Debug, Clone, Default derives to CleanerOptions - ([513bb52](https://github.com/noirbizarre/git-synchronizer/commit/513bb523d7273458f6924d4ca3897f4871acafac))
- **config** Extract config_remove_value onto Git - ([d2135bf](https://github.com/noirbizarre/git-synchronizer/commit/d2135bf3d60c61812e21be2d3342e95dbedc5bff))
- **config** Use SECTION constant instead of hardcoded 'sync.' strings - ([0ad468b](https://github.com/noirbizarre/git-synchronizer/commit/0ad468b26e119d89fd96298860dadfc8241253f9))
- **errors** Unify git error classification and reporting - ([6f67e2e](https://github.com/noirbizarre/git-synchronizer/commit/6f67e2e1693078bf05a06481aaf4c21c2807e18c))
- **git** Merge the two worktrunk removal methods - ([b1cc419](https://github.com/noirbizarre/git-synchronizer/commit/b1cc419235833496fb7bfd0a7abd6bccae8fb4d3))
- **git** Use a single working-directory mechanism - ([37ec64a](https://github.com/noirbizarre/git-synchronizer/commit/37ec64af33ff9fae05e1dfa0f9228e44e0866be1))
- **git** Route every git invocation through the shared run helper - ([5d8aa71](https://github.com/noirbizarre/git-synchronizer/commit/5d8aa71badbdae20e50e905fd628f256bad80ae2))
- **tests** Extract shared test helpers into test_helpers module - ([e99f0a5](https://github.com/noirbizarre/git-synchronizer/commit/e99f0a5e40490d242646e8576fbba2f5daa82715))
- **tests** Standardize test return types to Result<()> - ([c5c2afb](https://github.com/noirbizarre/git-synchronizer/commit/c5c2afbcbed18e804436a4e109db6f0062acf8f5))
- **ui** Render config output through the Ui abstraction - ([0f50e12](https://github.com/noirbizarre/git-synchronizer/commit/0f50e123782d67e56d91b13f266cd938281cd9e6))
- **ui** Rename Ui fields to avoid shadowing method names - ([40c8653](https://github.com/noirbizarre/git-synchronizer/commit/40c8653c2f9b5e094a11dcdd31b2006250ae2ae7))
- **worktrees** Integrate worktree selection into branch multiselect ([#32](https://github.com/noirbizarre/git-synchronizer/issues/32)) - ([9da4a1c](https://github.com/noirbizarre/git-synchronizer/commit/9da4a1c11548920f4b8a698df68d01cec455a2e0))
- Use Path types and consistent derives across peer APIs - ([4ba8639](https://github.com/noirbizarre/git-synchronizer/commit/4ba8639ea6a07aff08bc0dd1f05f13e9300900f4))

### 📚 Documentation

- **readme** Correct merge-detection diagram, refspec and worktrunk notes - ([dda94db](https://github.com/noirbizarre/git-synchronizer/commit/dda94dbaa68bedb2a07b44910d57534975ed869a))
- **readme** Describe the dirty-worktree confirmation pass - ([95b3d93](https://github.com/noirbizarre/git-synchronizer/commit/95b3d930a7b306eed4b9fedc1888caf299897713))
- **readme** Document prek hook installation in the dev workflow - ([ed15f09](https://github.com/noirbizarre/git-synchronizer/commit/ed15f09fe80bbc33dd6a942853f06a03a0d8a0ba))
- **readme** Correct the fetch step command and scope - ([f95d215](https://github.com/noirbizarre/git-synchronizer/commit/f95d21538a0d4e2bddcee097cdede75985b0b4c0))
- **readme** Document the full merge detection stack - ([fc0c576](https://github.com/noirbizarre/git-synchronizer/commit/fc0c576b1d42249cc10ad916594b7ed0eb6682de))
- **readme** List all four well-known protected branch names - ([bb4969d](https://github.com/noirbizarre/git-synchronizer/commit/bb4969d7ae5cf25e171c38fc07c80103606f700b))
- **readme** Clarify that fetch always targets all remotes - ([c067300](https://github.com/noirbizarre/git-synchronizer/commit/c067300ca30f3e33f2ded8dd113b2473dbca8d5a))
- **readme** Document the config set subcommand - ([4ac1bba](https://github.com/noirbizarre/git-synchronizer/commit/4ac1bba6b11398257052f733724c318e83b49266))
- **readme** Fix branch deletion flag from -d to -D - ([0a83097](https://github.com/noirbizarre/git-synchronizer/commit/0a8309798bf44a38779f2507ba00a5d614d4daf7))
- **rustdoc** Correct docs contradicting their implementations - ([9694c8a](https://github.com/noirbizarre/git-synchronizer/commit/9694c8a13e95854d1a0f0efaa8cef96b67e51cd7))
- **ui** Document why output methods discard I/O errors - ([1e483b2](https://github.com/noirbizarre/git-synchronizer/commit/1e483b23725347b03caaf59fffd76d05290cf327))

### 🧪 Tests

- **branches** Pin remote detection to ancestor merges - ([533bfae](https://github.com/noirbizarre/git-synchronizer/commit/533bfae3999cdd9191030e4eb4470c993f8e72a1))
- Drop the redundant test_ prefix from unit test names - ([0a379da](https://github.com/noirbizarre/git-synchronizer/commit/0a379daf760a3272153ed6e275a8a164e981beeb))
- Consolidate repository fixtures into test_helpers - ([980218d](https://github.com/noirbizarre/git-synchronizer/commit/980218d824be8c5d83f164795915fb69a9aefcb9))

### 🎨 Style

- **cleaner** Renumber workflow phase comments to match README - ([170d31b](https://github.com/noirbizarre/git-synchronizer/commit/170d31b4713dae8655389b89b77169f42ce35e7e))
- **ui** Add colored prefixes and per-fragment styling to output methods ([#34](https://github.com/noirbizarre/git-synchronizer/issues/34)) - ([09bde99](https://github.com/noirbizarre/git-synchronizer/commit/09bde99afd49f2deb70533b6e15b52651b408256))
- Unify imports, visibility, naming and doc coverage - ([cc8df59](https://github.com/noirbizarre/git-synchronizer/commit/cc8df595d04f9c4cb5d669367ef3651244af4334))

### 🏗️ Build

- **lint** Align mise lint task with the enforced clippy scope - ([fe06fd7](https://github.com/noirbizarre/git-synchronizer/commit/fe06fd783d45e8c51f1cb2e93ac1473a67d85379))

### 🔧 CI

- **codecov** Get Codecov under control ([#33](https://github.com/noirbizarre/git-synchronizer/issues/33)) - ([17e6e57](https://github.com/noirbizarre/git-synchronizer/commit/17e6e576f7d9f395780acbbbad907eeb84c691b4))
- **mise** Remove the debug path prefix ([#45](https://github.com/noirbizarre/git-synchronizer/issues/45)) - ([5cc6bff](https://github.com/noirbizarre/git-synchronizer/commit/5cc6bff6183848fede3ab3849f81078de4a1c9b5))
- **release** Fix release commit rule ([#19](https://github.com/noirbizarre/git-synchronizer/issues/19)) - ([67421dd](https://github.com/noirbizarre/git-synchronizer/commit/67421dda2904c95f204d84b2fa265879e9219fce))
- Release with gh-ship and git-cliff instead of release-plz ([#50](https://github.com/noirbizarre/git-synchronizer/issues/50)) - ([8e73537](https://github.com/noirbizarre/git-synchronizer/commit/8e73537f47f7b4add45a08c1c677c530079ac5e4))
- Force the worktrunk install so coverage stays deterministic - ([f9722bd](https://github.com/noirbizarre/git-synchronizer/commit/f9722bd7babc331eadb43c86f65e72b5a20de52a))

### 🧹 Chores

- **mise** Remove unused git-cliff dependency - ([e431570](https://github.com/noirbizarre/git-synchronizer/commit/e431570d59823e39cea8479374ac5b5248448c42))

## ❤️ New Contributors

* @noirbizbot[bot] made their first contribution in [#51](https://github.com/noirbizarre/git-synchronizer/pull/51)
## [v0.1.1](https://github.com/noirbizarre/git-synchronizer/compare/v0.1.0..v0.1.1) - 2026-04-09

### 📚 Documentation

- **changelog** Exclude ci, test, style and merge commits ([#10](https://github.com/noirbizarre/git-synchronizer/issues/10)) - ([12d7c86](https://github.com/noirbizarre/git-synchronizer/commit/12d7c86329d4801f161a14f6d9e1b61145e51740))

### 🔧 CI

- **release** Enable release for first trial - ([bfd5f1c](https://github.com/noirbizarre/git-synchronizer/commit/bfd5f1c62efcab3882cd7e3d6b7423b5a02123e2))
- Rewrite release-binaries workflow based on release-plz model ([#12](https://github.com/noirbizarre/git-synchronizer/issues/12)) - ([366fac8](https://github.com/noirbizarre/git-synchronizer/commit/366fac8289a9b91a5626cc75ecd2c9c71f77ee29))
- Replace check job with lint using prek-action v2 ([#8](https://github.com/noirbizarre/git-synchronizer/issues/8)) - ([28d5f1b](https://github.com/noirbizarre/git-synchronizer/commit/28d5f1bac7e3a6b80079a8f16a13eaa1e42914ec))
- Merge coverage into test job ([#6](https://github.com/noirbizarre/git-synchronizer/issues/6)) - ([115505b](https://github.com/noirbizarre/git-synchronizer/commit/115505bbee21cc49078fd11fcba0b081642a318b))
- Release and CI improvements - ([c41c73d](https://github.com/noirbizarre/git-synchronizer/commit/c41c73d2f970c00fd8f79ea327300c3911634d33))

## v0.1.0 - 2026-04-08

### 💫 Features

- **config** Per-branch config protection support - ([c950fb5](https://github.com/noirbizarre/git-synchronizer/commit/c950fb59a8f810bfa2f7a726304ff691aece4833))
- **worktrunk** Delegate worktree removal to worktrunk when available ([#1](https://github.com/noirbizarre/git-synchronizer/issues/1)) - ([926c4bf](https://github.com/noirbizarre/git-synchronizer/commit/926c4bf1248ffe0b4e980409994142474be8de16))

### 🐛 Bug Fixes

- **ci** Fix changelog config for release-plz ([#4](https://github.com/noirbizarre/git-synchronizer/issues/4)) - ([2089e1a](https://github.com/noirbizarre/git-synchronizer/commit/2089e1a63fb96ab9f982d4beb5a2e1640e1f6005))
- **config** Support repositories with extensions.worktreeConfig enabled ([#3](https://github.com/noirbizarre/git-synchronizer/issues/3)) - ([64a2ffa](https://github.com/noirbizarre/git-synchronizer/commit/64a2ffaf66e5065d0ae6fe72cd1bb0c9a3016b0a))

### 🔨 Refactor

- Rename into Git Synchronizer / `git-sync` - ([5d59a1a](https://github.com/noirbizarre/git-synchronizer/commit/5d59a1a57b891fb183bced2304f0a99c3a6074ed))

### 📚 Documentation

- **readme** Document per-branch protection and worktrunk integration ([#2](https://github.com/noirbizarre/git-synchronizer/issues/2)) - ([efc11b2](https://github.com/noirbizarre/git-synchronizer/commit/efc11b2b0baac20776de74595d35e69a74281e7f))

## ❤️ New Contributors

* @noirbizarre made their first contribution in [#4](https://github.com/noirbizarre/git-synchronizer/pull/4)
