# Warden: handoff do design para o frontend

> Tarefa D3, 01/10/2026. Para os agentes que vão construir a interface em React + TypeScript com **Tailwind CSS 4 + shadcn/ui (Radix) + lucide-react + CodeMirror 6** (ADR-0020).
> Referência visual e de comportamento: `design/prototipo-final/index.html` (cada tela tem a referência da SPEC na barra do protótipo).
> Regras de uso: [`DESIGN-SYSTEM.md`](DESIGN-SYSTEM.md). Estrutura obrigatória: [`ESTRUTURA.md`](ESTRUTURA.md).

## Sumário

1. [Como levar o tema para o app](#1-como-levar-o-tema-para-o-app)
2. [Tokens → utilitários do Tailwind](#2-tokens--utilitários-do-tailwind)
3. [Componentes → shadcn/ui e Radix](#3-componentes--shadcnui-e-radix)
4. [Estados e interações](#4-estados-e-interações)
5. [Layout e larguras](#5-layout-e-larguras)
6. [Movimento](#6-movimento)
7. [Acessibilidade obrigatória](#7-acessibilidade-obrigatória)
8. [Conteúdo, truncamento e casos de borda](#8-conteúdo-truncamento-e-casos-de-borda)
9. [O que é só do protótipo](#9-o-que-é-só-do-protótipo)
10. [Checklist de aceite visual](#10-checklist-de-aceite-visual)

---

## 1. Como levar o tema para o app

A fonte única é `design/system/tokens.json`. O script gera três saídas; **nenhuma deve ser editada à mão**.

```bash
node design/system/tools/build-tokens.mjs   # tokens.css, tailwind-theme.css, shadcn-theme.css
node design/system/tools/contraste.mjs      # falha se algum par de contraste reprovar (rodar no CI)
```

No frontend (Tailwind 4 é configurado por CSS, sem `tailwind.config.js`):

```css
/* src/styles/app.css */
@import "tailwindcss";
@import "./warden/fonts.css";            /* cópia de design/system/fonts/fonts.css + os .woff2 */
@import "./warden/tailwind-theme.css";   /* @theme do Warden: zera as paletas padrão do Tailwind */
@import "./warden/shadcn-theme.css";     /* --background, --primary, --ring… para os componentes do shadcn */
@import "./warden/components.css" layer(components); /* identidade (degrau, chanfro, slot, texturas) */
```

Recomendações:
- **Copie os arquivos gerados por script** (por exemplo, um passo `pnpm tokens` que roda o gerador e copia para `src/styles/warden/`), para o app nunca divergir do design.
- **Porte `components.css` como camada `components`.** O degrau de pixel usa `::before`/`::after` com `clip-path`, o chanfro usa sombras internas e as texturas são SVG: isso fica melhor como classes (`.btn`, `.testbtn`, `.secmenu__item`…) do que como utilitários soltos. Os componentes React aplicam essas classes (via `cva`) e usam utilitários do Tailwind só para layout (`flex`, `gap-3`, `grid-cols-…`).
- **Fontes embutidas** (decisão do dono: nada da internet): os `.woff2` vão no bundle do Vite; `font-display: swap`. Subconjunto latino cobre todo o português.
- **Tema escuro fixo:** `color-scheme: dark` já vem no `:root`. Não há tema claro.
- **Verificado executando** (Tailwind CSS 4 + `@tailwindcss/cli`, 01/10/2026): com `tailwind-theme.css` e `shadcn-theme.css` importados, geram CSS os utilitários `bg-surface-1`, `text-text-2`, `bg-primary`, `text-on-primary`, `p-4`, `rounded-none`, `rounded-xs`, `shadow-block-2`, `inset-shadow-bevel`, `font-display`, `text-sm`, `font-extrabold`, `ease-pixel`, `duration-(--duration-fast)`, `animate-sensor`, `outline-focus`, `h-(--size-control-md)` e os do shadcn (`bg-background`, `bg-accent`, `ring-ring`, `text-primary-foreground`, `border-input`); `bg-blue-500` e `rounded-lg` não geram nada.
- O `@theme` zera `--color-*`, `--shadow-*`, `--radius-*`, `--font-*` e `--text-*` padrão: `bg-blue-500` ou `rounded-lg` **deixam de existir**, de propósito. Se um utilitário não existe, o token não existe: peça um token novo em vez de escrever valor literal.

## 2. Tokens → utilitários do Tailwind

| Grupo | Variável CSS | Utilitário Tailwind | Observação |
|---|---|---|---|
| Cor | `--color-surface-1` | `bg-surface-1`, `border-surface-1` | Todos os papéis de `DESIGN-SYSTEM.md` §3.1 |
| Cor de texto | `--color-text-2` | `text-text-2` | O nome do papel repete o prefixo; é esperado |
| Ciano | `--color-primary`, `--color-on-primary` | `bg-primary text-on-primary` | O shadcn também enxerga `primary-foreground` com o mesmo valor |
| Foco | `--color-focus` | `outline-focus`, `ring-ring` | `ring` do shadcn = osso |
| Família | `--font-display/ui/mono` | `font-display`, `font-ui`, `font-mono` | `font-ui` é o padrão do `body` |
| Tamanho | `--text-sm` + `--text-sm--line-height` | `text-sm` | Já leva a altura de linha |
| Peso | `--font-weight-extrabold` | `font-extrabold` | |
| Espaçamento | `--spacing: 4px` | `p-4` = 16 px, `gap-2` = 8 px | `--space-N` existe para CSS puro |
| Raio | `--radius-none/xs/sm` | `rounded-none`, `rounded-xs`, `rounded-sm` | Componentes do shadcn vêm com `rounded-md`: **troque** por `rounded-none` (ou `rounded-xs` em campos) |
| Sombra | `--shadow-block-1/2/3`, `--shadow-glow` | `shadow-block-2`, `shadow-glow` | |
| Sombra interna | `--inset-shadow-bevel/-bevel-press/-slot` | `inset-shadow-bevel`, `inset-shadow-slot` | |
| Easing | `--ease-out/in-out/pixel` | `ease-out`, `ease-pixel` | |
| Duração | `--duration-fast/base/slow/pulse` | `duration-(--duration-fast)` | O Tailwind 4 não tem namespace de duração; use a variável |
| Degrau | `--notch-md/sm`, `--shape-notch-md/sm` | — | Usado dentro de `components.css` |
| Medidas | `--size-control-md`, `--size-pack-header`… | `h-(--size-control-md)`, `w-(--size-section-menu)` | |
| Camadas | `--z-dropdown/drawer/dialog/toast/tooltip` | `z-(--z-dialog)` | |
| Animação | `--animate-sensor` | `animate-sensor` | Pulso do Testar |

## 3. Componentes → shadcn/ui e Radix

Coluna “Classes” = classes de `components.css` que o componente aplica. Coluna “Props” = opções da função equivalente em `design/system/components.js` (servem de contrato).

| Componente Warden | Base | Classes | Props principais | Notas |
|---|---|---|---|---|
| `Button` | shadcn `Button` | `.btn` + `--primary/secondary/ghost/danger/danger-ghost/link`, `--sm/--lg/--icon` | `variant`, `size`, `icon`, `iconEnd`, `loading`, `count`, `asChild` | `loading`: desabilita, troca o ícone pelo `.loader`, texto no gerúndio, `aria-busy`. Sem `rounded-*`. Ícone só com `aria-label` + `Tooltip` |
| `TestButton` | `Button` + Radix `DropdownMenu` | `.testbtn`, `--preparing/--running/--disabled` | `state: "ready" \| "preparing" \| "running" \| "disabled"`, `progress` | Só no cabeçalho do pack. Antenas são 4 `<i>` decorativos. O ▾ é `DropdownMenuTrigger` com `aria-label="Mais opções do teste"` |
| `Input` | shadcn `Input` | `.input`, `.inputwrap` | `icon`, `end`, `loading`, `mono`, `size` | Sempre com `<Label>` visível; erro via `aria-invalid` + `aria-describedby` |
| `SearchOrLink` (busca ou link) | `Input` | `.omnibox`, `--link` | `state: empty \| loading \| search \| link \| invalid` | Reconhece link pelo formato (Modrinth, CurseForge, URL de .jar/.zip); a linha de status é `role="status"` |
| `Select` | shadcn `Select` (Radix) | `.select` (aparência) | `size` | No lado da linha de mod usar o tamanho `sm` sem borda até o hover |
| `Switch` | shadcn `Switch` | `.switch__track` | `checked`, `stateText` | Alavanca quadrada; transição com `ease-pixel` |
| `Checkbox` | shadcn `Checkbox` | `.check > input` | `checked`, `indeterminate`, `description` | Visto em pixel via `mask` (`--fx-check`) |
| `RadioGroup` | shadcn `RadioGroup` | `.check > input[type=radio]` | | Círculo em pixel (`--fx-radio*`) |
| `ChoiceCard` | `RadioGroup.Item` estilizado | `.choice`, `.choice-list--2` | `title`, `description`, `badge` | Formato de exportação, loader, público/privado, cofre/.env |
| `ModeToggle` | Radix `ToggleGroup type="single"` | `.segmented` | | Formulário × Texto. **Não** use `Tabs` |
| `MemorySlider` | shadcn `Slider` | `.memory`, `.range` | `value`, `recommended`, `ram`, `mods` | Faixa recomendada em verde; aviso acima de 10 GB e abaixo de 4 GB; `aria-valuetext="6 GB"` |
| `SourceTag`, `SideTag`, `Tag` | shadcn `Badge` | `.tag`, `.tag--modrinth/curseforge/local/url`, `.tag__mark` | `source`, `side` | Marcador de forma + texto |
| `StatusBadge` | shadcn `Badge` | `.badge--danger/warn/ok/info/neutral/p1` | | Sempre ícone + palavra |
| `Count` | — | `.count`, `--danger/--warn` | | Com `aria-label` quando o número sozinho não explica |
| `ModTable` / `ModRow` | shadcn `Table` + TanStack Table + **TanStack Virtual** | `.table`, `.modrow`, `--error/--warn/--new/--invalid` | `mod`, `selected` | Lista virtualizada (packs com centenas de mods). `aria-selected` na linha; o nome é um botão que abre o `Sheet` de detalhes |
| `SelectionBar` | — | `.selbar` | `count` | Aparece com 1+ selecionados; `role="region"` |
| `PackHeader` | — | `.packhead`, `.packhead__row`, `.packhead__alerts` | `pack`, `unsaved`, `alerts`, `testState` | Faixa de avisos só se `alerts.length > 0`, com `role="status"` |
| `SectionMenu` | TanStack Router `<Link>` | `.secmenu`, `.secmenu__item` | `active` | `aria-current="page"` (o Link já faz). Ordem fixa das 6 seções |
| `TopBar`, `StatusBar` | — | `.topbar`, `.statusbar`, `.statusbar__tasks--busy/error` | | O indicador de Tarefas abre o `Sheet` de Tarefas |
| `PackRow`, `PackCard` | `Table` | `.packrow`, `.packcard` | | Nada de grade de cartões |
| `BlockProgress` | shadcn `Progress` (Radix) | `.progress`, `--indeterminate/--waiting/--error/--done/--sm` | `value` (ou `null`), `label`, `meta` | `aria-valuetext` com o tempo restante |
| `Steps` | — | `.steps`, `.steps__item--done/--waiting/--error` | `items`, `current`, `waiting`, `error` | `<ol>` com `aria-current="step"`; não clicável |
| `Console` | — + TanStack Virtual | `.console`, `.console__line--warn/--error/--warden` | `lines`, `state` | `role="log"`, `aria-live="off"` (alto volume); filtros e busca acima. Texto cru, nunca traduzido |
| `Diff` | `@codemirror/merge` (arquivos) e componente simples (trechos) | `.diff`, `.kvdiff` | `file`, `lines` | Sinal + / − visível e texto “Adicionada/Removida” para leitor de tela |
| `ConfigEditor` | CodeMirror 6 | `.editor` (tema) | | Tema: comentário `text-3`, chave `primary-text`, booleano `ok-text`, número `warn-text`, seção `bone`; linha alterada com fundo `warn-soft` e barra `warn` |
| `ConfigForm` (P1) | React Hook Form + Zod | `.cfgform` | | Cada chave com a descrição original e “Desfazer” |
| `Alert` | shadcn `Alert` | `.alert--info/ok/warn/danger/neutral/ai`, `--compact` | `kind`, `title`, `actions` | `role="alert"` só para perigo |
| `Issue` (problema) | — | `.issue--danger/warn/info` | `title`, `text`, `evidence`, `actions` | “Como sabemos” num `Collapsible` |
| `Toast` | Radix `Toast` (via shadcn) | `.toast--ok/info/danger/warn` | | 6 s, pausa no hover e no foco; `role="status"` (erro `alert`). Nunca a única cópia de um erro |
| `Dialog` / `ConfirmDialog` | shadcn `Dialog` / `AlertDialog` | `.dialog`, `--sm/--lg` | `title`, `description`, `footer` | Foco inicial no título: `onOpenAutoFocus={e => { e.preventDefault(); titleRef.current?.focus() }}` com `tabIndex={-1}` no título. Destrutivo usa `AlertDialog` |
| `SidePanel` | shadcn `Sheet side="right"` | `.drawer` | | Detalhes do mod, Tarefas |
| `Menu` | shadcn `DropdownMenu` | `.menu`, `.menu__item`, `--danger`, `.menu__desc` | | Itens com uma linha de descrição quando ajudam |
| `Tooltip` | shadcn `Tooltip` | `.tooltip` | | Atraso de abertura ~400 ms; também no foco |
| `EmptyState` | — | `.empty`, `--ok/--error/--compact`, `.empty__art` | `glyph`, `title`, `text`, `actions` | Ilustração em pixel (`art()` em `components.js`) |
| `Skeleton` | shadcn `Skeleton` | `.skeleton--line/--tile/--btn` | | Mesma forma do conteúdo final |
| `Timeline` | — | `.timeline`, `.tl-item--unsaved/saved/final/published` | | `<ol>` |
| `AIAnswer` | — | `.ai-block`, `--loading`, `.meter` | `cause`, `confidence`, `mods`, `steps` | Aviso fixo “A IA pode errar.” |
| `FileTree` | — (role tree) | `.tree`, `.tree__item`, `--dirty` | | Roving tabindex, ↑ ↓ ← → Enter |
| `Disclosure` | shadcn `Collapsible` ou `<details>` | `.disclosure` | | “Mais opções”, “Por que não o Java 25?” |
| `Tabs` | **não usar** | — | — | Proibido pela estrutura |

Ícones: `lucide-react`, `size={16}` (14 e 20 nas variações), `strokeWidth={2}`, `aria-hidden` quando há texto. `Sparkles` só em coisas de IA.

## 4. Estados e interações

| Elemento | Estado | Comportamento |
|---|---|---|
| Botão | hover | Preenchimento um tom acima (`surface-3`; primário `primary-hover`), borda `border-control` |
| Botão | pressionado | Chanfro invertido (`bevel-press`) e desce 1 px |
| Botão | foco | Contorno `focus` de 2 px, afastado 2 px, não recortado pelo degrau |
| Botão | desabilitado | `surface-1` + texto `text-3`, sem chanfro, `cursor: not-allowed` |
| Botão | carregando | Desabilitado, `.loader` no lugar do ícone, texto no gerúndio |
| Testar | pronto | Pulso de brilho (3,4 s); clique inicia a checagem (etapa 1) |
| Testar | testando | Sem pulso; barra interna em blocos com o progresso geral; clique leva à tela do teste |
| Testar | jogo aberto | Fundo escuro, borda ciana acesa, ponto verde, antenas verdes; clique leva ao console |
| Campo | erro | Borda `danger` + barra interna à esquerda + mensagem com ícone abaixo |
| Linha de mod | selecionada | `primary-soft` + barra ciana de 3 px à esquerda |
| Linha de mod | problema | Barra `danger`/`warn` à esquerda + badge ao lado do nome |
| Item do menu de seções | ativo | Afundado (`slot`) + barra ciana com brilho |
| Diálogo | abrir/fechar | Entra subindo 12 px em 320 ms; Esc ou ✕ fecha; clique fora fecha (não em `AlertDialog`) |
| Toast | aparecer | Sobe 8 px em 200 ms; some em 6 s |

## 5. Layout e larguras

| Faixa | O que muda |
|---|---|
| ≥ 1181 px | Padrão: menu de seções 252 px; conteúdo até 1180 px; padding 32 px |
| 1024 a 1180 px | Menu de seções 236 px; descrição do mod até 210 px; “Salvar versão · 5” (sem a palavra “alterações”, que continua no `aria-label`); padding 20 px; busca da lista 220 px |
| < 1024 px | Não suportado: a janela tem mínimo de 1024 × 640 (configurar no `tauri.conf.json`) |

Estrutura de altura: barra do app 56 px ou cabeçalho do pack 76 px (+ ~36 px da faixa de avisos) / conteúdo com rolagem própria / rodapé 30 px. O menu de seções rola sozinho se a janela for baixa.

Tabelas sempre dentro de `.tablewrap` (rola dentro, nunca a página).

## 6. Movimento

| Elemento | Gatilho | Animação | Duração | Easing |
|---|---|---|---|---|
| Testar | pronto | brilho pulsando (`drop-shadow` 4 → 16 px) | `--duration-pulse` | `--ease-in-out` |
| Carregador | ocupado | 4 blocos acendendo em sequência | 3 × `--duration-slow` | `--ease-pixel` |
| Progresso | valor muda | largura | `--duration-slow` | `--ease-pixel` |
| Progresso indeterminado | — | trecho de 36 px andando | 1,6 s | `steps(16)` |
| Toggle | troca | alavanca desliza | `--duration-fast` | `--ease-pixel` |
| Menu | abrir | desce 4 px + aparece | `--duration-base` | `--ease-out` |
| Diálogo / painel | abrir | sobe 12 px / entra 24 px da direita | `--duration-slow` | `--ease-out` |
| Ponto ao vivo | jogo aberto | pisca em 2 passos | 1,6 s | `steps(2)` |

Com `prefers-reduced-motion: reduce` **ou** a opção “Menos movimento” de Configurações (`<html data-motion="reduced">`), tudo vira instantâneo e os loops param. Os tokens já fazem isso; animações novas devem usar as variáveis de duração para herdar.

## 7. Acessibilidade obrigatória

- **Troca de rota:** foco no `<h1>` da página (`tabIndex={-1}`, sem anel). O protótipo faz isso em `render()`.
- **Atalho “Pular para o conteúdo”** antes de tudo, movendo o foco por script (as rotas usam o hash).
- **Landmarks:** `<header>` (barra do app ou cabeçalho do pack), `<nav aria-label="Seções do pack">`, `<main>`, `<footer>` (rodapé). Um de cada por tela.
- **Teclado:** contrato da `DESIGN-SYSTEM.md` §4 (o Radix entrega a maior parte; conferir foco inicial dos diálogos).
- **Anúncios:** mudança de etapa do teste num `aria-live="polite"` (“Etapa 2 de 5: Preparar o Minecraft”); faixa de avisos do cabeçalho com `role="status"`; resultado da busca com `role="status"`.
- **Nunca cor sozinha:** fonte com marcador de forma; estado com ícone e palavra; diff com sinal.
- **Testes:** `vitest-axe` em cada componente e os mesmos fluxos de teclado do protótipo (`kbd.mjs` da verificação desta tarefa serve de roteiro).

## 8. Conteúdo, truncamento e casos de borda

| Caso | Regra |
|---|---|
| Nome do pack longo | Uma linha com reticências no cabeçalho; nome completo no `title` e no leitor de tela |
| Descrição do mod | Uma linha com reticências (até 300 px; 210 px em janela estreita) |
| Caminhos e arquivos | Fonte mono, quebra em qualquer caractere (`.path`) |
| Números, datas e tamanhos | `Intl` em pt-BR: “1.204”, “1,2 MB”, “há 6 dias”, “01/10/2026” |
| Lista com 0 itens | Estado vazio da seção (texto em `DESIGN-SYSTEM.md` e no protótipo) |
| Lista com centenas de itens | Virtualizar (TanStack Virtual); grupos recolhíveis |
| Item com metadado ilegível | Linha “Arquivo inválido” com o erro; o resto da lista funciona |
| Mod sem ícone na API | Ícone provisório em pixel gerado do nome (`tileSvg()` em `components.js`) |
| Sem internet | Erro com “Tentar de novo” e o que ficou guardado |
| Uma fonte fora do ar | Mostra a outra, com aviso de qual faltou |
| Sem chave (CurseForge ou Gemini) | Aviso com “Abrir Configurações”; nada é enviado sem chave |
| Textos | Todos em `src/i18n/pt-BR/`; os do protótipo são a base revisada (copiar, não reescrever) |

## 9. O que é só do protótipo

Não implementar: a barra amarela “Protótipo” no topo (seletor de tela e de estado, mapa, “Menos movimento” de atalho), as caixas tracejadas “Protótipo” dentro das telas (simulam o fim do jogo, links colados etc.), a navegação por `data-go` e o toast “Esta ação não faz parte do protótipo”. Os dados (`data.js`) são fictícios.

## 10. Checklist de aceite visual

- [ ] Nenhum `rounded-md/lg/full`, nenhuma cor fora dos tokens, nenhum `shadow-*` padrão do Tailwind.
- [ ] Botões com degrau de pixel e chanfro; foco em osso, inteiro, não recortado.
- [ ] Fonte pixel só em títulos curtos (≥ 16 px), menu de seções, Testar e números grandes; com sombra dura.
- [ ] Veia e camada de sculk só na barra do app e no cabeçalho do pack.
- [ ] Brilho só no Testar pronto, seção ativa, selecionado e progresso.
- [ ] `Sparkles` só em IA.
- [ ] Nenhuma aba.
- [ ] Comparar lado a lado com o protótipo em 1280 e 1024 px.
- [ ] axe sem violações; teclado conforme §7; movimento reduzido conferido.
