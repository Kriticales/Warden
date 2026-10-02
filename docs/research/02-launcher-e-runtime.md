# R2 — O que faz um modpack funcionar e como construir um launcher próprio offline

> Documento de pesquisa do projeto Warden (tarefa R2). Data da pesquisa: 2026-10-01.
> Escopo: lançamento do Minecraft Java Edition (vanilla e com loaders), gestão de Java, modo offline,
> estrutura da instância, logs/crashes e sincronização instância ↔ pack do packwiz.
>
> Convenção: **[verificado]** = conferido diretamente em dados/código primários durante esta pesquisa
> (JSONs da Mojang, instaladores baixados, APIs de meta); **[fonte]** = afirmado por documentação
> ou código citado; **[inferência]** = conclusão minha, não confirmada por fonte primária.

## Sumário

1. [Como o Minecraft Java é iniciado por um launcher](#1-como-o-minecraft-java-é-iniciado-por-um-launcher)
2. [Java: versões exigidas e gestão automática de runtimes](#2-java-versões-exigidas-e-gestão-automática-de-runtimes)
3. [Loaders: Fabric, Quilt, Forge e NeoForge](#3-loaders-fabric-quilt-forge-e-neoforge)
4. [Modo offline: perfil, UUID e termos da Mojang/Microsoft](#4-modo-offline-perfil-uuid-e-termos-da-mojangmicrosoft)
5. [Estrutura da instância (.minecraft)](#5-estrutura-da-instância-minecraft)
6. [Logs e crashes](#6-logs-e-crashes)
7. [Sincronização instância de teste ↔ pack do packwiz](#7-sincronização-instância-de-teste--pack-do-packwiz)
8. [Implicações para o Warden](#8-implicações-para-o-warden)
9. [Questões em aberto](#9-questões-em-aberto)

---

## 1. Como o Minecraft Java é iniciado por um launcher

### 1.1 Visão geral do pipeline

Um launcher de Minecraft Java não "executa um .exe do jogo": ele monta uma linha de comando Java.
O fluxo completo, igual em todos os launchers estudados (oficial, Prism, Modrinth App, HMCL,
ATLauncher), é:

1. Baixar o **version manifest** (lista de todas as versões).
2. Baixar o **JSON da versão** escolhida (ex.: `1.20.1.json`).
3. Se houver loader (Forge/NeoForge/Fabric), obter o JSON do loader, que **herda** do JSON vanilla
   (`inheritsFrom`) e acrescenta/sobrescreve bibliotecas, `mainClass` e argumentos.
4. Avaliar as **regras** (`rules`) de cada biblioteca e argumento para o SO/arquitetura/features atuais.
5. Baixar **libraries** (JARs) e **natives** (e extrair natives quando a versão exigir).
6. Baixar o **client.jar**.
7. Baixar o **asset index** e os **objetos de asset** (sons, idiomas, ícones...); para versões antigas,
   materializar os assets em layout "legacy/virtual" ou "resources".
8. Baixar o arquivo de **configuração de logging** (log4j2 XML), quando existir.
9. Selecionar/baixar o **Java** exigido (`javaVersion`).
10. Montar o **classpath**, os **argumentos de JVM** e os **argumentos de jogo**, substituindo
    placeholders (`${auth_player_name}`, `${classpath}` etc.).
11. Iniciar o processo `java`/`javaw` com diretório de trabalho = pasta da instância (`gameDir`),
    capturando stdout/stderr.

Fontes: wiki da comunidade sobre o formato (https://minecraft.wiki/w/Client.json,
https://minecraft.wiki/w/Version_manifest.json), código do Modrinth App/theseus
(https://github.com/modrinth/code/tree/main/packages/app-lib/src/launcher), Prism Launcher
(https://github.com/PrismLauncher/PrismLauncher/tree/develop/launcher/minecraft). Dados conferidos
diretamente nos endpoints da Mojang durante esta pesquisa **[verificado]**.

### 1.2 Version manifest (piston-meta)

- Endpoint atual: `https://piston-meta.mojang.com/mc/game/version_manifest_v2.json` **[verificado]**.
  (O antigo `launchermeta.mojang.com/mc/game/version_manifest.json` ainda existe; a v2 acrescenta
  `sha1` e `complianceLevel` por versão.)
- Estrutura:
  ```json
  {
    "latest": { "release": "26.3", "snapshot": "26.4-snapshot-2" },
    "versions": [
      { "id": "1.20.1", "type": "release",
        "url": "https://piston-meta.mojang.com/v1/packages/<sha1>/1.20.1.json",
        "time": "...", "releaseTime": "2023-06-12T13:25:51+00:00",
        "sha1": "c0a00f47b3dae01d83e21be9a646c9232379d9ab", "complianceLevel": 1 }
    ]
  }
  ```
- Em 2026-10-01 o manifesto tinha **917 versões**; `latest.release` = **`26.3`** **[verificado]**. A
  Mojang mudou para numeração por ano (`26.1`, `26.2`, `26.3`...) e snapshots no formato
  `26.4-snapshot-2`. **O Warden não pode assumir o padrão `1.x.y`** ao comparar versões: deve ordenar
  pela posição no manifesto ou por `releaseTime`, nunca por parsing semver de `id`.
- `type` ∈ `release`, `snapshot`, `old_beta`, `old_alpha`.
- `complianceLevel`: 0 para versões antigas, 1 para versões que suportam os recursos de segurança de
  chat/"player safety" mais novos (o launcher oficial usa isso para avisos). Para o Warden é
  irrelevante na prática **[inferência]**.
- O `url` de cada versão contém o SHA-1 do JSON; o launcher deve validar o hash e pode cachear o JSON
  por SHA-1 (conteúdo imutável). Observação: o campo `time` muda quando a Mojang republica o JSON
  (ex.: 1.21.1 republicado em 2026-09-29), então o cache deve ser por `sha1`, não por `id`
  **[verificado]**.

### 1.3 JSON da versão (client.json)

Chaves encontradas **[verificado]** nos JSONs 1.7.10, 1.12.2, 1.16.5, 1.20.1, 1.21.1 e 26.3:

| Chave | Conteúdo |
|---|---|
| `id`, `type`, `time`, `releaseTime` | identificação |
| `mainClass` | classe principal (`net.minecraft.client.main.Main` desde 1.6; `net.minecraft.launchwrapper.Launch` em versões pré-1.6 reempacotadas pela Mojang, ex.: 1.5.2, b1.7.3) |
| `minecraftArguments` | formato **antigo** (string única), até 1.12.2 |
| `arguments` | formato **moderno** (`game`, `jvm` e, em 26.x, `default-user-jvm`), a partir do snapshot 17w43a (1.13); 17w31a ainda usa `minecraftArguments` **[verificado]** |
| `libraries` | lista de bibliotecas com `downloads.artifact`, `downloads.classifiers`, `natives`, `extract`, `rules` |
| `downloads` | `client`, `server`, `client_mappings`, `server_mappings`, `windows_server` (cada um com `sha1`, `size`, `url`) |
| `assetIndex` | `{id, sha1, size, totalSize, url}` |
| `assets` | id do índice de assets (ex.: `legacy`, `pre-1.6`, `1.7.10`, `1.12`, `5`, `17`, `34`) |
| `javaVersion` | `{component, majorVersion}` (ex.: `java-runtime-delta`, 21) |
| `logging` | `client.argument` (`-Dlog4j.configurationFile=${path}`) e `client.file` (XML a baixar) |
| `minimumLauncherVersion` | número de compatibilidade do launcher oficial (13 em 1.7.10, 18 em 1.12.2, 21 em 1.13+) |
| `complianceLevel` | ver acima |
| `inheritsFrom`, `jar` | só aparecem em JSONs de loaders (herança) |

### 1.4 Libraries: download, regras por SO/arquitetura e natives

**Coordenada Maven → caminho.** `group:artifact:version[:classifier][@ext]` vira
`group/com/barras/artifact/version/artifact-version[-classifier].ext` sob `libraries/`. Quando a
biblioteca tem `downloads.artifact.path`/`url`/`sha1`/`size`, usar esses valores. Bibliotecas de
loaders às vezes trazem só `name` + `url` (base de um repositório Maven, ex.: Fabric) e o launcher
deriva o caminho **[verificado]** (perfil Fabric abaixo, seção 3). Também existem entradas com `url`
vazio (`""`), que significam "arquivo gerado localmente pelo instalador, não baixar" — é o caso do
JAR do Forge 1.12.2 dentro do `version.json` do instalador **[verificado]**.

**Regras (`rules`).** Lista avaliada em ordem; a **última regra que casa vence**; sem regras = permitido;
com regras e nenhuma casando = proibido. Cada regra tem `action` (`allow`/`disallow`) e opcionalmente:
- `os.name` ∈ `windows`, `osx`, `linux`;
- `os.arch` (ex.: `x86`) — compara com a arquitetura do **processo Java**/SO;
- `os.version` — **regex** contra a versão do SO (ex.: `"^10\\."` em 1.16.5 para injetar
  `-Dos.name=Windows 10`);
- **novo em 26.x**: `os.versionRange` com `min`/`max` (ex.: `{"min": "10.0.17134"}` para ativar ZGC
  só no Windows 10 1803+) **[verificado]** no JSON da 26.3. Um parser que não reconheça campos
  desconhecidos deve tratar a regra como "não casa" de forma segura e registrar aviso;
- `features` (só em argumentos): `is_demo_user`, `has_custom_resolution`, `has_quick_plays_support`,
  `is_quick_play_singleplayer`, `is_quick_play_multiplayer`, `is_quick_play_realms`.

Exemplos reais **[verificado]**:
- 1.12.2: `lwjgl 2.9.4-nightly` com `[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}}]`
  e a variante `2.9.2-nightly` só para `osx` — ou seja, a mesma biblioteca aparece duas vezes com
  versões diferentes por SO.
- 1.7.10: `tv.twitch:twitch-platform` com `disallow` em Linux e natives `natives-windows-${arch}`
  (`${arch}` → `32`/`64`).

**Natives — três gerações:**

| Geração | Versões | Como aparece no JSON | O que o launcher faz |
|---|---|---|---|
| LWJGL 2 | até 1.12.2 | biblioteca com `natives: {"windows": "natives-windows", ...}` + `extract: {"exclude": ["META-INF/"]}` + `downloads.classifiers` | baixa o JAR do classifier do SO (substituindo `${arch}`), **extrai** para a pasta de natives (excluindo `META-INF/`) e passa `-Djava.library.path=<natives>` |
| LWJGL 3 (transição) | 1.13 a 1.18.2 | mesmo esquema `natives` + `classifiers` (`natives-windows`, `natives-linux`, `natives-macos`) | idem: extrair |
| LWJGL 3 (moderno) | 1.19+ | natives viram **bibliotecas comuns** com classifier no nome (`org.lwjgl:lwjgl:3.3.1:natives-windows`) e `rules` por `os.name` | apenas colocar no classpath; o LWJGL 3 extrai sozinho para `-Dorg.lwjgl.system.SharedLibraryExtractPath` |

Detalhe crítico do formato moderno **[verificado]** (1.20.1): para Windows existem três entradas
(`natives-windows`, `natives-windows-x86`, `natives-windows-arm64`) **todas com a mesma regra**
`os.name = windows`, sem distinção de arquitetura. O launcher oficial coloca as três no classpath e
o LWJGL escolhe a correta em runtime (os JARs têm os binários em caminhos por arquitetura)
**[inferência sobre o launcher oficial, que é fechado]**. Não verifiquei se Prism/Modrinth filtram por
arquitetura. Recomendação para o Warden: incluir todas as natives que passam nas regras (comportamento
mais seguro) ou filtrar por sufixo do classifier com testes — nunca deixar de incluir a nativa do x64.
Transição medida **[verificado]**: 1.13, 1.16.5 e 1.18.2 ainda usam o mapa `natives` (8–16 entradas);
1.19 e 1.19.4 já trazem 42 bibliotecas `:natives-*` e nenhuma com mapa `natives`.

Em 26.3 o JVM args passa a apontar subpastas: `-Djava.library.path=${natives_directory}/java`,
`-Djna.tmpdir=${natives_directory}/jna`, `-Dorg.lwjgl.system.SharedLibraryExtractPath=${natives_directory}/lwjgl`,
`-Dio.netty.native.workdir=${natives_directory}/netty` **[verificado]**. Como os argumentos vêm do
JSON, um launcher que só substitui placeholders já funciona; não há lógica especial a implementar.

**Tamanho do problema** **[verificado]**: 33 bibliotecas em 1.7.10, 39 em 1.12.2, 57 em 1.16.5,
88 em 1.20.1, 97 em 1.21.1 e 114 em 26.3 (antes de filtrar regras).

### 1.5 client.jar

`downloads.client` traz `url`, `sha1`, `size`. O Mojang launcher salva em
`versions/<id>/<id>.jar`. Ele entra no **fim** do classpath no launcher oficial. Para loaders que
herdam (`inheritsFrom`), o client.jar é o do pai, salvo quando o JSON do loader tem `jar` (Forge
1.7.10 usa `"jar": "1.7.10"`) **[verificado]**. No Forge moderno (1.13+), o client.jar vanilla
**não** vai no classpath diretamente: o instalador gera JARs processados (`client-srg`, `client-extra`,
`forge-...-client.jar`) e o `version.json` do Forge os referencia (ver seção 3).

### 1.6 Asset index e objetos

- O `assetIndex.url` aponta para um JSON `{ "objects": { "<caminho lógico>": { "hash": "<sha1>", "size": N } } }`.
- Cada objeto é baixado de `https://resources.download.minecraft.net/<hash[0..2]>/<hash>` e salvo em
  `assets/objects/<hash[0..2]>/<hash>` (armazenamento por conteúdo, compartilhável entre todas as
  instâncias e versões) **[fonte]** https://minecraft.wiki/w/Tutorials/Game_assets.
- Tamanhos reais **[verificado]**: índice `1.7.10` = 686 objetos (~112 MB); `1.12` = 1305 objetos
  (~130 MB); `5` (1.20.1) ~650 MB; `17` (1.21.1) ~825 MB; `34` (26.3) ~484 MB. O download inicial de
  assets é o maior custo de tempo do primeiro "Testar".
- **Índices legados** **[verificado]**:
  - `legacy` (usado por 1.6.x; **não** por 1.7.10): JSON com `"virtual": true`. O launcher deve
    copiar cada objeto para `assets/virtual/legacy/<caminho lógico>` e passar esse diretório como
    `${game_assets}` / `--assetsDir`.
  - `pre-1.6` (usado por versões ≤1.5.2, alpha, beta): JSON com `"map_to_resources": true`. O
    launcher deve copiar os objetos para `<gameDir>/resources/<caminho lógico>` (dentro da instância).
  - `1.7.10` e posteriores: sem flags, o jogo lê direto de `assets/objects` via `--assetsDir` +
    `--assetIndex`.
- Placeholders relacionados: `${assets_root}` (= `assets/`), `${assets_index_name}` (= `assets` do JSON),
  `${game_assets}` (= diretório virtual/legacy nas versões antigas).

> Para o Warden: 1.7.10 já usa assets "modernos" (objetos por hash). O suporte a `virtual`/
> `map_to_resources` só é necessário se o Warden quiser rodar ≤1.6.4 — o requisito "qualquer versão"
> sugere que sim, mas loaders (Forge/Fabric) para essas versões são pouco relevantes. Ver questões
> em aberto.

### 1.7 Logging (log4j2)

- A partir de 1.7 o JSON traz `logging.client` **[verificado]**:
  - 1.7.10: `client-1.7.xml`; 1.12.2 até 1.21.1: `client-1.12.xml`; 26.3: `client-1.21.2.xml`.
  - `argument`: `-Dlog4j.configurationFile=${path}`, onde `${path}` é o caminho local do XML baixado
    (o launcher oficial guarda em `assets/log_configs/`).
- Esses XMLs configuram o log4j para emitir **eventos XML** (`<log4j:Event ...>`) no stdout, que o
  launcher oficial parseia para colorir/filtrar a saída. Também gravam `logs/latest.log` (texto) e
  rotação `logs/<data>-N.log.gz` **[fonte]** https://minecraft.wiki/w/Client.json e conteúdo dos XMLs.
- **Decisão importante — texto vs XML**: sem `-Dlog4j.configurationFile`, o log4j usa o `log4j2.xml`
  embutido no client.jar, que escreve **texto** no stdout (`[HH:mm:ss] [thread/LEVEL]: msg`). Com o XML
  da Mojang, o stdout vira um fluxo de eventos XML estruturados. O Prism atual tem um parser incremental
  que aceita os dois formatos na mesma stream (`LaunchTask::parseXmlLogs`, que transforma eventos
  `log4j:Event` em linhas `[hora] [thread/LEVEL] [logger]: msg` e, para texto puro, adivinha o nível pela
  linha) e aplica `censorPrivateInfo` (substitui token/nome/UUID antes de exibir) **[verificado]**
  (https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/launch/LaunchTask.cpp).
  Recomendação para o Warden: **respeitar o `logging` efetivo do perfil** — passar o XML da Mojang
  quando o perfil o herda (vanilla, Fabric; é a configuração com as correções de Log4Shell) e não passar
  nada quando o perfil o anula (Forge) ou quando o loader tem configuração própria melhor para
  diagnóstico (NeoForge, ver 3.1) — com um parser que aceite XML **e** texto puro na mesma stream (mods,
  o Forge e a própria JVM escrevem direto em `System.out`/`System.err`).
- Contexto de segurança: em dezembro de 2021 (Log4Shell, CVE-2021-44228) a Mojang corrigiu o cliente
  republicando os XMLs de logging (`client-1.7.xml`, `client-1.12.xml`) referenciados nos JSONs de
  versão, e para servidores/terceiros orientou: 1.7–1.11.2 → `-Dlog4j.configurationFile=log4j2_17-111.xml`;
  1.12–1.16.5 → `log4j2_112-116.xml`; 1.17+ → `-Dlog4j2.formatMsgNoLookups=true`; e alertou que
  launchers de terceiros precisavam ser atualizados **[fonte]**
  https://www.minecraft.net/en-us/article/important-message--security-vulnerability-java-edition
  (página oficial; conteúdo confirmado via resumo de busca, a página não carregou diretamente) e
  https://nodecraft.com/blog/service-updates/minecraft-java-edition-security-vulnerability.
  **Regra para o Warden**: nunca lançar uma versão ≤1.18 sem uma configuração log4j segura — o XML do
  JSON da Mojang (vanilla/Fabric) ou a configuração própria do loader quando ele anula o
  `logging` (Forge, cujo `log4j2.xml` usa `%msg{nolookups}`; ver 3.1). Mesmo offline, LAN/servidor local
  e textos vindos de mundos/mods de terceiros podem carregar a string maliciosa **[inferência]**. Ver
  seção 6 para a captura.

### 1.8 Montagem do classpath

Algoritmo (oficial/Prism/theseus convergem) **[fonte + inferência]**:

1. Resolver a cadeia de herança: JSON do loader → `inheritsFrom` → JSON vanilla. Bibliotecas do
   filho vêm **antes** das do pai no classpath (o launcher oficial faz `child.libraries + parent.libraries`).
2. Filtrar por `rules`.
3. **Deduplicar por `group:artifact[:classifier]`**: se o loader declara outra versão de uma
   biblioteca já presente no vanilla (ex.: ASM, Guava, log4j em Forge), só uma pode ficar. Esse é um
   bug clássico de launchers caseiros (duas versões do mesmo JAR no classpath → `NoSuchMethodError`).
   O Prism faz isso em `LaunchProfile::applyLibrary`: procura a biblioteca pelo nome (sem versão) e
   **mantém a de versão mais alta**; natives vão para uma lista separada **[verificado]**
   (https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/minecraft/LaunchProfile.cpp).
   Recomendação: mesma regra (maior versão vence), com log explícito de cada substituição.
4. Excluir bibliotecas que são só natives a extrair (LWJGL 2) — elas não entram no classpath; vão
   para a pasta de natives.
5. Adicionar o client.jar (ou o JAR indicado por `jar`) por último — **exceto** no Forge/NeoForge
   moderno, em que o JSON do loader define o que deve entrar.
6. Unir com o separador do SO (`;` no Windows, `:` no Linux/macOS) — `${classpath_separator}`.

**Windows: limite de linha de comando.** O `CreateProcess` do Windows limita a linha de comando a
32 767 caracteres. Com 100+ bibliotecas em caminhos longos (ex.: `C:\Users\<nome>\AppData\Roaming\...`)
e o `-p` (module path) do Forge moderno, isso pode estourar. Mitigações usadas na prática: caminhos
curtos para a pasta de bibliotecas, ou passar argumentos via **arquivo de argumentos do Java**
(`java @argfile`, suportado desde Java 9 — não existe no Java 8) **[fonte]**
https://docs.oracle.com/en/java/javase/17/docs/specs/man/java.html#java-command-line-argument-files
e https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessa.
Para Java 8 (1.7.10/1.12.2) o classpath é menor e o problema raramente ocorre **[inferência]**.

### 1.9 Argumentos de JVM e de jogo

**Formato antigo — `minecraftArguments` (até 1.12.2)** **[verificado]**:

```
1.7.10:  --username ${auth_player_name} --version ${version_name} --gameDir ${game_directory}
         --assetsDir ${assets_root} --assetIndex ${assets_index_name} --uuid ${auth_uuid}
         --accessToken ${auth_access_token} --userProperties ${user_properties} --userType ${user_type}
1.12.2:  ... --userType ${user_type} --versionType ${version_type}     (sem --userProperties)
1.6.4:   --username ${auth_player_name} --session ${auth_session} --version ${version_name}
         --gameDir ${game_directory} --assetsDir ${game_assets}
≤1.5.2:  ${auth_player_name} ${auth_session} --gameDir ${game_directory} --assetsDir ${game_assets}
```

Nesse formato **não há argumentos de JVM no JSON**: o launcher precisa fornecer por conta própria
`-Djava.library.path=<natives>`, `-cp <classpath>`, memória (`-Xmx/-Xms`) e, por convenção do launcher
oficial, `-Dminecraft.launcher.brand`/`version`. A string é dividida por espaços **antes** da
substituição dos placeholders (para que um caminho com espaço vire um único argumento).

**Formato moderno — `arguments` (1.13+)** **[verificado]**: arrays `game` e `jvm` cujos elementos são
strings ou objetos `{ "rules": [...], "value": "x" | ["x","y"] }`. JVM de 1.20.1/1.21.1:

```
-XstartOnFirstThread                         (só osx)
-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump  (só windows)
-Xss1M                                       (só arch x86)
-Djava.library.path=${natives_directory}
-Djna.tmpdir=${natives_directory}
-Dorg.lwjgl.system.SharedLibraryExtractPath=${natives_directory}
-Dio.netty.native.workdir=${natives_directory}
-Dminecraft.launcher.brand=${launcher_name}
-Dminecraft.launcher.version=${launcher_version}
-cp ${classpath}
```

O curioso `-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump`
existe para que drivers Intel antigos no Windows reconheçam o processo como Minecraft e ativem
otimizações/correções; **deve ser mantido** (vem do JSON).

**Novidade 26.x** **[verificado]**: chave `arguments.default-user-jvm`, com flags de memória/GC
sugeridas pela Mojang (`-Xms2G -Xmx4G -XX:+UseCompactObjectHeaders -XX:+AlwaysPreTouch
-XX:+UseStringDeduplication`, `-XX:+UseZGC` condicionado a Windows ≥ 10.0.17134, G1GC com tuning
caso contrário). São "padrões do usuário" — o launcher deve aplicá-los **somente se o usuário não
definiu memória/GC próprios**. Também em 26.3: `-XX:StackShadowPages=32`,
`--enable-native-access=ALL-UNNAMED`, `--add-exports java.base/jdk.internal.misc=ALL-UNNAMED`. Já o
argumento de jogo `--userType ${user_type}` **deixou de existir a partir da 1.21.9** (presente na 1.21.8,
ausente na 1.21.9 e em 26.x; continuam `--uuid`/`--accessToken`/`--clientId`/`--xuid`) **[verificado]**.

Ordem final da linha de comando (convenção oficial):
```
<java> <jvm args do usuário (memória etc.)> <jvm args do JSON (ou padrões, no formato antigo)>
       <-Dlog4j.configurationFile=...> <mainClass> <game args> <args extras do launcher>
```

**Placeholders** que o Warden precisa suportar (união dos vistos nos JSONs vanilla e de loaders)
**[verificado]**:

| Placeholder | Valor |
|---|---|
| `${auth_player_name}` | nome do jogador |
| `${auth_uuid}` | UUID sem hífens (ou com, ambos aceitos) — ver seção 4 |
| `${auth_access_token}` | token (offline: valor fictício não vazio) |
| `${auth_session}` | formato antigo: `token:<accessToken>:<uuid>` (≤1.6) |
| `${auth_xuid}`, `${clientid}` | Xbox user id e client id (offline: `0`/vazio-seguro) |
| `${user_type}` | `msa` / `mojang` / `legacy` (offline: ver seção 4) |
| `${user_properties}` | JSON de propriedades (`{}` em offline) — obrigatório em 1.7.x, senão crash |
| `${version_name}` | id da versão (ou id do perfil do loader) |
| `${version_type}` | `release`/`snapshot`; launchers costumam colocar o nome do launcher |
| `${game_directory}` | pasta da instância |
| `${assets_root}`, `${assets_index_name}`, `${game_assets}` | ver 1.6 |
| `${natives_directory}` | pasta de natives extraídas |
| `${launcher_name}`, `${launcher_version}` | "Warden" e versão |
| `${classpath}`, `${classpath_separator}`, `${library_directory}` | últimos dois usados pelo Forge/NeoForge modernos |
| `${resolution_width}`, `${resolution_height}` | se `has_custom_resolution` |
| `${quickPlayPath}`, `${quickPlaySingleplayer}`, `${quickPlayMultiplayer}`, `${quickPlayRealms}` | Quick Play (1.20+) — útil para o Warden abrir direto um mundo de teste |
| `${path}` | caminho do XML de logging |

> Quick Play (`--quickPlaySingleplayer <nome do mundo>`, disponível a partir de 1.20 / 23w14a) permite
> ao Warden oferecer "Testar e entrar direto no mundo X" sem cliques. Em versões anteriores não há
> equivalente vanilla **[fonte]** https://minecraft.wiki/w/Quick_Play.

### 1.10 Diferenças entre faixas de versão (resumo)

| Faixa | mainClass vanilla | Args | Natives | Assets | Java (oficial) | Observações |
|---|---|---|---|---|---|---|
| ≤1.5.2 / beta / alpha | `net.minecraft.launchwrapper.Launch` (reempacotado) | `minecraftArguments` posicional | LWJGL 2, extrair | `pre-1.6` → `resources/` | 8 (`jre-legacy`) | applet antigo encapsulado pela Mojang |
| 1.6.x | `net.minecraft.client.main.Main` | `--session` | LWJGL 2, extrair | `legacy` virtual | sem `javaVersion` no JSON → assumir 8 | |
| 1.7.2–1.7.10 | `...client.main.Main` | `minecraftArguments`, exige `--userProperties` | LWJGL 2, extrair | objetos por hash | 8 | Forge via launchwrapper/FML |
| 1.8–1.12.2 | `...client.main.Main` | `minecraftArguments` | LWJGL 2, extrair | objetos por hash | 8 | Forge via launchwrapper; 1.12.2 tem lwjgl diferente no macOS |
| 1.13–1.16.5 | `...client.main.Main` | `arguments` com regras | LWJGL 3, extrair | objetos | 8 | Forge com instalador "processors" (spec 0/1) e ModLauncher |
| 1.17–1.17.1 | idem | idem | LWJGL 3 | objetos | 16 (`java-runtime-alpha`) | |
| 1.18–1.20.4 | idem | idem (+quickPlay em 1.20) | 1.18.x: extrair; 1.19+: natives como libs no classpath | objetos | 17 (`beta`/`gamma`) | |
| 1.20.5–1.21.x | idem | idem | idem | objetos | 21 (`java-runtime-delta`) | |
| 26.1+ | idem | idem + `default-user-jvm`, sem `--userType` (desde 1.21.9), `os.versionRange` | idem, natives em subpastas | objetos | 25 (`java-runtime-epsilon`) | numeração por ano |

Tabela derivada da varredura de todos os JSONs de release do manifesto **[verificado]** (ver 2.1).

---

## 2. Java: versões exigidas e gestão automática de runtimes

### 2.1 Qual Java cada faixa do Minecraft exige

Varredura do campo `javaVersion` de **todas** as versões `release` do manifesto (2026-10-01)
**[verificado]**:

| Primeira release | `javaVersion.component` | Java major | Válido até |
|---|---|---|---|
| 1.0 … 1.5.2 | `jre-legacy` | 8 | — |
| 1.6.1 … 1.6.4 | *(ausente no JSON)* | assumir 8 | — |
| 1.7.2 … 1.16.5 | `jre-legacy` | 8 | 1.16.5 |
| 1.17 / 1.17.1 | `java-runtime-alpha` | 16 | 1.17.1 |
| 1.18 … 1.18.2 | `java-runtime-beta` | 17 | 1.18.2 |
| 1.19 … 1.20.4 | `java-runtime-gamma` | 17 | 1.20.4 |
| 1.20.5 … 1.21.x | `java-runtime-delta` | 21 | 1.21.x |
| 26.1 … 26.3 | `java-runtime-epsilon` | 25 | atual |

O meta do Prism declara `compatibleJavaMajors` por versão **[verificado]**
(https://meta.prismlauncher.org/v1/net.minecraft/1.17.1.json etc.): 1.7.10/1.12.2/1.16.5 → `[8]`;
1.17.1 → `[16, 17]`; 1.20.1 → `[17]`; 1.21.1 → `[21]`. Tabela de referência do Prism:
1.20.5+ → 21; 1.17–1.20.4 → 17; ≤1.16 → 8 **[fonte]** https://prismlauncher.org/wiki/getting-started/installing-java/.

**Regras práticas que o vanilla não diz (vêm dos loaders/mods)**:

- **Forge 1.7.10 e 1.12.2 exigem Java 8 exatamente.** O launchwrapper faz cast do class loader do
  sistema para `URLClassLoader`, o que falha a partir do Java 9 com
  `ClassCastException: ... AppClassLoader cannot be cast to ... URLClassLoader`. Não é "recomendado",
  é obrigatório **[fonte]** https://github.com/SKCraft/Launcher/issues/284 e
  https://github.com/MultiMC/Launcher/issues/5346 ("Pre-1.13 Forge only works with Java 8"). Exceção:
  projetos da comunidade como **lwjgl3ify** (GT New Horizons) e **Cleanroom** (1.12.2) permitem
  rodar 1.7.10/1.12.2 em Java 17+/21+, mas exigem perfil de lançamento próprio — tratar como recurso
  avançado e futuro.
- **Forge 1.16.5 antigo quebra em Java 8u321+** (`NoSuchMethodError: sun.security.util.ManifestEntryVerifier`
  no modlauncher); corrigido a partir do Forge 36.2.26 **[fonte]**
  https://prismlauncher.org/wiki/getting-started/installing-java/ e
  https://github.com/TeamMoegMC/TheWinterRescue/issues/44. O Warden deve, para Forge 1.16.5 < 36.2.26,
  alertar (diagnóstico determinístico) ou preferir um Java 8 ≤ 8u312.
- **Java 16 está fora de suporte e não é publicado pelo Adoptium** (consulta à API do Adoptium para
  `16` não retornou binários) **[verificado]**. Para 1.17.x, usar 17 (aceito pelo jogo e pelo
  Fabric/Forge 1.17) — é o que o Prism faz (`[16, 17]`).
- **Fabric Loader**: o perfil declara `launcherMeta.min_java_version: 8` **[verificado]** — o loader
  roda em 8, mas a versão do MC manda.
- **Mods podem exigir Java mais novo que o MC** (ex.: mods 1.20.1 compilados para 21) — erro
  `UnsupportedClassVersionError` (seção 6). Detecção determinística possível lendo o *major version*
  dos `.class` dentro dos JARs dos mods antes de lançar **[inferência]**.
- **Java 8 em 32 bits**: as natives de 1.7.10 têm variantes `natives-windows-32/64` (`${arch}`); o
  Warden pode **exigir JVM 64 bits** e simplificar (Windows x86 é irrelevante em 2026)
  **[inferência]**.

### 2.2 Runtime oficial da Mojang (`java-runtime` manifest)

- Índice: `https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json`
  **[verificado]**. Plataformas: `gamecore`, `linux`, `linux-i386`, `mac-os`, `mac-os-arm64`,
  `windows-arm64`, `windows-x64`, `windows-x86`. Componentes: `jre-legacy`, `java-runtime-alpha`,
  `java-runtime-beta`, `java-runtime-gamma`, `java-runtime-gamma-snapshot`, `java-runtime-delta`,
  `java-runtime-epsilon`, `minecraft-java-exe`.
- Cada entrada: `{ availability: {group, progress}, manifest: {sha1, size, url}, version: {name, released} }`.
  Versões atuais em `windows-x64` **[verificado]**: `jre-legacy` = **8u51** (`8u51-cacert462b08`,
  republicado em 2025-10-06 só com cacerts atualizados), `alpha` = 16.0.1, `beta`/`gamma` = 17.0.15,
  `delta` = 21.0.7, `epsilon` = 25.0.1. No Linux, `jre-legacy` = 8u202.
- O manifesto do componente lista **cada arquivo** (`files: { "bin/javaw.exe": { type: "file",
  executable: true, downloads: { raw: {sha1,size,url}, lzma: {sha1,size,url} } } }`, além de
  `type: "directory"` e `type: "link"` com `target`). Ex.: `java-runtime-delta` Windows = 484 entradas
  (402 arquivos, 82 diretórios), ~99 MB descompactado **[verificado]**. Não há ZIP: o launcher baixa
  arquivo a arquivo (preferindo `lzma` e descompactando), valida SHA-1 e marca executáveis.
- **Fornecedor/licença**: o arquivo `release` do `jre-legacy` Windows indica
  `JAVA_VERSION="1.8.0_51"`, `BUILD_TYPE="commercial"` — é um **JRE Oracle 8u51** (de 2015)
  **[verificado]**. Os runtimes 17/21/25 são builds OpenJDK (pasta `legal/` com GPLv2+CPE)
  **[verificado]**; a comunidade identifica-os como Microsoft Build of OpenJDK **[não confirmado]**.
- Implicações:
  - 8u51 é antigo: sem TLS moderno completo, sem correções de segurança de 10 anos. Para 1.7.10/1.12.2
    com Forge **um Java 8 atualizado (Temurin 8u4xx) é preferível**; só o Forge 1.16.5 < 36.2.26 exige
    o contrário (≤ 8u312).
  - Usar o runtime da Mojang é "o que o launcher oficial faz", e os binários são servidos pela Mojang
    para launchers; não há redistribuição pelo Warden. Ainda assim é dependência de um endpoint não
    documentado oficialmente **[inferência]**.

### 2.3 Adoptium/Temurin (e alternativas)

- API: `https://api.adoptium.net/v3/assets/latest/{major}/hotspot?os=windows&architecture=x64&image_type=jre`
  **[verificado]**. Retorna `binary.package.{name, link, size, checksum (sha256)}`. Em 2026-10-01:
  8u504, 17.0.20.1, 21.0.12.1, 25.0.4.1 (JRE Windows x64 em ZIP, 40–58 MB). LTS disponíveis:
  8, 11, 17, 21, 25; não há 16 **[verificado]**.
- Licença: GPLv2 + Classpath Exception; distribuição e download livres
  (https://adoptium.net/docs/faq/). JRE (não JDK) basta para o jogo; JDK só seria necessário se o
  Warden fosse compilar algo (não é o caso).
- Alternativas: Azul Zulu (tem Java 8 para macOS ARM e builds 16 arquivados), Microsoft Build of
  OpenJDK (sem Java 8). O Prism expõe as três fontes no seu meta: `net.minecraft.java` (Mojang),
  `net.adoptium.java`, `com.azul.java` (e `com.ibm.java`) **[verificado]** em
  https://meta.prismlauncher.org/v1/index.json.

### 2.4 Como outros launchers gerenciam Java

- **Launcher oficial**: lê `javaVersion.component`, baixa do manifesto `java-runtime` para
  `runtime/<component>/<plataforma>/<component>/` e usa `bin/javaw.exe`.
- **Prism Launcher (9.0+)**: opções "Autodetect Java version" e "Auto-download Mojang Java"
  **[fonte]** https://prismlauncher.org/wiki/getting-started/installing-java/. O meta do Prism
  (`generate_java.py`) agrega Mojang java-runtime, Adoptium, Azul e IBM, preferindo fornecedores não-Azul,
  e publica `net.minecraft.java/java21.json` etc. com `runtimes[]` (`vendor`, `runtimeOS`, `downloadType:
  "manifest"`, `url`). No cliente, `AutoInstallJava` lê `compatibleJavaName`/`compatibleJavaMajors` do
  perfil e tenta cada major em ordem; `VerifyJavaInstall` bloqueia o lançamento se o major não for
  compatível (a menos que o usuário marque "ignorar") e avisa sobre Java 32 bits com mais de 2048 MiB
  **[verificado]** (https://github.com/PrismLauncher/meta/blob/main/meta/run/generate_java.py,
  https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/minecraft/launch/AutoInstallJava.cpp,
  `launcher/minecraft/launch/VerifyJavaInstall.cpp`).
- **Modrinth App**: baixa **Azul Zulu JRE** via `https://api.azul.com/metadata/v1/zulu/packages?arch=...&java_version=...&os=...&archive_type=zip&java_package_type=jre`
  (`packages/app-lib/src/api/jre.rs`, `auto_install_java_inner`); não usa Adoptium nem o runtime da
  Mojang **[verificado]** (https://github.com/modrinth/code/blob/main/packages/app-lib/src/api/jre.rs).
- **HMCL**: runtimes da Mojang (`MojangJavaDownloadTask`) **e** a foojay Disco API
  (`https://api.foojay.io/disco/v3.0`: Temurin, Liberica, Zulu, GraalVM, Semeru, Corretto) **[verificado]**
  (https://github.com/HMCL-dev/HMCL/blob/main/HMCLCore/src/main/java/org/jackhuang/hmcl/download/java/disco/DiscoJavaDistribution.java).
- **ATLauncher**: manifesto `java-runtime` da Mojang **[verificado]**
  (https://github.com/ATLauncher/ATLauncher/blob/master/src/main/java/com/atlauncher/constants/Constants.java).

### 2.5 Recomendação para o Warden (Java)

1. Fonte padrão: **Adoptium Temurin JRE** por major (8, 17, 21, 25), validado por SHA-256 da API.
   Motivos: Java 8 atualizado, ZIP único (instalação mais simples e atômica), licença clara.
2. Fallback/alternativa: **runtime oficial da Mojang** via `javaVersion.component` (útil se Adoptium
   estiver fora do ar e para reproduzir exatamente o ambiente oficial). Para Java 16, mapear para 17.
3. Regras de seleção (ordem): (a) override manual do usuário por instância; (b) regra específica de
   loader (Forge 1.7.10/1.12.2 → 8; Forge 1.16.5 < 36.2.26 → 8 ≤ u312); (c) `javaVersion.majorVersion`
   do JSON (16 → 17); (d) ausência de `javaVersion` → 8.
4. Validar qualquer Java (gerenciado ou do sistema) executando-o uma vez (`java -XshowSettings:properties -version`
   imprime em stderr `java.version`, `os.arch`, `java.vendor`) e cachear o resultado por caminho + mtime.
5. Gerenciar runtimes numa pasta compartilhada do app (ex.: `%APPDATA%\Warden\runtimes\temurin-21.0.12.1-x64\`),
   instalação atômica (baixar → verificar → extrair em pasta temporária → renomear).
6. Usar `javaw.exe` no Windows (sem janela de console) — stdout/stderr continuam capturáveis por pipes
   (ver seção 6).

---

## 3. Loaders: Fabric, Quilt, Forge e NeoForge

### 3.1 Modelo comum: "perfil herdado"

Todos os loaders produzem, de um jeito ou de outro, um **JSON de versão no mesmo formato da Mojang**
com `inheritsFrom: "<versão vanilla>"`, que (a) adiciona bibliotecas, (b) troca a `mainClass`,
(c) acrescenta argumentos de JVM/jogo (ou, no formato antigo, **substitui** `minecraftArguments`
inteiro). O merge que o launcher deve fazer **[fonte: comportamento do launcher oficial; Prism/theseus
implementam o mesmo]**:

| Campo | Regra de merge |
|---|---|
| `libraries` | filho antes do pai; deduplicar por `group:artifact[:classifier]` (o Prism mantém a de versão mais alta — ver 1.8) |
| `mainClass` | filho sobrescreve |
| `minecraftArguments` | filho **substitui** (string completa) |
| `arguments.game` / `arguments.jvm` | pai + filho (concatenação) |
| `assetIndex`, `assets`, `downloads`, `javaVersion` | herdados do pai se o filho não define |
| `logging` | se o filho **define** a chave, ela vale — inclusive vazia. Os perfis do Forge (1.12.2, 1.16.5, 1.20.1) trazem `"logging": {}` de propósito: o Forge usa o **seu próprio** `log4j2.xml` embutido no JAR (com `%msg{nolookups}`, imune a Log4Shell, e que gera `debug.log`), então o launcher **não** deve passar o XML da Mojang. O NeoForge não define `logging` (herdaria o da Mojang) **[verificado]**, mas o FML atual (`net.neoforged.fml.startup.Entrypoint`) só usa o **seu** `log4j2.xml` (que gera `debug.log`) quando o launcher **não** passa `-Dlog4j.configurationFile`/`log4j2.configurationFile`; se passar, o FML respeita o do launcher **[verificado]** (https://github.com/neoforged/FancyModLoader/blob/main/loader/src/main/java/net/neoforged/fml/startup/Entrypoint.java). Como o NeoForge só existe para 1.20.1+ (log4j já corrigido), a recomendação é **não** passar o XML da Mojang para NeoForge e ficar com o `debug.log` **[inferência]** |
| `jar` | se presente, qual client.jar usar (Forge 1.7.10: `"jar": "1.7.10"`) |

### 3.2 Fabric (meta.fabricmc.net)

- API v2 **[verificado]**:
  - `GET https://meta.fabricmc.net/v2/versions/game` — versões do MC suportadas (529 entradas;
    **não inclui 1.7.10 nem 1.12.2**: o Fabric oficial começa nos snapshots da 1.14).
  - `GET https://meta.fabricmc.net/v2/versions/loader` — versões do loader (`0.19.5` estável em 2026-10-01).
  - `GET https://meta.fabricmc.net/v2/versions/loader/{mc}` — combinações loader × intermediary.
  - `GET https://meta.fabricmc.net/v2/versions/loader/{mc}/{loader}/profile/json` — **perfil pronto no
    formato Mojang**. Para 1.20.1 + 0.19.5: `id: fabric-loader-0.19.5-1.20.1`, `inheritsFrom: 1.20.1`,
    `mainClass: net.fabricmc.loader.impl.launch.knot.KnotClient`, `arguments.jvm: ["-DFabricMcEmu= net.minecraft.client.main.Main "]`,
    bibliotecas: ASM 9.x, `sponge-mixin`, `net.fabricmc:intermediary:<mc>`, `net.fabricmc:fabric-loader:<v>`,
    cada uma com `url` do Maven (`https://maven.fabricmc.net/`) e hashes `md5/sha1/sha256/sha512`
    (exceto intermediary/loader, só `name` + `url`).
  - Para 1.12.2 a rota de loader retorna **HTTP 400** **[verificado]**.
- Instalação = baixar o perfil + bibliotecas. **Não há processors nem instalador a executar**. É o
  loader mais simples de suportar e deve ser o primeiro a ser implementado.
- `mainClass` mudou historicamente (`net.fabricmc.loader.launch.knot.KnotClient` em loaders < 0.12;
  `...impl.launch.knot.KnotClient` depois) — irrelevante se o Warden usa o perfil do meta.
- **Fabric para 1.7.10/1.12.2 = Legacy Fabric** (projeto separado, MC 1.3–1.13.2, meta próprio
  `meta.legacyfabric.net` no mesmo formato v2 e Legacy Fabric API) **[fonte]**
  https://ftb.fandom.com/wiki/Legacy_Fabric, https://modrinth.com/mod/legacy-fabric-api. O endpoint não
  respondeu durante esta pesquisa (timeout) **[não verificado]**. Babric/Ornithe cobrem beta/outras
  faixas. Como o requisito do Warden é "Fabric em qualquer versão", 1.7.10/1.12.2 + Fabric implica
  **suportar Legacy Fabric como um "sabor" do loader Fabric** — e verificar se o packwiz aceita isso
  (ver questões em aberto).

### 3.3 Quilt (nota)

Meta em `https://meta.quiltmc.org/v3/versions/loader` (loader `0.31.0-beta.4` no topo da lista em
2026-10-01) **[verificado]**, mesmo modelo de perfil herdado do Fabric (`.../profile/json`). Fora do
escopo inicial (requisito cita só Fabric/NeoForge/Forge); se for adicionado, reaproveita 100% do
código do Fabric.

### 3.4 Forge

Forge **não tem API de perfil**: a fonte oficial é o **instalador** (`forge-<mc>-<ver>-installer.jar`)
no Maven `https://maven.minecraftforge.net/net/minecraftforge/forge/`. Versões promovidas:
`https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json` (ex.: `1.12.2-recommended: 14.23.5.2859`,
`1.20.1-recommended: 47.4.10`) **[verificado]**. Lista completa: `maven-metadata.xml` do artefato.

O instalador traz `install_profile.json` (+ `version.json` nos formatos novos). Existem **três
gerações** de formato **[verificado baixando os instaladores]**:

**(a) Legado — `install` + `versionInfo`** (todo o 1.7.10; 1.12.2 até 14.23.5.2847; demais ≤1.12):
- `versionInfo` já é o JSON de versão: `mainClass: net.minecraft.launchwrapper.Launch`,
  `minecraftArguments` com `--tweakClass cpw.mods.fml.common.launcher.FMLTweaker` (1.7.10) ou
  `--tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker` (1.12.2), `inheritsFrom`, `jar`.
- `libraries` com campos legados `serverreq`/`clientreq` e `checksums` (lista de SHA-1). Bibliotecas
  sem `url` vêm de `https://libraries.minecraft.net/`; com `url`, desse Maven.
- **Armadilha 1**: a entrada do próprio Forge (`net.minecraftforge:forge:1.7.10-10.13.4.1614-1.7.10`)
  aponta para um caminho **que não existe** no Maven (404); o arquivo real é o classifier
  `-universal` (200) **[verificado]**. O instalador o extrai de dentro de si (`install.filePath`).
  O Prism corrige isso no seu meta (`...:1.7.10-10.13.4.1614-1.7.10:universal`) **[verificado]**.
- **Armadilha 2**: historicamente as libs Scala/Typesafe do 1.7.10 eram servidas como `.jar.pack.xz`
  (Pack200); hoje o Maven do Forge serve o `.jar` simples (200) e o `.pack.xz` dá 404 **[verificado]**.
  Ou seja, não é mais preciso implementar Pack200, mas o launcher deve tolerar bibliotecas legadas
  sem hash e validar contra `checksums` quando houver.
- Não há processors: instalar = baixar libs + extrair/baixar o universal jar.

**(b) Instalador "v2", `spec: 0`** (1.12.2 de 14.23.5.2851 em diante; 1.13–1.16.5):
- `install_profile.json` com `data`, `processors`, `libraries`; `version.json` separado com o perfil.
- 1.12.2 (2860): `processors: []`, `data: {}` — só copia o JAR do Forge que vem **dentro** do instalador
  (`maven/net/minecraftforge/forge/.../forge-....jar`; no `version.json` a entrada tem `"url": ""`)
  **[verificado]**. Continua launchwrapper + `FMLTweaker`.
- 1.16.5 (36.2.34): 4 processors (`installertools`, `jarsplitter`, `SpecialSource`, `binarypatcher`);
  `mainClass: cpw.mods.modlauncher.Launcher`; game args `--launchTarget fmlclient --fml.forgeVersion ...
  --fml.mcVersion ... --fml.forgeGroup ... --fml.mcpVersion ...`; JVM `-XX:+IgnoreUnrecognizedVMOptions
  --add-exports=...`; a lib do Forge também tem `"url": ""` (vem do instalador) **[verificado]**.

**(c) `spec: 1`** (1.17+ até hoje; NeoForge também):
- Ex. Forge 1.20.1-47.4.10 **[verificado]**: 10 processors, dos quais 7 se aplicam ao client —
  `installertools` (`EXTRACT_FILES` e 2× `BUNDLER_EXTRACT`, só server; `MCP_DATA`, `DOWNLOAD_MOJMAPS`,
  `MERGE_MAPPING`), `jarsplitter` (uma entrada client, outra server), `ForgeAutoRenamingTool`,
  `binarypatcher` (aplica `data/client.lzma` ao jar renomeado). Processors têm `sides`
  (`client`/`server`); sem `sides` = ambos.
- `mainClass: cpw.mods.bootstraplauncher.BootstrapLauncher`; JVM args com **module path**
  (`-p ${library_directory}/cpw/mods/bootstraplauncher/...${classpath_separator}...`),
  `--add-modules ALL-MODULE-PATH`, `--add-opens/--add-exports`, `-DignoreList=...`,
  `-DlibraryDirectory=${library_directory}`; game args `--launchTarget forgeclient --fml.forgeVersion
  47.4.10 --fml.mcVersion 1.20.1 --fml.forgeGroup net.minecraftforge --fml.mcpVersion 20230612.114412`.

**Como executar os processors** (algoritmo do instalador oficial, reproduzido por Prism/ForgeWrapper,
theseus, HMCL) **[fonte: código do installer
https://github.com/MinecraftForge/Installer/blob/main/src/main/java/net/minecraftforge/installer/actions/PostProcessors.java]**:
1. Baixar todas as `libraries` do `install_profile.json` (ferramentas) e do `version.json`.
2. Montar o mapa de variáveis: as entradas de `data` (lado `client`) + `SIDE=client`,
   `MINECRAFT_JAR=<client.jar vanilla>`, `MINECRAFT_VERSION`, `ROOT=<pasta de instalação>`,
   `INSTALLER=<caminho do installer.jar>`, `LIBRARY_DIR=<libraries>`.
3. Valores de `data`: `[g:a:v:c@ext]` = caminho de artefato em `libraries/`; `'literal'` = string literal;
   `/data/client.lzma` = arquivo **dentro do installer** a extrair para um temporário.
4. Para cada processor aplicável ao lado client: classpath = `jar` + `classpath`; a `Main-Class` vem do
   `META-INF/MANIFEST.MF` do `jar`; substituir `{VAR}` e `[artefato]` em `args`; executar
   `java -cp <cp> <Main-Class> <args>` (com o **mesmo Java** usado no jogo ou qualquer ≥ requisito das
   ferramentas — Java 8 para spec 0; ≥17/21 para versões modernas).
5. Se o processor declara `outputs` (`{arquivo: sha1}`), conferir hashes; se já existem e batem,
   **pular** (instalação idempotente). Falha de hash = instalação corrompida.

Requisitos de rede dos processors: `DOWNLOAD_MOJMAPS` baixa os mappings oficiais da Mojang
(`client_mappings` do JSON da versão) — ou seja, a instalação do Forge moderno **precisa de internet
na primeira vez**, mesmo que o jogo depois rode offline.

Custo: a execução dos processors leva de dezenas de segundos a alguns minutos (renomeação e patch do
client.jar inteiro). Deve ser feita **uma vez por (MC, versão do Forge)** e cacheada na pasta compartilhada
de bibliotecas, não por instância **[inferência, alinhado ao que Prism/theseus fazem]**.

**Aviso ético do Forge** **[verificado]**: o `install_profile.json` contém `"_comment_": ["Please do not
automate the download and installation of Forge.", "Our efforts are supported by ads from the download
page.", "If you MUST automate this, please consider supporting the project through
https://www.patreon.com/LexManos/"]`. Todos os launchers grandes automatizam mesmo assim (ver 3.6).
Postura recomendada para o Warden: automatizar (sem isso o requisito é inviável), **creditar o Forge
na UI**, linkar a página de doação/Patreon e baixar sempre do Maven oficial (não espelhar instaladores).

### 3.5 NeoForge

- Maven: `https://maven.neoforged.net/releases/`. Duas coordenadas **[verificado]**:
  - `net.neoforged:forge` — só para **1.20.1** (fork inicial; versões `1.20.1-47.1.x`).
  - `net.neoforged:neoforge` — 1.20.2+ com versionamento próprio: `<mc minor>.<mc patch>.<build>`
    (ex.: `21.1.252` = MC 1.21.1; `21.11.45` = MC 1.21.11) e, para a numeração por ano,
    `26.3.0.x-beta` = MC 26.3. Sufixo `-beta` indica instável.
- API de versões **[verificado]**: `https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge`
  (1766 versões) e `.../api/maven/latest/version/releases/net/neoforged/neoforge?filter=21.1.` (atenção:
  `filter=21.1` sem ponto final também casa `21.11.x` — filtrar com o ponto) **[verificado]**.
- O instalador usa o mesmo `install_profile.json` `spec: 1` com processors (NeoForge mantém fork próprio
  do installer e das `installertools`) **[verificado]**:
  - 1.20.1 (`forge 47.1.106`) e 1.21.1 (`21.1.252`): 10 processors; `mainClass:
    cpw.mods.bootstraplauncher.BootstrapLauncher`; module path com BSL/securejarhandler; game args
    `--fml.neoForgeVersion ... --fml.fmlVersion ... --fml.mcVersion ... --fml.neoFormVersion ...
    --launchTarget forgeclient`.
  - 1.21.11 (`21.11.45`): **apenas 3 processors** (`EXTRACT_FILES` só server, `DOWNLOAD_MOJMAPS`,
    `PROCESS_MINECRAFT_JAR` do `installertools:4.0.6:fatjar`) e **nova `mainClass`
    `net.neoforged.fml.startup.Client`, sem BootstrapLauncher/module path** — JVM args só
    `-Djava.net.preferIPv6Addresses=system -DlibraryDirectory=... --add-opens ... --add-exports ...`
    **[verificado]**. Ou seja, o NeoForge simplificou muito o lançamento nas versões recentes; um
    executor genérico de processors cobre todas as gerações.
- Licença: o repositório `neoforged/NeoForge` é **LGPL-2.1** **[verificado via API do GitHub]**, e os
  `install_profile.json` do NeoForge (1.20.1, 1.21.1, 1.21.11) **não** têm o comentário "please do not
  automate" do Forge **[verificado]**.

### 3.5.1 Alternativa: rodar o instalador oficial em modo headless

O instalador do Forge (branch atual, `SimpleInstaller.java`) aceita `--installClient <dir>`,
`--installServer <dir>`, `--extract`, `--offline`, `--mirror <url>` **[verificado]**
(https://github.com/MinecraftForge/Installer/blob/main/src/main/java/net/minecraftforge/installer/SimpleInstaller.java).
Estratégia possível: executar `java -jar <installer> --installClient <pasta-temporária>` (a pasta precisa
de um `launcher_profiles.json` mínimo, que o instalador client exige **[fonte: comportamento conhecido do
instalador; a confirmar por versão]**) e depois ler `versions/<id>/<id>.json` + `libraries/` gerados.
Vantagem: fidelidade total ao instalador oficial. Desvantagens: instaladores antigos (1.7.10/1.12.2
legados) não têm `--installClient`; depende de Java para instalar; menos controle de progresso/erros.
**Recomendação [inferência]**: implementar o executor de processors próprio (seção 3.4) e manter o modo
headless como ferramenta de verificação nos testes de integração (comparar saídas).

### 3.6 Como os launchers existentes resolvem os loaders

| Launcher | Licença | Fonte de metadados | Forge moderno (processors) | Forge legado (1.7.10/1.12.2) | Java |
|---|---|---|---|---|---|
| **Prism Launcher** | GPL-3.0-only (partes Apache-2.0 do MultiMC) | `meta.prismlauncher.org/v1` (gerado pelo repo `PrismLauncher/meta`, MS-PL) com componentes `net.minecraft`, `net.minecraftforge`, `net.neoforged`, `net.fabricmc.*`, `org.lwjgl`, `org.lwjgl3`, `org.quiltmc.*`, `com.mumfrey.liteloader`, `*.java` | **ForgeWrapper** (MIT): a `mainClass` vira `io.github.zekerzhayard.forgewrapper.installer.Main`, o installer do Forge vai em `mavenFiles` (baixado de `maven.minecraftforge.net`) e, no 1º lançamento, o ForgeWrapper carrega o installer num `URLClassLoader` e chama o `PostProcessors` **do próprio installer dentro da JVM do jogo**, depois invoca a `mainClass` real | JSON do meta já corrigido (`:universal`, `+tweakers: [FMLTweaker]`, `mainClass` launchwrapper) | Mojang + Adoptium + Azul via meta |
| **Modrinth App** (`packages/app-lib`, ex-theseus) | GPL-3.0-only | `launcher-meta.modrinth.com/{minecraft,forge,neo,fabric,quilt}/v0/manifest.json`, gerado pelo `daedalus_client` (MIT) que **processa os installers no servidor**, converte o formato legado e publica os arquivos `data/` como coordenadas Maven sintéticas `com.modrinth.daedalus:{loader}-installer-extracts:...` (espelhamento) | O **cliente** executa os processors como processo filho com o Java da instância (`install_minecraft_inner`, `args::get_processor_arguments`, `get_processor_main_class`) | convertido para o formato moderno no servidor; 1.12.2 tem `processors: []` | Azul Zulu |
| **HMCL** | GPL-3.0 + termos adicionais (seção 7: mudar nome/versão em forks, manter copyright) | fontes oficiais ou o mirror chinês BMCLAPI **[não verificado nesta pesquisa]** | `ForgeNewInstallTask`: processors como **processo externo** com o Java do próprio HMCL | `ForgeOldInstallTask` | Mojang + foojay Disco |
| **ATLauncher** | GPL-3.0 | próprio, com **espelho** dos installers no CDN do ATLauncher | `Processor.process`: executa os processors **dentro da JVM do launcher** via reflexão | `LegacyForgeLoader` (+ `Forge113Loader`, `NeoForgeLoader`) | Mojang |
| **portablemc** (crate Rust) | Apache-2.0 | APIs oficiais (Mojang, Fabric meta, Maven do Forge/NeoForge) | baixa o installer e executa os processors (`RunInstallerProcessor`, erros tipados `InstallerProcessorFailed`/`Corrupted`) | trata `install_profile` legado e o JAR `universal` (`MissingUniversalClient`) | Java do sistema ou runtime da Mojang |

Fontes **[verificado pelo subagente de pesquisa no código; amostras conferidas por mim]**:
https://github.com/PrismLauncher/meta/blob/main/meta/run/generate_forge.py,
https://github.com/PrismLauncher/meta/blob/main/meta/common/forge.py,
https://github.com/ZekerZhayard/ForgeWrapper (MIT),
https://github.com/modrinth/code/blob/main/apps/daedalus_client/src/forge.rs,
https://github.com/modrinth/code/blob/main/packages/app-lib/src/launcher/mod.rs,
https://github.com/modrinth/code/blob/main/packages/app-lib/src/launcher/args.rs,
https://github.com/HMCL-dev/HMCL/blob/main/HMCLCore/src/main/java/org/jackhuang/hmcl/download/forge/ForgeNewInstallTask.java,
https://github.com/ATLauncher/ATLauncher/blob/master/src/main/java/com/atlauncher/data/minecraft/loaders/forge/Processor.java,
https://github.com/theorzr/portablemc/blob/main/portablemc/src/forge/mod.rs.

Pontos comuns e lições:
- **Versões quebradas conhecidas**: Prism (`BAD_VERSIONS`) e Modrinth (`BLACKLIST`) excluem
  `1.12.2-14.23.5.2851` **[verificado pelo subagente]**. O Warden deve ter uma *denylist* equivalente.
- Ninguém reimplementa os processors em código nativo: todos **executam os JARs de ferramentas do
  próprio installer** (em processo separado ou na JVM). O Warden deve fazer o mesmo — em processo
  separado (isolamento de falhas, sem JVM embutida no Tauri).
- Wrapper Java de lançamento: Prism (`org.prismlauncher.EntryPoint`, lê um "launch script" pelo stdin) e
  Modrinth (`com.modrinth.theseus.MinecraftLaunch`) iniciam o jogo por uma classe própria que depois
  invoca a `mainClass`. Isso permite contornar o limite de linha de comando do Windows, aplicar ajustes
  (ex.: título da janela, applets antigos) e ter um canal de controle. **[inferência]** Para o Warden,
  começar sem wrapper (linha de comando pura, ou `@argfile` no Java 9+) e só criar um wrapper se surgir
  necessidade concreta.

### 3.7 Bibliotecas reutilizáveis (Rust) e licenças

Consulta ao crates.io/GitHub em 2026-10-01 **[verificado pelo subagente; portablemc conferido por mim]**:

| Crate / projeto | Licença | Estado | Cobertura | Avaliação para o Warden |
|---|---|---|---|---|
| **portablemc** (v5, Rust) | **Apache-2.0** | ativo: 5.0.5 em 2026-09-09; 550★; ~17,6 mil downloads | vanilla, Fabric, Quilt, LegacyFabric, Babric, **Forge e NeoForge (inclui legado)**, offline e MSA; Java do sistema ou da Mojang; tokio + reqwest | **Melhor candidato** a reutilizar como biblioteca ou como referência permissiva. Avaliar em protótipo: API, progresso/eventos, cancelamento, Windows, tratamento de erros |
| **daedalus** (Modrinth) | MIT | crate 0.2.3 (2024-08); código vivo no monorepo `modrinth/code` | só modelos/parsing dos JSONs | vendorizar os tipos (com atribuição) se útil |
| **lighty-launcher** | MIT | jovem (2025), 26.9.3 | vanilla/Fabric/Quilt/Forge/NeoForge, Java Temurin/Zulu/... | promissor, imaturo |
| **lyceris** | MIT OR Apache-2.0 | pouco ativo | Forge só > 1.12.2 | não atende 1.7.10/1.12.2 |
| **ferinth** / **furse** | MIT | ativos | APIs Modrinth / CurseForge | fora do escopo desta tarefa (fontes de mods) |
| Modrinth `app-lib` (theseus) | **GPL-3.0-only** | ativo | completo | **só como referência de leitura**; copiar/traduzir código torna o Warden GPL |
| Prism, HMCL, ATLauncher | GPL-3.0 | ativos | completos | idem |
| mcvm/Nitrolaunch | GPL-3.0-or-later | ativo | multi-loader | idem |
| mc-launcher | AGPL-3.0-or-later | parado | básico | evitar |
| late-java-core | CC-BY-NC-4.0 | — | — | evitar (não comercial) |
| open_launcher, minecraft-launcher-core, scl-core, interpulse | vários | **abandonados** | — | evitar |

**Implicação de licença [inferência]**: a licença do Warden ainda não foi decidida. Se o Warden for
MIT/proprietário, só pode **incorporar** código MIT/Apache (portablemc, daedalus, ForgeWrapper) e deve
estudar os GPL apenas como especificação de comportamento (reimplementar a partir dos formatos JSON,
que são interface). Executar o installer do Forge/NeoForge (LGPL-2.1) como **programa separado** não
contamina o Warden. Se o Warden for GPL-3.0, todo o ecossistema acima fica disponível (exceto CC-BY-NC).

**Metadados de terceiros**: `launcher-meta.modrinth.com` e `meta.prismlauncher.org` são públicos e sem
autenticação, mas não há termos claros para uso por outros launchers; os Termos da Modrinth proíbem
acesso por "robot, spider or other automatic device" em termos genéricos **[fonte]**
https://modrinth.com/legal/terms. **Recomendação**: usar as **fontes primárias** (piston-meta,
meta.fabricmc.net, Maven do Forge/NeoForge) e, no máximo, usar os metas de terceiros como referência em
testes (ex.: comparar o JSON que o Warden gera com o do Prism para detectar regressões).

---

## 4. Modo offline: perfil, UUID e termos da Mojang/Microsoft

> Ressalva: a parte legal abaixo é interpretação de textos públicos, não parecer jurídico.

### 4.1 Mecânica do perfil offline

**UUID offline.** O próprio jogo deriva o UUID de jogadores offline com
`UUID.nameUUIDFromBytes(("OfflinePlayer:" + nome).getBytes(UTF_8))` — um UUID **v3 (MD5) sem namespace**
**[fonte: código descompilado de terceiros; mesma implementação no Prism e HMCL]**:
- Prism: MD5 de `"OfflinePlayer:%1"`, força versão 3 e variante IETF, serializa **sem hífens**
  (https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/minecraft/auth/MinecraftAccount.cpp).
- HMCL: idem (https://github.com/HMCL-dev/HMCL/blob/main/HMCLCore/src/main/java/org/jackhuang/hmcl/auth/offline/OfflineAccountFactory.java).
- Hífens são opcionais em `--uuid` (https://minecraft.wiki/w/Java_Edition_client_command_line_arguments).
- O hash diferencia maiúsculas/minúsculas (`Steve` ≠ `steve`).

Implementação em Rust **[inferência]**: `md5("OfflinePlayer:" + nome)`, depois `b[6] = (b[6] & 0x0f) | 0x30`
e `b[8] = (b[8] & 0x3f) | 0x80`. **Não** usar `Uuid::new_v3(namespace, ...)` da crate `uuid` (ela
prefixa um namespace; o Java não usa namespace). Cobrir com teste unitário contra valores conhecidos
gerados pelo Java. Importante para o Warden: o UUID offline é **estável por nome**, então mundos de
teste mantêm inventário/dados do jogador entre execuções desde que o nome não mude.

**Nome de usuário**: validar `^[A-Za-z0-9_]{3,16}$` (regra de nomes de conta Java) para evitar
problemas com mods que validam nome **[inferência]**.

**Valores passados por launchers conhecidos** **[fonte, código citado]**:

| Launcher | `accessToken` | `userType` | `userProperties` | `xuid` / `clientId` |
|---|---|---|---|---|
| Prism (offline) | `"0"` | `"offline"` | `{}` | não define |
| HMCL (offline) | UUID aleatório compacto | `"msa"` | `{}` | — |
| Modrinth App (só MSA) | token real | `"msa"` fixo | `{}` | xuid `"0"`, clientId UUID fixo |

Fontes: https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/minecraft/auth/AuthSession.cpp,
https://github.com/HMCL-dev/HMCL/blob/main/HMCLCore/src/main/java/org/jackhuang/hmcl/auth/offline/OfflineAccount.java,
https://github.com/modrinth/code/blob/main/packages/app-lib/src/launcher/args.rs.

Observações:
- Valores oficiais de `--userType`: `msa`, `mojang`, `legacy` (wiki citada acima). O `"0"` como token
  **não é especial no vanilla**; é um atalho do **Forge** (patch em `Minecraft.java`: "We use "0" in dev.
  Short circuit to stop exception spam") que evita chamadas de rede quando o token é `"0"`
  **[fonte]** https://github.com/MinecraftForge/MinecraftForge/blob/HEAD/patches/minecraft/net/minecraft/client/Minecraft.java.patch.
- Recomendação para o Warden: `accessToken = "0"` (compatível com o atalho do Forge, não vazio — alguns
  parsers de argumentos falham com string vazia), `userType = "legacy"` até 1.21.8 (valor oficial que
  significa "sem conta Microsoft"; o Prism usa `offline`, que o jogo também tolera **[inferência]**),
  `user_properties = {}`, `auth_xuid = "0"`, `clientid` = um UUID fixo do Warden **[inferência]**.

**O que muda por versão** **[verificado nos JSONs; demais itens com fonte]**:

| Faixa | Argumentos de identidade | Comportamento offline |
|---|---|---|
| ≤1.5.2 | posicionais `${auth_player_name} ${auth_session}` | sessão `-` |
| 1.6.x | `--username`, `--session` | Prism usa `session="-"` offline; com token, `token:<access>:<uuid>` |
| 1.7.2 | `--uuid`, `--accessToken` | — |
| 1.7.10–1.8.9 | + `--userProperties`, `--userType` | **sem `--userProperties` o jogo falha** com "Missing required option(s) [userProperties]" (https://github.com/MinecraftForge/ForgeGradle/issues/104) |
| 1.9–1.12.2 | `--userType`, `--versionType` (sem `userProperties`) | singleplayer ok |
| 1.13–1.18 | formato `arguments` | singleplayer ok |
| 1.16.4–1.16.5 | idem | **botões Multiplayer e Realms ficam cinza** com conta offline desde uma mudança nos servidores da Mojang em 2023 (o authlib dessas versões não confere o status HTTP). Contorno usado pela comunidade: `-Dminecraft.api.auth.host=https://nope.invalid` (e `account.host`, `session.host`, `services.host`) **[fonte]** https://github.com/PrismLauncher/PrismLauncher/issues/1487, https://github.com/FabricMC/fabric-loom/issues/915 |
| 1.19–1.21.8 | + `--clientId`, `--xuid` | chat signing: sem chave de perfil o jogo loga "Failed to retrieve profile key pair" e segue; servidores com `enforce-secure-profile=true` recusam o jogador. Singleplayer e LAN funcionam |
| **1.21.9+ / 26.x** | **sem `--userType`**; existe a flag vanilla **`--offlineDeveloperMode`** | com a flag, o jogo cria um serviço de autenticação offline, usa `UserApiService.OFFLINE` e não chama serviços da Mojang (perfil, atributos, blocklist, certificados) **[verificado]**: a opção aparece como código vanilla (linha de contexto) no patch do NeoForge https://github.com/neoforged/NeoForge/blob/HEAD/patches/net/minecraft/client/main/Main.java.patch; o NeoForge a ativa automaticamente em dev quando não há `--accessToken` |

**Chamadas de rede com perfil offline** **[fonte: código descompilado; inferência para versões
intermediárias]**: o cliente moderno consulta `/player/attributes` (privilégios), `/privacy/blocklist`
e `/player/certificates` (chaves de chat). Se falhar, loga "Failed to fetch user properties" e usa
propriedades offline (sem telemetria); as chamadas são assíncronas e não devem travar o menu. Com
token inválido, "An invalid token will allow singleplayer but show any error when attempting to join a
server" (wiki citada). Hosts configuráveis por propriedades de sistema (`minecraft.api.env`,
`minecraft.api.session.host`, `minecraft.api.services.host`, `minecraft.api.profiles.host`; em 26.3,
também `minecraft.api.discovery.host`) **[fonte]** https://github.com/unmojang/drasl/blob/HEAD/doc/usage.md.
**Skins**: offline mostra a skin padrão derivada do UUID (Steve/Alex/etc.); não há skin personalizada
sem mods (https://minecraft.wiki/w/Skin).

### 4.2 Termos da Mojang/Microsoft

Trechos literais **[fonte]**:
- **EULA** (https://www.minecraft.net/en-us/eula): "You may develop tools, plug-ins and services as long
  as they do not seem official or approved by us."; "When you buy our games, that means you can download,
  install, and play them."; proíbe "give copies of our game software or content to anyone else";
  "we need all game downloads and updates to come from a source that we authorize."
- **Usage Guidelines** (https://www.minecraft.net/en-us/usage-guidelines): "Do not redistribute our games
  or any alterations of our games or game files."; exigem o aviso "NOT AN OFFICIAL MINECRAFT [PRODUCT].
  NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT"; "Minecraft" só como elemento secundário do
  nome do produto.
- **Microsoft Services Agreement** (https://www.microsoft.com/en-us/servicesagreement): não contornar
  restrições de acesso aos serviços nem medidas técnicas de proteção.

**Avaliação honesta [inferência]**:
- **Permitido com clareza**: launcher de terceiros que não pareça oficial e que **baixe tudo dos
  servidores da Mojang** (é o modelo de todos os launchers conhecidos).
- **Proibido com clareza**: redistribuir `client.jar`/assets/bibliotecas da Mojang (inclusive dentro de
  um modpack exportado ou de um instalador do Warden); facilitar jogar sem ter comprado.
- **Não existe cláusula explícita** sobre "modo offline" em si.
- **Área cinzenta**: um launcher que roda o jogo completo **sem nunca verificar posse**. A Mojang já
  enviou DMCAs contra ferramentas que contornam a verificação de licença — ex.: 2026-08-04 contra
  "custom username tools that deliberately bypass Microsoft's OAuth license verification"
  (https://github.com/github/dmca/blob/master/2026/08/2026-08-04-mojang.md) e 2024-01-08 contra cliente que
  "simulate[s] a successful sign in" (https://github.com/github/dmca/blob/master/2024/01/2024-01-08-mojang.md).
  O padrão observado é ação contra **contorno de licença e redistribuição**, não contra uso offline por
  quem possui o jogo **[inferência]**.

### 4.3 Como os launchers sérios lidam com isso

- **Prism**: só permite adicionar conta offline depois de uma conta Microsoft que possua o jogo —
  "You must add a Microsoft account that owns Minecraft before you can add an offline account."
  **[verificado]** (`launcher/ui/pages/global/AccountListPage.cpp`). Desde o PR #4885 (refatoração do
  `LaunchController`, mesclado em 2026-02-17 **[verificado]**), no lançamento procura uma conta que possua
  o jogo (`ownsMinecraft()`); se não houver, pergunta "Play demo?" e inicia em **modo Demo**
  (`LaunchMode::Demo`) **[verificado]** (https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/LaunchController.cpp).
- **MultiMC**: "You need a premium Minecraft account"; offline "only works when you've successfully
  authenticated at least once" (https://github.com/MultiMC/Launcher/wiki/FAQ/120aae12309a33bb589d55b1c394cfd9366dc8ef).
- **HMCL**: opção offline desabilitada até haver conta Microsoft (exceto locale da China continental)
  (https://github.com/HMCL-dev/HMCL/blob/main/HMCL/src/main/java/org/jackhuang/hmcl/ui/account/AccountListPage.java).
- **ATLauncher**: exige login Microsoft; "Play Offline" só após login (https://wiki.atlauncher.com/getting-started/launching-minecraft/).
- **Modrinth App**: não tem conta offline (pedido aberto https://github.com/modrinth/code/issues/7408).
- **Contra-exemplos**: TLauncher/SKLauncher (associados a pirataria) e forks que removem a trava do
  Prism ("Prism-Launcher-Offline", PollyMC). Não são modelo para o Warden.

### 4.4 Login Microsoft (para referência futura)

Fluxo: app registrado no Azure (tenant `consumers`, escopo `XboxLive.signin`, sem client secret) → MSA →
XBL → XSTS → `login_with_xbox` → `/entitlements/mcstore` (posse) → `/minecraft/profile`
(https://minecraft.wiki/w/Microsoft_authentication). **Desde 2023-05-30 a Mojang revisa manualmente e
coloca em allowlist cada novo app ID** (formulário `aka.ms/mce-reviewappid`); sem aprovação a API
responde 403 **[fonte]** https://help.minecraft.net/hc/en-us/articles/16254801392141 (arquivo:
https://github.com/XeroAlpha/MinecraftHelpCenterArchive/blob/HEAD/help/Java-Edition-Support/Java-Edition-Game-Service-API-Review-or-Application-Process.md).
Não reutilizar client ID de outro launcher.

### 4.5 Postura recomendada para o Warden

**Obrigatório**
1. Nunca empacotar/redistribuir `client.jar`, assets ou bibliotecas da Mojang (nem no instalador do
   Warden, nem na exportação do pack). Baixar sempre de `piston-meta`/`piston-data`/`libraries.minecraft.net`/
   `resources.download.minecraft.net` na máquina do usuário.
2. Não contornar nada: sem patch de authlib, sem servidor Yggdrasil falso, sem redirecionar hosts para
   "enganar" servidores online.
3. Exibir "NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT."
   e não usar marca/visual oficiais; "Minecraft" só como descritivo.
4. Comunicação: o launcher existe **para testar o modpack localmente**; nunca anunciar como forma de
   jogar de graça; texto explícito "Você precisa possuir o Minecraft: Java Edition".

**Recomendado**
5. Perfil offline com UUID v3 `OfflinePlayer:`, token `"0"`, `user_properties {}`; em 1.21.9+, passar
   `--offlineDeveloperMode` (o jogo não toca nos serviços da Mojang); em 1.16.4/1.16.5, documentar o
   botão de multiplayer cinza (o contorno de hosts `.invalid` só mexe no próprio cliente, mas avaliar se
   vale o risco de parecer "contorno" — no Warden de testes o multiplayer online não é necessário).
6. Não oferecer multiplayer online (no máximo LAN/servidor local para testes).

**Opcional — o padrão mais defensável a médio prazo**
7. Verificação de posse única via login Microsoft (exige app aprovado pela Mojang) antes de liberar o
   perfil offline, como Prism/HMCL/MultiMC; sem conta, oferecer modo `--demo`. Decisão do dono do
   projeto (ver questões em aberto): implica registro no Azure e espera pela aprovação da Mojang.

---

## 5. Estrutura da instância (.minecraft)

### 5.1 Separação "launcher" vs "jogo"

O `.minecraft` padrão mistura duas coisas que um launcher moderno separa **[fonte]**
https://minecraft.wiki/w/.minecraft:

- **Diretório do launcher** (compartilhável entre instâncias): `assets/`, `libraries/`, `versions/`,
  runtimes de Java, `launcher_profiles.json`, `launcher_*.json`, caches.
- **Diretório do jogo (`--gameDir`)** — a "instância": tudo o que o jogo e os mods leem/escrevem.

Prism, Modrinth App, ATLauncher e HMCL (modo "isolado") usam um `gameDir` por instância e
compartilham assets/libraries. O Warden deve fazer o mesmo: `<app data>/Warden/shared/{assets,libraries,versions,runtimes}`
e `<app data>/Warden/instances/<pack-id>/minecraft/` como `gameDir` **[inferência, padrão de mercado]**.
O `hs_err_pid*.log` e outros arquivos da JVM caem no **diretório de trabalho** do processo, então o
`cwd` do processo deve ser o `gameDir`.

### 5.2 Conteúdo do gameDir e a quem pertence

Classificação para a exportação do pack (o que vai para o packwiz) — "Pack" = pertence ao pack;
"Runtime" = gerado pelo jogo/mods, nunca exportar; "Decidir" = depende de intenção do autor.

| Caminho | Quem cria | Classe | Observações |
|---|---|---|---|
| `mods/` | pack | **Pack** | via metafiles `.pw.toml` (Modrinth/CurseForge) ou arquivos locais; `mods/*.disabled` = mod desativado |
| `config/` | mods (gerado na 1ª execução) + autor | **Pack** (só o que o autor alterou) | muitos arquivos são regenerados/reformatados a cada boot (ver seção 7) |
| `defaultconfigs/` | Forge/NeoForge 1.13+ | **Pack** | configs "server" padrão copiadas para `saves/<mundo>/serverconfig/` ao criar mundo |
| `resourcepacks/` | autor/usuário | **Pack** (os adicionados pelo autor) | `.zip` ou pasta |
| `shaderpacks/` | autor/usuário | **Pack** | `shaderpacks/<pack>.txt` = settings do Iris/OptiFine por shader → **Decidir** |
| `kubejs/` | KubeJS | **Pack** (`server_scripts/`, `client_scripts/`, `startup_scripts/`, `assets/`, `data/`, `config/`) | `kubejs/exported/` e caches = **Runtime** **[inferência]** |
| `scripts/` | CraftTweaker | **Pack** | |
| `global_packs/`, `openloader/`, `globalresources/`, `paxi/` | mods de datapacks globais | **Pack** | |
| `options.txt` | jogo | **Decidir** | keybinds, gráficos, idioma, `resourcePacks:[...]` (ordem dos packs ativos). Contém `version:<DataVersion>`; tem chaves voláteis. Ver seção 7.3 (passo 5) |
| `optionsof.txt`, `optionsshaders.txt` | OptiFine | **Decidir** | |
| `servers.dat` | jogo (NBT binário) | **Decidir** | lista de servidores — alguns packs distribuem; o packwiz não tem tratamento especial |
| `saves/` | jogo | **Runtime** | mundos de teste; nunca exportar por padrão (no máximo, "mundo modelo" explícito) |
| `logs/`, `crash-reports/`, `debug/` | jogo | **Runtime** | |
| `screenshots/`, `stats/` (≤1.6) | jogo | **Runtime** | |
| `usercache.json`, `usernamecache.json` (Forge) | jogo | **Runtime** | |
| `hotbar.nbt`, `command_history.txt`, `realms_persistence.json` | jogo | **Runtime** | |
| `hs_err_pid*.log`, `*.heapdump`, `replay_pid*.log` | JVM | **Runtime** | no `cwd` |
| `.fabric/` | Fabric Loader (cache de remapeamento) | **Runtime** | |
| `.mixin.out/` | Mixin (export de debug) | **Runtime** | |
| `mods/.connector/`, `.cache/`, `local/` | mods diversos (Sinytra Connector, FTB/KubeJS "local") | **Runtime** | `local/` = dados locais por cliente **[inferência]** |
| `journeymap/data/`, `XaeroWaypoints/`, `XaeroWorldMap/` | mods de mapa | **Runtime** | `journeymap/config/` = **Decidir** |
| `schematics/` | usuário | **Runtime** | |
| `resources/` | launcher (assets `pre-1.6`) | **Runtime** | só em versões ≤1.5.2 |
| `natives/`, `bin/` | launchers antigos | **Runtime** | o Warden deve pôr natives **fora** do gameDir |
| `packwiz.json`, `packwiz-installer*.jar` | packwiz-installer | **Runtime** | estado do instalador (seção 7) |
| `pack.toml`, `index.toml`, `*.pw.toml` | packwiz | n/a | ficam no **repositório do pack**, não na instância |

### 5.3 Padrão de ignore sugerido para "trazer de volta" (instância → pack)

Ponto de partida (estilo `.gitignore`), a ser refinado por testes **[inferência]**:

```gitignore
# gerados pelo jogo / JVM
/logs/
/crash-reports/
/debug/
/saves/
/screenshots/
/stats/
/resources/
/usercache.json
/usernamecache.json
/hotbar.nbt
/command_history.txt
/realms_persistence.json
hs_err_pid*.log
replay_pid*.log
*.heapdump
# caches de loaders/mods
/.fabric/
/.quilt/
/.mixin.out/
/.cache/
/local/
/mods/.connector/
/kubejs/exported/
/journeymap/data/
/XaeroWaypoints/
/XaeroWorldMap/
# artefatos do launcher/packwiz-installer
/natives/
/packwiz.json
/packwiz-installer*.jar
# backups/temporários gerados por mods e pelo jogo
*.bak
*.old
*.tmp
/servers.dat_old
```

O packwiz já tem seus próprios ignores padrão para o **repositório do pack** (`.git/**`,
`.gitattributes`, `.gitignore`, `.DS_Store`, `/*.zip`, `*.mrpack`, `packwiz.exe`, `packwiz`) e lê um
`.packwizignore` no estilo gitignore **[verificado]**
(https://github.com/packwiz/packwiz/blob/main/core/index.go, `ignoreDefaults`). O Warden deve gerar
um `.packwizignore` coerente com a lista acima, para que um `packwiz refresh` no repositório nunca
indexe lixo mesmo que alguém copie a instância inteira por engano.

---

## 6. Logs e crashes

### 6.1 Onde ficam e formatos

| Artefato | Local | Formato / observações |
|---|---|---|
| `logs/latest.log` | gameDir | Vanilla/Fabric (config da Mojang): `[HH:mm:ss] [thread/LEVEL]: msg`; rotação para `logs/<yyyy-MM-dd>-<n>.log.gz`. Forge/NeoForge (config própria do loader, quando o launcher não impõe outra — ver 3.1): `[ddMMMyyyy HH:mm:ss.SSS] [thread/LEVEL] [logger/marker]: msg` **[fonte]** XML da Mojang (https://piston-data.mojang.com/v1/objects/bd65e7d2e3c237be76cfbef4c2405033d7f91521/client-1.12.xml), https://github.com/MinecraftForge/MinecraftForge/blob/HEAD/fmlloader/src/main/resources/log4j2.xml, https://github.com/neoforged/FancyModLoader/blob/HEAD/loader/src/main/resources/net/neoforged/fml/startup/log4j2.xml |
| `logs/debug.log` | gameDir | Forge/NeoForge, todos os níveis, rotação por tamanho. Introduzido no Forge em 2018 (era 1.12.2), substituindo `logs/fml-client-latest.log` — Forge 1.7.10 e 1.12.2 antigos têm `fml-client-latest.log` **[fonte]** https://github.com/MinecraftForge/MinecraftForge/commit/ae654edfc26f486f0d55bc4e01c20ea116fdebfa. Fabric não tem `debug.log` próprio **[não confirmado]** |
| `crash-reports/crash-<yyyy-MM-dd_HH.mm.ss>-client.txt` | gameDir | `---- Minecraft Crash Report ----`, comentário `//`, `Time:`, `Description:`, stack trace com `Caused by:`, "A detailed walkthrough of the error...", categorias `-- Head --` ... `-- System Details --`. Fabric API acrescenta `Fabric Mods:`; Forge acrescenta `Mod List` e `Suspected Mod(s):` (`CrashReportAnalyser`) **[fonte]** https://github.com/MinecraftForge/MinecraftForge/blob/HEAD/src/main/java/net/minecraftforge/logging/CrashReportAnalyser.java, https://github.com/FabricMC/fabric/blob/HEAD/fabric-crash-report-info-v1/src/main/java/net/fabricmc/fabric/mixin/crash/report/info/SystemReportMixin.java |
| `crash-reports/crash-<data>-fml.txt` | gameDir | falha de **carregamento de mods** no Forge/NeoForge moderno: categorias por mod (`MOD <id>` / `Mod loading issue for: <id>`) com `Mod File`, `Failure message`, `Mod Version`, `Exception message` **[fonte]** `CrashReportExtender.java` do Forge e do NeoForge. NeoForge também escreve no log `Loading errors encountered:` + linhas `\t- ...` (mesmas mensagens da tela de erro) |
| `hs_err_pid<pid>.log` | **diretório de trabalho** do processo (fallback: `%TEMP%`) | crash nativo da JVM (driver, memória nativa). Fixável com `-XX:ErrorFile=<dir>/hs_err_pid%p.log` **[fonte]** https://docs.oracle.com/en/java/javase/21/troubleshoot/fatal-error-log.html |
| stdout/stderr | pipes do processo | vanilla/Fabric com o XML da Mojang: **eventos XML log4j** (`LegacyXMLLayout`); Forge/NeoForge: texto com códigos ANSI; JVM/mods escrevem texto cru em ambos. Erros antes do log4j subir (JVM não inicia, classpath errado) **só** aparecem aqui |

### 6.2 Captura robusta do processo (Windows)

- `javaw.exe` não tem console, mas **stdout/stderr podem ser redirecionados por pipes** (`STARTF_USESTDHANDLES`
  no `CreateProcess`) — é o que o Prism faz **[fonte]**
  https://learn.microsoft.com/en-us/windows/win32/procthread/creating-a-child-process-with-redirected-input-and-output.
  Em Rust: `Command::new(javaw).stdout(Stdio::piped()).stderr(Stdio::piped()).current_dir(game_dir)`.
  Alternativa: `java.exe` com `creation_flags(CREATE_NO_WINDOW)`.
- **Ler stdout e stderr em tarefas separadas** (senão o pipe enche e o jogo trava); repassar linhas para a
  UI com backpressure e também gravar em arquivo próprio do Warden (`<instância>/.warden/logs/<timestamp>.log`),
  porque o `latest.log` não contém o que foi escrito antes do log4j iniciar.
- **Encoding**: JEP 400 (Java 18) tornou UTF-8 o padrão, **exceto I/O de console**; Java 19 criou
  `stdout.encoding`/`stderr.encoding` **[fonte]** https://openjdk.org/jeps/400,
  https://bugs.openjdk.org/browse/JDK-8283620. No Java 8/17 em Windows a saída sai em cp1252/cp850. O Prism
  decodifica como UTF-8 se Java ≥ 18 e com o codec do sistema caso contrário **[fonte: código do Prism]**.
  Recomendação: passar `-Dfile.encoding=UTF-8` (Java 8–17) e `-Dstdout.encoding=UTF-8 -Dstderr.encoding=UTF-8`
  (Java ≥ 19) e mesmo assim decodificar com fallback (UTF-8 → Windows-1252 via `encoding_rs`).
  **[inferência]** Atenção: `-Dfile.encoding=UTF-8` muda também a leitura/escrita de arquivos de alguns mods
  antigos; avaliar em testes antes de ativar para 1.7.10/1.12.2.
- **Fim do processo**: `child.wait()`; no Windows `ExitStatus::code()` devolve o NTSTATUS como `i32`
  (ex.: `-1073741819` = `0xC0000005` `STATUS_ACCESS_VIOLATION`, crash nativo → procurar `hs_err_pid`)
  **[fonte]** https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-erref/596a1078-e883-4972-9bbc-49e60bebca55.
  Exit code 0 com crash report novo também é "crash" (o jogo grava o relatório e sai normalmente).
  `-805306369` (`0xCFFFFFFF`) só tem fontes comunitárias — não usar como diagnóstico.
- **Encerrar**: "Parar" deve matar a **árvore** de processos (o jogo pode ter filhos); no Windows usar um
  *Job Object* com `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, o que também garante que o jogo morra se o Warden
  fechar **[inferência]**.
- **Coleta pós-execução**: registrar o instante do lançamento; ao sair, juntar exit code, saída capturada,
  `crash-reports/*` com mtime posterior, `logs/latest.log`, `logs/debug.log` e `hs_err_pid*.log` novos.
- **Privacidade antes de enviar à IA**: substituir nome do usuário do Windows nos caminhos
  (`C:\Users\<nome>`), nome do jogador, UUID e token — como o `censorPrivateInfo` do Prism.

### 6.3 Padrões detectáveis sem IA

Catálogo inicial (regex compatíveis com a crate `regex` do Rust; remover antes códigos `§x` e ANSI).
Compilado pelo subagente de pesquisa a partir das **mensagens-fonte** dos loaders e da JVM, e cruzado com
os catálogos do HMCL, do Refraction e do codex **[fonte por linha; amostras conferidas por mim nos
arquivos de mensagens]**:

| # | Padrão (regex sugerida) | Diagnóstico → ação | Escopo |
|---|---|---|---|
| 1 | `(?m)^---- Minecraft Crash Report ----$` | é um crash report: extrair `Description:` e a primeira exceção/`Caused by:` mais profunda | todos |
| 2 | `Manually triggered debug crash` | F3+C intencional → ignorar | todos |
| 3 | `(?m)^Description: Ticking (block )?entity` + `Entity Type: (.+)` / `Block: (.+)` | entidade/bloco corrompido no mundo → apontar o mod dono do tipo | todos |
| 4 | `Missing or unsupported mandatory dependencies:` + `Mod ID: '([^']+)', Requested by: '([^']+)', Expected range: '([^']+)', Actual version: '([^']+)'` | dependência ausente/versão errada → oferecer adicionar | Forge 1.13+ |
| 5 | `Mod (\S+) requires (\S+) (\S+)` | idem (texto da tela de erro) | Forge 1.13+/NeoForge |
| 6 | `net\.minecraftforge\.fml\.common\.MissingModsException: Mod \S+ \((.+)\) requires (.+)` | dependência ausente | Forge 1.8–1.12.2 |
| 7 | `cpw\.mods\.fml\.common\.MissingModsException` | dependência ausente (formato exato a confirmar) | Forge 1.7.10 |
| 8 | `Found duplicate mods:` / `Mod ID: '([^']+)' from mod files: (.+)` | mod duplicado → remover um | Forge moderno |
| 9 | `Found a duplicate mod (.+) at \[(.+)\]` | duplicado | Forge antigo |
| 10 | `Mod (\S+) is present in multiple files: (.+)` | duplicado | NeoForge |
| 11 | `Duplicate versions for mod ID '([^']+)'` / `Mods share ID with builtin mod` | duplicado | Fabric |
| 12 | `Incompatible mods found!` / `Some of your mods are incompatible with the game or each other!` / `Incompatible mod set!` | cabeçalho do bloco de resolução de dependências | Fabric |
| 13 | `(?m)^\s*- Mod '(.+?)' \((\S+)\) (\S+) requires (.+?) of (?:mod )?'(.+?)' \((\S+)\), which is missing!` | dependência ausente | Fabric ≥ 0.12 |
| 14 | `requires (.+?) of (.+?), but only the wrong versions? (?:is\|are) present: (.+)!` | versão errada da dependência | Fabric |
| 15 | `which is disabled for this environment \(client/server only\)!` | mod de um só lado → corrigir `side` no `.pw.toml` | Fabric |
| 16 | `is incompatible with (.+?) of (.+?), yet` / `conflicts with (.+?) of` | `breaks`/`conflicts` declarado | Fabric |
| 17 | `Could not find required mod: (\S+) requires \{(\S+) @ \[(.+?)\]\}` | dependência ausente | Fabric ≤ 0.11 |
| 18 | `Could not execute entrypoint stage '([^']+)' due to errors, provided by '([^']+)'` | o mod do grupo 2 falhou ao iniciar | Fabric/Quilt |
| 19 | `Caught exception from (.+?) \((.+)\)` / `Failed to create mod instance\. ModID: (\S+?),` | mod quebrou no carregamento | Forge |
| 20 | `File (.+) is a (Fabric\|Quilt\|LiteLoader) mod and cannot be loaded` / `is for Minecraft Forge or an older version of NeoForge` | JAR de outro loader → remover | Forge 1.13+/NeoForge |
| 21 | `Mod File (.+) needs language provider (\S+):(\S+) to load` | mod feito para outra versão do Forge/MC | Forge 1.13+ |
| 22 | `The mod (\S+) does not wish to run in Minecraft version` | versão do MC errada | Forge 1.12 |
| 23 | `Mixin apply failed\|Mixin prepare failed\|MixinApplyError\|\.mixins\.json\] FAILED during` / `Mixin apply for mod (\S+) failed` | Mixin falhou → mapear `<x>.mixins.json` para o mod (lendo os JARs) | todos com Mixin |
| 24 | `Critical injection failure: .+? in ([\w.\-]+\.mixins\.json)` / `InvalidInjectionException` | alvo do mixin mudou (mod para outra versão do MC) ou conflito entre mods | idem |
| 25 | `MixinTransformerError: An unexpected critical error was encountered` | Mixin fatal | Fabric |
| 26 | `ResolutionException: Modules? (\S+) (?:and (\S+) export package\|contains package)` | dois JARs exportam o mesmo pacote (JPMS) | Forge/NeoForge 1.17+ |
| 27 | `Invalid module name: '' is not a Java identifier` | nome de arquivo do JAR inválido → renomear | Forge 1.17+ |
| 28 | `UnsupportedClassVersionError: .+ \(class file version (\d+)\.\d+\), this version of the Java Runtime only recognizes class file versions up to (\d+)` | Java velho; Java necessário = major − 44 (52=8, 60=16, 61=17, 65=21, 69=25) | todos |
| 29 | `AppClassLoader cannot be cast to class java\.net\.URLClassLoader` / `NoSuchFieldException: ucp` | Java novo demais para launchwrapper → Java 8 | Forge/vanilla-tweakers 1.7.10–1.12.2 |
| 30 | `NoSuchMethodError: .*sun\.security\.util\.ManifestEntryVerifier` | Forge 1.16.5 antigo com Java 8u321+ → atualizar Forge ou Java ≤ 8u312 | Forge 1.16.5 |
| 31 | `java\.lang\.OutOfMemoryError` / `There is insufficient memory for the Java Runtime Environment` / `Could not reserve enough space for .*object heap` / `Invalid maximum heap size` | memória: heap pequeno, memória nativa, Java 32 bits | todos |
| 32 | `Pixel format not accelerated` / `GLFW error 65542` / `does not appear to support OpenGL` / `EXCEPTION_ACCESS_VIOLATION` com frame `# C  \[(ig\|atio\|nvoglv)` | driver de vídeo/OpenGL (65542 = `GLFW_API_UNAVAILABLE`) | LWJGL 2 (≤1.12) / LWJGL 3 |
| 33 | `Unrecognized VM option '(.+)'` / `Unrecognized option: (.+)` | argumento JVM inválido para esse Java (ex.: flag de GC removida) | todos |
| 34 | `com\.electronwill\.nightconfig\.core\.io\.ParsingException` / `Failed loading config file (.+?) of type .+? for modid (\S+)` | config TOML corrompida → restaurar a versão do pack | Forge/NeoForge |
| 35 | `NoClassDefFoundError: (\S+)` / `ClassNotFoundException: (\S+)` | dependência ausente ou mod para outra versão; mapear pacote → mod lendo os JARs | todos |

Fontes das mensagens: Fabric Loader `Messages.properties` e `ModResolver.java`
(https://github.com/FabricMC/fabric-loader/blob/master/src/main/resources/net/fabricmc/loader/Messages.properties);
Forge `ModSorter.java`, `UniqueModListBuilder.java`, `en_us.json`
(https://github.com/MinecraftForge/MinecraftForge/tree/HEAD/fmlloader/src/main/java/net/minecraftforge/fml/loading);
NeoForge FancyModLoader `en_us.json`/`ModLoadingException.java` (https://github.com/neoforged/FancyModLoader);
class file versions no JVMS (https://docs.oracle.com/javase/specs/jvms/se25/html/jvms-4.html);
GLFW (https://www.glfw.org/docs/latest/group__errors.html).

**Além de regex**, várias checagens são melhores **antes** de lançar (pré-flight, sem log) **[inferência]**:
ler `fabric.mod.json` / `quilt.mod.json` / `META-INF/mods.toml` / `META-INF/neoforge.mods.toml` /
`mcmod.info` de cada JAR para obter id, versão, dependências, loader e versão do MC; detectar duplicatas
por id; JAR de loader errado; dependências obrigatórias ausentes; major version das classes vs Java
escolhido. Essa camada é exatamente a "camada determinística" prevista nos requisitos do Warden e
reduz chamadas à IA.

### 6.4 Ferramentas existentes (referência) e licenças

| Ferramenta | Licença | O que oferece | Uso pelo Warden |
|---|---|---|---|
| **mclo.gs** (`aternosorg/mclogs`) + **codex** (`aternosorg/codex`) + **codex-minecraft** | **MIT** **[verificado]** | biblioteca PHP: *Detective* (tipo de log), *Parser*, *Analyser* produzindo *Problems* com *Solutions*; regras Fabric (DuplicateMod, EntryStage, IncompatibleMods, Mixin, ModDependency...), Forge (ModDependency, ModDuplicate, LanguageProviderVersion, MultipleModulesExport, WorldMissingMod...), vanilla (TickingEntity...) | **pode ser portado** para Rust mantendo o aviso de copyright; melhor fonte de regras |
| **HMCL** `CrashReportAnalyzer` | GPL-3.0 | ~60 regras nomeadas (TOO_OLD_JAVA, JDK_9, GRAPHICS_DRIVER, OUT_OF_MEMORY, DUPLICATED_MOD, MODMIXIN_FAILURE...) | só leitura (não copiar) |
| **Refraction** (bot do Discord do Prism) | GPL-3.0 | ~30 regras (`issues.rs`, em Rust) | só leitura |
| **Prism** upload de logs | GPL-3.0 | envia para mclo.gs por padrão | referência de UX |
| **Not Enough Crashes** | MIT | mod que mostra tela de crash e identifica o mod suspeito | referência |
| **Crash Assistant** | licença própria "All Rights Reserved" (**correção da tarefa D4:** é a KostromDan's Modded Minecraft License 1.1.3, com cláusula de não concorrência; ver R5A §1.2 e ADR-0033) | GUI pós-crash, upload, auto-fixes | só referência de UX; não portar; o Warden só inclui o mod no pack por referência e nunca usa o código dele |

Fontes: https://github.com/aternosorg/codex-minecraft, https://github.com/aternosorg/codex,
https://github.com/aternosorg/mclogs, https://github.com/HMCL-dev/HMCL/blob/main/HMCLCore/src/main/java/org/jackhuang/hmcl/game/CrashReportAnalyzer.java,
https://github.com/PrismLauncher/refraction/blob/main/src/handlers/event/analyze_logs/issues.rs,
https://github.com/natanfudge/Not-Enough-Crashes, https://github.com/KostromDan/Crash-Assistant/blob/HEAD/LICENSE.md.

---

## 7. Sincronização instância de teste ↔ pack do packwiz

### 7.1 packwiz-installer (pack → instância)

Fatos **[verificado no código, branch `main`; licença MIT]** (https://github.com/packwiz/packwiz-installer,
`Main.kt`, `UpdateManager.kt`, `metadata/ManifestFile.kt`):
- Último release **v0.5.14 (2024-04-21)**, último push 2024-06 — manutenção baixa **[verificado]**.
  Requer Java ≥ 8 (o Warden já terá um Java gerenciado). O `packwiz-installer-bootstrap` baixa/atualiza o
  installer (`--bootstrap-no-update` para não consultar a rede).
- CLI: `java -jar packwiz-installer-bootstrap.jar [-g] [-s client|server] [--pack-folder <dir>] [--meta-file packwiz.json] <pack>`.
- **Fonte do pack**: aceita `http(s)://`, URI `file:` **e caminho local simples** (outros esquemas dão erro)
  **[verificado]** em `Main.kt`. Ou seja, **não é preciso `packwiz serve`** para instalar a partir do
  repositório local do Warden. (`packwiz serve` sobe HTTP em `:8080` com refresh automático do índice a
  cada request — alternativa útil para depurar.)
- **Estado** em `<instância>/packwiz.json`: `packFileHash`, `indexFileHash`, `cachedSide` e `cachedFiles`
  (caminho → `{hash, linkedFileHash, cachedLocation, isOptional, optionValue, onlyOtherSide}`).
- **Algoritmo** (resumo): se o side mudou, invalida tudo; arquivos do cache que sumiram do disco são
  baixados de novo (só checa existência, **não** o hash); se o hash do `pack.toml` não mudou e nada foi
  invalidado, termina; arquivos que **saíram do index são apagados** (só os que o próprio installer
  instalou); para cada arquivo do index, se o hash do cache bate com o do index, não toca; senão,
  compara com o disco e **sobrescreve** se divergir — exceto se `preserve = true` e o arquivo já existir.
- **Consequências [inferência]**: (1) edição local num config cujo hash no pack não mudou é mantida
  (o installer nem olha); (2) se o pack mudar esse arquivo, a edição local é **sobrescrita sem aviso**, a
  menos que tenha `preserve`; (3) arquivos fora do índice nunca são tocados (configs gerados pelos mods
  ficam). Isso é adequado para o Warden **desde que** o Warden faça o "trazer de volta" antes de
  reinstalar.
- Campos do `index.toml` **[fonte]** https://packwiz.infra.link/reference/pack-format/index-toml/:
  `hash-format` (o packwiz gera sha256), `files[].file`, `hash`, `alias`, `metafile` (true para `.pw.toml`),
  `preserve` ("não sobrescrever se já existe"). Formatos de hash: sha1, sha256, sha512, md5, murmur2
  (fingerprint do CurseForge). Não há comando de CLI para marcar `preserve` — editar o TOML **[inferência]**.

### 7.2 Opções para materializar o pack na instância

| Opção | Prós | Contras |
|---|---|---|
| **A. packwiz-installer** (caminho local) | comportamento idêntico ao dos jogadores finais (o que é testado é o que será instalado); zero reimplementação; MIT | dependência Java extra; manutenção baixa; pouco controle de progresso/erros (saída de texto); downloads de mods CurseForge com distribuição bloqueada exigem passo manual |
| **B. Implementação nativa em Rust** do mesmo algoritmo (ler `pack.toml` → `index.toml` → metafiles → baixar/copiar, manter manifesto) | controle total, progresso na UI, reaproveita o downloader e o cache de mods entre instâncias, integra com o snapshot/diff | precisa replicar fielmente a semântica (side, optional, preserve, hash-formats); risco de divergir do installer |
| **C. Cópia direta do repositório + downloads** sem manifesto | simples | não remove arquivos que saíram do pack; sem `preserve` |

**Recomendação [inferência]**: começar com **B**, porque o Warden já terá downloader, cache e
integração com Modrinth/CurseForge, e o "trazer de volta" precisa do mesmo manifesto; manter um **teste
de conformidade** que instala o mesmo pack com o packwiz-installer real e compara a árvore de arquivos
resultante (garante que o pack exportado funciona para os usuários finais). Usar A como fallback/
validação. Em qualquer caso, gravar o manifesto em `<instância>/.warden/state.json` (e não depender do
`packwiz.json`).

### 7.3 Detectar mudanças feitas durante o teste (instância → pack)

Referências: o Prism exporta instâncias com árvore de seleção e ignora por padrão `logs`,
`crash-reports`, `.cache`, `.fabric`, `.quilt`, `.DS_Store`, `thumbs.db`, gravando a seleção em
`.packignore`; os pedidos de suporte a packwiz no Prism (#2117, #591) seguem abertos — não há ferramenta
consolidada de sync bidirecional para packwiz **[fonte]**
https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/ui/dialogs/ExportInstanceDialog.cpp,
https://github.com/PrismLauncher/PrismLauncher/issues/2117. O `.mrpack` usa `overrides/`,
`client-overrides/`, `server-overrides/` (https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack).

Algoritmo proposto **[inferência]**:

1. **Materializar** o pack (7.2) e fazer um **boot de aquecimento** opcional (o jogo gera os configs
   padrão dos mods). Tirar o **snapshot de baseline**: `{caminho, tamanho, mtime, sha256, origem: pack | gerado}`
   para todo arquivo fora da lista de ignore (seção 5.3), mais o **conteúdo** dos arquivos de texto
   pequenos de `config/`, `defaultconfigs/`, `kubejs/`, `options.txt` (necessário para diff semântico).
2. **Testar** (jogo rodando). Opcional: *file watcher* (`notify` crate) só para mostrar ao vivo "arquivos
   alterados"; a fonte da verdade é o diff ao final.
3. **Após o jogo fechar**: novo snapshot; filtro rápido por `tamanho+mtime`, confirmação por sha256;
   classificar em **novos / alterados / removidos**.
4. **Separar mudança real de reformatação**: parsear antes/depois e comparar a árvore de valores,
   ignorando comentários, espaços e ordem de chaves — TOML com `toml_edit` (preserva formatação ao
   escrever de volta), JSON/JSON5 com `serde_json`/`json5`, `.properties`, `.cfg` do Forge 1.12
   (`S:chave=valor`, listas `< >`) com parser próprio, SNBT com parser próprio. Se só mudou a formatação,
   atualizar o baseline em silêncio. Correções automáticas do Forge aparecem no log como
   `Configuration file ... is not correct. Correcting` / `Incorrect key ... was corrected from ... to its default`
   — tratar como "gerado" **[fonte]** https://github.com/MinecraftForge/MinecraftForge/blob/HEAD/fmlcore/src/main/java/net/minecraftforge/fml/config/ConfigFileTypeHandler.java.
   O Forge copia `defaultconfigs/` só quando o config ainda não existe; um config novo idêntico ao de
   `defaultconfigs/` é "gerado".
5. **`options.txt`**: parsear `chave:valor` (https://minecraft.wiki/w/Options.txt). Ignorar chaves
   voláteis (`version`, `lastServer`, `fullscreen`, `fullscreenResolution`, `overrideWidth`, `overrideHeight`,
   `tutorialStep`, `joinedFirstServer`, `onboardAccessibility`, `skipMultiplayerWarning`,
   `incompatibleResourcePacks`, `soundDevice`); perguntar antes de levar `resourcePacks`, `lang`,
   `guiScale`; propor `key_*` (keybinds) como intencionais. Sugerir `preserve = true` para `options.txt`
   no `index.toml` (o jogador mantém as próprias preferências após atualizações do pack).
6. **`mods/`**: nunca copiar JARs de volta. JAR novo → identificar por hash (Modrinth sha1/sha512;
   CurseForge murmur2) e sugerir adicioná-lo como metafile; JAR removido/renomeado para `.disabled` →
   sugerir remover/desativar no pack.
7. **Revisão pelo usuário**: diff por arquivo (aceitar/rejeitar, por hunk em TOML/JSON), copiar os aceitos
   para o repositório do pack e rodar `packwiz refresh`.
8. **Conflitos**: se o pack mudou um arquivo (no editor do Warden) **e** a instância também mudou, fazer
   merge three-way (base = baseline, local = instância, remoto = pack) e marcar conflito — o
   packwiz-installer sobrescreveria sem avisar.

---

## 8. Implicações para o Warden

### 8.1 Requisitos derivados (priorizados)

**P0 — sem isso o "Testar" não funciona**

1. **Resolver de versões a partir de fontes primárias**: piston-meta (manifesto + JSON por versão,
   cache por SHA-1), meta.fabricmc.net, Maven do Forge (`promotions_slim.json` + `maven-metadata.xml`) e
   Maven do NeoForge (API `/api/maven/versions/...`). Ordenar versões do MC pela ordem do manifesto /
   `releaseTime`, **nunca** por semver (ids `26.x`, snapshots `26.4-snapshot-2`).
2. **Modelo de perfil único** (formato Mojang + `inheritsFrom`) com merge determinístico (seção 3.1),
   avaliação de `rules` (`os.name`, `os.arch`, `os.version` regex, `os.versionRange`, `features`) e
   tratamento seguro de campos desconhecidos (regra não casa + aviso no log).
3. **Downloader** concorrente, com verificação SHA-1/SHA-256/tamanho, retomada, escrita atômica
   (`.part` → rename), store de assets por hash compartilhado e store de bibliotecas compartilhado
   entre instâncias.
4. **Natives** nas três gerações (extrair LWJGL 2 / LWJGL 3 antigo com `extract.exclude`; colocar no
   classpath as natives modernas) em pasta **por versão, fora do gameDir**.
5. **Assets**: objetos por hash; suporte a `virtual` (`assets/virtual/legacy`) e `map_to_resources`
   (`<gameDir>/resources`) para ≤1.6.4.
6. **Montador de linha de comando** com todos os placeholders da seção 1.9, ambos os formatos de
   argumentos, regra de `logging` (seção 3.1), deduplicação de bibliotecas (maior versão vence) e
   `@argfile` quando Java ≥ 9 e a linha passar de ~30 000 caracteres.
7. **Gerência de Java**: Temurin (Adoptium API, SHA-256) como padrão, runtime da Mojang como
   alternativa, detecção de Javas do sistema, validação executando o binário, seleção por regras
   (Forge legado → 8; Forge 1.16.5 < 36.2.26 → 8 ≤ u312; JSON `javaVersion`; 16 → 17; ausente → 8).
8. **Instalação de loaders**: Fabric (perfil do meta), Forge legado (`versionInfo` + JAR `universal`),
   Forge/NeoForge com processors (executor próprio em processo Java separado, idempotente por
   `outputs`/SHA-1, cache por par MC×loader), denylist (`1.12.2-14.23.5.2851`).
9. **Perfil offline** (seção 4): nome validado, UUID offline determinístico, token `"0"`,
   `userType`/`user_properties` corretos por versão, `--offlineDeveloperMode` em 1.21.9+.
10. **Processo do jogo**: `javaw.exe` com `cwd = gameDir`, stdout/stderr por pipes, leitura
    incremental com decodificação tolerante, parser log4j XML + texto, captura de exit code, botão
    "matar processo" (encerrar a árvore de processos no Windows).

**P1 — qualidade do teste e do diagnóstico**

11. **Diagnóstico determinístico pós-crash** (seção 6): coletar `latest.log`, `debug.log`,
    `crash-reports/` novos, `hs_err_pid*.log` novos e a saída capturada; aplicar o catálogo de
    padrões; só então oferecer envio à IA (com consentimento e redação de dados pessoais).
12. **Checagens pré-lançamento** (antes do "Testar"): Java compatível com MC/loader; major version das
    classes dos JARs de mods ≤ Java escolhido; mods de loader errado (ex.: `fabric.mod.json` sem
    `mods.toml` num pack Forge); duplicatas pelo id do mod; memória máxima vs RAM física.
13. **Sincronização pack → instância** (seção 7): materialização a partir do repositório packwiz
    local, com manifesto de estado próprio (`.warden/state.json`), respeitando `side`, opcionais e
    `preserve`, removendo o que saiu do pack.
14. **Captura instância → pack**: snapshot antes do teste, diff semântico depois, revisão pelo usuário.
15. **Quick Play** (1.20+): "Testar e abrir o mundo de teste" com `--quickPlaySingleplayer`.
16. **Legacy Fabric** para 1.7.10/1.12.2 (o requisito "Fabric em qualquer versão" depende disso),
    condicionado à questão em aberto 4 (suporte no packwiz).

**P2 — conveniências e robustez**

17. Wrapper Java próprio de lançamento (só se necessário).
18. Babric/Quilt e versões ≤1.6.4.
19. Modo "Java moderno para 1.7.10/1.12.2" (lwjgl3ify/Cleanroom).
20. Login Microsoft opcional (seção 4).

### 8.2 Arquitetura proposta do launcher (crates/módulos Rust)

```
warden-launcher/            (crate de biblioteca, sem dependência de Tauri)
├── meta/                   fontes de metadados (piston-meta, fabric, forge, neoforge), cache por SHA-1
│   ├── mojang.rs           manifesto, JSON de versão, asset index, java-runtime
│   ├── fabric.rs           perfis v2 (e legacyfabric no futuro)
│   ├── forge.rs            promotions, maven-metadata, leitura de installers (3 gerações)
│   └── neoforge.rs         API maven do NeoForge
├── profile/                modelo do JSON de versão, merge inheritsFrom, rules, dedupe
├── download/               fila concorrente, hashes, retomada, escrita atômica, progresso
├── store/                  layout em disco: libraries/, assets/, versions/, natives/, runtimes/
├── java/                   descoberta, validação, download (Temurin/Mojang), política de seleção
├── install/                orquestra: vanilla → loader → processors → verificação final
│   └── processors.rs       executor de processors (processo Java separado)
├── launch/                 montagem de args, placeholders, @argfile, spawn, pipes, kill
├── auth/                   perfil offline (e MSA futuramente)
├── logs/                   parser log4j XML/texto, stream de eventos para a UI, redação de PII
├── diagnose/               catálogo de padrões determinísticos, coleta de artefatos de crash
└── instance/               gameDir, materialização do pack, snapshot/diff de configs
```

Princípios **[inferência/recomendação]**:
- A crate de launcher **não conhece o Tauri**: expõe API assíncrona (tokio) com eventos de progresso
  tipados e cancelamento (`CancellationToken`); o app Tauri só faz a ponte com a UI. Isso permite testar
  tudo com `cargo test` e rodar testes de integração em CI.
- **Erros tipados** por etapa (`thiserror`), sempre com contexto (URL, caminho, hash esperado/obtido).
- **Idempotência**: cada etapa de instalação verifica o que já existe (hash) e só baixa/processa o
  que falta; reinstalar nunca corrompe uma instância.
- **Nada da Mojang é redistribuído**: tudo é baixado dos servidores oficiais no computador do usuário.

### 8.3 Fazer vs reutilizar

| Módulo | Recomendação | Justificativa |
|---|---|---|
| Modelos de JSON (versão, biblioteca, rules, processors) | **Escrever**, consultando `daedalus` (MIT) e portablemc (Apache-2.0) | núcleo do produto; formato estável e pequeno |
| Núcleo de install/launch | **Decisão a tomar com protótipo**: (A) usar **portablemc** como dependência; (B) escrever próprio, usando portablemc como referência | portablemc já cobre Forge legado e processors e é Apache-2.0; o risco é acoplamento à API dele e menos controle sobre progresso/eventos/erros. Sugestão: protótipo de 1–2 dias com portablemc nas 4 faixas de teste; se a API atender, adotar; senão, escrever próprio |
| Executor de processors | Escrever (≈ 200–400 linhas) ou via portablemc | algoritmo simples e bem definido (seção 3.4) |
| ForgeWrapper | **Não** usar como padrão; manter como plano B | adiciona JAR e lógica na JVM do jogo; útil se o executor próprio falhar em alguma versão |
| Download de Java | Escrever (Adoptium API + manifesto Mojang) | pouco código; controle de UX |
| Parser log4j XML | Escrever (`quick-xml` em modo streaming) | simples |
| Catálogo de diagnósticos | Escrever, com regras próprias inspiradas em ferramentas existentes (seção 6), respeitando licenças | diferencial do Warden |
| Materialização do pack | **Escrever** em Rust a mesma semântica do packwiz-installer (seção 7.2, opção B), com teste de conformidade contra o packwiz-installer real (que aceita caminho local) | reaproveita downloader/cache e o manifesto serve ao "trazer de volta" |
| Códigos GPL (Prism, Modrinth app-lib, HMCL, ATLauncher) | **Somente leitura/estudo**, nunca copiar | evitar contaminação de licença enquanto a licença do Warden não for decidida |

### 8.4 Riscos

| Risco | Impacto | Mitigação |
|---|---|---|
| Mudanças de formato da Mojang (ex.: `versionRange`, `default-user-jvm`, natives em subpastas em 26.x) | lançamento quebra em versões novas | parser tolerante; testes de contrato diários/semanais contra o manifesto real; alertas |
| Instaladores do Forge mudando processors | instalação falha | executor genérico guiado pelo `install_profile`; teste de integração por versão "recomendada" |
| Limite de 32 767 caracteres na linha de comando (Windows) | jogo não abre em packs grandes/Forge moderno | caminhos curtos, `@argfile` (Java ≥ 9) |
| Antivírus/SmartScreen bloqueando `javaw.exe` baixado ou arquivos em uso | falhas intermitentes no Windows | escrita atômica, retries com backoff, mensagens claras |
| Java 8 da Mojang (8u51) antigo | problemas de TLS/segurança | Temurin 8 atualizado como padrão |
| Endpoints de terceiros indisponíveis (Adoptium, Fabric meta, Maven do Forge) | não instala | cache local; fonte alternativa (Mojang runtime); mensagens de erro específicas |
| Questões legais do modo offline | reputação/takedown | postura da seção 4 |
| Espaço em disco (assets ~0,5–0,8 GB por índice moderno, runtimes ~100 MB cada) | reclamações | store compartilhado, limpeza de órfãos |

### 8.5 Ordem de implementação sugerida

1. **Vanilla moderno (1.20.1/1.21.1)**: manifesto → JSON → libs → natives (classpath) → assets →
   Java (Temurin 17/21) → args modernos → spawn + logs. Critério: chegar ao menu principal e criar mundo.
2. **Vanilla 1.12.2 e 1.7.10**: `minecraftArguments`, natives LWJGL 2 extraídas, `--userProperties {}`,
   Java 8. Critério: menu + mundo.
3. **Fabric** (1.20.1 e 1.21.1): perfil do meta. Critério: Fabric API + um mod Mixin simples carregam.
4. **Forge legado** (1.7.10 e 1.12.2): `versionInfo`/`version.json` spec 0 sem processors, JAR universal.
5. **Forge/NeoForge modernos**: executor de processors; 1.16.5 (ModLauncher), 1.20.1 (BSL), 1.21.1
   NeoForge (BSL) e NeoForge recente (`net.neoforged.fml.startup.Client`).
6. Diagnóstico determinístico + captura de crash.
7. Materialização do pack e captura de mudanças.
8. Legacy Fabric (1.7.10/1.12.2) e faixas extras (≤1.6.4 com assets legados, Quilt).

### 8.6 Estratégia de teste por faixa de versão

Executar o cliente em CI é viável: o projeto **mc-runtime-test** (MIT, ativo em 2026-08) roda o
cliente Minecraft em pipelines usando **HeadlessMC** (MIT) + **Xvfb**, com um mod leve que entra num
mundo singleplayer, espera os chunks carregarem e sai; declara suporte a Forge 1.7.10–26.2,
Fabric 1.16.5–26.2 (1.7.10/1.12.2 limitados) e NeoForge 1.20.2+ **[verificado]**
(https://github.com/headlesshq/mc-runtime-test, https://github.com/3arthqu4ke/headlessmc). O HeadlessMC
também ilustra a postura legal: "will not allow you to play without having bought Minecraft! Offline
accounts can only be used to run the game headlessly in CI/CD pipelines". Para o Warden **[inferência]**:
smoke tests do jogo em runner **Linux** com Xvfb (+ Mesa llvmpipe para OpenGL por software) validam o
pipeline completo; a parte específica de Windows (caminhos, `javaw.exe`, limite de linha de comando,
natives Windows) é coberta por testes unitários/golden em runner Windows e por smoke test manual ou em
runner self-hosted com GPU. Matriz mínima:

| Faixa | Combinações | O que testar automaticamente | Sinal de sucesso |
|---|---|---|---|
| 1.7.10 | vanilla; Forge 10.13.4.1614 | args antigos, `--userProperties {}`, natives LWJGL 2 (incl. `${arch}`), Java 8, JAR universal | log contém `LWJGL Version` / FML `Forge Mod Loader has successfully loaded` **[inferência sobre a string exata]** |
| 1.12.2 | vanilla; Forge 14.23.5.2859/2860 | lwjgl por SO, spec 0 sem processors, launchwrapper, `logging: {}` | `Forge Mod Loader ... loaded` + tela de título |
| 1.16.5 | vanilla; Forge 36.2.34; Fabric | LWJGL 3 extraído, ModLauncher, processors spec 0, regra Java 8 ≤ u312 para Forge antigo | `Launching target 'fmlclient'` |
| 1.20.1 | vanilla; Fabric; Forge 47.x; NeoForge 47.1.x | natives no classpath, BSL + module path, processors spec 1, Quick Play | `Launching target 'forgeclient'`/Fabric `Loading N mods` |
| 1.21.1 | vanilla; Fabric; NeoForge 21.1.x | Java 21 | idem |
| 26.x | vanilla; NeoForge 26.x | `default-user-jvm`, `versionRange`, natives em subpastas, Java 25 | idem |

Camadas de teste:
1. **Unitários (sem rede)**: avaliação de `rules`, merge de perfis, substituição de placeholders,
   montagem de classpath, parsing dos 3 formatos de `install_profile`, UUID offline, parser de log —
   usando **fixtures** (JSONs reais congelados no repositório de testes).
2. **Golden tests**: para cada combinação da matriz, gerar a linha de comando final e comparar com um
   "golden" revisado; comparar também com o perfil que o Prism gera (apenas como oráculo de teste).
3. **Integração com rede (agendado)**: instalar de verdade cada combinação, verificar hashes e que os
   processors produzem `outputs` com o SHA-1 esperado.
4. **Smoke test com o jogo** (manual ou runner com GPU): lançar, esperar o marcador de sucesso no
   log, encerrar. Para crash, um conjunto de **packs de teste propositalmente quebrados** (mod com
   dependência ausente, mod Fabric no Forge, mod compilado para Java 21 no 1.20.1/Java 17, dois mods
   duplicados, `-Xmx256m` para OOM) para validar o diagnóstico determinístico.

---

## 9. Questões em aberto

1. **Licença do Warden** (MIT/proprietária vs GPL-3.0): define se é possível incorporar código de
   Prism/Modrinth app-lib/HMCL ou só de projetos MIT/Apache (portablemc, daedalus, codex). Precisa de
   decisão antes de escolher dependências.
2. **portablemc como dependência ou só referência?** Requer protótipo curto nas faixas 1.7.10, 1.12.2,
   1.20.1 e 1.21.1 (Windows) avaliando API, progresso, cancelamento e erros.
3. **Verificação de posse (login Microsoft)**: o Warden vai exigir uma conta que possua o jogo antes do
   modo offline (modelo Prism/HMCL/MultiMC)? Exige registrar app no Azure e aprovação manual da Mojang
   (formulário `aka.ms/mce-reviewappid`); sem isso, o Warden fica no modelo "offline apenas, com avisos",
   que é área cinzenta (seção 4.2).
4. **"Fabric em qualquer versão"**: para 1.7.10/1.12.2 isso significa Legacy Fabric (meta próprio, não
   verificado nesta pesquisa por timeout). Confirmar se o **packwiz** representa Legacy Fabric em
   `pack.toml` (`[versions]`) de forma que o packwiz-installer e os launchers dos jogadores entendam —
   provável lacuna; investigar na tarefa sobre o packwiz.
5. **Versões ≤1.6.4**: o requisito "qualquer versão" inclui alpha/beta/1.5.2/1.6.4 (assets `pre-1.6`/
   `legacy`, `--session`, applet via launchwrapper)? Recomendo declarar suporte oficial a partir de
   1.7.10 e tratar anteriores como "melhor esforço".
6. **Fonte padrão de Java**: Temurin (recomendado aqui) vs runtime da Mojang; e se o Warden deve
   oferecer Java moderno para 1.7.10/1.12.2 via lwjgl3ify/Cleanroom no futuro.
7. **`-Dfile.encoding=UTF-8` em 1.7.10/1.12.2**: pode mudar como mods antigos leem/escrevem configs com
   acentos; validar com packs reais antes de ativar por padrão.
8. **Mods do CurseForge com distribuição de terceiros bloqueada**: a materialização (nativa ou via
   packwiz-installer) precisará de fluxo de download manual; pertence à tarefa de fontes de mods, mas
   afeta o "Testar".
9. **Metadados de terceiros** (`launcher-meta.modrinth.com`, `meta.prismlauncher.org`): usar apenas
   como oráculo de testes ou pedir permissão formal se algum dia forem usados em produção.
10. **CI com GPU**: definir se haverá runner self-hosted Windows com GPU para smoke tests reais, ou se
    bastam Linux + Xvfb/Mesa (mc-runtime-test/HeadlessMC) + testes unitários/golden em Windows.
11. **Hosts `.invalid` em 1.16.4/1.16.5**: aplicar ou não o contorno para reabilitar o botão de
    multiplayer (útil para testar em LAN/servidor local) — decidir à luz da postura legal (seção 4.5).
12. **Saves de teste**: o Warden deve manter um "mundo de teste" persistente por pack (ex.: para Quick
    Play) e permitir exportar um mundo modelo explicitamente? Hoje `saves/` é tratado como Runtime.

