# Warden — Especificação do produto

> Versão do documento: 1.1 (2026-10-01). Tarefa A1; estrutura de navegação, busca combinada, chaves, Java e publicação revistas na tarefa D2 com as decisões do dono de 01/10/2026 (`docs/design/ESTRUTURA.md`, ADR-0025 a ADR-0029).
> Base: relatórios em `docs/research/` (R1 a R4) e as decisões do dono registradas em `docs/decisions/`.
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

**Em palavras simples:** o Warden é um programa para o seu computador (Windows) que serve para montar modpacks de Minecraft. Com ele você escolhe a versão do Minecraft e o carregador de mods (Forge, NeoForge ou Fabric), procura e adiciona mods, resource packs e shaders, ajusta as configurações, abre o jogo para testar com um clique, entende por que o jogo travou, salva versões do pack com um resumo automático do que mudou, publica as versões finais no GitHub para os jogadores receberem as atualizações sozinhos e exporta só o que importa.

O pack que o Warden produz é 100% no formato do **packwiz** (o mesmo usado por packs conhecidos como o Fabulously Optimized). Isso quer dizer que o pack não fica "preso" ao Warden: qualquer ferramenta que entende packwiz consegue usá-lo.

Princípios que guiam todas as decisões:

1. **O pack é sagrado.** A pasta do pack só contém o que é do pack. Nada de cópias de segurança, arquivos temporários, logs ou caches dentro dela (lição dos projetos anteriores, R4 §4.1).
2. **Projeto separado do teste.** O jogo de teste roda numa pasta separada (a "instância de teste"). O que você muda durante o jogo só volta para o pack depois que você revisa e aprova.
3. **Funciona de verdade.** Nada é "pronto" sem ter sido provado contra o packwiz real e, no caso do teste, contra o jogo real.
4. **Sem surpresas.** Toda ação que apaga ou substitui algo pede confirmação e pode ser desfeita pelo histórico.
5. **Português do Brasil, linguagem simples.** Termos técnicos só quando ajudam; sempre com explicação.
6. **Privado.** Nenhum dado sai do computador sem você pedir. Logs só vão para a IA depois que você vê exatamente o que será enviado e aceita.

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
| **Problemas** | Seção do pack com o que o diagnóstico encontrou: o que pode impedir o jogo de abrir. |
| **✦ Diagnóstico com IA** | Seção do pack em que a IA (Gemini) explica um travamento, sempre depois de você ver e aprovar o texto enviado. O ícone de brilhinho marca tudo o que é IA. |
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

## 5. Mapa de telas

Estrutura aprovada pelo dono em 01/10/2026 (ADR-0026; detalhes e justificativas em `docs/design/ESTRUTURA.md`; rascunho clicável em `design/estrutura/index.html`).

```
Primeira execução (T01) ──► Meus packs (T02) ................. nível do APP
                               ├── Criar pack (T03)
                               ├── Abrir pack existente (T04)
                               ├── Configurações (T21), com "Sobre o Warden" (T23)
                               └── [abrir um pack] ──► Pack aberto (T05) ...... nível do PACK
                                     Cabeçalho: ← Meus packs · nome · Editar informações (T11)
                                                · avisos · Salvar versão (T16) · ▶ Testar ▾ (T13)
                                     Menu lateral (6 seções):
                                     ├── Mods (T06): mods, resource packs e shaders
                                     │     ├── Detalhes do item (T07)
                                     │     ├── Adicionar: busca combinada, link, arquivo (T08)
                                     │     │     └── Dependências e conflitos (T09)
                                     │     └── Atualizações (T10)
                                     ├── Configs (T12)
                                     ├── Problemas (T14)
                                     ├── ✦ Diagnóstico com IA (T14)
                                     ├── Histórico (T17), com Publicar versão (T18)
                                     └── Exportar (T19)
                                     Fora do menu:
                                     ├── Teste (T13): etapas, downloads manuais (T20), jogo aberto
                                     │     e resultado, com "O que mudou durante o teste" (T15)
                                     │     e "Por que travou" (T14)
                                     └── ▾ do Testar: ver último teste, ajustes do teste (T11)…
Tarefas em andamento (T22): indicador no rodapé de todas as telas, abre uma gaveta.
```

**Navegação:**
- **Dois níveis que não se misturam.** O nível do app não tem barra lateral: Meus packs é a tela inicial, com Criar pack, Abrir pack existente e Configurações no topo. Itens de um pack só existem depois de abrir um pack.
- **Dentro do pack**, o cabeçalho fixo sempre mostra qual é o pack e tem "← Meus packs". O menu lateral tem 6 seções, cada uma com nome, uma linha dizendo o que tem dentro e um contador quando faz sentido. O pack abre em Mods.
- **Testar** não é seção: é o botão principal do cabeçalho, presente em todas as telas do pack.
- **No máximo um nível de navegação visível.** Nenhuma tela tem abas; filtros, origens e modos de exibição usam caixas de seleção. Detalhes abrem em painel lateral e confirmações em diálogo.
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

**Conteúdo:** uma lista, uma linha por pack, com nome, ícone (se houver), versão do Minecraft, loader e versão, versão do pack, data da última alteração, resultado do último teste ("nunca testado", "abriu normalmente", "travou") e quantidade de alterações não salvas.

**Ações:** Abrir; **Criar pack** (T03); **Abrir pack existente** (T04); **Configurações** (T21), no topo; menu **⋯** de cada pack: Mostrar na pasta, Remover da lista (não apaga arquivos), Apagar pack (P1: move para a Lixeira após digitar o nome do pack).

**Estados:**
- Vazio: "Você ainda não tem packs." com os botões Criar pack e Abrir pack existente.
- Pasta sumiu: linha em destaque "Pasta não encontrada" com **Localizar…** (escolher a nova pasta) e **Remover da lista**.
- Pack inválido (`pack.toml` ilegível): linha "Não foi possível ler este pack" com **Ver detalhes** (erro técnico) e **Mostrar na pasta**.

| CA | Critério |
|---|---|
| CA-T02-01 | Com 50 packs registrados, a lista aparece em menos de 1 s (medido em teste de desempenho). |
| CA-T02-02 | Renomear a pasta de um pack por fora faz a linha do pack mostrar "Pasta não encontrada"; "Localizar…" com a nova pasta restaura a linha sem perder o histórico de testes. |
| CA-T02-03 | "Remover da lista" não altera nenhum arquivo no disco (verificado por hash da pasta antes/depois). |
| CA-T02-04 | O contador de alterações não salvas bate com a quantidade de arquivos alterados desde a última versão salva. |

---

### T03 — Criar pack (P0)

**Objetivo:** criar um pack novo, pronto para receber mods.

**Passos do assistente:**
1. **Nome e pasta:** nome (obrigatório, 1–64 caracteres), autor (padrão: nome configurado), descrição (opcional), pasta de destino (padrão `<pasta dos packs>\<nome-em-minúsculas-com-hífens>`). A pasta precisa não existir ou estar vazia.
2. **Versão do Minecraft:** lista das versões *release*, da mais nova para a mais antiga, ordenada pela ordem oficial da Mojang (nunca por comparação de texto; existem versões como `26.3`). Busca por texto. Versões anteriores a 1.7.10 com etiqueta "melhor esforço". Snapshots não aparecem na v1.
3. **Loader:** Forge, NeoForge, Fabric ou "Nenhum (vanilla)". Só aparecem os loaders que existem para a versão escolhida (NeoForge 1.20.1+; Fabric 1.14+; Forge conforme o repositório oficial). Versão do loader pré-selecionada: Forge "recomendada" (ou a mais recente se não houver recomendada), NeoForge a mais recente estável (sem `-beta`), Fabric a mais recente estável. Lista completa disponível em "Escolher outra versão".
4. **Resumo** e botão **Criar pack**. Depois de criar, o pack abre em Mods, vazio, com **Adicionar mods**.

**O que o Warden cria** (sem usar `packwiz init`, que falha com Forge antigo; R3 §1.5.1):
- `pack.toml` (versão inicial `0.1.0`), `index.toml` vazio, já com o índice atualizado pelo packwiz;
- `.gitattributes` com `* -text` (impede o Windows de mudar finais de linha e quebrar os hashes);
- `.packwizignore` e `.gitignore` padrão do Warden (ARCHITECTURE §6.4);
- `.warden/project.toml` (identificador do pack);
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

---

### T04 — Abrir pack existente (P0)

**Objetivo:** trazer para o Warden um pack packwiz que já existe (por exemplo, criado pelos apps anteriores ou clonado do GitHub).

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

---

### T05 — Pack aberto: cabeçalho e seções (P0)

**Cabeçalho fixo** (em todas as telas do pack):
- **← Meus packs**; nome do pack com **Editar informações** (T11); versão do Minecraft, loader e versão, versão do pack.
- Avisos passageiros: "Mudanças do teste para revisar" (abre o resultado do teste, T15); "O pack mudou desde o início do teste" (T13).
- Botão **Salvar versão · N alterações** (T16); N é a quantidade de alterações não salvas (sem número quando não há nenhuma).
- Botão **▶ Testar** com menu **▾** (T13). Durante a preparação vira "Testando… ver progresso"; com o jogo aberto, "● Jogo aberto: ver teste". Os dois levam à tela do teste.

**Menu lateral** (só existe dentro do pack), com nome, descrição e contador:

| Seção | Descrição na interface | Contador | Tela |
|---|---|---|---|
| Mods | Mods, resource packs e shaders | itens no pack | T06 (com T07–T10) |
| Configs | Arquivos de ajuste dos mods e do jogo | — | T12 |
| Problemas | O que pode impedir o jogo de abrir | problemas encontrados | T14 |
| ✦ Diagnóstico com IA | Pedir à IA para explicar um travamento | — | T14 |
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

**Remover:** diálogo de confirmação que lista o que será removido e avisa "Estes mods dependem dele: …" quando houver. P1: oferece também remover dependências que ficaram sem uso ("órfãs"). O Warden apaga o `.pw.toml` (ou o arquivo local) e atualiza o índice; não usa `packwiz remove` (R4 §2.3).

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

---

### T07 — Detalhes do item (P0)

**Conteúdo:** ícone, nome, autores, link da página (abre no navegador), descrição (Modrinth em Markdown; CurseForge em HTML, ambas higienizadas), informações da versão instalada (número, versões do Minecraft, loaders, data, nome do arquivo, tamanho, hashes), lado (editável), dependências declaradas (obrigatórias, opcionais e incompatíveis, cada uma com estado "no pack" ou "falta"), "Usado por" (itens que dependem deste), changelog da versão instalada.

**Ações:** Atualizar (quando houver), Remover, Abrir página; em **Mais opções**: **Trocar de versão** (P1: escolher qualquer versão compatível, inclusive mais antiga), **Opcional** e **Fixar versão** (P1, ver T11).

**Regras:** dados da CurseForge são buscados na hora e não são guardados em disco (termos da CurseForge, R3 §3.2). Dados do Modrinth ficam no cache local.

| CA | Critério |
|---|---|
| CA-T07-01 | Uma descrição com `<script>` ou `onerror=` não executa código (teste de componente com HTML malicioso). |
| CA-T07-02 | Sem internet, os detalhes de um mod do Modrinth abrem do cache; os de um mod da CurseForge mostram "Detalhes indisponíveis sem internet" e ainda exibem nome, arquivo, lado e hash a partir do `.pw.toml`. |

---

### T08 — Adicionar: busca combinada, link e arquivo (P0)

Página **Adicionar**, dentro da seção Mods (com "← Voltar para Mods"). Sem abas:
- seletor **Tipo** (mods, resource packs ou shaders), que já vem no tipo de onde você clicou;
- um **campo único** que busca pelo nome ou aceita um **link colado** (o Warden reconhece o link pelo formato);
- o botão **Escolher arquivo do computador…** (ou arrastar arquivos para a lista de Mods).

**Busca combinada (Modrinth + CurseForge; ADR-0027):**
- Uma busca consulta as duas fontes ao mesmo tempo e mostra **um resultado só**. Cada item mostra a fonte: "Modrinth", "CurseForge" ou "Modrinth e CurseForge".
- **Sem duplicatas:** o mesmo mod nas duas fontes vira um item só. Na lista, são o mesmo mod quando autor e nome (ou slug) coincidem nas duas fontes. Na pré-visualização, o Warden confere o arquivo da versão escolhida pelo SHA-1 nas duas fontes e avisa se forem diferentes. O item unificado tem o seletor **Fonte**; o padrão é o Modrinth (informa o lado e tem cache local).
- **Ordem:** relevância intercalada pela posição em cada fonte, sem favorecer nenhuma; ordenar por downloads, atualização recente ou novos usa os valores de cada fonte. O filtro **Fonte** (todas, só Modrinth, só CurseForge) fica em "Mais filtros".
- Filtros já aplicados e travados por padrão: versão do Minecraft do pack, loader do pack (mods) e tipo de projeto. "Mostrar também os sem versão compatível" desligado por padrão.
- Filtros extras (em "Mais filtros"): fonte, categorias, ambiente (cliente/servidor), ordenar por relevância, downloads, atualizados recentemente, novos.
- Resultado: ícone, nome, autor, resumo, downloads, data da última atualização, fonte, marca "Já no pack" e, na CurseForge, marca "Download manual necessário" quando o autor bloqueou downloads por apps de terceiros.
- Rolagem infinita, 20 itens por página (já combinados).
- Clicar abre a pré-visualização (mesmo layout de T07) com seletor de fonte (quando houver duas) e de versão. Versão padrão: a mais nova compatível do canal configurado (padrão: só versões estáveis; beta/alpha só com a opção em Configurações).
- **Adicionar ao pack** abre a tela de dependências (T09). Depois de adicionar, a busca continua aberta e o item passa a mostrar "Já no pack".
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

| CA | Critério |
|---|---|
| CA-T09-01 | Adicionar um mod Fabric que exige Fabric API num pack sem ela lista a Fabric API como obrigatória e marcada; confirmar grava os dois `.pw.toml` e atualiza o índice uma vez. |
| CA-T09-02 | Num pack NeoForge 1.21.1 com Sodium, adicionar o Embeddium mostra a incompatibilidade declarada pela API do Modrinth. |
| CA-T09-03 | Falha de rede no meio da gravação (simulada) não deixa nenhum `.pw.toml` novo no pack. |
| CA-T09-04 | Adicionar pelo Modrinth um mod que já está no pack pela CurseForge (detectado por projeto equivalente ou hash do arquivo) mostra a opção de substituir. |

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

| CA | Critério |
|---|---|
| CA-T11-01 | Mudar o nome do pack altera só a linha `name` do `pack.toml` (diff de uma linha) e o índice continua válido. |
| CA-T11-02 | Marcar um mod como opcional grava `[option] optional = true` com `description` e `default`; o packwiz-installer real, em modo sem interface, instala ou não o mod conforme `default` (teste de conformidade com `default = true` e `false`), e a instância do Warden faz o mesmo. |
| CA-T11-03 | Para um pack Forge 1.20.1, Ajustes do teste mostra "Automático: Java 17" e o motivo de não usar o Java mais novo; para um pack Forge 1.7.10, "Automático: Java 8" com o motivo do Forge antigo; para a versão mais nova do Minecraft, o Java mais novo, sem explicação extra (teste de componente com a política de L-01). |

---

### T12 — Configs (P0 editor de texto; P1 formulário)

**Objetivo:** editar as configurações do pack e, durante os testes, as da instância.

**Árvore de arquivos à esquerda** com o seletor de origem "Mostrando: arquivos do pack / instância de teste" (caixa de seleção, não abas):
- **Pack:** `config/`, `defaultconfigs/`, `kubejs/`, `scripts/`, `options.txt` e demais arquivos de texto do pack.
- **Instância de teste** (quando existir): `config/`, `defaultconfigs/`, `options.txt` e `saves/<mundo>/serverconfig/`. Edições aqui valem só para o teste e aparecem depois em "O que mudou durante o teste" (T15). Aberta pelo botão **Editar configs do teste** (T13), já nesta origem, com aviso fixo e **Voltar ao teste**.

Busca por nome de arquivo. Arquivos binários (ex.: `servers.dat`) aparecem sem edição na v1.

**Editor de texto (P0):** realce de sintaxe por extensão (TOML, JSON, JSON5, YAML, `.properties`, `.cfg`, JavaScript, texto), números de linha, buscar e substituir, desfazer. Arquivos acima de 2 MB abrem só para leitura com aviso.

**Salvar (P0):**
- Ctrl+S ou botão. Antes de gravar, mostra as diferenças (pode ser desligado em Configurações).
- Se o arquivo mudou no disco desde que foi aberto: "Este arquivo foi alterado fora do Warden." com **Recarregar** (descarta suas mudanças), **Ver diferenças** e **Sobrescrever**.
- Sair do arquivo com alterações não salvas pede confirmação.
- A gravação preserva tudo o que não foi editado (comentários, ordem, espaços), byte a byte.

**Formulário (P1):** para TOML do Forge/NeoForge, JSON/JSONC, JSON5, `.properties`, `.cfg` do Forge antigo e `options.txt`. Mostra as chaves por seção, com os comentários do arquivo como ajuda, faixas e valores permitidos quando o arquivo informa (`#Range`, `#Allowed Values`), e controles por tipo (liga/desliga, número com limite, lista de opções, texto, lista). Só os valores alterados são gravados. Alternar entre formulário e texto a qualquer momento (dois modos de ver o mesmo arquivo).

**Avisos contextuais:**
- Arquivo `*-server.toml` em `config/` (Forge/NeoForge 1.13+): "Este tipo de config vale por mundo. Para valer em mundos novos, ele precisa estar em `defaultconfigs/`." (P0) com **Copiar para defaultconfigs/** (P1).
- Arquivos do Forge/NeoForge: "O jogo pode reescrever este arquivo e apagar comentários que você adicionar."
- `options.txt` no pack: "Este arquivo substitui as preferências de quem já joga o pack." com a opção **Não substituir se o jogador já tiver** (grava `preserve = true` no índice; P1).

| CA | Critério |
|---|---|
| CA-T12-01 | Abrir e salvar sem mudanças qualquer arquivo do corpus de configs reais produz bytes idênticos (teste de ida e volta). |
| CA-T12-02 | Mudar um valor pelo formulário num TOML do Forge com comentários altera exatamente uma linha. |
| CA-T12-03 | Editar o arquivo por fora com o editor aberto e salvar no Warden mostra o aviso de alteração externa; nada é gravado sem escolha do usuário. |
| CA-T12-04 | Editar um config da instância durante o teste faz o arquivo aparecer em T15 ao fim do teste. |
| CA-T12-05 | Nenhum arquivo `.bak` ou temporário fica na pasta do pack depois de salvar (inclusive se o app for encerrado à força durante a gravação; teste de falha). |

---

### T13 — Testar (launcher) e console (P0)

**Objetivo:** abrir o Minecraft com o pack em um clique e acompanhar o que acontece.

**Onde:** o botão **▶ Testar** fica no cabeçalho de todas as telas do pack (T05). Ao clicar, a área principal vira a **tela do teste**, que não é uma seção do menu: dá para ir a outras seções com o jogo aberto e voltar pelo mesmo botão ("● Jogo aberto: ver teste").

**Menu ▾ do Testar:** **Ver último teste** (resultado, console e o que mudou); **Testar como o jogador recebe** (P1); **Ajustes do teste neste computador…** (T11); **Abrir pasta da instância de teste**; **Apagar mundos de teste…** (decisão D6); **Recriar instância de teste…**.

**Fluxo ao clicar em Testar** (indicador de etapas sempre visível):
1. **Verificar o pack (rápido):** roda a passagem rápida do diagnóstico pré-teste (T14), que usa o pack e os dados das APIs. Com erros: diálogo listando os erros, com **Corrigir** (quando houver correção automática), **Testar mesmo assim** e **Cancelar**. Só avisos: segue, com os avisos visíveis na aba Diagnóstico.
2. **Preparar o Minecraft:** baixa a versão do jogo, instala o loader e escolhe/baixa o Java (o mais novo que funciona, T11; ARCHITECTURE §7). Mostra progresso (arquivos e megabytes). Na primeira vez pode levar alguns minutos; o texto avisa isso.
3. **Copiar o pack para o teste** (sincronização): copia/baixa para a instância só o que mudou; remove o que saiu do pack; nunca toca em `saves/`, `screenshots/` e afins. Mods da CurseForge com download bloqueado abrem T20.
4. **Verificação final:** passagem completa do diagnóstico, que lê os próprios arquivos `.jar` (dependências, duplicatas por ID, versão do Java exigida). Erros abrem o mesmo diálogo do passo 1.
5. **Abrir o jogo** com o perfil offline (nome configurado).
6. **Jogo em execução:** console ao vivo; botões **Parar jogo** (confirmação: "O progresso não salvo do mundo pode ser perdido."), **Abrir pasta da instância**, **Editar configs do teste** (abre T12 na origem Instância).
7. **Jogo fechou:** a mesma tela mostra o resultado, com duração ("Fechado normalmente", "Travou" ou "Encerrado por você"), **Ver console** e **Testar de novo**, e logo abaixo:
   - **O que mudou durante o teste** (T15), quando houver mudanças;
   - se travou: **Por que travou** (diagnóstico pós-crash, T14), com a correção sugerida e, quando a causa não aparecer, **✦ Pedir ajuda à IA**.

**Console:**
- Linhas coloridas por nível (erro, aviso, info, debug), com horário e origem (jogo, Warden).
- Filtros por nível, busca, pausar a rolagem automática, copiar seleção, copiar tudo, salvar em arquivo.
- Mostra até 50.000 linhas na tela; tudo fica gravado no log da sessão.
- Sem tradução das mensagens do jogo (são mostradas como saíram).

**Memória automática:** pack com menos de 100 mods: 4 GB; 100–199: 6 GB; 200 ou mais: 8 GB; nunca acima de 60% da memória do computador; aviso acima de 8 GB em Java 8. Valor manual em Ajustes (T11).

**Regras:**
- Um jogo por vez no Warden inteiro (v1). Clicar em Testar com outro jogo aberto: "Já existe um jogo em execução (pack X)." com **Ir para o teste**.
- Fechar o Warden com o jogo aberto pede confirmação e encerra o jogo.
- Sem internet e com tudo já baixado, o teste funciona. Se faltar algo, a mensagem lista exatamente o que falta.
- Editar o pack com o jogo aberto é permitido; o cabeçalho avisa "O pack mudou desde o início do teste; as mudanças valem no próximo teste."
- Os mundos de teste ficam na instância e nunca vão para o pack.

**Testar como o jogador recebe (P1):** cria uma instância temporária limpa a partir do pack exportado (sem mundos, sem configs da instância de trabalho), abre o jogo e apaga a instância temporária ao final (mantendo os logs).

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

---

### T14 — Diagnóstico (P0 determinístico; P1 IA)

**Objetivo:** encontrar problemas antes de abrir o jogo e explicar por que ele travou.

**Camada determinística (P0)** — regras fixas, sem IA, sempre com a evidência que as sustenta:
- **Antes do teste**, em duas passagens — rápida (antes de baixar, com dados do pack e das APIs) e completa (depois de copiar o pack para o teste, lendo os `.jar`) — e também sob demanda pelo botão "Verificar agora" da seção Problemas: loader errado, versão do Minecraft incompatível, versão do loader fora da faixa exigida, dependência obrigatória ausente ou em versão errada, incompatibilidades declaradas, mod duplicado (mesmo ID, mesmo arquivo ou mesmo projeto de duas fontes), dois mods da mesma "categoria exclusiva" (ex.: dois renderizadores), mod só de cliente marcado como "cliente e servidor", Java incompatível, versões beta/alpha, conflitos conhecidos da lista curada, mods obsoletos. Lista de regras em ARCHITECTURE §9.
- **Depois de um travamento:** lê o resultado do processo, a saída capturada, `logs/latest.log`, `logs/debug.log`, os crash reports novos e os `hs_err_pid*.log` novos, e aplica o catálogo de padrões (dependência faltando, duplicado, falha de Mixin, Java errado, falta de memória, driver de vídeo, config corrompida, mod de cliente no servidor, entre outros).

**Apresentação:**
- **Seção Problemas** (T05): achados do pré-teste agrupados por gravidade, com contador no menu; no fim, "Último travamento" com **Ver o que causou** e **✦ Analisar com IA**. Os achados de um travamento aparecem no resultado do teste ("Por que travou", T13). Itens com problema também ganham a marca na lista de Mods (T06).
- Achados agrupados por gravidade: **Erro** (bloqueia o teste por padrão), **Aviso**, **Informação**.
- Cada achado: título em linguagem simples, explicação, itens envolvidos (com link para o item), evidência ("O arquivo fabric.mod.json do Sodium declara incompatibilidade com o OptiFine" ou o trecho do log com número da linha) e, quando houver, botões de correção (**Adicionar dependência X**, **Remover Y**, **Mudar lado para Só cliente**, **Atualizar Z**). Correções passam pelos fluxos normais, com confirmação.
- "Ignorar este aviso neste pack" (P1; gravado em `.warden/`).
- Sem conclusão: "Não encontramos a causa automaticamente." com **Pedir ajuda à IA** (P1) e **Abrir crash report**.

**✦ Diagnóstico com IA (P1; Gemini):**
- **Onde:** seção própria do pack, **✦ Diagnóstico com IA** (decisão do dono, ADR-0026), e o botão **✦ Pedir ajuda à IA** no resultado de um travamento. O ícone de brilhinho marca tudo o que é IA, e só isso.
- **Na seção:** escolher o que analisar: o **último travamento** (padrão), **outro teste** (sessões guardadas na instância) ou **um log ou crash report do computador** (arquivo de texto escolhido pelo diálogo do sistema). Campo opcional "Contar algo à IA" (ex.: "trava quando entro no Nether"). **✦ Preparar envio** abre o consentimento.
- **Respostas anteriores:** lista com data, o que foi analisado, causa provável e confiança, com **Ver resposta** e apagar. Ficam nos dados do Warden, por pack, nunca na pasta do pack.
- Situação da chave do Gemini na própria seção, com link para Configurações.

1. **Consentimento**, sempre (não existe "não perguntar de novo"):
   - mostra o texto **exato** que será enviado, já sem dados pessoais (nome de usuário do Windows nos caminhos, nome do jogador, UUID, IPs, e-mails, nome do computador, qualquer coisa com cara de chave/token), rolável;
   - informa tamanho, provedor (Google Gemini), modelo, que o uso é cobrado/limitado na sua chave e que, no plano gratuito, o Google pode usar o conteúdo enviado para melhorar seus produtos;
   - botões **Enviar** e **Cancelar**.
2. **Resposta da IA** (tela própria): causa provável, confiança (baixa/média/alta), passos sugeridos, mods envolvidos (com link para o item). Correções sugeridas passam pelos fluxos normais, com confirmação. Aviso fixo: "A IA pode errar. Confira antes de mudar o pack."
3. Sem chave do Gemini: "Para usar a IA, informe sua chave do Gemini em Configurações." com **Abrir Configurações**.

| CA | Critério |
|---|---|
| CA-T14-01 | Cada pack de teste propositalmente quebrado (dependência faltando, jar Fabric no Forge, mod duplicado, mod que exige Java 21 em pack de Java 17, OptiFine com Sodium) gera o achado esperado antes do teste, com evidência. |
| CA-T14-02 | Cada log real do corpus de crashes (`crates/warden-diagnostics/tests/corpus/`) gera o diagnóstico esperado, com a linha de evidência correta. |
| CA-T14-03 | O texto mostrado no consentimento é byte a byte o que é enviado (teste com servidor simulado). |
| CA-T14-04 | Num log contendo `C:\Users\Maria\AppData`, o nome do jogador, um IPv4 e uma string `$2a$10$...`, nenhum desses valores aparece no texto enviado. |
| CA-T14-05 | Cancelar o consentimento não faz nenhuma requisição de rede. |
| CA-T14-06 | Analisar um log escolhido no computador passa pela mesma redação e pelo mesmo consentimento (texto mostrado = texto enviado); a resposta aparece em "Respostas anteriores" depois de reiniciar o app, e nenhum arquivo é criado ou alterado na pasta do pack (teste com servidor simulado e hash da pasta antes/depois). |

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

Sempre ignorados: `logs/`, `crash-reports/`, `saves/` (exceto serverconfig acima), `screenshots/`, caches de mods e do loader, dados de minimapa, `usercache.json` e a lista completa em ARCHITECTURE §8.4.

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
- **Como os jogadores instalam (Prism Launcher ou MultiMC)**, com **Copiar instruções**: (1) criar uma instância com a versão do Minecraft e o loader do pack; (2) baixar o `packwiz-installer-bootstrap.jar` e colocar na pasta `.minecraft` da instância; (3) em Editar instância → Configurações → Comandos personalizados, colar no comando antes de iniciar `"$INST_JAVA" -jar packwiz-installer-bootstrap.jar <link do pack>`; (4) abrir o jogo: o pack é baixado agora e atualizado sozinho nas próximas vezes;
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

**Formatos:** **Pasta packwiz** (para hospedar em outro lugar) ou **Arquivo .zip do pack packwiz** (P0). Também na v1 (P1, decisão D1): **`.mrpack`** (app do Modrinth e launchers compatíveis) e **.zip da CurseForge** (app da CurseForge), gerados pelo packwiz (`modrinth export` / `curseforge export`) sobre uma cópia limpa do pack e validados depois de gerados.

**Formatos de outros launchers (P1):**
- Antes de gerar, o Warden mostra em linguagem simples o que se perde nesse formato (ex.: a CurseForge não guarda o lado cliente/servidor; o `.mrpack` não guarda descrição de opcionais) — ver R3 §2.3.
- `.mrpack` com mod da CurseForge: se o mesmo arquivo existir no Modrinth (mesmo hash), o Warden oferece trocar a fonte; se não existir e o mod bloquear distribuição por terceiros, a exportação para e explica o motivo; se não bloquear, avisa que o arquivo será embutido e pede confirmação (licença).
- Zip da CurseForge com mod que não é da CurseForge: aviso de que a CurseForge exige aprovação manual de arquivos de fora; pede confirmação.
- `version` do `pack.toml` é obrigatória (o `.mrpack` exige); se faltar, o Warden pede antes de exportar.

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
| CA-T19-04 | (P1) O `.mrpack` gerado abre como zip válido, contém `modrinth.index.json` válido pelo esquema do formato, `overrides/` só com arquivos do índice, e é importável pelo Modrinth App ou por um launcher compatível (teste de integração + verificação manual no marco). |
| CA-T19-05 | (P1) O zip da CurseForge gerado contém `manifest.json` válido, `overrides/` só com arquivos do índice, e nenhum jar de mod de terceiros sem confirmação explícita (teste com pack contendo mod da CurseForge, do Modrinth e local). |

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
| Teste | Memória padrão (Automático/fixo); **Java**: tabela com cada Java instalado pelo Warden, os packs que o usam e por que aquela versão (o mais novo que funciona; ADR-0029), com **Procurar atualizações do Java** e **Remover Javas sem uso**; mostrar versões beta/alpha dos mods; verificação automática de atualizações (intervalo). |
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

## 7. Regras que valem para o app inteiro

1. **A pasta do pack só recebe conteúdo do pack.** Nenhum `.bak`, temporário, cache, log, jar auxiliar ou configuração do app. Gravações usam arquivo temporário com sufixo `.warden-tmp` na mesma pasta seguido de renomeação; esse sufixo está no `.packwizignore` e sobras dele são apagadas ao abrir o pack.
2. **Confirmação e reversão.** Ação que apaga ou substitui conteúdo do pack pede confirmação nomeando o que será afetado, e cria ponto de segurança nos casos listados em ARCHITECTURE §11.
3. **Erros explicados.** Toda mensagem de erro diz o que aconteceu e o que fazer, em pt-BR, com **Detalhes técnicos** recolhível (código do erro e saída da ferramenta) e **Copiar**.
4. **Sem internet.** Tudo o que não depende de rede continua funcionando. O que depende mostra "Sem conexão com a internet" e **Tentar de novo**; nunca fica carregando para sempre (limite de tempo em toda requisição).
5. **Nada de botão morto.** Toda ação visível funciona ou está desabilitada com dica explicando por quê.
6. **Teclado e acessibilidade.** Toda ação acessível por teclado, foco visível, contraste AA, textos alternativos em ícones com função.
7. **Uma janela.** Só uma cópia do Warden aberta por vez; abrir de novo foca a janela existente.
8. **Privacidade.** Nada é enviado a terceiros além das chamadas necessárias às APIs (Mojang, Modrinth, CurseForge, Adoptium, Maven dos loaders, GitHub e, com consentimento, Gemini). Sem telemetria.
9. **Linguagem.** Português do Brasil, "você", frases curtas, sem gírias técnicas não explicadas (QUALITY §8).

## 8. Requisitos não funcionais

| Requisito | Meta |
|---|---|
| Abrir o app até a Lista de packs | < 3 s em máquina de referência (Windows 11, SSD, 16 GB). |
| Abrir um pack de 300 mods (com cache) | < 1 s até a lista visível. |
| Busca (Modrinth) | < 2 s em conexão comum; resultados parciais nunca travam a interface. |
| Atualizar índice (300 mods + 500 configs) | < 5 s. |
| Diagnóstico pré-teste (300 mods, cache quente) | < 3 s. |
| Interface | Nunca congela; trabalho pesado sempre fora da thread da interface. |
| Memória do app (sem jogo) | < 500 MB. |
| Robustez | Encerrar o app à força em qualquer momento não corrompe o pack (gravações atômicas, testes de falha). |
| Registros | Arquivos diários, guardados por 14 dias, sem chaves nem tokens. |

## 9. Fora da v1

- Quilt; Legacy Fabric para 1.7.10/1.12.2 (pode vir depois); versões anteriores a 1.7.10 além do "melhor esforço"; snapshots do Minecraft.
- Login Microsoft/Mojang, contas, skins, multiplayer online.
- Teste com servidor dedicado (P2), teste com mundo antigo (P2), "entrar direto no mundo" (Quick Play, P2).
- Publicar direto no Modrinth/CurseForge (upload pela API).
- Importar `.mrpack` ou zip da CurseForge (P2).
- Várias variantes do mesmo pack (versões do Minecraft diferentes no mesmo projeto) e assistente de migração de versão do Minecraft (P2).
- Editor de `servers.dat`, formulário para YAML, detecção de conflitos de teclas, sugestão de kits de performance, modelos de pack, datapacks globais (todos P2).
- Painel git completo (branches, merges), trazer mudanças do GitHub (P2), sincronização entre computadores além do GitHub.
- Atualização automática do Warden; macOS; empacotamento oficial para Linux (o app compila e roda em Linux para desenvolvimento).
- Temas além de claro/escuro do sistema; outros idiomas.

## 10. Decisões do dono

Respondidas pelo dono em 01/10/2026. O dono aceitou as recomendações, com exceção de D1. D10 a D14 vieram da revisão da estrutura (tarefa D2), no mesmo dia; D14 alterou D2.

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

## 11. Rastreabilidade

| Tela | Tarefas do ROADMAP |
|---|---|
| T01, T21 | P1-13 (cofre ou `.env`: F0-05; Java em Configurações: L-01; IA: D-04; Armazenamento: A-06; Sobre o Warden: F0-06) |
| T02, T03, T04 | P1-07 (inclui Apagar pack, P1) |
| T05, T06, T07 | P1-08 (layout do app sem barra lateral e rodapé de Tarefas: F0-06; recarga automática por mudança externa: A-05; trocar versão e dependências órfãs: P1-15) |
| T08, T09 | P1-09 (página Adicionar e motor da busca combinada com o Modrinth), P1-10 (CurseForge na busca combinada e link), P1-11 (link direto e arquivo) |
| T10 | P1-12 |
| T11 | P1-08 (Informações do pack), L-04 (Ajustes do teste), L-01 (Java automático e motivo), P1-14 (opcionais, fixar), P1-15 (trocar versão do loader) |
| T12 | C-01, C-02, C-04 |
| T13 | L-01, L-02, L-03, L-04, L-05, L-07 |
| T14 | D-01, D-02, D-03, D-04 |
| T15 | C-03 |
| T16, T17 | V-01, V-02 |
| T18 | V-03 (com E-01 para o conteúdo publicado, D-01 e L-04 para os avisos) |
| T19 | E-01, E-02 |
| T20 | L-06 |
| T22 | F0-05, F0-06 |
| T23 | F0-06 (aviso legal e versão), P1-13 (como seção de Configurações), A-02 (licenças de terceiros) |
