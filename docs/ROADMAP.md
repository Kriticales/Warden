# Warden — Plano de construção

> Versão do documento: 1.1 (2026-10-01). Tarefa A1; escopos ajustados na tarefa D2 às decisões do dono (estrutura de navegação, busca combinada, chaves no cofre ou `.env`, Java, seção de IA e publicação no GitHub; ADR-0025 a ADR-0029).
> Cada tarefa abaixo é entregue por um agente numa branch própria, seguindo `QUALITY.md`. O orquestrador despacha, revisa e integra.
> Referências: `SPEC.md` (telas T01–T23 e critérios CA-*), `ARCHITECTURE.md` (§ citados), `docs/decisions/` (ADR-*).

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
11. [Marcos](#11-marcos)
12. [Backlog P2](#12-backlog-p2)

---

## 1. Como ler este plano

- **ID estável:** `F0-xx` (fundação), `P1-xx` (núcleo do pack), `L-xx` (launcher), `C-xx` (configs), `D-xx` (diagnóstico), `V-xx` (versionamento), `E-xx` (exportação), `A-xx` (acabamento). `S1` é o spike do motor do launcher, já em andamento fora deste plano.
- **Fases** agrupam por assunto; a **ordem real** é dada pelas dependências. Uma tarefa pode começar assim que as dependências estiverem integradas na `main` (ondas na §3).
- **Posse:** arquivos e pastas que só aquela tarefa altera. Fora da posse, só os **registros acréscimo-apenas** (uma linha por entrada, conflitos resolvidos pelo orquestrador):
  - `Cargo.toml` da raiz (`[workspace.dependencies]`) e `Cargo.lock`;
  - `apps/desktop/package.json` (dependências) e `pnpm-lock.yaml`;
  - `apps/desktop/src-tauri/Cargo.toml` (dependências);
  - `apps/desktop/src-tauri/src/commands/mod.rs` (registro de comandos) e `src/state.rs` (campos do estado);
  - `apps/desktop/src/app/navigation.ts`, `apps/desktop/src/i18n/index.ts`, `apps/desktop/src/i18n/errors/index.ts`;
  - `xtask/src/main.rs` (registro de subcomandos);
  - `lib.rs` das crates divididas entre várias tarefas (`warden-project`, `warden-instance`, `warden-diagnostics`, `warden-versioning`): só declarações de módulos e reexportações;
  - enums de códigos de erro das crates (`crates/*/src/error.rs`, só acréscimo de variantes) e as frases correspondentes em `apps/desktop/src/i18n/errors/<domínio>.ts`;
  - `apps/desktop/src/features/pack-editor/header/slots.ts` e `apps/desktop/src/features/pack-editor/sections.ts` (botões/indicadores do cabeçalho e seções do menu do pack, criados pela P1-08);
  - `apps/desktop/src-tauri/src/test_hooks/mod.rs` (ganchos do Testar, criado pela L-04);
  - `apps/desktop/src/features/settings/sections.ts` (seções da tela de Configurações);
  - `apps/desktop/src/lib/ipc/bindings.ts` (gerado; regenerar após integrar);
  - `THIRD_PARTY.md` (uma seção por origem).
- **Critérios de aceite** citam os CA da SPEC quando existem; os demais são específicos da tarefa.
- **Verificação:** além dos comandos listados, toda tarefa roda `cargo xtask check` (QUALITY §12) e informa o resultado.
- **Branch:** `<tipo>/<id-minúsculo>-<descrição>` (QUALITY §7.1), indicada em cada tarefa.

## 2. Ações necessárias do dono

| Quando | Ação | Por quê |
|---|---|---|
| ~~Antes de F0-01~~ | **Feito em 01/10/2026 pelo orquestrador**, com autorização do dono: bibliotecas do Tauri instaladas no WSL (`webkitgtk-webdriver` substitui `webkit2gtk-driver` no Ubuntu 26.04). | — |
| ~~Antes de F0-04~~ | ~~Responder D5~~ — **respondido "sim" em 01/10/2026** (SPEC §10). | `cargo xtask win-dev` pode ser entregue. |
| Antes de F0-02 | No GitHub, em Settings → Secrets → Actions do repositório `Kriticales/Warden`, criar `CURSEFORGE_API_KEY` com a chave. | Testes de rede agendados na CI. |
| Antes de V-03 | Gerar um token do GitHub com permissão para criar repositórios (públicos e privados), enviar conteúdo e criar Releases (passo a passo virá no app) e salvá-lo no Warden. Para os testes de integração de V-03, uma conta ou repositórios de teste descartáveis. | Publicar versão para os jogadores (D14). |
| Antes de D-04 | Criar uma chave do Gemini (Google AI Studio). | IA do diagnóstico. |
| Em cada marco (§11) | Abrir o app no Windows e seguir o roteiro de aceite entregue. | Validação real no Windows. |

## 3. Ondas de paralelismo

Uma onda começa quando as dependências da anterior estão integradas. Dentro de cada onda as tarefas têm posse disjunta.

| Onda | Tarefas (em paralelo) |
|---|---|
| 0 | F0-01 |
| 1 | F0-03, F0-05, P1-01, P1-06, C-01 (+ S1 em andamento) |
| 2 | F0-02, F0-04, F0-06, P1-02, P1-03, V-01, D-02 |
| 3 | P1-04, P1-05, P1-13, L-01, A-02 |
| 4 | P1-07, L-02 (assim que o S1 concluir), L-03, D-01 |
| 5 | P1-08, E-01, L-05 |
| 6 | P1-09, P1-14, C-02, V-02, L-04, A-04, A-05, E-02 |
| 7 | P1-10, C-03, C-04, D-03, V-03, L-07, A-06 |
| 8 | P1-11, P1-12, L-06, D-04 |
| 9 | P1-15, A-01 |
| 10 | A-03 |

A tabela é derivada das dependências declaradas em cada tarefa; em caso de dúvida, valem as dependências. Se o S1 atrasar, L-02 (e tudo o que depende dela: L-04 a L-07, C-03, D-03, D-04) desliza junto, sem afetar as demais.

---

## Fase 0 — Fundação

Ao fim desta fase: o app abre (Linux e Windows), a CI roda em Linux e Windows, o sidecar do packwiz é compilado de commit fixado, o dono consegue abrir o app no Windows a partir do WSL, e toda a infraestrutura comum (erros, operações, travas, registros, configurações, cofre, layout, i18n, testes) existe.

### F0-01 — Esqueleto do monorepo e app compilando

- **Prioridade:** P0 · **Depende de:** ação do dono (pacotes de sistema) · **Branch:** `feat/f0-01-esqueleto`
- **Objetivo:** criar a estrutura do monorepo (ARCHITECTURE §2) com o app Tauri 2 + React/TypeScript abrindo uma janela, todas as crates de domínio como esqueletos vazios e as configurações de lint/format/teste.
- **Posse:** arquivos da raiz (`Cargo.toml`, `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `deny.toml`, `.config/nextest.toml`, `package.json`, `pnpm-workspace.yaml`, `.npmrc`, `.nvmrc`, `.editorconfig`, `.gitattributes`, `.gitignore`, `THIRD_PARTY.md`, `README.md`), `apps/desktop/**` (estrutura inicial), `crates/*/Cargo.toml` e `crates/*/src/lib.rs` (esqueletos de todas as crates da ARCHITECTURE §3), `xtask/**` (inicial, com os subcomandos listados abaixo).
- **Entregas:**
  - Workspace com `[workspace.lints]` e `[workspace.dependencies]` (QUALITY §2.1); Rust 1.98.1 fixado; edição 2024.
  - App Tauri 2 (`identifier = "dev.kriticales.warden"`, `productName = "Warden"`), React 19 + Vite + TS strict + ESLint/Prettier conforme QUALITY §2.2, TanStack Router com rotas por arquivo, i18next com pt-BR.
  - `tauri.conf.json` com a CSP da ARCHITECTURE §20 (o `bundle.externalBin` é acrescentado pela F0-03; o resto do bundle pela A-02).
  - `tauri-specta` integrado com um comando `app_info` (versão, commit) e `AppError` mínimo; `bindings.ts` gerado e versionado.
  - Subcomandos do xtask: `setup` (instala no espaço do usuário `cargo-nextest`, `cargo-deny`, `cargo-llvm-cov`, `tauri-cli`, e roda `pnpm install`), `dev` (carrega o `.env` do repositório principal sem imprimir valores e abre o app via WSLg), `check` e `check --fast` (QUALITY §12), `check-deps`, `check-docs` (links internos dos docs), `bindings` (com `--check`), `coverage` (mínimos da QUALITY §4.2, configuráveis por crate) e `test-network` (testes `#[ignore = "rede"]` com o `.env`).
  - `xtask check-deps`: falha se alguma crate fora de `warden-app` depender de `tauri`.
  - `README.md` da raiz com: pré-requisitos, `cargo xtask setup`, `cargo xtask dev` (abre o app via WSLg), `cargo xtask check`.
- **Critérios de aceite:**
  1. `cargo xtask check` passa num clone limpo após `cargo xtask setup`.
  2. `cargo xtask dev` abre a janela "Warden" no WSLg mostrando uma página inicial com texto vindo do catálogo pt-BR e a versão obtida por `app_info`.
  3. `cargo xtask check-deps` falha num teste em que uma crate de domínio declara `tauri` (teste do próprio xtask).
  4. `bindings.ts` regenerado é idêntico ao versionado (`cargo xtask bindings --check`).
- **Verificação:** `cargo xtask setup && cargo xtask check`; `cargo xtask dev` (captura de tela no relatório).

### F0-02 — CI no GitHub Actions (Linux + Windows)

- **Prioridade:** P0 · **Depende de:** F0-01, F0-03 · **Branch:** `ci/f0-02-github-actions`
- **Objetivo:** CI que garante o padrão de qualidade em Linux e Windows e produz o instalador de Windows como artefato.
- **Posse:** `.github/workflows/**`, `.github/actionlint.yaml`, `.gitleaks.toml`.
- **Entregas:**
  - `ci.yml`: em push/PR — Linux (`ubuntu-24.04`): `cargo xtask setup`, sidecar (com cache por commit do packwiz), `cargo xtask check`, `cargo xtask coverage`, E2E Linux (quando existir), `gitleaks`, `actionlint`. Windows (`windows-latest`): o mesmo `check` + `tauri build` gerando instalador NSIS como artefato `warden-windows-<sha>`; executado na `main`, em PRs com rótulo `windows` e à noite (D9).
  - `nightly.yml`: testes de rede (`CURSEFORGE_API_KEY` dos secrets), conformidade no Windows, E2E Windows.
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

### F0-04 — Abrir o app no Windows a partir do WSL

- **Prioridade:** P0 · **Depende de:** F0-01, F0-03, D5 respondida · **Branch:** `build/f0-04-windows-a-partir-do-wsl`
- **Objetivo:** o dono (ou o orquestrador) roda um comando no WSL e o Warden abre no Windows.
- **Posse:** `xtask/src/windows.rs`, `docs/DEV-WINDOWS.md`.
- **Entregas:**
  - `cargo xtask win-dev` (só se D5 = "sim" explícito): compila o app para `x86_64-pc-windows-msvc` com `cargo-xwin`, inclui o sidecar `.exe`, copia para `C:\Users\<usuário Windows>\Warden-dev\` (descoberto via `cmd.exe /c echo %USERPROFILE%` + `wslpath`), cria atalho na Área de Trabalho e abre o app (interop do WSL).
  - `cargo xtask win-install`: alternativa sem compilar — baixa com `gh run download` o último artefato de Windows da `main`, instala silenciosamente (`/S`) e abre.
  - `docs/DEV-WINDOWS.md`: passo a passo para o dono, em linguagem simples, com solução para o aviso do SmartScreen.
- **Critérios de aceite:**
  1. Em uma máquina com Windows 11 e WSL2, `cargo xtask win-dev` (ou, sem D5, `cargo xtask win-install`) termina com o Warden aberto no Windows mostrando a versão (roteiro manual com captura de tela).
  2. A janela usa o WebView2 do Windows (verificado em `app_info`, que informa a plataforma) e o sidecar `packwiz.exe` está ao lado do executável.
  3. Rodar de novo atualiza a cópia sem apagar dados do app.
- **Verificação:** roteiro manual; `cargo nextest run -p xtask` (descoberta de caminhos testada com valores simulados).

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
  - Registros com `tracing` + arquivo diário + `tauri-plugin-log`; gancho de pânico; `tauri-plugin-single-instance`; capabilities da ARCHITECTURE §20.
- **Critérios de aceite:**
  1. `atomic_write` com falha injetada entre escrita e renomeação deixa o arquivo original intacto e nenhum `.warden-tmp` após a limpeza.
  2. `resolve_inside` recusa `../x`, caminhos absolutos, `C:\x` e links que saem da raiz (testes com `proptest`).
  3. `secrets_set` + reinício mantém o estado, com o cofre de teste em arquivo (Linux), com o cofre real do Windows (CI Windows) e no modo `.env`; trocar de modo move as chaves, e uma falha injetada no meio não perde nenhuma; nenhum valor de segredo aparece em `settings.json` nem nos registros (teste de varredura). CA-T21-01 e CA-T21-03 (parte de backend).
  4. Duas operações de escrita no mesmo pack são serializadas; em packs diferentes, rodam em paralelo (teste de concorrência).
  5. Cancelar uma operação longa simulada encerra em < 2 s com `CANCELLED`.
  6. Abrir uma segunda cópia do app foca a primeira.
- **Verificação:** `cargo nextest run -p warden-core -p warden-secrets -p warden-app`; `cargo xtask check`.

### F0-06 — Fundação do frontend

- **Prioridade:** P0 · **Depende de:** F0-01, F0-05 · **Branch:** `feat/f0-06-fundacao-frontend`
- **Objetivo:** base visual e técnica para todas as telas.
- **Posse:** `apps/desktop/src/{app,components,lib,styles}/**` (exceto `lib/ipc/bindings.ts`, gerado), `apps/desktop/src/routes/{__root.tsx,index.tsx}`, `apps/desktop/src/features/about/**` (exceto `licenses/`, da A-02), `apps/desktop/src/i18n/pt-BR/{comum.ts,navegacao.ts,sobre.ts,tarefas.ts}`, `apps/desktop/e2e/**` (harness), `apps/desktop/wdio.conf.ts`, `apps/desktop/vitest.config.ts`.
- **Entregas:**
  - shadcn/ui + Tailwind 4 com tokens de cor, tema claro/escuro seguindo o sistema; layout do nível do app **sem barra lateral** (ADR-0026): topo com o nome do app e Configurações, rodapé com o indicador de Tarefas em todas as telas (do app e do pack), que abre a gaveta T22.
  - Componentes comuns: `EmptyState`, `ErrorPanel` (com "Detalhes técnicos" e Copiar), `LoadingState`, `ConfirmDialog` (com confirmação por digitação), `ProgressBar`, `OperationToast`, `DiffView`, `SafeHtml`, `SafeMarkdown`.
  - `lib/ipc`: wrappers de query/mutation, assinatura de eventos com invalidação de `pack-changed`, tradução de `AppError`.
  - Gaveta Tarefas (T22) consumindo `operations_list`/`operation-updated`/`operation_cancel`; componente "Sobre o Warden" (T23) com aviso legal e versão, que a P1-13 encaixa como última seção de Configurações pelo registro `features/settings/sections.ts`.
  - Harness de testes: Vitest + `mockIPC` + fábricas a partir dos tipos gerados + `vitest-axe`; E2E WebdriverIO + `tauri-driver` com um teste de fumaça (abre o app e abre a gaveta de Tarefas) e servidor local de fixtures (`e2e/mock-server/`).
  - Guarda de contrato (QUALITY §4.1): teste que compara comandos registrados com usos em `src/features/`.
- **Critérios de aceite:**
  1. Todos os componentes comuns têm testes de estado e passam no `axe`.
  2. `ErrorPanel` mostra frase traduzida para um `AppError` de cada domínio e "Detalhes técnicos" com código e `detail`.
  3. E2E de fumaça passa em Linux (Xvfb).
  4. CA-T01-04 (aviso legal no componente "Sobre o Warden"), CA-T22-01 parcial (cancelamento pela interface, com operação simulada).
- **Verificação:** `pnpm -C apps/desktop test && pnpm -C apps/desktop e2e`; `cargo xtask check`.

---

## Fase 1 — Núcleo do pack

### P1-01 — `warden-packwiz`: formato, hashes e higiene

- **Prioridade:** P0 · **Depende de:** F0-01 · **Branch:** `feat/p1-01-formato-packwiz`
- **Objetivo:** ler e escrever o formato packwiz exatamente como o packwiz (ARCHITECTURE §6.1–§6.4).
- **Posse:** `crates/warden-packwiz/**`, `xtask/src/fixtures_packwiz.rs`.
- **Entregas:** modelo de `pack.toml`, `index.toml`, `.pw.toml` (todos os campos de R3 §1.5–§1.7, inclusive `[option]`, `pin`, `preserve`, `[export.curseforge]`, `[options]`); leitura tolerante (erro por arquivo); escrita de arquivo novo no estilo do packwiz; edição mínima com `toml_edit`; nomes únicos de metafile; mapeamento de lado do Modrinth; `.packwizignore` com padrões embutidos do packwiz; hashes sha1/sha256/sha512/md5/murmur2 (variante CurseForge); templates de `.packwizignore`, `.gitignore`, `.gitattributes`; verificação de higiene (padrões da §6.4); `cargo xtask fixtures-packwiz` gera fixtures com o packwiz real.
- **Critérios de aceite:**
  1. Para cada fixture gerada pelo packwiz real (Modrinth, CurseForge, URL, `pack.toml` de cada loader, `index.toml` com `preserve`), ler e escrever produz bytes idênticos.
  2. Um `.pw.toml` escrito pelo Warden para o mesmo mod é idêntico ao do packwiz (base de CA-T08-01 e CA-T08-03).
  3. `murmur2` confere com as impressões digitais da CurseForge para 5 jars reais.
  4. O matcher de `.packwizignore` dá o mesmo resultado que o packwiz para um conjunto de 100 caminhos (comparado executando `packwiz refresh` num pack de teste).
  5. Um `.pw.toml` inválido produz erro só para aquele arquivo.
- **Verificação:** `cargo nextest run -p warden-packwiz`; cobertura ≥ 85%.

### P1-02 — `warden-packwiz-cli`: execução do sidecar, staging e conformidade

- **Prioridade:** P0 · **Depende de:** F0-03, P1-01, F0-05 · **Branch:** `feat/p1-02-packwiz-cli`
- **Objetivo:** chamar o packwiz com segurança e provar que um pack está "como o packwiz produziria" (ARCHITECTURE §6.3).
- **Posse:** `crates/warden-packwiz-cli/**`.
- **Entregas:** construtores de argv puros (`refresh`, `refresh --build`, `curseforge add <url>`, `list`, `modrinth export`, `curseforge export`); execução com tempo-limite, cancelamento, ambiente limpo, `CREATE_NO_WINDOW`, stdin controlado; decodificação de linhas e remoção de ANSI/progresso; detecção de falha por código e pós-condição; cópia de staging; `check_conformance(pack)` (refresh em staging + zero diferença).
- **Critérios de aceite:**
  1. `refresh` num pack de teste com 300 metafiles termina com sucesso e o índice relido confere.
  2. Um pack com `index.toml` desatualizado é reprovado por `check_conformance` com a lista de diferenças.
  3. Cancelar um `refresh` lento (pack sintético grande) mata o processo em < 2 s.
  4. Saída do packwiz com ANSI e barra de progresso vira linhas limpas (teste dourado com saída real gravada).
  5. A chave nunca aparece no argv nem nos registros (teste).
- **Verificação:** `WARDEN_REQUIRE_EXTERNALS=1 cargo nextest run -p warden-packwiz-cli`.

### P1-03 — `warden-http` e `warden-modrinth`

- **Prioridade:** P0 · **Depende de:** F0-05 · **Branch:** `feat/p1-03-http-modrinth`
- **Objetivo:** cliente HTTP comum e cliente completo do Modrinth (ARCHITECTURE §17).
- **Posse:** `crates/warden-http/**`, `crates/warden-modrinth/**`.
- **Entregas:** User-Agent, limitador por host, novas tentativas, download em streaming com hash e `.part`; endpoints do Modrinth (busca com facets, projeto(s), versões, `version_files`, `version_files/update`, tags); cache SQLite de projetos/versões; códigos de erro e traduções do domínio.
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
- **Entregas:** busca (com `modLoaderType`, `gameVersion`, classes de mods/resource packs/shaders — confirmar o `classId` de shaders pela API), mod(s), arquivos, `download-url`, impressões digitais, detecção de distribuição bloqueada; cache só em memória; ligação do `secrets_test` para a CurseForge.
- **Critérios de aceite:**
  1. Teste de rede: buscar "jei" para Forge 1.20.1 retorna o JEI; um mod com distribuição bloqueada conhecido é identificado como bloqueado.
  2. Chave inválida → `CURSEFORGE_KEY_INVALID`; sem chave → `CURSEFORGE_KEY_MISSING` sem requisição (CA-T08-06 parte backend, CA-T21-02).
  3. Nenhuma resposta da API da CurseForge (JSON, descrições, URLs de download) é escrita em disco (teste que inspeciona o diretório de dados após uma sessão, excluindo o cache de arquivos de mods baixados).
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
- **Entregas:** registro de packs (`packs.json`), `PackTransaction` com bloco obrigatório do `.packwizignore` (ARCHITECTURE §6.4–§6.5), criação sem `packwiz init` (com `git` via V-01), importação (`pack_import`) com validações, repositórios git existentes (ARCHITECTURE §11) e higiene, limpeza de sobras `.warden-tmp`, telas com todos os estados; (P1) Apagar pack para a Lixeira (`pack_trash`).
- **Critérios de aceite:** CA-T02-01 a CA-T02-04, CA-T03-01 a CA-T03-06, CA-T04-01 a CA-T04-04; `PackTransaction` reverte corretamente com falha injetada em cada passo.
- **Verificação:** `WARDEN_REQUIRE_EXTERNALS=1 cargo nextest run -p warden-project`; `pnpm -C apps/desktop test -- packs`; E2E `packs.e2e.ts`.

### P1-08 — Editor do pack: cabeçalho, inventário e detalhes

- **Prioridade:** P0 · **Depende de:** P1-07, P1-03, L-01 · **Branch:** `feat/p1-08-editor-do-pack`
- **Objetivo:** telas T05, T06 e T07 e a parte P0 de T11 (Informações do pack e o diálogo Ajustes do teste), na estrutura aprovada (ADR-0026).
- **Posse:** `crates/warden-project/src/{inventory.rs,details.rs,side.rs,remove.rs,meta.rs}`, `apps/desktop/src-tauri/src/commands/{inventory.rs,pack_meta.rs}`, `apps/desktop/src/features/pack-editor/**`, `apps/desktop/src/routes/packs/$packId/{route.tsx,index.tsx,mods.tsx}`, `apps/desktop/src/i18n/pt-BR/editor.ts`, `apps/desktop/e2e/editor.e2e.ts`.
- **Entregas:** layout do pack (cabeçalho fixo com ← Meus packs, nome e "Editar informações", identificação, avisos passageiros e "Salvar versão · N alterações"; menu lateral com as 6 seções, com descrição e contador; o pack abre em Mods); lista única de mods, resource packs e shaders agrupada por tipo; inventário guiado pelo índice e tolerante a erros, versão legível pelo cache, detalhes em painel lateral (Modrinth do cache; CurseForge ao vivo), lado individual e em lote, remover (com dependentes), Informações do pack (nome/autor/descrição no `pack.toml`) e o diálogo Ajustes do teste neste computador (memória, Java automático com motivo via `java_choice` da L-01, argumentos, recriar instância; gravados em `packs.json`, com efeito real a partir da L-04, que o expõe no menu ▾ do Testar). Cria os pontos de extensão `features/pack-editor/header/slots.ts` (botões e indicadores do cabeçalho registrados por L-04, V-02, C-03, D-03 e A-05) e `features/pack-editor/sections.ts` (seções do menu, registradas por C-02, D-03, D-04, V-02 e E-01).
- **Critérios de aceite:** CA-T05-01 a CA-T05-03, CA-T06-01 a CA-T06-04, CA-T07-01, CA-T07-02, CA-T11-01, CA-T11-03.
- **Verificação:** `cargo nextest run -p warden-project`; testes de componente; E2E `editor.e2e.ts`.

### P1-09 — Adicionar: página, busca combinada e Modrinth (com dependências)

- **Prioridade:** P0 · **Depende de:** P1-08, P1-06 · **Branch:** `feat/p1-09-adicionar-modrinth`
- **Objetivo:** página Adicionar sem abas (Tipo, campo único, Escolher arquivo) e o motor da busca combinada (ADR-0027) com a fonte Modrinth, mais a tela de dependências (T08, T09).
- **Posse:** `crates/warden-project/src/add/{mod.rs,plan.rs,modrinth.rs}`, `crates/warden-project/src/search/{mod.rs,merge.rs,modrinth.rs}`, `crates/warden-project/src/{deps.rs,dedup.rs}`, `apps/desktop/src-tauri/src/commands/add.rs`, `apps/desktop/src/features/add/{AddPanel.tsx,common/**,modrinth/**,dependencies/**}`, `apps/desktop/src/i18n/pt-BR/adicionar.ts`, `apps/desktop/e2e/add-modrinth.e2e.ts`.
- **Entregas:** `add_plan`/`add_apply` (estrutura usada também por P1-10 e P1-11); `search_projects` com fontes plugáveis (trait de fonte), intercalação por posição, deduplicação por autor e nome/slug, paginação por fonte e resultado parcial com `SEARCH_SOURCE_UNAVAILABLE` (ARCHITECTURE §17); fonte Modrinth com filtros travados; pré-visualização com seletores de fonte, versão e canal; resolução recursiva de dependências obrigatórias, incompatibilidades declaradas, deduplicação entre fontes, gravação atômica de vários itens.
- **Critérios de aceite:** CA-T08-01, CA-T09-01 a CA-T09-04; intercalação, deduplicação e falha parcial cobertas por testes do motor com duas fontes simuladas.
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
- **Posse:** `apps/desktop/src/features/settings/**` (exceto as subpastas `java/`, de L-01, `ai/`, de D-04, e `storage/`, de A-06), `apps/desktop/src/features/onboarding/**`, `apps/desktop/src/routes/{configuracoes.tsx,boas-vindas.tsx}`, `apps/desktop/src/i18n/pt-BR/{configuracoes.ts,boas-vindas.ts}`, `apps/desktop/e2e/settings.e2e.ts`.
- **Entregas:** assistente de primeira execução, tela de Configurações com chaves (status, testar, substituir, remover) e "Onde guardar as chaves" (cofre ou `.env`, com a confirmação e o aviso fixo), nome do jogador validado, pasta padrão, canal de versões, diferenças antes de salvar, nível de registros.
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

- **Prioridade:** P1 · **Depende de:** P1-09, P1-12 · **Branch:** `feat/p1-15-trocar-versoes`
- **Objetivo:** itens P1 de T06, T07 e T11 que dependem da resolução de dependências.
- **Posse:** `crates/warden-project/src/{change_version.rs,loader_version.rs,orphans.rs}`, `apps/desktop/src-tauri/src/commands/versions_change.rs`, `apps/desktop/src/features/pack-editor/change-version/**`, `apps/desktop/src/i18n/pt-BR/trocar-versao.ts`.
- **Entregas:** "Trocar de versão" de um item (qualquer versão compatível, inclusive mais antiga); trocar a versão do loader do pack (mesmo loader) com verificação de compatibilidade dos itens; ao remover, oferecer remover dependências órfãs.
- **Critérios de aceite:** trocar a versão de um mod grava a referência nova e passa em `check_conformance`; trocar a versão do loader altera só a linha correspondente em `[versions]` e roda o diagnóstico rápido; dependências órfãs são listadas corretamente num pack de teste com cadeia de dependências.
- **Verificação:** integração com packwiz real; testes de componente.

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
- **Entregas:** adaptador do motor, `GameSpec`/`LaunchOptions`/`LaunchCommand`, UUID offline, argumentos por faixa de versão, `@argfile`, processo com pipes, decodificação, parser log4j XML/texto, Job Object no Windows, classificação da saída, gravação da sessão.
- **Critérios de aceite:**
  1. UUID offline igual ao do Java para 10 nomes (valores gerados por um programa Java no teste).
  2. Golden tests da linha de comando para cada combinação da matriz L-05.
  3. CA-T13-02, CA-T13-03, CA-T13-07 (com processo Java de teste que imita o jogo: escreve acentos, cria filhos, trava).
  4. A escolha do motor e o motivo estão num ADR, com o resultado do S1 citado.
- **Verificação:** `cargo nextest run -p warden-launcher` em Linux e Windows (CI).

### L-03 — `warden-instance`: materialização do pack

- **Prioridade:** P0 · **Depende de:** P1-01, P1-03, P1-04 · **Branch:** `feat/l-03-materializacao`
- **Objetivo:** montar a instância a partir do pack com a semântica do packwiz-installer (ARCHITECTURE §8.2, ADR-0011).
- **Posse:** `crates/warden-instance/src/{materialize.rs,manifest.rs,downloads.rs,blocked.rs,optional_choices.rs}` (e `lib.rs` inicial), `crates/warden-instance/tests/conformance_*.rs`, `xtask/src/installer.rs` (baixa, por versão e hash fixados, o bootstrap do packwiz-installer e um JRE Temurin para testes, numa pasta de cache do usuário).
- **Entregas:** algoritmo completo (lado, opcionais, `preserve`, remoção do que saiu, cache por hash, CurseForge na hora), manifesto de estado, lista de bloqueados para T20, pausa antes de sobrescrever arquivo alterado e não revisado.
- **Critérios de aceite:**
  1. Conformidade: para 5 packs de teste (com opcionais, `preserve`, lado servidor, configs, resource packs), a árvore do Warden é idêntica à do packwiz-installer real.
  2. CA-T13-04 (parte de sincronização) e CA-T22-01 (cancelamento da sincronização).
  3. Segunda materialização sem mudanças não baixa nem escreve nada (contagem de operações).
- **Verificação:** `WARDEN_REQUIRE_EXTERNALS=1 cargo nextest run -p warden-instance`.

### L-04 — Testar e console

- **Prioridade:** P0 · **Depende de:** L-02, L-03, P1-08 · **Branch:** `feat/l-04-testar-e-console`
- **Objetivo:** T13 completa (exceto "Testar como o jogador recebe", L-07). Os passos de verificação (1 e 4) e a captura ficam em pontos de extensão criados pela L-04 como passagem direta: no backend, o trait `TestHooks` (`before_prepare`, `before_launch`, `after_exit`) em `apps/desktop/src-tauri/src/test_hooks/mod.rs`, implementado por `test_hooks/diagnostics.rs` (D-03) e `test_hooks/capture.rs` (C-03); na interface, `apps/desktop/src/features/test/verification/` (D-03). Até serem preenchidos, esses passos não aparecem na interface.
- **Posse:** `apps/desktop/src-tauri/src/commands/test.rs`, `apps/desktop/src-tauri/src/test_session.rs`, `apps/desktop/src-tauri/src/test_hooks/mod.rs` (inicial), `apps/desktop/src/features/test/**` (exceto `verification/`, de D-03, `blocked/`, de L-06, e `clean/`, de L-07), `apps/desktop/src/routes/packs/$packId/teste.tsx`, `apps/desktop/src/i18n/pt-BR/teste.ts`, `apps/desktop/e2e/test.e2e.ts`.
- **Entregas:** orquestração das etapas, memória automática, um jogo por vez, confirmação ao fechar o app, console virtualizado com filtros e busca, sessões anteriores; botão Testar no cabeçalho com os estados "Testando… ver progresso" e "● Jogo aberto: ver teste"; menu ▾ (Ver último teste, Ajustes do teste neste computador — diálogo da P1-08 —, Abrir pasta da instância, Apagar mundos de teste, Recriar instância); tela do teste com o resultado e os espaços para "O que mudou durante o teste" (C-03) e "Por que travou" (D-03); cada sessão registra o hash da árvore do pack testada (usado pelo aviso "versão não testada" da V-03).
- **Critérios de aceite:** CA-T13-01 (pelo menos 1.20.1 Fabric e 1.12.2 Forge nesta tarefa; matriz completa em L-05), CA-T13-02 a CA-T13-08.
- **Verificação:** E2E com jogo simulado (processo Java de teste); roteiro manual no Windows com pack real.

### L-05 — Matriz de versões: golden e jogo real

- **Prioridade:** P0 · **Depende de:** L-02, L-03 · **Branch:** `test/l-05-matriz-de-versoes`
- **Objetivo:** provar o suporte garantido de 1.7.10 até a mais nova.
- **Posse:** `crates/warden-launcher/tests/smoke_*.rs`, `crates/warden-launcher/tests/matrix/**`, `.github/workflows/smoke-game.yml` (criado por esta tarefa).
- **Matriz:** 1.7.10 (vanilla; Forge 10.13.4.1614), 1.12.2 (vanilla; Forge 14.23.5.2860), 1.14.4 (Fabric), 1.16.5 (Forge 36.2.34; Fabric), 1.18.2 (Forge 40.x; Fabric), 1.20.1 (Forge 47.x; NeoForge 47.1.x; Fabric), 1.21.1 (Forge; NeoForge 21.1.x; Fabric), versão mais nova (vanilla; Forge; Fabric; NeoForge). Versões exatas fixadas na tarefa.
- **Entregas:** packs mínimos por combinação (com um mod simples e Mixin quando aplicável), teste que materializa, lança em Xvfb com Mesa, espera o marcador de sucesso no log e encerra; marcadores documentados por faixa; registro de tempos.
- **Critérios de aceite:** CA-T13-01 para toda a matriz em Linux; teste de Log4Shell por faixa (ARCHITECTURE §7.4); relatório de execução manual no Windows para 1.7.10 Forge, 1.20.1 Forge e a mais nova NeoForge.
- **Verificação:** workflow `smoke-game.yml` executado (manual) com sucesso; log anexado.

### L-06 — Downloads manuais da CurseForge

- **Prioridade:** P0 (seleção de arquivo) / P1 (observar Downloads) · **Depende de:** L-04, P1-10 · **Branch:** `feat/l-06-downloads-manuais`
- **Objetivo:** T20.
- **Posse:** `crates/warden-instance/src/manual_downloads.rs`, `apps/desktop/src-tauri/src/commands/cf_blocked.rs`, `apps/desktop/src/features/test/blocked/**`, `apps/desktop/src/i18n/pt-BR/downloads-manuais.ts`.
- **Entregas:** lista com estados, abrir página, selecionar arquivo com verificação de hash, (P1) observar a pasta Downloads com `notify`, retomada automática do teste.
- **Critérios de aceite:** CA-T20-01, CA-T20-02.
- **Verificação:** testes de componente; integração com arquivo de hash conhecido.

### L-07 — Testar como o jogador recebe

- **Prioridade:** P1 · **Depende de:** L-04, E-01 · **Branch:** `feat/l-07-teste-limpo`
- **Objetivo:** teste em instância temporária limpa a partir do pack exportado (T13, P1).
- **Posse:** `crates/warden-instance/src/clean_instance.rs`, `apps/desktop/src/features/test/clean/**`.
- **Critérios de aceite:** a instância temporária não contém mundos nem configs da instância de trabalho; é apagada ao fim; logs da sessão ficam.
- **Verificação:** integração + E2E com jogo simulado.

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
- **Entregas:** linha de base antes do jogo, varredura depois, comparação semântica, classificação, diff por chave de `options.txt`, identificação de jars por hash, conflito de três vias, aplicar/descartar/decidir depois, aviso antes do próximo teste.
- **Critérios de aceite:** CA-T15-01 a CA-T15-05, CA-T12-04.
- **Verificação:** testes com instâncias sintéticas (antes/depois) + um roteiro com o jogo real (mudar distância de renderização e uma tecla).

### C-04 — Formulário de configs, `preserve` e classificação

- **Prioridade:** P1 · **Depende de:** C-02 · **Branch:** `feat/c-04-formulario-de-configs`
- **Objetivo:** T12, parte P1.
- **Posse:** `apps/desktop/src/features/configs/form/**`, `crates/warden-project/src/preserve.rs`, `apps/desktop/src-tauri/src/commands/configs_form.rs`, `apps/desktop/src/i18n/pt-BR/configs-formulario.ts`.
- **Entregas:** formulário por tipo de valor, alternância texto/formulário, "Não substituir se o jogador já tiver" (`preserve` no índice), botão "Copiar para defaultconfigs/".
- **Critérios de aceite:** CA-T12-02 pela interface; `preserve` mantido após `packwiz refresh` e respeitado pelo packwiz-installer real.
- **Verificação:** testes de componente; integração com packwiz/packwiz-installer.

---

## Fase 4 — Diagnóstico

### D-01 — Regras pré-teste

- **Prioridade:** P0 · **Depende de:** P1-01, P1-03, P1-04, P1-05, P1-06 · **Branch:** `feat/d-01-regras-pre-teste`
- **Objetivo:** motor de regras e todas as regras da ARCHITECTURE §9.2, nas passagens rápida e completa.
- **Posse:** `crates/warden-diagnostics/src/pretest/**`, `crates/warden-diagnostics/data/{exclusive-categories.toml,known-conflicts.toml,obsolete.toml}`, `crates/warden-diagnostics/tests/packs/**`.
- **Entregas:** regras sobre o modelo `Finding` (criado pela D-02), dados curados validados, correções sugeridas.
- **Critérios de aceite:** CA-T14-01 com os packs quebrados de teste; propriedade "todo achado tem evidência"; diagnóstico de 300 mods com cache quente < 3 s (medido).
- **Verificação:** `cargo nextest run -p warden-diagnostics`; cobertura ≥ 85%.

### D-02 — Análise pós-crash e redação

- **Prioridade:** P0 · **Depende de:** F0-05 · **Branch:** `feat/d-02-analise-de-crash`
- **Objetivo:** catálogo de padrões de log e redação de dados pessoais (ARCHITECTURE §9.3, §9.4).
- **Posse:** `crates/warden-diagnostics/src/{lib.rs,model.rs,postcrash/**,redact.rs}`, `crates/warden-diagnostics/data/log-patterns.toml`, `crates/warden-diagnostics/tests/corpus/**`, seção correspondente em `THIRD_PARTY.md`.
- **Entregas:** modelo `Finding` com evidência obrigatória (ARCHITECTURE §9.1), usado também pela D-01; coleta de artefatos de uma pasta de sessão, limpeza de `§`/ANSI, padrões (os 35 de R2 §6.3 + porte do codex-minecraft com atribuição), ordenação por probabilidade, redação.
- **Critérios de aceite:** CA-T14-02 com corpus de pelo menos 25 logs reais cobrindo Forge 1.7.10/1.12.2/moderno, NeoForge e Fabric; CA-T14-04.
- **Verificação:** `cargo nextest run -p warden-diagnostics`.

### D-03 — Seção Problemas e integração com o Testar

- **Prioridade:** P0 · **Depende de:** D-01, D-02, L-04 · **Branch:** `feat/d-03-tela-diagnostico`
- **Objetivo:** T14 sem IA: seção Problemas, "Por que travou" no resultado do teste e os diálogos de erro do Testar.
- **Posse:** `apps/desktop/src-tauri/src/commands/diagnostics.rs`, `apps/desktop/src/features/diagnostics/**`, `apps/desktop/src/routes/packs/$packId/problemas.tsx`, `apps/desktop/src/i18n/pt-BR/diagnostico.ts`, `apps/desktop/e2e/diagnostics.e2e.ts`, `apps/desktop/src/features/test/verification/**`, `apps/desktop/src-tauri/src/test_hooks/diagnostics.rs`.
- **Entregas:** passagens rápida e completa ligadas ao Testar pelo `TestHooks`, seção Problemas com lista por gravidade, contador no menu, "Verificar agora" e "Último travamento", evidências, correções pelos fluxos normais, relatório pós-crash como "Por que travou" no resultado do teste, marcas de problema na lista de Mods, "Copiar relatório" (redigido); (P1) "Ignorar este aviso neste pack" (`diagnostics_ignore`, gravado em `.warden/project.toml`).
- **Critérios de aceite:** CA-T13-06; correções "Adicionar dependência" e "Mudar lado" funcionam de ponta a ponta.
- **Verificação:** E2E com pack quebrado e jogo simulado que trava.

### D-04 — Seção ✦ Diagnóstico com IA (Gemini)

- **Prioridade:** P1 · **Depende de:** D-03, P1-13 · **Branch:** `feat/d-04-ia-gemini`
- **Objetivo:** seção ✦ Diagnóstico com IA e "✦ Pedir ajuda à IA" no resultado de um travamento, sempre com consentimento (T14, ARCHITECTURE §9.5, ADR-0026).
- **Posse:** `crates/warden-ai/**`, `apps/desktop/src-tauri/src/commands/ai.rs`, `apps/desktop/src/features/diagnostics/ai/**`, `apps/desktop/src/routes/packs/$packId/ia.tsx`, `apps/desktop/src/features/settings/ai/**`, `apps/desktop/src/i18n/pt-BR/ia.ts`.
- **Entregas:** seção com a escolha do que analisar (último travamento, outra sessão, log ou crash report do computador), campo "Contar algo à IA", respostas anteriores (`ai/<pack-id>/` nos dados locais, nunca no pack), diálogo de consentimento (sempre), cliente Gemini com esquema de resposta, tela "Resposta da IA" com aviso, escolha de modelo em Configurações, ligação do `secrets_test` para o Gemini; o ícone de brilhinho só em elementos de IA.
- **Critérios de aceite:** CA-T14-03, CA-T14-05, CA-T14-06; teste de rede com chave real (sob demanda) recebe resposta válida no esquema.
- **Verificação:** `cargo nextest run -p warden-ai`; E2E com servidor simulado.

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
- **Entregas:** `publish_plan` (árvore da versão via exportação da E-01, higiene, varredura de segredos, avisos de mods da CurseForge bloqueados com troca pelo Modrinth, de versão não testada e de problemas, notas da versão desde a última publicada); `publish_run` (linha de publicação `refs/warden/publish/main`, `CHANGELOG.md` de publicação, `.gitattributes` com `* -text`, tag, push com credencial em memória, GitHub Release, estado "Release pendente"); `github_setup` (criar público por padrão, privado como "só backup", ou vincular existente, com a explicação de visibilidade); área "Publicação para os jogadores" no Histórico; resultado com link do pack, passo a passo para jogadores (Prism/MultiMC com o packwiz-installer-bootstrap) e cópia das notas; divergência com substituição protegida; passo a passo do token (verificar e documentar o tipo de token e as permissões mínimas que funcionam para criar repositório, enviar e criar Release); ligação do `secrets_test` para o GitHub.
- **Critérios de aceite:** CA-T18-01 (teste sob demanda com conta descartável), CA-T18-02 a CA-T18-07.
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
- **Entregas:** `packwiz modrinth export` / `packwiz curseforge export` via sidecar sobre cópia limpa (staging); validação do arquivo gerado (zip abre, `modrinth.index.json`/`manifest.json` válidos, `overrides/` só com arquivos do índice); tela que explica o que se perde em cada formato; troca de fonte CurseForge → Modrinth por hash; bloqueio com explicação para mod da CurseForge sem distribuição por terceiros; confirmação para embutir arquivos de terceiros; exigência de `version` no `pack.toml`.
- **Critérios de aceite:** CA-T19-04, CA-T19-05.
- **Verificação:** integração com o packwiz real usando pack de teste com mods do Modrinth, da CurseForge e local; validação de esquema; importação manual do `.mrpack` no marco.

---

## Fase 6 — Acabamento

### A-01 — Revisão de experiência, textos e acessibilidade

- **Prioridade:** P0 · **Depende de:** fases 1–5 (P0) integradas · **Branch:** `fix/a-01-revisao-ux`
- **Objetivo:** passar por todas as telas com o glossário e a SPEC, corrigir estados faltantes, textos, foco e contraste.
- **Posse:** ajustes em `apps/desktop/src/i18n/pt-BR/**` e componentes apontados pela revisão (lista declarada antes de começar, aprovada pelo orquestrador).
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

- **Prioridade:** P0 · **Depende de:** P1-08, D-01, E-01 · **Branch:** `perf/a-04-packs-grandes`
- **Objetivo:** cumprir as metas da SPEC §8 com um pack de 300 mods e 500 configs.
- **Posse:** `crates/*/benches/**`, `apps/desktop/e2e/perf/**`, correções pontuais declaradas.
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

---

## 11. Marcos

| Marco | Contém | O que o dono consegue fazer |
|---|---|---|
| M0 — Fundação | Fase 0 | Abrir o Warden no Windows (vazio), ver a CI verde. |
| M1 — Montar packs | P1-01 a P1-13, V-01 | Criar/abrir packs, adicionar e atualizar mods do Modrinth, da CurseForge, por link e arquivo. |
| M2 — Testar | L-01 a L-06, D-01 a D-03 | Testar o pack no jogo, ver o console, entender travamentos. |
| M3 — Núcleo completo | C-01 a C-03, V-02, V-03, E-01 | Editar configs, trazer mudanças do jogo, salvar versões, publicar versões finais para os jogadores, exportar. Todas as funcionalidades P0 existem. |
| M4 — v1 | A-01 a A-04 (P0) e as P1 (P1-14, P1-15, C-04, D-04, L-07, A-05, A-06) | Versão para uso diário: revisada, com instalador e validada pelo dono. |

## 12. Backlog P2

Não planejado em tarefas ainda (SPEC §9): importar `.mrpack`; teste com servidor dedicado; teste com mundo antigo; Quick Play; Legacy Fabric; migração de versão do Minecraft; variantes do pack; formulário para YAML; editor de `servers.dat`; conflitos de teclas; kits de performance e modelos de pack; datapacks globais; trazer mudanças do GitHub; login no GitHub pelo navegador; atualização remota dos dados curados do diagnóstico; leitura de `@Mod` no bytecode para 1.7.10/1.12.2; mecanismos de "padrão na primeira execução" por mod (YOSBR, Default Options, Config Manager).
