# Warden: estrutura do app (arquitetura de informação e navegação)

> Tarefa D2, 01/10/2026. **Aprovada pelo dono no mesmo dia: Alternativa A** (ver §0). A SPEC, a ARCHITECTURE, o ROADMAP, o QUALITY e as ADRs 0025 a 0029 já foram atualizados; este documento registra a proposta e o porquê.
> **Tarefa D4, 01/10/2026:** as funções avançadas pedidas pelo dono (pesquisas R5A e R5B) entram na mesma estrutura, sem seção nova e sem abas. Onde fica cada uma e as poucas mudanças de nome estão na [§13](#13-funções-avançadas-d4-onde-fica-cada-uma); ela prevalece sobre as seções anteriores quando houver diferença.
> Rascunho clicável: `design/estrutura/index.html` (abre direto no navegador, sem internet), publicado em https://app.superset.sh/page/warden-estrutura-rascunho-zfldsl
> Base: `docs/SPEC.md` (telas T01 a T23), relatórios em `docs/research/` e o motivo da reprovação do protótipo D1.

Este documento define **como se navega no Warden**, antes de qualquer estilo visual. Os rascunhos são em escala de cinza de propósito: a ideia é avaliar só a organização. A linguagem visual aprovada no D1 (paleta Warden, fontes pixel, menus estilo Minecraft) entra na etapa seguinte, por cima desta estrutura.

## Sumário

0. [Decisões do dono](#0-decisões-do-dono)
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
13. [Funções avançadas (D4): onde fica cada uma](#13-funções-avançadas-d4-onde-fica-cada-uma)

---

## 0. Decisões do dono

Tomadas em 01/10/2026, a partir deste documento e dos comentários na página publicada. Valem as versões da SPEC 1.1 e das ADRs; aqui fica o resumo.

| Decisão | Onde está registrada |
|---|---|
| **Alternativa A** aprovada, com a sexta seção **✦ Diagnóstico com IA**; Alternativa B descartada. | SPEC §5 e T05, ADR-0026 |
| Mudanças **M1 a M12** (§7) aprovadas. | SPEC (telas citadas em cada M) |
| **Busca combinada** Modrinth + CurseForge num resultado só, com a fonte marcada e sem duplicatas; sem chave da CurseForge, só Modrinth com aviso. Some o seletor "Onde buscar". | SPEC T08, ADR-0027 |
| **Chaves:** cofre do Windows por padrão, com opção de arquivo `.env` na pasta de dados do Warden e aviso claro de que é texto legível por outros programas. | SPEC T21, ADR-0025 |
| **Java:** o mais novo que funciona com cada versão, sempre atualizado, com o motivo à vista. | SPEC T11 e T21, ADR-0029 |
| **GitHub distribui o pack:** salvar continua local; versões marcadas como **versão final** são publicadas com **Publicar versão** (nome escolhido no lugar de "Enviar ao GitHub"), com changelog, tag e Release; jogadores atualizam pelo link do `pack.toml`; repositório público recomendado, privado como "só backup". Substitui a D2 antiga ("sempre privado"). | SPEC T16–T18 e §10 (D14), ADR-0028 |

**Por que "Publicar versão"** (revisado com a skill `design:ux-copy`): forma par com "Salvar versão" e deixa clara a diferença entre as duas (salvar é para você; publicar é para os jogadores). "Enviar ao GitHub" descrevia o mecanismo, não o efeito; "Lançar" e "Release" soam técnicos ou ambíguos. A versão pronta para os jogadores se chama **versão final**, e o endereço que os jogadores usam se chama **link do pack**.

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
    ├── ✦ Diagnóstico com IA ... pedir à IA para explicar um travamento (pedido do dono)
    │   └── Resposta da IA
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
                      ✦ Diagnóstico com IA (pedido do dono)
                      Versões (atual, Salvar versão, GitHub)
                      Exportar
    Cada área abre uma página de detalhe com "← Painel do pack":
    ├── Lista completa de mods → Detalhes, Adicionar, Atualizar
    ├── Editor de configs
    ├── Problemas
    ├── ✦ Diagnóstico com IA → Resposta da IA
    ├── Histórico
    ├── Exportar
    └── Teste (pelo botão Testar)
```

## 3. As duas alternativas

As duas resolvem os 4 problemas do D1. A diferença está em **como se anda dentro do pack**.

### 3.1 Alternativa A: seções do pack

- Um menu lateral com **6 seções fixas** (Mods, Configs, Problemas, ✦ Diagnóstico com IA, Histórico, Exportar). A meta era até 5; a sexta foi pedida pelo dono num comentário na página (§10.4). Cada item mostra um contador quando faz sentido (128 itens, 3 problemas, 5 não salvas) e uma linha dizendo o que tem dentro.
- O cabeçalho fixo concentra as duas ações do pack: **▶ Testar** (a maior) e **Salvar versão · N alterações**.
- O teste não é seção: ao clicar em Testar, a área principal vira a tela do teste. Dá para navegar pelas seções com o jogo aberto, porque o botão do cabeçalho leva de volta ao console.
- Pack novo abre em **Mods** com o estado vazio "Nenhum mod ainda" e um botão **Adicionar mods**.

**Bom para:** o vai e vem do dia a dia (mods ↔ configs ↔ teste) a um clique; posições que não mudam; a situação do pack visível o tempo todo pelos contadores do menu.
**Custo:** um menu lateral sempre presente (pequeno: 6 itens).

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
| Itens de navegação dentro do pack | 6 seções | 5 áreas + páginas de detalhe |
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
- depois de salvar uma versão final, o diálogo oferece **Publicar versão** e **Exportar arquivo**.

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
| **Histórico** | Versões salvas e publicação | Termo do glossário. A descrição ganhou "publicação" com a decisão do GitHub (§0). Substitui "Versões", que era ambíguo: versão do Minecraft? do mod? do pack? |
| **Exportar** | Gerar o pack para quem vai jogar | O verbo diz a ação, e a descrição diz para quem. |
| **✦ Diagnóstico com IA** | Pedir à IA para explicar um travamento | Nome pedido pelo dono. O ✦ (ícone de brilhinho na etapa visual) identifica a IA. Fica logo abaixo de Problemas, porque é o passo seguinte quando a verificação automática não explica. |
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
| T05 | Abas do pack | Substituídas por 6 seções (A) ou painel + páginas (B). Ver §7. |
| T06 | Lista de mods, resource packs e shaders | **Mods** (A) / página "lista completa" (B). Lista única agrupada por tipo, grupos recolhíveis. |
| T06 | Buscar, filtrar, ordenar, seleção múltipla (lado, remover, atualizar) | Barra acima da lista. A barra de ações aparece quando há itens selecionados. |
| T06 | Arrastar .jar/.zip | Sobre a lista (texto de ajuda logo abaixo do título). |
| T06 | Remover com aviso de dependentes | Diálogo de confirmação. |
| T06 | Estados (vazio, arquivo inválido, fora do índice) | Na própria lista. |
| T07 | Detalhes do item | Painel lateral sobre a lista: você não perde o lugar. |
| T07, T11 | Trocar de versão, Fixar versão, Opcional (P1) | Dentro dos detalhes, em **Mais opções**. |
| T08 | Adicionar (Modrinth, CurseForge, Link, Arquivo) | Página **Adicionar** dentro de Mods. Um campo só (busca combinada no Modrinth e na CurseForge, ou link colado), **Tipo** e **Escolher arquivo do computador…**. Cada resultado mostra a fonte; o mesmo mod nas duas fontes aparece uma vez (decisão do dono, §0). |
| T08 | CurseForge sem chave | Só resultados do Modrinth, com aviso e **Abrir Configurações**. |
| T09 | Dependências e conflitos | Diálogo antes de gravar, ao clicar em Adicionar ao pack. |
| T10 | Verificar atualizações | Botão em Mods. Verificação automática ao abrir o pack. |
| T10 | Atualizar um item / Atualizar todos (P1) | Nos detalhes do item, na seleção múltipla e na faixa "N atualizações disponíveis" → diálogo de revisão. |
| T11 | Nome, autor, descrição, versão do loader (P1) | **Editar informações** ao lado do nome do pack (diálogo). |
| T11 | Ajustes do teste (memória, Java, JVM, opcionais no teste, recriar instância) | Menu **▾** do Testar → **Ajustes do teste neste computador…** O Java automático mostra por que é aquela versão ("Por que não o Java 25?"). |
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
| T14 | Diagnóstico com IA, lugar dedicado (P1) | Seção **✦ Diagnóstico com IA** (A) ou bloco no painel (B): escolher o que analisar (último travamento, outro teste, log do computador), contar algo à IA, consentimento, respostas anteriores. Também pelo botão **✦ Analisar com IA** em Problemas → Último travamento. Ver M12. |
| T14 | Resposta da IA (P1) | Página **Resposta da IA**: causa provável, confiança, mods envolvidos, passos com botões e o aviso "A IA pode errar". |
| T14 | IA Gemini com consentimento (P1) | **✦ Pedir ajuda à IA** (✦ = ícone de brilhinho que identifica a IA) no resultado do travamento → diálogo de consentimento com o texto exato. |
| T15 | Revisar mudanças do teste | Parte do resultado do teste: "O que mudou durante o teste". Ver §7. |
| T15 | Decidir depois | Aviso "Mudanças do teste para revisar" no cabeçalho até resolver. |
| T15 | Conflito de três vias | Dentro de "O que mudou", no arquivo em conflito. |
| T16 | Salvar versão | Botão do cabeçalho (qualquer tela do pack) e em Histórico → diálogo, com **Marcar como versão final**. Salvar é local. Depois de salvar uma versão final: **Publicar versão** / **Exportar arquivo**. |
| T17 | Alterações não salvas, versões, voltar para uma versão | **Histórico**. |
| T17 | Pontos de segurança (P1) | Link discreto no fim do Histórico. |
| T18 | Publicar versão (GitHub, para os jogadores) | Área "Publicação para os jogadores" no Histórico (repositório, link do pack, **Copiar link**, **Como os jogadores instalam**) e **Publicar versão** em cada versão final. Diálogo com avisos e notas; na primeira vez, repositório público (recomendado) ou privado ("só backup"); no fim, link do pack, passo a passo para jogadores e cópia das notas. |
| T19 | Exportar | **Exportar**: uma página de cima para baixo (conferências → formato → o que vai → exportar). |
| T19 | Exportar uma versão salva (P1) | Em Histórico, dentro de cada versão. |
| T20 | Downloads manuais da CurseForge | Etapa 3 do teste. Aviso antecipado na página Adicionar. |
| T21 | Configurações do app | Página única rolável, com seções e sem submenu. Chaves e contas logo depois de Geral, com "Onde guardar as chaves" (cofre ou `.env`). Em Teste, uma tabela de Java mostra qual pack usa cada versão e por que não é a mais nova de todas. |
| T22 | Tarefas | Indicador no rodapé de todas as telas → gaveta. |
| T23 | Sobre | Última seção de Configurações ("Sobre o Warden"). O aviso legal também aparece na primeira execução. |

## 7. Mudanças propostas na SPEC

Estas mudanças não removem nenhuma funcionalidade. Elas só mudam onde cada coisa fica ou juntam duas telas numa. Se aprovadas, a SPEC §5 e T05, T06, T08, T11, T13, T14, T15, T22 e T23 precisam ser atualizadas.

| # | Mudança | Justificativa |
|---|---|---|
| M1 | As 9 abas do pack (T05) viram 6 seções (A, contando a de IA da M12) ou painel + páginas (B). | Problema 4 (itens demais) e 2 (abas dentro de abas). Teste, Diagnóstico e Ajustes deixam de ser abas: o teste é um modo aberto pelo botão; o diagnóstico vira "Problemas"; os ajustes se dividem entre "Editar informações" e o menu do Testar. |
| M2 | Mods, Resource packs e Shaders (T06) viram **uma lista agrupada por tipo**. | Eram três abas idênticas, e com o menu lateral formavam abas dentro de abas. O comportamento era o mesmo nas três (a própria SPEC diz isso). |
| M3 | Adicionar (T08) troca as **4 abas** por um campo único que aceita busca ou link, mais "Tipo" e "Escolher arquivo do computador…". Com a decisão do dono, a busca é combinada (Modrinth + CurseForge) e o seletor "Onde buscar" saiu. | Para o leigo, colar um link e buscar são o mesmo gesto: "achar o mod". O Warden reconhece um link pelo formato. Arquivo é um botão porque é outra ação (abrir o explorador de arquivos). Some mais um nível de abas. |
| M4 | "Revisar mudanças do teste" (T15) passa a ser **o próprio resultado do teste**. | O usuário só chega lá depois de um teste. Ser uma tela à parte criava um passo a mais e um lugar a mais para lembrar. |
| M5 | O diagnóstico de travamento (T14) aparece **no resultado do teste**. A seção "Problemas" guarda o pré-teste e um link para o último travamento. | O travamento é visto na hora em que acontece, sem precisar ir buscar. |
| M6 | "Ajustes do pack" (T11) se divide: informações do pack num diálogo ("Editar informações"); ajustes do teste no menu ▾ do Testar. | Eram coisas de natureza diferente (o que vai no pack × o que vale só neste computador) juntas sob um nome vago. |
| M7 | "Diagnóstico" vira **Problemas** como nome de seção. "Diagnóstico" continua como termo do glossário para a verificação em si. | O nome passa a dizer o que tem dentro. Pedido de mudança no glossário do QUALITY §8.2: acrescentar "Problemas" como nome da seção. |
| M8 | **Tarefas** (T22) deixa de ser item da barra lateral e fica só como indicador no rodapé + gaveta. | A SPEC já previa o indicador no rodapé. O item de menu era redundante e misturava níveis. |
| M9 | **Sobre** (T23) vira a última seção de Configurações. | Uma visita por ano não justifica um item de menu. |
| M10 | A barra lateral do nível do app (Packs, Tarefas, Configurações, Sobre) **deixa de existir**. Meus packs é a tela inicial, com Configurações no topo. | Problema 1. Com M8 e M9, sobravam dois destinos, e eles cabem no topo da tela inicial. |
| M11 | "Sincronizar o pack" (etapa 3 do T13) passa a se chamar **Copiar o pack para o teste** na interface. | Diz o que acontece, em vez de usar um termo técnico. |
| M12 | **Diagnóstico com IA ganha lugar próprio** (pedido do dono): seção no menu (A) ou bloco no painel (B). Além do último travamento, permite analisar outro teste ou um log/crash report escolhido no computador, e guarda as respostas anteriores. | Pedido explícito do dono em comentário. A T14 só previa a IA como botão no resultado do travamento. As respostas guardadas ficam nos dados do Warden, nunca na pasta do pack (podem conter trechos de log). O consentimento a cada envio não muda. |

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

### Fluxo 5. Salvar a versão final e publicar para os jogadores

1. De qualquer tela do pack: **Salvar versão · 5 alterações** (cabeçalho).
2. Diálogo: número sugerido com o motivo, notas opcionais, resumo automático e **Marcar como versão final** (**Salvar versão 1.5.0**). Até aqui, tudo fica só no computador.
3. "Versão 1.5.0 salva como versão final" (**Publicar versão 1.5.0**).
4. Diálogo de publicação: avisos (mods da CurseForge que os jogadores não conseguem baixar sozinhos, com troca pelo Modrinth quando existe; versão não testada; problemas pendentes), notas da versão desde a última publicada e o que vai e não vai para o GitHub (**Publicar** ou **Publicar mesmo assim**).
5. Só na primeira vez: repositório **público** (recomendado, para o link funcionar) ou **privado** ("só backup") (**Criar repositório e publicar**).
6. "Versão 1.5.0 publicada": **link do pack** com **Copiar link**, passo a passo para os jogadores (Prism ou MultiMC com o packwiz-installer-bootstrap) e **Copiar texto das notas da versão**.

Para gerar um arquivo em vez de publicar, a seção **Exportar** continua igual.

**Contagem de etapas** (proposta original, nas duas alternativas): fluxo 1, 6 nas duas; fluxo 2, 5 nas duas; fluxo 3, 7 (A) e 8 (B); fluxo 4, 3 nas duas; fluxo 5 (antes da decisão do GitHub), 6 nas duas. Depois da decisão, o fluxo 5 da A tem 5 etapas (a do repositório só na primeira publicação). A diferença entre A e B aparece toda vez que se troca de seção. No uso real isso acontece muitas vezes por sessão, mais do que a contagem de um fluxo isolado mostra.

## 9. Como os 4 problemas do protótipo foram resolvidos

| Problema do D1 | Como foi resolvido |
|---|---|
| **1. Menu misturado:** itens do app e de um pack no mesmo menu, aparecendo até sem pack aberto ("conteúdo de qual pack?"). | Dois níveis separados. O nível do app não tem menu lateral: só Meus packs, com Criar, Abrir existente e Configurações. Itens de pack só existem depois de abrir um pack. Dentro do pack, o cabeçalho sempre diz qual é o pack e tem "← Meus packs". Configurações do app nunca aparecem dentro do pack, só por links de contexto (por exemplo, "Abrir Configurações" quando falta a chave da CurseForge). |
| **2. Abas dentro de abas.** | Nenhuma tela tem abas. Mods, resource packs e shaders ficam numa lista agrupada (M2). Adicionar usa um campo único (M3). Configs usa um seletor de origem. Exportar e Histórico são páginas corridas. Detalhes de mod abrem em painel lateral e confirmações em diálogo: são camadas temporárias, não navegação. |
| **3. Nomes pouco claros.** | Cada seção tem nome e descrição visível (§5). "Conteúdo" virou "Mods", com a descrição "Mods, resource packs e shaders". "Diagnóstico" virou "Problemas". "Versões" virou "Histórico". "Ajustes do pack" foi desfeito. |
| **4. Itens demais.** | De 9 abas no pack + 4 itens na barra lateral do app (13 destinos) para 6 seções no pack (a sexta, Diagnóstico com IA, a pedido do dono) e 2 destinos no app (A). Teste, Ajustes, Tarefas e Sobre deixaram de ser itens de menu e viraram botão, menu do Testar, indicador no rodapé e seção de Configurações. |

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
- ~~**"Onde buscar: Modrinth / CurseForge"** exige saber que existem duas lojas.~~ **Resolvido pelo dono:** busca combinada (§0).
- **O botão ▾ ao lado do Testar** pode passar despercebido. O que está nele é raro, então tudo bem ser discreto. A etapa visual deve garantir que ele pareça clicável.
- **Na B, Problemas e Teste não têm área própria no painel** (aparecem na faixa "Próximo passo" e em Situação). É coerente com a ideia do painel, mas é um ponto a mais para aprender.

### 10.3 "Cara de feito por IA" (pedido do dono, repassado pelo orquestrador)

Nesta etapa, que é só estrutura em cinza, a regra vale para textos e organização:
- textos diretos, sem slogans, sem exclamações, sem títulos óbvios; exemplos e dados realistas (nomes de mods, versões, trechos de log de verdade);
- densidade de ferramenta: listas em tabela onde há o que comparar, em vez de cartões iguais em grade;
- hierarquia com intenção: o botão Testar é o único elemento grande do cabeçalho; o painel da B é assimétrico;
- a IA é identificada pelo **ícone de brilhinho (sparkles)**, escolha do dono (correção do orquestrador em 01/10/2026). No rascunho em cinza ele aparece como o marcador "✦" no botão "Pedir ajuda à IA" e no título do diálogo de consentimento. Ele marca só o que é IA (o diagnóstico com Gemini), não é decoração. O botão continua em segundo plano, depois da causa determinística.

### 10.4 Comentários do dono na página publicada

| Comentário | O que foi feito |
|---|---|
| Java: "Sempre o mais recente, se não for possível, explique o porquê". | Atendido na estrutura. O Warden usa o Java mais novo que cada versão do Minecraft aceita, sempre com a atualização mais recente dele. Configurações → Teste ganhou uma tabela com cada Java, quais packs o usam e o motivo; Ajustes do teste mostra "Automático: Java 17" com "Por que não o Java 25?". Motivos tirados do R2 §2.1: o Forge 1.7.10/1.12.2 só abre no Java 8, e o Minecraft 1.18–1.20.4 e 1.20.5–1.21 foram feitos para o 17 e o 21. Sem mudança na regra de seleção do ADR-0012. Fica em aberto testar se versões como 1.18–1.20.4 funcionam bem num Java mais novo; isso cabe a um spike, não à estrutura. |
| Na seção Problemas: "Tem que ter uma opção dedicada ao Diagnóstico com IA". | Feito. Nova seção **✦ Diagnóstico com IA** logo abaixo de Problemas (A) e bloco no painel (B), com página de resposta. Ver M12. O menu da A passou de 5 para 6 itens. |
| Chaves: "Deve ficar .env nos arquivos do aplicativo". | **Decidido pelo dono: os dois.** Cofre do Windows por padrão, com opção de `.env` na pasta de dados do Warden e aviso claro (ADR-0025). Aplicado em Configurações → Chaves e contas (versão 5 da página); tópico respondido e resolvido. |

## 11. Como usar o rascunho clicável

- Abrir `design/estrutura/index.html` no navegador, ou pelo link publicado (https://app.superset.sh/page/warden-estrutura-rascunho-zfldsl).
- **Só a Alternativa A** (aprovada). A B saiu do rascunho; links antigos com `#B/…` abrem a mesma tela na A.
- **Mapa** e **Fluxos** (links no topo): a árvore do app, as decisões do dono e os 5 fluxos com cada passo clicável.
- **Índice à esquerda:** todas as telas, com a referência da SPEC (T01–T23).
- **Mostrar o que é clicável:** contorna em tracejado tudo o que leva a outra tela. Botões sem destino mostram "Não ligado neste rascunho" ao passar o mouse.
- Acima de cada tela há uma nota com o que observar, de onde se chega, para onde se vai e os outros estados daquela tela.

Verificação feita: todas as telas mais as páginas Mapa e Fluxos (52 combinações na versão final, só Alternativa A; 96 combinações quando havia A e B), abertas num Chromium headless sem erros de JavaScript e sem nenhum botão ou link apontando para tela inexistente.

## 12. Limites deste rascunho e pontos em aberto

- O rascunho mostra **organização**, não visual. Tamanhos, espaçamentos e a tipografia do sistema não são proposta.
- Nem todo botão está ligado. Os principais caminhos dos 5 fluxos estão completos; ações secundárias (por exemplo, "Alterar lado" em lote) mostram "Não ligado neste rascunho".
- A janela do rascunho tem largura fixa (1080 px) para parecer o app desktop. Em telas estreitas aparece rolagem horizontal só dentro da janela.
- **Decisões para o dono:** todas respondidas em 01/10/2026 (§0).

## 13. Funções avançadas (D4): onde fica cada uma

Depois de aprovar a estrutura, o dono pediu uma ferramenta **completa** de criação, edição e debug. As pesquisas `docs/research/05-diagnostico-avancado.md` (R5A) e `docs/research/06-criacao-edicao-descoberta.md` (R5B) detalharam as funções, e o dono decidiu o escopo em 01/10/2026 (SPEC §10, D15 a D25). Esta seção diz onde cada função mora. As regras da §1 continuam valendo sem exceção: **dois níveis, 6 seções no pack, nenhuma aba, o raro escondido**. Função nova entra como página de detalhe, modo de exibição ("Ver como: Lista · Grafo"), item do menu ▾ do Testar, painel lateral, diálogo ou modo da tela do teste.

### 13.1 Mapa atualizado

```
MEUS PACKS ................................ nível do APP
├── coluna nova: Saúde (nota do pack: "36 · Crítico")
├── Criar pack (5 etapas: Nome e pasta · Versão do Minecraft · Loader · Mods iniciais · Resumo)
├── Abrir ou importar… (pasta packwiz, .mrpack, zip da CurseForge, instância do Prism ou da CurseForge)
│   └── Importar modpack (verificação: convertido, arquivo local, revisar, lixo que não entra)
├── Configurações (+ "EULA do Minecraft" em Teste)
└── PACK ABERTO ........................... nível do PACK
    Cabeçalho: igual; o Testar mostra o perfil quando não é o padrão ("▶ Testar · PC fraco")
    Menu lateral (as mesmas 6 seções; três descrições mudam):
    ├── Mods ........ Mods, resource packs e shaders
    │   ├── Ver como: Lista · Grafo (seletor de modo, não aba)
    │   ├── Detalhes do item (painel): + Depende de / Usado por / Por que está no pack
    │   │                               + O que este mod altera no jogo (raio-x de mixins)
    │   │                               └── Ver todas as alterações (no mesmo painel, com "← Detalhes")
    │   ├── Adicionar → PÁGINA DE DESCOBERTA em tela cheia
    │   │     Tipo: Mods · Resource packs · Shaders · Modpacks
    │   │     campo vazio = início: Populares, Atualizados recentemente, Kits de desempenho, Categorias
    │   │     resultados com seleção múltipla → "Adicionar N ao pack" → Dependências (um diálogo só)
    │   │     pré-visualização à direita: Descrição · Galeria · Versões · Dependências · Links
    │   │     Tipo = Modpacks: abrir um modpack mostra os mods dele com caixas → "Adicionar N selecionados"
    │   └── Atualizar itens (diálogo)
    ├── Configs ..... Arquivos de ajuste e scripts do pack                       (descrição nova)
    │   ├── Buscar em todas as configs (acima da árvore) · filtro "Só o que mudou do padrão"
    │   ├── Formulário: padrão de cada chave e "Restaurar padrão"
    │   └── Scripts .js/.zs: o mesmo editor, com Trechos prontos, IDs, Erros dos scripts,
    │       Recarregar no jogo e Abrir no VS Code
    ├── Problemas ... Saúde do pack, problemas e travamentos                     (descrição nova)
    │   ├── Saúde do pack (nota, faixa e "O que tirou pontos"), no topo
    │   ├── Erros, Avisos e Informações (inclui "dois mods alteram o mesmo ponto do jogo")
    │   └── Travamentos (agrupados pela causa), com "Encontrar o mod culpado" e "✦ Conversar com a IA"
    ├── ✦ Diagnóstico com IA ... Conversar com a IA sobre um problema do pack     (descrição nova)
    │   ├── Nova conversa (do que se trata, o que analisar) → consentimento uma vez por conversa
    │   ├── Conversas (lista: Continuar, Apagar)
    │   └── Conversa (página de detalhe com "← Conversas"; substitui a página "Resposta da IA")
    ├── Histórico ... Versões salvas e publicação
    └── Exportar .... Gerar o pack para quem vai jogar
        └── + formato "Pacote para servidor (.zip)": baixar os mods pelo link do pack (padrão) ou mods dentro do zip
    Fora do menu:
    ├── Tela do teste (igual) + faixa de desempenho acima do console
    │     console: "Mostrar: Linha a linha · Agrupado por mod · Só problemas"
    │     com servidor: "Mostrando: Servidor · Jogo" e linha de comando do servidor
    │     com KubeJS ou CraftTweaker no pack: botão "Recarregar scripts"
    │     resultado: + "Abriu em 1 min 42 s" e, se travou, "Encontrar o mod culpado"
    ├── Busca do culpado: modo da tela do teste, com rodadas no lugar das 5 etapas;
    │     o botão do cabeçalho vira "Buscando o culpado: ver progresso"
    └── ▾ do Testar (em grupos):
          Ver último teste
          ── Outros testes: Testar como o jogador recebe · Testar como servidor… ·
                            Testar com perfil de desempenho · Encontrar o mod culpado…
          ── Perfil do teste: ◉ Padrão · ○ PC fraco · Ajustes do teste neste computador…
          ── Instância de teste: Abrir pasta · Apagar mundos de teste… · Recriar instância de teste…
```

### 13.2 Onde fica cada função nova

| Função (pesquisa) | Onde fica | Como se chega |
|---|---|---|
| Página de descoberta (R5B §7) | **Adicionar**, agora em tela cheia dentro do pack. O menu lateral recolhe para ícones enquanto ela está aberta e volta ao sair. | Botão **Adicionar** em Mods. "← Voltar para Mods · 2 adicionados". |
| Navegar por modpacks (R5B §6) | Valor **Modpacks** no seletor Tipo da página de descoberta. Abrir um modpack mostra os mods dele no painel da direita, com caixas de seleção. | Tipo = Modpacks. |
| Kits de desempenho (R5B §5.3) | Início da página de descoberta ("Kits de desempenho") e etapa **Mods iniciais** do Criar pack. | Sempre pelo diálogo de dependências, com caixas marcadas. Nada entra em silêncio. |
| Mods padrão: spark e Crash Assistant (R5A §2) | Etapa **Mods iniciais** do Criar pack, já marcados, cada um com uma frase dizendo para que serve. | Criar pack. |
| Importar `.mrpack`, zip da CurseForge e instância do Prism (R5B §5.1) | **Abrir ou importar…** em Meus packs → página **Importar modpack** (mesma verificação e limpeza do T04). | Meus packs. |
| Grafo de dependências (R5A §6.1) | **Mods → Ver como: Grafo**. Grafo focado num mod (o que ele exige à esquerda, quem depende dele à direita), com a explicação em texto ao lado. Nos detalhes do item: Depende de, Usado por, Por que está no pack. | Seletor no topo de Mods; links nos detalhes. |
| Raio-x de mixins (R5A §5) | **Detalhes do item** (painel): bloco "O que este mod altera no jogo" e **Ver todas as alterações** no mesmo painel. Sobreposições de risco viram aviso em **Problemas**. | Detalhes do mod, Problemas, resultado da busca do culpado. |
| Nota de saúde (R5A §6.2) | Topo de **Problemas** e coluna **Saúde** em Meus packs. | Sempre visível. |
| Travamentos (R5A §7.5) | **Problemas → Travamentos**: uma linha por causa, com quantas vezes, quando e em que versões. | Problemas. |
| Busca do culpado (R5A §3) | Modo da **tela do teste** (é um teste em várias rodadas). Configuração em diálogo; progresso por rodada; resultado com as rodadas. | Resultado "O jogo travou", Problemas → Travamentos, menu ▾ ("Encontrar o mod culpado…"), proposta da IA. |
| IA "médico" com ferramentas (R5A §4) | Seção **✦ Diagnóstico com IA**: Nova conversa e lista de Conversas; cada conversa é uma página de detalhe. | Seção do menu; **✦ Pedir ajuda à IA** no resultado do travamento abre uma conversa já com aquele travamento. |
| Console agrupado por mod (R5A §7.1) | **Tela do teste**: seletor "Mostrar: Linha a linha · Agrupado por mod · Só problemas" no console. | Durante e depois do teste. |
| Memória ao vivo e tempo de carregamento (R5A §7.2, §7.3) | **Tela do teste**: faixa acima do console; no resultado, "Abriu em…" e "O que mais demorou" (quando houver dados). | Automático em todo teste. |
| spark e perfil de desempenho (R5A §7.4) | Menu ▾ → **Testar com perfil de desempenho**; resultado ganha a parte "Desempenho". | Ação explícita. |
| Configs: busca, formulário com padrão, restaurar (R5B §2) | **Configs**: campo "Buscar em todas as configs" acima da árvore; no formulário, "padrão: X" e **Restaurar padrão** por chave. | Configs. |
| Editor de scripts KubeJS/CraftTweaker (R5B §3) | **Configs**: arquivos `.js` e `.zs` abrem no mesmo editor com recursos de script. Durante o teste, **Recarregar scripts** na tela do teste. | Árvore de Configs (`kubejs/`, `scripts/`). |
| Perfis de teste com Quick Play (R5B §4.1) | Menu ▾ → grupo **Perfil do teste** (troca rápida) e o diálogo **Ajustes do teste neste computador**, que ganha o seletor de perfil no topo. | Menu ▾. |
| Servidor local (R5B §4.2) | Menu ▾ → **Testar como servidor…**. Também valida o pacote para servidor e ajuda a busca do culpado quando preciso (sempre perguntando antes). Nunca faz parte do Testar normal. | Menu ▾, Exportar, busca do culpado. |
| Testar como o jogador recebe (R5B §4.3) | Menu ▾ (já aprovado). Agora instala pelo **link do pack**, como um jogador. | Menu ▾; aviso "versão não testada" ao publicar. |
| Pacote para servidor (R5B §5.2) | **Exportar**: formato novo, com a escolha de como os mods chegam ao servidor. | Exportar. |

### 13.3 Mudanças na estrutura (aprovadas pelo dono na D4)

| # | Mudança | Por que é indispensável |
|---|---|---|
| N1 | Descrição de **Problemas**: "Saúde do pack, problemas e travamentos" (era "O que pode impedir o jogo de abrir"). | A seção passa a ter a nota e a lista de travamentos. A descrição tem de dizer o que há dentro (regra 6). |
| N2 | **✦ Diagnóstico com IA** vira conversas. Descrição: "Conversar com a IA sobre um problema do pack". A página "Resposta da IA" deixa de existir: a resposta é a própria conversa. | Decisão do dono (D17): a IA consulta o pack aos poucos e conversa. Uma página de resposta fixa não comporta perguntas de seguimento. |
| N3 | Descrição de **Configs**: "Arquivos de ajuste e scripts do pack". | Decisão do dono (D20): os scripts KubeJS e CraftTweaker ficam ali. |
| N4 | **Abrir pack existente** vira **Abrir ou importar…** em Meus packs. | Decisão do dono (D20): também aceita arquivos de modpack de outros apps. |
| N5 | **Criar pack** ganha a etapa **Mods iniciais** (passa de 4 para 5 etapas). | Os mods padrão (spark e Crash Assistant, D16) e o kit de desempenho precisam estar à vista e desmarcáveis. Dentro do Resumo ficariam escondidos num lugar onde ninguém espera escolher nada. |
| N6 | **Adicionar** abre em tela cheia, e o menu lateral do pack recolhe para ícones (com nome no tooltip) enquanto ela está aberta. | A página de descoberta tem três colunas (filtros, resultados, pré-visualização) e precisa caber em 1024 px. É estado de exibição, não navegação nova: as 6 seções continuam a um clique. |
| N7 | O menu ▾ do Testar ganha grupos (Outros testes, Perfil do teste, Instância de teste) e quatro itens novos. O item "Testar", que repetia o botão principal, sai. | Os testes avançados são raros e ficam escondidos até serem precisos (regra 5). Os grupos com título mantêm o menu legível com 11 itens. |
| N8 | **Mods** ganha o seletor "Ver como: Lista · Grafo". | Modo de exibição dos mesmos dados (regra 3). |
| N9 | A tela do teste ganha a faixa de desempenho, o seletor "Mostrar" do console, "Mostrando: Servidor · Jogo" quando há servidor, e o modo **Busca do culpado**. | Tudo isso é sobre um teste em andamento ou já feito; o lugar natural é a tela do teste. |
| N10 | **Exportar** ganha o formato "Pacote para servidor (.zip)". | Decisão do dono (D22). |

Nenhuma sétima seção, nenhuma aba. A ADR-0036 registra estas mudanças.

### 13.4 Nomes novos

Revisados com a skill `design:ux-copy` e acrescentados ao glossário (`QUALITY.md` §8.2).

| Nome na interface | O que é | Por quê |
|---|---|---|
| **Encontrar o mod culpado** | Busca automática por rodadas (bisseção) | Diz o resultado que o usuário quer. "Bisseção" e "busca binária" são jargão. |
| **Rodada** | Cada vez que o jogo é aberto com parte dos mods | Palavra comum, dá a ideia de repetição com fim. |
| **Saúde do pack** | Nota de 0 a 100 com faixa (Ótimo, Bom, Atenção, Crítico) | Curto e familiar. O texto de apoio diz "resumo dos problemas conhecidos" para não parecer promessa de que o pack funciona. |
| **O que este mod altera no jogo** | Raio-x de mixins nos detalhes do mod | "Mixin" é termo técnico; a frase diz o efeito. Na lista completa, a coluna técnica mostra o tipo de alteração para quem quiser. |
| **Alteram o mesmo ponto do jogo** | Sobreposição de mixins | Nunca "são incompatíveis": a maioria das sobreposições é compatibilidade intencional (R5A §5.4). |
| **Ver como: Lista · Grafo** | Modo de exibição de Mods | "Grafo" fica com a explicação no tooltip ("quem precisa de quem"). |
| **Conversa** | Uma sessão de perguntas e respostas com a IA | O que a pessoa faz de fato. |
| **Enviado à IA** | Bloco que mostra, byte a byte, o que a IA consultou | Transparência pedida pelo dono (D17). |
| **Proposta** / **Aplicar** | Mudança sugerida pela IA e o botão que a executa | Deixa claro que nada muda sem o clique. |
| **Testar como servidor** | Teste com servidor local sob demanda | Par de "Testar como o jogador recebe". |
| **Perfil do teste** | Conjunto nomeado de ajustes do teste (memória, Java, mundo) | Os ajustes já se chamavam "Ajustes do teste"; o perfil é um conjunto deles com nome. |
| **Testar com perfil de desempenho** | Teste que mede tempo por mod e lê o spark | "Perfil" aqui no sentido de medição; o menu explica em uma linha ("mede o que mais pesa"). |
| **Pacote para servidor** | Server pack | Termo em português, igual ao padrão de "pack" do glossário. |
| **Abrir ou importar…** | Abrir pack packwiz ou importar de outro app | Decisão do dono. |
| **Mods iniciais** | Etapa do Criar pack com spark, Crash Assistant e kit | Diz o que se escolhe ali. |
| **Kit de desempenho** | Lista curada de mods de otimização | Termo já usado no R1. |

### 13.5 O que continua igual

- Os 5 fluxos principais da §8 não ganharam nenhum passo. As funções novas são caminhos laterais a partir deles (por exemplo, no fluxo 4, depois de "O jogo travou", **Encontrar o mod culpado** quando a causa não aparece).
- O botão **▶ Testar** continua fazendo só o teste normal. Servidor local, busca do culpado e perfil de desempenho nunca rodam sem o usuário pedir.
- O protótipo final (`design/prototipo-final/`) mostra as telas novas; o mapa do protótipo lista os caminhos.
