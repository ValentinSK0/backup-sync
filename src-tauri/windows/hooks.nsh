; NSIS hooky pre Backup Sync installer.

!macro NSIS_HOOK_POSTINSTALL
  ; Povedz Explorer-u, ze sa zmenili ikony/asociacie a ma zahodit cache
  ; (SHCNE_ASSOCCHANGED = 0x08000000, SHCNF_FLUSH = 0x1000).
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0x1000, p 0, p 0)'
  ; Navyse obnov icon cache aj pre Start/pripnute polozky.
  Exec 'ie4uinit.exe -show'
!macroend
