# Warden — Especificação do produto

> Versão do documento: 1.2 (2026-10-01). Tarefa A1; estrutura de navegação, busca combinada, chaves, Java e publicação revistas na tarefa D2 com as decisões do dono de 01/10/2026 (`docs/design/ESTRUTURA.md`, ADR-0025 a ADR-0029). Funções avançadas de criação, edição e debug acrescentadas na tarefa D4 (pesquisas R5A e R5B, decisões D15 a D26, ADR-0030 a ADR-0038).
> Base: relatórios em `docs/research/` (R1 a R4, R5A em `05-diagnostico-avancado.md` e R5B em `06-criacao-edicao-descoberta.md`) e as decisões do dono registradas em `docs/decisions/`.
> Documentos irmãos: `ARCHITECTURE.md` (como é construído), `QUALITY.md` (padrão obrigatório), `ROADMAP.md` (ordem de construção).

## Sumário

1. [O que é o Warden](#1-o-que-é-o-warden)
2. [Glossário](#2-glossário)
3. [Conceitos centrais](#3-conceitos-centrais)
4. [Prioridades e escopo](#4-prioridades-e-escopo)
5. [Mapa de telas](#5-mapa-de-telas)
6. [Telas e fluxos](#6-telas-e-fluxos)
7. [Regras que valem para o app inteiro](#7-regras-que-valem-para-o-app-inteiro)
8. [Requisitos não funcionais](#8-requisitos-não-funcionais)
9. [Fora da v1](#9-fora-da-v1)
10. [Decisões do dono](#10-decisões-do-dono)
11. [Rastreabilidade](#11-rastreabilidade)

---

## 1. O que é o Warden

**Em palavras simples:** o Warden é um programa para o seu computador (Windows) que serve para montar modpacks de Minecraft. Com ele você escolhe a versão do Minecraft e o carregador de mods (Forge, NeoForge ou Fabric), descobre e adiciona mods, resource packs e shaders (inclusive trazendo mods de outros modpacks), ajusta as configurações e os scripts, abre o jogo para testar com um clique, entende por que o jogo travou (com regras automáticas, uma busca que acha sozinha o mod culpado e uma IA que conversa com você citando as provas), salva versões do pack com um resumo automático do que mudou, publica as versões finais no GitHub para os jogadores receberem as atualizações sozinhos e exporta só o que importa, inclusive um pacote para servidor.

O pack que o Warden produz é 100% no formato do **packwiz** (o mesmo usado por packs conhecidos como o Fabulously Optimized). Isso quer dizer que o pack não fica "preso" ao Warden: qualquer ferramenta que entende packwiz consegue usá-lo.

Princípios que guiam todas as decisões:

1. **O pack é sagrado.** A pasta do pack só contém o que é do pack. Nada de cópias de segurança, arquivos temporários, logs ou caches dentro dela (lição dos projetos anteriores, R4 §4.1).
2. **Projeto separado do teste.** O jogo de teste roda numa pasta separada (a "instância de teste"). O que você muda durante o jogo só volta para o pack depois que você revisa e aprova.
3. **Funciona de verdade.** Nada é "pronto" sem ter sido provado contra o packwiz real e, no caso do teste, contra o jogo real.
4. **Sem surpresas.** Toda ação que apaga ou substitui algo pede confirmação e pode ser desfeita pelo histórico.
5. **Português do Brasil, linguagem simples.** Termos técnicos só quando ajudam; sempre com explicação.
6. **Privado.** Nenhum dado sai do computador sem você pedir. Logs só vão para a IA numa conversa que você começou e aceitou, sempre sem dados pessoais, e cada coisa enviada fica visível na conversa (ADR-0030).
7. **O pesado só quando você pede.** Servidor local, busca do culpado e perfil de desempenho nunca rodam sozinhos; o ▶ Testar continua fazendo só o teste normal.

Uso: pessoal e privado. O Warden nunca será distribuído publicamente (ADR-0004).

## 2. Glossário

Termos como aparecem na interface. A lista completa de termos obrigatórios fica em `QUALITY.md` §8.

| Termo na interface | Significado |
|---|---|
| **Pack** | O seu modpack: a pasta do projeto no formato packwiz. |
| **Mod** | Um arquivo `.jar` que muda o jogo. No pack, normalmente é uma *referência* (um arquivo `.pw.toml` que diz de onde baixar), não o `.jar` em si. |
| **Resource pack** | Pacote de texturas, sons e textos. |
| **Shader** | Pacote de efeitos visuais (exige Iris/Oculus ou similar). |
| **Loader** | O carregador de mods: Forge, NeoForge ou Fabric. |
| **Config** | Arquivo de configuração de um mod ou do jogo (`config/`, `defaultconfigs/`, `options.txt`). |
| **Instância de teste** | A pasta onde o Minecraft roda quando você clica em "Testar". É separada do pack. |
| **Lado** | Onde um mod precisa estar: "Cliente e servidor", "Só cliente" ou "Só servidor". |
| **Fonte** | De onde vem um item: Modrinth, CurseForge, Link direto ou Arquivo local. |
| **Versão do pack** | Número no formato `MAIOR.MENOR.CORREÇÃO` (ex.: `1.4.2`), chamado SemVer. |
| **Salvar versão** | Registrar o estado atual do pack no histórico, com número de versão e resumo das mudanças. |
| **Diagnóstico** | Verificação automática que procura problemas antes do teste e explica travamentos depois. |
| **Problemas** | Seção do pack com a saúde do pack, o que o diagnóstico encontrou e a lista de travamentos. |
| **✦ Diagnóstico com IA** | Seção do pack em que você conversa com a IA (Gemini) sobre um problema. A IA consulta o pack e os logs, sempre sem dados pessoais e com cada consulta à vista, e só propõe mudanças: nada muda sem você clicar em **Aplicar**. O ícone de brilhinho marca tudo o que é IA. |
| **Saúde do pack** | Nota de 0 a 100 com uma faixa (Ótimo, Bom, Atenção, Crítico) que resume os problemas conhecidos e diz o que tirou pontos. Não garante que o pack funciona. |
| **Travamentos** | Lista, em Problemas, dos testes que travaram, agrupados pela causa. |
| **Encontrar o mod culpado** | Busca automática em **rodadas**: o Warden abre o jogo várias vezes com partes do pack, numa cópia separada, até achar o mod (ou a dupla de mods) que causa o problema. |
| **O que este mod altera no jogo** | Raio-x dos mixins de um mod: quais partes do jogo ele altera e quais outros mods alteram o mesmo ponto. |
| **Conversa** | Uma sessão de perguntas e respostas com a IA sobre um problema do pack, guardada no computador. |
| **Proposta** | Mudança sugerida pela IA, mostrada com a diferença exata, que só acontece se você clicar em **Aplicar**. |
| **Perfil do teste** | Conjunto de ajustes do teste com nome (memória, Java, argumentos, como o jogo abre), guardado neste computador. |
| **Servidor local** | Servidor do Minecraft que o Warden abre neste computador só quando você pede (por exemplo, em **Testar como servidor**). Só aceita conexões do próprio computador. |
| **Pacote para servidor** | Arquivo .zip para quem vai rodar o pack num servidor. |
| **Mods iniciais** | Mods sugeridos ao criar um pack (spark, Crash Assistant e, se você quiser, um kit de desempenho). |
| **Kit de desempenho** | Lista curada de mods que deixam o jogo mais leve, por versão e loader, oferecida sempre com a sua confirmação. |
| **Ferramenta do jogador** | Mod que existe para quem joga (spark, Crash Assistant). O Crash Assistant fica fora dos seus testes normais. |
| **Versão final** | Versão salva marcada como pronta para os jogadores. Só versões finais podem ser publicadas. |
| **Publicar versão** | Enviar uma versão final ao GitHub do pack. Quem joga com o link do pack recebe a atualização sozinho. |
| **Link do pack** | Endereço do `pack.toml` publicado no GitHub (`https://raw.githubusercontent.com/<dono>/<repo>/main/pack.toml`), usado pelos jogadores no packwiz-installer-bootstrap. |

## 3. Conceitos centrais

### 3.1 Pack (projeto)

- Uma pasta no disco, por padrão `Documentos\Warden\<nome-do-pack-em-minúsculas-com-hífens>\`, escolhida pelo usuário.
- Contém os arquivos do packwiz (`pack.toml`, `index.toml`, `mods/*.pw.toml` etc.), as configs do pack, os arquivos de controle (`.gitignore`, `.gitattributes`, `.packwizignore`), o `CHANGELOG.md` e a pasta `.warden/` com informações do Warden sobre o pack (identificador e avisos do diagnóstico que você decidiu ignorar). A pasta `.warden/` e o `CHANGELOG.md` **não** entram no pack distribuído (ficam fora do índice do packwiz).
- É um repositório git. O usuário nunca vê git; vê "versões" e "histórico". O histórico de trabalho fica só no computador; o GitHub recebe só as versões publicadas (§3.3, T18).
- Tem exatamente **uma** versão do Minecraft e **um** loader (ou nenhum, para packs vanilla de resource packs/shaders).

### 3.2 Instância de teste

- Uma pasta por pack, em dados do Warden (`%LOCALAPPDATA%\dev.kriticales.warden\instances\<id>\minecraft\`).
- Montada a partir do pack a cada teste: o Warden copia/baixa só o que mudou desde o último teste e remove o que saiu do pack.
- Guarda os mundos de teste (`saves/`), que **nunca** vão para o pack.
- O usuário pode abrir a pasta, editar configs dela (pelo editor do Warden ou por fora) e, ao fim do teste, revisar o que mudou para trazer ao pack.

### 3.3 Versão salva

- Um ponto do histórico com número SemVer, data e changelog.
- Criada só pelo botão "Salvar versão", e só no computador. Mudanças ainda não salvas aparecem como "Alterações não salvas".
- Pode ser marcada como **versão final**. Só versões finais podem ser **publicadas** (T18), e os jogadores recebem só o que foi publicado.
- "Voltar para uma versão" nunca apaga o histórico; o estado anterior fica guardado num ponto de segurança.

### 3.4 Pontos de segurança

- Antes de ações que substituem ou apagam conteúdo do pack, o Warden registra automaticamente o estado atual num ponto de segurança escondido. A lista exata de ações está em ARCHITECTURE §11: voltar versão, limpeza de pack antigo, atualizar todos, remover dois ou mais itens, substituir um item por outro de outra fonte e trazer mudanças do teste que sobrescrevem arquivos do pack. Na P0 o ponto é criado e fica no repositório do pack; a tela para listar e recuperar pontos de segurança é P1 (T17).

## 4. Prioridades e escopo

| Prioridade | Significado |
|---|---|
| **P0** | Obrigatório na v1. Sem isso o Warden não cumpre o objetivo. |
| **P1** | Entra na v1, construído logo depois do P0 correspondente. |
| **P2** | Depois da v1. Está descrito para orientar o desenho, mas não é construído agora. |
| **Fora da v1** | Não planejado (§9). |

Suporte de versões (ADR-0005): **garantido de 1.7.10 até a versão mais nova do Minecraft** desde a primeira versão do app, com Forge (todas as versões, inclusive 1.7.10 e 1.12.2), NeoForge (1.20.1+) e Fabric (1.14+). Versões anteriores a 1.7.10 aparecem com a etiqueta "melhor esforço": podem ser criadas e testadas, mas não fazem parte dos testes automáticos e falhas nelas não bloqueiam entregas.

**Funções avançadas da D4 e prioridade** (decisão D15; R5A §10.1, R5B §9):

| Prioridade | Funções | Telas |
|---|---|---|
| P0 | Índice pacote → mod e config de mixin → mod; assinatura de travamento e lista de Travamentos; higiene dos arquivos do spark, do KubeJS e do ProbeJS; mods iniciais (spark e Crash Assistant) e ferramentas do jogador; RAM do processo e tempo de carregamento em todo teste; consultas do grafo (quem depende, por que está no pack, bibliotecas sem uso); busca em todas as configs; página de descoberta em tela cheia com início, categorias, seleção múltipla e descrição higienizada | T03, T04, T06, T07, T08, T12, T13, T14, T15 |
| P1 | Nota de saúde (Problemas e Meus packs); console agrupado; memória da JVM; busca do culpado (no cliente, modo assistido e com servidor local); IA com ferramentas e conversas; raio-x de mixins (camadas A e B); tempo por mod e perfil de desempenho com spark; grafo desenhado; formulário de configs em camadas, padrão e restaurar, validação de faixa; editor de scripts; perfis do teste com Quick Play; servidor local; teste "como o jogador recebe" pelo link; pacote para servidor; importar; navegar por modpacks; kits de desempenho; galeria, versões e links na pré-visualização | T02, T06, T07, T08, T11, T12, T13, T14, T19, T24, T25, T26, T27 |
| P2 | Ver §9 | — |

## 5. Mapa de telas

Estrutura aprovada pelo dono em 01/10/2026 (ADR-0026; detalhes e justificativas em `docs/design/ESTRUTURA.md`; rascunho clicável em `design/estrutura/index.html`).

```
Primeira execução (T01) ──► Meus packs (T02, com a coluna Saúde) ... nível do APP
                               ├── Criar pack (T03, 5 etapas com Mods iniciais)
                               ├── Abrir ou importar… (T04; importar de outros apps: T24)
                               ├── Configurações (T21), com "Sobre o Warden" (T23)
                               └── [abrir um pack] ──► Pack aberto (T05) ...... nível do PACK
                                     Cabeçalho: ← Meus packs · nome · Editar informações (T11)
                                                · avisos · Salvar versão (T16) · ▶ Testar ▾ (T13)
                                     Menu lateral (6 seções):
                                     ├── Mods (T06): Ver como: Lista · Grafo
                                     │     ├── Detalhes do item (T07), com o raio-x de mixins
                                     │     ├── Adicionar: página de descoberta em tela cheia (T08),
                                     │     │     com Tipo = Modpacks
                                     │     │     └── Dependências e conflitos (T09)
                                     │     └── Atualizações (T10)
                                     ├── Configs (T12): busca em todas as configs; scripts (T26)
                                     ├── Problemas (T14): saúde do pack, problemas, travamentos
                                     ├── ✦ Diagnóstico com IA (T14): conversas
                                     ├── Histórico (T17), com Publicar versão (T18)
                                     └── Exportar (T19), com Pacote para servidor
                                     Fora do menu:
                                     ├── Teste (T13): etapas, downloads manuais (T20), jogo aberto
                                     │     com faixa de desempenho e console agrupado, e resultado,
                                     │     com "O que mudou durante o teste" (T15) e "Por que travou" (T14)
                                     ├── Busca do culpado (T25): modo da tela do teste
                                     └── ▾ do Testar: ver último teste; outros testes (como o jogador
                                           recebe, como servidor (T27), perfil de desempenho,
                                           encontrar o mod culpado); perfil do teste e ajustes (T11);
                                           instância de teste
Tarefas em andamento (T22): indicador no rodapé de todas as telas, abre uma gaveta.
```

Onde fica cada função nova da D4 e por quê: `docs/design/ESTRUTURA.md` §13 (ADR-0036).

**Navegação:**
- **Dois níveis que não se misturam.** O nível do app não tem barra lateral: Meus packs é a tela inicial, com Criar pack, Abrir ou importar… e Configurações no topo. Itens de um pack só existem depois de abrir um pack.
- **Dentro do pack**, o cabeçalho fixo sempre mostra qual é o pack e tem "← Meus packs". O menu lateral tem 6 seções, cada uma com nome, uma linha dizendo o que tem dentro e um contador quando faz sentido. O pack abre em Mods.
- **Testar** não é seção: é o botão principal do cabeçalho, presente em todas as telas do pack.
- **No máximo um nível de navegação visível.** Nenhuma tela tem abas; filtros, origens e modos de exibição usam caixas de seleção. Detalhes abrem em painel lateral e confirmações em diálogo. A página de descoberta (T08) abre em tela cheia e recolhe o menu lateral para ícones enquanto está aberta; isso é estado de exibição, não navegação nova (ESTRUTURA N6).
- **O raro fica escondido até ser preciso:** menus "▾" e "⋯", "Mais opções" e links discretos no fim das páginas.

## 6. Telas e fluxos

Formato de cada tela: objetivo, conteúdo, ações, estados (vazio, carregando, erro), regras e **critérios de aceite** (CA). Cada CA é verificável por teste automatizado ou por roteiro manual descrito na tarefa.

Convenção dos CA: "Dado / Quando / Então" condensado numa frase. "Mensagem" sempre significa texto em pt-BR do catálogo de i18n.

---

### T01 — Primeira execução (P0)

**Objetivo:** preparar o Warden na primeira vez que ele abre.

**Passos:**
1. Boas-vindas com o aviso legal obrigatório (ADR-0010): "NÃO É UM PRODUTO OFICIAL DO MINECRAFT. NÃO É APROVADO PELA MOJANG OU PELA MICROSOFT NEM ASSOCIADO A ELAS." e "O Warden abre o Minecraft em modo offline só para testar seus packs. Você precisa possuir o Minecraft: Java Edition." Botão "Entendi".
2. Nome do jogador usado nos testes (padrão `Jogador`; regra: 3 a 16 caracteres, só letras sem acento, números e `_`).
3. Pasta onde os packs serão criados (padrão `Documentos\Warden`).
4. Chaves opcionais (CurseForge, Gemini, GitHub), com "Pular, configuro depois". Ficam no cofre do Windows; a opção de guardar num arquivo `.env` fica em Configurações (T21).

**Regras:** o assistente só aparece se não existir `settings.json`. Pode ser reaberto em Configurações → "Rever boas-vindas". Não tem menu: só o indicador de etapas.

| CA | Critério |
|---|---|
| CA-T01-01 | Com o diretório de configuração vazio, abrir o app mostra a primeira execução; ao concluir, `settings.json` existe e a Lista de packs aparece. |
| CA-T01-02 | O nome `Zé` é recusado com mensagem explicando a regra; `Ze_123` é aceito. |
| CA-T01-03 | Pular as chaves não impede concluir; a busca da CurseForge mostra depois o estado "chave não configurada" (T08). |
| CA-T01-04 | O aviso legal aparece também em "Sobre o Warden" (T23). |

---

### T02 — Lista de packs (P0)

**Objetivo:** ver e abrir os seus packs. É a tela inicial do app ("Meus packs").

**Conteúdo:** uma lista, uma linha por pack, com nome, ícone (se houver), versão do Minecraft, loader e versão, versão do pack, **saúde do pack** (P1: nota e faixa, iguais às de Problemas, T14), data da última alteração, resultado do último teste ("nunca testado", "abriu normalmente", "travou") e quantidade de alterações não salvas.

**Ações:** Abrir; **Criar pack** (T03); **Abrir ou importar…** (T04, e T24 para arquivos de outros apps); **Configurações** (T21), no topo; menu **⋯** de cada pack: Mostrar na pasta, Remover da lista (não apaga arquivos), Apagar pack (P1: move para a Lixeira após digitar o nome do pack).

**Estados:**
- Vazio: "Você ainda não tem packs." com os botões Criar pack e Abrir ou importar….
- Pasta sumiu: linha em destaque "Pasta não encontrada" com **Localizar…** (escolher a nova pasta) e **Remover da lista**.
- Pack inválido (`pack.toml` ilegível): linha "Não foi possível ler este pack" com **Ver detalhes** (erro técnico) e **Mostrar na pasta**.

| CA | Critério |
|---|---|
| CA-T02-01 | Com 50 packs registrados, a lista aparece em menos de 1 s (medido em teste de desempenho). |
| CA-T02-02 | Renomear a pasta de um pack por fora faz a linha do pack mostrar "Pasta não encontrada"; "Localizar…" com a nova pasta restaura a linha sem perder o histórico de testes. |
| CA-T02-03 | "Remover da lista" não altera nenhum arquivo no disco (verificado por hash da pasta antes/depois). |
| CA-T02-04 | O contador de alterações não salvas bate com a quantidade de arquivos alterados desde a última versão salva. |
| CA-T02-05 | (P1) A coluna Saúde mostra, para cada pack, a mesma nota e faixa que a seção Problemas do pack (T14); um pack que nunca passou pelo diagnóstico mostra "sem dados", nunca uma nota inventada; abrir a lista não roda o diagnóstico (a nota vem do último cálculo guardado). |

---

### T03 — Criar pack (P0)

**Objetivo:** criar um pack novo, pronto para receber mods.

**Passos do assistente** (5 etapas: Nome e pasta · Versão do Minecraft · Loader · Mods iniciais · Resumo):
1. **Nome e pasta:** nome (obrigatório, 1–64 caracteres), autor (padrão: nome configurado), descrição (opcional), pasta de destino (padrão `<pasta dos packs>\<nome-em-minúsculas-com-hífens>`). A pasta precisa não existir ou estar vazia.
2. **Versão do Minecraft:** lista das versões *release*, da mais nova para a mais antiga, ordenada pela ordem oficial da Mojang (nunca por comparação de texto; existem versões como `26.3`). Busca por texto. Versões anteriores a 1.7.10 com etiqueta "melhor esforço". Snapshots não aparecem na v1.
3. **Loader:** Forge, NeoForge, Fabric ou "Nenhum (vanilla)". Só aparecem os loaders que existem para a versão escolhida (NeoForge 1.20.1+; Fabric 1.14+; Forge conforme o repositório oficial). Versão do loader pré-selecionada: Forge "recomendada" (ou a mais recente se não houver recomendada), NeoForge a mais recente estável (sem `-beta`), Fabric a mais recente estável. Lista completa disponível em "Escolher outra versão".
4. **Mods iniciais** (ADR-0033; decisão D16): **spark** (mede o que deixa o jogo lento; lado "Cliente e servidor") e **Crash Assistant** (mostra uma janela clara para o jogador quando o jogo trava; o envio de dados ao autor do mod vem desligado; fica fora dos seus testes normais) já vêm **marcados**, cada um com uma frase dizendo para que serve e com a versão que será usada; dá para desmarcar. Em Fabric, o spark traz a Fabric API junto. Em 1.7.10 e 1.12.2, o spark é uma versão antiga da CurseForge, com o aviso "versão antiga, sem atualizações" (sem chave da CurseForge, o item aparece desabilitado com o motivo). Abaixo, **Kit de desempenho** (P1), desmarcado: escolher um kit da faixa mostra os mods que ele traz. Sem internet: a etapa mostra "Não foi possível consultar os mods iniciais agora" e deixa seguir sem eles.
5. **Resumo** e botão **Criar pack**. Depois de criar, o pack abre em Mods com os mods iniciais escolhidos (ou vazio, com **Adicionar mods**). Os mods iniciais entram pelo mesmo caminho de adicionar (T09), num ponto único depois do "Pack criado".

**O que o Warden cria** (sem usar `packwiz init`, que falha com Forge antigo; R3 §1.5.1), além das referências dos mods iniciais escolhidos:
- `pack.toml` (versão inicial `0.1.0`), `index.toml` vazio, já com o índice atualizado pelo packwiz;
- `.gitattributes` com `* -text` (impede o Windows de mudar finais de linha e quebrar os hashes);
- `.packwizignore` e `.gitignore` padrão do Warden (ARCHITECTURE §6.4);
- `.warden/project.toml` (identificador do pack e a lista das ferramentas do jogador, ADR-0033);
- com o Crash Assistant marcado, a config inicial dele com o envio de dados ao autor e o encurtador de links desligados (só as chaves documentadas pelo autor do mod);
- `CHANGELOG.md` com cabeçalho;
- repositório git com um ponto inicial "Pack criado" (não é uma versão salva).

**Estados:** carregando versões (esqueleto de lista); sem internet e sem catálogo em cache: "Não foi possível carregar as versões do Minecraft. Verifique a internet." com **Tentar de novo**; com catálogo em cache: usa o cache e mostra "Lista de versões de <data>".

| CA | Critério |
|---|---|
| CA-T03-01 | Criar um pack Forge 1.7.10 (`10.13.4.1614`) gera `[versions] forge = "10.13.4.1614"` e `minecraft = "1.7.10"`, e `packwiz refresh` roda nele com saída 0 e sem diferença nos arquivos (teste de integração contra o packwiz real). |
| CA-T03-02 | O mesmo vale para Forge 1.12.2, Forge 1.20.1, NeoForge 1.20.1, NeoForge 1.21.1, Fabric 1.20.1, Fabric na versão mais nova e "Nenhum (vanilla)". |
| CA-T03-03 | NeoForge não aparece para 1.19.2; Fabric não aparece para 1.12.2. |
| CA-T03-04 | Destino não vazio é recusado antes de qualquer escrita, com mensagem explicando. |
| CA-T03-05 | Depois de criar, a pasta contém exatamente os arquivos listados acima mais `.git/`; nenhum outro arquivo. |
| CA-T03-06 | Falha no meio da criação (simulada em teste) não deixa pasta parcial: ou o pack completo existe, ou a pasta volta ao estado anterior. |
| CA-T03-07 | Criar um pack Fabric 1.21.1 com os mods iniciais marcados grava `.pw.toml` do spark, da Fabric API e do Crash Assistant, a config inicial do Crash Assistant com o envio ao autor desligado e o Crash Assistant listado como ferramenta do jogador em `.warden/project.toml`; desmarcar os dois cria o pack sem nenhum `.pw.toml` (teste de integração com servidor simulado). |
| CA-T03-08 | Num pack Forge 1.7.10 e num Forge 1.12.2, a etapa Mods iniciais oferece o spark da CurseForge com o aviso "versão antiga, sem atualizações"; sem chave da CurseForge, o item aparece desabilitado com o motivo e a criação continua. |

---

### T04 — Abrir pack existente (P0)

**Objetivo:** trazer para o Warden um pack packwiz que já existe (por exemplo, criado pelos apps anteriores ou clonado do GitHub).

**Onde:** botão **Abrir ou importar…** de Meus packs (decisão D20). O Warden reconhece o que foi escolhido pelo conteúdo: pasta com `pack.toml` segue este fluxo; `.mrpack`, zip da CurseForge ou instância do Prism, MultiMC ou do app da CurseForge seguem para **Importar modpack** (T24).

**Fluxo:**
1. Escolher a pasta que contém `pack.toml`.
2. Validações, em ordem:
   - `pack.toml` legível; senão erro com detalhe técnico.
   - Loader suportado: Forge, NeoForge, Fabric ou nenhum. Quilt e LiteLoader: "Este pack usa Quilt, que o Warden ainda não suporta." (não abre). Mais de um loader: não abre, com explicação.
   - Sem `.warden/project.toml`: cria com identificador novo. Identificador já usado por outro pack registrado (pasta copiada): gera um novo e avisa.
   - Não é repositório git: cria o repositório e registra o estado atual como ponto inicial "Pack importado".
   - Já é repositório git (por exemplo, feito pelos apps antigos ou clonado): usa a branch atual sem mudar nada; a última versão salva é a tag `vX.Y.Z` mais recente (ARCHITECTURE §11). Repositório com merge/rebase em andamento ou HEAD destacado: abre só para leitura, com explicação.
3. **Verificação de higiene** (P0): procura no `index.toml` e na pasta arquivos que não deveriam estar no pack (`*.bak`, `*.tmp`, `*.log`, `packwiz-installer-bootstrap.jar`, `.packwiz.toml`, `logs/`, `crash-reports/`, `saves/`, caches de mods etc.; lista em ARCHITECTURE §6.4). Se encontrar, mostra "Encontramos N arquivos que não deveriam ir para quem joga o pack" com a lista (caminho, tamanho, motivo) e caixas marcadas. **Limpar** cria um ponto de segurança, apaga os marcados, adiciona as regras faltantes ao `.packwizignore` e atualiza o índice. **Agora não** abre o pack mesmo assim (o aviso reaparece em Exportar).
4. Arquivos de controle faltando ou diferentes do padrão (`.gitattributes`, `.packwizignore`): oferece adicionar, mostrando a diferença antes. Independentemente da resposta, as linhas obrigatórias que impedem `.warden/` e `CHANGELOG.md` de entrarem no pack são sempre acrescentadas ao `.packwizignore` (ARCHITECTURE §6.4), com aviso.

| CA | Critério |
|---|---|
| CA-T04-01 | Abrir um pack com `config/x.toml.bak`, `packwiz-installer-bootstrap.jar` e `.packwiz.toml` indexados lista os três; "Limpar" remove-os do disco e do `index.toml`, e o `packwiz refresh` seguinte não os reinclui. |
| CA-T04-02 | Abrir um pack Quilt mostra a mensagem de não suportado e não cria `.warden/` nem altera nada na pasta. |
| CA-T04-03 | Abrir uma cópia de um pack já registrado gera identificador novo; os dois aparecem na lista com instâncias de teste separadas. |
| CA-T04-04 | "Agora não" não altera nenhum arquivo além de criar `.warden/project.toml`, acrescentar o bloco obrigatório ao `.packwizignore` e criar o repositório git, se faltarem; o `index.toml` seguinte não contém `.warden/` nem `CHANGELOG.md`. |
| CA-T04-05 | Num pack com `kubejs/config/web_server.json`, `.probe/`, `.vscode/`, `local/kubejs/` e `config/spark/perfil.sparkprofile`, a verificação de higiene aponta os cinco; com o `.packwizignore` padrão do Warden (ARCHITECTURE §6.4), nenhum deles entra no `index.toml` depois do `packwiz refresh` (teste de integração com o packwiz real). |

---

### T05 — Pack aberto: cabeçalho e seções (P0)

**Cabeçalho fixo** (em todas as telas do pack):
- **← Meus packs**; nome do pack com **Editar informações** (T11); versão do Minecraft, loader e versão, versão do pack.
- Avisos passageiros: "Mudanças do teste para revisar" (abre o resultado do teste, T15); "O pack mudou desde o início do teste" (T13).
- Botão **Salvar versão · N alterações** (T16); N é a quantidade de alterações não salvas (sem número quando não há nenhuma).
- Botão **▶ Testar** com menu **▾** (T13). Durante a preparação vira "Testando… ver progresso"; com o jogo aberto, "● Jogo aberto: ver teste"; durante a busca do culpado (T25), "Buscando o culpado: ver progresso". Todos levam à tela do teste. Com um perfil do teste que não é o padrão, o botão mostra o nome do perfil ("▶ Testar · PC fraco", T11).

**Menu lateral** (só existe dentro do pack), com nome, descrição e contador:

| Seção | Descrição na interface | Contador | Tela |
|---|---|---|---|
| Mods | Mods, resource packs e shaders | itens no pack | T06 (com T07–T10) |
| Configs | Arquivos de ajuste e scripts do pack | — | T12, T26 |
| Problemas | Saúde do pack, problemas e travamentos | problemas encontrados | T14 |
| ✦ Diagnóstico com IA | Conversar com a IA sobre um problema do pack | — | T14 |
| Histórico | Versões salvas e publicação | alterações não salvas | T17, T18 |
| Exportar | Gerar o pack para quem vai jogar | — | T19 |

O pack abre em Mods. A tela do teste (T13) não aparece no menu.

**Regras:**
- O estado mostrado vem sempre do disco. Se a pasta do pack mudar por fora (editor externo, `git pull` manual), o Warden recarrega a seção em até 2 s e avisa com uma notificação discreta (P1; no P0 o Warden relê o disco ao abrir cada seção e antes de qualquer escrita).
- Toda escrita no pack passa por uma trava do pack: duas operações de escrita no mesmo pack nunca rodam ao mesmo tempo; a segunda espera na fila e aparece em Tarefas (T22).
- Nenhuma tela do pack tem abas (ADR-0026).

| CA | Critério |
|---|---|
| CA-T05-01 | Editar um config por fora e voltar ao Warden mostra o conteúdo novo sem reiniciar o app. |
| CA-T05-02 | Disparar "Atualizar todos" e "Remover mod" em sequência rápida resulta nas duas operações executadas em ordem, sem corromper `index.toml` (teste de concorrência). |
| CA-T05-03 | Sem pack aberto, nenhum item de pack aparece na interface; em qualquer tela do pack, o cabeçalho mostra o nome do pack e "← Meus packs", e o menu lateral tem exatamente as 6 seções da tabela (teste de componente das rotas). |

---

### T06 — Mods, Resource packs e Shaders (P0)

Seção **Mods**: uma lista única, agrupada por tipo (Mods, Resource packs, Shaders), com grupos recolhíveis e contagem. O comportamento é o mesmo nos três grupos; muda só o tipo de conteúdo e a pasta (`mods/`, `resourcepacks/`, `shaderpacks/`).

**Lista** (virtualizada; precisa ficar fluida com 500 itens):

| Coluna | Conteúdo |
|---|---|
| Ícone e nome | Nome do projeto (do `.pw.toml`); ícone do cache de metadados. |
| Versão | Versão legível (ex.: `0.6.0+mc1.21.1`). Modrinth: número da versão do cache local. CurseForge: nome do arquivo. Link/arquivo local: nome do arquivo. Nunca um ID interno. |
| Fonte | Modrinth, CurseForge, Link direto, Arquivo local. |
| Lado | Cliente e servidor / Só cliente / Só servidor. Editável na linha. |
| Atualização | Em dia / Atualização disponível / Não verificado / Não foi possível verificar / Não se aplica (arquivo local ou link). |
| Marcas | Fixado (P1), Opcional (P1), Problema (do diagnóstico, com cor por gravidade). |

**Ações:** buscar por nome; filtrar por tipo, fonte, lado, "com atualização", "com problema" (caixas de seleção); ordenar; selecionar vários e **Alterar lado**, **Remover**, **Atualizar** (a barra de ações aparece quando há seleção); **Adicionar** (T08); **Verificar atualizações** (T10); faixa "N atualizações disponíveis" com **Revisar e atualizar**; clicar abre Detalhes (T07) num painel lateral, sem sair da lista. Arrastar arquivos `.jar`/`.zip` para a lista inicia "Adicionar arquivo local" (T08).

**Remover:** diálogo de confirmação que lista o que será removido e avisa "Estes mods dependem dele: …" quando houver, considerando a cadeia inteira (quem depende de quem depende dele; consultas do grafo, ADR-0034). P1: oferece também remover dependências que ficaram sem uso ("órfãs"). O Warden apaga o `.pw.toml` (ou o arquivo local) e atualiza o índice; não usa `packwiz remove` (R4 §2.3).

**Ver como: Lista · Grafo** (seletor de modo no topo da seção; Grafo é P1): o grafo mostra **um mod no centro** (escolhido num seletor com busca, ou pelo link "Ver no grafo" dos detalhes), o que ele exige à esquerda e quem depende dele à direita, até 2 níveis. Tipos de ligação, sempre com legenda e nunca só pela cor: obrigatória (linha cheia), opcional ou recomendada (tracejada), incompatível (vermelha, com o ícone de alerta), inferida pela busca do culpado ou pelo log (pontilhada, com "inferida"); dependências embutidas aparecem dentro do nó do mod que as traz ("dentro do Create"). Ao lado, em texto: "Se você remover o Create, N mods param de funcionar: …", "Por que está no pack" e bibliotecas sem uso ("Kotlin for Forge: nenhum mod usa mais; pode ser removida"). Cada nó é um botão (teclado) que abre os detalhes (T07); as ligações também ficam listadas em texto. Opção "Ver o pack inteiro" com as bibliotecas agrupadas (P1, para packs pequenos; acima de 150 nós o Warden avisa que fica difícil de ler).

**Estados:**
- Vazio: "Nenhum mod ainda." + **Adicionar mods**.
- Item com arquivo inválido: a linha aparece com "Arquivo inválido" e ações **Ver erro** e **Abrir no editor de texto**; os demais itens continuam normais (um arquivo ruim nunca derruba a lista).
- Arquivo na pasta que não está no índice: linha com "Fora do índice" e ação **Incluir no pack**.

| CA | Critério |
|---|---|
| CA-T06-01 | Um `.pw.toml` com TOML inválido aparece como item com erro e os outros 99 itens aparecem normalmente. |
| CA-T06-02 | A coluna Versão nunca mostra um ID do Modrinth (8 caracteres alfanuméricos) nem um file-id numérico da CurseForge (teste de componente com dados reais). |
| CA-T06-03 | Mudar o lado de 10 mods de uma vez grava `side` nos 10 `.pw.toml`, atualiza o índice uma única vez e o diff do git mostra só as linhas `side`. |
| CA-T06-04 | Remover um mod apaga o `.pw.toml` e a entrada do `index.toml`; `packwiz refresh` seguinte não produz diferença. |
| CA-T06-05 | Com 500 itens, rolar a lista mantém 60 quadros por segundo em máquina de referência (medido no teste de desempenho A-04). |
| CA-T06-06 | Num pack de teste com a cadeia A → B → C (A depende de B, que depende de C), remover C lista B e A como dependentes; uma biblioteca que nenhum mod usa mais aparece como "sem uso"; as consultas consideram `provides` e jar-in-jar (teste de domínio com o modelo do diagnóstico). |
| CA-T06-07 | (P1) No modo Grafo, com o Create no centro de um pack de teste, aparecem à esquerda as dependências obrigatórias (com as embutidas dentro do nó) e à direita os dependentes, com o tipo de cada ligação na legenda e na lista em texto; todos os nós são alcançáveis por teclado e abrem os detalhes; `axe` sem violações (teste de componente). |

---

### T07 — Detalhes do item (P0)

**Conteúdo:** ícone, nome, autores, link da página (abre no navegador), descrição (Modrinth em Markdown; CurseForge em HTML, ambas higienizadas), informações da versão instalada (número, versões do Minecraft, loaders, data, nome do arquivo, tamanho, hashes), lado (editável), **Depende de** (obrigatórias, opcionais e incompatíveis, cada uma com estado "no pack" ou "falta"), **Usado por** (itens que dependem deste), **Por que está no pack** ("Você adicionou" ou a cadeia até um mod escolhido por você: "exigido por Sophisticated Backpacks, que você adicionou"), link **Ver no grafo** (P1), changelog da versão instalada.

**O que este mod altera no jogo** (P1, raio-x de mixins, ADR-0034): resumo ("Altera 214 partes do jogo, em 3 configs. 2 também alteradas por outros mods.") e as sobreposições com outros mods, cada uma com o ponto do jogo (nome legível quando houver), o tipo de alteração, o risco (alto, médio, baixo, com selo e palavra) e o motivo em linguagem simples, inclusive os rebaixadores ("compatibilidade intencional", "pode ser desligado pelo próprio mod", "par conhecido como compatível"). **Ver todas as alterações** troca o conteúdo do mesmo painel pela lista completa (filtrável por texto e por risco), com "← Detalhes do mod". Mods sem mixins: "Este mod não altera o código do jogo por mixins." A linguagem é sempre "alteram o mesmo ponto do jogo", nunca "são incompatíveis".

**Ações:** Atualizar (quando houver), Remover, Abrir página; em **Mais opções**: **Trocar de versão** (P1: escolher qualquer versão compatível, inclusive mais antiga), **Opcional** e **Fixar versão** (P1, ver T11).

**Regras:** dados da CurseForge são buscados na hora e não são guardados em disco (termos da CurseForge, R3 §3.2). Dados do Modrinth ficam no cache local.

| CA | Critério |
|---|---|
| CA-T07-01 | Uma descrição com `<script>` ou `onerror=` não executa código (teste de componente com HTML malicioso). |
| CA-T07-02 | Sem internet, os detalhes de um mod do Modrinth abrem do cache; os de um mod da CurseForge mostram "Detalhes indisponíveis sem internet" e ainda exibem nome, arquivo, lado e hash a partir do `.pw.toml`. |
| CA-T07-03 | "Por que está no pack" de uma biblioteca mostra a cadeia até o mod adicionado pelo usuário; de um mod adicionado diretamente, "Você adicionou" (teste de domínio com pack de teste). |
| CA-T07-04 | (P1) Para cada jar do corpus do raio-x (pelo menos Sodium, Lithium, Iris e ModernFix no Fabric 1.20.1; Embeddium, Oculus, ModernFix e FerriteCore no Forge 1.20.1; Sodium, Iris e FerriteCore no NeoForge 1.21.1, inclusive o jar aninhado em `META-INF/jarjar/`), o inventário (configs, classes de mixin e alterações por tipo) bate com o resultado dourado; nenhuma classe gera erro de leitura. |
| CA-T07-05 | (P1) As regras de risco seguem ARCHITECTURE §9.6: dois `@Redirect` no mesmo `@At` = alto; `@Overwrite` + injeção no corpo = médio; só `@Inject` = baixo; com plugin de mixin em todos os envolvidos ou mixin de compatibilidade (referencia classes do outro mod), o risco desce um nível e o motivo aparece; nenhuma sobreposição bloqueia o teste (testes de domínio com jars sintéticos e reais). |

---

### T08 — Adicionar: página de descoberta, busca combinada, link e arquivo (P0; partes P1 indicadas)

Página **Adicionar** dentro do pack, aberta pelo botão **Adicionar** da seção Mods, **em tela cheia**: o menu lateral do pack recolhe para ícones (com o nome no tooltip) enquanto ela está aberta, e o cabeçalho do pack continua visível (decisão D15, ESTRUTURA N6). No topo, "← Voltar para Mods" com quantos itens entraram ("· 2 adicionados"). Sem abas:
- seletor **Tipo** (Mods, Resource packs, Shaders e, P1, **Modpacks**), que já vem no tipo de onde você clicou;
- um **campo único** que busca pelo nome ou aceita um **link colado** (o Warden reconhece o link pelo formato);
- o botão **Escolher arquivo do computador…** (ou arrastar arquivos para a lista de Mods).

Três colunas: **filtros** (categorias, ambiente, ordenar; fonte em "Mais filtros"), **resultados** e **pré-visualização**. De 1024 a 1180 px, a coluna de filtros recolhe atrás do botão "Filtros".

**Início (P0), com o campo vazio:** listas (nunca grade de cartões) "Populares para <loader> <versão>" e "Atualizados recentemente", já filtradas para o pack, e a lista de **categorias** na coluna de filtros, com nomes em português de um mapeamento curado que junta as categorias do Modrinth e da CurseForge (categoria sem par filtra só a fonte que a tem). P1: "Kits de desempenho" da faixa do pack (ADR-0033), que abrem no diálogo de dependências (T09) com as caixas marcadas. Orçamento: no máximo 3 requisições por fonte para montar o início.

**Seleção múltipla (P0):** cada resultado tem uma caixa de seleção; a barra fixa "N selecionados · **Adicionar N ao pack**" abre **um** diálogo de dependências (T09) para o conjunto. Itens "Já no pack" não têm caixa.

**Busca combinada (Modrinth + CurseForge; ADR-0027):**
- Uma busca consulta as duas fontes ao mesmo tempo e mostra **um resultado só**. Cada item mostra a fonte: "Modrinth", "CurseForge" ou "Modrinth e CurseForge".
- **Sem duplicatas:** o mesmo mod nas duas fontes vira um item só. Na lista, são o mesmo mod quando autor e nome (ou slug) coincidem nas duas fontes. Na pré-visualização, o Warden confere o arquivo da versão escolhida pelo SHA-1 nas duas fontes e avisa se forem diferentes. O item unificado tem o seletor **Fonte**; o padrão é o Modrinth (informa o lado e tem cache local).
- **Ordem:** relevância intercalada pela posição em cada fonte, sem favorecer nenhuma; ordenar por downloads, atualização recente ou novos usa os valores de cada fonte. O filtro **Fonte** (todas, só Modrinth, só CurseForge) fica em "Mais filtros".
- Filtros já aplicados e travados por padrão: versão do Minecraft do pack, loader do pack (mods) e tipo de projeto. "Mostrar também os sem versão compatível" desligado por padrão.
- Filtros extras (em "Mais filtros"): fonte, categorias, ambiente (cliente/servidor), ordenar por relevância, downloads, atualizados recentemente, novos.
- Resultado: ícone, nome, autor, resumo, downloads, data da última atualização, fonte, marca "Já no pack" e, na CurseForge, marca "Download manual necessário" quando o autor bloqueou downloads por apps de terceiros.
- Rolagem infinita, 20 itens por página (já combinados).
- Clicar abre a pré-visualização com seletor de fonte (quando houver duas) e de versão. Versão padrão: a mais nova compatível do canal configurado (padrão: só versões estáveis; beta/alpha só com a opção em Configurações). A pré-visualização é uma página rolável, **sem abas**, com um índice fixo "Descrição · Galeria · Versões · Dependências · Links": cabeçalho (ícone, nome, autor, fontes, downloads, atualização, licença, lado, **Adicionar ao pack**); **descrição** (P0) com a higienização da ARCHITECTURE §18 (no Modrinth, `rehype-raw` antes de `rehype-sanitize`; vídeos viram miniatura com "Abrir no navegador", nunca `iframe`); P1: **galeria** (miniaturas; clicar mostra a imagem grande sobre a página), **versões** filtradas para o pack com o changelog de cada uma (expansível; na CurseForge, buscado só ao abrir a linha), **dependências** (obrigatórias, opcionais, incompatíveis, cada uma com "Já no pack" ou "Será adicionada") e **links** (issues, código, wiki, Discord; abrem no navegador).
- **Ordenar por downloads ou atualização** com as duas fontes mescla pelos números de cada uma; relevância continua intercalada pela posição (ADR-0027).
- **Adicionar ao pack** abre a tela de dependências (T09). Depois de adicionar, a busca continua aberta e o item passa a mostrar "Já no pack".
- **Imagens da CurseForge (P1):** servidas por um protocolo próprio do Warden, só em memória, para que o cache de disco do WebView não guarde dados da API (termos da CurseForge; ARCHITECTURE §17).
- **Sem chave da CurseForge:** a busca mostra só o Modrinth, com o aviso "Mostrando só o Modrinth. Para buscar também na CurseForge, informe sua chave em Configurações." e **Abrir Configurações**; nenhuma requisição vai para a CurseForge. **Chave recusada:** o mesmo, com "A CurseForge recusou a chave. Confira em Configurações."
- **Uma fonte com erro** (fora do ar, tempo esgotado): mostra os resultados da outra, com "Não foi possível buscar no <fonte> agora." e **Tentar de novo**.

**Link** (colado no campo único):
- Aceita link de projeto ou versão do Modrinth (resolvido pela API do Modrinth), link de projeto ou arquivo da CurseForge (passa pelo packwiz, conforme decisão do dono; ADR-0006) e link direto `https://` para um arquivo `.jar`/`.zip`.
- Link de **projeto** (Modrinth ou CurseForge) abre a pré-visualização com o seletor de versão, igual à busca; link de **arquivo/versão** usa exatamente aquele arquivo, avisando se ele não for compatível com o pack ou não for do canal configurado.
- Para link direto: o Warden baixa uma vez, calcula o hash, lê os metadados (se for mod), pede o tipo (mod, resource pack, shader) e grava a referência. Links `http://` são recusados.

**Arquivo:**
- Escolher um ou vários arquivos (ou arrastar para a lista).
- Para cada arquivo: o Warden lê os metadados (loader, versão do Minecraft) e procura o mesmo arquivo pelo hash no Modrinth e na CurseForge.
  - Encontrado: "Este arquivo é o Sodium 0.6.0 do Modrinth." com **Adicionar como referência ao Modrinth (recomendado)** ou **Adicionar como arquivo local**.
  - Não encontrado: entra como arquivo local (copiado para a pasta do pack).
- Loader ou versão do Minecraft incompatíveis: bloqueia com a explicação e oferece **Adicionar mesmo assim** (fica marcado como problema no diagnóstico).

**Regras comuns:**
- Nunca escolher um resultado sozinho: o que entra no pack é sempre o item que o usuário clicou (R3 §1.2).
- O lado é preenchido automaticamente a partir dos dados do Modrinth (tabela em ARCHITECTURE §6.2) ou dos metadados do jar. CurseForge, link e arquivo sem metadado de lado: "Cliente e servidor" com aviso "Lado desconhecido — confira se é só de cliente".
- Nomes de arquivo de referência são únicos no pack inteiro (ARCHITECTURE §6.2).

| CA | Critério |
|---|---|
| CA-T08-01 | Buscar "sodium" num pack Fabric 1.21.1 retorna o Sodium entre os 3 primeiros; adicionar grava `mods/sodium.pw.toml` com `[update.modrinth]`, hash `sha512` e `side = "client"`. |
| CA-T08-02 | Buscar "not enough items" num pack Forge 1.7.10 e escolher o NEI (resultado da CurseForge) adiciona o NEI (não outro resultado). |
| CA-T08-03 | Colar `https://www.curseforge.com/minecraft/mc-mods/jei` no campo de busca num pack Forge 1.20.1 adiciona o JEI via packwiz; o resultado é idêntico ao de adicionar o JEI pela busca da CurseForge (mesmo `.pw.toml`, exceto campos de lado definidos pelo Warden). |
| CA-T08-04 | Arrastar o `.jar` do Sodium baixado do Modrinth oferece "referência ao Modrinth"; aceitar grava `.pw.toml`, não copia o jar. |
| CA-T08-05 | Arrastar um jar Fabric num pack Forge mostra o bloqueio com explicação; "Adicionar mesmo assim" copia o arquivo e o diagnóstico passa a mostrar o erro de loader. |
| CA-T08-06 | Sem chave da CurseForge, a busca mostra só resultados do Modrinth, com o aviso descrito; nenhuma requisição é feita à CurseForge (verificado por mock). |
| CA-T08-07 | Um link `http://` é recusado com mensagem; nenhum arquivo é baixado. |
| CA-T08-08 | Com servidores simulados em que o JEI existe no Modrinth e na CurseForge com o mesmo arquivo, buscar "jei" num pack Forge 1.20.1 mostra o JEI uma única vez, marcado "Modrinth e CurseForge"; escolher a fonte CurseForge na pré-visualização grava `mode = "metadata:curseforge"`, e a Modrinth grava `[update.modrinth]`. Um mod só da CurseForge aparece marcado "CurseForge". |
| CA-T08-09 | Com a CurseForge respondendo erro 500 (simulado), a busca mostra os resultados do Modrinth e o aviso de fonte indisponível; com o Modrinth fora, mostra os da CurseForge; a interface nunca fica carregando para sempre. |
| CA-T08-10 | Com o campo vazio num pack Forge 1.20.1, o início mostra "Populares para Forge 1.20.1", "Atualizados recentemente" e as categorias, usando no máximo 3 requisições por fonte (servidor simulado contando chamadas); escolher a categoria "Tecnologia" filtra as duas fontes pelas categorias mapeadas. |
| CA-T08-11 | Marcar 3 resultados e clicar em "Adicionar 3 ao pack" abre um único diálogo de dependências com os 3 e as dependências deles; confirmar grava tudo de uma vez (uma transação, um `refresh`); falha simulada no meio não deixa nenhum `.pw.toml` novo. |
| CA-T08-12 | Uma descrição do Modrinth com HTML misturado ao Markdown (`<h1 style>`, `<iframe>` do YouTube, `<img onerror>`, `<script>`) aparece formatada, sem `iframe`, sem estilo inline e sem executar código; o vídeo vira miniatura com "Abrir no navegador" (teste de componente). |
| CA-T08-13 | (P1) A pré-visualização mostra galeria, versões com changelog e dependências com "Já no pack"/"Será adicionada"; na CurseForge, o changelog só é pedido ao abrir a linha (servidor simulado contando chamadas); nenhuma imagem da CurseForge fica no cache de disco do WebView (verificado pela pasta de dados do WebView depois de uma sessão). |

**Navegar por modpacks (P1; R5B §6):** com Tipo = **Modpacks**, os resultados são modpacks das duas fontes, já filtrados para o loader e a versão do pack. Abrir um modpack mostra, no painel da direita, o cabeçalho (nome, autor, downloads, versão do modpack, licença do projeto, com aviso quando não for livre) e **Mods deste modpack (N)**, com caixas de seleção e o estado de cada mod:
- **Já no seu pack** (pelo projeto na mesma fonte ou pela deduplicação entre fontes), sem caixa;
- **Compatível** (existe versão para o Minecraft e o loader do seu pack), com caixa;
- **Sem versão para o seu pack**, desabilitado, com o motivo;
- **Arquivo fora das lojas** (jars dentro do modpack), desabilitado: o Warden não copia jars de pacotes de terceiros.

Filtro "Só os que não estão no seu pack"; seletor **Versão**: "a mesma do modpack (testada junto)" (padrão quando o modpack é da mesma versão e do mesmo loader) ou "a mais nova compatível". **Adicionar N selecionados** abre um diálogo de dependências só (T09). No Modrinth, a lista de mods vem das dependências `embedded` da versão do modpack e, para detalhes, do `modrinth.index.json` lido por partes (HTTP Range), sem baixar o arquivo inteiro; na CurseForge, do `manifest.json` lido do mesmo jeito, só em memória. Modpack da CurseForge que bloqueia apps de terceiros: "Este modpack não pode ser aberto por apps de terceiros. Abra a página na CurseForge."

| CA | Critério |
|---|---|
| CA-T08-14 | (P1) Abrir um modpack do Modrinth (servidor simulado com dados reais gravados do "Create+") usa no máximo 4 requisições para listar os mods, marca corretamente "Já no seu pack", "Compatível", "Sem versão para o seu pack" e "Arquivo fora das lojas", e "Adicionar 5 selecionados" abre um único diálogo de dependências com as versões escolhidas pelo seletor Versão. |
| CA-T08-15 | (P1) A leitura por partes do `.mrpack` busca o `modrinth.index.json` com no máximo 6 requisições Range num arquivo de 3,5 MB (servidor simulado que conta bytes); se o servidor não aceitar Range, o Warden baixa o arquivo inteiro até 200 MB, com aviso, e recusa acima disso. |
| CA-T08-16 | (P1) Abrir um modpack da CurseForge lê o `manifest.json` por partes e não grava nenhuma resposta da API nem o manifesto em disco (inspeção da pasta de dados depois da sessão). |

---

### T09 — Dependências e conflitos ao adicionar (P0)

Aparece antes de gravar qualquer coisa no pack.

**Conteúdo:**
- **Obrigatórias** (resolvidas em cadeia): marcadas. Desmarcar mostra "Sem esta dependência o jogo provavelmente não abre."
- **Já no pack:** listadas em verde, sem ação.
- **Opcionais:** desmarcadas, só informativas.
- **Sem versão compatível:** "Nenhuma versão de X para 1.20.1 Forge." O item principal ainda pode ser adicionado (vai aparecer no diagnóstico).
- **Incompatibilidades declaradas** com itens do pack (Modrinth `incompatible`, CurseForge "incompatível", metadados do jar `breaks`/`incompatible`): aviso em vermelho com o motivo, quando o autor informar.
- **Duplicado entre fontes:** "Este mod já está no pack pela CurseForge." com **Substituir pela versão do Modrinth** e **Cancelar** (manter os dois faria o jogo travar com mod duplicado).

**Botão:** **Adicionar N itens**. A gravação é atômica: ou todos os itens confirmados entram, ou nenhum.

**Vários itens de uma vez** (seleção múltipla, kit de desempenho, mods de um modpack, mods iniciais): o diálogo agrupa por item escolhido, junta as dependências comuns (cada uma aparece uma vez) e mostra as incompatibilidades entre os próprios itens escolhidos.

| CA | Critério |
|---|---|
| CA-T09-01 | Adicionar um mod Fabric que exige Fabric API num pack sem ela lista a Fabric API como obrigatória e marcada; confirmar grava os dois `.pw.toml` e atualiza o índice uma vez. |
| CA-T09-02 | Num pack NeoForge 1.21.1 com Sodium, adicionar o Embeddium mostra a incompatibilidade declarada pela API do Modrinth. |
| CA-T09-03 | Falha de rede no meio da gravação (simulada) não deixa nenhum `.pw.toml` novo no pack. |
| CA-T09-04 | Adicionar pelo Modrinth um mod que já está no pack pela CurseForge (detectado por projeto equivalente ou hash do arquivo) mostra a opção de substituir. |
| CA-T09-05 | Adicionar de uma vez dois mods que exigem a mesma biblioteca lista a biblioteca uma vez só; dois itens escolhidos que se declaram incompatíveis geram o aviso antes de gravar. |

---

### T10 — Atualizações (P0 individual; P1 "Atualizar todos")

**Verificar:** botão na seção Mods e verificação automática ao abrir o pack se a última tiver mais de 24 h (configurável). Modrinth: uma única consulta em lote para o pack inteiro. CurseForge: consulta em lote por projeto. Um único "produtor" calcula o estado de cada item (R4 §2.4); a lista e os contadores leem desse mesmo resultado.

**Estados por item:** Em dia; Atualização disponível (mostra a versão nova); Não verificado (nunca checado); Não foi possível verificar (erro, com motivo); Não se aplica (arquivo local, link direto). Itens fixados (P1) mostram "Fixado" e não entram em "Atualizar todos".

**Atualizar um item (P0):** mostra versão atual → nova, changelog das versões no meio (Modrinth) ou da nova (CurseForge), novas dependências obrigatórias. **Atualizar** grava a nova referência e atualiza o índice.

**Atualizar todos (P1):** tela de revisão com cada item (atual → nova, canal, changelog recolhível, caixa marcada), novas dependências e incompatibilidades novas. Cria ponto de segurança antes. Depois de aplicar, roda o diagnóstico rápido.

| CA | Critério |
|---|---|
| CA-T10-01 | Num pack com 200 mods do Modrinth, a verificação faz no máximo 3 requisições à API do Modrinth (teste com servidor simulado contando chamadas). |
| CA-T10-02 | O botão Atualizar de uma linha só fica habilitado quando o estado da linha é "Atualização disponível". |
| CA-T10-03 | Com o canal "só estáveis", uma versão beta mais nova não é oferecida. |
| CA-T10-04 | Falha de rede numa verificação deixa os itens em "Não foi possível verificar", nunca em "Em dia". |

---

### T11 — Informações do pack, ajustes do teste, opcionais e fixar versão

**Informações do pack (P0):** diálogo aberto por **Editar informações**, ao lado do nome do pack no cabeçalho (T05): nome, autor, descrição (gravados no `pack.toml`), versão do loader (trocar para outra versão do mesmo loader; P1). Mostrar a pasta do pack e Apagar pack (P1) também ficam aqui.

**Ajustes do teste neste computador (P0):** diálogo aberto pelo menu **▾** do Testar (T13). Não fazem parte do pack e não contam como alteração não salva: memória do teste (Automático ou valor fixo; ver T13), Java do teste (Automático ou um Java instalado), argumentos extras da JVM (validados contra a versão do Java; argumentos desconhecidos geram aviso), "Recriar instância de teste" (apaga a instância; pergunta se mantém os mundos de teste).

**Java automático (ADR-0029):** o mais novo que funciona com a versão do Minecraft e o loader do pack, sempre na atualização mais recente desse Java (regra em ARCHITECTURE §7.3). A tela mostra a versão escolhida e o motivo; quando não é o Java mais novo, mostra "Por que não o Java N?" com a explicação (ex.: "O Forge 1.7.10 e 1.12.2 só abre no Java 8. A partir do Java 9 ele trava ao iniciar.").

**Mods opcionais (P1):** no detalhe do item, "Opcional" com descrição e "ligado por padrão". Grava a tabela `[option]` do `.pw.toml`. Aviso fixo: "O app do Modrinth instala todos os opcionais; o formato da CurseForge não suporta lado." Em Ajustes do teste, o usuário escolhe quais opcionais ligar na instância (padrão: os "ligados por padrão").

**Fixar versão (P1):** "Fixar versão" (em Mais opções do item, T07) grava `pin = true`; itens fixados não são atualizados em lote e mostram o cadeado.

**Perfis do teste (P1; R5B §4.1):** o diálogo Ajustes do teste ganha, no topo, o seletor **Perfil** (Padrão, mais os que você criar) com **Novo perfil**, **Renomear** e **Apagar**. Um perfil é um conjunto com nome de: memória, Java, argumentos da JVM, mods opcionais ligados, tamanho da janela e **Ao abrir o jogo**: "Menu principal" ou "Entrar direto no mundo de teste" (Quick Play; Minecraft 1.20 ou mais novo; nas versões antigas a opção aparece desabilitada com o motivo). Os perfis ficam neste computador, por pack, e não contam como alteração do pack. O menu ▾ do Testar mostra o grupo **Perfil do teste** para trocar com um clique; o botão Testar mostra o perfil quando não é o Padrão. Memória acima de 75% da RAM do computador gera aviso. Argumentos que impedem a leitura de memória pelo Warden (`-XX:+PerfDisableSharedMem`, `-XX:-UsePerfData`) geram aviso com **Remover estes argumentos** (ADR-0038).

| CA | Critério |
|---|---|
| CA-T11-01 | Mudar o nome do pack altera só a linha `name` do `pack.toml` (diff de uma linha) e o índice continua válido. |
| CA-T11-02 | Marcar um mod como opcional grava `[option] optional = true` com `description` e `default`; o packwiz-installer real, em modo sem interface, instala ou não o mod conforme `default` (teste de conformidade com `default = true` e `false`), e a instância do Warden faz o mesmo. |
| CA-T11-03 | Para um pack Forge 1.20.1, Ajustes do teste mostra "Automático: Java 17" e o motivo de não usar o Java mais novo; para um pack Forge 1.7.10, "Automático: Java 8" com o motivo do Forge antigo; para a versão mais nova do Minecraft, o Java mais novo, sem explicação extra (teste de componente com a política de L-01). |
| CA-T11-04 | (P1) Criar o perfil "PC fraco (4 GB)", escolhê-lo no menu ▾ e testar lança o jogo com `-Xmx4G`; o botão mostra "Testar · PC fraco"; nenhum arquivo do pack muda (hash da pasta antes e depois). |
| CA-T11-05 | (P1) Com "Entrar direto no mundo de teste" num pack 1.20.1, a linha de comando gerada tem `--quickPlaySingleplayer` com a pasta do mundo e o jogo entra no mundo sem passar pelo menu (golden test da linha de comando + roteiro manual); num pack 1.19.2, a opção aparece desabilitada com o motivo. |

---

### T12 — Configs (P0 editor de texto e busca; P1 formulário e padrões)

**Objetivo:** editar as configurações do pack e, durante os testes, as da instância. A descrição da seção passa a ser "Arquivos de ajuste e scripts do pack" (decisão D20); scripts KubeJS e CraftTweaker abrem no mesmo editor, com os recursos de T26.

**Árvore de arquivos à esquerda** com o seletor de origem "Mostrando: arquivos do pack / instância de teste" (caixa de seleção, não abas):
- **Pack:** `config/`, `defaultconfigs/`, `kubejs/`, `scripts/`, `options.txt` e demais arquivos de texto do pack.
- **Instância de teste** (quando existir): `config/`, `defaultconfigs/`, `options.txt` e `saves/<mundo>/serverconfig/`. Edições aqui valem só para o teste e aparecem depois em "O que mudou durante o teste" (T15). Aberta pelo botão **Editar configs do teste** (T13), já nesta origem, com aviso fixo e **Voltar ao teste**.

Busca por nome de arquivo. Arquivos binários (ex.: `servers.dat`) aparecem sem edição na v1.

**Buscar em todas as configs (P0; R5B §2.4):** campo acima da árvore. Procura nas chaves, nos valores, nos comentários e (P1) nos rótulos traduzidos de todos os arquivos de `config/`, `defaultconfigs/`, `kubejs/config/`, `options.txt` e demais arquivos de texto da origem escolhida, sem diferenciar maiúsculas nem acentos. Os resultados substituem a árvore enquanto a busca está ativa, agrupados por arquivo ("14 resultados em 6 arquivos"), com a chave, o valor e o trecho do comentário; clicar abre o arquivo **já na chave**, no modo em que você estava (formulário ou texto). Filtros (caixas de seleção): "Só o que mudou do padrão" (P1) e "Só deste mod". O índice se atualiza sozinho quando um arquivo muda.

**Editor de texto (P0):** realce de sintaxe por extensão (TOML, JSON, JSON5, YAML, `.properties`, `.cfg`, JavaScript, texto), números de linha, buscar e substituir, desfazer. Arquivos acima de 2 MB abrem só para leitura com aviso.

**Salvar (P0):**
- Ctrl+S ou botão. Antes de gravar, mostra as diferenças (pode ser desligado em Configurações).
- Se o arquivo mudou no disco desde que foi aberto: "Este arquivo foi alterado fora do Warden." com **Recarregar** (descarta suas mudanças), **Ver diferenças** e **Sobrescrever**.
- Sair do arquivo com alterações não salvas pede confirmação.
- A gravação preserva tudo o que não foi editado (comentários, ordem, espaços), byte a byte.

**Formulário (P1):** para TOML do Forge/NeoForge, JSON/JSONC, JSON5, `.properties`, `.cfg` do Forge antigo e `options.txt`. Mostra as chaves por seção, com os comentários do arquivo como ajuda, faixas e valores permitidos quando o arquivo informa (`#Range`, `#Allowed Values`, `[range: …]`, `Min`/`Max`, `Valid values`, prefixos `B:`/`I:`/`D:`/`S:` do `.cfg`), e controles por tipo (liga/desliga, número com limite, lista de opções, texto, lista, cor). Tipo, faixa e padrão vêm em camadas (R5B §2.2): metadados explícitos do arquivo, tipo pelo valor, frases do comentário ("Defaults to X", marcadas **deduzido**) e o **rótulo do próprio mod**, lido do arquivo de idioma do jar (`pt_br.json` quando existir, senão `en_us.json`), mostrado com a chave real ao lado. Só os valores alterados são gravados. Alternar entre formulário e texto a qualquer momento (dois modos de ver o mesmo arquivo). Arquivo que não volta idêntico na leitura de teste, estruturas muito profundas e arquivos acima de 2 MB abrem só em texto, com o motivo; um nó complexo pode ser editado como "trecho em texto".

**Padrão e restaurar (P1; R5B §2.5, camada A):** quando o arquivo informa o padrão (`Default:` do NeoForge, `[default: …]` do Forge antigo), cada chave mostra "padrão: X" (com a origem no tooltip) e uma marca quando o valor é diferente, com **Restaurar padrão**; o arquivo tem **Restaurar o arquivo todo para o padrão** no menu ⋯, sempre com a diferença antes de gravar. Chave sem padrão conhecido não mostra nada (nunca um padrão inventado). Valor fora da faixa informada aparece com erro no próprio campo ("12.0 está fora da faixa 0,0 a 10,0. O Forge troca por 1,0 sem avisar.") e também vira problema antes do teste (T14).

**Avisos contextuais:**
- Arquivo `*-server.toml` em `config/` (Forge/NeoForge 1.13+): "Este tipo de config vale por mundo. Para valer em mundos novos, ele precisa estar em `defaultconfigs/`." (P0) com **Copiar para defaultconfigs/** (P1).
- Arquivos do Forge/NeoForge: "O jogo pode reescrever este arquivo e apagar comentários que você adicionar." No NeoForge, acrescenta: "Valores fora da faixa voltam ao padrão e chaves que o mod não conhece são apagadas quando o jogo abre." (R5B §2.3)
- `options.txt` no pack: "Este arquivo substitui as preferências de quem já joga o pack." com a opção **Não substituir se o jogador já tiver** (grava `preserve = true` no índice; P1).

| CA | Critério |
|---|---|
| CA-T12-01 | Abrir e salvar sem mudanças qualquer arquivo do corpus de configs reais produz bytes idênticos (teste de ida e volta). |
| CA-T12-02 | Mudar um valor pelo formulário num TOML do Forge com comentários altera exatamente uma linha. |
| CA-T12-03 | Editar o arquivo por fora com o editor aberto e salvar no Warden mostra o aviso de alteração externa; nada é gravado sem escolha do usuário. |
| CA-T12-04 | Editar um config da instância durante o teste faz o arquivo aparecer em T15 ao fim do teste. |
| CA-T12-05 | Nenhum arquivo `.bak` ou temporário fica na pasta do pack depois de salvar (inclusive se o app for encerrado à força durante a gravação; teste de falha). |
| CA-T12-06 | Buscar "spawn" num corpus de 500 configs reais devolve os resultados agrupados por arquivo em menos de 200 ms com o índice pronto, encontra "Spawn" e "spawnRate" sem diferenciar maiúsculas, e clicar num resultado abre o arquivo na linha da chave (teste de domínio + componente). |
| CA-T12-07 | Editar um arquivo por fora atualiza os resultados da busca em até 2 s, sem reiniciar o app. |
| CA-T12-08 | (P1) Para um TOML do NeoForge com `# Default:`, `# Range:` e `#Allowed Values`, um `.cfg` do Forge 1.12.2 com `[range: …, default: …]` e um JSON do AutoConfig com a chave de tradução no jar, o formulário mostra o controle certo, a faixa, o padrão (quando há) e o rótulo do jar, com a chave real ao lado (teste com o corpus de configs reais e jars de teste). |
| CA-T12-09 | (P1) "Restaurar padrão" numa chave com padrão no comentário altera só aquela linha; o filtro "Só o que mudou do padrão" mostra exatamente as chaves diferentes do padrão; uma chave sem padrão conhecido nunca aparece como alterada. |

---

### T13 — Testar (launcher) e console (P0)

**Objetivo:** abrir o Minecraft com o pack em um clique e acompanhar o que acontece.

**Onde:** o botão **▶ Testar** fica no cabeçalho de todas as telas do pack (T05). Ao clicar, a área principal vira a **tela do teste**, que não é uma seção do menu: dá para ir a outras seções com o jogo aberto e voltar pelo mesmo botão ("● Jogo aberto: ver teste").

**Menu ▾ do Testar** (em grupos com título; ESTRUTURA N7):
- **Ver último teste** (resultado, console e o que mudou).
- **Outros testes:** **Testar como o jogador recebe** (P1); **Testar como servidor…** (P1, T27); **Testar com perfil de desempenho** (P1); **Encontrar o mod culpado…** (P1, T25).
- **Perfil do teste** (P1): os perfis, com o atual marcado, e **Ajustes do teste neste computador…** (T11).
- **Instância de teste:** **Abrir pasta da instância de teste**; **Apagar mundos de teste…** (decisão D6); **Recriar instância de teste…**.

**Fluxo ao clicar em Testar** (indicador de etapas sempre visível):
1. **Verificar o pack (rápido):** roda a passagem rápida do diagnóstico pré-teste (T14), que usa o pack e os dados das APIs. Com erros: diálogo listando os erros, com **Corrigir** (quando houver correção automática), **Testar mesmo assim** e **Cancelar**. Só avisos: segue, com os avisos visíveis na aba Diagnóstico.
2. **Preparar o Minecraft:** baixa a versão do jogo, instala o loader e escolhe/baixa o Java (o mais novo que funciona, T11; ARCHITECTURE §7). Mostra progresso (arquivos e megabytes). Na primeira vez pode levar alguns minutos; o texto avisa isso.
3. **Copiar o pack para o teste** (sincronização): copia/baixa para a instância só o que mudou; remove o que saiu do pack; nunca toca em `saves/`, `screenshots/` e afins. Mods da CurseForge com download bloqueado abrem T20.
4. **Verificação final:** passagem completa do diagnóstico, que lê os próprios arquivos `.jar` (dependências, duplicatas por ID, versão do Java exigida). Erros abrem o mesmo diálogo do passo 1.
5. **Abrir o jogo** com o perfil offline (nome configurado).
6. **Jogo em execução:** console ao vivo; botões **Parar jogo** (confirmação: "O progresso não salvo do mundo pode ser perdido."), **Abrir pasta da instância**, **Editar configs do teste** (abre T12 na origem Instância) e, quando o pack tem KubeJS ou CraftTweaker, **Recarregar scripts** (T26). Acima do console, a **faixa de desempenho** (ver abaixo).
7. **Jogo fechou:** a mesma tela mostra o resultado, com duração ("Fechado normalmente", "Travou" ou "Encerrado por você"), "Abriu em 1 min 42 s" (com a diferença para o último teste de outra versão do pack, quando houver), **Ver console** e **Testar de novo**, e logo abaixo:
   - **O que mudou durante o teste** (T15), quando houver mudanças;
   - se travou: **Por que travou** (diagnóstico pós-crash, T14), com a correção sugerida e, quando a causa não aparecer, **Encontrar o mod culpado** (P1, T25) como ação principal e **✦ Pedir ajuda à IA** (P1);
   - depois de **Testar com perfil de desempenho**: a parte **Desempenho** (ver abaixo).

**Console:**
- Linhas coloridas por nível (erro, aviso, info, debug), com horário e origem (jogo, Warden).
- Filtros por nível, busca, pausar a rolagem automática, copiar seleção, copiar tudo, salvar em arquivo.
- Mostra até 50.000 linhas na tela; tudo fica gravado no log da sessão.
- Sem tradução das mensagens do jogo (são mostradas como saíram).
- **Mostrar: Linha a linha · Agrupado por mod · Só problemas** (P1; R5A §7.1), seletor no topo do console. Agrupado: uma linha por mod (com link para o item do pack) com erros, avisos e repetições; "Jogo e loader" e "Desconhecido" são grupos próprios. Linhas repetidas viram um item só ("×1.284, primeira às 18:02:16, última às 18:04:51") pela forma da linha (números, coordenadas, UUIDs e caminhos trocados por marcadores). Stack trace é um item só, recolhido, com a primeira linha de mod em destaque. Linhas conhecidas que assustam mas são inofensivas ganham o selo "comum, geralmente inofensivo". "Só problemas" mostra só erros, avisos e stack traces. Nada disso altera o log gravado.
- A origem de cada linha vem, nesta ordem: do jar ou módulo no frame do stack trace, do nome do *handler* do mixin (Fabric), do nome do *logger* e do índice pacote → mod (ARCHITECTURE §9.6).

**Faixa de desempenho** (acima do console, em todo teste): **RAM do processo** e **Abriu em** (tempo até o jogo carregar e até entrar no mundo) são P0; **Memória do jogo** (heap da JVM usada/máxima, com mini-gráfico dos últimos minutos) e **Coletas de memória** (quantidade e tempo) são P1 (ADR-0038). Amostra a cada 1 s. Achados automáticos: "O jogo está quase sem memória. Aumente para 8 GB em Ajustes do teste." quando a memória depois das coletas fica acima de 90% do máximo por mais de 1 minuto; "Memória sobrando" (informação) quando nunca passa de 40% com mais de 8 GB configurados. Sem leitura da memória do jogo (argumentos que a desligam, JVM sem esse recurso): a faixa diz o motivo, sem número inventado.

**Testar com perfil de desempenho (P1; R5A §7.2, §7.4):** um teste normal que, além disso, mede o **tempo de carregamento por mod** onde o loader registra (Forge 1.12.2 sempre; Forge e NeoForge modernos com registro detalhado só neste teste; Fabric não tem, e o Warden diz isso) e lê os perfis do spark que você salvar no jogo (`/sparkc profiler stop --save-to-file`). O resultado ganha **Desempenho**: "O que mais demorou para carregar" (com a ressalva de que texturas e mixins não entram na medição), o resumo do perfil do spark (tempo por tick, mods que mais usaram processamento, agrupados pelo jar) e **Abrir no visualizador do spark** (abre o arquivo local no navegador; nada é enviado). No servidor local (T27), o Warden liga e desliga o spark sozinho pelo console do servidor.

**Memória automática:** pack com menos de 100 mods: 4 GB; 100–199: 6 GB; 200 ou mais: 8 GB; nunca acima de 60% da memória do computador; aviso acima de 8 GB em Java 8. Valor manual em Ajustes (T11).

**Regras:**
- Um jogo por vez no Warden inteiro (v1). Clicar em Testar com outro jogo aberto: "Já existe um jogo em execução (pack X)." com **Ir para o teste**.
- Fechar o Warden com o jogo aberto pede confirmação e encerra o jogo.
- Sem internet e com tudo já baixado, o teste funciona. Se faltar algo, a mensagem lista exatamente o que falta.
- Editar o pack com o jogo aberto é permitido; o cabeçalho avisa "O pack mudou desde o início do teste; as mudanças valem no próximo teste."
- Os mundos de teste ficam na instância e nunca vão para o pack.

**Testar como o jogador recebe (P1; R5B §4.3):** instala o pack do zero, como um jogador, numa instância temporária limpa (sem mundos e sem configs da instância de trabalho), usando o **packwiz-installer-bootstrap real** com o **link do pack**: o bootstrap e o packwiz-installer ficam no cache do Warden com versão e hash fixados, e o teste roda sempre com `--bootstrap-no-update`, para não depender da API do GitHub. Pack ainda não publicado: o Warden serve o pack exportado agora só neste computador (`127.0.0.1`) e avisa que esse é o caminho. Depois de baixar, confere se o `pack.toml` recebido é o da última versão publicada; se não for: "O GitHub ainda está entregando a versão anterior. Tente de novo em alguns minutos." Mods da CurseForge bloqueados para apps de terceiros falham aqui como falhariam para o jogador, e isso vira resultado ("3 mods não baixam sozinhos para os jogadores"). As ferramentas do jogador entram, inclusive o Crash Assistant (decisão D16). Ao final, a instância temporária é apagada e os logs ficam.

**Ferramentas do jogador nos testes normais (decisão D16):** o Crash Assistant não é copiado para a instância de teste nos testes normais nem na busca do culpado; o spark é copiado nos testes normais e desligado na busca do culpado, a menos que esteja entre os suspeitos.

| CA | Critério |
|---|---|
| CA-T13-01 | Para cada combinação da matriz de versões (ROADMAP L-05, que cobre de 1.7.10 à versão mais nova com Forge, NeoForge, Fabric e vanilla), um pack mínimo chega ao menu principal e cria um mundo (teste automatizado em Linux com servidor gráfico virtual + roteiro manual no Windows). |
| CA-T13-02 | "Parar jogo" encerra o processo do jogo e todos os processos filhos em até 5 s. |
| CA-T13-03 | Linhas com acentos no log aparecem corretas no console (sem `Ã©`), em Java 8 e Java 21 no Windows. |
| CA-T13-04 | Remover um mod do pack e testar de novo remove o jar da instância; um mundo criado no teste anterior continua lá. |
| CA-T13-05 | Desligar a internet depois de um teste bem-sucedido e testar de novo funciona. |
| CA-T13-06 | Um crash forçado (pack de teste quebrado) termina com resultado "Travou" e a área "Por que travou" no próprio resultado. |
| CA-T13-07 | Fechar o Warden durante o jogo, após confirmar, não deixa processo `java`/`javaw` órfão. |
| CA-T13-08 | Durante a preparação o botão do cabeçalho mostra "Testando… ver progresso" e, com o jogo aberto, "● Jogo aberto: ver teste"; a partir de qualquer seção do pack, um clique nele leva à tela do teste; "Ver último teste" no menu ▾ abre o resultado da última sessão (teste de componente). |
| CA-T13-09 | Toda sessão grava o pico de RAM do processo e os tempos até carregar e até entrar no mundo (quando entrar); o resultado mostra "Abriu em …" e a diferença para a última sessão de outra versão do pack (teste com jogo simulado que imprime os marcadores). |
| CA-T13-10 | (P1) No Windows, com Java 8, 17, 21 e 25, a memória do jogo lida pelo Warden fica a até 5% do valor do `jstat` de um JDK no mesmo instante (teste de integração na CI Windows, a partir do spike S-R5-2); com `-XX:+PerfDisableSharedMem` nos argumentos, a faixa mostra o motivo e Ajustes do teste oferece remover o argumento. |
| CA-T13-11 | (P1) Um processo de teste que mantém a heap acima de 90% depois das coletas por 70 s gera o aviso de memória quase cheia com o botão para Ajustes do teste; abaixo de 1 minuto, não gera. |
| CA-T13-12 | (P1) No modo Agrupado por mod, um log de teste com 1.284 repetições de `Missing texture for {id}` vira um item "×1.284" atribuído ao mod certo pelo índice de pacotes; um stack trace vira um item recolhido com a primeira linha de mod em destaque; "Só problemas" esconde as linhas de informação (teste de domínio com logs reais e de componente). |
| CA-T13-13 | (P1) "Testar com perfil de desempenho" num pack Forge 1.12.2 mostra o tempo por mod lido das linhas `Bar Step` do `debug.log`, e num Forge 1.20.1 com o registro detalhado ligado só nesse teste; um `.sparkprofile` salvo na instância vira o resumo com os mods que mais usaram processamento, e o arquivo nunca entra em "O que mudou durante o teste" (golden tests com arquivos reais). |
| CA-T13-14 | (P1) "Testar como o jogador recebe" com um pack publicado num servidor local que imita o `raw.githubusercontent.com` instala com o bootstrap real e `--bootstrap-no-update`, sem nenhuma chamada à API do GitHub (servidor simulado contando chamadas); com o `pack.toml` antigo sendo servido, mostra o aviso de versão anterior; um mod da CurseForge bloqueado aparece no resultado como "não baixa sozinho para os jogadores"; o Crash Assistant está na instância temporária. |
| CA-T13-15 | Num teste normal de um pack com o Crash Assistant marcado como ferramenta do jogador, o jar dele não está na instância de teste e o spark está; o `index.toml` do pack continua com os dois (teste de integração da materialização). |

---

### T14 — Diagnóstico (P0 determinístico; P1 IA e saúde do pack)

**Objetivo:** encontrar problemas antes de abrir o jogo, explicar por que ele travou e ajudar a achar a causa quando as regras não bastam.

**Camada determinística (P0)** — regras fixas, sem IA, sempre com a evidência que as sustenta:
- **Antes do teste**, em duas passagens — rápida (antes de baixar, com dados do pack e das APIs) e completa (depois de copiar o pack para o teste, lendo os `.jar`) — e também sob demanda pelo botão "Verificar agora" da seção Problemas: loader errado, versão do Minecraft incompatível, versão do loader fora da faixa exigida, dependência obrigatória ausente ou em versão errada, incompatibilidades declaradas, mod duplicado (mesmo ID, mesmo arquivo ou mesmo projeto de duas fontes), dois mods da mesma "categoria exclusiva" (ex.: dois renderizadores), mod só de cliente marcado como "cliente e servidor", Java incompatível, versões beta/alpha, conflitos conhecidos da lista curada, mods obsoletos; P1: valor de config fora da faixa que o próprio arquivo informa ("`maxChunks = 900` está fora da faixa 1–512 e o jogo vai trocar por 64 sem avisar") e **dois mods que alteram o mesmo ponto do jogo** (raio-x de mixins, T07: só aviso, nunca bloqueia o teste). Lista de regras em ARCHITECTURE §9.
- **Depois de um travamento:** lê o resultado do processo, a saída capturada, `logs/latest.log`, `logs/debug.log`, os crash reports novos e os `hs_err_pid*.log` novos, e aplica o catálogo de padrões (dependência faltando, duplicado, falha de Mixin com o mod dono da config de mixin citada, Java errado, falta de memória, driver de vídeo, config corrompida, config resetada pelo NeoForge, mod de cliente no servidor, entre outros). Cada travamento ganha uma **assinatura** (a causa normalizada, sem números de linha nem endereços), usada para agrupar repetições e pela busca do culpado (T25).

**Seção Problemas** (descrição "Saúde do pack, problemas e travamentos", decisão D20):
1. **Saúde do pack** (P1; R5A §6.2; ADR-0034), no topo: nota de 0 a 100, faixa (90–100 Ótimo, 75–89 Bom, 50–74 Atenção, abaixo de 50 Crítico) e **O que tirou pontos**, uma linha por categoria com link para o problema, o mod ou o teste: problemas do pré-teste (−15 por erro, −3 por aviso; teto −40), último teste (travou −20, não testado depois da última alteração −5, nunca testado −10; teto −25), travamentos recentes (−3 por causa diferente nas últimas 10 sessões; teto −10), mixins (−4 por sobreposição alta não rebaixada, −1 por média; teto −10; achados de mixin contam só aqui), manutenção (mod sem versão nova há mais de 18 meses para a versão do pack, projeto arquivado, versão alpha ou beta; teto −10), distribuição (−2 por mod da CurseForge que os jogadores não baixam sozinhos; teto −10), desempenho (tempo de carregamento 50% acima do último teste bom −3; memória abaixo da sugestão −2; teto −5). Texto fixo: "Resumo dos problemas conhecidos. Não garante que o pack funciona." Itens ignorados não tiram pontos e aparecem riscados ("ignorado por você"). A mesma nota aparece em Meus packs (T02; decisão D24).
2. **Achados do pré-teste** agrupados por gravidade: **Erro** (bloqueia o teste por padrão), **Aviso**, **Informação**, com contador no menu. Itens com problema também ganham a marca na lista de Mods (T06).
3. **Travamentos** (P0; R5A §7.5): uma linha por assinatura, com a causa em linguagem simples, quantas vezes travou, quando foi a última, em que versões do pack ("1.4.1 a 1.4.2"), a situação ("Causa encontrada", "Causa não encontrada", "Não voltou a acontecer desde a 1.4.4") e as ações **Ver o que causou**, **Encontrar o mod culpado** (P1, T25) e **✦ Conversar com a IA** (P1). Abrir uma linha mostra as sessões daquela causa, a busca do culpado e as conversas ligadas a ela. Sessões que fecharam normalmente são podadas (as últimas 30 ficam); sessões que travaram ficam até 500 MB por pack, com aviso antes de apagar.

**Cada achado:** título em linguagem simples, explicação, itens envolvidos (com link para o item), evidência ("O arquivo fabric.mod.json do Sodium declara incompatibilidade com o OptiFine" ou o trecho do log com número da linha) e, quando houver, botões de correção (**Adicionar dependência X**, **Remover Y**, **Mudar lado para Só cliente**, **Atualizar Z**, **Restaurar padrão**). Correções passam pelos fluxos normais, com confirmação. "Ignorar este aviso neste pack" (P1; gravado em `.warden/`). Os achados de um travamento aparecem no resultado do teste ("Por que travou", T13). Sem conclusão: "Não encontramos a causa automaticamente." com **Encontrar o mod culpado** (P1), **Pedir ajuda à IA** (P1) e **Abrir crash report**.

**✦ Diagnóstico com IA (P1; Gemini com ferramentas; ADR-0030, decisão D17):**
- **Onde:** seção própria do pack, **✦ Diagnóstico com IA** (descrição "Conversar com a IA sobre um problema do pack"), e os botões **✦ Pedir ajuda à IA** (resultado de um travamento) e **✦ Conversar com a IA** (Problemas → Travamentos), que abrem uma conversa nova já com aquele travamento. O ícone de brilhinho marca tudo o que é IA, e só isso.
- **Na seção:** **Nova conversa**: do que se trata (Trava ao abrir o jogo / Trava ao entrar no mundo / Está lento / Outra coisa), o que analisar (o **último travamento**, padrão; **outro teste**; **um log ou crash report do computador**; ou **só o pack**, sem log), campo opcional "Contar algo à IA" e **✦ Começar conversa**. Abaixo, **Conversas**: título (a causa provável, quando houver), data, o que foi analisado, situação ("Resolvido: Rubidium removido", "Aberta"), **Continuar** e **Apagar**. Situação da chave do Gemini e o modelo, com link para Configurações. As conversas ficam nos dados do Warden, por pack, nunca na pasta do pack.

1. **Consentimento, uma vez por conversa** (não existe "não perguntar de novo" entre conversas): o diálogo lista **o que a IA poderá consultar** durante a conversa: informações do pack e lista de mods; achados do Warden; logs e crash reports, sempre sem dados pessoais (nome de usuário do Windows nos caminhos, nome do jogador, UUID, IPs, e-mails, nome do computador, qualquer coisa com cara de chave ou token); **configs do pack** (caixa marcada, desmarcável: "Não deixar a IA ler configs"); **procurar problemas parecidos nas páginas dos mods no GitHub** (caixa marcada, desmarcável), enviando ao GitHub só o nome do mod e palavras do erro, nunca o log. Mostra o **texto inicial exato**, já redigido, rolável; o tamanho; o provedor (Google Gemini) e o modelo; o custo estimado de uma conversa típica; que o uso é cobrado ou limitado na sua chave; e que, no plano gratuito, o Google pode usar o conteúdo enviado e pessoas podem lê-lo. Texto fixo: "Cada coisa que a IA consultar aparece na conversa, exatamente como foi enviada. Nada muda no pack sem você clicar em Aplicar." Botões **✦ Começar conversa** e **Cancelar**.
2. **A conversa** (página de detalhe com "← Conversas"): mensagens suas e da IA; cada consulta da IA aparece como um bloco recolhido **Enviado à IA: trecho do latest.log, linhas 1.200–1.260** (ou "Enviado ao GitHub: busca de issues no Supplementaries com 'NullPointerException travel'"), que, aberto, mostra byte a byte o que saiu e o que voltou. A resposta traz as **afirmações com evidência**: cada evidência é um selo clicável que leva ao trecho (log, crash report, metadado do jar, config, issue, changelog); afirmação cuja evidência não confere aparece marcada **não verificado**. Medidor de confiança (baixa, média, alta). Aviso fixo: "A IA pode errar. Confira antes de mudar o pack." "Não sei" é resposta válida: quando as evidências não bastam, a IA diz isso e sugere **Encontrar o mod culpado**. Contador de tamanho ("Esta conversa: ~182 mil tokens"). Campo para continuar a conversa.
3. **Propostas:** qualquer mudança sugerida pela IA (adicionar, remover ou atualizar um item, trocar de versão, mudar o lado, editar uma chave de config, mudar memória ou Java do teste, testar de novo, começar a busca do culpado) aparece como um **cartão de proposta** com a diferença exata e a evidência que a justifica, com **Aplicar** e **Descartar**. Aplicar passa pelos fluxos normais (dependências, diferença antes de salvar) e cria ponto de segurança quando muda o pack. Proposta sem evidência é recusada pelo Warden e nem aparece.
4. **Continuar uma conversa antiga:** se o pack mudou desde então, o Warden avisa ("3 mods foram atualizados desde esta conversa") e oferece **Começar nova conversa**.
5. Sem chave do Gemini: "Para usar a IA, informe sua chave do Gemini em Configurações." com **Abrir Configurações**. Cota esgotada, tempo esgotado ou resposta inválida: a mensagem diz o que aconteceu, nada muda no pack e a conversa continua guardada.

| CA | Critério |
|---|---|
| CA-T14-01 | Cada pack de teste propositalmente quebrado (dependência faltando, jar Fabric no Forge, mod duplicado, mod que exige Java 21 em pack de Java 17, OptiFine com Sodium) gera o achado esperado antes do teste, com evidência. |
| CA-T14-02 | Cada log real do corpus de crashes (`crates/warden-diagnostics/tests/corpus/`) gera o diagnóstico esperado, com a linha de evidência correta. |
| CA-T14-03 | O texto inicial mostrado no consentimento e o conteúdo de cada bloco "Enviado à IA" são byte a byte o que foi enviado ao Gemini, e cada bloco "Enviado ao GitHub" é byte a byte a consulta enviada ao GitHub (teste com servidores simulados). |
| CA-T14-04 | Num log contendo `C:\Users\Maria\AppData`, o nome do jogador, um IPv4 e uma string `$2a$10$...`, nenhum desses valores aparece no texto enviado. |
| CA-T14-05 | Cancelar o consentimento não faz nenhuma requisição de rede. |
| CA-T14-06 | Uma conversa sobre um log escolhido no computador passa pela mesma redação e pelo mesmo consentimento; a conversa aparece em "Conversas" depois de reiniciar o app, e nenhum arquivo é criado ou alterado na pasta do pack (teste com servidor simulado e hash da pasta antes/depois). |
| CA-T14-07 | (P1) A nota de saúde de um pack de teste com 2 erros, 1 aviso, o último teste travado, 2 causas de travamento diferentes nas últimas 10 sessões, 1 sobreposição alta de mixin e 2 mods da CurseForge bloqueados é 33 (Crítico), com as linhas de "O que tirou pontos" na ordem da tabela; acrescentar um problema nunca aumenta a nota (teste de propriedade); ignorar um aviso devolve os pontos dele. |
| CA-T14-08 | Dois travamentos com a mesma causa e números diferentes (linha do log, endereço de memória, coordenadas) geram a mesma assinatura e aparecem como uma linha "Travou 2 vezes"; causas diferentes geram linhas diferentes; as versões do pack de cada sessão aparecem na linha (teste de domínio com logs reais). |
| CA-T14-09 | (P1) Um pack com dois mods que fazem `@Redirect` no mesmo ponto gera o aviso "alteram o mesmo ponto do jogo" com a evidência (configs de mixin e ponto); o aviso nunca bloqueia o Testar; um travamento cuja pilha cita a config de mixin de um deles sobe o risco para alto e cita a linha do log. |
| CA-T14-10 | (P1) Um TOML do NeoForge com `maxChunks = 900` e `#Range: 1 ~ 512` gera o aviso de faixa antes do teste com **Restaurar padrão**; a linha `Incorrect key … was corrected from … to its default` no log vira o achado "o jogo trocou um valor da sua config pelo padrão". |
| CA-T14-11 | (P1) Com um servidor do Gemini simulado que responde uma afirmação citando um trecho que não existe no resultado da ferramenta, a afirmação aparece marcada "não verificado"; um mod citado que não existe no pack nem na API não vira link nem proposta; uma proposta sem evidência não aparece. |
| CA-T14-12 | (P1) Uma proposta "editar `config/x.toml`, chave `a`, para 2" não altera nada até o clique em **Aplicar**; Aplicar mostra a diferença, grava só aquela linha e cria ponto de segurança; **Descartar** não grava nada (teste com servidor simulado e hash da pasta). |
| CA-T14-13 | (P1) Com a busca no GitHub permitida, a consulta enviada ao GitHub contém só `repo:<dono>/<repo>` e as palavras do erro (nenhuma linha de log, caminho ou nome do jogador); com a caixa desmarcada, nenhuma requisição vai ao GitHub; com a caixa de configs desmarcada, a ferramenta de configs devolve "não permitido nesta conversa" e nada de config é enviado (servidores simulados). |
| CA-T14-14 | (P1) O laço de ferramentas para em 8 rodadas por pergunta e pede a resposta final sem ferramentas; as *thought signatures* das respostas do modelo são reenviadas intactas (teste com servidor simulado que recusa histórico alterado, como a API real). |
| CA-T14-15 | (P1) Continuar uma conversa depois de atualizar 3 mods mostra o aviso de que o pack mudou e a opção de começar outra; apagar uma conversa remove o arquivo dela dos dados do Warden. |

---

### T15 — O que mudou durante o teste (P0)

**Objetivo:** trazer para o pack o que você ajustou durante o jogo, sem trazer lixo.

**Onde:** no próprio resultado do teste (T13), na área **O que mudou durante o teste**, não numa tela separada.

**Como funciona:** antes de abrir o jogo, o Warden tira uma "foto" (linha de base) da instância. Depois que o jogo fecha, compara e mostra só o que mudou, separando mudança real de simples reformatação (o jogo costuma reescrever arquivos sem mudar valores).

**Grupos:**

| Grupo | Exemplos | Pré-seleção |
|---|---|---|
| Configs alterados que vieram do pack | `config/sodium-options.json` | marcado |
| `options.txt` (por chave) | `renderDistance`, teclas `key_*`, `lang`, `guiScale` | marcadas as chaves não voláteis; chaves voláteis (`version`, `lastServer`, `fullscreen`, `tutorialStep` etc.) nunca aparecem |
| Arquivos novos gerados pelo jogo/mods | configs criados na primeira execução | desmarcado, grupo recolhido |
| Configs de mundo (`saves/<mundo>/serverconfig/`) | `create-server.toml` | desmarcado; destino proposto: `defaultconfigs/` |
| Mods colocados/removidos manualmente na pasta da instância | um `.jar` novo em `mods/` | nunca copiado; o Warden identifica pelo hash e oferece **Adicionar ao pack** pelo fluxo normal (T08) |
| Resource packs e shaders novos | `resourcepacks/x.zip` | desmarcado; oferece adicionar pelo fluxo normal |
| Datapacks em mundos | `saves/<mundo>/datapacks/` | só informativo na v1 |

Sempre ignorados: `logs/`, `crash-reports/`, `saves/` (exceto serverconfig acima), `screenshots/`, caches de mods e do loader, dados de minimapa, `usercache.json`, os perfis do spark (`config/spark/*.sparkprofile`, `*.sparkheap`, `tmp*`), o token do servidor web do KubeJS (`kubejs/config/web_server.json`), as tipagens e pastas de editor criadas pelo ProbeJS (`.probe/`, `.vscode/`, `local/kubejs/`) e a lista completa em ARCHITECTURE §8.4 (decisão D21). Scripts KubeJS e CraftTweaker alterados durante o teste aparecem como configs (T26).

**Cada arquivo:** diferença por valor (chave: antes → depois) e **Ver diferença de texto**.

**Conflito:** se o mesmo arquivo também mudou no pack desde o início do teste, mostra as três versões (início do teste, pack agora, instância agora) com **Manter a do pack**, **Usar a da instância** e **Editar e combinar**.

**Ações:** **Trazer selecionados para o pack** (grava, atualiza o índice e confirma onde as mudanças foram parar: "aparecem em Alterações não salvas", com **Salvar versão** e **Continuar editando**); **Descartar** (no próximo teste a instância volta a ter os arquivos do pack); **Decidir depois** (o cabeçalho mostra "Mudanças do teste para revisar"). Testar de novo com mudanças não revisadas pergunta antes: "Revisar agora", "Descartar e testar", "Cancelar".

| CA | Critério |
|---|---|
| CA-T15-01 | Mudar a distância de renderização no jogo faz aparecer só `renderDistance` no grupo options.txt; aceitar grava só essa chave no `options.txt` do pack (diff de uma linha). |
| CA-T15-02 | Um config que o Forge apenas reformatou (mesmos valores) não aparece na lista. |
| CA-T15-03 | Nenhum arquivo da lista "sempre ignorados" aparece, mesmo que tenha mudado. |
| CA-T15-04 | Um jar colocado manualmente em `mods/` da instância nunca é copiado para o pack; a ação oferecida é adicionar como referência quando o hash é reconhecido. |
| CA-T15-05 | O conflito de três vias é mostrado quando o arquivo foi alterado no pack (pelo editor) e na instância (no jogo) durante o mesmo teste. |
| CA-T15-06 | Arquivos novos ou alterados na instância em `config/spark/` (`*.sparkprofile`, `*.sparkheap`, `tmp*`), `kubejs/config/web_server.json`, `.probe/`, `.vscode/` e `local/kubejs/` nunca aparecem em "O que mudou durante o teste" (teste com instância sintética). |

---

### T16 — Salvar versão (P0)

**Onde:** botão **Salvar versão · N alterações** no cabeçalho do pack (T05) e em Histórico (T17). Salvar é sempre **local**: nada vai para o GitHub (para isso, T18).

**Diálogo:**
- **Versão sugerida** com o motivo, editável (validação SemVer; precisa ser maior que a última versão salva):
  - MAIOR: mudou a versão do Minecraft ou o loader; removeu mod que não é "só cliente" (pode apagar blocos/itens de mundos existentes); adicionou ou removeu mod de geração de mundo (categoria `worldgen` no Modrinth);
  - MENOR: adicionou mods, resource packs ou shaders;
  - CORREÇÃO: só atualizações de itens e mudanças de configs.
  - Primeira versão salva: sugere a versão atual do `pack.toml` (`0.1.0` em packs novos).
- **Changelog automático** (pré-visualização): Adicionados, Removidos, Atualizados (`versão antiga → versão nova`), Resource packs e shaders, Configs alteradas (lista de arquivos), Mudança de Minecraft/loader.
- **Notas** (texto livre, opcional) que entram no topo do changelog.
- **Marcar como versão final** (caixa, desmarcada por padrão): a versão fica pronta para os jogadores e pode ser publicada (T18). Também dá para marcar ou desmarcar depois, no Histórico, enquanto a versão não foi publicada.
- Avisos: "Você removeu mods que podem ter conteúdo nos mundos. Avise quem joga para fazer backup." quando aplicável; isso também entra no changelog numa seção "Atenção".
- Checklist (P1): diagnóstico sem erros; último teste depois da última mudança abriu normalmente; mudanças do teste revisadas.

**Ao salvar:** grava a versão no `pack.toml`, acrescenta a entrada no topo do `CHANGELOG.md`, atualiza o índice, registra no histórico com a versão e o changelog. Sem mudanças desde a última versão: "Nada mudou desde a versão X." e o botão fica desabilitado.

**Depois de salvar:** "Versão X salva" (ou "salva como versão final"), com **Publicar versão X** (só versão final; T18), **Exportar arquivo** (T19) e **Fechar**.

| CA | Critério |
|---|---|
| CA-T16-01 | Adicionar 2 mods e atualizar 1 sugere MENOR e o changelog lista exatamente os 2 adicionados e o atualizado com versões legíveis. |
| CA-T16-02 | Remover um mod "cliente e servidor" sugere MAIOR e inclui a seção "Atenção". |
| CA-T16-03 | Após salvar, o histórico mostra a versão, o `pack.toml` tem a nova versão e `git status` (verificado em teste) está limpo. |
| CA-T16-04 | Tentar salvar `1.0.0` quando a última versão é `1.2.0` é recusado com explicação. |
| CA-T16-05 | Salvar com "Marcar como versão final" deixa a versão como "Versão final · não publicada" no Histórico e oferece "Publicar versão"; nada é enviado ao GitHub (verificado com servidor git local); desmarcar não altera nenhum arquivo do pack. |

---

### T17 — Histórico (P0)

**Onde:** seção **Histórico** do pack (T05), cuja descrição na interface é "Versões salvas e publicação".

**Conteúdo:**
- No topo, **Alterações não salvas**: lista de itens adicionados/removidos/atualizados e arquivos alterados desde a última versão. P1: **Descartar** por arquivo (com confirmação).
- **Publicação para os jogadores** (T18): repositório, visibilidade, última versão publicada, link do pack com **Copiar link** e **Como os jogadores instalam**. Antes da primeira publicação, uma frase explica como os jogadores recebem o pack.
- Linha do tempo das versões salvas: número, data, resumo do changelog e **estado**: "Só salva" (com **Marcar como versão final**), "Versão final · não publicada" (com **Publicar versão** e **Desmarcar**) ou "Publicada" (com a data). Abrir mostra o changelog completo e **Ver diferenças para o estado atual**.
- **Voltar para esta versão:** confirmação explicando que o estado atual fica guardado num ponto de segurança; o pack passa a ficar igual à versão escolhida (arquivos que não existiam nela são removidos); o histórico não é apagado. Depois disso, "Salvar versão" cria uma versão nova a partir dali.
- **Pontos de segurança** (P1): link discreto no fim da seção; lista com data e motivo ("antes de voltar para 1.2.0") e **Recuperar**.

| CA | Critério |
|---|---|
| CA-T17-01 | Voltar para uma versão restaura exatamente a árvore de arquivos dela (comparação de hashes), inclusive removendo arquivos criados depois. |
| CA-T17-02 | Falha no meio da restauração (simulada) deixa o pack no estado anterior completo. |
| CA-T17-03 | Depois de voltar, o estado anterior pode ser recuperado (P0: pelo ponto de segurança registrado no repositório; P1: pela tela). |

---

### T18 — Publicar versão no GitHub (P0)

**Objetivo:** entregar uma versão final aos jogadores. O GitHub do pack distribui o pack: quem joga aponta o packwiz-installer-bootstrap para o **link do pack** e recebe as atualizações ao abrir o jogo (decisão do dono, ADR-0028).

**Onde:** **Publicar versão** em cada versão final do Histórico (T17) e no aviso depois de salvar uma versão final (T16). Só versões finais podem ser publicadas; os jogadores recebem só o que foi publicado.

**O que vai para o GitHub:** só o conteúdo do pack daquela versão: `pack.toml`, `index.toml` e os arquivos do índice (o mesmo conjunto da exportação, T19), mais `CHANGELOG.md` (notas de todas as versões publicadas, a mais nova no topo) e `.gitattributes` com `* -text` (o git nunca muda finais de linha, e os hashes não quebram). Nunca vão: `.warden/`, o histórico de trabalho, versões não publicadas, mundos de teste e registros. Cada publicação vira a ponta da branch `main` do repositório, uma tag `vX.Y.Z` e uma GitHub Release com as notas.

**Antes de publicar** (diálogo "Publicar a versão X para os jogadores"):
- **Avisos** (não impedem; com avisos, o botão final é **Publicar mesmo assim**):
  - mods da CurseForge com distribuição bloqueada: "N mods da CurseForge não podem ser baixados automaticamente pelos jogadores." com a lista; quando o mesmo arquivo existe no Modrinth, **Trocar pelo Modrinth** (muda o pack; depois é preciso salvar e publicar uma versão nova);
  - versão não testada: "A versão X não foi testada." quando nenhum teste que fechou normalmente usou exatamente o conteúdo da versão, com **Testar agora**;
  - problemas pendentes: "O pack tem N erros." com **Ver problemas**.
- **Bloqueios:** algo com cara de chave ou token no conteúdo a publicar ("Encontramos algo que parece uma chave em config/x.toml, linha N. Remova antes de publicar."); arquivos que a verificação de higiene reprova (os mesmos de T04 e T19), com **Limpar**.
- **Notas da versão**, geradas comparando com a **última versão publicada** (não com a última salva): Atenção (remoção de mod que não é só de cliente ou de mod de geração de mundo, como em T16), Adicionados, Removidos, Atualizados (`versão antiga → versão nova`), Resource packs e shaders, Configs alteradas, Mudança de Minecraft/loader; mais "Notas para os jogadores" (opcional). Vão para o `CHANGELOG.md` publicado e para a Release.
- Lista curta do que vai e do que não vai para o GitHub.

**Primeira publicação do pack:** "Primeira publicação deste pack", com o nome do repositório (padrão: nome do pack em minúsculas com hífens) e a visibilidade:
- **Público** (recomendado): "os jogadores recebem atualizações automáticas pelo link do pack. Qualquer pessoa com o link vê os arquivos do pack (mods e configs); nada do seu computador vai junto.";
- **Privado**: "só backup no GitHub. O link não funciona para os jogadores: eles não recebem atualizações automáticas."

Também há **Usar um repositório que já existe** (informar `dono/nome`; o Warden mostra a visibilidade dele). Usa o token do GitHub de Configurações (T21).

**Depois de publicar:** "Versão X publicada", com:
- **Link do pack** (`https://raw.githubusercontent.com/<dono>/<repo>/main/pack.toml`) e **Copiar link**;
- aviso de que o GitHub pode levar alguns minutos para mostrar a versão nova;
- **Como os jogadores instalam (Prism Launcher ou MultiMC)**, com **Copiar instruções**: (1) criar uma instância com a versão do Minecraft e o loader do pack; (2) baixar o `packwiz-installer-bootstrap.jar` e colocar na pasta `.minecraft` da instância; (3) em Editar instância → Configurações → Comandos personalizados, colar no comando antes de iniciar `"$INST_JAVA" -jar packwiz-installer-bootstrap.jar <link do pack>`; (4) abrir o jogo: o pack é baixado agora e atualizado sozinho nas próximas vezes. Linha de ajuda (R5B §11): "Se aparecer um erro 403, o GitHub limitou os downloads por um tempo. Abra o jogo de novo mais tarde ou baixe também o `packwiz-installer.jar` e coloque ao lado do bootstrap.";
- **Copiar texto das notas da versão** e **Abrir a Release no GitHub**.

Com repositório privado, o Histórico mostra "Só backup: o link não funciona para os jogadores" no lugar do passo a passo.

**Erros:** sem internet; token inválido ou expirado ("O GitHub recusou o token. Gere um novo e salve em Configurações."); token sem permissão para criar repositório, enviar ou criar Release (a mensagem diz qual permissão falta); repositório inexistente; o GitHub tem mudanças que não estão aqui (a `main` do repositório não é a última publicação do Warden): bloqueia com explicação e oferece **Substituir o GitHub pelo que está aqui** (dupla confirmação, digitando o nome do pack) ou **Cancelar**; envio feito mas Release não criada: a versão fica "publicada, Release pendente" e **Tentar de novo** cria só a Release. Trazer mudanças do GitHub (pull) é P2.

| CA | Critério |
|---|---|
| CA-T18-01 | Com um token válido e uma conta de teste, publicar a primeira versão final cria um repositório público cuja `main` contém exatamente {`pack.toml`, `index.toml`} ∪ {arquivos do índice} ∪ {`CHANGELOG.md`, `.gitattributes`}, com a tag `vX.Y.Z` e a Release com as notas (teste de integração sob demanda). |
| CA-T18-02 | O token nunca aparece em registros, mensagens de erro, URLs gravadas no `.git/config` ou no frontend (teste de varredura). |
| CA-T18-03 | O packwiz-installer-bootstrap real, apontado para o `pack.toml` publicado (servido por um servidor local que imita o `raw.githubusercontent.com` a partir do repositório remoto de teste), instala o pack numa pasta vazia; depois de publicar a versão seguinte, rodar de novo atualiza a pasta para ela (teste de conformidade). |
| CA-T18-04 | Salvar versões, marcar como final e testar não muda nada no repositório remoto; só "Publicar versão" muda, e a `main` remota nunca contém `.warden/`, commits de trabalho ou versões não publicadas (teste com servidor git local). |
| CA-T18-05 | As notas de uma publicação comparam com a última versão publicada: publicar 1.2.0 depois de 1.0.0, com 1.1.0 salva e não publicada no meio, lista as mudanças de 1.0.0 → 1.2.0, com a seção "Atenção" quando um mod que não é só de cliente foi removido. |
| CA-T18-06 | Antes de publicar: um mod da CurseForge com distribuição bloqueada gera o aviso, com a troca pelo Modrinth quando o mesmo arquivo existe lá; uma versão sem teste que fechou normalmente com o mesmo conteúdo gera "não foi testada"; um config contendo um token no formato `ghp_` bloqueia a publicação apontando arquivo e linha. |
| CA-T18-07 | Os arquivos publicados mantêm os bytes do pack: o hash de cada arquivo lido do remoto confere com o `index.toml`, inclusive para um config com finais de linha CRLF (`.gitattributes` com `* -text`). |

---

### T19 — Exportar (P0)

**Objetivo:** gerar o pack como arquivo ou pasta, só com o necessário. Para entregar aos jogadores pelo GitHub, o caminho é Publicar versão (T18), que usa o mesmo filtro.

**Onde:** seção **Exportar** do pack (T05): uma página de cima para baixo (antes de exportar → formato → o que vai → destino).

**Formatos:** **Pasta packwiz** (para hospedar em outro lugar) ou **Arquivo .zip do pack packwiz** (P0). Também na v1 (P1, decisão D1): **`.mrpack`** (app do Modrinth e launchers compatíveis) e **.zip da CurseForge** (app da CurseForge), gerados pelo packwiz (`modrinth export` / `curseforge export`) sobre uma cópia limpa do pack e validados depois de gerados; e **Pacote para servidor (.zip)** (P1, decisão D22; abaixo).

**Formatos de outros launchers (P1):**
- Antes de gerar, o Warden mostra em linguagem simples o que se perde nesse formato (ex.: a CurseForge não guarda o lado cliente/servidor; o `.mrpack` não guarda descrição de opcionais) — ver R3 §2.3.
- `.mrpack` com mod da CurseForge: se o mesmo arquivo existir no Modrinth (mesmo hash), o Warden oferece trocar a fonte; se não existir e o mod bloquear distribuição por terceiros, a exportação para e explica o motivo; se não bloquear, avisa que o arquivo será embutido e pede confirmação (licença).
- Zip da CurseForge com mod que não é da CurseForge: aviso de que a CurseForge exige aprovação manual de arquivos de fora; pede confirmação.
- `version` do `pack.toml` é obrigatória (o `.mrpack` exige); se faltar, o Warden pede antes de exportar.

**Pacote para servidor (P1; R5B §5.2; ADR-0035):** para quem vai rodar o pack num servidor. Seção "Como os mods chegam ao servidor", com duas escolhas:
- **Baixar os mods pelo link do pack** (padrão, recomendado): o zip leva os scripts `start.bat` e `start.sh`, que rodam o packwiz-installer-bootstrap com `-s server` e `--bootstrap-no-update` (com o installer junto) antes de iniciar o servidor; o servidor se atualiza a cada versão publicada. Exige o pack publicado (T18); sem publicação, a opção aparece desabilitada com o motivo e **Publicar versão**.
- **Mods dentro do zip**: o Warden baixa os jars e os coloca em `mods/`. Funciona sem internet, mas o zip fica grande; mods da CurseForge bloqueados para terceiros exigem download manual (T20) e o Warden pede confirmação para embutir arquivos de terceiros (licença).

As duas levam só o que o servidor usa: mods cujo lado não é "Só cliente" ("8 mods só de cliente ficam de fora: …"), `config/`, `defaultconfigs/`, `kubejs/` sem `client_scripts/`, `scripts/`, o que estiver em `server-overrides/` e `user_jvm_args.txt` com a memória recomendada; nunca `options.txt`, `servers.dat`, resource packs nem shaders. Os scripts instalam o loader na primeira execução e pedem o aceite da EULA a quem roda o servidor (o Warden nunca grava `eula=true` no pacote). Aviso "Este pacote ainda não foi testado num servidor" com **Testar como servidor** (T27); com a caixa "Testar no servidor deste computador depois de gerar" (marcada por padrão quando a EULA já foi aceita), o Warden descompacta numa pasta temporária e roda o servidor até "Done": "Pacote para servidor testado: o servidor abriu em 34 s."

**Antes de exportar (sempre):**
1. Verificação de higiene (a mesma de T04).
2. Diagnóstico: com erros, aviso "O pack tem N erros." com **Ver** e **Exportar mesmo assim**.
3. Alterações não salvas: "A exportação vai usar o estado atual, que não é uma versão salva. Recomendamos salvar uma versão antes." com **Salvar versão** e **Continuar**. P1: exportar uma versão salva específica.

**Pré-visualização:** árvore com exatamente o que vai, contagem e tamanho por pasta, e:
- mods/resource packs/shaders por referência (quantos; não incluem o arquivo);
- arquivos locais incluídos (marcados, com tamanho);
- configs incluídos;
- alertas: arquivos grandes (> 20 MB), padrões suspeitos de cache, `options.txt` sem "não substituir", arquivos soltos na raiz do pack.
- Excluir algo da exportação = **Excluir do pack** (adiciona regra ao `.packwizignore`, mostrando a mudança).

**Resultado:** contém só `pack.toml`, `index.toml` e os arquivos listados no índice. Nunca `.warden/`, `.git/`, `CHANGELOG.md` nem nada fora do índice. Mensagem final com **Abrir pasta**.

| CA | Critério |
|---|---|
| CA-T19-01 | O conjunto de arquivos exportados é exatamente {`pack.toml`, `index.toml`} ∪ {arquivos do índice} (teste comparando listas). |
| CA-T19-02 | Rodar `packwiz refresh` na pasta exportada não produz diferença; o packwiz-installer real instala a partir dela numa pasta vazia sem erro (teste de conformidade). |
| CA-T19-03 | Um `crash-reports/x.txt` criado à mão na pasta do pack aparece como alerta de higiene e não é exportado depois de "Limpar". |
| CA-T19-04 | (P1) O `.mrpack` gerado abre como zip válido, contém `modrinth.index.json` válido pelo esquema do formato (nenhum `env` com valor fora da especificação, como `unknown`), `overrides/` só com arquivos do índice, e é importável pelo Modrinth App ou por um launcher compatível (teste de integração + verificação manual no marco). |
| CA-T19-05 | (P1) O zip da CurseForge gerado contém `manifest.json` válido, `overrides/` só com arquivos do índice, e nenhum jar de mod de terceiros sem confirmação explícita (teste com pack contendo mod da CurseForge, do Modrinth e local). |
| CA-T19-06 | (P1) O pacote para servidor pelo link contém `start.bat`, `start.sh`, `user_jvm_args.txt`, o bootstrap e o installer fixados, configs e `kubejs/` sem `client_scripts/`, e nenhum mod "Só cliente", `options.txt`, resource pack ou shader; descompactado numa pasta vazia com o pack publicado num servidor local que imita o GitHub, `start.sh` instala os mods do lado servidor e o servidor chega a "Done" depois de aceitar a EULA (teste de conformidade em Linux; roteiro manual com `start.bat` no Windows). |
| CA-T19-07 | (P1) O pacote com mods dentro do zip contém os jars dos mods de servidor com o hash do índice; um mod da CurseForge bloqueado abre o download manual (T20) antes de gerar; nenhum arquivo de terceiros entra sem a confirmação de licença. |
| CA-T19-08 | (P1) Com "Testar no servidor deste computador depois de gerar", um pacote com um mod de cliente marcado como "Cliente e servidor" falha na validação com o trecho do log do servidor e o mod apontado; um pacote correto termina com "Pacote para servidor testado" e o tempo até "Done". |

---

### T20 — Downloads manuais da CurseForge (P0)

**Quando aparece:** ao adicionar (aviso antecipado) e na etapa "Copiar o pack para o teste" (T13), para mods cujo autor bloqueou downloads por apps de terceiros. Os mesmos mods geram um aviso antes de publicar (T18).

**Conteúdo:** lista com nome, arquivo esperado e estado (aguardando, encontrado, hash não confere). Para cada um: **Abrir página de download** (navegador) e **Selecionar arquivo baixado…**. P1: o Warden observa a pasta Downloads e reconhece os arquivos pelo hash automaticamente.

**Regras:** o arquivo vai para o cache do Warden, nunca para a pasta do pack. Hash diferente do esperado: recusa ("Este não é o arquivo certo."). Quando todos forem encontrados, o teste continua sozinho. Aviso: "Quem jogar o pack também vai precisar baixar esses mods manualmente."

| CA | Critério |
|---|---|
| CA-T20-01 | Selecionar um arquivo com hash errado é recusado; com o hash certo, o mod é instalado na instância e o teste prossegue. |
| CA-T20-02 | Após o fluxo, a pasta do pack não contém o jar. |

---

### T21 — Configurações do app (P0)

Página única, rolável, sem submenu, nas seções abaixo (nesta ordem). Abre pelo topo de Meus packs ou por links de contexto (ex.: "Abrir Configurações" quando falta uma chave) e volta para onde você estava.

| Seção | Conteúdo |
|---|---|
| Geral | Pasta padrão dos packs; nome do jogador do teste; "Rever boas-vindas". |
| Chaves e contas | CurseForge (chave), Gemini (chave e modelo), GitHub (token). Campos de senha. **Testar** em cada um. Depois de salvo, o valor nunca é mostrado de novo; só "Configurada" com **Substituir** e **Remover**. Passo a passo para criar cada chave, com links; o do GitHub indica as permissões necessárias para publicar (T18). **Onde guardar as chaves:** **Cofre do Windows** (padrão, recomendado) ou **Arquivo .env na pasta de dados do Warden** (`%APPDATA%\dev.kriticales.warden\.env`, fora dos packs e do GitHub). Escolher o `.env` pede confirmação: "O .env é um arquivo de texto comum. Qualquer programa no seu computador, e qualquer backup ou sincronização dessa pasta, consegue ler as chaves."; enquanto ele estiver em uso, o aviso fica fixo na seção, com **Voltar para o cofre**. Trocar de modo move as chaves e apaga a cópia anterior (ADR-0025). |
| Teste | Memória padrão (Automático/fixo); **Java**: tabela com cada Java instalado pelo Warden, os packs que o usam e por que aquela versão (o mais novo que funciona; ADR-0029), com **Procurar atualizações do Java** e **Remover Javas sem uso**; mostrar versões beta/alpha dos mods; verificação automática de atualizações (intervalo); P1: **EULA do Minecraft** ("Aceita em 01/10/2026" com **Revogar**, ou "Ainda não aceita"; usada pelo servidor local, T27, decisão D19). |
| Editor de configs | Mostrar diferenças antes de salvar (ligado). |
| Privacidade e registros | Abrir pasta de registros; nível de detalhe dos registros (normal/detalhado). |
| Armazenamento (P1) | Espaço usado por cache, arquivos do Minecraft, Javas e instâncias; **Limpar cache**. |
| Sobre o Warden | T23. |

**Regras:** chaves e tokens ficam no cofre do sistema (Gerenciador de Credenciais do Windows) ou, se você escolher, no `.env` da pasta de dados do Warden; nunca no `settings.json`, nos packs, nos registros ou no GitHub (ADR-0025).

| CA | Critério |
|---|---|
| CA-T21-01 | No modo cofre, salvar uma chave e reiniciar o app mantém o estado "Configurada"; o `settings.json` não contém a chave (teste de varredura). |
| CA-T21-02 | "Testar" da CurseForge com chave inválida mostra "A CurseForge recusou a chave." |
| CA-T21-03 | Trocar para o `.env` move as três chaves para `%APPDATA%\dev.kriticales.warden\.env` e as apaga do cofre; reiniciar mantém "Configurada"; voltar para o cofre move de volta e apaga o `.env`; uma falha simulada no meio da troca não perde nenhuma chave; em nenhum modo a chave aparece em `settings.json`, nos registros ou em pastas de packs (teste de varredura). |
| CA-T21-04 | A tabela de Java lista cada Java instalado com os packs que o usam e o motivo; para um pack Forge 1.7.10, o motivo diz que o Forge dessa versão só abre no Java 8; com uma atualização nova do Java 17 publicada (servidor simulado), "Procurar atualizações do Java" a instala, e a antiga é removida quando nenhum jogo a usa. |
| CA-T21-05 | (P1) Aceitar a EULA na primeira vez que um servidor local vai abrir grava o aceite com a data em `settings.json`; Configurações mostra a data e **Revogar**; depois de revogar, o próximo servidor local pede o aceite de novo e nenhum `eula.txt` com `eula=true` é gravado antes disso. |

---

### T22 — Tarefas em andamento (P0)

Painel (gaveta) aberto pelo indicador do rodapé, presente em todas as telas do app e do pack (não é item de menu): operações em andamento com progresso e **Cancelar** (quando a operação permite), e as últimas 50 concluídas com resultado. Toda operação que leva mais de 300 ms mostra progresso; toda operação que pode levar mais de 2 s e não altera o pack de forma irreversível pode ser cancelada. Erros têm **Ver detalhes**.

| CA | Critério |
|---|---|
| CA-T22-01 | Cancelar a sincronização do pack (download de mods para a instância) no meio para em até 2 s e não deixa arquivo incompleto com nome final (só `.part` no cache, limpos depois). O mesmo vale para a preparação do Minecraft, conforme os critérios do motor (ARCHITECTURE §7.1). |

---

### T23 — Sobre (P0)

Última seção de Configurações ("Sobre o Warden", T21), não uma tela à parte. Conteúdo: versão do Warden, commit do packwiz embutido, aviso legal da Mojang/Microsoft (T01), créditos e licenças de terceiros (gerados automaticamente), link de apoio ao Forge (pedido do próprio instalador do Forge; R2 §3.4).

---

### T24 — Importar modpack (P1)

**Objetivo:** trazer para o Warden um modpack feito em outro app (decisão D15; ADR-0035; R5B §5.1).

**Onde:** **Abrir ou importar…** em Meus packs (T04). O Warden reconhece o tipo pelo conteúdo: `.mrpack` (`modrinth.index.json`), zip da CurseForge (`manifest.json`), pasta ou zip de instância do Prism Launcher ou MultiMC (`instance.cfg` e `mmc-pack.json`), pasta de instância do app da CurseForge (`minecraftinstance.json`).

**Página Importar modpack** (nível do app, com "← Meus packs"):
- **O que foi reconhecido:** tipo do arquivo, nome, versão, Minecraft e loader; **licença do projeto** quando vier na fonte, com aviso se não for livre ("All Rights Reserved: as configs e os arquivos são trabalho do autor. Para uso pessoal, tudo bem; para redistribuir, peça permissão.").
- **Nome e pasta** do pack novo (mesmas regras do T03, etapa 1).
- **Resumo da conversão** (lista, com contagens e "Ver lista"): **viram referência** (Modrinth ou CurseForge, inclusive jars soltos identificados pelo hash); **ficam como arquivo local** (jars que não estão em nenhuma loja, com aviso de licença); **para revisar** (lado desconhecido, `env` fora da especificação, `client-overrides/`, opcionais sem padrão definido); **não entram** (lixo encontrado pela verificação de higiene do T04, como `.mixin.out/`, `xmcl.json`, `mods/.connector/temp/`, logs e caches). `server-overrides/` vai para a pasta `server-overrides/` do pack, que só o pacote para servidor usa.
- **Instância do Prism:** a memória e os argumentos da instância viram um **perfil do teste** (T11); `options.txt` e `servers.dat` são perguntados ("Levar suas preferências para o pack?").
- **Importar como pack novo**: cria o pack (repositório git, arquivos de controle, `.warden/`) e abre em Mods com o aviso "N itens para revisar" em Problemas. O arquivo de origem nunca é alterado.

**Estados:** lendo o arquivo (progresso); loader não suportado (Quilt: mesma mensagem do T04, nada é criado); zip da CurseForge sem chave da CurseForge: "Para importar um modpack da CurseForge, o Warden precisa da sua chave da CurseForge." com **Abrir Configurações**; arquivo corrompido ou com caminhos perigosos: recusa com o motivo, nada é criado.

| CA | Critério |
|---|---|
| CA-T24-01 | Importar um `.mrpack` real (corpus com o "Create+", que traz `.mixin.out/`, `xmcl.json`, `mods/.connector/temp/` e `env: "unknown"`) cria um pack em que cada arquivo do `cdn.modrinth.com` vira `.pw.toml` com `[update.modrinth]`, o lixo não entra, os itens com `env` desconhecido ficam "Cliente e servidor" com o aviso de lado desconhecido, e o `packwiz refresh` seguinte não produz diferença (teste de integração com o packwiz real e servidor simulado). |
| CA-T24-02 | Um `.mrpack` com `files[].path` contendo `..`, caminho absoluto ou letra de drive é recusado sem criar nada (teste de segurança). |
| CA-T24-03 | Importar um zip da CurseForge usa o `packwiz curseforge import` em staging e corrige o lado pelo Modrinth quando o mesmo arquivo (SHA-1) existe lá; sem chave da CurseForge, mostra a mensagem e não faz nenhuma requisição (servidores simulados). |
| CA-T24-04 | Importar uma instância do Prism com `mods/.index/*.pw.toml` reaproveita esses metadados (sem os campos `x-prismlauncher-*`), identifica por hash os jars sem metadados e cria o perfil do teste com a memória do `instance.cfg`; mundos, logs e caches da instância não entram. |

---

### T25 — Busca do culpado (P1)

**Objetivo:** achar sozinho o mod, ou a combinação de mods, que causa um problema que se repete, quando as regras e o log não bastam (decisão D15 e D23; ADR-0031; R5A §3).

**Onde:** é um **modo da tela do teste** (não é seção). Começa por **Encontrar o mod culpado** no resultado "O jogo travou", em Problemas → Travamentos, no menu ▾ do Testar ("Encontrar o mod culpado…") ou por proposta da IA. Durante a busca, o botão do cabeçalho mostra "Buscando o culpado: ver progresso", e as 5 etapas do teste dão lugar à **trilha de rodadas**.

**Configuração** (diálogo "Encontrar o mod culpado"):
- **O que acontece:** Trava ao abrir o jogo (padrão quando vem de um travamento no carregamento) · Trava ao entrar no mundo · Aparece uma mensagem no log (escolher a linha no console) · Outra coisa: eu digo se aconteceu (**modo assistido**).
- **Como funciona**, em linguagem simples: "O Warden abre o jogo várias vezes com partes do pack, numa cópia separada. Seu pack, a instância de teste e seus mundos não mudam. As bibliotecas exigidas sempre vão junto com quem precisa delas." Lista do que fica sempre ligado (o loader e bibliotecas que todos exigem) e do que fica desligado durante a busca (spark e Crash Assistant).
- **Estimativa:** "11 a 17 rodadas · cerca de 22 a 35 minutos neste pack. Dá para pausar e cancelar a qualquer momento." (decisão D23).
- Quando a entrada no mundo exige servidor (Minecraft anterior a 1.20, ou problema de servidor): aviso explicando por que o Warden precisa abrir um servidor local, com a memória necessária e o pedido da EULA se ainda não foi aceita (decisão D18). Sem o aceite, a busca usa o modo assistido.
- **Começar a busca** / **Cancelar**.

**Durante a busca:** título "Buscando o mod culpado"; linha "Rodada 6 de 11 a 17 · 7 mods ainda suspeitos · 12 min até agora"; trilha de rodadas (cada rodada com o resultado em ícone e palavra: Passou, Travou igual, Travou diferente (não conta), Tempo esgotado, Em andamento, Ainda não); painel **O que o Warden está fazendo agora** ("Rodada 6: abrindo o jogo com 69 dos 118 mods. Se travar igual, o culpado está entre os 3 mods acrescentados agora."); lista recolhível dos mods ainda suspeitos; tabela das rodadas (rodada, mods ligados, resultado, tempo, **Ver log**). Ações: **Pausar depois desta rodada** (o estado fica guardado e a busca continua depois, até em outro dia) e **Cancelar busca** (confirmação; encerra o jogo, apaga a cópia e guarda o resultado parcial: "O culpado está entre estes 7 mods").
- **Modo assistido:** cada rodada abre o jogo e pergunta "Faça o que causa o problema e diga o que aconteceu": **Aconteceu** · **Não aconteceu** · **Não deu para testar**. Se o jogo travar sozinho, a rodada é marcada sem perguntar.
- **Não reproduziu:** se o problema não aparecer em 3 testes seguidos na rodada 0: "O problema não apareceu em 3 testes seguidos. A busca só funciona com problemas que se repetem." Problema que aparece em menos de ~30% das vezes: o Warden recomenda não fazer a busca e mostra o tempo extra se você quiser mesmo assim.
- **Sem mods já trava:** "Mesmo sem mods o jogo trava. A causa está no Java, no driver de vídeo ou nos ajustes do teste." com link para Ajustes do teste.
- **Resultados que se contradizem:** a busca para e explica ("Os testes se contradizem: o mod X parece evitar o problema. Isso acontece com mods de compatibilidade."), com a tabela das rodadas.

**Resultado:** "Culpado encontrado: Epic Fight (junto com o Supplementaries)", a explicação ("Sem o Epic Fight, entrou no mundo normalmente; só com o Epic Fight e o Supplementaries, travou com a mesma mensagem."), a confiança ("alta: confirmado em 2 testes de controle"), rodadas e tempo, e as ações **Remover Epic Fight do pack** (confirmação normal), **Ver o que o Epic Fight altera no jogo** (raio-x já filtrado nos dois mods), **✦ Perguntar à IA** e **Ver as rodadas**. Quando a confirmação final mostra outro culpado independente: "Há outro problema além deste." com **Continuar a busca sem o Epic Fight**. O resultado fica ligado ao travamento em Problemas → Travamentos.

| CA | Critério |
|---|---|
| CA-T25-01 | Com um jogo simulado que trava sempre que os mods X e Y estão juntos, num pack sintético de 118 mods com cadeias de dependência, a busca encontra {X, Y} em no máximo 17 rodadas, nunca liga um mod sem as dependências obrigatórias dele e confirma o resultado com os 2 testes de controle (teste de domínio do algoritmo com o executor simulado). |
| CA-T25-02 | Uma rodada que termina com outra assinatura ("travou diferente") não conta como reprodução; um `NoClassDefFoundError` de uma classe de um mod que não estava ligado vira dependência inferida, com o aviso "o mod X usa Y sem declarar a dependência", e a rodada é refeita (teste de domínio). |
| CA-T25-03 | Cancelar no meio encerra o jogo em até 5 s, apaga a instância da busca, guarda o resultado parcial e não altera o pack, a instância de teste nem os mundos (hash antes e depois); pausar e reabrir o app continua da rodada seguinte, a partir do `state/bisect.json` (teste de integração). |
| CA-T25-04 | No modo assistido, cada rodada mostra as três respostas, e um travamento detectado sozinho marca a rodada sem perguntar (teste de componente). |
| CA-T25-05 | Para cada faixa da matriz L-05, os marcadores de sucesso e de falha usados pela busca são os documentados no spike S-R5-3, com golden tests de logs reais; em Forge 1.12.2, a busca usa `-Dfml.queryResult=confirm` para não parar na tela de mods ausentes do mundo; em Fabric, `-Dfabric.noGui`. |
| CA-T25-06 | (com T27) Num pack Forge 1.16.5 com o problema "trava ao entrar no mundo", a busca explica que precisa do servidor local e pede antes de abrir; com o aceite, cada rodada sobe o servidor em `127.0.0.1` com uma cópia descartável do mundo e fecha o servidor ao fim da rodada. |

---

### T26 — Editor de scripts KubeJS e CraftTweaker (P1)

**Objetivo:** editar os scripts do pack com ajuda, ver os erros que o jogo encontrou e recarregar sem fechar o jogo (decisão D15; ADR-0037; R5B §3).

**Onde:** em **Configs** (T12): arquivos `.js` de `kubejs/` e `.zs` de `scripts/` abrem no mesmo editor, com os recursos abaixo. Durante o teste, **Recarregar scripts** fica na tela do teste (T13).

**Conteúdo:**
- Barra do editor com a versão detectada ("KubeJS 6 · Forge 1.20.1", "CraftTweaker 14 · Forge 1.20.1"); pack sem KubeJS nem CraftTweaker: os arquivos abrem como texto comum.
- **Realce** de JavaScript e de ZenScript.
- **Trechos prontos ▾** dos eventos mais usados, por versão (KubeJS: receitas, tags, registro de itens, dicas de item; CraftTweaker: receitas de bancada e afins).
- **Autocompletar de IDs** de itens, blocos e tags (`minecraft:stone`, `#forge:ingots/iron`) lidos dos jars do pack, sem abrir o jogo; com o jogo aberto e o KubeJS 7, também os do servidor web local do KubeJS.
- **Erros dos scripts:** painel com os erros e avisos do último teste (logs `logs/kubejs/*.log` e `crafttweaker.log`; com o jogo aberto e o KubeJS 7, ao vivo), cada um com arquivo e linha clicáveis e a linha original do log.
- **Recarregar no jogo:** com KubeJS 7 e o jogo aberto, pelo servidor web local do KubeJS (o Warden lê o token na instância de teste); com o servidor local aberto (T27), pelo console do servidor; nos outros casos, o Warden mostra o comando certo para a versão ("/kubejs reload server_scripts", "/reload") com **Copiar**. Sem jogo aberto, o botão fica desabilitado com o motivo. Com o jogo aberto, salvar grava na instância de teste e oferece **Salvar e recarregar**; o script volta ao pack por "O que mudou durante o teste".
- **Abrir no VS Code** abre a pasta da instância de teste no VS Code (quando instalado).
- Fora da v1 (P2): autocompletar completo com tipagens do ProbeJS e servidor de linguagem.

| CA | Critério |
|---|---|
| CA-T26-01 | Num pack Forge 1.20.1 com KubeJS 6, `kubejs/server_scripts/receitas.js` abre com realce, a barra mostra "KubeJS 6 · Forge 1.20.1" e "Trechos prontos" oferece os eventos dessa versão; digitar `'minecraft:copper_` lista `minecraft:copper_ingot` a partir dos jars do pack (teste de componente + domínio com jars de teste). |
| CA-T26-02 | Depois de um teste em que o KubeJS registrou `receitas.js#16: … Invalid item: minecraft:copper_ingott`, o painel Erros dos scripts mostra o erro na linha 16 e clicar leva à linha (golden test com log real). |
| CA-T26-03 | Com KubeJS 7 e o jogo simulado expondo o servidor web local em `127.0.0.1`, **Recarregar no jogo** chama `POST /api/reload/server` com o token lido da instância de teste; o token nunca aparece na interface, nos registros nem no pack (teste com servidor simulado e varredura). |
| CA-T26-04 | Com KubeJS 6 e só o cliente aberto, **Recarregar no jogo** mostra o comando `/kubejs reload server_scripts` com **Copiar**; com o servidor local aberto, envia o comando pelo console do servidor e mostra a resposta. |

---

### T27 — Testar como servidor (servidor local, P1)

**Objetivo:** abrir o pack como servidor neste computador, só quando você pede, para conferir que ele funciona no servidor (decisões D18 e D19; ADR-0032; R5B §4.2).

**Onde:** menu ▾ do Testar → **Testar como servidor…**. O servidor local também é usado, sempre perguntando antes, para validar o pacote para servidor (T19) e pela busca do culpado (T25). Nunca faz parte do ▶ Testar normal.

**Fluxo:**
1. **EULA (só na primeira vez):** diálogo "Aceitar a EULA do Minecraft?" com o link https://aka.ms/MinecraftEULA e o texto "Para abrir um servidor do Minecraft, a Mojang exige que você aceite a EULA. O Warden pergunta só uma vez e lembra a sua resposta; dá para revogar em Configurações." Botões **Aceito a EULA** e **Agora não**.
2. **Opções:** **Abrir o servidor e entrar nele com o jogo** (recomendado: testa os dois lados) ou **Só o servidor** (mais leve). Memória do servidor e do jogo, a soma e a RAM livre agora ("Servidor 4 GB + jogo 6 GB = 10 GB · livre agora: 13 GB"); se não couber, aviso com a sugestão de "Só o servidor" ou de diminuir a memória. Texto fixo: "Só aceita conexões deste computador (127.0.0.1) e fecha sozinho quando o teste acabar." Lista dos mods que ficam de fora por serem só de cliente. **Abrir servidor e testar**.
3. **Tela do teste com servidor:** a etapa "Preparar o servidor" (instalar o servidor do loader na primeira vez, copiar o pack só com os mods de servidor) entra antes de "Abrir o jogo". Com os dois abertos, o console tem "Mostrando: Servidor · Jogo" (caixa de seleção, não aba) e o console do servidor tem uma linha de comando ("Enviar comando ao servidor"). O jogo entra direto no servidor. **Parar jogo e servidor** encerra os dois (o servidor recebe `stop` e, se não fechar em 60 s, é encerrado à força).
4. **Resultado:** o que aconteceu em cada lado ("O servidor abriu em 14 s e fechou normalmente; o jogo entrou no servidor"), com "Por que travou" para o lado que travou (por exemplo, "O mod X é de cliente e derrubou o servidor", com o trecho do log do servidor).

| CA | Critério |
|---|---|
| CA-T27-01 | Para Fabric 1.21.1, NeoForge 1.21.1, Forge 1.20.1 e Forge 1.12.2, o servidor local instala, chega a `Done (…)! For help, type "help"` e fecha com `stop` em até 60 s; o `server.properties` tem `server-ip=127.0.0.1`, `online-mode=false`, `enable-rcon=false` e uma porta livre (teste de integração sob demanda + matriz). |
| CA-T27-02 | Sem a EULA aceita, nenhum `eula.txt` com `eula=true` é gravado e o servidor não é iniciado; depois do aceite, a pergunta não aparece de novo (teste de integração). |
| CA-T27-03 | Com RAM livre menor que servidor + jogo (valor simulado), o diálogo mostra o aviso antes de abrir e sugere "Só o servidor"; nada é iniciado sem confirmação. |
| CA-T27-04 | Um pack com um mod só de cliente marcado como "Cliente e servidor" que derruba o servidor termina com o resultado "o servidor travou", o mod apontado e o trecho do log do servidor; mods com lado "Só cliente" não são copiados para a pasta do servidor. |
| CA-T27-05 | Fechar o Warden com o servidor aberto, depois de confirmar, não deixa processo `java` do servidor órfão; ao fim do teste, a porta é liberada (teste de integração). |

---

## 7. Regras que valem para o app inteiro

1. **A pasta do pack só recebe conteúdo do pack.** Nenhum `.bak`, temporário, cache, log, jar auxiliar ou configuração do app. Gravações usam arquivo temporário com sufixo `.warden-tmp` na mesma pasta seguido de renomeação; esse sufixo está no `.packwizignore` e sobras dele são apagadas ao abrir o pack.
2. **Confirmação e reversão.** Ação que apaga ou substitui conteúdo do pack pede confirmação nomeando o que será afetado, e cria ponto de segurança nos casos listados em ARCHITECTURE §11.
3. **Erros explicados.** Toda mensagem de erro diz o que aconteceu e o que fazer, em pt-BR, com **Detalhes técnicos** recolhível (código do erro e saída da ferramenta) e **Copiar**.
4. **Sem internet.** Tudo o que não depende de rede continua funcionando. O que depende mostra "Sem conexão com a internet" e **Tentar de novo**; nunca fica carregando para sempre (limite de tempo em toda requisição).
5. **Nada de botão morto.** Toda ação visível funciona ou está desabilitada com dica explicando por quê.
6. **Teclado e acessibilidade.** Toda ação acessível por teclado, foco visível, contraste AA, textos alternativos em ícones com função.
7. **Uma janela.** Só uma cópia do Warden aberta por vez; abrir de novo foca a janela existente.
8. **Privacidade.** Nada é enviado a terceiros além das chamadas necessárias às APIs (Mojang, Modrinth, CurseForge, Adoptium, Maven dos loaders, GitHub e, com o consentimento da conversa, Gemini e a busca de issues no GitHub). Sem telemetria. O Warden nunca faz upload de perfis do spark nem de logs para serviços de colagem.
9. **Linguagem.** Português do Brasil, "você", frases curtas, sem gírias técnicas não explicadas (QUALITY §8).
10. **O pesado só sob demanda.** Servidor local, busca do culpado e perfil de desempenho só rodam quando o usuário pede, sempre com pausar ou cancelar e com uma explicação do que está acontecendo. O ▶ Testar continua fazendo só o teste normal (decisão D18).
11. **Tudo que roda em rede local fica em 127.0.0.1.** Servidor local, servidor de arquivos do teste "como o jogador recebe" e chamadas ao servidor web do KubeJS nunca escutam nem chamam fora do próprio computador.

## 8. Requisitos não funcionais

| Requisito | Meta |
|---|---|
| Abrir o app até a Lista de packs | < 3 s em máquina de referência (Windows 11, SSD, 16 GB). |
| Abrir um pack de 300 mods (com cache) | < 1 s até a lista visível. |
| Busca (Modrinth) | < 2 s em conexão comum; resultados parciais nunca travam a interface. |
| Atualizar índice (300 mods + 500 configs) | < 5 s. |
| Diagnóstico pré-teste (300 mods, cache quente) | < 3 s. |
| Raio-x de mixins e índices de pacotes (300 mods) | < 3 s sem cache; < 300 ms com cache por hash dos jars. |
| Busca em todas as configs (500 arquivos) | Índice pronto em < 2 s ao abrir o pack; cada busca em < 200 ms. |
| Início da página de descoberta | < 2 s em conexão comum, com no máximo 3 requisições por fonte. |
| Amostragem de desempenho durante o teste | A cada 1 s, sem custo perceptível no jogo (< 1% de CPU do Warden). |
| Console agrupado | Agrupa 50.000 linhas sem travar a interface. |
| Interface | Nunca congela; trabalho pesado sempre fora da thread da interface. |
| Memória do app (sem jogo) | < 500 MB. |
| Robustez | Encerrar o app à força em qualquer momento não corrompe o pack (gravações atômicas, testes de falha). |
| Registros | Arquivos diários, guardados por 14 dias, sem chaves nem tokens. |

## 9. Fora da v1

Revisado na tarefa D4 (decisão D15): saíram desta lista o teste com servidor dedicado (agora T27, P1), "entrar direto no mundo" (Quick Play, agora nos perfis do teste, T11), importar `.mrpack` e zip da CurseForge (agora T24, P1) e os kits de performance (agora P1, T03 e T08).

- Quilt; Legacy Fabric para 1.7.10/1.12.2 (pode vir depois); versões anteriores a 1.7.10 além do "melhor esforço"; snapshots do Minecraft.
- Login Microsoft/Mojang, contas, skins, multiplayer online.
- Teste com um mundo antigo trazido de fora (P2).
- Publicar direto no Modrinth/CurseForge (upload pela API).
- Várias variantes do mesmo pack (versões do Minecraft diferentes no mesmo projeto) e assistente de migração de versão do Minecraft (P2).
- Editor de `servers.dat`, formulário para YAML, detecção de conflitos de teclas, modelos de pack ("Salvar como modelo", "Criar a partir de um modpack"), datapacks globais e datapacks como Tipo na página de descoberta (todos P2).
- Diagnóstico avançado P2 (R5A §10.1): raio-x de mixins com tradução de nomes sem refmap e aplicação a seco do Mixin; perfil automático do cliente (JFR ou agente do spark) e tempo por mod no Fabric; busca do culpado em configs, scripts ou por desempenho; saída estruturada do log4j por faixa.
- Edição avançada P2 (R5B §9): padrões de config gerados por servidor ou por cliente limpo ("Descobrir padrões"); metadados de config lidos do bytecode e dicionário curado de configs populares; autocompletar completo de KubeJS com `tsc` e ProbeJS e servidor de linguagem de ZenScript; matriz de perfis ("qual a memória mínima?"); "Ver como este modpack configurou o mod X"; importar perfil do app do Modrinth.
- Painel git completo (branches, merges), trazer mudanças do GitHub (P2), sincronização entre computadores além do GitHub.
- Atualização automática do Warden; atualização remota dos dados curados (conflitos, kits, categorias); macOS; empacotamento oficial para Linux (o app compila e roda em Linux para desenvolvimento).
- Temas além do Deep Dark; outros idiomas.

## 10. Decisões do dono

Respondidas pelo dono em 01/10/2026. O dono aceitou as recomendações, com exceção de D1. D10 a D14 vieram da revisão da estrutura (tarefa D2), no mesmo dia; D14 alterou D2. D15 a D26 vieram das pesquisas R5A e R5B (tarefa D4), também no mesmo dia.

| # | Pergunta | Decisão |
|---|---|---|
| D1 | Exportar já na v1 também em `.mrpack` (Modrinth) e zip da CurseForge? | **Sim, na v1** (P1; T19, tarefa E-02 do ROADMAP). |
| D2 | Cada pack com repositório próprio no GitHub? | ~~Sim, um repositório privado por pack.~~ **Alterada pelo dono em 01/10/2026:** um repositório por pack, **público recomendado**, porque o GitHub passa a distribuir o pack aos jogadores pelo link do `pack.toml`; privado fica como opção "só backup, sem atualização automática para jogadores" (T18, ADR-0028). |
| D3 | Permissão para publicar no GitHub? | **Token** colado no app (guardado como as demais chaves, T21) na v1; login pelo navegador depois. |
| D4 | Como instalar no Windows? | **Instalador .exe comum sem assinatura digital.** |
| D5 | Aceitar a licença do kit da Microsoft usado pelo `cargo-xwin` para gerar a versão de Windows no WSL? | **Sim** (resposta explícita do dono em 01/10/2026). |
| D6 | Mundos dos testes persistem entre testes? | **Sim**, na instância de teste do pack (nunca no pack), com botão para apagar. |
| D7 | Oferecer limpeza ao abrir pack dos apps antigos? | **Sim**, mostrando a lista antes e criando ponto de segurança; nada é apagado sem confirmação. |
| D8 | Só versões estáveis por padrão? | **Sim**, com opção em Configurações para incluir beta/alpha. |
| D9 | Quando rodar a CI de Windows? | **Ao integrar na `main` e uma vez por noite**; Linux em toda mudança. |
| D10 | Estrutura de navegação (tarefa D2)? | **Alternativa A**: dois níveis (app e pack), 6 seções no pack (Mods, Configs, Problemas, ✦ Diagnóstico com IA, Histórico, Exportar), Testar no cabeçalho; mudanças M1–M12 da `docs/design/ESTRUTURA.md` aplicadas (§5, ADR-0026). |
| D11 | Buscar mods por fonte separada ou junto? | **Busca combinada** Modrinth + CurseForge, com a fonte marcada e sem duplicatas; sem chave da CurseForge, só Modrinth com aviso (T08, ADR-0027). |
| D12 | Onde guardar as chaves? | **Os dois:** cofre do Windows por padrão, com opção de arquivo `.env` na pasta de dados do Warden e aviso claro (T21, ADR-0025). |
| D13 | Qual Java usar? | **O mais novo que funciona** com cada versão, sempre atualizado, com o motivo à vista quando não for o mais novo (T11, T21, ADR-0029). |
| D14 | Para que serve o GitHub? | **Distribuir o pack aos jogadores:** salvar é local; versões marcadas como finais são publicadas, com changelog e Release; jogadores atualizam pelo link do `pack.toml` (T16–T18, ADR-0028). Substitui o "sempre privado" da D2. |
| D15 | O app está "simplificado demais": o que entra das funções avançadas? | **Entram na v1**, com as prioridades P0/P1 das seções "Implicações para o Warden" da R5A (§10.1) e da R5B (§9): busca do culpado (T25); IA "médico" com ferramentas e conversas por pack (T14); raio-x de mixins, camadas A e B, só avisos (T07, T14); grafo de dependências e nota de saúde (T06, T14, T02); console agrupado por mod, memória ao vivo e tempo de carregamento (T13); spark e perfil de desempenho (T13); histórico de travamentos (T14); configs com formulário, busca em todas as configs e restaurar padrão (T12); editor de scripts KubeJS/CraftTweaker no escopo v1 da R5B (T26); perfis do teste com Quick Play (T11); "Testar como o jogador recebe" pelo link (T13); **importar** `.mrpack`, zip da CurseForge e instância do Prism (sobe para P1, T24); pacote para servidor (T19); **kits de desempenho** (sobem para P1, T03, T08); **página de descoberta** em tela cheia aberta por Adicionar, com populares, categorias, seleção múltipla, busca combinada e navegar por modpacks com "Adicionar N selecionados" (T08). |
| D16 | Mods padrão em pack novo? | **spark e Crash Assistant**, marcados no assistente e desmarcáveis, com o envio de dados ao autor do Crash Assistant desligado na config inicial. O Crash Assistant fica **fora dos testes normais** e **dentro do "Testar como o jogador recebe"**. O Warden só inclui o mod no pack e nunca usa o código dele (cláusula de não concorrência da licença; T03, T13, ADR-0033). |
| D17 | Como a IA pede permissão? | **Uma vez por conversa**, mostrando o que a IA poderá consultar; cada envio fica visível no chat; nada muda no pack sem clicar em **Aplicar**. A IA pode procurar problemas parecidos nas issues do GitHub dos mods, enviando o nome do mod e palavras do erro, nunca o log (T14, ADR-0030; substitui em parte a ADR-0014). |
| D18 | Servidor local? | **Ferramenta opcional e sob demanda**, nunca parte do ▶ Testar normal. Usos: "Testar como servidor" (menu ▾), validar o pacote para servidor e apoio à busca do culpado quando necessário (o Warden explica e pede antes de abrir). Escuta só em 127.0.0.1, fecha sozinho ao terminar e avisa se a RAM livre for insuficiente (T27, ADR-0032). |
| D19 | EULA da Mojang para o servidor local? | **Pedida uma vez, com o link, e lembrada**; dá para revogar em Configurações (T21, T27). |
| D20 | Nomes? | A seção Configs passa a ter a descrição **"Arquivos de ajuste e scripts do pack"**; **"Abrir pack existente" vira "Abrir ou importar…"** (T02, T04, T05, ESTRUTURA §13). |
| D21 | Higiene e segredos dos arquivos novos? | Acrescentar ao `.packwizignore` padrão e aos "sempre ignorados" da captura: `/kubejs/config/web_server.json`, `/.probe/`, `/.vscode/`, `/local/kubejs/`, `config/spark/*.sparkprofile`, `config/spark/*.sparkheap`, `config/spark/tmp*` (T04, T15, ARCHITECTURE §6.4 e §8.4). |
| D22 | Pacote para servidor? | **Duas variantes:** padrão = baixa os mods pelo link do pack (bootstrap com `-s server`); alternativa = zip com os mods dentro (T19, ADR-0035). |
| D23 | Busca do culpado demorada? | Pode levar **20 a 40 minutos** num pack grande, sempre com **pausar e cancelar** e explicação clara do que está fazendo (T25, ADR-0031). |
| D24 | Onde mostrar a nota de saúde? | Em **Problemas** e também na lista **Meus packs** (T02, T14). |
| D25 | Achados fora do escopo das pesquisas? | **Aplicar:** linha de ajuda sobre o erro 403 do bootstrap no passo a passo dos jogadores (T18); aviso de que o NeoForge apaga chaves desconhecidas e volta valores fora da faixa ao padrão (T12); `env: "unknown"` aceito na leitura de `.mrpack` e nunca gerado (T19, T24); correções de documentos da R5A §10.3 (ARCHITECTURE §7.4, §8.4, §9.5; ADR-0014; R2 §6.4). |
| D26 | Spikes recomendados? | **Entram no ROADMAP** como tarefas antes das que dependem deles: S-R5-1 (intermed), S-R5-2 (memória pelo `hsperfdata`), S-R5-3 (marcadores da busca do culpado), S-R5-4 (laço de ferramentas do Gemini). |

## 11. Rastreabilidade

| Tela | Tarefas do ROADMAP |
|---|---|
| T01, T21 | P1-13 (cofre ou `.env`: F0-05; Java em Configurações: L-01; IA: D-04; Armazenamento: A-06; Sobre o Warden: F0-06; EULA: L-09) |
| T02, T03, T04 | P1-07 (inclui Apagar pack, P1); coluna Saúde: D-08; Mods iniciais e kits: P1-18; higiene dos arquivos novos: P1-01 |
| T05, T06, T07 | P1-08 (layout do app sem barra lateral e rodapé de Tarefas: F0-06; recarga automática por mudança externa: A-05; trocar versão e dependências órfãs: P1-15); consultas do grafo e "Por que está no pack": D-07; Grafo: D-11; raio-x de mixins: D-10 |
| T08, T09 | P1-09 (página em tela cheia, seleção múltipla e motor da busca combinada com o Modrinth), P1-10 (CurseForge na busca combinada e link), P1-11 (link direto e arquivo), P1-16 (início, categorias e pré-visualização completa), P1-17 (navegar por modpacks), P1-18 (kits na página de descoberta) |
| T10 | P1-12 |
| T11 | P1-08 (Informações do pack), L-04 (Ajustes do teste), L-01 (Java automático e motivo), P1-14 (opcionais, fixar), P1-15 (trocar versão do loader), L-08 (perfis e Quick Play) |
| T12 | C-01, C-02, C-04 (formulário em camadas), C-05 (busca em todas as configs), C-06 (padrões, restaurar e validação de faixa) |
| T13 | L-01, L-02, L-03, L-04, L-05, L-07 (como o jogador recebe), L-08 (perfis), L-10 (RAM e tempo de carregamento), L-11 (memória da JVM), L-12 (perfil de desempenho), D-09 (console agrupado), P1-18 (ferramentas do jogador) |
| T14 | D-01, D-02, D-03, D-04 (IA com ferramentas e conversas), D-05 (índices), D-06 (assinatura e travamentos), D-08 (saúde do pack), D-10 (aviso de mixin), C-06 (aviso de faixa e de config resetada), D-14 (propostas e consultas externas da IA) |
| T15 | C-03 |
| T16, T17 | V-01, V-02 |
| T18 | V-03 (com E-01 para o conteúdo publicado, D-01 e L-04 para os avisos) |
| T19 | E-01, E-02, E-03 (pacote para servidor) |
| T20 | L-06 |
| T22 | F0-05, F0-06 |
| T23 | F0-06 (aviso legal e versão), P1-13 (como seção de Configurações), A-02 (licenças de terceiros) |
| T24 | P1-19 |
| T25 | D-12 (no cliente e modo assistido), D-13 (com o servidor local) |
| T26 | C-07 |
| T27 | L-09 |
