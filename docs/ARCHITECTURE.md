# Warden — Arquitetura técnica

> Versão do documento: 1.3 (2026-10-02). Tarefa A1; atualizada na tarefa D2 com as decisões do dono (ADR-0025 a ADR-0029): segredos, busca combinada, Java, IA e publicação no GitHub; na tarefa D4 com as funções avançadas (ADR-0030 a ADR-0038): busca do culpado, IA com ferramentas, raio-x de mixins, grafo, nota de saúde, desempenho, servidor local, descoberta, importação, scripts e pacote para servidor; e na tarefa D5 com o **Warden 1.1 "Profissional"** (ADR-0039 a ADR-0047): crate `warden-security`, módulos novos e os ganchos que a v1 deixa prontos (§21).
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
21. [Warden 1.1 "Profissional": módulos e ganchos na v1](#21-warden-11-profissional-módulos-e-ganchos-na-v1)

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
   sidecar packwiz   processos java     HTTPS (Mojang, Modrinth,   git (libgit2,
   (refresh/export/  (o jogo de teste;  CurseForge, Adoptium,      embutida)
   curseforge        servidor local     Fabric/Forge/NeoForge,
   import)           sob demanda, só    GitHub, Gemini)
                     em 127.0.0.1)
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
| `warden-jarmeta` | Leitura de metadados de jars sem extrair: `fabric.mod.json` (com `jars[]`), `quilt.mod.json` (só leitura, para identificar), `META-INF/mods.toml`, `META-INF/neoforge.mods.toml`, `META-INF/jarjar/metadata.json`, `mcmod.info` tolerante, `MANIFEST.MF` (coremods), versão de classe Java; dialetos de faixa (Fabric, Maven, `@Mod`), FlexVer. D4: **índices** pacote → jar → modid e config de mixin → mod, descendo em jars aninhados, com cache por hash (§9.6); leitura de arquivos de idioma (`assets/<modid>/lang/*.json`) e da lista de itens, blocos e tags (`assets/*/models/item`, `data/*/tags`) para os rótulos de config e os IDs dos scripts. | core |
| `warden-project` | Serviço de domínio do pack: registro de packs, criar/abrir, inventário guiado pelo índice, busca combinada Modrinth + CurseForge (§17), adicionar/remover/atualizar (inclusive vários itens de uma vez), dependências, deduplicação, lado, opcionais, fixar, higiene, transação de escrita (§6.5); D4: mods iniciais, kits de desempenho e ferramentas do jogador (dados versionados em `crates/warden-project/data/`). | core, packwiz, packwiz-cli, modrinth, curseforge, catalog, jarmeta, http |
| `warden-configs` | Formatos de config com preservação: TOML, JSON/JSONC, JSON5, `.properties`, `.cfg` do Forge antigo, `options.txt`; modelo em árvore; edições mínimas; comparação semântica. D4: metadados em camadas para o formulário (§10.1), padrões da camada A, índice de busca em todas as configs. | core, jarmeta |
| `warden-java` | Runtimes Java: descoberta, download (Adoptium; fallback Mojang), validação, política de seleção "mais novo que funciona" com motivo e atualização dos Javas instalados (§7.3). | core, http |
| `warden-launcher` | Interface `LauncherEngine`, adaptador do motor escolhido no spike S1, perfil offline, supervisão do processo, leitura de logs do jogo (log4j XML e texto). D4: Quick Play (entrar direto num mundo ou no servidor local) e propriedades de sistema por teste (§7.1). | core, java, catalog |
| `warden-instance` | Instância de teste: materialização pack → instância, manifesto de estado, linha de base, captura instância → pack, comparação de três vias. D4: filtro das ferramentas do jogador, materialização de subconjuntos com links físicos para a busca do culpado, instância limpa pelo link do pack com o bootstrap em cache (§8.5). | core, packwiz, configs, modrinth, curseforge, http |
| `warden-diagnostics` | Regras pré-teste, análise pós-crash, dados curados, redação de dados pessoais, montagem do relatório. D4: grafo de dependências e consultas, assinatura de travamento e histórico de travamentos, nota de saúde, console agrupado (normalização e atribuição), achados de faixa de config e de memória (§9.6, §9.8). | core, packwiz, jarmeta, catalog, modrinth, curseforge |
| `warden-ai` | Cliente Gemini com *function calling*: laço de ferramentas sem estado, registro de ferramentas de leitura e de proposta, conferência de evidências, consentimento por conversa, conversas por pack (§9.5, ADR-0030). As ferramentas leem por traits implementados pela `warden-app` (sem depender das crates de cima). | core, http, diagnostics |
| `warden-mixin` | Raio-x de mixins: configs de mixin, refmaps e anotações das classes (`cafebabe`), inventário por mod, sobreposições por ponto de aplicação e regras de risco com rebaixadores e elevadores (§9.6, ADR-0034). | core, jarmeta |
| `warden-bisect` | Busca do culpado: ordem topológica, prefixos, busca binária, minimização para pares, comparação de assinaturas, estado persistente e pausável; algoritmo puro, testado com um executor simulado (§9.7, ADR-0031). | core, diagnostics |
| `warden-server` | Servidor local sob demanda: instaladores de servidor por loader, `server.properties` controlado, EULA, ciclo de vida, comandos pelo stdin, detecção de "Done" (§7.6, ADR-0032). | core, http, java, catalog, instance |
| `warden-perf` | Desempenho do jogo: RAM do processo, leitor de `hsperfdata`, tempos de carregamento (total e por mod a partir dos logs), leitura de `.sparkprofile` (protobuf), achados de memória (§7.7, ADR-0038). | core |
| `warden-discovery` | Página de descoberta: início (populares, atualizados, kits), mapeamento curado de categorias, mescla por números, modpacks (lista de mods pela API do Modrinth e leitura por partes de `.mrpack`/`manifest.json`), compatibilidade com o pack (§17.1). | core, http, modrinth, curseforge |
| `warden-import` | Importar `.mrpack`, zip da CurseForge (via sidecar em staging) e instâncias do Prism/MultiMC e do app da CurseForge para o formato packwiz, com higiene obrigatória (§12.2, ADR-0035). | core, packwiz, packwiz-cli, modrinth, curseforge, http, jarmeta |
| `warden-scripts` | Scripts KubeJS e CraftTweaker: detecção de versão, trechos prontos, índice de IDs, leitura de erros dos logs, cliente do servidor web local do KubeJS 7, comando de recarga certo por versão (§10.2, ADR-0037). | core, jarmeta, http |
| `warden-versioning` | Git embutido (`git2`): repositório, ponto inicial, salvar versão, versão final, tags, pontos de segurança, restauração transacional, diffs, changelog e notas de publicação, publicação no GitHub (linha de publicação, push, Release pela API REST; §11.1). Nomes legíveis de versões vêm de um trait `VersionNameResolver` implementado pela `warden-app` (com Modrinth e CurseForge). | core, packwiz, http |
| `warden-security` (1.1) | Segurança dos mods: leitura dos jars sem executar (`zip` + `cafebabe`), descida em jars embutidos, sinais conhecidos e pontos de atenção a partir de `data/signatures.toml`, conferência com o arquivo oficial a partir dos resultados das APIs (recebidos por trait, sem depender dos clientes), confiança por hash (`.warden/trust.toml`), cache por hash (§21.1, ADR-0040, ADR-0041). Não existe na v1: a W-01 cria a crate. | core, jarmeta, packwiz (murmur2) |
| `warden-export` | Pré-visualização e exportação nativa (pasta/zip), conformidade; P1 (v1, decisão D1): `.mrpack`/CurseForge via sidecar em staging, com validação; P1 (D4): pacote para servidor nas duas variantes (§12.1). | core, packwiz, packwiz-cli, project |
| `warden-secrets` | Armazenamento de segredos: cofre do sistema (`keyring`, padrão) ou arquivo `.env` na pasta de configuração (escolha do usuário, ADR-0025), migração entre os dois, `SecretString`, fallback de desenvolvimento por variável de ambiente só em build de debug. | core |
| `warden-app` (`apps/desktop/src-tauri`) | Comandos, eventos, estado do app, configurações (`settings.json`), registro de operações, ligação de tudo. | todas |
| `xtask` | `setup`, `dev`, `check` (e `check --fast`), `check-deps`, `check-docs`, `bindings`, `coverage`, `test-network` (F0-01); `build-packwiz` (F0-03); `win-dev`, `win-install` (F0-04); `fixtures-packwiz` (P1-01); `installer` (L-03: bootstrap do packwiz-installer e JRE de testes; L-07 acrescenta o `packwiz-installer.jar`); `notices` (A-02); `check-kits` (P1-18: valida kits e mods iniciais contra a API do Modrinth). | — (ferramenta) |

Na 1.1 (§21), crescem também: `warden-diagnostics` (manutenção, lista de mods e versão do jogador, itens repetidos, categorias novas da saúde), `warden-jarmeta` (materiais, tags e geração de minério), `warden-perf` (histórico de desempenho), `warden-project` (notas e grupos, troca por substituto), `warden-discovery` (busca de parecidos) e `warden-versioning` (notas no resumo, checagens do Publicar).

Grafo sem ciclos; `warden-core` não depende de ninguém. Nenhuma crate de domínio chama outra "para cima" (ex.: `warden-instance` não depende de `warden-project`; quem orquestra é `warden-app`). A busca do culpado, o servidor local, a validação do pacote para servidor e as ferramentas da IA são orquestrados pela `warden-app` (`bisect_session.rs`, `server_session.rs`, `ai_tools/`), que liga instância, launcher, servidor e diagnóstico.

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
| secrets | `secrets_status`, `secrets_set`, `secrets_test`, `secrets_remove` (não existe "ler segredo"), `secrets_backend_get`, `secrets_backend_set` (cofre ↔ `.env`, move as chaves) |
| catalog | `catalog_minecraft_versions`, `catalog_loader_versions` |
| packs | `packs_list`, `pack_create`, `pack_import`, `pack_relocate`, `pack_reveal_folder`, `pack_forget`, `pack_trash` (P1), `pack_get`, `pack_update_meta`, `pack_hygiene_scan`, `pack_hygiene_fix` |
| inventory | `inventory_list`, `item_details`, `items_set_side`, `items_remove_plan`, `items_remove`, `item_set_optional` (P1), `item_set_pin` (P1), `item_change_version_plan` (P1), `pack_change_loader_version_plan` (P1), `instance_set_optional_choices` (P1) |
| search/add | `search_projects` (busca combinada; as fontes são internas), `project_details`, `project_versions`, `project_gallery` (P1), `project_changelog` (P1), `add_plan` (aceita vários itens), `add_apply`, `add_from_link_plan`, `add_local_files_plan` |
| updates | `updates_check`, `updates_plan`, `updates_apply` |
| configs | `config_tree`, `config_read`, `config_write`, `config_search` (P0), `config_structured_read` (P1, com metadados em camadas), `config_structured_apply` (P1), `config_restore_default` (P1), `config_set_preserve` (P1) |
| test | `test_start` (com o perfil e o modo: normal, como o jogador recebe, com perfil de desempenho, como servidor), `test_stop`, `test_session_get`, `test_sessions_list`, `instance_reveal_folder`, `instance_recreate`, `cf_blocked_list`, `cf_blocked_provide_file` |
| capture | `capture_changes`, `capture_apply`, `capture_discard` |
| diagnostics | `diagnostics_run`, `diagnostics_report_get`, `diagnostics_ignore` (P1) |
| ai | `ai_sources_list` (P1: último travamento, outras sessões), `ai_conversation_preview` (P1: texto inicial e o que a IA poderá consultar; aceita sessão ou arquivo escolhido pelo diálogo nativo), `ai_conversation_start` (P1, com as permissões escolhidas), `ai_conversation_send` (P1), `ai_conversations_list`, `ai_conversation_get`, `ai_conversation_delete` (P1), `ai_proposal_apply`, `ai_proposal_discard` (P1) |
| graph | `graph_dependents` (P0: transitivos), `graph_why_in_pack` (P0), `graph_orphans` (P0), `graph_focus` (P1: nós e ligações em volta de um item) |
| xray | `xray_mod` (P1: inventário e sobreposições de um mod), `xray_overlaps` (P1: todas as sobreposições do pack) |
| health | `health_get` (P1: nota, faixa e o que tirou pontos), `crashes_list` (P0: travamentos agrupados por assinatura), `crash_get` (P0) |
| bisect | `bisect_plan` (P1: estimativa, sempre ligados, desligados), `bisect_start`, `bisect_pause`, `bisect_resume`, `bisect_cancel`, `bisect_answer` (modo assistido), `bisect_get` (P1) |
| perf | eventos de amostra pelo canal do teste; `perf_report_get` (P1: tempos por mod e resumo do spark de uma sessão), `perf_open_spark_viewer` (P1: abre o arquivo local no navegador) |
| profiles | `test_profiles_list`, `test_profile_save`, `test_profile_delete`, `test_profile_select` (P1) |
| server | `server_plan` (P1: memória, mods de fora, EULA), `server_start`, `server_command`, `server_stop` (P1), `eula_status`, `eula_accept`, `eula_revoke` (P1) |
| discover | `discover_home` (P0: populares, atualizados, categorias; P1: kits), `discover_categories` (P0), `modpack_contents` (P1), `kits_list` (P1), `initial_mods_plan` (P0) |
| import | `import_detect` (P1), `import_plan` (P1: o que vira referência, arquivo local, revisão e lixo), `import_run` (P1) |
| scripts | `scripts_info` (P1: KubeJS/CraftTweaker e versão), `scripts_snippets`, `scripts_ids` (P1: IDs dos jars, e do servidor web com o jogo aberto), `scripts_errors` (P1), `scripts_reload` (P1), `scripts_open_vscode` (P1) |
| versioning | `versions_status`, `version_suggest`, `version_save` (com "versão final" opcional), `version_mark_final`, `version_unmark_final`, `history_list`, `history_diff`, `history_restore`, `safety_points_list` (P1), `safety_point_restore` (P1) |
| github | `github_status` (repositório, visibilidade, versão publicada, link do pack), `github_setup` (criar público/privado ou vincular existente), `publish_plan` (avisos e notas da versão), `publish_run` |
| export | `export_preview`, `export_run` (com versão salva opcional, P1; formato pacote para servidor com a variante, P1) |
| java | `java_runtimes_list` (com os packs que usam cada um e o motivo), `java_runtime_remove`, `java_runtimes_check_updates`, `java_choice` (Java automático de um pack, com motivo) |
| security (1.1) | `security_scan` (todos os arquivos ou uma lista; canal de progresso), `security_report_get`, `security_trust` (por hash, com o nome digitado), `security_replace_with_official` |
| maintenance (1.1) | `maintenance_check` (com ou sem cache), `maintenance_report_get`, `replacements_find` (item), `item_replace_plan`, `item_replace_apply` |
| player reports (1.1) | `player_report_from_link`, `player_report_from_files` (diálogo nativo), `player_report_from_text`, `player_reports_list`, `player_report_get`, `player_report_delete` |
| annotations (1.1) | `annotations_get`, `annotation_set_note`, `groups_create`, `groups_rename`, `groups_delete`, `items_set_groups` |
| duplicates (1.1) | `duplicates_report_get` |
| perf history (1.1) | `perf_history_get` (perfil; devolve versões, medianas, exclusões e o veredito "mais pesada") |

### 4.2 Eventos globais

Emitidos com `app.emit` para quem estiver ouvindo; tipados com `tauri-specta` (`#[derive(Event)]`):

| Evento | Carga | Uso |
|---|---|---|
| `pack-changed` | `{ packId, areas: ("inventory" \| "configs" \| "meta" \| "history")[] }` | Invalida queries do TanStack Query. Emitido após qualquer escrita do Warden e após mudança externa detectada (P1). |
| `operation-updated` | `OperationSnapshot` | Painel Tarefas (T22). |
| `game-state` | `{ packId, state: "preparing" \| "running" \| "bisecting" \| "exited", sessionId, profile }` | Cabeçalho do pack ("Buscando o culpado: ver progresso" no estado `bisecting`), bloqueio de "um jogo por vez" (a busca do culpado e o servidor local contam como jogo). |
| `server-state` | `{ packId, state: "installing" \| "starting" \| "ready" \| "stopping" \| "stopped", port }` | Tela do teste com servidor (T27). |
| `ai-conversation-updated` | `{ packId, conversationId }` | Lista de conversas e a conversa aberta. |

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
    PerfSample { rss_mb: u32, heap_used_mb: Option<u32>, heap_max_mb: Option<u32>,
                 gc_count: Option<u32>, gc_ms: Option<u32> },  // a cada 1 s durante o teste (§7.7)
    Round { index: u32, total_estimate: (u32, u32), enabled: u32, suspects: u32,
            result: Option<RoundResult> },                   // busca do culpado (§9.7)
    ToolCall { conversation_id: Ulid, sent_bytes: u64, label_key: String },  // IA (§9.5)
    Warning { error: AppError },
    Finished,                                                // o resultado vem no retorno do comando
}
```

O console do jogo usa o mesmo canal (`Log`). O backend agrega linhas em lotes de até 50 ms para não inundar o IPC. Com servidor local, as linhas levam a origem (`source: game | server`) e a interface filtra pelo seletor "Mostrando".

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
    // D4
    Mixin(MixinErrorCode), Bisect(BisectErrorCode), Server(ServerErrorCode), Perf(PerfErrorCode),
    Discovery(DiscoveryErrorCode), Import(ImportErrorCode), Scripts(ScriptsErrorCode),
    // D5 (Warden 1.1); a W-01 acrescenta o domínio com INTERNAL
    Security(SecurityErrorCode),
}
```

- `CoreErrorCode` (de `warden-core`) reúne o que é comum a todos: `CANCELLED`, `TIMEOUT`, `IO`, `PATH_OUTSIDE_ROOT`, `NETWORK_UNAVAILABLE`. `PACK_CHANGED_EXTERNALLY` pertence a `Project`.
- A F0-05 cria todos os domínios acima, exceto o `Security` da 1.1 (acrescentado pela W-01), já com um código `INTERNAL` (os domínios da D4 também, para que as tarefas novas só acrescentem códigos); tarefas posteriores só acrescentam códigos no enum da própria crate e as frases no arquivo `apps/desktop/src/i18n/errors/<domínio>.ts`, tipado como `Record<<Domínio>ErrorCode, string>` (o TypeScript falha se faltar tradução).
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
| `.mrpack` e zip da CurseForge (P1, v1) | `packwiz modrinth export` / `packwiz curseforge export` em staging, com validação do arquivo gerado. |
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
# Segredos e arquivos de ferramentas (decisão D21; R5A §2.1, R5B §11):
/kubejs/config/web_server.json
/.probe/
/.vscode/
/local/kubejs/
/config/spark/*.sparkprofile
/config/spark/*.sparkheap
/config/spark/tmp*
# Só para o pacote para servidor (§12.1); nunca vai para o jogador:
/server-overrides/
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

`.gitignore`: os mesmos padrões, **exceto** `/.warden/`, `/CHANGELOG.md`, `/README.md` e `/server-overrides/` (que são versionados).

O token do servidor web do KubeJS 7 (`kubejs/config/web_server.json`) é aleatório e a varredura de segredos por padrão (§11.1) não o reconheceria; por isso ele fica ignorado no pack e na captura, e a publicação recusa o arquivo pelo nome se ele aparecer no índice por qualquer motivo.

**Bloco obrigatório:** antes de qualquer `packwiz refresh`, a `PackTransaction` confere que o `.packwizignore` contém as quatro linhas do bloco obrigatório e, se faltar alguma, acrescenta (avisando na interface). Isso vale também para packs importados cujo usuário recusou os arquivos de controle padrão: o resto do `.packwizignore` dele é respeitado, mas `.warden/` e `CHANGELOG.md` nunca entram no índice.

Verificação de higiene (`pack_hygiene_scan`): procura entradas do índice e arquivos na pasta que casam com os padrões acima ou com as regras extras abaixo, e devolve itens com motivo:
- itens na raiz que não sejam `pack.toml`, `index.toml`, `options.txt`, `optionsof.txt`, `optionsshaders.txt`, `servers.dat`, arquivos de controle ou pastas de conteúdo conhecidas (`mods/`, `resourcepacks/`, `shaderpacks/`, `config/`, `defaultconfigs/`, `kubejs/`, `scripts/`, `datapacks/`, `openloader/`, `global_packs/`, `paxi/`, `server-overrides/`) — inclusive `local/`;
- arquivos de ferramentas e segredos listados no bloco "Segredos e arquivos de ferramentas" acima, mesmo que estejam fora do índice;
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
    pub game_dir: PathBuf,                       // instances/<id>/minecraft (ou bisect/, ou a temporária)
    pub java: JavaRuntime,                       // escolhido por warden-java
    pub memory_mb: u32,
    pub extra_jvm_args: Vec<String>,             // já validados
    pub system_props: Vec<(String, String)>,     // D4: -Dfml.queryResult=confirm, -Dfabric.noGui, log TRACE no perfil de desempenho
    pub quick_play: Option<QuickPlay>,           // D4: Singleplayer { world } (1.20+) | Multiplayer { host, port } (§7.8)
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
- **Política de seleção: o mais novo que funciona** (ADR-0029; primeira regra que se aplica): (a) Java escolhido pelo usuário em Ajustes do teste; (b) Forge ≤ 1.12.2 → 8; (c) Forge 1.16.5 com versão < 36.2.26 → Java 8 ≤ 8u312 (Temurin 8 da série compatível, ou aviso se não houver); (d) o major mais novo listado como compatível para a faixa de versão e o loader em `crates/warden-java/data/compatibility.toml` (dados versionados; partida = R2 §2.1: até 1.16.5 → 8; 1.17–1.20.4 → 17; 1.20.5–1.21.x → 21; 26.x → 25; uma faixa só ganha major mais novo depois de a matriz L-05 passar com ele); (e) faixa sem entrada: `javaVersion.majorVersion` do JSON da versão (16 → 17), ou 8 sem `javaVersion`. Exigências de mods (`[features] javaVersion`, `depends.java`) entram como verificação do diagnóstico (E-JAVA), não mudam a escolha sozinhas.
- **Motivo:** `java_choice` devolve `{ major, runtime, reason }`, com `reason` como código estável (`USER_CHOICE`, `FORGE_LEGACY_JAVA8`, `FORGE_1165_OLD`, `NEWEST_PROVEN_FOR_RANGE`, `NEWEST_AVAILABLE`, `FROM_VERSION_JSON`) traduzido pela interface. Ajustes do teste mostra "Por que não o Java N?" quando o escolhido não é o mais novo instalável.
- **Sempre a atualização mais recente** do major: a verificação roda junto com a de atualizações do pack (intervalo de Configurações) e ao preparar o teste; a atualização nova é instalada antes do teste, e a antiga é removida quando nenhum jogo a está usando.
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
- Sessão gravada em `instances/<id>/state/sessions/<data-hora>/` (`output.log` com tudo o que saiu dos pipes, `session.json` com resultado, duração, versão do pack e caminhos dos artefatos de crash). D4: `session.json` também guarda o hash da árvore do pack testada (commit ou árvore de trabalho), o resumo da lista de mods, o perfil do teste, o modo (normal, como o jogador recebe, perfil de desempenho, como servidor, rodada da busca), a **assinatura** do travamento (§9.6), o pico de RAM e os tempos até carregar e até entrar no mundo (§7.7). Retenção: sessões que fecharam normalmente ficam as 30 mais novas; as que travaram, até 500 MB por pack, com aviso antes de apagar.
- **Marcadores de "carregou" e "entrou no mundo"** por faixa de versão (os mesmos da matriz L-05, completados pelo spike S-R5-3): carregou = marcadores do S1 §3.4 (atlas de texturas e `Sound engine started`); entrou no mundo = `logged in with entity id` (igual de 1.7.10 à versão mais nova); servidor pronto = `Done (…)! For help, type "help"`.
- **Leitura de memória:** se os argumentos do teste tiverem `-XX:+PerfDisableSharedMem` ou `-XX:-UsePerfData` (comuns nas "flags do Aikar"), o Warden avisa que a memória da JVM não poderá ser lida e oferece remover (ADR-0038).

### 7.5 Forge

O instalador do Forge pede para não automatizar o download. O Warden automatiza (sem isso o requisito é inviável), baixa sempre do Maven oficial, não espelha instaladores e mostra em Sobre o crédito ao Forge com o link de apoio do próprio instalador (R2 §3.4).

### 7.6 Servidor local (ADR-0032)

`warden-server`, usado por "Testar como servidor" (T27), pela validação do pacote para servidor (§12.1) e pela busca do culpado (§9.7), sempre a pedido do usuário:

- **Pasta:** `instances/<pack-id>/server/` (separada da instância de cliente). O pack é materializado pelo mesmo algoritmo da §8.2 com o filtro inverso: entram os itens cujo lado não é `client`.
- **Instalação** (idempotente, cacheada por versão): vanilla pelo `downloads.server` do JSON da versão; Fabric pelo jar de servidor do Fabric meta (`/v2/versions/loader/<mc>/<loader>/<installer>/server/jar`); Forge e NeoForge pelo instalador oficial com `--installServer <pasta>` (Forge 1.17+ e NeoForge geram `run.bat`/`run.sh` e `*_args.txt`; Forge ≤ 1.16.5 gera um jar executável; Forge 1.7.10/1.12.2 usa o instalador legado e Java 8). O motor do launcher não instala servidor (R5B §4.2).
- **EULA:** `eula.txt` com `eula=true` só é gravado se `settings.json` tiver o aceite (`eulaAcceptedAt`), dado pelo diálogo da T27 com o link https://aka.ms/MinecraftEULA. Revogar apaga o aceite; o próximo servidor pergunta de novo.
- **`server.properties` controlado:** `server-ip=127.0.0.1`, `server-port` = porta livre testada antes, `online-mode=false`, `enable-rcon=false`, `enable-query=false`, `motd` com o nome do pack; o resto vem do `server.properties` do pack, se houver, com essas chaves sempre sobrescritas.
- **Processo:** mesmo supervisor da §7.4 (Job Object, pipes, decodificação), com stdin em pipe para comandos (`reload`, `kubejs reload server_scripts`, `spark profiler …`). Pronto quando casar `Done \([\d.,]+s\)! For help, type "help"` (também aceita o sufixo ` or "?"` de versões antigas).
- **Parar:** `stop\n` no stdin; se não sair em 60 s, encerra o job. Ao fim do teste ou da rodada, o servidor sempre é parado; fechar o Warden com servidor aberto pede confirmação e encerra.
- **Memória:** antes de abrir, `server_plan` compara servidor + jogo com a RAM livre (API do sistema) e devolve o aviso; nada abre sem confirmação.
- **Cliente conectado:** `QuickPlay::Multiplayer { host: "127.0.0.1", port }` (§7.8); em versões anteriores a 1.20, `--server`/`--port`.

### 7.7 Desempenho do jogo (ADR-0038)

`warden-perf`, ligado ao supervisor pela `warden-app` (`test_hooks/perf.rs`), com amostras a cada 1 s enviadas como `OperationEvent::PerfSample`:

- **RAM do processo** (P0): Windows `GetProcessMemoryInfo` (`PROCESS_MEMORY_COUNTERS_EX`: `WorkingSetSize`, `PrivateUsage`) pelo handle que o supervisor já tem, e `PeakProcessMemoryUsed` do Job Object; Linux pelo `/proc/<pid>/status`. Código `unsafe` isolado em `warden-perf/src/process/windows.rs`, com comentário `SAFETY`.
- **Tempos** (P0): do início do processo até o marcador de "carregou" e até "entrou no mundo" (§7.4), gravados na sessão e comparados com a última sessão de outra versão do pack.
- **Heap da JVM** (P1, depois do spike S-R5-2): leitor próprio do arquivo `hsperfdata` (Windows: *file mapping* nomeado `hsperfdata_<usuário>_<pid>` e o arquivo em `%TEMP%\hsperfdata_<usuário>\<pid>`, procurado por `hsperfdata_*\<pid>` para tolerar nomes com acento; Linux: `/tmp/hsperfdata_<usuário>/<pid>`). Cabeçalho `0xcafec0c0`; heap usada = soma de `sun.gc.generation.*.space.*.used`; máximo = `-Xmx` passado pelo Warden; coletas = `sun.gc.collector.N.invocations`/`time`. Sem o arquivo (OpenJ9, flags que desligam), a amostra vai sem heap e a interface diz o motivo.
- **Achados de memória** (P1): `W_LOW_HEAP` quando a heap depois das coletas fica acima de 90% do máximo por mais de 60 s; `I_HEAP_OVERSIZED` quando nunca passa de 40% com mais de 8 GB configurados.
- **Tempo por mod** (P1, "Testar com perfil de desempenho"): Forge 1.12.2 lê `Bar Step: <fase> - <mod> took X.XXXs` do `debug.log`; Forge 1.7.10 lê os pares `Sending event`/`Sent event` do `fml-client-latest.log`; Forge/NeoForge modernos ligam o nível `TRACE` só neste teste (`-Dforge.logging.debugFile.level=trace`) e casam `Loading mod instance`/`Loaded mod instance` e `Firing event`/`Fired event` por thread; NeoForge também `Mod '{}' took {} to run a deferred task.`; Fabric não tem dado por mod (o resultado diz isso). No NeoForge, nunca passar config de log4j própria (some o `debug.log`, R5A §7.1).
- **spark** (P1): os `.sparkprofile`/`.sparkheap` salvos na instância são lidos com `prost` a partir dos `.proto` públicos do spark (copiados com a licença GPL-3.0 registrada em `THIRD_PARTY.md`; o app é privado, ADR-0004), agrupando os frames por jar pelo índice de pacotes (§9.6). No servidor local, "perfil de desempenho" envia `spark profiler start --timeout 60 --save-to-file` pelo stdin. "Abrir no visualizador do spark" abre o arquivo local no navegador padrão; nunca há upload.

### 7.8 Perfis do teste e Quick Play

- Perfis guardados em `packs.json`, por pack: `{ id, nome, memoria_mb, java (auto ou caminho), jvm_args, opcionais, janela, ao_abrir: "menu" | "mundo" }`, com um "Padrão" que não pode ser apagado. O perfil ativo vai em `game-state` para o cabeçalho.
- **Quick Play:** a partir do 23w14a (Minecraft 1.20), `--quickPlaySingleplayer <pasta do mundo>` abre um mundo direto e `--quickPlayPath <arquivo>` faz o jogo gravar um JSON quando entra no mundo (sinal estruturado de "entrou"); `QuickPlay::Multiplayer` usa `--quickPlayMultiplayer host:port`. Antes de 1.20, só o multiplayer funciona, por `--server`/`--port` (removidos no 23w14a). O adaptador do motor converte (o portablemc já tem `set_quick_play` e a correção para versões antigas, R5B §4.1).

## 8. Instância de teste: materialização e captura

### 8.1 Pasta

`instances/<pack-id>/minecraft/` é o `gameDir`. `instances/<pack-id>/state/` guarda `manifest.json`, `baseline/`, `sessions/`, `optional-choices.json` e, durante uma busca do culpado, `bisect.json`. Nada de `state/` fica dentro do `gameDir`. D4: `instances/<pack-id>/bisect/` (instância da busca do culpado, recriada a cada rodada e apagada no fim), `instances/<pack-id>/server/` (servidor local) e `cache/tmp/player-test-<ULID>/` (instância temporária do "Testar como o jogador recebe").

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
10. **Ferramentas do jogador** (D4, ADR-0033): a lista vem de `.warden/project.toml` (`[player-tools]`). Na instância de teste normal, os itens marcados como "fora dos testes" (o Crash Assistant) são pulados como se fossem opcionais desligados; na busca do culpado, todas as ferramentas do jogador são puladas, a menos que estejam entre os suspeitos; no "Testar como o jogador recebe", nada é pulado (é o packwiz-installer real que instala).
11. **Subconjuntos da busca do culpado** (D4): a materialização recebe a lista de itens ligados e recria `mods/` com **links físicos** a partir de `cache/downloads/` (no mesmo volume; senão, cópia). Configs vêm do pack a cada rodada; mundos, de cópias descartáveis.

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
kubejs/config/web_server.json  .probe/  .vscode/  local/kubejs/
config/spark/*.sparkprofile  config/spark/*.sparkheap  config/spark/tmp*
packwiz.json  packwiz-installer*.jar
*.bak  *.old  *.tmp  servers.dat_old
```

(`local/` inteira já é ignorada na captura; `local/kubejs/` aparece explícita porque o KubeJS 7 grava ali exportações e scripts locais.) Os `*-N.toml.bak` que o NeoForge cria ao corrigir uma config caem em `*.bak`.

A lista vive em `crates/warden-instance/data/capture-ignore.txt` (formato gitignore), com testes.

### 8.5 Testar como o jogador recebe (D4; R5B §4.3)

1. Garantir no cache (`cache/installer/`) o `packwiz-installer-bootstrap.jar` e o `packwiz-installer.jar` com versão e SHA-256 fixados no código (baixados da URL de download do Release, que não passa pela API do GitHub).
2. Origem: o **link do pack** publicado (ADR-0028). Sem publicação, o Warden exporta o estado atual (§12) para `cache/staging/` e o serve por um servidor HTTP só em `127.0.0.1`, numa porta livre, durante o teste.
3. Pasta temporária `cache/tmp/player-test-<ULID>/`; rodar `java -jar bootstrap --bootstrap-no-update --bootstrap-main-jar <cache>/packwiz-installer.jar -g -s client <link>` com o Java do teste (sem janela).
4. Ler o `pack.toml` baixado e comparar o hash com o da última publicação; diferente → aviso de cache do GitHub (até 5 minutos, `cache-control: max-age=300`).
5. Falhas por mod da CurseForge bloqueado viram resultado do teste, não erro.
6. Abrir o jogo com o loader do `pack.toml` baixado, como numa sessão normal (mesma supervisão e diagnóstico). No fim, apagar a pasta temporária e manter os logs na sessão.

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
| `W_MIXIN_OVERLAP` | (P1, D4) Dois ou mais mods alteram o mesmo ponto do jogo com risco alto ou médio, depois dos rebaixadores (§9.6). Nunca bloqueia; conta só na categoria Mixins da nota de saúde. | aviso | jar |
| `W_CONFIG_RANGE` | (P1, D4) Valor de config fora da faixa ou dos valores permitidos que o próprio arquivo informa (§10.1). | aviso | arquivo do pack |
| `W_PERF_FLAGS` | (P1, D4) Argumentos do teste que impedem ler a memória da JVM (`-XX:+PerfDisableSharedMem`, `-XX:-UsePerfData`). | aviso | ajustes do teste |
| `W_UNDECLARED_DEP` | (P1, D4) Dependência não declarada descoberta pela busca do culpado ou pelo log (`NoClassDefFoundError` de uma classe de outro mod), guardada como aresta inferida do grafo. | aviso | log |

Dados curados em `crates/warden-diagnostics/data/` (`exclusive-categories.toml`, `known-conflicts.toml`, `obsolete.toml`, `mixin-known-compatible.toml`, `log-noise.toml`, `health-score.toml`), embutidos no binário com `include_str!` e validados por teste (esquema e IDs existentes). Atualização remota dos dados é P2. Achados de execução (`W_LOW_HEAP`, `I_HEAP_OVERSIZED`) vêm da `warden-perf` (§7.7).

### 9.3 Pós-crash

1. Coleta: código de saída, `output.log` da sessão, `logs/latest.log`, `logs/debug.log` (ou `logs/fml-client-latest.log` em Forge antigo), `crash-reports/*` e `hs_err_pid*.log` com data posterior ao início da sessão.
2. Limpeza: remove códigos `§x` e ANSI.
3. Catálogo de padrões em `crates/warden-diagnostics/data/log-patterns.toml`: cada padrão tem `id`, regex (crate `regex`), escopo (loader/faixa de versão), extratores de grupos e o `Finding` produzido. Ponto de partida: os 35 padrões de R2 §6.3 e as classes de problema do codex-minecraft (MIT, porte com atribuição em `THIRD_PARTY.md`). D4: o padrão de falha de Mixin aponta o mod dono da config citada pelo índice config → mod (§9.6); o padrão `Incorrect key {chave} was corrected from {valor} to its default` do NeoForge vira o achado "config resetada pelo jogo" (R5B §2.3); o Crash Assistant nunca é fonte (ADR-0033).
4. Evidência = arquivo + linha + trecho; achado mais provável primeiro (crash report > tela de erro do loader > exceção mais profunda > padrões genéricos).
5. Corpus de testes: logs reais em `crates/warden-diagnostics/tests/corpus/<caso>/` com o resultado esperado.
6. **Assinatura** (D4, §9.6): ao fim da análise, o achado principal gera a assinatura da sessão, gravada no `session.json`.

### 9.4 Redação de dados pessoais

`warden-diagnostics::redact` (usada antes de qualquer envio à IA, nas palavras de erro mandadas na busca de issues do GitHub e no "Copiar relatório"):

| Dado | Substituição |
|---|---|
| Nome de usuário do sistema em caminhos (`C:\Users\<x>\`, `/home/<x>/`) e a variável `USERNAME`/`USER` em qualquer lugar | `<usuário>` |
| Nome do jogador e UUID offline | `<jogador>`, `<uuid>` |
| Nome do computador (`COMPUTERNAME`/hostname) | `<computador>` |
| IPv4/IPv6 (exceto `127.0.0.1`/`::1`) | `<ip>` |
| E-mails | `<email>` |
| Padrões de segredo (`$2a$…`, `ghp_…`, `github_pat_…`, `AIza…`, `x-api-key`, `token=`) e os valores reais das chaves do cofre | `<segredo>` |

A redação é testada com casos positivos e negativos; o diálogo de consentimento mostra o texto **depois** da redação, e esse é exatamente o texto enviado.

### 9.5 IA (Gemini com ferramentas, P1; ADR-0030)

**API.** REST `generativelanguage.googleapis.com/v1beta/models/{modelo}:generateContent`, chave no cabeçalho `x-goog-api-key`, **sem estado** (o histórico inteiro da conversa é reenviado a cada rodada); a Interactions API com `store: true` não é usada (guarda dados no Google). Sem SDK: laço HTTP próprio em Rust pela `warden-http`.

**Modelo.** Configurável; o padrão é resolvido pela lista de modelos da conta (`models.list`): o Flash estável mais novo com `generateContent` e *function calling* (em 01/10/2026, `gemini-3.8-flash`), com o Flash-Lite como opção econômica. Nada fixo no código. Temperatura 1.0 (recomendação do Google para o Gemini 3).

**Ferramentas.** Declaradas como `tools[].functionDeclarations[]` com `parametersJsonSchema`. O registro de ferramentas fica na `warden-ai`; cada ferramenta lê dados por um trait implementado pela `warden-app` (`ai_tools/`), para a `warden-ai` não depender das crates de cima.

| Leitura (respondem na hora; tudo redigido pela §9.4) | Evidência citável |
|---|---|
| `get_pack_overview` (MC, loader, Java, memória, itens, nota de saúde, último teste) | `pack:overview` |
| `list_mods(filter?)`, `get_mod_details(mod_id)` (metadados do jar e da API, achados que o envolvem) | `mod:<id>`, `jar:<id>/<arquivo>`, `api:<fonte>/<id>` |
| `get_findings(kind?)` | `finding:<código>#n` |
| `list_sessions`, `get_session(id)`, `get_crash_report(session)`, `search_log(session, query)` (até 40 trechos), `read_log(session, file, from, to)` (até 300 linhas) | `session:<id>`, `crash:<session>`, `log:<session>/<arquivo>#L<n>` |
| `search_configs(query)`, `read_config(path)` (só com a permissão de configs da conversa) | `config:<path>#L<n>` |
| `get_history(limit)`, `diff_versions(a, b)` | `version:<a>..<b>` |
| `get_mixin_report(mod_id?)` (P1 com D-10), `get_dependents(mod_id)`, `why_in_pack(mod_id)`, `get_bisect_result` (P1 com D-12) | `mixin:<id>`, `graph:<id>`, `bisect:<id>` |
| `get_mod_changelog(mod_id, from, to)` (Modrinth `include_changelog`; CurseForge `/files/{id}/changelog` em HTML convertido) | `changelog:<id>@<versão>` |
| `search_mod_issues(mod_id, query)` (só com a permissão do GitHub da conversa): até 5 issues do repositório do mod, achado por `source_url`/`issues_url` (Modrinth) ou `links.sourceUrl`/`issuesUrl` (CurseForge); a consulta enviada é só `repo:<dono>/<repo> is:issue <termos>`; usa o token do GitHub quando existir (30 buscas/min) e cache; sem token, 10 buscas/min | `issue:<dono>/<repo>#<n>` |

**Propostas** (nunca têm efeito sozinhas): `propose_add_mod`, `propose_remove_mod`, `propose_update_mod`, `propose_change_version`, `propose_set_side`, `propose_edit_config(path, key, value)`, `propose_test_settings(memory, java, jvm_args)`, `propose_run_test`, `propose_start_bisect`. Cada proposta precisa citar pelo menos uma evidência válida; sem isso, o Warden devolve erro à IA e não mostra nada. `ai_proposal_apply` executa pelos mesmos serviços dos fluxos da interface (planos de adição, `config_write` com diferença, ajustes do teste), com ponto de segurança quando muda o pack.

**Laço.** Primeira rodada com as ferramentas de visão geral e log; changelog, issues e propostas entram quando a IA já citou mods suspeitos (`allowedFunctionNames`), para ficar na faixa recomendada de 10 a 20 ferramentas ativas. `toolConfig.functionCallingConfig.mode = VALIDATED` nas rodadas de ferramenta; `finishReason = MALFORMED_FUNCTION_CALL` é tratado como erro recuperável (uma nova tentativa). Cada `functionResponse` repete o `id` da chamada. As *thought signatures* exigem reenviar o `content` do modelo **inteiro e intacto** (nunca fundir partes). No máximo **8 rodadas de ferramenta** por pergunta e um teto de tokens por conversa; ao estourar, o Warden força `mode: NONE` e pede a resposta com o que já tem. Chamadas paralelas são suportadas; o spike S-R5-4 confere as assinaturas nesse caso.

**Resposta final.** JSON com `responseFormat`/`responseJsonSchema` (o `responseSchema` está obsoleto): `{ achados: [{ afirmacao, evidencias: [{ id, citacao }], confianca }], propostas: [...], semConclusao: bool }`. O Rust confere que cada `id` foi devolvido por uma ferramenta **nesta conversa** e que a `citacao` aparece literalmente no resultado daquela ferramenta; afirmação sem evidência válida vai com `verificada: false` e a interface a marca "não verificado". Todo mod citado precisa existir no pack ou na API. O *system prompt* manda citar a evidência, dizer "não sei" quando faltar e sugerir a busca do culpado; os achados determinísticos entram como fatos.

**Texto não confiável.** Resultados que contêm logs, issues ou changelogs vão delimitados e marcados como dados (injeção de instruções, OWASP LLM01); nenhuma ferramenta tem efeito sem o clique do usuário.

**Consentimento e transparência.** `ai_conversation_preview` devolve o texto inicial exato (visão geral, achados e o travamento escolhido, já redigidos), a contagem de tokens (`countTokens`) e as permissões; `ai_conversation_start` grava as permissões escolhidas (configs e GitHub) e só então faz a primeira requisição. Cada resultado de ferramenta enviado vira um bloco "Enviado à IA" com os bytes exatos; cada busca no GitHub, um bloco "Enviado ao GitHub". Origem do texto inicial: último travamento, outra sessão, um log escolhido no computador (só texto, até 20 MB lidos, mesma limpeza da §9.3 e redação da §9.4) ou só o pack.

**Conversas.** `ai/<pack-id>/<ULID>.json` nos dados locais (§13), nunca na pasta do pack: mensagens, chamadas e resultados **como enviados**, permissões, modelo, `usageMetadata` (tokens), propostas e o que foi aplicado, o hash da árvore do pack no início. Ao continuar, se o hash mudou, a interface avisa. `ai_conversation_delete` apaga o arquivo.

**Erros.** Tempo-limite de 90 s por requisição; `AI_KEY_INVALID`, `AI_QUOTA_EXCEEDED`, `AI_TIMEOUT`, `AI_BAD_RESPONSE`, `AI_TOOL_LIMIT`, `AI_PERMISSION_DENIED` (ferramenta não permitida nesta conversa, devolvida à IA, não ao usuário).

### 9.6 Índices, grafo, raio-x de mixins, nota de saúde e travamentos (D4; ADR-0034)

**Índices** (P0, `warden-jarmeta::index`), montados na passagem completa do diagnóstico e guardados em `cache/jarindex/<sha256 do jar>.json`:
- pacote → jar → modid, descendo nos jars aninhados (`META-INF/jars/`, `META-INF/jarjar/`); pacote ambíguo é atribuído ao jar de topo (o mod que o usuário adicionou);
- config de mixin (`*.mixins.json`, de `fabric.mod.json`, `MixinConfigs` do `MANIFEST.MF`, `[[mixins]]` do `neoforge.mods.toml`) → mod.
Usados pelo console agrupado (§9.8), pelo padrão de falha de Mixin (§9.3), pela dependência inferida da busca do culpado (§9.7) e pelo agrupamento dos perfis do spark (§7.7).

**Grafo** (`warden-diagnostics::graph`): nós = itens do pack e dependências embutidas (marcadas "dentro de X"); arestas tipadas obrigatória, opcional/recomendada, incompatível (jar, API, lista curada), embutida, fornece (`provides`), inferida (busca do culpado ou log). Consultas P0: dependentes transitivos (`graph_dependents`), cadeia até um item adicionado pelo usuário (`graph_why_in_pack`, a partir do histórico de adições ou, sem ele, de quem não é dependência de ninguém), bibliotecas órfãs (`graph_orphans`). P1: `graph_focus` devolve o subgrafo de até 2 níveis em volta de um item para o desenho (§18). Ciclos de dependência viram um só nó.

**Raio-x de mixins** (P1, `warden-mixin`; a implementação passa antes pelo spike S-R5-1 do intermed):
- Lê as configs de mixin (`package`, `mixins`/`client`/`server`, `priority`, `refmap`, `plugin`, `injectors.defaultRequire`) e os refmaps; abre cada classe de mixin com `cafebabe` (sem decodificar bytecode) e lê `@Mixin` (alvos e prioridade) em `RuntimeInvisibleAnnotations` e os injetores (`@Inject`, `@Redirect`, `@Overwrite`, `@ModifyArg(s)`, `@ModifyVariable`, `@ModifyConstant` e os do MixinExtras: `@WrapOperation`, `@ModifyExpressionValue`, `@ModifyReturnValue`, `@WrapWithCondition`, `@WrapMethod`) em `RuntimeVisibleAnnotations`, com o método-alvo e o `@At` traduzidos pelo refmap. Jars aninhados são obrigatórios (no Sodium NeoForge 26.3, todos os mixins estão no jar aninhado). `mods.toml` com parser TOML de verdade.
- **Ponto de aplicação** = classe-alvo + método-alvo + `@At` (tipo + alvo). Regras (R5A §5.5): `@Overwrite` de 2+ mods → alto; `@Redirect`/`@ModifyConstant` de 2+ mods no mesmo `@At` → alto; `@Overwrite` + injeção no corpo (`INVOKE`, `FIELD`, `CONSTANT`, `NEW`, `JUMP`) → médio; `@Redirect` + outro injetor no mesmo `@At` → médio; `@ModifyVariable`/`@ModifyArg(s)` de 2+ mods no mesmo `@At` → médio; `@Overwrite` + `HEAD`/`RETURN`/`TAIL` → baixo; só `@Inject` e os do MixinExtras → baixo.
- **Rebaixadores** (um nível, com o motivo): todos os mixins envolvidos têm `plugin` ("pode ser desligado pelo próprio mod"); o mixin de um mod referencia classes do outro, fica numa config/pacote `compat`, ou o mod declara o outro como dependência ou recomendação ("compatibilidade intencional"); o par está em `mixin-known-compatible.toml`. **Elevadores:** o par está em `known-conflicts.toml`; o último travamento citou uma das configs envolvidas (evidência = linha do log); `require`/`defaultRequire ≥ 1` no ponto disputado. MixinSquared presente baixa a confiança do raio-x inteiro (um mod pode cancelar mixins de outro).
- Ordem prevista de aplicação pela prioridade (menor aplica antes; padrão 1000). Alvos em outros mods (não só no jogo) entram naturalmente.
- Saída: inventário por mod (configs, classes, alterações por tipo, alvos) e sobreposições; `W_MIXIN_OVERLAP` para as de risco alto ou médio depois dos rebaixadores. Cache por conjunto de hashes dos jars em `cache/xray/`.

**Nota de saúde** (P1, `warden-diagnostics::health`): função pura sobre os achados, as sessões, o raio-x, as atualizações e os tempos, com pesos e tetos em `data/health-score.toml` (tabela da SPEC T14). Teste de propriedade: acrescentar um achado nunca aumenta a nota. Resultado `{ nota, faixa, linhas: [{ categoria, pontos, motivo, link }] }`, guardado por pack em `packs.json` para a coluna Saúde de Meus packs (sem recalcular ao listar).

**Assinatura e travamentos** (P0, `warden-diagnostics::signature`): assinatura = id da regra do catálogo + tipo de exceção + primeiro frame de mod normalizado (sem números de linha, endereços, coordenadas, UUIDs nem caminhos) + mods citados, com hash estável. `crashes_list` agrupa as sessões que travaram por assinatura, com contagem, primeira e última data, versões do pack (pelo hash da árvore registrado na sessão), a situação ("causa encontrada" quando há achado determinístico; "não voltou a acontecer desde a versão X" quando sessões posteriores daquela árvore fecharam normalmente) e as ligações com a busca do culpado e as conversas.

### 9.7 Busca do culpado (P1; ADR-0031)

`warden-bisect` (algoritmo puro) e `warden-app::bisect_session` (orquestração):

1. **Entrada:** grafo (§9.6) já com as arestas inferidas, a assinatura do problema (de uma sessão ou escolhida: travou ao abrir, travou no mundo, linha do log, modo assistido) e as listas de sempre ligados (loader e bibliotecas de que todos dependem) e de desligados (ferramentas do jogador, salvo suspeitas).
2. **Ordem topológica estável** do grafo de obrigatórias (desempate: bibliotecas primeiro, depois id em ordem alfabética); todo prefixo é fechado por dependência.
3. **Rodada 0:** pack inteiro, até 3 tentativas; estima a taxa de reprodução *p*. Se não reproduzir, para. Abaixo de ~30%, recomenda não seguir; entre 30% e 100%, cada "passou" é repetido `⌈ln 0,05 / ln(1−p)⌉` vezes.
4. **Rodada 1:** só os sempre ligados. Se falhar igual, a causa não é um mod.
5. **Busca binária no tamanho do prefixo** (`lo` = maior que passou, `hi` = menor que falhou) até `hi = lo + 1`; o item na posição `hi` é o último membro necessário.
6. **Minimização para pares** (variante do *ddmin*): repete a busca só entre os itens antes do encontrado, com ele sempre ligado, até o conjunto mínimo.
7. **Confirmação:** só o conjunto mínimo (deve falhar) e o pack sem ele (deve passar). Se o pack sem ele falhar igual, existe outro culpado: oferece continuar a busca sem o primeiro.
8. **Veredito de cada rodada** pela assinatura (§9.6): `Passed` (chegou ao marcador de sucesso e ficou estável por 20 s), `FailedSame`, `FailedOther` (inconclusiva, como o `skip` do `git bisect`), `Timeout`, e no modo assistido `UserYes`/`UserNo`/`UserSkip`. `NoClassDefFoundError`/`ClassNotFoundException` de um pacote de mod desligado vira aresta inferida e a rodada é refeita. "Congelou" = sem linha nova de log por N s depois do último marcador (N = 3× o maior intervalo da rodada de controle, mínimo 120 s). Monotonicidade quebrada (prefixo maior passa depois de um menor falhar) para a busca com explicação.
9. **Ambiente de cada rodada:** `instances/<pack-id>/bisect/` materializada com o subconjunto (§8.2 item 11); mundo de teste copiado para uma pasta descartável; cliente 1.20+ entra no mundo por Quick Play (§7.8); versões anteriores, ou problemas de servidor, usam o servidor local (§7.6) só depois do aceite do usuário; Forge 1.12.2 com `-Dfml.queryResult=confirm`; Fabric com `-Dfabric.noGui`. Marcadores por faixa vêm do spike S-R5-3.
10. **Estado** em `instances/<pack-id>/state/bisect.json` depois de cada rodada (ordem, `lo`, `hi`, resultados, assinatura, tempo); **pausar** espera o fim da rodada atual; **cancelar** encerra o jogo pelo Job Object, apaga `bisect/` e guarda o resultado parcial. Progresso por `OperationEvent::Round` e `game-state = bisecting`.
11. **Resultado** `{ culpados, confianca, rodadas: [...], duracao }`, ligado à assinatura em Problemas → Travamentos e disponível à IA (`get_bisect_result`).

### 9.8 Console agrupado (P1)

`warden-diagnostics::console`, sobre as `LogLine` do supervisor (o `output.log` continua bruto):
- **Atribuição** de cada linha a um mod, nesta ordem: jar no frame do stack trace (Forge 1.17+: `~[arquivo.jar%23NN!/:versão]`), módulo no frame (NeoForge 1.20.5+: `TRANSFORMER/modid@versão/`), nome do *handler* do mixin no Fabric (`handler$<uid>$<modid>$<método>`), "Mixins in Stacktrace", nome do logger (quando é um modid conhecido) e índice pacote → mod (§9.6). Sem sinal: grupo "Desconhecido"; classes do jogo e do loader: "Jogo e loader".
- **Normalização:** números, coordenadas, UUIDs, hexadecimais e caminhos viram marcadores (`Missing texture for {id} at {n}`); o hash do modelo agrupa repetições com contagem, primeira e última hora.
- **Stack trace** é um item só, recolhido, com a primeira linha de mod em destaque (pulando frames do Java, do jogo e do loader).
- **Ruído conhecido:** `data/log-noise.toml` (mesmo formato do catálogo de padrões) marca linhas inofensivas ("comum, geralmente inofensivo").
- A interface recebe os grupos já calculados, atualizados em lotes; nada de agrupamento no frontend.

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

### 10.1 Busca, formulário em camadas, padrões e validação (D4; R5B §2)

- **Busca em todas as configs** (P0, `warden-configs::index`): ao abrir o pack (e ao trocar a origem para a instância), varre `config/`, `defaultconfigs/`, `kubejs/config/`, `options.txt` e os demais arquivos de texto e guarda uma entrada por chave `{ arquivo, caminho da chave, valor, comentário, rótulo traduzido?, modid?, linha }`; arquivos sem parser são indexados por linha. Para 500 configs, ~50 mil entradas em memória. Busca por palavras, sem acento e sem diferenciar maiúsculas, com pontuação nome da chave > rótulo > comentário > valor, por um *matcher* aproximado (`nucleo-matcher`). Atualização incremental pelo mesmo observador de "alteração externa" (§15). `config_search` devolve os resultados agrupados por arquivo, com a linha.
- **Metadados em camadas** (P1) para cada chave, `ConfigField { caminho, tipo, valor, padrao?, faixa?, opcoes?, descricao?, rotulo?, confianca, origem }`, na ordem: (1) metadados explícitos do arquivo (`Range`, `Allowed Values`, `Default`, `[range: … default: …]`, `Min`/`Max`, `Valid values`, prefixo de tipo do `.cfg`), confiança alta; (2) tipo pelo valor; (3) frases do comentário (`Defaults to X`, `between X and Y`, cores `#RRGGBB`), confiança baixa, marcadas "deduzido"; (4) rótulo e ajuda do arquivo de idioma do jar (`warden-jarmeta::lang`), procurando as chaves dos padrões conhecidos (NeoForge `<modid>.configuration.<chave>` e `.tooltip`; AutoConfig `text.autoconfig.<nome>.option.<campo>`; owo `text.config.<nome>.option.<campo>`; MidnightLib `<modid>.midnightconfig.<campo>`), `pt_br.json` antes de `en_us.json`. Camadas 5 (bytecode) e 6 (dicionário curado) são P2.
- **Só texto** quando: a leitura de teste não volta byte a byte (o Warden lê, regrava em memória e compara), YAML, SNBT, scripts, `.json` de datapack, estruturas profundas (o nó vira "editar este trecho como texto") e arquivos acima de 2 MB.
- **Padrões** (P1, camada A): do comentário (`Default:` do NeoForge com faixa; `[default: …]` do `Configuration` do Forge antigo); `config_restore_default` grava só a linha da chave (ou o arquivo inteiro, com diferença). Camadas B e C (padrões gerados por servidor e por cliente limpos, em `cache/defaults/<hash do conjunto de mods>/`) são P2.
- **Validação de faixa** (P1): na passagem rápida do diagnóstico, cada valor é conferido contra a faixa e os valores permitidos do arquivo → `W_CONFIG_RANGE` com **Restaurar padrão** quando há padrão.

### 10.2 Scripts KubeJS e CraftTweaker (P1; ADR-0037)

`warden-scripts`:
- **Detecção:** versão do KubeJS (5, 6, 7, 8 por faixa) e do CraftTweaker pelos jars do pack (`warden-jarmeta`); a interface mostra "KubeJS 6 · Forge 1.20.1".
- **Trechos prontos** por versão em `crates/warden-scripts/data/snippets/*.toml` (KubeJS: `ServerEvents.recipes`, `ServerEvents.tags`, `StartupEvents.registry`, `ItemEvents.tooltip`; CraftTweaker: `craftingTable.addShaped` e afins).
- **IDs** de itens, blocos e tags dos jars (`assets/*/models/item/*.json`, `data/*/tags/**`, `data/*/recipes/**`), com cache por hash; itens registrados só por código ficam de fora (limitação documentada). Com o jogo aberto e o KubeJS 7, também `/api/registries/.../keys` e `/api/tags/...` do servidor web local.
- **Erros:** `logs/kubejs/{startup,server,client}.log` (KubeJS) e `logs/crafttweaker.log` ou `crafttweaker.log` na raiz (1.12.2; confirmado nos golden logs) da instância de teste, com arquivo e linha; com KubeJS 7 e o jogo aberto, `GET /api/console/server/errors` e o WebSocket de console.
- **Recarregar:** KubeJS 7 com o jogo aberto → `POST /api/reload/server` (ou `/startup`) em `127.0.0.1:<porta>`, com o token lido de `kubejs/config/web_server.json` **da instância de teste**, só no Rust (nunca vai para a interface nem para registros); servidor local aberto → `kubejs reload server_scripts` (KubeJS 6), `reload` (KubeJS 7 e CraftTweaker moderno) pelo stdin; outros casos → o comando certo para copiar. CraftTweaker 1.12.2 não tem recarga oficial: a interface diz "reinicie o jogo".
- **Editor:** CodeMirror 6 com `@codemirror/lang-javascript` (MIT); ZenScript por `StreamLanguage` com gramática própria baseada na TextMate MIT do `Yesterday17/ZenScript` (atribuição em `THIRD_PARTY.md`); autocompletar de IDs por `@codemirror/autocomplete`. Autocompletar completo com `tsc` 7 `--lsp --stdio` e ProbeJS é P2.
- **Abrir no VS Code:** procura o `code` no PATH e nos caminhos padrão do Windows; abre a pasta da instância de teste.

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
- **Versão final:** marcar uma versão salva como final cria `refs/warden/final/vX.Y.Z` apontando para o commit da versão (não muda a árvore nem conta como alteração); desmarcar apaga a referência, só enquanto a versão não foi publicada.

### 11.1 Publicação no GitHub (ADR-0028)

O GitHub distribui o pack: os jogadores apontam o packwiz-installer-bootstrap para `https://raw.githubusercontent.com/<dono>/<repo>/main/pack.toml` e recebem só o que foi publicado. A branch de trabalho local **nunca** é enviada.

1. **Plano (`publish_plan`)** para uma versão final:
   - árvore da versão lida do commit da tag e montada em staging pela exportação (§12, origem "versão salva"): `pack.toml`, `index.toml` e os arquivos do índice;
   - higiene (§6.4) e **varredura de segredos** na árvore (padrões da tabela da §9.4, os valores atuais do armazenamento de segredos e, pelo nome, `kubejs/config/web_server.json`): achado bloqueia com `PUBLISH_SECRET_FOUND`, apontando arquivo e linha;
   - avisos (não bloqueiam): mods da CurseForge com distribuição bloqueada, com a oferta de troca quando o mesmo arquivo existe no Modrinth (`version_files` por SHA-1); versão não testada (nenhuma sessão de teste "fechou normalmente" cuja árvore do pack, registrada na sessão, seja igual à da versão); diagnóstico rápido com erros;
   - **notas da versão**: o gerador de changelog com base na **última versão publicada** (não na última salva), com "Atenção" quando houver remoção perigosa (SPEC T16), mais as notas livres.
2. **Publicar (`publish_run`)**:
   - acrescenta à árvore em staging o `CHANGELOG.md` de publicação (notas de todas as versões publicadas, a mais nova no topo) e `.gitattributes` com `* -text`;
   - cria um commit com essa árvore na **linha de publicação** local `refs/warden/publish/main` (pai = publicação anterior; mensagem "<Nome do pack> X.Y.Z") e a tag anotada local `refs/warden/publish-tags/vX.Y.Z` com as notas;
   - envia com `git2`: `refs/warden/publish/main:refs/heads/main` e `refs/warden/publish-tags/vX.Y.Z:refs/tags/vX.Y.Z`, com credencial em memória (`x-access-token` + token do armazenamento de segredos, §14). O token nunca vai para a URL do remoto nem para `.git/config`;
   - cria a **GitHub Release** (`POST /repos/{dono}/{repo}/releases`, `tag_name = vX.Y.Z`, `name = "<Nome do pack> X.Y.Z"`, `body` = notas);
   - devolve o link do `pack.toml`, o passo a passo para jogadores e o texto das notas.
   Uma falha antes do envio não deixa nada no remoto; uma falha depois do envio e antes da Release deixa a publicação marcada como "Release pendente", e o próximo `publish_run` só cria a Release.
3. **Repositório (`github_setup`, primeira publicação):** `GET /user`; criar com `POST /user/repos` e `private: false` (padrão, recomendado: o link só funciona para jogadores em repositório público) ou `private: true` ("só backup"); ou vincular um existente (`GET /repos/{dono}/{repo}` informa a visibilidade). O repositório e a visibilidade ficam em `packs.json`.
4. **Estado:** "publicada" = existe `refs/warden/publish-tags/vX.Y.Z`. As tags de trabalho `vX.Y.Z` (locais) e as de publicação (remotas) não colidem, porque as locais nunca são enviadas.
5. **Divergência:** o `main` remoto diferente da última publicação local → `REMOTE_DIVERGED`; "substituir o GitHub" consulta o commit remoto e só força o envio se ele ainda for o esperado (equivalente ao `--force-with-lease`; como o `git2` não oferece a operação atômica, existe uma pequena janela de corrida, aceitável para um único usuário).
6. A API do GitHub usa `warden-http`; o `raw.githubusercontent.com` pode levar alguns minutos para refletir a publicação, e a interface avisa isso.

## 12. Exportação

1. Verificação de higiene (§6.4) e diagnóstico rápido.
2. Cópia de staging do pack (`cache/staging/<operação>/`), só com `pack.toml`, `index.toml`, `.packwizignore` e os arquivos não ignorados. Origem: a árvore de trabalho (estado atual) ou a árvore de uma versão salva, lida da tag pela `warden-versioning` (usada pela publicação, §11.1, e por "Exportar esta versão").
3. `packwiz refresh --build` na cópia; conferir zero diferença em relação ao pack (exceto hashes internos que o `--build` acrescenta quando `no-internal-hashes` estiver ligado).
4. Pré-visualização a partir do índice (tamanhos, contagens, alertas).
5. Saída: pasta (cópia de `pack.toml`, `index.toml` e dos arquivos do índice) ou `.zip` (crate `zip`, entradas de diretório presentes, ordem determinística, datas fixas, sem bits de permissão estranhos — corrige a issue #115 do packwiz).
6. Conformidade (nos testes e opcionalmente no app): packwiz-installer instala a partir da saída numa pasta vazia.

O validador de `.mrpack` (E-02) aceita `env: "unknown"` e outros valores fora da especificação **na leitura** (a importação, §12.2, os encontra em arquivos reais) e **nunca** os gera.

### 12.1 Pacote para servidor (P1; ADR-0035)

`warden-export::server_pack`, a partir da mesma cópia de staging:
1. **Conteúdo comum às duas variantes:** itens cujo lado não é `client`; `config/`, `defaultconfigs/`, `kubejs/` sem `client_scripts/`, `scripts/`, datapacks globais, `server.properties` de referência (se o pack tiver um) e o conteúdo de `server-overrides/`; fora `options.txt`, `servers.dat`, `resourcepacks/`, `shaderpacks/`. `user_jvm_args.txt` com `-Xms`/`-Xmx` recomendados (comentado como o do instalador). `start.bat` e `start.sh` mínimos e próprios (inspirados nos casos tratados pelo ServerPackCreator, LGPL-2.1, sem copiar código): verificar Java, instalar o loader se faltar (`--installServer` ou o jar do Fabric; para Forge 1.17+/NeoForge, opcionalmente o `server.jar` do ServerStarterJar), **perguntar** a EULA a quem roda e iniciar.
2. **Variante "pelo link do pack"** (padrão): os scripts rodam `java -jar packwiz-installer-bootstrap.jar --bootstrap-no-update --bootstrap-main-jar packwiz-installer.jar -g -s server <link do pack>` antes de iniciar; o zip leva os dois jars fixados. Exige publicação (ADR-0028).
3. **Variante "mods dentro do zip":** os jars vêm do cache de downloads, conferidos pelo hash do índice; mods da CurseForge bloqueados passam pelo fluxo da T20; arquivos de terceiros só entram depois da confirmação de licença.
4. **Validação opcional** (com a EULA aceita no Warden): descompactar em `cache/tmp/`, rodar o ciclo do servidor local (§7.6) até "Done" e parar; o resultado vai para a interface ("o servidor abriu em 34 s" ou o trecho do log que falhou).

### 12.2 Importação (P1; ADR-0035)

`warden-import`, sempre gerando um pack novo numa pasta escolhida e nunca alterando a origem:
- **Detecção** pelo conteúdo: `pack.toml` (vai para T04), `modrinth.index.json`, `manifest.json`, `instance.cfg` + `mmc-pack.json`, `minecraftinstance.json`.
- **`.mrpack`:** zip aberto com proteção contra *zip slip* (`files[].path` sem `..`, caminho absoluto ou letra de drive); `dependencies` → `pack.toml`; cada `files[]` do `cdn.modrinth.com` → metafile Modrinth (projeto e versão vêm do caminho da URL; senão, `POST /v2/version_files` por `sha1`, em lote); outros hosts → metafile em modo `url` com o hash; `env` → `side` pela tabela da §6.2 (valores desconhecidos → `both` com aviso); `optional` → `[option] optional = true` com `default = true` marcado para revisão; `overrides/` → arquivos do pack; jars em `overrides/mods/` identificados por hash no Modrinth e na CurseForge ou mantidos como arquivo local com aviso de licença; `client-overrides/` entra com aviso; `server-overrides/` → `server-overrides/` do pack.
- **Zip da CurseForge:** `packwiz curseforge import` numa cópia de staging (exige a chave; sem ela, `IMPORT_CURSEFORGE_KEY_MISSING` sem nenhuma requisição); depois, o lado é corrigido pelo Modrinth (SHA-1 → `/v2/version_files`) e os opcionais desligados viram revisão.
- **Prism/MultiMC:** `mmc-pack.json` dá Minecraft e loader; `mods/.index/*.pw.toml` viram metafiles quase direto (sem os campos `x-prismlauncher-*`); jars sem metadados são identificados por hash; `instance.cfg` (`MinMemAlloc`, `MaxMemAlloc`, argumentos) vira um perfil do teste (§7.8); `options.txt` e `servers.dat` são perguntados.
- **Higiene obrigatória** antes de gravar: a verificação da §6.4 e a lista de ignorados da captura (§8.4) removem `.mixin.out/`, `xmcl.json`, `mods/.connector/`, logs, caches e mundos; o resultado lista o que foi convertido, o que ficou como arquivo local, o que precisa de revisão e o que não entrou.
- Criação do pack pela mesma rotina do T03 (git, arquivos de controle, `.warden/`), seguida de `packwiz refresh` e `check_conformance`.

## 13. Layout em disco

Identificador do app: `dev.kriticales.warden`.

| O quê | Windows | Linux | Conteúdo |
|---|---|---|---|
| Packs (projetos) | escolhido pelo usuário; padrão `%USERPROFILE%\Documents\Warden\` | `~/Documentos/Warden/` ou `~/Documents/Warden/` (pasta de documentos do sistema) | Um subdiretório por pack (repositório git). |
| Configuração | `%APPDATA%\dev.kriticales.warden\` | `~/.config/dev.kriticales.warden/` | `settings.json` (com `schemaVersion`; D4: `eulaAcceptedAt`), `packs.json` (registro: id → caminho, último teste, preferências de teste do pack e, na D4, os perfis do teste e a última nota de saúde, repositório e visibilidade no GitHub), `.env` (só se o usuário escolher guardar as chaves em arquivo; §14). |
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
ai/<pack-id>/                         conversas com a IA (§9.5)
instances/<pack-id>/bisect/           instância da busca do culpado (recriada a cada rodada; apagada no fim)
instances/<pack-id>/server/           servidor local do pack (§7.6)
cache/jarindex/                       índices pacote → mod e config de mixin → mod, por hash do jar (§9.6)
cache/xray/                           raio-x de mixins por conjunto de jars (§9.6)
cache/installer/                      packwiz-installer-bootstrap e packwiz-installer fixados (§8.5)
cache/server-installers/              instaladores e jars de servidor por loader e versão (§7.6)
logs/                                 registros do app (§16)
perf/<pack-id>.jsonl                  (1.1, gravado desde a v1) uma linha de métricas por teste; não é podado (§21.6)
player-reports/<pack-id>/<ULID>/      (1.1) travamentos de jogadores: só a cópia redigida e o resultado (§21.3)
cache/security/<sha256>.json          (1.1) resultado da leitura de segurança de cada jar, com a versão da lista (§21.1)
```

Dentro de cada pack:

```
pack.toml  index.toml  .packwizignore  .gitignore  .gitattributes  CHANGELOG.md
mods/  resourcepacks/  shaderpacks/  config/  defaultconfigs/  kubejs/  …    (conteúdo)
.warden/project.toml                  id do pack, schemaVersion, avisos do diagnóstico ignorados (P1), ferramentas do jogador (D4),
                                      origem dos arquivos locais vindos de importação (gancho 1.1)
.warden/mods.toml                     (1.1) notas e grupos dos mods (§21.4, ADR-0044)
.warden/trust.toml                    (1.1) arquivos em que você confiou, por hash (§21.1)
server-overrides/                     (opcional) arquivos só do pacote para servidor; fora do índice (§6.4, §12.1)
.git/
```

`.warden/` é versionado no git e ignorado pelo packwiz. Dados de máquina **nunca** ficam em `.warden/`: as preferências de teste de cada pack (memória, Java escolhido, argumentos JVM) ficam em `packs.json`, e as escolhas de opcionais, a linha de base e as sessões ficam em `instances/<id>/state/`. Assim, ajustar a memória do teste não conta como alteração não salva do pack.

`PackId` é um ULID gerado na criação/importação e gravado em `.warden/project.toml`; instâncias, caches e registros usam esse id, não o caminho.

## 14. Segredos

ADR-0025 (substitui a ADR-0017).

- `warden-secrets` expõe o trait `SecretStore` com duas implementações de produção, escolhidas pelo usuário em Configurações → Chaves e contas e gravadas em `settings.json` como `secretsBackend`:
  - **`keyring` (padrão):** Gerenciador de Credenciais no Windows, Secret Service no Linux; serviço `dev.kriticales.warden`, contas `curseforge-api-key`, `gemini-api-key`, `github-token`.
  - **`envfile`:** arquivo `.env` na pasta de configuração (§13), com `CURSEFORGE_API_KEY`, `GEMINI_API_KEY` e `GITHUB_TOKEN` (aspas simples, como o `.env` de desenvolvimento, porque a chave da CurseForge começa com `$2a$`). Escrita atômica (`.warden-tmp` + renomeação); no Linux, permissão `0600`. Leitura tolerante (linhas desconhecidas são preservadas e ignoradas).
- **Troca de modo** (`secrets_backend_set`): para cada segredo, grava no destino, relê para conferir e só então apaga da origem; ao fim, grava o modo novo em `settings.json`. Falha no meio: o modo continua o antigo, e o que já foi copiado ao destino é apagado. Voltar para o cofre apaga o `.env`.
- Há também uma implementação de teste em arquivo temporário, selecionável só em build de debug por `WARDEN_SECRET_BACKEND=file:<pasta>`, usada pelos testes e E2E no WSL e no runner Linux, onde não há Secret Service. O teste com o cofre real roda na CI Windows.
- Valores circulam como `secrecy::SecretString` (o `Debug` imprime `[REDACTED]`), são lidos só no momento do uso e nunca vão para a interface: a interface só conhece `secrets_status() → { backend, curseforge: bool, gemini: bool, github: bool }`.
- Desenvolvimento: só em build de debug, se o armazenamento escolhido não tiver a chave, `warden-secrets` lê `CURSEFORGE_API_KEY`, `GEMINI_API_KEY` e `GITHUB_TOKEN` do ambiente. `cargo xtask dev` e `cargo xtask test-network` carregam o `.env` do **repositório** principal (descoberto por `git rev-parse --git-common-dir`, sem copiar o arquivo e sem imprimir valores). Esse `.env` de desenvolvimento não tem relação com o `.env` opcional da pasta de configuração. Em build de release a leitura do ambiente não existe (`#[cfg(debug_assertions)]`).
- CI: testes de rede usam o segredo `CURSEFORGE_API_KEY` do GitHub Actions, só no workflow agendado/manual.
- Ao passar a chave para o sidecar, só pela variável `WARDEN_CURSEFORGE_API_KEY` do processo filho; a linha de comando e o ambiente do filho nunca são registrados.
- A varredura de segredos antes de publicar (§11.1) e a redação (§9.4) usam também os valores atuais do armazenamento escolhido.

## 15. Concorrência, travas e cancelamento

- **Runtime:** Tokio (o do Tauri). Trabalho bloqueante (hash de arquivos grandes, zip, `git2`, leitura de jars) vai para `spawn_blocking`.
- **Travas por pack** (`PackLocks` em `warden-app`): `RwLock` por `PackId`. Leituras (inventário, detalhes, diagnóstico rápido) pegam leitura; mutações pegam escrita. A espera aparece em Tarefas como "Aguardando outra operação neste pack".
- **Registro de operações** (`OperationRegistry`): toda operação longa recebe `OperationId`, `CancellationToken`, tipo, pack e progresso; `operation_cancel` aciona o token. Operações que não podem ser canceladas no meio (gravação final de uma transação) ignoram o cancelamento só nesse trecho.
- **Um jogo por vez** no app (estado global em `warden-app`); a trava do pack **não** fica presa enquanto o jogo roda, só durante a materialização. A busca do culpado e o servidor local contam como "jogo" para essa regra (D4); a busca segura a trava de leitura do pack só ao calcular a ordem, e cada rodada trabalha numa cópia (`bisect/`).
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
- CurseForge: chave do armazenamento de segredos (§14) no cabeçalho `x-api-key`; lote (`POST /v1/mods`, `POST /v1/mods/files`, `POST /v1/fingerprints/432`); concorrência máxima 4; **nenhuma resposta da API é persistida** (cache só em memória durante a sessão; `downloadUrl` nunca gravada). Os arquivos de mods baixados para a instância ficam no cache de downloads por hash, como fazem o packwiz e o Prism.
- **Busca combinada** (`warden-project::search`, ADR-0027): consulta Modrinth e CurseForge em paralelo, com tempo-limite por fonte e paginação independente por fonte; intercala os resultados pela posição em cada fonte e entrega páginas de 20 itens já combinados. Deduplicação na lista: mesmo autor (Modrinth `author`; CurseForge `authors[].name`) e mesmo slug ou nome normalizados (minúsculas, sem acentos, espaços e pontuação). Na pré-visualização, a SHA-1 do arquivo da versão escolhida é comparada entre as fontes (Modrinth `files[].hashes.sha1`; CurseForge `hashes` com algoritmo 1) e a diferença vira aviso. O item unificado guarda as referências das duas fontes; a parte da CurseForge fica só em memória. Sem chave (ou chave recusada), a fonte CurseForge nem é consultada; uma fonte com erro devolve resultado parcial com o aviso `SEARCH_SOURCE_UNAVAILABLE` (domínio `Project`).
- GitHub: API REST (`GET /user`, `POST /user/repos`, `GET /repos/{dono}/{repo}`, `POST /repos/{dono}/{repo}/releases`) por `warden-http` (§11.1). D4: busca de issues para a IA (`GET /search/issues?q=repo:<dono>/<repo>+is:issue+<termos>`), com o token quando existir (30 buscas/min) ou sem ele (10 buscas/min), e cache de 1 h por consulta; downloads de Release do packwiz-installer pela URL de download (que não usa a API).
- Instaladores de servidor (D4): Fabric meta (`/v2/versions/loader/<mc>/<loader>/<installer>/server/jar`), Maven do Forge e do NeoForge (`--installServer`), `downloads.server` do JSON da versão.
- Gemini (D4): só pelo laço da §9.5; `countTokens` antes da primeira requisição de cada conversa.
- (1.1) Conferência de segurança: Modrinth `POST /v2/version_files` (sha512, em lote); CurseForge `POST /v1/fingerprints/432` (murmur2) e conferência do SHA-1 de `file.hashes` e do `fileStatus`; só hashes saem (§21.1).
- (1.1) Manutenção: Modrinth `GET /v2/projects?ids=` e `POST /v2/version_files`; CurseForge `POST /v1/mods` e `POST /v1/mods/files`; os lotes omitem IDs inexistentes, então o resultado é sempre comparado com o pedido (§21.2). Parecidos: Modrinth `/v2/search` com `categories`, `versions`, loader e `disclosure_types!=archived`; CurseForge `/v1/mods/search` com `categoryIds`, `gameVersion` e `modLoaderType`.
- (1.1) Logs de jogadores: só leitura de texto cru, por HTTPS, até 10 MB, tempo-limite de 30 s: `api.mclo.gs/1/raw/<id>`, `pastebin.com/raw/<id>`, `paste.ee/r/<id>`, gist cru, 0x0.st, `hst.sh/raw/<key>`, `api.paste.gg/v1/pastes/<id>?full=true` (§21.3).

### 17.1 Descoberta, modpacks e imagens (D4)

- **Início da página de descoberta:** Modrinth `/v2/search` com `index=downloads` e `index=updated` e os facets do pack (codificados na URL; com colchetes crus a API responde 400), `/v2/tag/category`; CurseForge `POST /v1/mods/featured` e `/v1/categories?gameId=432&classId=6`; no máximo 3 requisições por fonte. Mapeamento curado de categorias em `crates/warden-discovery/data/categories.toml` (nome em português → categorias do Modrinth e IDs da CurseForge).
- **Ordenar por downloads ou atualização** mescla pelos números (`downloads` × `downloadCount`; `date_modified` × `dateModified`); relevância continua intercalada pela posição (ADR-0027). Paginação de 20 por fonte, pré-carregando a próxima a 70% da rolagem; teto da CurseForge de 10 mil resultados.
- **Modpacks:** Modrinth `project_type:modpack` e, para a lista de mods, as dependências `embedded` da versão do modpack (com `project_id`/`version_id`; arquivos fora do Modrinth só com `file_name`), resolvidas em lote por `/v2/projects?ids=` e `/v2/versions?ids=` (~4 requisições por modpack); CurseForge `classId=4471`. Detalhes (caminho, `env`, hashes) pelo `modrinth.index.json` ou `manifest.json` lidos **por partes** com `Range` (`warden-http::range_reader`: `Read + Seek` sobre `reqwest`, entregue ao crate `zip`; diretório central no fim e só a entrada desejada). Servidor sem `Range`: download completo até 200 MB, com aviso. Na CurseForge, nada é gravado em disco; `downloadUrl` nulo → "Este modpack não pode ser aberto por apps de terceiros".
- **Classes da CurseForge** confirmadas (R5B §6.2): 6 mods, 12 resource packs, 6552 shaders, 4471 modpacks, 6945 datapacks.
- **Imagens:** as do Modrinth vão direto (`img-src https:`); as da CurseForge passam pelo protocolo `warden-img://` servido pelo Rust (tempo-limite, tamanho máximo de 5 MB, só tipos de imagem, cache só em memória e `Cache-Control: no-store`), para o WebView não guardar dados da API no cache de disco.
- **Cache:** Modrinth em `metadata.sqlite` também para descrição e galeria (24 h); busca em memória por 5 minutos; CurseForge só em memória.
- **Kits e mods iniciais:** `crates/warden-project/data/{kits.toml,initial-mods.toml}` com IDs do Modrinth (nunca slugs) e IDs da CurseForge quando o item só existe lá (sem guardar mais nada da CurseForge); o `cargo xtask check-kits`, rodado no CI semanal e a cada mudança desses arquivos, consulta `GET /v2/project/{id}/version?loaders=…&game_versions=…` para cada faixa e marca item sem versão como "não disponível nesta versão" no relatório.
- Mojang, Fabric meta, Maven do Forge/NeoForge: catálogo com cache (manifesto revalidado a cada 6 h; JSON de versão por `sha1`).
- Sem internet: erros `NETWORK_UNAVAILABLE` (retryable) e uso do que houver em cache.
- Testes: em build de debug, a URL base de cada API pode ser trocada por variável de ambiente (`WARDEN_API_BASE_MODRINTH`, `WARDEN_API_BASE_CURSEFORGE`, `WARDEN_API_BASE_MOJANG`, `WARDEN_API_BASE_FABRIC`, `WARDEN_API_BASE_FORGE`, `WARDEN_API_BASE_NEOFORGE`, `WARDEN_API_BASE_ADOPTIUM`, `WARDEN_API_BASE_GITHUB`, `WARDEN_API_BASE_GEMINI`), apontando para o servidor local de fixtures usado pelos E2E (`apps/desktop/e2e/mock-server/`). Em release essas variáveis são ignoradas; por isso os E2E rodam sempre sobre um build de debug (`tauri build --debug`), e o instalador de release é validado pelos roteiros manuais.

## 18. Frontend

- **Camadas:** `routes/` (páginas, carregam dados e compõem features) → `features/<f>/components` → `features/<f>/hooks` (TanStack Query sobre `lib/ipc`) → `lib/ipc/bindings.ts` (gerado). Componentes não chamam `invoke` direto.
- **Estado do servidor:** TanStack Query com chaves `['pack', packId, área, …]`. O evento `pack-changed` invalida as chaves das áreas indicadas. Mutations nunca fazem atualização otimista de dados do pack (o disco é a verdade); mostram progresso e invalidam ao terminar.
- **Estado de interface:** Zustand só para o que não vem do backend (seleção na lista, filtros do console, painel aberto). Nada de dados do pack no Zustand.
- **Navegação (ADR-0026, ADR-0036):** nível do app sem barra lateral: `/` (Meus packs), `/configuracoes` (inclui "Sobre o Warden" como última seção), `/boas-vindas`, `/packs/importar` (D4). Nível do pack: layout `/packs/$packId` com cabeçalho fixo (← Meus packs, nome e "Editar informações", identificação, avisos passageiros, Salvar versão · N alterações, Testar ▾) e menu lateral com as 6 seções, registradas em `features/pack-editor/sections.ts`: `mods` (com `?ver=grafo` como modo, e `adicionar` dentro), `configs`, `problemas`, `ia` (com `ia/$conversaId` como página de detalhe), `historico`, `exportar`; a tela do teste é `/packs/$packId/teste` (com `?modo=busca` para a busca do culpado) e não aparece no menu. A rota `/packs/$packId/adicionar` liga o estado "menu recolhido" do layout (ícones com tooltip) enquanto está aberta. Tarefas é uma gaveta global aberta pelo indicador do rodapé. Nenhuma tela tem abas.
- **Rotas:** TanStack Router com rotas por arquivo (`src/routes/`); `routeTree.gen.ts` é gerado e **não** versionado (gerado antes de `typecheck`, `test` e `build`). Parâmetros tipados: `/packs/$packId/mods`, `/packs/$packId/configs?path=…`.
- **Formulários:** React Hook Form + Zod (esquemas em `features/<f>/lib/schemas.ts`, mensagens via i18n).
- **Listas longas:** TanStack Virtual.
- **Editor de configs:** CodeMirror 6 (`@codemirror/lang-json`, `@codemirror/legacy-modes` para TOML, properties e YAML, `@codemirror/merge` para diferenças; D4: `@codemirror/lang-javascript`, `@codemirror/autocomplete` e um `StreamLanguage` próprio para ZenScript, §10.2).
- **Conteúdo remoto:** Markdown com `react-markdown` + `rehype-raw` **antes** de `rehype-sanitize` (o corpo do Modrinth mistura Markdown e HTML; a ordem é obrigatória), com lista de permissão explícita: títulos, parágrafos, listas, tabelas, `a`, `img`, `code`/`pre`, `details`/`summary` e o atributo `align`; sem `script`, `style`, atributos `on*`, `form`, `object`/`embed`, `iframe` (vídeos viram miniatura com "Abrir no navegador"). HTML da CurseForge com DOMPurify e a mesma lista; links externos abrem no navegador (plugin `opener`, só `https:`).
- **Grafo (D4):** o grafo focado (até 2 níveis, dezenas de nós) é desenhado com HTML + SVG próprios, com cada nó como botão e as ligações também listadas em texto (acessível); "Ver o pack inteiro" usa Cytoscape.js (MIT) com o layout `dagre` (MIT), em canvas, sempre com a lista em texto ao lado.
- **Conversa com a IA (D4):** mensagens, blocos "Enviado à IA" (recolhidos, com os bytes exatos), selos de evidência e cartões de proposta são componentes do design system (`docs/design/HANDOFF.md`); nenhum HTML vindo da IA é renderizado como HTML (Markdown simples, higienizado como o conteúdo remoto).
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
| Leitura de `.class` (D4) | `cafebabe` (0BSD), com `ParseOptions::parse_bytecode(false)`; `noak` (MIT/Apache-2.0) como reserva | Lê as anotações visíveis, invisíveis e de parâmetro sem teto de versão de classe (R5A §5.7). O intermed (MIT) é avaliado no spike S-R5-1. |
| Segurança dos mods (1.1) | `cafebabe` (o mesmo), com o bytecode decodificado só nas classes candidatas; `zip` (já usado) | Leitura sem executar nada, limites contra *zip bomb* (§21.1). YARA-X foi considerado e ficou para depois (ADR-0040). |
| Protobuf (D4) | `prost` (Apache-2.0) | Perfis do spark a partir dos `.proto` públicos (§7.7). |
| Busca aproximada (D4) | `nucleo-matcher` (MPL-2.0) | Busca em todas as configs (§10.1). |
| Processo e memória (D4) | `windows` (já usado) no Windows; `/proc` no Linux | RAM do processo (§7.7); leitor de `hsperfdata` próprio (sem crate: as existentes são imaturas ou GPL). |

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
| Markdown/HTML | `react-markdown`, `rehype-raw`, `rehype-sanitize`, `dompurify` | Descrições de mods seguras (`rehype-raw` antes de `rehype-sanitize`). |
| Scripts (D4) | `@codemirror/lang-javascript`, `@codemirror/autocomplete` | Editor de scripts (§10.2). |
| Grafo inteiro (D4, P1) | `cytoscape`, `cytoscape-dagre` (MIT) | Só para "Ver o pack inteiro"; o grafo focado é próprio (§18). |
| Testes | Vitest, Testing Library, jsdom, `@tauri-apps/api/mocks`, WebdriverIO + `tauri-driver` | QUALITY §4. |
| Qualidade | ESLint (typescript-eslint `strictTypeChecked`, react-hooks, jsx-a11y, i18next), Prettier | QUALITY §2. |

## 20. Segurança do app Tauri

- **Capabilities** mínimas em `capabilities/default.json`: eventos e comandos do app, `opener` restrito a URLs `https:`, `log`. Diálogos de arquivo/pasta e "mostrar na pasta" são acionados pelo Rust dentro dos comandos (§4.1), não pela interface. **Sem** plugin `shell` na interface, **sem** `fs` na interface (todo acesso a arquivo passa por comandos com validação).
- **CSP:** `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https: warden-img:; connect-src ipc: http://ipc.localhost; object-src 'none'; frame-src 'none'` (D4: o esquema `warden-img:` serve as imagens da CurseForge, §17.1).
- Nenhum comando genérico do tipo "execute isto" ou "leia este caminho absoluto" (R4 §1.5).
- Todo caminho relativo validado por `resolve_inside`; extração de zip protegida contra *zip slip*; `.mrpack`/arquivos de terceiros nunca extraídos fora do destino.
- Conteúdo remoto (descrições) sempre higienizado; nada de `dangerouslySetInnerHTML` fora do componente `SafeHtml`.
- Downloads só por HTTPS, com hash verificado antes de uso.
- **Rede local (D4):** o servidor local escuta só em `127.0.0.1` (§7.6); o servidor de arquivos do "Testar como o jogador recebe" também (§8.5); chamadas ao servidor web do KubeJS saem só do Rust, só para `127.0.0.1`, com o token lido na hora e nunca exposto à interface (§10.2). Nenhuma porta é aberta para a rede.
- **IA (D4):** texto de logs, issues e changelogs é dado não confiável; nenhuma ferramenta da IA tem efeito sem o clique do usuário (§9.5).
- **1.1:** jars são lidos, nunca carregados nem executados pelo Warden; a leitura tem limites de profundidade de jars embutidos, de tamanho descompactado e de quantidade de entradas (proteção contra *zip bomb*). Logs baixados de links são texto não confiável: tamanho máximo, sem seguir redirecionamento para outro host que não esteja na lista, nunca renderizados como HTML.

## 21. Warden 1.1 "Profissional": módulos e ganchos na v1

ADR-0039 a ADR-0047; telas SPEC T28 a T33; tarefas ROADMAP W-01 a W-12 (fase 7, marco M6). Nada desta seção é construído na v1, exceto os **ganchos** da §21.7, que entram nas tarefas da v1 com custo mínimo.

### 21.1 Segurança dos mods (`warden-security`; ADR-0040, ADR-0041)

- **Entrada:** a lista de arquivos de mod do pack com, para cada um, o caminho do jar (cache de downloads, instância ou pasta do pack), a fonte declarada (`.pw.toml`: Modrinth com `mod-id`/`version`, CurseForge com `project-id`/`file-id`, link direto, arquivo local) e se veio de importação.
- **Leitura local** (`scan`, função pura sobre bytes): abre o zip com limites (profundidade de jars embutidos 3, 512 MB descompactados, 50 mil entradas); lê cada `.class` com `cafebabe` (pool de constantes sempre; bytecode só nas classes cujo pool cita algum alvo da lista, para não decodificar tudo); reconstrói textos de `new String(new byte[]{…})` (sequências `bipush`/`sipush` + `bastore`) e constantes em Base64; compara com `data/signatures.toml`. Cada regra tem `id`, `nivel` (`sinal` ou `atencao`), `descricao` em pt-BR, `fonte` (URL pública do caso) e um padrão: constantes (texto exato ou expressão regular), chamadas (`classe.método` com descritor), sequência de instruções, ou hash SHA-256 do arquivo inteiro. Regras de nível `atencao` só são aplicadas a arquivos sem confirmação oficial (decidido pelo chamador). Saída por jar: `{ sha256, achados: [{ regra, nivel, jar_interno?, classe, metodo?, trecho }] }`, guardada em `cache/security/<sha256>.json` com a versão da lista; uma lista nova invalida o cache.
- **Conferência com o oficial** (`verify`): recebe as respostas das APIs pelo trait `OfficialLookup` (implementado pela `warden-app` com `warden-modrinth` e `warden-curseforge`), para a crate não depender dos clientes. Modrinth: o sha512 do jar tem de estar no `POST /version_files` e pertencer ao `project_id` e à versão do `.pw.toml`; o host do `url` tem de ser `cdn.modrinth.com`. CurseForge: murmur2 → `exactMatches` → `file.id` igual ao `file-id` do `.pw.toml` e SHA-1 local igual ao de `file.hashes` (o murmur2 de 32 bits sozinho pode colidir); `fileStatus = 6` vira "marcado pela plataforma". Arquivo local e link: o jar é procurado nas duas plataformas pelo hash; se os metadados do jar dizem ser um mod e uma versão que existem numa plataforma com outro hash, o resultado é "não confere".
- **Veredito e achados:** combina leitura e conferência nos resultados da SPEC T28 e gera `E_SEC_SIGNATURE`, `E_SEC_MISMATCH`, `E_SEC_PLATFORM_FLAGGED`, `W_SEC_ATTENTION`, `W_SEC_UNVERIFIED`, `I_SEC_OFF_PLATFORM` (modelo da §9.1, com evidência obrigatória). Hashes em `.warden/trust.toml` (`[[trusted]] sha256, mod, data, motivo`) rebaixam o achado para "você confiou neste arquivo".
- **Onde roda:** `test_hooks/security.rs` no `before_launch` (só jars novos ou alterados desde o último teste); antes de gravar em `add/local.rs` e `add/url.rs` e na importação; sob demanda (`security_scan`); no `publish_plan` como checagem obrigatória (§21.7 item 6), baixando antes os jars que faltam no cache.
- **Lista de sinais:** `crates/warden-security/data/signatures.toml`, embutida com `include_str!`, validada por `cargo xtask check-signatures` (esquema, IDs únicos, fonte presente e cada regra casando com o jar sintético correspondente de `tests/fixtures/`, gerado por `cargo xtask fixtures-security`; nunca há malware real no repositório). Atualizada a cada versão do Warden; atualização remota é P2 (ADR-0041).

### 21.2 Manutenção e substitutos (`warden-diagnostics::maintenance`, `warden-discovery::similar`; ADR-0042)

- Roda junto com o produtor único de atualizações (P1-12): o mesmo `GET /projects?ids=` traz `status`, `updated` e `game_versions`; o `POST /version_files` com os sha512 dos `.pw.toml` diz se cada arquivo ainda existe. CurseForge: `POST /v1/mods` (ModStatus, `isAvailable`, data do último arquivo) e `POST /v1/mods/files` (FileStatus, `isAvailable`). IDs pedidos e não devolvidos = removidos.
- Cache: Modrinth em `metadata.sqlite` por 24 h; CurseForge só em memória, consultada na primeira abertura do pack em cada execução do Warden. No `publish_plan`, sem cache.
- Situações em função pura (`classify(projeto, arquivo, agora, catálogo) → Situacao`), com o limiar de 18 meses em `data/maintenance.toml`; "sem versão para o Minecraft mais novo" compara a maior versão suportada pelo projeto com a mais nova do manifesto da Mojang (`warden-catalog`).
- Substitutos: `similar(projeto, pack)` monta a busca da mesma plataforma (categorias do projeto sem as de loader, versão e loader do pack, sem arquivados), tira o próprio projeto e os que já estão no pack, ordena por downloads e devolve até 8 candidatos com os motivos calculados (categorias em comum, versão compatível, data da última versão, downloads). O substituto conhecido vem de `obsolete.toml`. `item_replace_apply` usa o `add_plan` (dependências, T09) e a remoção na mesma `PackTransaction`, com ponto de segurança "antes de trocar X por Y".

### 21.3 Travamento de um jogador (`warden-diagnostics::{logsource, modlist, player_redact}`, `warden-app::player_reports`; ADR-0043)

- `logsource::resolve(link) → Option<RawSource>`: tabela fixa de hosts e formatos (SPEC T30) → endereço do texto cru; hosts fora da tabela e `http://` recusados. O download é feito pela `warden-app` com `warden-http` (só HTTPS, `Content-Type` de texto, até 10 MB, redirecionamento só para hosts da tabela, como `gist.githubusercontent.com`).
- `modlist::extract(texto) → ModListado { mc?, loader?, mods: [{ arquivo?, id?, versao?, versao_escondida }] }` para cada formato da T30 (Forge 1.7.10 `UCHIJAAAA`, tabela do 1.12.2, `Mod List:` do Forge e do NeoForge, `Found mod file`, `Loading N mods:` e `Fabric Mods:` do Fabric, seção `Mods:` do Prism, `packwiz.json`), com golden tests de logs reais.
- `modlist::match(listado, candidatas) → Correspondencia`: candidatas = versões salvas com o mesmo Minecraft e loader (as 20 mais recentes), lidas pela `warden-versioning` (árvore de cada tag) e pelos metadados dos jars em cache (`warden-jarmeta`); `packFileHash` do `packwiz.json` confere com o sha256 do `pack.toml` de cada versão publicada; senão, nome do jar contra o `filename` dos `.pw.toml`, e ID + versão contra os metadados. Cobertura = mods que conferem ÷ mods da versão; limiares: exata, ≥ 90% próxima, < 50% desconhecido.
- `player_redact` acrescenta à redação da §9.4 o nome do jogador (`Setting user:`, `--username`, listas de jogadores) e o nome da pasta da instância. Só a cópia redigida é gravada (`player-reports/<pack-id>/<ULID>/log.txt` e `resultado.json`).
- O diagnóstico usa o catálogo da §9.3 sobre o texto (gancho da D-02) e grava um registro de travamento com `origin = player` (gancho da D-06), agrupado pela assinatura. A IA recebe a origem "travamento de um jogador" no `ai_conversation_preview` e a ferramenta `get_player_report` (com a versão identificada e as diferenças).

### 21.4 Notas e grupos (`warden-project::annotations`; ADR-0044)

```toml
# .warden/mods.toml (versionado com o pack; fora do índice pelo bloco obrigatório /.warden/)
schemaVersion = 1

[[groups]]
id = "01JA2…"            # ULID
name = "Performance"

[items."mods/sodium.pw.toml"]
project = "modrinth:AANobbMI"   # ou "curseforge:238222"; arquivos locais: só o caminho
note = "Deixa o jogo mais leve"
groups = ["01JA2…"]
```

- Leitura tolerante (arquivo ilegível não derruba a lista); escrita pela `PackTransaction` (§6.5), com edição mínima (`toml_edit`). A chave é o caminho do metafile; se o caminho sumir e outro metafile tiver o mesmo `project`, a entrada é religada (atualização, troca de versão, renomeação). Remover um item remove a entrada na mesma transação.
- A lista de Mods recebe notas e grupos junto do inventário (`inventory_list`) e agrupa pela função genérica da P1-08. O changelog (`warden-versioning::changelog_notes`) acrescenta a nota aos itens adicionados e removidos quando a opção está marcada; a escolha fica em `packs.json` (dado de máquina).

### 21.5 Itens repetidos (`warden-jarmeta::materials`, `warden-diagnostics::duplicates`; ADR-0046)

- `materials::read(jar)`: tags de material nas três convenções (`data/forge/tags/items/<tipo>/<mat>.json`, `data/c/tags/items/<mat>_<tipo>s.json`, `data/c/tags/item/<tipo>/<mat>.json`), `worldgen/configured_feature` e `placed_feature` de minério, `forge/biome_modifier` e `neoforge/biome_modifier`, modelos `models/item/*` e chaves de idioma; em 1.7.10/1.12.2, chaves `.lang` com nomes do dicionário de minérios e JSON de geração conhecidos (CoFH). Resultado em cache junto do índice da D-05 (`cache/jarindex/`).
- `duplicates::analyze(pack)`: normaliza material e tipo por `data/materials.toml` (sinônimos, como `ingot_copper` e `copper_ingot`), conta mods por material (o Minecraft conta como um), marca "gera minério" (certo com `biome_modifier`; provável no Fabric e nas versões antigas) e escolhe a solução em `data/unifiers.toml` (faixa de versão + loader → mod, ID no Modrinth ou na CurseForge, o que unifica e o que não). Com um unificador no pack, lê a config dele (AlmostUnified: `config/almostunified/*.json`, inclusive `world_gen_unification` no NeoForge 1.21.1).

### 21.6 Desempenho entre versões (`warden-perf::history`; ADR-0047)

- Fonte: `perf/<pack-id>.jsonl`, gravado desde a v1 (gancho da L-10). Uma linha por teste: `{ sessao, data, modo, carregou_ms?, mundo_ms?, ram_pico_mb, heap_pico_mb?, tick_medio_ms?, tick_p95_ms?, perfil: { id, nome, assinatura }, computador, arvore, primeira_abertura, resultado }`. A impressão do computador é um SHA-256 de processador, núcleos, memória total, placa de vídeo e versão do sistema, calculado localmente e nunca enviado.
- `history::compare(linhas, versões, perfil, computador) → Relatorio`: filtra os comparáveis (com o motivo de cada exclusão), atribui à versão pela árvore, calcula a mediana por versão e aplica o critério da SPEC T33 (função pura, com tabela de casos e `proptest`). O resumo do spark por sessão vem da L-12.

### 21.7 Ganchos que a v1 deixa prontos

Custo mínimo, sem função visível na v1. Cada um entra nas entregas da tarefa citada ("Gancho 1.1:" no ROADMAP) e tem o seu teste.

| # | Tarefa da v1 | Gancho | Usado por |
|---|---|---|---|
| 1 | P1-03, L-03 | O download calcula sha1, sha256, sha512 e o murmur2 da CurseForge de uma vez, e o `cache/downloads/index.sqlite` guarda os quatro mais a origem (fonte, projeto, versão, URL) de cada jar. | W-01, W-02 (conferência sem reler os jars), W-06 (identificar jars por hash) |
| 2 | P1-19 | Jars que ficam como arquivo local num pack importado são marcados com a origem da importação em `.warden/project.toml`. | W-02 (atenção redobrada aos jars de packs importados) |
| 3 | L-04, L-10 | A sessão grava a assinatura do perfil, a impressão do computador e a marca de primeira abertura; cada teste acrescenta uma linha em `perf/<pack-id>.jsonl`, que a poda das sessões não apaga. | W-10 (sem isto, os testes feitos antes da 1.1 ficariam fora do gráfico) |
| 4 | P1-07, P1-08 | O leitor de `.warden/` preserva arquivos e tabelas desconhecidos (`.warden/mods.toml` e `.warden/trust.toml` reservados); cada item tem chave estável; o agrupamento da lista é uma função genérica. | W-02, W-08 |
| 5 | D-02, D-06 | A análise pós-crash aceita qualquer texto de log e a redação aceita regras extras; o registro de travamento tem o campo `origin`. | W-06, W-07 |
| 6 | V-03 | As conferências do `publish_plan` são uma lista de checagens plugáveis (trait `PublishCheck`). | W-03 |
| 7 | P1-03, P1-12 | O cache do Modrinth guarda `status`, `updated` e `game_versions` dos projetos, que já vêm na resposta. | W-04 |
