!macro NSIS_HOOK_PREINSTALL
  ${If} ${FileExists} "$LOCALAPPDATA\Remote Buddy\remote-buddy-desktop.exe"
    ${If} ${FileExists} "$LOCALAPPDATA\Remote Buddy\uninstall.exe"
      ExecWait '"$LOCALAPPDATA\Remote Buddy\uninstall.exe" /S _?=$LOCALAPPDATA\Remote Buddy'
    ${Else}
      Delete "$LOCALAPPDATA\Remote Buddy\remote-buddy-desktop.exe"
    ${EndIf}
  ${EndIf}
!macroend
