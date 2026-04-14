# Changelog

## 🚀 [0.1.2](https://github.com/noirbizarre/git-synchronizer/compare/v0.1.1...v0.1.2) (2026-04-14)

### 💫 New features

- **branches:** Add tree SHA comparison for merged branch detection ([#21](https://github.com/noirbizarre/git-synchronizer/pull/21)) ([3e9b570](https://github.com/noirbizarre/git-synchronizer/commit/3e9b570ef16b6e00b771dafb293720abcdf728fa))
- **branches:** Add empty diff detection for squash-merged branches ([#20](https://github.com/noirbizarre/git-synchronizer/pull/20)) ([454ee95](https://github.com/noirbizarre/git-synchronizer/commit/454ee95f77bf6eeabbf20639a8572fac1f8c5164))
- **cleaner:** Fast-forward target branches before merge detection ([#23](https://github.com/noirbizarre/git-synchronizer/pull/23)) ([8dd9386](https://github.com/noirbizarre/git-synchronizer/commit/8dd9386cc00d3bec0ed9f41416b3be55c00f1283))
- **worktrees:** Skip locked worktrees during cleanup ([#22](https://github.com/noirbizarre/git-synchronizer/pull/22)) ([32f7ab0](https://github.com/noirbizarre/git-synchronizer/commit/32f7ab011f381d6efc5f47f59754ec3a71f0f921))

### 🐛 Bug fixes

- **ci:** Move codecov status config under coverage top-level key ([#35](https://github.com/noirbizarre/git-synchronizer/pull/35)) ([f98ecc0](https://github.com/noirbizarre/git-synchronizer/commit/f98ecc0081c7d68aae811495a5f7499a68d79f55))
- **ci:** Use GitHub App token for release-plz to fix PR recreation ([#16](https://github.com/noirbizarre/git-synchronizer/pull/16)) ([0bce1f8](https://github.com/noirbizarre/git-synchronizer/commit/0bce1f831f0a4708c05e94a4746cd83d5a465ffa))
- **cli:** Show clean error when run outside a git repository ([#36](https://github.com/noirbizarre/git-synchronizer/pull/36)) ([aeb8fba](https://github.com/noirbizarre/git-synchronizer/commit/aeb8fbae8e9186bf41e66490c2147621486d4d8e))

### 🔧 Refactorings

- **cleaner:** Unify branch and worktree deletion into a single multiselect ([#37](https://github.com/noirbizarre/git-synchronizer/pull/37)) ([05b7f5d](https://github.com/noirbizarre/git-synchronizer/commit/05b7f5d719541884895da96b9a48f930876e568e))
- **worktrees:** Integrate worktree selection into branch multiselect ([#32](https://github.com/noirbizarre/git-synchronizer/pull/32)) ([9da4a1c](https://github.com/noirbizarre/git-synchronizer/commit/9da4a1c11548920f4b8a698df68d01cec455a2e0))



## 🚀 [0.1.1](https://github.com/noirbizarre/git-synchronizer/compare/v0.1.0...v0.1.1) (2026-04-09)

### 📖 Documentation

- **changelog:** Exclude ci, test, style and merge commits ([#10](https://github.com/noirbizarre/git-synchronizer/pull/10)) ([12d7c86](https://github.com/noirbizarre/git-synchronizer/commit/12d7c86329d4801f161a14f6d9e1b61145e51740))
