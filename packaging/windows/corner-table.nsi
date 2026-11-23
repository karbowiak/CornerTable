Unicode true
Name "Corner Table"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\Corner Table"
InstallDirRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\CornerTable" "InstallLocation"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!include "x64.nsh"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\CornerTable"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Function .onInit
    ${If} ${RunningX64}
        SetRegView 64
    ${Else}
        MessageBox MB_OK|MB_ICONSTOP "Corner Table requires 64-bit Windows." /SD IDOK
        SetErrorLevel 1
        Quit
    ${EndIf}
FunctionEnd

Section "Corner Table"
    SetOutPath "$INSTDIR"
    File "${STAGE}\corner-table.exe"
    File "${STAGE}\README.md"
    File "${STAGE}\CHANGELOG.md"
    File /r "${STAGE}\Third-Party Notices"
    WriteUninstaller "$INSTDIR\Uninstall.exe"
    CreateShortcut "$SMPROGRAMS\Corner Table.lnk" "$INSTDIR\corner-table.exe"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "Corner Table"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\corner-table.exe"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
    WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
    WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
    SetRegView 64
    Delete "$SMPROGRAMS\Corner Table.lnk"
    Delete "$INSTDIR\corner-table.exe"
    Delete "$INSTDIR\README.md"
    Delete "$INSTDIR\CHANGELOG.md"
    RMDir /r "$INSTDIR\Third-Party Notices"
    Delete "$INSTDIR\Uninstall.exe"
    RMDir "$INSTDIR"
    DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
