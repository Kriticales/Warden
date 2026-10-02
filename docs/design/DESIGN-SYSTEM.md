# Warden: design system (direção Deep Dark)

> Tarefa D3, 01/10/2026. Versão 1.0.
> Galeria com todos os componentes e estados: `design/system/index.html` (publicada como “Warden — Design System”).
> Protótipo final montado só com este sistema: `design/prototipo-final/index.html` (publicado como “Warden — Protótipo final”).
> Mapeamento para Tailwind CSS 4 + shadcn/ui: [`HANDOFF.md`](HANDOFF.md).
> Estrutura que este sistema veste (obrigatória, aprovada): [`ESTRUTURA.md`](ESTRUTURA.md).

Este documento diz **o que existe, quando usar cada coisa e o que evitar**. Os valores exatos ficam em um lugar só, `design/system/tokens.json`; aqui eles aparecem para leitura.

## Sumário

1. [Princípios](#1-princípios)
2. [Arquivos e como mudar um token](#2-arquivos-e-como-mudar-um-token)
3. [Fundamentos](#3-fundamentos): cor, contraste, tipografia, espaçamento, cantos e bordas, elevação, ícones, movimento, texturas
4. [Componentes](#4-componentes)
5. [Padrões de tela](#5-padrões-de-tela)
6. [Textos](#6-textos)
7. [Acessibilidade: o que foi verificado](#7-acessibilidade-o-que-foi-verificado)
8. [Crítica de design e o que foi corrigido](#8-crítica-de-design-e-o-que-foi-corrigido)
9. [Licenças e origem de cada coisa](#9-licenças-e-origem-de-cada-coisa)
10. [Pontos para o dono decidir](#10-pontos-para-o-dono-decidir)

---

## 1. Princípios

O visual parte do protótipo D1 (direção Deep Dark), que o dono aprovou: paleta do Warden, fontes pixel, menus com jeito de Minecraft, cantos em degrau, Testar como bloco ciano com pulso de sensor, progresso em blocos. A organização e o posicionamento do D1 foram descartados; tudo aqui veste a estrutura aprovada no D2.

1. **Pixel para reconhecer, sans para ler.** Fonte pixel em títulos curtos, no menu de seções, no Testar e em números grandes. Frases, descrições, tabelas, campos e botões comuns ficam na Manrope. Pixel só a partir de 16 px.
2. **Brilho só com significado.** O ciano acende onde algo está ativo: Testar pronto, item selecionado, seção atual, progresso. Nenhum brilho decorativo.
3. **Blocos, não bolhas.** Cantos retos e degrau de pixel; nada de raio grande nem pílula. Elevação com sombra dura e curta, como um bloco sobre o chão.
4. **Estado nunca só na cor.** Todo estado tem ícone e palavra. Fontes de mod têm marcadores de formas diferentes.
5. **Densidade de ferramenta.** Listas em tabela onde há o que comparar; nada de grade de cartões iguais.
6. **Sem abas.** A estrutura proíbe abas e abas dentro de abas. Use seção do menu, seletor de modo, caixa de seleção, página corrida ou camada (diálogo, painel lateral).
7. **Brilhinho (sparkles) só para IA.** Pedido do dono: o ícone marca o Diagnóstico com IA e mais nada.

## 2. Arquivos e como mudar um token

| Arquivo | O que é | Editar? |
|---|---|---|
| `design/system/tokens.json` | **Fonte única** dos tokens (formato Design Tokens, com referências `{grupo.nome}`) | Sim |
| `design/system/tokens.css` | Variáveis CSS geradas (galeria, protótipo) | Não: gerado |
| `design/system/tailwind-theme.css` | Bloco `@theme` do Tailwind CSS 4, gerado | Não: gerado |
| `design/system/shadcn-theme.css` | Variáveis que o shadcn/ui espera (`--background`, `--primary`…), geradas | Não: gerado |
| `design/system/components.css` | Componentes (só usam tokens) | Sim |
| `design/system/components.js` | Funções que geram o HTML de cada componente (as props viram props React) | Sim |
| `design/system/behavior.js` | Teclado e foco (menus, diálogos, tooltip, toggles) | Sim |
| `design/system/icons.js` | Ícones Lucide usados, gerado | Não: gerado |
| `design/system/fonts/` | Fontes OFL em woff2 e as licenças | Não |
| `design/system/contraste.md` | Tabela de contraste, gerada | Não: gerado |

Para mudar um valor:

```bash
# 1. edite design/system/tokens.json
node design/system/tools/build-tokens.mjs   # gera tokens.css, tailwind-theme.css, shadcn-theme.css
node design/system/tools/contraste.mjs      # confere os 88 pares de contraste; sai com erro se algum reprovar
```

Regra: **componente nunca usa cor, tamanho, raio, sombra ou duração literais**. Exceções documentadas: os SVGs das texturas e da pixel art (que levam as cores da paleta escritas dentro do desenho) e as porcentagens de layout.

## 3. Fundamentos

### 3.1 Cor

Abismo quase preto, corpo azul-petróleo do Warden, brilho ciano das almas, osso das costelas. Os componentes usam os **papéis** (`--color-*`); a paleta crua existe só como referência em `tokens.json`.

| Papel | Valor | Uso |
|---|---|---|
| `bg` | `#050d12` | Fundo da janela |
| `bg-sunken` | `#03090c` | Campos, console, item ativo do menu (afundados) |
| `surface-1` | `#0a1a21` | Painéis, cabeçalhos, menus |
| `surface-2` | `#0e2630` | Botão secundário, linha em hover |
| `surface-3` | `#12313c` | Hover sobre `surface-2`, trilhos |
| `border` / `border-strong` | `#15333e` / `#1e4552` | Divisórias / bordas de painel |
| `border-control` | `#4f8a98` | Contorno de campos e controles (3:1) |
| `text` / `text-2` / `text-3` | `#e4f3ef` / `#b4cfca` / `#8aacaa` | Principal / secundário / apagado |
| `primary` | `#19d3e0` | Ação principal, seleção, Testar, progresso |
| `primary-hover` / `primary-press` | `#0fb5c2` / `#0a8f9a` | Estados do botão primário |
| `on-primary` | `#021418` | Texto sobre o ciano (10,25:1) |
| `primary-text` | `#5fe3ec` | Ciano como texto ou link |
| `primary-soft` / `primary-line` / `primary-glow` | ciano a 12% / 40% / 45% | Fundo de selecionado / contorno / brilho |
| `focus` | `#cfe3da` (osso) | Anel de foco do teclado |
| `ok` · `warn` · `danger` · `info` | `#3ddc84` · `#f2b33d` · `#ff5c5c` · `#b3a6ff` | Marcadores de estado; cada um tem `-text` (para texto) e `-soft` (fundo) |
| `danger-solid` | `#c42f35` | Fundo do botão de perigo |
| `ai` / `ai-soft` / `ai-line` | osso, 7%, 45% | Tudo que é IA |
| `src-modrinth` / `src-curseforge` / `src-local` | verde / laranja / cinza | Marcadores de fonte |
| `diff-add` / `diff-del` | verde 14% / vermelho 14% | Linhas do diff |
| `console-bg` / `console-text` / `console-muted` | `#020709` / `#cfe6e1` / `#86a9a6` | Console |

Decisões:
- **Foco em osso, não em ciano.** Ciano já quer dizer “selecionado”. Com o foco também em ciano, não daria para distinguir onde o teclado está do que está escolhido.
- **Informação em lavanda.** Azul comum ficaria parecido com o ciano. A lavanda aparece pouco (selos e avisos informativos); nunca como gradiente.
- **IA em osso, nunca em roxo.** Roxo e gradiente são o clichê de “IA mágica” que o dono pediu para evitar.
- **Um tema só, escuro.** O tema claro (Calcita) do D1 saiu: a tarefa pede uma única direção. Os tokens estão prontos para receber outro tema no futuro (bastaria outro bloco de valores).

### 3.2 Contraste

Verificado por `node design/system/tools/contraste.mjs` (WCAG 2.1, cores translúcidas compostas sobre o fundo real): **88 pares, todos aprovados**. Tabela completa em `design/system/contraste.md` e na galeria. Destaques:

| Frente | Fundo | Contraste | Mínimo | Resultado |
|---|---|---|---|---|
| `text` | `bg` | 17,12:1 | 4,5:1 | AAA |
| `text-2` | `surface-1` | 10,75:1 | 4,5:1 | AAA |
| `text-3` | `surface-3` (pior caso) | 5,60:1 | 4,5:1 | AA |
| `primary-text` | `surface-3` | 8,94:1 | 4,5:1 | AAA |
| `on-primary` | `primary` (Testar, botão primário) | 10,25:1 | 4,5:1 | AAA |
| `on-primary` | `primary-press` | 4,85:1 | 4,5:1 | AA |
| `on-danger` | `danger-solid` | 5,52:1 | 4,5:1 | AA |
| `danger-text` | `surface-3` | 6,22:1 | 4,5:1 | AA |
| `border-control` | `surface-3` (pior caso) | 3,54:1 | 3:1 | AA |
| `focus` | `surface-3` | 10,23:1 | 3:1 | AA |
| `console-muted` | `console-bg` | 7,95:1 | 4,5:1 | AAA |

Estados desabilitados usam `text-3` sobre `surface-1` (7,24:1): continuam legíveis, sem depender de opacidade.

### 3.3 Tipografia

| Família | Token | Uso |
|---|---|---|
| **Pixelify Sans** | `--font-display` | Títulos curtos de tela e de painel, nome do pack, menu de seções, Testar, números grandes |
| **Manrope** | `--font-ui` | Todo o texto de leitura, botões, campos, tabelas, títulos de diálogo |
| **IBM Plex Mono** | `--font-mono` | Console, versões, caminhos, configs, diff |

Escala (desktop denso, base 14 px):

| Token | Tamanho / altura | Uso |
|---|---|---|
| `2xs` | 11/14 | Selos em caixa alta (`ERRO`, `AVISO`) e rótulos de seção do bloco da IA |
| `xs` | 12/16 | Metadados, legendas, descrição do menu |
| `sm` | 13/18 | Tabelas, descrições, texto de ajuda |
| `base` | 14/20 | Corpo do app |
| `md` | 16/22 | Itens do menu de seções e grupos (pixel) |
| `lg` | 20/24 | Título de painel (pixel) e de diálogo (sans 800) |
| `xl` | 24/28 | Título de tela e nome do pack (pixel) |
| `2xl` | 32/36 | Boas-vindas e números de destaque (pixel) |

Regras:
- Texto pixel sempre com `--shadow-pixel-text` (sombra dura de 2 px para baixo e para a direita), como o texto dos menus do jogo.
- **Pixel só em títulos curtos (até umas 4 palavras) e a partir de 16 px.** Frases e títulos de diálogo vão para a Manrope. Números pequenos (etapas, passos numerados) também: a 13 px o “2” da Pixelify parece um “8”.
- No máximo **uma palavra acesa** em ciano (`.t-hl`) por tela, quando ela é o assunto (“Boas-vindas ao **Warden**”).
- Caixa alta só nos selos de estado e nos rótulos do bloco da IA, com `--tracking-caps`.
- Números com `tabular-nums` (já no `body`).

### 3.4 Espaçamento

Base de 4 px: `--space-1` (4) a `--space-16` (64): 4, 8, 12, 16, 20, 24, 32, 40, 48, 64. No Tailwind, `--spacing: 4px`, então `p-4` = 16 px.

| Medida | Token | Valor |
|---|---|---|
| Altura de controle | `--size-control-sm/md/lg` | 28 / 36 / 48 px |
| Barra do app | `--size-topbar` | 56 px |
| Cabeçalho do pack | `--size-pack-header` | 76 px (mais a faixa de avisos, quando há) |
| Menu de seções | `--size-section-menu` | 252 px (236 abaixo de 1180 px) |
| Rodapé | `--size-statusbar` | 30 px |
| Painel lateral | `--size-drawer` | 440 px |
| Diálogos | `--size-dialog-sm/md/lg` | 480 / 600 / 780 px |
| Janela mínima | `--size-window-min-w` | 1024 px |

Respiro padrão: 24 px em volta do conteúdo, 16 a 20 px dentro de painéis, 8 px entre controles da mesma barra.

### 3.5 Cantos e bordas

- **Raio:** `none` (padrão de tudo), `xs` 2 px (campos), `sm` 4 px (raro). Nada acima disso.
- **Degrau de pixel** (`--notch-md` 4 px, `--notch-sm` 2 px): recorte em escada nos cantos de **elementos pequenos e interativos**: botões, Testar, selos, contadores, marcadores das etapas. O recorte fica em `::before` (borda) e `::after` (preenchimento), nunca no próprio botão, para o anel de foco não ser cortado.
- **Moldura dupla** (borda `border-strong` de 1 px + contorno externo quase preto de 1 px): diálogos, menus, tooltip, painel lateral, console. Cantos retos.
- **Larguras de borda:** `thin` 1 px (divisórias, painéis), `block` 2 px (progresso, foco), `bar` 3 px (barra de seleção, barra de alerta, item ativo do menu).

### 3.6 Elevação

| Token | Uso |
|---|---|
| `--shadow-block-1` | Painel forte, cartão de pack |
| `--shadow-block-2` | Menus, toast, painel lateral |
| `--shadow-block-3` | Diálogos |
| `--shadow-glow` | Só selecionado/ativo (ex.: painel escolhido) |
| `--inset-shadow-bevel` / `-bevel-press` | Chanfro de bloco nos botões / botão pressionado |
| `--inset-shadow-slot` | Campos e item ativo do menu: “afundado”, como um slot de inventário |

Sombras são duras e curtas (deslocadas para baixo), não nuvens desfocadas. Cada camada usa um nível só.

### 3.7 Ícones

- **Lucide** (licença ISC), traço 2 px, tamanhos 14, 16 (padrão) e 20. No React, `lucide-react` com os mesmos nomes (`triangle-alert` → `<TriangleAlert />`). Lista dos 72 usados em `design/system/icons.js` e na galeria.
- **`sparkles` só para IA**, sempre em `--color-ai` (osso) fora de botão primário.
- Ícone sem texto só em ações muito conhecidas (fechar, editar, mais ações), sempre com nome acessível e tooltip.
- **Pixel art própria**, desenhada por código: a marca do Warden (bloco com duas antenas e núcleo de alma), o ícone provisório de mod e de pack (padrão 8×8 simétrico gerado do nome, só quando a API não tem ícone oficial), as ilustrações dos estados vazios (bloco com símbolo), o visto do checkbox, o círculo do radio e a seta do select.

### 3.8 Movimento

| Token | Valor | Uso |
|---|---|---|
| `--duration-instant` | 80 ms | Botão pressionado |
| `--duration-fast` | 120 ms | Hover, foco, cor, alavanca do toggle |
| `--duration-base` | 200 ms | Abrir menu, toast |
| `--duration-slow` | 320 ms | Diálogo, painel lateral |
| `--duration-pulse` | 3,4 s | Pulso do sensor do Testar pronto |
| `--ease-out` | `cubic-bezier(0.2, 0, 0, 1)` | Entradas |
| `--ease-in-out` | `cubic-bezier(0.4, 0, 0.2, 1)` | Pulso |
| `--ease-pixel` | `steps(4, end)` | Animações em degraus (carregador de 4 blocos, progresso indeterminado, alavanca) |

**Movimento reduzido:** com `prefers-reduced-motion: reduce` ou a opção “Menos movimento” do app (`data-motion="reduced"` no `<html>`), todas as durações viram 0, o pulso do Testar para (fica um brilho fixo), o carregador fica parado, o progresso indeterminado vira uma barra apagada e o skeleton para de varrer. Verificado emulando a preferência.

### 3.9 Texturas e padrões

Desenhos próprios em SVG (nenhum asset da Mojang ou da Microsoft). **Pouco e sempre nos mesmos lugares.**

| Padrão | Onde pode | Onde não pode |
|---|---|---|
| **Veia de sculk** (`--fx-vein`, ciano a 7%) | Fundo da barra do app e do cabeçalho do pack | Atrás de listas, editores, console, diálogos |
| **Camada de sculk** (`--fx-sculk-edge`, 6 px) | Borda de baixo da barra do app e do cabeçalho do pack, como a grama no topo de um bloco | Qualquer outro lugar |
| **Separador em blocos** (`.sep-blocks`) | Entre grupos grandes de uma página, no máximo 1 ou 2 por tela | Entre itens de lista |
| **Trilho pontilhado em blocos** | Ligação das etapas, linha do tempo, separador de menu | Bordas de painel |

A cena de caverna desfocada atrás do conteúdo, que existia no D1, **saiu**: atrás de tabelas densas ela atrapalha a leitura e é justamente o traço que mais lembra tela de launcher (ver §10).

## 4. Componentes

Todos estão na galeria com os estados. As funções em `components.js` têm o mesmo nome do componente React sugerido no HANDOFF.

| Componente | Quando usar | Estados mostrados | Evite |
|---|---|---|---|
| **Botão** primário (bloco ciano) | A ação principal da área. Um por área | padrão, hover, foco, pressionado, desabilitado, carregando | Dois primários lado a lado |
| Botão secundário | Ações comuns | idem | Usar para a ação principal |
| Botão fantasma | Cancelar, ações terciárias, barras de ferramentas | idem | Ação principal |
| Botão perigo / perigo secundário | Remover, apagar, descartar. O sólido só na confirmação | idem | Perigo sólido fora de confirmação |
| Link | Ação dentro de frase, “Ver problemas” | padrão, hover, foco | Ações que mudam dados |
| **Testar** | Só no cabeçalho do pack | pronto (pulsa), hover, foco, testando (barra interna), jogo aberto (escuro, ponto verde), desabilitado | Outro botão com essa forma |
| Campo de texto | Entrada livre | vazio, preenchido, hover, foco, erro, desabilitado, somente leitura, com ícone, carregando | Placeholder no lugar do rótulo |
| **Busca ou link** | Adicionar: busca combinada e link colado no mesmo campo | vazio, buscando, com resultados, link reconhecido, link inválido | Abas por fonte |
| Select | Escolha em lista curta, filtros | padrão, foco, erro, desabilitado, pequeno (em tabela) | Muitas opções sem busca |
| Toggle | Ligar/desligar com efeito imediato | desligado, ligado, foco, desabilitado | Escolha que só vale ao salvar (use checkbox) |
| Checkbox | Marcar itens, opções de formulário | desmarcado, marcado, indeterminado, foco, desabilitado, erro | — |
| Radio (círculo de pixel) | Uma entre poucas opções | desmarcado, marcado, desabilitado | — |
| Escolha em bloco | Radio grande com explicação (formato, loader, público/privado, cofre/.env) | padrão, hover, marcado, desabilitado, foco | Mais de 6 opções |
| Seletor de modo | Formulário × Texto do mesmo arquivo | padrão, opção indisponível | Usar como abas de navegação |
| Memória | Memória do teste | dentro do recomendado, fora (aviso), desabilitado | — |
| Selo de fonte | Origem do mod: Modrinth (círculo de pixel), CurseForge (losango), arquivo local (quadrado vazado), link direto, as duas | — | Cor sem marcador |
| Selo de lado | Cliente e servidor, Só cliente, Só servidor, desconhecido | — | “Ambos”, “side” |
| Badge de estado | Erro, Aviso, Info, OK, P1 | — | Estado só com cor |
| Contador | Menu de seções e botões | neutro, perigo (há erro), aviso (não salvas) | Contador “0” |
| **Linha de mod** | Lista de Mods (densa, 2 linhas) | padrão, hover, selecionada, erro, aviso, atualização disponível, verificando, recém-adicionada, arquivo inválido, fora do índice, carregando | Cartões em grade |
| Barra de seleção | Quando há itens marcados | — | Ações em lote escondidas |
| **Cabeçalho do pack** | Topo de toda tela do pack | padrão, com aviso, testando, jogo aberto, nada para salvar | Menu de navegação aqui |
| **Menu de seções** | As 6 seções do pack, nesta ordem | ativo, hover, foco, contadores | Esconder a descrição; reordenar |
| Barra do app e rodapé | Nível do app; Tarefas no rodapé | rodapé ocioso, ocupado, com falha | Menu lateral no nível do app |
| Linha de pack | Meus packs | normal, travou, nunca testado, pasta não encontrada, ilegível | Grade de cartões |
| Cartão de pack | Resumo de UM pack (abrir existente, criar pack) | — | Vários cartões em grade |
| **Barra de progresso em blocos** | Downloads, verificação, exportação | determinada, indeterminada, esperando você, erro, concluída, pequena | Spinner para tarefa longa |
| **Etapas** | Teste (5 etapas), assistentes | em andamento, esperando você, erro, concluído | Etapas clicáveis |
| **Console** | Saída ao vivo do jogo, sem tradução | ao vivo, pausado, esperando o jogo, encerrado | Traduzir o log |
| **Diff** | Antes de gravar config, conflito, histórico | linhas adicionadas, removidas, contexto, trechos dobrados | Cor sem o sinal + ou − |
| Diff por chave | `options.txt` e afins | — | — |
| **Aviso inline** | Info, ok, aviso, perigo, neutro, IA | com e sem ações, compacto | Aviso flutuando sobre o conteúdo |
| Problema | Problemas e checagem do teste: o que é, por que importa, como sabemos, botão que corrige | erro, aviso, informação | Problema sem ação |
| **Toast** | Confirmar algo que acabou de acontecer | ok, info, erro | Única cópia de um erro que exige ação |
| **Diálogo** | Confirmação, revisão antes de gravar | pequeno, médio, grande; alertdialog para destrutivo | Diálogo sobre diálogo |
| **Painel lateral** | Detalhes sem perder o lugar, Tarefas | — | Formulários longos |
| Menu suspenso | ▾ do Testar, ⋯ de um pack | item com descrição, perigo, desabilitado, separador | Ações frequentes escondidas |
| **Tooltip** | O que não cabe no rótulo; botões só de ícone | — | Repetir o rótulo |
| **Estado vazio** | Lista sem itens, sem resultado, nenhum problema, erro de carregamento | 4 variantes | Texto de vitrine, centralizado |
| **Skeleton** | Carregamento com forma conhecida | — | Spinner girando no meio da tela |
| **Tabela** | Dados para comparar | ordenável, numérica | — |
| **Linha do tempo** | Histórico | não salvas, só salva, versão final, publicada, aberta com detalhes | — |
| **Bloco de resposta da IA** | Tudo que vem da IA | resposta, carregando, erro | IA sem o aviso “pode errar” |
| Editor de configs | Texto (CodeMirror) e formulário | linha alterada, linha com erro, carregando | Editor sem diferença antes de salvar |
| Árvore, chave-valor, divulgação, tecla | Configs, detalhes, “Mais opções” | — | — |
| **Abas** | **Não existem no Warden** | — | — |

Contrato de teclado (reproduzido em `behavior.js` e exigido no app via Radix):
- **Menu:** Enter, Espaço ou ↓ abrem e focam o primeiro item; ↑ ↓ Home End navegam; Esc fecha e devolve o foco ao botão.
- **Diálogo e painel lateral:** o foco entra no título; Tab e Shift+Tab ficam presos; Esc fecha; o foco volta para quem abriu.
- **Tooltip:** aparece no hover e no foco do teclado; Esc esconde. Com um tooltip aberto dentro de um diálogo, o primeiro Esc fecha só o tooltip e o segundo fecha o diálogo (mesmo comportamento do Radix).
- **Seletor de modo:** setas trocam a opção.
- **Árvore:** ↑ ↓ movem; Enter abre a pasta.
- **Troca de tela:** o foco vai para o título da página (para o leitor de tela anunciar onde se está).

## 5. Padrões de tela

Seguem a Alternativa A aprovada (ver `ESTRUTURA.md`):

- **Nível do app:** barra do app (marca, onde você está, Configurações), conteúdo, rodapé com Tarefas. Sem menu lateral.
- **Nível do pack:** cabeçalho fixo (← Meus packs, nome com “Editar informações”, Minecraft · loader · versão, Salvar versão · N alterações, ▶ Testar ▾); menu de 6 seções à esquerda; conteúdo; rodapé. Os **avisos do momento** (mudanças do teste para revisar, pack mudou durante o teste) ficam numa faixa dentro do cabeçalho, logo abaixo da linha principal, e só aparecem quando existem (ver §8).
- **Cabeçalho de página:** título curto em pixel, uma linha dizendo o que há ali, ações à direita. Voltar (“← Voltar para Mods”) acima do título quando é subpágina.
- **Teste:** é um modo, não uma seção (nenhum item do menu acende). Título, etapas sempre visíveis, conteúdo da etapa.
- **Larguras:** de 1024 a 1180 px o menu de seções vai para 236 px, a descrição do mod encurta e o botão Salvar versão mostra só o número (o nome completo continua no leitor de tela).

## 6. Textos

Revisados com a skill `design:ux-copy`, seguindo o glossário do `QUALITY.md` §8.2:
- Botão diz o que acontece: verbo + objeto (“Adicionar 2 itens”, “Trazer 3 selecionados para o pack”, “Remover JEI”). Confirmações repetem a ação, nunca “OK” ou “Sim”.
- Erro = o que aconteceu + o que fazer (“O download parou: a conexão caiu. O que já foi baixado fica guardado.”).
- Estado vazio = o que é + por que está vazio + o que fazer.
- Sem exclamação, sem slogan, sem excesso de travessão, sem título óbvio.
- Termos fixos: Testar (nunca Jogar), console, travou, instância de teste, Copiar o pack para o teste, O que mudou durante o teste, Salvar versão, versão final, Publicar versão, link do pack, Configs × Configurações, lado (Cliente e servidor / Só cliente / Só servidor).

## 7. Acessibilidade: o que foi verificado

Revisado com a skill `design:accessibility-review` (WCAG 2.1 AA). **Verificado executando** (Chromium headless, Playwright 1.63, axe-core 4):

| Verificação | Resultado |
|---|---|
| axe-core (WCAG 2.0/2.1 A e AA + boas práticas) na galeria, em 1280 e 1024 | 0 violações |
| axe-core nas **90 combinações** de tela e estado do protótipo, em 1280 e 1024 | 0 violações |
| Erros de JavaScript nas mesmas páginas | 0 |
| Rolagem horizontal da página ou conteúdo estourando | nenhum no protótipo; na galeria, só a tabela de packs em 1024 rola dentro do próprio contêiner (a galeria tem uma coluna de índice que o app não tem) |
| Contraste dos tokens | 88 pares, todos aprovados (§3.2) |
| Teclado (18 testes automatizados) | diálogo: foco entra no título, Tab e Shift+Tab presos, Esc fecha e devolve o foco; menu do Testar: Enter e ↓ abrem, ↓ e End navegam, Esc devolve o foco; menu de seções: Enter navega e o foco vai para o título; painel lateral: foco entra, Esc fecha; atalho “Pular para o conteúdo”; anel de foco visível em todos os controles amostrados |
| Movimento reduzido | emulado: pulso, carregador e durações param; a opção do app também funciona |
| Links do protótipo | nenhum aponta para tela inexistente |

Garantias do sistema: rótulo visível em todo campo; erros ligados ao campo (`aria-invalid`, `aria-describedby`); `aria-current` na seção ativa e na etapa atual; `role="progressbar"` com valores; console com `role="log"`; toasts com `role="status"` (erro com `role="alert"`); confirmações destrutivas com `role="alertdialog"`; alvos de clique de no mínimo 28 px (acima dos 24 px da WCAG 2.2; o critério de 44 px da 2.5.5 é AAA e voltado a toque).

**Não verificado** (fica para os testes do app): leitor de tela real (NVDA, Narrador), o WebView2 do Tauri e zoom de 200%. Sobre o zoom: o app tem janela mínima de 1024 px; com 200% numa tela de 1280 px, aparece rolagem horizontal. Para um app de mesa isso é aceitável, mas o frontend deve testar 150% e 200% (escala do Windows) e garantir que nada some.

## 8. Crítica de design e o que foi corrigido

Feita com a skill `design:design-critique` sobre capturas reais das telas, com quatro lentes: originalidade × cópia de launchers, “cara de IA”, clareza para leigo e consistência com o sistema.

### 8.1 Achados e correções

| Achado | Lente | Gravidade | Correção |
|---|---|---|---|
| No cabeçalho do pack, os avisos do momento disputavam espaço com o nome e os botões e se sobrepunham em 1024 px. | Clareza | Alta | Avisos foram para uma faixa própria dentro do cabeçalho, logo abaixo da linha principal, só quando existem. O conteúdo do cabeçalho continua o da estrutura. |
| A tabela de mods estourava a largura em 1280 e 1024 px (coluna Marcas cortada). | Clareza | Alta | As marcas foram para o lado do nome; versão e atualização viraram uma coluna só (“0.5.1.j” e embaixo “→ 0.5.1.k”); a coluna ⋯ saiu (as ações estão no painel de detalhes e na barra de seleção). |
| Radios quadrados pareciam checkboxes: o leigo não sabe se pode marcar vários. | Clareza | Média | Radio virou um círculo em pixel art, com ponto ciano. |
| Números das etapas em pixel a 13 px: o “2” parecia “8”. | Clareza | Média | Números pequenos voltaram para a Manrope; regra “pixel só a partir de 16 px”. |
| O aviso “4 atualizações disponíveis” em lavanda forte no topo de Mods chamava mais atenção que a lista. | Hierarquia / “cara de IA” | Média | Virou aviso neutro compacto. |
| Diálogos longos abriam com o foco (e o anel) no primeiro botão do meio do texto. | Acessibilidade | Média | O foco entra no título do diálogo. |
| “Pular para o conteúdo” usava o hash e, no protótipo, mudava de tela. | Acessibilidade | Média | O atalho move o foco por script. |
| No “Jogo aberto”, a caixa de simulação do protótipo cobria o fim do console. | Protótipo | Baixa | Altura do console recalculada. |
| Selos “P1” não diziam nada ao dono. | Clareza | Baixa | Tooltip: “Prioridade 1: entra logo depois da primeira versão do app”. |
| “Abrir página” quebrava para uma segunda linha no rodapé do painel de detalhes. | Consistência | Baixa | Foi para perto do nome do mod. |
| A diferença por chave espalhava a seta pela linha inteira. | Consistência | Baixa | Grade corrigida. |
| “Mais opções” usava o estilo de evidência de problema. | Consistência | Baixa | Novo componente “divulgação” (`.disclosure`). |
| Primeira publicação aparecia depois de o pack já ter repositório (história incoerente herdada do rascunho). | Coerência | Baixa | A primeira publicação aparece no Histórico de um pack nunca publicado (estado Vazio). |

### 8.2 O que ficou de propósito

- **Originalidade.** Nenhum hero, nenhum botão Jogar gigante embaixo, nenhuma arte de mundo atrás do conteúdo, nenhum trilho só de ícones: a organização é de ferramenta (cabeçalho fixo + menu com descrições + tabelas). O que é “de Minecraft” está nos detalhes feitos sob medida: fonte pixel com sombra dura, degrau de pixel, chanfro de bloco, slot afundado no item ativo, camada de sculk na borda do cabeçalho, antenas no Testar, progresso em blocos, radio e visto em pixel. O Testar grande no canto superior direito lembra um botão Jogar de launcher, mas é exigência da estrutura e tem forma própria (antenas, pulso de sensor, estados de teste).
- **“Cara de IA”.** Sem gradientes, sem vidro fosco, sem cartões iguais em grade, sem emojis, sem cantos arredondados, sem textos de marketing. O brilho existe em quatro lugares, todos com significado: Testar pronto, seção atual, item selecionado e progresso.
- **“Editar informações” como ícone de lápis** ao lado do nome do pack, com tooltip: em 1024 px não cabe o texto sem empurrar o Testar. Ver §10.

## 9. Licenças e origem de cada coisa

| Item | Origem | Licença |
|---|---|---|
| Pixelify Sans | The Pixelify Sans Project Authors (github.com/eifetx/Pixelify-Sans), arquivos do Fontsource 5.3.0 | SIL OFL 1.1 (`design/system/fonts/OFL-PixelifySans.txt`) |
| Manrope | The Manrope Project Authors (github.com/sharanda/manrope), Fontsource 5.3.0 | SIL OFL 1.1 (`OFL-Manrope.txt`) |
| IBM Plex Mono | IBM Corp., Fontsource 5.3.0 | SIL OFL 1.1 (`OFL-IBMPlexMono.txt`) |
| Ícones | Lucide v1.49.0 (lucide.dev), via `lucide-static` | ISC |
| Pixel art (marca, ícones provisórios, estados vazios, visto, radio, seta, antenas, veia e camada de sculk) | Criação original desta tarefa, desenhada por código | Do projeto |
| Ferramentas de verificação (fora do repositório) | Playwright 1.63 (Apache-2.0), axe-core 4 (MPL-2.0) | Só para verificar |

Nenhuma textura, logo, personagem, fonte ou asset da Mojang ou da Microsoft. “Warden”, sculk e Deep Dark são só inspiração de cor e tema. Nomes de mods reais aparecem como exemplo; versões, logs e datas são inventados.

## 10. Pontos para o dono decidir

1. **A arte de caverna atrás do conteúdo saiu.** No D1 havia uma cena em pixel art, escurecida e desfocada, atrás de tudo. Ela atrapalha a leitura das tabelas e é o que mais lembra tela de launcher, então ficou só a veia de sculk no cabeçalho. Se o dono sentir falta, ela pode voltar só na tela de boas-vindas e em Meus packs vazio.
2. **“Editar informações” como lápis ao lado do nome do pack** (com tooltip), e não como texto. É o único botão só de ícone que é ação própria do pack.
3. **Ícone provisório dos mods:** quando a API não tem ícone (arquivo local, link direto), o Warden mostra um desenho em pixel gerado do nome. Com ícone oficial, usa o oficial.
4. **Sem tema claro.** Só existe o Deep Dark. Um tema claro custaria só um novo bloco de valores nos tokens, se um dia for preciso.
