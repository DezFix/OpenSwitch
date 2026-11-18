// SPDX-License-Identifier: GPL-3.0-or-later
//! Build script: embeds the Windows resources into openswitch.exe —
//! application icon, version information (Explorer → Properties → Details) and the
//! application manifest (Per-Monitor V2 DPI awareness, Common Controls 6, asInvoker).
//!
//! On other targets it does nothing.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn rc_path(p: &Path) -> String {
    // Resource-script strings use C escaping.
    p.display().to_string().replace('\\', "\\\\")
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/openswitch.ico");
    println!("cargo:rerun-if-changed=assets/openswitch.manifest.in");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let major = env::var("CARGO_PKG_VERSION_MAJOR").unwrap();
    let minor = env::var("CARGO_PKG_VERSION_MINOR").unwrap();
    let patch = env::var("CARGO_PKG_VERSION_PATCH").unwrap();
    let arch = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86") => "x86",
        _ => "amd64",
    };

    let template =
        fs::read_to_string(manifest_dir.join("assets/openswitch.manifest.in")).expect("assets/openswitch.manifest.in");
    let manifest = template.replace("@VERSION4@", &format!("{major}.{minor}.{patch}.0")).replace("@ARCH@", arch);
    let manifest_path = out_dir.join("openswitch.exe.manifest");
    fs::write(&manifest_path, manifest).unwrap();

    let icon_path = manifest_dir.join("assets/openswitch.ico");
    let rc = format!(
        r#"#pragma code_page(65001)
#include <winver.h>

1 ICON "{icon}"
1 24 "{manifest}"

VS_VERSION_INFO VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS VOS_NT_WINDOWS32
FILETYPE VFT_APP
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "OpenSwitch contributors"
            VALUE "FileDescription", "OpenSwitch - keyboard layout switcher"
            VALUE "FileVersion", "{version}"
            VALUE "InternalName", "openswitch"
            VALUE "LegalCopyright", "GNU GPL v3.0 or later"
            VALUE "OriginalFilename", "openswitch.exe"
            VALUE "ProductName", "OpenSwitch"
            VALUE "ProductVersion", "{version}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#,
        icon = rc_path(&icon_path),
        manifest = rc_path(&manifest_path),
    );
    let rc_file = out_dir.join("openswitch.rc");
    fs::write(&rc_file, rc).unwrap();

    // The manifest carries DPI awareness: without it the tray menu and settings window are
    // blurry on scaled displays, so a missing resource compiler is an error, not a warning.
    embed_resource::compile_for(&rc_file, ["openswitch"], embed_resource::NONE)
        .manifest_required()
        .unwrap_or_else(|e| panic!("could not embed Windows resources: {e}"));
}
