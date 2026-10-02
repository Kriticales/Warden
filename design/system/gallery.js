/* Galeria do design system do Warden: fundamentos e todos os componentes com estados.
   Tudo aqui é montado com as funções de components.js, as mesmas do protótipo. */
(function () {
  "use strict";
  const W = window.W;
  const { icon, btn, testButton, input, select, omnibox, switchCtl, check, choice, segmented, memory, tag, source, side, badge, count, status, alert, issue, empty, toast, skeletonRows, progress, steps, consoleBox, diff, dialog, drawer, menu, aiBlock, tlItem, tile, brandMark, art, packHeader, sectionMenu, modRow, modTableHead, topbar, statusbar } = window.W;

  const S = []; // [id, grupo, título, html]
  const sec = (id, group, title, lead, body) => S.push([id, group, title, `<section class="gx-sec" id="${id}" aria-labelledby="${id}-h"><h2 id="${id}-h">${title}</h2>${lead ? `<p class="lead">${lead}</p>` : ""}${body}</section>`]);
  const st = (label, html) => `<div class="gx-state"><span class="gx-lbl">${label}</span>${html}</div>`;
  const states = (items, cls) => `<div class="gx-states ${cls || ""}">${items.join("")}</div>`;
  const rules = (dos, donts) => `<div class="gx-rules">${dos.map(([b, t]) => `<div class="do"><b>${b}</b>${t}</div>`).join("")}${donts.map(([b, t]) => `<div class="dont"><b>${b}</b>${t}</div>`).join("")}</div>`;
  const css = (v) => getComputedStyle(document.documentElement).getPropertyValue(v).trim();

  // ======================= FUNDAMENTOS =======================
  sec("principios", "Fundamentos", "Princípios",
    "O Warden é uma ferramenta de trabalho com cara de Minecraft: denso, direto, com pixel e blocos onde ajudam a reconhecer o app, e sans legível onde se lê. Estas regras decidem os casos que a galeria não cobre.",
    rules([
      ["Pixel para reconhecer, sans para ler", "Fonte pixel em títulos curtos, no menu de seções, no Testar e em números. Frases, descrições, tabelas e botões comuns ficam na Manrope."],
      ["Brilho só com significado", "O ciano acende onde algo está ativo: item selecionado, Testar pronto, progresso. Nada brilha só para enfeitar."],
      ["Blocos, não bolhas", "Cantos retos e degrau de pixel. Nenhum raio grande, nenhuma pílula. Elevação com sombra dura, como um bloco sobre o chão."],
      ["Estado nunca só na cor", "Todo estado tem ícone e palavra; fontes de mod têm marcadores de formas diferentes (círculo de pixel, losango, quadrado vazado)."],
    ], [
      ["Abas", "A estrutura aprovada proíbe abas e abas dentro de abas. Use seções do menu, seletor de modo, caixas de seleção ou páginas corridas."],
      ["Cara de feito por IA", "Gradientes roxo e azul, vidro fosco, cartões iguais em grade, hero com frase de efeito, emojis decorativos, tudo centralizado."],
      ["Brilhinho fora da IA", "O ícone sparkles marca só o que é IA (diagnóstico com Gemini). Nunca como decoração, novidade ou destaque."],
      ["Texto de vitrine", "Slogans, exclamações, “Desbloqueie”, títulos que repetem o óbvio. Diga o que é, o que aconteceu e o que fazer."],
    ]));

  const colorGroups = [
    ["Fundos e superfícies", ["bg", "bg-sunken", "surface-1", "surface-2", "surface-3", "overlay"]],
    ["Bordas e foco", ["border", "border-strong", "border-control", "focus"]],
    ["Texto", ["text", "text-2", "text-3", "bone"]],
    ["Destaque ciano (primary)", ["primary", "primary-hover", "primary-press", "on-primary", "primary-text", "primary-bright", "primary-soft", "primary-line", "primary-glow"]],
    ["Estados", ["ok", "ok-text", "ok-soft", "warn", "warn-text", "warn-soft", "danger", "danger-text", "danger-soft", "danger-solid", "info", "info-text", "info-soft"]],
    ["IA (osso)", ["ai", "ai-soft", "ai-line"]],
    ["Fontes de mod, diff e console", ["src-modrinth", "src-curseforge", "src-local", "diff-add", "diff-del", "console-bg", "console-text", "console-muted"]],
  ];
  sec("cor", "Fundamentos", "Cor",
    "Paleta do Warden e do Deep Dark: abismo quase preto, corpo azul-petróleo, brilho ciano das almas e osso das costelas. Componentes usam só os papéis abaixo (--color-*), nunca o hexadecimal. Fonte única: tokens.json.",
    colorGroups.map(([g, names]) => `<h3>${g}</h3><div class="gx-swatches">${names.map((n) => `<div class="gx-sw"><i style="background:var(--color-${n})${n.includes("soft") || n.includes("line") || n.includes("glow") || n === "overlay" || n.startsWith("diff") ? ";background-color:var(--color-" + n + ");box-shadow:inset 0 0 0 999px var(--color-" + n + ")" : ""}"></i><div><code>--color-${n}</code><span data-v="--color-${n}"></span></div></div>`).join("")}</div>`).join("") +
    `<p class="gx-note">Por que foco em osso e não em ciano: o ciano já quer dizer “selecionado”. Se o anel de foco também fosse ciano, não daria para distinguir “onde o teclado está” de “o que está escolhido”. Informação usa lavanda pelo mesmo motivo. A IA usa osso, e nunca roxo, para fugir do clichê de “IA mágica”.</p>`);

  sec("contraste", "Fundamentos", "Contraste verificado",
    "Calculado por design/system/tools/contraste.mjs a partir de tokens.json (WCAG 2.1). Cores translúcidas são compostas sobre o fundo antes do cálculo. Texto precisa de 4,5:1; contornos, ícones e marcadores, de 3:1.",
    `<div class="gx-contrast" tabindex="0" role="region" aria-label="Tabela de contraste"><table class="table"><thead><tr><th>Frente</th><th>Fundo</th><th class="num">Contraste</th><th>Mínimo</th><th>Resultado</th><th>Uso</th></tr></thead><tbody>${(window.WARDEN_CONTRAST || []).map((r) => `<tr><td><code>${r.fg}</code></td><td><code>${r.bg.replace(/@/g, " sobre ")}</code></td><td class="num">${r.ratio.toFixed(2).replace(".", ",")}:1</td><td>${String(r.min).replace(".", ",")}:1</td><td>${r.ratio >= r.min ? (r.min === 3 ? badge("ok", "AA") : r.ratio >= 7 ? badge("ok", "AAA") : badge("ok", "AA")) : badge("danger", "Reprova")}</td><td class="t-3">${r.use}</td></tr>`).join("")}</tbody></table></div>
     <p class="gx-note">${(window.WARDEN_CONTRAST || []).length} pares, ${(window.WARDEN_CONTRAST || []).filter((r) => r.ratio < r.min).length} reprovados. O texto sobre o ciano chega a 10,25:1 (AAA): o Testar merece leitura perfeita.</p>`);

  sec("tipografia", "Fundamentos", "Tipografia",
    "Três famílias livres (SIL OFL 1.1), embutidas no app: Pixelify Sans para o “jeito Minecraft”, Manrope para ler e IBM Plex Mono para console, versões e caminhos. Base de 14 px, porque é um app de mesa denso.",
    `<table class="gx-type" style="width:100%;border-collapse:collapse"><tbody>
      <tr><td>display 2xl · 32/36</td><td><span class="t-display-2xl">Boas-vindas ao <span class="t-hl">Warden</span></span></td></tr>
      <tr><td>display xl · 24/28</td><td><span class="t-display-xl">Vale Sereno</span> <span class="t-3 t-sm">nome do pack, título de tela</span></td></tr>
      <tr><td>display lg · 20/24</td><td><span class="t-display-lg">Publicação para os jogadores</span> <span class="t-3 t-sm">título de painel</span></td></tr>
      <tr><td>display md · 16/22</td><td><span class="t-display-md">Mods</span> <span class="t-display-md">Configs</span> <span class="t-display-md">Histórico</span> <span class="t-3 t-sm">menu de seções, grupos</span></td></tr>
      <tr><td>título · sans 20/24 800</td><td><span class="t-title">Remover Just Enough Items (JEI)?</span> <span class="t-3 t-sm">título de diálogo (frase)</span></td></tr>
      <tr><td>base · 14/20</td><td>O Waystones precisa do Balm, que não está no pack. Sem o Balm, o jogo para na tela de carregamento.</td></tr>
      <tr><td>sm · 13/18</td><td class="t-sm t-2">Resultados do Modrinth e da CurseForge juntos. Só o que funciona em Minecraft 1.20.1 com Forge.</td></tr>
      <tr><td>xs · 12/16</td><td class="t-xs t-3">por P3pp3rF1y · 48 mi downloads · atualizado há 6 dias</td></tr>
      <tr><td>2xs caps · 11/14</td><td>${badge("danger", "Erro")} ${badge("warn", "Aviso")} <span class="t-caps t-3">Passos sugeridos</span></td></tr>
      <tr><td>mono · 13 e 12</td><td><span class="t-mono">config/create-common.toml · 15.20.0.112</span></td></tr>
    </tbody></table>` +
    rules([["Pixel com sombra dura", "Todo texto pixel leva --shadow-pixel-text (2 px para baixo e para a direita, quase preto), como o texto dos menus do jogo."], ["Uma palavra acesa", "Um título pode ter UMA palavra em ciano (.t-hl) quando ela é o assunto. No máximo uma por tela."]],
      [["Pixel em frase longa", "Mais de umas 4 palavras ou abaixo de 16 px: volte para a Manrope. Títulos de diálogo são frases, então são sans."], ["Caixa alta em texto", "Só nos selos de estado e nos rótulos de seção do bloco da IA, com espaçamento --tracking-caps."]]));

  const spaces = [1, 2, 3, 4, 5, 6, 8, 10, 12, 16];
  sec("espaco", "Fundamentos", "Espaçamento, cantos e bordas",
    "Base de 4 px (no Tailwind, --spacing: 4px, então p-4 = 16 px). Controles: 28, 36 e 48 px de altura. O Warden não arredonda cantos: usa ângulo reto e o degrau de pixel recortado.",
    `<div class="gx-states gx-states--wide"><div class="gx-state" style="justify-items:stretch">${spaces.map((n) => `<div class="gx-spacer"><span style="width:72px">--space-${n}</span><i style="width:var(--space-${n})"></i><span>${css("--space-" + n)}</span></div>`).join("")}</div>
     <div class="gx-state"><span class="gx-lbl">Degrau de pixel</span><div class="gx-shape">
       <div><i style="clip-path:var(--shape-notch-md)"></i>notch md · 4 px<br>botões, Testar</div>
       <div><i style="clip-path:var(--shape-notch-sm);width:56px;height:24px"></i>notch sm · 2 px<br>botões pequenos, selos</div>
       <div><i style="background:var(--color-surface-2);box-shadow:inset 0 0 0 1px var(--color-border-strong),0 0 0 1px var(--color-pixel-shadow)"></i>moldura dupla<br>diálogos, menus</div></div>
       <span class="gx-lbl" style="margin-top:12px">Larguras de borda</span>
       <div class="t-sm t-2">thin 1 px: divisórias e painéis · block 2 px: progresso, foco · bar 3 px: barra de seleção e de alerta</div>
       <span class="gx-lbl" style="margin-top:12px">Raios</span><div class="t-sm t-2">none 0 (padrão) · xs 2 px (campos) · sm 4 px (raro). Nada acima disso.</div></div></div>`);

  sec("elevacao", "Fundamentos", "Elevação",
    "Sombras duras e curtas, como um bloco sobre o chão, em vez de nuvens desfocadas. Cada camada tem um nível só.",
    `<div class="gx-elev"><div style="box-shadow:var(--shadow-block-1)"><code>block-1</code><br>cartão de pack, painel forte</div><div style="box-shadow:var(--shadow-block-2)"><code>block-2</code><br>menu, toast, painel lateral</div><div style="box-shadow:var(--shadow-block-3)"><code>block-3</code><br>diálogo</div><div style="box-shadow:var(--shadow-glow)"><code>glow</code><br>só selecionado/ativo</div>
     <div style="box-shadow:var(--inset-shadow-bevel)"><code>bevel</code><br>chanfro dos botões</div><div style="box-shadow:var(--inset-shadow-bevel-press)"><code>bevel-press</code><br>botão pressionado</div><div style="background:var(--color-bg-sunken);box-shadow:var(--inset-shadow-slot)"><code>slot</code><br>campos, item ativo do menu</div><div style="background:var(--color-surface-1)"><code>sem sombra</code><br>superfícies comuns</div></div>`);

  const ICON_USE = [["play", "Testar"], ["settings", "Configurações"], ["arrow-left", "Voltar"], ["puzzle", "Mods"], ["file-code", "Configs"], ["triangle-alert", "Problemas"], ["sparkles", "IA (só IA)"], ["history", "Histórico"], ["package", "Exportar"], ["search", "Buscar"], ["link", "Link colado"], ["folder-open", "Escolher pasta"], ["save", "Salvar versão"], ["globe", "Publicar"], ["flag", "Versão final"], ["refresh-cw", "Atualizar"], ["trash-2", "Remover"], ["pencil", "Editar"], ["monitor", "Só cliente"], ["server", "Só servidor"], ["layers", "Cliente e servidor"], ["terminal", "Console"], ["square", "Parar jogo"], ["copy", "Copiar"], ["key-round", "Chaves"], ["shield", "Cofre"], ["coffee", "Java"], ["memory-stick", "Memória"], ["image", "Resource pack"], ["sun", "Shader"], ["file-diff", "Diferenças"], ["list", "Tarefas"]];
  sec("icones", "Fundamentos", "Ícones",
    "Lucide (licença ISC), traço 2 px, tamanhos 14, 16 e 20. No React: lucide-react com os mesmos nomes. Desenhos próprios em pixel ficam para a marca, os ícones provisórios de mod e as ilustrações dos estados vazios.",
    `<div class="gx-icons">${ICON_USE.map(([n, u]) => `<div>${icon(n, n === "sparkles" ? "icon--ai" : "")}<span>${u}<br><span class="t-3">${n}</span></span></div>`).join("")}</div>
     <h3>Pixel art própria</h3><div class="row row--gap-4 row--wrap">${brandMark("brand__mark")}<span class="t-sm t-2">Marca: bloco com duas antenas e núcleo de alma</span>
       ${tile("Create", "lg")}${tile("Sophisticated Backpacks", "lg")}${tile("Waystones", "lg")}<span class="t-sm t-2">Ícone provisório gerado do nome (só sem ícone oficial)</span>
       ${art("", "box", "empty__art")}${art("ok", "check", "empty__art")}${art("danger", "x", "empty__art")}<span class="t-sm t-2">Ilustrações de estado vazio</span></div>` +
    rules([["sparkles = IA", "Botões, títulos e blocos do Diagnóstico com IA. O dono gosta do ícone: use sem medo onde é IA."]], [["Ícone sem texto", "Só em ações muito conhecidas (fechar, mais ações, editar) e sempre com nome acessível e tooltip."]]));

  sec("movimento", "Fundamentos", "Movimento",
    "Rápido e discreto. Animações “em degraus” (steps) lembram o jogo: carregador de 4 blocos, progresso indeterminado, alavanca do toggle. Com prefers-reduced-motion (ou a opção do app), tudo vira instantâneo e o pulso para.",
    `<div class="gx-motion">${[["--duration-fast", "120 ms · hover, foco, cor", "var(--ease-out)"], ["--duration-base", "200 ms · menu, toast", "var(--ease-out)"], ["--duration-slow", "320 ms · diálogo, painel", "var(--ease-out)"], ["--ease-pixel", "steps(4) · em degraus", "var(--ease-pixel)"]].map(([n, d, e]) => `<div><code>${n}</code><span class="t-sm t-2">${d}</span><span class="bar"><i style="transition:transform var(${n.startsWith("--ease") ? "--duration-slow" : n}) ${e}"></i></span><span class="t-xs t-3">Passe o mouse para ver</span></div>`).join("")}
      <div><code>--duration-pulse</code><span class="t-sm t-2">3,4 s · pulso do sensor (só o Testar pronto)</span><div style="padding-top:14px">${testButton({ menuId: "gx-m0" })}</div></div>
      <div><code>carregador</code><span class="t-sm t-2">4 blocos acendendo em sequência</span><span class="row"><span class="loader t-primary" aria-hidden="true"><i></i><i></i><i></i><i></i></span><span class="t-sm">Buscando…</span></span></div></div>`);

  sec("texturas", "Fundamentos", "Texturas e padrões",
    "Desenhos próprios em SVG, sem nenhum asset da Mojang ou da Microsoft. Pouco e sempre nos mesmos lugares: é isso que faz parecer pensado, e não decorado.",
    `<div class="gx-tex">
      <div><div class="sample" style="background:var(--color-surface-1) var(--fx-vein);background-size:64px 64px"></div><p><b>Veia de sculk</b> (ciano a 7%). Só no fundo da barra do app e do cabeçalho do pack.</p></div>
      <div><div class="sample" style="background:var(--color-surface-1)"><span style="position:absolute;left:0;right:0;bottom:0;height:6px;background:var(--fx-sculk-edge) repeat-x left bottom / 48px 6px"></span></div><p><b>Camada de sculk.</b> A borda de baixo da barra do app e do cabeçalho do pack, como a grama no topo de um bloco. Em nenhum outro lugar.</p></div>
      <div><div class="sample" style="background:var(--color-surface-1);display:grid;place-items:center;padding:0 16px"><hr class="sep-blocks" style="width:100%;margin:0" /></div><p><b>Separador em blocos.</b> Entre grupos grandes de uma página. No máximo um ou dois por tela.</p></div>
    </div>` + rules([["Fundo liso no conteúdo", "Listas, editores e o console ficam sobre fundo liso: leitura primeiro."]], [["Cena de fundo atrás do conteúdo", "A arte de caverna desfocada do D1 saiu: atrás de listas densas ela atrapalha e lembra tela de launcher."]]));

  // ======================= COMPONENTES =======================
  sec("botoes", "Componentes", "Botões",
    "Bloco com chanfro (luz em cima, sombra embaixo) e degrau nos cantos. Primário: no máximo um por área (o principal do diálogo, da página ou do painel). O Testar tem componente próprio.",
    `<div class="gx-scroll"><table class="gx-matrix"><thead><tr><th>Variante</th><th>Padrão</th><th>Hover</th><th>Foco</th><th>Pressionado</th><th>Desabilitado</th><th>Carregando</th></tr></thead><tbody>
    ${[["Primário", "primary", "Adicionar ao pack"], ["Secundário", "secondary", "Verificar agora"], ["Fantasma", "ghost", "Cancelar"], ["Perigo", "danger", "Remover JEI"], ["Perigo secundário", "danger-ghost", "Descartar tudo"], ["Link", "link", "Ver diferenças"]].map(([n, v, l]) => `<tr><td>${n}</td><td>${btn(l, { variant: v })}</td><td>${btn(l, { variant: v, cls: "is-hover" })}</td><td>${btn(l, { variant: v, cls: "is-focus" })}</td><td>${btn(l, { variant: v, cls: "is-active" })}</td><td>${btn(l, { variant: v, disabled: true })}</td><td>${v === "link" ? '<span class="t-3 t-xs">não se aplica</span>' : btn(v === "primary" ? "Adicionando…" : v === "danger" ? "Removendo…" : "Verificando…", { variant: v, busy: true, disabled: true })}</td></tr>`).join("")}
    </tbody></table></div>
    <h3>Tamanhos, ícone e contador</h3>
    ${states([st("sm · 28 px", btn("Copiar link", { size: "sm", icon: "copy" })), st("md · 36 px (padrão)", btn("Escolher pasta…", { icon: "folder-open" })), st("lg · 48 px", btn("Exportar", { variant: "primary", size: "lg", icon: "package" })), st("só ícone (com tooltip)", btn("Mais ações", { variant: "ghost", iconOnly: true, icon: "ellipsis" }) + " " + btn("Editar informações", { variant: "ghost", size: "sm", iconOnly: true, icon: "pencil" })), st("com contador", btn("Salvar versão", { icon: "save", count: "5 alterações" })), st("IA (sparkles)", btn("Pedir ajuda à IA", { icon: "sparkles", iconCls: "icon--ai" }))])}` +
    rules([["Verbo + objeto", "“Adicionar 2 itens”, “Remover Rubidium”, “Trazer 3 selecionados para o pack”. O botão diz o que acontece."], ["Carregando mantém o texto", "Troque por um gerúndio (“Salvando…”) e desabilite; o carregador em blocos entra no lugar do ícone."]],
      [["“OK”, “Sim/Não”", "Em confirmações, os botões repetem a ação: “Manter no pack” e “Remover JEI”."], ["Dois primários juntos", "Se tudo é principal, nada é. O segundo vira secundário."]]));

  sec("testar", "Componentes", "Botão Testar",
    "A peça central do cabeçalho do pack: bloco ciano com antenas de pixel e pulso lento de sensor de sculk. É o único elemento grande do cabeçalho. O ▾ abre ações raras do teste.",
    states([
      st("Pronto (pulsa)", `<div style="padding-top:12px">${testButton({ menuId: "gx-m1" })}</div>`),
      st("Hover", `<div style="padding-top:12px">${testButton({ menuId: "gx-m2" }).replace("testbtn__main", "testbtn__main is-hover")}</div>`),
      st("Foco", `<div style="padding-top:12px">${testButton({ menuId: "gx-m3" }).replace("testbtn__main", "testbtn__main is-focus")}</div>`),
      st("Testando (preparação)", `<div style="padding-top:12px">${testButton({ state: "preparing", progress: 38, menuId: "gx-m4" })}</div>`),
      st("Jogo aberto", `<div style="padding-top:12px">${testButton({ state: "running", menuId: "gx-m5" })}</div>`),
      st("Desabilitado", `<div style="padding-top:12px">${testButton({ state: "disabled", menuId: "gx-m6" })}</div><span class="t-xs t-3">Raro: só enquanto o pack está sendo lido.</span>`),
    ], "gx-states--wide") +
    `<h3>Com o menu ▾ aberto</h3><div class="gx-stage" style="height:420px"><div style="position:absolute;right:24px;top:24px">${testButton({ menuId: "gx-testmenu", open: true })}</div>${menu([
      { label: "Testar", desc: "Abre o jogo com o pack, na instância de teste", icon: "play" },
      { label: "Testar como o jogador recebe", desc: "Instância limpa, sem os mundos e ajustes do teste", icon: "user", end: badge("p1", "P1") }, "sep",
      { label: "Ver último teste", desc: "Hoje, 14:20 · fechou normalmente", icon: "history" }, "sep",
      { label: "Ajustes do teste neste computador…", icon: "settings" },
      { label: "Abrir pasta da instância de teste", icon: "folder-open" },
      { label: "Apagar mundos de teste…", icon: "trash-2" },
      { label: "Recriar instância de teste…", icon: "rotate-ccw" },
    ], { id: "gx-testmenu", static: true, label: "Mais opções do teste", style: "right:24px;top:84px" })}</div>`);

  sec("campos", "Componentes", "Campos",
    "Campos são “slots”: fundo afundado com sombra interna. Rótulo sempre visível em cima; dica embaixo; erro com ícone e texto dizendo o que fazer.",
    `<h3>Texto</h3>${states([
      st("Vazio", input({ label: "Nome do pack", placeholder: "Ex.: Vale Sereno" })),
      st("Preenchido", input({ label: "Nome do pack", value: "Vale Sereno" })),
      st("Hover", input({ label: "Nome do pack", value: "Vale Sereno", state: "hover" })),
      st("Foco", input({ label: "Nome do pack", value: "Vale Sereno", state: "focus" })),
      st("Erro", input({ label: "Nome do pack", value: "Vale/Sereno", error: "O nome não pode ter / nem \\. Tire esses caracteres." })),
      st("Desabilitado", input({ label: "Versão do Minecraft", value: "1.20.1", disabled: true, hint: "Mudar a versão do Minecraft é outro pack." })),
      st("Somente leitura", input({ label: "Link do pack", value: "https://raw.githubusercontent.com/…/pack.toml", readonly: true, mono: true })),
      st("Com ícone", input({ label: "Buscar no pack", icon: "search", placeholder: "Nome, autor ou arquivo" })),
      st("Carregando", input({ label: "Buscar no pack", icon: "search", value: "create", loading: true })),
    ])}
    <h3>Busca ou link (Adicionar)</h3><p class="gx-note" style="margin-top:0">Um campo só para buscar e para colar um link. O Warden reconhece o link pelo formato. É a decisão M3 da estrutura.</p>
    ${states([
      st("Vazio", omnibox({ id: "gx-o1" })),
      st("Buscando", omnibox({ id: "gx-o2", value: "backpack", state: "loading" })),
      st("Com resultados", omnibox({ id: "gx-o3", value: "backpack", state: "search" })),
      st("Link reconhecido", omnibox({ id: "gx-o4", value: "https://modrinth.com/mod/sophisticated-backpacks", state: "link" })),
      st("Link inválido", omnibox({ id: "gx-o5", value: "https://exemplo.com/meu-mod", state: "invalid" })),
    ], "gx-states--wide")}
    <h3>Select</h3>${states([
      st("Padrão", select({ label: "Tipo", options: ["Mods", "Resource packs", "Shaders"], value: "Mods" })),
      st("Foco", select({ label: "Tipo", options: ["Mods"], state: "focus" })),
      st("Erro", select({ label: "Versão do Forge", options: ["Escolha uma versão"], error: "Escolha a versão do Forge para continuar." })),
      st("Desabilitado", select({ label: "Java", options: ["Automático: Java 17"], disabled: true })),
      st("Pequeno (em tabela)", select({ bare: true, size: "sm", ariaLabel: "Lado", options: ["Cliente e servidor", "Só cliente", "Só servidor"] })),
    ])}
    <h3>Toggle, checkbox e radio</h3>${states([
      st("Toggle desligado", switchCtl({ label: "Mostrar versões beta" })),
      st("Toggle ligado", switchCtl({ label: "Mostrar versões beta", checked: true })),
      st("Toggle foco", switchCtl({ label: "Mostrar versões beta", state: "focus" })),
      st("Toggle desabilitado", switchCtl({ label: "Opcional para o jogador", disabled: true })),
      st("Checkbox", check({ label: "Marcar como versão final" })),
      st("Checkbox marcado", check({ label: "Marcar como versão final", checked: true, desc: "pronta para os jogadores" })),
      st("Indeterminado", check({ label: "Selecionar todos", indeterminate: true })),
      st("Checkbox foco", check({ label: "Mostrar diferenças antes de salvar", checked: true, state: "focus" })),
      st("Checkbox desabilitado", check({ label: "Sophisticated Core", checked: true, disabled: true, desc: "obrigatória: não dá para desmarcar" })),
      st("Checkbox com erro", check({ label: "Entendi o aviso", state: "invalid" })),
      st("Radio", `<div class="stack-2">${check({ type: "radio", name: "gx-r", label: "Cofre do Windows", checked: true })}${check({ type: "radio", name: "gx-r", label: "Arquivo .env" })}${check({ type: "radio", name: "gx-r2", label: "Desabilitado", disabled: true })}</div>`),
    ])}
    <h3>Escolha em bloco</h3>${states([st("Grupo", `<div class="choice-list" style="width:100%">${choice({ name: "gx-c", title: "Pasta packwiz", desc: "Para hospedar em outro lugar.", checked: true })}${choice({ name: "gx-c", title: ".mrpack", desc: "Para o app do Modrinth.", state: "hover", badge: " " + badge("p1", "P1") })}${choice({ name: "gx-c", title: ".zip da CurseForge", desc: "Não guarda o lado dos mods.", disabled: true })}</div>`)], "gx-states--wide")}
    <h3>Seletor de modo</h3><p class="gx-note" style="margin-top:0">Troca a forma de ver o mesmo conteúdo (Formulário ou Texto do mesmo arquivo). Não é aba e não leva a outro lugar.</p>
    ${states([st("Padrão", segmented([["form", "Formulário"], ["text", "Texto"]], "text", "Modo do editor")), st("Opção indisponível", segmented([["form", "Formulário", true], ["text", "Texto"]], "text", "Modo do editor"))])}
    <h3>Memória</h3>${states([st("Dentro do recomendado", memory({ id: "gx-mem1", value: 6 })), st("Fora do recomendado (aviso)", memory({ id: "gx-mem2", value: 12 })), st("Desabilitado (automático)", memory({ id: "gx-mem3", value: 6, disabled: true }))], "gx-states--wide")}`);

  sec("selos", "Componentes", "Selos: fonte, lado, estado e contador",
    "Tag informa (de onde vem, onde roda); badge diz o estado. A fonte tem forma além da cor: círculo de pixel (Modrinth), losango (CurseForge), quadrado vazado (arquivo local).",
    states([
      st("Fonte", `<div class="row row--wrap">${source("modrinth")}${source("curseforge")}${source("local")}${source("url")}${source("both")}</div>`),
      st("Lado", `<div class="row row--wrap">${side("both")}${side("client")}${side("server")}${side("unknown")}</div>`),
      st("Estado (badge)", `<div class="row row--wrap">${badge("danger", "Erro")}${badge("warn", "Aviso")}${badge("info", "Info")}${badge("ok", "OK")}${badge("neutral", "Fixado")}${badge("p1", "P1")}</div>`),
      st("Tags de contexto", `<div class="row row--wrap">${tag("Já no pack", "primary", { icon: "check" })}${tag("Download manual", "warn", { icon: "download" })}${tag("Publicada", "ok", { icon: "globe" })}${tag("Gemini", "ai", { icon: "sparkles" })}</div>`),
      st("Contador", `<div class="row">${count(128)}${count(3, "danger", "3 problemas")}${count("5", "warn")}${count("12", "text")}</div>`),
      st("Situação em texto", `<div class="stack-2">${status("ok", "abriu normalmente")}${status("danger", "travou")}${status("muted", "nunca testado")}</div>`),
    ], "gx-states--wide"));

  const MODS = [
    { name: "Create", desc: "Engrenagens, eixos e máquinas que se mexem.", ver: "0.5.1.j", src: "modrinth", side: "both" },
    { name: "Just Enough Items (JEI)", desc: "Mostra todos os itens e receitas do jogo.", ver: "15.20.0.106", src: "curseforge", side: "both", update: "available", updateTo: "15.20.0.112", state: "hover" },
    { name: "Embeddium", desc: "Deixa o jogo mais leve.", ver: "0.3.31", src: "modrinth", side: "client", selected: true },
    { name: "Waystones", desc: "Pedras de teleporte entre lugares.", ver: "14.1.6", src: "modrinth", side: "both", kind: "error", flags: [badge("danger", "Erro")] },
    { name: "Xaero's Minimap", desc: "Minimapa no canto da tela.", ver: "24.6.1", src: "curseforge", side: "both", kind: "warn", flags: [badge("warn", "Aviso")] },
    { name: "Farmer's Delight", desc: "Culinária e plantações.", ver: "1.2.6", src: "modrinth", side: "both", update: "checking" },
    { name: "Sophisticated Backpacks", desc: "Mochilas com melhorias e filtros.", ver: "3.20.17", src: "modrinth", side: "both", kind: "new", flags: [tag("Não salvo", "warn")] },
    { kind: "invalid", file: "mods/rubidium.pw.toml", name: "rubidium.pw.toml", desc: "Não foi possível ler este arquivo. Os outros itens não são afetados.", update: "na", flags: [badge("danger", "Arquivo inválido")] },
    { name: "kotlinforforge-4.11.0-all.jar", desc: "Está na pasta mods/, mas fora do índice do pack.", ver: "4.11.0", src: "local", side: "both", update: "na", flags: [badge("warn", "Fora do índice")] },
  ];
  sec("modrow", "Componentes", "Linha de mod",
    "Densa: ícone, nome com as marcas ao lado, uma linha de descrição, versão (com a atualização disponível embaixo), fonte e lado editável. Clicar no nome abre os detalhes no painel lateral. Selecionada: fundo ciano suave e barra à esquerda. Problema: barra da cor do estado e selo.",
    `<div class="tablewrap"><table class="table">${modTableHead()}<tbody>${MODS.map((m) => modRow(m)).join("")}${skeletonRows(2, 5)}</tbody></table></div>
     <p class="gx-note">Linhas, de cima para baixo: padrão, hover com atualização disponível, selecionada, com erro, com aviso, verificando atualização, recém-adicionada (não salva), arquivo inválido (não derruba a lista), fora do índice, carregando (skeleton).</p>
     <h3>Barra de seleção múltipla</h3><div class="selbar"><span class="selbar__count">2 selecionados</span>${btn("Alterar lado", { size: "sm" })}${btn("Atualizar", { size: "sm" })}${btn("Remover", { size: "sm", variant: "danger-ghost", icon: "trash-2" })}<span class="grow"></span>${btn("Limpar seleção", { size: "sm", variant: "ghost" })}</div>`);

  const SECTIONS = (active, o = {}) => sectionMenu([
    { id: "mods", name: "Mods", desc: "Mods, resource packs e shaders", icon: "puzzle", count: 128 },
    { id: "configs", name: "Configs", desc: "Arquivos de ajuste dos mods e do jogo", icon: "file-code", state: o.hover ? "hover" : "" },
    { id: "problemas", name: "Problemas", desc: "O que pode impedir o jogo de abrir", icon: "triangle-alert", count: 3, countKind: "danger", countLabel: "3 problemas", state: o.focus ? "focus" : "" },
    { id: "ia", name: "Diagnóstico com IA", desc: "Pedir à IA para explicar um travamento", icon: "sparkles", ai: true },
    { id: "historico", name: "Histórico", desc: "Versões salvas e publicação", icon: "history", count: 5, countKind: "warn", countLabel: "5 alterações não salvas" },
    { id: "exportar", name: "Exportar", desc: "Gerar o pack para quem vai jogar", icon: "package" },
  ], active, { label: o.label });
  sec("secmenu", "Componentes", "Menu de seções do pack",
    "Os 6 lugares do pack, sempre na mesma ordem. Nome em pixel (o “menu de Minecraft”), contador quando ajuda, e uma linha dizendo o que tem dentro. O ativo afunda como um slot de inventário, com a barra ciana do sensor.",
    `<div class="row row--top row--gap-4 row--wrap">${["mods", "problemas", "ia"].map((a, i) => `<div><div class="gx-lbl t-caps t-3" style="margin-bottom:6px">${["Ativo: Mods · hover em Configs", "Ativo: Problemas · foco", "Ativo: Diagnóstico com IA"][i]}</div><div style="border:1px solid var(--color-border);height:430px;display:flex">${SECTIONS(a, { hover: i === 0, focus: i === 1, label: "Exemplo " + (i + 1) + " do menu de seções" })}</div></div>`).join("")}</div>` +
    rules([["Contador com sentido", "Itens (128), problemas (vermelho quando há erro), alterações não salvas (âmbar). Sem contador quando zero ou quando não há o que contar."]], [["Esconder a descrição", "A linha de descrição fica sempre visível: é ela que tira a dúvida do leigo."]]));

  sec("packhead", "Componentes", "Cabeçalho do pack",
    "Fixo em todas as telas do pack: voltar para Meus packs, qual pack é, avisos passageiros, Salvar versão com o contador e o Testar. Fundo com veia de sculk e camada de sculk na borda.",
    `<div class="stack-3">${[
      ["Padrão (5 alterações)", { unsaved: 5 }],
      ["Com aviso: mudanças do teste para revisar", { unsaved: 8, alerts: [{ kind: "warn", text: "Mudanças do teste para revisar" }] }],
      ["Testando", { unsaved: 5, test: { state: "preparing", progress: 62 } }],
      ["Jogo aberto, pack mudou durante o teste", { unsaved: 6, test: { state: "running" }, alerts: [{ kind: "info", icon: "info", text: "O pack mudou desde o início do teste" }] }],
      ["Nada para salvar", { unsaved: 0 }],
    ].map(([l, o], i) => `<div><div class="t-caps t-3" style="margin-bottom:6px">${l}</div><div class="gx-pack-demo" style="padding-top:2px">${packHeader(Object.assign({ name: "Vale Sereno", mc: "1.20.1", loader: "Forge 47.3.0", version: "1.4.2", menuId: "gx-ph" + i }, o))}</div></div>`).join("")}</div>`);

  sec("app", "Componentes", "Barra do app e rodapé",
    "Nível do app: só a marca, onde você está e Configurações. Nada de menu lateral. O rodapé tem o indicador de Tarefas em todas as telas.",
    `<div class="stack-3"><div class="gx-pack-demo">${topbar({})}</div><div class="gx-pack-demo">${topbar({ back: ["Meus packs"], where: "Criar pack", noSettings: true })}</div>
     <div class="gx-pack-demo">${statusbar({})}</div><div class="gx-pack-demo">${statusbar({ tasks: { state: "busy", text: "1 tarefa em andamento: baixando o Minecraft 1.20.1" } })}</div><div class="gx-pack-demo">${statusbar({ tasks: { state: "error", text: "Uma tarefa falhou: publicar versão" } })}</div></div>`);

  sec("pack", "Componentes", "Linha e cartão de pack",
    "Meus packs é uma lista em linhas, fácil de comparar: Minecraft e loader, versão, último teste, não salvas e quando mudou. O cartão resume UM pack em diálogos e na abertura de pack existente. Grade de cartões iguais não existe no Warden.",
    `<div class="tablewrap"><table class="table">${W.packTableHead()}<tbody>
      ${W.packRow({ name: "Vale Sereno", mc: "1.20.1", loader: "Forge 47.3.0", version: "1.4.2", test: ["ok", "abriu normalmente · hoje"], unsaved: 5, when: "hoje, 14:02" })}
      ${W.packRow({ name: "Técnico Clássico", mc: "1.7.10", loader: "Forge 10.13.4.1614", version: "2.0.1", test: ["danger", "travou · ontem"], unsaved: 0, when: "ontem" })}
      ${W.packRow({ name: "Leve e Bonito", mc: "1.21.1", loader: "Fabric 0.16.5", version: "0.3.0", test: ["muted", "nunca testado"], unsaved: 2, when: "12/09/2026" })}
      ${W.packRow({ name: "Antigo Survival", kind: "missing", path: "D:\\Packs\\antigo-survival" })}
      ${W.packRow({ name: "Pack do Lucas", kind: "unreadable" })}
    </tbody></table></div>
    <h3>Cartão de pack</h3><div style="max-width:520px"><div class="packcard">${tile("Meu pack antigo", "xl")}<div class="packcard__name">Meu pack antigo</div><div class="packcard__meta">Minecraft 1.20.1 · NeoForge 47.1.106 · D:\\Packs\\meu-pack-antigo</div>
      <dl class="packcard__facts"><div><dt>Itens</dt><dd>86 mods · 2 resource packs</dd></div><div><dt>Última versão salva</dt><dd>1.2.0</dd></div><div><dt>Histórico</dt><dd>12 versões</dd></div></dl></div></div>`);

  sec("progresso", "Componentes", "Barra de progresso em blocos",
    "Feita de blocos de 10 px que acendem. Determinada quando se sabe quanto falta; indeterminada (um trecho que anda em degraus) quando não. Com movimento reduzido, o indeterminado fica parado e apagado.",
    states([
      st("Determinada", progress({ label: "Baixando o Minecraft 1.20.1", value: 38, meta: "812 de 2.140 arquivos · 160 de 420 MB · cerca de 2 minutos" })),
      st("Indeterminada", progress({ label: "Instalando o Forge 47.3.0", value: null, meta: "Isso roda uma vez por versão do Forge." })),
      st("Esperando você", progress({ label: "Copiar o pack para o teste", value: 71, state: "waiting", meta: "Esperando 1 download manual da CurseForge" })),
      st("Erro", progress({ label: "Baixando bibliotecas", value: 54, state: "error", meta: "A conexão caiu. O que já foi baixado fica guardado." })),
      st("Concluída", progress({ label: "Exportação", value: 100, state: "done", meta: "216 arquivos, 1,2 MB" })),
      st("Pequena (tarefas, linhas)", progress({ label: "Verificando atualizações", value: 64, size: "sm" })),
    ], "gx-states--wide"));

  const TEST_STEPS = ["Verificar o pack", "Preparar o Minecraft", "Copiar o pack para o teste", "Verificação final", "Abrir o jogo"];
  sec("etapas", "Componentes", "Etapas do teste",
    "As 5 etapas ficam sempre visíveis durante o teste. Mostram onde está; não são clicáveis. Também usadas, com outros nomes, no assistente de criar pack e na primeira execução.",
    `<div class="gx-states gx-states--1">${[
      ["Em andamento (etapa 2)", steps(TEST_STEPS, 1)],
      ["Esperando você (download manual)", steps(TEST_STEPS, 2, { waiting: 2 })],
      ["Com erro na verificação", steps(TEST_STEPS, 0, { error: 0 })],
      ["Tudo pronto: jogo aberto", steps(TEST_STEPS, 5)],
    ].map(([l, h]) => st(l, `<div style="width:100%">${h}</div>`)).join("")}</div>`);

  const LOG = [
    ["warden", "14:20:01", "Warden", "Abrindo Minecraft 1.20.1 com Forge 47.3.0 · Java 17 · 6 GB · jogador Jogador"],
    ["info", "14:20:04", "modlauncher/Launcher", "ModLauncher running: args [--launchTarget, forgeclient]"],
    ["info", "14:20:11", "fml.loading/moddiscovery", "Found 128 mods"],
    ["warn", "14:20:19", "mixin", "Reference map 'xaerominimap.refmap.json' could not be read"],
    ["info", "14:20:38", "minecraft/Minecraft", "Reloading ResourceManager: vanilla, mod_resources"],
    ["error", "14:20:41", "fml/ModLoader", "Missing or unsupported mandatory dependencies: Mod ID 'balm', Requested by 'waystones'"],
  ];
  sec("console", "Componentes", "Console",
    "Saída crua do jogo, sem tradução, em colunas fixas: hora, nível, origem encurtada e mensagem. Avisos em âmbar, erros em vermelho com barra. Linhas do próprio Warden em ciano.",
    states([st("Ao vivo", consoleBox({ lines: LOG, height: 250, label: "Console: exemplo ao vivo" })), st("Rolagem pausada", consoleBox({ lines: LOG.slice(0, 4), state: "paused", height: 230, label: "Console: exemplo pausado" })), st("Esperando o jogo abrir", consoleBox({ lines: [], state: "waiting", height: 170, label: "Console: exemplo esperando" }))], "gx-states--1"));

  sec("diff", "Componentes", "Diferença entre arquivos",
    "Mostra exatamente o que muda antes de gravar. Sinal + e − além da cor; o resto do arquivo dobrado. Para options.txt e afins, diferença por chave.",
    `<div class="grid-2">${diff({ file: "config/create-common.toml", lines: [["fold", "", "", "… 18 linhas iguais"], ["ctx", 19, 19, "[kinetics]"], ["ctx", 20, 20, "\t#Velocidade máxima de rotação"], ["ctx", 21, 21, "\t#Range: > 64"], ["del", 22, "", "\tmaxRotationSpeed = 128"], ["add", "", 22, "\tmaxRotationSpeed = 256"], ["ctx", 23, 23, "\t#Multiplicador de stress"], ["fold", "", "", "… 41 linhas iguais"]] })}
      <div class="panel"><div class="panel__title panel__title--sans" style="margin-bottom:8px">options.txt · 2 de 31 mudanças</div><div class="kvdiff"><span>renderDistance</span><span class="old">12</span>${icon("arrow-right", "icon--sm")}<span class="new">16</span><span></span><span>guiScale</span><span class="old">0</span>${icon("arrow-right", "icon--sm")}<span class="new">3</span><span></span></div><p class="t-xs t-3" style="margin-top:8px">As 29 mudanças automáticas do jogo (posição da janela, último servidor) ficam escondidas.</p></div></div>`);

  sec("alertas", "Componentes", "Aviso inline e problema",
    "Aviso inline: barra colorida à esquerda, ícone, título curto, texto e ação. Fica no fluxo da página, não flutua. O “problema” é a forma longa usada em Problemas e na checagem do teste: o que é, por que importa, como sabemos e o botão que corrige.",
    `<div class="stack-3" style="max-width:860px">
      ${alert({ kind: "info", title: "Mostrando só o Modrinth.", text: "Para buscar também na CurseForge, informe sua chave em Configurações.", actions: btn("Abrir Configurações", { size: "sm" }) })}
      ${alert({ kind: "ok", title: "3 mudanças trazidas para o pack.", text: "Elas aparecem em Alterações não salvas, no Histórico." })}
      ${alert({ kind: "warn", title: "As chaves estão num arquivo de texto.", text: "Qualquer programa no seu computador consegue ler o .env.", actions: btn("Voltar para o cofre", { size: "sm" }) })}
      ${alert({ kind: "danger", title: "Não foi possível ler o pack.toml.", text: "Linha 4: falta fechar as aspas em name. Corrija o arquivo ou volte para a última versão salva.", actions: btn("Ver o arquivo", { size: "sm" }) })}
      ${alert({ kind: "neutral", compact: true, title: "A IA pode errar.", text: "Confira antes de mudar o pack." })}
      ${alert({ kind: "ai", title: "Não parece ser isso?", text: "A IA pode ler o log e sugerir uma causa. Antes, você vê exatamente o que será enviado.", actions: btn("Pedir ajuda à IA", { size: "sm", icon: "sparkles", iconCls: "icon--ai" }) })}
      <h3>Problema</h3>
      ${issue({ kind: "danger", id: "gx-is1", title: "O Waystones precisa do Balm, que não está no pack", text: "Sem o Balm, o jogo para na tela de carregamento.", evidence: "O arquivo META-INF/mods.toml do Waystones 14.1.6 declara a dependência obrigatória “balm” na faixa [7.3.0,).", actions: btn("Adicionar Balm", { variant: "primary", size: "sm" }) + btn("Ver Waystones", { variant: "ghost", size: "sm" }) })}
      ${issue({ kind: "warn", id: "gx-is2", title: "Xaero's Minimap está como Cliente e servidor, mas só funciona no cliente", evidence: "O Modrinth informa client_side: required, server_side: unsupported.", actions: btn("Mudar lado para Só cliente", { size: "sm" }) })}
    </div>`);

  sec("toast", "Componentes", "Toast",
    "Confirma algo que acabou de acontecer, no canto de baixo à direita. Some sozinho em 6 s (pausa com o mouse ou o foco em cima). Nunca leva a única cópia de uma informação importante: erro que exige ação vira aviso inline ou diálogo.",
    `<div class="row row--top row--wrap row--gap-3" style="max-width:1200px">${toast({ kind: "ok", title: "Versão 1.5.0 salva", text: "Guardada neste computador.", actions: btn("Ver no Histórico", { size: "sm", variant: "ghost" }) })}${toast({ kind: "info", text: "Link do pack copiado." })}${toast({ kind: "danger", title: "Não foi possível publicar", text: "Sem conexão com o GitHub. Nada foi enviado.", actions: btn("Tentar de novo", { size: "sm" }) })}</div>
     <div style="margin-top:16px">${btn("Mostrar um toast de verdade", { attrs: { id: "gx-toast-live" } })}</div>`);

  sec("dialogo", "Componentes", "Diálogo",
    "Moldura dupla, cantos retos, sombra de bloco. Título é a pergunta ou a ação (frase, então em sans). O botão principal repete a ação. Foco entra no diálogo, Tab fica preso, Esc fecha e o foco volta para quem abriu.",
    `<div class="gx-stage">${dialog({ static: true, title: "Remover Just Enough Items (JEI)?", body: `<p>Estes mods dependem dele: <b>Just Enough Resources</b>. Sem o JEI, eles podem não funcionar.</p><p class="t-sm t-3">O item sai do pack. Dá para voltar atrás pelo Histórico.</p>`, foot: btn("Manter no pack", { variant: "ghost" }) + btn("Remover JEI", { variant: "danger", icon: "trash-2" }), size: "sm" })}</div>
     <div style="margin-top:12px" class="row">${btn("Abrir um diálogo de verdade", { attrs: { id: "gx-dlg-live" } })}<span class="t-sm t-3">Teste o teclado: Tab, Shift+Tab e Esc.</span></div>`);

  sec("painel", "Componentes", "Painel lateral",
    "Abre por cima da lista sem tirar você do lugar (detalhes de um mod, Tarefas). Mesmo contrato de teclado do diálogo.",
    `<div class="gx-stage">${drawer({ static: true, title: "Just Enough Items (JEI)", body: `<div class="row row--gap-3">${tile("Just Enough Items (JEI)", "xl")}<div><div class="t-strong">Just Enough Items (JEI)</div><div class="t-xs t-3">por mezz · ${source("curseforge")}</div></div></div><p class="t-sm t-2">Mostra todos os itens e receitas do jogo.</p><dl class="kv"><dt>Versão instalada</dt><dd class="t-mono">15.20.0.106</dd><dt>Arquivo</dt><dd class="path">jei-1.20.1-forge-15.20.0.106.jar · 1,4 MB</dd><dt>Usado por</dt><dd>Just Enough Resources</dd></dl>`, foot: btn("Atualizar para 15.20.0.112", { variant: "primary", icon: "refresh-cw" }) + btn("Remover", { variant: "danger-ghost", icon: "trash-2" }) })}</div>
     <div style="margin-top:12px">${btn("Abrir um painel de verdade", { attrs: { id: "gx-drw-live" } })}</div>`);

  sec("menu", "Componentes", "Menu suspenso",
    "Guarda o que é raro: ▾ do Testar, ⋯ de um pack, Mais opções de um mod. Itens com ícone e, quando preciso, uma linha de explicação. Setas navegam, Esc fecha.",
    `<div class="row row--top row--gap-4 row--wrap"><div class="gx-stage" style="width:420px;height:300px">${menu([{ label: "Mostrar na pasta", icon: "folder-open" }, { label: "Remover da lista", desc: "Não apaga nenhum arquivo", icon: "x", state: "hover" }, "sep", { label: "Apagar pack…", icon: "trash-2", danger: true, end: badge("p1", "P1") }, { label: "Exportar", icon: "package", disabled: true, desc: "Abra o pack para exportar" }], { id: "gx-menu-s", static: true, label: "Ações do pack", style: "left:24px;top:24px" })}</div>
      <div style="position:relative">${btn("Ações do pack", { iconEnd: "chevron-down", attrs: { "aria-haspopup": "menu", "aria-expanded": "false", "aria-controls": "gx-menu-live" } })}${menu([{ label: "Mostrar na pasta", icon: "folder-open" }, { label: "Remover da lista", desc: "Não apaga nenhum arquivo", icon: "x" }, "sep", { label: "Apagar pack…", icon: "trash-2", danger: true }], { id: "gx-menu-live", label: "Ações do pack", style: "left:0;top:44px" })}<p class="t-sm t-3" style="margin-top:8px">Menu de verdade: clique ou use ↓.</p></div></div>`);

  sec("tooltip", "Componentes", "Tooltip",
    "Explica o que não cabe no rótulo, nunca o óbvio. Aparece no hover e no foco do teclado; some com Esc. Obrigatório em botões só de ícone.",
    `<div class="row row--gap-4 row--wrap"><span class="tooltip tooltip--static">Onde o mod precisa estar instalado: no jogo de quem joga (cliente), no servidor ou nos dois</span>
      ${btn("Editar informações", { variant: "ghost", iconOnly: true, icon: "pencil" })}<span tabindex="0" data-tip="Nada para salvar: o pack está igual à última versão salva" class="t-sm" style="border-bottom:1px dotted var(--color-text-3)">Passe o mouse ou use Tab aqui</span></div>`);

  sec("vazio", "Componentes", "Estado vazio",
    "Alinhado à esquerda, com uma ilustração pequena em pixel: o que é, por que está vazio e o que fazer. Uma ação principal.",
    `<div class="stack-3">${empty({ title: "Nenhum mod ainda", text: "O pack Vale Sereno foi criado para Minecraft 1.20.1 com Forge 47.3.0. Comece pelos mods, resource packs ou shaders.", actions: btn("Adicionar mods", { variant: "primary", icon: "plus" }), hint: "Ou arraste arquivos .jar e .zip para cá.", glyph: "plus" })}
      ${empty({ kind: "", glyph: "search", artKind: "muted", title: "Nada encontrado para “mochila voadora”", text: "Só aparecem mods que funcionam em Minecraft 1.20.1 com Forge. Tente outro nome ou cole o link do mod.", actions: btn("Mostrar também os sem versão compatível", { variant: "link" }), compact: true })}
      ${empty({ kind: "ok", glyph: "check", title: "Nenhum problema encontrado", text: "Última verificação há 3 minutos. Ela também roda sozinha ao clicar em Testar e antes de exportar.", actions: btn("Verificar agora", { size: "sm" }), compact: true })}
      ${empty({ kind: "error", glyph: "x", title: "Não foi possível carregar as versões do Minecraft", text: "Sem internet e sem cópia guardada. Verifique a conexão e tente de novo.", actions: btn("Tentar de novo", { icon: "refresh-cw" }), compact: true })}</div>`);

  sec("skeleton", "Componentes", "Skeleton",
    "Reserva o lugar do conteúdo enquanto carrega, com a mesma forma final, para nada pular de lugar. Some em até 300 ms quando os dados já estão em cache.",
    `<div class="panel" style="max-width:640px"><div class="row row--gap-3"><span class="skeleton skeleton--tile" style="width:40px;height:40px"></span><div class="grow"><span class="skeleton skeleton--line" style="--w:40%;height:14px"></span><span class="skeleton skeleton--line" style="--w:70%"></span></div><span class="skeleton skeleton--btn"></span></div></div>`);

  sec("tabela", "Componentes", "Tabela",
    "Cabeçalho fixo ao rolar, números alinhados à direita, linha com hover. Ordenar pelo cabeçalho (aria-sort). Usada para Meus packs, versões de Java, arquivos a limpar e respostas da IA.",
    `<div class="tablewrap" style="max-width:860px"><table class="table"><thead><tr><th aria-sort="descending"><button type="button" class="sort">Java ${icon("chevron-down", "icon--sm")}</button></th><th>Usado por</th><th>Por que esta versão</th></tr></thead><tbody>
      <tr><td class="t-strong">25</td><td class="t-3">nenhum pack ainda</td><td>O mais novo. Usado pelo Minecraft 26.1 e seguintes.</td></tr>
      <tr><td class="t-strong">21</td><td>Leve e Bonito (1.21.1)</td><td>O Minecraft 1.20.5 a 1.21 foi feito para o Java 21. Com o 25 não há garantia de que abre.</td></tr>
      <tr><td class="t-strong">17</td><td>Vale Sereno (1.20.1)</td><td>O Minecraft 1.18 a 1.20.4 foi feito para o Java 17.</td></tr>
      <tr><td class="t-strong">8</td><td>Técnico Clássico (1.7.10)</td><td>O Forge 1.7.10 e 1.12.2 só abre no Java 8.</td></tr></tbody></table></div>`);

  sec("timeline", "Componentes", "Linha do tempo do Histórico",
    "Do mais novo para o mais antigo. Marcadores: tracejado âmbar (alterações não salvas), quadrado de osso (só salva), ciano (versão final), ciano aceso (publicada).",
    `<ol class="timeline" style="max-width:900px">
      ${tlItem({ kind: "unsaved", version: "Agora", summary: "5 alterações: 2 mods adicionados, 1 atualizado, 2 configs", actions: btn("Salvar versão", { variant: "primary", size: "sm", icon: "save" }) })}
      ${tlItem({ kind: "final", version: "1.4.2", date: "28/09/2026", summary: "2 mods atualizados, 1 config alterada", open: true, actions: btn("Publicar versão", { variant: "primary", size: "sm", icon: "globe" }) + btn("Fechar detalhes", { size: "sm", variant: "ghost", icon: "chevron-up" }), detail: `<pre class="code">## 1.4.2 (28/09/2026)\n### Atualizados\n- Create 0.5.1.i → 0.5.1.j\n- Embeddium 0.3.30 → 0.3.31\n### Configs alteradas\n- config/create-common.toml</pre>` })}
      ${tlItem({ kind: "saved", version: "1.4.1", date: "24/09/2026", summary: "Ajuste de configs do Create", actions: btn("Marcar como versão final", { size: "sm", icon: "flag" }) + btn("Ver mudanças", { size: "sm", variant: "ghost", icon: "chevron-down" }) })}
      ${tlItem({ kind: "published", version: "1.4.0", date: "20/09/2026", summary: "3 mods adicionados, 1 shader", actions: btn("Ver mudanças", { size: "sm", variant: "ghost", icon: "chevron-down" }) })}
    </ol>`);

  sec("ia", "Componentes", "Bloco de resposta da IA",
    "Tudo que vem da IA fica dentro desta moldura em osso, com o ícone de brilhinho e o aviso fixo de que a IA pode errar. Causa provável, confiança em 3 blocos, mods envolvidos e passos com botões que passam pelos fluxos normais.",
    `<div class="stack-3" style="max-width:860px">${aiBlock({ meta: "Teste de 30/09 às 21:14 · Google Gemini (gemini-2.5-flash) · 21:20", cause: "Dois mods de renderização, Embeddium e Rubidium, carregados juntos. O log mostra os dois tentando substituir o mesmo código de desenho dos blocos.", confidence: "média", mods: [btn("Embeddium", { variant: "link" }), btn("Rubidium", { variant: "link" })], steps: [["Remover o Rubidium (o Embeddium é o substituto mais novo).", btn("Remover Rubidium", { size: "sm", variant: "danger-ghost", icon: "trash-2" })], ["Testar de novo.", btn("Testar", { size: "sm", icon: "play" })]] })}
      ${aiBlock({ loading: true, meta: "Google Gemini (gemini-2.5-flash)" })}
      ${aiBlock({ error: "A chave do Gemini foi recusada.", errorText: "O Google respondeu que a chave não é válida. Nada foi alterado no pack.", meta: "21:20", actions: btn("Abrir Configurações", { size: "sm" }) })}</div>`);

  sec("outros", "Componentes", "Árvore, chave-valor, divulgação e teclas",
    "Peças menores usadas no editor de configs, nos detalhes de um mod e nas instruções.",
    states([
      st("Árvore de arquivos", `<div role="tree" aria-label="Arquivos de config" class="tree" style="width:100%"><button type="button" role="treeitem" class="tree__item" aria-expanded="true">${icon("folder-open")}config/</button><div role="group"><button type="button" role="treeitem" class="tree__item tree__item--dirty" aria-selected="true">${icon("file-text")}create-common.toml</button><button type="button" role="treeitem" class="tree__item">${icon("file-text")}embeddium-options.json</button><button type="button" role="treeitem" class="tree__item" aria-expanded="false">${icon("folder")}jei/</button><div role="group" hidden></div></div><button type="button" role="treeitem" class="tree__item tree__item--friendly">${icon("file-text")}Opções do jogo (options.txt)</button></div>`),
      st("Chave-valor", `<dl class="kv"><dt>Minecraft</dt><dd>1.20.1</dd><dt>Loader</dt><dd>Forge 47.3.0</dd><dt>Java</dt><dd>17 (automático)</dd></dl>`),
      st("Divulgação (Mais opções)", `<details class="disclosure"><summary>Por que não o Java 25?</summary><p>O Minecraft 1.18 a 1.20.4 foi feito para o Java 17.</p></details>`),
      st("Teclas", `<span class="t-sm">${["Ctrl", "S"].map((k) => `<kbd>${k}</kbd>`).join(" + ")} salva o arquivo</span>`),
      st("Abas", `<span class="t-sm t-2">Não existem no Warden. Use seção do menu, seletor de modo, caixa de seleção ou página corrida.</span>`),
    ], "gx-states--wide"));

  // ======================= MONTAGEM =======================
  const nav = document.getElementById("gx-nav");
  const main = document.getElementById("conteudo");
  let groups = {};
  S.forEach(([id, g, t]) => { (groups[g] = groups[g] || []).push([id, t]); });
  nav.innerHTML = `<span class="brand">${brandMark()}<span class="brand__name">Warden</span></span>` + Object.entries(groups).map(([g, items]) => `<h2>${g}</h2>${items.map(([id, t]) => `<a href="#${id}">${t}</a>`).join("")}`).join("");
  main.innerHTML = `<div class="gx-intro"><div class="t-caps t-3">Design system · versão 1.0 · 01/10/2026</div><h1 class="t-display-2xl" style="margin-top:8px">Warden · <span class="t-hl">Deep Dark</span></h1>
    <p>Fundamentos e componentes do Warden, com todos os estados. Esta página é montada com o mesmo CSS e as mesmas funções que o protótipo final usa. Regras completas em <code>docs/design/DESIGN-SYSTEM.md</code>; mapeamento para Tailwind 4 e shadcn/ui em <code>docs/design/HANDOFF.md</code>. Fonte única dos tokens: <code>design/system/tokens.json</code>.</p></div>` + S.map((s) => s[3]).join("");

  document.querySelectorAll("[data-v]").forEach((el) => { el.textContent = css(el.dataset.v); });
  window.WardenUI.bind(main);

  // Demonstrações ao vivo
  document.getElementById("gx-toast-live").addEventListener("click", () => window.WardenUI.toast(toast({ kind: "ok", title: "Versão 1.5.0 salva", text: "Guardada neste computador.", actions: btn("Ver no Histórico", { size: "sm", variant: "ghost" }) })));
  const live = (trigger, html) => document.getElementById(trigger).addEventListener("click", (e) => {
    const wrap = document.createElement("div");
    wrap.innerHTML = html;
    const layer = wrap.firstElementChild;
    document.body.appendChild(layer);
    window.WardenUI.openLayer(layer, e.currentTarget);
    layer.querySelectorAll("[data-act]").forEach((b) => b.addEventListener("click", () => window.WardenUI.requestClose(layer, "action")));
  });
  live("gx-dlg-live", dialog({ title: "Remover Just Enough Items (JEI)?", body: `<p>Estes mods dependem dele: <b>Just Enough Resources</b>.</p>`, foot: btn("Manter no pack", { variant: "ghost", attrs: { "data-act": "" } }) + btn("Remover JEI", { variant: "danger", icon: "trash-2", attrs: { "data-act": "" } }), size: "sm" }));
  live("gx-drw-live", drawer({ title: "Tarefas", body: `${progress({ label: "Baixando o Minecraft 1.20.1", value: 38, size: "sm", meta: "Vale Sereno" })}<div class="t-sm t-2">Concluídas: Verificar atualizações · Leve e Bonito · 14:01</div>`, foot: btn("Fechar", { attrs: { "data-act": "" } }) }));

  // Índice: marca a seção visível
  const links = [...nav.querySelectorAll("a")];
  const io = new IntersectionObserver((ents) => ents.forEach((en) => { if (en.isIntersecting) links.forEach((a) => a.setAttribute("aria-current", String(a.getAttribute("href") === "#" + en.target.id))); }), { rootMargin: "-20% 0px -70% 0px" });
  document.querySelectorAll(".gx-sec").forEach((s) => io.observe(s));
})();
