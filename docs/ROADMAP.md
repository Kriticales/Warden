# Warden — Plano de construção

> Versão do documento: 1.5 (2026-10-04). Tarefa A1; escopos ajustados na tarefa D2 às decisões do dono (estrutura de navegação, busca combinada, chaves no cofre ou `.env`, Java, seção de IA e publicação no GitHub; ADR-0025 a ADR-0029); funções avançadas, spikes e ondas recalculadas na tarefa D4 (decisões D15 a D26; ADR-0030 a ADR-0038); **Warden 1.1 "Profissional"** na tarefa D5: fase 7 (W-01 a W-12), marco M6, ganchos nas tarefas da v1 e ondas recalculadas (decisões D27 a D33; ADR-0039 a ADR-0047); **desenvolvimento direto no Windows** na tarefa D6 (ADR-0048): F0-01, F0-04, F0-05, F0-06 e S-R5-2 revistas, ações do dono e ondas recalculadas; **o que as ondas 0 e 1 descobriram** na tarefa D7 (ADR-0049 a ADR-0056): spikes concluídos aplicados à D-04, D-10, D-12, L-05 e L-11, achados das tarefas F0-01, F0-03, F0-05, P1-01, P1-06 e C-01, e a tarefa nova E-04 (instância pronta para o Prism).
> Cada tarefa abaixo é entregue por um agente numa branch própria, seguindo `QUALITY.md`. O orquestrador despacha, revisa e integra.
> Referências: `SPEC.md` (telas T01–T33 e critérios CA-*), `ARCHITECTURE.md` (§ citados), `docs/decisions/` (ADR-*).

## Sumário

1. [Como ler este plano](#1-como-ler-este-plano)
2. [Ações necessárias do dono](#2-ações-necessárias-do-dono)
3. [Ondas de paralelismo](#3-ondas-de-paralelismo)
4. [Fase 0 — Fundação](#fase-0--fundação)
5. [Fase 1 — Núcleo do pack](#fase-1--núcleo-do-pack)
6. [Fase 2 — Launcher](#fase-2--launcher)
7. [Fase 3 — Configs e captura](#fase-3--configs-e-captura)
8. [Fase 4 — Diagnóstico](#fase-4--diagnóstico)
9. [Fase 5 — Versionamento e exportação](#fase-5--versionamento-e-exportação)
10. [Fase 6 — Acabamento](#fase-6--acabamento)
11. [Fase 7 — Warden 1.1 "Profissional"](#fase-7--warden-11-profissional)
12. [Spikes da D4](#spikes-da-d4)
13. [Marcos](#13-marcos)
14. [Backlog P2](#14-backlog-p2)

---

## 1. Como ler este plano

- **ID estável:** `F0-xx` (fundação), `P1-xx` (núcleo do pack), `L-xx` (launcher), `C-xx` (configs), `D-xx` (diagnóstico), `V-xx` (versionamento), `E-xx` (exportação), `A-xx` (acabamento), `W-xx` (Warden 1.1 "Profissional", fase 7, prioridade **1.1**), `S-R5-N` (spikes da D4, seção [Spikes da D4](#spikes-da-d4)). `S1` é o spike do motor do launcher, concluído fora deste plano (`docs/spikes/S1-motor-do-launcher.md`). As tarefas acrescentadas na D4 ficam na fase do assunto (P1-16 a P1-19, L-08 a L-12, C-05 a C-07, D-05 a D-14, E-03, A-07); as que já existiam e mudaram de escopo dizem "D4:" nas entregas. As tarefas da v1 que deixam um gancho pronto para a 1.1 dizem "Gancho 1.1:" nas entregas (custo mínimo, sem função visível; ADR-0039).
- **Fases** agrupam por assunto; a **ordem real** é dada pelas dependências. Uma tarefa pode começar assim que as dependências estiverem integradas na `main` (ondas na §3).
- **Posse:** arquivos e pastas que só aquela tarefa altera. Fora da posse, só os **registros acréscimo-apenas** abaixo (uma linha por entrada, conflitos resolvidos pelo orquestrador). **Não são registros, e não se edita nada fora da sua posse para criá-los (INFRA-01; ARCHITECTURE §4.1):** comandos e eventos do IPC (escreva a função em `commands/<domínio>.rs`, ou crie o arquivo), namespaces de i18n (crie `i18n/pt-BR/<nome>.ts` com o `declare module '../catalogo'`), subcomandos do xtask (crie `xtask/src/tasks/<nome>.rs`) e a pendência de tela de um comando (`// pendente-na-ui: <TAREFA> <motivo>` acima do `#[tauri::command]`). Depois de integrar, o `bindings.ts` é regenerado com `cargo xtask bindings`:
  - `Cargo.toml` da raiz (`[workspace.dependencies]`) e `Cargo.lock`;
  - `apps/desktop/package.json` (dependências) e `pnpm-lock.yaml`;
  - `apps/desktop/src-tauri/Cargo.toml` (dependências);
  - `apps/desktop/src-tauri/src/state.rs` (campos do estado);
  - `apps/desktop/src/app/navigation.ts`, `apps/desktop/src/i18n/errors/index.ts`;
  - `lib.rs` das crates divididas entre várias tarefas (`warden-project`, `warden-instance`, `warden-diagnostics`, `warden-versioning`; D4: `warden-jarmeta`, `warden-configs`, `warden-launcher`, `warden-ai`, `warden-perf`, `warden-discovery`, `warden-export`; D5: `warden-security`): só declarações de módulos e reexportações;
  - `crates/warden-diagnostics/data/log-patterns.toml` (D4: um padrão por bloco, só acréscimo); D5: `crates/warden-diagnostics/data/health-score.toml` (só acréscimo de categorias; a W-11 é a dona das categorias da 1.1);
  - D4: `apps/desktop/src/features/test/menu-items.ts` (itens do menu ▾ do Testar, criado pela L-04), `apps/desktop/src/features/pack-editor/details/blocks.ts` (blocos do painel de detalhes do item, criado pela P1-08), `apps/desktop/src/features/diagnostics/problems-blocks.ts` (blocos da seção Problemas, criado pela D-03), `apps/desktop/src-tauri/src/ai_tools/mod.rs` (registro das ferramentas da IA, criado pela D-04);
  - enums de códigos de erro das crates (`crates/*/src/error.rs`, só acréscimo de variantes) e as frases correspondentes em `apps/desktop/src/i18n/errors/<domínio>.ts`;
  - `apps/desktop/src/features/pack-editor/header/slots.ts` e `apps/desktop/src/features/pack-editor/sections.ts` (botões/indicadores do cabeçalho e seções do menu do pack, criados pela P1-08);
  - `apps/desktop/src-tauri/src/test_hooks/mod.rs` (ganchos do Testar, criado pela L-04);
  - `apps/desktop/src/features/settings/sections.ts` (seções da tela de Configurações);
  - `apps/desktop/src/lib/ipc/bindings.ts` (gerado e versionado; o driver de merge fica com o texto de quem recebe e `cargo xtask bindings` regenera; o `bindings --check` da CI barra o esquecimento);
  - `THIRD_PARTY.md` (uma seção por origem).
- **Critérios de aceite** citam os CA da SPEC quando existem; os demais são específicos da tarefa.
- **Verificação:** além dos comandos listados, toda tarefa roda `cargo xtask check` (QUALITY §12) e informa o resultado.
- **Branch:** `<tipo>/<id-minúsculo>-<descrição>` (QUALITY §7.1), indicada em cada tarefa.

## 2. Ações necessárias do dono

| Quando | Ação | Por quê |
|---|---|---|
| ~~Antes de F0-01~~ | ~~**Feito em 01/10/2026 pelo orquestrador**, com autorização do dono: bibliotecas do Tauri instaladas no WSL (`webkitgtk-webdriver` substitui `webkit2gtk-driver` no Ubuntu 26.04).~~ **Sem efeito desde a ADR-0048 (04/10/2026):** o desenvolvimento é no Windows, onde os pré-requisitos já estão instalados. | — |
| ~~Antes de F0-04~~ | ~~Responder D5 — **respondido "sim" em 01/10/2026** (SPEC §10).~~ **Sem efeito desde a ADR-0048 (04/10/2026):** o `cargo-xwin` saiu do plano. | ~~`cargo xtask win-dev` pode ser entregue.~~ |
| ~~Antes de F0-02~~ | **Feito em 04/10/2026 pelo orquestrador**, com autorização do dono: segredo `CURSEFORGE_API_KEY` criado com o `gh`, lido do `.env` sem exibir o valor. | Testes de rede agendados na CI. |
| Antes de V-03 | Gerar um token do GitHub com permissão para criar repositórios (públicos e privados), enviar conteúdo e criar Releases (passo a passo virá no app) e salvá-lo no Warden. Para os testes de integração de V-03, uma conta ou repositórios de teste descartáveis. | Publicar versão para os jogadores (D14). |
| ~~Antes de S-R5-4~~ | **Feito em 04/10/2026 pelo dono:** chave do Gemini no `.env` da raiz (`GEMINI_API_KEY`), testada pelo orquestrador. | Spike do laço de ferramentas e, depois, IA do diagnóstico (D-04). |
| ~~Antes de S-R5-2~~ | ~~Deixar o orquestrador rodar um teste curto no Windows da máquina (Java 8, 17, 21 e 25 baixados pelo próprio spike).~~ **Não precisa mais (ADR-0048):** o spike roda direto no Windows desta máquina, com os Javas baixados para uma pasta temporária, sem instalar nada. | — |
| Antes do roteiro da E-04 (marco M4) | Ter o Prism Launcher 11.x instalado no Windows (programa do dono; o agente não instala nada) para arrastar a instância pronta e conferir que o jogo abre. | Validação real da instância pronta para o Prism (ADR-0050), que a CI não consegue fazer. |
| Em cada marco (§13) | Instalar a versão de teste do marco (F0-04, `docs/DEV-WINDOWS.md`), abrir o app e seguir o roteiro de aceite entregue. | Validação real no Windows, inclusive do cofre real do Windows. |

## 3. Ondas de paralelismo

Uma onda começa quando as dependências da anterior estão integradas. Dentro de cada onda as tarefas têm posse disjunta.

| Onda | Tarefas (em paralelo) |
|---|---|
| 0 | F0-01, S-R5-1, S-R5-2, S-R5-3, S-R5-4 |
| 1 | F0-03, F0-05, P1-01, P1-06, C-01 |
| 2 | F0-02, F0-06, P1-02, P1-03, D-02, V-01 |
| 3 | F0-04, P1-04, P1-05, P1-13, L-01, A-02 |
| 4 | P1-07, L-02, L-03, D-01 |
| 5 | P1-08, L-05, D-05, E-01 |
| 6 | P1-09, P1-14, L-04, C-02, D-07, V-02, E-02, A-05 |
| 7 | P1-10, P1-18, L-08, L-10, C-03, C-04, C-05, C-07, D-03, D-09, D-11, V-03, A-06 |
| 8 | P1-11, P1-12, P1-16, P1-19, L-06, L-07, L-09, L-11, C-06, D-06, D-10, E-04, A-04 |
| 9 | P1-15, P1-17, L-12, D-04, D-08, D-12, E-03, A-01 |
| 10 | D-13, D-14, A-03 |
| 11 | A-07 |
| 12 | W-01, W-04, W-06, W-08, W-09, W-10 |
| 13 | W-02, W-05, W-07 |
| 14 | W-03, W-11 |
| 15 | W-12 |

A tabela é derivada das dependências declaradas em cada tarefa (recalculada por script na D4, na D5, na D6 e na D7: onda = 1 + a maior onda entre as dependências); em caso de dúvida, valem as dependências. Os spikes da D4 não dependem de nada; os quatro terminaram em 04/10/2026, e os relatórios já foram aplicados às tarefas que dependem deles (L-11, D-04, D-10, D-12) na D7. A-01 depende de todas as tarefas com parte P0; A-03, de todas as P0 e da A-01; A-07, de todas as tarefas P1 com interface. As ondas 12 a 15 são do Warden 1.1 (fase 7): começam depois do M5 (as tarefas W sem dependência de outra W dependem de A-03 e A-07). Muitas tarefas da onda 7 em diante são P1: o orquestrador pode adiar uma P1 sem travar as P0 da mesma onda.

---

## Fase 0 — Fundação

Ao fim desta fase: o app abre (Linux e Windows), a CI roda em Linux e Windows, o sidecar do packwiz é compilado de commit fixado, o dono instala e abre uma versão de teste no Windows com um comando, e toda a infraestrutura comum (erros, operações, travas, registros, configurações, cofre, layout, i18n, testes) existe.

### F0-01 — Esqueleto do monorepo e app compilando

- **Prioridade:** P0 · **Depende de:** — (os pré-requisitos do Windows já estão instalados; ADR-0048) · **Branch:** `feat/f0-01-esqueleto`
- **Objetivo:** criar a estrutura do monorepo (ARCHITECTURE §2) com o app Tauri 2 + React/TypeScript abrindo uma janela, todas as crates de domínio como esqueletos vazios e as configurações de lint/format/teste.
- **Posse:** arquivos da raiz (`Cargo.toml`, `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `deny.toml`, `.config/nextest.toml`, `package.json`, `pnpm-workspace.yaml`, `.npmrc`, `.nvmrc`, `.editorconfig`, `.gitattributes`, `.gitignore`, `THIRD_PARTY.md`, `README.md`), `apps/desktop/**` (estrutura inicial), `crates/*/Cargo.toml` e `crates/*/src/lib.rs` (esqueletos de todas as crates da ARCHITECTURE §3), `xtask/**` (inicial, com os subcomandos listados abaixo).
- **Entregas:**
  - Workspace com `[workspace.lints]` e `[workspace.dependencies]` (QUALITY §2.1); Rust 1.98.1 fixado; edição 2024.
  - App Tauri 2 (`identifier = "dev.kriticales.warden"`, `productName = "Warden"`; a biblioteca da `warden-app` com `crate-type = ["rlib"]`, só para desktop: o `staticlib`/`cdylib` do modelo do Tauri servem ao celular e geram aviso do linker no Windows), React 19 + Vite + TS strict + ESLint/Prettier conforme QUALITY §2.2, TanStack Router com rotas por arquivo, i18next com pt-BR.
  - `tauri.conf.json` com a CSP da ARCHITECTURE §20 (o `bundle.externalBin` é acrescentado pela F0-03; o resto do bundle pela A-02).
  - `tauri-specta` integrado com um comando `app_info` (versão, commit) e `AppError` mínimo; `bindings.ts` gerado e versionado.
  - Subcomandos do xtask, todos em Rust (funcionam no PowerShell do Windows e no Linux da CI, sem bash): `setup` (instala no espaço do usuário `cargo-nextest`, `cargo-deny`, `cargo-llvm-cov`, `tauri-cli`, `tauri-driver`, e roda `pnpm install`), `dev` (carrega o `.env` do repositório principal sem imprimir valores, define `WARDEN_DATA_ROOT` e `WARDEN_SECRET_BACKEND=file:` numa pasta própria do worktree, `%LOCALAPPDATA%\Warden-dev\<worktree>\`, e abre o app nativo no Windows com `tauri dev`; ADR-0048), `check` e `check --fast` (QUALITY §12), `check-deps`, `check-docs` (links internos dos docs), `bindings` (com `--check`), `coverage` (mínimos da QUALITY §4.2, configuráveis por crate) e `test-network` (testes `#[ignore = "rede"]` com o `.env`).
  - `xtask check-deps`: falha se alguma crate fora de `warden-app` depender de `tauri`.
  - `xtask setup`, `dev` e `check` avisam quando o caminho da pasta `target` passa de 100 caracteres, com a solução (QUALITY §13).
  - `.gitattributes` da ARCHITECTURE §2 (`* text=auto eol=lf`, `*.cmd`/`*.bat` com `eol=crlf`, fixtures do packwiz com `-text`, binários) e `.editorconfig` (QUALITY §2.3).
  - `README.md` da raiz com: pré-requisitos do Windows (QUALITY §13), `cargo xtask setup`, `cargo xtask dev` (abre o app no Windows), `cargo xtask check` e a nota sobre caminhos curtos.
- **Critérios de aceite:**
  1. `cargo xtask check` passa num clone limpo após `cargo xtask setup`, no PowerShell do Windows (no Linux, conferido pela CI da F0-02).
  2. `cargo xtask dev` abre a janela "Warden" no Windows mostrando uma página inicial com texto vindo do catálogo pt-BR e a versão obtida por `app_info`.
  3. `cargo xtask check-deps` falha num teste em que uma crate de domínio declara `tauri` (teste do próprio xtask).
  4. `bindings.ts` regenerado é idêntico ao versionado (`cargo xtask bindings --check`).
- **Verificação:** no PowerShell, `cargo xtask setup && cargo xtask check`; `cargo xtask dev` (captura de tela no relatório).

### F0-02 — CI no GitHub Actions (Linux + Windows)

- **Prioridade:** P0 · **Depende de:** F0-01, F0-03 · **Branch:** `ci/f0-02-github-actions`
- **Objetivo:** CI que garante o padrão de qualidade em Linux e Windows e produz o instalador de Windows como artefato.
- **Posse:** `.github/workflows/**`, `.github/actionlint.yaml`, `.gitleaks.toml`, a nova tentativa do clone em `xtask/src/packwiz.rs` (a F0-03 já integrada).
- **Entregas:**
  - D7: desde a F0-03, o `check` precisa do sidecar compilado. Todo job roda `cargo xtask setup` (ou `cargo xtask build-packwiz`) **antes** do `check`, com cache da pasta `binaries/` e do cache do Go pela chave commit + SHA-256 dos patches. O primeiro build precisa de rede (clone raso do packwiz e módulos Go): o `xtask build-packwiz` passa a tentar de novo (3 vezes, com espera crescente) quando o GitHub ou o proxy do Go não respondem, e a CI repete o passo uma vez antes de falhar.
  - `ci.yml`: em push/PR — Linux (`ubuntu-24.04`): `cargo xtask setup`, sidecar (com cache por commit do packwiz), `cargo xtask check`, `cargo xtask coverage`, E2E Linux (quando existir), `gitleaks`, `actionlint`. Windows (`windows-latest`): o mesmo `check` + `tauri build` gerando instalador NSIS como artefato `warden-windows-<sha>`; executado na `main`, em PRs com rótulo `windows` e à noite (D9).
  - `nightly.yml`: testes de rede (`CURSEFORGE_API_KEY` dos secrets), conformidade no Windows, E2E Windows. D7: o job Windows tem um JDK disponível (o `jstat`) para o teste de integração da L-11 (`actions/setup-java` com Temurin, só nesse job).
  - Caches de cargo, pnpm e Go; tempos-limite por job; concorrência que cancela execuções antigas da mesma branch.
- **Critérios de aceite:**
  1. Um PR com um aviso do clippy falha a CI; com formatação errada falha; com texto solto em JSX falha.
  2. A execução na `main` publica o artefato do instalador de Windows, que instala e abre o app numa máquina Windows (roteiro manual do orquestrador/dono).
  3. Um commit contendo uma string no formato de chave da CurseForge é barrado pelo `gitleaks` (teste em branch descartável, sem chave real).
- **Verificação:** `actionlint`; execução real dos workflows numa branch de teste (feita pelo orquestrador, que faz push).

### F0-03 — Sidecar do packwiz

- **Prioridade:** P0 · **Depende de:** F0-01 · **Branch:** `build/f0-03-sidecar-packwiz`
- **Objetivo:** compilar o packwiz de commit fixado, com o patch da chave em tempo de execução, e embuti-lo como sidecar (ARCHITECTURE §6.3, ADR-0007).
- **Posse:** `third_party/packwiz/**`, `xtask/src/packwiz.rs`, `apps/desktop/src-tauri/binaries/.gitignore`, `bundle.externalBin` no `apps/desktop/src-tauri/tauri.conf.json`.
- **Entregas:**
  - `third_party/packwiz/COMMIT` (hash completo do commit escolhido, partindo do `ef87d96` de R3), `LICENSE` do packwiz, `README.md` explicando atualização do commit e dos patches.
  - `patches/0001-chave-curseforge-em-tempo-de-execucao.patch` (chave via `WARDEN_CURSEFORGE_API_KEY`, sem recuo para a chave embutida do packwiz).
  - `cargo xtask build-packwiz`: clona/atualiza um cache local, aplica patches, compila Linux e Windows, grava `binaries/packwiz-<triplo>[.exe]` e `binaries/packwiz.commit`; idempotente (pula se o commit e os patches não mudaram).
  - `cargo xtask setup` passa a chamar `build-packwiz`.
- **Critérios de aceite:**
  1. `binaries/packwiz-x86_64-unknown-linux-gnu --help` roda; o `.exe` é um PE32+ válido (`file`).
  2. Sem `WARDEN_CURSEFORGE_API_KEY`, `packwiz curseforge add jei` (num pack temporário) falha com a mensagem do patch; com a variável definida a partir do `.env` (sem imprimir), adiciona o JEI. Nenhuma requisição usa a chave embutida do packwiz (teste lendo o binário: a string base64 da chave original não existe no executável).
  3. `packwiz init`/`refresh` num pack temporário funcionam igual ao binário sem patch (teste de fumaça).
- **Verificação:** `cargo xtask build-packwiz && cargo nextest run -p xtask`; roteiro do critério 2 descrito no relatório.

### F0-04 — Versão de teste do app para o dono no Windows

- **Prioridade:** P0 · **Depende de:** F0-01, F0-02, F0-03 · **Branch:** `build/f0-04-versao-de-teste-windows`
- **Objetivo:** o dono, sem saber programar, instala e abre no Windows a versão mais recente do Warden para testar, e repete isso a cada entrega (ADR-0048).
- **Posse:** `xtask/src/preview.rs`, `scripts/versao-de-teste.cmd`, `docs/DEV-WINDOWS.md`.
- **Entregas:**
  - `cargo xtask preview`: gera o instalador NSIS com `tauri build` (inclui o sidecar `packwiz.exe` da F0-03), instala para o usuário atual sem pedir administrador (`/S`) e abre o Warden instalado.
  - `cargo xtask preview --from-ci [--branch <nome>]`: alternativa sem compilar; baixa com `gh run download` o último artefato `warden-windows-<sha>` da `main` (ou da branch indicada), confere que a execução terminou com sucesso, instala e abre.
  - Antes de instalar, fecha o Warden instalado se ele estiver aberto (pedindo confirmação) e mostra a versão e o commit que vão ser instalados.
  - `scripts/versao-de-teste.cmd`: atalho de duplo clique que roda `cargo xtask preview --from-ci` e deixa a janela aberta com o resultado.
  - `docs/DEV-WINDOWS.md`: passo a passo para o dono, em linguagem simples: o duplo clique, o comando para gerar localmente, o aviso do SmartScreen ("O Windows protegeu o computador" → "Mais informações" → "Executar assim mesmo", com o motivo: o instalador não tem assinatura digital, decisão D4), onde ficam os dados do app, como desinstalar e o que mandar ao orquestrador quando algo der errado.
- **Critérios de aceite:**
  1. Nesta máquina Windows 11, `cargo xtask preview` termina com o Warden instalado e aberto mostrando a versão e o commit (roteiro manual com captura de tela); o mesmo com `--from-ci`, a partir de um artefato real da CI.
  2. O app instalado usa o WebView2 do Windows (verificado em `app_info`, que informa a plataforma) e o sidecar `packwiz.exe` está ao lado do executável.
  3. Rodar de novo atualiza a instalação sem apagar os dados do app (`%APPDATA%\dev.kriticales.warden\` e `%LOCALAPPDATA%\dev.kriticales.warden\` intactos, conferidos antes e depois).
  4. Sem o `gh` autenticado, ou sem artefato de execução com sucesso, `--from-ci` termina com uma frase que diz o que fazer, sem instalar nada.
- **Verificação:** roteiro manual (critérios 1 a 3); `cargo nextest run -p xtask` (escolha do artefato, leitura da versão e mensagens de erro testadas com respostas simuladas do `gh`).

### F0-05 — Serviços comuns do backend

- **Prioridade:** P0 · **Depende de:** F0-01 · **Branch:** `feat/f0-05-servicos-comuns`
- **Objetivo:** infraestrutura que todas as funcionalidades usam (ARCHITECTURE §4, §5, §13–§16).
- **Posse:** `crates/warden-core/**`, `crates/warden-secrets/**`, `apps/desktop/src-tauri/src/{lib.rs,main.rs,state.rs,error.rs,events.rs,settings.rs,operations.rs,locks.rs,logging.rs}`, `apps/desktop/src-tauri/src/commands/{app.rs,secrets.rs}`, `apps/desktop/src-tauri/capabilities/**`.
- **Entregas:**
  - `warden-core`: `PackId` (ULID), `OperationId`, `ProgressSink`, `AppPaths`, `atomic_write` (`.warden-tmp` + fsync + rename), `resolve_inside`, trait `DomainError`, feature `fault-injection` com pontos de falha.
  - `AppError`/`ErrorCode` com **todos** os domínios da ARCHITECTURE §5, cada um com `INTERNAL`; arquivos `src/i18n/errors/<domínio>.ts` correspondentes (criados vazios exceto `INTERNAL`, a serem preenchidos pelas donas).
  - `OperationRegistry` + `operations_list`/`operation_cancel` + evento `operation-updated`; `PackLocks`; canal `OperationEvent`.
  - `settings.json` com `schemaVersion` e migração; `settings_get`/`settings_update`.
  - `warden-secrets` com o trait `SecretStore` e as implementações cofre do sistema (`keyring`, padrão), arquivo `.env` na pasta de configuração (escolha do usuário) e cofre de teste em arquivo (só em debug), mais a troca de modo que move as chaves (ARCHITECTURE §14, ADR-0025); `secrets_status/set/test/remove` e `secrets_backend_get/set` (o `test` real de cada chave é ligado pelas tarefas das APIs; aqui só a infraestrutura); recuo de desenvolvimento por variável de ambiente só em debug.
  - Pastas de desenvolvimento (ADR-0048): só em build de debug, `AppPaths` usa `WARDEN_DATA_ROOT` (configuração e dados locais dentro dela) quando definida; com ela, a instância única vale só entre cópias com a mesma pasta, para o app de desenvolvimento não focar o Warden instalado.
  - Registros com `tracing` + arquivo diário + `tauri-plugin-log`; gancho de pânico; `tauri-plugin-single-instance`; capabilities da ARCHITECTURE §20.
- **Critérios de aceite:**
  1. `atomic_write` com falha injetada entre escrita e renomeação deixa o arquivo original intacto e nenhum `.warden-tmp` após a limpeza.
  2. `resolve_inside` recusa `../x`, caminhos absolutos, `C:\x` e links que saem da raiz (testes com `proptest`).
  3. `secrets_set` + reinício mantém o estado, com o cofre de teste em arquivo (local no Windows e no Linux), com o cofre real do Windows (só na CI Windows: localmente esse teste fica `#[ignore = "cofre-real"]`, para nunca tocar no cofre do dono) e no modo `.env`; trocar de modo move as chaves, e uma falha injetada no meio não perde nenhuma; nenhum valor de segredo aparece em `settings.json` nem nos registros (teste de varredura). CA-T21-01 e CA-T21-03 (parte de backend).
  4. Duas operações de escrita no mesmo pack são serializadas; em packs diferentes, rodam em paralelo (teste de concorrência).
  5. Cancelar uma operação longa simulada encerra em < 2 s com `CANCELLED`.
  6. Abrir uma segunda cópia do app foca a primeira; com `WARDEN_DATA_ROOT` diferente, as duas abrem, e nenhum arquivo é criado fora da pasta indicada (teste de integração).
- **Verificação:** `cargo nextest run -p warden-core -p warden-secrets -p warden-app`; `cargo xtask check`.

### F0-06 — Fundação do frontend

- **Prioridade:** P0 · **Depende de:** F0-01, F0-05 · **Branch:** `feat/f0-06-fundacao-frontend`
- **Objetivo:** base visual e técnica para todas as telas.
- **Posse:** `apps/desktop/src/{app,components,lib,styles}/**` (exceto `lib/ipc/bindings.ts`, gerado), `apps/desktop/src/routes/{__root.tsx,index.tsx}`, `apps/desktop/src/features/about/**` (exceto `licenses/`, da A-02), `apps/desktop/src/i18n/pt-BR/{comum.ts,navegacao.ts,sobre.ts,tarefas.ts}`, `apps/desktop/e2e/**` (harness), `apps/desktop/wdio.conf.ts`, `apps/desktop/vitest.config.ts`, `xtask/src/e2e.rs`.
- **Entregas:**
  - shadcn/ui + Tailwind 4 com os tokens do design system (`design/system/tokens.json`); **tema escuro fixo (Deep Dark), sem tema claro, e fontes embutidas no app** (`docs/design/HANDOFF.md` §1, DESIGN-SYSTEM §10 item 4; D7 corrigiu o texto antigo, que falava em tema claro/escuro seguindo o sistema); layout do nível do app **sem barra lateral** (ADR-0026): topo com o nome do app e Configurações, rodapé com o indicador de Tarefas em todas as telas (do app e do pack), que abre a gaveta T22.
  - Componentes comuns: `EmptyState`, `ErrorPanel` (com "Detalhes técnicos" e Copiar), `LoadingState`, `ConfirmDialog` (com confirmação por digitação), `ProgressBar`, `OperationToast`, `DiffView`, `SafeHtml`, `SafeMarkdown` (D4: `rehype-raw` antes de `rehype-sanitize`, com a lista de permissão da ARCHITECTURE §18; `iframe` vira miniatura com "Abrir no navegador").
  - `lib/ipc`: wrappers de query/mutation, assinatura de eventos com invalidação de `pack-changed`, tradução de `AppError`.
  - Gaveta Tarefas (T22) consumindo `operations_list`/`operation-updated`/`operation_cancel`; componente "Sobre o Warden" (T23) com aviso legal e versão, que a P1-13 encaixa como última seção de Configurações pelo registro `features/settings/sections.ts`.
  - Harness de testes: Vitest + `mockIPC` + fábricas a partir dos tipos gerados + `vitest-axe`; E2E WebdriverIO + `tauri-driver` com um teste de fumaça (abre o app e abre a gaveta de Tarefas) e servidor local de fixtures (`e2e/mock-server/`); os E2E rodam com `WARDEN_DATA_ROOT` e o cofre de teste em arquivo numa pasta temporária. `cargo xtask e2e-driver` baixa para o cache o `msedgedriver` da mesma versão do WebView2 instalado (Windows, sem instalar nada) e confere o `WebKitWebDriver` (Linux); o `wdio.conf.ts` usa o driver de cada plataforma.
  - Guarda de contrato (QUALITY §4.1): teste que compara comandos registrados com usos em `src/features/`.
- **Critérios de aceite:**
  1. Todos os componentes comuns têm testes de estado e passam no `axe`.
  2. `ErrorPanel` mostra frase traduzida para um `AppError` de cada domínio e "Detalhes técnicos" com código e `detail`.
  3. E2E de fumaça passa no Windows (local, com o `msedgedriver`) e no Linux (Xvfb, na CI).
  4. CA-T01-04 (aviso legal no componente "Sobre o Warden"), CA-T22-01 parcial (cancelamento pela interface, com operação simulada).
- **Verificação:** `pnpm -C apps/desktop test && pnpm -C apps/desktop e2e`; `cargo xtask check`.

---

## Fase 1 — Núcleo do pack

### P1-01 — `warden-packwiz`: formato, hashes e higiene

- **Prioridade:** P0 · **Depende de:** F0-01 · **Branch:** `feat/p1-01-formato-packwiz`
- **Objetivo:** ler e escrever o formato packwiz exatamente como o packwiz (ARCHITECTURE §6.1–§6.4).
- **Posse:** `crates/warden-packwiz/**`, `xtask/src/fixtures_packwiz.rs`.
- **Entregas:** modelo de `pack.toml`, `index.toml`, `.pw.toml` (todos os campos de R3 §1.5–§1.7, inclusive `[option]`, `pin`, `preserve`, `[export.curseforge]`, `[options]`); leitura tolerante (erro por arquivo); escrita de arquivo novo no estilo do packwiz; edição mínima com `toml_edit`; nomes únicos de metafile; mapeamento de lado do Modrinth; `.packwizignore` com padrões embutidos do packwiz; hashes sha1/sha256/sha512/md5/murmur2 (variante CurseForge); templates de `.packwizignore`, `.gitignore`, `.gitattributes` (D4: com o bloco "Segredos e arquivos de ferramentas" e `/server-overrides/` da ARCHITECTURE §6.4, decisão D21); verificação de higiene (padrões da §6.4); `cargo xtask fixtures-packwiz` gera fixtures com o packwiz real.
- **Critérios de aceite:**
  1. Para cada fixture gerada pelo packwiz real (Modrinth, CurseForge, URL, `pack.toml` de cada loader, `index.toml` com `preserve`), ler e escrever produz bytes idênticos.
  2. Um `.pw.toml` escrito pelo Warden para o mesmo mod é idêntico ao do packwiz (base de CA-T08-01 e CA-T08-03).
  3. `murmur2` confere com as impressões digitais da CurseForge para 5 jars reais.
  4. O matcher de `.packwizignore` dá o mesmo resultado que o packwiz para um conjunto de 100 caminhos (comparado executando `packwiz refresh` num pack de teste).
  5. Um `.pw.toml` inválido produz erro só para aquele arquivo.
  6. CA-T04-05 (no nível da crate: os templates e a higiene cobrem `kubejs/config/web_server.json`, `.probe/`, `.vscode/`, `local/kubejs/` e os arquivos do spark).
- **Verificação:** `cargo nextest run -p warden-packwiz`; cobertura ≥ 85%.

### P1-02 — `warden-packwiz-cli`: execução do sidecar, staging e conformidade

- **Prioridade:** P0 · **Depende de:** F0-03, P1-01, F0-05 · **Branch:** `feat/p1-02-packwiz-cli`
- **Objetivo:** chamar o packwiz com segurança e provar que um pack está "como o packwiz produziria" (ARCHITECTURE §6.3).
- **Posse:** `crates/warden-packwiz-cli/**`.
- **Entregas:** construtores de argv puros (`refresh`, `refresh --build`, `curseforge add <url>`, `list`, `modrinth export`, `curseforge export`), sempre com `--pack-file pack.toml` **relativo** e o diretório de trabalho na pasta do pack ou da cópia de staging (D7: com `--pack-file` absoluto o packwiz ignora os padrões ancorados do `.packwizignore` e todo `add` exige `--meta-folder-base` absoluto; achado da F0-03 e da P1-01, ARCHITECTURE §6.3, ADR-0049); falta da chave da CurseForge reconhecida pelo começo da mensagem do patch, `WARDEN_CURSEFORGE_API_KEY ausente` (constante `MISSING_KEY_MESSAGE` do `xtask/src/packwiz.rs`), virando `CURSEFORGE_KEY_MISSING`; execução com tempo-limite, cancelamento, ambiente limpo, `CREATE_NO_WINDOW`, stdin controlado; decodificação de linhas e remoção de ANSI/progresso; detecção de falha por código e pós-condição; cópia de staging; `check_conformance(pack)` (refresh em staging + zero diferença).
- **Critérios de aceite:**
  1. `refresh` num pack de teste com 300 metafiles termina com sucesso e o índice relido confere.
  2. Um pack com `index.toml` desatualizado é reprovado por `check_conformance` com a lista de diferenças.
  3. Cancelar um `refresh` lento (pack sintético grande) mata o processo em < 2 s.
  4. Saída do packwiz com ANSI e barra de progresso vira linhas limpas (teste dourado com saída real gravada).
  5. A chave nunca aparece no argv nem nos registros (teste).
  6. Um `.packwizignore` com padrão ancorado (`/local/`) é respeitado pelo `refresh` chamado pela crate, e um `curseforge add` com dependência termina sem `.pw.toml` fora do índice (testes com o sidecar real; D7).
- **Verificação:** `WARDEN_REQUIRE_EXTERNALS=1 cargo nextest run -p warden-packwiz-cli`.

### P1-03 — `warden-http` e `warden-modrinth`

- **Prioridade:** P0 · **Depende de:** F0-05 · **Branch:** `feat/p1-03-http-modrinth`
- **Objetivo:** cliente HTTP comum e cliente completo do Modrinth (ARCHITECTURE §17).
- **Posse:** `crates/warden-http/**`, `crates/warden-modrinth/**`.
- **Entregas:** User-Agent no formato que a documentação do Modrinth pede (conferir na tarefa e registrar a fonte; R7 §9 item 5), limitador por host, novas tentativas, download em streaming com hash e `.part`; D7: antes de pedir partes ou retomar com `Range`, o download resolve os redirecionamentos com um pedido sem `Range` e usa a URL final (o `edge.forgecdn.net` responde 404 a qualquer pedido com `Range` e só o `mediafilez.forgecdn.net`, para onde redireciona, aceita; R7 §9 item 1); leitura do lado pelo campo `environment` da **versão** e do projeto, mantendo `client_side`/`server_side` como reserva enquanto a API devolver os dois (R7 §9 item 4); endpoints do Modrinth (busca com facets, projeto(s), versões, `version_files`, `version_files/update`, tags); cache SQLite de projetos/versões; códigos de erro e traduções do domínio. Gancho 1.1: o download calcula de uma vez sha1, sha256, sha512 e o murmur2 da CurseForge, e o cache do Modrinth guarda `status`, `updated` e `game_versions` dos projetos (ADR-0039).
- **Critérios de aceite:**
  1. 429 com `X-Ratelimit-Reset` é respeitado (teste com `wiremock`).
  2. Download interrompido retoma com `Range` e o hash final confere.
  3. Testes de rede: busca "sodium" com facets de Fabric 1.21.1 retorna o Sodium; `version_files` identifica um jar real.
  4. Base de CA-T10-01: verificação de atualizações de 200 hashes usa uma requisição.
- **Verificação:** `cargo nextest run -p warden-http -p warden-modrinth`; `cargo xtask test-network`.

### P1-04 — `warden-curseforge`

- **Prioridade:** P0 · **Depende de:** P1-03, P1-01 · **Branch:** `feat/p1-04-curseforge`
- **Objetivo:** cliente da CurseForge com a chave do usuário, respeitando os termos (sem persistência).
- **Posse:** `crates/warden-curseforge/**`.
- **Entregas:** busca (com `modLoaderType`, `gameVersion`, classes de mods/resource packs/shaders — confirmar o `classId` de shaders pela API), mod(s), arquivos, `download-url`, impressões digitais, detecção de distribuição bloqueada; cache só em memória; ligação do `secrets_test` para a CurseForge. D7: o cabeçalho `x-api-key` vai também nos downloads da CDN da CurseForge (`edge.forgecdn.net` e o redirecionamento), como fazem o Prism 11.0.3 e o FTB App 1.30.0, para não quebrar se a CurseForge voltar a exigir chave nos downloads (R7 §9 item 2, ADR-0051): a crate expõe os cabeçalhos de download que a `warden-instance` (L-03) e a exportação (E-03, E-04) usam.
- **Critérios de aceite:**
  1. Teste de rede: buscar "jei" para Forge 1.20.1 retorna o JEI; um mod com distribuição bloqueada conhecido é identificado como bloqueado.
  2. Chave inválida → `CURSEFORGE_KEY_INVALID`; sem chave → `CURSEFORGE_KEY_MISSING` sem requisição (CA-T08-06 parte backend, CA-T21-02).
  3. Um download da CDN da CurseForge leva `x-api-key` (servidor simulado que confere o cabeçalho) e a chave não aparece nos registros.
  4. Nenhuma resposta da API da CurseForge (JSON, descrições, URLs de download) é escrita em disco (teste que inspeciona o diretório de dados após uma sessão, excluindo o cache de arquivos de mods baixados).
- **Verificação:** `cargo nextest run -p warden-curseforge`; `cargo xtask test-network`.

### P1-05 — `warden-catalog`

- **Prioridade:** P0 · **Depende de:** P1-03 · **Branch:** `feat/p1-05-catalogo-versoes`
- **Objetivo:** listas de versões do Minecraft e dos loaders a partir das fontes primárias (R2 §8.1 item 1).
- **Posse:** `crates/warden-catalog/**`, `apps/desktop/src-tauri/src/commands/catalog.rs`.
- **Entregas:** manifesto da Mojang na ordem oficial (nunca semver), JSON de versão com cache por `sha1`, versões Fabric por MC, Forge por MC (`maven-metadata.xml` + `promotions_slim.json`, tratando o sufixo `-1.7.10`), NeoForge (incluindo `net.neoforged:forge` para 1.20.1; filtro com ponto final), denylist (`1.12.2-14.23.5.2851`), etiqueta "melhor esforço" para < 1.7.10; comandos `catalog_minecraft_versions` e `catalog_loader_versions`.
- **Critérios de aceite:**
  1. `26.3` aparece antes de `1.21.11`, que aparece antes de `1.21.2` (ordem do manifesto).
  2. Forge 1.7.10 lista `10.13.4.1614` (sem sufixo) e marca a recomendada; NeoForge não existe para 1.19.2; Fabric não existe para 1.12.2 (CA-T03-03).
  3. Sem rede, usa o cache e informa a data.
- **Verificação:** `cargo nextest run -p warden-catalog` (fixtures reais gravadas); `cargo xtask test-network`.

### P1-06 — `warden-jarmeta`

- **Prioridade:** P0 · **Depende de:** F0-01 · **Branch:** `feat/p1-06-metadados-jar`
- **Objetivo:** ler metadados de mods direto dos jars (R3 §4).
- **Posse:** `crates/warden-jarmeta/**`.
- **Entregas:** leitura sem extrair de `fabric.mod.json` (com `jars[]` recursivo, `provides`), `quilt.mod.json` (identificação), `mods.toml`, `neoforge.mods.toml`, `META-INF/jarjar/metadata.json`, `mcmod.info` tolerante, `MANIFEST.MF` (coremods), versão de classe; modelo normalizado (R3 §4.5); avaliadores de faixa Fabric e Maven; FlexVer; parser de `@Mod.dependencies` em texto (leitura do bytecode fica para o backlog).
- **Critérios de aceite:**
  1. Corpus de pelo menos 30 jars reais (Fabric, Forge 1.7.10/1.12.2/1.20.1, NeoForge 1.21.1, multi-loader, jar-in-jar) produz o modelo esperado (dourado).
  2. `mcmod.info` com vírgula sobrando é lido; jar corrompido gera erro, não pânico (teste com entradas aleatórias).
  3. Faixas: casos de teste da especificação do Fabric e de Maven passam.
- **Verificação:** `cargo nextest run -p warden-jarmeta`; cobertura ≥ 85%.

### P1-07 — Packs: lista, criar e abrir existente

- **Prioridade:** P0 · **Depende de:** P1-01, P1-02, P1-05, V-01, F0-06 · **Branch:** `feat/p1-07-packs`
- **Objetivo:** telas T02, T03 e T04 completas, de ponta a ponta.
- **Posse:** `crates/warden-project/src/{registry.rs,create.rs,open.rs,hygiene.rs,transaction.rs,trash.rs}` (e `lib.rs` inicial), `apps/desktop/src-tauri/src/commands/packs.rs`, `apps/desktop/src/features/packs/**`, `apps/desktop/src/routes/packs/{index.tsx,novo.tsx,abrir.tsx}`, `apps/desktop/src/i18n/pt-BR/packs.ts`, `apps/desktop/e2e/packs.e2e.ts`.
- **Entregas:** (Gancho 1.1: o leitor de `.warden/` preserva arquivos e tabelas que não conhece; o nome `.warden/mods.toml` fica reservado; ADR-0039) registro de packs (`packs.json`), `PackTransaction` com bloco obrigatório do `.packwizignore` (ARCHITECTURE §6.4–§6.5), criação sem `packwiz init` (com `git` via V-01), importação (`pack_import`) com validações, repositórios git existentes (ARCHITECTURE §11) e higiene, limpeza de sobras `.warden-tmp`, telas com todos os estados; (P1) Apagar pack para a Lixeira (`pack_trash`). D4: o botão de Meus packs é **Abrir ou importar…** (detecta pasta packwiz; outros formatos ficam desabilitados com o motivo até a P1-19); o assistente Criar pack tem 5 etapas, com a etapa "Mods iniciais" registrada pela P1-18 (até lá, a etapa não aparece); a tabela de Meus packs deixa a coluna Saúde para a D-08.
- **Critérios de aceite:** CA-T02-01 a CA-T02-04, CA-T03-01 a CA-T03-06, CA-T04-01 a CA-T04-04; `PackTransaction` reverte corretamente com falha injetada em cada passo.
- **Verificação:** `WARDEN_REQUIRE_EXTERNALS=1 cargo nextest run -p warden-project`; `pnpm -C apps/desktop test -- packs`; E2E `packs.e2e.ts`.

### P1-08 — Editor do pack: cabeçalho, inventário e detalhes

- **Prioridade:** P0 · **Depende de:** P1-07, P1-03, L-01 · **Branch:** `feat/p1-08-editor-do-pack`
- **Objetivo:** telas T05, T06 e T07 e a parte P0 de T11 (Informações do pack e o diálogo Ajustes do teste), na estrutura aprovada (ADR-0026).
- **Posse:** `crates/warden-project/src/{inventory.rs,details.rs,side.rs,remove.rs,meta.rs}`, `apps/desktop/src-tauri/src/commands/{inventory.rs,pack_meta.rs}`, `apps/desktop/src/features/pack-editor/**`, `apps/desktop/src/routes/packs/$packId/{route.tsx,index.tsx,mods.tsx}`, `apps/desktop/src/i18n/pt-BR/editor.ts`, `apps/desktop/e2e/editor.e2e.ts`.
- **Entregas:** layout do pack (cabeçalho fixo com ← Meus packs, nome e "Editar informações", identificação, avisos passageiros e "Salvar versão · N alterações"; menu lateral com as 6 seções, com descrição e contador; o pack abre em Mods); lista única de mods, resource packs e shaders agrupada por tipo; inventário guiado pelo índice e tolerante a erros, versão legível pelo cache, detalhes em painel lateral (Modrinth do cache; CurseForge ao vivo), lado individual e em lote, remover (com dependentes), Informações do pack (nome/autor/descrição no `pack.toml`) e o diálogo Ajustes do teste neste computador (memória, Java automático com motivo via `java_choice` da L-01, argumentos, recriar instância; gravados em `packs.json`, com efeito real a partir da L-04, que o expõe no menu ▾ do Testar). Cria os pontos de extensão `features/pack-editor/header/slots.ts` (botões e indicadores do cabeçalho registrados por L-04, V-02, C-03, D-03 e A-05), `features/pack-editor/sections.ts` (seções do menu, registradas por C-02, D-03, D-04, V-02 e E-01) e, na D4, `features/pack-editor/details/blocks.ts` (blocos do painel de detalhes, registrados por D-07 e D-10); o diálogo Ajustes do teste fica em `features/pack-editor/test-settings/` (a L-08 acrescenta os perfis); o layout do pack aceita o estado "menu recolhido" (ícones com tooltip) usado pela página de descoberta (P1-09). Gancho 1.1: cada item tem uma chave estável (projeto do Modrinth ou da CurseForge, ou o caminho do metafile) e o agrupamento da lista por tipo é uma função genérica de agrupamento, para a W-08 acrescentar "por grupo" (ADR-0039). As descrições das seções seguem a ESTRUTURA §13 (Configs: "Arquivos de ajuste e scripts do pack"; Problemas: "Saúde do pack, problemas e travamentos"; ✦ Diagnóstico com IA: "Conversar com a IA sobre um problema do pack").
- **Critérios de aceite:** CA-T05-01 a CA-T05-03, CA-T06-01 a CA-T06-04, CA-T07-01, CA-T07-02, CA-T11-01, CA-T11-03.
- **Verificação:** `cargo nextest run -p warden-project`; testes de componente; E2E `editor.e2e.ts`.

### P1-09 — Adicionar: página, busca combinada e Modrinth (com dependências)

- **Prioridade:** P0 · **Depende de:** P1-08, P1-06 · **Branch:** `feat/p1-09-adicionar-modrinth`
- **Objetivo:** página Adicionar sem abas (Tipo, campo único, Escolher arquivo), em tela cheia com o menu lateral recolhido (D4, ESTRUTURA N6), com seleção múltipla, e o motor da busca combinada (ADR-0027) com a fonte Modrinth, mais a tela de dependências para um ou vários itens (T08, T09).
- **Posse:** `crates/warden-project/src/add/{mod.rs,plan.rs,modrinth.rs}`, `crates/warden-project/src/search/{mod.rs,merge.rs,modrinth.rs}`, `crates/warden-project/src/{deps.rs,dedup.rs}`, `apps/desktop/src-tauri/src/commands/add.rs`, `apps/desktop/src/features/add/{AddPanel.tsx,common/**,modrinth/**,dependencies/**}`, `apps/desktop/src/i18n/pt-BR/adicionar.ts`, `apps/desktop/e2e/add-modrinth.e2e.ts`.
- **Entregas:** `add_plan`/`add_apply` para um ou vários itens (estrutura usada também por P1-10, P1-11, P1-16, P1-17, P1-18 e P1-19); `search_projects` com fontes plugáveis (trait de fonte), intercalação por posição, mescla por números ao ordenar por downloads ou atualização, deduplicação por autor e nome/slug, paginação por fonte e resultado parcial com `SEARCH_SOURCE_UNAVAILABLE` (ARCHITECTURE §17); fonte Modrinth com filtros travados; resultados com caixa de seleção e a barra "N selecionados · Adicionar N ao pack"; pré-visualização com seletores de fonte, versão e canal e a descrição higienizada; resolução recursiva de dependências obrigatórias (comuns a vários itens aparecem uma vez), incompatibilidades declaradas (inclusive entre os itens escolhidos), deduplicação entre fontes, gravação atômica de vários itens; página em tela cheia com "← Voltar para Mods · N adicionados".
- **Critérios de aceite:** CA-T08-01, CA-T08-11, CA-T08-12, CA-T09-01 a CA-T09-05; intercalação, deduplicação e falha parcial cobertas por testes do motor com duas fontes simuladas.
- **Verificação:** testes de domínio com `wiremock` + integração com packwiz real (o pack resultante passa em `check_conformance`); E2E com servidor de fixtures.

### P1-10 — CurseForge na busca combinada e link da CurseForge

- **Prioridade:** P0 · **Depende de:** P1-09, P1-04, P1-02 · **Branch:** `feat/p1-10-adicionar-curseforge`
- **Objetivo:** CurseForge como fonte da busca combinada (T08, ADR-0027) e importação por link via packwiz em staging (ARCHITECTURE §6.1).
- **Posse:** `crates/warden-project/src/add/{curseforge.rs,link_curseforge.rs}`, `crates/warden-project/src/search/curseforge.rs`, `apps/desktop/src-tauri/src/commands/add_curseforge.rs`, `apps/desktop/src/features/add/curseforge/**`, `apps/desktop/src/i18n/pt-BR/adicionar-curseforge.ts`, `apps/desktop/e2e/add-curseforge.e2e.ts`.
- **Entregas:** fonte CurseForge da busca combinada; estados "sem chave" e "chave recusada" (só Modrinth, com aviso, sem requisição à CurseForge); itens marcados "CurseForge" ou unificados "Modrinth e CurseForge", com conferência por SHA-1 na pré-visualização; marca "Download manual necessário"; `.pw.toml` com `mode = "metadata:curseforge"` idêntico ao do packwiz; importação de link pelo packwiz em staging (link de arquivo direto; link de projeto com seletor de versão e `--addon-id`/`--file-id`), com stdin `n` para dependências e importação pelo caminho normal; lado padrão `both` com aviso.
- **Critérios de aceite:** CA-T08-02, CA-T08-03, CA-T08-06, CA-T08-08, CA-T08-09.
- **Verificação:** integração com packwiz real (link), `cargo xtask test-network` (busca real), E2E com fixtures.

### P1-11 — Adicionar por link e arquivo local

- **Prioridade:** P0 · **Depende de:** P1-09, P1-10 · **Branch:** `feat/p1-11-adicionar-link-e-arquivo`
- **Objetivo:** link colado no campo único e arquivo do computador em T08 (botão "Escolher arquivo do computador…" e arrastar arquivos para a lista).
- **Posse:** `crates/warden-project/src/add/{link.rs,url.rs,local.rs}`, `apps/desktop/src-tauri/src/commands/{add_link.rs,add_local.rs}`, `apps/desktop/src/features/add/{link/**,local/**}`, `apps/desktop/src/i18n/pt-BR/adicionar-arquivo.ts`, `apps/desktop/e2e/add-local.e2e.ts`.
- **Entregas:** classificação de links (Modrinth, CurseForge, direto), recusa de `http://`, download e hash de link direto, leitura de metadados de jars locais, identificação por hash no Modrinth e na CurseForge com oferta de referência, cópia de arquivo local, bloqueio de loader/versão com "Adicionar mesmo assim".
- **Critérios de aceite:** CA-T08-04, CA-T08-05, CA-T08-07.
- **Verificação:** integração com packwiz real; testes de componente; E2E.

### P1-12 — Atualizações

- **Prioridade:** P0 (individual) / P1 (todas) · **Depende de:** P1-09, P1-10 · **Branch:** `feat/p1-12-atualizacoes`
- **Objetivo:** T10 completa.
- **Posse:** `crates/warden-project/src/updates.rs`, `apps/desktop/src-tauri/src/commands/updates.rs`, `apps/desktop/src/features/updates/**`, `apps/desktop/src/i18n/pt-BR/atualizacoes.ts`, `apps/desktop/e2e/updates.e2e.ts`.
- **Entregas:** produtor único do relatório de atualizações (Modrinth em lote; CurseForge em lote), estados, verificação automática configurável, atualizar um item com changelog, "Atualizar todos" com revisão e ponto de segurança (via V-01).
- **Critérios de aceite:** CA-T10-01 a CA-T10-04.
- **Verificação:** testes com contagem de requisições (`wiremock`); integração com packwiz real; E2E.

### P1-13 — Configurações do app e primeira execução

- **Prioridade:** P0 · **Depende de:** F0-05, F0-06 · **Branch:** `feat/p1-13-configuracoes`
- **Objetivo:** T01 e T21 numa página única com as seções na ordem da SPEC, incluindo "Sobre o Warden" (componente da F0-06) como última (exceto "Armazenamento", P1, e itens que dependem de tarefas futuras, que ficam ocultos até existirem).
- **Posse:** `apps/desktop/src/features/settings/**` (exceto as subpastas `java/`, de L-01, `ai/`, de D-04, e `storage/`, de A-06), o comando `settings_choose_packs_dir` em `apps/desktop/src-tauri/src/commands/app.rs` (a F0-05 já integrada), `apps/desktop/src/features/onboarding/**`, `apps/desktop/src/routes/{configuracoes.tsx,boas-vindas.tsx}`, `apps/desktop/src/i18n/pt-BR/{configuracoes.ts,boas-vindas.ts}`, `apps/desktop/e2e/settings.e2e.ts`.
- **Entregas:** assistente de primeira execução, tela de Configurações com chaves (status, testar, substituir, remover) e "Onde guardar as chaves" (cofre ou `.env`, com a confirmação e o aviso fixo), nome do jogador validado, pasta padrão (D7: `packsDir` não muda pelo `settings_update`; o comando `settings_choose_packs_dir` abre o diálogo nativo de pasta pela API Rust do plugin `dialog`, valida a pasta e grava o `settings.json`, como manda a ARCHITECTURE §4.1), canal de versões, diferenças antes de salvar, nível de registros.
- **Critérios de aceite:** CA-T01-01 a CA-T01-03, CA-T21-01 a CA-T21-03 (CA-T21-02 com P1-04 integrado; até lá, teste com `secrets_test` simulado).
- **Verificação:** testes de componente; E2E `settings.e2e.ts`.

### P1-14 — Mods opcionais e fixar versão

- **Prioridade:** P1 · **Depende de:** P1-08, L-03 · **Branch:** `feat/p1-14-opcionais-e-fixar`
- **Objetivo:** parte P1 de T11: opcionais, fixar versão e escolha dos opcionais no teste.
- **Posse:** `crates/warden-project/src/{optional.rs,pin.rs}`, `apps/desktop/src-tauri/src/commands/item_options.rs`, `apps/desktop/src/features/pack-editor/item-options/**`, `apps/desktop/src/i18n/pt-BR/opcionais.ts`.
- **Entregas:** editar `[option]` e `pin`; marcas na lista; avisos de perda em outros formatos; escolha dos opcionais ligados na instância (`instance_set_optional_choices`, gravado no estado da instância pela API da `warden-instance`).
- **Critérios de aceite:** CA-T11-02; itens fixados excluídos de "Atualizar todos" (teste com P1-12 quando integrado).
- **Verificação:** integração com packwiz-installer real (conformidade de opcionais).

### P1-15 — Trocar versões e limpar dependências órfãs

- **Prioridade:** P1 · **Depende de:** P1-09, P1-12, D-07 · **Branch:** `feat/p1-15-trocar-versoes`
- **Objetivo:** itens P1 de T06, T07 e T11 que dependem da resolução de dependências.
- **Posse:** `crates/warden-project/src/{change_version.rs,loader_version.rs,orphans.rs}`, a função de plano em `crates/warden-project/src/remove.rs` (depois da P1-08 integrada), `apps/desktop/src-tauri/src/commands/versions_change.rs`, `apps/desktop/src/features/pack-editor/change-version/**`, `apps/desktop/src/i18n/pt-BR/trocar-versao.ts`.
- **Entregas:** "Trocar de versão" de um item (qualquer versão compatível, inclusive mais antiga); trocar a versão do loader do pack (mesmo loader) com verificação de compatibilidade dos itens; ao remover, mostrar os dependentes transitivos e oferecer remover dependências órfãs, pelas consultas do grafo da D-07 (D4: a P1-15 troca a consulta de dependentes do `items_remove_plan` em `crates/warden-project/src/remove.rs` pela consulta transitiva).
- **Critérios de aceite:** trocar a versão de um mod grava a referência nova e passa em `check_conformance`; trocar a versão do loader altera só a linha correspondente em `[versions]` e roda o diagnóstico rápido; dependências órfãs são listadas corretamente num pack de teste com cadeia de dependências; CA-T06-06 pela interface (diálogo de remover).
- **Verificação:** integração com packwiz real; testes de componente.


### P1-16 — Descoberta: início, categorias e pré-visualização completa

- **Prioridade:** P0 (início, categorias) / P1 (galeria, versões com changelog, dependências, links, imagens da CurseForge, kits no início) · **Depende de:** P1-09, P1-10 · **Branch:** `feat/p1-16-descoberta`
- **Objetivo:** completar a página de descoberta de T08 (D4; R5B §7; ARCHITECTURE §17.1).
- **Posse:** `crates/warden-discovery/**` (exceto `src/modpacks/**`, da P1-17), `apps/desktop/src-tauri/src/commands/discover.rs`, `apps/desktop/src-tauri/src/img_protocol.rs`, `apps/desktop/src/features/add/{discover/**,preview/**}`, `apps/desktop/src/i18n/pt-BR/descoberta.ts`, `apps/desktop/e2e/discover.e2e.ts`.
- **Entregas:** `discover_home` (populares e atualizados para o pack, no máximo 3 requisições por fonte) e `discover_categories` com o mapeamento curado `data/categories.toml`; coluna de filtros (categorias, ambiente, ordenar, fonte em "Mais filtros") que recolhe atrás de "Filtros" abaixo de 1180 px; pré-visualização rolável com índice "Descrição · Galeria · Versões · Dependências · Links" (P1: galeria com imagem grande sobre a página, versões com changelog sob demanda, dependências com "Já no pack"/"Será adicionada", links); protocolo `warden-img://` para imagens da CurseForge (só memória, `no-store`); cache de descrição e galeria do Modrinth (24 h); P1: "Kits de desempenho" no início (lista da P1-18, que abre o diálogo de dependências).
- **Critérios de aceite:** CA-T08-10, CA-T08-13.
- **Verificação:** testes de domínio com `wiremock` contando requisições; testes de componente (`axe`); E2E com servidor de fixtures; inspeção da pasta de dados do WebView depois de uma sessão com imagens da CurseForge.

### P1-17 — Navegar por modpacks

- **Prioridade:** P1 · **Depende de:** P1-16, P1-04 · **Branch:** `feat/p1-17-modpacks`
- **Objetivo:** Tipo = Modpacks na página de descoberta, com a lista de mods de um modpack e "Adicionar N selecionados" (T08; R5B §6).
- **Posse:** `crates/warden-discovery/src/modpacks/**`, `crates/warden-http/src/range_reader.rs`, `apps/desktop/src-tauri/src/commands/modpacks.rs`, `apps/desktop/src/features/add/modpacks/**`, `apps/desktop/src/i18n/pt-BR/modpacks.ts`.
- **Entregas:** busca de modpacks nas duas fontes (Modrinth `project_type:modpack`; CurseForge `classId=4471`); `modpack_contents` pelo Modrinth (dependências `embedded` da versão, resolvidas em lote) e, para detalhes ou na CurseForge, pelo `modrinth.index.json`/`manifest.json` lidos por partes (`range_reader`: `Read + Seek` sobre HTTP Range, com recuo para download completo até 200 MB; D7: o redirecionamento é resolvido antes, sem `Range`, porque o `edge.forgecdn.net` responde 404 a pedidos com `Range` e só o `mediafilez.forgecdn.net` aceita, R7 §9 item 1; um teste com servidor simulado que imita os dois hosts cobre CA-T08-15 e CA-T08-16); estados por mod (Já no seu pack, Compatível, Sem versão para o seu pack, Arquivo fora das lojas); filtro "Só os que não estão no seu pack"; seletor Versão (a mesma do modpack / a mais nova compatível); licença do projeto com aviso; "Adicionar N selecionados" pelo `add_plan` de vários itens; CurseForge só em memória; mensagem para modpack bloqueado a terceiros.
- **Critérios de aceite:** CA-T08-14, CA-T08-15, CA-T08-16.
- **Verificação:** testes de domínio com dados reais gravados (Create+) e servidor que conta requisições e bytes; testes de componente; E2E.

### P1-18 — Mods iniciais, ferramentas do jogador e kits de desempenho

- **Prioridade:** P0 (mods iniciais e ferramentas do jogador) / P1 (kits) · **Depende de:** P1-07, P1-09, P1-04, L-03 · **Branch:** `feat/p1-18-mods-iniciais-e-kits`
- **Objetivo:** etapa "Mods iniciais" do Criar pack, spark e Crash Assistant como ferramentas do jogador e kits de desempenho em dados versionados (T03, T08, T13; ADR-0033; decisão D16).
- **Posse:** `crates/warden-project/data/{initial-mods.toml,kits.toml}`, `crates/warden-project/src/{kits.rs,player_tools.rs,initial_mods.rs}`, `crates/warden-instance/src/player_tools.rs` e a chamada do filtro em `crates/warden-instance/src/materialize.rs` (a L-03 já integrada), `xtask/src/check_kits.rs`, `.github/workflows/kits.yml`, `apps/desktop/src/features/packs/create/initial-mods/**`, `apps/desktop/src/features/add/kits/**`, `apps/desktop/src/i18n/pt-BR/mods-iniciais.ts`.
- **Entregas:** `initial-mods.toml` por faixa (spark do Modrinth de 1.16.5 em diante, com a Fabric API no Fabric; spark da CurseForge 1.10.19 para 1.7.10 e 1.6.3 para 1.12.2 com o aviso "versão antiga"; Crash Assistant em todas as faixas) e `kits.toml` (IDs do Modrinth, nunca slugs; itens só da CurseForge pelo ID; partida = R1 §2.3.5); etapa "Mods iniciais" registrada no assistente da P1-07, com os dois marcados e o kit desmarcado; adição pelo `add_plan` de vários itens num ponto único depois do "Pack criado"; config inicial do Crash Assistant com o envio ao autor, o encurtador de links e a verificação de pirataria desligados (só chaves documentadas pelo autor do mod; o nome do arquivo e as chaves são confirmados na tarefa e registrados no relatório); `[player-tools]` em `.warden/project.toml`; filtro na materialização (Crash Assistant fora dos testes normais e da busca; spark fora só da busca); `cargo xtask check-kits` (consulta a API do Modrinth por item e faixa; item sem versão vira aviso no relatório) e o workflow semanal; "Kits de desempenho" para a P1-16 e para a etapa do assistente.
- **Critérios de aceite:** CA-T03-07, CA-T03-08, CA-T13-15; `cargo xtask check-kits` passa com os dados reais e falha num kit de teste com ID inexistente.
- **Verificação:** testes de domínio e integração com servidor simulado; `cargo xtask check-kits` (rede); testes de componente; integração da materialização.

### P1-19 — Importar modpack (`.mrpack`, CurseForge e Prism)

- **Prioridade:** P1 · **Depende de:** P1-07, P1-02, P1-04, P1-06, L-08 · **Branch:** `feat/p1-19-importar`
- **Objetivo:** T24 (ADR-0035; R5B §5.1; ARCHITECTURE §12.2).
- **Posse:** `crates/warden-import/**`, `apps/desktop/src-tauri/src/commands/import.rs`, `apps/desktop/src/features/packs/import/**`, `apps/desktop/src/routes/packs/importar.tsx`, `apps/desktop/src/i18n/pt-BR/importar.ts`, `apps/desktop/e2e/import.e2e.ts`, corpus em `crates/warden-import/tests/corpus/**` (com a origem de cada arquivo anotada).
- **Entregas:** detecção pelo conteúdo a partir do "Abrir ou importar…" da P1-07; conversão de `.mrpack` (proteção contra *zip slip*, metafiles do Modrinth pelo caminho da URL ou por `version_files` em lote, modo `url` para outros hosts, `env` → lado com valores desconhecidos como "Cliente e servidor" e aviso, opcionais para revisão, `overrides/`, jars soltos por hash, `client-overrides/` com aviso, `server-overrides/`); zip da CurseForge pelo `packwiz curseforge import` em staging com correção do lado pelo Modrinth; instância do Prism/MultiMC (`mmc-pack.json`, `mods/.index/*.pw.toml` sem os campos `x-prismlauncher-*`, jars por hash, `instance.cfg` → perfil do teste da L-08, pergunta sobre `options.txt`/`servers.dat`) e do app da CurseForge; higiene obrigatória; página "Importar modpack" com o resumo (referência, arquivo local, revisar, não entra) e a licença do projeto; criação do pack pela rotina do T03 seguida de `check_conformance`. Gancho 1.1: os jars que ficam como arquivo local são marcados com a origem da importação em `.warden/project.toml` (ADR-0039).
- **Critérios de aceite:** CA-T24-01 a CA-T24-04.
- **Verificação:** integração com o packwiz real e servidores simulados; testes de segurança com zips maliciosos; E2E.

---

## Fase 2 — Launcher

### L-01 — `warden-java`

- **Prioridade:** P0 · **Depende de:** P1-03 · **Branch:** `feat/l-01-java`
- **Objetivo:** escolher, baixar e validar o Java certo (ARCHITECTURE §7.3, ADR-0012).
- **Posse:** `crates/warden-java/**`, `apps/desktop/src-tauri/src/commands/java.rs`, `apps/desktop/src/features/settings/java/**`.
- **Entregas:** Adoptium (JRE 8/17/21/25, SHA-256), runtime da Mojang como alternativa, instalação atômica, descoberta e validação, política "o mais novo que funciona" (ADR-0029) com a tabela `crates/warden-java/data/compatibility.toml` e testes para cada regra, `java_choice` com motivo, atualização para a versão mais recente de cada major (`java_runtimes_check_updates`, remoção da antiga quando sem uso), comandos `java_runtimes_list`/`java_runtime_remove` e a seção Java de Configurações (tabela com os packs que usam cada Java e o motivo).
- **Critérios de aceite:**
  1. A política escolhe: 8 para Forge 1.7.10 e 1.12.2; 8 ≤ u312 para Forge 1.16.5 36.2.25; 17 para 1.17.1 e 1.20.1; 21 para 1.21.1; 25 para 26.3.
  2. Download interrompido não deixa runtime parcial instalado.
  3. Teste de rede: baixa e valida Temurin 21 no Linux e no Windows (CI noturna).
  4. Cada escolha da política vem com o motivo esperado; a tabela de compatibilidade é validada (faixas sem sobreposição, majors existentes no Adoptium).
  5. CA-T21-04.
- **Verificação:** `cargo nextest run -p warden-java`; `cargo xtask test-network`.

### L-02 — `warden-launcher`: motor, perfil offline e processo

- **Prioridade:** P0 · **Depende de:** S1 (decisão do motor), L-01, P1-05 · **Branch:** `feat/l-02-launcher`
- **Objetivo:** implementar a interface `LauncherEngine` com o motor escolhido no S1, o perfil offline e a supervisão do processo (ARCHITECTURE §7).
- **Posse:** `crates/warden-launcher/**`, `docs/decisions/` (ADR novo registrando a escolha do motor e eventuais ajustes da interface).
- **Entregas:** adaptador do motor (D7: sempre com a **versão exata do loader**, a do `pack.toml`, nunca "estável" ou "mais nova": sem versão fixa o portablemc consulta o meta do loader na rede a cada abertura, e duas rodadas do S-R5-3 falharam na instalação por isso; S-R5-3 §13), `GameSpec`/`LaunchOptions`/`LaunchCommand`, UUID offline, argumentos por faixa de versão, `@argfile`, processo com pipes, decodificação, parser log4j XML/texto, Job Object no Windows, classificação da saída, gravação da sessão.
- **Critérios de aceite:**
  1. UUID offline igual ao do Java para 10 nomes (valores gerados por um programa Java no teste).
  2. Golden tests da linha de comando para cada combinação da matriz L-05.
  3. CA-T13-02, CA-T13-03, CA-T13-07 (com processo Java de teste que imita o jogo: escreve acentos, cria filhos, trava).
  4. A escolha do motor e o motivo estão num ADR, com o resultado do S1 citado.
  5. Com a versão do loader fixada e a instalação no cache, abrir de novo não faz nenhuma requisição ao meta do loader (servidor simulado que conta pedidos).
- **Verificação:** `cargo nextest run -p warden-launcher` em Linux e Windows (CI).

### L-03 — `warden-instance`: materialização do pack

- **Prioridade:** P0 · **Depende de:** P1-01, P1-03, P1-04 · **Branch:** `feat/l-03-materializacao`
- **Objetivo:** montar a instância a partir do pack com a semântica do packwiz-installer (ARCHITECTURE §8.2, ADR-0011).
- **Posse:** `crates/warden-instance/src/{materialize.rs,manifest.rs,downloads.rs,blocked.rs,optional_choices.rs}` (e `lib.rs` inicial), `crates/warden-instance/tests/conformance_*.rs`, `xtask/src/installer.rs` (baixa, por versão e hash fixados, o bootstrap do packwiz-installer e um JRE Temurin para testes, numa pasta de cache do usuário).
- **Entregas:** algoritmo completo (lado, opcionais, `preserve`, remoção do que saiu, cache por hash, CurseForge na hora, com os cabeçalhos de download da P1-04 e o redirecionamento resolvido antes de qualquer `Range`), instalação do loader pelo motor sempre com a versão exata do `pack.toml` (D7, S-R5-3 §13), manifesto de estado, lista de bloqueados para T20, pausa antes de sobrescrever arquivo alterado e não revisado. Gancho 1.1: o `cache/downloads/index.sqlite` guarda, por jar, os quatro hashes e a origem (fonte, projeto, versão, URL) (ADR-0039).
- **Critérios de aceite:**
  1. Conformidade: para 5 packs de teste (com opcionais, `preserve`, lado servidor, configs, resource packs), a árvore do Warden é idêntica à do packwiz-installer real.
  2. CA-T13-04 (parte de sincronização) e CA-T22-01 (cancelamento da sincronização).
  3. Segunda materialização sem mudanças não baixa nem escreve nada (contagem de operações).
- **Verificação:** `WARDEN_REQUIRE_EXTERNALS=1 cargo nextest run -p warden-instance`.

### L-04 — Testar e console

- **Prioridade:** P0 · **Depende de:** L-02, L-03, P1-08 · **Branch:** `feat/l-04-testar-e-console`
- **Objetivo:** T13 completa (exceto "Testar como o jogador recebe", L-07). Os passos de verificação (1 e 4) e a captura ficam em pontos de extensão criados pela L-04 como passagem direta: no backend, o trait `TestHooks` (`before_prepare`, `before_launch`, `after_exit`) em `apps/desktop/src-tauri/src/test_hooks/mod.rs`, implementado por `test_hooks/diagnostics.rs` (D-03) e `test_hooks/capture.rs` (C-03); na interface, `apps/desktop/src/features/test/verification/` (D-03). Até serem preenchidos, esses passos não aparecem na interface.
- **Posse:** `apps/desktop/src-tauri/src/commands/test.rs`, `apps/desktop/src-tauri/src/test_session.rs`, `apps/desktop/src-tauri/src/test_hooks/mod.rs` (inicial), `apps/desktop/src/features/test/**` (exceto `verification/`, de D-03, `blocked/`, de L-06, `clean/`, de L-07, e as subpastas da D4: `profiles/` (L-08), `perf-strip/` (L-10), `perf-report/` (L-12), `server/` (L-09), `console-grouped/` (D-09), `reload-scripts/` (C-07)), `apps/desktop/src/routes/packs/$packId/teste.tsx`, `apps/desktop/src/i18n/pt-BR/teste.ts`, `apps/desktop/e2e/test.e2e.ts`.
- **Entregas:** orquestração das etapas, memória automática, um jogo por vez, confirmação ao fechar o app, console virtualizado com filtros e busca, sessões anteriores; botão Testar no cabeçalho com os estados "Testando… ver progresso" e "● Jogo aberto: ver teste"; menu ▾ em grupos (Ver último teste; Outros testes; Perfil do teste com Ajustes do teste neste computador — diálogo da P1-08 —; Instância de teste: Abrir pasta, Apagar mundos de teste, Recriar instância), com o registro `features/test/menu-items.ts` para os itens das tarefas da D4 (L-07, L-08, L-09, L-12, D-12); tela do teste com o resultado e os espaços para "O que mudou durante o teste" (C-03) e "Por que travou" (D-03); `test_start` aceita o modo (normal, como o jogador recebe, perfil de desempenho, como servidor) e o perfil; cada sessão registra o hash da árvore do pack testada (usado pelo aviso "versão não testada" da V-03), o modo e o perfil; o canal do teste já transporta `PerfSample` (vazio até a L-10). Gancho 1.1: a sessão grava a assinatura do perfil do teste (memória, Java, argumentos, janela, "ao abrir o jogo"), a impressão do computador e a marca de primeira abertura (ADR-0039). D7: o perfil padrão do teste fixa o coletor G1 (`-XX:+UseG1GC`) quando os argumentos do usuário não escolhem outro, porque no Java 25 com Parallel ou Serial a memória do jogo só é atualizada nas coletas e a faixa mostraria um valor velho (S-R5-2 §5.3); o motivo fica num comentário e no relatório.
- **Critérios de aceite:** CA-T13-01 (pelo menos 1.20.1 Fabric e 1.12.2 Forge nesta tarefa; matriz completa em L-05), CA-T13-02 a CA-T13-08; golden test da linha de comando com `-XX:+UseG1GC` no perfil padrão e sem ele quando os argumentos do usuário já escolhem um coletor.
- **Verificação:** E2E com jogo simulado (processo Java de teste); roteiro manual no Windows com pack real.

### L-05 — Matriz de versões: golden e jogo real

- **Prioridade:** P0 · **Depende de:** L-02, L-03 · **Branch:** `test/l-05-matriz-de-versoes`
- **Objetivo:** provar o suporte garantido de 1.7.10 até a mais nova.
- **Posse:** `crates/warden-launcher/tests/smoke_*.rs`, `crates/warden-launcher/tests/matrix/**`, `.github/workflows/smoke-game.yml` (criado por esta tarefa).
- **Matriz (D7: as 20 combinações que o S-R5-3 abriu até o mundo no Windows, §3 e §15):** Forge 1.7.10-10.13.4.1614, 1.12.2-14.23.5.2860, 1.16.5-36.2.34, 1.18.2-40.3.0, 1.19.2-43.5.0, 1.20.1-47.4.10, 1.21.1-52.1.0 e 26.2-65.1.0; NeoForge 1.20.1-47.1.106, 21.1.252 e 26.2.0.88; Fabric (loader 0.19.5) em 1.16.5, 1.18.2, 1.19.2, 1.20.1, 1.21.1, 26.2 e 26.3; Quilt 1.20.1 (loader 0.24.0), só como fumaça do motor (Quilt continua fora da v1, SPEC §9); vanilla 26.3. "Versão mais nova" = 26.3 (vanilla e Fabric); Forge e NeoForge ficam em 26.2 até haver versão estável para 26.3. A tarefa confere se surgiu versão mais nova e registra a troca no relatório.
- **Entregas:** packs mínimos por combinação (com um mod simples e Mixin quando aplicável), sempre com a versão exata do loader (sem ela o portablemc consulta a rede a cada abertura); teste que materializa, lança em Xvfb com Mesa, espera os marcadores de "pronto" da S-R5-3 §3.1 e entra no mundo por Quick Play (1.20+) ou pelo servidor local com `--server/--port` (antes de 1.20), com `onboardAccessibility:false` no `options.txt`, `showLoadWarnings = false` no `config/forge-client.toml` (Forge) e `-Dfabric.noGui=true` (Fabric e Quilt), e encerra; marcadores documentados por faixa, com os golden logs do S-R5-3 (`spikes/s-r5-3/fixtures/`) reaproveitados como fixtures em `crates/warden-launcher/tests/matrix/**`; registro de tempos (referência no Windows com GPU real: pronto em 11–37 s, mundo em 12–48 s; a CI Linux mede os seus).
- **O que a CI Linux confere (S-R5-3 §10, nada disso foi verificado em Linux):** os marcadores de pronto e de mundo com Mesa/llvmpipe; Quick Play com `onboardAccessibility:false` em 1.20.1, 1.21.1 e 26.x e o arquivo do `--quickPlayPath`; `--server/--port` com servidor local em 1.7.10, 1.12.2, 1.16.5, 1.18.2 e 1.19.2; `-Dfml.queryResult=confirm` no Forge 1.12.2 e `-Dfabric.noGui=true` sem servidor gráfico; encerramento por `SIGTERM`/`SIGKILL` no lugar do Job Object; os cenários de falha da §5 pelos golden logs.
- **Critérios de aceite:** CA-T13-01 para toda a matriz em Linux; teste de Log4Shell por faixa (ARCHITECTURE §7.4); relatório de execução manual no Windows para 1.7.10 Forge, 1.20.1 Forge e 26.2 NeoForge, **sem nenhum clique na janela do jogo** (o `options.txt` final de cada execução vai no relatório como prova; S-R5-3 §9).
- **Verificação:** workflow `smoke-game.yml` executado (manual) com sucesso; log anexado.

### L-06 — Downloads manuais da CurseForge

- **Prioridade:** P0 (seleção de arquivo) / P1 (observar Downloads) · **Depende de:** L-04, P1-10 · **Branch:** `feat/l-06-downloads-manuais`
- **Objetivo:** T20.
- **Posse:** `crates/warden-instance/src/manual_downloads.rs`, `apps/desktop/src-tauri/src/commands/cf_blocked.rs`, `apps/desktop/src/features/test/blocked/**`, `apps/desktop/src/i18n/pt-BR/downloads-manuais.ts`.
- **Entregas:** lista com estados, abrir página, selecionar arquivo com verificação de hash, (P1) observar a pasta Downloads com `notify`, retomada automática do teste.
- **Critérios de aceite:** CA-T20-01, CA-T20-02.
- **Verificação:** testes de componente; integração com arquivo de hash conhecido.

### L-07 — Testar como o jogador recebe (pelo link do pack)

- **Prioridade:** P1 · **Depende de:** L-04, E-01, V-03, P1-18 · **Branch:** `feat/l-07-teste-pelo-link`
- **Objetivo:** "Testar como o jogador recebe" instalando do zero com o packwiz-installer-bootstrap real e o link do pack (T13, ARCHITECTURE §8.5; D4: R5B §4.3).
- **Posse:** `crates/warden-instance/src/{clean_instance.rs,player_install.rs,installer_cache.rs}`, `apps/desktop/src/features/test/clean/**`, `xtask/src/installer.rs` (acrescenta o `packwiz-installer.jar` fixado; a L-03 já integrada).
- **Entregas:** cache do bootstrap e do installer com versão e SHA-256 fixados (download pela URL do Release); origem = link do pack publicado, ou o pack exportado agora servido só em `127.0.0.1` quando não há publicação; execução com `--bootstrap-no-update` e `-s client`; conferência do hash do `pack.toml` recebido com a última publicação e o aviso do cache do GitHub; mods da CurseForge bloqueados como resultado do teste; ferramentas do jogador incluídas (Crash Assistant); instância temporária apagada no fim, logs mantidos; item no menu ▾.
- **Critérios de aceite:** CA-T13-14; a instância temporária não contém mundos nem configs da instância de trabalho; é apagada ao fim; logs da sessão ficam.
- **Verificação:** integração com o bootstrap real e um servidor local que imita o `raw.githubusercontent.com`; E2E com jogo simulado.


### L-08 — Perfis do teste e Quick Play

- **Prioridade:** P1 · **Depende de:** L-04 · **Branch:** `feat/l-08-perfis-do-teste`
- **Objetivo:** perfis com nome no diálogo Ajustes do teste, troca rápida no menu ▾ e "entrar direto no mundo de teste" (T11; ARCHITECTURE §7.8; R5B §4.1).
- **Posse:** `apps/desktop/src-tauri/src/test_profiles.rs`, `apps/desktop/src-tauri/src/commands/profiles.rs`, `crates/warden-launcher/src/quick_play.rs`, `apps/desktop/src/features/test/profiles/**`, `apps/desktop/src/features/pack-editor/test-settings/**` (a P1-08 já integrada), `apps/desktop/src/i18n/pt-BR/perfis.ts`.
- **Entregas:** perfis em `packs.json` (Padrão fixo, novo, renomear, apagar); campos memória, Java, argumentos, opcionais, janela e "Ao abrir o jogo"; grupo "Perfil do teste" no menu ▾ (itens de rádio); botão "Testar · <perfil>"; `QuickPlay::Singleplayer` (1.20+) e `Multiplayer` (com `--server`/`--port` antes de 1.20) no adaptador do motor; D7: com "Ao abrir o jogo: entrar no mundo" na 1.21+, a instância recebe `onboardAccessibility:false` no `options.txt` antes da linha de base da captura (a tela de boas-vindas de acessibilidade segura o Quick Play; S-R5-3 §4.1); aviso de memória acima de 75% da RAM; aviso e remoção de `-XX:+PerfDisableSharedMem`/`-XX:-UsePerfData` (`W_PERF_FLAGS`).
- **Critérios de aceite:** CA-T11-04, CA-T11-05.
- **Verificação:** golden tests da linha de comando com Quick Play por faixa; testes de componente; roteiro manual com o jogo real.

### L-09 — Servidor local e Testar como servidor

- **Prioridade:** P1 · **Depende de:** L-04, L-08, P1-05 · **Branch:** `feat/l-09-servidor-local`
- **Objetivo:** servidor local sob demanda, com a EULA uma vez, e o fluxo "Testar como servidor" (T27, T21; ARCHITECTURE §7.6; ADR-0032; decisões D18 e D19).
- **Posse:** `crates/warden-server/**`, `apps/desktop/src-tauri/src/server_session.rs`, `apps/desktop/src-tauri/src/commands/server.rs`, `apps/desktop/src/features/test/server/**`, `apps/desktop/src/features/settings/eula/**`, `apps/desktop/src/i18n/pt-BR/servidor.ts`, `apps/desktop/e2e/server.e2e.ts`.
- **Entregas:** instaladores de servidor por loader e faixa (vanilla, Fabric meta, `--installServer` do Forge e do NeoForge, Forge antigo com Java 8), cache dos instaladores; materialização só com os itens de servidor; `server.properties` controlado (`127.0.0.1`, porta livre, `online-mode=false`, RCON e query desligados); EULA em `settings.json` com aceite, data e revogar; supervisão com stdin para comandos e "Done" por expressão regular; parada com `stop` e encerramento forçado após 60 s; conferência de RAM livre; tela do teste com a etapa "Preparar o servidor", console com "Mostrando: Servidor · Jogo" e linha de comando, "Parar jogo e servidor", resultado por lado; cliente entrando pelo Quick Play da L-08; item "Testar como servidor…" no menu ▾; evento `server-state`.
- **Critérios de aceite:** CA-T21-05, CA-T27-01 a CA-T27-05.
- **Verificação:** integração sob demanda com servidores reais (Fabric 1.21.1, NeoForge 1.21.1, Forge 1.20.1 e 1.12.2) em Linux; roteiro manual no Windows (sem aviso do firewall); E2E com servidor simulado (processo Java de teste que imprime "Done").

### L-10 — Desempenho em todo teste: RAM do processo e tempo de carregamento

- **Prioridade:** P0 · **Depende de:** L-04 · **Branch:** `feat/l-10-desempenho-base`
- **Objetivo:** faixa de desempenho com RAM do processo e "Abriu em", em todo teste (T13; ARCHITECTURE §7.7; ADR-0038).
- **Posse:** `crates/warden-perf/{Cargo.toml,src/lib.rs,src/process/**,src/load_time.rs}`, `apps/desktop/src-tauri/src/test_hooks/perf.rs`, `apps/desktop/src/features/test/perf-strip/**`, `apps/desktop/src/i18n/pt-BR/desempenho.ts`.
- **Entregas:** crate `warden-perf` com a leitura da RAM do processo (Windows `GetProcessMemoryInfo` e pico do Job Object, com `unsafe` isolado e `SAFETY`; Linux `/proc`), amostras `PerfSample` a cada 1 s pelo canal do teste, tempos até carregar e até entrar no mundo pelos marcadores da §7.4, gravados na sessão e comparados com a última sessão de outra versão do pack; faixa acima do console e "Abriu em …" no resultado. Gancho 1.1: cada sessão acrescenta uma linha de métricas em `perf/<pack-id>.jsonl` nos dados locais (tempos, picos, perfil, computador, árvore do pack, modo, primeira abertura), que a poda das sessões não apaga (ADR-0039, ADR-0047).
- **Critérios de aceite:** CA-T13-09.
- **Verificação:** `cargo nextest run -p warden-perf` (Linux e Windows na CI); E2E com jogo simulado.

### L-11 — Memória da JVM pelo `hsperfdata` e achados de memória

- **Prioridade:** P1 · **Depende de:** L-10, S-R5-2 · **Branch:** `feat/l-11-memoria-da-jvm`
- **Objetivo:** "Memória do jogo" e "Coletas de memória" na faixa, sem JDK e sem mexer no jogo, e os achados de memória (T13; ARCHITECTURE §7.7).
- **Posse:** `crates/warden-perf/src/{hsperf/**,findings.rs}`, `apps/desktop/src/features/test/perf-strip/heap/**`.
- **Entregas** (D7: os 9 itens de "Implicações para a tarefa L-11" do `docs/spikes/S-R5-2-memoria-da-jvm.md`; ARCHITECTURE §7.7):
  1. **Leitor:** porte de `spikes/s-r5-2/src/perfdata.rs` para `crates/warden-perf/src/hsperf/` (parser puro, sem `unsafe`, buffer ruim vira erro) e de `src/win.rs` para a parte Windows (`OpenFileMappingW(FILE_MAP_READ)` + `MapViewOfFile` + `VirtualQuery`, com `SAFETY`), com a leitura do arquivo `<pid>` (compartilhamento `READ|WRITE|DELETE`) como alternativa e o Linux por `/tmp/hsperfdata_*/<pid>`; cada amostra copia a vista inteira (64 KB, < 1 ms).
  2. **Localização:** procurar `<TEMP do jogo>\hsperfdata_*\<pid>` (o `TMP`/`TEMP` que o Warden passa ao processo) e montar o nome `hsperfdata_<sufixo da pasta>_<pid>`, nunca a partir do nome do usuário; com várias pastas, da mais nova para a mais velha, aceitando a primeira cujo mapeamento abra.
  3. **Tentativas e vida útil:** tentar logo depois de abrir o processo (fica legível em ~50–70 ms) e a cada amostra; só aceitar buffer com `accessible = 1`; reler a lista de contadores quando `num_entries` ou `mod_time_stamp` mudar; **fechar o mapeamento assim que o processo sair** (senão um jogo novo com o mesmo pid fica sem leitura).
  4. **Valores:** heap usada = soma de `sun.gc.generation.*.space.*.used`; máximo = `-Xmx` do perfil, com a regra de reserva (`Copy:MSC`/`ParScav:MSC` somam as gerações, os outros usam a maior); coletas e tempo = soma de `sun.gc.collector.*.invocations` e `.time` ÷ `sun.os.hrt.frequency`, aceitando índices faltando; com ZGC e Shenandoah a faixa diz "Pausas de coleta", não "Coletas".
  5. **Heap depois da coleta** para o `W_LOW_HEAP`: a heap usada da primeira amostra depois de cada aumento de `invocations` (confiável em todos os coletores). No Java 25 com Parallel ou Serial escolhidos pelo usuário, a "Memória do jogo" ao vivo mostra o valor da última coleta com a legenda "na última coleta" (o perfil padrão usa G1, L-04).
  6. **Sem leitura:** amostra sem heap e motivo visível: com `-XX:+PerfDisableSharedMem` ou `-XX:-UsePerfData` nos argumentos, diz qual e oferece removê-lo em Ajustes do teste; sem essas flags, "O Java deste teste não publica a memória do jogo." (OpenJ9, nome de usuário ou pasta temporária com letras fora da code page do Windows).
  7. (Opcional, P2 dentro da tarefa) Usuário fora da code page ANSI: detectar com `WideCharToMultiByte` e só então passar ao jogo `TMP`/`TEMP` em caminho curto 8.3 e `USERNAME` em ASCII, com `-Duser.name=<nome real>`; acento comum de português funciona sem isso.
  8. **Testes:** fixtures de `spikes/s-r5-2/fixtures/` (8 `hsperfdata` + `jstat`) copiadas para `crates/warden-perf/tests/fixtures/hsperf/` com o teste `fixtures_match_jstat`; testes de buffer truncado ou corrompido e de localização com pastas `hsperfdata_João` e `hsperfdata_Lukasz` numa pasta temporária.
  9. **Privacidade:** do `hsperfdata` só saem números; o buffer cru (PATH, caminhos, `sun.rt.javaCommand` com os argumentos do jogo) nunca é gravado nem enviado, nem à IA; fixtures novas passam pela anonimização do spike.
- Também: mini-gráfico dos últimos minutos; `W_LOW_HEAP` e `I_HEAP_OVERSIZED`.
- **Critérios de aceite:** CA-T13-10 (medido com o jogo parado), CA-T13-11; o mapeamento é fechado em até 1 amostra depois de o processo sair (teste).
- **Verificação:** testes com arquivos `hsperfdata` reais gravados (Java 8, 17, 21, 25); teste de integração na CI Windows: baixa os JREs 8/17/21/25 e um JDK (só para o `jstat`; a F0-02 deixa o JDK no job Windows), roda um programa que aloca e **para**, e compara com o `jstat` nesse momento estável; em movimento, compara com a leitura logo depois do `jstat`.

### L-12 — Testar com perfil de desempenho (tempo por mod e spark)

- **Prioridade:** P1 · **Depende de:** L-10, L-09 · **Branch:** `feat/l-12-perfil-de-desempenho`
- **Objetivo:** o teste com perfil de desempenho e a parte "Desempenho" do resultado (T13; ARCHITECTURE §7.7; R5A §7.2, §7.4).
- **Posse:** `crates/warden-perf/src/{mod_timing.rs,spark/**}`, `crates/warden-perf/proto/**` (com a licença registrada em `THIRD_PARTY.md`), `apps/desktop/src-tauri/src/commands/perf.rs`, `apps/desktop/src/features/test/perf-report/**`, `apps/desktop/src/i18n/pt-BR/perfil-desempenho.ts`.
- **Entregas:** item "Testar com perfil de desempenho" no menu ▾; tempo por mod (Forge 1.12.2 `Bar Step`; Forge 1.7.10 `Sending event`/`Sent event`; Forge/NeoForge modernos com `TRACE` só neste teste; NeoForge "deferred task"; Fabric com a mensagem de que não há dado); leitura de `.sparkprofile`/`.sparkheap` com `prost` e agrupamento por jar (índice da D-05); spark ligado e desligado pelo stdin do servidor local; "Abrir no visualizador do spark" (arquivo local); a parte "Desempenho" do resultado.
- **Critérios de aceite:** CA-T13-13.
- **Verificação:** golden tests com `debug.log` e `.sparkprofile` reais; roteiro manual com o spark no jogo real.

---

## Fase 3 — Configs e captura

### C-01 — `warden-configs`

- **Prioridade:** P0 · **Depende de:** F0-01 · **Branch:** `feat/c-01-formatos-de-config`
- **Objetivo:** ler, editar e comparar configs preservando o arquivo (ARCHITECTURE §10, ADR-0013).
- **Posse:** `crates/warden-configs/**`.
- **Entregas:** TOML, JSON/JSONC, JSON5, `.properties`, `.cfg` legado, `options.txt`; árvore com metadados (comentários, `#Range`, `#Allowed Values`); edições mínimas; comparação semântica; corpus real.
- **Critérios de aceite:** CA-T12-01 e CA-T12-02 no nível da crate; corpus com pelo menos 15 arquivos por formato vindos de mods reais (origem anotada); entradas malformadas nunca causam pânico.
- **Verificação:** `cargo nextest run -p warden-configs`; cobertura ≥ 85%.

### C-02 — Editor de configs (texto)

- **Prioridade:** P0 · **Depende de:** C-01, P1-08 · **Branch:** `feat/c-02-editor-de-configs`
- **Objetivo:** T12, parte P0.
- **Posse:** `crates/warden-project/src/configs.rs`, `apps/desktop/src-tauri/src/commands/configs.rs`, `apps/desktop/src/features/configs/**`, `apps/desktop/src/routes/packs/$packId/configs.tsx`, `apps/desktop/src/i18n/pt-BR/configs.ts`, `apps/desktop/e2e/configs.e2e.ts`.
- **Entregas:** árvore com origem Pack/Instância, CodeMirror com realce, salvar com diferenças, concorrência otimista, guarda de alterações não salvas, avisos contextuais (serverconfig, regravação pelo Forge, `options.txt`).
- **Critérios de aceite:** CA-T12-01, CA-T12-03, CA-T12-05.
- **Verificação:** testes de componente; E2E; teste de falha (encerramento durante gravação).

### C-03 — O que mudou durante o teste

- **Prioridade:** P0 · **Depende de:** L-04, C-01 · **Branch:** `feat/c-03-captura-do-teste`
- **Objetivo:** T15, exibida no resultado do teste pelo espaço criado pela L-04 (ARCHITECTURE §8.3, §8.4).
- **Posse:** `crates/warden-instance/src/{baseline.rs,capture.rs,three_way.rs}`, `crates/warden-instance/data/capture-ignore.txt`, `apps/desktop/src-tauri/src/commands/capture.rs`, `apps/desktop/src-tauri/src/test_hooks/capture.rs`, `apps/desktop/src/features/capture/**`, `apps/desktop/src/i18n/pt-BR/captura.ts`, `apps/desktop/e2e/capture.e2e.ts`.
- **Entregas:** linha de base antes do jogo, varredura depois, comparação semântica, classificação, diff por chave de `options.txt`, identificação de jars por hash, conflito de três vias, aplicar/descartar/decidir depois, aviso antes do próximo teste. D4: `capture-ignore.txt` com os arquivos do spark, o token do KubeJS e as pastas do ProbeJS (ARCHITECTURE §8.4, decisão D21); scripts `.js`/`.zs` alterados aparecem como configs.
- **Critérios de aceite:** CA-T15-01 a CA-T15-06, CA-T12-04.
- **Verificação:** testes com instâncias sintéticas (antes/depois) + um roteiro com o jogo real (mudar distância de renderização e uma tecla).

### C-04 — Formulário de configs em camadas, `preserve` e classificação

- **Prioridade:** P1 · **Depende de:** C-02, P1-06 · **Branch:** `feat/c-04-formulario-de-configs`
- **Objetivo:** T12, parte P1 do formulário (D4: metadados em camadas da R5B §2.2; ARCHITECTURE §10.1).
- **Posse:** `apps/desktop/src/features/configs/form/**` (exceto `defaults/`, da C-06), `crates/warden-configs/src/meta/**`, `crates/warden-jarmeta/src/lang.rs`, `crates/warden-project/src/preserve.rs`, `apps/desktop/src-tauri/src/commands/configs_form.rs`, `apps/desktop/src/i18n/pt-BR/configs-formulario.ts`.
- **Entregas:** formulário por tipo de valor (liga/desliga, número com limite, opções, texto, lista, cor), alternância texto/formulário, metadados nas camadas 1 a 4 (arquivo, tipo pelo valor, frases do comentário marcadas "deduzido", rótulo do arquivo de idioma do jar com a chave real ao lado), queda para texto com o motivo (leitura que não volta idêntica, estruturas profundas como "trecho em texto", arquivos acima de 2 MB ou fora de UTF-8, que a `warden-configs` já recusa; ADR-0054), "Não substituir se o jogador já tiver" (`preserve` no índice), botão "Copiar para defaultconfigs/", aviso do NeoForge sobre chaves apagadas e valores corrigidos.
- **Critérios de aceite:** CA-T12-02 pela interface; CA-T12-08; `preserve` mantido após `packwiz refresh` e respeitado pelo packwiz-installer real.
- **Verificação:** testes de componente; testes de domínio com o corpus de configs e jars de teste; integração com packwiz/packwiz-installer.


### C-05 — Buscar em todas as configs

- **Prioridade:** P0 · **Depende de:** C-02 · **Branch:** `feat/c-05-busca-em-configs`
- **Objetivo:** o campo "Buscar em todas as configs" de T12 (D4; R5B §2.4; ARCHITECTURE §10.1).
- **Posse:** `crates/warden-configs/src/index/**`, `apps/desktop/src-tauri/src/commands/configs_search.rs`, `apps/desktop/src/features/configs/search/**`, `apps/desktop/src/i18n/pt-BR/configs-busca.ts`.
- **Entregas:** índice em memória por origem (pack e instância) com uma entrada por chave e por linha nos arquivos sem parser; busca sem acento e sem diferenciar maiúsculas com `nucleo-matcher` e pontuação chave > rótulo > comentário > valor; atualização incremental pelo observador de mudanças; resultados agrupados por arquivo que abrem o editor na linha da chave; filtro "Só deste mod" (o filtro "Só o que mudou do padrão" entra com a C-06).
- **Critérios de aceite:** CA-T12-06, CA-T12-07.
- **Verificação:** `cargo nextest run -p warden-configs` (corpus de 500 configs, com tempo medido); testes de componente; E2E.

### C-06 — Padrões, restaurar padrão e validação de faixa

- **Prioridade:** P1 · **Depende de:** C-04, D-01, D-02 · **Branch:** `feat/c-06-padroes-de-config`
- **Objetivo:** "padrão: X", "Restaurar padrão", o filtro "Só o que mudou do padrão" e os avisos de faixa e de config resetada (T12, T14; R5B §2.3 e §2.5, camada A).
- **Posse:** `crates/warden-configs/src/defaults.rs`, `crates/warden-diagnostics/src/pretest/config_range.rs`, os padrões novos em `crates/warden-diagnostics/data/log-patterns.toml` (acréscimo), `apps/desktop/src/features/configs/form/defaults/**`, `apps/desktop/src/i18n/pt-BR/configs-padrao.ts`.
- **Entregas:** padrões da camada A (`Default:` do NeoForge, `[default: …]` do Forge antigo) com a origem; `config_restore_default` por chave e para o arquivo todo, sempre com diferença; marca de alterado e filtro "Só o que mudou do padrão" no formulário e na busca; `W_CONFIG_RANGE` na passagem rápida com **Restaurar padrão**; padrão de log `Incorrect key … was corrected` como achado "config resetada pelo jogo".
- **Critérios de aceite:** CA-T12-09, CA-T14-10.
- **Verificação:** testes de domínio com o corpus de configs reais e logs reais do NeoForge; testes de componente.

### C-07 — Editor de scripts KubeJS e CraftTweaker

- **Prioridade:** P1 · **Depende de:** C-02, P1-06, L-04 · **Branch:** `feat/c-07-editor-de-scripts`
- **Objetivo:** T26 (ADR-0037; ARCHITECTURE §10.2; R5B §3).
- **Posse:** `crates/warden-scripts/**`, `crates/warden-jarmeta/src/assets.rs`, `apps/desktop/src-tauri/src/commands/scripts.rs`, `apps/desktop/src/features/configs/scripts/**`, `apps/desktop/src/features/test/reload-scripts/**`, `apps/desktop/src/i18n/pt-BR/scripts.ts`, `apps/desktop/e2e/scripts.e2e.ts`.
- **Entregas:** detecção da versão do KubeJS e do CraftTweaker; realce JavaScript e ZenScript (`StreamLanguage` com gramática própria, atribuição em `THIRD_PARTY.md`); trechos prontos por versão em dados; autocompletar de IDs a partir dos jars (`warden-jarmeta::assets`, com cache por hash); painel "Erros dos scripts" dos logs da instância de teste; com KubeJS 7 e o jogo aberto, cliente do servidor web local (token lido só no Rust, recarga e erros ao vivo); recarga pelo stdin do servidor local quando ele estiver aberto (com a L-09 integrada; antes disso, só o comando para copiar); comando certo para copiar nos demais casos; "Salvar e recarregar" com o jogo aberto; "Abrir no VS Code"; botão "Recarregar scripts" na tela do teste.
- **Critérios de aceite:** CA-T26-01 a CA-T26-04.
- **Verificação:** testes de domínio com jars e logs reais; servidor simulado do KubeJS 7; varredura do token nos registros; testes de componente; E2E com jogo simulado.

---

## Fase 4 — Diagnóstico

### D-01 — Regras pré-teste

- **Prioridade:** P0 · **Depende de:** P1-01, P1-03, P1-04, P1-05, P1-06 · **Branch:** `feat/d-01-regras-pre-teste`
- **Objetivo:** motor de regras e todas as regras da ARCHITECTURE §9.2, nas passagens rápida e completa.
- **Posse:** `crates/warden-diagnostics/src/pretest/**`, `crates/warden-diagnostics/data/{exclusive-categories.toml,known-conflicts.toml,obsolete.toml}`, `crates/warden-diagnostics/tests/packs/**`.
- **Entregas:** regras sobre o modelo `Finding` (criado pela D-02), dados curados validados, correções sugeridas. D7, a partir da P1-06 integrada:
  - os mods de um jar vêm de `JarMetadata::effective_mods_for_loader(loader)` (o descritor do loader do pack; o `fabric.mod.json` só vale em Forge/NeoForge para jar sem descritor do loader, isto é, mod Fabric carregado pelo Sinytra Connector, que o pack com Connector precisa ler);
  - faixas Maven (Forge, NeoForge) com `VersionRange::matches_with(versão, MavenFlavor::for_forge(versão do loader))` ou `MavenFlavor::for_neoforge(…)`, porque a comparação muda com a versão do loader; a faixa Maven `"1.0"` quer dizer **qualquer versão** (versão recomendada, não mínima; a R3 §4.3 diz "≥ 1.0", o que está errado);
  - FlexVer **não é transitivo** (o próprio FlexVer avisa): nada de ordenar listas grandes com ele como se fosse ordem total; comparar só pares;
  - a normalização da versão do Minecraft no Fabric (equivalente ao `McVersionLookup` do Fabric Loader: snapshots, `1.21.1-rc1`, versões 26.x) fica nesta tarefa ou no catálogo (P1-05), não na `warden-jarmeta`; a tarefa decide e registra;
  - dependência **sem `type` é obrigatória** no `neoforge.mods.toml`; no `mods.toml` do Forge vale `mandatory` (S-R5-3 §14 item 8);
  - **mod duplicado, mod de outro loader e mod para outra versão do Minecraft** são responsabilidade desta análise: os loaders modernos os aceitam em silêncio ou falham antes do menu, e a busca do culpado não os trata (S-R5-3 §14 item 11; `E_DUPLICATE_ID`, `E_DUPLICATE_FILE`, `E_LOADER`, `E_MCVERSION`).
- **Critérios de aceite:** CA-T14-01 com os packs quebrados de teste; um pack NeoForge com dois jars do mesmo mod, um mod só Fabric sem Connector e um mod de outra versão do Minecraft gera os três erros antes de abrir o jogo (D7); propriedade "todo achado tem evidência"; diagnóstico de 300 mods com cache quente < 3 s (medido).
- **Verificação:** `cargo nextest run -p warden-diagnostics`; cobertura ≥ 85%.

### D-02 — Análise pós-crash e redação

- **Prioridade:** P0 · **Depende de:** F0-05 · **Branch:** `feat/d-02-analise-de-crash`
- **Objetivo:** catálogo de padrões de log e redação de dados pessoais (ARCHITECTURE §9.3, §9.4).
- **Posse:** `crates/warden-diagnostics/src/{lib.rs,model.rs,postcrash/**,redact.rs}`, `crates/warden-diagnostics/data/log-patterns.toml`, `crates/warden-diagnostics/tests/corpus/**`, seção correspondente em `THIRD_PARTY.md`.
- **Entregas:** modelo `Finding` com evidência obrigatória (ARCHITECTURE §9.1), usado também pela D-01; coleta de artefatos de uma pasta de sessão, limpeza de `§`/ANSI, padrões (os 35 de R2 §6.3 + porte do codex-minecraft com atribuição), ordenação por probabilidade, redação. Gancho 1.1: a análise aceita qualquer texto de log, não só uma pasta de sessão (a IA já precisa disso para um log do computador), e a redação aceita regras extras (ADR-0039).
- **Critérios de aceite:** CA-T14-02 com corpus de pelo menos 25 logs reais cobrindo Forge 1.7.10/1.12.2/moderno, NeoForge e Fabric; CA-T14-04.
- **Verificação:** `cargo nextest run -p warden-diagnostics`.

### D-03 — Seção Problemas e integração com o Testar

- **Prioridade:** P0 · **Depende de:** D-01, D-02, L-04 · **Branch:** `feat/d-03-tela-diagnostico`
- **Objetivo:** T14 sem IA: seção Problemas, "Por que travou" no resultado do teste e os diálogos de erro do Testar.
- **Posse:** `apps/desktop/src-tauri/src/commands/diagnostics.rs`, `apps/desktop/src/features/diagnostics/**`, `apps/desktop/src/routes/packs/$packId/problemas.tsx`, `apps/desktop/src/i18n/pt-BR/diagnostico.ts`, `apps/desktop/e2e/diagnostics.e2e.ts`, `apps/desktop/src/features/test/verification/**`, `apps/desktop/src-tauri/src/test_hooks/diagnostics.rs`.
- **Entregas:** passagens rápida e completa ligadas ao Testar pelo `TestHooks`, seção Problemas com lista por gravidade, contador no menu, "Verificar agora" e "Último travamento" (substituído pela lista de Travamentos quando a D-06 for integrada), evidências, correções pelos fluxos normais, relatório pós-crash como "Por que travou" no resultado do teste, marcas de problema na lista de Mods, "Copiar relatório" (redigido); (P1) "Ignorar este aviso neste pack" (`diagnostics_ignore`, gravado em `.warden/project.toml`). D4: o registro `features/diagnostics/problems-blocks.ts` para os blocos da seção (Saúde do pack, da D-08; Travamentos, da D-06).
- **Critérios de aceite:** CA-T13-06; correções "Adicionar dependência" e "Mudar lado" funcionam de ponta a ponta.
- **Verificação:** E2E com pack quebrado e jogo simulado que trava.

### D-04 — ✦ Diagnóstico com IA: laço de ferramentas e conversas (Gemini)

- **Prioridade:** P1 · **Depende de:** S-R5-4, D-03, D-06, P1-13, V-01 · **Branch:** `feat/d-04-ia-conversas`
- **Objetivo:** a IA "médico" com ferramentas de leitura e conversas por pack, com consentimento uma vez por conversa e cada envio visível (T14; ARCHITECTURE §9.5; ADR-0030; decisão D17). As propostas aplicáveis e as consultas externas ficam na D-14.
- **Posse:** `crates/warden-ai/**` (exceto `src/proposals/**` e `src/external/**`, da D-14), `apps/desktop/src-tauri/src/commands/ai.rs`, `apps/desktop/src-tauri/src/ai_tools/` (registro `mod.rs` e as ferramentas de leitura sobre dados que já existem), `apps/desktop/src/features/diagnostics/ai/**` (exceto `proposals/`, da D-14), `apps/desktop/src/routes/packs/$packId/ia.tsx`, `apps/desktop/src/routes/packs/$packId/ia.$conversaId.tsx`, `apps/desktop/src/features/settings/ai/**`, `apps/desktop/src/i18n/pt-BR/ia.ts`, `apps/desktop/e2e/ai.e2e.ts`.
- **Entregas:** laço `generateContent` sem estado a partir do relatório do S-R5-4 (*thought signatures* intactas, `VALIDATED`, até 8 rodadas). D7: os 12 itens da §16 do `docs/spikes/S-R5-4-laco-do-gemini.md` (ADR-0055; ARCHITECTURE §9.5):
  1. modelo padrão = o Flash-Lite estável mais novo pelo `models.list` (hoje `gemini-3.5-flash-lite`) e a opção "Análise mais cuidadosa" com o Flash estável mais novo (hoje `gemini-3.8-flash`), com o aviso de poucas conversas por dia no plano gratuito; custo esperado por conversa no consentimento (US$ 0,005 a 0,04 no Flash-Lite; nenhum nome de modelo fixo no código);
  2. resposta final pela ferramenta `responder` (esquema em `parametersJsonSchema`, sempre em `allowedFunctionNames`), com `mode: NONE` + `responseFormat` como reserva quando vier texto e ao estourar 8 rodadas;
  3. declarações fixas por conversa (as ~17 de leitura + `responder`), escalonamento só por `allowedFunctionNames` (a D-14 entra no mesmo esquema);
  4. `content` do modelo guardado e reenviado como `serde_json::Value` intacto; todas as `functionResponse` numa mensagem, com o `id` da chamada; temperatura 1.0; tempo-limite de 90 s; o *system prompt* nunca pede texto antes das chamadas (causa `MALFORMED_FUNCTION_CALL`);
  5. novas tentativas: 503, 500 e tempo-limite de rede até 4 vezes com espera exponencial; `MALFORMED_FUNCTION_CALL`, `UNEXPECTED_TOOL_CALL`, `MALFORMED_RESPONSE`, resposta sem `content`, texto vazio e resposta final vazia até 2 vezes, sem mexer no histórico; 429 por minuto → `AI_RATE_LIMITED` com espera visível ("Aguardando o limite por minuto do plano gratuito"); 429 diário → `AI_QUOTA_EXCEEDED` na hora, **sem prometer horário**, sugerindo trocar de modelo; sinal de cobrança (402, `BILLING_DISABLED`) vira erro claro;
  6. plano gratuito tratado como caso comum: contador de requisições da conversa e espera visível;
  7. conferência de evidência como o `evidence.rs` do spike (literal; ids que não vieram de uma ferramenta desta conversa recusados); **linha de chat do log não prova sozinha**: selo próprio "baseado em mensagem de jogador"; o selo de evidência diz **de onde veio o trecho, nunca que a conclusão está certa**, e a interface deixa isso escrito; o medidor de confiança não repete a confiança declarada pelo modelo;
  8. três camadas contra injeção (regra no *system prompt*, aviso nos resultados vindos de arquivos do jogo, conferência);
  9. servidor simulado (`wiremock` com `Respond` próprio, regras do `mock.rs`) e fixtures de `spikes/s-r5-4/fixtures/` copiadas para `crates/warden-ai/tests/fixtures/` com a origem;
  10. `countTokens` com `generateContentRequest` (conta sistema e ferramentas) no consentimento;
  11. teste de rede com o modelo padrão, aceitando 503 e 429 por minuto como normais;
  12. (opcional, com autorização do orquestrador) mais um dia de lotes nos Flash, de manhã, antes de fechar a escolha; se o Flash acertar bem mais, "Analisar com mais cuidado" vira botão na conversa.

  Além disso: ferramentas de leitura sobre o que já existe (visão geral, mods, achados, sessões, crash report, busca e leitura de log, busca e leitura de configs com a permissão, histórico e diferença entre versões, dependentes e "por que está no pack"); conferência de evidências e de nomes de mods; consentimento por conversa com as permissões (configs, GitHub) e o texto inicial exato com `countTokens`; blocos "Enviado à IA" com os bytes exatos; seção com Nova conversa e Conversas (continuar, apagar, aviso de pack mudado); página da conversa com mensagens, selos de evidência, "não verificado", medidor de confiança e campo para continuar; escolha de modelo em Configurações e `secrets_test` do Gemini; botão **✦ Pedir ajuda à IA** no resultado do travamento e **✦ Conversar com a IA** em Travamentos; ícone de brilhinho só em elementos de IA.
- **Critérios de aceite:** CA-T14-03, CA-T14-05, CA-T14-06, CA-T14-11, CA-T14-14, CA-T14-15, CA-T14-18, CA-T14-19; teste de rede com chave real (sob demanda) completa uma conversa com pelo menos uma chamada de ferramenta e resposta final pela ferramenta `responder` válida no esquema.
- **Verificação:** `cargo nextest run -p warden-ai` (servidor simulado do Gemini que exige as assinaturas); E2E com servidor simulado; teste de rede sob demanda.


### D-05 — Índices pacote → mod e config de mixin → mod

- **Prioridade:** P0 · **Depende de:** P1-06, D-01 · **Branch:** `feat/d-05-indices-de-jars`
- **Objetivo:** os índices usados pelo console agrupado, pela falha de Mixin, pela busca do culpado e pelo spark (ARCHITECTURE §9.6; R5A §7.1, §10.1 item 1).
- **Posse:** `crates/warden-jarmeta/src/index/**`, `crates/warden-diagnostics/src/pretest/index_pass.rs`.
- **Entregas:** índice pacote → jar → modid descendo nos jars aninhados (pacote ambíguo vai para o jar de topo); índice config de mixin → mod (Fabric, `MixinConfigs` do Forge, `[[mixins]]` do NeoForge); cache em `cache/jarindex/` por hash; montagem na passagem completa do diagnóstico; o padrão de falha de Mixin do catálogo passa a apontar o mod dono da config.
- **Critérios de aceite:** com os jars do corpus da P1-06 e os 5 jars Fabric da R5A §5.3, o índice tem no máximo os 2 pacotes ambíguos conhecidos (ambos da Fabric API) e mapeia cada config de mixin ao mod certo (dourado); a segunda passagem com o mesmo pack usa só o cache (contagem de leituras de jar = 0); um log com `Mixin apply failed … epicfight.mixins.json` gera o achado apontando o Epic Fight.
- **Verificação:** `cargo nextest run -p warden-jarmeta -p warden-diagnostics`.

### D-06 — Assinatura de travamento e Travamentos em Problemas

- **Prioridade:** P0 · **Depende de:** D-03, D-05 · **Branch:** `feat/d-06-travamentos`
- **Objetivo:** assinatura gravada em cada sessão e a lista de Travamentos agrupada pela causa (T14; ARCHITECTURE §9.6; R5A §7.5).
- **Posse:** `crates/warden-diagnostics/src/{signature.rs,crash_history.rs}`, `apps/desktop/src-tauri/src/test_hooks/crash_history.rs`, `apps/desktop/src-tauri/src/commands/crashes.rs`, `apps/desktop/src/features/diagnostics/crashes/**`, `apps/desktop/src/i18n/pt-BR/travamentos.ts`.
- **Entregas:** (Gancho 1.1: o registro de travamento tem o campo `origin`, por enquanto sempre "teste"; ADR-0039) assinatura normalizada (regra, exceção, primeiro frame de mod, mods citados) com hash estável, gravada no `session.json` pelo gancho `after_exit`; `crashes_list`/`crash_get` com contagem, datas, versões do pack, situação e ligações (busca do culpado e conversas, quando existirem); bloco "Travamentos" na seção Problemas (registro da D-03), substituindo "Último travamento"; retenção das sessões (30 que fecharam normalmente; as que travaram até 500 MB por pack, com aviso).
- **Critérios de aceite:** CA-T14-08; a retenção apaga as sessões certas num teste com 40 sessões sintéticas e avisa antes de passar de 500 MB.
- **Verificação:** `cargo nextest run -p warden-diagnostics` com o corpus de crashes; testes de componente; E2E com jogo simulado que trava duas vezes com números diferentes.

### D-07 — Consultas do grafo de dependências

- **Prioridade:** P0 · **Depende de:** D-01, P1-08 · **Branch:** `feat/d-07-grafo-consultas`
- **Objetivo:** dependentes transitivos, "Por que está no pack" e bibliotecas sem uso (T06, T07; ARCHITECTURE §9.6; R5A §6.1).
- **Posse:** `crates/warden-diagnostics/src/graph/**`, `apps/desktop/src-tauri/src/commands/graph.rs`, `apps/desktop/src/features/pack-editor/details/dependencies/**`, `apps/desktop/src/i18n/pt-BR/grafo.ts`.
- **Entregas:** grafo a partir do modelo do diagnóstico (arestas tipadas, `provides`, jar-in-jar dentro do nó, ciclos como um nó, arestas inferidas guardadas por pack); `graph_dependents`, `graph_why_in_pack`, `graph_orphans`; bloco "Depende de / Usado por / Por que está no pack" nos detalhes do item (registro `details/blocks.ts`).
- **Critérios de aceite:** CA-T06-06 (consultas), CA-T07-03.
- **Verificação:** `cargo nextest run -p warden-diagnostics` (packs de teste com cadeias, `provides` e jar-in-jar); testes de componente.

### D-08 — Nota de saúde do pack

- **Prioridade:** P1 · **Depende de:** D-06, D-10, P1-12, L-10 · **Branch:** `feat/d-08-saude-do-pack`
- **Objetivo:** a nota em Problemas e na coluna Saúde de Meus packs (T02, T14; ADR-0034; decisão D24).
- **Posse:** `crates/warden-diagnostics/src/health.rs`, `crates/warden-diagnostics/data/health-score.toml`, `apps/desktop/src-tauri/src/commands/health.rs`, `apps/desktop/src/features/diagnostics/health/**`, a coluna Saúde em `apps/desktop/src/features/packs/PackTable.tsx` (a P1-07 já integrada), `apps/desktop/src/i18n/pt-BR/saude.ts`.
- **Entregas:** função pura com pesos e tetos em dados versionados (tabela da SPEC T14); "O que tirou pontos" com links; itens ignorados riscados e sem desconto; última nota guardada em `packs.json`; bloco no topo de Problemas e coluna em Meus packs ("sem dados" quando não há cálculo).
- **Critérios de aceite:** CA-T02-05, CA-T14-07.
- **Verificação:** testes de domínio e de propriedade (`proptest`); testes de componente.

### D-09 — Console agrupado por mod

- **Prioridade:** P1 · **Depende de:** D-05, L-04 · **Branch:** `feat/d-09-console-agrupado`
- **Objetivo:** "Mostrar: Linha a linha · Agrupado por mod · Só problemas" no console (T13; ARCHITECTURE §9.8; R5A §7.1).
- **Posse:** `crates/warden-diagnostics/src/console/**`, `crates/warden-diagnostics/data/log-noise.toml`, `apps/desktop/src/features/test/console-grouped/**`, `apps/desktop/src/i18n/pt-BR/console.ts`.
- **Entregas:** atribuição de cada linha a um mod (frame com jar ou módulo, handler do mixin no Fabric, "Mixins in Stacktrace", logger, índice de pacotes); normalização e agrupamento de repetições; stack trace recolhido com a primeira linha de mod; ruído conhecido em dados; grupos enviados em lotes à interface; seletor no console.
- **Critérios de aceite:** CA-T13-12; 50.000 linhas agrupadas sem travar a interface (medido).
- **Verificação:** `cargo nextest run -p warden-diagnostics` com logs reais de Forge, NeoForge e Fabric; testes de componente; teste de desempenho.

### D-10 — Raio-x de mixins (camadas A e B)

- **Prioridade:** P1 · **Depende de:** S-R5-1, D-05, D-03, P1-08 · **Branch:** `feat/d-10-raio-x-de-mixins`
- **Objetivo:** "O que este mod altera no jogo" nos detalhes do item e o aviso `W_MIXIN_OVERLAP` em Problemas, só avisos (T07, T14; ADR-0034; ARCHITECTURE §9.6).
- **Posse:** `crates/warden-mixin/**`, `crates/warden-diagnostics/src/pretest/mixin_overlap.rs`, `crates/warden-diagnostics/data/mixin-known-compatible.toml`, `apps/desktop/src-tauri/src/commands/xray.rs`, `apps/desktop/src/features/pack-editor/xray/**`, `apps/desktop/src/i18n/pt-BR/raio-x.ts`.
- **Entregas:** D7: **implementação própria com `cafebabe`** a partir do protótipo do S-R5-1 (`spikes/s-r5-1/harness/src/proto.rs`, branch do spike; ADR-0056); o intermed não entra como dependência, nem portado, nem no `THIRD_PARTY.md`. Itens da §9 do `docs/spikes/S-R5-1-intermed.md`:
  - leitura de configs, refmaps e anotações só do loader do pack (o `fabric.mod.json` vale em Forge/NeoForge apenas para jar sem descritor do loader, isto é, mod Fabric via Sinytra Connector); config declarada e ausente no jar é ignorada sem erro;
  - **jars aninhados por versão maior:** cópias aninhadas repetidas ficam com a de versão maior; no empate, a do jar que traz mais aninhados (o agregador, como a Fabric API);
  - ponto de aplicação com o seletor de `@ModifyVariable`/`@ModifyArg` (`ordinal`, `index`, `name`, `argsOnly` e tipo do parâmetro; `@ModifyArgs` = todos);
  - **rebaixadores novos**, além dos da R5A §5.5: opções desligadas por `lithium:options`/`canary:options`/`radium:options` nos descritores e pelas configs do pack (`config/<mod>.properties`; a mixin some do cruzamento); "o plugin ou a config do mod cita o outro mod" (textos das classes `*Config*`/`*Plugin*`/`*Compat*`); "declara dependência" não vale quando o outro é biblioteca (Fabric API, Architectury, Forgified Fabric API…); `@Redirect` + `@Inject` no mesmo ponto passa a baixo;
  - **dados curados iniciais:** `mixin-known-compatible.toml` com Lithium/Radium/Canary × Redirected, ModernFix × Redirected, ModernFix × Noisium e STFU × Charm; um arquivo de "opções que ligam mixins" (`crates/warden-mixin/data/mixin-options.toml`; FerriteCore `useSmallThreadingDetector` → `threaddetec` e afins), com o mesmo teste de esquema dos outros dados;
  - **mods via Sinytra Connector** (Fabric em pack Forge/NeoForge) marcados no raio-x como "não comparados com os mods nativos" (nomes *intermediary* × SRG/Mojang) até existir a camada D;
  - leitura paralela por jar e **cache num arquivo binário único** em `cache/xray/` (o JSON por jar do protótipo deu 160–360 ms);
  - ordem prevista por prioridade; elevadores da R5A;
  - corpus: `spikes/s-r5-1/fixtures/` (configs e descritores reais, `resultado-<pack>.json`, `veredictos.json` com os 31 casos altos julgados; `ORIGEM.json` vira o `FIXTURES.md` da QUALITY §4); os jars reais baixados pela lista `jars-<pack>.json` (URL e sha1) num teste `rede_*`; os testes comuns usam jars sintéticos e as configs reais. Casos a reproduzir: sodium-extra × sodiumextras e badoptimizations × citadel = alto; Krypton × Lithium = some; Lithium × ModernFix e Iris × Sodium = baixo.
  - **Pendente do dono (não aplicar sem a decisão; SPEC §10):** a proposta do S-R5-1 de levar a Problemas só o risco alto (`W_MIXIN_OVERLAP`), deixando os médios no raio-x e na IA. Até lá vale "alto ou médio" (ARCHITECTURE §9.6); a tarefa deixa a gravidade mínima do aviso num dado, para a troca ser de uma linha.

  Também: bloco nos detalhes (registro `details/blocks.ts`) com "Ver todas as alterações" no mesmo painel; aviso em Problemas que nunca bloqueia o teste; elevação pelo travamento que citou a config.
- **Critérios de aceite:** CA-T07-04, CA-T07-05, CA-T14-09; raio-x de 300 mods em menos de 3 s sem cache e 300 ms com cache (medido, também com disco frio); os casos do corpus do S-R5-1 dão o resultado esperado.
- **Verificação:** `cargo nextest run -p warden-mixin` com o corpus de jars reais (origem anotada) e jars sintéticos; testes de componente.

### D-11 — Grafo desenhado em Mods

- **Prioridade:** P1 · **Depende de:** D-07, P1-08 · **Branch:** `feat/d-11-grafo-desenhado`
- **Objetivo:** "Ver como: Lista · Grafo" em Mods (T06; ARCHITECTURE §18).
- **Posse:** `crates/warden-diagnostics/src/graph/focus.rs` (a D-07 já integrada), `apps/desktop/src/features/pack-editor/graph/**`, `apps/desktop/src/i18n/pt-BR/grafo-desenho.ts`.
- **Entregas:** `graph_focus` no backend (nós e ligações até 2 níveis); grafo focado em HTML + SVG com o mod no centro, nós como botões, ligações com legenda e lista em texto, embutidas dentro do nó; explicação em texto ao lado; "Ver no grafo" nos detalhes; "Ver o pack inteiro" com Cytoscape.js e `dagre`, com aviso acima de 150 nós.
- **Critérios de aceite:** CA-T06-07.
- **Verificação:** testes de componente com `axe` e teclado; teste visual no protótipo de referência.

### D-12 — Busca do culpado no cliente (com modo assistido)

- **Prioridade:** P1 · **Depende de:** S-R5-3, D-06, D-07, L-03, L-04, L-08 · **Branch:** `feat/d-12-busca-do-culpado`
- **Objetivo:** T25 no cliente, com Quick Play nas versões 1.20+ e o modo assistido (ADR-0031; ARCHITECTURE §9.7; decisões D15 e D23).
- **Posse:** `crates/warden-bisect/**`, `crates/warden-instance/src/subset.rs`, `apps/desktop/src-tauri/src/bisect_session.rs`, `apps/desktop/src-tauri/src/commands/bisect.rs`, `apps/desktop/src/features/bisect/**`, `apps/desktop/src/i18n/pt-BR/busca-do-culpado.ts`, `apps/desktop/e2e/bisect.e2e.ts`.
- **Entregas:** algoritmo (ordem topológica, rodadas 0 e 1, busca binária no prefixo, minimização para pares, confirmação, repetições para problemas intermitentes, contradições) testado com um executor simulado; materialização de subconjuntos com links físicos e mundos descartáveis; veredito por assinatura, "travou diferente" como inconclusiva e dependência inferida. D7: itens da §14 do `docs/spikes/S-R5-3-marcadores-da-busca.md` (ARCHITECTURE §9.7):
  1. **catálogo de marcadores por faixa** em `warden-bisect`, testado com os golden logs de `spikes/s-r5-3/fixtures/`: "pronto" pela §3.1 (1.7.10 com o atlas definitivo, não o `16x16`; 1.8–1.12.2 com `successfully loaded` + `textures-atlas` + som; 1.13+ com `blocks.png-atlas` + som, casando só o sufixo); "entrou no mundo" por `logged in with entity id` no cliente (1.20+, Quick Play) ou no servidor local (antes de 1.20), `joined the game` como confirmação e o JSON do `--quickPlayPath` como sinal secundário; parser que aceita XML do log4j e texto puro na mesma execução;
  2. **sinais de falha com prioridade sobre os de menu:** no NeoForge e no Forge modernos a tela de erro de carregamento dispara os marcadores de "pronto"; `Error during pre-loading phase`, `Missing or unsupported mandatory dependencies` e `Cowardly refusing to send event ... broken mod state` marcam a rodada como falha mesmo com som e atlas vistos;
  3. lista de sinais de falha da §5.2 no veredito, com a âncora e a assinatura: `Couldn't place player in world` (falha ao entrar no mundo **sem crash e sem o processo fechar**), `Mixin apply ... failed` (assinatura = `.mixins.json` + alvo), `OutOfMemoryError` em qualquer thread, crash report novo, `Incompatible mods found!`, `MissingModsException`, `DuplicateModsFoundException`, e Java errado (`UnsupportedClassVersionError`, `Unsupported major.minor version`, `ClassCastException ... URLClassLoader`) como problema de ambiente, não de mod;
  4. assinatura = âncora + primeira exceção depois dela, com números trocados por `#` (a primeira exceção do log pega ruído, como o 401 do perfil offline; lista de ruído da §5.2);
  5. **encerrar ao ver o sinal:** várias falhas não fecham o jogo; o supervisor encerra o Job Object na hora e não gasta a estabilidade numa rodada que já falhou;
  6. **jogo vivo e calado** (travamento e falta de memória no NeoForge deixam o processo aberto sem log): sem linha nova depois de `Starting integrated minecraft server` (ou `Connected to a modded server`) e sem `logged in` em N s = "travou", e o Warden encerra o processo; N = 3× o intervalo entre `Starting integrated...` e `logged in` da rodada de controle, mínimo 60 s;
  7. **preparo da instância de cada rodada**, além dos mods: `options.txt` com `onboardAccessibility:false` (obrigatório na 1.21+; sem isso o Quick Play para na tela de boas-vindas de acessibilidade); Forge 1.20.1 com `config/forge-client.toml` contendo `showLoadWarnings = false` (a tela de avisos bloqueia o Quick Play); `-Dfabric.noGui=true` em todo Fabric e Quilt; `-Dfml.queryResult=confirm` no Forge 1.12.2 (e nas faixas com `StartupQuery`, 1.8–1.12.2, a confirmar); a pasta do mundo existe antes de abrir; mundo de teste gerado sem mods (servidor vanilla da mesma versão, semente fixa) e copiado a cada rodada;
  8. **grafo de dependências:** no `neoforge.mods.toml`, dependência **sem `type` é obrigatória** (no `mods.toml` do Forge vale `mandatory`); pack com Sinytra Connector precisa ler também o `fabric.mod.json` e os ids dos jars embutidos (`fabric-api` e `fabric_api` são o mesmo mod); dependência inferida do log ligada desde o início (`Mod X requires Y` no NeoForge/Forge e `requires ... of mod '...'` no Fabric viram aresta, a ordem é recalculada e a busca recomeça sem repetir as rodadas 0 e 1); faixa que aceita a versão errada cai na inferência pelo `NoClassDefFoundError`;
  9. **estimativa nova** no diálogo (§8 do spike): rodada ≈ W + 20 s quando passa e ≈ W quando falha, com W = tempo até "entrou no mundo" **medido na rodada 0** (o tamanho do prefixo quase não muda o tempo); 1 culpado = ⌈log₂ n⌉ + 4 rodadas; par com o parceiro na posição p = ⌈log₂ n⌉ + ⌈log₂ p⌉ + 5; problema intermitente multiplica pelas repetições;
  10. a rodada 0 com repetição cobre a queda nativa intermitente ("travou diferente" é inconclusiva e repete);
  11. mod duplicado, de outro loader ou de outra versão não são casos da busca: ficam com a análise antes de abrir (D-01);
  12. ferramentas do jogador (spark, Crash Assistant, Not Enough Crashes) desligadas durante a busca, salvo suspeitas;
  13. verificação manual sem ninguém mexendo na janela do jogo, com o `options.txt` final conferido (S-R5-3 §9).

  Também: estado em `state/bisect.json` com pausar, continuar e cancelar com resultado parcial; diálogo de configuração com estimativa; modo da tela do teste com a trilha de rodadas, "O que o Warden está fazendo agora", suspeitos e tabela; modo assistido; resultado com as ações; `game-state = bisecting`; itens de entrada (resultado do travamento, Travamentos, menu ▾).
- **Critérios de aceite:** CA-T25-01 a CA-T25-05, CA-T25-07, CA-T25-08.
- **Verificação:** `cargo nextest run -p warden-bisect` (executor simulado, `proptest` sobre grafos aleatórios: nenhum prefixo sem dependências; golden logs do S-R5-3, inclusive `neo-dep-faltando.log` e `forge-dep-faltando.log`); integração com jogo simulado; roteiro manual com um pack real quebrado no Windows (referência do S-R5-3: Create: Complete 2.0.0, NeoForge 1.21.1, 154 mods, par achado em 20 rodadas e 26 min 55 s).

### D-13 — Busca do culpado com o servidor local

- **Prioridade:** P1 · **Depende de:** D-12, L-09 · **Branch:** `feat/d-13-busca-com-servidor`
- **Objetivo:** a busca em versões anteriores a 1.20 e em problemas de servidor, abrindo o servidor local só depois de explicar e pedir (T25; decisão D18).
- **Posse:** `apps/desktop/src-tauri/src/bisect_server.rs`, `apps/desktop/src/features/bisect/server/**`.
- **Entregas:** explicação e pedido no diálogo de configuração (com memória necessária e EULA); cada rodada sobe o servidor com uma cópia descartável do mundo e o cliente entra por `--server`/`--port` ou Quick Play; servidor parado ao fim de cada rodada; veredito com os logs dos dois lados.
- **Critérios de aceite:** CA-T25-06.
- **Verificação:** integração com servidor real (Forge 1.16.5) sob demanda; E2E com processos simulados.

### D-14 — IA: propostas aplicáveis e consultas externas

- **Prioridade:** P1 · **Depende de:** D-04, D-10, D-12, P1-12, C-06 · **Branch:** `feat/d-14-ia-propostas`
- **Objetivo:** cartões de proposta com Aplicar, busca de issues no GitHub, changelogs entre versões e as ferramentas do raio-x e da busca do culpado (T14; ADR-0030).
- **Posse:** `crates/warden-ai/src/{proposals/**,external/**}`, `apps/desktop/src-tauri/src/ai_tools/{proposals.rs,external.rs,xray.rs,bisect.rs}`, `apps/desktop/src/features/diagnostics/ai/proposals/**`, `apps/desktop/src/i18n/pt-BR/ia-propostas.ts`.
- **Entregas:** ferramentas `propose_*` com evidência obrigatória e `ai_proposal_apply`/`ai_proposal_discard` pelos fluxos normais (dependências, diferença antes de salvar, ponto de segurança); `search_mod_issues` (repositório pelo `source_url`/`issues_url`, consulta só com repositório e termos redigidos, token do GitHub quando existir, cache de 1 h, respeito aos limites) e `get_mod_changelog`; `get_mixin_report` e `get_bisect_result`; blocos "Enviado ao GitHub"; liberação por etapa com `allowedFunctionNames`.
- **Critérios de aceite:** CA-T14-12, CA-T14-13.
- **Verificação:** `cargo nextest run -p warden-ai` com servidores simulados do Gemini e do GitHub; E2E.

---

## Fase 5 — Versionamento e exportação

### V-01 — `warden-versioning`: base git

- **Prioridade:** P0 · **Depende de:** P1-01, F0-05 · **Branch:** `feat/v-01-versionamento-base`
- **Objetivo:** operações git escondidas (ARCHITECTURE §11, ADR-0015).
- **Posse:** `crates/warden-versioning/**`.
- **Entregas:** iniciar repositório com ponto inicial, status (alterações não salvas), commit de versão com tag anotada, pontos de segurança em `refs/warden/safety/*`, restauração transacional, diff entre versões e árvore de trabalho, changelog estruturado a partir dos metafiles, sugestão SemVer (função pura), escrita do `CHANGELOG.md`.
- **Critérios de aceite:** CA-T17-01, CA-T17-02 no nível da crate; sugestão SemVer coberta por tabela de casos (SPEC T16); nomes e acentos em arquivos funcionam (teste com `config/ação.toml`).
- **Verificação:** `cargo nextest run -p warden-versioning`.

### V-02 — Salvar versão e histórico

- **Prioridade:** P0 · **Depende de:** V-01, P1-08 · **Branch:** `feat/v-02-salvar-versao-e-historico`
- **Objetivo:** T16 e T17.
- **Posse:** `apps/desktop/src-tauri/src/commands/versioning.rs`, `apps/desktop/src/features/versioning/**`, `apps/desktop/src/routes/packs/$packId/historico.tsx`, `apps/desktop/src/i18n/pt-BR/versoes.ts`, `apps/desktop/e2e/versioning.e2e.ts`.
- **Entregas:** diálogo Salvar versão com sugestão, changelog e "Marcar como versão final"; botão "Salvar versão · N alterações" no cabeçalho; seção Histórico com alterações não salvas, versões com estado (só salva, versão final, publicada; marcar e desmarcar versão final) e o espaço da área "Publicação para os jogadores" (preenchido pela V-03); voltar para versão; (P1) pontos de segurança e descartar por arquivo; (P1) checklist.
- **Critérios de aceite:** CA-T16-01 a CA-T16-05, CA-T17-01 a CA-T17-03.
- **Verificação:** E2E; testes de componente.

### V-03 — Publicar versão no GitHub para os jogadores

- **Prioridade:** P0 · **Depende de:** V-02, P1-13, E-01, D-01, L-04, P1-04 · **Branch:** `feat/v-03-publicar-versao`
- **Objetivo:** T18 (ADR-0028): publicar versões finais no GitHub para os jogadores atualizarem pelo link do `pack.toml`.
- **Posse:** `crates/warden-versioning/src/{github/**,publish/**}`, `apps/desktop/src-tauri/src/commands/{github.rs,publish.rs}`, `apps/desktop/src/features/github/**`, `apps/desktop/src/i18n/pt-BR/publicar.ts`, `apps/desktop/e2e/publish.e2e.ts`.
- **Entregas:** (Gancho 1.1: as conferências do `publish_plan` são uma lista de checagens plugáveis, para a W-03 acrescentar segurança e manutenção; ADR-0039) `publish_plan` (árvore da versão via exportação da E-01, higiene, varredura de segredos, avisos de mods da CurseForge bloqueados com troca pelo Modrinth, de mods **só da CurseForge** (D7, ADR-0051: "N mods só existem na CurseForge e podem deixar de baixar para os jogadores…", com **Trocar pelo Modrinth** pelo mesmo arquivo quando o hash existir lá, ou pela versão compatível do mesmo projeto achada pela regra de deduplicação da busca combinada, mostrada antes de trocar; aviso, nunca bloqueio), de versão não testada e de problemas, notas da versão desde a última publicada); `publish_run` (linha de publicação `refs/warden/publish/main`, `CHANGELOG.md` de publicação, `.gitattributes` com `* -text`, tag, push com credencial em memória, GitHub Release, estado "Release pendente"); `github_setup` (criar público por padrão, privado como "só backup", ou vincular existente, com a explicação de visibilidade); área "Publicação para os jogadores" no Histórico; resultado com link do pack, passo a passo para jogadores (Prism/MultiMC com o packwiz-installer-bootstrap), um espaço para a "Instância pronta para o Prism" (preenchido pela E-04) e cópia das notas; o `publish_run` devolve o id da Release criada, para a E-04 anexar o arquivo; divergência com substituição protegida; passo a passo do token (verificar e documentar o tipo de token e as permissões mínimas que funcionam para criar repositório, enviar e criar Release); ligação do `secrets_test` para o GitHub.
- **Critérios de aceite:** CA-T18-01 (teste sob demanda com conta descartável), CA-T18-02 a CA-T18-08.
- **Verificação:** `cargo nextest run -p warden-versioning` (servidor git local para push e servidor HTTP local imitando o `raw.githubusercontent.com` e a API de Releases); conformidade com o packwiz-installer-bootstrap real (CA-T18-03); teste de rede sob demanda.

### E-01 — Exportação nativa

- **Prioridade:** P0 · **Depende de:** P1-02, P1-07, D-01, V-01 · **Branch:** `feat/e-01-exportacao`
- **Objetivo:** T19 (ARCHITECTURE §12).
- **Posse:** `crates/warden-export/**`, `apps/desktop/src-tauri/src/commands/export.rs`, `apps/desktop/src/features/export/**`, `apps/desktop/src/routes/packs/$packId/exportar.tsx`, `apps/desktop/src/i18n/pt-BR/exportar.ts`, `apps/desktop/e2e/export.e2e.ts`.
- **Entregas:** pré-verificações, pré-visualização com alertas, "Excluir do pack", saída em pasta e zip determinístico, conformidade; exportação a partir de uma versão salva (árvore lida da tag via V-01) como função P0, usada pela publicação (V-03); na interface, "Exportar esta versão" continua P1.
- **Critérios de aceite:** CA-T19-01 a CA-T19-03.
- **Verificação:** integração com packwiz e packwiz-installer reais; E2E.

### E-02 — Exportar `.mrpack` e zip da CurseForge

- **Prioridade:** P1 (v1, decisão D1) · **Depende de:** E-01, P1-02, P1-04 · **Branch:** `feat/e-02-exportar-outros-formatos`
- **Objetivo:** formatos de outros launchers em T19 (ARCHITECTURE §12).
- **Posse:** `crates/warden-export/src/formats/**`, `crates/warden-export/tests/formats/**`, `apps/desktop/src/features/export/formats/**`, `apps/desktop/src/i18n/pt-BR/exportar-formatos.ts`, `apps/desktop/e2e/export-formats.e2e.ts` (depois de E-01 integrada; não roda em paralelo com E-01).
- **Entregas:** `packwiz modrinth export` / `packwiz curseforge export` via sidecar sobre cópia limpa (staging); validação do arquivo gerado (validador reaproveitado pela E-04; zip abre, `modrinth.index.json`/`manifest.json` válidos, `overrides/` só com arquivos do índice; D4: o validador aceita `env: "unknown"` na leitura, para a P1-19, e reprova o `.mrpack` gerado que o contenha); tela que explica o que se perde em cada formato; troca de fonte CurseForge → Modrinth por hash; bloqueio com explicação para mod da CurseForge sem distribuição por terceiros; confirmação para embutir arquivos de terceiros; exigência de `version` no `pack.toml`.
- **Critérios de aceite:** CA-T19-04, CA-T19-05.
- **Verificação:** integração com o packwiz real usando pack de teste com mods do Modrinth, da CurseForge e local; validação de esquema; importação manual do `.mrpack` no marco.


### E-03 — Pacote para servidor

- **Prioridade:** P1 · **Depende de:** E-01, L-09, V-03 · **Branch:** `feat/e-03-pacote-para-servidor`
- **Objetivo:** o formato "Pacote para servidor (.zip)" em Exportar, nas duas variantes (T19; ADR-0035; ARCHITECTURE §12.1; decisão D22).
- **Posse:** `crates/warden-export/src/server_pack/**`, `crates/warden-export/tests/server_pack/**`, `apps/desktop/src/features/export/server-pack/**`, `apps/desktop/src/i18n/pt-BR/pacote-servidor.ts`, `apps/desktop/e2e/server-pack.e2e.ts`.
- **Entregas:** conteúdo de servidor (filtro por lado, `kubejs/` sem `client_scripts/`, `server-overrides/`, sem `options.txt`, resource packs e shaders), `user_jvm_args.txt`, `start.bat`/`start.sh` próprios (verificar Java, instalar o loader, perguntar a EULA, iniciar); variante pelo link (bootstrap e installer fixados, `-s server`, exige publicação) e variante com mods dentro (jars do cache conferidos, T20 para bloqueados, confirmação de licença); validação opcional no servidor local até "Done"; tela com a escolha da variante, o que vai e o aviso de pacote não testado.
- **Critérios de aceite:** CA-T19-06, CA-T19-07, CA-T19-08.
- **Verificação:** teste de conformidade em Linux com o bootstrap real e um servidor local que imita o GitHub; roteiro manual com `start.bat` no Windows; E2E.

### E-04 — Instância pronta para o Prism

- **Prioridade:** P1 (v1, decisão do dono de 04/10/2026; R7 decisão 1) · **Depende de:** E-02, V-03, P1-04, L-03 · **Branch:** `feat/e-04-instancia-pronta-prism`
- **Objetivo:** o jogador arrasta um arquivo para o Prism Launcher e o Prism baixa sozinho os mods, inclusive os da CurseForge com a chave dele (T18, T19; ADR-0050; ARCHITECTURE §12.3).
- **Posse:** `crates/warden-export/src/prism/**`, `crates/warden-export/tests/prism/**`, `crates/warden-versioning/src/publish/release_assets.rs` (a V-03 já integrada), `apps/desktop/src/features/export/prism/**`, `apps/desktop/src/features/github/prism/**` (a V-03 já integrada), `apps/desktop/src/i18n/pt-BR/instancia-prism.ts`, `apps/desktop/e2e/prism-instance.e2e.ts`.
- **Entregas:** gerador próprio de `.mrpack` (não o `packwiz modrinth export`, que embute os mods da CurseForge como jars) a partir da mesma cópia de staging da E-01, para o estado atual ou para uma versão salva: `modrinth.index.json` com `dependencies` (Minecraft e loader com a versão exata); Modrinth pela URL de `cdn.modrinth.com`; CurseForge pela URL da CDN montada do ID do arquivo e do nome do `.pw.toml` (`https://edge.forgecdn.net/files/<id ÷ 1000>/<id mod 1000>/<arquivo>`), só para arquivos que permitem distribuição por terceiros (conferido na API, só em memória); link direto pela URL do metafile; `sha1`, `sha512` e `fileSize` do cache de downloads (baixando o que faltar, com progresso e cancelamento); `env` pelo lado (nunca `unknown`) e `optional`; `overrides/` com os arquivos do índice que não são referência, pelo filtro da exportação; mod bloqueado para terceiros para a geração com **Trocar pelo Modrinth** quando houver; arquivo local de terceiros só com a confirmação de licença; validação do arquivo pelo validador da E-02 e pelas regras do Prism 11.x lidas do código (ADR-0050); formato "Instância pronta para o Prism (.mrpack)" em Exportar, com o texto do que o jogador vai ver; no resultado do Publicar versão, **Baixar instância pronta para o Prism**, o arquivo anexado à GitHub Release (`POST …/releases/{id}/assets`, com a mesma credencial em memória da V-03) e o passo a passo ("arraste para a janela do Prism"; o Prism vai pedir para confirmar os mods de fora do Modrinth; a memória recomendada; para atualizar, arraste o arquivo da versão nova ou use o link do pack). A tarefa confere nos termos da API da CurseForge que gravar a URL da CDN no arquivo do jogador é permitido; se não for, para e devolve a decisão ao orquestrador (alternativa: zip da CurseForge, ADR-0050).
- **Critérios de aceite:** CA-T19-09, CA-T19-10, CA-T18-09.
- **Verificação:** `cargo nextest run -p warden-export -p warden-versioning` (pack de teste com mods do Modrinth, da CurseForge, de link e local; servidor simulado da API de Releases); teste `rede_*` que baixa por HTTPS, sem `Range`, 3 arquivos reais da CurseForge pela URL montada e confere o `sha1`; testes de componente; E2E com servidores simulados; roteiro do marco: arrastar o arquivo para o Prism 11.x no Windows, confirmar os mods de fora do Modrinth e entrar no mundo (§2).

---

## Fase 6 — Acabamento

### A-01 — Revisão de experiência, textos e acessibilidade

- **Prioridade:** P0 · **Depende de:** todas as tarefas com parte P0 integradas (fases 1–5, inclusive P1-16, P1-18, L-10, C-05, D-05, D-06 e D-07 da D4) · **Branch:** `fix/a-01-revisao-ux`
- **Objetivo:** passar por todas as telas com o glossário e a SPEC, corrigir estados faltantes, textos, foco e contraste.
- **Posse:** ajustes nos arquivos de textos das telas P0 (pasta i18n/pt-BR do app) e nos componentes apontados pela revisão, numa lista declarada antes de começar e aprovada pelo orquestrador; nunca arquivos de tarefas P1 em andamento na mesma onda.
- **Critérios de aceite:** nenhuma violação séria do `axe` em nenhuma tela; todos os CA de estados vazios/erro conferidos; revisão de textos registrada.
- **Verificação:** `pnpm -C apps/desktop test`; roteiro manual.

### A-02 — Instalador, ícone, Sobre e avisos de terceiros

- **Prioridade:** P0 · **Depende de:** F0-02 · **Branch:** `build/a-02-instalador`
- **Objetivo:** instalador NSIS do Windows pronto para uso pessoal (D4).
- **Posse:** `apps/desktop/src-tauri/icons/**`, configuração de bundle no `tauri.conf.json`, `xtask/src/notices.rs`, `apps/desktop/src/features/about/licenses/**`.
- **Entregas:** ícone, instalador por usuário (sem admin), geração automática de avisos de terceiros (Rust com `cargo-about`, npm, packwiz, motor do launcher) exibidos em Sobre, crédito e link do Forge.
- **Critérios de aceite:** instalar, abrir, desinstalar sem resíduos fora dos dados do usuário; Sobre lista todas as licenças.
- **Verificação:** roteiro manual no Windows; CI produz o instalador.

### A-03 — Fluxo completo de ponta a ponta e roteiro de aceite do dono

- **Prioridade:** P0 · **Depende de:** todas as demais tarefas P0 (inclusive A-01, A-02 e A-04) · **Branch:** `test/a-03-fluxo-completo`
- **Objetivo:** E2E do fluxo completo (criar → adicionar pela busca combinada → configurar → testar → trazer o que mudou → salvar versão final → publicar versão → instalar com o packwiz-installer-bootstrap pelo link do pack → exportar) e roteiro para o dono.
- **Posse:** `apps/desktop/e2e/full-flow.e2e.ts`, `docs/ROTEIRO-DE-ACEITE.md`.
- **Critérios de aceite:** E2E passa em Linux e Windows (CI noturna); roteiro executado pelo dono com um pack real.
- **Verificação:** CI noturna; relatório do dono.

### A-04 — Desempenho com packs grandes

- **Prioridade:** P0 · **Depende de:** P1-08, D-01, E-01, C-05, D-05 · **Branch:** `perf/a-04-packs-grandes`
- **Objetivo:** cumprir as metas P0 da SPEC §8 com um pack de 300 mods e 500 configs (D4: inclusive índices de pacotes e busca em todas as configs; as metas P1 da D4, como raio-x e console agrupado, são medidas pelas tarefas donas).
- **Posse:** `crates/{warden-packwiz,warden-project,warden-configs,warden-jarmeta,warden-diagnostics,warden-export}/benches/**`, `apps/desktop/e2e/perf/**`, correções pontuais declaradas.
- **Critérios de aceite:** metas da SPEC §8 medidas e registradas; CA-T02-01, CA-T06-05.
- **Verificação:** `cargo bench`; E2E de desempenho.

### A-05 — Mudanças externas no pack

- **Prioridade:** P1 · **Depende de:** P1-08 · **Branch:** `feat/a-05-mudancas-externas`
- **Objetivo:** recarregar automaticamente quando o pack muda por fora (T05, P1; ARCHITECTURE §15).
- **Posse:** `apps/desktop/src-tauri/src/watcher.rs`, `crates/warden-project/src/watch.rs`.
- **Critérios de aceite:** CA-T05-01 em até 2 s sem trocar de seção; escritas do próprio Warden não geram recarga dupla.
- **Verificação:** teste de integração com escrita externa.

### A-06 — Armazenamento em Configurações

- **Prioridade:** P1 · **Depende de:** L-04, P1-13 · **Branch:** `feat/a-06-armazenamento`
- **Objetivo:** seção "Armazenamento" de T21.
- **Posse:** `apps/desktop/src-tauri/src/commands/storage.rs`, `apps/desktop/src/features/settings/storage/**`, `apps/desktop/src/i18n/pt-BR/armazenamento.ts`.
- **Entregas:** espaço usado por cache, arquivos do Minecraft, Javas e instâncias (`storage_usage`); "Limpar cache" (`storage_clear_cache`) sem apagar instâncias nem mundos, recusando enquanto houver jogo ou operação em andamento.
- **Critérios de aceite:** os tamanhos batem com a medição direta no disco (teste); limpar o cache não afeta packs nem instâncias, e o próximo teste volta a baixar só o necessário.
- **Verificação:** testes de integração; teste de componente.


### A-07 — Revisão da ferramenta completa e fluxos avançados de ponta a ponta

- **Prioridade:** P1 · **Depende de:** A-01 e todas as tarefas P1 com interface (P1-14 a P1-19, L-07 a L-09, L-11, L-12, C-04, C-06, C-07, D-04, D-08 a D-14, E-02 a E-04, A-05, A-06) · **Branch:** `test/a-07-ferramenta-completa`
- **Objetivo:** fazer pelas funções da D4 o que a A-01 e a A-03 fazem pelo núcleo: revisão de textos, estados, foco e contraste com o glossário e o design system, e E2E dos fluxos 6 a 10 do protótipo (descobrir e adicionar vários mods; trazer mods de um modpack; travou sem causa: encontrar o culpado e conversar com a IA; testar como servidor e gerar o pacote para servidor; importar um modpack).
- **Posse:** `apps/desktop/e2e/advanced-flows.e2e.ts`, a seção "Funções avançadas" de `docs/ROTEIRO-DE-ACEITE.md` (a A-03 já integrada), ajustes em `apps/desktop/src/i18n/pt-BR/**` e componentes apontados pela revisão (lista declarada antes de começar, aprovada pelo orquestrador).
- **Critérios de aceite:** nenhuma violação séria do `axe` nas telas da D4; menu ▾ do Testar e página de descoberta navegáveis só por teclado; textos conferidos com o glossário (QUALITY §8.2); E2E dos 5 fluxos avançados passa em Linux (CI noturna) e no Windows; roteiro executado pelo dono.
- **Verificação:** `pnpm -C apps/desktop test`; `pnpm -C apps/desktop e2e`; roteiro manual.

---

## Fase 7 — Warden 1.1 "Profissional"

As seis funções que o dono aprovou depois da D4 (SPEC T28 a T33; decisões D27 a D33; ADR-0039 a ADR-0047). Formam a **versão 1.1 do app**: começam **depois do marco M5** (por isso as tarefas sem dependência de outra W dependem de A-03 e A-07, que fecham a v1) e não mudam nenhuma tarefa da v1 além dos ganchos marcados "Gancho 1.1:". Prioridade de todas: **1.1**. O lugar de cada função na interface está na ESTRUTURA §14; a arquitetura, na ARCHITECTURE §21.

### W-01 — `warden-security`: leitura dos jars e lista de sinais

- **Prioridade:** 1.1 · **Depende de:** A-03, A-07, P1-06, D-05 · **Branch:** `feat/w-01-sinais-de-seguranca`
- **Objetivo:** a parte local da checagem de segurança dos mods: abrir os jars sem executar nada e procurar sinais conhecidos e pontos de atenção (T28; ADR-0040, ADR-0041; ARCHITECTURE §21.1).
- **Posse:** `crates/warden-security/**` (exceto `src/verify/**`, da W-02), `xtask/src/check_signatures.rs`, `xtask/src/fixtures_security.rs`, a seção "warden-security" de `THIRD_PARTY.md`.
- **Entregas:** crate nova `warden-security` (`core`, `jarmeta`, `packwiz` para o murmur2) com `zip` e `cafebabe` (com `parse_bytecode(true)` só nas classes candidatas, para ler as instruções); descida nos jars embutidos (`META-INF/jars/`, `META-INF/jarjar/`) com limite de profundidade e de tamanho (proteção contra zip bomb); reconstrução de textos montados com `new String(new byte[]{…})` e de constantes em Base64; `data/signatures.toml` com os sinais conhecidos (padrões de instruções e de constantes do estágio 0 do fractureiser e hashes SHA-256 de arquivos maliciosos publicados), a data da lista e o nível (sinal conhecido × ponto de atenção), no formato inspirado nos modelos da Concoction (MIT, atribuição); pontos de atenção (`URLClassLoader` com endereço da internet, `defineClass` com bytes vindos de rede, `Runtime.exec`/`ProcessBuilder`, webhooks do Discord, sites de colagem, IPs fixos, caminhos de dados de navegador e de contas de launcher, `HKCU\…\Run`, `systemd/user`; D7, R7 §9 item 6: mod que procura arquivos de outro launcher no disco e manda o resultado ao servidor, e mod com "penalidades escondidas" ativadas por condição, como o Gabou's Libs de 08/2026, a confirmar como padrão concreto na tarefa); resultado por jar com evidência (classe, método, trecho); cache em `cache/security/<sha256>.json` com a versão da lista; `cargo xtask fixtures-security` gera os jars sintéticos de teste (nenhum malware real no repositório) e `cargo xtask check-signatures` valida o esquema e confere cada sinal contra os jars sintéticos.
- **Critérios de aceite:** CA-T28-01; CA-T28-04 (parte de domínio: o mesmo padrão gera ponto de atenção só quando o jar é marcado como "não oficial" na entrada); 300 jars reais do corpus da P1-06 e da D-05 sem nenhum sinal conhecido (sem falso positivo de sinal) e o tempo da primeira leitura medido.
- **Verificação:** `cargo nextest run -p warden-security`; `cargo xtask check-signatures`; cobertura ≥ 85%.

### W-02 — Conferência com o arquivo oficial e página Segurança dos mods

- **Prioridade:** 1.1 · **Depende de:** W-01, P1-10, P1-11, P1-19, D-03, L-04 · **Branch:** `feat/w-02-seguranca-dos-mods`
- **Objetivo:** juntar a conferência pelo hash com o Modrinth e a CurseForge à leitura da W-01 e levar o resultado ao app: página, detalhes do mod, Testar, adicionar e importar (T28).
- **Posse:** `crates/warden-security/src/verify/**`, `apps/desktop/src-tauri/src/commands/security.rs`, `apps/desktop/src-tauri/src/test_hooks/security.rs`, `apps/desktop/src/features/security/**`, `apps/desktop/src/routes/packs/$packId/problemas.seguranca.tsx`, `apps/desktop/src/i18n/pt-BR/seguranca.ts`, `apps/desktop/e2e/security.e2e.ts`, os pontos de chamada da checagem em `crates/warden-project/src/add/{local.rs,url.rs}` e `crates/warden-import/src/security.rs` (as tarefas donas já integradas na v1).
- **Entregas:** conferência em lote (Modrinth `POST /v2/version_files` com sha512 e conferência de projeto e versão; CurseForge `POST /v1/fingerprints` seguida da conferência do SHA-1 de `file.hashes` e de `fileStatus`; arquivos do computador e links pelo hash nas duas plataformas; endereço de download dos `.pw.toml`); só hashes saem do computador; regras `E_SEC_SIGNATURE`, `E_SEC_MISMATCH`, `E_SEC_PLATFORM_FLAGGED`, `W_SEC_ATTENTION`, `W_SEC_UNVERIFIED`, `I_SEC_OFF_PLATFORM` no modelo de achados (contam só na categoria "Segurança" da saúde, W-11); `.warden/trust.toml` com "Confiar neste arquivo" por hash (confirmação por digitação); comandos `security_scan`, `security_report_get`, `security_trust`, `security_replace_with_official`; gancho `before_launch` (só jars novos ou alterados) e o diálogo de erros do Testar sem "Testar mesmo assim" para erros de segurança; checagem antes de gravar no adicionar por arquivo e por link e no importar; página "Segurança dos mods" com o aviso "não é um antivírus", a data da lista e o aviso de lista com mais de 180 dias; bloco "Segurança do arquivo" nos detalhes (registro `details/blocks.ts`).
- **Critérios de aceite:** CA-T28-02, CA-T28-03, CA-T28-05, CA-T28-06 (parte do Testar e do adicionar), CA-T28-07, CA-T28-08, CA-T28-09.
- **Verificação:** testes de domínio com respostas reais gravadas das duas APIs; integração com o packwiz real (trocar pelo oficial passa em `check_conformance`); testes de componente (`axe`); E2E com servidores simulados.

### W-03 — Checagens obrigatórias do Publicar versão

- **Prioridade:** 1.1 · **Depende de:** W-02, W-04 · **Branch:** `feat/w-03-checagens-do-publicar`
- **Objetivo:** segurança e manutenção sempre antes de publicar, bloqueando o que impede o jogador de jogar com segurança (T18 parte 1.1).
- **Posse:** `crates/warden-versioning/src/publish/checks/**`, `apps/desktop/src/features/github/checks/**`, `apps/desktop/src/i18n/pt-BR/publicar-checagens.ts`.
- **Entregas:** duas checagens plugadas no `publish_plan` (gancho da V-03): segurança de todos os arquivos da versão (baixa os jars que faltam no cache, com progresso e cancelamento) e manutenção sem cache; bloqueio por erro de segurança não resolvido nem confiado e por "Arquivo removido", com as ações de cada achado; os demais resultados viram avisos; bloco "Checagens obrigatórias" no diálogo, com o botão final desabilitado enquanto rodam.
- **Critérios de aceite:** CA-T29-04; CA-T28-06 (parte do Publicar).
- **Verificação:** testes de domínio do plano; testes de componente; E2E com servidores simulados (git local e APIs).

### W-04 — Manutenção dos mods

- **Prioridade:** 1.1 · **Depende de:** A-03, A-07, P1-12, D-03 · **Branch:** `feat/w-04-manutencao-dos-mods`
- **Objetivo:** situações de manutenção de cada mod e a página "Manutenção dos mods" (T29; ADR-0042).
- **Posse:** `crates/warden-diagnostics/src/maintenance/**`, `crates/warden-diagnostics/data/maintenance.toml`, `apps/desktop/src-tauri/src/commands/maintenance.rs`, `apps/desktop/src/features/maintenance/**` (exceto `replacements/`, da W-05), `apps/desktop/src/routes/packs/$packId/problemas.manutencao.tsx`, `apps/desktop/src/i18n/pt-BR/manutencao.ts`.
- **Entregas:** consulta em lote junto com a verificação de atualizações (Modrinth `GET /projects?ids=` e `POST /version_files`, com cache de 24 h; CurseForge `POST /v1/mods` e `POST /v1/mods/files`, só em memória, refeita a cada execução do Warden), comparação do que foi pedido com o que voltou; situações e gravidades da T29 (limiar de 18 meses em dados); regras `E_MAINT_FILE_REMOVED`, `W_MAINT_PROJECT_REMOVED`, `W_MAINT_ARCHIVED`, `I_MAINT_STALE`, `I_MAINT_NO_LATEST_MC`; filtro "Sem manutenção" e marca na lista de Mods; bloco "Manutenção" nos detalhes; página com os grupos e as ações.
- **Critérios de aceite:** CA-T29-01, CA-T29-02, CA-T29-03.
- **Verificação:** testes de domínio com respostas reais gravadas; servidor simulado contando chamadas; inspeção da pasta de dados (nada da CurseForge gravado); testes de componente.

### W-05 — Substitutos

- **Prioridade:** 1.1 · **Depende de:** W-04, P1-15, P1-16 · **Branch:** `feat/w-05-substitutos`
- **Objetivo:** "Procurar substituto" e "Trocar por este" (T29).
- **Posse:** `crates/warden-discovery/src/similar.rs`, `crates/warden-project/src/replace.rs`, `apps/desktop/src-tauri/src/commands/replace.rs`, `apps/desktop/src/features/maintenance/replacements/**`, `apps/desktop/src/i18n/pt-BR/substitutos.ts`.
- **Entregas:** busca de parecidos na mesma plataforma (Modrinth `/search` com as categorias do mod, versão, loader e `disclosure_types!=archived`; CurseForge com as categorias, versão e loader, só em memória), sem o próprio mod e sem os que já estão no pack; substituto conhecido da lista curada primeiro; motivos reais por candidato; painel de detalhes com "← Detalhes"; troca numa transação só (adição com dependências pelo `add_plan` + remoção), com ponto de segurança.
- **Critérios de aceite:** CA-T29-05, CA-T29-06.
- **Verificação:** servidor simulado conferindo as consultas; integração com o packwiz real; testes de componente.

### W-06 — Log de um jogador: leitura, redação e versão do pack

- **Prioridade:** 1.1 · **Depende de:** A-03, A-07, D-02, D-06, V-01, P1-06 · **Branch:** `feat/w-06-log-do-jogador`
- **Objetivo:** a parte de domínio da T30: links aceitos, extração da lista de mods dos logs e identificação da versão salva (ADR-0043).
- **Posse:** `crates/warden-diagnostics/src/{logsource.rs,player_redact.rs}`, `crates/warden-diagnostics/src/modlist/**`, `crates/warden-diagnostics/tests/corpus-player/**` (logs reais redigidos, com a origem anotada).
- **Entregas:** `logsource` (link → endereço do texto cru, função pura, com a tabela de serviços da T30); extração da lista de mods por formato (crash report do Forge 1.7.10, 1.12.2 e moderno, do NeoForge, `latest.log` do Forge e do Fabric, `Fabric Mods:`, seção `Mods:` do Prism, `modlist.txt` do Crash Assistant, `packwiz.json`); versões mascaradas pelo mclo.gs como curinga; redação extra (nome do jogador, nome da instância) sobre a da D-02; comparação com as versões salvas (pela ordem de confiança da T30, lendo as árvores das versões pela `warden-versioning` e os metadados dos jars pelo cache), com cobertura, diferenças e limiares; resultado puro, sem rede.
- **Critérios de aceite:** CA-T30-01 (normalização dos links), CA-T30-02, CA-T30-03, CA-T30-05; CA-T30-04 (parte de domínio da redação).
- **Verificação:** `cargo nextest run -p warden-diagnostics` com o corpus de logs de jogadores e um pack de teste com 5 versões salvas.

### W-07 — Página Travamento de um jogador, Travamentos e IA

- **Prioridade:** 1.1 · **Depende de:** W-06, D-04, D-06 · **Branch:** `feat/w-07-travamento-de-jogador`
- **Objetivo:** a T30 no app: entrada por link, texto ou arquivos, resultado, Travamentos com a origem e a conversa com a IA.
- **Posse:** `apps/desktop/src-tauri/src/player_reports.rs`, `apps/desktop/src-tauri/src/commands/player_reports.rs`, `apps/desktop/src-tauri/src/ai_tools/player.rs`, `apps/desktop/src/features/diagnostics/player/**`, a coluna Origem em `apps/desktop/src/features/diagnostics/crashes/**` (a D-06 já integrada), `apps/desktop/src/routes/packs/$packId/problemas.jogador.tsx`, `apps/desktop/src/i18n/pt-BR/travamento-jogador.ts`, `apps/desktop/e2e/player-crash.e2e.ts`.
- **Entregas:** download pelo Rust (só HTTPS, só texto, até 10 MB, tempo-limite), escolha de vários arquivos e texto colado; cópia redigida em `player-reports/<pack-id>/<ULID>/` (dados locais); diagnóstico com o catálogo da D-02 e os achados "mod acrescentado pelo jogador" e "parece já corrigido na versão X"; registro em Travamentos com `origin = player`, agrupado pela assinatura; "✦ Conversar com a IA" com o mesmo consentimento (origem "travamento de um jogador" no `ai_conversation_preview`); "Começar pelo: Travamento de um jogador" em ✦ Diagnóstico com IA; apagar análise.
- **Critérios de aceite:** CA-T30-01 (download e mensagens), CA-T30-04, CA-T30-06, CA-T30-07.
- **Verificação:** servidores simulados (mclo.gs e Gemini); varredura da cópia guardada e do corpo enviado à IA; testes de componente; E2E.

### W-08 — Notas e grupos de mods

- **Prioridade:** 1.1 · **Depende de:** A-03, A-07, P1-08, V-02 · **Branch:** `feat/w-08-notas-e-grupos`
- **Objetivo:** T31 inteira (ADR-0044).
- **Posse:** `crates/warden-project/src/annotations.rs`, `crates/warden-versioning/src/changelog_notes.rs`, `apps/desktop/src-tauri/src/commands/annotations.rs`, `apps/desktop/src/features/pack-editor/annotations/**`, `apps/desktop/src/features/versioning/notes-option/**`, `apps/desktop/src/i18n/pt-BR/notas-e-grupos.ts`, `apps/desktop/e2e/annotations.e2e.ts`.
- **Entregas:** `.warden/mods.toml` (esquema versionado; ligação pelo caminho do metafile e pelo projeto; leitura tolerante; escrita pela `PackTransaction`); comandos `annotations_get`, `annotation_set_note`, `groups_create/rename/delete`, `items_set_groups`; remoção da entrada junto com o mod; "Notas e grupos" em Alterações não salvas; nota na linha do mod, busca nas notas, "Agrupar por" e filtro "Grupo" sobre a função de agrupamento genérica da P1-08; "Pôr no grupo ▾"; diálogo "Grupos do pack"; bloco "Nota e grupos" nos detalhes; opção das notas no resumo do Salvar versão e do Publicar; notas na ferramenta `list_mods` da IA e no texto do consentimento.
- **Critérios de aceite:** CA-T31-01 a CA-T31-06.
- **Verificação:** integração com o packwiz real (`.warden/` fora do índice); testes de componente; E2E; teste de desempenho com 500 mods.

### W-09 — Itens repetidos entre mods

- **Prioridade:** 1.1 · **Depende de:** A-03, A-07, D-05, D-08 · **Branch:** `feat/w-09-itens-repetidos`
- **Objetivo:** T32 (ADR-0046), sem a parte da saúde (W-11).
- **Posse:** `crates/warden-jarmeta/src/materials/**`, `crates/warden-diagnostics/src/duplicates/**`, `crates/warden-diagnostics/data/{materials.toml,unifiers.toml}`, `apps/desktop/src-tauri/src/commands/duplicates.rs`, `apps/desktop/src/features/diagnostics/duplicates/**`, `apps/desktop/src/routes/packs/$packId/problemas.repetidos.tsx`, `apps/desktop/src/i18n/pt-BR/itens-repetidos.ts`, corpus em `crates/warden-jarmeta/tests/corpus-materials/**` (origem anotada).
- **Entregas:** leitura das três convenções de tag, de `worldgen/configured_feature`/`placed_feature` e dos `biome_modifier`, de nomes de modelos e do arquivo de idioma, e das heurísticas de 1.7.10/1.12.2, com cache por hash (junto do índice da D-05); análise por material com confiança; soluções por faixa em `unifiers.toml`; leitura da config do AlmostUnified (inclusive `world_gen_unification` no NeoForge 1.21.1); página com o botão de adicionar pelo fluxo normal.
- **Critérios de aceite:** CA-T32-01, CA-T32-02, CA-T32-03, CA-T32-05.
- **Verificação:** `cargo nextest run -p warden-jarmeta -p warden-diagnostics` com jars reais (Mekanism Forge 1.20.1 e NeoForge 1.21.1, Tech Reborn Fabric 1.20.1, Thermal Foundation 1.12.2); testes de componente.

### W-10 — Desempenho entre versões

- **Prioridade:** 1.1 · **Depende de:** A-03, A-07, L-10, L-11, L-12, V-02 · **Branch:** `feat/w-10-desempenho-entre-versoes`
- **Objetivo:** T33 (ADR-0047), sobre as métricas que a v1 já grava (gancho da L-10).
- **Posse:** `crates/warden-perf/src/history/**`, `apps/desktop/src-tauri/src/commands/perf_history.rs`, `apps/desktop/src/features/versioning/perf-history/**`, `apps/desktop/src/features/test/perf-compare/**`, `apps/desktop/src/routes/packs/$packId/historico.desempenho.tsx`, `apps/desktop/src/i18n/pt-BR/desempenho-versoes.ts`.
- **Entregas:** leitura de `perf/<pack-id>.jsonl`; filtro de testes comparáveis com o motivo de cada exclusão; atribuição à versão salva pela árvore; mediana por versão; critério de "mais pesada" (função pura com tabela de casos); tempo por tick a partir dos resumos do spark da L-12; página com o gráfico, a tabela equivalente, os seletores e "Ver por quê"; bloco no Histórico; aviso no resultado do teste.
- **Critérios de aceite:** CA-T33-01 (a gravação já vem da L-10; aqui, o teste com a poda), CA-T33-02 a CA-T33-05.
- **Verificação:** testes de domínio e de propriedade (mediana e critério); testes de componente (`axe`); E2E com sessões sintéticas.

### W-11 — Saúde do pack com as categorias da 1.1 e painel Verificações do pack

- **Prioridade:** 1.1 · **Depende de:** W-02, W-04, W-09, W-10 · **Branch:** `feat/w-11-saude-e-verificacoes`
- **Objetivo:** as categorias "Segurança", "Manutenção" (ampliada), "Itens repetidos" e o novo critério de "Desempenho" na nota, e o painel no topo de Problemas (T14 parte 1.1).
- **Posse:** as categorias novas em `crates/warden-diagnostics/data/health-score.toml` e o ajuste em `crates/warden-diagnostics/src/health.rs` (a D-08 já integrada), `apps/desktop/src/features/diagnostics/verify-panel/**`, `apps/desktop/src/i18n/pt-BR/verificacoes.ts`.
- **Entregas:** pesos e tetos da T14 (1.1) em dados; achados de segurança e de manutenção contados só nas suas categorias; "Itens repetidos" com o teto de −3; painel "Verificações do pack" registrado em `problems-blocks.ts`, com as três linhas e os links.
- **Critérios de aceite:** CA-T14-16, CA-T14-17, CA-T32-04; a propriedade "acrescentar um achado nunca aumenta a nota" continua valendo com as categorias novas (`proptest`).
- **Verificação:** testes de domínio e de propriedade; testes de componente.

### W-12 — Revisão da 1.1 e fluxos de ponta a ponta

- **Prioridade:** 1.1 · **Depende de:** W-01 a W-11 · **Branch:** `test/w-12-revisao-1-1`
- **Objetivo:** fazer pela 1.1 o que a A-07 faz pelas funções da D4: textos, estados, foco e contraste conferidos com o glossário e o design system, e E2E dos fluxos 11 a 13 do protótipo.
- **Posse:** `apps/desktop/e2e/pro-flows.e2e.ts`, a seção "Warden 1.1" de `docs/ROTEIRO-DE-ACEITE.md`, ajustes em `apps/desktop/src/i18n/pt-BR/**` e componentes apontados pela revisão (lista declarada antes de começar, aprovada pelo orquestrador).
- **Critérios de aceite:** nenhuma violação séria do `axe` nas telas da 1.1; página de segurança, página do jogador e gráfico de desempenho navegáveis só por teclado; textos conferidos com o glossário (QUALITY §8.2); E2E dos fluxos 11 (conferir a segurança e trocar um mod removido antes de publicar), 12 (analisar o travamento de um jogador) e 13 (notas e grupos, desempenho entre versões) passa em Linux e no Windows; roteiro executado pelo dono.
- **Verificação:** `pnpm -C apps/desktop test`; `pnpm -C apps/desktop e2e`; roteiro manual.

---

## Spikes da D4

Investigações curtas, com relatório em `docs/spikes/` e código descartável numa branch `spike/...` (como o S1), recomendadas pela R5A §10.2 e aprovadas pelo dono (decisão D26). Não dependem de nada e rodam logo; as tarefas que dependem delas esperam o relatório. Cada relatório termina com "Implicações para a tarefa X", e o orquestrador ajusta a tarefa antes de despachá-la. **Os quatro terminaram em 04/10/2026** e as implicações já estão nas tarefas (D7): S-R5-1 → D-10 (ADR-0056), S-R5-2 → L-04 e L-11, S-R5-3 → D-12, L-05, L-02, L-03 e D-01, S-R5-4 → D-04 (ADR-0055).

### S-R5-1 — intermed como base do raio-x de mixins

- **Prioridade:** P1 (antes da D-10) · **Depende de:** — · **Branch:** `spike/s-r5-1-intermed`
- **Pergunta:** o intermed (Rust, MIT, https://github.com/jarettr/intermed) pode ser usado como biblioteca ou portado para o raio-x? Qual a qualidade em 3 packs reais (Fabric 1.20.1, Forge 1.20.1, NeoForge 1.21.1) e a taxa de falsos positivos com os rebaixadores da R5A §5.5, comparada com o protótipo da R5A?
- **Posse:** `docs/spikes/S-R5-1-intermed.md`.
- **Situação:** concluído em 04/10/2026; relatório em `docs/spikes/S-R5-1-intermed.md`.
- **Entregas:** relatório com licença e maturidade do projeto, API, desempenho, resultado nos 3 packs, comparação com o protótipo e recomendação (dependência, porte com atribuição ou implementação própria com `cafebabe`).
- **Verificação:** comandos e saídas no relatório; jars usados com a origem.

### S-R5-2 — Memória da JVM sem JDK no Windows

- **Prioridade:** P1 (antes da L-11) · **Depende de:** — · **Branch:** `spike/s-r5-2-hsperfdata`
- **Pergunta:** o `hsperfdata` pode ser lido pelo *file mapping* nomeado no Windows com Java 8, 17, 21 e 25, inclusive com nome de usuário acentuado, e os valores batem com o `jstat` de um JDK?
- **Posse:** `docs/spikes/S-R5-2-memoria-da-jvm.md`.
- **Situação:** concluído em 04/10/2026; relatório em `docs/spikes/S-R5-2-memoria-da-jvm.md`.
- **Entregas:** leitor de prova em Rust; tabela de comparação com o `jstat` por versão de Java; comportamento com `-XX:+PerfDisableSharedMem`; arquivos `hsperfdata` gravados para os testes da L-11.
- **Verificação:** execução real direto no Windows desta máquina (ADR-0048), com os Javas baixados para uma pasta temporária, registrada no relatório.

### S-R5-3 — Marcadores e mecanismos da busca do culpado

- **Prioridade:** P1 (antes da D-12) · **Depende de:** — (pode usar o protótipo do S1 na branch `spike/launcher-engine`) · **Branch:** `spike/s-r5-3-marcadores`
- **Pergunta:** para cada faixa da matriz L-05: quais são os marcadores de sucesso no cliente e de "entrou no mundo"; `--quickPlaySingleplayer` (1.20+) e `--quickPlayPath` funcionam como esperado; `--server/--port` com servidor local funciona antes de 1.20; `-Dfml.queryResult=confirm` (1.12.2) e `-Dfabric.noGui` fazem o que a R5A diz; quanto dura uma rodada num pack de ~150 mods?
- **Posse:** `docs/spikes/S-R5-3-marcadores-da-busca.md`.
- **Situação:** concluído em 04/10/2026; relatório em `docs/spikes/S-R5-3-marcadores-da-busca.md`.
- **Entregas:** tabela de marcadores e argumentos por faixa, golden logs para os testes da D-12, tempos medidos e a estimativa de rodadas revisada.
- **Verificação:** execuções reais registradas no relatório (Linux com servidor gráfico virtual; Windows para pelo menos 1.12.2 Forge e 1.21.1 NeoForge).

### S-R5-4 — Laço de ferramentas do Gemini

- **Prioridade:** P1 (antes da D-04) · **Depende de:** — (chave do Gemini, §2) · **Branch:** `spike/s-r5-4-laco-gemini`
- **Pergunta:** um laço `generateContent` sem estado com 10 a 20 ferramentas, *thought signatures*, chamadas paralelas, modo `VALIDATED` e resposta final em JSON com conferência de evidência funciona com o Gemini 3.x? Quanto custa e quanto demora uma conversa típica?
- **Posse:** `docs/spikes/S-R5-4-laco-do-gemini.md`.
- **Situação:** concluído em 04/10/2026; relatório em `docs/spikes/S-R5-4-laco-do-gemini.md`.
- **Entregas:** protótipo do laço contra um servidor simulado e depois com a chave real; respostas reais gravadas (redigidas) para os testes da D-04; erros encontrados (assinaturas em chamadas paralelas, `MALFORMED_FUNCTION_CALL`); tokens e custo medidos.
- **Verificação:** execuções registradas no relatório; nenhuma chave em arquivo do repositório.

---

## 13. Marcos

| Marco | Contém | O que o dono consegue fazer |
|---|---|---|
| M0 — Fundação | Fase 0 | Abrir o Warden no Windows (vazio), ver a CI verde. |
| M1 — Montar packs | P1-01 a P1-13, P1-16, V-01 | Criar/abrir packs, descobrir e adicionar vários mods de uma vez (Modrinth, CurseForge, link e arquivo), atualizar. |
| M2 — Testar | L-01 a L-06, L-10, D-01 a D-03, D-05 a D-07 | Testar o pack no jogo, ver o console, a RAM e o tempo de carregamento, entender travamentos e ver a lista de Travamentos. |
| M3 — Núcleo completo | C-01 a C-03, C-05, V-02, V-03, E-01, P1-18 | Editar e buscar configs, trazer mudanças do jogo, salvar versões, publicar versões finais, exportar, criar packs com os mods iniciais. Todas as funcionalidades P0 existem. |
| M4 — Ferramenta completa | S-R5-1, S-R5-2, S-R5-3, S-R5-4, P1-14, P1-15, P1-17, P1-19, L-07 a L-09, L-11, L-12, C-04, C-06, C-07, D-04, D-08 a D-14, E-02 a E-04, A-05, A-06 | Busca do culpado, IA com ferramentas, raio-x, grafo, nota de saúde, console agrupado, perfil de desempenho, servidor local, perfis, scripts, formulário de configs, importar, modpacks, kits, pacote para servidor, instância pronta para o Prism. |
| M5 — v1 | A-01 a A-04, A-07 | Versão para uso diário: revisada, com instalador e validada pelo dono. A-01, A-02 e A-04 podem acontecer antes, em paralelo com o M4. |
| M6 — Warden 1.1 "Profissional" | W-01 a W-12 | Conferir a segurança dos mods antes de testar e de publicar, ser avisado de mods removidos ou abandonados e trocá-los por substitutos, analisar o travamento que um jogador mandou, organizar os mods com notas e grupos, ver os itens repetidos entre mods e acompanhar o desempenho entre versões. Começa depois do M5. |

## 14. Backlog P2

Não planejado em tarefas ainda (SPEC §9). Da D5: atualização remota da lista de sinais de segurança; procurar rastros de malware no computador; enviar arquivos a serviços de análise de terceiros e regras YARA; configurar o unificador de itens sozinho e unificar líquidos; comparar desempenho entre computadores; desempenho medido automaticamente com o spark; receber travamentos dos jogadores automaticamente; editor de quests. Antes da D5: teste com mundo antigo trazido de fora; Legacy Fabric; migração de versão do Minecraft; variantes do pack; formulário para YAML; editor de `servers.dat`; conflitos de teclas; modelos de pack ("Salvar como modelo", "Criar a partir de um modpack"); datapacks globais e datapacks como Tipo na descoberta; trazer mudanças do GitHub; login no GitHub pelo navegador; atualização remota dos dados curados (diagnóstico, kits, categorias); leitura de `@Mod` no bytecode para 1.7.10/1.12.2; mecanismos de "padrão na primeira execução" por mod (YOSBR, Default Options, Config Manager). Da D4 (R5A §10.1 e R5B §9): raio-x camadas D (nomes sem refmap) e E (aplicação a seco do Mixin); perfil automático do cliente (JFR ou agente do spark) e tempo por mod no Fabric; busca do culpado em configs, scripts e por desempenho; saída estruturada do log4j por faixa; "Descobrir padrões" (camadas B e C); metadados de config por bytecode e dicionário curado; autocompletar completo de KubeJS (`tsc` 7 + ProbeJS) e servidor de linguagem de ZenScript; matriz de perfis; "Ver como este modpack configurou o mod X"; importar perfil do app do Modrinth.
