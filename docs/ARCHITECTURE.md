# Warden — Arquitetura técnica

> Versão do documento: 1.0 (2026-10-01). Tarefa A1.
> Público: agentes de implementação e orquestrador. O dono pode ler a §1 para ter a visão geral.
> Fonte de verdade das decisões: `docs/decisions/` (ADRs citados como ADR-00NN). O que é produto está em `SPEC.md`; padrões obrigatórios em `QUALITY.md`; ordem de construção em `ROADMAP.md`.

## Sumário

1. [Visão geral](#1-visão-geral)
2. [Estrutura do monorepo](#2-estrutura-do-monorepo)
3. [Crates Rust e dependências](#3-crates-rust-e-dependências)
4. [Fronteira com a interface: comandos, eventos e canais](#4-fronteira-com-a-interface-comandos-eventos-e-canais)
5. [Modelo de erros](#5-modelo-de-erros)
6. [Pack e packwiz](#6-pack-e-packwiz)
7. [Launcher e Java](#7-launcher-e-java)
8. [Instância de teste: materialização e captura](#8-instância-de-teste-materialização-e-captura)
9. [Diagnóstico](#9-diagnóstico)
10. [Configs](#10-configs)
11. [Versionamento](#11-versionamento)
12. [Exportação](#12-exportação)
13. [Layout em disco](#13-layout-em-disco)
14. [Segredos](#14-segredos)
15. [Concorrência, travas e cancelamento](#15-concorrência-travas-e-cancelamento)
16. [Registros (logging)](#16-registros-logging)
17. [Rede e APIs externas](#17-rede-e-apis-externas)
18. [Frontend](#18-frontend)
19. [Bibliotecas escolhidas](#19-bibliotecas-escolhidas)
20. [Segurança do app Tauri](#20-segurança-do-app-tauri)

---

## 1. Visão geral

```
┌──────────────────────────── Warden (um processo Tauri) ────────────────────────────┐
│  Interface React (WebView2 no Windows)                                             │
│    rotas → features → hooks TanStack Query → bindings gerados (tauri-specta)       │
│                       ▲ eventos globais / canais por operação                      │
│ ──────────────────────┼──────────────── IPC Tauri ──────────────────────────────── │
│  warden-app (única crate que conhece o Tauri)                                      │
│    comandos finos → serviços de domínio (crates warden-*) → resultados tipados     │
│    estado: registro de operações, travas por pack, configurações, cofre            │
└─────────┬───────────────┬──────────────────┬────────────────┬─────────────────────┘
          │               │                  │                │
   sidecar packwiz   processo java      HTTPS (Mojang, Modrinth,   git (libgit2,
   (refresh/export)  (o jogo de teste)  CurseForge, Adoptium,      embutida)
                                         Fabric/Forge/NeoForge,
                                         GitHub, Gemini)
```

Princípios:
- **Domínio sem Tauri.** Toda regra de negócio vive em crates `warden-*` testáveis com `cargo test`, sem janela. `warden-app` só traduz entre IPC e domínio (ADR-0002).
- **Estado no disco.** O pack é a fonte da verdade; o Warden relê arquivos, nunca confia em saída de texto do packwiz (R3 §1.4).
- **Pack explícito.** Todo comando que atua num pack recebe o `PackId`; não existe "pack atual" no backend (R4 §4.1 item 4; ADR-0019).
- **Contratos gerados.** Tipos e comandos do IPC são gerados do Rust para TypeScript; não há tipos escritos à mão dos dois lados (ADR-0018).
- **Fatias verticais.** Uma funcionalidade só está pronta com domínio + comando + tela + teste ponta a ponta (QUALITY §5).

## 2. Estrutura do monorepo

```
Warden/
├── Cargo.toml                    # workspace: membros, [workspace.dependencies], [workspace.lints]
├── Cargo.lock
├── rust-toolchain.toml           # canal fixo (1.98.1), componentes rustfmt/clippy, alvo x86_64-pc-windows-msvc
├── rustfmt.toml  clippy.toml  deny.toml  .config/nextest.toml
├── package.json  pnpm-workspace.yaml  pnpm-lock.yaml  .npmrc  .nvmrc   # Node 24, pnpm 12
├── .editorconfig  .gitattributes  .gitignore
├── apps/
│   └── desktop/
│       ├── package.json  vite.config.ts  tsconfig.json  eslint.config.js  index.html
│       ├── wdio.conf.ts                       # E2E (WebdriverIO + tauri-driver)
│       ├── src/
│       │   ├── main.tsx
│       │   ├── app/                           # providers, roteador, layout, navigation.ts
│       │   ├── routes/                        # rotas por arquivo (TanStack Router)
│       │   ├── features/<feature>/            # components/, hooks/, lib/, *.test.ts(x)
│       │   ├── components/ui/                 # primitivos shadcn/ui
│       │   ├── components/common/             # EmptyState, ErrorPanel, ConfirmDialog, ProgressBar…
│       │   ├── lib/ipc/bindings.ts            # GERADO por tauri-specta (versionado; CI confere)
│       │   ├── lib/ipc/                       # wrappers de query/mutation, assinatura de eventos
│       │   ├── i18n/                          # index.ts, pt-BR/<área>.ts, errors/<domínio>.ts
│       │   └── styles/
│       ├── e2e/                               # specs ponta a ponta
│       └── src-tauri/                         # crate warden-app
│           ├── Cargo.toml  build.rs  tauri.conf.json
│           ├── capabilities/default.json
│           ├── binaries/                      # sidecar packwiz gerado (ignorado pelo git)
│           └── src/
│               ├── main.rs  lib.rs  state.rs  error.rs  events.rs
│               └── commands/{mod.rs, <domínio>.rs}
├── crates/                                    # domínio (§3)
├── third_party/
│   └── packwiz/                               # COMMIT, patches/, LICENSE, README.md (ADR-0007)
├── xtask/                                     # automação de dev: `cargo xtask <tarefa>`
├── docs/
└── .github/workflows/
```

Regras do monorepo:
- Uma crate por domínio; crates não dependem de `tauri` (verificado por `cargo xtask check-deps`, que falha se alguma crate fora de `warden-app` tiver `tauri` na árvore de dependências).
- Dependências e versões declaradas uma vez em `[workspace.dependencies]`; crates usam `dep.workspace = true`.
- Lints declarados em `[workspace.lints]` (QUALITY §2); toda crate de domínio e a `warden-app` têm `[lints] workspace = true`. O `xtask` declara lints próprios (permite `println!` e `anyhow`).
- `.gitattributes` da raiz: `* text=auto eol=lf`, exceto fixtures de packwiz (`crates/warden-packwiz/tests/fixtures/** -text`) e binários.
- Arquivos de registro compartilhados são **acréscimo-apenas** (uma linha por entrada), como `apps/desktop/src-tauri/src/commands/mod.rs` (registro de comandos), `apps/desktop/src/app/navigation.ts` e `apps/desktop/src/i18n/index.ts`. A lista completa está em ROADMAP §1. Conflitos neles são resolvidos pelo orquestrador na integração.

## 3. Crates Rust e dependências

| Crate | Responsabilidade | Depende de (internas) |
|---|---|---|
| `warden-core` | Tipos comuns: `PackId`, `OperationId`, `Progress`/`ProgressSink`, `CancellationToken` (reexport de `tokio-util`), `AppPaths`, escrita atômica, `resolve_inside` (anti path traversal), trait `DomainError`, versão do Minecraft como identificador opaco. | — |
| `warden-packwiz` | Formato packwiz: modelo de `pack.toml`, `index.toml`, `*.pw.toml`; leitura tolerante; escrita idêntica à do packwiz para arquivos novos e edição mínima (`toml_edit`) para existentes; `.packwizignore` (semântica gitignore + padrões embutidos do packwiz); hashes (sha1, sha256, sha512, md5, murmur2 da CurseForge); regras de higiene. | core |
| `warden-packwiz-cli` | Execução do sidecar: construtores de argv puros, execução com tempo-limite e cancelamento, sem janela, decodificação de linhas, remoção de ANSI, cópia de *staging*, verificação de conformidade. | core, packwiz |
| `warden-http` | Cliente HTTP único: User-Agent, limitador por host, novas tentativas com espera, download em streaming com hash e `.part`. | core |
| `warden-modrinth` | Cliente da API v2 do Modrinth e cache persistente de metadados (SQLite). | core, http |
| `warden-curseforge` | Cliente da API da CurseForge (chave injetada), detecção de distribuição bloqueada, impressão digital murmur2. Sem persistência (termos da CurseForge). | core, http, packwiz (murmur2) |
| `warden-catalog` | Catálogo de versões: manifesto da Mojang (ordem oficial), Fabric meta, Forge (maven-metadata + promotions, sufixos legados), NeoForge (API maven, `net.neoforged:forge` para 1.20.1), denylist de versões quebradas, cache. | core, http |
| `warden-jarmeta` | Leitura de metadados de jars sem extrair: `fabric.mod.json` (com `jars[]`), `quilt.mod.json` (só leitura, para identificar), `META-INF/mods.toml`, `META-INF/neoforge.mods.toml`, `META-INF/jarjar/metadata.json`, `mcmod.info` tolerante, `MANIFEST.MF` (coremods), versão de classe Java; dialetos de faixa (Fabric, Maven, `@Mod`), FlexVer. | core |
| `warden-project` | Serviço de domínio do pack: registro de packs, criar/abrir, inventário guiado pelo índice, adicionar/remover/atualizar, dependências, deduplicação, lado, opcionais, fixar, higiene, transação de escrita (§6.5). | core, packwiz, packwiz-cli, modrinth, curseforge, catalog, jarmeta, http |
| `warden-configs` | Formatos de config com preservação: TOML, JSON/JSONC, JSON5, `.properties`, `.cfg` do Forge antigo, `options.txt`; modelo em árvore; edições mínimas; comparação semântica. | core |
| `warden-java` | Runtimes Java: descoberta, download (Adoptium; fallback Mojang), validação, política de seleção. | core, http |
| `warden-launcher` | Interface `LauncherEngine`, adaptador do motor escolhido no spike S1, perfil offline, supervisão do processo, leitura de logs do jogo (log4j XML e texto). | core, java, catalog |
| `warden-instance` | Instância de teste: materialização pack → instância, manifesto de estado, linha de base, captura instância → pack, comparação de três vias. | core, packwiz, configs, modrinth, curseforge, http |
| `warden-diagnostics` | Regras pré-teste, análise pós-crash, dados curados, redação de dados pessoais, montagem do relatório. | core, packwiz, jarmeta, catalog, modrinth, curseforge |
| `warden-ai` | Cliente Gemini, montagem do conteúdo a enviar (a partir do relatório já redigido), esquema de resposta. | core, http, diagnostics |
| `warden-versioning` | Git embutido (`git2`): repositório, ponto inicial, salvar versão, tags, pontos de segurança, restauração transacional, diffs, changelog, GitHub (API REST + push). Nomes legíveis de versões vêm de um trait `VersionNameResolver` implementado pela `warden-app` (com Modrinth e CurseForge). | core, packwiz, http |
| `warden-export` | Pré-visualização e exportação nativa (pasta/zip), conformidade; P2: `.mrpack`/CurseForge via sidecar. | core, packwiz, packwiz-cli, project |
| `warden-secrets` | Cofre do sistema (`keyring`), `SecretString`, fallback de desenvolvimento por variável de ambiente só em build de debug. | core |
| `warden-app` (`apps/desktop/src-tauri`) | Comandos, eventos, estado do app, configurações (`settings.json`), registro de operações, ligação de tudo. | todas |
| `xtask` | `setup`, `dev`, `check` (e `check --fast`), `check-deps`, `check-docs`, `bindings`, `coverage`, `test-network` (F0-01); `build-packwiz` (F0-03); `win-dev`, `win-install` (F0-04); `fixtures-packwiz` (P1-01); `installer` (L-03: bootstrap do packwiz-installer e JRE de testes); `notices` (A-02). | — (ferramenta) |

Grafo sem ciclos; `warden-core` não depende de ninguém. Nenhuma crate de domínio chama outra "para cima" (ex.: `warden-instance` não depende de `warden-project`; quem orquestra é `warden-app`).

## 4. Fronteira com a interface: comandos, eventos e canais

### 4.1 Comandos

- Declarados em `apps/desktop/src-tauri/src/commands/<domínio>.rs` com `#[tauri::command]` + `#[specta::specta]`; registrados em `commands/mod.rs`; tipos e funções TypeScript gerados em `apps/desktop/src/lib/ipc/bindings.ts` por `tauri-specta` (`cargo xtask bindings`; a CI falha se o arquivo estiver desatualizado).
- Nome: `<domínio>_<verbo>` em snake_case no Rust (`pack_create`); o TypeScript recebe `commands.packCreate(...)`.
- Todo comando retorna `Result<T, AppError>`. Nada lança exceção pelo IPC.
- Todo comando que atua num pack recebe `pack_id: PackId` como primeiro argumento.
- Caminhos vindos da interface são **relativos** ao pack ou à instância e passam por `resolve_inside`. Caminhos absolutos escolhidos pelo usuário (abrir/criar/realocar pack, arquivo local, arquivo baixado manualmente, destino de exportação) **não vêm da interface**: o próprio comando abre o diálogo nativo pela API Rust do plugin `dialog` (ou recebe o evento de arrastar e soltar da janela, tratado no Rust) e valida o resultado (existência, tipo, sem link simbólico).
- Comandos são finos: validam entrada, chamam o serviço de domínio, convertem erro. Nenhuma regra de negócio no comando.
- Comandos longos recebem um `Channel<OperationEvent>` e retornam o resultado final; o progresso vai pelo canal (§4.3).

Contrato inicial (nomes estáveis; parâmetros detalhados pelas tarefas donas de cada domínio):

| Domínio | Comandos |
|---|---|
| app | `app_info`, `settings_get`, `settings_update`, `operations_list`, `operation_cancel`, `storage_usage` (P1), `storage_clear_cache` (P1) |
| secrets | `secrets_status`, `secrets_set`, `secrets_test`, `secrets_remove` (não existe "ler segredo") |
| catalog | `catalog_minecraft_versions`, `catalog_loader_versions` |
| packs | `packs_list`, `pack_create`, `pack_import`, `pack_relocate`, `pack_reveal_folder`, `pack_forget`, `pack_trash` (P1), `pack_get`, `pack_update_meta`, `pack_hygiene_scan`, `pack_hygiene_fix` |
| inventory | `inventory_list`, `item_details`, `items_set_side`, `items_remove_plan`, `items_remove`, `item_set_optional` (P1), `item_set_pin` (P1), `item_change_version_plan` (P1), `pack_change_loader_version_plan` (P1), `instance_set_optional_choices` (P1) |
| search/add | `search_modrinth`, `search_curseforge`, `project_details`, `project_versions`, `add_plan`, `add_apply`, `add_from_link_plan`, `add_local_files_plan` |
| updates | `updates_check`, `updates_plan`, `updates_apply` |
| configs | `config_tree`, `config_read`, `config_write`, `config_structured_read` (P1), `config_structured_apply` (P1), `config_set_preserve` (P1) |
| test | `test_start`, `test_stop`, `test_session_get`, `test_sessions_list`, `instance_reveal_folder`, `instance_recreate`, `cf_blocked_list`, `cf_blocked_provide_file` |
| capture | `capture_changes`, `capture_apply`, `capture_discard` |
| diagnostics | `diagnostics_run`, `diagnostics_report_get`, `diagnostics_ignore` (P1) |
| ai | `ai_preview` (P1), `ai_analyze` (P1) |
| versioning | `versions_status`, `version_suggest`, `version_save`, `history_list`, `history_diff`, `history_restore`, `safety_points_list` (P1), `safety_point_restore` (P1) |
| github | `github_status`, `github_setup`, `github_push` |
| export | `export_preview`, `export_run` (com versão salva opcional, P1) |
| java | `java_runtimes_list`, `java_runtime_remove` |

### 4.2 Eventos globais

Emitidos com `app.emit` para quem estiver ouvindo; tipados com `tauri-specta` (`#[derive(Event)]`):

| Evento | Carga | Uso |
|---|---|---|
| `pack-changed` | `{ packId, areas: ("inventory" \| "configs" \| "meta" \| "history")[] }` | Invalida queries do TanStack Query. Emitido após qualquer escrita do Warden e após mudança externa detectada (P1). |
| `operation-updated` | `OperationSnapshot` | Painel Tarefas (T22). |
| `game-state` | `{ packId, state: "preparing" \| "running" \| "exited", sessionId }` | Cabeçalho do pack, bloqueio de "um jogo por vez". |

### 4.3 Canais por operação

`tauri::ipc::Channel<OperationEvent>` passado ao comando:

```rust
#[derive(Serialize, specta::Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum OperationEvent {
    Started { operation_id: OperationId, kind: OperationKind },
    Stage { stage: StageId, label_key: String },           // ex.: "test.stage.prepareMinecraft"
    Progress { current: u64, total: Option<u64>, unit: ProgressUnit },
    Log { lines: Vec<LogLine> },                             // console do jogo, saída do packwiz (lotes)
    Warning { error: AppError },
    Finished,                                                // o resultado vem no retorno do comando
}
```

O console do jogo usa o mesmo canal (`Log`). O backend agrega linhas em lotes de até 50 ms para não inundar o IPC.

## 5. Modelo de erros

- Cada crate define seu `enum Error` com `thiserror` e implementa `warden_core::DomainError`, que expõe `code()`, `params()`, `detail()` e `retryable()`.
- Cada crate define também o seu `enum <Domínio>ErrorCode` (`#[serde(rename_all = "SCREAMING_SNAKE_CASE")]`, `specta::Type`). O código é estável: renomear um código é mudança de contrato.
- `warden-app` compõe tudo em `AppError`:

```rust
#[derive(Debug, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,                    // { domain: "packwiz", code: "REFRESH_FAILED" }
    pub params: BTreeMap<String, String>,   // valores para a frase traduzida (nome do mod, versão…)
    pub detail: Option<String>,             // evidência técnica: saída do packwiz, status HTTP, caminho
    pub retryable: bool,
    pub operation_id: Option<OperationId>,
}

#[derive(Debug, Serialize, specta::Type)]
#[serde(tag = "domain", content = "code", rename_all = "camelCase")]
pub enum ErrorCode {
    App(AppErrorCode), Core(CoreErrorCode), Packwiz(PackwizErrorCode), PackwizCli(PackwizCliErrorCode),
    Project(ProjectErrorCode), Jarmeta(JarmetaErrorCode),
    Modrinth(ModrinthErrorCode), Curseforge(CurseforgeErrorCode), Catalog(CatalogErrorCode),
    Configs(ConfigsErrorCode), Java(JavaErrorCode), Launcher(LauncherErrorCode),
    Instance(InstanceErrorCode), Diagnostics(DiagnosticsErrorCode), Ai(AiErrorCode),
    Versioning(VersioningErrorCode), Export(ExportErrorCode), Secrets(SecretsErrorCode),
    Http(HttpErrorCode),
}
```

- `CoreErrorCode` (de `warden-core`) reúne o que é comum a todos: `CANCELLED`, `TIMEOUT`, `IO`, `PATH_OUTSIDE_ROOT`, `NETWORK_UNAVAILABLE`. `PACK_CHANGED_EXTERNALLY` pertence a `Project`.
- A F0-05 cria todos os domínios acima já com um código `INTERNAL`; tarefas posteriores só acrescentam códigos no enum da própria crate e as frases no arquivo `apps/desktop/src/i18n/errors/<domínio>.ts`, tipado como `Record<<Domínio>ErrorCode, string>` (o TypeScript falha se faltar tradução).
- Frontend: `ErrorPanel` mostra a frase traduzida, a sugestão de ação, e "Detalhes técnicos" com `domain.code`, `detail` e `operationId`, com botão Copiar.
- `detail` nunca contém segredo (§14) e é truncado em 16 KB.
- `INTERNAL` existe para bugs: todo `INTERNAL` visto em teste ou uso vira tarefa para criar um código específico.

## 6. Pack e packwiz

### 6.1 Divisão de responsabilidades (integração híbrida, ADR-0006)

| Operação | Quem faz |
|---|---|
| Ler `pack.toml`, `index.toml`, `*.pw.toml` | Warden (`warden-packwiz`), sempre do disco. |
| Criar `pack.toml` | Warden (catálogo próprio de versões; contorna o bug do `packwiz init` com Forge antigo, R3 §1.5.1), seguido de `packwiz refresh`. |
| Adicionar do Modrinth / da CurseForge pela busca | Warden escreve o `.pw.toml` com os dados da API (versão escolhida pelo usuário, arquivo `primary`, hash `sha512` no Modrinth; `sha1` e `mode = "metadata:curseforge"` na CurseForge), depois `refresh`. |
| Adicionar da CurseForge por link | Link de **arquivo**: `packwiz curseforge add <url>`. Link de **projeto**: o Warden mostra a pré-visualização com seletor de versão (padrão: a mais nova compatível do canal configurado) e chama `packwiz curseforge add --addon-id <projeto> --file-id <arquivo>`, para o packwiz nunca escolher a versão sozinho. Em ambos os casos o comando roda numa **cópia de staging** do pack, com stdin respondendo `n` à pergunta de dependências; o Warden lê o `.pw.toml` gerado e o importa pelo caminho normal de escrita (colisão de nomes, lado, deduplicação, dependências pela API). Nunca roda direto na pasta do pack (o packwiz sobrescreve arquivos em silêncio, R3 §1.7). |
| Link direto | Warden baixa, calcula `sha256` e escreve o `.pw.toml` como o `packwiz url add` escreveria. |
| Arquivo local | Warden copia o arquivo para a pasta do tipo e roda `refresh`. |
| Remover | Warden apaga o `.pw.toml`/arquivo e roda `refresh` (não usa `packwiz remove`: nome ambíguo e não determinístico, R4 §2.3). |
| Lado, opcional, fixar, atualizar | Warden edita/escreve o `.pw.toml` e roda `refresh`. |
| `preserve` | Warden edita a entrada no `index.toml` e roda `refresh` (o packwiz mantém o campo e recalcula o hash do índice no `pack.toml`). |
| `[options]` (`acceptable-game-versions`, `datapack-folder`…) | Warden escreve na tabela `[options]` do `pack.toml` (único lugar que o packwiz lê, R4 §2.3). |
| Validação | `packwiz refresh` numa cópia de staging; o pack é válido se o resultado for idêntico ao original (zero diferença). |
| Exportação nativa | Staging + `packwiz refresh --build` + cópia só dos arquivos indexados (§12). |
| `.mrpack` e zip da CurseForge (P2) | `packwiz modrinth export` / `packwiz curseforge export` em staging, com validação do arquivo gerado. |
| **Nunca usados** | `init`, `modrinth add`, `curseforge add` por busca ou com `-y`, `update`, `migrate`, `serve`, `remove`, `pin`/`unpin`, `curseforge detect`, `curseforge import`. |

Equivalência obrigatória: para cada tipo de `.pw.toml` que o Warden escreve, existe teste dourado comparando com o arquivo que o packwiz real escreve para o mesmo item (fixtures em `crates/warden-packwiz/tests/fixtures/packwiz-output/`, geradas por `cargo xtask fixtures-packwiz` e versionadas com `-text`).

### 6.2 Regras do formato

- **Escrita de arquivo novo:** mesma ordem de chaves e mesmo estilo do packwiz (sem indentação, `[update]` seguido de `[update.<fonte>]`, aspas duplas), para que um `refresh` posterior não gere diferença.
- **Edição de arquivo existente:** `toml_edit`, alterando só a chave tocada.
- **Sem comentários** nos arquivos que o packwiz reescreve (`pack.toml`, `index.toml`): o `refresh` os apaga (issue #167). Metadados do Warden vão para `.warden/`.
- **Nomes únicos:** o nome base do `.pw.toml` é único no pack inteiro (o `FindMod` do packwiz é não determinístico com nomes repetidos). Padrão `<slug>.pw.toml`; se existir outro projeto com o mesmo nome base, `<slug>-<fonte>.pw.toml`, depois `-2`, `-3`.
- **Sem subpastas em `filename`** e sem `alias` (issues #52 e #228). Sem links simbólicos na pasta do pack (#191).
- **Pastas:** mods em `mods/`, resource packs em `resourcepacks/`, shaders em `shaderpacks/`. O inventário classifica pelo diretório do arquivo e respeita a opção `meta-folder`/`mods-folder` se existir.
- **Lado a partir do Modrinth** (corrige o caso vazio do packwiz, R3 §1.7):

| `environment` (Modrinth) | `side` gravado | Observação na interface |
|---|---|---|
| `client_only`, `singleplayer_only` | `client` | — |
| `dedicated_server_only` | `server` | — |
| `client_and_server`, `server_only`, `client_or_server_prefers_both`, `client_only_server_optional`, `server_only_client_optional` | `both` | — |
| `client_or_server` | `both` | "Funciona em qualquer um dos lados" |
| `unknown` ou ausente | `both` | "Lado desconhecido — confira" |

- **Lado a partir do jar** (local, link, CurseForge): Fabric `environment: "client"` → sugere `client`; Forge `clientSideOnly = true` ou `displayTest = "IGNORE_ALL_VERSION"` → sugere `client`. Sugestão aparece no diálogo de adição; o padrão é `both`.
- **Hashes:** índice em `sha256`; download Modrinth `sha512`; CurseForge `sha1` (ou `md5`/`murmur2` quando a API não der `sha1`); link direto `sha256`.
- **Loaders compatíveis:** pack NeoForge aceita mods `neoforge`; mods `forge` são aceitos em NeoForge 1.20.1. Em NeoForge ≥ 1.20.2, um mod marcado só como `forge` na API gera aviso na passagem rápida e **erro** na passagem completa se o jar não tiver `META-INF/neoforge.mods.toml`. Pack Fabric aceita `fabric`. Packs com mais de um loader não são abertos.

### 6.3 Sidecar do packwiz (ADR-0007)

- Código: `packwiz/packwiz` (MIT) no commit registrado em `third_party/packwiz/COMMIT` (inicialmente o `ef87d96` analisado em R3; a F0-03 grava o hash completo).
- Patches em `third_party/packwiz/patches/`, aplicados em ordem:
  - `0001-chave-curseforge-em-tempo-de-execucao.patch`: em `curseforge/request.go`, a chave passa a vir da variável de ambiente `WARDEN_CURSEFORGE_API_KEY`; sem ela, os comandos da CurseForge falham com mensagem clara. **Remove o uso da chave embutida do packwiz** (pedido explícito do autor do packwiz a projetos derivados; R3 §1.12).
- Build (`cargo xtask build-packwiz`): clona o commit, aplica os patches, compila com `CGO_ENABLED=0 go build -trimpath -ldflags "-s -w"` para `x86_64-unknown-linux-gnu` e `x86_64-pc-windows-msvc` (via `GOOS=windows GOARCH=amd64`), grava em `apps/desktop/src-tauri/binaries/packwiz-<triplo>[.exe]` e registra o commit em `binaries/packwiz.commit`. Requer Go ≥ 1.24.
- Empacotamento: `bundle.externalBin: ["binaries/packwiz"]` no `tauri.conf.json`. O Tauri copia o binário para junto do executável sem o sufixo do triplo.
- Execução: pelo Rust com `tokio::process::Command` (sem o plugin `shell`, que não é exposto à interface). Localização: variável `WARDEN_PACKWIZ_BIN` (testes) ou a pasta do executável.
- Toda chamada:
  - `--pack-file <absoluto>/pack.toml`, `--cache <cache>/packwiz`, `--config <dados>/packwiz/packwiz.toml` (arquivo vazio criado pelo Warden, isolando de configurações globais do usuário);
  - diretório de trabalho = pasta do pack (ou da cópia de staging);
  - ambiente limpo (só `PATH`, `SYSTEMROOT`, `TEMP`, `TMP`, `HOME`/`USERPROFILE`, `LOCALAPPDATA`, `APPDATA`); `WARDEN_CURSEFORGE_API_KEY` só nos comandos que precisam;
  - stdin fechado (ou com as respostas definidas, no `curseforge add`);
  - Windows: `CREATE_NO_WINDOW`;
  - tempo-limite: `refresh` 120 s; `curseforge add` 120 s; exportações 600 s; cancelável;
  - saída lida em bytes, cortada em linhas completas, decodificada como UTF-8 com substituição, sem sequências ANSI e sem linhas de barra de progresso; vai para o log em nível `debug` e as últimas 200 linhas viram `detail` em caso de erro.
- Sucesso = código 0 **e** pós-condição verificada relendo os arquivos (o packwiz escreve erros no stdout e às vezes sai com 0 em falhas, R3 §1.4).
- Versão do packwiz exibida em Sobre: commit lido de `binaries/packwiz.commit`, embutido no build.

### 6.4 Higiene e arquivos de controle

Arquivos criados no pack pelo Warden:

`.gitattributes`
```gitattributes
* -text
```

`.packwizignore` (o packwiz já ignora `.git/**`, `.gitattributes`, `.gitignore`, `.DS_Store`, `/*.zip`, `*.mrpack`, `packwiz.exe`, `packwiz`). Pastas de dados de execução são **ancoradas na raiz** (`/logs/`, não `logs/`), porque na semântica gitignore um padrão sem `/` inicial casa em qualquer profundidade e excluiria conteúdo legítimo como `kubejs/assets/` ou `config/<mod>/debug/`:
```gitignore
# Gerado pelo Warden. Nada abaixo deve chegar a quem joga o pack.
# Bloco obrigatório (o Warden sempre garante estas linhas):
/.warden/
/CHANGELOG.md
/README.md
*.warden-tmp
# Dados de execução do jogo e do launcher:
/logs/
/crash-reports/
/saves/
/screenshots/
/debug/
/stats/
/natives/
/libraries/
/versions/
/assets/
/resources/
/.mixin.out/
/.fabric/
/.quilt/
/.cache/
/mods/.connector/
/modernfix/
/journeymap/data/
/XaeroWaypoints/
/XaeroWorldMap/
/xaero/
/kubejs/probe/
/kubejs/exported/
/usercache.json
/usernamecache.json
/launcher_profiles*.json
/servers.dat_old
/command_history.txt
/packwiz.json
/packwiz-installer*.jar
/.packwiz.toml
# Em qualquer profundidade:
jsconfig.json
*.log
*.log.gz
hs_err_pid*.log
replay_pid*.log
*.heapdump
Thumbs.db
desktop.ini
*.tmp
*.bak
*.old
*.disabled
```

Por que lista de bloqueio e não lista de permissão (`/*` + `!/config/`…, como Craftoria, R1 §1.A): packs usam pastas de mods variadas (`kubejs/`, `openloader/`, `paxi/`, `global_packs/`, `schematics/`…); numa lista de permissão, uma pasta nova seria excluída em silêncio. Aqui ela entra no índice e a verificação de higiene (abaixo) aponta qualquer item desconhecido na raiz para o usuário decidir. `local/` não é ignorada por padrão (alguns packs, como o ATM10, a usam como conteúdo); a higiene a sinaliza para revisão.

`.gitignore`: os mesmos padrões, **exceto** `/.warden/`, `/CHANGELOG.md` e `/README.md` (que são versionados).

**Bloco obrigatório:** antes de qualquer `packwiz refresh`, a `PackTransaction` confere que o `.packwizignore` contém as quatro linhas do bloco obrigatório e, se faltar alguma, acrescenta (avisando na interface). Isso vale também para packs importados cujo usuário recusou os arquivos de controle padrão: o resto do `.packwizignore` dele é respeitado, mas `.warden/` e `CHANGELOG.md` nunca entram no índice.

Verificação de higiene (`pack_hygiene_scan`): procura entradas do índice e arquivos na pasta que casam com os padrões acima ou com as regras extras abaixo, e devolve itens com motivo:
- itens na raiz que não sejam `pack.toml`, `index.toml`, `options.txt`, `optionsof.txt`, `optionsshaders.txt`, `servers.dat`, arquivos de controle ou pastas de conteúdo conhecidas (`mods/`, `resourcepacks/`, `shaderpacks/`, `config/`, `defaultconfigs/`, `kubejs/`, `scripts/`, `datapacks/`, `openloader/`, `global_packs/`, `paxi/`) — inclusive `local/`;
- arquivos acima de 20 MB que não sejam jars de mods;
- pastas desconhecidas (fora da lista acima) com mais de 1.000 arquivos (sinal de cache).

A correção (`pack_hygiene_fix`) cria ponto de segurança, apaga os itens escolhidos, acrescenta padrões faltantes ao `.packwizignore`/`.gitignore` e roda `refresh`.

### 6.5 Transação de escrita no pack

Toda mutação do pack segue o mesmo caminho (`warden-project::PackTransaction`):

1. Adquire a trava de escrita do pack (§15) e garante o bloco obrigatório do `.packwizignore` (§6.4).
2. Relê do disco os arquivos que vai alterar e confere que não mudaram desde que a operação foi planejada (hash); se mudaram, aborta com `PACK_CHANGED_EXTERNALLY`.
3. Guarda em memória (ou em `cache/tmp/<operação>/`, nunca no pack) o conteúdo anterior de cada arquivo tocado.
4. Escreve cada arquivo de forma atômica: `<nome>.warden-tmp` na mesma pasta → `fsync` → renomeia sobre o destino.
5. Roda `packwiz refresh` e relê `index.toml`/`pack.toml`, conferindo que os arquivos esperados estão indexados.
6. Em qualquer falha: restaura o conteúdo anterior, roda `refresh` de novo e devolve o erro original.
7. Libera a trava e emite `pack-changed`.

Sobras `*.warden-tmp` (encerramento à força) são apagadas ao abrir o pack.

## 7. Launcher e Java

### 7.1 Interface do motor (ADR-0010)

A escolha do motor (biblioteca do Modrinth App `app-lib`/theseus ou `portablemc`) sai do spike S1. O resto do Warden só conhece esta interface, definida em `warden-launcher` (forma final ajustada pela L-02 com base no resultado do S1):

```rust
pub struct GameSpec {
    pub minecraft: MinecraftVersionId,          // ex.: "1.20.1", "26.3"
    pub loader: LoaderSpec,                      // Vanilla | Fabric { version } | Forge { version } | NeoForge { version }
}

pub struct LaunchOptions {
    pub game_dir: PathBuf,                       // instances/<id>/minecraft
    pub java: JavaRuntime,                       // escolhido por warden-java
    pub memory_mb: u32,
    pub extra_jvm_args: Vec<String>,             // já validados
    pub player: OfflineProfile,
}

#[async_trait::async_trait]
pub trait LauncherEngine: Send + Sync {
    /// Instala (idempotente) versão, bibliotecas, assets e loader no armazenamento compartilhado.
    async fn install(&self, spec: &GameSpec, java: &JavaRuntime,
                     progress: &dyn ProgressSink, cancel: &CancellationToken)
                     -> Result<InstalledGame, LauncherError>;
    /// Monta a linha de comando final (programa, argumentos, ambiente, diretório).
    fn command(&self, game: &InstalledGame, opts: &LaunchOptions) -> Result<LaunchCommand, LauncherError>;
}
```

- **O Warden inicia e supervisiona o processo** (não o motor): isso garante captura de logs, codificação, encerramento da árvore e classificação de saída iguais para qualquer motor. Se o S1 mostrar que um motor só sabe iniciar o processo sozinho, o adaptador entrega os mesmos `GameEvent` (§7.4) e a interface ganha um método `spawn` — decisão registrada em ADR pela L-02.
- O armazenamento compartilhado (`shared/`) pertence ao adaptador do motor (§13). Natives ficam fora do `gameDir`.
- Golden tests: para cada combinação da matriz (ROADMAP L-05), a `LaunchCommand` gerada é comparada a um arquivo dourado revisado (caminhos normalizados).
- Requisitos que o motor escolhido precisa cumprir (critérios do S1): Forge legado (1.7.10, 1.12.2) com jar `universal`, Forge/NeoForge com processors, Fabric, regras de bibliotecas por SO, natives das três gerações, deduplicação de bibliotecas, `@argfile` quando a linha passar de ~30.000 caracteres em Java ≥ 9, progresso e cancelamento sem deixar arquivos incompletos com nome final, erros tipados, funcionamento no Windows.

### 7.2 Perfil offline

- Nome do jogador das Configurações (`^[A-Za-z0-9_]{3,16}$`).
- UUID: MD5 de `"OfflinePlayer:" + nome`, com versão 3 e variante IETF ajustadas (sem namespace; **não** usar `Uuid::new_v3`), testado contra valores gerados pelo Java.
- `accessToken = "0"`, `user_properties = {}`, `xuid = "0"`, `clientId` = UUID fixo do Warden, `userType = "legacy"` até 1.21.8; a partir de 1.21.9 passa `--offlineDeveloperMode`.
- Nenhum contorno de autenticação (sem servidor falso, sem redirecionar hosts). Em 1.16.4/1.16.5 o botão de multiplayer fica cinza; isso é documentado, não contornado (R2 §4.5).

### 7.3 Java (ADR-0012)

`warden-java`:
- **Fonte padrão:** Adoptium Temurin JRE (8, 17, 21, 25), validado por SHA-256 da API; **alternativa:** runtime oficial da Mojang pelo `javaVersion.component`.
- **Política de seleção** (primeira regra que se aplica): (a) Java escolhido pelo usuário nos Ajustes do pack; (b) Forge ≤ 1.12.2 → 8; (c) Forge 1.16.5 com versão < 36.2.26 → Java 8 ≤ 8u312 (Temurin 8 da série compatível, ou aviso se não houver); (d) `javaVersion.majorVersion` do JSON da versão (16 → 17); (e) sem `javaVersion` → 8. Exigências de mods (`[features] javaVersion`, `depends.java`) entram como verificação do diagnóstico (E-JAVA), não mudam a escolha sozinhas.
- Instalação atômica em `shared/runtimes/<fornecedor>-<versão>-<arch>/` (baixar → verificar → extrair em temporário → renomear).
- Validação de qualquer Java executando `java -XshowSettings:properties -version` uma vez; resultado em cache por caminho + data de modificação.
- Só JVM 64 bits.

### 7.4 Processo do jogo

- Programa: `javaw.exe` no Windows, `java` no Linux. Diretório de trabalho = `gameDir`.
- `stdout` e `stderr` por pipes, lidos em tarefas separadas; linhas completas; decodificação UTF-8 com recuo para Windows-1252 (`encoding_rs`).
- Argumentos de codificação: `-Dstdout.encoding=UTF-8 -Dstderr.encoding=UTF-8` em Java ≥ 19; `-Dfile.encoding=UTF-8` só em 17/18 (em Java 8 fica desligado até ser validado com packs reais, R2 §9 item 7).
- `-XX:ErrorFile=<gameDir>/hs_err_pid%p.log`.
- Logs: parser incremental que aceita eventos XML do log4j e texto puro na mesma stream, produzindo `LogLine { ts, level, thread, logger, text, source }`.
- Configuração de log4j: segue o `logging` efetivo do perfil (XML da Mojang para vanilla/Fabric; nada para NeoForge, que usa o próprio `debug.log`; nada para os perfis do Forge que anulam `logging` com configuração própria **verificada** como segura, como 1.12.2, 1.16.5 e 1.20.1 em R2 §3.1). Nunca lançar versão ≤ 1.18 sem configuração log4j segura (R2 §1.7): onde a segurança da configuração do loader não foi verificada (ex.: Forge 1.7.10, com log4j 2.0-beta9, que não tem `nolookups`), usa-se o XML corrigido da Mojang da faixa (`client-1.7.xml`/`client-1.12.xml`). A L-02 verifica cada faixa da matriz com um teste que confirma que `${jndi:…}` não é expandido.
- Encerramento: Windows usa *Job Object* com `KILL_ON_JOB_CLOSE` (o jogo morre se o Warden fechar); "Parar jogo" encerra o job. Linux usa grupo de processos e `SIGTERM` → `SIGKILL` após 3 s. O código `unsafe` necessário fica isolado em `warden-launcher/src/process/windows.rs`, com comentário `SAFETY` em cada bloco.
- Classificação da saída: "Encerrado por você" se foi pedido; "Travou" se código ≠ 0, se apareceu crash report novo ou `hs_err_pid` novo; senão "Fechado normalmente".
- Sessão gravada em `instances/<id>/state/sessions/<data-hora>/` (`output.log` com tudo o que saiu dos pipes, `session.json` com resultado, duração, versão do pack e caminhos dos artefatos de crash).

### 7.5 Forge

O instalador do Forge pede para não automatizar o download. O Warden automatiza (sem isso o requisito é inviável), baixa sempre do Maven oficial, não espelha instaladores e mostra em Sobre o crédito ao Forge com o link de apoio do próprio instalador (R2 §3.4).

## 8. Instância de teste: materialização e captura

### 8.1 Pasta

`instances/<pack-id>/minecraft/` é o `gameDir`. `instances/<pack-id>/state/` guarda `manifest.json`, `baseline/`, `sessions/` e `optional-choices.json`. Nada de `state/` fica dentro do `gameDir`.

### 8.2 Materialização (pack → instância; ADR-0011)

Mesma semântica do packwiz-installer, implementada em Rust (`warden-instance`):

1. Ler `pack.toml` e `index.toml` do pack.
2. Para cada entrada do índice:
   - metafile: ler o `.pw.toml`; pular `side = "server"`; pular opcionais desligados na instância; destino = pasta do metafile + `filename`; origem = download (`url`; ou, em `metadata:curseforge`, URL obtida da API na hora, nunca gravada);
   - arquivo comum: origem = o próprio arquivo do pack.
3. Se a entrada tem `preserve = true` e o destino já existe, não tocar.
4. Se o hash do destino já confere com o esperado, não tocar.
5. Downloads vão para `cache/downloads/` (endereçado por `sha256`, com tabela de equivalência para os outros hashes), verificados contra o hash declarado; depois copiados para a instância com gravação atômica.
6. Remover da instância os arquivos que estavam no `manifest.json` anterior e saíram do pack (só os que o Warden instalou).
7. Antes de sobrescrever um arquivo gerenciado que foi alterado na instância e ainda não revisado, a operação para e pede revisão (T15).
8. Gravar o novo `manifest.json` (`caminho → { sha256, origem, hashDoÍndice }`).
9. Mods da CurseForge com download bloqueado: a operação devolve a lista para T20 e espera os arquivos.

**Teste de conformidade:** instalar o mesmo pack com o packwiz-installer real (lado cliente, a partir do caminho local) e com o Warden, em pastas vazias, e exigir árvores idênticas (ignorando `packwiz.json` e `state/`).

### 8.3 Linha de base e captura (instância → pack)

1. Logo antes de abrir o jogo: varrer a instância (fora da lista de ignorados §8.4) e gravar `{ caminho, tamanho, mtime, sha256 }`; guardar conteúdo dos arquivos de texto ≤ 1 MB em `config/`, `defaultconfigs/`, `kubejs/`, `options.txt` e `saves/*/serverconfig/` em `state/baseline/` (endereçado por hash).
2. Depois que o jogo fecha: varrer de novo; filtrar por tamanho+mtime e confirmar por `sha256`; classificar em novo/alterado/removido.
3. Para arquivos de formato conhecido, comparar a árvore de valores (`warden-configs`): se só mudou formatação, ordem ou comentários gerados, o arquivo é "reformatado" — não aparece e a linha de base é atualizada.
4. Classificar a origem: alterado e presente no `manifest.json` → "veio do pack"; novo → "gerado"; `saves/*/serverconfig/*` → "config de mundo"; `mods/*.jar` → identificar por hash (Modrinth `sha1`/`sha512`, CurseForge murmur2); etc. (SPEC T15).
5. `options.txt`: diff por chave; chaves voláteis nunca aparecem: `version`, `lastServer`, `fullscreen`, `fullscreenResolution`, `overrideWidth`, `overrideHeight`, `tutorialStep`, `joinedFirstServer`, `onboardAccessibility`, `skipMultiplayerWarning`, `incompatibleResourcePacks`, `soundDevice`.
6. Aplicar no pack: só os itens escolhidos, por `PackTransaction` (§6.5); `options.txt` é editado chave a chave.
7. Conflito de três vias: base = linha de base, "pack" = arquivo atual do pack, "instância" = arquivo atual da instância.

### 8.4 Sempre ignorados na captura

```
logs/  crash-reports/  debug/  saves/ (exceto saves/*/serverconfig/ e saves/*/datapacks/ como informação)
screenshots/  stats/  resources/  natives/  bin/
usercache.json  usernamecache.json  hotbar.nbt  command_history.txt  realms_persistence.json
hs_err_pid*.log  replay_pid*.log  *.heapdump
.fabric/  .quilt/  .mixin.out/  .cache/  local/  mods/.connector/  modernfix/
kubejs/exported/  kubejs/probe/  journeymap/data/  XaeroWaypoints/  XaeroWorldMap/  xaero/
packwiz.json  packwiz-installer*.jar
*.bak  *.old  *.tmp  servers.dat_old
```

A lista vive em `crates/warden-instance/data/capture-ignore.txt` (formato gitignore), com testes.

## 9. Diagnóstico

### 9.1 Modelo

```rust
pub struct Finding {
    pub rule: RuleCode,                 // ex.: E_MISSING_DEP
    pub severity: Severity,             // Error | Warning | Info
    pub title_key: String,              // chave i18n
    pub params: BTreeMap<String, String>,
    pub items: Vec<ItemRef>,            // itens do pack envolvidos
    pub evidence: Vec<Evidence>,        // obrigatório: pelo menos uma
    pub fixes: Vec<SuggestedFix>,       // AddDependency, RemoveItem, SetSide, UpdateItem, …
}
pub enum Evidence {
    JarMetadata { item: ItemRef, file_in_jar: String, excerpt: String },
    Api { source: Source, url: String, field: String },
    Log { file: String, line: u32, excerpt: String },
    Curated { rule_id: String, note: String },
    PackFile { path: String, excerpt: String },
}
```

Um achado sem evidência é bug (teste de propriedade garante).

### 9.2 Pré-teste em duas passagens

- **Rápida** (antes de baixar, no passo 1 do Testar): usa o pack, os metadados da API (Modrinth em cache; CurseForge na hora) e o catálogo.
- **Completa** (depois de sincronizar a instância, antes de abrir o jogo): acrescenta os metadados dos jars (incluindo jars aninhados) e a versão de classe Java.

Regras (códigos estáveis):

| Código | Regra | Gravidade | Fonte |
|---|---|---|---|
| `E_LOADER` | Jar sem metadados do loader do pack, ou versão da plataforma sem o loader. Mods `forge` em NeoForge 1.20.1 são aceitos; em ≥ 1.20.2, aviso na passagem rápida e erro na completa se o jar não tiver `neoforge.mods.toml` (§6.2). | erro | jar + API |
| `E_MCVERSION` | Versão do Minecraft fora do que o mod declara (`depends.minecraft`, `versionRange`, `acceptedMinecraftVersions`, `mcversion`, `game_versions`). Se a versão estiver em `acceptable-game-versions`, vira aviso. | erro/aviso | jar + API |
| `E_LOADERVERSION` | Versão do loader do pack fora da faixa exigida (`fabricloader`, `loaderVersion`, dependência `forge`/`neoforge`). | erro | jar |
| `E_MISSING_DEP` | Dependência obrigatória ausente, considerando `provides` e jars aninhados, e o lado da dependência. | erro | jar + API |
| `E_DEP_VERSION` | Dependência presente fora da faixa (inclui opcional presente fora da faixa no Forge/NeoForge). | erro | jar |
| `E_BREAKS` | `breaks` (Fabric), `incompatible` (NeoForge) com o outro mod presente e na faixa. | erro | jar |
| `W_CONFLICTS` | `conflicts` (Fabric), `discouraged` (NeoForge), `recommends` não satisfeito. | aviso | jar |
| `W_INCOMPATIBLE_API` | Modrinth `incompatible` ou CurseForge relação 5 com projeto presente. | aviso | API |
| `E_DUPLICATE_ID` | Mesmo mod id em dois jars. | erro | jar |
| `E_DUPLICATE_FILE` | Mesmo hash duas vezes; mesmo projeto por duas fontes. | erro | hash + API |
| `E_EXCLUSIVE` | Mais de um item na mesma categoria exclusiva (renderizadores; implementações do Lithium; ports do Sodium para 1.12.2; motores de luz). | erro | curada |
| `E_KNOWN_CONFLICT` / `W_KNOWN_CONFLICT` | Conflito conhecido não declarado (ex.: OptiFine com Sodium/Iris/Embeddium/ModernFix; Indium com Sodium ≥ 0.6). | erro/aviso | curada |
| `W_OBSOLETE` | Mod obsoleto para a versão (Starlight/Phosphor em ≥ 1.20, LazyDFU em ≥ 1.19.4, Embeddium/Rubidium/Oculus em NeoForge ≥ 1.21, projeto arquivado). | aviso | curada + API |
| `W_SIDE` | Mod só de cliente marcado `both`/`server`, ou o contrário. | aviso | jar + API |
| `E_JAVA` | Java escolhido incompatível com MC/loader, ou classes do jar mais novas que o Java escolhido, ou `javaVersion`/`depends.java` do mod. | erro | jar + catálogo |
| `I_PRERELEASE` | Versão `alpha`/`beta` em uso. | informação | API |
| `I_LEGACY_COREMOD` | Coremod/tweaker em 1.7.10/1.12.2. | informação | MANIFEST |
| `I_PACK_FORMAT` | `pack_format` de resource pack incompatível com a versão. | informação | zip |
| `W_DOWNLOAD_BLOCKED` | Mod da CurseForge com distribuição bloqueada. | aviso | API |

Dados curados em `crates/warden-diagnostics/data/` (`exclusive-categories.toml`, `known-conflicts.toml`, `obsolete.toml`), embutidos no binário com `include_str!` e validados por teste (esquema e IDs existentes). Atualização remota dos dados é P2.

### 9.3 Pós-crash

1. Coleta: código de saída, `output.log` da sessão, `logs/latest.log`, `logs/debug.log` (ou `logs/fml-client-latest.log` em Forge antigo), `crash-reports/*` e `hs_err_pid*.log` com data posterior ao início da sessão.
2. Limpeza: remove códigos `§x` e ANSI.
3. Catálogo de padrões em `crates/warden-diagnostics/data/log-patterns.toml`: cada padrão tem `id`, regex (crate `regex`), escopo (loader/faixa de versão), extratores de grupos e o `Finding` produzido. Ponto de partida: os 35 padrões de R2 §6.3 e as classes de problema do codex-minecraft (MIT, porte com atribuição em `THIRD_PARTY.md`).
4. Evidência = arquivo + linha + trecho; achado mais provável primeiro (crash report > tela de erro do loader > exceção mais profunda > padrões genéricos).
5. Corpus de testes: logs reais em `crates/warden-diagnostics/tests/corpus/<caso>/` com o resultado esperado.

### 9.4 Redação de dados pessoais

`warden-diagnostics::redact` (usada antes de qualquer envio à IA e no "Copiar relatório"):

| Dado | Substituição |
|---|---|
| Nome de usuário do sistema em caminhos (`C:\Users\<x>\`, `/home/<x>/`) e a variável `USERNAME`/`USER` em qualquer lugar | `<usuário>` |
| Nome do jogador e UUID offline | `<jogador>`, `<uuid>` |
| Nome do computador (`COMPUTERNAME`/hostname) | `<computador>` |
| IPv4/IPv6 (exceto `127.0.0.1`/`::1`) | `<ip>` |
| E-mails | `<email>` |
| Padrões de segredo (`$2a$…`, `ghp_…`, `github_pat_…`, `AIza…`, `x-api-key`, `token=`) e os valores reais das chaves do cofre | `<segredo>` |

A redação é testada com casos positivos e negativos; o diálogo de consentimento mostra o texto **depois** da redação, e esse é exatamente o texto enviado.

### 9.5 IA (Gemini, P1)

- API REST `generativelanguage.googleapis.com/v1beta/models/{modelo}:generateContent`, chave no cabeçalho `x-goog-api-key`.
- Modelo configurável; padrão resolvido na primeira configuração pela lista de modelos da conta (`models.list`), preferindo a família "flash" mais recente estável. Nada fixo no código.
- Conteúdo: resumo do pack (versão MC, loader, lista de mods com id/versão/lado), achados determinísticos, trechos relevantes dos logs (orçamento total de 200 KB de texto), tudo redigido.
- Resposta em JSON com esquema (`responseSchema`): `{ causaProvavel, confianca: "baixa"|"media"|"alta", passos: string[], modsEnvolvidos: string[] }`; validada antes de exibir.
- Tempo-limite 90 s; erros mapeados (`AI_KEY_INVALID`, `AI_QUOTA_EXCEEDED`, `AI_TIMEOUT`, `AI_BAD_RESPONSE`).

## 10. Configs

- Toda edição estruturada acontece no Rust (`warden-configs`); a interface recebe a árvore e devolve uma lista de edições `{ caminho, novoValor }`. A interface nunca regrava o arquivo inteiro, exceto no editor de texto, onde o conteúdo é o texto do próprio usuário.
- Formatos e bibliotecas:

| Formato | Leitura/edição | Observação |
|---|---|---|
| TOML (Forge/NeoForge e outros) | `toml_edit` | Comentários `#Range`, `#Allowed Values`, `#Default` viram metadados da chave. |
| JSON / JSONC | `jsonc-parser` (feature `cst`) | Preserva comentários e formatação. |
| JSON5 | `json-five` (avaliar na C-01); recuo: leitura com `json5` + edição por faixa de bytes | — |
| `.properties` | parser próprio por linha | Comentários `#`/`!`, continuação `\`, escapes `\uXXXX`. |
| `.cfg` do Forge antigo | parser próprio por linha | Prefixos `S/I/B/D/C/M`, categorias aninhadas, listas `< >`. |
| `options.txt` | parser próprio por linha | Preserva ordem e linhas desconhecidas. |
| YAML | só editor de texto na v1 | Formulário é P2 (`yaml-edit` a avaliar). |

- Invariantes testados para cada formato com um corpus de arquivos reais (`crates/warden-configs/tests/corpus/`): ler e gravar sem edição = bytes idênticos; uma edição = diff de uma linha (ou do trecho do valor); comparação semântica ignora formatação.
- Concorrência otimista: `config_read` devolve `{ conteúdo, hash }`; `config_write` exige o `hash` esperado e falha com `FILE_CHANGED_ON_DISK` se o arquivo mudou.
- Escrita no pack pela `PackTransaction` (§6.5); escrita na instância com gravação atômica (sem `refresh`).

## 11. Versionamento

`warden-versioning` sobre `git2` (libgit2 compilada junto, sem depender de git instalado; ADR-0015):

- **Packs criados pelo Warden:** branch única `main`, com commits de só dois tipos: o ponto inicial ("Pack criado") e "Versão X.Y.Z" (cada "Salvar versão"). Assim, "alterações não salvas" = diferença entre a árvore de trabalho e o último commit.
- **Packs importados que já são repositórios git** (branch com outro nome, histórico próprio, commits feitos fora do Warden): o Warden usa a branch atual sem renomear; "última versão salva" = a tag `v<SemVer>` mais recente alcançável a partir de `HEAD` (sem tag: nenhuma versão salva); "alterações não salvas" = diferença entre a árvore de trabalho e `HEAD`; o changelog e a sugestão SemVer comparam com a última versão salva. Repositório em estado especial (HEAD destacado, merge ou rebase em andamento, conflitos) bloqueia escritas no pack com explicação até ser resolvido por fora. Packs não versionados recebem repositório novo com o ponto inicial "Pack importado".
- **Tags anotadas** `vX.Y.Z` com o changelog da versão como mensagem.
- **Pontos de segurança:** commits fora da branch, referenciados por `refs/warden/safety/<data-hora>-<motivo>`, com a árvore de trabalho completa no momento. Lista canônica de quando são criados: restaurar versão; limpeza de higiene; atualizar todos; remover dois ou mais itens; substituir um item por outro de outra fonte; trazer mudanças do teste que sobrescrevem arquivos do pack.
- **Restaurar versão:** cria ponto de segurança; calcula a lista de operações (escrever/apagar) para a árvore de trabalho ficar igual à da versão (arquivos ignorados pelo `.gitignore` não são tocados); aplica com gravações atômicas; em falha, reaplica o ponto de segurança. Não move branch nem tags.
- **Changelog:** compara os `.pw.toml` da última versão salva (lidos dos blobs) com os atuais: mesmo projeto por `[update.modrinth].mod-id` ou `[update.curseforge].project-id` (ou caminho, para link/local) → atualizado se a versão mudou; senão adicionado/removido. Nomes de versão legíveis pelo `VersionNameResolver` (cache do Modrinth ou API da CurseForge, no momento de salvar); recuo: nome do arquivo. Configs: lista de arquivos alterados fora de `mods/`, `resourcepacks/`, `shaderpacks/`.
- **Sugestão SemVer:** regras de SPEC T16, implementadas como função pura testada.
- **CHANGELOG.md:** cada versão acrescenta no topo:

```markdown
## 1.3.0 — 2026-10-01

Notas livres do usuário.

### Atenção
- Mods removidos podem apagar blocos ou itens de mundos existentes. Faça backup antes de atualizar.

### Mods adicionados
- Sodium 0.6.0 (Modrinth)

### Mods removidos
- OptiFine HD U I6 (arquivo local)

### Mods atualizados
- Lithium 0.14.1 → 0.14.3

### Resource packs e shaders
- …

### Configurações alteradas
- config/sodium-options.json
```

- **Identidade dos commits:** nome do autor do pack; e-mail `<login>@users.noreply.github.com` se o GitHub estiver configurado, senão `warden@localhost`.
- **GitHub:** API REST (`GET /user`, `POST /user/repos` com `private: true`) por `warden-http`; push por `git2` com credencial em memória (`x-access-token` + token do cofre). O token nunca vai para a URL do remoto nem para `.git/config`. Histórico divergente → erro `REMOTE_DIVERGED`; "substituir o GitHub" consulta o commit remoto e só força o envio se ele ainda for o esperado (equivalente ao `--force-with-lease`; como o `git2` não oferece a operação atômica, existe uma pequena janela de corrida, aceitável para um único usuário).

## 12. Exportação

1. Verificação de higiene (§6.4) e diagnóstico rápido.
2. Cópia de staging do pack (`cache/staging/<operação>/`), só com `pack.toml`, `index.toml`, `.packwizignore` e os arquivos não ignorados.
3. `packwiz refresh --build` na cópia; conferir zero diferença em relação ao pack (exceto hashes internos que o `--build` acrescenta quando `no-internal-hashes` estiver ligado).
4. Pré-visualização a partir do índice (tamanhos, contagens, alertas).
5. Saída: pasta (cópia de `pack.toml`, `index.toml` e dos arquivos do índice) ou `.zip` (crate `zip`, entradas de diretório presentes, ordem determinística, datas fixas, sem bits de permissão estranhos — corrige a issue #115 do packwiz).
6. Conformidade (nos testes e opcionalmente no app): packwiz-installer instala a partir da saída numa pasta vazia.

## 13. Layout em disco

Identificador do app: `dev.kriticales.warden`.

| O quê | Windows | Linux | Conteúdo |
|---|---|---|---|
| Packs (projetos) | escolhido pelo usuário; padrão `%USERPROFILE%\Documents\Warden\` | `~/Documentos/Warden/` ou `~/Documents/Warden/` (pasta de documentos do sistema) | Um subdiretório por pack (repositório git). |
| Configuração | `%APPDATA%\dev.kriticales.warden\` | `~/.config/dev.kriticales.warden/` | `settings.json` (com `schemaVersion`), `packs.json` (registro: id → caminho, último teste, preferências de teste do pack). |
| Dados locais | `%LOCALAPPDATA%\dev.kriticales.warden\` | `~/.local/share/dev.kriticales.warden/` | ver abaixo |

Dentro de "dados locais":

```
instances/<pack-id>/minecraft/        gameDir da instância de teste
instances/<pack-id>/state/            manifest.json, baseline/, sessions/, optional-choices.json
shared/                               armazenamento do motor do launcher (bibliotecas, assets, versões, natives)
shared/runtimes/                      Javas instalados pelo Warden
cache/downloads/                      jars e arquivos baixados, por sha256 (+ index.sqlite de hashes)
cache/packwiz/                        cache do sidecar (--cache)
cache/metadata.sqlite                 cache do Modrinth e do catálogo
cache/staging/<operação>/             cópias temporárias do pack (apagadas ao fim; limpeza ao iniciar)
cache/tmp/                            temporários diversos
packwiz/packwiz.toml                  config vazia passada ao sidecar (--config)
logs/                                 registros do app (§16)
```

Dentro de cada pack:

```
pack.toml  index.toml  .packwizignore  .gitignore  .gitattributes  CHANGELOG.md
mods/  resourcepacks/  shaderpacks/  config/  defaultconfigs/  kubejs/  …    (conteúdo)
.warden/project.toml                  id do pack, schemaVersion, avisos do diagnóstico ignorados (P1)
.git/
```

`.warden/` é versionado no git e ignorado pelo packwiz. Dados de máquina **nunca** ficam em `.warden/`: as preferências de teste de cada pack (memória, Java escolhido, argumentos JVM) ficam em `packs.json`, e as escolhas de opcionais, a linha de base e as sessões ficam em `instances/<id>/state/`. Assim, ajustar a memória do teste não conta como alteração não salva do pack.

`PackId` é um ULID gerado na criação/importação e gravado em `.warden/project.toml`; instâncias, caches e registros usam esse id, não o caminho.

## 14. Segredos

- `warden-secrets` usa `keyring` (Gerenciador de Credenciais no Windows, Secret Service no Linux) com serviço `dev.kriticales.warden` e contas `curseforge-api-key`, `gemini-api-key`, `github-token`.
- O acesso passa por um trait `SecretStore`. Além da implementação do sistema, há uma implementação de teste em arquivo temporário, selecionável só em build de debug por `WARDEN_SECRET_BACKEND=file:<pasta>` — usada pelos testes e E2E no WSL e no runner Linux, onde não há Secret Service. O teste com o cofre real roda na CI Windows.
- Valores circulam como `secrecy::SecretString` (o `Debug` imprime `[REDACTED]`), são lidos do cofre só no momento do uso e nunca vão para a interface: a interface só conhece `secrets_status() → { curseforge: bool, gemini: bool, github: bool }`.
- Desenvolvimento: só em build de debug, se o cofre não tiver a chave, `warden-secrets` lê `CURSEFORGE_API_KEY`, `GEMINI_API_KEY` e `GITHUB_TOKEN` do ambiente. `cargo xtask dev` e `cargo xtask test-network` carregam o `.env` do repositório principal (descoberto por `git rev-parse --git-common-dir`, sem copiar o arquivo e sem imprimir valores). Em build de release essa leitura não existe (`#[cfg(debug_assertions)]`).
- CI: testes de rede usam o segredo `CURSEFORGE_API_KEY` do GitHub Actions, só no workflow agendado/manual.
- Ao passar a chave para o sidecar, só pela variável `WARDEN_CURSEFORGE_API_KEY` do processo filho; a linha de comando e o ambiente do filho nunca são registrados.

## 15. Concorrência, travas e cancelamento

- **Runtime:** Tokio (o do Tauri). Trabalho bloqueante (hash de arquivos grandes, zip, `git2`, leitura de jars) vai para `spawn_blocking`.
- **Travas por pack** (`PackLocks` em `warden-app`): `RwLock` por `PackId`. Leituras (inventário, detalhes, diagnóstico rápido) pegam leitura; mutações pegam escrita. A espera aparece em Tarefas como "Aguardando outra operação neste pack".
- **Registro de operações** (`OperationRegistry`): toda operação longa recebe `OperationId`, `CancellationToken`, tipo, pack e progresso; `operation_cancel` aciona o token. Operações que não podem ser canceladas no meio (gravação final de uma transação) ignoram o cancelamento só nesse trecho.
- **Um jogo por vez** no app (estado global em `warden-app`); a trava do pack **não** fica presa enquanto o jogo roda, só durante a materialização.
- **Tempos-limite:** HTTP (conexão 10 s, resposta 30 s, download sem limite total mas com 30 s sem dados), sidecar (§6.3), processors do Forge (10 min), validação de Java (20 s).
- **Instância única do app:** `tauri-plugin-single-instance`.
- **Mudanças externas (P1):** `notify` com espera de 500 ms na pasta do pack; eventos causados pelo próprio Warden são descartados por "fichas de escrita" registradas pela `PackTransaction`; o resto vira `pack-changed`.

## 16. Registros (logging)

- `tracing` + `tracing-subscriber` + `tracing-appender`: arquivo diário `logs/warden.AAAA-MM-DD.log`, 14 dias de retenção, texto legível com horário, nível, alvo, `operation_id` e `pack_id` nos spans.
- Nível padrão `info`; "detalhado" em Configurações liga `debug` para `warden_*`.
- Registros do frontend chegam pelo `tauri-plugin-log` ao mesmo arquivo.
- Pânicos: gancho registra a mensagem e a pilha e mostra erro amigável.
- Proibido registrar: chaves, tokens, cabeçalhos de autorização, ambiente de processos filhos, corpo de respostas da CurseForge (termos). Permitido: caminhos locais (o arquivo é local; a redação acontece antes de qualquer envio).
- Logs do jogo não vão para o registro do app; ficam na sessão (§7.4).

## 17. Rede e APIs externas

- `warden-http`: `reqwest` com `rustls`, User-Agent `Kriticales/Warden/<versão> (+https://github.com/Kriticales/Warden)`, limitador por host (`governor`), novas tentativas com espera exponencial (máx. 3) em 429/5xx/tempo esgotado, só para requisições idempotentes, respeitando `Retry-After` e `X-Ratelimit-Reset`.
- Downloads: streaming para `.part` no cache, hash calculado durante o download, retomada com `Range` quando o servidor aceita, renomeação ao final.
- Modrinth: ≤ 300 req/min; uso de lote (`/projects?ids=`, `/versions?ids=`, `/version_files`, `/version_files/update`); cache em `metadata.sqlite` (versões são imutáveis; projetos expiram em 24 h).
- CurseForge: chave do cofre no cabeçalho `x-api-key`; lote (`POST /v1/mods`, `POST /v1/mods/files`, `POST /v1/fingerprints/432`); concorrência máxima 4; **nenhuma resposta da API é persistida** (cache só em memória durante a sessão; `downloadUrl` nunca gravada). Os arquivos de mods baixados para a instância ficam no cache de downloads por hash, como fazem o packwiz e o Prism.
- Mojang, Fabric meta, Maven do Forge/NeoForge: catálogo com cache (manifesto revalidado a cada 6 h; JSON de versão por `sha1`).
- Sem internet: erros `NETWORK_UNAVAILABLE` (retryable) e uso do que houver em cache.
- Testes: em build de debug, a URL base de cada API pode ser trocada por variável de ambiente (`WARDEN_API_BASE_MODRINTH`, `WARDEN_API_BASE_CURSEFORGE`, `WARDEN_API_BASE_MOJANG`, `WARDEN_API_BASE_FABRIC`, `WARDEN_API_BASE_FORGE`, `WARDEN_API_BASE_NEOFORGE`, `WARDEN_API_BASE_ADOPTIUM`, `WARDEN_API_BASE_GITHUB`, `WARDEN_API_BASE_GEMINI`), apontando para o servidor local de fixtures usado pelos E2E (`apps/desktop/e2e/mock-server/`). Em release essas variáveis são ignoradas; por isso os E2E rodam sempre sobre um build de debug (`tauri build --debug`), e o instalador de release é validado pelos roteiros manuais.

## 18. Frontend

- **Camadas:** `routes/` (páginas, carregam dados e compõem features) → `features/<f>/components` → `features/<f>/hooks` (TanStack Query sobre `lib/ipc`) → `lib/ipc/bindings.ts` (gerado). Componentes não chamam `invoke` direto.
- **Estado do servidor:** TanStack Query com chaves `['pack', packId, área, …]`. O evento `pack-changed` invalida as chaves das áreas indicadas. Mutations nunca fazem atualização otimista de dados do pack (o disco é a verdade); mostram progresso e invalidam ao terminar.
- **Estado de interface:** Zustand só para o que não vem do backend (seleção na lista, filtros do console, painel aberto). Nada de dados do pack no Zustand.
- **Rotas:** TanStack Router com rotas por arquivo (`src/routes/`); `routeTree.gen.ts` é gerado e **não** versionado (gerado antes de `typecheck`, `test` e `build`). Parâmetros tipados: `/packs/$packId/mods`, `/packs/$packId/configs?path=…`.
- **Formulários:** React Hook Form + Zod (esquemas em `features/<f>/lib/schemas.ts`, mensagens via i18n).
- **Listas longas:** TanStack Virtual.
- **Editor de configs:** CodeMirror 6 (`@codemirror/lang-json`, `@codemirror/legacy-modes` para TOML, properties e YAML, `@codemirror/merge` para diferenças).
- **Conteúdo remoto:** Markdown com `react-markdown` + `rehype-sanitize`; HTML da CurseForge com DOMPurify; links externos abrem no navegador (plugin `opener`, só `https:`).
- **i18n:** i18next + react-i18next com um único idioma (pt-BR); recursos tipados (chaves inexistentes não compilam); arquivos por área em `src/i18n/pt-BR/<área>.ts`; ESLint `i18next/no-literal-string` proíbe texto solto em JSX.
- **Componentes base** em `components/common/`: `EmptyState`, `ErrorPanel`, `LoadingState`, `ConfirmDialog` (com digitação de confirmação opcional), `ProgressBar`, `OperationToast`, `DiffView`.
- **Testes:** Vitest + Testing Library + `@tauri-apps/api/mocks` (`mockIPC`) com fábricas de dados tipadas a partir dos bindings; E2E com WebdriverIO + `tauri-driver` contra o app real (QUALITY §4).

## 19. Bibliotecas escolhidas

Versões exatas fixadas no `Cargo.lock`/`pnpm-lock.yaml` pela F0-01; atualizações passam por revisão.

### 19.1 Rust

| Uso | Biblioteca | Por quê |
|---|---|---|
| App desktop | `tauri` 2, `tauri-build`, plugins `dialog` (usado só pelo Rust), `opener`, `log`, `single-instance`, `window-state` | Decisão do dono (ADR-0002); plugins oficiais mínimos. |
| Contrato IPC | `specta`, `tauri-specta` (2.0 RC fixado) | Gera tipos e chamadas TS a partir do Rust (ADR-0018). Risco: ainda RC; plano B `ts-rs` + wrappers gerados por xtask. |
| Assíncrono | `tokio`, `tokio-util` (`CancellationToken`), `futures`, `async-trait` | Padrão do ecossistema; cancelamento cooperativo. |
| Erros | `thiserror` 2 | Erros tipados por crate; `anyhow` só em `xtask` e testes. |
| Serialização | `serde`, `serde_json`, `toml`, `toml_edit` | `toml_edit` preserva formatação. |
| HTTP | `reqwest` (rustls), `governor` | Cliente maduro; limitador por host. |
| Hash | `sha1`, `sha2`, `md-5` (RustCrypto); murmur2 próprio (variante CurseForge) | Sem dependência nativa. |
| Zip | `zip` | Leitura de jars e exportação. |
| Git | `git2` (libgit2 vendorizada) | Push HTTPS com credencial em memória; `gix` ainda não cobre push de forma estável. |
| Cofre | `keyring`, `secrecy` | Windows Credential Manager / Secret Service. |
| Banco local | `rusqlite` (bundled) | Cache de metadados e índice de downloads. |
| Configs | `jsonc-parser` (cst), `json-five`, parsers próprios | R3 §5.6. |
| NBT (P2) | `fastnbt` | `servers.dat` não comprimido. |
| Logs | `tracing`, `tracing-subscriber`, `tracing-appender` | Spans com contexto de operação. |
| Regex / XML / encoding | `regex`, `quick-xml`, `encoding_rs` | Análise de logs e stream log4j. |
| Vigiar pastas (P1) | `notify` + `notify-debouncer-full` | Mudanças externas. |
| Ids | `ulid`, `uuid` | `PackId` ordenável; UUID offline. |
| Versões | `semver`; FlexVer e faixas Maven próprias | R3 §5.6. |
| Windows | `windows` (Job Objects) | Encerramento da árvore de processos. |
| Testes | `cargo-nextest`, `insta` (dourados), `wiremock`, `tempfile`, `proptest`, `criterion`, `cargo-llvm-cov`, `cargo-deny` | QUALITY §4. |

### 19.2 Frontend

| Uso | Biblioteca | Por quê |
|---|---|---|
| Base | React 19, TypeScript (strict), Vite | Decisão do dono (ADR-0002). |
| Rotas | TanStack Router | Parâmetros e busca tipados; rotas por arquivo evitam conflito entre agentes. |
| Dados assíncronos | TanStack Query | Cache, estados de carregamento/erro, invalidação por evento. |
| Estado de interface | Zustand | Pequeno, sem cerimônia; só estado local de UI. |
| Componentes | shadcn/ui (Radix UI) + Tailwind CSS 4 + `lucide-react` | Acessíveis por padrão, código no repositório (sem caixa-preta), tema claro/escuro. |
| Formulários | React Hook Form + Zod | Validação tipada e performática. |
| Editor de configs | CodeMirror 6 (+ `@codemirror/merge`) | Leve, modular, sem web workers; Monaco é pesado para o WebView. |
| Listas | TanStack Virtual | Listas de centenas de mods e console. |
| i18n | i18next + react-i18next | Plural, interpolação, chaves tipadas. |
| Markdown/HTML | `react-markdown`, `rehype-sanitize`, `dompurify` | Descrições de mods seguras. |
| Testes | Vitest, Testing Library, jsdom, `@tauri-apps/api/mocks`, WebdriverIO + `tauri-driver` | QUALITY §4. |
| Qualidade | ESLint (typescript-eslint `strictTypeChecked`, react-hooks, jsx-a11y, i18next), Prettier | QUALITY §2. |

## 20. Segurança do app Tauri

- **Capabilities** mínimas em `capabilities/default.json`: eventos e comandos do app, `opener` restrito a URLs `https:`, `log`. Diálogos de arquivo/pasta e "mostrar na pasta" são acionados pelo Rust dentro dos comandos (§4.1), não pela interface. **Sem** plugin `shell` na interface, **sem** `fs` na interface (todo acesso a arquivo passa por comandos com validação).
- **CSP:** `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; connect-src ipc: http://ipc.localhost; object-src 'none'; frame-src 'none'`.
- Nenhum comando genérico do tipo "execute isto" ou "leia este caminho absoluto" (R4 §1.5).
- Todo caminho relativo validado por `resolve_inside`; extração de zip protegida contra *zip slip*; `.mrpack`/arquivos de terceiros nunca extraídos fora do destino.
- Conteúdo remoto (descrições) sempre higienizado; nada de `dangerouslySetInnerHTML` fora do componente `SafeHtml`.
- Downloads só por HTTPS, com hash verificado antes de uso.
