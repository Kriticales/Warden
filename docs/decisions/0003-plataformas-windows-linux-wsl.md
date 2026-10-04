# ADR-0003 — Plataformas: Windows primeiro, Linux depois, desenvolvimento em WSL2

- **Status:** substituída pela [ADR-0048](0048-desenvolvimento-no-windows.md) (04/10/2026: o desenvolvimento passou para o Windows nativo e o WSL saiu do fluxo; o texto abaixo fica como registro) · **Data:** 2026-10-01 · **Origem:** decisão do dono (plataformas) + decisão técnica (A1) (caminho de build)

## Contexto

O dono joga e usa o app no Windows. O desenvolvimento acontece em WSL2 (Linux), onde os agentes rodam. Os apps anteriores só tinham detecção e scripts para Windows e testavam em Linux (R4).

## Decisão

- **Windows 10/11 x64** é a plataforma alvo da v1. **Linux x64** compila, roda (para desenvolvimento via WSLg) e passa na CI, mas empacotamento e suporte oficial ficam para depois da v1. macOS fora.
- CI em Linux e Windows (ADR-0022).
- Para o dono abrir o app no Windows a partir do WSL:
  1. `cargo xtask win-dev`: compila para `x86_64-pc-windows-msvc` com `cargo-xwin`, copia para `C:\Users\<usuário>\Warden-dev\` e abre. Só existe se o dono responder "sim" à decisão D5 da SPEC (licença do kit da Microsoft); essa decisão não é tomada por omissão;
  2. `cargo xtask win-install`: baixa o instalador gerado pela CI da `main` e instala.
- O código trata caminhos, processos (`javaw.exe`, Job Objects), codificação de console e limite de linha de comando do Windows desde o início.

## Alternativas consideradas

- Compilar no próprio Windows: exigiria instalar Rust, Node e Go no Windows do dono (leigo).
- Alvo `x86_64-pc-windows-gnu` (MinGW): menos suportado pelo Tauri e exige pacotes do sistema.
- Só artefato da CI: ciclo de teste lento (depende de push e fila da CI).

## Consequências

- Testes específicos de Windows (processo, caminhos longos, encoding) rodam na CI Windows e em roteiros manuais por marco.
- Bibliotecas de sistema do Tauri no WSL exigem uma instalação com `sudo` pelo dono (ROADMAP §2).
