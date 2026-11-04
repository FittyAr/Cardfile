; Cardfile Windows installer (Inno Setup 7).
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
DefaultDirName={code:DefaultDirectory}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=commandline
Uninstallable=IsInstalled
CreateUninstallRegKey=IsInstalled
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
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked; Check: IsInstalled

[Files]
Source: "{#SourceDir}\{#AppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\dictionaries\*"; DestDir: "{app}\dictionaries"; Flags: ignoreversion recursesubdirs createallsubdirs skipifsourcedoesntexist
Source: "{#SourceDir}\THIRD_PARTY_LICENSES.html"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#SourceDir}\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Check: IsInstalled
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}"; Tasks: desktopicon; Check: IsInstalled

[Run]
Filename: "{app}\{#AppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(AppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
; Cardfile's self-updater runs this installer with /VERYSILENT /RELAUNCH after quitting:
; start the app again, as the user who launched the update (not elevated).
Filename: "{app}\{#AppExeName}"; Flags: nowait runasoriginaluser skipifnotsilent; Check: ShouldRelaunch

[UninstallDelete]
; User data lives in %APPDATA%\FittyAr\Cardfile and is intentionally kept.
Type: filesandordirs; Name: "{app}\dictionaries"
Type: files; Name: "{app}\installed.flag"

[CustomMessages]
ModeTitle=Choose how to use Cardfile
ModeDescription=Install on this computer or prepare a portable folder.
ModePrompt=A portable folder can be placed on a USB drive or in OneDrive. It keeps its data beside the program.
InstalledMode=Install on this computer (current user)
PortableMode=Create a portable copy
AllUsersMode=Install for all users (administrator)
ModeConflict=This folder belongs to another installation mode. Choose a different folder. Existing data will not be converted or overwritten.
PortableAdmin=A portable copy must be created without administrator privileges. Restart this installer normally.
InvalidMode=Invalid /MODE. Use installed or portable.
spanish.ModeTitle=Elegí cómo usar Cardfile
spanish.ModeDescription=Instalá en este equipo o prepará una carpeta portable.
spanish.ModePrompt=La carpeta portable puede estar en un pendrive o en OneDrive. Los datos se guardan junto al programa.
spanish.InstalledMode=Instalar en este equipo (usuario actual)
spanish.PortableMode=Crear una copia portable
spanish.AllUsersMode=Instalar para todos los usuarios (administrador)
spanish.ModeConflict=Esta carpeta pertenece a otro modo de instalación. Elegí otra carpeta. Los datos existentes no se convertirán ni sobrescribirán.
spanish.PortableAdmin=La copia portable debe crearse sin permisos de administrador. Volvé a iniciar este instalador normalmente.
spanish.InvalidMode=El valor de /MODE no es válido. Usá installed o portable.
brazilianportuguese.ModeTitle=Escolha como usar o Cardfile
brazilianportuguese.ModeDescription=Instale neste computador ou prepare uma pasta portátil.
brazilianportuguese.ModePrompt=A pasta portátil pode ficar em um pendrive ou no OneDrive. Os dados são salvos junto ao programa.
brazilianportuguese.InstalledMode=Instalar neste computador (usuário atual)
brazilianportuguese.PortableMode=Criar uma cópia portátil
brazilianportuguese.AllUsersMode=Instalar para todos os usuários (administrador)
brazilianportuguese.ModeConflict=Esta pasta pertence a outro modo de instalação. Escolha outra pasta. Os dados existentes não serão convertidos nem substituídos.
brazilianportuguese.PortableAdmin=A cópia portátil deve ser criada sem privilégios de administrador. Reinicie este instalador normalmente.
brazilianportuguese.InvalidMode=Valor inválido de /MODE. Use installed ou portable.
french.ModeTitle=Choisissez comment utiliser Cardfile
french.ModeDescription=Installez sur cet ordinateur ou préparez un dossier portable.
french.ModePrompt=Le dossier portable peut se trouver sur une clé USB ou dans OneDrive. Les données sont enregistrées à côté du programme.
french.InstalledMode=Installer sur cet ordinateur (utilisateur actuel)
french.PortableMode=Créer une copie portable
french.AllUsersMode=Installer pour tous les utilisateurs (administrateur)
french.ModeConflict=Ce dossier correspond à un autre mode d’installation. Choisissez un autre dossier. Les données existantes ne seront ni converties ni écrasées.
french.PortableAdmin=La copie portable doit être créée sans droits d’administrateur. Relancez cet installateur normalement.
french.InvalidMode=Valeur /MODE incorrecte. Utilisez installed ou portable.
german.ModeTitle=Wählen Sie, wie Sie Cardfile verwenden möchten
german.ModeDescription=Auf diesem Computer installieren oder einen portablen Ordner erstellen.
german.ModePrompt=Der portable Ordner kann auf einem USB-Stick oder in OneDrive liegen. Die Daten werden neben dem Programm gespeichert.
german.InstalledMode=Auf diesem Computer installieren (aktueller Benutzer)
german.PortableMode=Portable Kopie erstellen
german.AllUsersMode=Für alle Benutzer installieren (Administrator)
german.ModeConflict=Dieser Ordner gehört zu einem anderen Installationsmodus. Wählen Sie einen anderen Ordner. Vorhandene Daten werden weder konvertiert noch überschrieben.
german.PortableAdmin=Eine portable Kopie muss ohne Administratorrechte erstellt werden. Starten Sie dieses Installationsprogramm normal neu.
german.InvalidMode=Ungültiger /MODE-Wert. Verwenden Sie installed oder portable.

[Code]
var
  ModePage: TInputOptionWizardPage;
  SelectedMode: String;
  ModeParameter: String;
  LastMode: String;

function IsInstalled: Boolean;
begin
  Result := SelectedMode <> 'portable';
end;

function DefaultDirectory(Param: String): String;
begin
  if SelectedMode = 'portable' then Result := ExpandConstant('{userdocs}\Cardfile portable')
  else if IsAdminInstallMode then Result := ExpandConstant('{autopf}\Cardfile')
  else Result := ExpandConstant('{localappdata}\Programs\FittyAr\Cardfile');
end;

function InitializeSetup: Boolean;
begin
  ModeParameter := Lowercase(ExpandConstant('{param:MODE|}'));
  SelectedMode := ModeParameter;
  if SelectedMode = '' then SelectedMode := 'installed';
  Result := (SelectedMode = 'installed') or (SelectedMode = 'portable');
  if not Result then MsgBox(CustomMessage('InvalidMode'), mbError, MB_OK);
  if Result and (SelectedMode = 'portable') and IsAdminInstallMode then begin
    MsgBox(CustomMessage('PortableAdmin'), mbError, MB_OK);
    Result := False;
  end;
end;

procedure InitializeWizard;
begin
  ModePage := CreateInputOptionPage(wpWelcome, CustomMessage('ModeTitle'),
    CustomMessage('ModeDescription'), CustomMessage('ModePrompt'), True, False);
  ModePage.Add(CustomMessage('InstalledMode'));
  ModePage.Add(CustomMessage('PortableMode'));
  ModePage.Add(CustomMessage('AllUsersMode'));
  if SelectedMode = 'portable' then ModePage.SelectedValueIndex := 1
  else if IsAdminInstallMode then ModePage.SelectedValueIndex := 2
  else ModePage.SelectedValueIndex := 0;
  LastMode := '';
end;

function ShouldSkipPage(PageID: Integer): Boolean;
begin
  Result := (PageID = ModePage.ID) and (ModeParameter <> '');
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  if CurPageID = wpSelectDir then begin
    if ModePage.SelectedValueIndex = 1 then SelectedMode := 'portable'
    else SelectedMode := 'installed';
    if (LastMode <> SelectedMode) and (ExpandConstant('{param:DIR|}') = '') then begin
      if SelectedMode = 'portable' then
        WizardForm.DirEdit.Text := ExpandConstant('{userdocs}\Cardfile portable')
      else if IsAdminInstallMode then
        WizardForm.DirEdit.Text := ExpandConstant('{autopf}\Cardfile')
      else
        WizardForm.DirEdit.Text := ExpandConstant('{localappdata}\Programs\FittyAr\Cardfile');
    end;
    LastMode := SelectedMode;
  end;
end;

function NextButtonClick(CurPageID: Integer): Boolean;
var
  ResultCode: Integer;
begin
  Result := True;
  if CurPageID = ModePage.ID then begin
    if (ModePage.SelectedValueIndex = 2) and not IsAdminInstallMode then begin
      Result := False;
      if ShellExec('runas', ExpandConstant('{srcexe}'), '/MODE=installed /ALLUSERS', '', SW_SHOWNORMAL, ewNoWait, ResultCode) then
        WizardForm.Close;
    end;
    if (ModePage.SelectedValueIndex = 1) and IsAdminInstallMode then begin
      MsgBox(CustomMessage('PortableAdmin'), mbError, MB_OK);
      Result := False;
    end;
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Destination: String;
  HasPortable, HasInstalled: Boolean;
begin
  Result := '';
  Destination := ExpandConstant('{app}');
  HasPortable := FileExists(Destination + '\portable.flag') or FileExists(Destination + '\data\cardfile.db');
  HasInstalled := FileExists(Destination + '\installed.flag') or FileExists(Destination + '\unins000.exe');
  if ((SelectedMode = 'portable') and HasInstalled) or
     ((SelectedMode = 'installed') and HasPortable) then
    Result := CustomMessage('ModeConflict');
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then begin
    if SelectedMode = 'portable' then
      SaveStringToFile(ExpandConstant('{app}\portable.flag'), 'Cardfile portable' + #13#10, False)
    else
      SaveStringToFile(ExpandConstant('{app}\installed.flag'), 'Cardfile installed' + #13#10, False);
  end;
end;

// True when the command line has /RELAUNCH (added by cardfile-updater).
function ShouldRelaunch: Boolean;
var
  I: Integer;
begin
  Result := False;
  for I := 1 to ParamCount do
    if CompareText(ParamStr(I), '/RELAUNCH') = 0 then
      Result := True;
end;
