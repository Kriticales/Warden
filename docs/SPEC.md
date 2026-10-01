# Warden — Especificação do produto

> Versão do documento: 1.0 (2026-10-01). Tarefa A1.
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
10. [Decisões pendentes do dono](#10-decisões-pendentes-do-dono)
11. [Rastreabilidade](#11-rastreabilidade)

---

## 1. O que é o Warden

**Em palavras simples:** o Warden é um programa para o seu computador (Windows) que serve para montar modpacks de Minecraft. Com ele você escolhe a versão do Minecraft e o carregador de mods (Forge, NeoForge ou Fabric), procura e adiciona mods, resource packs e shaders, ajusta as configurações, abre o jogo para testar com um clique, entende por que o jogo travou, guarda versões do pack com um resumo automático do que mudou e exporta só o que importa.

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
| **Salvar versão** | Guardar o estado atual do pack no histórico, com número de versão e resumo das mudanças. |
| **Diagnóstico** | Verificação automática que procura problemas antes do teste e explica travamentos depois. |

## 3. Conceitos centrais

### 3.1 Pack (projeto)

- Uma pasta no disco, por padrão `Documentos\Warden\<nome-do-pack>\`, escolhida pelo usuário.
- Contém os arquivos do packwiz (`pack.toml`, `index.toml`, `mods/*.pw.toml` etc.), as configs do pack, os arquivos de controle (`.gitignore`, `.gitattributes`, `.packwizignore`), o `CHANGELOG.md` e a pasta `.warden/` com informações do Warden sobre o pack (identificador, preferências do pack). A pasta `.warden/` e o `CHANGELOG.md` **não** entram no pack distribuído (ficam fora do índice do packwiz).
- É um repositório git. O usuário nunca vê git; vê "versões" e "histórico".
- Tem exatamente **uma** versão do Minecraft e **um** loader (ou nenhum, para packs vanilla de resource packs/shaders).

### 3.2 Instância de teste

- Uma pasta por pack, em dados do Warden (`%LOCALAPPDATA%\dev.kriticales.warden\instances\<id>\minecraft\`).
- Montada a partir do pack a cada teste: o Warden copia/baixa só o que mudou desde o último teste e remove o que saiu do pack.
- Guarda os mundos de teste (`saves/`), que **nunca** vão para o pack.
- O usuário pode abrir a pasta, editar configs dela (pelo editor do Warden ou por fora) e, ao fim do teste, revisar o que mudou para trazer ao pack.

### 3.3 Versão salva

- Um ponto do histórico com número SemVer, data e changelog.
- Criada só pelo botão "Salvar versão". Mudanças ainda não salvas aparecem como "Alterações não salvas".
- "Voltar para uma versão" nunca apaga o histórico; o estado anterior fica guardado num ponto de segurança.

### 3.4 Pontos de segurança

- Antes de qualquer ação que substitui ou apaga conteúdo do pack (voltar versão, limpeza de pack antigo, atualizar todos os mods), o Warden guarda automaticamente o estado atual num ponto de segurança escondido. O usuário pode recuperá-lo pela tela de Histórico (P1; na v1 P0 o ponto existe e é recuperável pelo orquestrador/suporte).

## 4. Prioridades e escopo

| Prioridade | Significado |
|---|---|
| **P0** | Obrigatório na v1. Sem isso o Warden não cumpre o objetivo. |
| **P1** | Entra na v1, construído logo depois do P0 correspondente. |
| **P2** | Depois da v1. Está descrito para orientar o desenho, mas não é construído agora. |
| **Fora da v1** | Não planejado (§9). |

Suporte de versões (ADR-0005): **garantido de 1.7.10 até a versão mais nova do Minecraft** desde a primeira versão do app, com Forge (todas as versões, inclusive 1.7.10 e 1.12.2), NeoForge (1.20.1+) e Fabric (1.14+). Versões anteriores a 1.7.10 aparecem com a etiqueta "melhor esforço": podem ser criadas e testadas, mas não fazem parte dos testes automáticos e falhas nelas não bloqueiam entregas.

## 5. Mapa de telas

```
Primeira execução (T01) ──► Lista de packs (T02)
                               ├── Criar pack (T03)
                               ├── Abrir pack existente (T04)
                               └── Editor do pack (T05)
                                     ├── Mods / Resource packs / Shaders (T06)
                                     │     ├── Detalhes do item (T07)
                                     │     ├── Adicionar: busca, link, arquivo (T08)
                                     │     │     └── Dependências e conflitos (T09)
                                     │     └── Atualizações (T10)
                                     ├── Configs (T12)
                                     ├── Testar + console (T13)
                                     │     ├── Diagnóstico (T14)
                                     │     ├── Revisar mudanças do teste (T15)
                                     │     └── Downloads manuais da CurseForge (T20)
                                     ├── Histórico + Salvar versão (T16, T17)
                                     │     └── Enviar ao GitHub (T18)
                                     ├── Exportar (T19)
                                     └── Ajustes do pack (T11)
Configurações do app (T21)     Tarefas em andamento (T22)     Sobre (T23)
```

Navegação: barra lateral fixa com **Packs**, **Tarefas**, **Configurações** e **Sobre**. Dentro de um pack, um cabeçalho fixo (nome, versão do Minecraft, loader, versão do pack, avisos) com os botões principais **Testar** e **Salvar versão**, e abas: Mods, Resource packs, Shaders, Configs, Teste, Diagnóstico, Histórico, Exportar, Ajustes.

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
4. Chaves opcionais (CurseForge, Gemini, GitHub), com "Pular, configuro depois".

**Regras:** o assistente só aparece se não existir `settings.json`. Pode ser reaberto em Configurações → "Rever boas-vindas".

| CA | Critério |
|---|---|
| CA-T01-01 | Com o diretório de configuração vazio, abrir o app mostra a primeira execução; ao concluir, `settings.json` existe e a Lista de packs aparece. |
| CA-T01-02 | O nome `Zé` é recusado com mensagem explicando a regra; `Ze_123` é aceito. |
| CA-T01-03 | Pular as chaves não impede concluir; a busca da CurseForge mostra depois o estado "chave não configurada" (T08). |
| CA-T01-04 | O aviso legal aparece também em Sobre (T23). |

---

### T02 — Lista de packs (P0)

**Objetivo:** ver e abrir os seus packs.

**Conteúdo:** um cartão por pack com nome, ícone (se houver), versão do Minecraft, loader e versão, versão do pack, data da última alteração, resultado do último teste ("nunca testado", "abriu normalmente", "travou") e quantidade de alterações não salvas.

**Ações:** Abrir; **Criar pack** (T03); **Abrir pack existente** (T04); menu do cartão: Mostrar na pasta, Remover da lista (não apaga arquivos), Apagar pack (P1: move para a Lixeira após digitar o nome do pack).

**Estados:**
- Vazio: "Você ainda não tem packs." com os botões Criar pack e Abrir pack existente.
- Pasta sumiu: cartão em destaque "Pasta não encontrada" com **Localizar…** (escolher a nova pasta) e **Remover da lista**.
- Pack inválido (`pack.toml` ilegível): cartão "Não foi possível ler este pack" com **Ver detalhes** (erro técnico) e **Mostrar na pasta**.

| CA | Critério |
|---|---|
| CA-T02-01 | Com 50 packs registrados, a lista aparece em menos de 1 s (medido em teste de desempenho). |
| CA-T02-02 | Renomear a pasta de um pack por fora faz o cartão mostrar "Pasta não encontrada"; "Localizar…" com a nova pasta restaura o cartão sem perder o histórico de testes. |
| CA-T02-03 | "Remover da lista" não altera nenhum arquivo no disco (verificado por hash da pasta antes/depois). |
| CA-T02-04 | O contador de alterações não salvas bate com a quantidade de arquivos alterados desde a última versão salva. |

---

### T03 — Criar pack (P0)

**Objetivo:** criar um pack novo, pronto para receber mods.

**Passos do assistente:**
1. **Nome e pasta:** nome (obrigatório, 1–64 caracteres), autor (padrão: nome configurado), descrição (opcional), pasta de destino (padrão `<pasta dos packs>\<nome-em-minúsculas-com-hífens>`). A pasta precisa não existir ou estar vazia.
2. **Versão do Minecraft:** lista das versões *release*, da mais nova para a mais antiga, ordenada pela ordem oficial da Mojang (nunca por comparação de texto; existem versões como `26.3`). Busca por texto. Versões anteriores a 1.7.10 com etiqueta "melhor esforço". Snapshots não aparecem na v1.
3. **Loader:** Forge, NeoForge, Fabric ou "Nenhum (vanilla)". Só aparecem os loaders que existem para a versão escolhida (NeoForge 1.20.1+; Fabric 1.14+; Forge conforme o repositório oficial). Versão do loader pré-selecionada: Forge "recomendada" (ou a mais recente se não houver recomendada), NeoForge a mais recente estável (sem `-beta`), Fabric a mais recente estável. Lista completa disponível em "Escolher outra versão".
4. **Resumo** e botão **Criar pack**.

**O que o Warden cria** (sem usar `packwiz init`, que falha com Forge antigo; R3 §1.5.1):
- `pack.toml` (versão inicial `0.1.0`), `index.toml` vazio, já com o índice atualizado pelo packwiz;
- `.gitattributes` com `* -text` (impede o Windows de mudar finais de linha e quebrar os hashes);
- `.packwizignore` e `.gitignore` padrão do Warden (ARCHITECTURE §6.4);
- `.warden/project.toml` (identificador do pack e preferências);
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
3. **Verificação de higiene** (P0): procura no `index.toml` e na pasta arquivos que não deveriam estar no pack (`*.bak`, `*.tmp`, `*.log`, `packwiz-installer-bootstrap.jar`, `.packwiz.toml`, `logs/`, `crash-reports/`, `saves/`, caches de mods etc.; lista em ARCHITECTURE §6.4). Se encontrar, mostra "Encontramos N arquivos que não deveriam ir para quem joga o pack" com a lista (caminho, tamanho, motivo) e caixas marcadas. **Limpar** cria um ponto de segurança, apaga os marcados, adiciona as regras faltantes ao `.packwizignore` e atualiza o índice. **Agora não** abre o pack mesmo assim (o aviso reaparece em Exportar).
4. Arquivos de controle faltando ou diferentes do padrão (`.gitattributes`, `.packwizignore`): oferece adicionar, mostrando a diferença antes.

| CA | Critério |
|---|---|
| CA-T04-01 | Abrir um pack com `config/x.toml.bak`, `packwiz-installer-bootstrap.jar` e `.packwiz.toml` indexados lista os três; "Limpar" remove-os do disco e do `index.toml`, e o `packwiz refresh` seguinte não os reinclui. |
| CA-T04-02 | Abrir um pack Quilt mostra a mensagem de não suportado e não cria `.warden/` nem altera nada na pasta. |
| CA-T04-03 | Abrir uma cópia de um pack já registrado gera identificador novo; os dois aparecem na lista com instâncias de teste separadas. |
| CA-T04-04 | "Agora não" não altera nenhum arquivo além de criar `.warden/project.toml` e o repositório git, se faltarem. |

---

### T05 — Editor do pack: cabeçalho e abas (P0)

**Cabeçalho fixo:**
- Nome do pack, versão do Minecraft, loader e versão, versão do pack.
- Indicadores: "N alterações não salvas" (abre Histórico), "Mudanças do teste para revisar" (abre T15), "N problemas" (abre Diagnóstico), "Jogo em execução" (abre Teste).
- Botões **Testar** (T13) e **Salvar versão** (T16).

**Abas:** Mods, Resource packs, Shaders (T06), Configs (T12), Teste (T13), Diagnóstico (T14), Histórico (T17), Exportar (T19), Ajustes (T11).

**Regras:**
- O estado mostrado vem sempre do disco. Se a pasta do pack mudar por fora (editor externo, `git pull` manual), o Warden recarrega a aba em até 2 s e avisa com uma notificação discreta (P1; no P0 o Warden relê o disco ao abrir cada aba e antes de qualquer escrita).
- Toda escrita no pack passa por uma trava do pack: duas operações de escrita no mesmo pack nunca rodam ao mesmo tempo; a segunda espera na fila e aparece em Tarefas (T22).

| CA | Critério |
|---|---|
| CA-T05-01 | Editar um config por fora e voltar ao Warden mostra o conteúdo novo sem reiniciar o app. |
| CA-T05-02 | Disparar "Atualizar todos" e "Remover mod" em sequência rápida resulta nas duas operações executadas em ordem, sem corromper `index.toml` (teste de concorrência). |

---

### T06 — Mods, Resource packs e Shaders (P0)

As três abas têm o mesmo comportamento; muda só o tipo de conteúdo e a pasta (`mods/`, `resourcepacks/`, `shaderpacks/`).

**Lista** (virtualizada; precisa ficar fluida com 500 itens):

| Coluna | Conteúdo |
|---|---|
| Ícone e nome | Nome do projeto (do `.pw.toml`); ícone do cache de metadados. |
| Versão | Versão legível (ex.: `0.6.0+mc1.21.1`). Modrinth: número da versão do cache local. CurseForge: nome do arquivo. Link/arquivo local: nome do arquivo. Nunca um ID interno. |
| Fonte | Modrinth, CurseForge, Link direto, Arquivo local. |
| Lado | Cliente e servidor / Só cliente / Só servidor. Editável na linha. |
| Atualização | Em dia / Atualização disponível / Não verificado / Não foi possível verificar / Não se aplica (arquivo local ou link). |
| Marcas | Fixado (P1), Opcional (P1), Problema (do diagnóstico, com cor por gravidade). |

**Ações:** buscar por nome; filtrar por fonte, lado, "com atualização", "com problema"; ordenar; selecionar vários e **Alterar lado**, **Remover**, **Atualizar**; **Adicionar** (T08); **Verificar atualizações** (T10); clicar abre Detalhes (T07). Arrastar arquivos `.jar`/`.zip` para a lista inicia "Adicionar arquivo local" (T08).

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

**Ações:** Atualizar (quando houver), Remover, Abrir página, **Trocar de versão** (P1: escolher qualquer versão compatível, inclusive mais antiga), **Opcional** e **Fixar versão** (P1, ver T11).

**Regras:** dados da CurseForge são buscados na hora e não são guardados em disco (termos da CurseForge, R3 §3.2). Dados do Modrinth ficam no cache local.

| CA | Critério |
|---|---|
| CA-T07-01 | Uma descrição com `<script>` ou `onerror=` não executa código (teste de componente com HTML malicioso). |
| CA-T07-02 | Sem internet, os detalhes de um mod do Modrinth abrem do cache; os de um mod da CurseForge mostram "Detalhes indisponíveis sem internet" e ainda exibem nome, arquivo, lado e hash a partir do `.pw.toml`. |

---

### T08 — Adicionar: busca, link e arquivo (P0)

Painel com quatro abas: **Modrinth**, **CurseForge**, **Link**, **Arquivo**. Abre já no tipo da aba de origem (mods, resource packs ou shaders).

**Modrinth e CurseForge (busca):**
- Campo de busca; filtros já aplicados e travados por padrão: versão do Minecraft do pack, loader do pack (mods) e tipo de projeto. "Mostrar itens sem versão compatível" desligado por padrão.
- Filtros extras: categorias, ambiente (cliente/servidor), ordenar por relevância, downloads, atualizados recentemente, novos.
- Resultado: ícone, nome, autor, resumo, downloads, data da última atualização, marca "Já no pack" e, na CurseForge, marca "Download manual necessário" quando o autor bloqueou downloads por apps de terceiros.
- Rolagem infinita, 20 por página.
- Clicar abre a pré-visualização (mesmo layout de T07) com seletor de versão. Versão padrão: a mais nova compatível do canal configurado (padrão: só versões estáveis; beta/alpha só com a opção em Configurações).
- **Adicionar** abre a tela de dependências (T09). Depois de adicionar, a busca continua aberta e o item passa a mostrar "Já no pack".
- CurseForge sem chave: estado vazio "Para buscar na CurseForge, informe sua chave de API." com botão **Abrir Configurações**. Chave recusada pela CurseForge: "A CurseForge recusou a chave. Confira em Configurações."

**Link:**
- Aceita link de projeto ou versão do Modrinth (resolvido pela API do Modrinth), link de projeto ou arquivo da CurseForge (passa pelo packwiz, conforme decisão do dono; ADR-0006) e link direto `https://` para um arquivo `.jar`/`.zip`.
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
| CA-T08-02 | Buscar "not enough items" num pack Forge 1.7.10 na CurseForge e escolher o NEI adiciona o NEI (não outro resultado). |
| CA-T08-03 | Colar `https://www.curseforge.com/minecraft/mc-mods/jei` num pack Forge 1.20.1 adiciona o JEI via packwiz; o resultado é idêntico ao de adicionar o JEI pela busca da CurseForge (mesmo `.pw.toml`, exceto campos de lado definidos pelo Warden). |
| CA-T08-04 | Arrastar o `.jar` do Sodium baixado do Modrinth oferece "referência ao Modrinth"; aceitar grava `.pw.toml`, não copia o jar. |
| CA-T08-05 | Arrastar um jar Fabric num pack Forge mostra o bloqueio com explicação; "Adicionar mesmo assim" copia o arquivo e o diagnóstico passa a mostrar o erro de loader. |
| CA-T08-06 | Sem chave da CurseForge, a aba mostra o estado vazio descrito; nenhuma requisição é feita à CurseForge (verificado por mock). |
| CA-T08-07 | Um link `http://` é recusado com mensagem; nenhum arquivo é baixado. |

---

### T09 — Dependências e conflitos ao adicionar (P0)

Aparece antes de gravar qualquer coisa no pack.

**Conteúdo:**
- **Obrigatórias** (resolvidas em cadeia): marcadas. Desmarcar mostra "Sem esta dependência o jogo provavelmente não abre."
- **Já no pack:** listadas em verde, sem ação.
- **Opcionais:** desmarcadas, só informativas.
- **Sem versão compatível:** "Nenhuma versão de X para 1.20.1 Forge." O item principal ainda pode ser adicionado (vai aparecer no diagnóstico).
- **Incompatibilidades declaradas** com itens do pack (Modrinth `incompatible`, CurseForge "incompatível", metadados do jar `breaks`/`incompatible`): aviso em vermelho com o motivo, quando o autor informar.
- **Duplicado entre fontes:** "Este mod já está no pack pela CurseForge." com **Substituir pela versão do Modrinth**, **Manter os dois (não recomendado)** e **Cancelar**.

**Botão:** **Adicionar N itens**. A gravação é atômica: ou todos os itens confirmados entram, ou nenhum.

| CA | Critério |
|---|---|
| CA-T09-01 | Adicionar um mod Fabric que exige Fabric API num pack sem ela lista a Fabric API como obrigatória e marcada; confirmar grava os dois `.pw.toml` e atualiza o índice uma vez. |
| CA-T09-02 | Adicionar o Embeddium num pack com Sodium mostra a incompatibilidade declarada. |
| CA-T09-03 | Falha de rede no meio da gravação (simulada) não deixa nenhum `.pw.toml` novo no pack. |
| CA-T09-04 | Adicionar pelo Modrinth um mod que já está no pack pela CurseForge (detectado por projeto equivalente ou hash do arquivo) mostra a opção de substituir. |

---

### T10 — Atualizações (P0 individual; P1 "Atualizar todos")

**Verificar:** botão na aba e verificação automática ao abrir o pack se a última tiver mais de 24 h (configurável). Modrinth: uma única consulta em lote para o pack inteiro. CurseForge: consulta em lote por projeto. Um único "produtor" calcula o estado de cada item (R4 §2.4); a lista e os contadores leem desse mesmo resultado.

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

### T11 — Ajustes do pack, opcionais e fixar versão

**Ajustes do pack (P0):** nome, autor, descrição, versão do loader (trocar para outra versão do mesmo loader; P1), memória do teste (Automático ou valor fixo; ver T13), Java do teste (Automático ou um Java instalado), argumentos extras da JVM (validados contra a versão do Java; argumentos desconhecidos geram aviso), "Recriar instância de teste" (apaga a instância; pergunta se mantém os mundos de teste).

**Mods opcionais (P1):** no detalhe do item, "Opcional" com descrição e "ligado por padrão". Grava a tabela `[option]` do `.pw.toml`. Aviso fixo: "O app do Modrinth instala todos os opcionais; o formato da CurseForge não suporta lado." No teste, o usuário escolhe quais opcionais ligar na instância (padrão: os "ligados por padrão").

**Fixar versão (P1):** "Fixar versão" grava `pin = true`; itens fixados não são atualizados em lote e mostram o cadeado.

| CA | Critério |
|---|---|
| CA-T11-01 | Mudar o nome do pack altera só a linha `name` do `pack.toml` (diff de uma linha) e o índice continua válido. |
| CA-T11-02 | Marcar um mod como opcional grava `[option] optional = true` com `description` e `default`; o packwiz-installer real oferece a escolha ao instalar (teste de conformidade). |

---

### T12 — Configs (P0 editor de texto; P1 formulário)

**Objetivo:** editar as configurações do pack e, durante os testes, as da instância.

**Árvore de arquivos à esquerda** com seletor de origem:
- **Pack:** `config/`, `defaultconfigs/`, `kubejs/`, `scripts/`, `options.txt` e demais arquivos de texto do pack.
- **Instância de teste** (quando existir): `config/`, `defaultconfigs/`, `options.txt` e `saves/<mundo>/serverconfig/`. Edições aqui valem só para o teste e aparecem depois em "Revisar mudanças do teste" (T15).

Busca por nome de arquivo. Arquivos binários (ex.: `servers.dat`) aparecem sem edição na v1.

**Editor de texto (P0):** realce de sintaxe por extensão (TOML, JSON, JSON5, YAML, `.properties`, `.cfg`, JavaScript, texto), números de linha, buscar e substituir, desfazer. Arquivos acima de 2 MB abrem só para leitura com aviso.

**Salvar (P0):**
- Ctrl+S ou botão. Antes de gravar, mostra as diferenças (pode ser desligado em Configurações).
- Se o arquivo mudou no disco desde que foi aberto: "Este arquivo foi alterado fora do Warden." com **Recarregar** (descarta suas mudanças), **Ver diferenças** e **Sobrescrever**.
- Sair do arquivo com alterações não salvas pede confirmação.
- A gravação preserva tudo o que não foi editado (comentários, ordem, espaços), byte a byte.

**Formulário (P1):** para TOML do Forge/NeoForge, JSON/JSONC, JSON5, `.properties`, `.cfg` do Forge antigo e `options.txt`. Mostra as chaves por seção, com os comentários do arquivo como ajuda, faixas e valores permitidos quando o arquivo informa (`#Range`, `#Allowed Values`), e controles por tipo (liga/desliga, número com limite, lista de opções, texto, lista). Só os valores alterados são gravados. Alternar entre formulário e texto a qualquer momento.

**Avisos contextuais:**
- Arquivo `*-server.toml` em `config/` (Forge/NeoForge 1.13+): "Este tipo de config vale por mundo. Para valer em mundos novos, ele precisa estar em `defaultconfigs/`." com **Copiar para defaultconfigs/**.
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

**Fluxo ao clicar em Testar** (indicador de etapas sempre visível):
1. **Verificar o pack (rápido):** roda a passagem rápida do diagnóstico pré-teste (T14), que usa o pack e os dados das APIs. Com erros: diálogo listando os erros, com **Corrigir** (quando houver correção automática), **Testar mesmo assim** e **Cancelar**. Só avisos: segue, com os avisos visíveis na aba Diagnóstico.
2. **Preparar o Minecraft:** baixa a versão do jogo, instala o loader e escolhe/baixa o Java certo (ARCHITECTURE §7). Mostra progresso (arquivos e megabytes). Na primeira vez pode levar alguns minutos; o texto avisa isso.
3. **Sincronizar o pack:** copia/baixa para a instância só o que mudou; remove o que saiu do pack; nunca toca em `saves/`, `screenshots/` e afins. Mods da CurseForge com download bloqueado abrem T20.
4. **Verificação final:** passagem completa do diagnóstico, que lê os próprios arquivos `.jar` (dependências, duplicatas por ID, versão do Java exigida). Erros abrem o mesmo diálogo do passo 1.
5. **Abrir o jogo** com o perfil offline (nome configurado).
6. **Jogo em execução:** console ao vivo; botões **Parar jogo** (confirmação: "O progresso não salvo do mundo pode ser perdido."), **Abrir pasta da instância**, **Editar configs da instância** (abre T12 na origem Instância).
7. **Jogo fechou:** resumo com duração e resultado — "Fechado normalmente", "Travou" ou "Encerrado por você" — e:
   - "Mudanças detectadas: N arquivos" → **Revisar** (T15);
   - se travou: **Ver diagnóstico** (T14).

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
| CA-T13-01 | Para cada combinação da matriz de versões (ROADMAP L-05: 1.7.10 Forge, 1.12.2 Forge, 1.16.5 Forge e Fabric, 1.20.1 Forge, NeoForge e Fabric, 1.21.1 NeoForge e Fabric, versão mais nova vanilla, Fabric e NeoForge), um pack mínimo chega ao menu principal e cria um mundo (teste automatizado em Linux com servidor gráfico virtual + roteiro manual no Windows). |
| CA-T13-02 | "Parar jogo" encerra o processo do jogo e todos os processos filhos em até 5 s. |
| CA-T13-03 | Linhas com acentos no log aparecem corretas no console (sem `Ã©`), em Java 8 e Java 21 no Windows. |
| CA-T13-04 | Remover um mod do pack e testar de novo remove o jar da instância; um mundo criado no teste anterior continua lá. |
| CA-T13-05 | Desligar a internet depois de um teste bem-sucedido e testar de novo funciona. |
| CA-T13-06 | Um crash forçado (pack de teste quebrado) termina com resultado "Travou" e o botão Ver diagnóstico. |
| CA-T13-07 | Fechar o Warden durante o jogo, após confirmar, não deixa processo `java`/`javaw` órfão. |

---

### T14 — Diagnóstico (P0 determinístico; P1 IA)

**Objetivo:** encontrar problemas antes de abrir o jogo e explicar por que ele travou.

**Camada determinística (P0)** — regras fixas, sem IA, sempre com a evidência que as sustenta:
- **Antes do teste**, em duas passagens — rápida (antes de baixar, com dados do pack e das APIs) e completa (depois de sincronizar, lendo os `.jar`) — e também sob demanda pelo botão "Verificar agora": loader errado, versão do Minecraft incompatível, versão do loader fora da faixa exigida, dependência obrigatória ausente ou em versão errada, incompatibilidades declaradas, mod duplicado (mesmo ID, mesmo arquivo ou mesmo projeto de duas fontes), dois mods da mesma "categoria exclusiva" (ex.: dois renderizadores), mod só de cliente marcado como "cliente e servidor", Java incompatível, versões beta/alpha, conflitos conhecidos da lista curada, mods obsoletos. Lista de regras em ARCHITECTURE §9.
- **Depois de um travamento:** lê o resultado do processo, a saída capturada, `logs/latest.log`, `logs/debug.log`, os crash reports novos e os `hs_err_pid*.log` novos, e aplica o catálogo de padrões (dependência faltando, duplicado, falha de Mixin, Java errado, falta de memória, driver de vídeo, config corrompida, mod de cliente no servidor, entre outros).

**Apresentação:**
- Achados agrupados por gravidade: **Erro** (bloqueia o teste por padrão), **Aviso**, **Informação**.
- Cada achado: título em linguagem simples, explicação, itens envolvidos (com link para o item), evidência ("O arquivo fabric.mod.json do Sodium declara incompatibilidade com o OptiFine" ou o trecho do log com número da linha) e, quando houver, botões de correção (**Adicionar dependência X**, **Remover Y**, **Mudar lado para Só cliente**, **Atualizar Z**). Correções passam pelos fluxos normais, com confirmação.
- "Ignorar este aviso neste pack" (P1; gravado em `.warden/`).
- Sem conclusão: "Não encontramos a causa automaticamente." com **Pedir ajuda à IA** (P1) e **Abrir crash report**.

**IA Gemini (P1):**
1. **Pedir ajuda à IA** abre o diálogo de consentimento, sempre (não existe "não perguntar de novo"):
   - mostra o texto **exato** que será enviado, já sem dados pessoais (nome de usuário do Windows nos caminhos, nome do jogador, UUID, IPs, e-mails, nome do computador, qualquer coisa com cara de chave/token), rolável;
   - informa tamanho, provedor (Google Gemini), modelo, que o uso é cobrado/limitado na sua chave e que, no plano gratuito, o Google pode usar o conteúdo enviado para melhorar seus produtos;
   - botões **Enviar** e **Cancelar**.
2. Resposta estruturada: causa provável, confiança (baixa/média/alta), passos sugeridos, mods envolvidos. Aviso fixo: "A IA pode errar. Confira antes de mudar o pack."
3. Sem chave do Gemini: "Para usar a IA, informe sua chave do Gemini em Configurações."

| CA | Critério |
|---|---|
| CA-T14-01 | Cada pack de teste propositalmente quebrado (dependência faltando, jar Fabric no Forge, mod duplicado, mod que exige Java 21 em pack de Java 17, OptiFine com Sodium) gera o achado esperado antes do teste, com evidência. |
| CA-T14-02 | Cada log real do corpus de crashes (`crates/warden-diagnostics/tests/corpus/`) gera o diagnóstico esperado, com a linha de evidência correta. |
| CA-T14-03 | O texto mostrado no consentimento é byte a byte o que é enviado (teste com servidor simulado). |
| CA-T14-04 | Num log contendo `C:\Users\Maria\AppData`, o nome do jogador, um IPv4 e uma string `$2a$10$...`, nenhum desses valores aparece no texto enviado. |
| CA-T14-05 | Cancelar o consentimento não faz nenhuma requisição de rede. |

---

### T15 — Revisar mudanças do teste (P0)

**Objetivo:** trazer para o pack o que você ajustou durante o jogo, sem trazer lixo.

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

**Ações:** **Trazer selecionados para o pack** (grava, atualiza o índice); **Descartar** (no próximo teste a instância volta a ter os arquivos do pack); **Decidir depois** (o cabeçalho mostra "Mudanças do teste para revisar"). Testar de novo com mudanças não revisadas pergunta antes: "Revisar agora", "Descartar e testar", "Cancelar".

| CA | Critério |
|---|---|
| CA-T15-01 | Mudar a distância de renderização no jogo faz aparecer só `renderDistance` no grupo options.txt; aceitar grava só essa chave no `options.txt` do pack (diff de uma linha). |
| CA-T15-02 | Um config que o Forge apenas reformatou (mesmos valores) não aparece na lista. |
| CA-T15-03 | Nenhum arquivo da lista "sempre ignorados" aparece, mesmo que tenha mudado. |
| CA-T15-04 | Um jar colocado manualmente em `mods/` da instância nunca é copiado para o pack; a ação oferecida é adicionar como referência quando o hash é reconhecido. |
| CA-T15-05 | O conflito de três vias é mostrado quando o arquivo foi alterado no pack (pelo editor) e na instância (no jogo) durante o mesmo teste. |

---

### T16 — Salvar versão (P0)

**Diálogo:**
- **Versão sugerida** com o motivo, editável (validação SemVer; precisa ser maior que a última versão salva):
  - MAIOR: mudou a versão do Minecraft ou o loader; removeu mod que não é "só cliente" (pode apagar blocos/itens de mundos existentes); adicionou ou removeu mod de geração de mundo (categoria `worldgen` no Modrinth);
  - MENOR: adicionou mods, resource packs ou shaders;
  - CORREÇÃO: só atualizações de itens e mudanças de configs.
  - Primeira versão salva: sugere a versão atual do `pack.toml` (`0.1.0` em packs novos).
- **Changelog automático** (pré-visualização): Adicionados, Removidos, Atualizados (`versão antiga → versão nova`), Resource packs e shaders, Configs alteradas (lista de arquivos), Mudança de Minecraft/loader.
- **Notas** (texto livre, opcional) que entram no topo do changelog.
- Avisos: "Você removeu mods que podem ter conteúdo nos mundos. Avise quem joga para fazer backup." quando aplicável; isso também entra no changelog numa seção "Atenção".
- Checklist (P1): diagnóstico sem erros; último teste depois da última mudança abriu normalmente; mudanças do teste revisadas.

**Ao salvar:** grava a versão no `pack.toml`, acrescenta a entrada no topo do `CHANGELOG.md`, atualiza o índice, registra no histórico com a versão e o changelog. Sem mudanças desde a última versão: "Nada mudou desde a versão X." e o botão fica desabilitado.

| CA | Critério |
|---|---|
| CA-T16-01 | Adicionar 2 mods e atualizar 1 sugere MENOR e o changelog lista exatamente os 2 adicionados e o atualizado com versões legíveis. |
| CA-T16-02 | Remover um mod "cliente e servidor" sugere MAIOR e inclui a seção "Atenção". |
| CA-T16-03 | Após salvar, o histórico mostra a versão, o `pack.toml` tem a nova versão e `git status` (verificado em teste) está limpo. |
| CA-T16-04 | Tentar salvar `1.0.0` quando a última versão é `1.2.0` é recusado com explicação. |

---

### T17 — Histórico (P0)

**Conteúdo:**
- No topo, **Alterações não salvas**: lista de itens adicionados/removidos/atualizados e arquivos alterados desde a última versão. P1: **Descartar** por arquivo (com confirmação).
- Linha do tempo das versões salvas: número, data, resumo do changelog; abrir mostra o changelog completo e **Ver diferenças para o estado atual**.
- **Voltar para esta versão:** confirmação explicando que o estado atual fica guardado num ponto de segurança; o pack passa a ficar igual à versão escolhida (arquivos que não existiam nela são removidos); o histórico não é apagado. Depois disso, "Salvar versão" cria uma versão nova a partir dali.
- **Pontos de segurança** (P1): lista com data e motivo ("antes de voltar para 1.2.0") e **Recuperar**.

| CA | Critério |
|---|---|
| CA-T17-01 | Voltar para uma versão restaura exatamente a árvore de arquivos dela (comparação de hashes), inclusive removendo arquivos criados depois. |
| CA-T17-02 | Falha no meio da restauração (simulada) deixa o pack no estado anterior completo. |
| CA-T17-03 | Depois de voltar, o estado anterior pode ser recuperado (P0: pelo ponto de segurança registrado no repositório; P1: pela tela). |

---

### T18 — Enviar ao GitHub (P0)

**Configuração (uma vez por pack):** usa o token do GitHub guardado em Configurações (T21). Nome do repositório (padrão: nome do pack em minúsculas com hífens), sempre **privado**. **Criar repositório** (pela API do GitHub) ou **Usar repositório existente** (informar `dono/nome`).

**Enviar:** envia as versões salvas e o histórico. Estado: "Tudo enviado" ou "N versões não enviadas". Opção (desligada por padrão) "Enviar automaticamente após salvar versão".

**Erros:** sem internet; token inválido ou expirado ("O GitHub recusou o token. Gere um novo e salve em Configurações."); repositório inexistente ou sem permissão; o GitHub tem mudanças que não estão aqui (histórico divergente): bloqueia com explicação e oferece **Substituir o GitHub pelo que está aqui** (dupla confirmação, digitando o nome do pack) ou **Cancelar**. Trazer mudanças do GitHub (pull) é P1.

| CA | Critério |
|---|---|
| CA-T18-01 | Com um token válido, criar e enviar resulta num repositório privado contendo os arquivos do pack e as versões como tags (teste de integração com conta de teste, executado sob demanda). |
| CA-T18-02 | O token nunca aparece em logs, mensagens de erro, URLs gravadas no `.git/config` ou no frontend (teste de varredura). |

---

### T19 — Exportar (P0)

**Objetivo:** gerar o pack para distribuir, só com o necessário.

**Formatos:** **Pasta packwiz** (para hospedar, por exemplo no GitHub) ou **Arquivo .zip do pack packwiz**. Formatos de outros launchers (`.mrpack`, zip da CurseForge) são P2 (ver §10, decisão D1).

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

---

### T20 — Downloads manuais da CurseForge (P0)

**Quando aparece:** ao adicionar (aviso antecipado) e ao sincronizar a instância de teste, para mods cujo autor bloqueou downloads por apps de terceiros.

**Conteúdo:** lista com nome, arquivo esperado e estado (aguardando, encontrado, hash não confere). Para cada um: **Abrir página de download** (navegador) e **Selecionar arquivo baixado…**. P1: o Warden observa a pasta Downloads e reconhece os arquivos pelo hash automaticamente.

**Regras:** o arquivo vai para o cache do Warden, nunca para a pasta do pack. Hash diferente do esperado: recusa ("Este não é o arquivo certo."). Quando todos forem encontrados, o teste continua sozinho. Aviso: "Quem jogar o pack também vai precisar baixar esses mods manualmente."

| CA | Critério |
|---|---|
| CA-T20-01 | Selecionar um arquivo com hash errado é recusado; com o hash certo, o mod é instalado na instância e o teste prossegue. |
| CA-T20-02 | Após o fluxo, a pasta do pack não contém o jar. |

---

### T21 — Configurações do app (P0)

| Seção | Conteúdo |
|---|---|
| Geral | Pasta padrão dos packs; nome do jogador do teste; "Rever boas-vindas". |
| Teste | Memória padrão (Automático/fixo); Javas instalados pelo Warden (listar, remover); mostrar versões beta/alpha dos mods; verificação automática de atualizações (intervalo). |
| Editor | Mostrar diferenças antes de salvar (ligado). |
| Chaves e contas | CurseForge (chave), Gemini (chave e modelo), GitHub (token). Campos de senha. **Testar** em cada um. Depois de salvo, o valor nunca é mostrado de novo; só "Configurada" com **Substituir** e **Remover**. Passo a passo para criar cada chave, com links. |
| Privacidade e registros | Abrir pasta de registros; nível de detalhe dos registros (normal/detalhado). |
| Armazenamento (P1) | Espaço usado por cache, arquivos do Minecraft, Javas e instâncias; **Limpar cache**. |

**Regras:** chaves e tokens ficam no cofre de credenciais do sistema (Gerenciador de Credenciais do Windows), nunca em arquivo de configuração (ADR-0017).

| CA | Critério |
|---|---|
| CA-T21-01 | Salvar uma chave e reiniciar o app mantém o estado "Configurada"; o `settings.json` não contém a chave (teste de varredura). |
| CA-T21-02 | "Testar" da CurseForge com chave inválida mostra "A CurseForge recusou a chave." |

---

### T22 — Tarefas em andamento (P0)

Painel (gaveta) acessível pela barra lateral e por um indicador no rodapé: operações em andamento com progresso e **Cancelar** (quando a operação permite), e as últimas 50 concluídas com resultado. Toda operação que leva mais de 300 ms mostra progresso; toda operação que pode levar mais de 2 s e não altera o pack de forma irreversível pode ser cancelada. Erros têm **Ver detalhes**.

| CA | Critério |
|---|---|
| CA-T22-01 | Cancelar o download do Minecraft no meio para em até 2 s e não deixa arquivos parciais com nome final (só `.part` no cache, limpos depois). |

---

### T23 — Sobre (P0)

Versão do Warden, commit do packwiz embutido, aviso legal da Mojang/Microsoft (T01), créditos e licenças de terceiros (gerados automaticamente), link de apoio ao Forge (pedido do próprio instalador do Forge; R2 §3.4).

---

## 7. Regras que valem para o app inteiro

1. **A pasta do pack só recebe conteúdo do pack.** Nenhum `.bak`, temporário, cache, log, jar auxiliar ou configuração do app. Gravações usam arquivo temporário com sufixo `.warden-tmp` na mesma pasta seguido de renomeação; esse sufixo está no `.packwizignore` e sobras dele são apagadas ao abrir o pack.
2. **Confirmação e reversão.** Ação que apaga ou substitui conteúdo do pack pede confirmação nomeando o que será afetado, e cria ponto de segurança quando apaga mais de um item ou restaura versão.
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
- Exportar `.mrpack` e zip da CurseForge (P2, ver D1); publicar no Modrinth/CurseForge.
- Importar `.mrpack` ou zip da CurseForge (P2).
- Várias variantes do mesmo pack (versões do Minecraft diferentes no mesmo projeto) e assistente de migração de versão do Minecraft (P2).
- Editor de `servers.dat`, formulário para YAML, detecção de conflitos de teclas, sugestão de kits de performance, modelos de pack, datapacks globais (todos P2).
- Painel git completo (branches, merges), trazer mudanças do GitHub (P1), sincronização entre computadores além do GitHub.
- Atualização automática do Warden; macOS; empacotamento oficial para Linux (o app compila e roda em Linux para desenvolvimento).
- Temas além de claro/escuro do sistema; outros idiomas.

## 10. Decisões pendentes do dono

Cada item tem uma recomendação. Se o dono não responder, o projeto segue a recomendação.

| # | Pergunta (em linguagem simples) | Recomendação |
|---|---|---|
| D1 | Você quer, já na primeira versão, exportar o pack também no formato do app do Modrinth (`.mrpack`) ou da CurseForge, para amigos que usam esses apps? | **Não na v1.** A primeira versão exporta só no formato packwiz (o que você decidiu). Os outros formatos ficam para depois; o packwiz já sabe gerá-los, então não é difícil. |
| D2 | Cada pack deve ter o seu próprio repositório privado no GitHub? | **Sim, um repositório privado por pack.** Fica mais simples de entender e de voltar versões. |
| D3 | Para enviar ao GitHub, o Warden precisa de uma permissão sua. Pode ser colando um "token" (um código que você gera no site do GitHub, com passo a passo no app) ou fazendo login pelo navegador. | **Token na v1** (mais simples de construir e seguro, fica no cofre do Windows). Login pelo navegador fica para depois. |
| D4 | Como instalar o Warden no seu Windows? | **Instalador comum (.exe) sem assinatura digital.** Na primeira vez o Windows pode mostrar um aviso "Windows protegeu o seu computador"; basta clicar em "Mais informações" e "Executar assim mesmo". Assinatura digital custa dinheiro e não é necessária para uso pessoal. |
| D5 | Para gerar a versão de Windows direto do WSL, a ferramenta usa o kit de desenvolvimento da Microsoft, cuja licença precisa ser aceita uma vez. Você autoriza? | **Sim.** Sem isso, a versão de Windows só sai pelo GitHub Actions (funciona, mas é mais lento para testar). |
| D6 | Os mundos que você cria nos testes devem continuar existindo entre um teste e outro? | **Sim**, guardados na instância de teste de cada pack (nunca vão para o pack). Há um botão para apagar. |
| D7 | Ao abrir um pack feito nos apps antigos (packwiz-gui), o Warden deve oferecer limpar arquivos que não deveriam estar lá? | **Sim, mostrando a lista antes e guardando um ponto de segurança.** Nada é apagado sem você confirmar. |
| D8 | Ao procurar e atualizar mods, mostrar só versões estáveis? | **Sim, só estáveis por padrão**, com opção em Configurações para incluir beta/alpha. |
| D9 | Testes automáticos no Windows pelo GitHub gastam mais minutos do plano gratuito (repositório privado). | **Rodar Windows só ao integrar na `main` e uma vez por noite;** Linux em toda mudança. |

## 11. Rastreabilidade

| Tela | Tarefas do ROADMAP |
|---|---|
| T01, T21, T23 | F0-06, P1-13, A-02 |
| T02, T03, T04 | P1-07 |
| T05, T06, T07 | P1-08 |
| T08, T09 | P1-09, P1-10, P1-11 |
| T10 | P1-12 |
| T11 | P1-08 (ajustes básicos), P1-14 (opcionais, fixar) |
| T12 | C-01, C-02, C-04 |
| T13 | L-01 a L-05, L-07 |
| T14 | D-01 a D-04 |
| T15 | C-03 |
| T16, T17 | V-01, V-02 |
| T18 | V-03 |
| T19 | E-01 |
| T20 | L-06 |
| T22 | F0-05, F0-06 |
