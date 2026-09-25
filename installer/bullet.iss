; Inno Setup script for Bullet — League of Legends skin changer for Windows
; Architecture: x64 native

#define MyAppName "Bullet"
; Injected by `cargo xtask installer` from Cargo.toml, so the installer can never claim a version
; the binary does not have. The fallback only applies when ISCC is run by hand.
#ifndef MyAppVersion
  #define MyAppVersion "0.0.0-manual"
#endif
; VersionInfoVersion only accepts numbers (1.0.0.0): a pre-release suffix (1.0.0-rc.1) is cut here
; and kept in the text versions.
#if Pos("-", MyAppVersion) > 0
  #define MyAppNumericVersion Copy(MyAppVersion, 1, Pos("-", MyAppVersion) - 1)
#else
  #define MyAppNumericVersion MyAppVersion
#endif
#define MyAppPublisher "Isllan Toso"
#define MyAppDescription "League of Legends skin changer for Windows"
#define MyAppCopyright "Copyright (c) 2026 Isllan Toso. MIT License."
#define MyAppURL "https://github.com/Isllanrx/Bullet"
#define MyPublisherURL "https://isllan.dev/"
#define MyAppExeName "bullet.exe"

[Setup]
AppId={{D387A5B1-8C56-4D2A-94B8-975DE11C6B45}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyPublisherURL}
AppSupportURL={#MyAppURL}/issues
AppUpdatesURL={#MyAppURL}/releases
AppCopyright={#MyAppCopyright}
AppComments={#MyAppDescription}
; Properties > Details of the Setup exe.
VersionInfoVersion={#MyAppNumericVersion}
VersionInfoTextVersion={#MyAppVersion}
VersionInfoProductName={#MyAppName}
VersionInfoProductVersion={#MyAppNumericVersion}
VersionInfoProductTextVersion={#MyAppVersion}
VersionInfoCompany={#MyAppPublisher}
VersionInfoDescription={#MyAppName} Setup - {#MyAppDescription}
VersionInfoCopyright={#MyAppCopyright}
; Programs and Features entry.
UninstallDisplayName={#MyAppName}
; Program Files, not %LOCALAPPDATA%\Programs. ADR-007 forbids anything that runs elevated from
; living in a folder a normal user can write to, and #131 puts the injection tools under
; {app}\tools. With PrivilegesRequired=lowest, {autopf} used to send the whole install to
; LocalAppData — writable by the user, which is exactly what the ADR rules out.
DefaultDirName={commonpf}\{#MyAppName}
; Backlog #136: Do not inherit old install path from LocalAppData\Programs on upgrade
UsePreviousAppDir=no
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputDir=..\dist\installer
; Conventional installer name: product, purpose, version and arch, so a user with several downloads
; can tell them apart (e.g. Bullet-Setup-0.1.0-x64.exe).
OutputBaseFilename=Bullet-Setup-{#MyAppVersion}-x64
SetupIconFile=..\assets\bullet.ico
; Shown as the license page of the wizard; the same files are installed next to the program.
LicenseFile=..\LICENSE
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
; Bullet lives in the tray, so on an upgrade it is almost always running. This is the mutex the app
; holds for its whole life (bullet-platform::single_instance): Setup detects it up front and asks the
; user to close Bullet, instead of failing to replace a file in use or demanding a reboot.
AppMutex=Local\Bullet_SingleInstance_bullet
CloseApplications=yes
RestartApplications=no
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
; Off by default. Same value the tray's "Start with Windows" item toggles (bullet-platform::autostart),
; so either side can undo what the other set.
Name: "autostart"; Description: "{cm:AutoStartProgram,{#MyAppName}}"; GroupDescription: "{cm:AutoStartProgramGroupDescription}"; Flags: unchecked

[InstallDelete]
; Leftovers from installs made before ADR-014. These plugins speak a bridge protocol this build does
; not implement, and they sit inside Pengu Loader's folder, where they would still be loaded if the
; user keeps Pengu active. An upgrade has to remove them, not merely stop shipping them.
;
; ISCC warns that per-user paths in an admin install may not resolve to the desktop user's profile.
; True, and accepted: this is best effort for the common single-user case. The reliable cleanup is
; the app's, on first run, where the real profile is resolved by API (backlog #134).
Type: filesandordirs; Name: "{localappdata}\Rose\Pengu Loader\plugins\bullet-bridge-client"
Type: filesandordirs; Name: "{localappdata}\Rose\Pengu Loader\plugins\bullet-selectors"
Type: filesandordirs; Name: "{localappdata}\Rose\Pengu Loader\plugins\bullet-skin-monitor"
Type: filesandordirs; Name: "{localappdata}\Rose\Pengu Loader\plugins\bullet-chroma-wheel"
Type: filesandordirs; Name: "{localappdata}\Bullet\plugins"
; Clean up old user-level install directory from pre-ADR-007 versions (Backlog #136)
Type: filesandordirs; Name: "{localappdata}\Programs\Bullet"
; Strays that installers with a `dist\tools\*` wildcard shipped (#166): an unrelated 52 MB binary and
; backup copies. [Files] lists tools one by one now, so an upgrade must also take these out.
Type: files; Name: "{app}\tools\cloudflared.exe"
Type: files; Name: "{app}\tools\*.orig"
Type: files; Name: "{app}\tools\*.bak"
; Legacy injection binaries no longer shipped (removed: broke on 16.19). An upgrade takes them out.
Type: files; Name: "{app}\tools\cslol-dll.dll"
Type: files; Name: "{app}\tools\mod-tools.exe"
; Overlays built by an earlier Bullet: rebuilt on demand (the cache is keyed by builder revision),
; so dropping them on upgrade frees gigabytes and costs one build.
Type: filesandordirs; Name: "{localappdata}\Bullet\overlay"

[Dirs]
; Bullet's own tools folder (#131): where the injector (LTK host + DLL) and the fallback tools belong, under Program
; Files as ADR-007 requires — never read from another product's install.
Name: "{app}\tools"
Name: "{localappdata}\Bullet\state"
; The skin library is created empty and filled at runtime — official skins are generated from the
; installed game (Fase J), custom mods are the user's. Nothing is shipped into it (see [Files]),
; so there is no library under Program Files and no 160 MB of static .fantome in the installer.
Name: "{localappdata}\Bullet\library"

[Files]
Source: "..\dist\bullet.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\bullet.ico"; DestDir: "{app}\assets"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\THIRD-PARTY-NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
; Injection backend: NOT shipped. The LTK patcher license forbids redistributing League Toolkit's signed
; binaries outside an official LTK Manager release, so the user copies ltk_patcher_host.exe and
; ltk_patcher_dll.dll from one into {app}\tools (created below). Bullet validates both by SHA-256 at runtime
; and tells the user the exact path when they are missing.
; Default party mode configuration (ADR-020). onlyifdoesntexist preserves user customizations on upgrade.
Source: "..\dist\party.json"; DestDir: "{localappdata}\Bullet\state"; Flags: onlyifdoesntexist
; No skin library is shipped. Official skins are generated from the installed game per patch
; (Fase J), so bundling 160 MB of static .fantome only bloated the installer and duplicated the
; library under both Program Files and the profile. The library folder is created empty above and
; filled at runtime; custom mods live in {localappdata}\Bullet\custom_mods.
; Plugins deliberadamente fora do instalador: a ADR-014 tirou os plugins do cliente do caminho
; crítico. Instalá-los colocaria JS que fala um protocolo que este build não implementa, dentro de
; um loader que não é ativado.

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\assets\bullet.ico"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\assets\bullet.ico"; Tasks: desktopicon

[Registry]
; Remove any legacy RUNASADMIN flag on Bullet so that it runs at standard user integrity (ADR-026).
; Both hives: an older installer wrote HKLM, and the "Run this program as an administrator" checkbox
; of the file's Properties writes HKCU. Uninstall removes them too, leaving the registry as it was.
Root: HKLM; Subkey: "SOFTWARE\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers"; ValueType: none; ValueName: "{app}\{#MyAppExeName}"; Flags: deletevalue uninsdeletevalue
Root: HKCU; Subkey: "Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers"; ValueType: none; ValueName: "{app}\{#MyAppExeName}"; Flags: deletevalue uninsdeletevalue
; Start with Windows (optional task). HKCU because Bullet runs unelevated as the desktop user
; (ADR-026); an admin install elevated by the same user writes that user's hive. The quoted path is
; what the app writes too, so the tray shows the item checked.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Bullet"; ValueData: """{app}\{#MyAppExeName}"""; Flags: uninsdeletevalue; Tasks: autostart
; Always registered for removal, task or not: the entry may have been turned on later from the tray,
; and an uninstall must not leave Windows launching a program that is gone. `none` writes nothing.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Bullet"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent runasoriginaluser

[UninstallDelete]
; What Bullet creates at runtime and that has no value without it: logs, state (caches, history,
; party config), the WebView2 profile, built overlays (gigabytes), generated mods, and a tools copy
; in the profile. User content (skins and custom mods) is handled in [Code]: asked, not assumed
; (backlog #77).
;
; Best effort, with the same caveat as [InstallDelete]: an admin uninstall may resolve {localappdata}
; to the elevating admin's profile, not the desktop user's, so a multi-user machine can be left with
; the real user's folder untouched. The reliable, profile-correct cleanup is the app's own, which
; resolves the desktop user by API (backlog #134). `cargo xtask install-audit uninstalled` checks the
; result in the single-user case.
Type: filesandordirs; Name: "{localappdata}\Bullet\logs"
Type: filesandordirs; Name: "{localappdata}\Bullet\state"
Type: filesandordirs; Name: "{localappdata}\Bullet\webview2"
Type: filesandordirs; Name: "{localappdata}\Bullet\overlay"
Type: filesandordirs; Name: "{localappdata}\Bullet\mods"
Type: filesandordirs; Name: "{localappdata}\Bullet\tools"
Type: dirifempty; Name: "{app}\tools"
Type: dirifempty; Name: "{app}\assets"
Type: dirifempty; Name: "{app}"

[CustomMessages]
brazilianportuguese.DeleteUserContent=Remover também as skins e os mods personalizados salvos pelo Bullet?%n%n%1%n%nEscolha "Não" para mantê-los.
english.DeleteUserContent=Also remove the skins and custom mods saved by Bullet?%n%n%1%n%nChoose "No" to keep them.

[Code]
// Skins and custom mods are the user's; the rest of what Bullet made is removed by [UninstallDelete].
// A silent uninstall keeps them (the safe default): nothing the user may want is deleted unasked.
// Same profile caveat as [UninstallDelete]: under an admin uninstall {localappdata} may be the
// admin's profile, so this prompt then lists and removes that profile's folders, not the desktop
// user's. Correct per-user cleanup is the app's job (backlog #134).
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DataDir, Listing: String;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    DataDir := ExpandConstant('{localappdata}\Bullet');
    if DirExists(DataDir + '\library') or DirExists(DataDir + '\skins') or DirExists(DataDir + '\custom_mods') then
    begin
      Listing := DataDir + '\library' + #13#10 + DataDir + '\skins' + #13#10 + DataDir + '\custom_mods';
      if SuppressibleMsgBox(FmtMessage(CustomMessage('DeleteUserContent'), [Listing]), mbConfirmation, MB_YESNO, IDNO) = IDYES then
      begin
        DelTree(DataDir + '\library', True, True, True);
        DelTree(DataDir + '\skins', True, True, True);
        DelTree(DataDir + '\custom_mods', True, True, True);
      end;
    end;
    // Only when nothing is left: never a tree delete of the data folder.
    RemoveDir(DataDir);
  end;
end;

