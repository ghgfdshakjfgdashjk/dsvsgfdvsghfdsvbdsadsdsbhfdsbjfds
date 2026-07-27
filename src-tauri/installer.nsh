!macro NSIS_HOOK_PREINSTALL

  ${If} ${RunningX64}
    SetRegView 64
    Call RemoveLegacyInstall
    SetRegView 32
  ${EndIf}
  Call RemoveLegacyInstall

  RMDir /r "$APPDATA\com.kylei.boots-autoclicker"
!macroend

Function RemoveLegacyInstall

  ReadRegStr $R0 HKCU \
    "Software\Microsoft\Windows\CurrentVersion\Uninstall\com.kylei.boots-autoclicker" \
    "UninstallString"
  ${If} $R0 == ""
    ReadRegStr $R0 HKLM \
      "Software\Microsoft\Windows\CurrentVersion\Uninstall\com.kylei.boots-autoclicker" \
      "UninstallString"
  ${EndIf}

  ${If} $R0 != ""

    StrCpy $R1 $R0 1
    ${If} $R1 == '"'
      StrLen $R2 $R0
      IntOp $R2 $R2 - 2
      StrCpy $R0 $R0 $R2 1
    ${EndIf}

    ${GetParent} "$R0" $R2

    nsExec::Exec 'taskkill /F /IM BootsAutoClicker.exe'
    Pop $0
    Sleep 400

    ExecWait '"$R0" /S _?=$R2' $0
    Sleep 400

    Delete "$R0"
    RMDir "$R2"
  ${EndIf}

  DeleteRegKey HKCU \
    "Software\Microsoft\Windows\CurrentVersion\Uninstall\com.kylei.boots-autoclicker"
  DeleteRegKey HKLM \
    "Software\Microsoft\Windows\CurrentVersion\Uninstall\com.kylei.boots-autoclicker"
FunctionEnd
