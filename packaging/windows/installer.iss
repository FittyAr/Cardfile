; Cardfile Windows installer (Inno Setup 6).
; Built in CI by .github/workflows/package.yml:
;   iscc /DAppVersion=2.0.0-beta.1 /DFileVersion=2.0.0 /DSourceDir=path\to\unzipped /DArch=x64 packaging\windows\installer.iss

#define AppName "Cardfile"
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
; VersionInfoVersion must be numeric (no "-alpha.1"): CI passes the version without
; its pre-release suffix.
#ifndef FileVersion
  #define FileVersion "0.0.0"
#endif
#ifndef Arch
  #define Arch "x64"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\target\release"
#endif
#define AppPublisher "FittyAr"
#define AppURL "https://github.com/FittyAr/Cardfile"
#define AppExeName "cardfile.exe"

[Setup]
; Keep this AppId: it identifies Cardfile for upgrades and uninstall (also for 1.x installs).
AppId={{9F7B2D1D-53C4-496F-B339-2274DB737CE8}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/issues
AppUpdatesURL={#AppURL}/releases
VersionInfoVersion={#FileVersion}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\out
OutputBaseFilename=Cardfile-{#AppVersion}-setup-{#Arch}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
#if Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
SetupIconFile=cardfile.ico
UninstallDisplayIcon={app}\{#AppExeName}
WizardImageFile=..\..\assets\logo_trasparente.png
WizardSmallImageFile=..\..\assets\logo_trasparente_1.png
LicenseFile=..\..\LICENSE-MIT
CloseApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "french"; MessagesFile: "compiler:Languages\French.isl"
Name: "german"; MessagesFile: "compiler:Languages\German.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\{#AppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\dictionaries\*"; DestDir: "{app}\dictionaries"; Flags: ignoreversion recursesubdirs createallsubdirs skipifsourcedoesntexist
Source: "{#SourceDir}\THIRD_PARTY_LICENSES.html"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#SourceDir}\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExeName}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(AppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; User data lives in %APPDATA%\FittyAr\Cardfile and is intentionally kept.
Type: filesandordirs; Name: "{app}\dictionaries"
