#define AppName "Thumbpad"
#ifndef AppVersion
  #define AppVersion "0.1.10"
#endif
#ifndef BuildDir
  #define BuildDir "..\target\release"
#endif
[Setup]
AppId={{C2854D5D-78B0-4D9E-A4E4-19B03D497E97}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=Thumbpad contributors
AppPublisherURL=https://github.com/kathoc/thumbpad
AppSupportURL=https://github.com/kathoc/thumbpad/issues
AppUpdatesURL=https://github.com/kathoc/thumbpad/releases
DefaultDirName={localappdata}\Programs\Thumbpad
DefaultGroupName=Thumbpad
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.22000
OutputDir=..\dist
OutputBaseFilename=thumbpad-setup-x64
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
UninstallDisplayIcon={app}\thumbpad.exe
LicenseFile=..\LICENSE
SetupLogging=yes
[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"
[Tasks]
Name: "startup"; Description: "Start Thumbpad with Windows"; Flags: unchecked
[Files]
Source: "{#BuildDir}\thumbpad.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\TESTING.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\VALIDATION.md"; DestDir: "{app}"; Flags: ignoreversion
[Icons]
Name: "{group}\Thumbpad"; Filename: "{app}\thumbpad.exe"
Name: "{group}\Uninstall Thumbpad"; Filename: "{uninstallexe}"
[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Thumbpad"; ValueData: """{app}\thumbpad.exe"" --background"; Tasks: startup; Flags: uninsdeletevalue
[Run]
Filename: "{app}\thumbpad.exe"; WorkingDir: "{app}"; Description: "Launch Thumbpad"; Flags: nowait postinstall skipifsilent
Filename: "{app}\thumbpad.exe"; Parameters: "--background"; WorkingDir: "{app}"; Flags: nowait runhidden; Check: IsUpgrade
[UninstallRun]
Filename: "{app}\thumbpad.exe"; Parameters: "--quit"; Flags: runhidden waituntilterminated; RunOnceId: "CloseThumbpad"
[UninstallDelete]
Type: files; Name: "{app}\README.md"
[Code]
function IsUpgrade: Boolean;
var
  I: Integer;
begin
  Result := False;
  for I := 1 to ParamCount do
    if CompareText(ParamStr(I), '/UPGRADE') = 0 then Result := True;
end;
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  ResultCode: Integer;
begin
  Result := '';
  if FileExists(ExpandConstant('{app}\thumbpad.exe')) then
  begin
    if not Exec(ExpandConstant('{app}\thumbpad.exe'), '--quit', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
      Result := 'Could not close Thumbpad. Please exit it from the tray and retry.';
    Sleep(750);
  end;
end;
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'Thumbpad');
end;
