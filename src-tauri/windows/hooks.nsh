; KaiSync installer hooks (included by Tauri's NSIS template).
; The template already creates/removes the desktop shortcut ($DESKTOP\KaiSync.lnk).

!macro NSIS_HOOK_POSTUNINSTALL
  ; Remove the "Start with Windows" entry the app registers (tauri-plugin-autostart).
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "KaiSync"
  ${EndIf}
!macroend
