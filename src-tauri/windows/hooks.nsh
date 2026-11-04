; Prepared exit and an exclusive profile guard precede changes to installed files.
Var IslandMaintenanceHandle
Var IslandMaintenancePath

!macro ISLAND_MAINTENANCE_ERROR text
  SetErrorLevel 10
  IfSilent +2
    MessageBox MB_OK|MB_ICONSTOP "${text}"
  Quit
!macroend

; RequestExecutionLevel user does not prevent a caller from explicitly elevating.
!macro ISLAND_REQUIRE_STANDARD_TOKEN
  System::Call 'kernel32::GetCurrentProcess() p .r0'
  System::Call 'advapi32::OpenProcessToken(p r0, i 0x0008, *p .r1) i .r2'
  ${If} $2 = 0
    !insertmacro ISLAND_MAINTENANCE_ERROR "Die Rechte des aktuellen Prozesses konnten nicht geprüft werden."
  ${EndIf}
  ; TokenElevation is a four-byte TOKEN_ELEVATION structure.
  System::Call 'advapi32::GetTokenInformation(p r1, i 20, *i .r3, i 4, *i .r4) i .r2'
  System::Call 'kernel32::CloseHandle(p r1)'
  ${If} $2 = 0
    !insertmacro ISLAND_MAINTENANCE_ERROR "Die Rechte des aktuellen Prozesses konnten nicht geprüft werden."
  ${EndIf}
  ${If} $3 != 0
    !insertmacro ISLAND_MAINTENANCE_ERROR "Bitte starten Sie Setup oder Deinstallation ohne erhöhte Administratorrechte."
  ${EndIf}
!macroend

!macro ISLAND_PREPARE
  ; File embeds the helper separately in the installer and uninstaller sections.
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  ClearErrors
  File /oname=island-maintenance.exe "${MAINBINARYSRCPATH}"
  ${If} ${Errors}
    !insertmacro ISLAND_MAINTENANCE_ERROR "Das sichere Beenden konnte nicht vorbereitet werden. Es wurden keine Programmdateien geändert."
  ${EndIf}
  ClearErrors
  ExecWait '"$PLUGINSDIR\island-maintenance.exe" --prepare-install "$INSTDIR\${MAINBINARYNAME}.exe"' $0
  ${If} ${Errors}
    !insertmacro ISLAND_MAINTENANCE_ERROR "Die Insel konnte nicht regulär beendet werden. Bitte die Insel über das Tray beenden und erneut versuchen."
  ${EndIf}
  ${If} $0 != 0
    !insertmacro ISLAND_MAINTENANCE_ERROR "Die Insel konnte nicht sicher beendet werden. Installation oder Deinstallation wurde angehalten; die Programmdateien bleiben erhalten."
  ${EndIf}
  !insertmacro ISLAND_ACQUIRE "$APPDATA" "${BUNDLEID}"
!macroend

!macro ISLAND_ACQUIRE parent profile
  ; Match instance::maintenance_path and Tauri's current-user roaming profile.
  ; This file stays outside the optional app-data removal directory.
  System::Call 'kernel32::GetCurrentProcessId() i .r0'
  System::Call 'kernel32::ProcessIdToSessionId(i r0, *i .r1) i .r2'
  ${If} $2 = 0
    !insertmacro ISLAND_MAINTENANCE_ERROR "Die aktuelle Windows-Sitzung konnte nicht geprüft werden."
  ${EndIf}
  StrCpy $IslandMaintenancePath "${parent}\.${profile}-maintenance-$1.lock"
  System::Call 'kernel32::CreateFileW(w "$IslandMaintenancePath", i 0xC0000000, i 0, p 0, i 4, i 0x80, p 0) p .r0'
  ${If} $0 P= -1
    !insertmacro ISLAND_MAINTENANCE_ERROR "Die Insel wurde inzwischen erneut gestartet oder ein anderer Installer ist aktiv. Bitte beenden und erneut versuchen."
  ${EndIf}
  StrCpy $IslandMaintenanceHandle $0
!macroend

!macro ISLAND_RELEASE
  ${If} $IslandMaintenanceHandle != ""
    System::Call 'kernel32::CloseHandle(p $IslandMaintenanceHandle)'
    StrCpy $IslandMaintenanceHandle ""
    Delete "$IslandMaintenancePath"
    ClearErrors
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro ISLAND_PREPARE
!macroend
!macro NSIS_HOOK_POSTINSTALL
  !insertmacro ISLAND_RELEASE
!macroend
!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro ISLAND_PREPARE
!macroend
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    ClearErrors
    ExecWait '"$PLUGINSDIR\island-maintenance.exe" --finish-uninstall "$INSTDIR\${MAINBINARYNAME}.exe"' $0
    ${If} ${Errors}
      !insertmacro ISLAND_MAINTENANCE_ERROR "Der Deinstallationsabschluss konnte nicht gestartet werden. Bitte den Autostart der Insel prüfen."
    ${EndIf}
    ${If} $0 != 0
      !insertmacro ISLAND_MAINTENANCE_ERROR "Der eigene Autostarteintrag konnte nicht bestätigt entfernt werden. Bitte den Autostart der Insel prüfen."
    ${EndIf}
  ${EndIf}
  !insertmacro ISLAND_RELEASE
!macroend
