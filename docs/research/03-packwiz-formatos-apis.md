# R3 — packwiz, formatos de pack, APIs de mods e formatos de config

> Documento de pesquisa do projeto Warden. Data da pesquisa: 2026-10-01.
> Base de código analisada: `packwiz/packwiz` no commit `ef87d96` (2026-09-06), `packwiz/packwiz-installer` no commit `7420866` (2024-04-21) e `packwiz/packwiz-website` (docs).
> Os comportamentos marcados como **[testado]** foram reproduzidos localmente com um binário do packwiz compilado a partir desse commit (Go 1.26, Linux). O que estiver marcado como **[inferência]** não foi confirmado em fonte primária.

## Sumário

1. [packwiz em profundidade](#1-packwiz-em-profundidade)
2. [Formatos de exportação: .mrpack e zip da CurseForge](#2-formatos-de-exportação-mrpack-e-zip-da-curseforge)
3. [APIs: Modrinth v2 e CurseForge](#3-apis-modrinth-v2-e-curseforge)
4. [Metadados de compatibilidade dentro dos .jar](#4-metadados-de-compatibilidade-dentro-dos-jar)
5. [Formatos de config usados por mods](#5-formatos-de-config-usados-por-mods)
6. [Implicações para o Warden](#6-implicações-para-o-warden)
7. [Questões em aberto](#7-questões-em-aberto)

---

## 1. packwiz em profundidade

### 1.1 Visão geral e arquitetura interna

- Repositório: <https://github.com/packwiz/packwiz> — licença **MIT** (arquivo `LICENSE`, "Copyright (c) 2019"). Escrito em Go (`go 1.23.0`, toolchain `go1.24.6` no `go.mod`).
- CLI baseada em **cobra** + **viper** (`cmd/root.go`). Cada fonte é um "módulo" Go registrado por `import _` em `main.go`: `curseforge`, `github`, `migrate`, `modrinth`, `settings`, `url`, `utils`.
- Núcleo em `core/`:
  - `core/pack.go` — struct `Pack` (pack.toml), `CurrentPackFormat = "packwiz:1.1.0"`.
  - `core/index.go` + `core/indexfiles.go` — `Index` (index.toml), `Refresh()`, regras de ignore padrão.
  - `core/mod.go` — struct `Mod` (metafile `.pw.toml`).
  - `core/hash.go` — algoritmos de hash suportados.
  - `core/download.go` — sessão de download com cache endereçado por conteúdo.
  - `core/versionutil.go` — descoberta de versões de loaders (Maven do Fabric/Quilt/Forge/NeoForge/LiteLoader).
- Extensibilidade interna por dois registros globais (`core/interfaces.go`): `core.Updaters` (chave = nome da seção em `[update.<nome>]`, hoje `modrinth`, `curseforge`, `github`) e `core.MetaDownloaders` (chave = sufixo de `mode = "metadata:<nome>"`, hoje só `curseforge`).

Fonte: código em <https://github.com/packwiz/packwiz/tree/main/core>.

### 1.2 Flags globais (valem para todos os subcomandos)

Definidas em `cmd/root.go`:

| Flag | Padrão | Efeito |
|---|---|---|
| `--pack-file <path>` | `pack.toml` | Caminho do pack.toml. O index é resolvido relativo à pasta dele. Permite rodar o packwiz **sem `cd`** na pasta do pack (útil para subprocesso). Atenção: issue #265 relata que `curseforge add` não respeitava essa flag em algum momento. |
| `--meta-folder <dir>` (alias `--mods-folder`) | vazio = por categoria | Pasta onde novos `.pw.toml` são criados. |
| `--meta-folder-base <dir>` | `.` | Base para resolver `meta-folder`. |
| `--cache <dir>` | `os.UserCacheDir()/packwiz/cache` (Windows: `%LocalAppData%\packwiz\cache`) | Cache de downloads (ver 1.9). |
| `--config <file>` | `os.UserConfigDir()/packwiz/.packwiz.toml` (Windows: `%AppData%\packwiz\.packwiz.toml`; Linux: `$XDG_DATA_HOME/packwiz` tem prioridade) | Config global do viper. |
| `-y`, `--yes` | `false` | **Modo não interativo**: todos os prompts aceitam o padrão/"sim". Na busca, **escolhe o primeiro resultado** (ver armadilha abaixo). |

Também é lido o ambiente: `viper.SetEnvPrefix("packwiz")` + `AutomaticEnv()` com `.`→`_`, ou seja, qualquer chave do viper pode ser passada como variável `PACKWIZ_<CHAVE>` (ex.: `PACKWIZ_NON_INTERACTIVE=true`, `PACKWIZ_CACHE_DIRECTORY=...`) **[inferência a partir do código do viper; não testado]**. Quando existe um arquivo de config global, o packwiz imprime `Using config file: ...` no stdout — mais uma linha que a UI precisa tolerar.

As opções em `[options]` do `pack.toml` são mescladas no viper em `LoadPack()` (`viper.MergeConfigMap(modpack.Options)`), então `acceptable-game-versions`, `meta-folder`, `meta-folder-base`, `no-internal-hashes` e `datapack-folder` podem viver no próprio pack (doc: <https://packwiz.infra.link/reference/additional-options/>).

> **Armadilha crítica de automação [testado]:** `packwiz cf add "not enough items" -y` num pack 1.7.10 adicionou **"NEI Addons"** (primeiro resultado da busca), não o NEI. Com `-y` a busca nunca deve ser usada; o Warden deve sempre passar IDs explícitos (`--project-id`/`--version-id` no Modrinth, `--addon-id`/`--file-id` na CurseForge) depois de o usuário escolher o resultado na UI do próprio Warden.

### 1.3 Referência de comandos

Todos os comandos abaixo foram lidos no código-fonte. "Prompt" indica onde o comando lê stdin (e, portanto, onde `-y` é obrigatório para automação).

| Comando (aliases) | Arquivo | O que faz | Flags úteis | Prompts |
|---|---|---|---|---|
| `init` | `cmd/init.go` | Cria `pack.toml` e `index.toml` vazio. Consulta `launchermeta.mojang.com/mc/game/version_manifest.json` e o Maven do loader. | `--name`, `--author`, `--version`, `--index-file`, `--mc-version`, `-l/--latest`, `-s/--snapshot`, `-r/--reinit`, `--modloader {fabric,forge,neoforge,quilt,liteloader,none}`, `--<loader>-version`, `--<loader>-latest` | nome, autor, versão, MC, loader, versão do loader (todos pulados por flags ou `-y`) |
| `refresh` | `cmd/refresh.go` | Reescaneia a pasta, recalcula hashes e reescreve `index.toml` e o hash do index em `pack.toml`. | `--build` (força hashes mesmo com `no-internal-hashes`) | — |
| `update [nome]` (`upgrade`) | `cmd/update.go` | Atualiza um metafile (pelo nome do arquivo `.pw.toml` sem extensão) ou todos. Respeita `pin`. | `-a/--all` | "Do you want to update? [Y/n]" (só em `--all`) |
| `remove <nome>` (`delete`, `uninstall`, `rm`) | `cmd/remove.go` | Apaga o `.pw.toml` e remove do index. **Não remove dependências órfãs** (issue #97). | — | — |
| `pin <nome>` / `unpin <nome>` (`hold`/`unhold`) | `cmd/pin.go` | Liga/desliga `pin = true` no metafile. | — | — |
| `list` | `cmd/list.go` | Lista nomes dos metafiles, ordenados. | `-v/--version` (imprime `Nome (filename)`), `-s/--side` | — |
| `rehash <sha1\|sha256\|sha512>` | `cmd/rehash.go` | Baixa todos os arquivos externos e troca o hash dos metafiles para o formato pedido. | — | — |
| `serve` (`server`) | `cmd/serve.go` | Servidor HTTP de desenvolvimento (padrão porta 8080) que serve `pack.toml`, `index.toml` e **somente** arquivos indexados; refaz o refresh a cada GET de `pack.toml`. Escuta em `:porta` (todas as interfaces). | `-p/--port`, `-r/--refresh` (padrão true), `--basic` | — |
| `modrinth add` (`mr`; `install`, `get`) | `modrinth/install.go` | Adiciona projeto por URL, slug, ID ou busca; resolve dependências **required** recursivamente (até 20 ciclos). | `--project-id`, `--version-id`, `--version-filename` | menu de busca; "Would you like to add them? [Y/n]" para dependências |
| `modrinth export` | `modrinth/export.go` | Gera `.mrpack` (ver seção 2). | `-o/--output`, `--restrictDomains` (padrão `true`) | — |
| `curseforge add` (`cf`, `curse`; `install`, `get`) | `curseforge/install.go` | Adiciona por URL, slug, ID ou busca; resolve dependências `relationType == 3` (required). | `--addon-id`, `--file-id`, `--game` (padrão `minecraft`), `--category` | menu de busca; prompt de dependências |
| `curseforge export` | `curseforge/export.go` | Gera zip da CurseForge. | `-s/--side` (padrão **`client`**), `-o/--output` | — |
| `curseforge import <arquivo>` | `curseforge/import.go` | Importa zip de modpack CF ou `minecraftinstance.json`. **HTTP não suportado.** | — | — |
| `curseforge detect` | `curseforge/detect.go` | "Experimental": calcula fingerprint murmur2 dos `.jar` em `mods/` e cria metafiles CF para os reconhecidos (endpoint `/v1/fingerprints`). | — | — |
| `curseforge open <nome>` (`doc`) | `curseforge/open.go` | Abre a página do projeto no navegador. | — | — |
| `url add <nome> <url>` (`install`, `get`) | `url/install.go` | Cria metafile para um link direto (http/https). Baixa o arquivo para calcular SHA-256. Recusa URLs do Modrinth/CurseForge sem `--force`. Pasta padrão: `mods`. Side sempre `both`. | `--force`, `--meta-name` | — |
| `github add <url\|owner/repo>` (`gh`) | `github/install.go` | Adiciona asset de GitHub Releases (módulo novo, merge de 2024-08). Ignora `-dev-preshadow.jar` etc. por padrão. | `--branch`, `--regex` | — |
| `migrate minecraft <versão>` (`mc`) | `migrate/minecraft.go` | Troca a versão do MC no pack.toml; depois pergunta se atualiza o loader e **todos os mods**. | — | 2 prompts — com `-y` roda `update --all` automaticamente |
| `migrate loader <versão\|latest\|recommended>` | `migrate/loader.go` | Troca a versão do loader (falha se houver mais de um loader). `recommended` só faz sentido no Forge (`promotions_slim.json`). | — | — |
| `settings acceptable-versions [lista]` | `settings/acceptable_versions.go` | Gerencia `options.acceptable-game-versions`. | `-a/--add`, `-r/--remove` | — |
| `utils markdown` | `utils/markdown.go` | Gera documentação Markdown da CLI. | `--dir` | — |

Observação: **não existe comando `packwiz install` no topo nem `packwiz add` genérico** — "add/install" são sempre subcomandos de uma fonte (`mr`, `cf`, `url`, `gh`). Arquivos locais (um `.jar` que o usuário arrastou) **não têm comando**: basta copiá-los para a pasta do pack e rodar `refresh` (ver 1.8).

### 1.4 Saída, códigos de saída e o que dá (ou não) para usar numa UI

Constatações do código e dos testes:

1. **Tudo vai para stdout** via `fmt.Println`/`Printf`, inclusive erros. Stderr praticamente não é usado (só panics do Go e o `cobra` em erro de argumentos).
2. **Código de saída**: erros fatais fazem `os.Exit(1)`; erros de parse de argumentos do cobra também saem com 1 (`cmd.Execute()`). Não há códigos distintos por tipo de erro.
3. **Há erros que saem com 0** (bugs):
   - `modrinth export`: se `index.Refresh()`/`Write()` falhar, o código faz `fmt.Println(err); return` → **exit 0** (`modrinth/export.go`).
   - `update --all`: falha de checagem/atualização de um mod individual só imprime e continua (comentários `// TODO: do we return err code 1?`).
   - Exportações: falha no download de um arquivo específico é impressa ("Download of X failed") e o export continua, gerando um pack **incompleto** com exit 0.
   - `cmdshared.AddNonMetafileOverrides` ignora erros de leitura de arquivos (`// TODO: exit(1)?`).
4. **Mods da CurseForge com download bloqueado** fazem export/rehash imprimir a lista de "manual downloads" e sair com 1 (`cmdshared.ListManualDownloads`).
5. **Barra de progresso com ANSI** (`mpb`) é impressa mesmo sem TTY **[testado]**: `ESC[1A ESC[J Refreshing index... 0 % [---...] done`. A UI precisa remover sequências ANSI e linhas de progresso.
6. **Mensagens úteis para a UI** (strings estáveis no código, mas sem garantia de estabilidade):
   - `Project "<Título>" successfully added! (<filename>)` e `Dependency "<Título>" successfully added! (<filename>)`.
   - `Warning: Project (<slug>) is marked for both server and client...` (ambiente `client_or_server` no Modrinth).
   - `Failed to add project: no projects found` / `mod not available for the configured Minecraft version(s)...` / `no valid versions found`.
   - `Can't find this file; please ensure you have run packwiz refresh and use the name of the .pw.toml file (defaults to the project slug)`.
   - `Version is pinned; run the unpin command to allow updating`.
   - `Found N manual downloads; these mods are unable to be downloaded by packwiz...` seguido de `Nome (arquivo) from URL` e `Once you have done so, place these files in <cache>/import and re-run this command.`
   - `Warning: Modrinth versions for X inconsistent between latest version number and newest release date (a vs b)`.
   - `429 error type: 'ratelimit_error' - You are being rate-limited...` (repassado da API do Modrinth; issue #376).
7. A issue **#139 "Porcelain commands/machine-friendly input for automated usage"** está aberta sem implementação: **não existe saída JSON**. Qualquer integração por subprocesso depende de parsing de texto frágil.

Conclusão para a UI: o resultado de um comando deve ser determinado **relendo os arquivos TOML** depois da execução (diff do index/metafiles), e não interpretando o stdout. O stdout serve só para log e para mensagens de erro exibidas ao usuário.

### 1.5 Formato do `pack.toml`

Struct `core.Pack` (`core/pack.go`). Exemplo real gerado por `packwiz init` **[testado]**:

```toml
name = "T"
author = "A"
version = "1.0.0"
pack-format = "packwiz:1.1.0"

[index]
file = "index.toml"
hash-format = "sha256"
hash = "7b81c9e9d9e6e21a9865a2dded0dfd77e7683d679b98fe5b0114eaed9a31f17e"

[versions]
fabric = "0.19.5"
minecraft = "1.21.1"
```

| Campo | Tipo | Notas |
|---|---|---|
| `name` | string | obrigatório na prática; usado no nome do arquivo exportado (`<name>-<version>.mrpack`). |
| `author`, `version`, `description` | string, opcionais | `version` vazio gera aviso no export Modrinth (o `.mrpack` exige `versionId`). `description` vira `summary` no `.mrpack`. |
| `pack-format` | string | `packwiz:1.1.0` atual; `1.0.0` é migrado automaticamente; aceita `~1` (semver). Sem o campo, assume `1.1.0`. |
| `[index]` | `file`, `hash-format`, `hash` | caminho do index relativo ao pack.toml (barra normal). O hash é sempre recalculado em SHA-256. |
| `[versions]` | mapa string→string | chaves: `minecraft` (obrigatória), `fabric`, `quilt`, `forge`, `neoforge`, `liteloader`. Valores **sem prefixo do MC** para Forge (`14.23.5.2864`) e NeoForge 1.20.1 (`47.1.106`). |
| `[export.<formato>]` | mapa | hoje só `[export.curseforge] project-id = <u32>` (vai para `projectID` do manifest). |
| `[options]` | mapa | ver 1.2. |

Loaders "compatíveis" (`GetCompatibleLoaders`): se há `quilt`, aceita mods `quilt` e `fabric`; se há `neoforge`, aceita `neoforge` **e `forge`** (compatibilidade retroativa que, a partir de 1.20.2, pode trazer mods Forge que não rodam no NeoForge — risco para o diagnóstico). Ter mais de um loader é tolerado na leitura mas `migrate loader` recusa, e o packwiz não suporta packs multi-loader (issue #369).

#### 1.5.1 Bug confirmado: `init` com Forge em 1.7.10, 1.8.9, 1.9.4 etc. **[testado]**

O Maven do Forge lista versões antigas com sufixo de MC: `1.7.10-10.13.4.1614-1.7.10`. Em `core/versionutil.go`, `fetchForgeStyle` corta no primeiro `-` e guarda `10.13.4.1614-1.7.10`; depois `init` aplica `cmdshared.GetRawForgeVersion`, que pega o segundo pedaço após `-` (`1.7.10`) e compara com a lista — nunca bate. Resultado:

```
$ packwiz init -y --mc-version 1.7.10 --modloader forge --forge-latest
Given Forge version cannot be found!     (exit 1)
```

Testei também `--forge-version 10.13.4.1614`, `10.13.4.1614-1.7.10` e `1.7.10-10.13.4.1614-1.7.10`: todos falham. Afeta as versões do MC cujo Maven tem entradas com dois hífens: **1.7.2, 1.7.10, 1.8, 1.8.8, 1.8.9, 1.9, 1.9.4, 1.10, 1.10.2, 1.11, 1.11.2 e parte das builds de 1.12.2** (levantado do `maven-metadata.xml` em <https://maven.minecraftforge.net/releases/net/minecraftforge/forge/maven-metadata.xml>). A última build "recomendada" do 1.12.2 (`14.23.5.2864`/`2860`) não tem sufixo e funciona **[testado]**. Issue aberta: **#393 "Given Forge version cannot be found (1.7.10)"** (2026-04-13), sem correção. `migrate loader` tem o mesmo problema.

**Contorno [testado]:** `packwiz init --modloader none ...` e depois escrever `forge = "10.13.4.1614"` em `[versions]`. Com isso, `cf add journeymap` e `mr add angelica` funcionaram normalmente no pack 1.7.10. Portanto o Warden **deve gerar o `pack.toml` ele mesmo** (com sua própria lista de versões de loader), e não depender de `packwiz init`.

Outro bug de formato de loader: issue **#366** — `curseforge export` grava `neoforge-47.1.106` para NeoForge 1.20.1, mas a CurseForge espera `neoforge-1.20.1-47.1.106` (o packwiz remove o prefixo e não o recoloca). Vale só para o NeoForge "legado" de 1.20.1.

### 1.6 Formato do `index.toml`

Exemplo real **[testado]**:

```toml
hash-format = "sha256"

[[files]]
file = "config/test.properties"
hash = "fe3209d6d4f51935b391288a43df48d9ddece1a992597ae53387ca16611a9179"

[[files]]
file = "mods/sodium.pw.toml"
hash = "869af67848bf3b9ba4384bc9fb0cbe303d35263bd8de75eeabcba796edf4d7c6"
metafile = true
```

Campos de cada `[[files]]` (`core/indexfiles.go`):

| Campo | Significado |
|---|---|
| `file` | caminho relativo à pasta do index, sempre com `/`. |
| `hash` | hash do arquivo **em disco** (para metafiles, é o hash do `.pw.toml`, não do jar). Vazio se `no-internal-hashes`. |
| `hash-format` | só aparece quando difere do `hash-format` global do index. |
| `alias` | permite instalar o mesmo arquivo em outro caminho; **ignorado pelas exportações** (issue #228) e com remoção planejada. Não usar. |
| `metafile` | `true` para `.pw.toml` (o instalador baixa o arquivo descrito nele em vez de copiar o próprio TOML). |
| `preserve` | `true` = o packwiz-installer não sobrescreve o arquivo se ele já existir no destino (útil para configs que o jogador pode editar, ex.: `options.txt`). Não é setado por nenhum comando; só editando o index à mão — **e `refresh` mantém o campo** porque atualiza a entrada existente em vez de recriá-la (`updateFileEntry`). |

Como o hash/index funciona (`core/index.go`):

- `Refresh()` percorre a pasta do index com `filepath.WalkDir` (**não segue symlinks de diretório** — issue #191), pula diretórios ignorados inteiros, pula o próprio `pack.toml`, o `index.toml` e o `.packwizignore`, calcula SHA-256 de cada arquivo, marca `metafile` pelo sufixo `.pw.toml`, e **remove do index** as entradas cujo arquivo não existe mais. A ordenação é por caminho (determinística, bom para git).
- Depois o `pack.toml` recebe o SHA-256 do `index.toml`.
- Não há hashing paralelo (comentário `// TODO: If needed, multithreaded hashing`): packs com muitos arquivos de config grandes demoram.
- Os metafiles não guardam o hash "do index"; o hash do **jar** fica dentro do metafile (`[download] hash`).
- Algoritmos aceitos (`core/hash.go`): `sha1`, `sha256`, `sha512`, `md5`, `murmur2` (variante da CurseForge, impresso como inteiro decimal). Preferência para validar download: `murmur2 < md5 < sha1 < sha256 < sha512` (usa o "mais forte" disponível).

### 1.7 Formato dos metafiles `.pw.toml`

Struct `core.Mod` (`core/mod.go`). Exemplos reais **[testado]**:

Modrinth:
```toml
name = "Mod Menu"
filename = "modmenu-11.0.5.jar"
side = "client"

[download]
url = "https://cdn.modrinth.com/data/mOgUt4GM/versions/6lgOkclV/modmenu-11.0.5.jar"
hash-format = "sha512"
hash = "92e8e962...96c7"

[update]
[update.modrinth]
mod-id = "mOgUt4GM"
version = "6lgOkclV"
```

CurseForge:
```toml
name = "Jade 🔍"
filename = "Jade-1.21.1-Fabric-15.10.6.jar"
side = "both"

[download]
hash-format = "sha1"
hash = "bed67c605cbc991f474b270581cfa04ed48228df"
mode = "metadata:curseforge"

[update]
[update.curseforge]
file-id = 8591528
project-id = 324717
```

| Campo | Notas |
|---|---|
| `name` | nome exibido (vem do título do projeto; pode ter emoji). |
| `filename` | nome do arquivo final, relativo à pasta do metafile. Pode conter subpasta, mas `update` sobrescreve (issue #52). |
| `side` | `client`, `server`, `both` ou vazio (= `both`). |
| `pin` | `true` impede `update`. |
| `[download] url` | link direto (modo `url`, padrão). Ausente no modo CurseForge. |
| `[download] hash-format` / `hash` | hash do arquivo baixado. Modrinth: `sha512` preferido; CurseForge: `sha1`, senão `md5`, senão `murmur2`; `url add`: `sha256`. |
| `[download] mode` | vazio/`url` ou `metadata:curseforge` (URL resolvida em tempo de download pela API da CF, porque o termo de uso da CF não permite gravar o link — ver 3.2). |
| `[update.modrinth]` | `mod-id` (= project ID, nome legado) e `version` (= version ID). |
| `[update.curseforge]` | `project-id`, `file-id`. |
| `[update.github]` | dados do release do GitHub (módulo `github/`). |
| `[option]` | `optional` (bool), `description` (string), `default` (bool). Só o packwiz-installer apresenta escolha ao usuário. Nenhum comando de `add` seta `optional` (issues #404, #241); só `cf import` (para `required: false`) e edição manual. |

Nome do arquivo: `<slug>.pw.toml` (Modrinth/CurseForge) ou `SlugifyName(nome)` (`url add`). Pastas por tipo:

- Modrinth (`getProjectTypeFolder`): `mod` → `mods` (ou `plugins` para loaders de servidor); `resourcepack` → `resourcepacks`; `shader` → `shaderpacks` (ou `resourcepacks` se o loader do shader for `canvas`/`vanilla`); datapack → exige `datapack-folder`; `modpack` → erro.
- CurseForge (`defaultFolders`): classId 6 → `mods`, 12 → `resourcepacks`, 5 → `plugins`, 17 → `saves`; desconhecido (ex.: shaders, classId 6552) → **pasta raiz do pack** (issue #150 pede aviso).

**Sobrescrita silenciosa [testado]:** "If the file already exists, this will overwrite it!!!" (`createFileMeta`/`createModFile`). No teste 1.7.10, `cf add journeymap` criou `mods/unimixins.pw.toml` (dependência da CF) e em seguida `mr add angelica` sobrescreveu o mesmo arquivo com a versão do Modrinth. Não existe deduplicação entre fontes (issue #29, aberta desde 2019) nem detecção de "mesmo mod em outra fonte".

**Lado (side) a partir do Modrinth** — o packwiz já usa o novo campo `environment` da versão (commit `9066bf8`, 2026-09-02, "Use new modrinth environment data"), mapeando (`modrinth/modrinth.go:getSide`):

| `environment` (Modrinth) | side no packwiz |
|---|---|
| `client_only`, `singleplayer_only` | `client` |
| `dedicated_server_only` | `server` |
| `server_only`, `client_and_server`, `client_or_server_prefers_both` | `both` |
| `client_or_server` | `both` + aviso ("either") |
| `server_only_client_optional`, `client_only_server_optional` | **bug**: em Go `case` vazio não "cai" para o próximo, então retornam `""` e geram o aviso "unknown environment value" — o resultado final ainda é `both`. |

Na CurseForge e em `url add`, o side é sempre `both`; cabe ao usuário (ou ao Warden) ajustar.

### 1.8 `.packwizignore` e o que é ignorado por padrão

Sintaxe idêntica à de `.gitignore` (biblioteca `github.com/sabhiram/go-gitignore`). O arquivo fica na **pasta do index** (`<packRoot>/.packwizignore`). A documentação oficial do formato está literalmente como "TODO" (<https://packwiz.infra.link/reference/pack-format/packwizignore/>); o comportamento real está no código (`core/index.go`, `ignoreDefaults`):

```
.git/**
.gitattributes
.gitignore
.DS_Store
/*.zip          # zips exportados da CF na raiz
*.mrpack        # exports do Modrinth
packwiz.exe
packwiz         # também ignora uma pasta chamada packwiz/
```

Os padrões do usuário são concatenados **depois** dos padrões acima, então `!padrão` consegue reincluir. Também são sempre ignorados: o `pack.toml`, o `index.toml` e o próprio `.packwizignore`.

**Tudo o mais na pasta entra no pack** — inclusive lixo. No teste, um `o1.txt` esquecido na raiz foi para `overrides/o1.txt` no `.mrpack` e no zip da CF **[testado]**; a documentação de exportação avisa "Be wary of including files that you don't want". O packwiz não conhece `logs/`, `saves/`, `crash-reports/` etc. porque presume que a pasta do pack **não** é uma instância de jogo. Isso é decisivo para o Warden (ver seção 6): a pasta do pack e a pasta da instância de teste têm de ser separadas.

Limitações: não há whitelist (issue #336), não há ignore global (issue #193), e não há "side" para arquivos não-metafile como configs (issue #82, 14 comentários): **todo arquivo comum vai para `overrides/` e é instalado nos dois lados**.

Arquivos locais: um `.jar` copiado para `mods/` vira uma entrada comum do index (hash SHA-256, sem metafile, sem side, sem atualização) e é exportado como override. Para mods conhecidos, é melhor transformá-los em metafiles (o `cf detect` faz isso por fingerprint para a CF; para o Modrinth não existe comando — mas a API `/version_files` permite, ver 3.1).

URL: `url add` baixa uma vez para calcular o SHA-256 e grava `[download] url`. Não há atualização para URLs (issue #375). Na exportação Modrinth, URLs fora de `cdn.modrinth.com`, `github.com`, `raw.githubusercontent.com`, `gitlab.com` são baixadas e embutidas em `overrides/` (ver 2.3).

### 1.9 Cache de downloads (`core/download.go`)

- Pasta: `--cache` ou `%LocalAppData%\packwiz\cache` no Windows. Estrutura endereçada por SHA-256: `<cache>/<2 primeiros hex>/<resto>`; `index.json` com listas paralelas de hashes por formato; `temp/`; `import/`.
- Desde o commit `52b1230` (2025-11) **todo download passa pelo cache**.
- Downloads manuais (mods bloqueados da CF): o usuário coloca o arquivo em `<cache>/import/`; na próxima execução o packwiz move para o cache e o encontra pelo hash (re-hasheando o cache se preciso — pode ser lento).
- Não há limpeza do cache (issue #320).
- Downloads são **sequenciais** (um goroutine, loop simples) e sem retry.

### 1.10 Busca e escolha de versão

- **Modrinth** (`getLatestVersion`): chama `GET /v2/project/{id}/version?game_versions=[...]&loaders=[...]` com as versões aceitas (MC principal + `acceptable-game-versions`) e os loaders compatíveis **mais** `canvas`, `iris`, `optifine`, `vanilla`, `minecraft` (e `datapack` se `datapack-folder`). Entre os resultados, escolhe por data de publicação (com desempate por versão do MC preferida e preferência de loader `quilt > fabric > neoforge > forge > ...`); avisa se a "maior versão" por FlexVer difere. **Não filtra por `version_type`** (alpha/beta podem ser escolhidas). Busca: `GET /v2/search` com `facets=[["versions:X", ...]]`, `limit=5`, sem filtro de loader nem de tipo de projeto (issue #186).
- **CurseForge** (`findLatestFile`): usa `latestFiles` e `latestFilesIndexes` do projeto, filtra por versão (convertendo snapshots para o rótulo `1.x-Snapshot` da CF) e loader; desempata por **maior file ID**. Também não respeita canal release/beta (comentário TODO).
- **Dependências**: só `required` são seguidas (Modrinth `dependency_type == "required"`; CF `relationType == 3`). **`incompatible`, `optional` e `embedded` são ignorados** — o packwiz não avisa sobre incompatibilidades declaradas entre mods (nenhum código trata `incompatible`). Há substituições fixas para Quilt (Fabric API → QFAPI, FLK → QKL). Dependências já presentes são detectadas **só por project ID da mesma fonte**.

### 1.11 packwiz-installer e packwiz-installer-bootstrap

- <https://github.com/packwiz/packwiz-installer> (Kotlin/Java, MIT) + <https://github.com/packwiz/packwiz-installer-bootstrap> (jar pequeno que baixa/atualiza o installer pelos GitHub Releases). Uso típico: pre-launch command do MultiMC/Prism: `"$INST_JAVA" -jar packwiz-installer-bootstrap.jar https://host/pack.toml`.
- Opções (`Main.kt`): `-s/--side client|server` (padrão `client`), `--pack-folder`, `--multimc-folder`, `--meta-file` (padrão `packwiz.json`, guarda o estado da última instalação), `-t/--timeout` (segundos para escolher opcionais, padrão 10), `-g/--no-gui`, `--title`; bootstrap: `--bootstrap-update-url`, `--bootstrap-update-token`, `--bootstrap-no-update`, `--bootstrap-main-jar`.
- Aceita `pack.toml` por `http(s)://`, `file:` URI ou caminho local.
- Respeita `side`, `option.optional/default/description` (janela de seleção) e `preserve` (não sobrescreve arquivo existente). Atualiza a versão do loader no `mmc-pack.json` do MultiMC/Prism (inclusive NeoForge, commit `7420866`).
- Para modo `metadata:curseforge`, consulta `POST /v1/mods/files` com **a mesma chave de API embutida** do packwiz; se `downloadUrl` vier nulo, lista o mod como download manual e falha.
- Último commit em 2024-04: projeto praticamente parado.

Relevância para o Warden: o Warden tem launcher próprio e não precisa do installer para testar. Mas o formato packwiz "cru" (pack.toml hospedado) continua sendo consumido por ele, então o Warden deve **gerar packs que o installer entende** (respeitar `side`, `option`, `preserve`, hashes internos) caso o usuário publique o pack em um host estático.

### 1.12 Distribuição do binário: build, versões, licença e empacotamento com Tauri

- **Não há releases nem tags** no repositório (verificado via API: `releases` e `tags` vazios). Binários oficiais só existem como artefatos de GitHub Actions, que **expiram em 90 dias** (issue #403, 2026-05, vários pedidos; projetos como Create: Astral passaram a manter fork só para publicar binários). A versão do Go-module é pseudo-versão (`v0.0.0-20260906154125-ef87d964f8cb`).
- Há relato de falso positivo de antivírus no build Windows (issue #374) — relevante para um app que distribui o `.exe` embutido. Builds Go sem assinatura são alvo comum de heurísticas.
- Build reproduzível testado **[testado]**:
  ```bash
  git clone https://github.com/packwiz/packwiz && cd packwiz
  git checkout <commit fixado>
  CGO_ENABLED=0 GOOS=windows GOARCH=amd64 \
    go build -trimpath -ldflags="-s -w -X 'github.com/packwiz/packwiz/curseforge.cfApiKey=<CHAVE_DO_WARDEN>'" \
    -o packwiz-x86_64-pc-windows-msvc.exe .
  ```
  Resultado: PE32+ de ~13 MB (com `-s -w`), sem dependências de DLL (CGO desligado). Compila em Linux para Windows sem toolchain C.
- **Chave da CurseForge**: o packwiz embute uma chave em base64 (`curseforge/request.go`) com o pedido explícito "If you fork/derive from packwiz, I request that you obtain your own API key", e expõe `-X github.com/packwiz/packwiz/curseforge.cfApiKey=...` para injetar outra no build. A issue #349 pede leitura da chave por config/CLI (não implementado). O Warden **deve** compilar com a própria chave (e não usar a do packwiz em nenhum caminho próprio).
- **Licença MIT**: permite redistribuir o binário; exige manter o aviso de copyright e a licença (incluir `LICENSE` do packwiz nos "third-party notices" do instalador do Warden). As dependências Go são majoritariamente MIT/BSD/Apache-2.0 — recomenda-se gerar a lista com `go-licenses` no CI **[inferência; não auditado dependência por dependência]**.
- **Tauri sidecar**: o Tauri 2 suporta `bundle.externalBin` em `tauri.conf.json`; o binário precisa existir com o sufixo do target triple (ex.: `binaries/packwiz-x86_64-pc-windows-msvc.exe`) e é executado via `tauri-plugin-shell` (`app.shell().sidecar("packwiz")`) com permissão `shell:allow-execute`/`shell:allow-spawn` restrita ao sidecar. Fonte: <https://v2.tauri.app/develop/sidecar/>. Alternativa equivalente: chamar o binário com `std::process::Command`/`tokio::process::Command` a partir do Rust, com `CREATE_NO_WINDOW` (0x08000000) no Windows para não piscar console. Detalhes de empacotamento ficam com a pesquisa de Tauri, se houver.
- O cobra no Windows inclui o `mousetrap` (mostra "This is a command line tool" se aberto pelo Explorer). Não afeta execução como subprocesso, já que o processo pai é o Warden e não o `explorer.exe` **[inferência a partir do comportamento documentado do mousetrap]**.

### 1.13 Limitações e issues abertas relevantes (seleção)

| Issue | Tema | Impacto no Warden |
|---|---|---|
| [#393](https://github.com/packwiz/packwiz/issues/393) | `init` falha para Forge 1.7.10/1.8.9/1.9.4... | Warden gera `pack.toml` sozinho. |
| [#366](https://github.com/packwiz/packwiz/issues/366) | NeoForge 1.20.1 com id errado no manifest CF | Corrigir manifest no pós-processamento ou gerar o export no Warden. |
| [#385](https://github.com/packwiz/packwiz/issues/385) | Panic (nil pointer) no `cf export` com fontes mistas e metafiles na raiz; zip truncado | Validar o zip gerado; preferir export próprio. |
| [#376](https://github.com/packwiz/packwiz/issues/376) | 429 do Modrinth em `update --all` de packs grandes; sem backoff | Fazer checagem de updates pela API no Rust, com rate limit. |
| [#167](https://github.com/packwiz/packwiz/issues/167) | `refresh` reescreve TOML e **apaga comentários** e campos com valor padrão | Warden não deve guardar nada importante como comentário nos TOML do packwiz. |
| [#139](https://github.com/packwiz/packwiz/issues/139) | Sem saída "porcelain"/JSON | Parsing de stdout frágil. |
| [#145](https://github.com/packwiz/packwiz/issues/145), [#211](https://github.com/packwiz/packwiz/issues/211) | Sem listagem de mods incompatíveis com a versão do MC (ex.: ao migrar de versão) | Diagnóstico de versão/loader é responsabilidade do Warden. |
| [#29](https://github.com/packwiz/packwiz/issues/29), [#161](https://github.com/packwiz/packwiz/issues/161) | Sem deduplicação Modrinth×CurseForge | Warden deduplica (ver 6). |
| [#82](https://github.com/packwiz/packwiz/issues/82), [#282](https://github.com/packwiz/packwiz/issues/282) | Sem side para arquivos comuns (configs) / sem client/server-overrides | Só resolvível num export próprio. |
| [#97](https://github.com/packwiz/packwiz/issues/97) | `remove` não remove dependências órfãs | Warden calcula órfãos. |
| [#115](https://github.com/packwiz/packwiz/issues/115) | Zips sem entradas de diretório e com bits de permissão estranhos (falha em alguns unzip, inclusive no crate `zip`) | Export próprio resolve. |
| [#191](https://github.com/packwiz/packwiz/issues/191) | Symlinks de diretório quebram `refresh` | Não usar symlinks/junctions dentro da pasta do pack. |
| [#228](https://github.com/packwiz/packwiz/issues/228) | `alias` ignorado nas exportações | Não usar `alias`. |
| [#52](https://github.com/packwiz/packwiz/issues/52) | `update` sobrescreve `filename` com subpasta | Evitar subpastas em `filename`. |
| [#403](https://github.com/packwiz/packwiz/issues/403), [#219](https://github.com/packwiz/packwiz/issues/219) | Sem releases versionadas | Warden compila e fixa commit. |
| [#374](https://github.com/packwiz/packwiz/issues/374) | Falso positivo de antivírus | Assinar o binário com o certificado do Warden. |
| [#349](https://github.com/packwiz/packwiz/issues/349) | Chave CF só por `-ldflags` | Compilar com chave própria. |
| [#300](https://github.com/packwiz/packwiz/issues/300) | Resource packs adicionados como arquivo local são embutidos no `.mrpack` mesmo quando existem no Modrinth (sem lookup por hash) | Export próprio pode resolver por hash via `/v2/version_files`. |
| [#417](https://github.com/packwiz/packwiz/issues/417), [#122](https://github.com/packwiz/packwiz/issues/122) | Sem flag para excluir mods opcionais/sem embutir jars no `mr export` | Export próprio. |

Lista completa de issues abertas (≈160 em 2026-10-01): <https://github.com/packwiz/packwiz/issues>.

Atividade: commits esparsos em 2025–2026 (manutenção por colaboradores; `comp500`, autor original, aparece pouco). O próprio autor comentou na #167 que várias limitações "would be resolved in a future rewrite" — reescrita que não aconteceu.

---

## 2. Formatos de exportação: .mrpack e zip da CurseForge

### 2.1 Especificação do `.mrpack` (Modrinth)

Fonte primária: <https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack>.

- Zip com MIME `application/x-modrinth-modpack+zip`, extensão `.mrpack`.
- Raiz do zip:
  - `modrinth.index.json` (UTF-8) — obrigatório.
  - `overrides/` — copiado para a raiz da instância (`.minecraft`) nos dois lados.
  - `client-overrides/` — aplicado **depois** de `overrides/` só no cliente (sobrescreve arquivos iguais).
  - `server-overrides/` — idem, só no servidor.
- `modrinth.index.json`:

| Campo | Obrigatório | Conteúdo |
|---|---|---|
| `formatVersion` | sim | `1` |
| `game` | sim | `"minecraft"` |
| `versionId` | sim | versão do modpack (string) |
| `name` | sim | nome |
| `summary` | não | descrição curta |
| `files[]` | sim | arquivos baixáveis |
| `files[].path` | sim | destino relativo à raiz da instância; **proibido** `..`, caminho absoluto ou letra de drive (o launcher deve validar — risco de path traversal) |
| `files[].hashes` | sim | `sha1` **e** `sha512` obrigatórios; outros opcionais |
| `files[].env` | não | `{ "client": v, "server": v }` com `v ∈ {"required","optional","unsupported"}` |
| `files[].downloads[]` | sim | URLs HTTPS RFC 3986 (sem espaço não codificado); seguir redirects (≥3) |
| `files[].fileSize` | sim | inteiro (bytes) |
| `dependencies` | sim | chaves `minecraft`, `forge`, `neoforge`, `fabric-loader`, `quilt-loader` |

- Domínios permitidos em `downloads` **para packs publicados em modrinth.com**: `cdn.modrinth.com`, `github.com`, `raw.githubusercontent.com`, `gitlab.com`. Qualquer outro arquivo precisa ir embutido em `overrides/` (e o autor assume a responsabilidade pela licença: <https://support.modrinth.com/en/articles/8797527-obtaining-modpack-permissions>).
- O Modrinth App instala todos os opcionais (mesmo os "desligados por padrão"); o Prism Launcher pergunta pelos opcionais quando o `.mrpack` é importado como arquivo (documentação do packwiz, <https://packwiz.infra.link/tutorials/hosting/modrinth/>).
- Valores das versões dos loaders em `dependencies`: o Modrinth usa o mesmo formato "sem prefixo do MC" que o packwiz para NeoForge 1.20.1 (`"neoforge": "47.1.106"`), conforme comentário na issue #366.

### 2.2 Zip de modpack da CurseForge

Não há especificação formal publicada; o formato é o que o app da CurseForge exporta, documentado de forma informal em <https://support.curseforge.com/support/solutions/articles/9000197908-exporting-a-modpack-for-curseforge-project-submission> e consumido por launchers (GDLauncher: <https://gdlauncher.com/docs/modpack-manifest-format/>).

Estrutura:

```
manifest.json
modlist.html        (opcional, lista HTML dos mods)
overrides/          (nome configurável pelo campo "overrides")
```

`manifest.json` (exemplo real gerado pelo packwiz **[testado]**):

```json
{
  "minecraft": {
    "version": "1.21.1",
    "modLoaders": [ { "id": "fabric-0.19.5", "primary": true } ]
  },
  "manifestType": "minecraftModpack",
  "manifestVersion": 1,
  "name": "T",
  "version": "1.0.0",
  "author": "A",
  "projectID": 0,
  "files": [ { "projectID": 324717, "fileID": 8591528, "required": true } ],
  "overrides": "overrides"
}
```

- `modLoaders[].id`: `forge-<versão>` (ex.: `forge-47.2.0`, `forge-14.23.5.2860`), `fabric-<versão>`, `neoforge-<versão>` (para 1.20.1 a CF espera `neoforge-1.20.1-47.1.106`, issue #366), `quilt-<versão>`.
- `files[]`: só referências `projectID`/`fileID`; `required: false` significa mod **desabilitado por padrão** (a CF não tem conceito real de opcional; a doc do packwiz diz que o app da CF apenas o deixa desligado).
- **Não existe informação de lado (client/server)** no formato. Servidores da CF são gerados à parte pelo autor ("server pack").
- Regras de submissão à CurseForge: mods hospedados na CF **não podem** ir dentro do zip (devem estar no manifest); jars de terceiros em `overrides/mods` só se estiverem na lista de mods aprovados (licença MIT/GPL ou equivalente, distribuível oficial e sem modificação); perfis "vanilla" não são aceitos. Fonte: artigo acima e <https://support.curseforge.com/en/support/solutions/articles/9000197913-non-curseforge-mods>.

### 2.3 Como o packwiz exporta cada formato e o que se perde

Fluxo comum (`modrinth/export.go`, `curseforge/export.go`): faz `refresh` (o export **altera** `index.toml`/`pack.toml` em disco), carrega todos os metafiles, baixa o que precisa via cache, escreve o zip e depois copia **todo arquivo não-metafile do index** para `overrides/<caminho>` (`cmdshared.AddNonMetafileOverrides`).

**`packwiz modrinth export`** (saída padrão `<name>-<version>.mrpack` na pasta atual):

- Metafiles em modo `url` cujo host está na whitelist (com `--restrictDomains=true`, padrão) → entram em `files[]` com `sha1`, `sha512` e `fileSize` calculados baixando o arquivo (cache), URL re-codificada para RFC 3986.
- `env` derivado de `side` + `option.optional`: `both`/vazio → `client = server = required|optional`; `client` → `server = "unsupported"`; `server` → `client = "unsupported"`. `optional` vira `"optional"`; **`option.default` e `option.description` se perdem** (o formato não tem onde guardar).
- Metafiles **da CurseForge** (modo `metadata:curseforge`) e URLs fora da whitelist → o jar é **baixado e embutido** em `overrides/`, `client-overrides/` ou `server-overrides/` conforme o side **[testado: o Jade da CF foi para `overrides/mods/Jade-...jar`]**. Isso exige licença de redistribuição; o packwiz só imprime um disclaimer. Se o mod da CF bloquear download de terceiros, o export **aborta** (exit 1) pedindo download manual.
- Arquivos comuns (configs, resource packs locais, `options.txt`...) → sempre `overrides/` (sem side; issue #82). **Não há como gerar `client-overrides/`/`server-overrides/` para configs.**
- `dependencies`: `minecraft` + **um único** loader, na ordem `quilt-loader`, `fabric-loader`, `forge`, `neoforge`.
- `summary` = `description`; `versionId` = `version` (aviso se vazio).
- `files[]` ordenado por `path` (reprodutível desde o commit `9cca744`).
- Defeitos: zip sem entradas de diretório e com permissões estranhas (issue #115); `alias` ignorado (#228); arquivos esquecidos na pasta vão junto **[testado]**; erro no refresh sai com código 0.

**`packwiz curseforge export`** (saída padrão `<name>-<version>.zip`):

- Filtra mods por `--side` (padrão **`client`**): mods `server` são descartados silenciosamente.
- Metafiles com `[update.curseforge]` → `files[]` com `required = !(optional && !default)`.
- Todo o resto (Modrinth, URL, GitHub) → **jar embutido em `overrides/mods/...`** **[testado: Fabric API, Mod Menu, Sodium e Placeholder API foram embutidos]**. Isso viola a política de submissão da CF a menos que os mods estejam na lista aprovada.
- `modlist.html` com links `https://www.curseforge.com/projects/<id>` (mods de outras fontes aparecem sem link).
- `projectID` vem de `[export.curseforge] project-id` (0 se ausente).
- Perde: side (formato não suporta), `option.description`, a distinção "opcional ligado por padrão" (vira `required: true`), metadados de Modrinth.
- Bugs: #366 (NeoForge 1.20.1), #385 (panic com fontes mistas/metafiles na raiz, zip truncado).

Resumo do que **só o formato packwiz nativo preserva**: side por arquivo de mod, opcionais com descrição e padrão, `preserve` em configs, origem/atualização por fonte (`[update.*]`), `pin`. Os dois formatos de distribuição são "achatamentos" com perda.

### 2.4 Importação (fora do escopo principal, nota rápida)

- `packwiz curseforge import` aceita zip CF ou `minecraftinstance.json` (sem HTTP).
- **Não existe import de `.mrpack`** (issue #155). Se o Warden quiser importar `.mrpack`, terá de fazê-lo sozinho (o mapeamento é simples: `files[]` → metafiles Modrinth via `/v2/version_files` por sha1/sha512; `overrides/` → arquivos comuns).

---

## 3. APIs: Modrinth v2 e CurseForge

### 3.1 Modrinth API v2

Documentação: <https://docs.modrinth.com/api/>. Base: `https://api.modrinth.com/v2` (staging: `staging-api.modrinth.com`). Sem chave para leitura.

**Regras de uso**

- **User-Agent obrigatório e identificável**; UAs genéricos (`okhttp/4.9.3`, `reqwest/x`) podem ser bloqueados. Formato recomendado: `github_username/project_name/1.56.0 (contato)`. Para o Warden: `Kriticales/Warden/<versão> (<contato>)`. O packwiz usa `packwiz/packwiz` (`core/download.go`).
- **Rate limit: 300 requisições/minuto por IP**, igual com ou sem autenticação. Cabeçalhos `X-Ratelimit-Limit`, `X-Ratelimit-Remaining`, `X-Ratelimit-Reset` (segundos) **[testado: `x-ratelimit-limit: 300`]**. Excedido → HTTP 429 com corpo `{"error":"ratelimit_error","description":"You are being rate-limited. Please wait N milliseconds. 0/300 remaining."}` (texto observado na issue #376 do packwiz).
- Autenticação (PAT/OAuth) só para escrita ou conteúdo privado — desnecessária para o Warden.
- Versões quebradas da API ganham novo prefixo (`/v3` já existe para uso interno); versões descontinuadas passam a responder 410.
- CORS liberado (útil se alguma chamada for feita do frontend, embora o recomendado seja centralizar no Rust por causa do rate limit e do cache).

**Endpoints relevantes** (todos verificados ao vivo em 2026-10-01 **[testado]**):

| Uso no Warden | Endpoint | Observações |
|---|---|---|
| Busca | `GET /search?query=&facets=&index=&offset=&limit=` | `limit` máx. 100; `index` ∈ `relevance`, `downloads`, `follows`, `newest`, `updated`. |
| Projeto(s) | `GET /project/{id\|slug}`, `GET /projects?ids=[...]` | slug e ID são aceitos indistintamente. |
| Versões de um projeto | `GET /project/{id}/version?loaders=["fabric"]&game_versions=["1.21.1"]&include_changelog=false` | filtro por loader/versão no servidor; `include_changelog=false` reduz muito a resposta. |
| Versão(ões) | `GET /version/{id}`, `GET /versions?ids=[...]` | resolução em lote de dependências. |
| **Identificar arquivo local** | `POST /version_files` com `{"hashes":[...],"algorithm":"sha1"\|"sha512"}` | devolve mapa hash → versão. Permite converter jars locais em metafiles Modrinth e deduplicar por hash. |
| **Checar atualizações em lote** | `POST /version_files/update` com `{"hashes":[...],"algorithm":"sha1","loaders":[...],"game_versions":[...]}` | devolve mapa hash → versão mais nova compatível. Uma chamada para o pack inteiro (o packwiz faz uma por mod, daí os 429). |
| Listas de referência | `GET /tag/loader`, `GET /tag/game_version`, `GET /tag/category` | cachear localmente. |

**Sintaxe de facets**: string JSON de lista de listas; elementos **dentro** da mesma lista são OR, listas diferentes são AND. Operadores `:`, `!=`, `>=`, `>`, `<=`, `<`. Ex.: `[["categories:fabric"],["versions:1.21.1"],["project_type:mod"]]` **[testado: 46 resultados para "sodium"]**. Tipos de facet: `project_type`, `all_project_types`, `categories` (loaders também são categorias), `versions`, `environment`, `open_source`, `license`, `downloads`, `title`, `author`, `follows`, `project_id`, `created_timestamp`, `modified_timestamp`, `disclosure_types`. `client_side`/`server_side` estão **depreciados** em favor de `environment`. Fonte: <https://docs.modrinth.com/api/operations/searchprojects/>.

**Modelo de versão** (campos observados **[testado]**): `id`, `project_id`, `name`, `version_number`, `version_type` (`release`/`beta`/`alpha`), `game_versions[]`, `loaders[]`, `environment` (novo), `status`, `date_published`, `files[]` (`url`, `filename`, `hashes.sha1`, `hashes.sha512`, `primary`, `size`, `file_type`), `dependencies[]`.

- `dependencies[]`: `{ version_id?, project_id?, file_name?, dependency_type }` com `dependency_type ∈ {required, optional, incompatible, embedded}`. **`incompatible` existe e é usado na prática** — ex.: Embeddium 1.0.15+mc1.21.1 declara dois projetos incompatíveis; versões antigas do Sodium também **[testado]**. `file_name` aparece em dependências de mods externos ao Modrinth (só nome do arquivo).
- `environment` da versão: `client_and_server`, `client_only`, `client_only_server_optional`, `singleplayer_only`, `server_only`, `server_only_client_optional`, `dedicated_server_only`, `client_or_server`, `client_or_server_prefers_both`, `unknown`. O projeto também expõe `environment[]` e ainda os antigos `client_side`/`server_side`. Fonte: <https://docs.modrinth.com/api/operations/getprojectversions/>.
- `file_type`: `required-resource-pack`, `optional-resource-pack`, `sources-jar`, `dev-jar`, `javadoc-jar`, `signature`, `unknown` ou `null`. **Escolher o arquivo `primary`** e ignorar `sources-jar`/`dev-jar`.

**Loaders do Modrinth** (`/tag/loader`, **[testado]**): `babric`, `bta-babric`, `bukkit`, `bungeecord`, `canvas`, `datapack`, `fabric`, `folia`, `forge`, `geyser`, `iris`, `java-agent`, `legacy-fabric`, `liteloader`, `minecraft`, `modloader`, `neoforge`, `nilloader`, `optifine`, `ornithe`, `paper`, `purpur`, `quilt`, `rift`, `spigot`, `sponge`, `vanilla`, `velocity`, `waterfall`. Atenção: **"Fabric" em 1.7.10/1.12.2 é `legacy-fabric` (ou `ornithe`/`babric`)**, que o packwiz não conhece (ver Questões em aberto).

### 3.2 CurseForge API (Core API v1)

Documentação: <https://docs.curseforge.com/rest-api/>. Base: `https://api.curseforge.com`. Minecraft = `gameId 432`.

**Chave de API**

- Obrigatória em toda requisição: cabeçalho `x-api-key`.
- Obtenção: formulário de aplicação revisado pela Overwolf (<https://support.curseforge.com/support/solutions/articles/9000208346-about-the-curseforge-api-and-how-to-apply-for-a-key>, formulário em <https://forms.monday.com/forms/dce5ccb7afda9a1c21dab1a1aa1d84eb?r=use1>); depois de aprovada, a chave é gerada no console <https://console.curseforge.com/>. Critérios declarados: impacto na receita dos autores, carga nos servidores/CDN e respeito ao consentimento dos autores para distribuição por terceiros. Sem prazo de aprovação publicado.
- Termos: <https://support.curseforge.com/support/solutions/articles/9000207405-curse-forge-3rd-party-api-terms-and-conditions>. Pontos que afetam o Warden:
  - §2.2: a chave é intransferível e não pode ser compartilhada. Na prática, launchers (Prism, packwiz) embutem a chave no binário; mantenedores do Prism relatam que a CurseForge aceitou isso informalmente, mas **forks devem usar chave própria** (discussão em <https://github.com/PrismLauncher/PrismLauncher/issues/263>). Não há documento oficial com essa exceção.
  - §2.3: acima de uma cota (não divulgada, "a critério da Overwolf") pode haver exigência de contrato/pagamento. **Não há rate limit público.**
  - §3.1(e): proibido "save or cache any data obtained through the API". Interpretação conservadora **[inferência]**: não persistir respostas da CF (busca, descrições, ícones) além da sessão; guardar só os identificadores necessários no pack (`project-id`, `file-id`, hash) — que é o que o próprio formato de manifesto da CF e o packwiz fazem — e **nunca** gravar `downloadUrl`.
  - Também proíbe ocultar identidade/localização (proxy/VPN) ao acessar a API.
- O packwiz embute a chave dele (base64 em `curseforge/request.go`) e pede que derivados usem a própria; o packwiz-installer usa a mesma chave.

**Endpoints relevantes**

| Uso | Endpoint |
|---|---|
| Busca | `GET /v1/mods/search?gameId=432&classId=6&gameVersion=1.20.1&modLoaderType=1&searchFilter=...&sortField=2&sortOrder=desc&index=0&pageSize=50` (`pageSize` máx. 50; `index + pageSize ≤ 10000`; `gameVersions` até 4; `modLoaderTypes` até 5; `categoryIds` até 10; `modLoaderType` exige `gameVersion`) |
| Mod(s) | `GET /v1/mods/{modId}`, `POST /v1/mods` `{"modIds":[...]}` |
| Arquivos | `GET /v1/mods/{modId}/files?gameVersion=&modLoaderType=`, `GET /v1/mods/{modId}/files/{fileId}`, `POST /v1/mods/files` `{"fileIds":[...]}` |
| URL de download | `GET /v1/mods/{modId}/files/{fileId}/download-url` |
| Fingerprint | `POST /v1/fingerprints/432` `{"fingerprints":[...]}` (murmur2 da CF; existe também a variante "fuzzy") |
| Versões/loader | `GET /v1/minecraft/version`, `GET /v1/minecraft/modloader` |
| Categorias/classes | `GET /v1/categories?gameId=432[&classesOnly=true]` |

**Enums** (do schema da documentação):
- `ModLoaderType`: 0 Any, 1 Forge, 2 Cauldron, 3 LiteLoader, 4 Fabric, 5 Quilt, 6 NeoForge.
- `FileRelationType`: 1 EmbeddedLibrary, 2 OptionalDependency, 3 RequiredDependency, 4 Tool, **5 Incompatible**, 6 Include.
- `FileReleaseType`: 1 Release, 2 Beta, 3 Alpha.
- `HashAlgo`: 1 Sha1, 2 Md5.
- Classes do Minecraft usadas pelo packwiz: 6 mods, 12 resource packs, 5 Bukkit plugins, 17 worlds (`saves`); shaders têm classe própria (6552) não mapeada pelo packwiz **[o ID 6552 é inferência; confirmar via `/v1/categories?classesOnly=true`]**.

**Distribuição de terceiros e mods bloqueados**

- Cada projeto tem o "Project Distribution Toggle" (<https://support.curseforge.com/support/solutions/articles/9000207877-project-distribution-toggle>): padrão ligado para projetos existentes em 2022; quando desligado, "any 3rd-party service, website or client will not have access to your projects or files via the 3rd-party API".
- No schema: `Mod.allowModDistribution: boolean|null` e `File.downloadUrl` — para mods com distribuição desligada, o `downloadUrl` vem **nulo** (comportamento em que packwiz, packwiz-installer e Prism se baseiam). `Mod.isAvailable` indica disponibilidade para busca (pode ser falso para mods experimentais/deletados/só-alpha), não a distribuição.
- Como o packwiz lida (`curseforge/curseforge.go: cfDownloader.GetFilesMetadata` + `core/download.go`):
  1. No `add`, nada muda: o metafile é criado normalmente com `mode = "metadata:curseforge"` e o hash (sha1/md5/murmur2) — o `add` **não avisa** que o mod é bloqueado (issue #123 pede isso).
  2. Na hora de baixar (export, rehash), `POST /v1/mods/files`; se `downloadUrl == ""`, busca o mod (`POST /v1/mods`) para montar o link da página (`<websiteUrl>/files/<fileId>`) e marca como download manual.
  3. Antes de desistir, procura no cache um arquivo com o mesmo hash (inclusive re-hasheando tudo — `GetHandleFromHashForce`).
  4. Se faltar algum, imprime a lista e instrui a colocar os arquivos em `<cache>/import/`, saindo com código 1.
- O Prism Launcher resolve com melhor UX: mostra a lista de mods bloqueados com links e **observa a pasta Downloads do usuário**, validando por hash e copiando automaticamente (PR <https://github.com/PrismLauncher/PrismLauncher/pull/304>). É o modelo a seguir no Warden.
- No export Modrinth do packwiz, um mod CF bloqueado aborta o export; se não bloqueado, o jar é **embutido** no `.mrpack` — o que, para mods da CF, geralmente viola a licença/termos. O Warden deve preferir trocar a fonte para o Modrinth quando o mesmo arquivo existir lá (match por sha1/sha512 via `/v2/version_files`).

---

## 4. Metadados de compatibilidade dentro dos .jar

Todo `.jar` de mod é um zip. Os arquivos de metadados abaixo são a fonte **local e determinística** para o diagnóstico (independem de API e funcionam para mods locais/URL). Ordem de detecção recomendada para um jar: `quilt.mod.json` → `fabric.mod.json` → `META-INF/neoforge.mods.toml` → `META-INF/mods.toml` → `mcmod.info` → anotação `@Mod` nas classes (fallback) → `META-INF/MANIFEST.MF` (coremods/tweakers).

### 4.1 `fabric.mod.json` (Fabric; também lido pelo Quilt)

Spec: <https://wiki.fabricmc.net/documentation:fabric_mod_json_spec>.

| Campo | Uso no diagnóstico |
|---|---|
| `schemaVersion` | `1` atual (ausente = v0 legado). |
| `id` | `^[a-z][a-z0-9-_]{1,63}$`. Chave para duplicatas e para resolver dependências. |
| `version` | preferencialmente SemVer; comparar com o algoritmo do Fabric (SemVer estendido) — **[recomendação]** usar FlexVer como fallback para versões não SemVer. |
| `environment` | `"*"` (padrão), `"client"` ou `"server"` (string ou array). `client` num servidor dedicado = mod não carrega. Cruzar com o `side` do metafile. |
| `depends` | mapa id → faixa de versão. **Falha dura** se ausente ou fora da faixa. |
| `recommends` | aviso se ausente/fora da faixa. |
| `suggests` | só informativo. |
| `breaks` | **falha dura** se o outro mod estiver presente e na faixa. |
| `conflicts` | aviso se presente e na faixa. |
| `provides` | ids alternativos que este mod "fornece" (aliases) — necessário para não acusar dependência faltando (campo confirmado no parser `V1ModMetadataParser.java` do fabric-loader). |
| `jars[]` | jars aninhados (`{"file": "META-INF/jars/x.jar"}`) — **devem ser abertos recursivamente**: muitas dependências (ex.: partes da Fabric API, MixinExtras) vêm embutidas e satisfazem `depends`. |
| `mixins` | lista de configs de mixin (útil para a IA: mods com muitos mixins são suspeitos frequentes de conflito). |

Faixas de versão: `*`, igualdade, `=`, `>=`, `>`, `<=`, `<`, `^` (mesma major), `~` (mesma minor), wildcards `1.x`/`1.2.*`; **array = OR**, espaço dentro da string = AND. IDs especiais usados como dependência: `minecraft` (fornecido pelo `MinecraftGameProvider`), `java` (versão da especificação do Java, criado em `ModDiscoverer.java`) e `fabricloader` — "built-in mods" do fabric-loader (<https://github.com/FabricMC/fabric-loader>). O parser também aceita o campo legado `requires`. A versão do `minecraft` vista pelo Fabric é normalizada para SemVer (ex.: snapshots `1.21-alpha.24.12.a`), o que o Warden precisa replicar ou aproximar ao avaliar `depends.minecraft`.

### 4.2 `quilt.mod.json` (Quilt)

Spec: <https://github.com/QuiltMC/rfcs/blob/main/specification/0002-quilt.mod.json.md>.

- `schema_version: 1`; dados em `quilt_loader`: `id`, `version`, `depends[]`, `breaks[]`, `provides[]`, `jars[]`; ambiente em `minecraft.environment` ∈ `"*"`, `"client"`, `"dedicated_server"`.
- Dependência é string (`"modid"`) ou objeto `{ id ("grupo:modid" ou "modid"), versions (padrão "*"), reason, optional (padrão false), unless }`. `breaks` usa o mesmo objeto. `unless` permite exceção condicional ("conflita com X a menos que Y esteja presente").
- O Quilt também lê `fabric.mod.json`. O escopo do Warden é Fabric/Forge/NeoForge, mas o packwiz suporta Quilt; o parser custa pouco.

### 4.3 `META-INF/mods.toml` (MinecraftForge 1.13+) e `META-INF/neoforge.mods.toml` (NeoForge)

Fontes: Forge <https://docs.minecraftforge.net/en/latest/gettingstarted/modfiles/>; NeoForge <https://docs.neoforged.net/docs/gettingstarted/modfiles/>; anúncio do NeoForge 20.5 <https://neoforged.net/news/20.5release/>; código do Forge `fmlloader/.../moddiscovery/ModInfo.java` e `ModSorter.java` (branches `1.20.1`, `1.21.1`, `26.3` em <https://github.com/MinecraftForge/MinecraftForge>).

Estrutura comum (TOML):

```toml
modLoader = "javafml"
loaderVersion = "[47,)"
license = "MIT"

[[mods]]
modId = "examplemod"          # 2-64 chars, [a-z0-9_]
version = "${file.jarVersion}"  # pode vir do MANIFEST.MF (Implementation-Version)
displayName = "Example"

[[dependencies.examplemod]]
modId = "minecraft"
versionRange = "[1.20.1,1.21)"
ordering = "NONE"
side = "BOTH"
```

Campos de dependência:

| Campo | MinecraftForge (`mods.toml`, inclusive 1.21.x e 26.x) | NeoForge (`neoforge.mods.toml`) |
|---|---|---|
| obrigatoriedade | `mandatory = true\|false` (obrigatório no Forge; erro "Missing required field mandatory" se ausente) | `type = "required"` (padrão) \| `"optional"` \| `"incompatible"` \| `"discouraged"` |
| `versionRange` | faixa **Maven** (`[1.0,2.0)`, `[1.0,)`, `(,1.0]`, `1.0` = "≥1.0" recomendado, vírgula separa conjuntos) | idem |
| `ordering` | `BEFORE`/`AFTER`/`NONE` | idem (ciclos causam crash) |
| `side` | `CLIENT`/`SERVER`/`BOTH` — a dependência só é exigida naquele lado | idem |
| `reason` | — | texto exibido ao usuário (útil para o diagnóstico) |
| `referralUrl` | URL para baixar a dependência | idem |

Semântica real (confirmada no `ModSorter` do Forge 1.20.1): uma dependência **opcional que está presente mas fora do `versionRange` também é erro** (`fml.modloading.missingdependency.optional`). O Forge **não tem tipo "incompatible"** — autores expressam incompatibilidade com `mandatory=false` + faixa impossível, ou não expressam. No NeoForge, `incompatible` impede o carregamento e `discouraged` só avisa.

Outras informações úteis:
- NeoForge exige `neoforge.mods.toml` a partir do **20.5 (MC 1.20.5)**; antes usava `mods.toml`. O campo `type` substituiu `mandatory` no NeoForge (confirmado na doc atual; a versão exata da troca dentro do ciclo 20.x **não foi confirmada**). Para NeoForge 1.20.1 (fork do Forge 47) vale o formato do Forge.
- `[features.<modid>] javaVersion = "[21,)"` (NeoForge) — exigência de versão do Java; essencial para o launcher.
- Forge: `clientSideOnly = true` e `displayTest` (`MATCH_VERSION`, `IGNORE_SERVER_VERSION`, `IGNORE_ALL_VERSION`, `NONE`) indicam mods só de cliente.
- IDs especiais: `minecraft`, `forge` (Forge), `neoforge` (NeoForge), `javafml`.
- Jar-in-jar: `META-INF/jarjar/metadata.json` lista jars embutidos em `META-INF/jarjar/` (Forge 1.18.2+/NeoForge) — abrir recursivamente **[estrutura conhecida, não verificada em fonte primária nesta pesquisa]**.
- `[[mixins]]` e `[[accessTransformers]]` (NeoForge) também são sinais úteis para a IA.

### 4.4 `mcmod.info` e anotação `@Mod` (Forge 1.7.10 – 1.12.2)

Fonte: <https://docs.minecraftforge.net/en/1.12.x/gettingstarted/structuring/>; `Mod.java` do branch `1.12.x` do Forge.

- `mcmod.info` é JSON na raiz do jar: array de objetos (ou, em alguns mods, `{"modListVersion": 2, "modList": [...]}`). Campos: `modid`, `name`, `description`, `version`, `mcversion`, `url`, `updateUrl`, `authorList`, `credits`, `logoFile`, `screenshots`, `parent`, `requiredMods`, `dependencies`, `dependants`, `useDependencyInformation`.
- **As listas de dependência do `mcmod.info` só valem com `useDependencyInformation: true` e `@Mod(useMetadata = true)`** — raro. Na prática o `mcmod.info` serve para **identificar** o mod (modid, versão, mcversion), não para dependências. Muitos arquivos são JSON inválido (vírgulas sobrando, quebras de linha em string) — o parser precisa ser tolerante e nunca derrubar o diagnóstico.
- A informação real de dependência está na anotação `@Mod` nas classes (`net/minecraftforge/fml/common/Mod` em 1.8+; `cpw/mods/fml/common/Mod` em 1.7.10): `modid`, `version`, `dependencies` (string no formato `"required-after:forge@[14.23.5.2847,);after:jei"` — prefixos `before`, `after`, `required-before`, `required-after`, com faixa Maven após `@`), `acceptedMinecraftVersions` (faixa Maven), `clientSideOnly`/`serverSideOnly` (presentes no 1.12.x; **não confirmados no 1.7.10**), `acceptableRemoteVersions` (`"*"` = cliente-only aceitável). Ler isso exige parsear o bytecode (atributo `RuntimeVisibleAnnotations`) — viável em Rust **[recomendação; ver crates na seção 6]**.
- Coremods/tweakers: `META-INF/MANIFEST.MF` com `FMLCorePlugin`, `FMLCorePluginContainsFMLMod`, `TweakClass` (ex.: OptiFine, Mixin bootstrap). Coremods são causa clássica de crash em 1.7.10/1.12.2; marcar no relatório.

### 4.5 Como ler para o diagnóstico determinístico

1. Para cada arquivo em `mods/` (após resolver metafiles → jars do cache), abrir o zip **sem extrair** e ler apenas os arquivos de metadados; repetir para jars aninhados (`jars[]` do Fabric/Quilt, `META-INF/jarjar/`).
2. Normalizar tudo num modelo único: `{ arquivo, origem (modrinth/cf/url/local), loader_alvo, ids[] (id + provides), versão, ambiente (client/server/both), deps[] { id, faixa, tipo: required|optional|recommends|suggests|incompatible|discouraged|breaks|conflicts, side, motivo } , mc_range, loader_range, java_range, flags (coremod, mixins) }`.
3. Avaliar faixas com o dialeto correto: SemVer-Fabric para Fabric/Quilt, Maven para Forge/NeoForge e `@Mod`.
4. Combinar com os metadados da API (Modrinth `dependencies[].dependency_type == "incompatible"`, CF `relationType == 5`), que cobrem incompatibilidades que os autores não colocam no jar.
5. O desenho completo das regras está em 6.4.

---

## 5. Formatos de config usados por mods

### 5.1 Panorama de formatos e onde ficam

| Formato | Onde aparece | Observações |
|---|---|---|
| **TOML do Forge/NeoForge** (`ForgeConfigSpec` / `ModConfigSpec`, lido com NightConfig) | `config/<modid>-client.toml`, `-common.toml`, `-startup.toml` (NeoForge); `-server.toml` por mundo em `saves/<mundo>/serverconfig/` | ver 5.2 |
| **JSON** | Fabric em geral (Cloth Config/AutoConfig com serializador JSON, MidnightLib, configs próprias), também Forge | ex.: `config/sodium-options.json` |
| **JSON5 / Jankson** | YACL, owo-config, AutoConfig com Jankson (`.json5`), mods Quilt | comentários `//` e `/* */`, chaves sem aspas, vírgulas finais |
| **YAML** | raro em mods de cliente; comum em plugins e em alguns mods de servidor | |
| **`.cfg` do Forge antigo** (`net.minecraftforge.common.config.Configuration`) | 1.7.10–1.12.2, `config/*.cfg` | formato próprio, ver 5.3 |
| **`.properties`** | Iris (`iris.properties`), OptiFine shaders, `server.properties`, vários mods | `java.util.Properties` |
| **`options.txt`** | raiz da instância | ver 5.4; mais `optionsof.txt` (OptiFine), `optionsshaders.txt` |
| **`servers.dat`** | raiz da instância | NBT não comprimido, ver 5.5 |
| Outros | `.snbt`/`.nbt` (FTB Quests, mundos), `.js`/`.zs` (KubeJS/CraftTweaker em `kubejs/`, `scripts/`), `.json` de datapacks/recursos | tratar como texto ou binário opaco |

Implicação: o editor do Warden precisa de **um editor de texto genérico com realce de sintaxe para todos** e **editores estruturados (formulário) só para os formatos com estrutura confiável**: TOML do Forge/NeoForge, JSON/JSON5, `.properties`, `options.txt`, `.cfg` legado.

### 5.2 TOML do Forge/NeoForge (`ForgeConfigSpec`/`ModConfigSpec`)

Fonte: <https://docs.neoforged.net/docs/misc/config/>; NightConfig: <https://github.com/TheElectronWill/night-config>.

- Tipos (NeoForge atual): `STARTUP` (lido no registro, não sincronizado), `CLIENT` (só cliente físico), `COMMON` (cliente e servidor), `SERVER` (sincronizado com clientes; sobreposto por mundo em `saves/<mundo>/serverconfig/` ou `<servidor>/world/serverconfig/`). Nome de arquivo `<modid>-<tipo>.toml`.
- No Forge 1.13–1.20 os configs `SERVER` existem **só por mundo**; a pasta **`defaultconfigs/`** na raiz da instância é copiada para o `serverconfig/` de cada mundo novo — é ali que um modpack coloca padrões de servidor **[comportamento amplamente documentado pela comunidade; não confirmado em fonte primária nesta pesquisa]**. No NeoForge atual a doc indica que o arquivo global serve de base e o por-mundo sobrepõe.
- O arquivo é gerado pelo mod a partir do "spec": cada valor ganha comentários gerados (`#Range: 1 ~ 64`, `#Allowed Values: A, B, C`, `#Default: ...` em versões recentes) e as seções usam indentação com tab. Na carga, valores inválidos/faltando são **corrigidos e o arquivo é regravado**, o que **descarta comentários do usuário e chaves desconhecidas** **[comportamento conhecido do NightConfig/ModConfig; confirmar no código ao implementar]**. Consequência: comentários que o Warden ou o usuário adicionem nesses arquivos não sobrevivem a uma execução do jogo; os comentários gerados são a "documentação" a exibir no formulário (faixa, valores permitidos).
- Edição segura: alterar só o valor da chave, preservando o resto byte a byte → `toml_edit`.

### 5.3 `.cfg` do Forge antigo (1.7.10–1.12.2)

Lido do código `net/minecraftforge/common/config/Configuration.java` e `Property.java` (branch `1.12.x`):

```cfg
# Configuration file

~CONFIG_VERSION: 1.2

general {
    # Comentário gerado pelo mod
    B:enableFeature=true
    I:maxStack=64
    D:speed=1.5
    S:mode=fast
    S:blacklist <
        minecraft:stone
        minecraft:dirt
     >

    sub {
        I:value=3
    }
}
```

- Prefixo de tipo de 1 caractere: `S` (string), `I` (inteiro), `B` (booleano), `D` (double), `C` (cor), `M` (mod id).
- Categorias aninhadas com `nome {` … `}`; nomes com espaço vão entre aspas (`"my category" {`).
- Listas com `<` … `>` (um item por linha).
- Comentários com `#`; `~CONFIG_VERSION:` opcional; blocos `START: "x"`/`END: "x"` em arquivos com múltiplas configs.
- Caracteres permitidos em nomes sem aspas: letras, dígitos e `._-`. Encoding UTF-8.
- **Não existe biblioteca Rust/TS mantida** para esse formato; é preciso um parser próprio, orientado a linhas, que guarde a linha original e altere só o trecho do valor (round-trip garantido). O Forge regrava o arquivo inteiro ao salvar (comentários vêm da definição do mod).
- Muitos mods 1.12.2 usam a anotação `@Config` (mesmo formato `.cfg`), e alguns usam JSON ou formatos próprios.

### 5.4 `options.txt` (e afins)

Fonte: <https://minecraft.wiki/w/Options.txt>.

- Linhas `chave:valor`. `version:<DataVersion>` indica a versão que gravou o arquivo; o jogo aplica DataFixer ao abrir com versão mais nova, e uma versão mais antiga pode **resetar** o arquivo.
- Teclas: em 1.13+ `key_key.forward:key.keyboard.w`; em 1.7.10/1.12.2 os valores são **códigos numéricos** (LWJGL 2), ex.: `key_key.forward:17`. Mods adicionam suas próprias teclas `key_<id>`.
- `resourcePacks:["vanilla","file/MeuPack.zip"]` e `incompatibleResourcePacks:[...]` (lista JSON). Em versões antigas os nomes não têm o prefixo `file/` **[inferência; validar por versão]**.
- O jogo **reescreve o arquivo inteiro** ao fechar o menu de opções/sair (sem comentários, ordem própria). Mods (OptiFine: `optionsof.txt`; Iris: `iris.properties`; Sodium: `sodium-options.json`) mantêm configs gráficas separadas.
- Distribuição: colocar `options.txt` em `overrides/` **sobrescreve as preferências do jogador a cada atualização** do pack. Alternativas usadas por packs: mods "Your Options Shall Be Respected (YOSBR)"/"Default Options", que aplicam padrões só na primeira execução (YOSBR lê de `config/yosbr/`) **[nomes e caminhos de memória; confirmar nas páginas dos mods]**, ou o campo `preserve = true` do index do packwiz (respeitado só pelo packwiz-installer).
- Edição: parser trivial linha a linha, preservando ordem e linhas desconhecidas.

### 5.5 `servers.dat` (NBT)

Fonte: <https://minecraft.wiki/w/Servers.dat_format>.

- **NBT não comprimido** (diferente de `level.dat`, que é gzip). Raiz: compound com uma lista `servers` de compounds `{ name: String, ip: String, icon: String (PNG base64, opcional), acceptTextures: Byte (opcional), hidden: Byte, acceptedCodeOfConduct: Int (versões recentes) }`.
- O jogo mantém um `servers.dat_old` como backup **[inferência; a página não cobre]**.
- Editar com crate NBT em Rust (ver 5.6). Útil para o pack trazer servidores pré-configurados.

### 5.6 Como editar preservando comentários e formatação — bibliotecas recomendadas

Princípio: **toda edição de config acontece no backend Rust** (um único lugar com testes de round-trip), o frontend recebe uma árvore com valores + metadados (comentário, faixa, tipo, posição) e devolve só "mudanças" (`caminho = novo valor`). O frontend nunca regrava o arquivo inteiro. O editor de texto cru (CodeMirror/Monaco) continua disponível para tudo.

Dados de crates.io/npm consultados em 2026-10-01:

| Formato | Rust (backend) — recomendado | TypeScript (frontend) — se precisar |
|---|---|---|
| TOML | **`toml_edit` 0.25** (format-preserving, mantém comentários e espaçamento; base do `cargo add`). `toml` 1.x só para (de)serialização com serde (pack.toml/index.toml/metafiles). | `@decimalturn/toml-patch` 3.3 (patch preservando formatação) ou `@shopify/toml-patch` (WASM do `toml_edit`); `smol-toml` 1.9 para leitura simples. |
| JSON / JSONC | **`jsonc-parser` 0.34 com feature `cst`** (API de manipulação que preserva comentários e formatação). `serde_json` para leitura. | **`jsonc-parser` 3.3** (Microsoft/VS Code): `modify()` + `applyEdits()` preservam formatação. |
| JSON5 / Jankson | **`json-five` 0.3** ("round-trip capabilities", compatível com serde) — avaliar na prática; fallback: leitura com `json5` 1.3 e edição textual por faixa de bytes. | `json5` 2.2 (não preserva formatação). |
| YAML | **`yaml-edit` 0.3** ("lossless parser and editor", recente — avaliar maturidade); leitura com `yaml-rust2` 0.13 ou `saphyr`. **Evitar `serde_yaml`** (marcado *deprecated* em 2024) e `serde_yml` (marcado *unmaintained*). | **`yaml` 2.9** (eemeli): API `Document` preserva comentários — a opção mais madura. |
| `.properties` | parser próprio orientado a linhas (preserva comentários `#`/`!`, continuações `\`, escapes `\uXXXX`); `java-properties` 2.0 (parado desde 2023) só para leitura. | — |
| `.cfg` Forge legado | parser próprio (5.3). | — |
| `options.txt` | parser próprio (linha a linha). | — |
| NBT (`servers.dat`) | **`fastnbt` 2.6** (serde; suporta NBT não comprimido) ou `simdnbt` 0.10; `quartz_nbt`/`valence_nbt` estão parados. | `prismarine-nbt` 2.8. |
| Zip/jar (metadados de mods, export) | **`zip` 8.x** | — |
| Bytecode Java (`@Mod` em 1.7.10/1.12.2) | **`noak` 0.7** (ativo em 2026) ou `cafebabe` 0.9 | — |
| Versões | `semver` (só para casos SemVer puros); comparação FlexVer: `flexver-rs` está parado (2022) — portar o algoritmo FlexVer (<https://github.com/unascribed/FlexVer>) é simples; faixas Maven: implementação própria. | — |

Testes obrigatórios para qualquer editor estruturado: *round-trip* (ler e gravar sem mudanças ⇒ bytes idênticos) com um corpus de configs reais de mods populares em cada formato, e *edição mínima* (mudar um valor ⇒ diff de uma linha).

---

## 6. Implicações para o Warden

### 6.1 Integração com o packwiz: subprocesso, TOML direto ou híbrido?

**Avaliação das alternativas** (com base nas seções 1.4–1.13):

| Critério | Só CLI (subprocesso) | Só TOML direto (reimplementar) | Híbrido recomendado |
|---|---|---|---|
| Robustez de resultado | Ruim: sem JSON (#139), erros no stdout, exit 0 em falhas, ANSI no output, panics (#385) | Boa (controle total) | Boa: o estado é sempre lido dos arquivos |
| Cobertura de requisitos | Falha em 1.7.10/1.8.9/... (#393), sem side/option/preserve por comando, busca com `-y` escolhe o 1º resultado, dependências escolhidas pelo packwiz, sobrescrita silenciosa | Total | Total |
| Fidelidade ao formato packwiz | Máxima | Risco de divergência sutil | Máxima: o packwiz valida/gera o index |
| Desempenho | Downloads sequenciais, 1 requisição por mod em updates (429, #376) | Paralelismo e endpoints em lote | Idem ao direto |
| Dependência de um projeto pouco ativo/sem releases | Total | Nenhuma | Baixa (só refresh/validação/exports legados) |

**Recomendação: híbrido, com o Warden como dono do modelo de dados e o packwiz como motor de index/compatibilidade.**

1. **Leitura sempre direta** em Rust (`serde` + `toml`) de `pack.toml`, `index.toml` e `*.pw.toml`. Nunca inferir estado a partir do stdout.
2. **Escrita direta** (Rust, `toml_edit` para preservar o que já existe) de:
   - `pack.toml` na criação (o Warden mantém sua própria lista de versões do MC e de loaders, inclusive Forge com sufixo — contorna #393) e em mudanças de versão/loader;
   - metafiles `.pw.toml` ao adicionar/atualizar/trocar de fonte, usando os dados que o Warden já obteve da API (projeto, versão escolhida pelo usuário, arquivo `primary`, hash `sha512` no Modrinth / `sha1` na CF, `side` calculado, `option`, `pin`);
   - campos que nenhum comando expõe: `side`, `[option]`, `preserve` no index, `[export.curseforge]`, `[options]`.
   - Emitir no mesmo estilo do packwiz (sem indentação, mesma ordem de chaves) para que um `refresh` posterior não gere diff.
3. **Motor = packwiz CLI** (binário embutido) para `refresh` depois de cada lote de mudanças: `packwiz --pack-file <abs>/pack.toml refresh`, com `cwd` na pasta do pack, timeout, `CREATE_NO_WINDOW`, `--cache` apontando para uma pasta do Warden, saída capturada para log. Sucesso = exit 0 **e** `index.toml` relido coerente com os arquivos em disco (o Warden pode verificar hashes por amostragem).
4. **Validação de conformidade** com o próprio packwiz: na exportação e na suíte de testes do Warden, copiar o pack para um diretório temporário, rodar `packwiz refresh` e `packwiz list` e exigir **zero diff** — prova de que o pack é "o que o packwiz produziria".
5. **Não usar via CLI**: `init` (bug #393), `mr add`/`cf add` por busca ou com `-y` (escolhe resultado/dependências sozinho), `update --all` (sem rate limit, exit 0 em falhas parciais), `migrate` (dispara `update --all`), `serve` (desnecessário), `cf detect` (usar a API de fingerprints direto). Se por algum motivo usar `add`, sempre com IDs explícitos e **sem** `-y`, respondendo "n" ao prompt de dependências via stdin e adicionando dependências pelo fluxo do Warden.
6. **Exportações `.mrpack`/zip da CurseForge**: fase 1 pode chamar `packwiz mr export`/`cf export` **sobre uma cópia limpa** (6.3) e validar o zip resultante (abre? tem `modrinth.index.json`/`manifest.json` válido? nenhum arquivo fora da whitelist?); fase 2: export nativo em Rust que corrige as perdas (configs em `client-overrides/`/`server-overrides/`, não embutir jars de terceiros sem confirmação, trocar fonte CF→Modrinth por hash, id NeoForge 1.20.1 correto, entradas de diretório no zip).
7. **Binário**: compilar o packwiz no CI do Warden a partir de um **commit fixado** (não há releases), `CGO_ENABLED=0 GOOS=windows GOARCH=amd64 go build -trimpath -ldflags "-s -w -X github.com/packwiz/packwiz/curseforge.cfApiKey=<chave do Warden>"`, nomear com o target triple e registrar como sidecar do Tauri (`bundle.externalBin`); assinar junto com o app (mitiga #374); incluir a licença MIT do packwiz nos avisos de terceiros. Opcionalmente manter um fork mínimo (`Kriticales/packwiz`) só com correções que o Warden precise (ex.: #393, códigos de saída), enviando PRs upstream.

Observação: se o dono do projeto exigir que **toda** mutação passe pela CLI ("packwiz como motor" no sentido estrito), os itens 2 e 5 conflitam com isso — ver Questões em aberto.

### 6.2 Layout de pastas: projeto do pack ≠ instância de teste

O packwiz trata **tudo** que está na pasta do pack como conteúdo (só ignora git, `.DS_Store`, zips e o binário). Portanto:

- **Pasta do projeto** (o "pack"): `pack.toml`, `index.toml`, `.packwizignore`, `mods/*.pw.toml`, `resourcepacks/`, `shaderpacks/`, `config/`, `defaultconfigs/`, `kubejs/` etc. É o que vai para git e para a exportação.
- **Pasta da instância** (o ".minecraft" do launcher interno): separada, em diretório de dados do Warden. Montada a partir do projeto (jars do cache + cópia dos arquivos comuns) e **nunca** usada como pasta do pack.
- Metadados próprios do Warden (estado da última sincronização, baseline de configs para o diff, preferências do projeto) ficam em `.warden/` na raiz do projeto **e** essa pasta entra no `.packwizignore`; ou fora do projeto, se não precisarem ir para o git.
- Não usar symlinks/junctions dentro da pasta do pack (#191).

### 6.3 Regras de exportação e `.packwizignore` padrão

Regra geral: **whitelist para o que entra no pack, blacklist como rede de segurança**. O Warden decide o que copia da instância de volta para o projeto (6.5); o `.packwizignore` impede que lixo que chegue à pasta do projeto entre no index.

**Entra no pack (whitelist, relativo à raiz da instância):**

| Caminho | Regra |
|---|---|
| `mods/` | Somente como metafiles (Modrinth/CF/URL). Jar local só se o usuário confirmar (sem fonte conhecida); tentar antes identificar por hash (Modrinth `/version_files`, CF `/fingerprints`) e converter em metafile. Jars aninhados nunca. |
| `resourcepacks/`, `shaderpacks/` | Metafile se identificável; senão arquivo local. |
| `config/` | Arquivos alterados/adicionados pelo usuário (diff contra baseline), excluindo caches conhecidos de mods. |
| `defaultconfigs/` | Idem (configs de servidor padrão). |
| `kubejs/`, `scripts/`, `global_packs/`, `datapacks/` (se `datapack-folder`), `openloader/`, `resources/` | Só se existirem e o usuário habilitar. |
| `options.txt`, `optionsof.txt`, `optionsshaders.txt` | Opcional, com aviso de que sobrescreve preferências; sugerir `preserve = true` no index e/ou YOSBR. |
| `servers.dat` | Opcional, com `preserve = true`. |

**Nunca entra** (nem por cópia, nem no index): `logs/`, `crash-reports/`, `saves/` (exceto mundo explicitamente marcado), `screenshots/`, `debug/`, `.mixin.out/`, `.fabric/`, `.quilt/`, `.cache/`, `.connector/`, `natives/`, `libraries/`, `versions/`, `assets/`, `*.log`, `hs_err_pid*.log`, `usercache.json`, `usernamecache.json`, `launcher_profiles*.json`, `servers.dat_old`, `command_history.txt`, dados de minimapa/waypoints (`journeymap/data/`, `XaeroWaypoints/`, `xaero/`), `.warden/`, arquivos de sistema (`Thumbs.db`, `desktop.ini`, `.DS_Store`), exports (`*.mrpack`, `*.zip` na raiz), binários do packwiz. **[a lista de pastas de dados de mods é baseada em conhecimento geral; revisar com instâncias reais]**

**`.packwizignore` padrão gerado pelo Warden:**

```gitignore
# Gerado pelo Warden — não inclua dados de execução no pack
.warden/
.git/
logs/
crash-reports/
saves/
screenshots/
debug/
.mixin.out/
.fabric/
.quilt/
.cache/
.connector/
natives/
libraries/
versions/
assets/
journeymap/data/
XaeroWaypoints/
xaero/
*.log
hs_err_pid*.log
usercache.json
usernamecache.json
launcher_profiles*.json
servers.dat_old
command_history.txt
Thumbs.db
desktop.ini
*.tmp
*.bak
/*.mrpack
/*.zip
packwiz.exe
```

**Regras de exportação (formato packwiz nativo, que é o requisito):**

1. Antes de exportar: `refresh` + validação de conformidade (6.1, item 4) + varredura que **lista ao usuário** todo arquivo não-metafile do index (para ele ver exatamente o que vai em `overrides/`).
2. Exportar = copiar para o destino apenas `pack.toml`, `index.toml`, `.packwizignore` e os arquivos listados no index (nada mais), em vez de copiar a pasta inteira.
3. Garantir `version` preenchido em `pack.toml` (o `.mrpack` exige `versionId`).
4. Se houver mods CF com distribuição bloqueada, avisar já no momento de adicionar (o packwiz só descobre no export) e, na exportação `.mrpack`, oferecer trocar para Modrinth (match por hash) em vez de embutir.
5. Exportações `.mrpack`/CF são derivadas e com perda; mostrar ao usuário o que se perde (side na CF; `default`/`description` de opcionais; side de configs).

### 6.4 Desenho do diagnóstico determinístico (antes da IA)

Pipeline executado ao clicar em "Testar" (e sob demanda):

1. **Materializar**: garantir que todos os arquivos do pack estejam no cache (verificar hash do metafile); mods CF bloqueados ausentes → achado `E-DOWNLOAD-BLOCKED` com link e fluxo "baixe no navegador; o Warden detecta na pasta Downloads por hash" (modelo Prism).
2. **Identificar** cada jar: sha1/sha512/murmur2 → Modrinth `POST /v2/version_files` (sha1) e CF `POST /v1/fingerprints/432` (murmur2). Resultado: projeto, versão, `game_versions`, `loaders`, `environment`, dependências declaradas na plataforma.
3. **Ler metadados locais** do jar e jars aninhados (seção 4) → modelo normalizado.
4. **Aplicar regras** (cada uma gera `Finding { severidade, código, mods[], mensagem, evidência, correções sugeridas }`):

| Código | Regra | Severidade | Fonte |
|---|---|---|---|
| `E-LOADER` | Jar não tem metadados do loader do pack (ex.: só `fabric.mod.json` num pack NeoForge) ou a versão da plataforma não lista o loader. Exceção: mods "forge" em pack NeoForge 1.20.1 (compatível) e ≥1.20.2 (aviso). | erro | jar + API |
| `E-MCVERSION` | `depends.minecraft`/`versionRange` de `minecraft`/`acceptedMinecraftVersions`/`mcversion` não aceita a versão do pack; ou `game_versions` da API não contém a versão (se estiver em `acceptable-game-versions`, rebaixa para aviso). | erro/aviso | jar + API |
| `E-LOADERVERSION` | `fabricloader`, `loaderVersion`, dependência `forge`/`neoforge` fora da faixa da versão de loader do pack. | erro | jar |
| `E-MISSING-DEP` | Dependência obrigatória ausente (Fabric `depends`, Quilt `depends` sem `optional`, Forge `mandatory=true`, NeoForge `type=required`, `required-after` no `@Mod`, API `required`/`relationType 3`), considerando `provides` e jars aninhados, e respeitando `side` da dependência. | erro | jar + API |
| `E-DEP-VERSION` | Dependência presente mas fora da faixa (inclui opcional presente fora da faixa no Forge/NeoForge). | erro | jar |
| `E-BREAKS` | Fabric `breaks`, Quilt `breaks`, NeoForge `incompatible` com o outro mod presente (e na faixa). | erro | jar |
| `W-CONFLICTS` | Fabric `conflicts`, NeoForge `discouraged`, `recommends` não satisfeito. | aviso | jar |
| `W-INCOMPATIBLE-API` | Modrinth `dependency_type = incompatible` ou CF `relationType = 5` com o projeto presente. | aviso alto (declarado pelo autor, mas sem faixa de versão) | API |
| `E-DUPLICATE-ID` | Mesmo mod id em dois jars (Fabric e Forge recusam iniciar). | erro | jar |
| `E-DUPLICATE-FILE` | Mesmo hash duas vezes; mesmo projeto via Modrinth e CF; dois metafiles para o mesmo projeto. | erro | hash + API |
| `W-SIDE` | `environment`/`clientSideOnly` = cliente mas metafile `both`/`server` (e vice-versa); relevante para export servidor. | aviso | jar + API |
| `E-JAVA` | `depends.java`, `[features] javaVersion` ou versão do MC exigem Java diferente do que o launcher vai usar. | erro | jar + tabela MC→Java |
| `W-PRERELEASE` | Versão escolhida é `alpha`/`beta` (o packwiz não filtra). | info/aviso | API |
| `W-LEGACY-COREMOD` | `FMLCorePlugin`/`TweakClass` em 1.7.10/1.12.2; vários coremods. | info | MANIFEST |
| `W-PACK-FORMAT` | `pack.mcmeta` de resource/data pack com `pack_format` incompatível com a versão. | info | zip |
| `K-KNOWN` | Base curada de incompatibilidades conhecidas e não declaradas (ex.: OptiFine com Sodium/Embeddium) — JSON versionado no repositório do Warden. | erro/aviso | curada |

5. **Saída**: relatório estruturado (JSON) para a UI e, só depois, como contexto compacto para o Gemini (lista de mods com id/versão/loader/ambiente + achados determinísticos + trecho do log em caso de crash). Erros bloqueiam "Testar" por padrão, com "testar mesmo assim".
6. **Avaliadores de faixa**: implementar e testar três dialetos — SemVer/predicados do Fabric (`*`, `^`, `~`, `x`, operadores, arrays OR), Maven ranges (`[a,b)`, `(,b]`, `a` = mínimo, uniões com vírgula) e strings `@Mod.dependencies`. Comparação de versões: SemVer quando válido, FlexVer como fallback (é o que o packwiz usa).
7. **Cache**: respostas do Modrinth podem ser cacheadas localmente (por hash de arquivo, imutáveis); respostas da CurseForge **não devem ser persistidas** além da sessão (ToS §3.1(e)) — guardar só IDs e hashes no próprio pack.

### 6.5 Configs, teste e "trazer de volta para o pack"

1. Ao montar a instância, gravar um **baseline** (hash + conteúdo) de cada arquivo copiado do pack e de cada arquivo de config gerado na primeira execução.
2. Após o teste, comparar a instância com o baseline nas pastas da whitelist (6.3) e mostrar: arquivos novos, alterados e removidos, com diff por formato (diff estrutural para TOML/JSON/JSON5/properties/options.txt/.cfg; diff de texto para o resto).
3. O usuário escolhe o que trazer; o Warden copia para o projeto e roda `refresh`.
4. Lembrar que configs do Forge/NeoForge são regeneradas pelo jogo (comentários do usuário somem) e que `options.txt` é reescrito inteiro; o editor deve avisar.
5. Editores estruturados só pelos módulos Rust da tabela 5.6, com testes de round-trip.

### 6.6 Clientes de API

- Um cliente HTTP único no Rust (`reqwest`), com `User-Agent: Kriticales/Warden/<versão> (<contato>)`, limitador de taxa (≤300/min no Modrinth, respeitando `X-Ratelimit-*` e `429`, com backoff), uso de endpoints em lote (`/projects?ids=`, `/versions?ids=`, `/version_files`, `/version_files/update`, `POST /v1/mods`, `POST /v1/mods/files`).
- Chave da CurseForge própria do Warden, injetada no build (no Rust e no binário do packwiz via `-ldflags`), nunca em código-fonte público; tratar ausência de chave desabilitando a fonte CF graciosamente.
- Busca: sempre na UI do Warden, com filtros de loader, versão e tipo de projeto (o packwiz não filtra loader na busca do Modrinth); a escolha do usuário vira ID explícito.
- Dependências: o Warden resolve `required` (como o packwiz), **mostra** `optional` e **avisa** `incompatible` — tudo antes de gravar metafiles.
- Deduplicação entre fontes: antes de criar um metafile, procurar o mesmo projeto/arquivo já presente (por project ID na mesma fonte e por hash entre fontes); nunca sobrescrever um `.pw.toml` existente sem confirmação (o packwiz sobrescreve silenciosamente).

### 6.7 Lista priorizada de requisitos derivados

**P0 (fundação)**
1. Modelo de dados Rust do formato packwiz (`pack.toml`, `index.toml`, `.pw.toml`) com leitura/escrita compatíveis byte a byte com a saída do packwiz.
2. Criação de pack pelo Warden (sem `packwiz init`), com catálogo próprio de versões do MC e loaders (Forge legado com sufixo tratado).
3. packwiz embutido como sidecar, compilado de commit fixado com chave CF do Warden; wrapper de subprocesso com timeout, sem janela, log de saída e remoção de ANSI.
4. `refresh` via packwiz após mutações + verificação por releitura; teste de conformidade (zero diff) no CI e na exportação.
5. Separação pasta do projeto × pasta da instância; `.packwizignore` padrão (6.3).
6. Clientes Modrinth/CF com rate limit, UA identificável e endpoints em lote.

**P1 (funcionalidades principais)**
7. Adicionar mod por busca na UI (Modrinth e CF), arquivo local e URL, com resolução de dependências controlada pelo Warden, aviso de incompatibilidades e deduplicação.
8. Detecção antecipada de mods CF com distribuição bloqueada + fluxo de download manual com detecção por hash na pasta Downloads.
9. Diagnóstico determinístico (6.4) com parsers de `fabric.mod.json`, `quilt.mod.json`, `mods.toml`, `neoforge.mods.toml`, `mcmod.info` e jars aninhados.
10. Exportação do pack no formato packwiz nativo, com lista explícita do que vai em overrides.
11. Diff de configs pós-teste e "trazer de volta".

**P2 (qualidade/extra)**
12. Editores estruturados com round-trip para TOML (Forge), JSON/JSONC, JSON5, `.properties`, `options.txt`, `.cfg` legado, `servers.dat`.
13. Export `.mrpack`/CF nativo sem as perdas do packwiz; troca automática CF→Modrinth por hash.
14. Leitura de `@Mod` no bytecode para 1.7.10/1.12.2.
15. Base curada de incompatibilidades conhecidas.
16. Importação de `.mrpack` e de zip CF.

---

## 7. Questões em aberto

1. **"packwiz como motor" no sentido estrito?** A recomendação (6.1) escreve `pack.toml` e metafiles diretamente e usa a CLI só para `refresh`, validação e exportações iniciais. Se o dono do projeto quiser que toda mutação passe pela CLI, será preciso conviver com #393 (Forge legado), escolhas automáticas de dependência/busca e ausência de comandos para `side`/`option`/`preserve` — ou manter um fork do packwiz com essas correções e uma saída JSON. **Decisão do dono do projeto.**
2. **Fabric em 1.7.10/1.12.2**: o Fabric oficial só suporta 1.14+. Em versões antigas existem **Legacy Fabric** (loader `legacy-fabric` no Modrinth), **Ornithe** e **Babric**. O packwiz não conhece esses loaders (`GetCompatibleLoaders` só trata `fabric`, `quilt`, `forge`, `neoforge`), o `.mrpack` não tem chave de dependência para eles e o zip da CF também não. O requisito "Fabric em qualquer versão" precisa ser redefinido (Forge para versões antigas e Fabric a partir de 1.14?) ou exigirá suporte próprio do Warden fora do packwiz.
3. **NeoForge** só existe a partir de 1.20.1; não há o que fazer em versões antigas. Confirmar que a UI esconde a opção.
4. **Chave de API da CurseForge**: precisa ser solicitada pelo formulário da Overwolf (sem prazo de aprovação publicado). Até lá, a fonte CF fica indisponível no Warden (não usar a chave do packwiz no código próprio). Confirmar também a interpretação de "não cachear dados" (§3.1(e)) para o diagnóstico (que precisa dos dados da CF no momento do "Testar").
5. **Fork do packwiz**: vale manter `Kriticales/packwiz` com correções (#393, códigos de saída coerentes, eventualmente `--json`) e enviar PRs upstream? O projeto upstream tem manutenção esporádica e nenhum processo de release.
6. **Licenças ao exportar**: embutir jars de Modrinth/URL num zip da CF (ou jars da CF num `.mrpack`) depende de licença; o Warden deve bloquear, avisar ou deixar o usuário decidir? Sugestão: bloquear por padrão e trocar de fonte por hash quando possível.
7. **Itens a confirmar em fonte primária** (marcados como inferência no texto): ID de classe dos shaders na CF (6552?); comportamento exato de `defaultconfigs/` no Forge e da sobreposição global/por-mundo no NeoForge; regravação dos TOML pelo NightConfig (perda de comentários); caminhos do YOSBR; estrutura de `META-INF/jarjar/metadata.json`; prefixo `file/` em `resourcePacks` por versão; existência de `clientSideOnly` no `@Mod` do 1.7.10; versão exata do NeoForge em que `type` substituiu `mandatory`.
8. **Tabela MC → versão de Java** (8/16/17/21/25...) e download do runtime: fora do escopo desta pesquisa (launcher), mas é insumo da regra `E-JAVA`.
9. **Escopo do diagnóstico em packs grandes**: identificar centenas de jars por hash custa poucas requisições em lote no Modrinth, mas fingerprints da CF também precisam de lote; medir latência real para decidir se o diagnóstico roda inteiro a cada "Testar" ou incrementalmente (só mods alterados).
10. **Quilt**: o packwiz suporta e o custo é baixo, mas não está nos requisitos. Incluir ou não na UI?

---

### Fontes principais

- packwiz (código, commit `ef87d96`): <https://github.com/packwiz/packwiz> — arquivos citados: `cmd/*.go`, `core/{pack,index,indexfiles,mod,hash,download,versionutil,storeutil}.go`, `modrinth/{install,modrinth,export,updater,pack}.go`, `curseforge/{curseforge,request,install,export,import,detect}.go`, `curseforge/packinterop/*.go`, `url/install.go`, `migrate/*.go`, `cmdshared/*.go`.
- Issues do packwiz: <https://github.com/packwiz/packwiz/issues> (#29, #52, #82, #97, #115, #139, #145, #150, #155, #167, #191, #211, #228, #300, #336, #349, #366, #374, #376, #385, #393, #403, #417).
- Documentação do packwiz: <https://packwiz.infra.link/> (fonte em <https://github.com/packwiz/packwiz-website>).
- packwiz-installer: <https://github.com/packwiz/packwiz-installer>; bootstrap: <https://github.com/packwiz/packwiz-installer-bootstrap>.
- Maven do Forge: <https://maven.minecraftforge.net/releases/net/minecraftforge/forge/maven-metadata.xml>.
- Formato `.mrpack`: <https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack>.
- Modrinth API: <https://docs.modrinth.com/api/>, <https://docs.modrinth.com/api/operations/searchprojects/>, <https://docs.modrinth.com/api/operations/getprojectversions/>.
- CurseForge API: <https://docs.curseforge.com/rest-api/>; termos: <https://support.curseforge.com/support/solutions/articles/9000207405-curse-forge-3rd-party-api-terms-and-conditions>; chave: <https://support.curseforge.com/support/solutions/articles/9000208346-about-the-curseforge-api-and-how-to-apply-for-a-key>; distribuição: <https://support.curseforge.com/support/solutions/articles/9000207877-project-distribution-toggle>; export de modpack: <https://support.curseforge.com/support/solutions/articles/9000197908-exporting-a-modpack-for-curseforge-project-submission>.
- Prism Launcher (referência de UX/chave CF): <https://github.com/PrismLauncher/PrismLauncher/issues/263>, <https://github.com/PrismLauncher/PrismLauncher/pull/304>.
- Fabric: <https://wiki.fabricmc.net/documentation:fabric_mod_json_spec>, <https://github.com/FabricMC/fabric-loader>.
- Quilt: <https://github.com/QuiltMC/rfcs/blob/main/specification/0002-quilt.mod.json.md>.
- Forge: <https://docs.minecraftforge.net/en/latest/gettingstarted/modfiles/>, <https://docs.minecraftforge.net/en/1.12.x/gettingstarted/structuring/>, <https://github.com/MinecraftForge/MinecraftForge>.
- NeoForge: <https://docs.neoforged.net/docs/gettingstarted/modfiles/>, <https://docs.neoforged.net/docs/misc/config/>, <https://neoforged.net/news/20.5release/>.
- Minecraft Wiki: <https://minecraft.wiki/w/Options.txt>, <https://minecraft.wiki/w/Servers.dat_format>.
- Tauri sidecar: <https://v2.tauri.app/develop/sidecar/>.
- crates.io / npm (versões consultadas em 2026-10-01): `toml_edit`, `toml`, `jsonc-parser`, `json-five`, `json5`, `yaml-rust2`, `saphyr`, `yaml-edit`, `serde_yaml`, `serde_yml`, `java-properties`, `fastnbt`, `simdnbt`, `zip`, `noak`, `cafebabe`, `semver`, `flexver-rs`; npm `jsonc-parser`, `yaml`, `smol-toml`, `@decimalturn/toml-patch`, `@shopify/toml-patch`, `json5`, `prismarine-nbt`.
