# R1 — Boas práticas e como os modpacks famosos são montados

> Documento de pesquisa do projeto Warden (tarefa R1). Status: **concluído**.
> Data da pesquisa: 2026-10-01.

## Sumário

1. Estudos de caso: como os modpacks famosos são montados e mantidos
2. Curadoria: escolha de mods, compatibilidade, performance, mods base, licenças
3. Configs: o que se ajusta e como se distribui
4. Testes e QA de modpack, JVM e Java
5. Erros comuns de iniciantes e prevenção automática
6. Implicações para o Warden
7. Questões em aberto

> Convenções: **[Inferência]** marca dedução própria, sem confirmação explícita em fonte. Dados de versões e loaders de mods vêm da API do Modrinth, e os de Java vêm do manifest da Mojang, ambos consultados em 2026-10-01. A versão mais recente do Minecraft nessa data é a 26.3.

## 1. Estudos de caso: como os modpacks famosos são montados e mantidos

> Método: leitura dos repositórios públicos (via API do GitHub e clones rasos), das páginas de projeto e dos próprios artefatos publicados. Nos `.mrpack` do Modrinth foram lidos o `modrinth.index.json` e a lista de arquivos do zip. O que está marcado como **[Inferência]** é dedução, sem confirmação explícita na fonte.

### 1.A Packs mantidos com packwiz ou ferramentas declarativas

#### Fabulously Optimized (FO): o caso de referência para o Warden

Repo: https://github.com/Fabulously-Optimized/fabulously-optimized

- **Raiz do repo:** `Packwiz/`, `Modrinth/`, `CurseForge/`, `MultiMC/`, `MultiMC-Packwiz/`, `CLI tools/`, `Resource Packs/`, `CHANGELOG.md`, `INCLUDED-MODS.md`, `DEVELOPER-README.md`, `.github/workflows/`, `crowdin.yml`.
- **Uma pasta packwiz por versão do Minecraft:** `Packwiz/1.16.5/` … `Packwiz/1.21.11/`, `Packwiz/26.1.1/`. Cada uma tem `pack.toml`, `index.toml`, `.packwizignore`, `config/`, `mods/` e `resourcepacks/`. https://github.com/Fabulously-Optimized/fabulously-optimized/tree/main/Packwiz
- **`pack.toml` (1.21.10):** `version = "11.3.3"`, `pack-format = "packwiz:1.1.0"`, `[index] hash-format = "sha256"`, `[versions] fabric = "0.19.3"`, `minecraft = "1.21.10"`. https://github.com/Fabulously-Optimized/fabulously-optimized/blob/main/Packwiz/1.21.10/pack.toml
- **Metafiles `.pw.toml`:** `[download]` aponta para `cdn.modrinth.com` com sha512 e `[update.modrinth]`. Não há mods opcionais.
- **Arquivos ignorados e compartilhados:** `.packwizignore` é um symlink compartilhado entre versões (`packwiz`, `Readme.md`, `*.exe`, `*.bat`, `*.jar`, `*.url`). Nenhum JAR é versionado no git. https://github.com/Fabulously-Optimized/fabulously-optimized/blob/main/DEVELOPER-README.md
- **Configs padrão:**
  - Versões antigas (1.20.1): ficavam em `config/yosbr/` (mod YOSBR).
  - Versões recentes (1.21.10): o pack migrou para o **Config Manager**, com padrões em `config/modpack_defaults/options.txt` e `config/modpack_defaults/config/*` (`sodium-options.json`, `iris.properties`, `modmenu.json`…). O mod se descreve como forma de "enviar configs sem bagunçar as preferências do usuário": https://modrinth.com/mod/configmanager
  - Configs autoritativos ficam direto em `config/` (ex.: `fabric_loader_dependencies.json`).
- **Resource packs próprios:** o código-fonte fica em `Resource Packs/`. No pack, entram como metafile Modrinth ou zip local.
- **Fluxo de build (DEVELOPER-README), híbrido e parcialmente manual:**
  1. Edita e testa no launcher do CurseForge.
  2. Roda `Update version.py`.
  3. Exporta o ZIP do CurseForge.
  4. Converte para MultiMC e para packwiz com scripts Python. A conversão usa **mmc-export** (`-f packwiz --provider-priority Modrinth CurseForge Other`) ou `packwiz curseforge import`.

  Fontes: https://github.com/Fabulously-Optimized/fabulously-optimized/blob/main/CLI%20tools/CurseForge%20to%20Packwiz-Modrinth.py, https://github.com/RozeFound/mmc-export
- **CI (`auto-publish.yml`), disparado por `release: published`:**
  - Roda `packwiz modrinth export` na pasta da versão.
  - Gera atestação de proveniência (`actions/attest-build-provenance`).
  - Publica no Modrinth e no CurseForge com `Kira-NT/mc-publish` (antigo Kir-Antipov/mc-publish) e anexa o `.mrpack` à release do GitHub.

  https://github.com/Fabulously-Optimized/fabulously-optimized/blob/main/.github/workflows/auto-publish.yml
- **Auto-update em MultiMC/Prism:** `instance.cfg` com `PreLaunchCommand="$INST_JAVA" -jar packwiz-installer-bootstrap.jar https://raw.githubusercontent.com/.../Packwiz/1.21.4/pack.toml`. O raw do GitHub serve de hospedagem.
- **Changelog:** `CHANGELOG.md` manual, agrupado por "era" (ex.: "Wilderness Bound (15.x.x)"), depois por versão do MC. Cada release (`### 15.0.0-alpha.4 (2026-09-27)`) lista "Readded / Updated / Temporarily removed". Versionamento **semver com pré-releases**. https://github.com/Fabulously-Optimized/fabulously-optimized/blob/main/CHANGELOG.md

#### Additive / Adrenaline / Adrenaserver (SkywardMC)

Repo: https://github.com/skywardmc/additive. Não pertence à organização do FO.

- **Layout `versions/<loader>/<mc>/`** (ex.: `versions/fabric/26.3/`), com `archived/` para versões antigas.
- **Versão com metadado de build no `pack.toml`:** `version = "26.5.1+mc26.3.fabric"`. Usa `[options] no-internal-hashes = true` para evitar conflitos de merge.
- **`side` bem usado:** na versão 26.3 há 21 mods `both`, 33 `client` e 2 `server` (`krypton`, `vmp-fabric`).
- **Arquivos por versão:**
  - `.gitattributes` com `* -text` ("para não mudar hashes no Windows").
  - `.gitignore` com `*.zip`, `*.mrpack`.
  - `.packwizignore` com `/README.md`.
  - Configs via Config Manager (`config/modpack_defaults/`).
- **`justfile`:** receitas `export`, `refresh` e `update` (`packwiz update --all`) iteram por todas as pastas de versão. https://github.com/skywardmc/additive/blob/main/justfile
- **CI `build-dist.yml`:** copia as pastas com `pack.toml` para um branch órfão `dist` e roda `packwiz refresh --build`, que gera os hashes omitidos. Tags viram `dist/<tag>`. **[Inferência]** Isso dá uma URL estável para o packwiz-installer. https://github.com/skywardmc/additive/blob/main/.github/workflows/build-dist.yml
- **Servidor (README do Adrenaserver):** documenta 4 vias.
  1. `mrpack-install`
  2. Docker `itzg/minecraft-server` com `MODRINTH_MODPACK`
  3. `mcman`
  4. `java -jar packwiz-installer-bootstrap.jar -g -s server <url>/pack.toml`

  https://github.com/skywardmc/adrenaserver

#### Craftoria (TeamAOF, NeoForge 1.21.1)

Repo: https://github.com/TeamAOF/Craftoria

- **O repositório é a própria instância**, com packwiz na raiz. `pack.toml` com `[versions] neoforge = "21.1.249"`. O campo `neoforge` funciona, embora a página de referência do packwiz ainda não o liste.
- **`.packwizignore` em modo *allowlist*:** `/*` seguido de `!/config`, `!/mods`, `!/kubejs`…, e ainda exclui caches e configs pessoais (`/config/xaero`, `/config/iris.properties`, `*.disabled`). **Esse é o padrão que o Warden deve adotar** (ver E7).
- **`automation/`:** `modpack-uploader.ps1` + `settings.ps1` fazem o upload para o CurseForge, com changelog gerado pelo ModListCreator.
- **`server_files/`:** `server-setup-config.yaml` + `startserver.sh`. **[Inferência]** É o formato do ServerStarter.
- **`InstanceSync.jar`** roda num hook git `post-merge` para sincronizar a instância após um `git pull`.
- **Números do clone:** 526 mods, todos `mode = "metadata:curseforge"` (`project-id`/`file-id`), e **todos com `side = "both"`**. Mesmo usando packwiz, o pack não marca mods só de cliente.
- **Uploader com packwiz:** `automation/modpack-uploader.ps1` exige packwiz, roda `packwiz refresh` e gera o zip com `packwiz cf export`, usando o fork https://github.com/WhitePhant0m/packwiz.
- **Changelog por commits:** `automation/scripts/generate-changelog.js` gera o changelog a partir de **Conventional Commits**. Entram só `feat` e `fix`; os escopos `dev` e `dx` ficam fora; o corte é no último bump de versão ou tag. https://github.com/TeamAOF/Craftoria/tree/main/automation/scripts
- **Arquivos de controle:** `.gitignore` e `.packwizignore` começam com `/*` e reabrem só o necessário (https://github.com/TeamAOF/Craftoria/blob/main/.gitignore, https://github.com/TeamAOF/Craftoria/blob/main/.packwizignore). O `.packwizignore` exclui `ae2-client.toml`, `sodium-fingerprint.json` e `kubejs/*/jsconfig.json`.
- **Servidor:** fork do ServerStarter (https://github.com/TeamAOF/ServerStarter) com `modpackFormat: curse` e `modpackUrl` apontando para o forgecdn.

#### GregTech Odyssey (Forge 1.20.1)

Repo: https://github.com/GregTech-Odyssey/GregTech-Odyssey

- **Mod opcional real:** `mods/oculus.pw.toml` com `side = "client"` e `[option] optional = true, description = "..."`.
- **CI:** `refresh.yml` roda `packwiz refresh` e faz auto-commit. `package.yml` gera o export do CurseForge via Nix.

#### Packs com outras ferramentas declarativas

- **TerraFirmaGreg Modern** usa **Pakku** em vez de packwiz (`pakku.json`, `pakku-lock.json`). Separa `overrides`, `client_overrides` e `server_overrides`, e marca `projects.<slug>.side = "CLIENT"`. https://github.com/TerraFirmaGreg-Team/Modpack-Modern
- **Monifactory** e **Star Technology**: ver 1.C.

### 1.B Packs montados em launcher e exportados (sem repositório público)

O que se vê nos artefatos publicados no Modrinth:

- **Better MC [Forge] BMC4** (https://modrinth.com/modpack/better-mc-forge-bmc4):
  - No `.mrpack` v59, os 340 arquivos estão **todos** marcados `client=required, server=required`, sem separação de lado.
  - 21 JARs embutidos em `overrides/mods/`.
  - Configs: `overrides/defaultconfigs/`, `overrides/configureddefaults/options.txt` (mod Configured Defaults: https://modrinth.com/mod/configured-defaults) e também `overrides/options.txt`.
  - Paxi, FTB Quests, FancyMenu, `resourcepackoverrides.json` e **3256 arquivos de cache em `overrides/modernfix/structureCacheV1`**.
  - O server pack é um arquivo separado no CurseForge (cerca de 1 GB): https://www.curseforge.com/minecraft/modpacks/better-mc-forge-bmc4/files/7736926
  - **[Inferência]** Foi exportado de uma instância jogada, e o cache do ModernFix vazou para o pack.
- **Prominence II: Hasturian Era** (https://modrinth.com/modpack/prominence-2-fabric):
  - 555 arquivos: 345 nos dois lados e 210 com `server: unsupported`.
  - 36 JARs em `overrides/mods/`.
  - YOSBR (`config/yosbr/options.txt`), mas **também** `overrides/options.txt`.
  - 658 arquivos em `shaderpacks/` e cache do ModernFix.
  - Server pack separado no CurseForge: https://www.curseforge.com/minecraft/modpacks/prominence-2-hasturian-era/files/8726519
- **Cobblemon Official Modpack [Fabric]** (https://modrinth.com/modpack/cobblemon-fabric):
  - 75 arquivos, todos do CDN do Modrinth; 41 com `server: unsupported`.
  - YOSBR distribui inclusive um **mundo tutorial** (`config/yosbr/saves/Tutorial World v4/`), copiado só na primeira execução.
  - Open Loader (datapacks globais), FancyMenu, CraftPresence.
- **COBBLEVERSE** (https://modrinth.com/modpack/cobbleverse):
  - 168 arquivos, todos `required/required`.
  - Default Options (`config/defaultoptions/options.txt`, `keybindings.txt`, `servers.dat`).
  - Inclui `overrides/licenses/` com 159 arquivos de licença e um PDF "Third-Party Licenses", um bom exemplo de cuidado com redistribuição.
- **Simply Optimized** está **descontinuado**: a API do Modrinth retorna `status: withheld` (https://api.modrinth.com/v2/project/sop). Continua como "Simply Optimized Forked", que promete virar projeto packwiz aberto: https://www.curseforge.com/minecraft/modpacks/simply-optimized-forked

**Lição dos casos 1.B:** packs enormes e populares saem com **lixo de instância** (caches do ModernFix, milhares de arquivos de shaderpack), **sem marcação de lado** e com `options.txt` sobrescrevendo o jogador **ao mesmo tempo** que usam YOSBR. Isso confirma, com dados, os erros E5, E6 e E7 da seção 5 e o valor de uma ferramenta que previna esses problemas.

### 1.C Packs grandes "de curadoria pesada" (kitchen sink, expert, questing)

#### All The Mods (ATM9 / ATM10)

Repos: https://github.com/AllTheMods/ATM-9, https://github.com/AllTheMods/ATM-10

- **Só os *overrides* vão para o git.** O repo não tem lista de mods (`/mods` no `.gitignore` do ATM-9). O pack sai no CurseForge como All Rights Reserved.
  - ATM-9: `config/`, `defaultconfigs/`, `kubejs/`, `packmenu/`.
  - ATM-10: `config/`, `defaultconfigs/`, `kubejs/`, `datapacks/`, `local/`, `changelogs/`, `CHANGELOG.md`, `MOD_ISSUES.md`.
- **`.gitignore` do ATM-9:** `/mods`, `/saves`, `/logs`, `/resourcepacks`, `options.txt`, `servers.dat`, `usercache.json`, `/journeymap`, `/XaeroWorldMap`, `kubejs/probe/`, `jsconfig.json`, `/local`, `/simplebackups`. Esta é a lista de "lixo" típica, útil como base do filtro do Warden. https://github.com/AllTheMods/ATM-9/blob/main/.gitignore
- **Empacotamento histórico (ATM6)**, PowerShell: https://github.com/AllTheMods/atm6-packaging/blob/main/export.ps1
  - Lê o `minecraftinstance.json` do launcher do CurseForge.
  - Mantém só `config`, `defaultconfigs`, `kubejs` e `packmenu`.
  - Tira a versão de `config/allthetweaks-common.toml`.
  - Monta o server pack removendo uma lista de **projectIDs** de mods só de cliente (Xaero's Minimap, Ding, FancyMenu, Oculus, Embeddium, PackMenu…).
- **Versionamento `MAJOR.MINOR`** (2.25 … 8.2), com candidatos `-rc1`.
- **Changelog:** um arquivo por salto, `changelogs/CHANGELOG-ATM10-8.1-8.2.md`, com "General changes", lista de commits, mods Added/Updated no formato `Mod (v1) -> (v2)` e até **diff das receitas** alteradas. https://github.com/AllTheMods/ATM-10/blob/main/changelogs/CHANGELOG-ATM10-8.1-8.2.md. No passado o ATM usou o https://github.com/TheRandomLabs/ChangelogGenerator.
- **Quests:** `config/ftbquests/quests/` (`chapters/`, `chapter_groups.snbt`, `data.snbt`, `reward_tables/`, `lang/<locale>/` em 15 idiomas, incl. `pt_br`). O FTB Quests Lang Splitter separa as traduções.
- **Outros configs:** BetterCompatibilityChecker (`bcc-common.toml`, valida que cliente e servidor estão na mesma versão do pack) e `crash_assistant`.
- **Servidor:** server pack em "Additional Files" com `startserver.sh/.bat`, `user_jvm_args.txt` e o instalador do NeoForge, executado na primeira vez. https://allthemods.github.io/alltheguides/help/server/. O script histórico é https://github.com/AllTheMods/Server-Scripts: instala o loader, checa Java e EULA, reinicia após crash.
- **Issues:** templates YAML (crash, quest, recipe) com "Modpack Version" obrigatório. `MOD_ISSUES.md` aponta o tracker de cada mod.

#### Enigmatica (Enigmatica 9; o 6 segue o mesmo modelo)

Repo: https://github.com/EnigmaticaModpacks/Enigmatica9

- **O repo é uma instância do launcher do CurseForge.** O dev clona dentro de `Instances/` e roda `automation/InstanceSyncSetup.bat`. O `InstanceSync.jar` baixa os mods a partir do `minecraftinstance.json` versionado.
- **Publicação com o ModpackUploader** (PowerShell): https://github.com/NillerMedDild/ModpackUploader, configurado em `automation/settings.ps1`.
  - `$FOLDERS_TO_INCLUDE_IN_CLIENT_FILES`: config, defaultconfigs, kubejs, local, packmenu, patchouli_books, schematics.
  - `$CONFIGS_TO_REMOVE_FROM_CLIENT_FILES`: `*-client.toml`, `jei/bookmarks.ini`, `defaultoptions/servers.dat`…
  - Módulos para client zip, server zip, ServerStarter, changelog e modlist.
  - Canal `alpha|beta|release`. Token em `automation/secrets.ps1` (no `.gitignore`).
  - https://github.com/EnigmaticaModpacks/Enigmatica9/blob/master/automation/settings.ps1
- **`.gitignore`:** ignora `**/*.jar`, `/mods/`, `/saves/`, `/logs/`, `options.txt`, `eula.txt`, `ops.json`, e reabre `!config/defaultoptions/options.txt` e `!server_files/**`. https://github.com/EnigmaticaModpacks/Enigmatica9/blob/master/.gitignore
- **Versão:** SemVer `1.MINOR.PATCH`.
- **Changelog em três camadas:**
  - `changelogs/CHANGELOG.md` curado (🐛 Fixed Bugs, 🎁 New Mods Added, 🌟 Improvements), com links para issues.
  - `changelog_mods_<ver>.md`, gerado comparando com o zip anterior.
  - `modlist_<ver>.md`.
  - `prepare-for-release.ps1` fecha automaticamente as issues com a label "Fixed In Next Release".
  - https://github.com/EnigmaticaModpacks/Enigmatica9/blob/master/changelogs/CHANGELOG.md
- **Defaults:** `config/defaultoptions/options.txt` + `keybindings.txt` (Default Options) e `defaultconfigs/` com serverconfigs.
- **Servidor:**
  - `automation/remove-client-mods.ps1` remove do server pack os jars cujo nome **começa com** itens de uma lista (Ding, Neat, PackMenu, BetterF3…).
  - `server_files/` tem `server-setup-config.yaml` e `start-server.sh/.bat`, que baixam o **ServerStarter** (https://github.com/EnigmaticaModpacks/ServerStarter). O `modpackUrl` é reescrito a cada release.
  - O modo Expert tem `server_files_expert/`.
- **Memória:** o guia do pack diz que "Assigning more than shown here is never recommend, as you may actually get worse performance". https://github.com/EnigmaticaModpacks/Docs-Main/blob/master/help-desk/guides/allocating-memory.md

#### Monifactory (Omicron-Industries)

Repo: https://github.com/Omicron-Industries/Monifactory. É o exemplo mais avançado de QA automatizado.

- **Build próprio:** `manifest.json` (formato CurseForge, Forge 1.20.1, 223 mods) + build em TypeScript (`build/`, sobre o juke) com alvos `switch-pack-mode`, `run-dedicated-server` (monta o servidor do zero a partir do checkout) e `run-gametest-server`. https://github.com/Omicron-Industries/Monifactory/blob/main/build/README.md
- **Mods só de cliente:** lista hardcoded `clientMods` (oculus, embeddium, fancymenu…), filtrada por substring do nome do jar.
- **Modos de dificuldade:** `config-overrides/{normal,hardmode,expert}/` guarda variantes de arquivos (inclusive capítulos de quest), copiadas por cima na troca de modo. https://github.com/Omicron-Industries/Monifactory/blob/main/CONTRIBUTING.md
- **QA em CI:**
  - `pr-check-gametests.yml` sobe um **servidor de gametest para cada modo** (matriz normal/hard/expert) sempre que um PR muda config, kubejs ou manifest.
  - Lint de KubeJS (ESLint).
  - `release.yml` (manual) escolhe stable/pre-release, tipo no CurseForge e **incremento SemVer major/minor/patch**, gera release notes e publica no GitHub e no CurseForge.
  - Traduções via Weblate (branch `lang`).
- **Depreciação controlada:** itens removidos passam por `kubejs/startup_scripts/deprecations.js` e só somem duas minors depois. É uma boa prática contra o problema E8 (mundo quebrado por remoção).

#### GregTech: New Horizons (GTNH, 1.7.10)

- **Montagem pelo DreamAssemblerXXL** (Python, CLI e GUI): https://github.com/GTNewHorizons/DreamAssemblerXXL
  - `gtnh-assets.json`: catálogo de mods e versões (maioria de releases do GitHub).
  - `releases/manifests/<ver>.json`: por release, `config` (tag do repo de configs) e mods com `{version, side: BOTH|CLIENT|SERVER}`.
  - `gtnh-modpack.json`: `server_exclusions`, `client_exclusions`, `server_java8_exclusions`, `server_java9_exclusions`.
  - Alvos: zip cliente/servidor, Prism/MultiMC, Technic; Curse/Modrinth em andamento.
- **Builds diários** por cron (`daily-modpack-build.yml`), publicados em `GTNewHorizons/GTNH-Daily-Builds` com retenção de 14 dias, além de builds "experimental" e do workflow `modpack-test.yml`. https://github.com/GTNewHorizons/DreamAssemblerXXL/blob/master/.github/workflows/daily-modpack-build.yml
- **Repo de configs:** https://github.com/GTNewHorizons/GT-New-Horizons-Modpack
  - `.gitignore` em *allowlist* granular: só os configs **efetivamente alterados** pela equipe são versionados (ex.: `!config/GregTech/GregTech.cfg`), o resto é bloqueado.
  - Quests em BetterQuesting (`config/betterquesting/DefaultQuests/`).
  - `changelog.py <from> <to>` gera o changelog.
- **Versão:** `2.MINOR.PATCH` com `-beta-N`, `-RC-N`, `-pre`.
- **Distribuição** pelo próprio site, não pelo CurseForge.
- **Java e memória:** com lwjgl3ify, o pack roda em Java 17–25; o Java 25 é recomendado para o 2.8+. Também recomenda 4–6 GB e avisa: "Allocating more than 8GB with Java 8 will likely cause serious stuttering". https://wiki.gtnewhorizons.com/wiki/Installing_and_Migrating

#### FTB (FTB Evolution, StoneBlock, Skies)

- **Sem repositório público dos packs.** Publicação via API própria, consumida pelo FTB App (https://github.com/FTBTeam/FTB-App) e também no CurseForge.
- **Manifest de versão** (ex.: FTB Evolution 1.44.0): https://api.feed-the-beast.com/v1/modpacks/public/modpack/125/100506
  - `targets`: MC 1.21.1, NeoForge 21.1.248 e **Java 21.0.4 como runtime**.
  - `specs`: RAM mínima e recomendada.
  - `files[]`: cerca de 7.600 entradas com `path`, `url` endereçada por hash, `hashes` (sha1/sha256/sha512/murmur), `size`, **`clientonly`/`serveronly`** (29 arquivos só de cliente no Evolution).
- **Defaults:** o mod **Configured Defaults** espelha `configureddefaults/` (ex.: `configureddefaults/options.txt`) para `.minecraft` só se o destino não existir. Também usa `defaultconfigs/` e `config/defaultoptions/`.
- **FTB Pack Companion:** utilitários de pack (gamerules forçadas, aviso de shaders, remoção de toasts, seed forçada). https://github.com/FTBTeam/FTB-Pack-Companion
- **Versão:** `1.MINOR.PATCH` com release/beta/alpha.
- **Changelog:** seções manuais (Changed/Fixed) + "Mod Updates" **gerado automaticamente** como diff de nomes de jar (Added/Removed/Updated `a.jar -> b.jar`).
- **Servidor:** FTB-Server-Installer, binário Go (`-auto`, `-validate`, `-latest`, `-no-java`) que baixa o manifest, os arquivos não `clientonly` e o loader. https://github.com/FTBTeam/FTB-Server-Installer
- **Issues:** tracker único para todos os packs. https://github.com/FTBTeam/FTB-Modpack-Issues

#### Create: Above and Beyond (CABA)

Repo: https://github.com/simibubi/Above-and-Beyond (inativo desde 2023, MC 1.16.5).

- Repo de instância exportada do launcher do CurseForge: `manifest.json` (136 mods) + `modlist.html` + `config/`, `defaultconfigs/`, `kubejs/`, `openloader/` (datapacks e resources globais), `options.txt` versionado, `config/ftbquests`, `config/defaultoptions`.
- `.gitignore` clássico: `mods/**`, `saves/**`, `logs/**`, `servers.dat`, `usercache.json`, `optionsof.txt`.
- Sem automação nem tags.

#### Vault Hunters

- Não há repositório público do pack (https://github.com/orgs/Iskallia/repositories). Distribuição no CurseForge, com server files em Additional Files.
- O **VH-ServerHelper** (`vh-setup.bat`) instala o Forge e gera `run.bat/.sh` + `user_jvm_args.txt`. https://github.com/Iskallia/VH-ServerHelper
- Bugs são reportados no Discord (segundo um resumo de busca, não verificado na fonte primária).

#### Star Technology (GregTech CEu Modern, 1.20.1)

Repo: https://github.com/StarT-Dev-Team/Star-Technology

- `compile_data/manifest.json` (CurseForge) + `config/`, `defaultconfigs/`, `kubejs/`, `tools/`. ESLint e Prettier para KubeJS, e até `AGENTS.md`/`CLAUDE.md` com instruções para agentes de IA.
- `.gitignore` em *allowlist*.
- **Build em CI:** a action `compile-modpack` monta `overrides/`, gera o zip e avisa no Discord. https://github.com/StarT-Dev-Team/Star-Technology/tree/main/.github/actions/compile-modpack
- Outra cópia do repo (https://github.com/trulyno/Star-Technology) mostra *pack modes* (`packmode/` + `packmode_picker.{py,sh,bat}`). *Em aberto: qual dos dois repositórios é o atual.*

### 1.D Padrões transversais (o que todos fazem, com ou sem packwiz)

1. **O repositório é a instância, filtrada por *allowlist*.** O padrão moderno é `.gitignore`/`.packwizignore` começando com `/*` e reabrindo só `config/`, `defaultconfigs/`, `kubejs/` etc. (Monifactory, Craftoria, Star Technology, GTNH). O GTNH vai além e versiona **só os configs alterados**.
2. **Sempre fora do pack:** `mods/*.jar` (são referenciados, não embutidos), `saves/`, `logs/`, `crash-reports/`, `options.txt` raiz (vai via mecanismo de "padrão na primeira execução"), `servers.dat`, `usercache.json`, `journeymap/`/`XaeroWorldMap/`, `kubejs/probe/`, `jsconfig.json`, `local/`, backups, caches (`modernfix/`, `.mixin.out`), segredos.
3. **A lista de mods é declarativa e com hash:** `minecraftinstance.json`/`manifest.json` (CurseForge), `.pw.toml` (packwiz), `pakku-lock.json` (Pakku) ou manifest próprio (FTB, GTNH).
4. **Lado (cliente/servidor):** só FTB, GTNH, FO, Additive e Pakku/TFG guardam isso como **metadado**. Os demais mantêm listas hardcoded de nomes de jar no script de servidor, um sistema frágil. Mesmo com packwiz, o Craftoria deixa tudo `both`. **Oportunidade clara para o Warden:** preencher `side` automaticamente a partir do Modrinth e do jar.
5. **"Padrão na primeira execução" para preferências:** Default Options (`config/defaultoptions/`), Configured Defaults (`configureddefaults/`), YOSBR (`config/yosbr/`), Config Manager (`config/modpack_defaults/`), e `defaultconfigs/` para serverconfigs.
6. **Changelog em duas partes:** texto curado (CHANGELOG.md manual ou Conventional Commits) **+ diff automático de mods** (Added/Removed/Updated com versões), às vezes com diff de receitas (ATM10). Ferramentas: TheRandomLabs ChangelogGenerator, ModListCreator, modpack-changelogger (`-o old.mrpack -n new.mrpack`, https://github.com/TheBossMagnus/modpack-changelogger), mrpack-diff-viewer (https://github.com/karilaa-dev/mrpack-diff-viewer).
7. **Versionamento:** SemVer `1.x.y` com canal alpha/beta/release (Enigmatica, FTB, Craftoria, Monifactory, FO); `MAJOR.MINOR` (ATM10); pré-lançamentos `-beta-N`/`-RC-N`/dailies (GTNH). Metadado de build com MC e loader no Additive (`26.5.1+mc26.3.fabric`).
8. **Server pack:** artefato separado. Pode vir de ServerStarter + yaml, scripts `startserver` + instalador do loader, instalador binário (FTB), packwiz-installer `-s server` (Additive) ou montagem do zero no build (Monifactory, GTNH). Sempre com `user_jvm_args.txt` ou equivalente.
9. **QA:** a maioria não documenta QA. Os melhores exemplos são o Monifactory (gametests em servidor dedicado no CI, por modo de dificuldade) e o GTNH (dailies + experimental + `modpack-test.yml`). Templates de issue exigem a versão do pack. BetterCompatibilityChecker e Crash Assistant aparecem nos packs grandes.
10. **Proveniência e reprodutibilidade:** o FO atesta a proveniência do `.mrpack` em CI. O packwiz recomenda `.gitattributes` com `* -text` para os hashes não mudarem no Windows (https://packwiz.infra.link/tutorials/creating/git/). **Isso é crítico para o Warden, cuja plataforma principal é Windows.**

### 1.E O que o packwiz oferece (e onde estão as armadilhas) para essas práticas

Fontes: a documentação oficial e o código-fonte do packwiz.

- **Campos do `.pw.toml`** (`core/mod.go`, https://github.com/packwiz/packwiz/blob/main/core/mod.go): `name`, `filename`, `side` (`client`, `server`, `both` ou vazio, que equivale a `both`), `pin`, `[download] url/hash-format/hash/mode` (`mode = "metadata:curseforge"` para mods do CurseForge), `[update.<provider>]`, `[option] optional/description/default`.
- **Export para Modrinth** (`modrinth/export.go`, https://github.com/packwiz/packwiz/blob/main/modrinth/export.go):
  - `optional = true` vira `env = "optional"`.
  - `side = "client"` vira `server: "unsupported"`, e `side = "server"` vira `client: "unsupported"`.
  - Arquivos sem metadado Modrinth vão para `overrides/`, `client-overrides/` ou `server-overrides/`, conforme o lado.
  - O `--restrictDomains` vem ligado por padrão: arquivos fora da whitelist do Modrinth (inclusive **todo mod do CurseForge**) são **embutidos** nos overrides, com aviso. Isso só é aceitável se a licença permitir (seção 2.4).
- **Limitações por plataforma** (https://packwiz.infra.link/tutorials/hosting/modrinth/, https://packwiz.infra.link/tutorials/hosting/curseforge/):
  - O app oficial do Modrinth **não pergunta pelos opcionais e instala todos**. O Prism pergunta.
  - O formato CurseForge **não tem `side`**. `packwiz curseforge export --side` escolhe o que entra (o padrão é o cliente).
  - Jars que não vêm do CurseForge exigem aprovação manual na plataforma.
- **`[options]` do `pack.toml`** (https://packwiz.infra.link/reference/additional-options/): `acceptable-game-versions`, `meta-folder`, `datapack-folder`, `no-internal-hashes` (evita conflitos de merge; requer `packwiz refresh --build` antes de distribuir).
- **`.packwizignore`:** sintaxe de `.gitignore`, com *defaults* embutidos (`.git/**`, `.gitattributes`, `.gitignore`, `/*.zip`, `*.mrpack`, `packwiz.exe`, `packwiz`). https://github.com/packwiz/packwiz/blob/main/core/index.go. A página de referência da doc ainda é "TODO".
- **`preserve`** em entradas do `index.toml`: não sobrescreve o arquivo se ele já existir (seção 3.2).
- **Servidor:** `java -jar packwiz-installer-bootstrap.jar -g -s server <url>/pack.toml` instala só `both` e `server` (https://packwiz.infra.link/tutorials/installing/packwiz-installer/). A imagem Docker `itzg/minecraft-server` aceita `PACKWIZ_URL`.
- **Mods do CurseForge com distribuição bloqueada:** a API retorna `downloadUrl = null`, e o packwiz-installer cai em "This mod is excluded from the CurseForge API and must be downloaded manually". https://github.com/packwiz/packwiz-installer/blob/main/src/main/kotlin/link/infra/packwiz/installer/metadata/curseforge/CurseForgeSourcer.kt
- **Fluxo básico documentado:** `init` → `modrinth install`/`curseforge install` → `refresh` → `serve` (testa em `http://localhost:8080/pack.toml`) → git. https://packwiz.infra.link/tutorials/creating/getting-started/
- **CI:** o binário do packwiz vem por `go install`, por artifact ou pela action `hudsonm62/packwiz-action`. A publicação usa `Kira-NT/mc-publish`. https://github.com/Kira-NT/mc-publish
- **Não há gerador de changelog nativo no packwiz.** Verificado: um resumo de busca que dizia o contrário não procede.


## 2. Curadoria: escolha de mods, compatibilidade, performance, mods base e licenças

> Faixas de versão e loaders vêm da API do Modrinth (`/v2/project/{slug}` e `/v2/project/{slug}/version`, consultada em 2026-10-01). Essas faixas são o agregado de `game_versions` e podem ter buracos no meio. A versão mais recente do Minecraft é a **26.3** (manifest da Mojang, seção 4).

### 2.1 Princípios de escolha de mods (síntese dos estudos de caso)

Os princípios abaixo saem do que os packs da seção 1 fazem na prática. A formulação como regras é **[Inferência]** minha.

1. **Tema e escopo antes da lista.** Packs de otimização (FO, Additive) têm escopo estrito: nada que mude a jogabilidade. Packs expert (GTNH, Monifactory, Star Technology) giram em torno de um mod central e reescrevem receitas via KubeJS/CraftTweaker. *Kitchen sink* (ATM) prioriza variedade, mas dedica um trabalho grande a unificação e quests.
2. **Uma única opção por "categoria exclusiva".** Exemplos: um renderizador (Sodium/Embeddium/OptiFine), um motor de luz, um *recipe viewer* principal (JEI, REI ou EMI), um minimapa e um sistema de slots de acessórios (Curios, Trinkets ou Accessories) quando possível.
3. **Unificação de materiais** em packs com muitos mods de tecnologia: vários mods geram "cobre", "estanho" etc. Packs resolvem isso com Almost Unified (https://modrinth.com/mod/almostunified, ARR, 1.18.2–1.21.1), Emendatus Enigmatica ou KubeJS.
4. **Mods mantidos.** Preferir projetos com versão recente para a versão alvo. Vários mods populares foram **abandonados ou arquivados** (tabela 2.3), e incluí-los em pack novo é dívida técnica.
5. **Mods com lado corretamente declarado** e, se possível, hospedados no Modrinth (melhor metadado, `environment`, dependências tipadas, licença SPDX).
6. **Cuidado com camadas de compatibilidade entre loaders.** Sinytra Connector (Forge/NeoForge, 1.20.1–26.1.2, MIT: "Lets you play Fabric mods on NeoForge", https://modrinth.com/mod/connector) amplia o catálogo, mas adiciona uma classe inteira de bugs. Nenhum dos packs grandes estudados o usa. A NeoForge rejeita jars com metadados Fabric + Forge sem NeoForge: https://github.com/Sinytra/Connector/issues/1934.
7. **Depreciar antes de remover** conteúdo que existe no mundo (Monifactory, `deprecations.js`, seção 1.C).

### 2.2 Checagem de compatibilidade: camadas

1. **Metadados declarados** (seção 5.1):
   - Modrinth: `dependencies[].dependency_type`, `game_versions`, `loaders`, `environment`.
   - Dentro do jar: `fabric.mod.json`, `neoforge.mods.toml`, `mods.toml`, `mcmod.info`.
2. **Regras curadas** para conflitos que **não** estão declarados (tabela 2.3.4).
3. **Teste real**: boot do cliente, mundo novo, boot de servidor dedicado (seção 4).
4. **Análise do log** com padrões conhecidos (codex-minecraft/mclo.gs, seção 4.4) e, por último, IA.

### 2.3 Mods de performance por loader e versão

#### 2.3.1 Versões modernas (1.16.5+)

| Mod | Loaders | Versões MC | Função | Status / incompatibilidades |
|---|---|---|---|---|
| **Sodium** | Fabric, **NeoForge**, Quilt | 1.16.3 – 26.3 | Motor de renderização | NeoForge desde 0.6 (`mc1.21-0.6.0-beta.1-neoforge`, 22/08/2024). Exige OpenGL 4.5+ nas versões recentes. Licença Polyform Shield (não é OSI). https://modrinth.com/mod/sodium, https://github.com/CaffeineMC/sodium |
| **Iris** | Fabric, NeoForge, Quilt | 1.16.5 – 26.3 | Shaders (com Sodium) | NeoForge desde 1.8.0 (1.21). https://modrinth.com/mod/iris |
| **Sodium Extra** | Fabric, NeoForge, Quilt | 1.16.2 – 26.3 | Opções extras para Sodium | https://modrinth.com/mod/sodium-extra |
| **Indium** | Fabric, Quilt | 1.16.5 – 1.21.1 | FRAPI no Sodium | **Obsoleto e incompatível com Sodium ≥0.6** (que já inclui FRAPI). https://github.com/comp500/Indium |
| **Embeddium** | Forge (≤1.20.1), NeoForge | 1.16.5 – 1.21.1 | Fork do Sodium 0.5.8 | Último arquivo NeoForge de 01/2025. **[Inferência]** Substituído pelo Sodium no NeoForge 1.21+. Use-o em Forge 1.18.2–1.20.1. https://modrinth.com/mod/embeddium |
| **Rubidium** | Forge | 1.16.5 – 1.20.1 | Port antigo do Sodium | Parado desde 09/2023. Preferir Embeddium. https://modrinth.com/mod/rubidium |
| **Oculus** | Forge, NeoForge | 1.16.5 – 1.20.1 | Iris para Forge | No 1.21+ usar Iris no NeoForge. https://modrinth.com/mod/oculus |
| **Lithium** | Fabric, **NeoForge**, Quilt | 1.16.2 – 26.3 | Lógica de jogo (física, IA, ticks) sem mudar comportamento | NeoForge desde 0.14 (10/2024). https://modrinth.com/mod/lithium |
| **Canary** / **Radium** | Forge / Forge, NeoForge | 1.18.2–1.20.4 / 1.20–1.21.1 | Forks não oficiais do Lithium | Parados (2024). **Não combinar** entre si nem com o Lithium. |
| **ModernFix** | Forge, NeoForge (Fabric até 1.20.1) | 1.16.4 – 26.1.2 | Boot, memória, correções | Não suporta OptiFine. O fork comunitário **ModernFix-mVUS** cobre versões que o oficial pula. https://modrinth.com/mod/modernfix, https://modrinth.com/mod/modernfix-mvus |
| **FerriteCore** | Fabric, Forge, NeoForge, Quilt | 1.16.5 – 26.3 | Memória | https://modrinth.com/mod/ferrite-core |
| **Starlight** | Fabric/Forge | 1.17 – 1.20.4 | Motor de luz | **Arquivado em 03/2024.** O motor vanilla da 1.20 incorporou o design dele. https://github.com/Tuinity/Starlight |
| **ScalableLux** | Fabric, NeoForge | 1.21 – 26.3 | Sucessor do Starlight | https://modrinth.com/mod/scalablelux |
| **C2ME** | Fabric | 1.17.1 – 26.3 | Geração e I/O de chunks em paralelo | Pode quebrar geradores de mods. Faça backup. https://modrinth.com/mod/c2me-fabric |
| **EntityCulling** | Fabric, Forge, NeoForge | b1.7.3 – 26.3 | Culling de entidades | Só cliente. https://modrinth.com/mod/entityculling |
| **ImmediatelyFast** | Fabric, Forge, NeoForge | 1.18.2 – 26.3 | Renderização em modo imediato (HUD, texto) | Só cliente. https://modrinth.com/mod/immediatelyfast |
| **More Culling** | Fabric, NeoForge | 1.18 – 26.3 | Culling extra | Só cliente. https://modrinth.com/mod/moreculling |
| **Dynamic FPS** | Fabric, Forge, NeoForge | 1.14.3 – 26.3 | Reduz FPS em segundo plano | https://modrinth.com/mod/dynamic-fps |
| **BadOptimizations** | Fabric, Forge, NeoForge | 1.19.1 – 26.3 | Micro-otimizações | https://modrinth.com/mod/badoptimizations |
| **Krypton** | Fabric | 1.16.2 – 26.3 | Rede | Additive marca como `server`. https://modrinth.com/mod/krypton |
| **Debugify** | Fabric, Forge | 1.18.2 – 26.3 | Correções de bugs vanilla | https://modrinth.com/mod/debugify |
| **Clumps** | Fabric, Forge, NeoForge | 1.10.2 – 26.3 | Agrupa orbes de XP | https://modrinth.com/mod/clumps |
| **Noisium** | Fabric, Forge, NeoForge | 1.20 – 1.21.6 | Worldgen | *Unlisted* e parado desde 06/2025. |
| **Saturn** | Forge, NeoForge | 1.16.5 – 1.21.1 | Vazamentos de memória | Parado desde 01/2025. |
| **LazyDFU** | Fabric | 1.14 – 1.20.6 | DFU preguiçoso | O autor diz que desde a 1.19.4 "is no longer necessary in many configurations". https://modrinth.com/mod/lazydfu |
| **Smooth Boot** | vários forks | — | Threads no boot | Original arquivado. https://modrinth.com/mod/smoothboot-fabric |
| **spark** | Fabric, Forge, NeoForge | 1.16.5 – 26.3 | Profiler (`/spark profiler`, `/spark health`, `/spark heapsummary`) | Ferramenta de QA, não de otimização. https://spark.lucko.me/docs/ |
| **OptiFine** | próprio, fechado | — | Renderização e shaders | Incompatível com Sodium/Iris/Embeddium. O autor do ModernFix diz que ele "patches the game in a way that easily breaks Forge". **Não recomendar em packs.** |

#### 2.3.2 1.12.2 (Forge, e Cleanroom como alternativa)

| Mod | Função | Notas |
|---|---|---|
| **VintageFix** | Memória e boot (partes do FerriteCore + modelos dinâmicos do VanillaFix) | Exige **MixinBooter**. Versão "mais enxuta e eficaz" do FoamFix. https://modrinth.com/mod/vintagefix |
| **MixinBooter** | Mixin para 1.8–1.12.2 | https://modrinth.com/mod/mixinbooter |
| **CensoredASM** (ex-LoliASM) | RAM | A partir da v5 é incompatível com VanillaFix e TexFix. Só no CurseForge. https://www.curseforge.com/minecraft/mc-mods/lolasm |
| **Alfheim** | Motor de luz (linhagem Phosphor) | https://modrinth.com/mod/alfheim-lighting-engine |
| **Vintagium / Relictium** | Ports do Sodium | https://modrinth.com/mod/relictium |
| **Celeritas** | Fork do Embeddium + shaders | Sem binário oficial no Modrinth. **Incompatível com Vintagium e forks.** https://github.com/GTNewHorizons/Celeritas |
| **FoamFix** | Memória | Antigo. **[Inferência]** Substituído pelo VintageFix. |
| **Cleanroom** | Loader derivado do FML para 1.12.2 em **Java 21+** | Os *relaunchers* rodam instâncias Forge 1.12.2 nele. https://github.com/CleanroomMC/CleanroomRelauncher |

A lista curada TheUsefulLists mantém tabelas por versão (1.7.10 a 1.21), com incompatibilidades conhecidas, inclusive um documento específico de incompatibilidades do UniversalTweaks: https://github.com/TheUsefulLists/UsefulMods/tree/main/Performance

#### 2.3.3 1.7.10 (ecossistema GTNH)

| Mod | Função | Notas |
|---|---|---|
| **Angelica** | Backport de Sodium + shaders | Exige UniMixins, GTNHLib e lwjgl3ify. **Incompatível** com OptiFine, FastCraft, BetterFPS e ArchaicFix ≤0.6.2. https://modrinth.com/mod/angelica |
| **lwjgl3ify** | Roda o 1.7.10 em Java 17+ com LWJGL3 | https://github.com/GTNewHorizons/lwjgl3ify |
| **UniMixins** | Unifica os loaders de Mixin do 1.7.10 | https://modrinth.com/mod/unimixins |
| **Hodgepodge**, **ArchaicFix** | Correções e performance | https://modrinth.com/mod/hodgepodge, https://modrinth.com/mod/archaicfix |

#### 2.3.4 Regras de conflito curadas (para o diagnóstico determinístico)

Derivadas das tabelas acima (**[Inferência]** quando não declaradas pelos próprios mods):

| Regra | Severidade sugerida |
|---|---|
| Indium + Sodium ≥0.6, ou Indium + Embeddium | erro |
| OptiFine + (Sodium, Iris, Embeddium, Rubidium, ModernFix, Angelica) | erro |
| Mais de um renderizador (Sodium, Embeddium, Rubidium, Vintagium/Relictium, Celeritas, Angelica) | erro |
| Mais de uma implementação de Lithium (Lithium, Canary, Radium) | erro |
| Celeritas + Vintagium/Relictium | erro |
| Angelica + FastCraft/BetterFPS/ArchaicFix ≤0.6.2 | erro |
| CensoredASM v5+ + VanillaFix/TexFix | erro |
| Starlight ou Phosphor em MC ≥1.20 | aviso ("obsoleto") |
| Embeddium/Rubidium/Oculus em NeoForge 1.21+ | aviso ("use Sodium/Iris") |
| LazyDFU em MC ≥1.19.4 | aviso ("desnecessário") |
| Mod com última versão para o MC alvo há mais de N meses ou projeto arquivado | informação |
| Forge <1.13 com Java ≠ 8 sem lwjgl3ify/Cleanroom | erro (seção 4) |

#### 2.3.5 Kits de performance sugeridos por (loader, versão)

**[Inferência]** a partir das tabelas e dos packs FO, Additive e GTNH; sujeito a validação com testes:

- **Fabric 1.21+/26.x (cliente):** Sodium, Lithium, FerriteCore, ImmediatelyFast, EntityCulling, More Culling, Dynamic FPS, ScalableLux, BadOptimizations, ModernFix-mVUS (quando houver). C2ME é opcional, com aviso de worldgen.
- **NeoForge 1.21.1+:** Sodium, Iris, Lithium, FerriteCore, ModernFix, ImmediatelyFast, EntityCulling, ScalableLux.
- **Forge 1.20.1:** Embeddium, Oculus (se houver shaders), ModernFix, FerriteCore, ImmediatelyFast, EntityCulling, Radium/Canary (escolher um), Saturn opcional.
- **Forge 1.18.2–1.19.x:** Embeddium/Rubidium, ModernFix, FerriteCore, Starlight (até 1.20.4), EntityCulling.
- **Forge 1.12.2:** VintageFix + MixinBooter, CensoredASM, Alfheim, um port do Sodium (Vintagium/Relictium **ou** Celeritas). Avaliar Cleanroom para Java 21.
- **Forge 1.7.10:** UniMixins, Hodgepodge, ArchaicFix, Angelica + lwjgl3ify (Java 17+).

### 2.4 Mods "base" (bibliotecas e utilidades) mais comuns

| Mod | Loaders | Versões | Licença | Observação |
|---|---|---|---|---|
| Fabric API | Fabric | 1.14 – 26.3 | Apache-2.0 | Dependência de quase todo mod Fabric. https://modrinth.com/mod/fabric-api |
| Architectury API | Fabric, Forge, NeoForge | 1.16.5 – 26.3 | LGPL-3.0 | Obrigatório nos dois lados. |
| Cloth Config / YACL | Fabric, Forge, NeoForge | 1.14/1.19 – 26.3 | LGPL-3.0 | Telas de config. |
| Mod Menu | Fabric, Quilt | 1.14.4 – 26.3 | MIT | Lista de mods no Fabric. |
| JEI / REI / EMI | Fabric, Forge, NeoForge | JEI 1.8 – 26.3; REI 1.13 – 26.3; EMI 1.18.2 – 1.21.1 | MIT | *Recipe viewers* (escolher um principal). |
| Jade / WTHIT | Fabric, Forge, NeoForge | 1.16 – 26.x | CC-BY-NC-SA-4.0 | Tooltip do bloco olhado (só cliente na prática). |
| Curios / Trinkets / Accessories | — | Curios 1.13.2 – 26.3; Trinkets até 1.21.1 (parado); Accessories 1.20.1 – 1.21.10 | LGPL/MIT | Slots de acessórios. |
| GeckoLib | todos | 1.12.2 – 26.3 | MIT | Animações. |
| Kotlin for Forge / Fabric Language Kotlin | Forge/NeoForge; Fabric | — | LGPL-2.1 / Apache-2.0 | Runtime Kotlin. |
| Balm, Puzzles Lib, Resourceful Lib | multi | — | **Balm é ARR**; MPL; MIT | Bibliotecas de autores populares. |
| FancyMenu + Drippy Loading Screen | Fabric, Forge, NeoForge | 1.12/1.16.2 – 26.3 | Licença própria (DSMSL) | Branding: menu principal e tela de carregamento. |
| Crash Assistant | Fabric, Forge, NeoForge, Quilt, Legacy Fabric | 1.6.4 – 26.3 | Licença própria (MML) | Ver seção 4.4. |
| BetterCompatibilityChecker | Fabric, Forge, NeoForge | 1.16.5 – 26.3 | **ARR** | Valida versão do pack cliente vs. servidor. |
| PackMenu / Custom Main Menu | Forge | 1.14–1.21.1 / ≤1.12.2 | — | Só no CurseForge. |
| KubeJS / CraftTweaker | multi | — | — | Scripts de receitas e eventos (seção 3). |
| FTB Quests / BetterQuesting | multi / 1.7.10–1.12.2 | — | — | Quests (seção 1). |
| Paxi / Open Loader | multi | — | — | Datapacks e resource packs globais. |
| Default Options / YOSBR / Config Manager / Configured Defaults | multi | — | — | Defaults sem sobrescrever (seção 3.2). |

Fonte das faixas e licenças: páginas Modrinth de cada projeto (`https://modrinth.com/mod/<slug>`), consultadas pela API.

### 2.5 Licenças e permissões de redistribuição

#### CurseForge

- **Toggle de distribuição por terceiros:** cada autor escolhe se o projeto pode ser distribuído por serviços de terceiros. Se desligar, clientes de terceiros não acessam os arquivos pela API. https://support.curseforge.com/support/solutions/articles/9000207877-project-distribution-toggle
- **Na API:** o mod tem `allowModDistribution`, e arquivos bloqueados vêm com `downloadUrl = null` (https://docs.curseforge.com/rest-api/). **Para o Warden, que é um cliente de terceiros, esses mods exigirão download manual pelo usuário.** É o mesmo fluxo do packwiz-installer e do Prism (diálogo com links + detecção do arquivo na pasta Downloads: https://github.com/PrismLauncher/PrismLauncher/issues/2372).
- **Modpack CurseForge (`manifest.json`):** referencia mods por `projectID`/`fileID`, sem embutir. https://support.curseforge.com/support/solutions/articles/9000198500-exporting-a-modpack-for-curseforge-project-submission
- **Chave da API:** a API do CurseForge exige chave. *Em aberto:* como o Warden obterá a chave (o packwiz embute uma). Ver Questões em aberto.

#### Modrinth

- **Formato `.mrpack`:** os downloads só podem vir de `cdn.modrinth.com`, `github.com`, `raw.githubusercontent.com` e `gitlab.com`. Cada arquivo tem `sha1`, `sha512`, `env`, `fileSize`. As camadas são `overrides` → `server-overrides`/`client-overrides`. https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack
- **Permissões:** para cada arquivo que você não criou e que vai embutido, ele precisa satisfazer uma destas condições: estar hospedado no Modrinth; ter licença aberta que permita redistribuir; ter permissão na descrição do projeto; ou ter permissão direta do autor, com evidência enviada na aba Moderation. Sem isso, há risco de remoção. https://support.modrinth.com/en/articles/8797527-obtaining-modpack-permissions
- **Regras de conteúdo:** proibido reupload sem direitos; crédito obrigatório. https://modrinth.com/legal/rules
- **Interação com o packwiz:** com `restrictDomains`, todo mod do CurseForge vai **embutido** no `.mrpack`. Portanto, **um pack com mods do CurseForge exportado para Modrinth só é publicável se cada um desses jars tiver licença ou permissão de redistribuição.**

#### Triagem por licença

**[Inferência]** a partir dos textos das licenças:

| Licença (SPDX / tipo) | Embutir em overrides? |
|---|---|
| MIT, Apache-2.0, BSD, MPL-2.0, LGPL, GPL | Sim, preservando o aviso de licença (o COBBLEVERSE inclui `overrides/licenses/` e um PDF de licenças de terceiros) |
| CC-BY(-SA) | Sim, com atribuição. Em CC-BY-NC(-SA), só sem fins comerciais |
| `LicenseRef-All-Rights-Reserved` (ARR) | **Não.** Referenciar pela plataforma ou obter permissão |
| `LicenseRef-*` customizadas (Polyform Shield, tr7zw Protective, DSMSL, MML) | Revisão manual |
| Desconhecida (jar local sem metadado) | Bloquear o export para plataformas públicas até o usuário confirmar a permissão |



## 3. Configs: o que se ajusta e como se distribui

### 3.1 Onde ficam as configurações (por loader e época)

| Local | Loader/época | O que é | Observações para o pack |
|---|---|---|---|
| `config/` | todos | Configs de mods (`.toml` no Forge/NeoForge 1.13+, `.cfg` no Forge 1.7.10/1.12.2, `.json`/`.json5`/`.toml`/`.properties` no Fabric, conforme o mod) | É o grosso do que o pack distribui. |
| `config/*-client.toml`, `*-common.toml`, `*-startup.toml` | Forge/NeoForge 1.13+ | Configs tipados do sistema de config da FML | `CLIENT` só existe no cliente; `COMMON`/`STARTUP` em ambos os lados. |
| `saves/<mundo>/serverconfig/*-server.toml` e `<servidor>/world/serverconfig/` | Forge/NeoForge 1.13+ | Configs `SERVER`, **por mundo**, sincronizados do servidor para o cliente | Editar `config/` não basta: o valor efetivo vive dentro do mundo. |
| `defaultconfigs/` | Forge/NeoForge 1.13+ | Modelos copiados quando o arquivo de config ainda não existe | Caminho configurável em `config/fml.toml` (`defaultConfigPath`). |
| `options.txt` | vanilla | Opções de vídeo, som, idioma, keybinds, resource packs ativos | Sobrescrever num update apaga as preferências do jogador. |
| `servers.dat` | vanilla | Lista de servidores (NBT **não comprimido**: `name`, `ip`, `icon` em Base64, `acceptTextures`, `hidden`) | Packs com servidor oficial costumam pré-preencher. |
| `kubejs/` (`startup_scripts/`, `server_scripts/`, `client_scripts/`, `assets/`, `data/`, `config/`) | KubeJS (Forge/NeoForge/Fabric) | Receitas, tags, loot, eventos, textos | Código do pack; versionar sempre. |
| `scripts/*.zs` | CraftTweaker (inclui 1.12.2) | Scripts ZenScript | Idem. |
| `config/ftbquests/quests/` | FTB Quests | Árvore de quests em SNBT | Idem. |
| `config/openloader/`, `config/paxi/` | Open Loader / Paxi | Datapacks e resource packs **globais** | Datapacks vanilla são por mundo; estes mods os aplicam a todos os mundos. |
| `resourcepacks/`, `shaderpacks/` | todos | Arquivos `.zip` ou pastas | Ativação padrão depende de `options.txt` ou de um mod (ver 3.3). |

Detalhes confirmados na fonte:

- **Tipos de config da NeoForge.** `STARTUP`, `CLIENT` e `COMMON` ficam em `.minecraft/config`. `SERVER` fica em `.minecraft/saves/<world_name>/serverconfig` (cliente) ou `<server_folder>/world/serverconfig` (servidor) e é "synced across the network to the client". Fonte: https://docs.neoforged.net/docs/misc/config/
- **`defaultconfigs/` vale para qualquer config que ainda não exista, não só para `serverconfig`.** No `ConfigTracker` da FancyModLoader (NeoForge), `setupConfigFile` procura `defaultConfigPath.resolve(fileName)` e, se o arquivo existir, faz `Files.copy` para o destino. Caso contrário, grava os valores padrão do spec. Isso ocorre para qualquer `ModConfig` cujo arquivo esteja ausente. Fonte (código): https://github.com/neoforged/FancyModLoader/blob/main/loader/src/main/java/net/neoforged/fml/config/ConfigTracker.java. O caminho vem de `FMLConfig.defaultConfigPath()` (chave `defaultConfigPath` em `config/fml.toml`). O uso clássico, segundo a comunidade, é "copied to the serverconfigs folder of any newly created world": https://forums.minecraftforge.net/topic/89268-defaultconfigs-folder-questions/. *Inferência:* o Forge (não Neo) 1.13–1.20.1 tem a mesma lógica herdada. Falta confirmar no código do Forge de cada versão.
- **Fabric não tem equivalente nativo de `defaultconfigs/`.** Cada mod grava sua config em `config/`, e o pack usa YOSBR (ver 3.2). *Inferência* baseada na ausência desse recurso na documentação do Fabric Loader.
- **Formato de `options.txt`.** A chave `version` guarda o *data version* do cliente que gravou o arquivo e é usada para migrar configurações antigas. Sem ela, o arquivo volta ao padrão. `resourcePacks`/`incompatibleResourcePacks` são listas no formato `["vanilla","file/MeuPack.zip"]`. Keybinds seguem `key_key.forward:key.keyboard.w`. Antes da 1.13 eram códigos numéricos. Fonte: https://minecraft.wiki/w/Options.txt
- **`servers.dat`** é NBT não comprimido. Fonte: https://minecraft.wiki/w/Servers.dat_format

### 3.2 O problema central: "padrões" vs. "preferências do jogador"

Num update, o jogador já tem `options.txt` e configs próprios. Se o pack sobrescrever esses arquivos, apaga as personalizações (keybinds, FOV, volume, resource packs). Se nunca sobrescrever, correções de balanceamento não chegam. As soluções que a comunidade usa:

1. **YOSBR ("Your Options Shall Be Respected").** Os arquivos ficam em `config/yosbr/`, espelhando a árvore real (ex.: `config/yosbr/options.txt`, `config/yosbr/config/roughlyenoughitems/config.json5`). O mod só copia um arquivo se o destino não existir. Roda só no cliente, em Fabric/Quilt, da 1.8.9 até as versões 26.x. Fabulously Optimized é o exemplo canônico (ver seção 1). Fonte: https://modrinth.com/mod/yosbr
2. **Default Options (BlayTheNinth).** Funciona em Forge, Fabric e NeoForge e depende de **Balm**. O autor do pack roda `/defaultoptions saveAll` no jogo, o que salva opções, keybinds e lista de servidores em `config/defaultoptions/`. Só instalações novas (sem `options.txt`) recebem os padrões. Keybinds do pack viram os novos *defaults* do jogo, de modo que as mudanças do jogador sobrevivem a updates. A lista de servidores é pré-preenchida sem apagar os servidores do jogador. Fonte: https://www.curseforge.com/minecraft/mc-mods/default-options
3. **`defaultconfigs/` da FML (Forge/NeoForge 1.13+).** Mecanismo nativo, com a mesma regra de "copiar só se não existir" (ver 3.1).
4. **`preserve = true` no `index.toml` do packwiz.** O packwiz-installer não sobrescreve o arquivo se ele já existir ("to preserve changes made by a user"). Fonte: https://packwiz.infra.link/reference/pack-format/index-toml/. *Em aberto:* como o CLI do packwiz define e mantém esse campo após `packwiz refresh` (ver Questões em aberto).
5. **Config Manager** (TheBossMagnus; Fabric/NeoForge/Quilt, 1.20+, LGPL-3.0). Os arquivos vão em `config/modpack_defaults/` (ex.: `config/modpack_defaults/options.txt` → `.minecraft/options.txt`) e são copiados **só se não existirem**. A GUI tem **Update**, que sobrescreve só os arquivos que o pack distribui, e **Reset**, que apaga `config/` e restaura os do pack. Usado pelo Fabulously Optimized e pelo Additive (seção 1). https://modrinth.com/mod/configmanager
6. **Configured Defaults** (Fabric/Forge/NeoForge, 1.18+, MPL-2.0): "providing defaults for files absent in .minecraft like configs". Os arquivos vão em `configureddefaults/`. Usado pela FTB e pelo BMC4. https://modrinth.com/mod/configured-defaults
7. **Alternativas menores:** EasyConfigs (salvar/carregar perfis de config, importar/exportar zip): https://github.com/KdGaming0/iRespectYourOptions

Regra prática que sai disso:

- **Configs de balanceamento/gameplay** (receitas, worldgen, dificuldade, quests) **devem** ser sobrescritos a cada versão. Ficam em `config/`, `kubejs/`, `defaultconfigs/` (serverconfig) e são de autoridade do pack.
- **Configs de preferência** (`options.txt`, keybinds, HUD, minimapa, volume, zoom) **não devem** ser sobrescritos. Vão por YOSBR, Default Options, `defaultconfigs/` (client) ou `preserve`.

### 3.3 O que normalmente se ajusta

Lista consolidada a partir dos estudos de caso (seção 1) e da documentação dos mods citados:

- **`options.txt` padrão:** distância de renderização e simulação moderadas, `guiScale`, idioma (`lang`), som, desligar o *narrator*/tutorial (`tutorialStep:none`), `resourcePacks` ativos (ex.: o resource pack do pack ou de compatibilidade), `skipMultiplayerWarning`, `joinedFirstServer`, `onboardAccessibility:false` (1.19.4+, evita a tela de acessibilidade na primeira execução). *As chaves citadas existem no `options.txt` vanilla (https://minecraft.wiki/w/Options.txt); a escolha de quais ajustar é prática comum, não regra oficial.*
- **Keybinds:** resolver conflitos entre mods (vários mods usam as mesmas teclas por padrão). Distribuídos via `options.txt` padrão ou Default Options.
- **Resource packs ativos por padrão:** `options.txt` → `resourcePacks`, ou Resource Pack Overrides (Fuzss: define conjunto e ordem padrão, trava packs obrigatórios, esconde packs técnicos; config em `config/resourcepackoverrides.json`, só cliente, Fabric/NeoForge): https://modrinth.com/mod/resource-pack-overrides. Paxi e Open Loader carregam resource packs e datapacks globais: https://github.com/YUNG-GANG/Paxi, https://docs.darkhax.net/1.20.1/open-loader/
- **Datapacks globais:** como datapacks são por mundo, packs usam Paxi (`config/paxi/datapacks`), Open Loader (`config/openloader/data`) ou KubeJS (`kubejs/data`) para que receitas/tags valham em todo mundo novo.
- **Configs de mods:** itens escondidos no JEI/EMI/REI, desativar features duplicadas entre mods (ex.: dois mods adicionando o mesmo minério, unificação de materiais), worldgen, spawn rates, ajustes de performance (distâncias de entidades, *culling*), integração visual (menus, tela de carregamento, branding do pack com FancyMenu, Drippy Loading Screen e afins).
- **Servidor:** `server.properties` de referência (ex.: `allow-flight=true`, porque mods de voo e jetpacks são chutados pelo anticheat vanilla; *prática comum, verificar caso a caso*), configs `-server.toml` em `defaultconfigs/`, listas de *chunk loading*.
- **`servers.dat`:** só em packs ligados a um servidor oficial.


### 3.4 Ferramentas/mods "de autor de pack" recorrentes

Levantamento da lista curada TheUsefulLists, "Modpack Tool Mods" (1.19.x), cruzado com os estudos de caso: https://github.com/TheUsefulLists/UsefulMods/tree/main/ModpackTools

- **Defaults sem sobrescrever:** Default Options, YOSBR, OneTimeOverrides ("non-overwriting override files").
- **Conteúdo e regras do pack:** KubeJS, CraftTweaker, Game Stages, BiomeTweaker, Bad Mobs, Emendatus Enigmatica (unificação de materiais).
- **Datapacks/resource packs globais:** Open Loader, Paxi.
- **Identidade do pack:** Tips / Loading Screen Tips, Splashy (*splash texts*), FancyMenu e afins.
- **Diagnóstico:** MixinTrace (lista de mixins no stack trace do crash), FTB Pack Companion (correções e ferramentas para autores de pack), World Stripper (inspecionar a geração de minérios).
- **Servidor:** My Server Is Compatible (remove o falso aviso "Incompatible FML modded server").


## 4. Testes e QA de modpack, memória/JVM e versão de Java

### 4.1 Como os autores testam (o que se observa nos packs)

- **Instância de desenvolvimento ≠ instância de teste limpa.**
  - O FO edita no launcher do CurseForge e depois **testa a exportação no MultiMC** (seção 1.A).
  - O Enigmatica e o Craftoria mantêm a instância de dev sincronizada com o git (InstanceSync), mas o artefato publicado é gerado de forma limpa pelo uploader.
  - **[Inferência]** A lição é testar o **artefato exportado** numa instância nova, não só a instância de trabalho, que acumula arquivos que o pack não tem.
- **Servidor dedicado de verdade:**
  - O Monifactory sobe um servidor de *gametest* em CI para cada modo de dificuldade a cada PR que mexe em config/kubejs/manifest.
  - O Monifactory e o GTNH montam o servidor do zero no build.
  - É a única forma de pegar o erro E5 (mod de cliente no servidor).
- **Builds frequentes e canais:** GTNH (dailies com retenção de 14 dias + experimental + RC), FO (alpha/beta), ATM (`-rc`), canais alpha/beta/release no CurseForge (Enigmatica, FTB, Monifactory).
- **Lint de scripts:** ESLint/Prettier para KubeJS (Enigmatica, Monifactory, Star Technology, Craftoria `kubejs-lint.yml`).
- **Versão do pack obrigatória nos reports:** templates de issue (ATM, FTB) e BetterCompatibilityChecker (impede cliente e servidor em versões diferentes do pack).
- **Validação do questbook em PR** (GTNH `build-and-quest.yml`).

### 4.2 Versão de Java por versão do Minecraft

Fonte primária: o campo `javaVersion` de cada versão no manifest da Mojang (https://piston-meta.mojang.com/mc/game/version_manifest_v2.json; exemplo: https://piston-meta.mojang.com/v1/packages/4fe1aa1ef8da1cb95c5bad1fb98890ca56dd8ca3/26.3.json).

| Versão MC | `javaVersion.majorVersion` | `component` |
|---|---|---|
| ≤1.16.5 (inclui 1.7.10, 1.12.2) | 8 | `jre-legacy` |
| 1.17 – 1.17.1 | 16 | `java-runtime-alpha` |
| 1.18 – 1.20.4 | 17 | `java-runtime-beta` / `java-runtime-gamma` |
| 1.20.5 – 1.21.11 | 21 | `java-runtime-delta` |
| **26.1 – 26.3** | **25** | `java-runtime-epsilon` |

**O launcher do Warden deve ler `javaVersion` do manifest em vez de manter uma tabela fixa.** A doc do Prism ainda não cita o Java 25 (https://prismlauncher.org/wiki/getting-started/installing-java/), sinal de que tabelas fixas envelhecem.

Exceções relevantes ao loader:

- **Forge <1.13 (1.7.10, 1.12.2) só roda em Java 8.** No Java 9+, o LaunchWrapper falha com `ClassCastException: ... AppClassLoader cannot be cast to ... URLClassLoader`. https://github.com/MinecraftForge/MinecraftForge/issues/7596
- **1.7.10 com lwjgl3ify (GTNH):** Java 17–25, com o 25 recomendado para GTNH 2.8+. https://wiki.gtnewhorizons.com/wiki/Installing_and_Migrating
- **1.12.2 com Cleanroom:** Java 21+. https://github.com/CleanroomMC/CleanroomRelauncher
- **FTB** fixa o runtime no manifest (ex.: `java 21.0.4`), em vez de deixar o usuário escolher (seção 1.C).

### 4.3 Memória e argumentos de JVM

- **Excesso de RAM piora.** O guia do Enigmatica diz: "Assigning more than shown here is never recommend, as you may actually get worse performance" e "Never allocate more memory than your PC can spare". https://github.com/EnigmaticaModpacks/Docs-Main/blob/master/help-desk/guides/allocating-memory.md
- **GTNH** recomenda 4–6 GB em single player e avisa que mais de 8 GB em Java 8 causa *stuttering*. Trocar para um Java moderno rende mais do que ajustar o GC. https://wiki.gtnewhorizons.com/wiki/Installing_and_Migrating
- **FTB** publica RAM mínima e recomendada no próprio manifest (`specs`). O server pack do ATM10 traz `user_jvm_args.txt` (valores citados por fontes secundárias: `-Xms4G -Xmx8G`, https://winternode.com/blog/minecraft/atm10-setup, não verificados na fonte oficial).
- **Flags de Aikar (servidor):** `-Xms` igual a `-Xmx`, G1GC com `MaxGCPauseMillis=200`, `G1NewSizePercent=30`, `G1HeapRegionSize=8M`, `InitiatingHeapOccupancyPercent=15`, `AlwaysPreTouch`… https://docs.papermc.io/paper/aikars-flags
- **ZGC:** o Generational ZGC precisa de `-XX:+UseZGC -XX:+ZGenerational` no Java 21. No Java 23 o modo geracional virou padrão, e no Java 24 o não geracional foi removido (https://openjdk.org/jeps/474). **[Inferência]** G1 continua o padrão seguro. ZGC é opcional para heaps grandes, com mais uso de memória.
- **`-XX:+UseCompactObjectHeaders`** virou recurso de produto no JDK 25 (https://openjdk.org/jeps/519). Relevante para MC 26.x e para GTNH com Java 25.
- **Presets sugeridos [Inferência]:**

| Pack | `-Xmx` sugerido |
|---|---|
| Leve (<100 mods) | 3–4 GB |
| Médio (100–200 mods) | 6 GB |
| Pesado (200+ mods) | 8–10 GB |

  Limitar a cerca de 50–60% da RAM física. Avisar acima de 8 GB em Java 8. No cliente, nunca `-Xms` muito alto.

### 4.4 Ferramentas de diagnóstico e o que dá para reaproveitar

- **Crash Assistant** (mod; Fabric/Forge/NeoForge/Quilt/Legacy Fabric, 1.6.4–26.3), https://modrinth.com/mod/crash-assistant:
  - Após o crash, abre uma GUI com logs, `crash-reports` e `hs_err`.
  - Envia ao mclo.gs com um clique, com redação de dados sensíveis.
  - Mostra o **diff da lista de mods do usuário contra a lista oficial do pack**.
  - Detecta problemas conhecidos (`atio6axx.dll` do driver AMD, `OutOfMemoryError`, dependências).
  - Tem uma lista de "mods problemáticos" configurável pelo autor do pack.
  - É usado pelo ATM10.
- **API do mclo.gs** (https://api.mclo.gs/):
  - Endpoints: `POST /1/log`, `GET /1/raw/{id}`, `GET /1/insights/{id}`, **`POST /1/analyse` (analisa sem salvar)**, `GET /1/limits`, `GET /1/filters`.
  - Limites: 10 MiB, 25.000 linhas, 60 req/min por IP.
- **codex-minecraft (Aternos)**, o motor de *insights* do mclo.gs, PHP, https://github.com/aternosorg/codex-minecraft:
  - Um *Detective* identifica o tipo de log (Vanilla, Forge, NeoForge, Fabric, Prism…).
  - *Analysers* aplicam regex e emitem *Problem* + *Solution* + *Information* (versões de Java, MC e loader).
  - Exemplos: `FabricModDependencyProblem` (`- Mod X requires version Y or later of Z`), `FabricDuplicateModProblem`, `FabricMixinProblem`, `Forge/ModDependencyProblem`, `ModDuplicateProblem`, `ModWrongMinecraftVersionProblem`, `WorldMissingModProblem`, `TickingEntityProblem`.
  - Lista real de classes de problema em `src/Analysis/Problem/`:
    - **Fabric:** `FabricDuplicateModProblem`, `FabricEntryStageProblem`, `FabricIncompatibleModsProblem`, `FabricMixinProblem`, `FabricModDependencyProblem`, `FabricModIncompatibleMinecraftVersionProblem`.
    - **Forge:** `FmlConfirmProblem`, `LanguageProviderVersionProblem`, `MissingDatapackModProblem`, `ModDependencyProblem`, `ModDuplicateProblem`, `ModExceptionProblem`, `ModFatalProblem`, `ModWrongMinecraftVersionProblem`, `MultipleModulesExportProblem`, `UndefinedModDependencyProblem`, `WorldMissingModProblem`, `WorldModVersionProblem`.
    - **Vanilla:** `DatapackParsingProblem`, `MalformedEncodingProblem`, `TickingBlockEntityProblem`…
    - **CrashReport:** `TickingEntityProblem`, `TickingBlockEntityProblem`.
  - **Licença MIT** (repositório ativo; último push em 08/2026), o que permite portar as regras (regex) para Rust com atribuição.
  - **É a melhor referência de catálogo de padrões de log para a camada determinística do Warden.**
- **Not Enough Crashes:** volta ao menu em vez de fechar o jogo e lista os mods suspeitos (1.16.5–1.21.11). https://modrinth.com/mod/notenoughcrashes
- **Neruina:** impede que crashes de *ticking* inutilizem o mundo. https://modrinth.com/mod/neruina
- **MixinTrace:** adiciona os mixins envolvidos ao stack trace (seção 3.4).
- **spark:** profiler de CPU/memória/TPS. É o padrão para investigar lag. https://spark.lucko.me/docs/

### 4.5 Checklist de QA consolidado

**[Inferência]** a partir das práticas acima:

1. Validação estática (metadados, regras curadas, licenças, lado) sem erros.
2. Boot do cliente numa **instância limpa gerada a partir do pack exportado**.
3. Criar **mundo novo**, entrar, andar alguns chunks, abrir o *recipe viewer*, abrir o livro de quests.
4. Boot de **servidor dedicado** com o server pack gerado. Conectar o cliente, se possível.
5. Se for update: carregar um **mundo da versão anterior** (backup) e conferir "missing registry entries".
6. Revisar o log em busca de `ERROR`/`WARN` novos em relação à versão anterior (diff de log).
7. Perfil rápido com spark, se houver suspeita de lag.
8. Changelog gerado e revisado; versão incrementada conforme o impacto.

## 5. Erros comuns de iniciantes e como uma ferramenta poderia preveni-los

### 5.1 Fontes de metadados disponíveis para checagem automática

Antes da lista de erros, vale saber o que dá para checar sem IA. Os metadados declarados são a base do diagnóstico determinístico:

| Fonte | Campos úteis | Referência |
|---|---|---|
| API do Modrinth (versão) | `dependencies[].dependency_type` ∈ `required`, `optional`, `incompatible`, `embedded`; `game_versions`; `loaders`; `files[].hashes.sha1/sha512` | https://docs.modrinth.com/api/operations/getversion/ |
| API do Modrinth (projeto) | `environment` ∈ `client_and_server`, `client_only`, `client_only_server_optional`, `singleplayer_only`, `server_only`, `server_only_client_optional`, `dedicated_server_only`, `client_or_server`, `client_or_server_prefers_both`, `unknown` (substitui `client_side`/`server_side`, agora *deprecated*); `license.id` (SPDX) | https://docs.modrinth.com/api/operations/getproject/ |
| `fabric.mod.json` (dentro do jar) | `depends` (falha dura), `recommends` (aviso), `suggests` (só metadado), `conflicts` (aviso se casar), `breaks` (falha dura se casar); `environment` ∈ `*`, `client`, `server` | https://wiki.fabricmc.net/documentation:fabric_mod_json_spec |
| `META-INF/neoforge.mods.toml` (NeoForge; **não** `mods.toml`) | `[[dependencies.<modid>]]` com `type` ∈ `required`, `optional`, `incompatible`, `discouraged`, `versionRange` (Maven), `ordering`, `side` ∈ `CLIENT`/`SERVER`/`BOTH`, `reason` | https://docs.neoforged.net/docs/gettingstarted/modfiles/ |
| `META-INF/mods.toml` (Forge 1.13+; também NeoForge 1.20.1–1.20.4) | `[[dependencies.<modid>]]` com `modId`, `mandatory` ("whether the game should crash when this dependency is not met"), `versionRange`, `ordering`, `side`. Não tem `incompatible` explícito como a NeoForge. | https://docs.minecraftforge.net/en/latest/gettingstarted/modfiles/ |
| `mcmod.info` (Forge 1.7.10/1.12.2) | Nome, versão, `requiredMods`/`dependencies` (frequentemente vazios). A dependência real costuma estar só na anotação `@Mod(dependencies="required-after:...")`, no bytecode. | *Inferência baseada em prática conhecida; a confirmar.* |

Consequência: **para 1.7.10/1.12.2 a camada determinística é bem mais fraca.** Os metadados são pobres, e o Modrinth tem pouca cobertura dessas versões em comparação com o CurseForge. O peso fica nas regras de log (seção 4) e em listas curadas.

### 5.2 Catálogo de erros comuns e prevenção possível

| # | Erro comum | Sintoma | Como a ferramenta pode prevenir |
|---|---|---|---|
| E1 | **Mod do loader errado** (jar Fabric num pack Forge/NeoForge e vice-versa) ou da versão errada do MC | Mod ignorado ou crash na inicialização | Bloquear na adição: filtrar `loaders`/`game_versions` na API e ler os metadados do jar local (`fabric.mod.json` vs `mods.toml`/`neoforge.mods.toml`/`mcmod.info`). Jars multi-loader: a NeoForge rejeita um jar que tem metadados Fabric + Forge mas não NeoForge (https://github.com/Sinytra/Connector/issues/1934). |
| E2 | **Dependência obrigatória faltando** (Fabric API, Architectury, Cloth Config, GeckoLib, Balm…) | Tela de erro do loader | Resolver `required` automaticamente ao adicionar (o packwiz já pergunta) e revalidar o pack inteiro antes de testar/exportar. |
| E3 | **Mods declaradamente incompatíveis** (`incompatible`, `breaks`, `conflicts`, `discouraged`) | Crash ou aviso | Regra determinística direta, citando o campo `reason` quando houver. |
| E4 | **Duplicatas**: o mesmo mod duas vezes (versões diferentes, ou Modrinth + CurseForge + jar local), ou dois mods que fazem a mesma coisa (dois renderizadores, Sodium + Embeddium, OptiFine + Sodium, dois minimapas, JEI + REI + EMI sem plugin de ponte) | Crash por *duplicate mod id*, *mixin conflict* ou comportamento estranho | Detectar por mod id (dos metadados do jar) e por hash. Manter uma lista curada de "categorias exclusivas" (renderizador, *light engine*, *recipe viewer*), ver seção 2. |
| E5 | **Mod só de cliente no servidor** | Servidor dedicado crasha com `Attempted to load class net/minecraft/client/... for invalid dist DEDICATED_SERVER`; cada linha dessas aponta um mod diferente | Marcar `side = "client"` no `.pw.toml` com base no `environment` do Modrinth e no `environment`/`side` do jar. Testar também em servidor dedicado. Fontes: https://forums.minecraftforge.net/topic/122792-attempted-to-load-class-netminecraftclientkeymapping-for-invalid-dist-dedicated_server/, https://space-node.net/blog/modded-server-wont-start-client-side-mods-fix-2026 |
| E6 | **Distribuir `options.txt`/configs de preferência por cima do jogador** | Jogadores perdem keybinds e ajustes a cada update | Separar no editor "padrões só na primeira execução" (YOSBR/Default Options/`defaultconfigs`/`preserve`) de "configs autoritativos" (seção 3.2). |
| E7 | **Incluir lixo no export** (`saves/`, `logs/`, `crash-reports/`, `screenshots/`, `.cache`, `journeymap/data`, `xaero` waypoints, `usercache.json`, `*.log`, `local/`, `.mixin.out`, `.fabric/`, `config/*.bak`, *caches* de shader, `downloads/`) | Pack inchado, vazamento de dados pessoais (waypoints, nome do usuário, IPs) | *Allowlist* em vez de *denylist*: o pack só leva o que o usuário marcou como conteúdo (`mods/`, `resourcepacks/`, `shaderpacks/`, `config/`, `defaultconfigs/`, `kubejs/`, `scripts/`, etc.), e o restante passa por revisão. Gerar `.packwizignore` padrão. |
| E8 | **Remover ou trocar um mod que tem conteúdo no mundo** (blocos, itens, dimensões) num update | "missing registry entries": blocos viram ar, itens somem, sem volta sem backup | Ao remover um mod com blocos/itens entre versões publicadas, mostrar aviso de quebra de mundo, sugerir *major bump* e nota no changelog ("faça backup"). Fontes: https://nodecraft.com/support/games/minecraft/mods/how-to-remove-a-mod-from-a-modpack, https://mc-node.net/blog/en/update-modpack-without-losing-world/ |
| E9 | **Trocar mods de worldgen no meio do pack** | Fronteiras de chunk feias, biomas quebrados | Aviso específico para mods de worldgen/dimensão em updates (lista curada + tags de categoria do Modrinth). |
| E10 | **Versão de Java errada** para a versão do MC/loader | Crash imediato (`UnsupportedClassVersionError`, erros de módulo) | Launcher próprio escolhe o Java pelo `javaVersion` do manifest da Mojang e pelas exceções do loader (seção 4). |
| E11 | **RAM demais ou de menos; JVM args copiados da internet** | GC pauses longas, OOM, flags inválidas no Java novo | Presets por tamanho de pack e versão do Java; validar flags (seção 4). |
| E12 | **Atualizar mods "às cegas" (update all) logo antes do release** | Regressões não testadas | Fluxo "update → testar → publicar": diff de versões no changelog e teste obrigatório antes da exportação "release". |
| E13 | **Ignorar licenças / redistribuir jars** (`overrides/mods/*.jar` de mod ARR, ou mods do CurseForge com distribuição de terceiros desligada) | Pack rejeitado no Modrinth/CurseForge, DMCA, instalação falhando | Preferir sempre referência (metafile) a embutir jar. Avisar quando um jar local não tiver licença permissiva conhecida (seção 2). |
| E14 | **Dois mods registrando a mesma tecla** | Ações que "não funcionam" | Ler keybinds do `options.txt` da instância de teste e listar conflitos (mesma tecla, mesmo contexto). |
| E15 | **Datapack colocado em `datapacks/` de um mundo de teste** | Some no export, não vale para outros mundos | Detectar `saves/*/datapacks/` novos durante o teste e oferecer mover para Paxi/Open Loader/KubeJS (seção 3.3). |
| E16 | **Config editado em `config/` quando o valor efetivo está em `saves/<mundo>/serverconfig/`** (Forge/NeoForge `-server.toml`) | "Mudei o config e nada aconteceu" | Explicar na aba de configs; promover `serverconfig` alterado no teste para `defaultconfigs/`. |
| E17 | **Mods sem versão fixa** (URL "latest", jar baixado manualmente sem hash) | Builds não reprodutíveis | O packwiz já fixa hash. Exigir hash também para arquivos por URL. |
| E18 | **Testar só no cliente com mundo antigo** | Bugs que só aparecem em mundo novo/servidor | Checklist de QA: instância limpa + mundo novo + servidor dedicado (seção 4). |

*Observação:* E1–E5, E8, E13 e E17 têm fonte primária ou são consequência direta dos formatos documentados. E7, E9, E12, E14–E16 são prática observada na comunidade e inferência minha sobre como prevenir.


## 6. Implicações para o Warden

Lista priorizada. **P0** = necessário para a primeira versão funcional. **P1** = logo em seguida. **P2** = desejável. Cada item cita a seção que o justifica.

### 6.1 Estrutura do projeto e versionamento (P0)

1. **Template de projeto gerado no `init`** (1.A, 1.D, 1.E):
   - `pack.toml`, `index.toml`.
   - `.gitattributes` com `* -text`. **Crítico no Windows**: sem ele, o git altera finais de linha e os hashes do packwiz quebram.
   - `.gitignore` e `.packwizignore` **em modo *allowlist*** (`/*` + `!/mods`, `!/config`, `!/defaultconfigs`, `!/kubejs`, `!/scripts`, `!/resourcepacks`, `!/shaderpacks`, `!/datapacks`…), mais a exclusão de arquivos sabidamente pessoais ou gerados (`/config/xaero`, `sodium-fingerprint.json`, `iris.properties`, `*-client.toml` só se o usuário quiser, `kubejs/probe`, `jsconfig.json`, `*.disabled`).
   - O usuário pode reabrir pastas, mas o padrão é "só entra o que foi marcado".
2. **Separar "instância de teste" de "projeto do pack"** (1.D, 4.1). A instância do launcher próprio é **derivada** do projeto, e o caminho inverso (instância → projeto) passa sempre por uma tela de revisão de diferenças. Isso evita o vazamento de caches observado no BMC4 e no Prominence II (`modernfix/structureCacheV1`, milhares de arquivos de shader).
3. **Versionamento SemVer do pack** com sugestão automática do incremento (1.D item 7, 1.C Monifactory):
   - **major**: remoção de mod com conteúdo no mundo, troca de mod de worldgen, mudança de versão do MC/loader;
   - **minor**: mods adicionados, mudanças de balanceamento;
   - **patch**: updates de mods e correções de config.
   - Canais `alpha`/`beta`/`release`.
   - Metadado opcional `+mc<versão>.<loader>` (padrão do Additive).
4. **Suporte a múltiplas variantes** (P1): uma pasta por versão do MC/loader (`versions/<loader>/<mc>/`, como no Additive e no FO), com operações em lote (refresh, update, export).

### 6.2 Adição de mods e metadados (P0)

5. **Filtro rígido de loader e versão do MC** na busca (Modrinth `loaders`/`game_versions`; CurseForge equivalente). Para jars locais, ler `fabric.mod.json`, `neoforge.mods.toml`, `mods.toml` e `mcmod.info` e recusar o loader errado (E1).
6. **Resolução automática de dependências `required`** com confirmação. Revalidação global antes de testar e de exportar (E2).
7. **Preenchimento automático de `side`** a partir do `environment` do Modrinth e do `environment`/`side` do jar, com possibilidade de override manual. Para CurseForge e jars sem metadado, usar `both` mais um aviso "lado desconhecido" (1.D item 4, E5).
8. **Mods opcionais** (`[option] optional`) com aviso claro: o app do Modrinth instala todos os opcionais, e o formato CurseForge não suporta `side` (1.E).
9. **Hash obrigatório** para qualquer arquivo por URL; nada de "latest" (E17).

### 6.3 Diagnóstico determinístico, antes da IA (P0)

10. **Motor de regras sobre o pack inteiro**, rodado ao clicar em "Testar" e antes de exportar. Três severidades: erro (bloqueia o teste), aviso e informação. Regras mínimas:
    - **R-LOADER / R-MCVER:** mod incompatível com o loader ou a versão do pack (E1).
    - **R-DEP-MISSING:** dependência `required`/`depends` ausente ou fora do `versionRange` (E2).
    - **R-DECLARED-INCOMPAT:** `incompatible` (Modrinth e NeoForge), `breaks` (falha) e `conflicts`/`discouraged` (aviso), exibindo o `reason` (E3).
    - **R-DUPLICATE:** mesmo mod id ou mesmo hash duas vezes, ou o mesmo projeto de fontes diferentes (E4).
    - **R-EXCLUSIVE:** mais de um mod numa "categoria exclusiva" (renderizador, implementação de Lithium, motor de luz, port do Sodium para 1.12.2), conforme a tabela 2.3.4.
    - **R-CURATED-CONFLICT:** tabela 2.3.4 completa, mantida como **dados versionados** (JSON/TOML), atualizável sem nova versão do app.
    - **R-OBSOLETE:** Indium com Sodium ≥0.6, Starlight/Phosphor em 1.20+, LazyDFU em 1.19.4+, Embeddium/Rubidium/Oculus em NeoForge 1.21+, projetos arquivados ou sem update.
    - **R-JAVA:** Java incompatível com MC/loader (4.2), incluindo Forge <1.13 → Java 8, salvo lwjgl3ify/Cleanroom.
    - **R-SIDE:** mod `client_only` marcado como `both` (aviso, porque quebra o servidor dedicado).
    - **R-LICENSE:** jar embutido sem licença que permita redistribuição (2.5).
11. **Analisador de log determinístico pós-crash** inspirado no **codex-minecraft** (MIT; portar as regex com atribuição, 4.4). Precisa detectar:
    - dependências faltando;
    - mods duplicados;
    - erros de mixin;
    - versão errada do MC;
    - `invalid dist DEDICATED_SERVER`, que aponta o mod culpado;
    - `OutOfMemoryError`;
    - `UnsupportedClassVersionError` e o `ClassCastException` do LaunchWrapper (Java errado);
    - *missing registry entries*;
    - *ticking entity/block entity*.

    O resultado vira contexto estruturado para a IA, que só entra quando as regras não explicam o crash.
12. **Redação de dados sensíveis** antes de enviar logs à IA: nome de usuário, caminhos com o nome do usuário do Windows, IPs, tokens. O Crash Assistant faz o mesmo antes de subir ao mclo.gs (4.4).

### 6.4 Configs (P0/P1)

13. **Dois "tipos" de arquivo de config no editor** (3.2):
    - **Autoritativo**, que o pack sempre sobrescreve: balanceamento, receitas, quests, worldgen, `kubejs/`, `defaultconfigs/*-server.toml`.
    - **Padrão de primeira execução**: `options.txt`, keybinds, HUD, minimapa, `servers.dat`.

    O Warden escolhe o mecanismo conforme o loader/versão e o instala automaticamente:
    - Fabric/Quilt: **Config Manager** (1.20+) ou **YOSBR** (1.8.9+);
    - NeoForge: Config Manager (1.20.1+) ou **Default Options** (exige Balm) ou **Configured Defaults** (1.18+);
    - Forge 1.13+: `defaultconfigs/` nativo para configs FML, mais Default Options ou Configured Defaults para `options.txt`;
    - Forge 1.7.10/1.12.2: Default Options, se disponível para a versão (ver Questões em aberto). Fallback: `preserve = true` no `index.toml`.
14. **Nunca exportar `options.txt` na raiz** quando houver mecanismo de primeira execução configurado. Aviso se o usuário tentar (E6; BMC4 e Prominence II fazem as duas coisas ao mesmo tempo).
15. **Detecção de mudanças durante o teste** (requisito do dono do projeto), com classificação automática do destino:
    - `config/*` alterado → autoritativo ou padrão (pergunta);
    - `options.txt` → padrão de primeira execução, **só as chaves alteradas** (diff por chave, não o arquivo inteiro);
    - `saves/<mundo>/serverconfig/*-server.toml` → `defaultconfigs/` (E16);
    - `saves/<mundo>/datapacks/*` → Paxi/Open Loader/`kubejs/data` (E15);
    - `resourcepacks/` novo + `options.txt resourcePacks` → resource pack padrão.

    Ignorar sempre: caches, `logs/`, `crash-reports/`, `screenshots/`, `journeymap/`, `XaeroWorldMap/`, `usercache.json`, `modernfix/`, `.mixin.out/`, `local/`, backups.
16. **Detecção de conflitos de keybind** a partir do `options.txt` da instância de teste (E14). P2.
17. **Editor de `options.txt` com chaves conhecidas** (render distance, `guiScale`, `lang`, `resourcePacks`, `onboardAccessibility`, `tutorialStep`) e preservação de `version` (data version). P1.

### 6.5 Launcher de teste (P0)

18. **Java por versão:** ler `javaVersion` do manifest da Mojang (8/16/17/21/25), com exceções do loader (Forge <1.13 → 8; lwjgl3ify → 17–25; Cleanroom → 21+). Baixar o runtime automaticamente.
19. **Presets de memória por tamanho do pack** (4.3): 3–4 / 6 / 8–10 GB, limitados a cerca de 50–60% da RAM. G1GC por padrão. Aviso acima de 8 GB em Java 8. Flags validadas contra a versão do Java (ex.: `-XX:+ZGenerational` só no Java 21–23; `UseCompactObjectHeaders` só no 25+). Nada de colar flags arbitrárias sem validação.
20. **Teste em instância limpa gerada a partir do pack exportado** (4.1, 4.5): botão "Testar como o jogador vai receber", separado do "Testar instância de trabalho".
21. **Teste de servidor dedicado local** (P1): gerar o server pack (só `both` + `server`), subir o servidor headless, esperar `Done (`, parar. Detecta E5 automaticamente. O Monifactory faz isso em CI (1.C).
22. **Teste de update com mundo antigo** (P2): carregar um backup de mundo da versão anterior e procurar *missing registry entries* (E8).

### 6.6 Exportação e release (P0/P1)

23. **Exportação limpa:** só arquivos rastreados pelo projeto. Relatório pré-export com contagem de arquivos e tamanho por pasta e alerta de pastas suspeitas (caches, mais de N arquivos em `shaderpacks/`, jars em `overrides/mods`).
24. **Checagem de licença por destino** (2.5):
    - Modrinth: todo arquivo embutido (inclusive mods do CurseForge que o `restrictDomains` embute) precisa de licença permissiva ou permissão registrada pelo usuário.
    - CurseForge: jars não CurseForge exigem aprovação manual da plataforma.
    - Gerar automaticamente `licenses/` / `THIRD-PARTY-LICENSES` (como o COBBLEVERSE).
25. **Mods do CurseForge com distribuição bloqueada** (`allowModDistribution=false` / `downloadUrl=null`): fluxo guiado de download manual (link + detecção do arquivo na pasta Downloads, como o Prism) para a instância de teste, e aviso de que jogadores terão o mesmo atrito.
26. **Server pack** (P1): gerado a partir do `side`, com:
    - `start.bat`/`start.sh`;
    - `user_jvm_args.txt` com o preset de memória;
    - instalador do loader;
    - `server.properties` de referência;
    - alternativa `packwiz-installer-bootstrap -g -s server <url>` documentada.
27. **Changelog automático em duas partes** (1.D item 6):
    - (a) diff de mods entre a versão anterior e a atual (Added/Removed/Updated com versões e links), gerado a partir dos metafiles, sem depender de nomes de jar;
    - (b) diff de configs/scripts por arquivo;
    - (c) campo de notas curadas;
    - (d) seção automática "**Atenção: faça backup do mundo**" quando houver remoção de mod com conteúdo ou troca de worldgen.
28. **Checklist de release** (4.5) como tela antes de publicar:
    - validação estática OK;
    - teste em instância limpa;
    - mundo novo;
    - servidor dedicado;
    - mundo antigo (se for update);
    - changelog revisado;
    - versão incrementada;
    - licenças OK.

    Cada item com status e data do último teste.
29. **Publicação** (P2): exportar `.mrpack` e zip do CurseForge (o packwiz já faz). Upload direto pelas APIs ou geração de workflow do GitHub Actions com `mc-publish`, como o FO.

### 6.7 Curadoria assistida (P1/P2)

30. **Sugestão de kit de performance por (loader, versão)** (2.3.5), apresentado como "recomendado" com explicação, nunca instalado em silêncio. O catálogo fica num arquivo de dados versionado.
31. **Sugestão de mods "de autor de pack"** conforme o conteúdo:
    - pack com KubeJS → ESLint/Prettier e `kubejs/probe` no ignore;
    - pack com servidor → BetterCompatibilityChecker;
    - pack grande → Crash Assistant;
    - pack com quests → FTB Quests e Lang Splitter.
32. **Indicadores de saúde por mod:** última atualização para a versão alvo, projeto arquivado/*withheld*, licença, lado, fonte (Modrinth, CurseForge, local).
33. **Templates de pack** (P2): "Otimização vanilla+" (estilo FO), "Kitchen sink", "Expert/tech com KubeJS", "Legacy 1.12.2" e "Legacy 1.7.10". Cada um com mecanismo de defaults, kit de performance e estrutura de pastas pré-configurados.

## 7. Questões em aberto

1. **`preserve` no packwiz:** como o CLI define e mantém `preserve = true` no `index.toml`? Sobrevive a `packwiz refresh`? O export para Modrinth/CurseForge respeita isso? (O `.mrpack` não tem campo equivalente; *inferência*: perde-se no export.) Precisa de teste prático. Isso afeta a escolha entre `preserve` e os mods de defaults.
2. **Forge (não Neo) e `defaultconfigs/` para configs não-server:** confirmar no código do Forge 1.16–1.20.1 se a cópia de `defaultconfigs/` vale para todos os tipos de config, como na FancyModLoader (3.1).
3. **Mecanismo de defaults para 1.7.10 e 1.12.2:** confirmar quais versões do Default Options e do Configured Defaults existem para essas versões. Se não houver, o fallback é `preserve` ou a aplicação pelo próprio launcher do Warden (que só funciona no teste interno, não para jogadores).
4. **Chave da API do CurseForge:** como o Warden vai obtê-la (chave própria do projeto via formulário da Overwolf, ou a embutida no packwiz) e quais termos de uso se aplicam a um app desktop de terceiros. Fora do escopo desta pesquisa; deve ser tratada em outra tarefa.
5. **Cobertura de metadados para 1.7.10/1.12.2:** a maioria dos mods dessas versões está só no CurseForge, com `mcmod.info` pobre. Avaliar se vale ler a anotação `@Mod(dependencies=...)` do bytecode (parse de classe Java em Rust), ou se basta depender de regras de log.
6. **Divergência no repositório do Star Technology** (`StarT-Dev-Team` vs `trulyno`): não afeta decisões, mas a seção 1.C fica marcada como incerta.
7. **EMI após a 1.21.1:** a API do Modrinth não mostra build mais recente. Confirmar se o mod mudou de slug ou está parado, antes de recomendá-lo em templates 1.21.2+.
8. **Valores oficiais de RAM do ATM10:** só há fontes secundárias. Usar os presets próprios (4.3) até haver fonte primária.
9. **Sinytra Connector:** decidir se o Warden suporta oficialmente packs NeoForge com mods Fabric via Connector, ou se só emite aviso. Nenhum pack grande estudado usa.
10. **Lista curada de conflitos e de mods só de cliente:** definir quem mantém, em que formato (arquivo de dados no repo do Warden, atualizado remotamente?) e como versionar. Depende da arquitetura do diagnóstico, definida em outra tarefa.
11. **Formato próprio vs. packwiz puro para metadados extras** (licença confirmada pelo usuário, classificação "autoritativo vs. padrão" de cada config, histórico de testes): guardar em arquivo próprio (ex.: `warden.toml`, que precisaria entrar no `.packwizignore`) ou em comentários? Decisão de arquitetura.
