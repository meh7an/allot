; Setup for Allot: a per-user install that needs no admin. The files themselves are
; placed by `allot.exe install`, which already replaces binaries that running sessions hold open
; and sets up Claude Code, the logon task, and the tray app; this setup unpacks the build,
; runs it with the choices made in the wizard, and adds the Apps & features entry whose
; uninstaller runs `allot.exe uninstall`.
;
; Build: ISCC.exe /DAppVersion=<x.y.z> /DPayload=<dir> /O<output dir> allot.iss
; where <dir> holds allot.exe, allotd.exe, allot-mcp.exe, and the tray app's publish folder as tray\.

#ifndef AppVersion
  #error Pass /DAppVersion=<x.y.z>
#endif
#ifndef Payload
  #error Pass /DPayload=<folder with allot.exe, allotd.exe, allot-mcp.exe and tray\>
#endif

#define AppName "Allot"
#define Home "{%USERPROFILE}\.allot"

[Setup]
; Fixed for every release, so an upgrade replaces the install instead of adding a second one.
AppId={{7C1E4B2A-3D5F-4E8B-9A61-2F0C5D8E4B17}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Mehran
AppPublisherURL=https://github.com/meh7an/allot
AppSupportURL=https://github.com/meh7an/allot/issues
AppUpdatesURL=https://github.com/meh7an/allot/releases
; Only the copy of allot.exe the uninstaller runs lives here; the install itself is in
; %USERPROFILE%\.allot.
DefaultDirName={localappdata}\Programs\{#AppName}
; Not the folder of an install from before the rename to Allot, which [InstallDelete] removes.
UsePreviousAppDir=no
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
; allot.exe install quits the tray app and renames files in use aside itself.
CloseApplications=no
RestartApplications=no
OutputBaseFilename=allot-{#AppVersion}-setup
SetupIconFile=..\tray\src\Allot.Tray\Assets\AppIcon.ico
UninstallDisplayIcon={#Home}\bin\tray\allot-tray.exe
UninstallDisplayName={#AppName}
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
LicenseFile=..\LICENSE-MIT

[Tasks]
Name: claude; Description: "Set up Claude Code: the MCP server, hooks, resource-guard skill, and CLAUDE.md rule"
Name: launchatlogon; Description: "Start the tray app when I sign in"

[Files]
Source: "{#Payload}\*"; DestDir: "{tmp}\payload"; Excludes: "*.pdb"; Flags: recursesubdirs ignoreversion
Source: "{#Payload}\allot.exe"; DestDir: "{app}"; Flags: ignoreversion

; What a setup from before the rename to Allot, Resources Helper, left: its folder with its
; uninstaller, and its Start menu entry. allot.exe install takes over the rest.
[InstallDelete]
Type: filesandordirs; Name: "{localappdata}\Programs\Resources Helper"
Type: files; Name: "{userprograms}\Resources Helper.lnk"

[UninstallDelete]
Type: files; Name: "{app}\install.log"

[Icons]
Name: "{userprograms}\{#AppName}"; Filename: "{#Home}\bin\tray\allot-tray.exe"; Comment: "Shows and stops the jobs of your Claude Code sessions"

[Code]
var
  InstallLog: String;

// Runs allot.exe with its output in a log file, and returns its exit code.
function RunRh(const Exe, Arguments, Log: String): Integer;
var
  ResultCode: Integer;
begin
  if not Exec(ExpandConstant('{cmd}'),
    '/S /C ""' + Exe + '" ' + Arguments + ' > "' + Log + '" 2>&1"',
    ExtractFileDir(Exe), SW_HIDE, ewWaitUntilTerminated, ResultCode) then
    ResultCode := -1;
  Result := ResultCode;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Payload, Arguments: String;
begin
  if CurStep <> ssPostInstall then
    exit;
  Payload := ExpandConstant('{tmp}\payload');
  Arguments := 'install --from "' + Payload + '"';
  if WizardIsTaskSelected('launchatlogon') then
    Arguments := Arguments + ' --launch-at-logon on'
  else
    Arguments := Arguments + ' --launch-at-logon off';
  if not WizardIsTaskSelected('claude') then
    Arguments := Arguments + ' --no-claude';
  InstallLog := ExpandConstant('{app}\install.log');
  WizardForm.StatusLabel.Caption := 'Installing and setting up Allot...';
  if RunRh(Payload + '\allot.exe', Arguments, InstallLog) <> 0 then
    SuppressibleMsgBox('Allot could not be installed. What happened is in' + #13#10
      + InstallLog, mbError, MB_OK, IDOK);
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  if CurPageID = wpFinished then
    WizardForm.FinishedLabel.Caption := WizardForm.FinishedLabel.Caption + #13#10#13#10
      + 'Restart open Claude Code sessions: they load the MCP server and CLAUDE.md when they '
      + 'start. What the setup did is in ' + InstallLog + '.';
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Arguments: String;
begin
  if CurUninstallStep <> usUninstall then
    exit;
  Arguments := 'uninstall';
  if not UninstallSilent() and (MsgBox('Also delete the lease history, settings, and logs in '
    + ExpandConstant('{#Home}') + '?', mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES) then
    Arguments := Arguments + ' --purge';
  // The log goes to the temp folder, because the uninstall removes {app}.
  RunRh(ExpandConstant('{app}\allot.exe'), Arguments, ExpandConstant('{tmp}\uninstall.log'));
end;
