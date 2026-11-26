// SPDX-License-Identifier: GPL-3.0-or-later
//! The request for the latest version (WinHTTP, built into Windows) and the small state file
//! `%LOCALAPPDATA%\OpenSwitch\update.txt` (when it last asked, which version it already
//! announced). Only used when the user turned "Check for new versions" on; see
//! `openswitch::update` for what is sent.

use std::ffi::c_void;
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::time::{SystemTime, UNIX_EPOCH};

use openswitch::update::{is_newer, latest_tag, API_HOST, API_PATH, INTERVAL_SECS};
use windows_sys::Win32::Networking::WinHttp::*;

use crate::sys::w;

/// Result of one check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    /// A newer release, by its tag (v1.2.3).
    Newer(String),
    Latest,
    Failed(String),
}

/// Closes a WinHTTP handle when dropped.
struct Handle(*mut c_void);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

fn get(host: &str, path: &str) -> Result<String, String> {
    let err = |what: &str| format!("{what}: {}", std::io::Error::last_os_error());
    unsafe {
        // A generic User-Agent (GitHub requires one): the program and its version only.
        let agent = w(&format!("OpenSwitch/{}", env!("CARGO_PKG_VERSION")));
        let session = Handle(WinHttpOpen(agent.as_ptr(), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, null(), null(), 0));
        if session.0.is_null() {
            return Err(err("WinHttpOpen"));
        }
        WinHttpSetTimeouts(session.0, 10_000, 10_000, 10_000, 15_000);
        let conn = Handle(WinHttpConnect(session.0, w(host).as_ptr(), INTERNET_DEFAULT_HTTPS_PORT, 0));
        if conn.0.is_null() {
            return Err(err("connect"));
        }
        let req = Handle(WinHttpOpenRequest(
            conn.0,
            w("GET").as_ptr(),
            w(path).as_ptr(),
            null(),
            null(),
            null(),
            WINHTTP_FLAG_SECURE,
        ));
        if req.0.is_null() {
            return Err(err("request"));
        }
        let headers = w("Accept: application/vnd.github+json\r\n");
        if WinHttpSendRequest(req.0, headers.as_ptr(), u32::MAX, null(), 0, 0, 0) == 0
            || WinHttpReceiveResponse(req.0, null_mut()) == 0
        {
            return Err(err("send"));
        }
        let mut status = 0u32;
        let mut size = 4u32;
        WinHttpQueryHeaders(
            req.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            null(),
            &mut status as *mut u32 as *mut c_void,
            &mut size,
            null_mut(),
        );
        let mut body = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let mut read = 0u32;
            if WinHttpReadData(req.0, buf.as_mut_ptr() as *mut c_void, buf.len() as u32, &mut read) == 0 {
                return Err(err("read"));
            }
            if read == 0 || body.len() > 1 << 20 {
                break;
            }
            body.extend_from_slice(&buf[..read as usize]);
        }
        if status != 200 {
            return Err(format!("HTTP {status}"));
        }
        Ok(String::from_utf8_lossy(&body).into_owned())
    }
}

/// Ask GitHub now (blocks for up to ~15 s: call it on a worker thread).
pub fn check_now() -> Check {
    let result = match get(API_HOST, API_PATH) {
        Ok(json) => match latest_tag(&json) {
            Some(tag) if is_newer(&tag, env!("CARGO_PKG_VERSION")) => Check::Newer(tag),
            Some(_) => Check::Latest,
            None => Check::Failed("no version in the answer".into()),
        },
        Err(e) => Check::Failed(e),
    };
    let mut st = State::load();
    st.checked = now();
    st.save();
    match &result {
        Check::Failed(e) => log::info!("update check failed: {e}"),
        r => log::info!("update check: {r:?}"),
    }
    result
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// What `update.txt` remembers.
#[derive(Default)]
pub struct State {
    /// When the last check ran (Unix time).
    pub checked: u64,
    /// The release tag the user was already told about.
    pub announced: String,
}

fn state_path() -> PathBuf {
    crate::logging::log_dir().parent().map(|p| p.join("update.txt")).unwrap_or_else(|| PathBuf::from("update.txt"))
}

impl State {
    pub fn load() -> State {
        let mut st = State::default();
        for line in std::fs::read_to_string(state_path()).unwrap_or_default().lines() {
            match line.split_once('=') {
                Some(("checked", v)) => st.checked = v.trim().parse().unwrap_or(0),
                Some(("announced", v)) => st.announced = v.trim().to_string(),
                _ => {}
            }
        }
        st
    }

    pub fn save(&self) {
        let p = state_path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, format!("checked={}\r\nannounced={}\r\n", self.checked, self.announced));
    }

    /// Time for the daily check?
    pub fn due(&self) -> bool {
        now().saturating_sub(self.checked) >= INTERVAL_SECS
    }
}

#[cfg(test)]
mod tests {
    /// Real request to GitHub (network): cargo test github_latest -- --ignored
    #[test]
    #[ignore]
    fn github_latest() {
        let json = super::get(openswitch::update::API_HOST, openswitch::update::API_PATH).expect("request");
        let tag = openswitch::update::latest_tag(&json).expect("tag_name");
        println!("latest release: {tag}");
        assert!(openswitch::update::parse_version(&tag).is_some());
        // a missing page is an error, not a crash
        assert!(super::get(openswitch::update::API_HOST, "/repos/DezFix/OpenSwitch/releases/tags/no-such-tag").is_err());
    }
}
