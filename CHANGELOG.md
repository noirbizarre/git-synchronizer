# Changelog

## 🚀 [0.1.2](https://github.com/noirbizarre/git-synchronizer/compare/v0.1.1...v0.1.2) (2026-08-01)

### 💫 New features

- **branches:** Detect branches whose upstream was deleted ([8a0b3d1](https://github.com/noirbizarre/git-synchronizer/commit/8a0b3d1a509d68cd193893cdc239fa9afc68843b))
- **branches:** Detect squash-merged branches via combined patch-id ([#43](https://github.com/noirbizarre/git-synchronizer/pull/43)) ([a83bc0a](https://github.com/noirbizarre/git-synchronizer/commit/a83bc0ad2ab5f9b3898a27609edf631c3888cecf))
- **branches:** Detect branches whose simulated merge adds nothing ([#41](https://github.com/noirbizarre/git-synchronizer/pull/41)) ([ce92247](https://github.com/noirbizarre/git-synchronizer/commit/ce92247b47a267665771661c2862fa164e32a20a))
- **branches:** Add patch-id match for merged branch detection ([#40](https://github.com/noirbizarre/git-synchronizer/pull/40)) ([725fdfa](https://github.com/noirbizarre/git-synchronizer/commit/725fdfaf65800eab68621869e5fcc66759604909))
- **branches:** Add tree SHA comparison for merged branch detection ([#21](https://github.com/noirbizarre/git-synchronizer/pull/21)) ([3e9b570](https://github.com/noirbizarre/git-synchronizer/commit/3e9b570ef16b6e00b771dafb293720abcdf728fa))
- **branches:** Add empty diff detection for squash-merged branches ([#20](https://github.com/noirbizarre/git-synchronizer/pull/20)) ([454ee95](https://github.com/noirbizarre/git-synchronizer/commit/454ee95f77bf6eeabbf20639a8572fac1f8c5164))
- **cleaner:** Fast-forward target branches before merge detection ([#23](https://github.com/noirbizarre/git-synchronizer/pull/23)) ([8dd9386](https://github.com/noirbizarre/git-synchronizer/commit/8dd9386cc00d3bec0ed9f41416b3be55c00f1283))
- **ui:** Show spinners during slow operations ([#48](https://github.com/noirbizarre/git-synchronizer/pull/48)) ([b15df03](https://github.com/noirbizarre/git-synchronizer/commit/b15df038676125763c65e8c9a2dba9171ace41d4))
- **ui:** Add reverse-selection shortcut to multi-select prompts ([#47](https://github.com/noirbizarre/git-synchronizer/pull/47)) ([ec50ef5](https://github.com/noirbizarre/git-synchronizer/commit/ec50ef5ddc0e5cb18892896f0528cfc2c61e47f4))
- **worktrees:** Skip locked worktrees during cleanup ([#22](https://github.com/noirbizarre/git-synchronizer/pull/22)) ([32f7ab0](https://github.com/noirbizarre/git-synchronizer/commit/32f7ab011f381d6efc5f47f59754ec3a71f0f921))
- Add worktree force-removal prompts with dirty/unmerged detection ([#39](https://github.com/noirbizarre/git-synchronizer/pull/39)) ([7353707](https://github.com/noirbizarre/git-synchronizer/commit/73537073dc1fbec50aa60b367fef8661522420ed))

### 🐛 Bug fixes

- **ci:** Move codecov status config under coverage top-level key ([#35](https://github.com/noirbizarre/git-synchronizer/pull/35)) ([f98ecc0](https://github.com/noirbizarre/git-synchronizer/commit/f98ecc0081c7d68aae811495a5f7499a68d79f55))
- **ci:** Use GitHub App token for release-plz to fix PR recreation ([#16](https://github.com/noirbizarre/git-synchronizer/pull/16)) ([0bce1f8](https://github.com/noirbizarre/git-synchronizer/commit/0bce1f831f0a4708c05e94a4746cd83d5a465ffa))
- **cleaner:** Honour --dry-run when worktrunk handled the worktree ([fe93761](https://github.com/noirbizarre/git-synchronizer/commit/fe93761ea19d389a6fc7a767a0773c3155248e54))
- **cleaner:** Delete branches worktrunk leaves behind on worktree removal ([#46](https://github.com/noirbizarre/git-synchronizer/pull/46)) ([2f46528](https://github.com/noirbizarre/git-synchronizer/commit/2f465285cff5377582fb536ea28f37c0dbe73802))
- **cleaner:** Skip force-removal prompt for merged-but-unreachable branches ([#44](https://github.com/noirbizarre/git-synchronizer/pull/44)) ([79c3d50](https://github.com/noirbizarre/git-synchronizer/commit/79c3d50e855d7df060125f39b3beba0a43682b4b))
- **cli:** Show clean error when run outside a git repository ([#36](https://github.com/noirbizarre/git-synchronizer/pull/36)) ([aeb8fba](https://github.com/noirbizarre/git-synchronizer/commit/aeb8fbae8e9186bf41e66490c2147621486d4d8e))
- **git:** Handle network failures gracefully ([#42](https://github.com/noirbizarre/git-synchronizer/pull/42)) ([3587d93](https://github.com/noirbizarre/git-synchronizer/commit/3587d93dd9929a3b54a5fc2423bbe8a5f55cc02b))

### 🔧 Refactorings

- **cleaner:** Simplify deleted-upstream messaging ([ad896b1](https://github.com/noirbizarre/git-synchronizer/commit/ad896b19602ca43cbe6c047d0c4f4f30222a8fcd))
- **cleaner:** Unify branch and worktree deletion into a single multiselect ([#37](https://github.com/noirbizarre/git-synchronizer/pull/37)) ([05b7f5d](https://github.com/noirbizarre/git-synchronizer/commit/05b7f5d719541884895da96b9a48f930876e568e))
- **worktrees:** Integrate worktree selection into branch multiselect ([#32](https://github.com/noirbizarre/git-synchronizer/pull/32)) ([9da4a1c](https://github.com/noirbizarre/git-synchronizer/commit/9da4a1c11548920f4b8a698df68d01cec455a2e0))



## 🚀 [0.1.1](https://github.com/noirbizarre/git-synchronizer/compare/v0.1.0...v0.1.1) (2026-04-09)

### 📖 Documentation

- **changelog:** Exclude ci, test, style and merge commits ([#10](https://github.com/noirbizarre/git-synchronizer/pull/10)) ([12d7c86](https://github.com/noirbizarre/git-synchronizer/commit/12d7c86329d4801f161a14f6d9e1b61145e51740))
