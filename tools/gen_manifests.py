#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Package manager manifests for a release: winget (3 YAML files) and Scoop (JSON).

    python tools/gen_manifests.py <version> <dist dir> [<out dir>]

<dist dir> holds the release files and SHA256SUMS.txt (as CI makes them). The hashes come
from SHA256SUMS.txt, the download links point at the GitHub release of tag v<version>.
How to submit them: docs/RELEASING.md.
"""
import json
import os
import sys

REPO = "DezFix/OpenSwitch"
WINGET_ID = "DezFix.OpenSwitch"
DESCRIPTION = "Free, open-source automatic keyboard layout switcher (EN/RU/UA/BY) for Windows"
MANIFEST_VERSION = "1.6.0"


def sums(dist):
    out = {}
    with open(os.path.join(dist, "SHA256SUMS.txt"), encoding="ascii") as f:
        for line in f:
            parts = line.split()
            if len(parts) == 2:
                out[parts[1]] = parts[0].upper()
    return out


def winget(version, setup, sha):
    url = f"https://github.com/{REPO}/releases/download/v{version}/{setup}"
    head = f"# yaml-language-server: $schema=https://aka.ms/winget-manifest.{{kind}}.{MANIFEST_VERSION}.schema.json\n"
    common = f"PackageIdentifier: {WINGET_ID}\nPackageVersion: {version}\n"
    version_yaml = (
        head.format(kind="version")
        + common
        + f"DefaultLocale: en-US\nManifestType: version\nManifestVersion: {MANIFEST_VERSION}\n"
    )
    installer_yaml = (
        head.format(kind="installer")
        + common
        + "InstallerType: inno\n"
        + "UpgradeBehavior: install\n"
        + "Installers:\n"
        + "  - Architecture: x64\n"
        + "    Scope: user\n"
        + f"    InstallerUrl: {url}\n"
        + f"    InstallerSha256: {sha}\n"
        + "    InstallerSwitches:\n"
        + "      Custom: /CURRENTUSER\n"
        + "  - Architecture: x64\n"
        + "    Scope: machine\n"
        + f"    InstallerUrl: {url}\n"
        + f"    InstallerSha256: {sha}\n"
        + "    InstallerSwitches:\n"
        + "      Custom: /ALLUSERS\n"
        + f"ManifestType: installer\nManifestVersion: {MANIFEST_VERSION}\n"
    )
    locale_yaml = (
        head.format(kind="defaultLocale")
        + common
        + "PackageLocale: en-US\n"
        + "Publisher: OpenSwitch contributors\n"
        + f"PublisherUrl: https://github.com/{REPO}\n"
        + f"PublisherSupportUrl: https://github.com/{REPO}/issues\n"
        + "PackageName: OpenSwitch\n"
        + f"PackageUrl: https://github.com/{REPO}\n"
        + "License: GPL-3.0-or-later\n"
        + f"LicenseUrl: https://github.com/{REPO}/blob/main/LICENSE\n"
        + f"ShortDescription: {DESCRIPTION}\n"
        + "Description: Fixes words typed in the wrong keyboard layout (ghbdtn -> привет), works offline, no AI, nothing is sent anywhere.\n"
        + "Tags:\n  - keyboard\n  - keyboard-layout\n  - layout-switcher\n  - punto-switcher\n  - russian\n  - ukrainian\n"
        + f"ReleaseNotesUrl: https://github.com/{REPO}/releases/tag/v{version}\n"
        + f"ManifestType: defaultLocale\nManifestVersion: {MANIFEST_VERSION}\n"
    )
    return {
        f"{WINGET_ID}.yaml": version_yaml,
        f"{WINGET_ID}.installer.yaml": installer_yaml,
        f"{WINGET_ID}.locale.en-US.yaml": locale_yaml,
    }


def scoop(version, zipname, sha):
    base = f"https://github.com/{REPO}/releases/download"
    return {
        "version": version,
        "description": DESCRIPTION,
        "homepage": f"https://github.com/{REPO}",
        "license": "GPL-3.0-or-later",
        "architecture": {"64bit": {"url": f"{base}/v{version}/{zipname}", "hash": sha.lower()}},
        "bin": "openswitch.exe",
        "shortcuts": [["openswitch.exe", "OpenSwitch"]],
        "notes": [
            "Settings live in %APPDATA%\\OpenSwitch and are kept on update.",
            "After an update, turn 'Start with Windows' off and on again in the settings.",
        ],
        "checkver": "github",
        "autoupdate": {
            "architecture": {
                "64bit": {"url": f"{base}/v$version/OpenSwitch-v$version-windows-x64.zip"}
            }
        },
    }


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    version = sys.argv[1].lstrip("v")
    dist = sys.argv[2]
    out = sys.argv[3] if len(sys.argv) > 3 else dist
    h = sums(dist)
    setup = f"OpenSwitch-{version}-setup.exe"
    zipname = f"OpenSwitch-v{version}-windows-x64.zip"
    missing = [n for n in (setup, zipname) if n not in h]
    if missing:
        sys.exit(f"not in SHA256SUMS.txt: {', '.join(missing)}")
    wdir = os.path.join(out, "winget", "manifests", "d", "DezFix", "OpenSwitch", version)
    os.makedirs(wdir, exist_ok=True)
    for name, text in winget(version, setup, h[setup]).items():
        with open(os.path.join(wdir, name), "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
    with open(os.path.join(out, "openswitch.json"), "w", encoding="utf-8", newline="\n") as f:
        json.dump(scoop(version, zipname, h[zipname]), f, indent=4, ensure_ascii=False)
        f.write("\n")
    print(f"winget: {wdir}\nscoop: {os.path.join(out, 'openswitch.json')}")


if __name__ == "__main__":
    main()
