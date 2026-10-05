; Ganchos do instalador NSIS do Warden (A-02; `bundle.windows.nsis.installerHooks`).
;
; O modelo do Tauri grava em HKCU\Software\<publisher>\<produto> a pasta de instalação e o
; idioma do instalador, e só apaga essa chave quando o usuário marca "apagar os dados do
; aplicativo". Ela não é dado do usuário: sem este gancho, sobraria no registro depois de
; desinstalar. Numa atualização (`$UpdateMode = 1`) a chave fica, porque o instalador novo usa
; a pasta gravada nela. Os dados do usuário (%APPDATA% e %LOCALAPPDATA% do identificador)
; continuam sob a escolha da caixa de seleção do Tauri.

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    DeleteRegKey SHCTX "${MANUPRODUCTKEY}"
    DeleteRegKey /ifempty SHCTX "${MANUKEY}"
    DeleteRegKey HKCU "${MANUPRODUCTKEY}"
    DeleteRegKey /ifempty HKCU "${MANUKEY}"
  ${EndIf}
!macroend
