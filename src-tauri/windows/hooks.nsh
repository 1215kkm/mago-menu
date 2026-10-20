; MAGO 설치 파일 훅 (tauri.conf.json bundle.windows.nsis.installerHooks)
; 지우기(언인스톨) 때 '윈도우 켤 때 같이 켜기' 등록(HKCU Run 의 MAGO 값)도 지운다 — 없는 exe 를 가리키는 시작프로그램이 남지 않게.
; 자동 업데이트도 지우기 → 깔기를 거친다($UpdateMode = 1). 그때 지우면 사용자가 켜 둔 설정이 사라지니 건드리지 않는다.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "MAGO"
  ${EndIf}
!macroend
