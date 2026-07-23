; NSIS hooky pre Backup Sync installer.

!macro NSIS_HOOK_POSTINSTALL
  ; Obnov Windows icon cache, aby odkazy na ploche a v Starte
  ; ukazovali aktualne logo hned po instalacii/aktualizacii.
  Exec 'ie4uinit.exe -show'
!macroend
