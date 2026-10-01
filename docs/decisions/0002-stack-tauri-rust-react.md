# ADR-0002 — Stack: Tauri 2, Rust e React com TypeScript

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono

## Contexto

Os apps anteriores usaram Electron porque não havia toolchain Rust na máquina e assim todos os agentes usavam uma só linguagem (R4 §3.2). O Warden precisa de um launcher, leitura de jars, hashes, processos e muito I/O, onde Rust é forte; o ecossistema de launchers em Rust (portablemc, app-lib do Modrinth) é aproveitável. A máquina agora tem Rust 1.98.1, Go 1.26 e Node 24.

## Decisão

- App desktop em **Tauri 2**: backend em Rust, interface em **React 19 + TypeScript strict** (Vite).
- Toda regra de negócio fica em crates `warden-*` **sem dependência do Tauri**; só a crate `warden-app` conhece o Tauri (ARCHITECTURE §3).
- Windows usa o WebView2 do sistema; Linux usa WebKitGTK.

## Alternativas consideradas

- **Electron** (como antes): app mais pesado, launcher teria de ser escrito em Node; descartado pelo dono.
- **Interface nativa em Rust** (egui, Slint, Iced): menos componentes prontos e acessibilidade mais fraca para muitas telas de formulário.

## Consequências

- Diferenças entre WebView2 e WebKitGTK exigem testes E2E nas duas plataformas.
- Compilação Rust mais lenta; mitigada com cache na CI e crates pequenas.
- Domínio testável sem janela (`cargo test`), o que permite muitos testes de integração reais.
