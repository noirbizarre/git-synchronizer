//! Build script generating the `git-sync` man pages and shell completions.
//!
//! Both are derived from the very same clap definition as the binary, so they
//! can never drift from `--help`.
//!
//! `git <subcommand> --help` is rewritten by git into `git help <subcommand>`,
//! which runs `man git-sync`. Without an installed man page that command fails
//! with "No manual entry for git-sync", hence the pages generated here.
//!
//! Artifacts land in `$OUT_DIR`:
//!
//! - `man/git-sync.1` and one page per subcommand
//! - `completions/` for bash, zsh, fish, elvish and PowerShell
//!
//! Use `mise run man` to collect them into `dist/`.

use std::path::PathBuf;
use std::{env, fs, io};

use clap::CommandFactory;
use clap_complete::Shell;

#[path = "src/cli.rs"]
#[allow(dead_code)]
mod cli;

// `cli` names it through `crate::duration`, which here resolves against the
// build script's own root.
#[path = "src/duration.rs"]
#[allow(dead_code)]
mod duration;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-changed=src/duration.rs");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is always set by cargo"));

    let man_dir = out_dir.join("man");
    fs::create_dir_all(&man_dir)?;
    clap_mangen::generate_to(cli::Cli::command(), &man_dir)?;

    let completions_dir = out_dir.join("completions");
    fs::create_dir_all(&completions_dir)?;
    let mut cmd = cli::Cli::command();
    for shell in [
        Shell::Bash,
        Shell::Zsh,
        Shell::Fish,
        Shell::Elvish,
        Shell::PowerShell,
    ] {
        clap_complete::generate_to(shell, &mut cmd, "git-sync", &completions_dir)?;
    }

    Ok(())
}
