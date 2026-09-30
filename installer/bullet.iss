#define MyAppName "Bullet"
#ifndef MyAppVersion
  #define MyAppVersion "0.0.0-manual"
#endif
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
VersionInfoVersion={#MyAppNumericVersion}
VersionInfoTextVersion={#MyAppVersion}
VersionInfoProductName={#MyAppName}
VersionInfoProductVersion={#MyAppNumericVersion}
VersionInfoProductTextVersion={#MyAppVersion}
VersionInfoCompany={#MyAppPublisher}
VersionInfoDescription={#MyAppName} Setup - {#MyAppDescription}
VersionInfoCopyright={#MyAppCopyright}
VersionInfoOriginalFileName=Bullet-Setup-{#MyAppVersion}-x64.exe
UninstallDisplayName={#MyAppName} {#MyAppVersion}
DefaultDirName={commonpf}\{#MyAppName}
UsePreviousAppDir=no
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputDir=..\dist\installer
OutputBaseFilename=Bullet-Setup-{#MyAppVersion}-x64
SetupIconFile=..\assets\bullet.ico
LicenseFile=..\LICENSE
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
AppMutex=Local\Bullet_SingleInstance_bullet
CloseApplications=yes
RestartApplications=no
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "autostart"; Description: "{cm:AutoStartProgram,{#MyAppName}}"; GroupDescription: "{cm:AutoStartProgramGroupDescription}"; Flags: unchecked

[InstallDelete]
Type: filesandordirs; Name: "{localappdata}\Programs\Bullet"
Type: files; Name: "{app}\tools\*.orig"
Type: files; Name: "{app}\tools\*.bak"
Type: filesandordirs; Name: "{localappdata}\Bullet\overlay"

[Dirs]
Name: "{app}\tools"
Name: "{localappdata}\Bullet\state"
Name: "{localappdata}\Bullet\library"

[Files]
Source: "..\dist\bullet.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\bullet.ico"; DestDir: "{app}\assets"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\THIRD-PARTY-NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\party.json"; DestDir: "{localappdata}\Bullet\state"; Flags: onlyifdoesntexist

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\assets\bullet.ico"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\assets\bullet.ico"; Tasks: desktopicon

[Registry]
Root: HKLM; Subkey: "SOFTWARE\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers"; ValueType: none; ValueName: "{app}\{#MyAppExeName}"; Flags: deletevalue uninsdeletevalue
Root: HKCU; Subkey: "Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers"; ValueType: none; ValueName: "{app}\{#MyAppExeName}"; Flags: deletevalue uninsdeletevalue
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Bullet"; ValueData: """{app}\{#MyAppExeName}"""; Flags: uninsdeletevalue; Tasks: autostart
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Bullet"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent runasoriginaluser

[UninstallDelete]
Type: filesandordirs; Name: "{localappdata}\Bullet\logs"
Type: filesandordirs; Name: "{localappdata}\Bullet\state"
Type: filesandordirs; Name: "{localappdata}\Bullet\webview2"
Type: filesandordirs; Name: "{localappdata}\Bullet\overlay"
Type: filesandordirs; Name: "{localappdata}\Bullet\mods"
Type: filesandordirs; Name: "{localappdata}\Bullet\tools"
Type: files; Name: "{app}\tools\ltk_patcher_host.exe"
Type: files; Name: "{app}\tools\ltk_patcher_dll.dll"
Type: dirifempty; Name: "{app}\tools"
Type: dirifempty; Name: "{app}\assets"
Type: dirifempty; Name: "{app}"

[CustomMessages]
brazilianportuguese.DeleteUserContent=Remover também as skins e os mods personalizados salvos pelo Bullet?%n%n%1%n%nEscolha "Não" para mantê-los.
english.DeleteUserContent=Also remove the skins and custom mods saved by Bullet?%n%n%1%n%nChoose "No" to keep them.

[Code]
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
    RemoveDir(DataDir);
  end;
end;

