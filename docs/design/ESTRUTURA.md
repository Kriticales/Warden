# Warden: estrutura do app (arquitetura de informação e navegação)

> Tarefa D2, 01/10/2026. Rascunho para aprovação do dono.
> Rascunho clicável: `design/estrutura/index.html` (abre direto no navegador, sem internet).
> Base: `docs/SPEC.md` (telas T01 a T23), relatórios em `docs/research/` e o motivo da reprovação do protótipo D1.

Este documento define **como se navega no Warden**, antes de qualquer estilo visual. Os rascunhos são em escala de cinza de propósito: a ideia é avaliar só a organização. A linguagem visual aprovada no D1 (paleta Warden, fontes pixel, menus estilo Minecraft) entra na etapa seguinte, por cima desta estrutura.

## Sumário

1. [Regras que guiaram a estrutura](#1-regras-que-guiaram-a-estrutura)
2. [Mapa do app](#2-mapa-do-app)
3. [As duas alternativas](#3-as-duas-alternativas)
4. [Recomendação](#4-recomendação)
5. [Nomes escolhidos e por quê](#5-nomes-escolhidos-e-por-quê)
6. [Onde fica cada tela e funcionalidade da SPEC](#6-onde-fica-cada-tela-e-funcionalidade-da-spec)
7. [Mudanças propostas na SPEC](#7-mudanças-propostas-na-spec)
8. [Os 5 fluxos principais, passo a passo](#8-os-5-fluxos-principais-passo-a-passo)
9. [Como os 4 problemas do protótipo foram resolvidos](#9-como-os-4-problemas-do-protótipo-foram-resolvidos)
10. [Crítica das alternativas e o que foi corrigido](#10-crítica-das-alternativas-e-o-que-foi-corrigido)
11. [Como usar o rascunho clicável](#11-como-usar-o-rascunho-clicável)
12. [Limites deste rascunho e pontos em aberto](#12-limites-deste-rascunho-e-pontos-em-aberto)

---

## 1. Regras que guiaram a estrutura

1. **Dois níveis que nunca se misturam.**
   - **Nível do app:** a lista de packs, criar pack, abrir pack existente e as Configurações do Warden.
   - **Nível do pack:** tudo o que pertence a um pack aberto.

   Sem pack aberto, nenhum item de pack aparece.
2. **Dentro do pack, sempre se sabe qual pack é e como sair.** Um cabeçalho fixo mostra nome, Minecraft, loader e versão do pack, com "← Meus packs" à esquerda.
3. **Um nível de navegação visível por vez.** Nada de abas dentro de abas. Filtros, seletores e modos de exibição usam caixas de seleção, nunca abas.
4. **Testar fica sempre a um clique.** É o botão maior do cabeçalho do pack, presente em todas as telas do pack. Não é item de menu. Enquanto o jogo está aberto, o mesmo botão vira "● Jogo aberto: ver teste".
5. **O raro fica escondido até ser preciso.** Ajustes do teste, pasta da instância, fixar versão, pontos de segurança e coisas assim ficam em menus "▾", "⋯" ou "Mais opções", ou no fim da página.
6. **Nomes que dizem o que tem dentro.** Cada seção tem um nome curto e uma linha de descrição sempre visível.
7. **O dia a dia é o caminho mais curto:** abrir pack → adicionar mods → ajustar configs → testar → ver o que mudou → salvar versão → exportar.
8. **Texto de ferramenta de trabalho.** Frases diretas e úteis, sem slogans, exclamações ou textos de preenchimento. Isso segue o pedido do dono de que nada tenha "cara de feito por IA" (ver §10.3).

## 2. Mapa do app

O nível do app é igual nas duas alternativas. O que muda é o nível do pack.

### 2.1 Alternativa A: seções do pack (recomendada)

```
Primeira execução (assistente, só na primeira vez)
│
MEUS PACKS ............................ nível do APP: só packs e Configurações
├── Criar pack (assistente em 4 etapas)
├── Abrir pack existente (com verificação e limpeza)
├── Configurações (inclui a seção "Sobre o Warden")
└── [Abrir um pack]
    │
    PACK ABERTO ....................... nível do PACK
    Cabeçalho fixo: ← Meus packs · nome · Minecraft, loader e versão do pack
                    · avisos do momento · [Salvar versão · N alterações] · [▶ Testar ▾]
    Menu lateral (só existe dentro do pack):
    ├── Mods ........ mods, resource packs e shaders (lista única)
    │   ├── Detalhes de um item (painel lateral)
    │   ├── Adicionar → Dependências (diálogo)
    │   └── Atualizar itens (diálogo)
    ├── Configs ..... arquivos de ajuste dos mods e do jogo
    ├── Problemas ... o que pode impedir o jogo de abrir
    ├── Histórico ... versões salvas, alterações não salvas, GitHub
    └── Exportar .... gerar o pack para quem vai jogar
    Fora do menu (ações e telas de contexto):
    ├── ▶ Testar → tela do teste: checagem → preparação → jogo aberto → resultado
    ├── Salvar versão (diálogo, abre de qualquer tela do pack)
    ├── Editar informações (diálogo, ao lado do nome)
    └── ▾ do Testar: ver último teste, ajustes do teste, pasta da instância,
                     apagar mundos de teste, recriar instância, testar como o jogador recebe
Rodapé em todas as telas: indicador de Tarefas (abre a gaveta)
```

### 2.2 Alternativa B: painel do pack

```
(nível do app igual ao da A)
└── [Abrir um pack]
    │
    PAINEL DO PACK .................... página única, sem menu lateral
    Cabeçalho fixo: igual ao da A, mais o aviso "N problemas"
    Faixa "Próximo passo" (o que fazer agora, uma sugestão por vez)
    Coluna principal: Mods, resource packs e shaders (prévia da lista, busca, Adicionar)
                      Configs (editados há pouco, Abrir editor)
    Coluna lateral:   Situação (problemas, último teste, não salvas)
                      Versões (atual, Salvar versão, GitHub)
                      Exportar
    Cada área abre uma página de detalhe com "← Painel do pack":
    ├── Lista completa de mods → Detalhes, Adicionar, Atualizar
    ├── Editor de configs
    ├── Problemas
    ├── Histórico
    ├── Exportar
    └── Teste (pelo botão Testar)
```

## 3. As duas alternativas

As duas resolvem os 4 problemas do D1. A diferença está em **como se anda dentro do pack**.

### 3.1 Alternativa A: seções do pack

- Um menu lateral com **5 seções fixas** (Mods, Configs, Problemas, Histórico, Exportar). Cada item mostra um contador quando faz sentido (128 itens, 3 problemas, 5 não salvas) e uma linha dizendo o que tem dentro.
- O cabeçalho fixo concentra as duas ações do pack: **▶ Testar** (a maior) e **Salvar versão · N alterações**.
- O teste não é seção: ao clicar em Testar, a área principal vira a tela do teste. Dá para navegar pelas seções com o jogo aberto, porque o botão do cabeçalho leva de volta ao console.
- Pack novo abre em **Mods** com o estado vazio "Nenhum mod ainda" e um botão **Adicionar mods**.

**Bom para:** o vai e vem do dia a dia (mods ↔ configs ↔ teste) a um clique; posições que não mudam; a situação do pack visível o tempo todo pelos contadores do menu.
**Custo:** um menu lateral sempre presente (pequeno: 5 itens).

### 3.2 Alternativa B: painel do pack

- O pack abre numa **página única** que resume tudo: prévia da lista de mods, configs editados há pouco, situação (problemas, último teste, não salvas), versões e exportação.
- Uma faixa **"Próximo passo"** sugere uma coisa por vez, em ordem de prioridade: mudanças do teste para revisar, depois erros, depois alterações não salvas.
- Cada área leva a uma página de detalhe. Voltar é sempre "← Painel do pack" (com o caminho "Vale Sereno › Configs" ao lado).
- Não existe menu lateral.

**Bom para:** primeira vez num pack, ver de relance como ele está e ser guiado.
**Custo:** trocar de lugar (por exemplo, de Configs para Mods) exige voltar ao painel: dois cliques em vez de um. O painel tende a crescer com o tempo, porque toda função nova "pede" uma área.

### 3.3 Comparação

| Critério | A · Seções | B · Painel |
|---|---|---|
| Itens de navegação dentro do pack | 5 seções | 4 áreas + páginas de detalhe |
| Trocar de Mods para Configs | 1 clique | 2 cliques (voltar ao painel, entrar) |
| Testar | 1 clique, de qualquer tela | 1 clique, de qualquer tela |
| Ver a situação do pack | Contadores no menu e avisos no cabeçalho | Painel inteiro dedicado a isso |
| Orientação para quem está começando | Estado vazio e avisos | Faixa "Próximo passo" |
| Risco de crescer e virar bagunça | Baixo: o menu é fixo | Médio: o painel acumula áreas |
| Fluxo diário completo (fluxo 3 da §8) | 7 etapas | 8 etapas |

## 4. Recomendação

**Recomendo a Alternativa A.**

O uso real do Warden é repetitivo: adicionar um mod, mexer numa config, testar, voltar, mexer de novo. Na A, cada um desses lugares está a um clique e sempre na mesma posição. Isso é o que torna uma ferramenta "óbvia" depois da primeira semana. A situação do pack, que é o ponto forte da B, já aparece na A pelos contadores do menu e pelos avisos do cabeçalho. A B ganha na primeira visita, mas cobra um clique extra em toda troca de lugar, para sempre.

Da B, a A herda a ideia de sugerir o próximo passo nos momentos certos:
- depois de criar o pack, o estado vazio leva a **Adicionar mods**;
- depois do teste, o resultado oferece **Trazer para o pack** e, em seguida, **Salvar versão**;
- depois de salvar, o diálogo oferece **Exportar esta versão** e **Enviar ao GitHub**.

## 5. Nomes escolhidos e por quê

Termos seguem o glossário de `QUALITY.md` §8.2. Revisados com a skill `design:ux-copy`: nome curto, sem jargão, dizendo o que tem dentro.

### 5.1 Nível do app

| Nome | O que é | Por quê |
|---|---|---|
| **Meus packs** | Tela inicial com a lista de packs | Diz de quem são e o que são. Antes era "Modpacks", genérico, e ficava misturado com itens do pack. |
| **Criar pack** / **Abrir pack existente** | Botões da tela inicial | Verbo + objeto, como pede o QUALITY §8.1. |
| **Configurações** | Preferências do Warden | Termo obrigatório do glossário para preferências do app. Fica só no nível do app. |
| **Tarefas** | Gaveta de operações em andamento | Não é item de menu: é um indicador no rodapé ("Nenhuma tarefa em andamento", "1 tarefa em andamento"). |

### 5.2 Nível do pack (Alternativa A)

| Nome no menu | Linha de descrição | Por quê |
|---|---|---|
| **Mods** | Mods, resource packs e shaders | "Mods" é a palavra que todo jogador usa para "o que se adiciona ao jogo". A descrição deixa claro que resource packs e shaders também estão ali. Substitui "Conteúdo", que não dizia nada. |
| **Configs** | Arquivos de ajuste dos mods e do jogo | "Config" é termo obrigatório do glossário para arquivo de configuração de mod. Não pode ser "Configurações", que é o app. A descrição explica o termo para o leigo e cita o jogo (`options.txt`). |
| **Problemas** | O que pode impedir o jogo de abrir | Substitui "Diagnóstico", palavra técnica que descreve o processo, não o resultado. O usuário procura "o que está errado", não "o diagnóstico". Sem nada a mostrar: "Nenhum problema encontrado". |
| **Histórico** | Versões salvas e o que mudou | Termo do glossário. Substitui "Versões", que era ambíguo: versão do Minecraft? do mod? do pack? |
| **Exportar** | Gerar o pack para quem vai jogar | O verbo diz a ação, e a descrição diz para quem. |
| **▶ Testar** (botão) | — | Termo obrigatório do glossário ("Testar", nunca "Jogar"). |
| **Salvar versão · 5 alterações** (botão) | — | Termo do glossário. O contador mostra que há algo a salvar sem precisar de outro aviso. |
| **Editar informações** (link ao lado do nome) | — | Substitui "Ajustes do pack": o que fica ali é nome, autor e descrição, ou seja, informações. |
| **Ajustes do teste neste computador** (menu ▾) | — | Deixa claro que não faz parte do pack e não conta como alteração. |

Na Alternativa B, as áreas do painel usam os mesmos nomes. A única diferença é "Versões", porque ali o número da versão atual aparece junto ("Atual: 1.4.2").

### 5.3 Telas e estados do teste

| Nome | Por quê |
|---|---|
| Etapas: **Verificar o pack · Preparar o Minecraft · Copiar o pack para o teste · Verificação final · Abrir o jogo** | "Sincronizar", termo da SPEC, virou "Copiar o pack para o teste", que diz o que acontece. |
| **O que mudou durante o teste** | Substitui "Revisar mudanças do teste" como título. Na prática é uma pergunta que o usuário faz. |
| **Por que travou** | Título da área de diagnóstico do travamento. Glossário: "travou", nunca "crash" na interface (exceto "crash report", o arquivo). |
| **Mudanças do teste para revisar** | Aviso do cabeçalho quando o usuário escolhe "Decidir depois" (texto da SPEC mantido). |

## 6. Onde fica cada tela e funcionalidade da SPEC

"A" e "B" indicam diferenças entre as alternativas. Quando não há indicação, vale para as duas.

| SPEC | Funcionalidade | Onde fica |
|---|---|---|
| T01 | Primeira execução (aviso legal, nome do jogador, pasta, chaves) | Assistente em tela cheia antes de Meus packs. Reabre em Configurações → Geral → "Rever boas-vindas". |
| T02 | Lista de packs | **Meus packs** (tela inicial). Lista em linhas, com Minecraft/loader, versão, último teste, não salvas e data. |
| T02 | Estados vazio, pasta sumiu, pack ilegível | Na própria lista. Pasta sumiu: linha com **Localizar…** e **Remover da lista**. |
| T02 | Mostrar na pasta, Remover da lista, Apagar pack (P1) | Menu **⋯** de cada pack (escondido). |
| T03 | Criar pack (4 etapas) | Assistente em tela cheia a partir de Meus packs. No fim abre o pack em Mods (A) ou no Painel (B), vazio, com **Adicionar mods**. |
| T04 | Abrir pack existente + limpeza | Botão em Meus packs → escolher pasta → tela de verificação com a lista de arquivos a limpar, **Agora não** e **Limpar N arquivos e abrir**. |
| T05 | Cabeçalho do pack | Cabeçalho fixo em todas as telas do pack: ← Meus packs, nome, Minecraft/loader/versão, avisos do momento, Salvar versão, Testar. |
| T05 | Indicadores (não salvas, mudanças do teste, problemas, jogo em execução) | Não salvas: no botão Salvar versão. Mudanças do teste: aviso no cabeçalho. Problemas: contador no menu (A) ou aviso no cabeçalho (B). Jogo em execução: o botão Testar vira "● Jogo aberto: ver teste". |
| T05 | Abas do pack | Substituídas por 5 seções (A) ou painel + páginas (B). Ver §7. |
| T06 | Lista de mods, resource packs e shaders | **Mods** (A) / página "lista completa" (B). Lista única agrupada por tipo, grupos recolhíveis. |
| T06 | Buscar, filtrar, ordenar, seleção múltipla (lado, remover, atualizar) | Barra acima da lista. A barra de ações aparece quando há itens selecionados. |
| T06 | Arrastar .jar/.zip | Sobre a lista (texto de ajuda logo abaixo do título). |
| T06 | Remover com aviso de dependentes | Diálogo de confirmação. |
| T06 | Estados (vazio, arquivo inválido, fora do índice) | Na própria lista. |
| T07 | Detalhes do item | Painel lateral sobre a lista: você não perde o lugar. |
| T07, T11 | Trocar de versão, Fixar versão, Opcional (P1) | Dentro dos detalhes, em **Mais opções**. |
| T08 | Adicionar (Modrinth, CurseForge, Link, Arquivo) | Página **Adicionar** dentro de Mods. Um campo só (busca ou link colado), **Onde buscar** (Modrinth/CurseForge), **Tipo** e **Escolher arquivo do computador…**. Ver §7. |
| T08 | CurseForge sem chave | Estado da página Adicionar com **Abrir Configurações**. |
| T09 | Dependências e conflitos | Diálogo antes de gravar, ao clicar em Adicionar ao pack. |
| T10 | Verificar atualizações | Botão em Mods. Verificação automática ao abrir o pack. |
| T10 | Atualizar um item / Atualizar todos (P1) | Nos detalhes do item, na seleção múltipla e na faixa "N atualizações disponíveis" → diálogo de revisão. |
| T11 | Nome, autor, descrição, versão do loader (P1) | **Editar informações** ao lado do nome do pack (diálogo). |
| T11 | Ajustes do teste (memória, Java, JVM, opcionais no teste, recriar instância) | Menu **▾** do Testar → **Ajustes do teste neste computador…** |
| T12 | Editor de configs | **Configs**: árvore à esquerda, editor à direita. |
| T12 | Origem Pack / Instância de teste | Seletor "Mostrando: arquivos do pack ▾". Durante o teste, **Editar configs do teste** abre o editor já na instância, com aviso fixo. |
| T12 | Formulário (P1) × Texto | Alternância de modo no topo do editor (mesmo arquivo, duas formas de ver). |
| T12 | Diferenças antes de salvar, alteração externa, avisos contextuais | Diálogo ao salvar e avisos acima do editor. |
| T13 | Testar | Botão do cabeçalho. Abre a tela do teste com as 5 etapas sempre visíveis. |
| T13 | Console, Parar jogo, Abrir pasta da instância, Editar configs da instância | Tela do teste, estado "Jogo aberto". |
| T13 | Resultado (fechou, travou, encerrado por você) | Tela do teste, estado "O jogo fechou" ou "O jogo travou". |
| T13 | Testar como o jogador recebe (P1) | Menu **▾** do Testar. |
| T13 | Um jogo por vez | O botão vira "● Jogo aberto: ver teste". Em outro pack, a mensagem da SPEC com **Ir para o teste**. |
| T13 | Rever o último teste depois | Menu **▾** do Testar → **Ver último teste**. Na B também em Situação → Último teste. |
| T14 | Diagnóstico antes do teste | Etapa 1 do teste (diálogo com erros, **Testar mesmo assim**, **Cancelar**) e seção **Problemas**. |
| T14 | Verificar agora | Botão em **Problemas**. |
| T14 | Diagnóstico depois de travar | Resultado do teste ("Por que travou"), com link em Problemas → Último travamento. |
| T14 | Ignorar aviso (P1) | Em cada problema. |
| T14 | IA Gemini com consentimento (P1) | **Pedir ajuda à IA** no resultado do travamento → diálogo de consentimento com o texto exato. |
| T15 | Revisar mudanças do teste | Parte do resultado do teste: "O que mudou durante o teste". Ver §7. |
| T15 | Decidir depois | Aviso "Mudanças do teste para revisar" no cabeçalho até resolver. |
| T15 | Conflito de três vias | Dentro de "O que mudou", no arquivo em conflito. |
| T16 | Salvar versão | Botão do cabeçalho (qualquer tela do pack) e em Histórico → diálogo. Depois de salvar: **Exportar esta versão** / **Enviar ao GitHub**. |
| T17 | Alterações não salvas, versões, voltar para uma versão | **Histórico**. |
| T17 | Pontos de segurança (P1) | Link discreto no fim do Histórico. |
| T18 | Enviar ao GitHub | Linha "GitHub" no Histórico e no diálogo pós-salvar. A primeira vez pede para conectar o pack. Token em Configurações → Chaves e contas. |
| T19 | Exportar | **Exportar**: uma página de cima para baixo (conferências → formato → o que vai → exportar). |
| T19 | Exportar uma versão salva (P1) | Em Histórico, dentro de cada versão. |
| T20 | Downloads manuais da CurseForge | Etapa 3 do teste. Aviso antecipado na página Adicionar. |
| T21 | Configurações do app | Página única rolável, com seções e sem submenu. |
| T22 | Tarefas | Indicador no rodapé de todas as telas → gaveta. |
| T23 | Sobre | Última seção de Configurações ("Sobre o Warden"). O aviso legal também aparece na primeira execução. |

## 7. Mudanças propostas na SPEC

Estas mudanças não removem nenhuma funcionalidade. Elas só mudam onde cada coisa fica ou juntam duas telas numa. Se aprovadas, a SPEC §5 e T05, T06, T08, T11, T13, T14, T15, T22 e T23 precisam ser atualizadas.

| # | Mudança | Justificativa |
|---|---|---|
| M1 | As 9 abas do pack (T05) viram 5 seções (A) ou painel + páginas (B). | Problema 4 (itens demais) e 2 (abas dentro de abas). Teste, Diagnóstico e Ajustes deixam de ser abas: o teste é um modo aberto pelo botão; o diagnóstico vira "Problemas"; os ajustes se dividem entre "Editar informações" e o menu do Testar. |
| M2 | Mods, Resource packs e Shaders (T06) viram **uma lista agrupada por tipo**. | Eram três abas idênticas, e com o menu lateral formavam abas dentro de abas. O comportamento era o mesmo nas três (a própria SPEC diz isso). |
| M3 | Adicionar (T08) troca as **4 abas** por um campo único que aceita busca ou link, mais "Onde buscar", "Tipo" e "Escolher arquivo do computador…". | Para o leigo, colar um link e buscar são o mesmo gesto: "achar o mod". O Warden reconhece um link pelo formato. Arquivo é um botão porque é outra ação (abrir o explorador de arquivos). Some mais um nível de abas. |
| M4 | "Revisar mudanças do teste" (T15) passa a ser **o próprio resultado do teste**. | O usuário só chega lá depois de um teste. Ser uma tela à parte criava um passo a mais e um lugar a mais para lembrar. |
| M5 | O diagnóstico de travamento (T14) aparece **no resultado do teste**. A seção "Problemas" guarda o pré-teste e um link para o último travamento. | O travamento é visto na hora em que acontece, sem precisar ir buscar. |
| M6 | "Ajustes do pack" (T11) se divide: informações do pack num diálogo ("Editar informações"); ajustes do teste no menu ▾ do Testar. | Eram coisas de natureza diferente (o que vai no pack × o que vale só neste computador) juntas sob um nome vago. |
| M7 | "Diagnóstico" vira **Problemas** como nome de seção. "Diagnóstico" continua como termo do glossário para a verificação em si. | O nome passa a dizer o que tem dentro. Pedido de mudança no glossário do QUALITY §8.2: acrescentar "Problemas" como nome da seção. |
| M8 | **Tarefas** (T22) deixa de ser item da barra lateral e fica só como indicador no rodapé + gaveta. | A SPEC já previa o indicador no rodapé. O item de menu era redundante e misturava níveis. |
| M9 | **Sobre** (T23) vira a última seção de Configurações. | Uma visita por ano não justifica um item de menu. |
| M10 | A barra lateral do nível do app (Packs, Tarefas, Configurações, Sobre) **deixa de existir**. Meus packs é a tela inicial, com Configurações no topo. | Problema 1. Com M8 e M9, sobravam dois destinos, e eles cabem no topo da tela inicial. |
| M11 | "Sincronizar o pack" (etapa 3 do T13) passa a se chamar **Copiar o pack para o teste** na interface. | Diz o que acontece, em vez de usar um termo técnico. |

## 8. Os 5 fluxos principais, passo a passo

Os mesmos fluxos estão no rascunho, na página "Fluxos principais", com cada passo clicável. Entre parênteses, o botão clicado.

### Fluxo 1. Primeiro uso: criar o primeiro pack

1. Boas-vindas com o aviso legal (**Entendi**) e, em seguida, nome do jogador, pasta e chaves (com "Pular, configuro depois").
2. Meus packs, vazio: "Você ainda não tem packs" (**Criar pack**).
3. Criar pack, etapa 1: nome, autor, descrição e pasta (**Próximo**).
4. Etapa 2: versão do Minecraft, com busca (**Próximo**).
5. Etapa 3: loader. Só aparecem os que existem para a versão escolhida (**Próximo**).
6. Etapa 4: resumo (**Criar pack**).
7. O pack abre vazio, em Mods (A) ou no Painel (B), com "Nenhum mod ainda" e **Adicionar mods**.

### Fluxo 2. Adicionar mods

1. Meus packs (**Abrir** no pack).
2. A: abre em Mods. B: abre no Painel (**Adicionar**, nos dois casos).
3. Adicionar: busca já filtrada para a versão e o loader do pack. Clicar num resultado mostra a pré-visualização à direita (**Adicionar ao pack**).
4. Dependências: o que entra, o que é obrigatório, avisos (**Adicionar 2 itens**).
5. A busca continua aberta; o item mostra "Já no pack". Repete para outros mods ou volta (**← Voltar para Mods**).

### Fluxo 3. Ajustar configs, testar e trazer o que mudou (o dia a dia)

1. A: Configs (1 clique no menu). B: Painel → **Abrir editor**.
2. Escolher o arquivo na árvore, editar (**Salvar**) → diálogo com a diferença (**Salvar**).
3. **▶ Testar** (cabeçalho). Etapa 1, verificação: se houver erro, o diálogo explica e oferece corrigir; senão segue sozinho.
4. Etapas 2 a 4: preparar o Minecraft, copiar o pack para o teste, verificação final, com progresso. Se algum mod da CurseForge exigir download manual, a etapa 3 pede e espera.
5. Jogo aberto: console ao vivo. Opcional: **Editar configs do teste**, que abre o editor na instância, com aviso.
6. O jogo fecha: "O jogo fechou normalmente" e, logo abaixo, **O que mudou durante o teste**, já pré-selecionado (**Trazer 3 selecionados para o pack**).
7. Confirmação: "3 mudanças trazidas para o pack" (**Salvar versão** ou **Continuar editando**).

### Fluxo 4. O jogo travou: entender e corrigir

1. Durante o teste, o jogo trava.
2. Resultado: "O jogo travou" e **Por que travou**: explicação simples, trecho do log com o número da linha e o botão que corrige (**Adicionar Balm**).
3. Diálogo de dependências, o mesmo do fluxo 2 (**Adicionar**).
4. **▶ Testar** de novo.
5. Se a causa não aparecer: **Pedir ajuda à IA** (P1) → diálogo com o texto exato que será enviado, sem dados pessoais (**Enviar** ou **Cancelar**).

### Fluxo 5. Salvar versão, exportar e enviar ao GitHub

1. De qualquer tela do pack: **Salvar versão · 5 alterações** (cabeçalho).
2. Diálogo: número sugerido com o motivo, notas opcionais e resumo automático (**Salvar versão 1.5.0**).
3. "Versão 1.5.0 salva" com os próximos passos (**Exportar esta versão**).
4. Exportar: conferências, formato, o que vai no pack (**Exportar**) → "Pack exportado" (**Abrir pasta**).
5. Histórico → linha GitHub (**Enviar ao GitHub**). Na primeira vez, conectar o pack a um repositório privado. Também dá para enviar direto do diálogo do passo 3.

**Contagem de etapas** (página Fluxos do rascunho): fluxo 1, 6 nas duas; fluxo 2, 5 nas duas; fluxo 3, 7 (A) e 8 (B); fluxo 4, 3 nas duas; fluxo 5, 6 nas duas. A diferença entre A e B aparece toda vez que se troca de seção. No uso real isso acontece muitas vezes por sessão, mais do que a contagem de um fluxo isolado mostra.

## 9. Como os 4 problemas do protótipo foram resolvidos

| Problema do D1 | Como foi resolvido |
|---|---|
| **1. Menu misturado:** itens do app e de um pack no mesmo menu, aparecendo até sem pack aberto ("conteúdo de qual pack?"). | Dois níveis separados. O nível do app não tem menu lateral: só Meus packs, com Criar, Abrir existente e Configurações. Itens de pack só existem depois de abrir um pack. Dentro do pack, o cabeçalho sempre diz qual é o pack e tem "← Meus packs". Configurações do app nunca aparecem dentro do pack, só por links de contexto (por exemplo, "Abrir Configurações" quando falta a chave da CurseForge). |
| **2. Abas dentro de abas.** | Nenhuma tela tem abas. Mods, resource packs e shaders ficam numa lista agrupada (M2). Adicionar usa um campo único (M3). Configs usa um seletor de origem. Exportar e Histórico são páginas corridas. Detalhes de mod abrem em painel lateral e confirmações em diálogo: são camadas temporárias, não navegação. |
| **3. Nomes pouco claros.** | Cada seção tem nome e descrição visível (§5). "Conteúdo" virou "Mods", com a descrição "Mods, resource packs e shaders". "Diagnóstico" virou "Problemas". "Versões" virou "Histórico". "Ajustes do pack" foi desfeito. |
| **4. Itens demais.** | De 9 abas no pack + 4 itens na barra lateral do app (13 destinos) para 5 seções no pack e 2 destinos no app (A). Teste, Ajustes, Tarefas e Sobre deixaram de ser itens de menu e viraram botão, menu do Testar, indicador no rodapé e seção de Configurações. |

## 10. Crítica das alternativas e o que foi corrigido

Feita com a skill `design:design-critique`, olhando como um usuário leigo, sobre o rascunho navegado num navegador real (capturas de tela das telas principais).

### 10.1 Achados e correções aplicadas

| Achado | Gravidade | O que foi feito |
|---|---|---|
| Na A, os avisos "3 problemas" e "5 alterações não salvas" no cabeçalho repetiam os contadores do menu lateral. Era ruído e lembrava o excesso de itens do D1. | Moderada | A contagem de não salvas foi para dentro do botão ("Salvar versão · 5 alterações"). O aviso de problemas ficou só na B, que não tem menu. Na A, o cabeçalho só mostra avisos passageiros (jogo aberto, mudanças do teste para revisar, pack mudou durante o teste). |
| Durante a preparação do teste, o botão ainda dizia "Testar", o que convida a clicar de novo e gera a dúvida "começou ou não?". | Moderada | O botão vira "Testando… ver progresso" na preparação e "● Jogo aberto: ver teste" com o jogo aberto. |
| Na A, depois que o jogo fechava e o usuário saía da tela, não havia caminho óbvio para rever o último teste (console, o que mudou). | Moderada | **Ver último teste** entrou no menu ▾ do Testar. Na B, também em Situação. |
| "Trazer selecionados para o pack" levava direto ao Histórico, o que desorienta: "por que vim parar aqui?". | Moderada | Agora aparece uma confirmação dizendo onde as mudanças foram parar, com **Salvar versão** e **Continuar editando**. |
| A grade de cartões idênticos em Meus packs e as 4 caixas iguais no painel da B tinham "cara de feito por IA" (pedido do dono) e mostravam pouca informação por pack. | Moderada | Meus packs virou uma lista densa em linhas, mais fácil de comparar. O painel da B ficou assimétrico: coluna principal com a prévia da lista de mods e configs; coluna lateral com situação, versões e exportar. |
| Estados vazios centralizados, com texto "de vitrine". | Pequena | Alinhados à esquerda, com texto direto: o que é, por que está vazio, o que fazer. |
| Excesso de travessões nos textos. | Pequena | Trocados por dois-pontos, parênteses ou "·". |

### 10.2 Achados mantidos de propósito (para o dono avaliar)

- **"Lado" pode confundir o leigo.** É o termo do glossário ("Cliente e servidor / Só cliente / Só servidor"). Sugestão para a etapa visual: uma dica ao passar o mouse no cabeçalho da coluna ("Onde o mod precisa estar instalado").
- **"Onde buscar: Modrinth / CurseForge"** exige saber que existem duas lojas. Mantido porque a busca combinada nas duas fontes muda o comportamento da T08 (ordem, duplicatas, chave) e é uma decisão maior que esta tarefa.
- **O botão ▾ ao lado do Testar** pode passar despercebido. O que está nele é raro, então tudo bem ser discreto. A etapa visual deve garantir que ele pareça clicável.
- **Na B, Problemas e Teste não têm área própria no painel** (aparecem na faixa "Próximo passo" e em Situação). É coerente com a ideia do painel, mas é um ponto a mais para aprender.

### 10.3 "Cara de feito por IA" (pedido do dono, repassado pelo orquestrador)

Nesta etapa, que é só estrutura em cinza, a regra vale para textos e organização:
- textos diretos, sem slogans, sem exclamações, sem títulos óbvios; exemplos e dados realistas (nomes de mods, versões, trechos de log de verdade);
- densidade de ferramenta: listas em tabela onde há o que comparar, em vez de cartões iguais em grade;
- hierarquia com intenção: o botão Testar é o único elemento grande do cabeçalho; o painel da B é assimétrico;
- nenhum ícone de "brilhinho" para a IA: "Pedir ajuda à IA" é um botão comum, em segundo plano, depois da causa determinística.

## 11. Como usar o rascunho clicável

- Abrir `design/estrutura/index.html` no navegador, ou pelo link publicado (no relatório da tarefa).
- **Seletor no topo:** troca entre Alternativa A e B mantendo a tela atual. As telas do nível do app são iguais nas duas.
- **Mapa** e **Fluxos** (links no topo): a árvore do app, a recomendação e os 5 fluxos com cada passo clicável.
- **Índice à esquerda:** todas as telas, com a referência da SPEC (T01–T23).
- **Mostrar o que é clicável:** contorna em tracejado tudo o que leva a outra tela. Botões sem destino mostram "Não ligado neste rascunho" ao passar o mouse.
- Acima de cada tela há uma nota com o que observar, de onde se chega, para onde se vai e os outros estados daquela tela.

Verificação feita: as 43 telas mais as páginas Mapa e Fluxos, nas duas alternativas (90 combinações), abertas num Chromium headless sem erros de JavaScript e sem nenhum botão ou link apontando para tela inexistente.

## 12. Limites deste rascunho e pontos em aberto

- O rascunho mostra **organização**, não visual. Tamanhos, espaçamentos e a tipografia do sistema não são proposta.
- Nem todo botão está ligado. Os principais caminhos dos 5 fluxos estão completos; ações secundárias (por exemplo, "Alterar lado" em lote) mostram "Não ligado neste rascunho".
- A janela do rascunho tem largura fixa (1080 px) para parecer o app desktop. Em telas estreitas aparece rolagem horizontal só dentro da janela.
- **Decisões para o dono:**
  1. Alternativa A ou B (recomendação: A).
  2. Aprovar as mudanças M1 a M11 na SPEC (§7).
  3. Manter "Onde buscar" separado por fonte ou pedir a busca combinada Modrinth + CurseForge (§10.2).
