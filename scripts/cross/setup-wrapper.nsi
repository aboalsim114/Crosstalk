; Crosstalk-setup.exe: a windowless NSIS shell around the real installer
; (src-installer, "Crosstalk Installer" design). It
;   1. installs Microsoft Edge WebView2 if it is missing (the installer's UI needs it),
;   2. unpacks crosstalk-setup.exe and WebView2Loader.dll in a temporary folder and runs it,
;   3. once Crosstalk is installed (exit code 0), writes the uninstaller and the
;      "Applications et fonctionnalités" entry.
; /S installs silently (crosstalk-setup.exe --silent).
;
; Defines (scripts/cross-build-windows-gnu.sh): VERSION, OUTFILE, SETUP_EXE,
; LOADER, ICON, FILES_NSH (one Delete per installed file), APP_KB.

Unicode true
SetCompressor /SOLID lzma
RequestExecutionLevel user
SilentInstall silent
SilentUnInstall silent

!include LogicLib.nsh
!include FileFunc.nsh
!include x64.nsh

!define APPNAME "Crosstalk"
!define APPKEY "Software\Crosstalk"
!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Crosstalk"
!define WEBVIEW2GUID "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

Name "${APPNAME}"
OutFile "${OUTFILE}"
Icon "${ICON}"
UninstallIcon "${ICON}"
InstallDir "$LOCALAPPDATA\Crosstalk"
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${APPNAME}"
VIAddVersionKey "FileDescription" "Installation de Crosstalk"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "Crosstalk contributors"

Var Quiet

Function .onInit
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/S" $1
  ${If} ${Errors}
    StrCpy $Quiet 0
  ${Else}
    StrCpy $Quiet 1
  ${EndIf}
FunctionEnd

Function HasWebView2
  ${If} ${RunningX64}
    ReadRegStr $0 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\${WEBVIEW2GUID}" "pv"
  ${Else}
    ReadRegStr $0 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WEBVIEW2GUID}" "pv"
  ${EndIf}
  ${If} $0 == ""
  ${OrIf} $0 == "0.0.0.0"
    ReadRegStr $0 HKCU "SOFTWARE\Microsoft\EdgeUpdate\Clients\${WEBVIEW2GUID}" "pv"
  ${EndIf}
  ${If} $0 == "0.0.0.0"
    StrCpy $0 ""
  ${EndIf}
FunctionEnd

Function InstallWebView2
  ${If} $Quiet == 0
    MessageBox MB_OKCANCEL|MB_ICONINFORMATION "Crosstalk a besoin du composant Microsoft Edge WebView2, absent de ce PC.$\n$\nIl va être téléchargé depuis le site de Microsoft (environ 2 Mo) puis installé automatiquement." IDOK +2
    Quit
    Banner::show /NOUNLOAD "Installation de Microsoft Edge WebView2…"
  ${EndIf}
  StrCpy $1 "$PLUGINSDIR\MicrosoftEdgeWebview2Setup.exe"
  NSISdl::download /TIMEOUT=30000 "https://go.microsoft.com/fwlink/p/?LinkId=2124703" "$1"
  Pop $0
  ${If} $0 != "success"
    nsExec::Exec `powershell -NoProfile -ExecutionPolicy Bypass -Command "[Net.ServicePointManager]::SecurityProtocol='Tls12'; Invoke-WebRequest -UseBasicParsing -Uri 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile '$1'"`
    Pop $0
  ${EndIf}
  ${IfNot} ${FileExists} "$1"
    ${If} $Quiet == 0
      Banner::destroy
      MessageBox MB_ICONSTOP "Téléchargement de WebView2 impossible. Vérifiez la connexion Internet, puis relancez l'installation."
    ${EndIf}
    SetErrorLevel 2
    Quit
  ${EndIf}
  ExecWait '"$1" /silent /install' $0
  ${If} $Quiet == 0
    Banner::destroy
  ${EndIf}
  ${If} $0 <> 0
    ${If} $Quiet == 0
      MessageBox MB_ICONSTOP "L'installation de WebView2 a échoué (code $0)."
    ${EndIf}
    SetErrorLevel 2
    Quit
  ${EndIf}
FunctionEnd

Section
  InitPluginsDir
  Call HasWebView2
  ${If} $0 == ""
    Call InstallWebView2
  ${EndIf}

  SetOutPath "$PLUGINSDIR\setup"
  File "/oname=crosstalk-setup.exe" "${SETUP_EXE}"
  File "/oname=WebView2Loader.dll" "${LOADER}"
  ${If} $Quiet == 1
    ExecWait '"$PLUGINSDIR\setup\crosstalk-setup.exe" --silent' $0
  ${Else}
    ExecWait '"$PLUGINSDIR\setup\crosstalk-setup.exe"' $0
  ${EndIf}
  SetOutPath "$TEMP"

  ${If} $0 == 0
    ReadRegStr $INSTDIR HKCU "${APPKEY}" "InstallDir"
    ${If} $INSTDIR != ""
      WriteUninstaller "$INSTDIR\uninstall.exe"
      WriteRegStr HKCU "${UNINSTKEY}" "DisplayName" "${APPNAME}"
      WriteRegStr HKCU "${UNINSTKEY}" "DisplayVersion" "${VERSION}"
      WriteRegStr HKCU "${UNINSTKEY}" "DisplayIcon" "$INSTDIR\crosstalk.exe"
      WriteRegStr HKCU "${UNINSTKEY}" "Publisher" "Crosstalk"
      WriteRegStr HKCU "${UNINSTKEY}" "InstallLocation" "$INSTDIR"
      WriteRegStr HKCU "${UNINSTKEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
      WriteRegStr HKCU "${UNINSTKEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
      WriteRegDWORD HKCU "${UNINSTKEY}" "NoModify" 1
      WriteRegDWORD HKCU "${UNINSTKEY}" "NoRepair" 1
      WriteRegDWORD HKCU "${UNINSTKEY}" "EstimatedSize" ${APP_KB}
    ${EndIf}
  ${EndIf}
  SetErrorLevel $0
SectionEnd

; ---------- Uninstaller ----------

Function un.onInit
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/S" $1
  ${If} ${Errors}
    StrCpy $Quiet 0
    MessageBox MB_YESNO|MB_ICONQUESTION "Désinstaller Crosstalk ?" IDYES +2
    Quit
  ${Else}
    StrCpy $Quiet 1
  ${EndIf}
FunctionEnd

Section "Uninstall"
  ; A running Crosstalk cannot be removed: ask to quit it first.
  ClearErrors
  Delete "$INSTDIR\crosstalk.exe"
  ${If} ${Errors}
    ${If} $Quiet == 0
      MessageBox MB_ICONEXCLAMATION "Crosstalk est ouvert : quittez-le (icône près de l'horloge › Quitter), puis relancez la désinstallation."
    ${EndIf}
    SetErrorLevel 1
    Quit
  ${EndIf}
  !include "${FILES_NSH}"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\Crosstalk.lnk"
  Delete "$DESKTOP\Crosstalk.lnk"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Crosstalk"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "Crosstalk"
  DeleteRegKey HKCU "${APPKEY}"
  DeleteRegKey HKCU "${UNINSTKEY}"

  ; Models (about 800 MB) and settings are kept unless the user asks.
  ${If} $Quiet == 0
    MessageBox MB_YESNO|MB_ICONQUESTION "Supprimer aussi les modèles téléchargés (environ 800 Mo) et vos réglages ?" IDNO keep
    RMDir /r "$LOCALAPPDATA\Crosstalk\models"
    RMDir /r "$LOCALAPPDATA\app.crosstalk.desktop"
    RMDir "$LOCALAPPDATA\Crosstalk"
    keep:
    MessageBox MB_ICONINFORMATION "Crosstalk a été désinstallé."
  ${EndIf}
SectionEnd
