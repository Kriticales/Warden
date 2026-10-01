# S1 — Escolha do motor do launcher (spike)

> Tarefa S1 do projeto Warden. Data: 2026-10-01. Branch: `spike/launcher-engine`.
> Código descartável em `spikes/launcher-engine/`, na branch `spike/launcher-engine` (ver o README de lá
> para reproduzir).
>
> Convenção: **[verificado]** = executei e vi o resultado; **[código]** = li no código-fonte do
> projeto citado; **[inferência]** = conclusão minha, não testada.

## 1. Resumo e decisão recomendada

**Recomendação: usar a crate `portablemc` (5.0.5, Apache-2.0) como motor de instalação, atrás de
uma trait `LauncherEngine` do Warden, e deixar o processo do jogo (iniciar, capturar logs, detectar
"carregou", parar) inteiramente com código do próprio Warden.**

- O portablemc instalou e abriu, em modo offline com o jogador `WardenTest`, **todas** as combinações
  pedidas — Fabric 1.20.1, Forge 1.12.2 (14.23.5.2860), NeoForge 1.21.1 (21.1.252), Forge 1.7.10
  (10.13.4.1614) — e também Forge 1.16.5 (36.2.34), com Java baixado automaticamente, em diretórios
  escolhidos por nós. O jogo terminou de carregar e chegou ao menu — tela de título (1.7.10, 1.12.2,
  1.16.5) ou a tela de boas-vindas que o jogo mostra na primeira abertura a partir do 1.19 (1.20.1,
  1.21.1) —, confirmado por log **e** por captura de tela, e foi encerrado pelo nosso código em menos de 1 s **[verificado]**.
- Funcionou também **no Windows nativo** (compilado com MSVC e executado via interop do WSL): Fabric
  1.20.1 e Forge 1.12.2 abriram e foram encerrados **[verificado]**.
- O app-lib do Modrinth (theseus) **também conseguiu** abrir as mesmas versões, mas só depois de
  injetar uma credencial falsa no banco SQLite interno dele (não há conta offline), e mostrou
  problemas sérios de encaixe: estado global com banco e serviços de fundo (Discord, WebSocket da
  Modrinth), módulo `launcher` privado, build que exige JDK + Gradle, metadados de terceiros
  (`launcher-meta.modrinth.com`), troca silenciosa da versão do Forge pedida e captura de log que
  reteve a saída do NeoForge até o processo terminar **[verificado]**. **Descartado.**
- Escrever o próprio motor (opção C) não é necessário agora; fica como plano B, partindo de um fork
  do portablemc (a licença Apache-2.0 permite), se ele for abandonado ou não atender.

## 2. Ambiente e método

| Item | Valor |
|---|---|
| Máquina | WSL2 (Ubuntu 26.04, kernel 6.18.33.2-microsoft-standard-WSL2), 16 CPUs, 15 GB RAM |
| Rust | 1.98.1 (portablemc); 1.95.0 para o theseus (fixado pelo monorepo do Modrinth) |
| Vídeo no Linux | **Xvfb sem root**: `apt-get download` + `dpkg -x` (sem `sudo`). O socket Unix do WSLg (`/tmp/.X11-unix`) é montado somente leitura, então o Xvfb escuta só via TCP (`DISPLAY=127.0.0.1:77`). OpenGL por software: **Mesa 26.0.8 llvmpipe** (já instalado no sistema) — o log do 1.7.10 mostra `Renderer: 'llvmpipe (LLVM 21.1.8, 256 bits)'` **[verificado]** |
| Áudio | `ALSOFT_DRIVERS=null` (na 1ª execução o OpenAL abriu o dispositivo "RDP Sink" do WSLg e poderia tocar som no Windows) |
| Windows | Windows real do dono via interop (`/mnt/c/Windows/System32/cmd.exe`), Rust 1.98.1 MSVC + Visual Studio 2022 já instalados. Dados em `%LOCALAPPDATA%\warden-spike\pmc` |
| Dados | `~/.local/share/warden-spike/` (fora do repositório). Nada de Java/jars/assets no git |

Critério de "carregou" (marcadores no log, todos precisam aparecer, sem padrão fatal), seguido de
10 s de estabilidade, captura de tela (Linux) e encerramento pelo nosso código:

| Faixa | Marcadores usados (definidos empiricamente nos logs reais) |
|---|---|
| 1.7.10 Forge | `Forge Mod Loader has successfully loaded`, `textures/blocks-atlas`, `Sound engine started` |
| 1.12.2 Forge | `Forge Mod Loader has successfully loaded`, `textures-atlas`, `Sound engine started` |
| 1.16.5 Forge | `Launching target 'fmlclient'`, `Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas`, `Sound engine started` |
| 1.20.1 Fabric | `Loading Minecraft 1.20.1 with Fabric Loader`, o mesmo `Created: ...blocks.png-atlas`, `Sound engine started` |
| 1.21.1 NeoForge | `Launching target 'forgeclient'`, o mesmo `Created: ...blocks.png-atlas`, `Sound engine started` |

Observações sobre os marcadores (úteis para o diagnóstico do Warden):
- O formato da linha do atlas muda por faixa: `Created: 512x256 textures/blocks-atlas` (1.7.10),
  `Created: 512x512 textures-atlas` (1.12.2), `Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas`
  (1.16+). O tamanho varia com os mods, então o Warden deve casar só o sufixo. **[verificado]**
- **Falso positivo:** o FML do 1.7.10 imprime de propósito um `---- Minecraft Crash Report ----` com
  `Description: Loading screen debug info` / `THIS IS NOT A ERROR` a partir de
  `cpw.mods.fml.client.SplashProgress`. Na 1ª tentativa meu detector tratou isso como crash. O
  diagnóstico determinístico do Warden precisa ignorar esse caso. **[verificado]**
- O 1.7.10 cria atlas `16x16` *antes* do FML terminar; a seção 8.6 do relatório 02 sugeria
  `Forge Mod Loader has successfully loaded` — confirmado que existe em 1.7.10 e 1.12.2.

## 3. Candidato A — portablemc (crate Rust, Apache-2.0)

### 3.1 O que é e como a API se encaixa **[código]**

- Crate `portablemc` 5.0.5 (2026-09-09), MSRV 1.88, ~9,6 mil linhas. Módulos `moj` (vanilla),
  `fabric` (Fabric, Quilt, LegacyFabric, Babric), `forge` (Forge e NeoForge, inclusive instaladores
  legados e processors), `msa` (login Microsoft — não usado).
- Cada instalador tem `install(handler) -> Result<Game>`. `Game` é uma struct pública com
  `jvm_file`, `mc_dir`, `main_class`, `jvm_args`, `game_args` e `command() -> std::process::Command`.
  Ou seja: **o motor só monta a linha de comando; quem inicia o processo somos nós.** É exatamente o
  corte que o Warden precisa.
- Todos os diretórios são configuráveis separadamente: `versions`, `libraries`, `assets`, `jvm`
  (runtimes), `bin` (natives, em subpasta com hash) e `mc_dir` (o `.minecraft` da instância).
- Eventos: `Handler::on_event(Event)` com enums `#[non_exhaustive]` por camada (base → moj →
  fabric/forge). Há eventos de etapa, de progresso de download (`count/total_count/size/total_size`),
  de Java escolhido e de cada processor do Forge/NeoForge (`RunInstallerProcessor { name, task }`).
- Cancelamento: só existe um ponto, o evento `DownloadResources { cancel: &mut bool }`, emitido
  antes de cada lote de downloads. Não dá para interromper no meio de um lote.
- Erros: `thiserror`, tipados por camada (ex.: `forge::Error::InstallerNotFound`,
  `InstallerProcessorFailed`, `base::Error::JvmNotFound`), com uma variante `Internal` que embrulha
  io/json/zip/reqwest com a origem.
- Offline: `set_auth_offline_username(nome)` deriva o UUID como o jogo (`OfflinePlayer:<nome>`, MD5,
  versão 3). Os argumentos `--clientId`, `--xuid` e `--userType` saem **vazios**.
- API **síncrona**: internamente cria um runtime tokio `current_thread` e faz `block_on`
  (`src/tokio.rs`). Consequência: no Tauri deve ser chamada em `spawn_blocking`/thread própria
  (chamar de dentro de uma task tokio provocaria pânico de "runtime dentro de runtime" —
  **[inferência]** pelo comportamento documentado do tokio; não testei).
- Java: política `Mojang` (runtime oficial), `System`, as combinações, ou `Static(caminho)`.

### 3.2 Protótipo

`spikes/launcher-engine/pmc-proto/`:
- `src/engine.rs`: esboço da trait `LauncherEngine` e `PortableMcEngine` (tradução dos eventos
  emprestados do portablemc para um enum próprio `EngineEvent`; `CancelFlag`; `PreparedLaunch`).
- `src/supervisor.rs`: `RunningGame` — `spawn` com stdout/stderr em pipes lidos por duas threads,
  decodificação tolerante (`from_utf8_lossy`), cópia de tudo num arquivo próprio, `stop(grace)` com
  SIGTERM e, se não fechar no prazo, SIGKILL (no Windows, `TerminateProcess`).
- `src/main.rs`: casos de teste, marcadores, captura de tela.

### 3.3 Comandos executados

```bash
spikes/launcher-engine/scripts/xvfb.sh 77            # em segundo plano
export DISPLAY=127.0.0.1:77 ALSOFT_DRIVERS=null
cd spikes/launcher-engine/pmc-proto && cargo build --release
./target/release/pmc-proto fabric-1.20.1   --timeout 600 --print-command
./target/release/pmc-proto forge-1.12.2    --timeout 600 --screenshot
./target/release/pmc-proto neoforge-1.21.1 --timeout 900 --screenshot
./target/release/pmc-proto forge-1.7.10    --timeout 300 --screenshot
./target/release/pmc-proto forge-1.16.5    --timeout 600 --screenshot
./target/release/pmc-proto fabric-1.20.1   --timeout 600 --screenshot
./target/release/pmc-proto neoforge-1.21.1 --java ~/.local/share/warden-spike/jdk/jdk-21.0.12.1+1/bin/java
# Windows (via interop, código copiado para %TEMP%\warden-spike\pmc-proto):
cmd.exe /c "cargo build --release"
cmd.exe /c "set WARDEN_SPIKE_DATA=C:\Users\solel\AppData\Local\warden-spike\pmc&& target\release\pmc-proto.exe fabric-1.20.1 --install-only --print-command"
cmd.exe /c "set WARDEN_SPIKE_DATA=...&& set ALSOFT_DRIVERS=null&& target\release\pmc-proto.exe fabric-1.20.1 --timeout 300 --settle 5"
cmd.exe /c "...&& target\release\pmc-proto.exe forge-1.12.2 --timeout 300 --settle 5"
```

### 3.4 Resultados (todos **[verificado]**)

| Caso | Java (baixado automaticamente) | Instalação (1ª vez) | Até "carregou" | Encerramento | Tela |
|---|---|---|---|---|---|
| Fabric 1.20.1 (loader 0.19.5), Linux | Mojang `java-runtime-gamma` 17.0.15 | 66,7 s, 3770 arquivos, ~836 MB (inclui Java e assets) | 12,4 s (8,7 s na 2ª vez) | SIGTERM, saiu em 0,2 s, código 143, sem forçar | boas-vindas/acessibilidade (captura) |
| Forge 1.12.2-14.23.5.2860, Linux | Mojang `jre-legacy` 8u202 | 25,3 s | 11,4–16,2 s | 0,2–0,3 s, 143 | título (captura) |
| NeoForge 21.1.252 (MC 1.21.1), Linux | Mojang `java-runtime-delta` 21.0.7 | 44,0 s, com 6 processors (`MCP_DATA`, `DOWNLOAD_MOJMAPS`, `MERGE_MAPPING`, `jarsplitter`, `AutoRenamingTool`, `binarypatcher`) | 13,2 s | 0,5 s, 143 | boas-vindas/acessibilidade (captura; 1ª abertura de instância nova) |
| NeoForge 21.1.252 com **Temurin 21.0.12.1** (`JvmPolicy::Static`) | — | (cache) | 14,8 s | 0,5 s, 143 | — |
| Forge 1.7.10-10.13.4.1614, Linux | Mojang `jre-legacy` 8u202 | 8,6 s | 9,1 s | 0,2 s, 143 | título (captura) |
| Forge 1.16.5-36.2.34, Linux | Mojang `jre-legacy` 8u202 | 22,6 s, 4 processors | 15,1 s | 0,5 s, 143 | título (captura) |
| Fabric 1.20.1, **Windows** | Mojang 17.0.15 (`javaw.exe`) | 47,7 s | 30,0 s (GPU real) | `TerminateProcess`, 0,3 s, código 1 | (janela real, sem captura) |
| Forge 1.12.2, **Windows** | Mojang **8u51** (`javaw.exe`) | 23,9 s | 25,1 s | 0,3 s, código 1 | (janela real) |

Reexecutar uma instalação já feita leva ~1–2 s (verificação de arquivos existentes; sem rede para
Forge com versão fixa — testado com proxy inválido) **[verificado]**.

Trechos reais dos logs capturados pelo nosso supervisor:

```text
# Fabric 1.20.1 (Linux) — eventos XML do log4j, conteúdo extraído
Loading Minecraft 1.20.1 with Fabric Loader 0.19.5
Loading 4 mods:
Setting user: WardenTest
Backend library: LWJGL version 3.3.1 SNAPSHOT
OpenAL initialized on device No Output
Sound engine started
Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas

# Forge 1.12.2
Forge Mod Loader version 14.23.5.2860 for Minecraft 1.12.2 loading
Setting user: WardenTest
LWJGL Version: 2.9.4
Sound engine started
Created: 512x512 textures-atlas
Forge Mod Loader has successfully loaded 4 mods

# Forge 1.7.10
Forge Mod Loader version 7.99.40.1614 for Minecraft 1.7.10 loading
Setting user: WardenTest
LWJGL Version: 2.9.1
Forge Mod Loader has successfully loaded 3 mods
Created: 512x256 textures/blocks-atlas
Sound engine started

# Forge 1.16.5 (o próprio Forge mascara o token)
Launching target 'fmlclient' with arguments [--version, forge-1.16.5-36.2.34, ..., --uuid,
  6c5aa2b1c08439d799a8edc5b372e5f9, --username, WardenTest, --assetIndex, 1.16, --accessToken,
  ❄❄❄❄❄❄❄❄, --userType, , --versionType, release

# NeoForge 1.21.1 — texto puro (não XML) depois que o FML assume o log
[11.153s out] [18:02:16] [Render thread/INFO] [minecraft/Minecraft]: Setting user: WardenTest
[11.492s out] [18:02:16] [modloading-worker-0/INFO] [ne.ne.ne.co.NeoForgeMod/NEOFORGE-MOD]: NeoForge mod loading, version 21.1.252, for MC 1.21.1
[14.640s out] [18:02:20] [Render thread/INFO] [minecraft/SoundEngine]: Sound engine started
[14.787s out] [18:02:20] [Render thread/INFO] [minecraft/TextureAtlas]: Created: 1024x512x4 minecraft:textures/atlas/blocks.png-atlas
```

O UUID `6c5aa2b1c08439d799a8edc5b372e5f9` é exatamente `MD5("OfflinePlayer:WardenTest")` com versão 3
e variante RFC 4122, calculado de forma independente em Python **[verificado]**.

Saída do nosso programa no caso Forge 1.7.10 (resumida):

```text
== mainClass: net.minecraft.launchwrapper.Launch
== tamanho da linha de comando: 6150 caracteres
== jogo iniciado (pid 335382), saída em .../runs/forge-1.7.10-1790891056.log
[    8.8s] marcador: textures/blocks-atlas
[    8.9s] marcador: Forge Mod Loader has successfully loaded
[    9.1s] marcador: Sound engine started
== CARREGOU em 9.1s após o spawn; aguardando 10s de estabilidade
== captura de tela: .../runs/forge-1.7.10-1790891056.png
== encerrado: status=Some("exit status: 143") forçado=false em 0.2s
```

Testes de controle **[verificado]**:

| Teste | Resultado |
|---|---|
| Cancelar antes dos downloads (`--cancel-before-download`, pasta de dados vazia) | Parou em 2,8 s com `EngineError::Cancelled`; ficaram 524 KB no disco (só metadados). Na 1ª tentativa o meu código marcava o cancelamento tarde demais e a instalação foi até o fim — o ponto de checagem do portablemc é só um por lote |
| Forge inexistente `1.12.2-14.23.5.9999` | Falhou em 1,0 s: `installer not found: 1.12.2-14.23.5.9999` |
| Fabric para MC inexistente `1.99.9` | Falhou em 0,5 s: `latest version not found (stable: true)` — mensagem **enganosa** (o problema é a versão do jogo) |
| Sem rede (proxy inválido), pasta vazia | Falhou na hora com cadeia completa: `... error sending request for url (https://meta.fabricmc.net/v2/versions/loader/1.20.1) ... tcp connect error <- Connection refused (os error 111)` (a cadeia repete o mesmo texto em 4 camadas) |
| Sem rede, Forge 1.12.2 já instalado | Instalação "concluída" e linha de comando gerada normalmente |

Linhas de comando: 6–8 mil caracteres no Linux; **10 364** (Fabric 1.20.1) no Windows; 13,8 mil para
NeoForge 1.21.1 no Linux. Abaixo do limite de 32 767 do Windows, mas o portablemc não gera `@argfile`
— fica com o Warden (relatório 02, seção 8.1, item 6).

Cache compartilhado após todos os testes no Linux: assets 883 MB, runtimes 426 MB, libraries 381 MB,
versions 137 MB, natives 19 MB **[verificado]**.

## 4. Candidato B — app-lib do Modrinth App ("theseus", GPL-3.0-only)

### 4.1 Como a API se encaixa **[código]** (checkout `modrinth/code` em `e0bd3bc`, 2026-10-01)

- `lib.rs` declara `mod launcher;` **privado**. A única forma de lançar é `instance::run(id,
  QuickPlayType)`, que lê a credencial ativa de um **banco SQLite interno** (`app.db`) via
  `Credentials::get_default_credential`; sem credencial: `NoCredentialsError`. **Não existe conta
  offline** (pedido aberto https://github.com/modrinth/code/issues/7408, citado no relatório 02).
- Tudo passa por um **singleton global** `State::init(app_identifier)`, que abre o banco, roda
  migrações e inicia em segundo plano Discord Rich Presence, o WebSocket de amigos da Modrinth e o
  refresh de credenciais da Modrinth (`state/mod.rs`, `tokio::try_join!` + `friends_socket.connect`).
- Pasta de dados: só pela variável de ambiente `THESEUS_CONFIG_DIR` (ou configuração `custom_dir`
  no banco). Instâncias ficam em `<dados>/profiles/<nome>`; o cache em `<dados>/meta/...`. O Warden
  não escolhe o caminho de cada instância.
- Metadados de versões: **`launcher-meta.modrinth.com`** (variável de compilação
  `MODRINTH_LAUNCHER_META_URL`) — fonte de terceiros que o relatório 02 recomendou não usar em
  produção. As versões de Forge disponíveis são as que o meta da Modrinth lista.
- Progresso/cancelamento: sistema de *jobs* (`install::create_instance` → `get_job(id)` com fase,
  `progress {current,total}`; `cancel_job`, `pause_job`, `resume_job`). Sem a feature `tauri`, os
  eventos não são emitidos; só dá para consultar por *polling*. É mais rico que o portablemc nesse
  ponto.
- Processo: o theseus faz o `spawn` e guarda o `Child`; o stdout é processado por um parser próprio
  que grava `logs/launcher_log.txt`. `process::kill` chama `child.kill()` (SIGKILL — não há
  encerramento gracioso).
- Build: o `build.rs` executa **Gradle** (precisa de JDK 17+) para compilar JARs Java próprios; as
  URLs da Modrinth entram em tempo de compilação via `.env` (`env!(...)`); `sqlx` exige
  `SQLX_OFFLINE=true`; toolchain fixada em Rust 1.95.0.

### 4.2 Protótipo e o contorno necessário

`spikes/launcher-engine/theseus-proto/` (dependência por caminho para o checkout local):
`EventState::init` → `State::init("warden-spike")` → **inserção direta em `minecraft_users`** de uma
linha com o UUID offline, `access_token = '0'` e validade de 10 anos (para o theseus não tentar
renovar pela Microsoft) → `install::create_instance` → *polling* de `get_job` → `instance::run` →
leitura de `logs/launcher_log.txt` → `process::kill` + `wait_for`. **Isso é um contorno que depende
do esquema interno do banco; não seria aceitável em produção.**

```bash
cp packages/app-lib/.env.prod packages/app-lib/.env      # no checkout do modrinth/code
JAVA_HOME=<temurin-21> SQLX_OFFLINE=true cargo build -p theseus   # 4 min 58 s, pico 3,7 GB RAM
cd spikes/launcher-engine/theseus-proto
JAVA_HOME=<temurin-21> SQLX_OFFLINE=true cargo build              # 2 min 23 s (pasta de build nova, compilando tudo)
theseus-proto fabric-1.20.1 | forge-1.12.2-2860 | forge-1.7.10 | neoforge-1.21.1
```

### 4.3 Resultados (todos **[verificado]**, Linux/Xvfb)

| Caso | Java (Azul Zulu, baixado) | Instalação | Até "carregou" | Encerramento |
|---|---|---|---|---|
| Fabric 1.20.1 | Zulu 17.0.20.1 | 143,4 s (fases `ResolvingLoader` → `PreparingJava` → `DownloadingMinecraft` → `Finalizing`) | 11,5 s | `kill()` + `wait_for()` em 0,9 s |
| Forge 1.12.2 pedindo `"1.12.2-14.23.5.2860"` | Zulu 8u504 | 24,4 s | 10,0 s | 0,8 s |
| Forge 1.12.2 pedindo `"14.23.5.2860"` | Zulu 8u504 | 6,8 s | 11,0 s | 0,3 s |
| Forge 1.7.10 (`"10.13.4.1614"`) | Zulu 8u504 | 8,6 s | 26,7 s | 0,6 s |
| NeoForge 1.21.1 (`"21.1.252"`) | Zulu 21.0.12.1 | 97,6 s (10 processors) | o jogo carregou (o `latest.log` do jogo tem `Sound engine started` e o atlas ~18 s após o `run()`), **mas a captura de saída do theseus ficou retida a partir da linha 164** e só foi gravada quando o processo morreu; meu protótipo, que lê essa captura, esgotou o tempo de 600 s sem ver os marcadores | `kill()` + `wait_for()` em 0,7 s |

Problemas encontrados:

1. **Troca silenciosa de versão**: pedindo o Forge `"1.12.2-14.23.5.2860"` (formato do Maven), o
   theseus instalou e abriu **14.23.5.2864** sem erro nem aviso (`Forge Mod Loader version
   14.23.5.2864 for Minecraft 1.12.2 loading`). O id esperado é o do meta da Modrinth (`14.23.5.2860`);
   um id desconhecido cai na versão mais recente (`get_loader_version_from_profile`). Para um app que
   versiona modpacks, isso é grave.
2. **Captura de log retida no NeoForge**: o parser do theseus lê o stdout como XML do log4j quando
   a versão declara configuração de log, mas o NeoForge passa a escrever texto puro. No primeiro stack
   trace com `<init>` (`GameNarrator.<init>`), o parser tratou `<init>` como tag (no arquivo o
   `<init>` some e a linha quebra) e passou a **reter** a saída: `launcher_log.txt` ficou parado na
   linha 164 durante ~10 minutos, enquanto o jogo seguia normalmente, e as linhas restantes
   (`Sound engine started`, atlas etc., com horário 17:56:33) só foram gravadas quando o processo foi
   encerrado (arquivo modificado às 18:06:22, total 210 linhas). Para detectar "carregou" ou um crash
   em tempo real, isso equivale a não ter log.
3. Encerramento só por SIGKILL (sem chance de o jogo salvar) **[código]**.

## 5. Candidato C — escrever o próprio motor

Não foi prototipado, porque A atendeu. Estimativa **[inferência]**, usando a lista P0 do relatório 02
(seção 8.1) e o tamanho do portablemc (~9,6 mil linhas Rust para o mesmo escopo) como referência:

| Bloco | Esforço estimado (dev experiente) |
|---|---|
| Modelos JSON, regras, merge `inheritsFrom`, placeholders, classpath | 4–6 dias |
| Downloader com hash, retomada, escrita atômica, assets/natives (3 gerações) | 4–6 dias |
| Java (Temurin + runtime Mojang) | 2–3 dias |
| Fabric + Forge legado (1.7.10/1.12.2, JAR `universal`, Pack200 não mais necessário) | 3–4 dias |
| Executor de processors (Forge 1.13+ e NeoForge, 3 gerações de `install_profile`) | 4–6 dias |
| Testes por faixa (1.7.10 → 26.x) e correções de casos de borda (ex.: `14.23.5.2851`, LWJGL do 1.12.2 no macOS, authlib 1.16.5) | 7–10 dias |
| **Total** | **~5–7 semanas** e manutenção contínua a cada mudança da Mojang/loaders |

Conclusão: só compensa se o portablemc deixar de ser mantido; mesmo assim o melhor caminho seria
um **fork** dele (Apache-2.0), não começar do zero.

## 6. Tabela comparativa

| Critério | A — portablemc | B — theseus (Modrinth app-lib) | C — próprio |
|---|---|---|---|
| Embutível sem Tauri | **Sim**: biblioteca pura, sem estado global, sem banco | Compila sem Tauri, mas exige `State` global, SQLite, serviços de fundo (Discord, WebSocket Modrinth) e Gradle/JDK no build | Sim |
| Modo offline | **Nativo** (`set_auth_offline_username`, UUID igual ao do jogo) | **Inexistente**; só com credencial falsa injetada no banco | A implementar (simples) |
| Controle de diretórios | **Total** e separado: versions, libraries, assets, runtimes, natives, instância | Uma raiz por variável de ambiente; instâncias em `profiles/<nome>` | Total |
| Progresso | Eventos por callback (etapas, downloads, Java, cada processor) | Jobs com fase e `current/total`, por *polling* (eventos só com feature `tauri`) | A implementar |
| Cancelamento | Só antes de cada lote de downloads (`DownloadResources`) — **verificado** | `cancel_job`/`pause_job`/`resume_job` (não testado) | A implementar |
| Erros | Tipados por camada; algumas mensagens enganosas; `Internal` genérico | `crate::Error` amplo; troca silenciosa de versão | A implementar |
| 1.7.10 / 1.12.2 / 1.16.5 / 1.20+ | **Todas verificadas** (+ 1.21.1 NeoForge) | 1.7.10, 1.12.2, 1.20.1, 1.21.1 instalam e abrem (1.16.5 não testado) | — |
| Windows | **Verificado** (MSVC, `javaw.exe`, natives Windows, Fabric e Forge abriram) | Não testado (o Modrinth App é distribuído para Windows) | — |
| Captura de stdout/stderr | **Total**: o Warden recebe um `Command` e faz o `spawn` | Interna; só um arquivo de texto após o parser deles (que reteve a saída do NeoForge até o fim do processo) | Total |
| Java | Runtime Mojang automático (no Windows o Java 8 é **8u51**) ou `Static(caminho)` (**Temurin 21 verificado**) | Azul Zulu automático (8u504, 17.0.20, 21.0.12) | A implementar |
| Fonte de metadados | Primárias: piston-meta, meta.fabricmc.net, Maven do Forge/NeoForge | `launcher-meta.modrinth.com` (terceiro) | Primárias |
| Tamanho / dependências | 223 crates; binário release 13 MB (Linux), 9,2 MB (Windows); build ~2 min | 519 crates; build 5 min, 3,7 GB de RAM, binário debug 550 MB; precisa JDK + Gradle no build | Menor |
| Manutenção | Ativa (5.0.5 em 2026-09-09; 185 commits em 12 meses), mas **um único mantenedor** | Muito ativa (88 commits em `app-lib` em 3 meses), equipe da Modrinth; API interna muda sem aviso (não é publicada no crates.io) | Toda nossa |
| Licença | **Apache-2.0** (permite fork e qualquer licença do Warden) | GPL-3.0-only (aceitável para uso privado; obriga GPL se um dia distribuir) | Nossa |
| Esforço para manter atualizado | Trocar a versão fixada; testes por faixa | Acompanhar monorepo inteiro, migrações de banco e mudanças internas | Alto |

## 7. Recomendação final, justificativa e riscos

**Adotar o portablemc**, com versão fixada exatamente (`portablemc = "=5.0.5"`), atrás da trait
`LauncherEngine` do Warden.

Justificativa: é o único candidato que (1) faz o que precisamos sem contornos, (2) devolve uma linha
de comando em vez de esconder o processo — o que permite ao Warden capturar logs, detectar marcadores,
diagnosticar crashes e encerrar o jogo do jeito que quiser —, (3) usa fontes primárias de metadados,
(4) funcionou em todas as faixas pedidas e no Windows, e (5) tem licença permissiva, o que deixa o
fork como plano B real.

| Risco | Impacto | Mitigação |
|---|---|---|
| Um único mantenedor (bus factor 1) | Atualizações para novas versões do jogo podem atrasar | Versão fixada; testes de fumaça por faixa no CI; se parar, fork (Apache-2.0) mantido pelo Warden |
| Cancelamento só entre lotes de download | "Cancelar" pode demorar até o lote atual terminar (ex.: ~800 MB na 1ª instalação) | Mostrar "cancelando..." na UI; propor upstream um ponto de cancelamento no progresso de download (ou patch local) |
| API síncrona com runtime tokio próprio | Pânico se chamada dentro de task tokio **[inferência]** | Sempre `spawn_blocking`/thread dedicada (encapsulado no adaptador) |
| Java da Mojang (Windows: 8u51 de 2015) | TLS/segurança antigos no Forge 1.7.10/1.12.2 | Warden gerencia Temurin (relatório 02, seção 2.5) e passa `JvmPolicy::Static` — **verificado com Temurin 21** |
| Mensagens de erro enganosas / variante `Internal` | Mensagem ruim para um usuário leigo | Warden valida versões antes (com o próprio resolvedor de versões) e traduz erros para português |
| Argumentos `--userType`, `--clientId`, `--xuid` vazios; sem `-Xmx`, sem `@argfile`, sem `--offlineDeveloperMode` (1.21.9+) | Detalhes de fidelidade e limites do Windows | O `PreparedLaunch` é nosso: o Warden ajusta os argumentos antes do `spawn` (memória, `-XX:ErrorFile`, `userType=legacy`, `--offlineDeveloperMode`, `@argfile`) |
| Correções automáticas do portablemc (ex.: `fix_broken_authlib` para 1.16.4/1.16.5, proxy legado) | Podem tocar na postura legal da seção 4.5 do relatório 02 | Revisar cada `fix_*` e desligar o que não for desejado (todos têm setter) |
| Forge pede "please do not automate" | Ético/imagem | Mesmo de qualquer launcher: creditar o Forge na UI (relatório 02, seção 3.4) |

## 8. Encaixe na arquitetura

Corte validado no protótipo: **o motor instala e devolve um plano de lançamento; o Warden é dono do
processo.** Trocar de motor no futuro afeta só o adaptador.

```rust
// crate warden-launcher (sem Tauri)

pub trait LauncherEngine: Send + Sync {
    /// Instala (idempotente) MC + loader + Java e devolve o plano de lançamento.
    /// Bloqueante: o app chama em spawn_blocking.
    fn install(
        &self,
        req: &InstallRequest,          // mc_version, LoaderSpec, instance_dir, JavaSpec, jogador offline
        events: &mut dyn FnMut(EngineEvent), // Stage, Progress, JavaSelected, ProcessorStarted, Warning
        cancel: &CancelFlag,
    ) -> Result<LaunchPlan, EngineError>;
}

pub struct LaunchPlan { pub java: PathBuf, pub working_dir: PathBuf,
                        pub jvm_args: Vec<String>, pub main_class: String, pub game_args: Vec<String> }

// Parte do Warden, independente do motor:
pub struct GameRunner;              // start(plan, LaunchOptions) -> GameHandle
pub struct GameHandle;              // events() -> Receiver<GameEvent>; stop(grace); kill()
pub enum GameEvent { Line { stream, text }, Ready, CrashDetected(..), Exited(ExitStatus) }
```

- `PortableMcEngine` implementa `LauncherEngine` (protótipo em `pmc-proto/src/engine.rs`);
  `LaunchOptions` aplica os ajustes do Warden ao `LaunchPlan` (memória, encoding, `@argfile`,
  `--offlineDeveloperMode` etc.).
- `GameRunner` = `supervisor.rs` evoluído: duas threads de leitura, arquivo
  `<instância>/.warden/logs/<data>.log`, parser log4j XML **e** texto (o NeoForge mistura os dois — o
  defeito do theseus mostra por que isso importa), marcadores por faixa, detector de crash com a
  exceção do `SplashProgress`.
- Parar: Linux — SIGTERM e, após o prazo, SIGKILL (verificado: 0,2–0,5 s, código 143). Windows —
  hoje `TerminateProcess` (código 1, sem encerramento gracioso); em produção, colocar o jogo num *Job
  Object* (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) e tentar fechar a janela com `WM_CLOSE` antes de
  `TerminateJobObject` **[inferência]**.
- Diretórios: caches compartilhados (`versions`, `libraries`, `assets`, `runtimes`, `natives`) na pasta
  de dados do Warden; o `instance_dir` é o `.minecraft` da instância de teste, separado do projeto
  do pack.
- Tauri: comando `install` em `spawn_blocking`, eventos repassados por `Channel`; `CancelFlag`
  ligado ao botão "Cancelar".

## 9. O que ficou sem verificar

- theseus no Windows (não compilei lá: exige JDK + Gradle no Windows).
- theseus com Forge 1.16.5; `cancel_job`/`pause_job` do theseus.
- portablemc: Legacy Fabric (1.7.10/1.12.2), Quilt, versões 26.x (Java 25, `--offlineDeveloperMode`),
  macOS, NeoForge 1.20.1 (`net.neoforged:forge`) e NeoForge recente (nova `mainClass`
  `net.neoforged.fml.startup.Client`).
- Pack com mods de verdade (todos os testes foram com zero mods além dos do próprio loader); tempo de
  carregamento e tamanho da linha de comando crescem com packs grandes.
- Captura de tela no Windows (a janela abriu na área de trabalho real; só o log foi verificado).
- Encerramento gracioso no Windows (só `TerminateProcess` foi testado).
- Chamar `install` de dentro de uma task tokio (o pânico esperado é inferência).

## 10. Achados fora do escopo (para outras tarefas)

- Instância nova de 1.19+ abre na tela de boas-vindas de acessibilidade; o Warden pode pré-gravar
  `onboardAccessibility:false` no `options.txt` da instância de teste (editor de configs/instância).
- `promotions_slim.json` em 2026-10-01: 1.12.2 recomendado `14.23.5.2859`, mais recente `2864`;
  1.16.5 recomendado `36.2.34`; 1.20.1 recomendado `47.4.10`. NeoForge 1.21.1 mais recente `21.1.252`.
- Áudio: no WSL2 o OpenAL usa o PulseAudio do WSLg e toca no Windows; testes automatizados devem usar
  `ALSOFT_DRIVERS=null`.
- Chamadas a serviços da Mojang com perfil offline geram `401` em `/player/attributes` (1.20.1) e
  `Failed to fetch user properties` (1.21.1) — esperados (relatório 02, seção 4.1); o diagnóstico não
  deve tratá-los como erro.
