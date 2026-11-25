!macro NSIS_HOOK_PREINSTALL
  ${If} ${FileExists} "$LOCALAPPDATA\Remote Buddy\remote-buddy-desktop.exe"
    nsExec::ExecToLog '"$SYSDIR\taskkill.exe" /F /IM remote-buddy-desktop.exe'
    Pop $0
    Sleep 1500
    ${If} ${FileExists} "$LOCALAPPDATA\Remote Buddy\uninstall.exe"
      ExecWait '"$LOCALAPPDATA\Remote Buddy\uninstall.exe" /S _?=$LOCALAPPDATA\Remote Buddy'
    ${EndIf}
    Delete "$LOCALAPPDATA\Remote Buddy\remote-buddy-desktop.exe"
    Delete "$LOCALAPPDATA\Remote Buddy\uninstall.exe"
    RMDir "$LOCALAPPDATA\Remote Buddy"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Remote Buddy"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  SetShellVarContext current
  CreateShortCut "$DESKTOP\Remote Buddy.lnk" "$INSTDIR\remote-buddy-desktop.exe" "" "$INSTDIR\remote-buddy-desktop.exe" 0
  SetShellVarContext all
!macroend
