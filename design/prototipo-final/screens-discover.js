/* Página de descoberta (D4): o Adicionar em tela cheia, com o menu lateral recolhido para ícones.
   Tipo: Mods · Resource packs · Shaders · Modpacks. Início com o campo vazio, resultados com
   seleção múltipla, pré-visualização rolável e "navegar por modpacks". Sem abas. */
(function () {
  "use strict";
  const W = window.W, D = window.DATA;
  const { def, go, B, packShell, pageHead, sim } = window.P;
  const shell = (content, o = {}) => packShell("mods", content, Object.assign({ compact: true }, o));

  // ---------- Partes comuns ----------
  function head(o = {}) {
    const back = o.added ? `Voltar para Mods · ${o.added} adicionados` : "Voltar para Mods";
    return pageHead("Adicionar ao pack", null, null, { back: [back, "mods"] });
  }
  function top(o = {}) {
    const omni = o.omni || { state: "empty" };
    const type = W.select({ bare: true, ariaLabel: "Tipo", options: [["mods", "Tipo: Mods"], ["rp", "Tipo: Resource packs"], ["sh", "Tipo: Shaders"], ["mp", "Tipo: Modpacks"]], value: o.type || "mods", attrs: { "data-type-select": "" } });
    return `<div class="add-top">${type}${W.omnibox(Object.assign({ id: "add-q" }, omni))}${B("Escolher arquivo do computador…", "", { icon: "file-plus" })}</div>`;
  }
  function filters(o = {}) {
    const cats = D.CATEGORIES.map(([c, n], i) => W.check({ label: `${c} <span class="t-3">${n}</span>`, checked: o.cat === c || (o.checkedFirst && i === 3) })).join("");
    return `<aside class="disc__filters" aria-label="Filtros">
      <p class="filters-lock">${W.icon("lock", "icon--sm")}<span>Só o que funciona em <b>Minecraft 1.20.1 com Forge</b>.</span></p>
      <div class="disc__fgroup"><div class="field__label">Categorias</div><div class="stack-2">${cats}</div></div>
      ${W.select({ id: "df-env", label: "Ambiente", options: ["Qualquer", "Funciona no cliente", "Funciona no servidor"] })}
      ${W.select({ id: "df-sort", label: "Ordenar", options: o.type === "mp" ? ["Mais baixados", "Atualizados recentemente", "Mais novos"] : ["Relevância", "Mais baixados", "Atualizados recentemente", "Mais novos"] })}
      <details class="disclosure"><summary>Mais filtros</summary><div class="stack-2" style="margin-top:6px">${W.select({ id: "df-src", label: "Fonte", options: ["Modrinth e CurseForge", "Só Modrinth", "Só CurseForge"] })}${W.check({ label: "Mostrar também os sem versão compatível" })}</div></details></aside>`;
  }
  const row = (r, extra = {}) => W.discoverRow(Object.assign({ name: r[0], author: r[1], desc: r[2], src: r[5], meta: `<span>${r[3]} downloads</span><span>atualizado ${r[4]}</span>`, state: r[6] ? "inpack" : "normal", attrs: go("adicionar-busca") }, extra));

  // ---------- Início (campo vazio) ----------
  function startContent() {
    const kit = `<li class="drow drow--nobox"><span class="tile tile--lg" aria-hidden="true" style="display:grid;place-items:center;color:var(--color-primary-text)">${W.icon("gauge", "icon--lg")}</span><span class="drow__main"><span class="drow__line"><b>Desempenho (Forge 1.20.1)</b> ${W.tag("Kit", "plain")}</span><span class="drow__desc">Embeddium, FerriteCore, ModernFix e mais 2. Deixam o jogo mais leve sem mudar como ele é jogado. 1 já está no pack.</span></span><span class="drow__end">${B("Ver o kit", "kit", { size: "sm" })}</span></li>`;
    return head() + top() + `<div class="disc disc--start">${filters()}<div class="disc__main stack">
      <section aria-labelledby="ds-pop"><div class="disc__h"><h2 class="group-title" id="ds-pop" style="margin:0">Populares para Forge 1.20.1</h2>${B("Ver mais", "adicionar-busca", { variant: "link" })}</div><ul class="dlist">${D.POPULAR.map((r) => row(r)).join("")}</ul><p class="t-xs t-3" style="margin-top:6px">Create, Jade, Farmer's Delight e Waystones também estão entre os mais baixados, mas já estão no seu pack.</p></section>
      <section aria-labelledby="ds-upd"><div class="disc__h"><h2 class="group-title" id="ds-upd" style="margin:0">Atualizados recentemente</h2>${B("Ver mais", "adicionar-busca", { variant: "link" })}</div><ul class="dlist">${D.UPDATED.map((r) => row(r)).join("")}</ul></section>
      <section aria-labelledby="ds-kit"><h2 class="group-title" id="ds-kit">Kits de desempenho</h2><ul class="dlist">${kit}</ul></section></div></div>
      ${sim("Ver outros casos:", [["buscar “backpack”", "adicionar-busca"], ["colar um link do Modrinth", "adicionar-link"], ["sem chave da CurseForge", "adicionar-semchave"], ["Tipo: Modpacks", "adicionar-modpacks"]])}`;
  }
  def("adicionar", { group: "pack", title: "Adicionar: início da descoberta (tela cheia)", spec: "T08", d4: true, render: () => shell(startContent()) });

  // ---------- Resultados com seleção múltipla ----------
  const RESULTS = [
    { name: "Sophisticated Backpacks", author: "P3pp3rF1y", desc: "Mochilas com melhorias, filtros e muito espaço.", src: "both", meta: "<span>48 mi downloads</span><span>atualizado hoje</span>", state: "selected", current: true },
    { name: "Traveler's Backpack", author: "Tiviacz1337", desc: "Mochilas temáticas com tanques de líquido.", src: "both", meta: "<span>30 mi downloads</span><span>atualizado há 1 mês</span>", state: "inpack" },
    { name: "Sophisticated Storage", author: "P3pp3rF1y", desc: "Baús e barris com melhorias.", src: "both", meta: "<span>21 mi downloads</span><span>atualizado há 6 dias</span>", state: "selected" },
    { name: "Backpacked", author: "MrCrayfish", desc: "Uma mochila simples que aparece nas costas.", src: "curseforge", meta: "<span>9 mi downloads</span><span>atualizado há 2 meses</span>", state: "manual" },
    { name: "Packed Up", author: "Ender", desc: "Mochilas que combinam com o estilo do jogo.", src: "modrinth", meta: "<span>1,2 mi downloads</span><span>atualizado há 4 meses</span>", state: "selected" },
    { name: "Backpacks!", author: "Lorie", desc: "Mochilas coloridas com 27 espaços.", src: "modrinth", meta: "<span>840 mil downloads</span><span>atualizado há 1 ano</span>", state: "noversion", why: "Sem versão para Forge 1.20.1 (só Fabric)" },
  ];
  function preview(o = {}) {
    const only = o.onlyModrinth;
    const vers = [["3.20.17", "Estável", "hoje", "- Filtro de itens no cinto\n- Corrige mochila sumindo ao morrer no Nether", true], ["3.20.16", "Estável", "há 9 dias", "- Melhoria de desempenho ao abrir a mochila"], ["3.20.11", "Beta", "há 1 mês", "- Teste do novo layout"]];
    return `<aside class="panel panel--strong preview disc__preview" aria-label="Pré-visualização: Sophisticated Backpacks">
      <div class="row row--gap-3">${W.tile("Sophisticated Backpacks", "xl")}<div><div class="t-display-lg">Sophisticated Backpacks</div><div class="t-xs t-3" style="margin-top:4px">por P3pp3rF1y · 48 mi downloads · atualizado hoje</div><div class="row row--wrap" style="margin-top:6px">${W.source(only ? "modrinth" : "both")}${W.side("both")}${W.tag("Licença ARR", "plain", { title: "All Rights Reserved: referenciar no pack é livre" })}</div></div></div>
      <div class="stack-2" style="margin-top:12px">${W.select({ id: "pv-src", label: "Fonte", options: only ? ["Modrinth"] : ["Modrinth (recomendada)", "CurseForge"], hint: only ? null : "O mesmo arquivo existe nas duas. O Modrinth deixa os jogadores baixarem sozinhos." })}
        ${W.select({ id: "pv-ver", label: "Versão", options: ["3.20.17 (mais nova compatível)", "3.20.16", "3.20.11 (beta)"] })}
        ${o.added ? W.alert({ kind: "ok", compact: true, title: "Já no pack.", text: "Adicionado com a Sophisticated Core." }) : B("Adicionar ao pack", "dependencias", { variant: "primary", icon: "plus", block: true })}</div>
      <nav class="anchors" aria-label="Partes desta página" style="margin-top:12px"><a href="#pv-desc" data-anchor>Descrição</a><a href="#pv-gal" data-anchor>Galeria</a><a href="#pv-ver-h" data-anchor>Versões</a><a href="#pv-dep" data-anchor>Dependências</a><a href="#pv-links" data-anchor>Links</a></nav>
      <section id="pv-desc" style="margin-top:12px"><h2 class="field__label">Descrição</h2><p class="t-sm t-2">Mochilas com melhorias, filtros e muito espaço. Dá para pendurar no cinto e usar sem abrir. Cada mochila aceita melhorias: recolher itens do chão, comer sozinho, tocar discos e mais.</p>
        <div class="row" style="margin-top:6px">${W.icon("image", "icon--sm t-3")}<span class="t-xs t-3">Vídeo do YouTube na descrição: ${B("Abrir no navegador", "", { variant: "link", iconEnd: "external-link" })}</span></div></section>
      <section id="pv-gal" style="margin-top:14px"><h2 class="field__label">Galeria</h2>${W.galleryStrip(["Mochilas no inventário", "Melhorias", "Filtro de itens", "Cinto"])}</section>
      <section id="pv-ver-h" style="margin-top:14px"><h2 class="field__label">Versões para Forge 1.20.1</h2><ul class="stack-2" style="margin-top:6px">${vers.map(([v, ch, d, log, open]) => `<li><details class="disclosure"${o.full && open ? " open" : ""}><summary><span class="t-mono">${v}</span> · ${ch} · ${d}</summary><pre class="code">${log}</pre></details></li>`).join("")}</ul></section>
      <section id="pv-dep" style="margin-top:14px"><h2 class="field__label">Dependências</h2><dl class="kv" style="margin-top:6px"><dt>Obrigatória</dt><dd>Sophisticated Core ${W.tag("Será adicionada", "warn")}</dd><dt>Opcional</dt><dd>Sophisticated Backpacks Create Integration</dd><dt>Incompatível</dt><dd class="t-3">nenhuma declarada</dd></dl></section>
      <section id="pv-links" style="margin-top:14px"><h2 class="field__label">Links</h2><div class="row row--wrap t-sm">${B("Problemas conhecidos (issues)", "", { variant: "link", iconEnd: "external-link" })}${B("Código", "", { variant: "link", iconEnd: "external-link" })}${B("Wiki", "", { variant: "link", iconEnd: "external-link" })}${B("Página no Modrinth", "", { variant: "link", iconEnd: "external-link" })}</div></section></aside>`;
  }
  function selbar(n, target) {
    return `<div class="selbar disc__selbar" role="region" aria-label="Itens selecionados"><span class="selbar__count">${n} selecionados</span>${B("Limpar seleção", "", { size: "sm", variant: "ghost" })}<span class="grow"></span>${B(target === "dependencias-modpack" ? `Adicionar ${n} selecionados` : `Adicionar ${n} ao pack`, target, { variant: "primary", icon: "plus" })}</div>`;
  }
  function resultsContent(state, o = {}) {
    const only = o.onlyModrinth || state === "erro";
    const omni = o.link ? { state: "link", value: "https://modrinth.com/mod/sophisticated-backpacks", linkSource: "Modrinth" } : state === "carregando" ? { state: "loading", value: "backpack" } : state === "vazio" ? { state: "search", value: "mochila voadora", statusText: "Nenhum resultado" } : { state: "search", value: "backpack", statusText: only ? "31 resultados do Modrinth para “backpack”" : "48 resultados para “backpack”, do Modrinth e da CurseForge" };
    const notes = [];
    if (o.onlyModrinth) notes.push(W.alert({ kind: "info", title: "Mostrando só o Modrinth.", text: "Para buscar também na CurseForge, informe sua chave em Configurações. Nenhuma busca vai para a CurseForge sem chave.", actions: B("Abrir Configurações", "config-app", { size: "sm" }) }));
    if (state === "erro") notes.push(W.alert({ kind: "warn", title: "A CurseForge não respondeu.", text: "Mostrando só os resultados do Modrinth. O Warden tenta de novo na próxima busca.", actions: B("Tentar de novo", "adicionar-busca", { size: "sm", icon: "refresh-cw" }) }));
    let main, right = "";
    if (state === "carregando") main = `<ul class="dlist" aria-busy="true">${[0, 1, 2, 3, 4].map(() => `<li class="drow" aria-hidden="true"><span class="skeleton" style="width:18px;height:18px"></span><span class="skeleton" style="width:40px;height:40px"></span><span><span class="skeleton skeleton--line" style="--w:45%;height:12px"></span><span class="skeleton skeleton--line" style="--w:80%"></span></span><span></span></li>`).join("")}</ul>`;
    else if (state === "vazio") main = W.empty({ glyph: "search", artKind: "muted", title: "Nada encontrado para “mochila voadora”", text: "Só aparecem mods que funcionam em Minecraft 1.20.1 com Forge. Tente outro nome, em inglês, ou cole o link do mod.", actions: B("Mostrar também os sem versão compatível", "", { variant: "link" }), compact: true });
    else if (o.link) { main = W.alert({ kind: "ok", title: "Link do Modrinth reconhecido.", text: "É o Sophisticated Backpacks, versão 3.20.17 para Minecraft 1.20.1 com Forge. Confira ao lado e adicione." }); right = preview({ onlyModrinth: true }); }
    else {
      const list = RESULTS.filter((r) => !only || r.src !== "curseforge").map((r) => W.discoverRow(Object.assign({}, r, { src: only && r.src === "both" ? "modrinth" : r.src, attrs: go("adicionar-detalhe"), state: o.added && r.state === "selected" ? "inpack" : r.state }))).join("");
      main = `<ul class="dlist" aria-label="Resultados">${list}</ul><p class="t-xs t-3" style="padding:8px 0">Rolar carrega mais 20.</p>${o.added ? "" : selbar(3, "dependencias-varios")}`;
      right = preview({ onlyModrinth: only, added: o.added, full: o.full });
    }
    return head({ added: o.added }) + top({ omni }) + (notes.length ? `<div class="stack-2" style="margin-top:12px">${notes.join("")}</div>` : "") + `<div class="disc">${filters({ cat: "Armazenamento" })}<div class="disc__main">${main}</div>${right}</div>`;
  }
  def("adicionar-busca", { group: "pack", title: "Adicionar: resultados com seleção", spec: "T08", d4: true, render: (s) => shell(resultsContent(s)), states: { carregando: "Buscando", vazio: "Nenhum resultado", erro: "CurseForge fora do ar" } });
  def("adicionar-detalhe", { group: "pack", title: "Adicionar: pré-visualização completa", spec: "T07, T08", d4: true, hidden: true, render: () => shell(resultsContent("normal", { full: true })),
    after: () => { const p = document.querySelector(".disc__preview"); const t = document.getElementById("pv-ver-h"); if (p && t) p.scrollTop = t.offsetTop - p.offsetTop - 8; } });
  def("adicionar-link", { group: "pack", title: "Adicionar: link colado", spec: "T08", hidden: true, render: () => shell(resultsContent("normal", { link: true })) });
  def("adicionar-semchave", { group: "pack", title: "Adicionar sem chave da CurseForge", spec: "T08", hidden: true, render: () => shell(resultsContent("normal", { onlyModrinth: true })) });
  def("adicionado", { group: "pack", title: "Adicionados: a busca continua aberta", spec: "T08, T09", hidden: true, render: () => shell(resultsContent("normal", { added: 3 }), { unsaved: 9 }),
    after: () => window.WardenUI.toast(W.toast({ kind: "ok", title: "4 itens adicionados ao pack", text: "Sophisticated Backpacks, Sophisticated Storage, Packed Up e Sophisticated Core.", actions: B("Desfazer", "adicionar-busca", { size: "sm", variant: "ghost" }) }), 9000) });

  // ---------- Dependências: um diálogo só para o conjunto ----------
  function depsDialog(o = {}) {
    const chosen = o.chosen || [["Sophisticated Backpacks", "3.20.17"]];
    const n = chosen.length + (o.required || [["Sophisticated Core", "0.6.26"]]).length;
    return W.dialog({ esc: o.esc || "adicionar-busca", title: o.title || "Adicionar Sophisticated Backpacks", sub: "Nada foi gravado ainda. Confira o que vai entrar no pack.", body: `
      <div><div class="t-caps t-3" style="margin-bottom:6px">O que você escolheu</div><div class="stack-2">${chosen.map(([m, v]) => W.check({ checked: true, label: `<b>${m}</b> <span class='t-mono t-sm'>${v}</span>` })).join("")}</div></div>
      <div><div class="t-caps t-3" style="margin-bottom:6px">Obrigatórias</div>${(o.required || [["Sophisticated Core", "0.6.26"]]).map(([m, v]) => W.check({ checked: true, disabled: true, label: `<b>${m}</b> <span class='t-mono t-sm'>${v}</span>`, desc: "Sem ela o jogo não abre. Não dá para desmarcar." })).join("")}</div>
      ${o.optional === false ? "" : `<div><div class="t-caps t-3" style="margin-bottom:6px">Opcionais</div>${W.check({ label: "Sophisticated Backpacks Create Integration", desc: "Mochilas que se ligam às máquinas do Create." })}</div>`}
      ${o.more || ""}
      ${W.alert({ kind: "ok", compact: true, title: "Nenhuma incompatibilidade declarada", text: "com os 128 itens que já estão no pack." })}`,
      foot: B("Cancelar", o.esc || "adicionar-busca", { variant: "ghost" }) + B(`Adicionar ${n} itens`, o.done || "adicionado", { variant: "primary", icon: "plus" }) });
  }
  window.P.depsDialog = depsDialog;
  def("dependencias", { group: "pack", title: "Dependências ao adicionar (diálogo)", spec: "T09", render: () => shell(resultsContent("normal"), { overlay: depsDialog() }) });
  def("dependencias-varios", { group: "pack", title: "Dependências de vários mods de uma vez", spec: "T09", d4: true, hidden: true, render: () => shell(resultsContent("normal"), { overlay: depsDialog({ title: "Adicionar 3 mods", chosen: [["Sophisticated Backpacks", "3.20.17"], ["Sophisticated Storage", "0.10.41"], ["Packed Up", "1.0.30"]] }) }) });
  def("kit", { group: "pack", title: "Kit de desempenho (diálogo)", spec: "T08, T09", d4: true, hidden: true, render: () => shell(startContent(), { overlay: depsDialog({ esc: "adicionar", title: "Adicionar o kit Desempenho (Forge 1.20.1)", chosen: [["FerriteCore", "6.0.1"], ["ModernFix", "5.27.85"], ["Clumps", "12.0.0.4"], ["ImmediatelyFast", "1.2.18"]], required: [], optional: false, more: `<div><div class="t-caps t-3" style="margin-bottom:6px">Já no pack</div><p class="t-sm">Embeddium 0.3.31 ${W.tag("Já no pack", "primary", { icon: "check" })}</p></div>`, done: "adicionado" }) }) });

  // ---------- Navegar por modpacks ----------
  function modpacksContent(o = {}) {
    const omni = { state: "search", value: "create", statusText: "37 modpacks para Forge 1.20.1 com “create”" };
    const list = D.MODPACKS.map(([name, author, desc, dl, upd, src, mods], i) => W.discoverRow({ name, author, desc, src, nobox: true, meta: `<span>${dl} downloads</span><span>${mods}</span><span>atualizado ${upd}</span>`, current: o.open && i === 0, attrs: go("modpack"), end: B("Ver os mods", i === 0 ? "modpack" : "", { size: "sm", attrs: { "aria-label": `Ver os mods de ${name}` } }) })).join("");
    let right = "";
    if (o.open) {
      const rows = D.MODPACK_MODS.map(([name, desc, st, src, why]) => W.discoverRow({ name, desc, src, state: st, why, compact: true })).join("");
      right = `<aside class="panel panel--strong preview disc__preview" aria-label="Mods do modpack Cozy Create">
        <div class="row row--gap-3">${W.tile("Cozy Create", "xl")}<div><div class="t-display-lg">Cozy Create</div><div class="t-xs t-3" style="margin-top:4px">por harborlight · versão 2.3.0 · Forge 1.20.1 · 1,2 mi downloads</div></div></div>
        ${W.alert({ kind: "info", compact: true, icon: "scroll-text", title: "Licença do modpack: All Rights Reserved.", text: "Trazer mods é livre: eles vêm das lojas. As configs do modpack são trabalho do autor e não são copiadas." })}
        <h2 class="field__label" style="margin-top:12px">Mods deste modpack (168)</h2>
        <p class="t-xs t-2" style="margin:2px 0 8px">41 já no seu pack · 112 compatíveis · 9 sem versão para o seu pack · 6 arquivos fora das lojas</p>
        <div class="row row--wrap" style="margin-bottom:8px">${W.check({ label: "Só os que não estão no seu pack", checked: false })}${W.select({ bare: true, size: "sm", ariaLabel: "Versão dos mods", options: ["Versão: a mesma do modpack (testada junto)", "Versão: a mais nova compatível"] })}</div>
        <ul class="dlist" aria-label="Mods do modpack">${rows}</ul><p class="t-xs t-3" style="padding:6px 0">… e mais 158 mods.</p>${selbar(5, "dependencias-modpack")}</aside>`;
    }
    return head() + top({ omni, type: "mp" }) + `<div class="disc ${o.open ? "disc--modpack" : "disc--start"}">${o.open ? "" : filters({ type: "mp" })}<div class="disc__main"><ul class="dlist" aria-label="Modpacks">${list}</ul>${o.open ? `<p class="t-xs t-3" style="margin-top:6px">Os filtros voltam quando você fecha o modpack. ${B("Fechar o modpack", "adicionar-modpacks", { variant: "link" })}</p>` : ""}</div>${right}</div>`;
  }
  def("adicionar-modpacks", { group: "pack", title: "Adicionar: Tipo Modpacks", spec: "T08", d4: true, render: () => shell(modpacksContent()) });
  def("modpack", { group: "pack", title: "Modpack aberto: escolher mods dele", spec: "T08", d4: true, render: () => shell(modpacksContent({ open: true })) });
  def("dependencias-modpack", { group: "pack", title: "Dependências dos mods do modpack", spec: "T09", d4: true, hidden: true, render: () => shell(modpacksContent({ open: true }), { overlay: depsDialog({ esc: "modpack", title: "Adicionar 5 mods do Cozy Create", chosen: [["Create Steam 'n' Rails", "1.6.4 (a mesma do modpack)"], ["Create: Connected", "0.8.2 (a mesma do modpack)"], ["Storage Drawers", "12.0.3"], ["Handcrafted", "3.0.6"], ["Comforts", "6.4.0"]], required: [["Architectury API", "9.2.14"]], optional: false }) }) });

  // Índice da pré-visualização: rola até o título, sem mudar a rota do protótipo
  document.addEventListener("click", (e) => {
    const a = e.target.closest && e.target.closest("[data-anchor]");
    if (!a) return;
    e.preventDefault();
    const t = document.getElementById(a.getAttribute("href").slice(1));
    const box = a.closest(".disc__preview");
    if (t && box) { box.scrollTop = t.offsetTop - box.offsetTop - 8; t.setAttribute("tabindex", "-1"); t.focus({ preventScroll: true }); }
  });
  // Seletor Tipo: Modpacks leva à lista de modpacks; Mods volta para os resultados
  document.addEventListener("change", (e) => { if (e.target.matches && e.target.matches("[data-type-select]")) window.P.navigate(e.target.value === "mp" ? "adicionar-modpacks" : "adicionar-busca"); });
})();
