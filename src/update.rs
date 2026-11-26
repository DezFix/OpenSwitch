// SPDX-License-Identifier: GPL-3.0-or-later
//! Optional check for a new version (off by default; the user turns it on).
//!
//! Once a day the tray app asks GitHub for the latest release of OpenSwitch: one HTTPS
//! request to `api.github.com` with a generic User-Agent; nothing the user typed and no
//! settings are sent. When there is a newer version, a notification offers its page —
//! OpenSwitch never downloads or installs anything by itself.
//!
//! This file is the part that does not depend on Windows: reading the answer and comparing
//! versions. The request itself is in `src/updates.rs`.

/// Where the latest release is asked for.
pub const API_HOST: &str = "api.github.com";
pub const API_PATH: &str = "/repos/DezFix/OpenSwitch/releases/latest";
/// The page of a release, for the notification.
pub fn release_page(tag: &str) -> String {
    format!("https://github.com/DezFix/OpenSwitch/releases/tag/{tag}")
}

/// How often the tray app asks (seconds).
pub const INTERVAL_SECS: u64 = 24 * 60 * 60;

/// `1.2.3` from `v1.2.3`, `1.2.3-beta`, `1.2` (missing parts are 0).
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let core = s.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u32>());
    let major = parts.next()?.ok()?;
    let minor = parts.next().unwrap_or(Ok(0)).ok()?;
    let patch = parts.next().unwrap_or(Ok(0)).ok()?;
    Some((major, minor, patch))
}

/// Is `latest` newer than `current`?
pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse_version(latest), parse_version(current)), (Some(l), Some(c)) if l > c)
}

/// The `tag_name` of the GitHub answer (the release's tag, e.g. `v0.7.0`).
pub fn latest_tag(json: &str) -> Option<String> {
    let at = json.find("\"tag_name\"")?;
    let rest = json[at + "\"tag_name\"".len()..].trim_start().strip_prefix(':')?.trim_start();
    let value = rest.strip_prefix('"')?;
    let end = value.find('"')?;
    let tag = &value[..end];
    // A tag is a short plain string; anything else is not trusted.
    (tag.len() <= 32 && tag.chars().all(|c| c.is_ascii_alphanumeric() || ".-+_".contains(c))).then(|| tag.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("v0.7.0"), Some((0, 7, 0)));
        assert_eq!(parse_version("1.2"), Some((1, 2, 0)));
        assert_eq!(parse_version("v1.0.0-beta.1"), Some((1, 0, 0)));
        assert_eq!(parse_version("latest"), None);
        assert!(is_newer("v0.7.0", "0.3.0"));
        assert!(is_newer("v0.10.0", "0.9.5"));
        assert!(!is_newer("v0.7.0", "0.7.0"));
        assert!(!is_newer("v0.6.9", "0.7.0"));
        assert!(!is_newer("nonsense", "0.7.0"));
    }

    #[test]
    fn github_answer() {
        let json =
            r#"{"url":"x","html_url":"https://github.com/a","id":1,"tag_name" : "v0.7.0","name":"OpenSwitch v0.7.0"}"#;
        assert_eq!(latest_tag(json).as_deref(), Some("v0.7.0"));
        assert_eq!(latest_tag(r#"{"message":"Not Found"}"#), None);
        assert_eq!(latest_tag(r#"{"tag_name":"v1<script>"}"#), None);
        assert_eq!(release_page("v0.7.0"), "https://github.com/DezFix/OpenSwitch/releases/tag/v0.7.0");
    }
}
