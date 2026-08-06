!macro NSIS_HOOK_PREINSTALL

  ; The app lives in the tray, and Windows will not let the installer replace
  ; an exe that is still running. Close it under either name -- it shipped as
  ; BootsAutoClicker before it was called Syntax.
  nsExec::Exec 'taskkill /F /IM Syntax.exe'
  Pop $0
  nsExec::Exec 'taskkill /F /IM BootsAutoClicker.exe'
  Pop $0
  Sleep 400

  ; An upgrade reuses the old folder, so the old exe would sit there forever.
  Delete "$INSTDIR\BootsAutoClicker.exe"
!macroend
