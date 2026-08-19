//! Detecting whether a pid embedded in a worktree lock reason is still alive.
//!
//! Locking tools commonly stamp a pid into the `--reason` they pass to
//! `git worktree lock`, but there is no single agreed-upon format. This module
//! extracts a plausible pid from a free-form reason string and checks whether
//! that process is still running, so [`crate::cleaner`] can tell a *stale*
//! lock (owner gone) from a *held* one (owner still there) or an *opaque* one
//! (no pid to check at all).

#[cfg(any(all(unix, not(target_os = "linux")), windows))]
use std::process::Command;

/// Extract a process id from a free-form lock reason string.
///
/// Looks for a `pid` token (case-insensitive) that is not itself part of a
/// longer word — the character right before it, if any, must not be ASCII
/// alphabetic, so `rapid=1234` does not match but `owner-pid=1234` does —
/// followed by zero or more of `:`, `=` or whitespace, then one or more
/// digits. Matches `pid=1234`, `pid: 1234`, `PID 1234`, `owner-pid=1234` and
/// `pid1234`. Returns `None` when no such token is found, or when the digits
/// found do not fit in a `u32`.
pub fn extract_pid(reason: &str) -> Option<u32> {
    let bytes = reason.as_bytes();
    let lower = reason.to_ascii_lowercase();
    let lower_bytes = lower.as_bytes();

    let mut search_from = 0;
    while search_from + 3 <= lower_bytes.len() {
        let Some(rel_idx) = lower_bytes[search_from..]
            .windows(3)
            .position(|w| w == b"pid")
        else {
            break;
        };
        let idx = search_from + rel_idx;
        search_from = idx + 3;

        let preceded_by_letter = idx > 0 && (bytes[idx - 1] as char).is_ascii_alphabetic();
        if preceded_by_letter {
            continue;
        }

        let mut i = idx + 3;
        while i < bytes.len() && matches!(bytes[i], b':' | b'=' | b' ' | b'\t') {
            i += 1;
        }
        let digits_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i > digits_start
            && let Ok(pid) = reason[digits_start..i].parse::<u32>()
        {
            return Some(pid);
        }
    }

    None
}

/// Whether `pid` is currently a running process.
///
/// `None` means liveness could not be determined on this platform or in this
/// environment. Callers must treat `None` the same as "alive" — the
/// conservative choice, since acting on a lock we cannot confirm is dead would
/// risk destroying a worktree someone is still using.
#[cfg(target_os = "linux")]
pub fn pid_is_alive(pid: u32) -> Option<bool> {
    Some(std::path::Path::new(&format!("/proc/{pid}")).exists())
}

/// macOS and BSD have no `/proc` by default; `kill -0` is POSIX and sends no
/// signal, only checks whether the target process (or process group) exists
/// and is one we could otherwise signal.
#[cfg(all(unix, not(target_os = "linux")))]
pub fn pid_is_alive(pid: u32) -> Option<bool> {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .ok()
        .map(|out| out.status.success())
}

#[cfg(windows)]
pub fn pid_is_alive(pid: u32) -> Option<bool> {
    let out = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let pid_str = pid.to_string();
    Some(text.split_whitespace().any(|tok| tok == pid_str))
}

#[cfg(not(any(unix, windows)))]
pub fn pid_is_alive(_pid: u32) -> Option<bool> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_pid_equals_sign() {
        assert_eq!(extract_pid("pid=1234"), Some(1234));
    }

    #[test]
    fn extract_pid_colon_with_space() {
        assert_eq!(extract_pid("pid: 1234"), Some(1234));
    }

    #[test]
    fn extract_pid_uppercase_with_space() {
        assert_eq!(extract_pid("PID 1234"), Some(1234));
    }

    #[test]
    fn extract_pid_prefixed_token() {
        assert_eq!(extract_pid("owner-pid=1234"), Some(1234));
    }

    #[test]
    fn extract_pid_no_separator() {
        assert_eq!(extract_pid("pid1234"), Some(1234));
    }

    #[test]
    fn extract_pid_embedded_in_longer_reason() {
        assert_eq!(
            extract_pid("locked by agent-session pid=4242 on host x"),
            Some(4242)
        );
    }

    #[test]
    fn extract_pid_rejects_part_of_a_longer_word() {
        assert_eq!(extract_pid("rapid=1234"), None);
    }

    #[test]
    fn extract_pid_no_match() {
        assert_eq!(extract_pid("do not touch"), None);
    }

    #[test]
    fn extract_pid_no_digits_after_token() {
        assert_eq!(extract_pid("pid=unknown"), None);
    }

    #[test]
    fn extract_pid_empty_reason() {
        assert_eq!(extract_pid(""), None);
    }

    #[test]
    fn pid_is_alive_of_current_process() {
        assert_eq!(pid_is_alive(std::process::id()), Some(true));
    }

    #[test]
    fn pid_is_alive_of_a_pid_beyond_any_real_pid_max() {
        // Far beyond any real OS's pid_max (Linux: 4194304 by default even at
        // its highest configurable ceiling; macOS/BSD/Windows are far lower),
        // so this is guaranteed dead without spawning and reaping a process.
        assert_eq!(pid_is_alive(4_000_000_000), Some(false));
    }
}
