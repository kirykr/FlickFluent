; Windows installer hook (Tauri 2 NSIS). Runs the hardware check after the files are copied.
; If the computer is below the minimum spec, show the message and remove what was just installed.
; Exit code 10 means "blocked" (see core/src/cli.rs).
;
; UNTESTED: written on macOS. Verify on a Windows machine (task T058).

!macro NSIS_HOOK_POSTINSTALL
  nsExec::ExecToStack '"$INSTDIR\${MAINBINARYNAME}.exe" --check-hardware --message'
  Pop $0
  Pop $1
  ${If} $0 == 10
    MessageBox MB_OK|MB_ICONSTOP "$1"
    ExecWait '"$INSTDIR\uninstall.exe" /S _?=$INSTDIR'
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"
    Abort
  ${EndIf}
!macroend
