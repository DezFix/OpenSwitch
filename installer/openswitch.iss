; SPDX-License-Identifier: GPL-3.0-or-later
; OpenSwitch installer (Inno Setup 6). Built by CI for every release:
;   ISCC.exe /DAppVersion=0.7.0 /O"dist" installer\openswitch.iss
; Expects target\release\openswitch.exe to be built.
;
; Installs for the current user without administrator rights (or for all users, if chosen),
; adds a Start menu shortcut and, if ticked, starts OpenSwitch with Windows. Uninstalling
; closes OpenSwitch and removes the autostart, including the "run as administrator" task.
; The user's settings in %APPDATA%\OpenSwitch are kept.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{525886F6-0E17-4F4A-82D9-D778B7EE080E}
AppName=OpenSwitch
AppVersion={#AppVersion}
AppVerName=OpenSwitch {#AppVersion}
AppPublisher=OpenSwitch contributors
AppPublisherURL=https://github.com/DezFix/OpenSwitch
AppSupportURL=https://github.com/DezFix/OpenSwitch/issues
AppUpdatesURL=https://github.com/DezFix/OpenSwitch/releases
DefaultDirName={autopf}\OpenSwitch
DefaultGroupName=OpenSwitch
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputBaseFilename=OpenSwitch-{#AppVersion}-setup
SetupIconFile=..\assets\openswitch.ico
UninstallDisplayIcon={app}\openswitch.exe
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=no

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"
Name: "uk"; MessagesFile: "compiler:Languages\Ukrainian.isl"

[CustomMessages]
en.Autostart=Start OpenSwitch with Windows
ru.Autostart=Запускать OpenSwitch вместе с Windows
uk.Autostart=Запускати OpenSwitch разом з Windows
en.DesktopIcon=Shortcut on the desktop
ru.DesktopIcon=Ярлык на рабочем столе
uk.DesktopIcon=Ярлик на робочому столі
en.RunNow=Start OpenSwitch
ru.RunNow=Запустить OpenSwitch
uk.RunNow=Запустити OpenSwitch

[Tasks]
Name: "autostart"; Description: "{cm:Autostart}"
Name: "desktopicon"; Description: "{cm:DesktopIcon}"; Flags: unchecked

[Files]
Source: "..\target\release\openswitch.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"
Source: "..\README.md"; DestDir: "{app}"

[Icons]
Name: "{group}\OpenSwitch"; Filename: "{app}\openswitch.exe"
Name: "{autodesktop}\OpenSwitch"; Filename: "{app}\openswitch.exe"; Tasks: desktopicon

[Registry]
; The same value OpenSwitch itself uses for "Start with Windows".
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "OpenSwitch"; ValueData: """{app}\openswitch.exe"""; Tasks: autostart; Flags: uninsdeletevalue

[Run]
Filename: "{app}\openswitch.exe"; Description: "{cm:RunNow}"; Flags: nowait postinstall skipifsilent

[Code]
const
  WM_QUIT_APP = $8009; { WM_APP + 9: "quit" for OpenSwitch's hidden window }

{ Ask a running OpenSwitch to quit (also one with administrator rights: it lets this message
  through); versions without that message get a normal close request from taskkill. }
procedure CloseOpenSwitch();
var
  Wnd: HWND;
  I, Code: Integer;
begin
  Wnd := FindWindowByClassName('OpenSwitchWindow');
  if Wnd = 0 then
    Exit;
  PostMessage(Wnd, WM_QUIT_APP, 0, 0);
  for I := 1 to 30 do
  begin
    Sleep(100);
    if FindWindowByClassName('OpenSwitchWindow') = 0 then
      Exit;
  end;
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/IM openswitch.exe', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Sleep(1000);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  CloseOpenSwitch();
  Result := '';
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Code: Integer;
begin
  if CurUninstallStep = usUninstall then
  begin
    CloseOpenSwitch();
    { "Run as administrator" task: deleting it needs administrator rights (one UAC prompt). }
    if Exec(ExpandConstant('{sys}\schtasks.exe'), '/Query /TN OpenSwitch', '', SW_HIDE, ewWaitUntilTerminated, Code) and (Code = 0) then
      ShellExec('runas', ExpandConstant('{sys}\schtasks.exe'), '/Delete /TN OpenSwitch /F', '', SW_HIDE, ewWaitUntilTerminated, Code);
  end;
end;
