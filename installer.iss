#define AppName "Cardfile"
#ifndef AppVersion
  #define AppVersion "1.0.1"
#endif
#define AppPublisher "FittyAr"
#define AppURL "https://github.com/FittyAr/Cardfile"
#define AppExeName "Cardfile.exe"

[Setup]
AppId={{9F7B2D1D-53C4-496F-B339-2274DB737CE8}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}
AppUpdatesURL={#AppURL}
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
OutputBaseFilename=Cardfile-setup-{#AppVersion}-x64
Compression=lzma
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64
ArchitecturesInstallIn64BitMode=x64
SetupIconFile=assets\icon.ico
UninstallDisplayIcon={app}\assets\icon.ico
WizardImageFile=assets\logo_trasparente.png
WizardSmallImageFile=assets\logo_trasparente_2.png

[Languages]
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\Cardfile.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\config.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\lang\*"; DestDir: "{app}\lang"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#SourceDir}\assets\*"; DestDir: "{app}\assets"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExeName}"; IconFilename: "{app}\assets\icon.ico"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon; IconFilename: "{app}\assets\icon.ico"

[Run]
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(AppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
