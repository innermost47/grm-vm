; Inno Setup Script for grm-vm
; Language: English

#define MyAppId "41b9ee0e-5b4c-47e4-aa49-821e8ef47048"
#define MyAppName "grm-vm"
#define MyAppVersion "0.2.0"
#define MyAppPublisher "Anthony Charretier"
#define MyAppURL "https://github.com/innermost47/grm-vm"
#define MyAppExeName "grm-app.exe"
#define MyCliExeName "grm.exe"

[Setup]
; Unique identifier for the application (do not change in future versions)
AppId={#MyAppId}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}

; Installation directories
DefaultDirName={userappdata}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes

; Output configuration
OutputDir=target\installer
OutputBaseFilename=grm-vm-{#MyAppVersion}-setup
SetupIconFile=assets\icon.ico
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern

; Uninstaller configuration
Uninstallable=yes
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName={#MyAppName}

; Architecture (64-bit Windows)
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "startmenuicon"; Description: "Create a Start Menu shortcut"; GroupDescription: "{cm:AdditionalIcons}"
Name: "addtopath"; Description: "Add CLI tool (grm) to user PATH"; GroupDescription: "System Integration:"

[Files]
; Rust release binaries built with `cargo build --release`
Source: "target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\release\{#MyCliExeName}"; DestDir: "{app}"; Flags: ignoreversion

; Application assets, styles, and built-in GRML libraries
Source: "apps\*"; DestDir: "{app}\apps"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "screens\*"; DestDir: "{app}\screens"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "style\*"; DestDir: "{app}\style"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "widgets\*"; DestDir: "{app}\widgets"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "assets\*"; DestDir: "{app}\assets"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "mind\*"; DestDir: "{app}\mind"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "data\*"; DestDir: "{app}\data"; Flags: ignoreversion recursesubdirs createallsubdirs

; License file (optional, uncomment if LICENSE exists at repository root)
; Source: "LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
; Desktop shortcut (optional, controlled by user task)
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; IconFilename: "{app}\assets\icon.ico"; Tasks: desktopicon

; Start Menu shortcuts (optional, controlled by user task)
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; IconFilename: "{app}\assets\icon.ico"; Tasks: startmenuicon
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"; IconFilename: "{app}\assets\icon.ico"; Tasks: startmenuicon

[Registry]
; Add installation directory to user PATH so `grm` can be run from any terminal (checked by default)
Root: HKA; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Tasks: addtopath; Check: NeedsAddPath('{app}')

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; WorkingDir: "{app}"; Flags: nowait postinstall skipifsilent

[Code]
function NeedsAddPath(Param: string): boolean;
var
  OrigPath: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', OrigPath)
  then begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + Param + ';', ';' + OrigPath + ';') = 0;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  AppDir: string;
  Answer: Integer;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    AppDir := ExpandConstant('{app}');
    if DirExists(AppDir) then
    begin
      Answer := MsgBox(
        'Do you want to delete all remaining files and data in "' + AppDir + '"?',
        mbConfirmation,
        MB_YESNO or MB_DEFBUTTON2
      );
      if Answer = IDYES then
      begin
        DelTree(AppDir, True, True, True);
      end;
    end;
  end;
end;