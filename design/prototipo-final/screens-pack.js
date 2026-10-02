/* Telas do nível do PACK: as 6 seções do menu e os diálogos que abrem a partir delas. */
(function () {
  "use strict";
  const W = window.W, D = window.DATA;
  const { def, go, B, packShell, pageHead } = window.P;
  const AI = W.icon("sparkles", "icon--ai");

  // ======================= MODS =======================
  const toRow = (m, extra = {}) => ({ name: m[0], desc: m[1], ver: m[2], src: m[3], side: m[4], update: m[5], updateTo: m[6], kind: m[7], flags: (m[8] || []).map((f) => (f === "Erro" ? W.badge("danger", "Erro") : f === "Aviso" ? W.badge("warn", "Aviso") : f === "Não salvo" ? W.tag("Não salvo", "warn") : f === "Download manual" ? W.tag("Download manual", "warn", { icon: "download" }) : f)), ...extra });
  function group(id, title, n, rows, o = {}) {
    return `<section class="listgroup" aria-labelledby="${id}-h"><div class="listgroup__head"><button type="button" class="listgroup__toggle" id="${id}-h" aria-expanded="true" aria-controls="${id}-b">${W.icon("chevron-down")}${title}</button><span class="t-3 t-sm">${n}</span></div>
      <div id="${id}-b"><div class="tablewrap"><table class="table">${W.modTableHead()}<tbody>${rows}${o.more ? `<tr class="more-row"><td></td><td colspan="4">${o.more}</td></tr>` : ""}</tbody></table></div></div></section>`;
  }
  // "Ver como: Lista · Grafo": modo de exibição dos mesmos dados (não é aba)
  function viewAs(v) {
    return `<span class="row"><span class="t-xs t-3" id="viewas-l">Ver como</span><span data-viewas>${W.segmented([["list", "Lista"], ["graph", "Grafo"]], v, "Ver como")}</span></span>`;
  }
  function modsContent(state, o = {}) {
    const actions = B("Verificar atualizações", "", { icon: "refresh-cw" }) + B("Adicionar", "adicionar", { variant: "primary", icon: "plus" });
    if (state === "vazio") {
      return pageHead("Mods", "Mods, resource packs e shaders do pack.", B("Adicionar", "adicionar", { variant: "primary", icon: "plus" })) +
        W.empty({ title: "Nenhum mod ainda", text: "O pack Vale Sereno foi criado para Minecraft 1.20.1 com Forge 47.3.0. Comece pelos mods; resource packs e shaders entram pelo mesmo botão.", actions: B("Adicionar mods", "adicionar", { variant: "primary", icon: "plus" }), hint: "Ou arraste arquivos .jar e .zip do computador para cá.", glyph: "plus" });
    }
    const head = pageHead("Mods", `128 itens: 124 mods, 3 resource packs e 1 shader. <span class="dropnote">${W.icon("download", "icon--sm")}Arraste arquivos .jar ou .zip para cá para adicionar do computador.</span>`, actions);
    const tools = `<div class="toolbar">${viewAs("list")}${W.input({ bare: true, icon: "search", placeholder: "Buscar no pack", ariaLabel: "Buscar no pack" })}${W.select({ bare: true, ariaLabel: "Fonte", options: ["Fonte: todas", "Modrinth", "CurseForge", "Arquivo local"] })}${W.select({ bare: true, ariaLabel: "Lado", options: ["Lado: todos", "Cliente e servidor", "Só cliente", "Só servidor"] })}${W.select({ bare: true, ariaLabel: "Mostrar", options: ["Mostrar: tudo", "Com problemas (6)", "Com atualização (5)", "Não salvos (2)"] })}</div>`;
    if (state === "carregando") return head + tools + `<div class="tablewrap"><table class="table" aria-busy="true">${W.modTableHead()}<tbody>${W.skeletonRows(8, 5)}</tbody></table></div>`;
    const banner = `<div class="mods-banner">${W.alert({ kind: "neutral", compact: true, icon: "refresh-cw", title: "5 atualizações disponíveis.", text: "Nenhuma é aplicada sem você revisar antes.", actions: B("Revisar e atualizar", "atualizar", { size: "sm" }) })}</div>`;
    const sel = `<div class="selbar" style="margin-bottom:12px" role="region" aria-label="Ações para os selecionados"><span class="selbar__count">1 selecionado</span>${B("Alterar lado", "", { size: "sm" })}${B("Atualizar", "", { size: "sm", icon: "refresh-cw" })}${B("Remover", "", { size: "sm", variant: "danger-ghost", icon: "trash-2" })}<span class="grow"></span>${B("Limpar seleção", "", { size: "sm", variant: "ghost" })}</div>`;
    let mods = D.MODS.map((m) => toRow(m, { selected: m[0] === "Embeddium", attrs: go(m[0].startsWith("Just") ? "mods-detalhe" : m[0] === "Epic Fight" || m[0] === "Supplementaries" ? "mods-raio-x" : m[0] === "Waystones" || m[0] === "Rubidium" ? "problemas" : m[0] === "Create" ? "mods-grafo" : "") }));
    if (state === "erro") {
      mods = [{ kind: "invalid", file: "mods/sodium-extra.pw.toml", name: "sodium-extra.pw.toml", desc: "Não foi possível ler: falta fechar as aspas na linha 3. Os outros itens não são afetados.", update: "na", flags: [W.badge("danger", "Arquivo inválido")] },
        { name: "kotlinforforge-4.11.0-all.jar", desc: "Está na pasta mods/, mas fora do índice do pack. Não vai para quem joga.", ver: "4.11.0", src: "local", side: "both", update: "na", flags: [W.badge("warn", "Fora do índice")] }, ...mods];
    }
    const err = state === "erro" ? `<div class="mods-banner">${W.alert({ kind: "danger", title: "1 arquivo do pack não pôde ser lido.", text: "O item aparece na lista com o erro. O resto do pack continua funcionando.", actions: B("Ver o arquivo", "", { size: "sm" }) })}</div>` : "";
    return head + (o.top || "") + err + banner + tools + sel +
      group("g-mods", "Mods", D.MODS_TOTAL, mods.map((m) => W.modRow(m)).join(""), { more: `… e mais ${D.MODS_TOTAL - D.MODS.length} mods. ${B("Mostrar todos", "", { variant: "link" })}` }) +
      group("g-rp", "Resource packs", 3, D.RESOURCEPACKS.map((m) => W.modRow(toRow(m))).join("")) +
      group("g-sh", "Shaders", 1, D.SHADERS.map((m) => W.modRow(toRow(m))).join(""));
  }
  window.P.modsContent = modsContent;
  const viewAsAfter = (to) => () => document.querySelectorAll("[data-viewas] [role=radio]").forEach((b) => { if (b.getAttribute("aria-checked") !== "true") b.setAttribute("data-go", to); });
  def("mods", { group: "pack", title: "Mods, resource packs e shaders", spec: "T05, T06, T10", render: (s) => packShell("mods", modsContent(s), s === "vazio" ? { unsaved: 0, problems: 0, empty: true } : {}), states: { carregando: "Lendo o índice do pack", vazio: "Pack sem mods", erro: "Um arquivo inválido e um fora do índice" }, after: viewAsAfter("mods-grafo") });
  def("pack-novo", { group: "pack", title: "Pack recém-criado (vazio)", spec: "T03, T06", hidden: true, render: () => packShell("mods", modsContent("vazio"), { unsaved: 0, problems: 0, empty: true, version: "0.1.0" }) });
  def("pack-revisar", { group: "pack", title: "Pack com mudanças do teste para revisar", spec: "T05, T15", hidden: true, render: () => packShell("mods", modsContent("normal"), { unsaved: 5, alerts: [{ kind: "warn", text: "Mudanças do teste para revisar", attrs: go("teste-fechou") }] }) });

  function detailDrawer() {
    return W.drawer({ esc: "mods", title: "Detalhes do mod", body: `<div class="row row--gap-3">${W.tile("Just Enough Items (JEI)", "xl")}<div><div class="t-display-lg">Just Enough Items</div><div class="t-xs t-3 row" style="margin-top:4px">por mezz · ${W.source("curseforge")} ${B("Abrir página", "", { variant: "link", iconEnd: "external-link" })}</div></div></div>
      <p class="t-sm t-2">Mostra todos os itens e receitas do jogo.</p>
      ${W.alert({ kind: "info", icon: "refresh-cw", compact: true, title: "Atualização disponível: 15.20.0.112", text: "Corrige um travamento ao abrir receitas do Create." })}
      <dl class="kv"><dt>Versão instalada</dt><dd class="t-mono">15.20.0.106</dd><dt>Para</dt><dd>Minecraft 1.20.1 · Forge</dd><dt>Publicada em</dt><dd>12/03/2025</dd><dt>Arquivo</dt><dd class="path">jei-1.20.1-forge-15.20.0.106.jar · 1,4 MB</dd></dl>
      ${W.select({ id: "dt-side", label: "Lado", options: [["both", "Cliente e servidor"], ["client", "Só cliente"], ["server", "Só servidor"]], value: "both", hint: "Onde o mod precisa estar instalado. Informado pela CurseForge." })}
      <dl class="kv"><dt>Depende de</dt><dd class="t-3">Nenhum mod (só o Forge)</dd><dt>Usado por</dt><dd>${B("Just Enough Resources", "", { variant: "link" })} <span class="t-3">· obrigatória</span></dd><dt>Por que está no pack</dt><dd>Você adicionou em 12/08/2026</dd></dl>
      <div><div class="field__label">O que este mod altera no jogo</div><p class="t-sm t-2">Nada por mixin: o JEI usa só as APIs do Forge.</p></div>
      <details class="disclosure"><summary>Novidades da versão 15.20.0.112</summary><pre class="code" style="margin-top:6px">- Corrige travamento ao abrir receitas do Create
- Melhora a busca por nome de mod (@create)</pre></details>
      <details class="disclosure"><summary>Mais opções</summary><div class="stack-2" style="margin-top:8px">${B("Trocar de versão…", "", { size: "sm" })}${W.switchCtl({ label: "Fixar versão (não atualizar)" })}${W.switchCtl({ label: "Opcional para o jogador", disabled: true })} ${W.badge("p1", "P1")}</div></details>`,
      foot: B("Atualizar para 15.20.0.112", "atualizar", { variant: "primary", icon: "refresh-cw" }) + B("Remover", "remover", { variant: "danger-ghost", icon: "trash-2" }) });
  }
  def("mods-detalhe", { group: "pack", title: "Detalhes de um mod (painel lateral)", spec: "T07, T11", render: () => packShell("mods", modsContent("normal"), { overlay: detailDrawer() }) });


  // ---------- Raio-x de mixins (D4): detalhes do Epic Fight ----------
  const OVERLAPS = [
    { target: "LivingEntity#travel (movimento das criaturas)", cls: "net.minecraft.world.entity.LivingEntity", who: [["Epic Fight", "@Redirect"], ["Supplementaries", "@Redirect"]], risk: "high", why: "Os dois trocam a mesma chamada dentro do método. Só um vale; o outro é pulado em silêncio. O travamento de 30/09 citou epicfight.mixins.json." },
    { target: "PlayerRenderer#render (desenho do jogador)", cls: "net.minecraft.client.renderer.entity.player.PlayerRenderer", who: [["Epic Fight", "@Inject no começo"], ["Embeddium", "@Inject no fim"]], risk: "low", why: "Pontos diferentes do mesmo método.", lower: "Feitos para conviver" },
  ];
  function xrayDrawer(all) {
    const back = `<div class="back-link">${B("Detalhes do mod", "mods-raio-x", { variant: "ghost", size: "sm", icon: "arrow-left" })}</div>`;
    if (all) {
      const rows = [["LivingEntity#travel", "@Redirect", "Também Supplementaries", "high"], ["LivingEntity#hurt", "@Inject (começo)", "", "low"], ["LivingEntity#die", "@Inject (fim)", "", "low"], ["PlayerRenderer#render", "@Inject (começo)", "Também Embeddium", "low"], ["Player#attack", "@Overwrite", "", "low"], ["ItemInHandRenderer#renderArmWithItem", "@WrapOperation", "", "low"], ["Mob#doHurtTarget", "@Inject (começo)", "", "low"], ["Camera#setup", "@ModifyArgs", "", "low"]];
      return W.drawer({ esc: "mods", title: "Todas as alterações do Epic Fight", body: `${back}<p class="t-sm t-2">214 alterações em 3 configs de mixin (epicfight.mixins.json, epicfight.client.mixins.json, epicfight.compat.mixins.json). Ordem prevista: prioridade 1000, igual à da maioria dos mods.</p>
        <div class="row">${W.input({ bare: true, size: "sm", icon: "search", placeholder: "Filtrar por parte do jogo", ariaLabel: "Filtrar alterações" })}${W.check({ label: "Só as que outros mods também alteram", checked: false })}</div>
        <div class="tablewrap"><table class="table"><thead><tr><th>Parte do jogo</th><th>Tipo</th><th>Outros mods</th></tr></thead><tbody>${rows.map(([t, k, o, r]) => `<tr><td class="t-mono" style="font-size:var(--text-xs);word-break:break-all">${t}</td><td class="t-mono t-2" style="font-size:var(--text-xs)">${k}</td><td>${o ? (r === "high" ? W.badge("danger", "Alto") + " " : "") + `<span class="t-xs">${o}</span>` : '<span class="t-3 t-xs">nenhum</span>'}</td></tr>`).join("")}<tr class="more-row"><td colspan="3">… e mais 206 alterações</td></tr></tbody></table></div>
        <details class="disclosure"><summary>Como ler os tipos</summary><p class="t-sm" style="margin-top:6px">@Inject acrescenta código num ponto e convive bem com outros. @Redirect e @Overwrite trocam código: quando dois mods fazem isso no mesmo ponto, só um vale.</p></details>` });
    }
    return W.drawer({ esc: "mods", title: "Detalhes do mod", body: `<div class="row row--gap-3">${W.tile("Epic Fight", "xl")}<div><div class="t-display-lg">Epic Fight</div><div class="t-xs t-3 row" style="margin-top:4px">por Yesman · ${W.source("curseforge")} ${B("Abrir página", "", { variant: "link", iconEnd: "external-link" })}</div></div></div>
      <p class="t-sm t-2">Combate com animações e golpes novos.</p>
      <dl class="kv"><dt>Versão instalada</dt><dd class="t-mono">20.9.4</dd><dt>Depende de</dt><dd>Forge 47.1 ou mais novo</dd><dt>Usado por</dt><dd class="t-3">nenhum mod do pack</dd><dt>Por que está no pack</dt><dd>Você adicionou em 02/09/2026</dd></dl>
      <section aria-labelledby="xr-h"><h3 class="field__label" id="xr-h">O que este mod altera no jogo</h3><p class="t-sm" style="margin:4px 0 8px">214 partes do jogo, em 3 configs. <b>2 também alteradas por outros mods.</b></p>
        <ul class="mixins">${OVERLAPS.map(W.mixinRow).join("")}</ul>
        <div class="row row--wrap" style="margin-top:8px">${B("Ver todas as alterações", "mods-raio-x~todas", { size: "sm", icon: "list" })}${B("Ver em Problemas", "problemas", { size: "sm", variant: "ghost" })}</div>
        <p class="t-xs t-3" style="margin-top:8px">“Alteram o mesmo ponto” não quer dizer que não funcionam juntos: muitas vezes é de propósito. Este resumo é lido dos arquivos dos mods, sem abrir o jogo.</p></section>`,
      foot: B("Encontrar o mod culpado…", "culpado-config", { icon: "target" }) + B("Remover", "", { variant: "danger-ghost", icon: "trash-2" }) });
  }
  def("mods-raio-x", { group: "pack", title: "Detalhes: o que o mod altera no jogo (raio-x)", spec: "T07", d4: true, render: (s) => packShell("mods", modsContent("normal"), { overlay: xrayDrawer(s === "todas") }), states: { todas: "Ver todas as alterações" } });

  // ---------- Grafo de dependências (D4) ----------
  function graphContent() {
    const head = pageHead("Mods", "128 itens: 124 mods, 3 resource packs e 1 shader.", B("Verificar atualizações", "", { icon: "refresh-cw" }) + B("Adicionar", "adicionar", { variant: "primary", icon: "plus" }));
    const tools = `<div class="toolbar">${viewAs("graph")}<label class="t-xs t-3" for="gr-center">Mod no centro</label>${W.select({ bare: true, id: "gr-center", options: ["Create", "Sophisticated Backpacks", "Epic Fight", "Supplementaries", "Waystones"], value: "Create" })}${W.select({ bare: true, ariaLabel: "Profundidade", options: ["1 nível de cada lado", "2 níveis de cada lado"] })}${W.check({ label: "Mostrar mods que alteram o mesmo ponto do jogo" })}</div>`;
    const graph = W.depGraph({
      center: { name: "Create", sub: "0.5.1.j · Modrinth · você adicionou", inside: "Flywheel 0.6.11 e Registrate 1.3.3" },
      left: [{ name: "Forge", sub: "47.1 ou mais novo", kind: "required" }, { name: "Just Enough Items (JEI)", sub: "no pack", kind: "optional", attrs: go("mods-detalhe") }],
      right: [{ name: "Create Slice & Dice", sub: "no pack", kind: "required" }, { name: "Create Crafts & Additions", sub: "no pack", kind: "required" }, { name: "Sophisticated Backpacks Create Integration", sub: "não está no pack", kind: "optional" }],
    });
    const text = `<section class="panel" aria-labelledby="gr-txt"><h2 class="panel__title panel__title--sans" id="gr-txt">Em palavras</h2><ul class="stack-2 t-sm" style="margin-top:8px">
      <li><b>Create precisa de:</b> Forge 47.1 ou mais novo. Usa o JEI se ele estiver no pack, mas não precisa. Flywheel e Registrate vêm dentro do próprio Create.</li>
      <li><b>Precisam do Create:</b> Create Slice & Dice e Create Crafts & Additions. ${W.tag("Se você remover o Create, 2 mods param de funcionar", "warn", { icon: "triangle-alert" })}</li>
      <li><b>Por que está no pack:</b> você adicionou em 18/08/2026.</li></ul></section>`;
    const orphan = W.alert({ kind: "info", compact: true, icon: "box", title: "1 biblioteca sem uso: Kotlin for Forge.", text: "Nenhum mod do pack precisa mais dela (sobrou de um mod removido). Pode ser removida.", actions: B("Ver Kotlin for Forge", "", { size: "sm" }) });
    return head + tools + `<div class="stack">${graph}${text}${orphan}<p class="t-xs t-3">Clique num mod para ele ir para o centro. O grafo inteiro, com todos os 124 mods, fica em ${B("Ver o pack inteiro", "", { variant: "link" })}.</p></div>`;
  }
  def("mods-grafo", { group: "pack", title: "Mods: ver como grafo", spec: "T06", d4: true, render: () => packShell("mods", graphContent()), after: viewAsAfter("mods") });

  function updateDialog() {
    const r = [["Just Enough Items (JEI)", "15.20.0.106", "15.20.0.112"], ["Xaero's Minimap", "24.6.1", "24.6.2"], ["Create", "0.5.1.j", "0.5.1.k"], ["Farmer's Delight", "1.2.6", "1.2.7"], ["Supplementaries", "2.8.17", "2.8.21"]];
    return W.dialog({ esc: "mods", size: "lg", title: "Atualizar 5 itens", sub: "Confira antes de aplicar. Nada muda até você confirmar.", body: `<div class="tablewrap"><table class="table table--plain"><thead><tr><th class="shrink"><span class="sr-only">Incluir</span></th><th>Item</th><th>Agora</th><th>Nova</th><th>Novidades</th></tr></thead><tbody>${r.map((x) => `<tr><td>${W.check({ checked: true, ariaLabel: "Atualizar " + x[0] })}</td><td class="t-strong">${x[0]}</td><td class="t-mono t-2">${x[1]}</td><td class="t-mono t-primary">${x[2]}</td><td>${B("Ver novidades", "", { variant: "link" })}</td></tr>`).join("")}</tbody></table></div>
      <ul class="checklist"><li class="is-ok">${W.icon("circle-check")}<span>Nenhuma dependência nova.</span></li><li class="is-ok">${W.icon("circle-check")}<span>Nenhuma incompatibilidade nova com o que já está no pack.</span></li></ul>
      <p class="t-sm t-3">Antes de aplicar, o Warden guarda um ponto de segurança. Depois, verifica o pack de novo.</p>`, foot: B("Cancelar", "mods", { variant: "ghost" }) + B("Atualizar 5 itens", "mods", { variant: "primary", icon: "refresh-cw" }) });
  }
  def("atualizar", { group: "pack", title: "Atualizar itens (diálogo)", spec: "T10", hidden: true, render: () => packShell("mods", modsContent("normal"), { overlay: updateDialog() }) });
  function removeDialog() {
    return W.dialog({ esc: "mods-detalhe", size: "sm", alert: true, title: "Remover Just Enough Items (JEI)?", body: `<p>Este mod depende dele: <b>Just Enough Resources</b>. Sem o JEI, ele pode não funcionar.</p><p class="t-sm t-3">O item sai do pack. Dá para voltar atrás pelo Histórico.</p>`, foot: B("Manter no pack", "mods-detalhe", { variant: "ghost" }) + B("Remover JEI", "mods", { variant: "danger", icon: "trash-2" }) });
  }
  def("remover", { group: "pack", title: "Remover item (confirmação)", spec: "T06", hidden: true, render: () => packShell("mods", modsContent("normal"), { overlay: removeDialog() }) });

  // ======================= CONFIGS =======================
  function highlight(line) {
    const e = W.esc(line);
    if (/^\s*#/.test(line)) return `<span class="tk-c">${e}</span>`;
    if (/^\s*\[/.test(line)) return `<span class="tk-s">${e}</span>`;
    const m = e.match(/^(\s*)([\w.]+)(\s*=\s*)(.*)$/);
    if (m) return `${m[1]}<span class="tk-k">${m[2]}</span>${m[3]}<span class="${/^(true|false)$/.test(m[4]) ? "tk-v" : "tk-n"}">${m[4]}</span>`;
    return e;
  }
  function configsContent(state, o = {}) {
    const inst = o.instance;
    const tree = inst
      ? `<button type="button" role="treeitem" class="tree__item" aria-expanded="true">${W.icon("folder-open")}config/</button><div role="group"><button type="button" role="treeitem" class="tree__item tree__item--dirty" aria-selected="true">${W.icon("file-text")}embeddium-options.json</button><button type="button" role="treeitem" class="tree__item">${W.icon("file-text")}create-client.toml</button><button type="button" role="treeitem" class="tree__item" aria-expanded="false">${W.icon("folder")}jei/</button><div role="group" hidden></div></div>
         <button type="button" role="treeitem" class="tree__item tree__item--friendly">${W.icon("file-text")}Opções do jogo (options.txt)</button><button type="button" role="treeitem" class="tree__item" aria-expanded="true">${W.icon("folder-open")}saves/Mundo de teste/serverconfig/</button><div role="group"><button type="button" role="treeitem" class="tree__item">${W.icon("file-text")}create-server.toml</button></div>`
      : `<button type="button" role="treeitem" class="tree__item" aria-expanded="true">${W.icon("folder-open")}config/ <span class="t-3">(86)</span></button><div role="group"><button type="button" role="treeitem" class="tree__item ${o.form || o.script ? "" : "tree__item--dirty"}"${o.script ? "" : ' aria-selected="true"'}>${W.icon("file-text")}create-common.toml</button><button type="button" role="treeitem" class="tree__item">${W.icon("file-text")}embeddium-options.json</button><button type="button" role="treeitem" class="tree__item">${W.icon("file-text")}waystones-common.toml</button><button type="button" role="treeitem" class="tree__item">${W.icon("file-text")}xaerominimap.txt</button><button type="button" role="treeitem" class="tree__item" aria-expanded="false">${W.icon("folder")}jei/</button><div role="group" hidden></div><span class="tree__item t-3" style="cursor:default">… mais 81 arquivos</span></div>
         <button type="button" role="treeitem" class="tree__item" aria-expanded="false">${W.icon("folder")}defaultconfigs/ <span class="t-3">(2)</span></button><div role="group" hidden></div>${o.script ? `<button type="button" role="treeitem" class="tree__item" aria-expanded="true">${W.icon("folder-open")}kubejs/server_scripts/</button><div role="group"><button type="button" role="treeitem" class="tree__item tree__item--dirty" aria-selected="true">${W.icon("braces")}receitas.js</button><button type="button" role="treeitem" class="tree__item">${W.icon("braces")}tags.js</button></div>` : `<button type="button" role="treeitem" class="tree__item" aria-expanded="false" data-go="scripts">${W.icon("folder")}kubejs/</button><div role="group" hidden></div>`}<button type="button" role="treeitem" class="tree__item tree__item--friendly">${W.icon("file-text")}Opções do jogo (options.txt)</button>`;
    const head = pageHead("Configs", "Arquivos de ajuste e scripts do pack. Comentários e formatação são preservados.");
    if (state === "vazio") return head + W.empty({ glyph: "dots", title: "Este pack ainda não tem configs", text: "Os mods criam os arquivos de config na primeira vez que o jogo abre. Teste o pack uma vez e eles aparecem aqui.", actions: B("Testar", "teste-checagem", { variant: "primary", icon: "play" }) });
    const instBanner = inst ? `<div style="margin-bottom:12px">${W.alert({ kind: "warn", title: "Você está editando a instância de teste, não o pack.", text: "Vale só para este teste. Quando o jogo fechar, a mudança aparece em “O que mudou durante o teste” e você decide se traz para o pack.", actions: B("Voltar ao teste", "teste-jogo", { size: "sm", icon: "terminal" }) })}</div>` : "";
    const ext = state === "erro" ? `<div style="margin-bottom:12px">${W.alert({ kind: "danger", title: "Este arquivo foi alterado fora do Warden.", text: "Outro programa mudou config/create-common.toml depois que você abriu. Escolha o que fazer antes de salvar.", actions: B("Recarregar", "configs", { size: "sm" }) + B("Ver diferenças", "configs-diff", { size: "sm" }) + B("Sobrescrever", "", { size: "sm", variant: "danger-ghost" }) })}</div>` : "";
    const file = inst ? "config/embeddium-options.json" : "config/create-common.toml";
    const lines = inst ? ["{", '  "quality": {', '    "weather_quality": "FANCY",', '    "leaves_quality": "FAST",', '    "enable_vignette": false', "  },", '  "performance": {', '    "chunk_builder_threads": 4', "  }", "}"] : D.CONFIG_FILE;
    const changed = inst ? 7 : 9;
    let editorBody;
    if (state === "carregando") editorBody = `<div class="editor__lines" aria-busy="true" style="padding:16px">${[60, 40, 75, 30, 55, 68, 20, 50].map((w) => `<span class="skeleton skeleton--line" style="--w:${w}%"></span>`).join("")}</div>`;
    else if (o.form) {
      const bad = state === "erro";
      editorBody = `<div class="editor__lines" style="font-family:var(--font-ui)"><div class="cfgform"><div class="cfgform__group">[worldgen]</div>
        ${W.cfgRow({ label: "Desligar a geração de minérios", key: "disableWorldGen", desc: "Desliga a geração de minérios do Create.", def: "false", defSource: "do comentário do arquivo", control: W.switchCtl({ label: "disableWorldGen", checked: false }).replace("<label", '<label class="sr-only"') })}
        <div class="cfgform__group">[kinetics]</div>
        ${W.cfgRow({ label: "Velocidade máxima de rotação", key: "maxRotationSpeed", desc: "Faixa: acima de 64.", def: "256", defSource: "do comentário do arquivo", changed: true, control: W.input({ bare: true, value: "512", size: "sm", ariaLabel: "Velocidade máxima de rotação" }) })}
        ${W.cfgRow({ label: "Multiplicador de stress", key: "stressMultiplier", desc: "Faixa: 0,0 a 10,0.", def: "1.0", defSource: "do comentário do arquivo", changed: bad, error: bad ? "12.0 está fora da faixa 0,0 a 10,0. O Forge troca por 1,0 sem avisar." : null, errId: "cf-err-stress", control: W.input({ bare: true, value: bad ? "12.0" : "1.0", size: "sm", ariaLabel: "Multiplicador de stress", attrs: bad ? { "aria-invalid": "true", "aria-describedby": "cf-err-stress" } : {} }) })}
        ${W.cfgRow({ label: "Alcance do ventilador", key: "fanPushDistance", desc: "Até quantos blocos o ventilador empurra. Defaults to 20.", def: "20", defSource: "tirado do texto", inferred: true, control: W.input({ bare: true, value: "20", size: "sm", ariaLabel: "Alcance do ventilador" }) })}
        <div class="cfgform__group">[fluids]</div>
        ${W.cfgRow({ label: "Alcance da bomba", key: "mechanicalPumpRange", desc: "Quantos blocos o cano leva o líquido. Faixa: 1 a 256.", def: "16", defSource: "do comentário do arquivo", control: W.input({ bare: true, value: "16", size: "sm", ariaLabel: "Alcance da bomba" }) })}</div></div>`;
    } else editorBody = `<div class="editor__lines" role="textbox" aria-multiline="true" aria-label="Conteúdo de ${file}" tabindex="0">${lines.map((l, i) => `<div class="editor__line ${i === changed ? "editor__line--changed" : ""}"><span>${highlight(l) || " "}</span></div>`).join("")}</div>`;
    const kindTag = inst ? W.tag("Instância de teste", "warn", { icon: "terminal" }) : W.tag("Config do pack", "plain", { title: "Sempre substitui a do jogador ao atualizar" });
    const editor = `<section class="editor" aria-label="Editor" style="position:relative"><div class="editor__bar"><span class="editor__file">${file}</span>${kindTag}<span class="grow"></span>
        ${W.segmented([["form", "Formulário"], ["text", "Texto"]], o.form ? "form" : "text", "Modo do editor")}${B("Buscar e substituir", "", { size: "sm", variant: "ghost", icon: "search" })}${o.form ? B("Mais ações do arquivo", "", { size: "sm", variant: "ghost", iconOnly: true, icon: "ellipsis", attrs: { "aria-haspopup": "menu", "aria-expanded": "false", "aria-controls": "menu-cfg", "data-go": null } }) + W.menu([{ label: "Restaurar o arquivo todo para o padrão…", desc: "Mostra as diferenças antes de gravar", icon: "rotate-ccw", attrs: go("") }, { label: "Copiar para defaultconfigs/", icon: "copy", attrs: go("") }], { id: "menu-cfg", label: "Mais ações do arquivo", style: "right:12px;top:44px" }) : ""}${B("Salvar", inst ? "teste-jogo" : o.form ? "configs-diff" : "configs-diff", { size: "sm", variant: "primary", icon: "save" })}</div>
      ${inst || o.form ? "" : `<div style="padding:8px 12px;border-bottom:1px solid var(--color-border)" class="t-xs t-3">${W.icon("info", "icon--sm")} Arquivo do Forge: o jogo pode reescrever este arquivo e apagar comentários que você adicionar.</div>`}
      ${editorBody}<div class="editor__foot"><span class="t-warn">1 alteração não salva neste arquivo</span><span class="grow"></span><span>${inst ? "JSON" : "TOML"} · UTF-8 · linha ${changed + 1}, coluna 24</span></div></section>`;
    return head + instBanner + ext + `<div class="cfg"><div class="cfg__side">${W.select({ bare: true, ariaLabel: "Origem dos arquivos", options: inst ? ["Mostrando: instância de teste (jogo aberto)", "Mostrando: arquivos do pack"] : ["Mostrando: arquivos do pack", "Mostrando: instância de teste"] })}${W.input({ bare: true, icon: "search", placeholder: "Buscar em todas as configs", ariaLabel: "Buscar em todas as configs", size: "sm", value: o.search ? "spawn" : null, attrs: o.search ? {} : { "data-go": "configs-busca" } })}${inst ? "" : W.check({ label: "Só o que mudou do padrão", checked: false })}<div role="tree" aria-label="Arquivos" class="tree">${tree}</div></div>${o.search ? searchResults() : o.script ? scriptEditor(state) : editor}</div>`;
  }

  // ---------- Busca em todas as configs (D4) ----------
  function searchResults() {
    const n = D.CONFIG_HITS.reduce((a, g) => a + g.hits.length, 0);
    return `<section class="panel" aria-labelledby="cfg-hits-h" style="overflow:auto;min-height:0"><div class="panel__head"><div><h2 class="panel__title panel__title--sans" id="cfg-hits-h">${n} resultados em ${D.CONFIG_HITS.length} arquivos para “spawn”</h2><p class="t-xs t-3" style="margin-top:2px">Procura no nome da chave, no nome traduzido do mod, no comentário e no valor. Clique para abrir o arquivo já na chave.</p></div>${B("Limpar busca", "configs", { size: "sm", variant: "ghost", icon: "x" })}</div>
      <div class="row row--wrap" style="margin-bottom:10px">${W.check({ label: "Só o que mudou do padrão" })}${W.select({ bare: true, size: "sm", ariaLabel: "Só deste mod", options: ["Todos os mods", "Só do Create", "Só do Supplementaries", "Só do Waystones"] })}</div>
      ${W.cfgHits(D.CONFIG_HITS, { attrs: go("configs-formulario") })}</section>`;
  }
  // ---------- Editor de scripts KubeJS (D4) ----------
  function hlJs(line, n) {
    let e = W.esc(line);
    if (/^\s*\/\//.test(line)) return `<span class="tk-c">${e}</span>`;
    e = e.replace(/(&#39;|')([^']*)(&#39;|')/g, (m, a, body) => `<span class="tk-str${n === 16 && body.includes("copper_ingott") ? " tk-err" : ""}">'${body}'</span>`);
    e = e.replace(/\b(ServerEvents|event)\b/g, '<span class="tk-kw">$1</span>').replace(/\.(recipes|shaped|mixing|create|heated)\b/g, '.<span class="tk-fn">$1</span>');
    return e;
  }
  function scriptEditor(state) {
    const game = state === "jogoaberto", srv = state === "servidor";
    const reload = srv ? B("Recarregar no jogo", "scripts~servidor", { size: "sm", icon: "refresh-cw", attrs: { "data-reload": "" } })
      : game ? B("Recarregar no jogo", "scripts~jogoaberto", { size: "sm", icon: "refresh-cw" })
      : B("Recarregar no jogo", "", { size: "sm", icon: "refresh-cw", disabled: true, tip: "Abra o jogo pelo Testar para recarregar os scripts sem reiniciar" });
    const lines = D.SCRIPT.map((l, i) => `<div class="editor__line ${i === 15 ? "editor__line--error" : ""}"><span>${hlJs(l, i + 1) || " "}</span></div>`).join("");
    const errLine = state === "carregando" ? "" : W.completions([["copper_ingot", "item", "Minecraft"], ["copper_block", "bloco", "Minecraft"], ["copper_ore", "bloco", "Minecraft"], ["copper_door", "bloco", "Minecraft"]], { typed: "minecraft:copper_", id: "cmp-ids", label: "IDs de itens e blocos do pack" });
    const note = game ? `<div style="padding:8px 12px;border-bottom:1px solid var(--color-border)">${W.alert({ kind: "info", compact: true, icon: "terminal", title: "No KubeJS 6, recarregar precisa de um comando no jogo.", text: "Cole no chat do jogo e aperte Enter. Com o servidor deste computador aberto, o Warden envia sozinho.", actions: B("Copiar /kubejs reload server_scripts", "", { size: "sm", icon: "copy" }), actionsBelow: true })}</div>` : "";
    return `<section class="editor" aria-label="Editor de script" style="position:relative"><div class="editor__bar"><span class="editor__file">kubejs/server_scripts/receitas.js</span>${W.tag("KubeJS 6 · Forge 1.20.1", "plain", { icon: "braces" })}<span class="grow"></span>
        ${B("Trechos prontos", "", { size: "sm", variant: "ghost", iconEnd: "chevron-down", tip: "Modelos dos eventos mais usados: receitas, tags, itens novos" })}${reload}${B("Abrir no VS Code", "", { size: "sm", variant: "ghost", icon: "external-link" })}${B("Salvar", "", { size: "sm", variant: "primary", icon: "save" })}</div>
      ${note}
      <div class="editor__lines" role="textbox" aria-multiline="true" aria-label="Conteúdo de receitas.js" tabindex="0" style="position:relative">${lines}<div style="position:absolute;left:150px;top:344px">${errLine}</div></div>
      ${W.scriptErrors([{ where: "receitas.js, linha 16", msg: "O item <span class=\"t-mono\">minecraft:copper_ingott</span> não existe. Quis dizer <span class=\"t-mono\">minecraft:copper_ingot</span>?", log: "[14:20:44] [ERROR] receitas.js#16: Failed to create recipe for type 'create:mixing': Item 'minecraft:copper_ingott' not found" }], { level: 2, meta: "do último teste, hoje às 14:20 · lido de logs/kubejs/server.log" })}
      <div class="editor__foot"><span class="t-warn">1 alteração não salva neste arquivo</span><span class="grow"></span><span>JavaScript · KubeJS 6 · linha 16, coluna 22</span></div></section>`;
  }
  window.P.configsContent = configsContent;
  def("configs", { group: "pack", title: "Configs (editor em texto)", spec: "T12", render: (s) => packShell("configs", configsContent(s)), states: { carregando: "Abrindo o arquivo", vazio: "Pack nunca testado: sem configs", erro: "Arquivo alterado fora do Warden" },
    after: () => document.querySelector('.segmented [data-value="form"]')?.setAttribute("data-go", "configs-formulario") });
  def("configs-formulario", { group: "pack", title: "Configs (formulário com padrão)", spec: "T12", d4: true, render: (s) => packShell("configs", configsContent(s, { form: true })), states: { erro: "Valor fora da faixa" },
    after: () => document.querySelector('.segmented [data-value="text"]')?.setAttribute("data-go", "configs") });
  def("configs-busca", { group: "pack", title: "Configs: buscar em todas as configs", spec: "T12", d4: true, render: () => packShell("configs", configsContent("normal", { search: true })) });
  def("scripts", { group: "pack", title: "Configs: editor de scripts (KubeJS)", spec: "T12", d4: true, render: (s) => packShell("configs", configsContent(s === "carregando" ? "normal" : s, { script: true }), s === "jogoaberto" || s === "servidor" ? { game: "running", runTarget: s === "servidor" ? "teste-servidor" : "teste-jogo" } : {}), states: { jogoaberto: "Jogo aberto (KubeJS 6: comando)", servidor: "Com o servidor deste computador" },
    after: (s) => { const ed = document.querySelector('.editor__lines[aria-label="Conteúdo de receitas.js"]'); if (ed) ed.scrollTop = 150; if (s === "servidor") window.WardenUI.toast(W.toast({ kind: "ok", title: "Scripts recarregados no servidor", text: "O Warden enviou kubejs reload server_scripts. Nenhum erro novo." }), 8000); } });
  function cfgDiffDialog() {
    return W.dialog({ esc: "configs", size: "lg", title: "Salvar config/create-common.toml?", sub: "Só esta linha muda. O resto do arquivo fica igual, inclusive os comentários.",
      body: W.diff({ file: "config/create-common.toml", lines: [["fold", "", "", "… 6 linhas iguais"], ["ctx", 7, 7, "[kinetics]"], ["ctx", 8, 8, "\t#Velocidade máxima de rotação"], ["ctx", 9, 9, "\t#Range: > 64"], ["del", 10, "", "\tmaxRotationSpeed = 128"], ["add", "", 10, "\tmaxRotationSpeed = 256"], ["ctx", 11, 11, "\t#Multiplicador de stress"], ["fold", "", "", "… 7 linhas iguais"]] }) + `<p class="t-sm t-3">Dá para desligar esta confirmação em Configurações.</p>`,
      foot: B("Continuar editando", "configs", { variant: "ghost" }) + B("Salvar", "configs", { variant: "primary", icon: "save" }) });
  }
  def("configs-diff", { group: "pack", title: "Salvar config: diferenças", spec: "T12", hidden: true, render: () => packShell("configs", configsContent("normal"), { overlay: cfgDiffDialog() }) });

  // ======================= PROBLEMAS =======================
  // Saúde do pack no topo, achados por gravidade, travamentos agrupados pela causa (D4)
  function healthPanel(score, losses, o = {}) {
    return `<section class="panel panel--strong" aria-labelledby="hp-h"><h2 class="sr-only" id="hp-h">Saúde do pack</h2><div class="row row--between row--top row--wrap">${W.health(score)}
      <div class="row">${o.actions || ""}</div></div>
      ${losses ? `<details class="disclosure" style="margin-top:12px"${o.open ? " open" : ""}><summary>O que tirou pontos</summary>${W.healthLosses(losses)}<p class="t-xs t-3" style="margin-top:6px">Faixas: 90 a 100 Ótimo, 75 a 89 Bom, 50 a 74 Atenção, abaixo de 50 Crítico. Avisos que você mandar ignorar não tiram pontos.</p></details>` : ""}</section>`;
  }
  function crashesTable() {
    return `<div class="tablewrap"><table class="table"><thead><tr><th>Causa</th><th class="num">Vezes</th><th>Última vez</th><th>Situação</th><th class="shrink"><span class="sr-only">Ações</span></th></tr></thead><tbody>
      ${D.CRASHES.map((c, i) => `<tr><td class="t-strong">${c.title}</td><td class="num">${c.times}</td><td class="t-2">${c.last}<div class="t-xs t-3">versão ${c.versions}</div></td><td><span style="white-space:normal">${W.status(c.state[0], c.state[1]).replace('class="status ', 'style="white-space:normal" class="status ')}</span></td><td style="width:1%"><div class="row row--wrap" style="justify-content:flex-end;min-width:150px">${i === 0 ? B("Ver o que causou", "teste-travou", { size: "sm" }) : i === 1 ? B("Encontrar o mod culpado", "culpado-config", { size: "sm", variant: "primary", icon: "target" }) + B("Conversar com a IA", "ia-consent", { size: "sm", variant: "ghost", icon: "sparkles", iconCls: "icon--ai" }) : B("Ver detalhes", "", { size: "sm", variant: "ghost" })}</div></td></tr>`).join("")}</tbody></table></div>
      <p class="t-xs t-3" style="margin-top:6px">Travamentos com a mesma causa ficam numa linha só. Os testes que fecharam normalmente são guardados só os 30 mais recentes.</p>`;
  }
  function problemsContent(state) {
    if (state === "vazio") return pageHead("Problemas", "Última verificação há 3 minutos.", B("Verificar agora", "problemas~carregando", { icon: "refresh-cw" })) + `<div class="stack">${healthPanel(100, null)}` + W.empty({ kind: "ok", glyph: "check", title: "Nenhum problema encontrado", text: "Dependências, versões, Java, duplicatas, conflitos conhecidos e o que os mods alteram no jogo foram conferidos nos 128 itens. A verificação também roda sozinha ao clicar em Testar e antes de exportar." }) + `</div>`;
    if (state === "carregando") return pageHead("Problemas", "Verificando…", W.btn("Verificando…", { busy: true, disabled: true })) + `<div class="panel" style="max-width:720px">${W.progress({ label: "Conferindo 128 itens", value: 64, meta: "Dependências, versões, Java, duplicatas, conflitos conhecidos e o que os mods alteram no jogo" })}</div>`;
    return pageHead("Problemas", "4 encontrados · última verificação há 3 minutos. Também roda sozinha ao clicar em Testar e antes de exportar.", B("Verificar agora", "problemas~carregando", { icon: "refresh-cw" })) +
      healthPanel(D.HEALTH, D.HEALTH_LOSSES) +
      `<h2 class="group-title">Erros <span class="t-3">impedem o teste, a menos que você escolha testar mesmo assim</span></h2>
      ${W.issue({ kind: "danger", id: "pb-1", title: "O Waystones precisa do Balm, que não está no pack", text: "Sem o Balm, o jogo para na tela de carregamento.", evidence: "O arquivo META-INF/mods.toml do Waystones 14.1.6 declara a dependência obrigatória “balm” na faixa [7.3.0,).", actions: B("Adicionar Balm", "dependencias", { variant: "primary", size: "sm", icon: "plus" }) + B("Ver Waystones", "mods", { variant: "ghost", size: "sm" }) + B("Ignorar neste pack", "", { variant: "ghost", size: "sm" }) + W.badge("p1", "P1") })}
      ${W.issue({ kind: "danger", id: "pb-2", title: "Dois mods de renderização no pack: Embeddium e Rubidium", text: "Os dois fazem a mesma coisa e travam o jogo quando estão juntos. Mantenha só um. O Embeddium é o mais novo.", evidence: "Lista de conflitos conhecidos do Warden, categoria “renderizador”.", actions: B("Remover Rubidium", "", { size: "sm", icon: "trash-2" }) + B("Remover Embeddium", "", { size: "sm", variant: "ghost" }) })}
      <h2 class="group-title">Avisos</h2>
      ${W.issue({ kind: "warn", id: "pb-3", title: "Xaero's Minimap está como Cliente e servidor, mas só funciona no cliente", text: "Num servidor, ele impede o servidor de abrir.", evidence: "O Modrinth informa client_side: required, server_side: unsupported.", actions: B("Mudar lado para Só cliente", "", { size: "sm" }) })}
      ${W.issue({ kind: "warn", id: "pb-4", title: "Epic Fight e Supplementaries alteram o mesmo ponto do jogo", text: "Os dois trocam a mesma chamada no movimento das criaturas (LivingEntity#travel). Só uma das trocas vale, e a outra some sem aviso. Pode ser a causa do travamento de 30/09.", evidence: "Lido dos arquivos dos dois mods: @Redirect no mesmo ponto, nos dois com prioridade 1000. O crash report de 30/09 às 21:14 cita epicfight.mixins.json.", actions: B("Ver o que o Epic Fight altera", "mods-raio-x", { size: "sm", icon: "scan-search" }) + B("Encontrar o mod culpado", "culpado-config", { size: "sm", variant: "ghost", icon: "target" }) + W.badge("p1", "P1") })}
      <h2 class="group-title">Travamentos <span class="t-3">dos seus testes, agrupados pela causa</span></h2>${crashesTable()}`;
  }
  def("problemas", { group: "pack", title: "Problemas (saúde e travamentos)", spec: "T14", d4: true, render: (s) => packShell("problemas", problemsContent(s), s === "vazio" ? { problems: 0 } : {}), states: { carregando: "Verificando agora", vazio: "Nenhum problema" } });

  // ======================= HISTÓRICO =======================
  function historyContent(state) {
    const head = pageHead("Histórico", "Salvar versão guarda o pack só neste computador. Os jogadores só recebem as versões que você publica.");
    if (state === "vazio") {
      return head + `<section class="panel" aria-labelledby="pub-h"><h2 class="panel__title" id="pub-h">Publicação para os jogadores</h2><p class="t-sm t-2" style="margin-top:6px">Este pack ainda não foi publicado. Salve uma versão marcada como versão final e publique: os jogadores recebem pelo link do pack.</p></section>
        <ol class="timeline" style="margin-top:16px">${W.tlItem({ kind: "final", version: "0.1.0", date: "hoje, 15:02", summary: "Pack criado (Minecraft 1.20.1, Forge 47.3.0)", actions: B("Publicar versão", "publicar-repo", { variant: "primary", size: "sm", icon: "globe" }) })}</ol>`;
    }
    const pub = `<section class="panel" aria-labelledby="pub-h"><div class="panel__head"><h2 class="panel__title" id="pub-h">Publicação para os jogadores</h2>${B("Como os jogadores instalam", "publicado", { variant: "link" })}</div>
      <dl class="kv"><dt>Repositório</dt><dd>github.com/kriticales/vale-sereno ${W.tag("Público", "plain", { icon: "globe" })}</dd><dt>Versão publicada</dt><dd>1.4.0, em 20/09/2026</dd></dl>
      <div class="field" style="margin-top:12px"><label class="field__label" for="pub-link">Link do pack</label><div class="linkbox">${W.input({ id: "pub-link", bare: true, value: D.PACK_LINK, readonly: true })}${B("Copiar link", "", { size: "sm", icon: "copy" })}</div><p class="field__hint">Quem joga com este link recebe as atualizações sozinho ao abrir o jogo.</p></div></section>`;
    const unsaved = W.tlItem({ kind: "unsaved", version: "Agora", summary: "5 alterações desde a 1.4.2", actions: B("Salvar versão", "salvar", { variant: "primary", size: "sm", icon: "save" }),
      detail: `<div class="grid-2 t-sm"><div><div class="t-caps t-3">Adicionados</div><p>Sophisticated Backpacks 3.20.17<br>Sophisticated Core 0.6.26</p></div><div><div class="t-caps t-3">Atualizados</div><p>JEI 15.20.0.106 → 15.20.0.112</p><div class="t-caps t-3" style="margin-top:8px">Configs alteradas</div><p class="path">config/create-common.toml · options.txt</p></div></div><div style="margin-top:8px">${B("Descartar a alteração de um arquivo…", "", { variant: "link" })} ${W.badge("p1", "P1")}</div>` });
    const items = D.VERSIONS.map((v, i) => W.tlItem({ kind: v.kind, version: v.v, date: v.date, summary: v.summary, open: i === 0,
      actions: (v.kind === "final" ? B("Publicar versão", "publicar-142", { variant: "primary", size: "sm", icon: "globe" }) : v.kind === "saved" ? B("Marcar como versão final", "", { size: "sm", icon: "flag" }) : "") + (i === 3 ? B("Voltar para esta versão", "voltar", { size: "sm", icon: "rotate-ccw" }) : "") + B(i === 0 ? "Fechar detalhes" : "Ver mudanças", "", { size: "sm", variant: "ghost", icon: i === 0 ? "chevron-up" : "chevron-down" }),
      detail: i === 0 ? `<pre class="code">## 1.4.2 (28/09/2026)\n### Atualizados\n- Create 0.5.1.i → 0.5.1.j\n- Embeddium 0.3.30 → 0.3.31\n### Configs alteradas\n- config/create-common.toml</pre><div class="btn-row" style="margin-top:8px">${B("Ver diferenças para o estado atual", "", { size: "sm", icon: "file-diff" })}${B("Exportar esta versão", "exportar", { size: "sm", icon: "package" })}${W.badge("p1", "P1")}</div>` : "" })).join("");
    return head + pub + `<h2 class="group-title" style="margin-top:24px">Versões</h2><ol class="timeline">${unsaved}${items}</ol>
      <p class="t-sm" style="margin-top:12px">${B("Pontos de segurança…", "", { variant: "link" })} ${W.badge("p1", "P1")} <span class="t-3">cópias automáticas feitas antes de ações que apagam ou substituem</span></p>`;
  }
  def("historico", { group: "pack", title: "Histórico", spec: "T16, T17, T18", render: (s) => packShell("historico", historyContent(s), s === "vazio" ? { unsaved: 0, problems: 0, version: "0.1.0" } : {}), states: { vazio: "Pack novo, nunca publicado" } });

  function saveDialog() {
    return W.dialog({ esc: "historico", size: "lg", title: "Salvar versão", sub: "Guarda o pack como está agora, neste computador. Dá para voltar a ela quando quiser.", body: `
      <div class="grid-2"><div>${W.input({ id: "sv-num", label: "Número da versão", value: "1.5.0", mono: true })}</div>
        <div class="t-sm t-2" style="align-self:end;padding-bottom:6px">Sugerido: <b class="t-primary">1.5.0</b>, porque você adicionou mods.</div></div>
      <details class="disclosure"><summary>Como o número é escolhido</summary><ul class="stack-2 t-sm" style="margin-top:8px"><li><span class="t-mono">1.4.2 → 1.4.3</span> só atualizações e ajustes de config</li><li><span class="t-mono">1.4.2 → 1.5.0</span> mods novos ou mudanças de equilíbrio</li><li><span class="t-mono">1.4.2 → 2.0.0</span> saiu um mod com blocos nos mundos: os mundos dos jogadores podem perder coisas</li></ul></details>
      ${W.textarea({ id: "sv-notes", label: "Notas", optional: true, placeholder: "Ex.: mochilas novas e ajuste de velocidade do Create", rows: 2, hint: "Entram no topo do resumo automático." })}
      ${W.check({ label: "<b>Marcar como versão final</b>", desc: "Pronta para os jogadores. Depois de salvar, dá para publicar.", checked: true })}
      <div><div class="field__label" style="margin-bottom:6px">Resumo automático</div><pre class="code">### Adicionados\n- Sophisticated Backpacks 3.20.17\n- Sophisticated Core 0.6.26\n### Atualizados\n- Just Enough Items 15.20.0.106 → 15.20.0.112\n### Configs alteradas\n- config/create-common.toml\n- options.txt</pre></div>
      <div class="panel"><div class="panel__title panel__title--sans" style="margin-bottom:8px">Antes de salvar ${W.badge("p1", "P1")}</div><ul class="checklist"><li class="is-bad">${W.icon("circle-x")}<span>O pack tem 2 erros. ${B("Ver problemas", "problemas", { variant: "link" })}</span></li><li class="is-ok">${W.icon("circle-check")}<span>O último teste é depois da última mudança e abriu normalmente.</span></li><li class="is-ok">${W.icon("circle-check")}<span>As mudanças do teste foram revisadas.</span></li></ul><p class="t-xs t-3" style="margin-top:6px">Isso não impede salvar: é só para você saber.</p></div>`,
      foot: B("Cancelar", "historico", { variant: "ghost" }) + B("Salvar versão 1.5.0", "salvo", { variant: "primary", icon: "save" }) });
  }
  def("salvar", { group: "pack", title: "Salvar versão (diálogo)", spec: "T16", render: () => packShell("historico", historyContent("normal"), { overlay: saveDialog() }) });
  def("salvo", { group: "pack", title: "Versão salva: próximos passos", spec: "T16, T18", hidden: true, render: () => packShell("historico", historyContent("normal"), { unsaved: 0, version: "1.5.0",
    overlay: W.dialog({ esc: "historico", size: "sm", title: "Versão 1.5.0 salva como versão final", body: `<p>Ela está guardada neste computador. Os jogadores só recebem quando você publicar.</p>`, foot: B("Fechar", "historico", { variant: "ghost" }) + B("Exportar arquivo", "exportar", { icon: "package" }) + B("Publicar versão 1.5.0", "publicar", { variant: "primary", icon: "globe" }) }) }) });

  function publishDialog(v) {
    return W.dialog({ esc: "historico", size: "lg", title: `Publicar a versão ${v} para os jogadores`, sub: "Vai para o GitHub (kriticales/vale-sereno). Quem joga com o link do pack recebe a atualização ao abrir o jogo.", body: `
      <div><div class="t-caps t-3" style="margin-bottom:6px">Antes de publicar</div>
        ${W.issue({ kind: "warn", title: "2 mods da CurseForge não podem ser baixados automaticamente pelos jogadores", text: "O autor bloqueou downloads por outros apps. Cada jogador vai ter que baixar à mão.", actions: `<div class="stack-2" style="width:100%"><div class="row row--between"><span class="t-sm"><b>Epic Fight</b> <span class="t-3">· o mesmo arquivo existe no Modrinth</span></span>${B("Trocar pelo Modrinth", "", { size: "sm" })}</div><div class="t-sm"><b>Xaero's World Map</b> <span class="t-3">· não existe no Modrinth</span></div></div>` })}
        ${W.issue({ kind: "warn", title: `A versão ${v} não foi testada`, text: "O último teste foi antes das últimas mudanças.", actions: B("Testar agora", "teste-checagem", { size: "sm", icon: "play" }) })}
        ${W.issue({ kind: "danger", title: "O pack tem 2 erros", text: "Os jogadores podem não conseguir abrir o jogo.", actions: B("Ver problemas", "problemas", { size: "sm" }) })}
        <p class="t-xs t-3" style="margin-top:6px">Trocar um mod muda o pack: depois é preciso salvar uma versão nova e publicar essa.</p></div>
      <div><div class="field__label">Notas da versão</div><p class="field__hint" style="margin-bottom:6px">Mudanças desde a 1.4.0, a última publicada. Vão para o CHANGELOG.md e para a Release do GitHub.</p>
        <pre class="code code--scroll" tabindex="0" aria-label="Notas da versão">## Vale Sereno ${v} (01/10/2026)\n### Adicionados\n- Sophisticated Backpacks 3.20.17\n- Sophisticated Core 0.6.26\n### Atualizados\n- Create 0.5.1.h → 0.5.1.j\n- Embeddium 0.3.28 → 0.3.31\n- Just Enough Items 15.20.0.106 → 15.20.0.112\n### Configs alteradas\n- config/create-common.toml\n- options.txt</pre></div>
      ${W.textarea({ id: "pb-notes", label: "Recado para os jogadores", optional: true, placeholder: "Ex.: façam backup dos mundos antes de atualizar", rows: 2 })}
      <div class="grid-2 t-sm"><div><div class="t-caps t-3">Vai para o GitHub</div><p class="t-2" style="margin-top:4px">pack.toml, index.toml, os arquivos do índice, CHANGELOG.md e .gitattributes</p></div><div><div class="t-caps t-3">Não vai</div><p class="t-2" style="margin-top:4px">.warden/, mundos de teste, registros, versões não publicadas</p></div></div>`,
      foot: B("Cancelar", "historico", { variant: "ghost" }) + B("Publicar mesmo assim", "publicado", { variant: "primary", icon: "globe" }) });
  }
  def("publicar", { group: "pack", title: "Publicar versão: avisos e notas", spec: "T18", render: () => packShell("historico", historyContent("normal"), { unsaved: 0, version: "1.5.0", overlay: publishDialog("1.5.0") }) });
  def("publicar-142", { group: "pack", title: "Publicar a 1.4.2 (pelo Histórico)", spec: "T18", hidden: true, render: () => packShell("historico", historyContent("normal"), { overlay: publishDialog("1.4.2") }) });
  def("publicar-repo", { group: "pack", title: "Primeira publicação: repositório", spec: "T18", hidden: true, render: () => packShell("historico", historyContent("vazio"), { unsaved: 0, problems: 0, version: "0.1.0",
    overlay: W.dialog({ esc: "historico~vazio", title: "Primeira publicação deste pack", sub: "O Warden cria um repositório no seu GitHub para o pack.", body: `
      <div class="field"><label class="field__label" for="rp-name">Nome do repositório</label><div class="row">${W.input({ id: "rp-name", bare: true, value: "vale-sereno", mono: true })}<span class="t-sm t-3 path" style="white-space:nowrap">github.com/kriticales/vale-sereno</span></div></div>
      <fieldset style="border:0;padding:0;margin:0"><legend class="field__label" style="margin-bottom:6px">Quem pode ver</legend><div class="choice-list">
        ${W.choice({ name: "rp-vis", title: "Público", badge: " " + W.tag("Recomendado", "ok"), desc: "Os jogadores recebem atualizações automáticas pelo link do pack. Qualquer pessoa com o link vê os arquivos do pack (mods e configs). Nada do seu computador vai junto.", checked: true })}
        ${W.choice({ name: "rp-vis", title: "Privado", desc: "Só backup no GitHub. O link não funciona para os jogadores: eles não recebem atualizações automáticas." })}</div></fieldset>
      <div>${B("Usar um repositório que já existe", "", { variant: "link" })}</div>`, foot: B("Cancelar", "historico~vazio", { variant: "ghost" }) + B("Criar repositório e publicar", "publicado", { variant: "primary", icon: "globe" }) }) }) });
  def("publicado", { group: "pack", title: "Versão publicada: link e passo a passo", spec: "T18", render: () => packShell("historico", historyContent("normal"), { unsaved: 0, version: "1.5.0",
    overlay: W.dialog({ esc: "historico", size: "lg", title: "Versão 1.5.0 publicada", sub: "Quem já joga com o link recebe a atualização na próxima vez que abrir o jogo. O GitHub pode levar alguns minutos para mostrar a versão nova.", body: `
      <div class="field"><label class="field__label" for="pd-link">Link do pack</label><div class="linkbox">${W.input({ id: "pd-link", bare: true, value: D.PACK_LINK, readonly: true })}${B("Copiar link", "", { variant: "primary", icon: "copy" })}</div></div>
      <div class="panel"><div class="panel__head"><h3 class="panel__title panel__title--sans">Como os jogadores instalam (Prism Launcher ou MultiMC)</h3>${B("Copiar instruções", "", { size: "sm", icon: "copy" })}</div>
        <ol class="howto"><li><span>Criar uma instância com Minecraft 1.20.1 e Forge 47.3.0.</span></li><li><span>Baixar o packwiz-installer-bootstrap.jar e colocar na pasta <code>.minecraft</code> da instância.</span></li>
          <li><span>Em Editar instância → Configurações → Comandos personalizados, colar no “comando antes de iniciar”:<pre class="code" style="margin-top:6px">"$INST_JAVA" -jar packwiz-installer-bootstrap.jar ${D.PACK_LINK}</pre></span></li><li><span>Abrir o jogo. O pack é baixado agora e atualizado sozinho nas próximas vezes.</span></li></ol></div>
      <div class="btn-row">${B("Copiar texto das notas da versão", "", { size: "sm", icon: "copy" })}${B("Abrir a Release no GitHub", "", { size: "sm", icon: "external-link" })}</div>`, foot: B("Fechar", "historico", { variant: "primary" }) }) }) });
  def("voltar", { group: "pack", title: "Voltar para uma versão", spec: "T17", hidden: true, render: () => packShell("historico", historyContent("normal"), {
    overlay: W.dialog({ esc: "historico", size: "sm", alert: true, title: "Voltar o pack para a versão 1.3.0?", body: `<p>O pack fica exatamente como estava na 1.3.0. Arquivos que não existiam nela são removidos.</p><p class="t-sm t-2">O estado de agora, com as 5 alterações não salvas, fica guardado num ponto de segurança. O histórico não é apagado.</p>`, foot: B("Cancelar", "historico", { variant: "ghost" }) + B("Voltar para 1.3.0", "historico", { variant: "primary", icon: "rotate-ccw" }) }) }) });

  // ======================= EXPORTAR =======================
  function exportContent(state, o = {}) {
    const srv = !!o.server;
    const head = pageHead("Exportar", "Gera o pack para quem vai jogar, só com o necessário. O pack no Warden não muda.");
    const checks = `<section class="panel" aria-labelledby="ex-1"><h2 class="panel__title panel__title--sans" id="ex-1">1. Antes de exportar</h2><ul class="checklist" style="margin-top:10px">
      <li class="is-ok">${W.icon("circle-check")}<span>Nenhum arquivo que não deveria ir para quem joga (registros, cópias de segurança, caches).</span></li>
      <li class="is-bad">${W.icon("circle-x")}<span>O pack tem 2 erros. ${B("Ver problemas", "problemas", { variant: "link" })}</span></li>
      <li class="is-warn">${W.icon("triangle-alert")}<span>Há 5 alterações não salvas: a exportação usa o estado de agora. ${B("Salvar versão antes", "salvar", { variant: "link" })}</span></li></ul></section>`;
    const fmt = `<section class="panel" aria-labelledby="ex-2"><h2 class="panel__title panel__title--sans" id="ex-2">2. Formato</h2><fieldset style="border:0;padding:0;margin:10px 0 0"><legend class="sr-only">Formato</legend><div class="choice-list choice-list--2">
      ${W.choice({ name: "ex-f", title: "Pasta packwiz", desc: "Para hospedar em outro lugar. Para o GitHub, use Publicar versão no Histórico.", checked: !srv })}
      ${W.choice({ name: "ex-f", title: "Arquivo .zip do pack packwiz", desc: "A mesma coisa, num arquivo só." })}
      ${W.choice({ name: "ex-f", title: ".mrpack", badge: " " + W.badge("p1", "P1"), desc: "Para o app do Modrinth e launchers compatíveis." })}
      ${W.choice({ name: "ex-f", title: ".zip da CurseForge", badge: " " + W.badge("p1", "P1"), desc: "Para o app da CurseForge. Não guarda o lado dos mods." })}
      <span ${srv ? "" : 'data-go="exportar-servidor"'}>${W.choice({ name: "ex-f", title: "Pacote para servidor (.zip)", badge: " " + W.badge("p1", "P1"), desc: "Para abrir o pack num servidor (em casa ou numa hospedagem). Mods só de cliente ficam de fora.", checked: srv })}</span></div></fieldset></section>`;
    const how = `<section class="panel" aria-labelledby="ex-how"><h2 class="panel__title panel__title--sans" id="ex-how">Como os mods chegam ao servidor</h2><fieldset style="border:0;padding:0;margin:10px 0 0"><legend class="sr-only">Como os mods chegam ao servidor</legend><div class="choice-list choice-list--2">
      ${W.choice({ name: "ex-srv", title: "Baixar os mods pelo link do pack", badge: " " + W.tag("Recomendado", "ok"), desc: "O servidor baixa os mods ao iniciar e se atualiza sozinho a cada versão publicada. Precisa do pack publicado (está: versão 1.4.0).", checked: true })}
      ${W.choice({ name: "ex-srv", title: "Mods dentro do zip", desc: "Funciona sem internet, mas o zip fica grande (cerca de 610 MB) e 2 mods da CurseForge precisam ser baixados à mão." })}</div></fieldset>
      <div class="row row--wrap" style="margin-top:12px">${W.select({ id: "ex-mem", label: "Memória do servidor (user_jvm_args.txt)", options: ["6 GB (recomendado para 120 mods)", "4 GB", "8 GB"] })}</div></section>`;
    const whatSrv = `<section class="panel" aria-labelledby="ex-3s"><h2 class="panel__title panel__title--sans" id="ex-3s">O que vai no pacote</h2><div class="export-tree" style="margin-top:10px">
      <b>start.bat</b> · <b>start.sh</b> <span class="t-3">(verificam o Java, instalam o Forge na primeira vez e perguntam sobre a EULA)</span><br><b>user_jvm_args.txt</b> <span class="t-3">-Xms6G -Xmx6G</span><br><b>packwiz-installer-bootstrap.jar</b> <span class="t-3">(baixa os mods pelo link)</span><br>
      ${W.icon("chevron-right", "icon--sm")} <b>config/</b> 86 arquivos · <b>defaultconfigs/</b> 2 · <b>kubejs/</b> sem client_scripts/</div>
      ${W.alert({ kind: "neutral", compact: true, icon: "monitor", title: "8 mods só de cliente ficam de fora.", text: "Embeddium, Rubidium, Jade, Xaero's Minimap e mais 4. Também ficam de fora options.txt, resource packs e shaders." })}</section>`;
    const untested = W.alert({ kind: "warn", title: "Este pacote ainda não foi testado num servidor.", text: "Às vezes um mod diz que funciona no servidor e não funciona. Testar abre um servidor neste computador, só para você.", actions: B("Testar como servidor", "servidor-opcoes", { size: "sm", icon: "server" }) });
    const what = `<section class="panel" aria-labelledby="ex-3"><h2 class="panel__title panel__title--sans" id="ex-3">3. O que vai no pack</h2><div class="export-tree" style="margin-top:10px">
      <b>pack.toml</b> · <b>index.toml</b><br>${W.icon("chevron-right", "icon--sm")} <b>mods/</b> 124 referências <span class="t-3">(os .jar não vão: quem joga baixa)</span><br>${W.icon("chevron-right", "icon--sm")} <b>resourcepacks/</b> 3 referências · <b>shaderpacks/</b> 1 referência<br>
      ${W.icon("chevron-right", "icon--sm")} <b>config/</b> 86 arquivos, 412 KB<br>${W.icon("chevron-right", "icon--sm")} <b>defaultconfigs/</b> 2 arquivos<br><b>options.txt</b> ${W.tag("Substitui as preferências de quem já joga", "warn")} ${B("Não substituir se o jogador já tiver", "", { variant: "link" })} ${W.badge("p1", "P1")}</div>
      <p class="t-sm" style="margin-top:10px">Algo aqui não deveria ir? ${B("Excluir do pack…", "", { variant: "link" })}</p></section>`;
    let foot;
    if (state === "carregando") foot = `<div class="panel" style="margin-top:16px">${W.progress({ label: "Exportando", value: 72, meta: "156 de 216 arquivos · conferindo com o packwiz no fim" })}<div style="margin-top:10px">${B("Cancelar", "exportar", { variant: "ghost", size: "sm" })}</div></div>`;
    else foot = `${state === "erro" ? `<div style="margin-top:16px">${W.alert({ kind: "danger", title: "Não foi possível exportar: o disco D: está cheio.", text: "Faltam 1,2 MB. Nada foi gravado pela metade. Libere espaço ou escolha outra pasta.", actions: B("Escolher outra pasta", "", { size: "sm", icon: "folder-open" }) })}</div>` : ""}
      <div class="export-foot"><div class="field grow"><label class="field__label" for="ex-dir">Pasta de destino</label>${W.input({ id: "ex-dir", bare: true, value: state === "erro" ? "D:\\Exportados\\vale-sereno-1.4.2" : "Documents\\Warden\\exportados\\vale-sereno-1.4.2", mono: true, attrs: state === "erro" ? { "aria-invalid": "true" } : {} })}</div>${B("Escolher pasta…", "", { icon: "folder-open" })}${B("Exportar", "exportado", { variant: "primary", size: "lg", icon: "package" })}</div>`;
    if (srv) foot = `<div class="stack" style="margin-top:16px">${untested}${W.check({ label: "Testar no servidor deste computador depois de gerar (recomendado)", checked: true })}</div><div class="export-foot"><div class="field grow"><label class="field__label" for="ex-dir2">Pasta de destino</label>${W.input({ id: "ex-dir2", bare: true, value: "Documents\\Warden\\exportados\\vale-sereno-1.4.2-servidor.zip", mono: true })}</div>${B("Escolher pasta…", "", { icon: "folder-open" })}${B("Gerar pacote para servidor", "", { variant: "primary", size: "lg", icon: "server" })}</div>`;
    return head + `<div class="stack" style="max-width:920px">${checks}${fmt}${srv ? how + whatSrv : what}</div><div style="max-width:920px">${foot}</div>`;
  }
  def("exportar", { group: "pack", title: "Exportar", spec: "T19", render: (s) => packShell("exportar", exportContent(s)), states: { carregando: "Exportando", erro: "Disco cheio" } });
  def("exportar-servidor", { group: "pack", title: "Exportar: pacote para servidor", spec: "T19", d4: true, render: () => packShell("exportar", exportContent("normal", { server: true })) });
  def("exportado", { group: "pack", title: "Exportado", spec: "T19", hidden: true, render: () => packShell("exportar", exportContent("normal"), { overlay: W.dialog({ esc: "exportar", size: "sm", title: "Pack exportado", body: `<p>216 arquivos, 1,2 MB, em <span class="path">Documents\\Warden\\exportados\\vale-sereno-1.4.2</span>.</p>${W.alert({ kind: "ok", compact: true, title: "Conferido com o packwiz.", text: "O pack exportado é exatamente o que o packwiz geraria." })}`, foot: B("Fechar", "exportar", { variant: "ghost" }) + B("Abrir pasta", "exportar", { variant: "primary", icon: "folder-open" }) }) }) });

  // ======================= INFORMAÇÕES DO PACK =======================
  def("info", { group: "pack", title: "Informações do pack (diálogo)", spec: "T11", render: () => packShell("mods", modsContent("normal"), {
    overlay: W.dialog({ esc: "mods", title: "Informações do pack", body: `${W.input({ id: "if-name", label: "Nome", value: "Vale Sereno" })}${W.input({ id: "if-author", label: "Autor", value: "Kriticales" })}${W.textarea({ id: "if-desc", label: "Descrição", optional: true, placeholder: "Uma frase sobre o pack", rows: 2 })}
      ${W.select({ id: "if-forge", label: "Versão do Forge", options: ["47.3.0 (recomendada)", "47.2.20", "47.1.106"], hint: "Minecraft e loader não mudam aqui: mudar a versão do Minecraft é outro pack." })}
      <hr class="sep" /><div class="btn-row">${B("Mostrar pasta do pack", "", { size: "sm", icon: "folder-open" })}${B("Apagar pack…", "", { size: "sm", variant: "danger-ghost", icon: "trash-2" })}${W.badge("p1", "P1")}</div>`,
      foot: B("Cancelar", "mods", { variant: "ghost" }) + B("Salvar", "mods", { variant: "primary" }) }) }) });
})();
