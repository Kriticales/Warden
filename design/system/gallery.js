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
    `<div class="stack-3" style="max-width:860px">${aiBlock({ meta: "Teste de 30/09 às 21:14 · Google Gemini (gemini-3.8-flash) · 21:20", cause: "Dois mods de renderização, Embeddium e Rubidium, carregados juntos. O log mostra os dois tentando substituir o mesmo código de desenho dos blocos.", confidence: "média", mods: [btn("Embeddium", { variant: "link" }), btn("Rubidium", { variant: "link" })], steps: [["Remover o Rubidium (o Embeddium é o substituto mais novo).", btn("Remover Rubidium", { size: "sm", variant: "danger-ghost", icon: "trash-2" })], ["Testar de novo.", btn("Testar", { size: "sm", icon: "play" })]] })}
      ${aiBlock({ loading: true, meta: "Google Gemini (gemini-3.8-flash)" })}
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

  // ======================= FUNÇÕES AVANÇADAS (D4) =======================
  const G = "Funções avançadas (D4)";
  sec("saude", G, "Saúde do pack",
    "Nota de 0 a 100, explicável e estável: começa em 100 e cada problema conhecido tira pontos. Faixa sempre com ícone e palavra (nunca só a cor). Texto de apoio fixo: não é promessa de que o pack funciona. Grande no topo de Problemas; pequena na coluna Saúde de Meus packs.",
    states([
      st("Ótimo (90 a 100)", W.health(97)), st("Bom (75 a 89)", W.health(88)), st("Atenção (50 a 74)", W.health(61)), st("Crítico (abaixo de 50)", W.health(33)), st("Sem dados", W.health(null, { note: "O pack ainda não foi verificado." })),
      st("Na tabela", `<div class="stack-2">${[97, 88, 61, 33].map((n) => W.health(n, { size: "sm" })).join("<br>")}</div>`),
    ], "gx-states--wide") +
    `<h3>O que tirou pontos</h3><div style="max-width:760px">${W.healthLosses([["−30", "Problemas do pré-teste:", "2 erros."], ["−20", "Último teste:", "travou hoje às 14:40.", btn("Ver", { size: "sm", variant: "ghost" })], ["−3", "Problemas do pré-teste:", "1 aviso.", "", true]])}</div>` +
    rules([["Explique cada ponto", "Toda linha diz a categoria, o motivo e leva ao problema."]], [["Prometer", "Nunca “o pack está saudável, pode publicar”. A nota resume o que se sabe."]]));

  sec("rodadas", G, "Busca do culpado: rodadas e suspeitos",
    "Trilha de rodadas no lugar das 5 etapas quando a tela do teste está no modo Busca do culpado. Cada resultado tem um ícone de forma diferente (x, visto, menos, relógio, carregador, tracejado) e o nome completo para leitor de tela. A faixa de suspeitos mostra quanto do pack ainda pode ser o culpado.",
    `<div class="stack" style="max-width:900px">${W.rounds([{ n: 0, mods: 118, result: "same" }, { n: 1, mods: 0, result: "pass" }, { n: 2, mods: 59, result: "pass" }, { n: 3, mods: 88, result: "same" }, { n: 4, mods: 73, result: "diff" }, { n: 5, mods: 66, result: "timeout" }, { n: 6, mods: 69, result: "now" }, { n: 7, result: "todo" }, { n: 8, result: "todo" }])}
      ${states([st("Rodada pausada", W.rounds([{ n: 5, result: "pass" }, { n: 6, result: "paused" }], { legend: false })), st("Esperando você (modo assistido)", W.rounds([{ n: 5, result: "pass" }, { n: 6, result: "ask" }], { legend: false })), st("Suspeitos: começo", W.suspects(118, 118, { from: 0 })), st("Suspeitos: rodada 6", W.suspects(7, 118, { from: 22 }))], "gx-states--wide")}</div>`);

  sec("conversa", G, "Conversa com a IA",
    "Mensagens, blocos do que foi enviado, evidências e propostas. Tudo que vem da IA fica na moldura de osso com o brilhinho. Cada consulta da IA aparece como um bloco “Enviado à IA” (ou “Enviado ao GitHub”) com o texto exato. Afirmação sem evidência conferida pelo Warden aparece riscada. Proposta só muda o pack com o clique em Aplicar.",
    `<div class="stack-3" style="max-width:860px">
      ${W.chatMsg({ from: "user", meta: "15:40", body: "<p>Trava quando entro no mundo. Já aconteceu 3 vezes.</p>" })}
      ${W.chatMsg({ from: "ai", meta: "gemini-3.8-flash · 15:41", body: W.toolCall({ what: "visão geral do pack", size: "2,1 KB", sent: "Minecraft 1.20.1 · Forge 47.3.0 · 128 itens" }) + W.toolCall({ what: "crash report de 30/09, linhas 1 a 80", size: "6,8 KB", open: true, sent: "Description: Ticking entity\njava.lang.NullPointerException…", back: "Linhas 1 a 80 do crash report." }) + W.toolCall({ kind: "github", what: "busca de issues no Supplementaries", size: "0,1 KB", sent: "GET …/search/issues?q=repo:MehVahdJukaar/Supplementaries+NullPointerException+travel" }) + W.toolCall({ state: "running", what: "o que o Epic Fight altera no jogo" }) + W.toolCall({ state: "error", what: "changelog do Epic Fight", error: "A CurseForge não respondeu. A IA seguiu sem esse dado." }) })}
      <ul class="claims">${W.claim("O pack está com a 2.8.17; a correção saiu na 2.8.21.", [W.evidence("issue #3121"), W.evidence("changelog 2.8.21")])}${W.claim("Também acontece em servidores grandes.", [W.evidence("", { unverified: true })], { unverified: true })}</ul>
      ${states([
        st("Proposta pendente", W.proposal({ title: "Atualizar Supplementaries 2.8.17 → 2.8.21", text: "Corrige o travamento.", evidence: W.evidence("issue #3121"), actions: btn("Aplicar", { variant: "primary", size: "sm", icon: "check" }) + btn("Descartar", { size: "sm", variant: "ghost" }) })),
        st("Aplicada", W.proposal({ title: "Atualizar Epic Fight 20.9.4 → 20.9.6", state: "applied", appliedText: "Aplicada às 15:52 · ponto de segurança criado", appliedActions: btn("Desfazer", { size: "sm", variant: "ghost", icon: "undo-2" }) })),
        st("Descartada", W.proposal({ title: "Remover Epic Fight", state: "dismissed" })),
      ], "gx-states--wide")}</div>` +
    rules([["Mostrar o que foi enviado", "Byte a byte, num bloco recolhido. É o que o consentimento por conversa promete."]], [["IA mudando o pack", "Nenhuma ferramenta da IA escreve no pack. Só o clique em Aplicar, que passa pelos fluxos normais."]]));

  sec("desempenho", G, "Faixa de desempenho",
    "Acima do console, durante o teste. A memória do jogo vem com um mini-gráfico em blocos dos últimos minutos; os blocos ficam âmbar acima de 90% do máximo. Sem leitura (argumento que desliga), o número dá lugar ao motivo.",
    `<div class="stack-3" style="max-width:980px">${W.perfStrip({ mem: ["3,1", 6], series: [2.4, 2.6, 2.8, 2.7, 3.0, 2.8, 3.1, 2.9, 3.2, 3.1, 3.0, 3.1], seriesLabel: "Memória nos últimos minutos", rss: "4,2", gc: "214", gcSub: "1,2 s parado em coletas", loaded: "1 min 42 s", loadedSub: "18 s a mais que na 1.4.2" })}
      ${W.perfStrip({ label: "Desempenho: memória alta", state: "warn", mem: ["5,9", 6], series: [5.0, 5.3, 5.5, 5.6, 5.7, 5.8, 5.9, 5.9, 5.8, 5.9, 5.9, 5.9], seriesLabel: "Memória alta", rss: "7,1", gc: "1.902", gcSub: "18,4 s parado", loaded: "1 min 42 s" })}
      ${W.perfStrip({ label: "Desempenho: sem leitura", state: "nodata", rss: "4,2", gc: "—", gcSub: "sem leitura", loaded: "1 min 42 s" })}</div>`);

  sec("console-agrupado", G, "Console agrupado e linha de comando",
    "Modo do console escolhido em “Mostrar: Linha a linha · Agrupado por mod · Só problemas” (seletor, não aba). Linhas repetidas viram uma com “×N”; ruído conhecido ganha o selo “comum, geralmente inofensivo”; stack trace vira um item recolhido com a primeira linha de mod em destaque. Com servidor, “Mostrando: Servidor · Jogo” e uma linha de comando.",
    `<div class="stack-3" style="max-width:980px">${consoleBox({ id: "gx-cg", label: "Console agrupado por mod", show: "grouped", height: 320, groups: [{ mod: "Supplementaries", errors: 0, warns: 1290, total: 1290, open: true, lines: [[1284, "warn", "Unable to load model: 'supplementaries:{id}'", true]] }, { mod: "Epic Fight", errors: 1, warns: 0, total: 51, open: true, lines: [], stack: { title: "java.lang.NullPointerException", frames: ["at net.minecraft.world.entity.LivingEntity.travel(LivingEntity.java:2108)", "at yesman.epicfight.world.capabilities.entitypatch.LivingEntityPatch.onTravel(LivingEntityPatch.java:412)"], hidden: 44, modFrame: 1 } }, { mod: "Jogo e loader", errors: 0, warns: 0, total: 412, lines: [[1, "info", "Found 128 mods"]] }] })}
      ${consoleBox({ id: "gx-cs", label: "Console do servidor", show: "lines", height: 200, source: { value: "server", options: [["server", "Servidor"], ["game", "Jogo"]] }, command: true, lines: [["info", "15:10:31", "minecraft/DedicatedServer", "Done (6.912s)! For help, type \"help\""]] })}</div>`);

  sec("raio-x", G, "Raio-x de mixins",
    "Linha de alteração: a parte do jogo em linguagem legível, quem altera e como, o risco com selo e o motivo, e o que diminui o risco (compatibilidade intencional, pode ser desligado pelo próprio mod). Linguagem: “alteram o mesmo ponto”, nunca “são incompatíveis”.",
    `<ul class="mixins" style="max-width:640px">${W.mixinRow({ target: "LivingEntity#travel (movimento das criaturas)", cls: "net.minecraft.world.entity.LivingEntity", who: [["Epic Fight", "@Redirect"], ["Supplementaries", "@Redirect"]], risk: "high", why: "Os dois trocam a mesma chamada. Só um vale." })}${W.mixinRow({ target: "Biome#getTemperature", who: [["Lithium", "@Overwrite"], ["ModernFix", "@Overwrite"]], risk: "medium", why: "Os dois substituem o método inteiro.", lower: "Pode ser desligado pelo próprio mod: o ModernFix desliga esta opção quando o Lithium está presente" })}${W.mixinRow({ target: "ModelPart#render", who: [["Sodium", "@Overwrite"], ["Iris", "@Overwrite"]], risk: "low", why: "Rebaixado de alto.", lower: "Compatibilidade intencional: o Iris referencia o Sodium neste ponto" })}</ul>`);

  sec("grafo", G, "Grafo focado de dependências",
    "Um mod no centro; o que ele exige à esquerda e quem depende dele à direita. Nós são botões (teclado: Tab; Enter leva o mod para o centro). O desenho das ligações é decorativo: as mesmas ligações vão em texto ao lado. No app, o grafo do pack inteiro (opcional) usa Cytoscape.js.",
    W.depGraph({ center: { name: "Create", sub: "0.5.1.j · você adicionou", inside: "Flywheel e Registrate" }, left: [{ name: "Forge", sub: "47.1 ou mais novo", kind: "required" }, { name: "JEI", sub: "no pack", kind: "optional" }], right: [{ name: "Create Slice & Dice", sub: "no pack", kind: "required" }, { name: "OptiFine", sub: "exemplo", kind: "incompatible" }, { name: "Kotlin for Forge", sub: "deduzida do log", kind: "inferred" }] }));

  sec("descoberta", G, "Descoberta: resultado com seleção e galeria",
    "Linha de resultado com caixa para seleção múltipla. Estados: normal, selecionado, já no pack (sem caixa), sem versão para o pack e arquivo fora das lojas (caixa desabilitada com o motivo), download manual. Galeria em miniaturas próprias desenhadas por código (nenhuma imagem da Mojang).",
    `<ul class="dlist" style="max-width:760px">
      ${W.discoverRow({ name: "Sophisticated Storage", author: "P3pp3rF1y", desc: "Baús e barris com melhorias.", src: "both", meta: "<span>21 mi downloads</span>" })}
      ${W.discoverRow({ name: "Sophisticated Backpacks", author: "P3pp3rF1y", desc: "Mochilas com melhorias.", src: "both", state: "selected", meta: "<span>48 mi downloads</span>" })}
      ${W.discoverRow({ name: "Traveler's Backpack", author: "Tiviacz1337", desc: "Mochilas temáticas.", src: "both", state: "inpack" })}
      ${W.discoverRow({ name: "Backpacked", author: "MrCrayfish", desc: "Uma mochila simples.", src: "curseforge", state: "manual" })}
      ${W.discoverRow({ name: "Backpacks!", author: "Lorie", desc: "Mochilas coloridas.", src: "modrinth", state: "noversion", why: "Sem versão para Forge 1.20.1" })}
      ${W.discoverRow({ name: "Cozy Create Tweaks", desc: "Ajustes do autor do modpack.", state: "external", compact: true })}</ul>
      <h3>Galeria</h3>${W.galleryStrip(["Mochilas", "Melhorias", "Filtro", "Cinto"])}
      <h3>Seleção múltipla</h3><div class="selbar" style="max-width:760px"><span class="selbar__count">3 selecionados</span>${btn("Limpar seleção", { size: "sm", variant: "ghost" })}<span class="grow"></span>${btn("Adicionar 3 ao pack", { variant: "primary", icon: "plus" })}</div>`);

  sec("configs-avancado", G, "Configs: busca em todas e formulário com padrão",
    "Resultado da busca agrupado por arquivo, com o nome traduzido do mod quando existe e a chave em mono. Linha do formulário com o padrão e de onde ele veio, marca de alterado, erro de faixa e valor deduzido (baixa confiança). Restaurar padrão por chave.",
    `<div class="stack" style="max-width:900px">${W.cfgHits([{ file: "config/waystones-common.toml", hits: [{ label: "Gerar pedras nas vilas", key: "spawnInVillages", value: "true", ctx: "#Se as vilas geram uma pedra. Valores pequenos geram muitas perto do <mark>spawn</mark>." }, { key: "worldGenFrequency", value: "25", changed: true }] }])}
      <div class="cfgform" style="padding:0">${W.cfgRow({ label: "Velocidade máxima de rotação", key: "maxRotationSpeed", desc: "Faixa: acima de 64.", def: "256", defSource: "do comentário do arquivo", changed: true, control: input({ bare: true, size: "sm", value: "512", ariaLabel: "Velocidade máxima de rotação" }) })}
        ${W.cfgRow({ label: "Multiplicador de stress", key: "stressMultiplier", def: "1.0", defSource: "do comentário do arquivo", error: "12.0 está fora da faixa 0,0 a 10,0. O Forge troca por 1,0 sem avisar.", errId: "gx-cf-err", control: input({ bare: true, size: "sm", value: "12.0", ariaLabel: "Multiplicador de stress", attrs: { "aria-invalid": "true", "aria-describedby": "gx-cf-err" } }) })}
        ${W.cfgRow({ label: "Alcance do ventilador", key: "fanPushDistance", def: "20", defSource: "tirado do texto", inferred: true, control: input({ bare: true, size: "sm", value: "20", ariaLabel: "Alcance do ventilador" }) })}
        ${W.cfgRow({ key: "customKey", control: input({ bare: true, size: "sm", value: "abc", ariaLabel: "customKey" }) })}</div></div>`);

  sec("scripts", G, "Scripts: autocompletar de IDs e erros do jogo",
    "IDs de itens, blocos e tags lidos dos jars do pack (sem abrir o jogo). Erros dos scripts vêm do log do último teste, com a linha original. Realce: string em verde, palavra-chave em lavanda, função em ciano, ID errado sublinhado em vermelho.",
    `<div class="grid-2" style="max-width:980px"><div style="position:relative;min-height:140px"><div class="editor" style="min-height:140px"><div class="editor__lines" style="position:relative"><div class="editor__line editor__line--error"><span>    <span class="tk-str tk-err">'minecraft:copper_ingott'</span>,</span></div></div></div>
      <div style="position:absolute;left:40px;top:40px">${W.completions([["copper_ingot", "item", "Minecraft"], ["copper_block", "bloco", "Minecraft"], ["copper_ore", "bloco", "Minecraft"]], { typed: "minecraft:copper_", id: "gx-cmp" })}</div></div>
      ${W.scriptErrors([{ where: "receitas.js, linha 16", msg: "O item <span class='t-mono'>minecraft:copper_ingott</span> não existe.", log: "Item 'minecraft:copper_ingott' not found" }], { id: "gx-se", meta: "último teste" })}</div>`);

  sec("testar-avancado", G, "Testar: perfil e busca do culpado; menu com grupos e rádio",
    "O botão Testar mostra o perfil quando ele não é o Padrão. Durante a busca do culpado o botão fica escuro com antenas âmbar e leva ao progresso. O menu ▾ separa os testes extras, o perfil (itens de rádio, role menuitemradio) e a instância em grupos com título.",
    `${states([st("Com perfil", testButton({ profile: "PC fraco", menuId: "gx-mp1" })), st("Buscando o culpado", testButton({ state: "bisect", progress: 44, menuId: "gx-mp2" }))])}
      <div class="gx-stage" style="height:520px"><div class="fake">Menu aberto (estático para a galeria)</div>${menu([{ label: "Ver último teste", desc: "Hoje, 14:40 · travou", icon: "history" }, { group: "Outros testes" }, { label: "Testar como servidor…", desc: "Abre um servidor só neste computador e entra nele", icon: "server" }, { label: "Encontrar o mod culpado…", desc: "Abre o jogo em rodadas até achar o mod", icon: "target" }, { group: "Perfil do teste" }, { radio: "gx-p", label: "Padrão", checked: true, end: '<span class="t-xs t-3">6 GB</span>' }, { radio: "gx-p", label: "PC fraco", end: '<span class="t-xs t-3">4 GB</span>' }, { label: "Ajustes do teste neste computador…", icon: "settings" }], { id: "gx-menu-adv", static: true, label: "Menu do Testar", style: "left:24px;top:48px" })}</div>`);

  sec("secmenu-compacto", G, "Menu de seções recolhido",
    "Só na página de descoberta, que precisa de três colunas. Ícones com o nome e a descrição no tooltip e no nome acessível; o ponto no canto repete o contador (vermelho com erro, âmbar com não salvas). Volta ao normal ao sair da página.",
    `<div style="height:330px;display:flex;border:1px solid var(--color-border)">${sectionMenu([{ id: "mods", name: "Mods", desc: "Mods, resource packs e shaders", icon: "puzzle", count: 128 }, { id: "configs", name: "Configs", desc: "Arquivos de ajuste e scripts do pack", icon: "file-code" }, { id: "problemas", name: "Problemas", desc: "Saúde do pack, problemas e travamentos", icon: "triangle-alert", count: 4, countKind: "danger", countLabel: "4 problemas" }, { id: "ia", name: "Diagnóstico com IA", desc: "Conversar com a IA sobre um problema do pack", icon: "sparkles", ai: true }, { id: "historico", name: "Histórico", desc: "Versões salvas e publicação", icon: "history", count: 5, countKind: "warn", countLabel: "5 alterações não salvas" }, { id: "exportar", name: "Exportar", desc: "Gerar o pack para quem vai jogar", icon: "package" }], "mods", { compact: true, label: "Seções do pack (recolhido)" })}<div class="fake" style="padding:16px">Página de descoberta</div></div>`);

  // ======================= WARDEN 1.1 (PROFISSIONAL) =======================
  const G2 = "Warden 1.1 (Profissional)";
  const VER_ROWS = (o = {}) => [
    { icon: "shield", title: "Segurança dos mods", summary: o.secSum || "Cada arquivo conferido com o oficial do Modrinth ou da CurseForge.", statusKind: o.sec || "ok", statusText: o.secText || "128 conferem", linkAttrs: { "data-demo": "" } },
    { icon: "wrench", title: "Manutenção dos mods", summary: o.mntSum || "Mods removidos, arquivados ou sem atualização.", statusKind: o.mnt || "ok", statusText: o.mntText || "Nenhum problema" },
    { icon: "boxes", title: "Itens repetidos entre mods", summary: o.dupSum || "Materiais que mais de um mod adiciona.", statusKind: o.dup || "ok", statusText: o.dupText || "Nada repetido" },
  ];
  sec("v11-verificacoes", G2, "Verificações do pack",
    "Painel curto no topo de Problemas, logo abaixo da saúde. Uma linha por verificação: o que é, o resultado com ícone e palavra e o link “Ver” para a página de detalhe (← Problemas). A linha inteira não é botão; o link é o controle, com nome acessível completo (“Ver: Segurança dos mods”). Itens repetidos aparecem como Conselho (lavanda), nunca como erro.",
    states([
      st("Tudo certo", verifyPanelDemo({ meta: "Última verificação: hoje, 14:52" })),
      st("Com perigo", verifyPanelDemo({ meta: "Última verificação: hoje, 14:52", sec: "danger", secText: "1 arquivo não confere", secSum: "<b>Waystones</b> não é igual ao arquivo oficial do Modrinth.", mnt: "warn", mntText: "2 mods sem manutenção" })),
      st("Com aviso", verifyPanelDemo({ meta: "Última verificação: ontem, 21:10", mnt: "warn", mntText: "2 mods sem manutenção", mntSum: "1 removido do Modrinth, 1 arquivado pelo autor." })),
      st("Verificando", verifyPanelDemo({ meta: "Verificando agora", sec: "loading", secText: "Conferindo 74 de 128", mnt: "neutral", mntText: "Na fila", dup: "neutral", dupText: "Na fila" })),
      st("Conselho (itens repetidos)", verifyPanelDemo({ meta: "Última verificação: hoje, 14:52", dup: "info", dupText: "Conselho: 3 materiais", dupSum: "Cobre, estanho e chumbo aparecem em mais de um mod." })),
    ], "gx-states--1") +
    rules([["Resumo aqui, detalhe na página", "Três linhas, sempre na mesma ordem. As dezenas de resultados ficam na página de cada verificação."], ["O link é o controle", "Só o “Ver” é clicável. Ele leva à página de detalhe com “← Problemas”."]],
      [["Linha inteira clicável", "Esconde o que é controle e briga com a seleção de texto do resumo."], ["Conselho como aviso", "Itens repetidos não quebram o jogo: lavanda e a palavra “Conselho”, nunca âmbar ou vermelho."]]));
  function verifyPanelDemo(o) { return W.verifyPanel(VER_ROWS(o), { meta: o.meta, actions: o.sec === "loading" ? "" : btn("Verificar de novo", { size: "sm", variant: "ghost", icon: "refresh-cw" }) }); }

  sec("v11-seguranca", G2, "Selo de segurança do arquivo",
    "Resultado da conferência de cada arquivo, com escudo de forma diferente para cada caso e o texto sempre visível. Aparece na página Segurança dos mods (uma linha por arquivo), no bloco “Segurança do arquivo” dos detalhes do mod e, em versão curta, na linha de mod quando o resultado pede atenção. Acima dos resultados, sempre o aviso honesto de que o Warden não é um antivírus.",
    states([
      st("Oficial (confere)", W.secStatus("oficial", "Confere com o arquivo oficial do Modrinth")),
      st("Arquivo do computador", W.secStatus("escaneado")),
      st("Não confere", W.secStatus("naoconfere")),
      st("Sinal de malware conhecido", W.secStatus("suspeito")),
      st("Pontos de atenção", W.secStatus("atencao", "2 pontos de atenção")),
      st("Não deu para conferir", W.secStatus("semconferir")),
      st("Ainda não conferido", W.secStatus("pendente")),
      st("Você confiou", W.secStatus("confiado")),
      st("Curto, na linha de mod", `<div class="row row--wrap">${W.secStatus("naoconfere", null, { size: "sm" })}${W.secStatus("suspeito", null, { size: "sm" })}${W.secStatus("semconferir", null, { size: "sm" })}</div>`),
    ], "gx-states--wide") +
    `<h3>Aviso honesto (W.alert neutro, sem componente novo)</h3><div style="max-width:760px">${alert({ kind: "neutral", icon: "shield", title: "O Warden não é um antivírus.", text: "Ele confere se cada arquivo é igual ao oficial e procura sinais de programas maliciosos já conhecidos em mods. Um mod malicioso novo pode passar sem ser notado." })}</div>
     <p class="gx-note">Fica no topo da página Segurança dos mods e no bloco do painel de detalhes. “Malware” virou “programas maliciosos” (glossário: “malware” é jargão). Sempre neutro: não é erro nem alarme.</p>` +
    rules([["Dizer o que foi feito", "“Confere com o arquivo oficial do Modrinth” diz a origem da conferência. “Hash” só nos detalhes técnicos."], ["Escudo diferente por resultado", "Visto, x, exclamação, interrogação, reticências, pessoa: a forma diz o resultado sem a cor."]],
      [["Prometer segurança", "Nada de “seguro”, “protegido” ou “livre de vírus”. O Warden só sabe o que conferiu."], ["Selo verde em toda linha de mod", "Na lista de Mods só aparecem os resultados que pedem atenção; os bons ficam na página e no painel."]]));

  sec("v11-substituto", G2, "Substituto sugerido",
    "Lista de candidatos no painel de detalhes do mod (“Procurar substituto”, com “← Detalhes”). Cada linha diz por que foi sugerida, em 2 a 4 motivos curtos tirados dos dados das plataformas. Lista densa, nunca cartões em grade. “Trocar por este” passa pelo fluxo normal de adicionar e remover.",
    `<p class="gx-note" style="margin:0 0 8px">Largura do painel de detalhes (440 px). Exemplo: substitutos do Rubidium, arquivado pelo autor.</p><ul class="replist" style="max-width:440px">
      ${W.replacementRow({ name: "Embeddium", author: "embeddedt", src: "modrinth", downloads: "21 mi", updated: "há 3 dias", reasons: ["mesma categoria no Modrinth: Otimização", "tem versão para Forge 1.20.1", "também é Só cliente, como o Rubidium", "o mais baixado da categoria para Forge"] })}
      ${W.replacementRow({ name: "ModernFix", author: "embeddedt", src: "both", downloads: "38 mi", updated: "há 2 semanas", reasons: ["mesma categoria: Otimização", "tem versão para Forge 1.20.1"], state: "inpack" })}
      ${W.replacementRow({ id: "gx-rep-sod", name: "Sodium", author: "jellysquid3", src: "modrinth", downloads: "64 mi", updated: "há 5 dias", reasons: ["mesma categoria: Otimização", "faz o mesmo que o Rubidium no Fabric"], state: "noversion", why: "Sem versão para Forge 1.20.1" })}
      ${W.replacementRow({ state: "loading" })}</ul>
     <p class="gx-note">De cima para baixo: normal, já no pack, sem versão para o pack (botão desabilitado com o motivo ao lado), carregando (skeleton).</p>` +
    rules([["Motivos verificáveis", "Categoria, versão para o pack, data da última atualização, downloads: dados que a pessoa pode conferir na página do mod."]],
      [["Ordem por “nota”", "Nada de estrelas ou pontuação inventada. A ordem vem da plataforma e os motivos explicam."]]));

  sec("v11-log", G2, "Campo do log do jogador",
    "Variação da busca ou link para a página Travamento de um jogador: colar o link de um serviço de logs ou escolher o arquivo. Rótulo visível, status ligado ao campo (aria-describedby) e o erro dizendo o que fazer.",
    states([
      st("Vazio", W.logSource({ id: "gx-log1" })),
      st("Link reconhecido", W.logSource({ id: "gx-log2", state: "recognized", value: "https://mclo.gs/8fKq2Lw" })),
      st("Link inválido", W.logSource({ id: "gx-log3", state: "invalid", value: "https://drive.google.com/file/d/1aB…" })),
      st("Baixando", W.logSource({ id: "gx-log4", state: "loading", value: "https://mclo.gs/8fKq2Lw" })),
    ].map((x) => x.replace('class="gx-state"', 'class="gx-state" style="max-width:680px"')), "gx-states--1") +
    rules([["Aceitar link e arquivo no mesmo lugar", "O jogador manda o que tiver: link do mclo.gs, pastebin, gist, ou o arquivo latest.log e o crash report."]], [["Abas Link | Arquivo", "Um campo e um botão ao lado resolvem sem trocar de modo."]]));

  sec("v11-versao-log", G2, "Versão do pack no log do jogador",
    "Resultado da comparação entre a lista de mods do log e as versões salvas do pack. Título que responde a pergunta (“O jogador usa a versão 1.4.2”), medidor textual com barra em blocos e as diferenças num bloco recolhível, com versões em mono.",
    states([
      st("Exata", W.versionMatch({ id: "gx-vm1", variant: "exact", version: "1.4.2", date: "18/09/2026", matched: 128, total: 128 })),
      st("Próxima", W.versionMatch({ id: "gx-vm2", variant: "near", version: "1.4.0", matched: 125, total: 128, open: true, diffs: { extra: [["OptiFine", "HD_U_I6"]], missing: [["Xaero's Minimap", "24.6.1"]], changed: [["Create", "0.5.1.j", "0.5.1.f"]] } })),
      st("Desconhecida", W.versionMatch({ id: "gx-vm3", variant: "unknown", matched: 12, total: 128 })),
      st("Incompleta", W.versionMatch({ id: "gx-vm4", variant: "incomplete" })),
    ], "gx-states--wide") +
    rules([["Responder no título", "A primeira linha diz a versão (ou que não deu para saber). O número de mods que conferem vem em texto, a barra só reforça."]], [["Esconder a dúvida", "Se o log não traz a lista de mods, dizer isso e o que pedir ao jogador; não chutar uma versão."]]));

  sec("v11-grupos", G2, "Grupos e nota do mod",
    "Grupo é uma etiqueta criada pelo usuário (Desempenho, Geração de mundo…): bloco reto com um ponto em pixel, diferente das tags de fonte e de estado. O seletor de grupos é uma lista de caixas com “Novo grupo…” no fim, usada no menu “Pôr no grupo ▾” e no painel. O bloco “Nota e grupos” fica no topo do painel de detalhes e salva sozinho ao sair do campo.",
    states([
      st("Chip", `<div class="row row--wrap">${W.groupChip("Desempenho")}${W.groupChip("Geração de mundo")}${W.groupChip("Desempenho", { size: "sm" })}</div>`),
      st("Chip removível", `<div class="row row--wrap">${W.groupChip("Desempenho", { removable: true })}${W.groupChip("Geração de mundo", { removable: true })}</div>`),
      st("Seletor com marcados", W.groupPicker({ framed: true, groups: [["Desempenho", true, 12], ["Geração de mundo", false, 9], ["Visual", true, 6], ["Só para o servidor", false, 3]] })),
      st("Seletor vazio", W.groupPicker({ framed: true, groups: [] })),
    ], "gx-states--wide") +
    `<h3>Nota e grupos (topo do painel de detalhes)</h3>` +
    states([
      st("Vazio", W.noteBlock({ state: "empty" })),
      st("Preenchido", W.noteBlock({ state: "filled", note: "Deixa o jogo mais leve em PCs fracos. Não tirar sem testar o Embeddium.", groups: ["Desempenho"] })),
      st("Editando (perto do limite)", W.noteBlock({ state: "editing", note: "Precisa ficar: o servidor usa as receitas dele para as máquinas do Create. Testado na 1.4.2 com 6 GB. Se travar ao entrar no mundo, olhar primeiro o Supplementaries, que altera o mesmo ponto.", groups: ["Desempenho", "Só para o servidor"] })),
      st("Salvo", W.noteBlock({ state: "saved", note: "Deixa o jogo mais leve em PCs fracos.", groups: ["Desempenho", "Visual"] })),
    ], "gx-states--wide") +
    rules([["Nota curta e útil", "Até 200 caracteres, com contador. Responde “por que este mod está no pack?”."]],
      [["Cor por grupo", "Grupos não ganham cores próprias: elas competiriam com as cores de estado e de fonte."], ["Chamar de categoria ou tag", "“Categoria” é das plataformas; “tag” se confunde com as tags de itens."]]));

  const MODS11 = [
    { name: "Embeddium", note: "Deixa o jogo mais leve em PCs fracos.", desc: "Deixa o jogo mais leve.", ver: "0.3.31", src: "modrinth", side: "client", groups: ["Desempenho"] },
    { name: "Create", desc: "Engrenagens, eixos e máquinas que se mexem.", ver: "0.5.1.j", src: "modrinth", side: "both", groups: ["Tecnologia", "Servidor", "Visual"] },
    { name: "Waystones", note: "Pedido dos jogadores do servidor.", ver: "14.1.6", src: "modrinth", side: "both", kind: "error", sec: "naoconfere" },
    { name: "Xaero's Minimap", desc: "Minimapa no canto da tela.", ver: "24.6.1", src: "curseforge", side: "both", kind: "warn", maint: { kind: "removed", text: "Removido da CurseForge" } },
    { name: "Clumps", desc: "Junta orbes de experiência.", ver: "12.0.0.4", src: "modrinth", side: "both", maint: { kind: "archived" }, groups: ["Desempenho"] },
  ];
  sec("v11-modrow", G2, "Linha de mod com nota, grupos e marcas",
    "Opções novas e opcionais do modRow. A nota toma o lugar da descrição (com o ícone de nota e texto mais claro). Até 2 grupos ao lado do nome; o resto vira “+N”. A marca de manutenção e o selo curto de segurança entram ao lado do nome, como as outras marcas, e só quando pedem atenção. Cabe em 1024 px: o nome quebra linha antes de a tabela estourar.",
    `<div class="tablewrap"><table class="table">${modTableHead()}<tbody>${MODS11.map((m) => modRow(m)).join("")}</tbody></table></div>
     <p class="gx-note">Linhas: com nota e grupo; 3 grupos (“+1”); com erro de segurança (não confere) e nota; removido da plataforma; arquivado pelo autor com grupo. “Sem atualização há 2 anos” e “Sem versão para o 1.21” usam a mesma marca, neutra: ${W.maintTag("stale")} ${W.maintTag("nonewer")}</p>` +
    rules([["Uma marca de cada tipo", "No máximo: grupos, um selo de estado, uma marca de manutenção e o selo de segurança. O detalhe fica no painel."]], [["Selo verde de segurança na lista", "“Confere com o oficial” em 128 linhas vira ruído. Na lista só entra o que pede atenção."]]));

  const PERF = [
    { version: "1.2.0", date: "12/08/2026", value: 82, tests: 3 },
    { version: "1.3.0", date: "29/08/2026", value: 86, tests: 4 },
    { version: "1.4.0", date: "05/09/2026", value: null, tests: 0 },
    { version: "1.4.2", date: "18/09/2026", value: 91, tests: 3 },
    { version: "1.5.0", date: "30/09/2026", value: 112, tests: 3 },
  ];
  const MEM = PERF.map((r, i) => Object.assign({}, r, { value: [4.1, 4.3, null, 4.4, 5.6][i] }));
  const TICK = PERF.map((r, i) => Object.assign({}, r, { value: [31, 33, null, 34, 36][i] }));
  sec("v11-desempenho", G2, "Desempenho entre versões",
    "Página de detalhe no Histórico. Uma barra feita de blocos por versão salva, com a mediana dos testes neste computador e no mesmo perfil; o número fica em cima da barra (Manrope) e a contagem de testes embaixo. Versão sem testes comparáveis: barra vazia tracejada. Versão mais pesada: blocos e contorno âmbar mais o selo “Mais pesada” (ícone e palavra). A tabela logo abaixo é o conteúdo acessível; o desenho fica escondido do leitor de tela.",
    `<div class="stack-3" style="max-width:760px">
      <div class="row row--wrap"><span class="t-sm t-2">Mostrar</span>${segmented([["open", "Tempo para abrir"], ["mem", "Memória máxima"], ["tick", "Tempo por tick"]], "open", "Medida do gráfico")}<span class="grow"></span><span class="t-xs t-3">Perfil Padrão · este computador</span></div>
      ${alert({ kind: "warn", title: "A versão 1.5.0 está mais pesada", text: "Abre em 1 min 52 s, 21 s a mais que a 1.4.2 (+23%). A memória máxima também subiu de 4,4 para 5,6 GB.", actions: btn("Ver o que mudou na 1.5.0", { size: "sm" }) })}
      ${W.versionChart({ values: PERF, unit: "s", label: "Tempo para abrir", highlight: 4, caption: "Mediana dos testes de cada versão. Barras começam do zero." })}</div>
     <h3>Outras medidas e estados</h3>` +
    states([
      st("Memória máxima (sem destaque, role=img)", W.versionChart({ values: MEM, unit: "GB", label: "Memória máxima", table: false })),
      st("Tempo por tick", W.versionChart({ values: TICK, unit: "ms", label: "Tempo por tick", table: false })),
      st("Carregando", W.versionChart({ loading: true })),
    ], "gx-states--1") +
    `<p class="gx-note">Com <code>table: false</code> o gráfico vira <code>role="img"</code> com um resumo em texto no nome acessível; use só onde a tabela já aparece em outro lugar da tela. Cada coluna tem tooltip no hover (versão, mediana, testes e diferença para a anterior).</p>` +
    rules([["Comparar com o mesmo perfil e computador", "Só entram testes comparáveis. Sem eles, a barra fica vazia e diz isso."], ["Barra a partir do zero", "Sem eixo cortado: a diferença visual é a diferença real."]],
      [["Destaque só na cor", "A versão mais pesada tem o selo com ícone e palavra, contorno e a linha marcada na tabela."], ["Dois eixos ou duas medidas juntas", "Uma medida por vez, escolhida em “Mostrar” (seletor de modo, não aba)."]]));

  sec("v11-repetidos", G2, "Itens repetidos entre mods",
    "Uma linha por material na página Itens repetidos entre mods. É um conselho, não um erro: barra lavanda e selo “Conselho”. Resumo em uma frase, mods recolhidos com o ID do item em mono e a marca “gera minério” ou “só item”, de onde veio a informação e a solução certa para a versão do Minecraft.",
    `<ul class="dups" style="max-width:820px">
      ${W.dupMaterial({ material: "Cobre", summary: "4 mods têm o próprio lingote · 3 geram minério", open: true, mods: [{ mod: "Minecraft", id: "minecraft:copper_ingot", ore: true }, { mod: "Mekanism", id: "mekanism:ingot_copper", ore: true }, { mod: "Thermal Foundation", id: "thermal:copper_ingot", ore: false }, { mod: "Immersive Engineering", id: "immersiveengineering:ingot_copper", ore: true }], evidence: "tag <span class=\"t-mono\">c:ingots/copper</span> nos jars de 4 mods", solution: { name: "AlmostUnified", text: "o AlmostUnified faz os mods usarem um item só e para de gerar os minérios repetidos." } })}
      ${W.dupMaterial({ material: "Engrenagem de ferro", state: "items", summary: "3 mods têm a própria engrenagem · nenhum gera minério", mods: [{ mod: "Thermal Foundation", id: "thermal:iron_gear" }, { mod: "Mekanism", id: "mekanism:gear_iron" }, { mod: "Create", id: "create:iron_gear" }], evidence: "tag <span class=\"t-mono\">c:gears/iron</span>", solution: { name: "AlmostUnified", text: "o AlmostUnified faz as receitas usarem uma engrenagem só." } })}
      ${W.dupMaterial({ material: "Estanho", state: "solved", solvedBy: "AlmostUnified", summary: "3 mods têm o próprio lingote · 2 geram minério", mods: [{ mod: "Mekanism", id: "mekanism:ingot_tin", ore: true }, { mod: "Thermal Foundation", id: "thermal:tin_ingot", ore: true }, { mod: "Immersive Engineering", id: "immersiveengineering:ingot_tin" }], solution: { attrs: {}, label: "Abrir a config do AlmostUnified" } })}
      ${W.dupMaterial({ material: "Cobre", state: "legacy", mc: "Minecraft 1.12.2", summary: "3 mods têm o próprio lingote · 3 geram minério", mods: [{ mod: "Thermal Foundation", id: "thermalfoundation:material:128", ore: true }, { mod: "Mekanism", id: "mekanism:ingot:5", ore: true }, { mod: "IndustrialCraft 2", id: "ic2:ingot:2", ore: true }], evidence: "nome no dicionário de minérios <span class=\"t-mono\">ingotCopper</span>", solution: { name: "UniDict", text: "o UniDict faz o mesmo papel nesta versão." } })}</ul>
     <p class="gx-note">De cima para baixo: com minério, só itens, já resolvido (unificador no pack), versão antiga do Minecraft (sugestão diferente).</p>` +
    rules([["Explicar o efeito no jogo", "“4 mods têm o próprio lingote” é o que a pessoa vê no inventário. “Unificar” vem explicado: faz os mods usarem um item só."]], [["Tratar como erro", "Nada quebra por isso. Sem vermelho, sem contar na lista de erros e avisos."]]));

  // ======================= MONTAGEM =======================
  const nav = document.getElementById("gx-nav");
  const main = document.getElementById("conteudo");
  let groups = {};
  S.forEach(([id, g, t]) => { (groups[g] = groups[g] || []).push([id, t]); });
  nav.innerHTML = `<span class="brand">${brandMark()}<span class="brand__name">Warden</span></span>` + Object.entries(groups).map(([g, items]) => `<h2>${g}</h2>${items.map(([id, t]) => `<a href="#${id}">${t}</a>`).join("")}`).join("");
  main.innerHTML = `<div class="gx-intro"><div class="t-caps t-3">Design system · versão 1.1 · 01/10/2026</div><h1 class="t-display-2xl" style="margin-top:8px">Warden · <span class="t-hl">Deep Dark</span></h1>
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
