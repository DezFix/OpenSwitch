// SPDX-License-Identifier: GPL-3.0-or-later
//! "Run as administrator": a Task Scheduler task that starts OpenSwitch with full rights at
//! logon. Windows does not let a normal program see or change typing in programs started as
//! administrator; an elevated OpenSwitch can. The task is created once (one UAC prompt);
//! after that OpenSwitch starts elevated without asking, also when started by hand: a plain
//! start runs the task instead.

use std::os::windows::process::CommandExt;
use std::process::Command;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::*;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

use crate::sys::{self, w};

pub const TASK_NAME: &str = "OpenSwitch";
/// Argument the task starts us with.
pub const FROM_TASK: &str = "--from-task";
/// Argument of the elevated helper: `--admin-task on|off`.
pub const HELPER: &str = "--admin-task";

/// Does this process run with full administrator rights?
pub fn is_elevated() -> bool {
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut e = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut len = 0;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut e as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        );
        CloseHandle(token);
        ok != 0 && e.TokenIsElevated != 0
    }
}

/// SID of the current user as a string (S-1-5-21-...).
fn user_sid() -> Option<String> {
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return None;
        }
        let mut buf = [0u64; 64]; // u64: TOKEN_USER needs 8-byte alignment
        let mut len = 0;
        let ok = GetTokenInformation(token, TokenUser, buf.as_mut_ptr() as *mut _, (buf.len() * 8) as u32, &mut len);
        CloseHandle(token);
        if ok == 0 {
            return None;
        }
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut s: *mut u16 = null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut s) == 0 || s.is_null() {
            return None;
        }
        let n = (0..).take_while(|&i| *s.add(i) != 0).count();
        let sid = String::from_utf16_lossy(std::slice::from_raw_parts(s, n));
        LocalFree(s as _);
        Some(sid)
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Task definition: at logon of this user, with the highest rights, no time limit, also on
/// battery, normal priority (the default 7 is too low for a keyboard hook). Only this user
/// (and administrators) may read and start the task.
fn task_xml(exe: &str, sid: &str) -> String {
    let exe = xml_escape(exe);
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>OpenSwitch with administrator rights (fixes typing in programs started as administrator)</Description>
    <SecurityDescriptor>D:(A;;FA;;;BA)(A;;FA;;;SY)(A;;GRGX;;;{sid})</SecurityDescriptor>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{sid}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{sid}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>"{exe}"</Command>
      <Arguments>{FROM_TASK}</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    )
}

/// Run schtasks.exe without a console window; true on success.
fn schtasks(args: &[&str]) -> bool {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    match Command::new("schtasks.exe").args(args).creation_flags(CREATE_NO_WINDOW).output() {
        Ok(o) if o.status.success() => true,
        Ok(o) => {
            log::warn!("schtasks {:?}: {}", args.first(), String::from_utf8_lossy(&o.stderr).trim());
            false
        }
        Err(e) => {
            log::warn!("schtasks: {e}");
            false
        }
    }
}

/// Start the task (an elevated OpenSwitch) without a UAC prompt; false if it is not there.
pub fn run_task() -> bool {
    schtasks(&["/Run", "/TN", TASK_NAME])
}

/// The task file as schtasks wants it: UTF-16 with BOM (as the XML header says), CRLF.
fn task_file(exe: &str, sid: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(task_xml(exe, sid).replace('\n', "\r\n").encode_utf16().flat_map(|u| u.to_le_bytes()));
    bytes
}

/// `--admin-task on|off`, run elevated (the settings window starts it with a UAC prompt).
/// On: create the task, save the option, drop the Run key, close the running copy and start
/// the elevated one through the task. Off: delete the task, save the option, start with
/// Windows through the Run key again and close the running copy (the settings window starts
/// a normal one). Everything is done here because a normal process may not be allowed to
/// start the task. Returns the process exit code.
pub fn helper(on: bool) -> i32 {
    if on {
        let (Ok(exe), Some(sid)) = (std::env::current_exe(), user_sid()) else { return 2 };
        let path = std::env::temp_dir().join(format!("openswitch-task-{}.xml", std::process::id()));
        if std::fs::write(&path, task_file(&exe.display().to_string(), &sid)).is_err() {
            return 3;
        }
        let ok = schtasks(&["/Create", "/TN", TASK_NAME, "/XML", &path.display().to_string(), "/F"]);
        let _ = std::fs::remove_file(&path);
        if !ok {
            return 4;
        }
    } else if !schtasks(&["/Delete", "/TN", TASK_NAME, "/F"]) {
        return 1;
    }
    sys::save_setting("run_as_admin", if on { "true" } else { "false" });
    sys::set_startup(!on);
    sys::quit_tray();
    if on && !run_task() {
        return 5;
    }
    0
}

/// Why turning the option on or off did not work.
pub enum Failure {
    /// The user answered "No" to the UAC prompt.
    Cancelled,
    Error(String),
}

/// Run our exe elevated (UAC prompt) as the helper and wait for it.
pub fn run_helper(on: bool) -> Result<(), Failure> {
    let exe = std::env::current_exe().map_err(|e| Failure::Error(e.to_string()))?;
    let file = w(&exe.display().to_string());
    let params = w(&format!("{HELPER} {}", if on { "on" } else { "off" }));
    let verb = w("runas");
    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = SW_HIDE;
        if ShellExecuteExW(&mut info) == 0 {
            let e = GetLastError();
            return Err(if e == ERROR_CANCELLED { Failure::Cancelled } else { Failure::Error(format!("error {e}")) });
        }
        if info.hProcess.is_null() {
            return Err(Failure::Error("no process".into()));
        }
        WaitForSingleObject(info.hProcess, 60_000);
        let mut code = 1u32;
        GetExitCodeProcess(info.hProcess, &mut code);
        CloseHandle(info.hProcess);
        if code == 0 {
            Ok(())
        } else {
            Err(Failure::Error(format!("schtasks, code {code}")))
        }
    }
}

/// Turn "run as administrator" on or off from the settings window. Blocks for a few seconds
/// (UAC prompt, restarting the tray app): call it on a worker thread.
pub fn switch(on: bool) -> Result<(), Failure> {
    let result = run_helper(on);
    if on && result.is_ok() {
        // The task needs a moment to start the elevated copy.
        for _ in 0..50 {
            if !sys::tray_window().is_null() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    if (!on && result.is_ok()) || sys::tray_window().is_null() {
        // Turned off, or the elevated copy did not start: run a normal one.
        start_tray_normal();
    }
    result
}

/// Start the tray app without administrator rights, also from an elevated process
/// (Explorer starts it, with the user's normal rights).
pub fn start_tray_normal() {
    let Ok(exe) = std::env::current_exe() else { return };
    if is_elevated() {
        let _ = Command::new("explorer.exe").arg(&exe).spawn();
    } else {
        let _ = Command::new(exe).spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_definition() {
        let x = task_xml(r"C:\Tools & Co\openswitch.exe", "S-1-5-21-1-2-3-1001");
        assert!(x.contains(r#"<Command>"C:\Tools &amp; Co\openswitch.exe"</Command>"#));
        assert!(x.contains("<UserId>S-1-5-21-1-2-3-1001</UserId>"));
        assert!(x.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert!(x.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(x.contains("(A;;GRGX;;;S-1-5-21-1-2-3-1001)"));
        assert!(x.contains(&format!("<Arguments>{FROM_TASK}</Arguments>")));
        assert!(x.contains("<Priority>4</Priority>"));
    }

    /// Writes the task file for a manual `schtasks /Create /XML` check:
    /// `TASK_XML=path cargo test task_xml_file -- --ignored`.
    #[test]
    #[ignore]
    fn task_xml_file() {
        let path = std::env::var("TASK_XML").expect("TASK_XML");
        std::fs::write(path, task_file(&std::env::current_exe().unwrap().display().to_string(), &user_sid().unwrap()))
            .unwrap();
    }

    #[test]
    fn current_user_sid() {
        let sid = user_sid().expect("sid");
        assert!(sid.starts_with("S-1-"), "{sid}");
    }
}
