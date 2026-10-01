# R4 — Análise dos projetos anteriores (packwiz-gui e packwiz-gui-manager)

> Documento de pesquisa do projeto Warden. Tarefa R4. Data da análise: 01/10/2026.
> Os dois repositórios analisados são **apenas referência**: nada deles deve ser copiado às cegas para o Warden.

## Método

- Repositórios clonados com `gh repo clone` em `/tmp/claude-1000/warden-ref/` (fora do repositório Warden), sem nenhuma modificação, push, issue ou PR.
  - `Kriticales/packwiz-gui` (branch `main`, HEAD `c982456`).
  - `Kriticales/packwiz-gui-manager` (branch `master`, HEAD `2c93cc8`).
- Leitura do código-fonte, dos documentos internos (`README.md`, `RELATORIO-CORRECOES.md`, `docs/SPEC.md`, `docs/ARCHITECTURE.md`, `docs/RESUME.md`) e do histórico de commits.
- Execução dos gates de cada projeto **localmente, sem instalar nada global**: `npm ci` com `ELECTRON_SKIP_BINARY_DOWNLOAD=1` dentro do clone (o binário do Electron não foi baixado, por isso a interface não foi aberta), depois typecheck, testes e build. Ambiente: WSL2 (Linux), Node 24.20.0, npm 12.0.2.
- Verificação de comportamentos do packwiz **contra o código-fonte do packwiz** (módulo `github.com/packwiz/packwiz@v0.0.0-20260902165313-9066bf845f7e`, presente no cache de módulos Go da máquina) e contra o binário `packwiz` instalado, num pack temporário no scratchpad. Quando algo foi confirmado executando, isso é dito; quando é inferência a partir do código, também.

Fontes primárias desta seção: os próprios repositórios (privados) e o código do packwiz em <https://github.com/packwiz/packwiz>.

## Resumo executivo

- **packwiz-gui** (18/08/2026, 2 commits) é o protótipo "v7" em Electron com JavaScript puro (sem framework, sem bundler, ~8,9 mil linhas). É um painel de controle do packwiz pensado para o modpack Zoonicraft (NeoForge 1.21.1). Cobre muita coisa de forma rasa e tem apenas 8 testes.
- **packwiz-gui-manager** (29/08 a 02/09/2026, 27 commits) é a reescrita a partir de uma especificação formal (`docs/SPEC.md`), em Electron + React + TypeScript strict, construída por vários agentes de IA (um orquestrador, um agente de frontend e um de backend). É muito maior (~6,4 mil linhas no main, ~23,9 mil no renderer, ~12,3 mil de testes) e muito mais disciplinado: **typecheck sem erros, 893 testes verdes em 74 arquivos e build ok** na verificação desta pesquisa.
- Os dois têm a **mesma premissa**: embrulhar o CLI do packwiz, ler o estado do pack direto do disco e usar o **Prism Launcher** (externo) para testar. Nenhum dos dois tem launcher próprio, IA, suporte a versões antigas do Minecraft como requisito, busca no CurseForge nem detecção de mudanças durante o teste com linha de base. São exatamente as lacunas que o Warden precisa cobrir.
- Encontramos e **confirmamos executando o packwiz real** defeitos que valem como lição para o Warden: remoção de mod pelo caminho relativo falha no manager; a opção de datapacks é gravada num arquivo que o packwiz não lê; e os dois apps deixam lixo (`*.bak`, `packwiz-installer-bootstrap.jar`, `.packwiz.toml`) dentro da pasta do pack, e esse lixo **entra no `index.toml`** e seria distribuído aos jogadores.

---

## 1. packwiz-gui (Kriticales/packwiz-gui)

### 1.1 Visão geral

| Item | Situação |
|---|---|
| Propósito | "Painel de controle desktop" do packwiz para o dia a dia de um modpack (o Zoonicraft), sem terminal, mostrando sempre o `stdout`/`stderr` cru dos comandos (`README.md`). |
| Stack | Electron 33 + Node; **JavaScript puro** (CommonJS no main, ES modules no renderer), **sem framework de UI nem bundler**; `@iarna/toml`, `json5`, `yaml`. Empacotamento com electron-builder (NSIS x64). |
| Tamanho | 57 arquivos versionados; ~8,9 mil linhas de JS/CSS/JSON (o maior arquivo é `styles.css`, com 1.063 linhas). |
| Estrutura | `src/main/` (um módulo por área: `packwiz-runner.js`, `pack-reader.js`, `pack-writer.js`, `remote-search.js`, `config-files.js`, `config-model.js`, `prism.js`, `git.js`, `github.js`, `secure-store.js`, `snapshots.js`, `app-config.js`, `file-utils.js`, `main.js`, `preload.js`); `src/renderer/` (`app.js` com estado global, um arquivo por tela em `screens/`, `ui.js` com um helper `el()` de DOM, `i18n.js` e `locales/`); `test/` (2 arquivos); `scripts/check.js`; utilitários Windows (`Iniciar Packwiz GUI.bat`, `criar-atalho.ps1`); `assets/packwiz-installer-bootstrap.jar` embutido. |
| Como rodar | `npm install && npm start` (`npm run dev` abre DevTools). Validação: `npm run check` (sintaxe + JSON dos locales), `npm test` (`node --test`), `npm run smoke` (sobe o Electron oculto e percorre telas). Empacotar: `npm run dist`. Requer `packwiz` no PATH ou caminho configurado. |
| Histórico | **Só 2 commits, ambos em 18/08/2026**: `966ac5a` "Estado inicial: Packwiz GUI (v7 + auditoria de segurança/i18n)" (54 arquivos, +14.119 linhas) e `c982456` "Redesign visual: estilo Modrinth + launchers de Minecraft". O "v7" indica que houve iterações anteriores **fora do git**; não há histórico de como se chegou ali. O `RELATORIO-CORRECOES.md` registra uma auditoria feita por IA no mesmo dia. |

### 1.2 Funcionalidades e grau de completude

Verificação desta pesquisa: `npm run check` → "Source syntax and locale JSON: OK"; `npm test` → **8/8 testes aprovados**. O `smoke` e a interface não foram executados porque exigem o binário do Electron e um display (o `npm ci` foi feito com `ELECTRON_SKIP_BINARY_DOWNLOAD=1` para não baixar nada além das dependências locais). A avaliação de completude abaixo vem da leitura do código.

| Funcionalidade | Como funciona | Completude aparente |
|---|---|---|
| Abrir pack / recentes / wizard de `init` | Escolhe a pasta com `pack.toml`; recentes em `%APPDATA%\Packwiz GUI\packwiz-gui-config.json`; wizard monta `packwiz init ...`. | Funcional. |
| Dashboard | Varre recursivamente todos os `*.pw.toml` (pula `.git`, `node_modules`...), mostra nome, "versão", fonte, lado e categoria; filtros, ordenação, edição de `side` por linha e em lote; erro de parse marcado **por arquivo** (`parseError`), sem derrubar a tela. | Funcional. A "versão" exibida é o **ID de versão do Modrinth** ou o `file-id` do CurseForge (o `.pw.toml` não tem campo de versão legível; ver 1.3). |
| Checagem de updates | Agrupa os mods por algoritmo de hash e chama o endpoint em lote `POST /v2/version_files/update` do Modrinth (`remote-search.js:232`), com estados "tem update", "em dia" e "não foi possível checar". Revisão do changelog antes de atualizar. | Funcional e eficiente (poucas requisições). Só Modrinth. |
| Adicionar mod | Busca no Modrinth com filtros vindos das tags da API, paginação, tela de detalhes (Markdown sanitizado, galeria); checagem de dependências (obrigatórias pré-marcadas, incompatíveis com aviso). CurseForge **só por link** (`packwiz curseforge add <url>`); URL externa (`packwiz url add`). | Funcional para Modrinth. |
| "Import" de Modrinth | Botão em Manutenção que roda `packwiz modrinth import <url>`. | **Quebrado**: esse subcomando não existe no packwiz (só `add` e `export`; confirmado com `packwiz modrinth --help`). |
| Editor de configs | Árvore de `config/`, editor estruturado para TOML/JSON/YAML/`.properties`/`.cfg` e editor bruto (JSON5 só bruto), diff antes de salvar, recusa salvar se o `mtime` mudou, gravação atômica com `.bak`. | Funcional, só para `config/`. |
| `pack.toml` | Editor direto (`pack-writer.js`) que reserializa com `@iarna/toml`. | Funcional, mas **perde comentários e formatação** (o próprio código avisa). |
| GitHub | Log de commits como changelog com diff de mods por commit, publicar (título `<Nome> - <Versão>`), stage por arquivo, branches, remotes, pull, tags, `reset --hard` com confirmação; PAT criptografado com `safeStorage`; criação de repositório, visibilidade e Release via API REST do GitHub. | Funcional, amplo. |
| Manutenção | `refresh`, `refresh --build` (oferece snapshot antes), `serve` start/stop, "checar problemas" (dependências em todos os mods Modrinth), snapshots zip fora do repositório com **restauração transacional** (extrai em temp, valida `pack.toml`, move o atual para recuperação, instala, faz rollback se falhar). | Funcional. |
| Testar modpack (Prism) | Se existe instância no Prism com o nome do pack, roda `prismlauncher --launch`; se não, garante o `packwiz-installer-bootstrap.jar` **na raiz do pack**, monta um zip (`instance.cfg` + `mmc-pack.json` + `.minecraft/` só com o jar) via `Compress-Archive` do PowerShell e abre o importador do Prism (`--import`). O pre-launch aponta para a URL do `packwiz serve`. | Funcional só no Windows (depende do PowerShell). O casamento de instância é por nome normalizado. |
| Logs | Lê o fim de `latest.log` e o crash report mais novo da instância, com busca e realce de `ERROR`/`FATAL`/`Exception`/`Caused by`. | Funcional, sem análise. |
| i18n | pt-BR (padrão), en, es. | Parcial: o próprio relatório admite textos de fluxos avançados ainda fixos em português; títulos de tela estão fixos em `app.js`. |

### 1.3 Integração com o packwiz

- **Subprocesso**: `packwiz-runner.js` usa `spawn(bin, args)` sem shell, com argumentos em array, e transmite `stdout`/`stderr` ao renderer por eventos com um id sequencial; `packwiz serve` roda como processo em segundo plano rastreado num `Map` e encerrado ao sair. O binário vem da configuração ou do PATH. As flags globais (`--pack-file`, `--meta-folder`, `--meta-folder-base`, `--cache`, `--config`) vêm das configurações; o `-y` é anexado por ação.
- **Quem monta os argumentos é o renderer**: há um canal IPC genérico `packwiz:run` que recebe `cwd` e `args` arbitrários vindos da interface (`main.js:285`). As telas montam `['update', slug]`, `['remove', slug]` etc. Isso espalha o conhecimento do CLI pela UI e abre a porta para qualquer comando em qualquer pasta.
- **Leitura de TOML**: o estado vem do disco (`pack-reader.js`), nunca do CLI. `.pw.toml` e `pack.toml` são lidos com `@iarna/toml`; a fonte (Modrinth/CurseForge/URL) é inferida pela tabela `[update]`. O índice (`index.toml`) **não** é consultado: a lista vem da varredura de pastas.
- **Escrita de TOML**: `pack.toml` e o `side` dos `.pw.toml` são reserializados inteiros (perdem comentários e a ordem original). Gravação atômica (`file-utils.js`: arquivo temporário + rename) com cópia `.bak` **ao lado do arquivo, dentro do pack**.
- **Tratamento de erros**: cada handler devolve `{ ok, error }` com strings, misturando português e inglês; não há códigos de erro. O resultado do processo carrega `code`, `stdout` e `stderr` e a UI mostra a saída crua. Detalhe importante, confirmado no código do packwiz (`cmd/remove.go`): **o packwiz imprime erros no `stdout` e sai com código 1**, então quem olha só o `stderr` perde a mensagem.

### 1.4 Qualidade

- **Arquitetura**: simples e legível, mas sem camadas. Estado global mutável em `app.js`; telas constroem DOM imperativamente com o helper `el()` (15 usos restantes de `innerHTML`, a maioria com conteúdo estático após a auditoria). Main process com 82 handlers IPC num único `main.js`.
- **Tipagem**: nenhuma (JS puro, sem JSDoc sistemático).
- **Testes**: 8 testes de regressão de segurança/integridade (gravação atômica, path traversal, refs git com cara de opção, bloqueio de navegação, allowlist de protocolos do Markdown, locales). Nenhum teste de fluxo de packwiz, Prism, updates ou UI.
- **Segurança**: boa base (`contextIsolation`, `nodeIntegration: false`, CSP, navegação bloqueada, links externos só http/https, validação de refs git, `safeStorage` para o PAT). Porém `sandbox: false`, o canal `packwiz:run` genérico e `shell:openPath` com qualquer caminho vindo do renderer.
- **UX**: rica para um protótipo (console dock com 500 linhas, revisão de changelog antes de atualizar, snapshot oferecido antes de ações arriscadas, estados "não foi possível checar" distintos de "em dia").
- **Dívidas e bugs aparentes**:
  1. `packwiz modrinth import` não existe (confirmado).
  2. `.bak` gravados dentro da pasta do pack e o `packwiz-installer-bootstrap.jar` copiado para a raiz do pack: **o `packwiz refresh` indexa os dois** (confirmado num pack temporário: `config/x.toml.bak`, `mods/*.pw.toml.bak` e `packwiz-installer-bootstrap.jar` apareceram no `index.toml`). Ou seja, o app polui o pack distribuído. As listas padrão de ignorados do packwiz cobrem só `.git/**`, `.gitattributes`, `.gitignore`, `.DS_Store`, `/*.zip`, `*.mrpack` e o binário `packwiz` (`core/index.go`, `ignoreDefaults`).
  3. Testar depende de PowerShell (`Compress-Archive`) e de `packwiz serve` rodando; casamento de instância por nome.
  4. Reserialização de TOML destrói comentários do usuário.
  5. CurseForge sem busca e sem checagem de updates.

### 1.5 O que aproveitar e o que evitar

**Aproveitar (como ideia, não como código)**
- Checagem de updates **em lote por hash** (`/v2/version_files/update`) em vez de uma requisição por mod.
- Erro de parse **por arquivo** no dashboard: um `.pw.toml` ruim não derruba a lista.
- Restauração de snapshot **transacional** com rollback e pasta de recuperação.
- Codificação correta do `PreLaunchCommand` no formato QSettings do Prism (valor inteiro entre aspas, aspas internas escapadas). O comentário em `prism.js:209` explica o bug histórico `javaw.exe-jar`, que bate com a issue [PrismLauncher#1134](https://github.com/PrismLauncher/PrismLauncher/issues/1134).
- Revisão de changelog antes de atualizar; snapshot oferecido antes de `refresh --build`; recusa de salvar config alterada externamente (checagem de `mtime`).
- Console sempre visível com a saída crua, sem tradução.

**Evitar**
- Canal genérico "rode este comando com estes argumentos" exposto à UI.
- Escrever qualquer arquivo auxiliar (`.bak`, jar, temporários) dentro da pasta do pack.
- Reserializar TOML do usuário.
- Dependência de ferramentas do sistema (PowerShell) para tarefas que uma biblioteca resolve.
- Strings de erro livres e misturadas em vários idiomas.

---

## 2. packwiz-gui-manager (Kriticales/packwiz-gui-manager)

### 2.1 Visão geral

| Item | Situação |
|---|---|
| Propósito | Mesmo objetivo do packwiz-gui ("nunca tocar no terminal no dia a dia do modpack"), agora a partir de uma **especificação de produto formal** (`docs/SPEC.md`) e um **contrato de arquitetura** (`docs/ARCHITECTURE.md`). Integra packwiz, `git`, GitHub CLI (`gh`) e Prism Launcher. Caso principal: NeoForge 1.21.1 (Zoonicraft), mas sem nada fixo num pack. |
| Stack | Electron 44 + electron-vite + electron-builder (NSIS); React 18 + TypeScript strict; react-router (HashRouter); **Zustand** (estado de UI) + **TanStack Query** (tudo assíncrono); Tailwind 3 + tokens CSS; i18next (pt/en/es); `electron-store`; `@ltd/j-toml`, `yaml`, `json5`, `archiver`/`yauzl`, `react-markdown` + `rehype-sanitize`, `dompurify`; Vitest + Testing Library + jsdom. |
| Tamanho | 267 arquivos; ~6,4 mil linhas de TS de produção no main/preload/shared, ~23,9 mil no renderer, ~12,3 mil de testes; 3 locales com 1.316 linhas cada. |
| Estrutura | `src/shared/` (contrato: `ipc.ts` com nomes de canais e a superfície `Api`, `types.ts` com os tipos de domínio); `src/main/` (`index.ts`, `ipc/register.ts` + `ipc/failure.ts`, `services/` com um serviço por área: `packwiz`, `packs`, `packToml`, `toml`, `indexScanner`, `mods`, `updateEngine`, `curseForgeUpdates`, `modrinth`, `modVersions`, `dependencies`, `configs`, `snapshots`, `git`, `gh`, `prism*`, `crashAnalysis`, `fsSafe`, `shellSafe`, `settings`, `events`); `src/preload/`; `src/renderer/` (`screens/`, `components/` por área + `components/ui/` com primitivos próprios, `hooks/` com as queries, `lib/` com lógica pura testada, `stores/`, `locales/`, `styles/`); `resources/` com o bootstrap jar e `THIRD_PARTY_LICENSES.md`; `docs/` (SPEC, ARCHITECTURE, RESUME). |
| Como rodar | `npm ci`, `npm run dev`; gates `npm run typecheck`, `npm test`, `npm run build` (`build:win` gera o instalador). Suítes opcionais contra o ambiente real: `npm run test:prism` e `npm run test:github`. |
| Histórico | **27 commits entre 29/08 e 02/09/2026.** O primeiro (`eb6be47`, "Ponto de salvamento: design atual e app funcional") já traz 196 arquivos e +43.644 linhas, ou seja, o app foi construído antes de entrar no git. Os seguintes são correções e "vagas" de funcionalidades, com mensagens descritivas em português europeu ("Fecha o beco sem saída da instância do Prism", "Dá porta à análise de crash, que existia e era inalcançável"). Houve uma divergência de histórico reunida por merge (`2c93cc8`). O `docs/RESUME.md` é um diário de bordo detalhado das sessões, com números de testes medidos e erros de método admitidos. |

**Como foi construído**: por um orquestrador de IA ("Claudinei") coordenando dois agentes (um "Claude Frontend" e um "Codex Backend") pelo HiveTerm, com fronteiras rígidas de propriedade de arquivos (`hive.yml`, `docs/ARCHITECTURE.md` §2). A partir de 02/09 passou a ser conduzido pelo Claude Code via Superset. Esse modelo explica tanto a disciplina (contratos, guardas, testes) quanto o principal tipo de defeito encontrado: **funcionalidades completas de um lado da fronteira e inalcançáveis do outro** (ver 2.4).

### 2.2 Funcionalidades e grau de completude

**Verificação desta pesquisa (WSL2, Node 24.20.0)**: `npm run typecheck` → exit 0; `npx vitest run` → **74 arquivos, 893 testes aprovados, 0 falhas**; `npx electron-vite build` → ok (bundle do renderer com 1,63 MB). As suítes `test:prism` e `test:github` não foram executadas (dependem do Prism instalado e de um `gh` autenticado com escopo `delete_repo`). A interface não foi aberta (sem o binário do Electron). O próprio `RESUME.md` registra que a validação "na janela viva" era feita por CDP no Windows e que **o instalador em `release/` é de 29/08 e não contém nada do que veio depois**.

| Funcionalidade | Completude aparente |
|---|---|
| Seleção de pack, recentes, criação (`packwiz init`) com pasta-base em Documentos, inspeção da pasta de destino antes de criar | Funcional; o botão de criar do menu de recentes ficou "morto" em produção até 31/08 (estado do diálogo desmontado junto com o menu; corrigido em `bb322af`/`95be6e0`). |
| Dashboard com seletor de categoria (mods, resource packs, shaders, datapacks), filtros, ordenação, edição de `side` inline e em lote, `pin`/`unpin` com guarda também no backend (`PINNED_MOD_UPDATE`) | Funcional. A coluna "versão" mostra o ID de versão do Modrinth ou o file-id do CurseForge, pelo mesmo motivo do packwiz-gui. |
| Relatório de updates com **uma única fonte de verdade** (`UpdateReport` com estados `up-to-date`, `outdated`, `unknown` e `unsupported`, e `checkedAt: null` = "nunca checado"); Modrinth por API e CurseForge pelo **próprio packwiz numa cópia temporária do pack** | Funcional. Modrinth faz **uma requisição por mod** (com pool de 6 e respeito a `Retry-After`/`X-Ratelimit-Reset`); a "mais recente" é escolhida por data de publicação sem filtrar `version_type`, então é provável que sugira alfas/betas (inferência pelo código). |
| Adicionar: busca Modrinth por tipo de projeto com facetas vindas das tags da API, paginação, filtros padrão do `pack.toml`, detalhes com galeria e changelog por versão, adicionar versão específica (`--project-id`/`--version-id`), dependências obrigatórias/opcionais/incompatíveis; CurseForge por link; URL; GitHub (`packwiz github add`) | Funcional para Modrinth. Datapacks: **quebrado** (ver 2.3). |
| Exportar `.mrpack` e zip do CurseForge (`packwiz modrinth/curseforge export`), `migrate`, `rehash`, `curseforge import`/`detect`/`open`, `acceptable-versions` pelo CLI | Funcional (o `RESUME.md` diz que `migrate` e `rehash` foram provados contra o packwiz real). |
| Configs: árvore, editor estruturado preservando comentários (TOML com `@ltd/j-toml` + patch por linha, YAML com a API `Document`, `.properties`/`.cfg` com parser próprio), diff antes de salvar, recusa por `mtime`, `.bak`; **"puxar da instância"**: compara `config/` da instância do Prism com o do pack (`new`/`modified`/`unchanged`) e copia só o que o usuário marcar | Funcional, só para `config/` (não cobre `options.txt`, `defaultconfigs/`, `kubejs/` etc.) e **sem linha de base** (ver 2.5). |
| Manutenção: refresh/`--build`, serve, checagem retroativa de dependências, snapshots, visualizador de logs | Snapshots: a restauração **mescla** o zip sobre a pasta, não apaga o que foi adicionado depois, não é atômica e não guarda o estado anterior (achado P1 registrado no próprio `RESUME.md`; confirmado no código, `snapshots.ts:114`). É uma regressão em relação ao packwiz-gui. |
| Análise de crash determinística (`crashAnalysis.ts`): extrai descrição, exceção, versão do MC e loader, lista de mods carregados, suspeitos "explícitos" (linha `Suspected Mods`, seções `-- MOD x --`, falhas de mixin) e "inferidos" (id do mod no stack trace), e classifica em `out-of-memory`, `java-version`, `graphics-driver`, `missing-dependency`, `suspected-mod` ou `unknown`, sempre com a evidência | Funcional e testada com 12 testes de UI e testes de lógica; nunca validada contra um crash real (admitido no `RESUME.md`). Lê só `crash-reports/`: erros que não geram crash report (tela de erro de carregamento do NeoForge, "Incompatible mods" do Fabric no `latest.log`) ficam de fora (inferência sobre o escopo). |
| Prism: detecção do executável (Program Files, LocalAppData, PATH), **ligação explícita pack → instância** escolhida pelo usuário e persistida, estado como união discriminada (`prism-not-found`, `not-linked`, `healthy`, `unmanaged`, `unreadable`), reparo (`OverrideCommands` + `PreLaunchCommand`), ressincronização rodando o packwiz-installer com o Java do próprio Prism e relatando contagens por categoria, RAM (`null` = herdar do Prism), ícone da instância, testar (lançar a existente ou exportar zip, instalar e lançar) | Funcional no Windows, com ressalvas: `prismSync.ts` só escreve em `minecraft/`, enquanto `prism.ts` lê `minecraft/` e `.minecraft/` (P1 do próprio projeto); o preparo do lançamento **lê o log do Prism procurando um texto** e espera o componente `org.lwjgl3` aparecer no `mmc-pack.json` (`prism.ts`, `preparePrismLaunch`), o que é frágil. |
| GitHub: `git` puro (sem token) + `gh` para visibilidade, criação de repositório e Release; status/stage, branches, remotes, pull, tags, reset, changelog automático a partir do diff staged, log paginado com diff de mods por commit | Funcional depois das correções de 02/09 (paginação que pulava os 20 commits mais recentes, acentos em nomes de arquivo quebrando o diff, `git restore --staged` sem HEAD, `ext::` executando comando, push travado esperando senha). |
| Settings, onboarding, "reset app" restrito ao estado do próprio app, temas (padrão e "Minecraft"), i18n pt-BR/en/es com guardas de paridade e de chave existente | Funcional. |

### 2.3 Integração com o packwiz

- **Subprocesso** (`services/packwiz.ts`): `spawn` sem shell, `stdin` fechado imediatamente (evita travar em prompt), saída transmitida linha a linha como evento `log:line`, `CommandExecutionError` com `stdout`/`stderr`/exit code. O binário é resolvido pela configuração ou por `where.exe`/`which`; a versão do packwiz é lida da **tabela de build info do binário Go** (o packwiz não tem comando de versão), com fallback honesto para "available".
- **Construtores de argumentos puros e testados**: `buildModrinthAddArgs`, `buildPackwizInitArgs`, `buildPackwizExportArgs`, `packwizMetadataName` etc. O renderer chama métodos de domínio (`mods.add(spec)`), nunca monta argv. Esse é o desenho certo.
- **Limitações do CLI documentadas a partir do binário** (`ARCHITECTURE.md`, "Limitações conhecidas"): não existe import de `.mrpack`; `pin`/`unpin` só aceitam o nome do metadado (sem caminho); `--version-id` exige `--project-id` e nenhum argumento posicional; `--modloader` aceita caixa mista.
- **Leitura de TOML**: `@ltd/j-toml` com opções que preservam comentários e ordem; edições pontuais por **patch de linha** (`toml.ts`, `patchTomlKey`) em vez de reserializar, o que preserva a formatação do usuário. O inventário (`indexScanner.ts`) varre só as pastas fixas `mods/`, `resourcepacks/`, `shaderpacks/` e `datapacks/` (ignora `mods-folder`, `--meta-folder` e o `index.toml`) e usa `Promise.all`: **um único `.pw.toml` inválido derruba a lista inteira** (inferência pelo código).
- **Tratamento de erros**: `Result<T>` em todo IPC ("nada lança através do IPC"), com `code` estável classificado no main e traduzido no renderer, e `detail` com a saída da ferramenta. Fechado para Prism (5 códigos) e Git/gh (22 códigos). **Fora dessas áreas, 88 `throw new Error()` em inglês continuam virando `INTERNAL_ERROR`** com a mensagem crua (o próprio `RESUME.md` aponta ~90).
- **Concorrência**: nenhuma fila ou trava por pack. Dois comandos packwiz podem rodar ao mesmo tempo no mesmo pack (por exemplo, um update em lote e um refresh), e o packwiz reescreve `index.toml` e `pack.toml` a cada comando. Não há timeout nem cancelamento.
- **Defeitos confirmados executando o packwiz real** (pack temporário, binário `packwiz` da máquina):
  1. **Remover mod falha**: o IPC `mods.remove` passa o `relPath` (`mods/x.pw.toml`) direto para `packwiz remove` (`ipc/register.ts:163`, `packwiz.ts`, método `remove`). O packwiz só aceita o nome do metadado (`core/index.go`, `FindMod`, compara o basename sem `.pw.toml`). Resultado real: `Can't find this file; please ensure you have run packwiz refresh and use the name of the .pw.toml file`, exit 1. O wrapper já tinha o helper certo (`packwizMetadataName`) e o usava em `update`/`pin`/`unpin`, mas não em `remove`; nenhum teste cobre `remove`. Observação para o Warden: `FindMod` percorre um *map* do Go, então dois metadados com o mesmo basename em pastas diferentes (por exemplo, `mods/foo.pw.toml` e `resourcepacks/foo.pw.toml`) tornam a escolha **não determinística**.
  2. **Datapacks do Modrinth não funcionam**: antes de `modrinth add`, o app grava `datapack-folder = "datapacks"` num `.packwiz.toml` **na raiz do pack** (`packwiz.ts`, método `add`). O packwiz lê esse arquivo do diretório de configuração do usuário (ou de `--config`), não da pasta do pack (`cmd/root.go`, `initConfig`); a forma suportada é a tabela `[options]` do `pack.toml`, que é mesclada no viper (`core/pack.go`, `viper.MergeConfigMap(modpack.Options)`). Teste real: com o `.packwiz.toml` na raiz, `packwiz modrinth add blazeandcaves-advancements-pack` falhou com "no valid versions found ... specify the datapack-folder option"; com `[options] datapack-folder = "datapacks"` no `pack.toml`, adicionou. E pior: o `.packwiz.toml` órfão **entra no `index.toml`** no próximo refresh.
  3. **Lixo indexado**: o mesmo problema do packwiz-gui com `*.bak` (gravados ao lado de `pack.toml`, `.pw.toml` e configs) e com o `packwiz-installer-bootstrap.jar` copiado para a raiz do pack (`prism.ts`, `bootstrapSource`). Nenhum dos dois é adicionado ao `.packwizignore`.
- **Prism e packwiz-installer**: o pre-launch gerado é `"$INST_JAVA" -jar "$INST_MC_DIR/packwiz-installer-bootstrap.jar" "$INST_MC_DIR/pack.toml"`, com o pack copiado para dentro da instância e instalado **a partir de arquivo local**, sem `packwiz serve`. A documentação oficial só mostra URL HTTP(S) ([packwiz.infra.link](https://packwiz.infra.link/tutorials/installing/packwiz-installer/)), mas o código do packwiz-installer aceita caminho local e `file:` (`Main.kt`: "None of the above matches -> interpret as file path", [packwiz-installer](https://github.com/packwiz/packwiz-installer/blob/main/src/main/kotlin/link/infra/packwiz/installer/Main.kt)). **Risco provável, não verificado em execução**: o valor é gravado no `instance.cfg` com aspas sem escape. O próprio packwiz-gui documenta que o QSettings do Prism come essas aspas (`$INST_JAVA-jar`), e a issue [PrismLauncher#1134](https://github.com/PrismLauncher/PrismLauncher/issues/1134) descreve exatamente essa perda de aspas e espaços em valores não escapados (o PR [#1174](https://github.com/PrismLauncher/PrismLauncher/pull/1174) adicionou uma migração para arquivos legados). Como o reparo edita um `instance.cfg` já criado pelo Prism, é plausível que o comando seja corrompido. O `RESUME.md` só confirma que o arquivo foi escrito, não que o jogo baixou os mods.

### 2.4 Qualidade

**Pontos fortes (acima da média)**
- Contrato tipado entre processos (`src/shared/ipc.ts` + `types.ts`), `Result<T>` sem exceções atravessando o IPC, códigos de erro fechados por área, cada um com teste provando que existe texto nos três idiomas.
- Regras de domínio escritas e defendidas por testes: **um único produtor do estado de update**, contagens derivadas do mesmo mapa, botão de update habilitado só se o estado da linha for `outdated` (bug que já tinha acontecido antes); estados "não checado" e "não foi possível checar" distintos.
- Segurança de comandos: argv sempre em array, `buildCommandString` testado para o único lugar que precisa de string; git com `LC_ALL=C`, `GIT_TERMINAL_PROMPT=0`, bloqueio de `ext::`, NUL como delimitador no parser do log, validação assíncrona (rejeita, não lança de forma síncrona).
- Filesystem: `resolveInside` contra path traversal, recusa de symlinks na promoção de configs, gravação atômica (temp → fsync → rename), checagem de `mtime`.
- Guardas meta: `ipcConsumerGuard` (todo canal do contrato precisa de consumidor no renderer, com allowlist que só encolhe), paridade e existência de chaves i18n, contraste e tokens de tema, estabilidade de seletores do Zustand.
- Documentação honesta: o `RESUME.md` registra o que foi provado, o que não foi, e onde o próprio orquestrador errou.

**Problemas e dívidas evidentes**
1. **Funcionalidades órfãs entre backend e frontend**: análise de crash, `curseforgeDetect`/`open`, ícone da instância e o contrato de status/reparo do Prism foram entregues no backend e ficaram inalcançáveis na UI até uma auditoria (`RESUME.md`, 30/08). O app inteiro passava nos testes enquanto o usuário batia num "beco sem saída" ("escolha a instância", sem lugar para escolher).
2. **Estado global "pack atual" no main** (`packs.requireCurrentPath()`): todo comando age sobre o pack que estiver selecionado *no momento da execução*. Trocar de pack com uma operação em andamento, ou com um diálogo destrutivo aberto, liga a ação ao pack errado (P1 registrado: `usePacks.ts:65`).
3. Defeitos de integração com o packwiz descritos em 2.3 (remove, datapacks, lixo indexado), nenhum coberto por teste. Os testes "contra o packwiz real" (`packwizReal.test.ts`) cobrem outros comandos.
4. Snapshot que mescla em vez de restaurar; `prismSync` com layout de pasta fixo; lançamento via leitura de log do Prism.
5. Inventário frágil (um TOML ruim derruba tudo) e cego para `index.toml`, `mods-folder` e metafolders customizados.
6. Checagem de updates do Modrinth com N requisições e sem filtro de canal de release.
7. Complexidade alta para o escopo: ~24 mil linhas de renderer, 86 canais IPC, bundle de 1,6 MB, temas e tokens com 302 variáveis. Boa parte do esforço foi para polimento visual e meta-guardas, enquanto fluxos básicos (remover mod, datapacks, restaurar snapshot) seguiam quebrados.
8. Português europeu em parte das strings e dos commits, apesar do padrão pt-BR (o projeto registrou essa lição em 02/09).
9. Dependência total de ferramentas externas para o teste: Prism instalado, Java do Prism, `gh` autenticado.
10. Nenhum suporte a mods opcionais do packwiz (tabela `[option]` do `.pw.toml`), nem nos dois projetos.

### 2.5 O que aproveitar e o que evitar

**Aproveitar (ideia, lógica ou fluxo de UX)**
- O **modelo de erros**: `Result<T>`, código estável classificado no backend, frase traduzida no frontend e saída crua da ferramenta como evidência secundária. No Warden isso vira um `enum` de erro serializável no Rust (por exemplo, com `thiserror` + `serde`) e um mapa de traduções no React.
- A **regra do produtor único** para o relatório de updates e a distinção entre "não checado", "não foi possível checar", "não suportado" e "em dia".
- Construtores de argv puros, testados, e as **limitações do CLI documentadas** (pin/unpin por nome, `--version-id` + `--project-id`, ausência de import de `.mrpack`).
- Leitura da versão do packwiz pela build info do binário Go.
- Checagem de updates do CurseForge **rodando o packwiz numa cópia temporária** (o packwiz não tem `update --check`). Vale avaliar se o Warden faz isso direto pela API do CurseForge.
- Edição de TOML por patch de linha preservando comentários; editor de configs com parsers que preservam comentários (TOML, YAML `Document`, `.properties`/`.cfg` linha a linha).
- O fluxo "**puxar configs da instância**" com estados `new`/`modified`/`unchanged` e cópia seletiva: é a semente da funcionalidade "detectar o que mudou durante o teste" do Warden.
- A **análise de crash determinística com evidência obrigatória** (o veredito só existe junto com a linha que o sustenta): é a semente da camada determinística antes da IA.
- O reparo/ressincronização que **só toca nos arquivos do `index.toml`** e nunca em `saves/`, `options.txt` ou `screenshots/`.
- RAM com `null` = herdar; ligação explícita pack → instância em vez de casar por nome.
- Dependências do Modrinth no momento de adicionar (obrigatórias pré-marcadas, opcionais informativas, incompatíveis como aviso sem bloquear).
- Git sem token próprio (credenciais do sistema) e `gh` só para o que o git não faz; `LC_ALL=C` e `GIT_TERMINAL_PROMPT=0`.
- A regra de aceitação de testes (`ARCHITECTURE.md` §9.2: o teste precisa ter sido visto falhando pela razão certa) e o teste de colocação de diálogos com mousedown + mouseup + click.

**Evitar**
- Estado implícito de "pack atual" no backend: o Warden deve passar o identificador do pack em **todo** comando e serializar as operações por pack.
- Entregar backend sem a porta na UI (e vice-versa): cada funcionalidade só está pronta com um teste ponta a ponta que passa pela interface.
- Arquivos auxiliares dentro da pasta do pack; opções do packwiz gravadas fora do lugar que o packwiz lê.
- Depender do log de outro programa para saber se algo ficou pronto.
- Gastar o orçamento em tema e meta-guardas antes de os fluxos básicos estarem provados contra o packwiz real.
- Strings em português europeu.

---

## 3. Comparação e evolução

### 3.1 Comparação lado a lado

| Aspecto | packwiz-gui | packwiz-gui-manager |
|---|---|---|
| Linguagem / UI | JS puro, DOM imperativo | TypeScript strict, React, Tailwind, primitivos próprios |
| Estado | Objeto global em `app.js` com contadores de requisição para descartar respostas obsoletas | Zustand + TanStack Query, invalidação por chave |
| Contrato IPC | 82 handlers soltos, `{ok, error}` com string | `src/shared/ipc.ts` tipado, `Result<T>` com `code`/`message`/`detail` |
| Pack alvo dos comandos | `packDir` explícito em cada chamada (vindo do renderer, sem validação) | "Pack atual" global no main (implícito) |
| Montagem de argv do packwiz | No renderer, via canal genérico `packwiz:run` | No main, por construtores puros testados |
| Inventário | Varre o pack todo; erro de parse por arquivo | Varre 4 pastas fixas; um erro derruba tudo |
| Escrita de TOML | Reserializa (perde comentários) | Patch por linha (preserva) |
| Updates | Modrinth em lote por hash | Modrinth por mod + CurseForge via packwiz em cópia temporária; produtor único |
| CurseForge | Só adicionar por link | Link, import, detect, open, export e checagem de updates via packwiz |
| Snapshots | Restauração transacional com rollback | Restauração que mescla (regressão) |
| Teste no jogo | Prism externo; zip via PowerShell; pre-launch aponta para `packwiz serve` | Prism externo; ligação explícita; reparo/resync com o Java do Prism; pre-launch com arquivo local |
| Logs / crash | Visualizador com realce | Visualizador + análise determinística com evidência |
| Configs | Editor de `config/` | Editor de `config/` + "puxar da instância" |
| Git/GitHub | git + API REST com PAT em `safeStorage` | git puro + `gh`; 22 códigos de erro |
| Testes | 8 | 893 (+ suítes opcionais contra Prism e GitHub reais) |
| Gates verificados nesta pesquisa | check ok, 8/8 testes | typecheck ok, 893/893 testes, build ok |
| Plataforma | Windows (PowerShell, `.bat`, `.ps1`) | Windows como alvo; testes rodam em Linux |
| Lixo no pack indexado pelo packwiz | Sim (`.bak`, bootstrap jar) | Sim (`.bak`, bootstrap jar, `.packwiz.toml`) |

Em resumo: o manager é melhor em praticamente todo eixo de engenharia (tipagem, contratos, testes, erros, preservação de formatação), mas regrediu em dois pontos que o packwiz-gui fazia melhor (inventário tolerante a erro e snapshot transacional) e ficou mais pesado. Os dois compartilham as mesmas lacunas de produto e o mesmo erro de higiene do pack.

### 3.2 Linha do tempo da ideia

| Quando | O que aconteceu | Fonte |
|---|---|---|
| Antes de 18/08/2026 | Pelo menos seis iterações anteriores, fora do git (o commit inicial se chama "v7"). Bugs dessa época viraram requisitos explícitos da SPEC do manager: botão de update ativo em todas as linhas, pre-launch `javaw.exe-jar` por falta de espaço/aspas, controles sem espaçamento. | `packwiz-gui` `966ac5a`; `docs/SPEC.md` (packwiz-gui-manager) |
| 18/08/2026 | packwiz-gui v7 versionado, auditoria de segurança/i18n por IA (`RELATORIO-CORRECOES.md`) e redesign "estilo Modrinth + launchers" no mesmo dia. Foco no Zoonicraft (NeoForge 1.21.1). | commits `966ac5a`, `c982456` |
| Entre 18/08 e 29/08 | Escrita da SPEC formal e do contrato de arquitetura; decisão de reconstruir com um orquestrador de IA e dois agentes. **Electron escolhido em vez de Tauri** porque não havia toolchain Rust na máquina e assim os dois agentes compartilhariam uma só linguagem. | `docs/SPEC.md`, `docs/ARCHITECTURE.md` §1 |
| 29/08 | Primeiro commit do manager, já com o app funcional (+43,6 mil linhas) e instalador NSIS gerado às 23:25. | `eb6be47`, `RESUME.md` |
| 30/08 | Quatro bugs reportados pelo usuário (scroll da paginação, abas de resource pack/shader/datapack sempre vazias, `git pull` sem upstream, nada chegando à instância do Prism); segunda e terceira "vagas" de funcionalidades durante a madrugada; descoberta do beco sem saída da instância do Prism e auditoria de cinco canais IPC órfãos; criação do `ipcConsumerGuard` e da guarda de i18n. | commits de `8331b60` a `633e5aa` |
| 31/08 | Botão "criar modpack" morto no switcher; regra estrutural §9 (estado de diálogo em dono estável) e regra de aceitação de guardas §9.2. | `bb322af` a `c7c4d5b` |
| 02/09 | Sessão conduzida pelo Claude Code via Superset: primeira execução da suíte em Linux, revisão geral com 20 achados (três P1 ainda abertos), contrato de erros do GitHub, correção do histórico que pulava 20 commits, arrumação. O modpack real (Zoonicraft) **ainda não existia**; o instalador continuava o de 29/08. | `4fa8a3e` a `2c93cc8`, `RESUME.md` |
| Depois de 02/09 | Início do Warden: Tauri + Rust, launcher próprio, IA, CurseForge com busca e suporte a todas as versões. Ou seja, o Warden **reverte a decisão Electron-vs-Tauri** e troca o Prism externo por um launcher integrado. | contexto desta tarefa |

A trajetória mostra três movimentos: (1) de "painel que roda comandos" para "app com domínio próprio e contratos"; (2) de "testar = abrir o Prism" para "gerenciar a instância do Prism" (ligação, reparo, resync, RAM), que é o caminho natural até o launcher próprio do Warden; (3) de "mostrar o log" para "diagnosticar o crash com evidência", que é o caminho até o diagnóstico com IA. A dor que se repete do começo ao fim é a **ponte com o Prism** (casamento de instância, formato do `instance.cfg`, pasta `minecraft/` vs `.minecraft/`, Java, pre-launch), o que reforça a decisão do Warden de ter um launcher próprio.

---

## 4. Implicações para o Warden

### 4.1 Problemas a não repetir (regras para o Warden), em ordem de prioridade

1. **A pasta do pack é sagrada.** O Warden nunca escreve dentro do pack nada que não seja conteúdo do modpack. Backups (`.bak`), jars auxiliares (bootstrap), arquivos temporários, caches e configuração do app ficam no diretório de dados do app. Antes de qualquer `packwiz refresh`/export, um **verificador de higiene** confere o `index.toml` contra uma lista de padrões proibidos (`*.bak`, `*.tmp`, `packwiz-installer-bootstrap.jar`, `.packwiz.toml`, logs, `crash-reports/`, `saves/`...) e propõe entradas no `.packwizignore`. Motivo: confirmamos que o packwiz indexa tudo o que não está nos seus poucos padrões padrão.
2. **Opções do packwiz no lugar que o packwiz lê.** Opções por pack (`datapack-folder`, `acceptable-game-versions`, `mods-folder`, `no-internal-hashes`, `meta-folder`) vão na tabela `[options]` do `pack.toml` (ou pelo `packwiz settings`, quando existir comando). Cada escrita de opção tem um teste contra o binário real provando que o packwiz a respeita.
3. **Identificar metadados pelo nome que o packwiz espera** (basename sem `.pw.toml`) em `remove`/`update`/`pin`/`unpin`, e **garantir nomes de metadado únicos no pack** (o `FindMod` do packwiz é não determinístico com basenames repetidos). Alternativa a avaliar: o Warden remove direto (apaga o `.pw.toml` e roda `refresh`), que é o que o próprio `packwiz remove` faz.
4. **Todo comando recebe o identificador do pack explicitamente**; nada de "pack atual" implícito no backend. Uma **fila/trava por pack** serializa comandos que escrevem `index.toml`/`pack.toml`; operações longas têm cancelamento e timeout.
5. **Nenhuma funcionalidade "pronta" sem caminho pela UI**: critério de pronto inclui um teste ponta a ponta (backend Rust + comando Tauri + tela) e uma checagem automática de que todo comando Tauri registrado tem consumidor no frontend (equivalente ao `ipcConsumerGuard`).
6. **Testes de integração contra o packwiz real para cada comando usado** (add por Modrinth/CurseForge/URL, remove, update, pin, refresh, export, migrate, datapack), em packs temporários, rodando no CI com a versão de packwiz que o Warden embute.
7. **Inventário tolerante a erro e guiado pelo `index.toml`**: um metadado inválido vira um item com erro, nunca derruba a lista; respeitar `mods-folder` e metafolders customizados.
8. **Restauração de snapshot transacional** (extrai em temp, valida, troca, rollback) e com semântica de "restaurar" (remove o que não estava no snapshot).
9. **Não depender do log de outro programa para sincronizar estado**, nem de ferramentas do sistema (PowerShell) para tarefas que uma crate resolve (zip, por exemplo).
10. **Saída de subprocesso decodificada corretamente**: ler bytes e decodificar UTF-8 só em linhas completas (os dois apps fazem `chunk.toString()` por chunk, o que pode quebrar caracteres multibyte e linhas no meio; inferência pelo código). E lembrar que **o packwiz escreve erros no `stdout`** e sai com código 1.
11. **pt-BR em toda string**, com glossário fornecido a qualquer agente que escreva texto.
12. **Prioridade de esforço**: fluxos básicos provados contra o packwiz real antes de tema, animações e meta-guardas.

### 4.2 Funcionalidades e ideias a manter

1. **Modelo de erros com código estável** (backend classifica, frontend traduz, saída crua como evidência). Em Rust: `enum` de erro serializável por área.
2. **Relatório de updates com produtor único** e estados `outdated`/`up-to-date`/`unknown`/`unsupported`/"nunca checado"; checagem **em lote por hash** no Modrinth (`/v2/version_files/update`), com filtro de canal de release configurável; CurseForge por API (ou pelo packwiz numa cópia temporária, se a API não bastar).
3. **Construtores de argv puros e testados** e uma lista viva das limitações do CLI do packwiz.
4. **Edição de TOML preservando comentários** (patch pontual) e editor de configs multiformato com preservação de comentários, diff antes de salvar e recusa por alteração externa.
5. **Dependências no momento de adicionar** (obrigatórias pré-marcadas, opcionais informativas, incompatíveis como aviso) e **checagem retroativa** do pack inteiro: base da camada determinística de diagnóstico.
6. **Análise de crash determinística com evidência obrigatória**, ampliada para `latest.log`/`debug.log` e para as telas de erro de carregamento dos loaders, servindo de entrada estruturada para a IA.
7. **"Puxar configs da instância"** com `new`/`modified`/`unchanged` e seleção por arquivo, evoluído para diff contra linha de base (ver 4.3).
8. **Sincronização da instância que só toca no que está no índice** e nunca em `saves/`, `screenshots/` e afins; RAM com `null` = padrão.
9. **Revisão de changelog antes de atualizar**; snapshot oferecido antes de ações arriscadas; edição de `side` em lote; `pin`/`unpin` com guarda no backend.
10. **Console com saída crua e sem tradução**, sempre acessível.
11. **Git sem token próprio** (credenciais do sistema), `LC_ALL=C` e `GIT_TERMINAL_PROMPT=0`, bloqueio de `ext::`, changelog de mods por commit (diff de `.pw.toml` entre commits) e título `<Nome> - <Versão>`; `gh` opcional para Release/visibilidade.
12. **Exportação** `.mrpack` e zip do CurseForge pelo packwiz.
13. Leitura da versão do packwiz pela build info do binário Go.

### 4.3 Lacunas que o Warden precisa cobrir (nenhum dos dois projetos tem)

1. **Launcher próprio offline** (substitui o Prism): baixar versão, bibliotecas, assets e loader (Fabric, Forge e NeoForge, inclusive 1.7.10 e 1.12.2, com Java correto por versão) e montar a instância a partir do pack sem `packwiz serve` nem bootstrap. Os dois projetos dependiam do Prism e do Java dele; a ponte com o Prism foi a fonte de bugs mais recorrente.
2. **Suporte declarado a todas as versões do Minecraft**: os dois foram pensados para NeoForge 1.21.1; nada foi testado em 1.7.10/1.12.2 (Forge antigo, LaunchWrapper, Java 8).
3. **CurseForge completo** (busca, detalhes, dependências, updates), não só adicionar por link. Exige chave de API e decisão sobre como distribuí-la.
4. **Detecção de mudanças durante o teste com linha de base**: registrar o estado da instância ao lançar (hash de `config/`, `options.txt`, `defaultconfigs/`, `kubejs/`, resource packs habilitados...) e, ao fechar o jogo, mostrar só o que mudou, distinguindo "o mod gerou o arquivo padrão" de "o usuário alterou". O "puxar da instância" do manager compara só com o pack e mostra todo config novo como "new".
5. **Edição de `options.txt`** e das configurações feitas dentro do jogo (nenhum dos dois toca em `options.txt`; o manager o exclui de propósito do resync).
6. **Diagnóstico com IA (Gemini)** antes de lançar e após crash, com consentimento explícito para enviar logs. A camada determinística do manager (crash) e as checagens de dependência dos dois são o ponto de partida, mas faltam versão/loader errado, duplicatas (mesmo mod por fontes diferentes, jar e metadado duplicados) e conflitos declarados no Modrinth checados antes do "Testar".
7. **Mods opcionais do packwiz** (tabela `[option]` no `.pw.toml`).
8. **Arquivos locais como fonte** (jar solto adicionado pelo usuário), com política clara: `packwiz url add` exige URL; jar local entra no índice como arquivo comum. O comportamento e o tamanho do repositório precisam ser decididos.
9. **Versão legível dos mods**: o `.pw.toml` não tem campo de versão (confirmado na struct `Mod` do packwiz); os dois apps mostram IDs opacos. O Warden precisa de um cache local de metadados da API (nome da versão, ícone, autores), indexado pelo hash ou pelo ID de versão.
10. **Exportação "só o necessário" verificável**: um passo de pré-visualização do que será distribuído (lista do `index.toml` + higiene do item 4.1.1).
11. **Linux** como segunda plataforma: os dois têm detecção e scripts só para Windows.
12. **Concorrência e cancelamento** de operações longas (download, update em lote, lançamento).

---

## 5. Questões em aberto

1. **packwiz embutido ou do sistema?** Os dois projetos dependem do `packwiz` no PATH. O Warden vai empacotar um binário fixo (packwiz não publica releases versionadas; a versão desta máquina é um pseudo-version `v0.0.0-20260902165313-9066bf845f7e`) ou compilar a partir de um commit fixado? Isso afeta os testes de integração do item 4.1.6. (Fora do escopo desta tarefa; provavelmente coberto por outra pesquisa.)
2. **Uso de lógica de domínio em Rust em vez do CLI** para operações simples (ler/escrever `.pw.toml`, remover, editar `side`): o packwiz-gui-manager já reimplementou edições pontuais de TOML. Onde fica a fronteira entre "chamar o packwiz" e "escrever o arquivo direto"? Recomenda-se uma ADR.
3. **Formato do `instance.cfg` do Prism**: confirmar em execução se o `PreLaunchCommand` com aspas não escapadas (como o manager grava) é corrompido pelo Prism atual. Só importa se o Warden mantiver alguma exportação para o Prism (por exemplo, "exportar instância para o Prism").
4. **O que fazer com packs criados pelos projetos anteriores**: oferecer limpeza automática do lixo indexado (`*.bak`, bootstrap jar, `.packwiz.toml`) ao abrir um pack antigo no Warden?
5. **Git/GitHub no Warden**: o requisito fala em "versionar"; os dois projetos têm um painel Git completo. Qual o escopo desejado (commit/publicar com changelog de mods, ou todo o painel de branches/remotes/tags)?
6. **Snapshots locais além do git**: manter (os dois tinham) ou confiar só no git?
7. **Chave de API do CurseForge**: o packwiz-gui-manager evitou propositalmente ("nenhuma chave em lugar nenhum"). Para busca no CurseForge no Warden, como obter e proteger a chave?
8. **Temas**: o manager investiu num tema "Minecraft" alternativo e numa fonte pixel (Monocraft, SIL OFL). Vale para o Warden ou fica fora do escopo inicial?
9. **Mensagem de crash da tela de carregamento** (NeoForge/Forge mostram erros de carregamento sem gerar crash report; o Fabric lista mods incompatíveis no log): confirmar onde cada loader e cada era de versão grava essas informações, para alimentar a camada determinística. Fora do escopo desta tarefa; cabe à pesquisa de diagnóstico.

---

## Fontes

- Repositórios privados analisados (clonados em 01/10/2026): `Kriticales/packwiz-gui` (HEAD `c982456`) e `Kriticales/packwiz-gui-manager` (HEAD `2c93cc8`), incluindo `README.md`, `RELATORIO-CORRECOES.md`, `docs/SPEC.md`, `docs/ARCHITECTURE.md` e `docs/RESUME.md`.
- Código-fonte do packwiz: <https://github.com/packwiz/packwiz> (consultado localmente na versão `v0.0.0-20260902165313-9066bf845f7e`): `cmd/remove.go`, `cmd/root.go`, `core/index.go` (`FindMod`, `ignoreDefaults`), `core/pack.go` (`Options` → `viper.MergeConfigMap`), `core/mod.go` (struct `Mod`), `modrinth/modrinth.go` (`datapack-folder`).
- Documentação do packwiz-installer: <https://packwiz.infra.link/tutorials/installing/packwiz-installer/>
- Código do packwiz-installer (aceita caminho local e `file:`): <https://github.com/packwiz/packwiz-installer/blob/main/src/main/kotlin/link/infra/packwiz/installer/Main.kt>
- Prism Launcher, perda de aspas no `PreLaunchCommand`: <https://github.com/PrismLauncher/PrismLauncher/issues/1134> e <https://github.com/PrismLauncher/PrismLauncher/pull/1174>
- Experimentos desta pesquisa (pack temporário no scratchpad, binário `packwiz` local): `packwiz remove mods/<x>.pw.toml` falha e `packwiz remove <x>` funciona; `datapack-folder` só funciona em `[options]` do `pack.toml`; `packwiz refresh` indexa `*.bak`, `.packwiz.toml` e `packwiz-installer-bootstrap.jar`; `packwiz modrinth --help` lista só `add` e `export`.
