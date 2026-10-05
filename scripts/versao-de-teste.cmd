@echo off
rem Versão de teste do Warden (F0-04; docs/DEV-WINDOWS.md): baixa da CI o instalador mais
rem recente da main, instala e abre o Warden. Duplo clique neste arquivo.
chcp 65001 >nul
setlocal
cd /d "%~dp0.."
echo Versão de teste do Warden: baixando da CI o instalador mais recente...
echo.
cargo xtask preview --from-ci %*
if errorlevel 1 (
  echo.
  echo Algo deu errado. Leia a mensagem acima; se não entender, copie tudo e mande ao orquestrador.
) else (
  echo.
  echo Tudo certo. Pode fechar esta janela.
)
echo.
pause
