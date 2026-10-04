# ADR-0048 — Plataformas: Windows primeiro, desenvolvimento direto no Windows

- **Status:** aceita · **Data:** 2026-10-04 · **Origem:** decisão do dono (sair do WSL) + decisão técnica (tarefa D6) · **Substitui:** ADR-0003

## Contexto

A [ADR-0003](0003-plataformas-windows-linux-wsl.md) supunha que os agentes desenvolviam no WSL2 e que o dono abria o app no Windows a partir do WSL (`cargo xtask win-dev` com `cargo-xwin`, decisão D5 da SPEC). Desde 03/10/2026 o dono usa o Orca no Windows e os agentes rodam no Claude Code nativo do Windows; em 04/10/2026 o dono mandou migrar o projeto para o Windows. Ainda não existe código do app.

Os pré-requisitos do Tauri já estão na máquina (conferidos pelo orquestrador em 03/10/2026): MSVC Build Tools 2022 com o componente C++, WebView2, Rust stable `x86_64-pc-windows-msvc`, `cargo-tauri`, Node 24, pnpm 12, Go e git com `core.autocrlf=false`. A prova da tarefa D6 (app Tauri 2 mínimo compilado nesta máquina, janela aberta e instalador NSIS gerado) mostrou duas armadilhas próprias do Windows: o `link.exe` da Microsoft não aceita caminhos com mais de 260 caracteres, e um app de desenvolvimento usa as mesmas pastas de dados do app instalado.

## Decisão

- **Windows 10/11 x64** é a plataforma alvo da v1. **Linux x64** compila e passa na CI (runner Ubuntu), sem empacotamento nem suporte oficial na v1. macOS fora.
- **O desenvolvimento acontece direto no Windows nativo**, com o alvo `x86_64-pc-windows-msvc`. O WSL não faz parte do fluxo; `cargo-xwin` e `cargo xtask win-dev` deixam de existir, e a decisão D5 da SPEC fica sem efeito.
- CI em Linux e Windows ([ADR-0022](0022-testes-e-ci.md)). O Linux da CI é a única execução em Linux: tudo que o `xtask` faz precisa funcionar no PowerShell do Windows e no Linux da CI, sem depender de bash.
- **Versão de teste para o dono** (F0-04): um comando gera o instalador com `tauri build` (ou baixa o da CI com `gh run download`), instala para o usuário atual e abre o Warden, com um passo a passo em linguagem simples (`docs/DEV-WINDOWS.md`).
- **O desenvolvimento não toca nos dados nem no cofre reais do dono.** Só em build de debug, o Warden aceita `WARDEN_DATA_ROOT` (pastas de configuração e de dados locais dentro dela) e `WARDEN_SECRET_BACKEND=file:<pasta>` (cofre de teste em arquivo). `cargo xtask dev`, os testes e os E2E sempre definem os dois, com uma pasta por worktree; com `WARDEN_DATA_ROOT` definido, a instância única passa a valer só entre cópias com a mesma pasta, para o app de desenvolvimento não focar o Warden instalado. O cofre real do Windows é testado na CI Windows e no roteiro manual dos marcos, com a versão instalada.
- **Caminhos curtos:** a pasta `target` de cada worktree tem de caber no limite de 260 caracteres do `link.exe`. O `xtask` avisa quando o caminho da pasta `target` passa de 100 caracteres e explica como usar `CARGO_TARGET_DIR` (QUALITY §13).
- O código trata caminhos, processos (`javaw.exe`, Job Objects), codificação de console e limite de linha de comando do Windows desde o início (sem mudança em relação à ADR-0003).

## Alternativas consideradas

- Continuar desenvolvendo no WSL: o dono não usa mais o WSL, e a cópia para o Windows (`cargo-xwin`, `wslpath`) era uma camada a mais para manter.
- Só artefato da CI para o dono testar: ciclo lento (push e fila da CI); fica como segunda opção do mesmo comando.
- Pasta `target` única para todos os worktrees: caminhos curtos, mas os agentes em paralelo disputariam a trava do cargo e invalidariam o cache uns dos outros.

## Consequências

- Os comandos de verificação e os roteiros passam a ser escritos para o PowerShell; o `xtask` (Rust) é o ponto único de automação nas duas plataformas.
- Testes específicos de Windows (processo, caminhos longos, encoding, cofre) passam a rodar também localmente; a CI Windows continua obrigatória na `main`.
- O E2E local usa o `msedgedriver` da mesma versão do WebView2, baixado pelo `xtask` para o cache (sem instalar nada no Windows); a CI Linux usa o WebKitGTK com o `WebKitWebDriver`.
- O antivírus pode deixar a compilação mais lenta; excluir a pasta `target` da verificação em tempo real é uma escolha do dono, nunca um passo automático (QUALITY §13).
