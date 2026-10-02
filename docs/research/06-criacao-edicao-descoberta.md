# R5B — Criação, edição e descoberta de conteúdo

> Tarefa R5B, 01/10/2026. Agente de pesquisa. Cobre as funções **(5) a (10)** pedidas pelo dono: configs com formulário inteligente e busca, editor de scripts KubeJS/CraftTweaker, testes avançados, importar/exportar/kits/templates, navegar por outros modpacks e a página dedicada de descoberta de mods. As funções (1) a (4) e os mods padrão (spark, crash report) ficam com o relatório paralelo R5A.
> Base: `docs/SPEC.md` 1.1, `docs/ARCHITECTURE.md`, `docs/design/ESTRUTURA.md` (Alternativa A aprovada), relatórios R1 a R4 e o spike S1. Não repito o que já está lá: cito a seção e aprofundo.
> Marcas: **[verificado]** = executei; **[código]** = li o código-fonte; **[inferência]** = dedução não confirmada; **[documentação]** = documentação oficial lida, sem teste. Os experimentos rodaram em `~/.local/share/warden-r5/b/` (fora do repositório), com o JDK 21 do spike S1. A chave da CurseForge **não** foi usada.

## Sumário

0. [Resumo](#0-resumo)
1. [Onde cada função entra na estrutura aprovada](#1-onde-cada-função-entra-na-estrutura-aprovada)
2. [(5) Configs com formulário inteligente, busca e comparação com o padrão](#2-5-configs-com-formulário-inteligente-busca-e-comparação-com-o-padrão)
3. [(6) Editor de scripts KubeJS e CraftTweaker](#3-6-editor-de-scripts-kubejs-e-crafttweaker)
4. [(7) Testes avançados](#4-7-testes-avançados)
5. [(8) Importar, exportar server pack, kits e templates](#5-8-importar-exportar-server-pack-kits-e-templates)
6. [(9) Navegar por outros modpacks](#6-9-navegar-por-outros-modpacks)
7. [(10) Página dedicada para descobrir e adicionar mods](#7-10-página-dedicada-para-descobrir-e-adicionar-mods)
8. [Dependências entre as funções](#8-dependências-entre-as-funções)
9. [Implicações para o Warden](#9-implicações-para-o-warden)
10. [Questões para o dono](#10-questões-para-o-dono)
11. [Achados fora do escopo (para o orquestrador)](#11-achados-fora-do-escopo-para-o-orquestrador)
12. [Fontes](#12-fontes)

---

## 0. Resumo

| # | Função | Viável no Warden? | Esforço | Prioridade proposta | Onde fica |
|---|---|---|---|---|---|
| 5a | Formulário de configs a partir dos comentários do arquivo | Sim; os formatos e os metadados foram confirmados no código dos loaders e das bibliotecas | M | P1 (v1), já previsto (C-04) | Configs (alternância Formulário/Texto, já aprovada) |
| 5b | Busca em todas as configs (nome da chave, comentário, valor, rótulo traduzido) | Sim, índice em memória no Rust | P | **P0** | Configs (campo de busca no topo da árvore) |
| 5c | Comparar com o padrão e "restaurar padrão" | Sim, em camadas: comentário `Default:` → padrões gerados por um servidor local → padrões gerados por uma abertura limpa do cliente | M/G | P1 (camada A), P2 (camadas B e C) | Configs (marca "alterado" por chave + filtro "Só o que mudou do padrão") |
| 6 | Editor KubeJS/CraftTweaker com realce, autocompletar e verificação | Sim. Realce e trechos prontos são baratos. O autocompletar de verdade depende do **ProbeJS** (gera tipagens) e de um servidor de linguagem TypeScript **fora do WebView** (o `tsc` 7 tem `--lsp --stdio`) | M (realce + recarga), G (autocompletar) | P1 (realce, erros do log, recarga); P2 (autocompletar via ProbeJS) | Configs (os scripts já estão na árvore: `kubejs/`, `scripts/`) |
| 7a | Perfis de teste (memória/Java/flags/opcionais) | Sim, extensão direta do T11 | P | P1 | Menu ▾ do Testar |
| 7b | Servidor dedicado local | Sim; o portablemc **não** instala servidor, mas os instaladores oficiais fazem isso sem interface, e o ciclo instalar → "Done" → parar pelo stdin foi verificado em Fabric e NeoForge | M/G | P1 (P2 na SPEC hoje) | Menu ▾ do Testar → "Testar com servidor local" |
| 7c | Teste "como o jogador recebe" pelo link do GitHub | Sim; o bootstrap real instalou um pack público em 6,9 s, mas **falha** se a API do GitHub estiver no limite (verificado) | P/M | P1 (já é L-07) | Menu ▾ do Testar (já aprovado) |
| 8a | Importar `.mrpack`, zip da CurseForge e instância do Prism | Sim; o packwiz só importa CurseForge, então `.mrpack` e Prism são do Warden | M | P1 | Meus packs → "Abrir pack existente" vira "Abrir ou importar" |
| 8b | Exportar server pack | Sim, com filtro por lado + scripts de início + `user_jvm_args.txt` | M | P1 | Exportar (novo formato) |
| 8c | Kits de performance e templates | Sim, como dados versionados no repositório e validados no CI pela API do Modrinth | M | Kits P1 (a SPEC hoje diz P2); templates P2 | Criar pack (etapa 4) e Mods → Adicionar |
| 9 | Navegar por outros modpacks e trazer mods deles | Sim. No Modrinth, a lista de mods de uma versão de modpack vem **da própria API** (dependências `embedded`) ou do `modrinth.index.json` lido com 5 requisições Range (verificado). Na CurseForge, lendo o `manifest.json` por Range | M | P1 | Página de descoberta, Tipo = Modpacks |
| 10 | Página dedicada de descoberta | Sim; amplia a página Adicionar já aprovada (T08) | M | **P0** (a base já é P0) + P1 (destaques, galeria, changelog) | Mods → Adicionar, em tela cheia, sem virar seção do menu |

Achados que mudam decisões ou pedem correção em documentos já aprovados (detalhes nas seções e no §11):

1. **O KubeJS 7 grava um token de acesso em `kubejs/config/web_server.json`**, dentro da pasta que vai no pack [código]. Hoje esse arquivo iria para o GitHub e para os jogadores. Precisa entrar no `.packwizignore` padrão e na verificação de segredos.
2. **O NeoForge corrige sozinho valores fora da faixa e apaga chaves desconhecidas**, guardando cópias `*-N.toml.bak` [código]. O formulário pode validar antes do teste e o console pode avisar "sua config foi resetada".
3. **O packwiz-installer-bootstrap depende da API do GitHub sem login (60 requisições/hora por IP).** No limite, ele falha com HTTP 403 e não instala nada [verificado]. O teste "como o jogador recebe" deve usar o installer já baixado (`--bootstrap-no-update`).
4. **A API do Modrinth já entrega a lista de mods de uma versão de modpack** como dependências `embedded` (154 de 155 arquivos no exemplo) [verificado]. Para ver os mods de um modpack, não é preciso baixar o arquivo dele.
5. **O `tsc` 7.0 (TypeScript nativo, Apache-2.0) é um binário único de 24 MB com `--lsp --stdio`** [verificado]. O ProbeJS 8 gera tipagens pensadas justamente para ele. Isso torna viável um autocompletar sério de KubeJS sem Node e sem rodar TypeScript dentro do WebView.

---

## 1. Onde cada função entra na estrutura aprovada

Regra do ESTRUTURA §1: dois níveis, um nível de navegação visível por vez, nada de abas dentro de abas, o raro escondido. A proposta abaixo **não cria seção nova** no menu do pack. As 6 seções continuam: Mods, Configs, Problemas, ✦ Diagnóstico com IA, Histórico e Exportar.

| Função | Onde entra | Mudança na estrutura |
|---|---|---|
| Página de descoberta (10) e outros modpacks (9) | A página **Adicionar** (T08) vira uma **página em tela cheia** dentro do pack, aberta por **Adicionar mods** em Mods. Ganha uma tela inicial (populares e atualizados para a versão do pack, categorias) quando o campo de busca está vazio. O seletor **Tipo** ganha a opção **Modpacks**. | Pequena: a página já existe; muda o conteúdo da tela vazia e entra um valor no seletor Tipo. Sem nova seção. Ver §7.6. |
| Formulário, busca e padrão (5) | **Configs**: campo "Buscar em todas as configs" acima da árvore; filtro "Só o que mudou do padrão"; no formulário, cada chave mostra "padrão: X" e "Restaurar padrão". | Nenhuma: tudo cabe na tela de Configs (alternância Formulário/Texto já aprovada, T12). |
| Scripts (6) | **Configs**, porque `kubejs/` e `scripts/` já aparecem na árvore (T12). Arquivos `.js`/`.zs` abrem no mesmo editor com recursos de script. Durante o teste: botão **Recarregar scripts** na tela do teste. | Nenhuma. Renomear a descrição da seção para "Arquivos de ajuste e scripts do pack" (sugestão, §10). |
| Perfis de teste (7a) | Menu **▾** do Testar → "Ajustes do teste neste computador…" passa a ter **perfis** (caixa de seleção no topo do diálogo). O botão Testar mostra o perfil ativo quando não é o padrão ("▶ Testar · 4 GB"). | Nenhuma. |
| Servidor local (7b) | Menu **▾** do Testar → **Testar com servidor local**. A tela do teste mostra dois consoles (jogo e servidor) com uma caixa de seleção "Mostrando: jogo / servidor" (não abas). | Nenhuma. |
| Como o jogador recebe (7c) | Menu ▾ do Testar (já aprovado). | Nenhuma. |
| Importar (8a) | **Meus packs**: o botão "Abrir pack existente" vira **Abrir ou importar…** e aceita pasta packwiz, `.mrpack`, zip da CurseForge ou pasta de instância do Prism/CurseForge. | Troca de rótulo (§10). |
| Server pack (8b) | **Exportar**: novo formato "Pacote para servidor (.zip)". | Nenhuma. |
| Kits e templates (8c) | **Criar pack**, etapa 4 (resumo): "Começar com: pack vazio / kit de desempenho / modelo…". Na página Adicionar: "Adicionar kit de desempenho" na tela inicial. | Nenhuma. |

---

## 2. (5) Configs com formulário inteligente, busca e comparação com o padrão

O R3 §5 já lista os formatos, as bibliotecas de edição sem perda (`toml_edit`, `jsonc-parser` com `cst`, parsers próprios) e as regras de *round-trip*. A ARCHITECTURE §10 fixou a divisão: o Rust edita, a interface recebe uma árvore e devolve `{ caminho, novoValor }`. Esta seção trata do que o R3 não cobriu: **de onde vêm tipos, limites, padrões e rótulos**, como inferir o que falta, a busca e a comparação com o padrão.

### 2.1 O que cada formato traz dentro do próprio arquivo

Tudo abaixo foi confirmado no código-fonte da versão indicada **[código]** ou num arquivo gerado de verdade **[verificado]** (configs das instâncias do spike S1 em `~/.local/share/warden-spike/theseus/profiles/`).

| Formato (gerador) | Onde fica | Tipo | Faixa | Valores permitidos | Padrão | Descrição | Rótulo bonito |
|---|---|---|---|---|---|---|---|
| **NeoForge 1.21.x `ModConfigSpec`** | `config/<modid>-{client,common,startup,server}.toml` | Pelo valor TOML (bool, int, float, string, lista) | `# Range: 1 ~ 64` (só em `defineInRange`) | `#Allowed Values: A, B, C` (só enums) | `# Default: X` **só para valores com faixa** | Comentário do mod (linhas `#` acima da chave) | Chave de tradução `<modid>.configuration.<chave>` e `.tooltip` no `assets/<modid>/lang/*.json` do jar |
| **Forge 1.13–1.20.x `ForgeConfigSpec`** | igual, `-server.toml` por mundo | Pelo valor | `#Range: 1 ~ 64` | `#Allowed Values: …` | **Não grava** | Comentário do mod | Depende do mod (não há padrão de chave) |
| **Forge 1.7.10–1.12.2 `Configuration`** (`.cfg`) | `config/*.cfg` | **Prefixo** `B:` `I:` `D:` `S:` (e listas `< >`) | `[range: 0 ~ 10, default: 5]` no fim do comentário (`getInt`/`getFloat`) | `validValues` não é escrito no arquivo pela API clássica | `[default: X]` no fim do comentário (todos os `get*` com comentário) | Comentário | `langKey` existe na API, mas não vai para o arquivo |
| **Forge 1.12.2 `@Config`** (anotações) | `config/<nome>.cfg` | Prefixo | Linhas `Min: X` e `Max: Y` no comentário | `Valid values:` seguido de um valor por linha (enums) | **Não grava** | `@Comment` | `@LangKey` (não vai para o arquivo) |
| **YACL v2 (`GsonConfigSerializer`)** | `config/*.json` ou `*.json5` | Pelo valor JSON | Não grava | Não grava | Não grava | Só se o mod ligar JSON5 **e** preencher `@SerialEntry(comment=…)` | Telas do YACL usam texto definido no código |
| **Cloth Config / AutoConfig** | `config/<nome>.json` (Gson), `.json5` (Jankson) ou `.toml` (Toml4j) | Pelo valor | Não grava (`@BoundedDiscrete` fica no código) | Não grava | Não grava | Só Jankson grava `@Comment` | `text.autoconfig.<nome>.option.<campo>` no lang do jar |
| **owo-config** | `config/<nome>.json5` (Jankson) | Pelo valor | Não grava (`@RangeConstraint` fica no código) | Não grava | Não grava | Comentários do Jankson quando o mod anota | `text.config.<nome>.option.<campo>` no lang do jar |
| **MidnightLib** | `config/<modid>.json` (Gson, sem comentários) | Pelo valor | Não grava (`@Entry(min, max)` fica no código) | Não grava | Não grava | Não há | `<modid>.midnightconfig.<campo>` e `.tooltip` no lang do jar |
| `.properties` (Iris, OptiFine, `server.properties`) | vários | Texto (inferir) | — | — | — | Comentários `#` quando houver | — |
| `options.txt` | raiz | Texto (inferir; o R3 §5.4 descreve) | — | — | — | — | Nomes conhecidos do vanilla (tabela fixa no Warden) |

Exemplos reais (NeoForge 1.21.1, gerados no spike) [verificado]:

```toml
#A config option mainly for developers. Logs out modded tags that are using the 'forge' namespace when running on integrated server. Defaults to DEV_SHORT.
#Allowed Values: SILENCED, DEV_SHORT, DEV_VERBOSE, PROD_SHORT, PROD_VERBOSE
logLegacyTagWarnings = "DEV_SHORT"
#Set this to true to enable showing debug information about attributes on an item when advanced tooltips is on.
attributeAdvancedTooltipDebugInfo = true
```

Duas observações saem daí. Booleanos e strings **não** recebem `Default:` no NeoForge; o padrão às vezes aparece só em texto livre ("Defaults to DEV_SHORT."), que dá para extrair com uma expressão regular de baixa confiança. Já no `forge.cfg` do Forge 1.12.2 do spike nenhuma chave tem `[default: …]`, porque o próprio Forge usa a forma sem comentário de padrão [verificado]. Portanto, mesmo no formato "rico", **o padrão nem sempre está no arquivo**.

Fontes no código:
- `ModConfigSpec.defineInRange` grava `" Default: "` e `" Range: "`; `defineEnum` grava `"Allowed Values: "` (NeoForge, branch `1.21.1`, linhas 366–367 e 728).
- `ForgeConfigSpec` (Forge `1.20.1`) grava só `"Range: "` e `"Allowed Values: "` (linhas 334 e 485).
- `Configuration.java` (Forge `1.12.x`) acrescenta `" [range: " + min + " ~ " + max + ", default: " + def + "]"` e `" [default: …]"` (linhas 1544–1811). `FieldWrapper.java` (`@Config`) grava `"Min: "`, `"Max: "` e `"Valid values:"` (linhas 285–370).
- Tela de config automática do NeoForge: a chave de tradução padrão é `modId + ".configuration." + key`, e o tooltip é a mesma chave + `".tooltip"` (`ConfigurationScreen.java`, linhas 556–574).
- YACL: comentário só sai com JSON5 (`GsonConfigSerializer.java`, linhas 54–66). owo-config: `.json5` via Jankson e chave `"text.config." + configName + ".option." + key` (`ConfigWrapper.java` linha 221; `Option.java` linha 74). AutoConfig: `"text.autoconfig.%s"` + `"%s.option.%s"` (`ConfigScreenProvider.java`, linhas 53–55). MidnightLib: `config/<modid>.json`, `@Entry(min, max)` e chaves `<modid>.midnightconfig.<campo>` (`MidnightConfig.java`).

### 2.2 Como inferir tipo, limite e rótulo (algoritmo em camadas)

Para cada chave, o Warden monta um `ConfigField { caminho, tipo, valor, padrão?, faixa?, opções?, descrição?, rótulo?, confiança }`, aplicando as fontes em ordem. A primeira fonte que responder ganha, e a confiança diz ao formulário se o controle é "travado" (faixa conhecida) ou "livre com aviso":

1. **Metadados explícitos do arquivo** (confiança alta): `Range`, `Allowed Values`, `Default`, `[range: … default: …]`, `Min/Max`, `Valid values`, prefixo de tipo do `.cfg`.
2. **Tipo pelo valor** (alta para bool, inteiro e lista; média para float e string): `true/false` vira liga/desliga; inteiro vira número; lista TOML/JSON vira editor de lista; string vira texto.
3. **Heurísticas de texto** (baixa): `Defaults to X`, `default: X`, `(min X, max Y)`, `between X and Y`, e cores `#RRGGBB`/`0xAARRGGBB` (vira seletor de cor). Tudo marcado "deduzido".
4. **Rótulo e ajuda vindos do jar** (alta quando existe): o Warden já lê o jar para o diagnóstico (`warden-jarmeta`, P1-06). Basta também ler `assets/<modid>/lang/pt_br.json` (se houver; senão `en_us.json`) e procurar as chaves dos padrões conhecidos (NeoForge, AutoConfig, owo, MidnightLib). O formulário mostra o rótulo traduzido e, ao lado, a chave real. **Bônus:** vários mods já têm `pt_br.json`, e então o formulário sai em português sem tradução por IA [inferência: nem todo mod traduz as chaves de config].
5. **Anotações no bytecode** (P2, alta quando existe): `@Entry(min, max)` do MidnightLib, `@RangeConstraint` do owo e `@ConfigEntry.BoundedDiscrete` do AutoConfig ficam na tabela `RuntimeVisibleAnnotations` das classes. O crate `noak`, já citado no R3 §5.6 para o `@Mod`, lê isso. Exige achar a classe de config (pelo nome do arquivo e pelo `modid`). Esforço M; vale só para os mods Fabric mais usados.
6. **Dicionário curado** (P2): para mods muito populares cujas configs não trazem metadados (Sodium, Iris, Xaero, JEI/EMI), um arquivo de dados versionado no Warden com faixa e padrão. Mesma mecânica dos kits (§5.3).

**Quando cair para o editor de texto** (o formulário mostra "Este arquivo só pode ser editado como texto" e abre o texto):
- arquivo que o parser não consegue ler sem perda (o teste de *round-trip* falha na abertura: o Warden lê, reescreve em memória e compara os bytes; se der diferente, não oferece formulário);
- YAML (fora da v1, ARCHITECTURE §10), SNBT, scripts, `.json` de datapack/receita;
- estruturas profundas (listas de objetos, mapas dinâmicos de IDs para valores, por exemplo listas de biomas): o formulário mostra o nó como "Editar este trecho como texto" e abre um mini-editor só daquele trecho;
- arquivos acima de 2 MB (já é regra do T12).

### 2.3 O loader corrige o arquivo sozinho (e isso afeta o formulário)

Lido no código **[código]**:
- Ao carregar, o NeoForge chama `isCorrect`/`correct` no spec: valores fora da faixa ou do tipo voltam ao **padrão**, com o aviso `Incorrect key {chave} was corrected from {valor} to its default, {padrão}.` no log. Chaves desconhecidas são **removidas** (`CorrectionAction.REMOVE`), e comentários diferentes do spec são reescritos (`ModConfigSpec.java`, linhas 115–121, 204–279).
- Antes de corrigir, o `ConfigTracker` do FancyModLoader faz uma cópia: `<nome>-1.toml.bak`, girando até 5 cópias (`backUpConfig`, linhas 275–297).

Consequências práticas para o Warden:
1. **Validação antes de testar** (P1, barata): na etapa 1 do teste, conferir cada valor contra `Range`/`Allowed Values`. Fora da faixa vira Problema: "`maxChunks = 900` está fora da faixa 1–512 e o jogo vai trocar por 64 sem avisar."
2. **Aviso pelo console** (P1): o padrão `Incorrect key … was corrected` entra nas regras do diagnóstico do log. Assim o usuário fica sabendo que a config dele foi resetada.
3. **Os `.bak` aparecem na instância** e já são pegos pela higiene (`*.bak`, ARCHITECTURE §6.4). Nada a mudar, só confirmar que a captura (ARCHITECTURE §8.4) não os traz.
4. O aviso já previsto no T12 ("O jogo pode reescrever este arquivo e apagar comentários") está correto e ganha base no código. Também apaga **chaves que o mod não conhece**: se o usuário colar uma chave de outra versão do mod, ela some.

### 2.4 Busca em todas as configs

Pedido: achar, por exemplo, "spawn" em 400 arquivos de config.

- **Índice** no Rust (`warden-configs`): ao abrir o pack, varrer `config/`, `defaultconfigs/`, `kubejs/config/`, `options.txt` etc. e guardar uma linha por chave `{ arquivo, caminho da chave, valor, comentário, rótulo traduzido, modid }`. Para arquivos sem parser (texto), indexar por linha. Com o tamanho do critério de desempenho da SPEC (500 configs), cabe em memória: estimativa de ~50 mil entradas e poucos MB [inferência].
- **Busca:** por palavras, sem acento e sem diferenciar maiúsculas, com pontuação para "casou no nome da chave" > "no rótulo" > "no comentário" > "no valor". Um *matcher* aproximado, como `nucleo-matcher` (MPL-2.0, o mesmo do editor Helix) ou `fuzzy-matcher` (MIT), resolve. `tantivy` seria exagero para esse volume.
- **Atualização** incremental pelo observador de arquivos que o Warden já terá para "alteração externa" (T12).
- **Resultado** agrupado por arquivo ("12 resultados em 5 arquivos"), com a linha e um clique que abre o arquivo **já na chave**, no modo em que o usuário estava (formulário ou texto).
- **Filtros** (caixas de seleção, não abas): "Só o que mudou do padrão", "Só deste mod", "Pack / Instância de teste" (o seletor de origem que já existe).
- Esforço **P**. Proponho **P0**: é a forma mais rápida de achar a config certa num pack grande, e não depende do formulário.

### 2.5 "Comparar com o padrão": de onde tirar o padrão

O padrão de uma chave pode vir de quatro fontes, da mais barata para a mais cara:

| Camada | Como | Cobre | Custo |
|---|---|---|---|
| A. Comentário no arquivo | `Default:` (NeoForge com faixa), `[default: …]` (Forge 1.12 `Configuration`) | Parte das chaves do Forge/NeoForge; nada do Fabric | Zero |
| B. Padrões gerados por um **servidor local** sem a pasta `config/` do pack | Instala o servidor do loader (§4.2), copia só os mods do pack que rodam no servidor, sem `config/` nem `defaultconfigs/`, sobe até "Done", manda `stop` e guarda o `config/` gerado em `cache/defaults/<hash do conjunto de mods>/` | Configs `COMMON`, `SERVER` e `STARTUP` de mods que rodam no servidor | ~15–60 s por conjunto de mods (os servidores Fabric e NeoForge do experimento chegaram ao "Done" em 22,9 s e 13,7 s, **sem mods**) [verificado] |
| C. Padrões gerados por uma **abertura limpa do cliente** | Igual à B, mas no cliente: abre o jogo numa instância descartável sem `config/`, espera o fim do carregamento dos mods e fecha | Configs `CLIENT` e configs de mods só de cliente (a maioria do Fabric) | Um carregamento completo do cliente com a janela aberta; minutos em packs grandes |
| D. `defaultconfigs/` do próprio pack | Já é o "padrão do pack" para mundos novos | Só o que o autor colocou ali | Zero |

Detalhes e riscos:
- **B e C são o mesmo mecanismo** que o ServerPackCreator usa para descobrir se um mod quebra o servidor ("boot a real server … and watch for a crash", módulo `serverpackcreator-clientside`). Isso mostra que a ideia é usada na prática.
- **Quando parar o cliente (C):** as configs `CLIENT`/`COMMON`/`STARTUP` são criadas durante a carga dos mods, antes do menu principal (ConfigTracker do FML). Dá para encerrar o processo quando o log mostrar o fim da carga (no NeoForge, a tela de título; no Fabric, `Sound engine started` ou similar) [inferência: o marcador exato por loader/versão precisa ser fixado com os golden logs da L-05]. Fechar à força depois disso não perde configs já gravadas [inferência].
- **Chave do cache:** o hash dos jars + versão do loader. Mudou um mod, só as configs daquele mod ficam "sem padrão conhecido" até a próxima geração.
- **Configs que dependem de outros mods** (integrações) podem ter chaves a mais quando outros mods estão presentes; por isso o cache é por **conjunto** de mods, não por mod isolado.
- **Mundos:** no Forge 1.13–1.20, `SERVER` fica por mundo. A camada B cria um mundo descartável (`level-type=flat` para ser rápido) e lê o `world/serverconfig/`. No NeoForge 1.21.1, o servidor do experimento gravou `config/neoforge-server.toml` global e só um `readme.txt` em `world/serverconfig/` [verificado], o que confirma o modelo novo descrito no R1 §3.1.

**Proposta de comportamento:**
- Formulário: ao lado de cada valor, "padrão: 64" (com a fonte no *tooltip*: "do comentário do arquivo" ou "gerado em 01/10 com este conjunto de mods"). Valor diferente do padrão ganha uma marca. Cada chave tem **Restaurar padrão**; o arquivo tem **Restaurar tudo para o padrão** (com diferenças antes de gravar, como o T12).
- Filtro "Só o que mudou do padrão" na árvore e na busca. Útil também para a IA (R5A) e para as notas de versão.
- Botão **Descobrir padrões** (P2) em Configs → "⋯": roda a camada B (e, se o usuário aceitar abrir o jogo, a C) como Tarefa no rodapé.

### 2.6 Viabilidade, riscos e esforço

- Viabilidade: alta. Tudo acontece no Rust com as bibliotecas já escolhidas; a parte nova é o leitor de lang do jar e as heurísticas.
- Riscos: comentário enganoso (o mod escreve "1–10", mas aceita 0); mods que regravam o arquivo em outro formato entre versões; configs com chaves dinâmicas. Mitigação: confiança visível e o texto sempre a um clique.
- Esforço: formulário com camadas 1–4 = **M** (já é a C-04); busca = **P**; validação pré-teste e aviso de reset = **P**; padrões por camada A = **P**; camadas B/C = **M** (dependem do servidor local, §4.2); anotações no bytecode e dicionário curado = **M** cada (P2).

---

## 3. (6) Editor de scripts KubeJS e CraftTweaker

### 3.1 Versões por loader e Minecraft

Levantado na API do Modrinth em 01/10/2026 (versão mais nova por par loader × Minecraft) **[verificado]**:

| Ferramenta | Faixa | Loaders | Observações |
|---|---|---|---|
| **KubeJS 5** (`1802.5.x`, `1900.5.x`) | 1.18.2, 1.19 | Forge, Fabric, Quilt | Parado desde 2023. 1.16.5 só na CurseForge [inferência: não aparece no Modrinth]. |
| **KubeJS 6** (`1902.6.x`, `2001.6.5`) | 1.19.2, 1.20.1 | Forge, Fabric, Quilt (e o build Forge marcado também como NeoForge 1.20.1) | `2001.6.5-build.26` publicado em 2026-05: a linha 1.20.1 ainda recebe correções. |
| **KubeJS 7** (`2004.7.0`, `2101.7.2`) | 1.20.4, 1.21, 1.21.1 | **Só NeoForge** | `2101.7.2-build.377` de 2026-09-09. O Fabric deixou de ter KubeJS a partir daqui. |
| **KubeJS 8** (`26.1.2-8.0.6`) | 26.1.2 | Só NeoForge | Versão nova do Minecraft no esquema de numeração 26.x. |
| **ProbeJS** | 5.3.4 (1.19.2), 6.0.0 (Fabric 1.20.1), 7.0.0 (Forge/NeoForge 1.20.1), **8.0.3 (NeoForge 1.21.1)** | Forge, Fabric, NeoForge | Nada para 26.x ainda. Licença: o GitHub diz GPL-3.0; o Modrinth diz LGPL-2.1-only. Não importa para o Warden, que não embute o ProbeJS: ele é um mod que o usuário adiciona ao pack. |
| **CraftTweaker** | 1.7.10/1.12.2 (3.x/4.x, ZenScript clássico) até 1.21.1 (21.0.38) | Forge; Fabric e NeoForge nas versões modernas | MIT. 1.12.2 e 1.21.1 recebem correções em 2026. |
| **Rhino** (motor JS do KubeJS) | 1.18.2 → 26.1.2 | todos | Dependência obrigatória do KubeJS; o diálogo de dependências (T09) já a traria. |

Logs dos scripts [código]: `logs/kubejs/startup.log`, `server.log` e `client.log` (`ScriptType.getLogFile`, KubeJS `2101`). No CraftTweaker, `logs/crafttweaker.log` nas versões modernas e `crafttweaker.log` na raiz no 1.12.2 [inferência a confirmar nos golden logs].

### 3.2 ProbeJS: tipagens para autocompletar

Lido no repositório `Prunoideae/ProbeJS`, branch `1.21` **[código]**:
- É um mod. No jogo, `/probejs` abre uma tela com o botão de dump. O dump gera:
  - `.minecraft/.probe/` com as declarações TypeScript (`@package`, `@side-only/<lado>`, `@special`);
  - `kubejs/<tipo>_scripts/jsconfig.json` para cada tipo de script (startup, server, client), apontando para `../../.probe/...` (modelo em `assets/probejs/dumps/jsconfig.jsonc`, com `"checkJs": false` e `"skipLibCheck": true`);
  - `.vscode/settings.json`, trechos prontos (*snippets*) e `local/kubejs/export/generated_code`.
- **ProbeJS 8** reescreveu as tipagens para o **TypeScript nativo** (`tsgo`/TypeScript 7). O README cita "less than 500ms item literal completion time … on ATM10" e "intensive support of agents and LLMs".
- A partir do KubeJS 7.2, várias funções exigem a **extensão ProbeJS do VS Code conectada a um jogo aberto** ("almost mandatory", `docs/extension-usage/page.md`).

O que isso significa para o Warden:
- As tipagens só existem **depois** que o jogo roda com o ProbeJS e alguém clica no dump. O Warden pode facilitar: botão **Gerar tipagens** que abre o teste e lembra o usuário de rodar `/probejs` (P2: automatizar. O ProbeJS 8 registra `POST /api/probejs/run-command`, com token, no servidor web do KubeJS (`ProbeJSWeb.java`, §3.4)).
- O resultado (`.probe/`, `.vscode/`, `jsconfig.json`, `local/`) **não pode ir para o pack**. Hoje o ignore padrão (ARCHITECTURE §6.4) tem `/kubejs/probe/` e `/kubejs/exported/`, e `jsconfig.json` em qualquer profundidade, mas falta `/.probe/`, `/.vscode/` e `/local/kubejs/` (ver §11). Esses arquivos nascem na **instância de teste**, não na pasta do pack; o risco é a captura "O que mudou durante o teste" (ARCHITECTURE §8.3) oferecer trazê-los.
- Tamanho: para packs grandes, as tipagens passam de dezenas de MB [inferência: o próprio changelog 7.7.2 fala em "reduce generated file size"; não medi].

### 3.3 Autocompletar e verificação: TypeScript fora do WebView

A ADR-0020 escolheu CodeMirror 6 e descartou o Monaco por peso. Isso continua certo. A pergunta é onde roda o "cérebro" do autocompletar.

| Opção | Como | Prós | Contras |
|---|---|---|---|
| A. TypeScript **dentro do WebView** | `@valtown/codemirror-ts` (ISC) + `@typescript/vfs` (MIT) com o TypeScript 5.x/6.x em JavaScript, num *web worker* | Sem processo externo | Carregar dezenas de MB de `.d.ts` dentro do WebView2/WebKitGTK pesa (memória e travadas). O TypeScript 7 não tem mais a API JS que essas bibliotecas usam [inferência a partir do pacote 7.0.2, que só traz binários nativos], então fica preso ao 6.x, enquanto o ProbeJS 8 mira o 7. |
| **B. `tsc` 7 como servidor de linguagem** (recomendada) | Embutir o binário `tsc` 7.x como sidecar e rodar `tsc --lsp --stdio`. O Rust repassa as mensagens LSP para o frontend por um canal Tauri. No CodeMirror, `@codemirror/lsp-client` 6.3 (MIT, oficial) | Mesmo motor que o ProbeJS 8 recomenda; rápido; memória fora do WebView; serve também para JSON/JS comuns | Mais ~24–28 MB no instalador por plataforma; ponte LSP ↔ Tauri para escrever (esforço M) |
| C. Abrir no VS Code | Botão "Abrir no VS Code" com a pasta da instância | Zero esforço; o fluxo oficial do ProbeJS | Sai do Warden; exige VS Code e a extensão ProbeJS |

Experimento **[verificado]**: o pacote `@typescript/typescript-linux-x64` 7.0.2 traz um binário único `lib/tsc` de 24 MB (o de Windows tem 28 MB). `tsc --lsp -h` lista `-stdio`, `-pipe` e `-socket`. Um projeto JS com `checkJs` e um tipo declarado acusou, em 0,3 s:

```
a.js(4,42): error TS2322: Type 'number' is not assignable to type 'string'.
a.js(5,7): error TS2339: Property 'shapless' does not exist on type '{ shaped(...) ... }'.
```

**Recomendação:** B como alvo (P2), com C desde o início como saída fácil (P1: "Abrir no VS Code", um botão). Na v1, o editor do Warden entrega **sem** TypeScript:
- realce JS (`@codemirror/lang-javascript`, MIT) e ZenScript (gramática simples via `StreamLanguage`; não existe pacote CM6 pronto, mas há gramática TextMate MIT no `Yesterday17/ZenScript` para servir de base);
- **trechos prontos** dos eventos mais usados (`ServerEvents.recipes`, `ServerEvents.tags`, `StartupEvents.registry`, `ItemEvents.tooltip`; no CraftTweaker, `craftingTable.addShaped` etc.), por versão do KubeJS/CraftTweaker;
- **autocompletar de IDs** (`minecraft:stone`, `#forge:ingots/iron`) a partir dos jars do pack: ler `assets/*/models/item/*.json`, `data/*/tags/**` e `data/*/recipes/**` dos jars dá a lista de itens, blocos e tags sem abrir o jogo [inferência: itens registrados só por código e sem modelo ficam de fora; o servidor web do KubeJS 7 resolve isso com o jogo aberto, §3.4];
- **erros vindos do jogo** (§3.5), que são os que importam de verdade.

Por que não confiar só no `checkJs`: o próprio ProbeJS desliga `checkJs` no `jsconfig.json` porque "fully TS-compatible typing generation is not possible". Uma verificação estrita daria **falso positivo**. O TypeScript serve para autocompletar e documentação ao passar o mouse; erro "de verdade" é o que o KubeJS registra ao carregar.

ZenScript (CraftTweaker):
- **1.12.2:** existe o servidor de linguagem `raylras/zenscript-intelli-sense` (MIT, Kotlin, roda numa JVM, último push em 2024-10), que lê dumps `.dzs` gerados pelo mod **ProbeZS** (MIT, "only targets at MC 1.12.2"). O Warden já gerencia Java, então rodar esse servidor é viável (P2).
- **Modernas (1.14+):** não achei servidor de linguagem mantido [inferência]. O CraftTweaker 1.21.1 tem **`/ct syntax`**, que carrega os scripts e informa erros **sem executá-los** (`ScriptCommands.java`) [código]. É a verificação a usar.

### 3.4 O servidor web local do KubeJS 7 (achado)

Lido no KubeJS branch `2101` (1.21.1) **[código]**:
- Liga por padrão (`enabled = true`), em `127.0.0.1`, porta **61423** (tenta até +9 se estiver ocupada) (`WebServerProperties.java`, `LocalWebServer.java`).
- As configurações e o **token** ficam em **`kubejs/config/web_server.json`**. O token é gerado aleatoriamente (33 bytes, Base64) na primeira execução.
- Rotas sem token: `/api/mods`, `/api/registries`, `/api/registries/{ns}/{path}/keys` (todos os IDs de um registro), `/api/tags/...` (tags e seus valores), `/api/assets.zip`, WebSocket `/api/updates`.
- Rotas com `Authorization: Bearer <token>`: **`POST /api/reload/startup`**, **`POST /api/reload/server`** (executa `/reload` no servidor interno), `GET /api/console/{startup|server}/errors` e `/warnings`, WebSocket `/api/console/{tipo}/stream`, `/api/browse/...` (`KubeJSWeb.java`, `LocalWebServerRegistry.java`).

Para o Warden, com KubeJS 7 no pack e o jogo aberto pelo teste:
1. **Recarregar scripts com um botão**, sem o usuário digitar comando: o Warden lê o token em `kubejs/config/web_server.json` da **instância de teste** e chama `POST /api/reload/server` (P1).
2. **Erros dos scripts em tempo real**: `GET /api/console/server/errors` ou o WebSocket, mostrados no editor com a linha (P1).
3. **Autocompletar de IDs completo** com `/api/registries/.../keys` e `/api/tags/...` (P2).

**Cuidado de segurança:** esse arquivo é um segredo e fica dentro de `kubejs/`, que vai no pack. Sem regra, ele iria para o GitHub e para os jogadores (§11).

KubeJS 6 (1.20.1) não tem esse servidor. Os comandos são `/kubejs reload startup_scripts|server_scripts|client_scripts|lang|textures` e `/reload`; o KubeJS 7 trocou para `/kubejs reload config|startup|server|client` (`KubeJSCommands.java`, branches `2001` e `2101`) [código].

### 3.5 Verificação de erros e recarga durante o teste

| Situação | Como recarregar | Como ver o erro |
|---|---|---|
| KubeJS 7 + jogo aberto | `POST /api/reload/server` (ou `/startup`) pelo servidor web | `/api/console/*/errors` ou WebSocket |
| KubeJS 6, ou 7 sem servidor web | Usuário digita `/kubejs reload server_scripts` ou `/reload` (o Warden mostra o comando com "Copiar") | Ler `logs/kubejs/server.log` (o Warden já acompanha arquivos da instância) |
| CraftTweaker moderno | `/reload` (o `/ct reload` só manda usar `/reload`, `MiscCommands.java`) | `/ct syntax` + `logs/crafttweaker.log` |
| CraftTweaker 1.12.2 | Não há recarga oficial: reiniciar o jogo [inferência; existem mods de terceiros para isso] | `crafttweaker.log` |
| **Com servidor local (§4.2)** | O Warden escreve `reload` (ou `kubejs reload server_scripts`) **no stdin do servidor**. Verificado no servidor Fabric: `reload` → "Reloading!", "Loaded 1290 recipes" | Console do servidor + logs |

O cliente do jogo não tem console de entrada. Sem o servidor web do KubeJS 7, a única automação confiável é pelo **servidor dedicado**. Esse é um bom motivo para o servidor local entrar na v1 (§4.2).

Salvar no Warden com o jogo aberto: o editor de scripts grava na **instância de teste** (como "Editar configs do teste", T12) e oferece "Salvar e recarregar". Depois, "O que mudou durante o teste" leva os scripts para o pack, como já acontece com as configs.

### 3.6 Proposta de comportamento e esforço

- Em **Configs**, arquivos `.js` e `.zs` abrem com: realce, trechos prontos, autocompletar de IDs, botão **Recarregar no jogo** (com o jogo aberto) e um painel "Erros dos scripts" alimentado pelo log ou pelo servidor web. A barra do editor mostra a versão detectada ("KubeJS 7 · NeoForge 1.21.1").
- Pack sem KubeJS e sem CraftTweaker: os arquivos abrem como texto comum.
- Esforço: realce + trechos + IDs pelos jars = **M**; recarga e erros (log + servidor web) = **M**; LSP com `tsc` 7 + ProbeJS = **G**; LSP ZenScript 1.12.2 = **M**.

---

## 4. (7) Testes avançados

### 4.1 Perfis de teste

Hoje o T11 tem um único conjunto de "Ajustes do teste neste computador" (memória, Java, JVM, opcionais). Perfis são apenas **vários desses conjuntos com nome**:

| Campo | Exemplo | Observação |
|---|---|---|
| Nome | "Padrão", "PC fraco (3 GB)", "Shaders", "Java 25 experimental" | |
| Memória | `-Xms`/`-Xmx` | Aviso se passar de 75% da RAM do computador [inferência: limiar prático]. |
| Java | Automático (ADR-0029) ou versão fixa | Fixar mostra o motivo do automático, como já acontece. |
| Flags da JVM | Nenhuma / predefinição / texto livre | Predefinições por versão de Java (G1 com ajustes no Java 8/17; ZGC geracional no 21+, onde no 23+ o modo geracional já é o padrão) [inferência; os valores devem vir da pesquisa de desempenho do R5A]. |
| Opcionais | Quais mods opcionais ligar | Já previsto no T11. |
| Mundo | Novo / último mundo / mundo escolhido / entrar direto (Quick Play) | O portablemc tem `set_quick_play(QuickPlay::Singleplayer{name} \| Multiplayer{host,port})`, com correção para versões antigas (`fix_legacy_quick_play`, `moj/mod.rs`) [código]. |
| Janela | Tamanho, tela cheia | |
| Servidor | Nenhum / servidor local (§4.2) | |

Os perfis ficam nos dados do Warden (não no pack), como todos os "Ajustes do teste neste computador" (ESTRUTURA §5.2). O menu ▾ do Testar mostra "Perfil: Padrão ▸" com a lista. O botão mostra o perfil quando ele não é o padrão.

**Extra (P2): matriz de perfis.** "Testar em todos os perfis" roda os perfis em sequência (por exemplo 3, 4 e 6 GB) e mostra uma tabela "abriu / travou / tempo até o menu / pico de memória". O pico de memória vem do monitor do R5A. Isso responde "qual a memória mínima recomendada para o meu pack?", algo que o autor sempre precisa escrever na descrição. Depende do monitor de desempenho do R5A.

Esforço: perfis = **P**; matriz = **M**.

### 4.2 Servidor dedicado local

**O portablemc não instala servidor.** No crate 5.0.5, o instalador do Forge/NeoForge chama `try_install(..., serde::InstallSide::Client, ...)` com o lado fixo em cliente (`forge/mod.rs` linha 276), e o módulo Fabric só monta perfis de cliente [código]. O enum `InstallSide::Server` existe, mas não é usado. Então o servidor fica a cargo do Warden, por instaladores oficiais:

| Loader | Como instalar o servidor | Como iniciar | Verificado |
|---|---|---|---|
| Vanilla | `downloads.server` do JSON da versão (piston-meta, R2 §1.3) | `java -jar server.jar nogui` | — |
| **Fabric** | `GET https://meta.fabricmc.net/v2/versions/loader/<mc>/<loader>/<installer>/server/jar`: devolve um jar executável (181 KB) que, na primeira execução, baixa o servidor vanilla e as bibliotecas | `java -Xmx… -jar server.jar nogui` | **Sim**: 1.21.1 + loader 0.19.5 + installer 1.1.2; "Done" em 22,9 s na primeira execução, já contando os downloads |
| **NeoForge** | `java -jar neoforge-<v>-installer.jar --installServer <pasta>` (sem interface) | `java @user_jvm_args.txt @libraries/net/neoforged/neoforge/<v>/unix_args.txt nogui` (no Windows, `win_args.txt`), ou o `run.sh`/`run.bat` gerado | **Sim**: 21.1.252 instalou em 25 s (177 MB em `libraries/`); "Done" em 13,7 s |
| Forge 1.17+ | Igual ao NeoForge (`--installServer`, gera `run.sh`/`run.bat`, `user_jvm_args.txt` e `*_args.txt`) | Igual ao NeoForge | Pela documentação do instalador (R2 §3.5.1) e pelo script do ServerPackCreator |
| Forge ≤ 1.16.5 | `--installServer` gera um `forge-<v>.jar` executável | `java -jar forge-<v>.jar nogui` | [inferência; o script `default_template.sh` do ServerPackCreator trata as duas formas, linhas 243–272] |
| Forge 1.7.10/1.12.2 | Instalador legado: `--installServer` existe nas versões que o Forge ainda publica [inferência a verificar]; Java 8 obrigatório | `java -jar forge-<v>-universal.jar nogui` | — |
| Quilt | `quilt-installer install server <mc> <loader>` | jar gerado | [inferência] |

Alternativa para Forge 1.17+/NeoForge: o **ServerStarterJar** (NeoForged, LGPL-2.1) é um `server.jar` que lê o `run.sh`/`run.bat`, os arquivos de argumentos, e inicia o servidor no mesmo processo; também roda o instalador se faltar. Útil no server pack (§5.2), não no teste.

**Ciclo do teste com servidor**, verificado em Fabric e NeoForge [verificado]:
1. Pasta própria por pack nos dados do Warden (`instances/<pack>/server/`), separada da instância de cliente. O pack é copiado com o mesmo materializador (ADR-0011), mas **só com os mods cujo lado não é "Só cliente"** (`side = "client"` fica de fora).
2. `eula.txt` com `eula=true`. **Só depois** que o usuário aceitar uma vez o EULA da Mojang (link `https://aka.ms/MinecraftEULA`), com a decisão guardada nos dados do Warden. Sem isso, o servidor para na hora pedindo o EULA.
3. `server.properties` controlado pelo Warden: `online-mode=false` (o cliente é offline, ADR-0010), `server-port=<porta livre>` (o Warden testa antes; 25565 pode estar ocupada), **`server-ip=127.0.0.1`** para não expor o servidor na rede e para o Windows não abrir o aviso do firewall [inferência: o aviso aparece quando o Java escuta em todas as interfaces], `enable-rcon=false`, `motd` com o nome do pack. As demais chaves vêm do `server.properties` do pack, se existir.
4. Processo com stdin/stdout em pipe, como o do cliente (spike S1). Pronto quando casar `Done \([\d.,]+s\)! For help, type "help"`: a mesma linha apareceu nos dois loaders [verificado]. Versões antigas acrescentam ` or "?"` [inferência], e a expressão regular acima cobre.
5. O cliente abre já entrando no servidor: `QuickPlay::Multiplayer { host: "127.0.0.1", port }` no portablemc [código].
6. **Comandos pelo stdin:** o Warden escreve `reload`, `kubejs reload server_scripts`, `spark profiler …` (R5A) etc. Em Fabric, `reload` respondeu "Reloading!" e "Loaded 1290 recipes" em menos de 1 s [verificado].
7. **Parar:** `stop\n` no stdin e espera; se passar de 60 s, encerra à força. Nos experimentos, o servidor salvou o mundo e saiu com código 0 em 6 a 9 s [verificado].

O que o servidor local resolve:
- **"O mod é de cliente e o servidor quebra"**: o problema clássico que o ServerPackCreator ataca subindo um servidor de verdade, porque o lado declarado pelo mod às vezes "mente" (`serverpackcreator-clientside/README.md`). Para o autor, é a prova de que o server pack funciona.
- **Configs `SERVER` e sincronização** (valores do servidor vencem os do cliente).
- **Automação**: recarga de scripts, comandos de teste e geração de padrões (§2.5 camada B), coisas que o cliente não aceita por falta de console de entrada.

Riscos e limites: dois processos Java somam memória (o perfil precisa avisar); mundos de servidor crescem (usar "Apagar mundos de teste", T13); Windows Defender pode atrasar o primeiro início; Forge 1.7.10/1.12.2 exige Java 8 também no servidor (o mesmo da ADR-0029).

**Onde fica:** menu ▾ do Testar → **Testar com servidor local**. A tela do teste ganha a etapa "Preparar o servidor" e um seletor "Mostrando: jogo / servidor" sobre o console (caixa de seleção, regra 3 do ESTRUTURA). O console do servidor tem uma linha de comando. "Parar jogo" vira "Parar jogo e servidor". O resultado do teste lista o que aconteceu nos dois lados.

Esforço: **M/G** (instaladores por loader e faixa + ciclo de vida + interface). A SPEC hoje coloca "teste com servidor dedicado" como P2 (§9). Proponho **P1**, porque destrava o server pack (§5.2), a recarga de scripts (§3.5) e os padrões de config (§2.5).

### 4.3 Teste "como o jogador recebe" pelo link do GitHub

A L-07 e a SPEC T13 já preveem uma instância temporária limpa "a partir do pack exportado". O dono pediu mais: **instalar do zero pelo link do GitHub**, como o jogador faz (ADR-0028). Experimento **[verificado]**:

1. `packwiz-installer-bootstrap.jar` v0.0.3 (o último release, de 2020, 99 KB) apontado para `https://raw.githubusercontent.com/Fabulously-Optimized/fabulously-optimized/main/Packwiz/1.21.1/pack.toml`, com `-g -s client`: **falhou** com `HTTP 403 for URL: https://api.github.com/repos/comp500/packwiz-installer/releases/latest`, porque o limite sem login da API do GitHub (60/h por IP) estava esgotado (`/rate_limit` → `remaining: 0`). O bootstrap não tem plano B: termina com `ClassNotFoundException` e não instala nada.
2. Com o `packwiz-installer.jar` v0.5.14 baixado do Release (URL de download, que não passa pela API) e `--bootstrap-no-update --bootstrap-main-jar packwiz-installer.jar`, instalou **113 arquivos em 6,9 s** numa pasta vazia (`mods/`, `config/`, `resourcepacks/`, `packwiz.json` com o estado).
3. `raw.githubusercontent.com` responde com `cache-control: max-age=300` [verificado]: depois de **Publicar versão**, o link pode servir o `pack.toml` antigo por até 5 minutos.

Proposta de comportamento:
- O Warden guarda no cache, com versão e hash fixados, o bootstrap **e** o installer. A `cargo xtask installer` da L-03 já baixa o bootstrap para os testes; basta incluir o `packwiz-installer.jar`, e o app passa a fazer o mesmo na primeira vez que usar o recurso. O teste sempre roda com `--bootstrap-no-update`, então não depende da API do GitHub.
- Passos: pasta temporária → `java -jar bootstrap --bootstrap-no-update --bootstrap-main-jar <cache>/packwiz-installer.jar -g -s client <link do pack>` → ler o `pack.toml` baixado e conferir se o hash é o da versão publicada; se não for, avisar "O GitHub ainda está entregando a versão anterior. Tente de novo em alguns minutos." → abrir o jogo com a versão do loader do `pack.toml` → no fim, apagar a pasta e manter os logs.
- Mods da CurseForge em modo `metadata:curseforge` passam pela chave embutida no installer. **Mods bloqueados para terceiros falham aqui exatamente como falhariam para o jogador**, e isso vira um resultado útil do teste ("3 mods não baixam sozinhos para os jogadores").
- O passo a passo para jogadores (T18) pode citar o 403: se o bootstrap falhar com "403", basta abrir de novo mais tarde, ou baixar o `packwiz-installer.jar` e colocá-lo ao lado do bootstrap.

Esforço: **P/M** (a L-07 já existe; muda a origem: link em vez de pasta exportada, mais a conferência de hash).

---

## 5. (8) Importar, exportar server pack, kits e templates

### 5.1 Importar para o formato packwiz

| Origem | O packwiz faz? | Como o Warden faz | O que se perde ou precisa de revisão |
|---|---|---|---|
| **`.mrpack`** | **Não** (issue #155; R3 §2.4) | Próprio, no Rust. (1) Abrir o zip com proteção contra *zip slip* e validar `files[].path` (proibidos `..`, caminho absoluto e letra de drive). (2) `dependencies` → `pack.toml` (`minecraft` + `forge`/`neoforge`/`fabric-loader`/`quilt-loader`). (3) Cada `files[]` de `cdn.modrinth.com` → metafile Modrinth: o caminho da URL já traz projeto e versão (`/data/<projeto>/versions/<versão>/<arquivo>`) [verificado]; senão, `POST /v2/version_files` pelo `sha1`, em lote. (4) Outros hosts (GitHub) → metafile em modo `url` com o hash. (5) `env` → `side` (tabela da ARCHITECTURE §6.2); `optional` → `[option] optional = true`. (6) `overrides/` → arquivos do pack. (7) Jars dentro de `overrides/mods/` → identificar por hash no Modrinth e na CurseForge e oferecer "trocar por referência"; se não achar, ficam como arquivo local com aviso de licença. | `client-overrides/` não tem equivalente no packwiz (issue #82): entra no pack com aviso. `server-overrides/` vai para uma pasta `server-overrides/` na raiz do pack, fora do índice pelo `.packwizignore` (o jogador não a recebe), usada só pelo server pack (§5.2). `option.default` não existe no `.mrpack`: o Warden assume "ligado" (como o app do Modrinth, que instala todos) e marca para revisão. |
| **Zip da CurseForge** | Sim: `packwiz curseforge import` (zip local ou `minecraftinstance.json`; "HTTP not supported") | Rodar o sidecar em staging (ADR-0006) e depois pós-processar | O packwiz grava **todos os mods como `side = "both"`** (`core.UniversalSide`) e transforma `required: false` em opcional desligado (`curseforge.go`, `createModFile`) [código]. Jars em `overrides/` são copiados como arquivos comuns (`import.go`, linhas 280–335). O Warden corrige o lado buscando o mesmo arquivo no Modrinth (sha1 da CF → `/v2/version_files`). **Exige chave da CurseForge** (resolve `projectID/fileID`); sem chave, o Warden explica que não dá para importar. |
| **Instância do Prism/MultiMC** (pasta ou zip exportado) | Não | `instance.cfg` (INI: nome, memória `MinMemAlloc`/`MaxMemAlloc`, JVM), que vira um **perfil de teste** (§4.1); `mmc-pack.json`: componentes `net.minecraft`, `net.fabricmc.fabric-loader`, `net.minecraftforge`, `net.neoforged`, `org.quiltmc.quilt-loader` com versões; pasta `.minecraft/` (ou `minecraft/`). **Achado:** o Prism grava metadados no **formato packwiz** em `mods/.index/*.pw.toml` para os mods instalados por ele, com campos extras `x-prismlauncher-*` (`Packwiz.cpp`, linhas 107–239) [código]. Esses viram metafiles quase direto (descartando os campos `x-*`). Jars sem `.index` → identificar por hash. | Dados de execução (mundos, logs, caches) ficam de fora pela mesma lista de ignorados da captura (ARCHITECTURE §8.4). `options.txt` e `servers.dat` são perguntados ("Levar suas preferências para o pack?"). |
| Instância do app da CurseForge (`minecraftinstance.json`) | Sim (mesmo comando) | Igual ao zip da CF | Igual ao zip da CF. |
| Perfil do app do Modrinth | Não | P2 | — |

**Higiene na importação é obrigatória.** O `.mrpack` real do modpack "Create+" (NeoForge 1.21.1, 1,35 milhão de downloads) traz [verificado]:
- `overrides/.mixin.out/class/...` (centenas de classes geradas por depuração de mixin);
- `xmcl.json` (arquivo de outro launcher);
- `mods/.connector/temp/...jar` (cache do Sinytra Connector) listado em `files[]`;
- `env` com o valor `"unknown"`, que **não existe na especificação** do formato.

O importador passa tudo pela verificação de higiene do T04 antes de gravar, e trata `env` desconhecido como "Cliente e servidor" com o aviso de lado desconhecido do T08.

Onde fica: Meus packs → **Abrir ou importar…** (hoje "Abrir pack existente"), que detecta o tipo pelo conteúdo (`pack.toml`, `modrinth.index.json`, `manifest.json`, `instance.cfg`), como faz o `InstanceImportTask` do Prism [código]. O resultado abre na mesma tela de verificação e limpeza do T04, com a lista "o que foi convertido / o que ficou como arquivo local / o que precisa de revisão".

Esforço: `.mrpack` = **M**; CurseForge = **P** (sidecar) + **P** (correção de lado); Prism = **M**.

### 5.2 Exportar server pack

O Exportar (T19) já gera pasta/zip packwiz, `.mrpack` e zip da CurseForge. Server pack é o **formato 5**, e o mais pedido por quem joga com amigos.

Conteúdo:
- Mods cujo lado não é "Só cliente". Mods "Cliente e servidor" que quebram o servidor só aparecem no **teste com servidor local** (§4.2), que deve rodar antes de exportar ("Este server pack ainda não foi testado num servidor").
- `config/`, `defaultconfigs/`, `kubejs/` (sem `client_scripts/`), `scripts/`, datapacks globais, `server.properties` de referência se o pack tiver um, e o que veio de `server-overrides/` (§5.1).
- Fora: `options.txt`, `servers.dat`, `resourcepacks/`, `shaderpacks/`, configs `*-client.toml` (opcional; não atrapalham).
- **`user_jvm_args.txt`** com `-Xms`/`-Xmx` recomendados (comentados como no arquivo do instalador) e **scripts `start.bat` e `start.sh`**.

Duas variantes, escolhidas na tela de exportação:

| Variante | Como funciona | Prós | Contras |
|---|---|---|---|
| **A. Mods dentro do zip** | O Warden baixa os jars e põe em `mods/`; os scripts instalam o loader na primeira execução (`--installServer`, ou o jar do Fabric) e iniciam | Funciona offline, em qualquer hospedagem | Redistribui jars de terceiros (licença); mods da CF bloqueados exigem download manual (T20); zip grande |
| **B. Mods baixados ao iniciar** | Os scripts rodam `java -jar packwiz-installer-bootstrap.jar --bootstrap-no-update -g -s server <link do pack>` antes de iniciar o servidor | Sem redistribuição; o servidor se atualiza sozinho a cada versão publicada | Exige o pack publicado (ADR-0028); mods bloqueados da CF falham |

Recomendação: oferecer as duas, com **B** como padrão para packs publicados e **A** quando não houver publicação. Scripts de referência: o `default_template.sh/.bat/.ps1` do ServerPackCreator (LGPL-2.1) mostra os casos a tratar (Java errado, Forge antigo × novo, `run.sh` vs. jar, reinício automático, limpeza de arquivos do instalador). Não copiar o código; implementar o mínimo: verificar Java, instalar o loader se faltar, aceitar o EULA com pergunta, iniciar. Para Forge 1.17+/NeoForge, o `server.jar` do **ServerStarterJar** evita lidar com `run.sh` em hospedagens que só aceitam `java -jar`.

Validação automática (P1): depois de gerar, descompactar numa pasta temporária e rodar o ciclo do §4.2 até "Done". Se passar: "Server pack testado: o servidor abriu em 34 s."

Esforço: **M** (variante B) + **M** (variante A, por causa dos downloads e da licença).

### 5.3 Kits de desempenho e templates

**Kits.** O R1 §2.3.5 já listou os kits por (loader, versão). Falta o **formato e a manutenção**:

```toml
# data/kits/desempenho.toml (no repositório do Warden, versionado)
[[kit]]
id = "desempenho-fabric-moderno"
nome = "Desempenho (Fabric, 1.21+)"
loaders = ["fabric"]
minecraft = ">=1.21"
descricao = "Mods que melhoram FPS e uso de memória sem mudar o jogo."

  [[kit.item]]
  modrinth = "AANobbMI"     # Sodium (ID do projeto, estável; o slug pode mudar)
  lado = "client"
  marcado = true            # vem marcado no diálogo
  [[kit.item]]
  modrinth = "gvQqBUqZ"     # Lithium
  lado = "both"
  marcado = true
  conflita_com = ["embeddium", "optifine"]   # regra de conflito (R1 §2.3.4); slugs só para leitura aqui, IDs no arquivo real
```

- **IDs do Modrinth**, não slugs (slugs mudam). Itens só da CurseForge levam `curseforge = <id>` (sem guardar mais nada, por causa da regra de não guardar dados da API nos termos da CurseForge, §3.1(e); R3 §3.2).
- **CI do repositório do Warden** (semanal + a cada mudança): para cada kit e cada versão do Minecraft da faixa, `GET /v2/project/{id}/version?loaders=…&game_versions=…`; item sem versão compatível gera um aviso no relatório (o kit continua válido, e o item aparece como "não disponível nesta versão" no diálogo). Isso detecta mods abandonados sem depender de memória.
- **Aplicar um kit** passa pelo fluxo normal (diálogo de dependências T09, com caixas marcadas): nada entra no pack em silêncio.
- **Atualização dos dados:** junto com as versões do Warden; atualização remota assinada já está na lista "fora da v1" da SPEC §9 ("atualização remota dos dados curados") e serve aos kits também.

**Templates (modelos de pack).** Um modelo é: `pack.toml` base, kits pré-escolhidos, configs de partida e arquivos de controle. Exemplos de configs de partida: `options.txt` com distância de renderização moderada e `onboardAccessibility:false`, `config/fml.toml` com `earlyWindowControl`, e mecanismo de "padrão na primeira execução" (YOSBR ou Default Options, R1 §3.2). Também os **mods padrão do dono** (spark e o mod de crash report; a escolha está no R5A) e o `.gitignore`/`.packwizignore`/`.gitattributes` (ARCHITECTURE §6.4).

- Modelos embutidos: um por loader (Fabric moderno, NeoForge 1.21.1, Forge 1.20.1, Forge 1.12.2) e um "vazio".
- **Salvar como modelo** (P2): a partir de um pack do usuário, guarda lista de mods, configs e ajustes, sem histórico. Fica nos dados do Warden.
- **A partir de um modpack público** (§6): "Criar pack a partir deste modpack" é a importação do §5.1. Referências a mods são livres; configs e arquivos de `overrides/` são trabalho do autor do modpack. O Warden mostra a licença do projeto (Modrinth `license.id`, por exemplo `LicenseRef-All-Rights-Reserved` no "Fresh & Smooth" [verificado]) e avisa quando não for livre.

Onde fica: **Criar pack**, etapa 4, "Começar com: pack vazio · modelo · kit de desempenho" (caixa de seleção); página Adicionar, tela inicial, "Adicionar kit de desempenho".

Esforço: kits = **M** (formato, CI, diálogo); templates = **M**.

---

## 6. (9) Navegar por outros modpacks

Objetivo do dono: abrir um modpack do Modrinth ou da CurseForge, ver os mods dele e trazer os que interessarem para o próprio pack.

### 6.1 Modrinth

Tudo verificado na API em 01/10/2026 **[verificado]**:

1. **Buscar modpacks:** `GET /v2/search` com `facets=[["project_type:modpack"],["categories:neoforge"],["versions:1.21.1"]]` e `index=downloads` devolveu 2.102 modpacks. Cada resultado traz `title`, `author`, `description`, `downloads`, `follows`, `categories` (inclui os loaders), `display_categories`, `icon_url`, `featured_gallery`, `gallery`, `date_modified`, `latest_version`, `client_side`/`server_side`/`environment` e `license`. Atenção: os **facets precisam ir codificados na URL** (com colchetes e aspas crus, a API respondeu 400).
2. **Versões do modpack:** `GET /v2/project/{id}/version?loaders=["neoforge"]&game_versions=["1.21.1"]&include_changelog=false`. No "Create+", 6 versões, cada uma com um arquivo `.mrpack` (`primary: true`).
3. **Lista de mods de uma versão, sem baixar nada:** o campo `dependencies` da **versão do modpack** traz cada mod como `dependency_type: "embedded"` com `project_id` e `version_id`. Arquivos que não estão no Modrinth (jars dentro de `overrides/`) aparecem só com `file_name`. No "Create+ 6.0.0-alpha-f": 168 dependências, 154 com `version_id` e 14 só com nome de arquivo. Comparando com o `modrinth.index.json` real, **154 dos 155 arquivos** baixáveis batem. Nomes, ícones e lados vêm em lote com `GET /v2/projects?ids=[…]` e `GET /v2/versions?ids=[…]`.
4. **Detalhes completos (caminho, `env`, hashes) lendo só um pedaço do `.mrpack`:** o `cdn.modrinth.com` aceita `Range` (`accept-ranges: bytes`, respostas 206). Com um leitor de zip sobre HTTP Range (diretório central no fim do arquivo e depois só a entrada desejada), o `modrinth.index.json` (100 KB) foi lido com **5 requisições e 173 KB transferidos**, de um `.mrpack` de 3,5 MB com 768 entradas. Em Rust, isso é um `Read + Seek` sobre `reqwest` com `Range`, entregue ao crate `zip`.

Custo de abrir um modpack: 1 busca + 1 lista de versões + 1–2 chamadas em lote (projetos e versões) ≈ **4 requisições**, bem abaixo do limite de 300/min.

### 6.2 CurseForge

- **Buscar:** `GET /v1/mods/search?gameId=432&classId=4471&gameVersion=…&modLoaderType=…&sortField=…`. O `classId` **4471** (modpacks) e os demais (6 mods, 12 resource packs, **6552 shaders**, 6945 datapacks, 17 mundos) foram confirmados no mapeamento do Prism (`FlameAPI.cpp`, linhas 171–175) [código]. Isso também confirma o 6552, que o R3 §3.2 deixou como inferência.
- **Arquivos:** `GET /v1/mods/{id}/files`. O objeto `File` tem `isServerPack`, `serverPackFileId` (o server pack oficial do autor), `parentProjectFileId`, `alternateFileId` e `modules[]` (só nomes e fingerprints das entradas de primeiro nível, não a lista de mods) [documentação].
- **Lista de mods:** a API **não** tem rota com o conteúdo do modpack. É preciso o `manifest.json` de dentro do zip. A CDN aceita `Range`: `mediafilez.forgecdn.net` respondeu `accept-ranges: bytes` e 206 a um pedido parcial [verificado com um arquivo público]. Então dá para ler o `manifest.json` como no §6.1, sem baixar o zip inteiro [inferência: não testei com um zip de modpack da CF porque exigiria a chave; a técnica de zip é a mesma verificada no Modrinth]. Depois, `POST /v1/mods` (nomes e ícones) e `POST /v1/mods/files` (versões), em lote.
- `downloadUrl` pode vir **nulo** se o autor bloquear distribuição por terceiros (R3 §3.2). Para modpacks isso é raro, mas o Warden precisa mostrar "Este modpack não pode ser aberto por apps de terceiros. Abra a página na CurseForge."
- **Termos:** proibido guardar dados da API (§3.1(e), R3 §3.2). Tudo da CurseForge fica **só em memória** durante a sessão (ARCHITECTURE §17). Ler o manifest por Range usa a URL entregue pela própria API e não grava nada.

### 6.3 Trazer mods de um modpack para o próprio pack

Na página do modpack (dentro da página de descoberta, §7), a lista de mods aparece com caixas de seleção, já marcando:
- **"Já no seu pack"** (por `project_id` da mesma fonte, ou pela deduplicação entre fontes da ADR-0027), sem caixa;
- **"Compatível"** quando existe versão para o Minecraft e o loader do **seu** pack. Se o modpack for da mesma versão e do mesmo loader, o padrão é "a mesma versão que o modpack usa" (o `version_id` exato, que já foi testado junto com os outros), com a opção "a mais nova compatível";
- **"Sem versão para o seu pack"** (desabilitado, com o motivo);
- **"Arquivo fora das lojas"** (os `file_name` do §6.1 e jars em `overrides/`), também desabilitado: o Warden não copia jars de dentro de pacotes de terceiros.

**Adicionar N selecionados** abre **um** diálogo de dependências (T09) para o conjunto. Mods com conflito declarado (`incompatible`) são avisados ali.

Extra (P2): **"Ver como este modpack configurou o mod X"**: a lista de entradas do zip (diretório central, já baixado) mostra `overrides/config/x.toml`; um clique lê só aquele arquivo por Range e o abre lado a lado com o do seu pack (`@codemirror/merge`). Copiar trechos fica a cargo do usuário, com o aviso de licença do projeto.

### 6.4 Viabilidade, riscos e esforço

- Viável com as APIs atuais. Os riscos são: modpacks com `dependencies` incompletas (o Range no `.mrpack` cobre), mudança de CDN da CurseForge (o leitor cai para download completo com limite de tamanho, por exemplo 200 MB, com aviso), e cota da CurseForge desconhecida (lote e só sob demanda).
- Esforço: **M** (leitor de zip por Range, telas, mapeamento de compatibilidade). Depende da página de descoberta (§7) e do diálogo de dependências (T09).

---

## 7. (10) Página dedicada para descobrir e adicionar mods

A base já está aprovada: T08 (busca combinada, filtros travados na versão e no loader do pack, rolagem infinita, pré-visualização, "Já no pack") e ADR-0027 (intercalação e deduplicação). Esta seção acrescenta o que falta para uma **página de descoberta completa**.

### 7.1 O que cada API oferece

| Recurso | Modrinth v2 | CurseForge v1 |
|---|---|---|
| Busca | `/v2/search` (`query`, `facets`, `index`, `offset`, `limit` ≤ 100). Ordens: `relevance`, `downloads`, `follows`, `newest`, `updated` | `/v1/mods/search` (`searchFilter`, `classId`, `categoryIds` ≤ 10, `gameVersions` ≤ 4, `modLoaderTypes` ≤ 5, `sortField`, `sortOrder`, `index` + `pageSize` ≤ 50, teto de 10 mil resultados) |
| Ordenação ("populares") | `downloads` e `follows` | `sortField`: 1 Featured, 2 Popularity, 3 LastUpdated, 4 Name, 5 Author, 6 TotalDownloads, 7 Category, 8 GameVersion, 9 EarlyAccess, 10 FeaturedReleased, 11 ReleasedDate, 12 Rating [documentação] |
| Destaques | Não há rota; usar `index=follows`/`downloads` com os filtros do pack | `POST /v1/mods/featured` (`gameId`, `excludedModIds`, `gameVersionTypeId`) devolve destaques, populares e atualizados recentemente |
| Categorias | `/v2/tag/category`: `{ name, project_type, header, icon (SVG) }`. Para mods: 19 categorias; resource packs agrupados em `categories`, `features` e `resolutions`; shaders em `categories`, `features` e `performance impact` [verificado] | `/v1/categories?gameId=432&classId=6` (hierárquico, com `parentCategoryId` e `iconUrl`) |
| Descrição | `body` do projeto: **Markdown com HTML misturado** (no exemplo: `<h1 style="text-align: center;">` e `<iframe … youtube-nocookie …>`) [verificado] | `GET /v1/mods/{id}/description`: **HTML** |
| Galeria | `gallery[]`: `url` (miniatura `_350.webp`), `raw_url` (original), `featured`, `title`, `description`, `ordering` [verificado] | `Mod.screenshots[]`: `thumbnailUrl`, `url`, `title`, `description`; `Mod.logo` |
| Changelog | `version.changelog` (Markdown), com `include_changelog=true` (padrão) [verificado no Sodium] | `GET /v1/mods/{id}/files/{fileId}/changelog` (HTML) |
| Dependências | `version.dependencies[]` (`required`, `optional`, `incompatible`, `embedded`) e `GET /v2/project/{id}/dependencies` (`{ projects, versions }`) [verificado] | `File.dependencies[]` com `relationType` (1 embutida, 2 opcional, 3 obrigatória, 4 ferramenta, 5 incompatível, 6 inclusa) |
| Links | `issues_url`, `source_url`, `wiki_url`, `discord_url`, `donation_urls` [verificado] | `Mod.links`: `websiteUrl`, `wikiUrl`, `issuesUrl`, `sourceUrl` |
| Licença | `license { id, name, url }` (SPDX) | Não há campo de licença no schema [inferência: não encontrei na documentação] |
| Contadores | `downloads`, `follows` | `downloadCount`, `thumbsUpCount`, `rating`, `gamePopularityRank` |
| Limites | 300 req/min por IP; User-Agent identificável | Sem limite público; cota "a critério da Overwolf"; nada pode ser guardado |

### 7.2 Busca combinada, ordenação e paginação

O ADR-0027 já define a intercalação por posição, a deduplicação por autor + nome/slug, a conferência por SHA-1 na pré-visualização e as páginas de 20 itens. Complementos:
- **Ordenar por downloads ou atualização** com as duas fontes: em vez de intercalar por posição, **mesclar pelos números** (`downloads` × `downloadCount`; `date_modified` × `dateModified`). Os números são comparáveis entre as fontes [inferência: as duas contam downloads de arquivos; a CF tende a números maiores por causa do app dela]. Para "relevância" continua a intercalação.
- **Mapeamento de categorias:** uma tabela curada (dados versionados, como os kits) liga as categorias das duas fontes a um conjunto único em português ("Tecnologia" = Modrinth `technology` + CF "Technology"). Sem correspondência, a categoria filtra só a fonte que a tem.
- **Paginação:** por fonte, pedir 20 de cada (o Modrinth aceita até 100; a CF até 50) e pré-carregar a página seguinte quando o usuário chega a 70% da rolagem. Teto da CF: 10 mil resultados (`index + pageSize ≤ 10.000`).
- **Digitação:** espera de 300 ms antes de buscar e cancelamento da busca anterior (ADR-0019, cancelamento por operação).
- Medição: uma busca no Modrinth com `limit=100` levou **0,49 s e 57 KB** [verificado]. Com 20 itens, a resposta fica em ~12 KB.

### 7.3 Descrição: Markdown/HTML com sanitização

- A ARCHITECTURE §18 já decidiu: Markdown com `react-markdown` + `rehype-sanitize`; HTML da CurseForge com DOMPurify; links no navegador (plugin `opener`, só `https:`). Isso basta, com uma lista de permissão explícita: títulos, parágrafos, listas, tabelas, `a`, `img`, `code`/`pre`, `details`/`summary` e o atributo `align`; sem `script`, `style`, `on*`, `form`, `object`/`embed`. O corpo do Modrinth mistura Markdown e HTML [verificado], então é preciso ligar `rehype-raw` **antes** do `rehype-sanitize` (ordem obrigatória, senão o HTML aparece como texto ou passa sem filtro). O próprio Modrinth faz algo equivalente com `markdown-it` + `xss` (`packages/utils/parse.ts`; o código é GPL-3.0, então só a ideia serve).
- **`iframe`**: nunca dentro do Warden (a CSP já tem `frame-src 'none'`). Vídeos do YouTube viram uma miniatura com "Abrir no navegador".
- **Imagens externas:** a CSP atual (ARCHITECTURE §20) libera `img-src https:`, o que funciona. Há porém um ponto com os termos da CurseForge: o WebView guarda imagens no **cache de disco** dele, e os termos proíbem guardar dados obtidos pela API. Proposta de refinamento (P1): servir as imagens da CurseForge por um protocolo próprio (`warden-img://`) no Rust, com tempo-limite, tamanho máximo, só tipos de imagem, cache **só em memória** e `Cache-Control: no-store` para o WebView; as do Modrinth podem seguir direto. Se o orquestrador preferir simplicidade, manter `img-src https:` e registrar o risco.
- Esforço: **P/M**.

### 7.4 Galeria, versões, changelog e dependências na pré-visualização

A pré-visualização (T07/T08) vira uma página rolável **sem abas**, com títulos de seção e um índice fixo de âncoras no topo ("Descrição · Galeria · Versões · Dependências · Links"), na mesma regra das páginas corridas de Exportar e Histórico:
- **Cabeçalho:** ícone, nome, autor, fonte(s), downloads, atualização, licença, lado, botão **Adicionar ao pack** (com seletor de versão) e "Já no pack" quando for o caso.
- **Galeria:** faixa de miniaturas (`_350.webp` no Modrinth; `thumbnailUrl` na CF). Clicar abre a imagem grande sobre a página (camada temporária).
- **Versões:** lista filtrada para o pack, com canal (estável/beta/alpha), data e o **changelog** de cada uma, expansível. Na CF, o changelog vem por pedido (`/changelog`) só quando a linha é aberta.
- **Dependências:** obrigatórias, opcionais e **incompatíveis**, cada uma com "Já no pack" ou "Será adicionada".
- **Links:** problemas (issues), código, wiki, Discord: abrem no navegador.

### 7.5 Desempenho, cache e limites

- Modrinth: cache em `metadata.sqlite` (ARCHITECTURE §17) também para descrição e galeria (24 h); busca em memória por 5 minutos.
- CurseForge: só memória (sessão). Ao fechar o Warden, some.
- Imagens: miniaturas carregadas sob demanda (`loading="lazy"` sobre o protocolo próprio) e lista virtualizada (TanStack Virtual, ADR-0020) para rolagem longa.
- Orçamento por abertura da tela inicial: ~3 requisições por fonte (populares, atualizados e categorias) com o pack aberto.

### 7.6 Como encaixar na estrutura aprovada

**Recomendação: a página Adicionar (T08) vira a página de descoberta em tela cheia, aberta pelo botão "Adicionar mods" da seção Mods, sem virar a sétima seção do menu.**

Por quê:
- Descobrir mods é uma **tarefa com começo e fim** ("vim buscar mods para este pack"), como o teste. A ESTRUTURA já trata esses modos fora do menu (Testar, Salvar versão).
- O menu continua com 6 itens fixos. Uma sétima seção ("Explorar") dividiria "Mods" em dois lugares para a mesma coisa: o que está no pack e o que pode entrar.
- O cabeçalho do pack continua visível (com **▶ Testar**), e "← Voltar para Mods" mostra quantos itens entraram ("← Voltar para Mods · 5 adicionados").

Layout proposto (densidade de ferramenta, em lista, como pede o dono):

```
[cabeçalho fixo do pack · ▶ Testar]
← Voltar para Mods                                   Tipo: [Mods ▾]  (Mods · Resource packs · Shaders · Modpacks)
[ Buscar mods ou colar um link…                                  ] [Escolher arquivo do computador…]
Filtros (coluna recolhível)   | Resultados (lista densa)                         | Pré-visualização (painel)
 Categorias (lista)           |  ícone · nome · autor · resumo · downloads ·    |  cabeçalho, galeria,
 Ambiente (cliente/servidor)  |  atualizado · fonte · [Já no pack] [☐]          |  versões+changelog,
 Ordenar (relevância, …)      |  …                                               |  dependências, links
 Fonte (Mais filtros)         |                                                  |
                              | [☐ selecionados: 3 · Adicionar 3 ao pack]        |
```

- **Campo vazio = tela inicial:** "Populares para NeoForge 1.21.1", "Atualizados recentemente", "Kits de desempenho" (§5.3) e a lista de categorias. Tudo em listas, não em cartões iguais.
- **Seleção múltipla** com uma barra "Adicionar N ao pack" (um diálogo T09 só).
- **Tipo = Modpacks** muda a lista para modpacks (§6). Clicar num modpack abre a lista de mods dele no painel da direita, com caixas de seleção.
- **No nível do app** (sem pack aberto), a mesma página pode abrir em modo "Modpacks" a partir de **Criar pack → A partir de um modpack** (§5.3), terminando na importação. Isso respeita os dois níveis: a página é uma ferramenta, e o que muda é para onde vai o resultado.

Mudança mínima na ESTRUTURA: o rótulo do botão em Mods ("Adicionar mods") fica igual. Para caberem as três colunas na janela de 1080 px do rascunho, o menu lateral do pack recolhe para ícones enquanto a página estiver aberta e volta ao sair; isso é estado de exibição, não navegação nova. T08 ganha a tela inicial, a seleção múltipla, o valor "Modpacks" no Tipo e as seções da pré-visualização. Nenhuma seção nova, nenhuma aba.

Esforço: tela inicial + categorias + seleção múltipla = **M**; pré-visualização completa (galeria, changelog, links, sanitização, protocolo de imagens) = **M**.

---

## 8. Dependências entre as funções

```
Leitor de jar (P1-06 warden-jarmeta) ──► rótulos de config (lang) ──► Formulário (5a)
                                    └──► IDs de itens/tags ─────────► Autocompletar de scripts (6)
Busca em configs (5b) ── independente (só warden-configs)
Servidor local (7b) ──► padrões gerados (5c, camada B)
                    ├──► recarga de scripts sem digitar (6)
                    ├──► validação do server pack (8b)
                    └──► console/spark de servidor (R5A)
Leitor de zip por Range ──► navegar por modpacks (9) ──► "criar a partir de modpack" (8a/8c)
Importar .mrpack (8a) ◄── mesmo mapeamento files[] → metafile usado em (9)
Página de descoberta (10) ──► (9) é um "Tipo" dela; kits (8c) aparecem na tela inicial
Diálogo de dependências (T09) ◄── usado por (8c), (9), (10)
Perfis de teste (7a) ◄── importados de instance.cfg do Prism (8a); usados pela matriz (R5A: monitor de memória)
Teste como o jogador recebe (7c) ◄── Publicar versão (ADR-0028) e installer em cache (L-03)
```

Ordem natural: busca em configs e descoberta (já P0) → servidor local → formulário/padrões, server pack e recarga de scripts → importação e modpacks → autocompletar completo.

---

## 9. Implicações para o Warden

Lista priorizada. "v1" = entra no escopo da primeira versão.

### P0 (v1, junto com o que já é P0)

1. **Busca em todas as configs** (§2.4): índice no `warden-configs`, resultados agrupados por arquivo, abre já na chave. Esforço P. Entra na C-02 ou numa tarefa nova pequena.
2. **Página Adicionar como página de descoberta** (§7.6), com o que já é P0 no T08 e mais: tela inicial (populares e atualizados para a versão do pack, categorias com mapeamento curado), **seleção múltipla** com um diálogo de dependências só, e sanitização com `rehype-raw` antes do `rehype-sanitize` (§7.3). Muda a P1-09/P1-10.
3. **Correções de higiene e segredos** (§11): `/kubejs/config/web_server.json`, `/.probe/`, `/.vscode/`, `/local/kubejs/` no `.packwizignore` padrão e na lista "sempre ignorados" da captura (ARCHITECTURE §6.4 e §8.4).

### P1 (v1)

4. **Formulário de configs** com as camadas 1–4 do §2.2 (comentários, tipo pelo valor, heurísticas e **rótulos do lang do jar**), validação de faixa antes do teste e aviso "config resetada pelo jogo" a partir do log (§2.3). Já é a C-04; amplia os critérios de aceite.
5. **Padrão pelo comentário** (camada A, §2.5) com "Restaurar padrão" e o filtro "Só o que mudou do padrão".
6. **Servidor local** (§4.2) para Fabric, NeoForge e Forge 1.17+ (Forge antigo e Quilt depois), com EULA consentido, `server-ip=127.0.0.1`, porta livre, "Done" por expressão regular, comandos pelo stdin e parada limpa. **Sobe de P2 para P1** na SPEC §9.
7. **Teste como o jogador recebe pelo link** (§4.3) com installer em cache (`--bootstrap-no-update`) e conferência do hash do `pack.toml` (cache de 5 min do raw). Ajusta a L-07.
8. **Perfis de teste** (§4.1), inclusive Quick Play para entrar direto num mundo ou no servidor local.
9. **Scripts:** realce JS/ZenScript, trechos prontos, autocompletar de IDs pelos jars, "Abrir no VS Code", erros dos logs `logs/kubejs/*.log` e `crafttweaker.log`, **Recarregar no jogo** (servidor web do KubeJS 7 com o token da instância; stdin do servidor local; senão, comando para copiar) (§3.3–§3.5).
10. **Server pack** (§5.2), variante B (bootstrap com `-s server`) como padrão para packs publicados e variante A (mods dentro) como alternativa, validado no servidor local.
11. **Importar `.mrpack`, zip da CurseForge e instância do Prism** (§5.1), com higiene obrigatória e revisão de lado. Sobe de P2 para P1 na SPEC §9.
12. **Navegar por modpacks** (§6) como Tipo = Modpacks na página de descoberta, com a lista de mods pela API do Modrinth (dependências `embedded`), leitura por Range do `.mrpack`/`manifest.json` e "Adicionar N selecionados".
13. **Kits de desempenho** em dados versionados com validação no CI (§5.3), oferecidos na criação do pack e na tela inicial da página de descoberta. A SPEC hoje diz P2; sugiro P1 porque o custo é baixo depois que (2) existe.

### P2 (depois da v1)

14. Padrões gerados por servidor e por cliente limpo (camadas B e C, §2.5) e "Descobrir padrões".
15. Metadados por bytecode (`@Entry`, `@RangeConstraint`, `@BoundedDiscrete`) e dicionário curado de configs populares (§2.2).
16. Autocompletar completo de KubeJS com `tsc` 7 `--lsp --stdio` + ProbeJS (§3.3) e LSP de ZenScript para 1.12.2 (§3.3).
17. Matriz de perfis ("qual a memória mínima?", §4.1), com o monitor do R5A.
18. Modelos de pack: "Salvar como modelo" e "Criar a partir de modpack" (§5.3); "Ver como este modpack configurou o mod X" (§6.3).
19. Importar perfil do app do Modrinth; datapacks como Tipo na página de descoberta (CF `classId` 6945; Modrinth loader `datapack`).

---

## 10. Questões para o dono

1. **Servidor local já na v1?** Ele permite testar o pack como servidor, recarregar scripts com um botão e gerar o "pacote para servidor" já testado. Custa mais ou menos o mesmo que o teste do jogo levou na parte de launcher. Recomendo **sim**.
2. **"Pacote para servidor": como os mods chegam ao servidor?** (a) o servidor baixa os mods sozinho pelo link do pack ao iniciar e se atualiza a cada versão publicada (recomendado se você publica no GitHub); (b) os mods vão dentro do zip (funciona sem internet, mas o zip fica grande e alguns mods da CurseForge podem exigir download manual). Posso oferecer os dois, com (a) como padrão?
3. **Importar modpacks de outras pessoas** (`.mrpack`, CurseForge, Prism) já na v1? A SPEC hoje deixa para depois. Com a navegação por modpacks pedida, recomendo trazer para a v1.
4. **Kits de desempenho na v1?** São uma lista curada (Sodium, Lithium, FerriteCore…) por loader e versão, que o Warden oferece ao criar o pack, sempre com a sua confirmação. Recomendo **sim**.
5. **Autocompletar "de verdade" nos scripts KubeJS** exige rodar o jogo com o mod ProbeJS uma vez e embutir no Warden um componente de ~25 MB (o TypeScript novo). Na v1, proponho realce, trechos prontos, nomes de itens e erros do jogo, mais um botão "Abrir no VS Code". O autocompletar completo fica para depois. Concorda?
6. **Nome da seção Configs:** como os scripts ficam lá, a linha de descrição pode virar "Arquivos de ajuste e scripts do pack". Concorda?
7. **Botão "Abrir pack existente" vira "Abrir ou importar…"** em Meus packs, para aceitar também arquivos de modpack. Concorda?
8. **Aceitar o EULA da Mojang** para rodar o servidor local: o Warden pergunta uma vez, com o link, e guarda a resposta. Tudo bem assim?

---

## 11. Achados fora do escopo (para o orquestrador)

1. **Segredo do KubeJS dentro do pack.** `kubejs/config/web_server.json` guarda um token de acesso (`auth`) gerado na primeira execução do KubeJS 7 [código]. Se o arquivo vier da instância para o pack ("O que mudou durante o teste") ou for criado pelo autor, vai para o GitHub e para os jogadores. A varredura de segredos (ARCHITECTURE §11.1) usa padrões e as chaves do Warden, e **não pegaria** um token aleatório. Ação: incluir `/kubejs/config/web_server.json` no bloco obrigatório do `.packwizignore` (ARCHITECTURE §6.4) e nos "sempre ignorados" da captura (§8.4).
2. **Ignorados faltando** para scripts: `/.probe/` (tipagens do ProbeJS 7+, que saíram de `kubejs/probe/`), `/.vscode/` (criado pelo ProbeJS) e `/local/kubejs/` (exportações e scripts locais do KubeJS 7, `KubeJSPaths.LOCAL`). O `jsconfig.json` já está coberto.
3. **classId 6552 de shaders confirmado** (Prism `FlameAPI.cpp`), resolvendo a inferência do R3 §3.2. Datapacks: 6945.
4. **SPEC T18 (instruções aos jogadores):** o bootstrap falha com 403 quando a API do GitHub sem login está no limite (60/h por IP). Vale uma linha de ajuda no passo a passo.
5. **O NeoForge apaga chaves desconhecidas** e reescreve comentários ao corrigir um arquivo (§2.3). O aviso do T12 pode citar isso.
6. **`.mrpack` reais trazem lixo** (`.mixin.out`, `xmcl.json`, `mods/.connector/temp`) e `env: "unknown"` (§5.1). O validador de `.mrpack` da E-02 (CA-T19-04) deve aceitar `unknown` na leitura e nunca gerar esse valor.
7. O FO (Fabulously Optimized) 1.21.1 instalado no experimento traz `config/crash_assistant/...`, ou seja, usa o **Crash Assistant**. É um dado para o R5A (mod de crash report padrão).

---

## 12. Fontes

Código-fonte (lido em 01/10/2026):
- NeoForge `ModConfigSpec.java` e `ConfigurationScreen.java` (branch `1.21.1`): https://github.com/neoforged/NeoForge/blob/1.21.1/src/main/java/net/neoforged/neoforge/common/ModConfigSpec.java · https://github.com/neoforged/NeoForge/blob/1.21.1/src/main/java/net/neoforged/neoforge/client/gui/ConfigurationScreen.java
- FancyModLoader `ConfigTracker.java`: https://github.com/neoforged/FancyModLoader/blob/main/loader/src/main/java/net/neoforged/fml/config/ConfigTracker.java
- Forge `ForgeConfigSpec.java` (1.20.1): https://github.com/MinecraftForge/MinecraftForge/blob/1.20.1/src/main/java/net/minecraftforge/common/ForgeConfigSpec.java
- Forge 1.12.x `Configuration.java`, `FieldWrapper.java`, `ConfigManager.java`: https://github.com/MinecraftForge/MinecraftForge/tree/1.12.x/src/main/java/net/minecraftforge/common/config
- MidnightLib `MidnightConfig.java`: https://github.com/TeamMidnightDust/MidnightLib/blob/multiversion/src/main/java/eu/midnightdust/lib/config/MidnightConfig.java
- owo-lib `ConfigWrapper.java`, `Option.java`: https://github.com/wisp-forest/owo-lib/tree/1.21/src/main/java/io/wispforest/owo/config
- YACL `SerialEntry.java`, `GsonConfigSerializer.java`: https://github.com/isXander/YetAnotherConfigLib/tree/main/src/main/java/dev/isxander/yacl3/config/v2
- Cloth Config / AutoConfig `ConfigScreenProvider.java`, `JanksonConfigSerializer.java`: https://github.com/shedaniel/cloth-config/tree/v15/common/src/main/java/me/shedaniel/autoconfig
- ProbeJS (branch `1.21`): https://github.com/Prunoideae/ProbeJS — `ProbePaths.java`, `assets/probejs/dumps/jsconfig.jsonc`, `docs/extension-usage/page.md`, `docs/changelog.md`, `misc/endpoints/ProbeJSWeb.java`
- KubeJS (branches `2101` e `2001`): https://github.com/KubeJS-Mods/KubeJS — `web/LocalWebServer.java`, `web/WebServerProperties.java`, `web/LocalWebServerRegistry.java`, `web/local/KubeJSWeb.java`, `KubeJSPaths.java`, `script/ScriptType.java`, `command/KubeJSCommands.java`
- CraftTweaker (branch `1.21.1`): https://github.com/CraftTweaker/CraftTweaker — `impl/command/type/script/ScriptCommands.java`, `impl/command/type/MiscCommands.java`
- ZenScript IntelliSense: https://github.com/raylras/zenscript-intelli-sense · ProbeZS: https://github.com/friendlyhj/ProbeZS · gramática ZenScript: https://github.com/Yesterday17/ZenScript
- portablemc 5.0.5 (crate, `src/forge/mod.rs`, `src/forge/serde.rs`, `src/moj/mod.rs`): https://crates.io/crates/portablemc
- Prism Launcher (branch `develop`): `modplatform/packwiz/Packwiz.cpp`, `InstanceImportTask.cpp`, `modplatform/flame/FlameAPI.cpp`: https://github.com/PrismLauncher/PrismLauncher
- packwiz `curseforge/import.go` e `curseforge/curseforge.go`: https://github.com/packwiz/packwiz/tree/main/curseforge
- ServerPackCreator (`serverpackcreator-clientside/README.md`, `server_files/default_template.sh`): https://github.com/Griefed/ServerPackCreator
- ServerStarterJar: https://github.com/neoforged/ServerStarterJar
- Modrinth (frontend, sanitização) `packages/utils/parse.ts`: https://github.com/modrinth/code

Documentação e APIs:
- Modrinth API v2: https://docs.modrinth.com/api/ (busca, projetos, versões, `version_files`, tags)
- CurseForge Core API: https://docs.curseforge.com/rest-api/ (busca, `ModsSearchSortField`, `File`, `Mod`, descrição, changelog, `featured`)
- Fabric Meta (servidor): https://meta.fabricmc.net/v2/versions/loader/1.21.1/0.19.5/1.1.2/server/jar
- NeoForged Maven: https://maven.neoforged.net/releases/net/neoforged/neoforge/
- packwiz-installer-bootstrap: https://github.com/packwiz/packwiz-installer-bootstrap · packwiz-installer: https://github.com/packwiz/packwiz-installer
- KubeJS Wiki, ProbeJS: https://kubejs.com/wiki/addons/probejs
- TypeScript 7 (pacote npm `typescript` 7.0.2 e `@typescript/typescript-*-x64`): https://www.npmjs.com/package/typescript
- CodeMirror LSP client: https://www.npmjs.com/package/@codemirror/lsp-client · `@valtown/codemirror-ts`: https://www.npmjs.com/package/@valtown/codemirror-ts
- EULA da Mojang: https://aka.ms/MinecraftEULA

Experimentos (em `~/.local/share/warden-r5/b/`, todos em 01/10/2026):
- Busca de modpacks, versões, dependências `embedded`, categorias, projeto, changelog e dependências no Modrinth (`search_mp.json`, `mp_versions.json`).
- Leitura parcial de `.mrpack` por HTTP Range (`rangezip.py`, `createplus_index.json`): 5 requisições, 173 KB de 3,5 MB.
- Range na CDN da CurseForge (`mediafilez.forgecdn.net`, 206).
- Servidor Fabric 1.21.1 (`fabric-server/run.log`) e NeoForge 21.1.252 (`neo-server/install.log`, `run.log`): instalação, "Done", `reload`, `stop`.
- packwiz-installer-bootstrap contra o FO 1.21.1 (`player/boot.log` com o 403; `player/boot2.log` com 113 arquivos em 6,9 s).
- TypeScript 7.0.2 `tsc --lsp` e verificação de JS tipado (`ts7/`).
