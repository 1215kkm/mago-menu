; MAGO 설치 파일 훅 (tauri.conf.json bundle.windows.nsis.installerHooks)
; 지우기(언인스톨) 때 '윈도우 켤 때 같이 켜기' 등록(HKCU Run 의 MAGO 값)도 지운다 — 없는 exe 를 가리키는 시작프로그램이 남지 않게.
; 자동 업데이트도 지우기 → 깔기를 거친다($UpdateMode = 1). 그때 지우면 사용자가 켜 둔 설정이 사라지니 건드리지 않는다.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "MAGO"
  ${EndIf}
!macroend

; 깔고 나서 윈도우 아이콘 그림 기억(캐시)을 새로 고친다 — 0.1.2 까지의 쓰레기통 아이콘이 바탕화면 바로 가기에 남지 않게 (SHCNE_ASSOCCHANGED)
!macro NSIS_HOOK_POSTINSTALL
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
