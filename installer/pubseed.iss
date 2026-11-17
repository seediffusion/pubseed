; Builds Pubseed's setup program with Inno Setup 6.
;
;   cargo build --release
;   iscc installer\pubseed.iss
;
; The setup program is written to target\installer.

#define AppName "Pubseed"
#define AppExe "pubseed.exe"
; The version shown in Windows comes from the program itself, which gets it
; from Cargo.toml.
#define AppVersion GetStringFileInfo("..\target\release\" + AppExe, "ProductVersion")

[Setup]
; Identifies Pubseed to Windows so a newer setup upgrades the installed copy.
; Never change it.
AppId={{5CC43BE3-6F66-4DFB-A036-2A076FB87A72}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=Seediffusion
AppPublisherURL=https://github.com/seediffusion/pubseed
AppSupportURL=https://github.com/seediffusion/pubseed/issues
AppUpdatesURL=https://github.com/seediffusion/pubseed/releases
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
DisableDirPage=auto
; Shows the MIT licence, to be accepted before installing.
LicenseFile=..\LICENSE
OutputDir=..\target\installer
OutputBaseFilename=pubseed-setup-{#AppVersion}
UninstallDisplayName={#AppName}
UninstallDisplayIcon={app}\{#AppExe}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Installs for everyone by default, as most Windows programs do, and lets
; someone without administrator rights install it for themselves instead.
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog
; Asks to close a running copy rather than failing to replace it.
CloseApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\THIRD-PARTY-NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
