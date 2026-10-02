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
  function modsContent(state, o = {}) {
    const actions = B("Verificar atualizações", "", { icon: "refresh-cw" }) + B("Adicionar", "adicionar", { variant: "primary", icon: "plus" });
    if (state === "vazio") {
      return pageHead("Mods", "Mods, resource packs e shaders do pack.", B("Adicionar", "adicionar", { variant: "primary", icon: "plus" })) +
        W.empty({ title: "Nenhum mod ainda", text: "O pack Vale Sereno foi criado para Minecraft 1.20.1 com Forge 47.3.0. Comece pelos mods; resource packs e shaders entram pelo mesmo botão.", actions: B("Adicionar mods", "adicionar", { variant: "primary", icon: "plus" }), hint: "Ou arraste arquivos .jar e .zip do computador para cá.", glyph: "plus" });
    }
    const head = pageHead("Mods", `128 itens: 124 mods, 3 resource packs e 1 shader. <span class="dropnote">${W.icon("download", "icon--sm")}Arraste arquivos .jar ou .zip para cá para adicionar do computador.</span>`, actions);
    const tools = `<div class="toolbar">${W.input({ bare: true, icon: "search", placeholder: "Buscar no pack", ariaLabel: "Buscar no pack" })}${W.select({ bare: true, ariaLabel: "Fonte", options: ["Fonte: todas", "Modrinth", "CurseForge", "Arquivo local"] })}${W.select({ bare: true, ariaLabel: "Lado", options: ["Lado: todos", "Cliente e servidor", "Só cliente", "Só servidor"] })}${W.select({ bare: true, ariaLabel: "Mostrar", options: ["Mostrar: tudo", "Com problemas (3)", "Com atualização (4)", "Não salvos (2)"] })}</div>`;
    if (state === "carregando") return head + tools + `<div class="tablewrap"><table class="table" aria-busy="true">${W.modTableHead()}<tbody>${W.skeletonRows(8, 5)}</tbody></table></div>`;
    const banner = `<div class="mods-banner">${W.alert({ kind: "info", icon: "refresh-cw", title: "4 atualizações disponíveis.", text: "Nenhuma é aplicada sem você revisar antes.", actions: B("Revisar e atualizar", "atualizar", { size: "sm" }) })}</div>`;
    const sel = `<div class="selbar" style="margin-bottom:12px" role="region" aria-label="Ações para os selecionados"><span class="selbar__count">1 selecionado</span>${B("Alterar lado", "", { size: "sm" })}${B("Atualizar", "", { size: "sm", icon: "refresh-cw" })}${B("Remover", "", { size: "sm", variant: "danger-ghost", icon: "trash-2" })}<span class="grow"></span>${B("Limpar seleção", "", { size: "sm", variant: "ghost" })}</div>`;
    let mods = D.MODS.map((m) => toRow(m, { selected: m[0] === "Embeddium", attrs: go(m[0].startsWith("Just") ? "mods-detalhe" : m[0] === "Waystones" || m[0] === "Rubidium" ? "problemas" : "") }));
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
  def("mods", { group: "pack", title: "Mods, resource packs e shaders", spec: "T05, T06, T10", render: (s) => packShell("mods", modsContent(s), s === "vazio" ? { unsaved: 0, problems: 0, empty: true } : {}), states: { carregando: "Lendo o índice do pack", vazio: "Pack sem mods", erro: "Um arquivo inválido e um fora do índice" } });
  def("pack-novo", { group: "pack", title: "Pack recém-criado (vazio)", spec: "T03, T06", hidden: true, render: () => packShell("mods", modsContent("vazio"), { unsaved: 0, problems: 0, empty: true, version: "0.1.0" }) });
  def("pack-revisar", { group: "pack", title: "Pack com mudanças do teste para revisar", spec: "T05, T15", hidden: true, render: () => packShell("mods", modsContent("normal"), { unsaved: 5, alerts: [{ kind: "warn", text: "Mudanças do teste para revisar", attrs: go("teste-fechou") }] }) });

  function detailDrawer() {
    return W.drawer({ esc: "mods", title: "Detalhes do mod", body: `<div class="row row--gap-3">${W.tile("Just Enough Items (JEI)", "xl")}<div><div class="t-display-lg">Just Enough Items</div><div class="t-xs t-3" style="margin-top:4px">por mezz · ${W.source("curseforge")}</div></div></div>
      <p class="t-sm t-2">Mostra todos os itens e receitas do jogo.</p>
      ${W.alert({ kind: "info", icon: "refresh-cw", compact: true, title: "Atualização disponível: 15.20.0.112", text: "Corrige um travamento ao abrir receitas do Create." })}
      <dl class="kv"><dt>Versão instalada</dt><dd class="t-mono">15.20.0.106</dd><dt>Para</dt><dd>Minecraft 1.20.1 · Forge</dd><dt>Publicada em</dt><dd>12/03/2025</dd><dt>Arquivo</dt><dd class="path">jei-1.20.1-forge-15.20.0.106.jar · 1,4 MB</dd></dl>
      ${W.select({ id: "dt-side", label: "Lado", options: [["both", "Cliente e servidor"], ["client", "Só cliente"], ["server", "Só servidor"]], value: "both", hint: "Onde o mod precisa estar instalado. Informado pela CurseForge." })}
      <div><div class="field__label">Dependências</div><p class="t-sm t-3">Nenhuma obrigatória.</p></div>
      <div><div class="field__label">Usado por</div><p class="t-sm">${B("Just Enough Resources", "", { variant: "link" })} <span class="t-3">(no pack)</span></p></div>
      <details class="issue__evidence"><summary>Novidades da versão 15.20.0.112</summary><pre class="code" style="margin-top:6px">- Corrige travamento ao abrir receitas do Create
- Melhora a busca por nome de mod (@create)</pre></details>
      <details class="issue__evidence"><summary>Mais opções</summary><div class="stack-2" style="margin-top:8px">${B("Trocar de versão…", "", { size: "sm" })}${W.switchCtl({ label: "Fixar versão (não atualizar)" })}${W.switchCtl({ label: "Opcional para o jogador", disabled: true })} ${W.badge("p1", "P1")}</div></details>`,
      foot: B("Atualizar para 15.20.0.112", "atualizar", { variant: "primary", icon: "refresh-cw" }) + B("Remover", "remover", { variant: "danger-ghost", icon: "trash-2" }) + `<span class="grow"></span>` + B("Abrir página", "", { variant: "ghost", icon: "external-link" }) });
  }
  def("mods-detalhe", { group: "pack", title: "Detalhes de um mod (painel lateral)", spec: "T07, T11", render: () => packShell("mods", modsContent("normal"), { overlay: detailDrawer() }) });

  function updateDialog() {
    const r = [["Just Enough Items (JEI)", "15.20.0.106", "15.20.0.112"], ["Xaero's Minimap", "24.6.1", "24.6.2"], ["Create", "0.5.1.j", "0.5.1.k"], ["Farmer's Delight", "1.2.6", "1.2.7"]];
    return W.dialog({ esc: "mods", size: "lg", title: "Atualizar 4 itens", sub: "Confira antes de aplicar. Nada muda até você confirmar.", body: `<div class="tablewrap"><table class="table table--plain"><thead><tr><th class="shrink"><span class="sr-only">Incluir</span></th><th>Item</th><th>Agora</th><th>Nova</th><th>Novidades</th></tr></thead><tbody>${r.map((x) => `<tr><td>${W.check({ checked: true, ariaLabel: "Atualizar " + x[0] })}</td><td class="t-strong">${x[0]}</td><td class="t-mono t-2">${x[1]}</td><td class="t-mono t-primary">${x[2]}</td><td>${B("Ver novidades", "", { variant: "link" })}</td></tr>`).join("")}</tbody></table></div>
      <ul class="checklist"><li class="is-ok">${W.icon("circle-check")}<span>Nenhuma dependência nova.</span></li><li class="is-ok">${W.icon("circle-check")}<span>Nenhuma incompatibilidade nova com o que já está no pack.</span></li></ul>
      <p class="t-sm t-3">Antes de aplicar, o Warden guarda um ponto de segurança. Depois, verifica o pack de novo.</p>`, foot: B("Cancelar", "mods", { variant: "ghost" }) + B("Atualizar 4 itens", "mods", { variant: "primary", icon: "refresh-cw" }) });
  }
  def("atualizar", { group: "pack", title: "Atualizar itens (diálogo)", spec: "T10", hidden: true, render: () => packShell("mods", modsContent("normal"), { overlay: updateDialog() }) });
  function removeDialog() {
    return W.dialog({ esc: "mods-detalhe", size: "sm", alert: true, title: "Remover Just Enough Items (JEI)?", body: `<p>Este mod depende dele: <b>Just Enough Resources</b>. Sem o JEI, ele pode não funcionar.</p><p class="t-sm t-3">O item sai do pack. Dá para voltar atrás pelo Histórico.</p>`, foot: B("Manter no pack", "mods-detalhe", { variant: "ghost" }) + B("Remover JEI", "mods", { variant: "danger", icon: "trash-2" }) });
  }
  def("remover", { group: "pack", title: "Remover item (confirmação)", spec: "T06", hidden: true, render: () => packShell("mods", modsContent("normal"), { overlay: removeDialog() }) });

  // ======================= ADICIONAR =======================
  function result(r, i, o = {}) {
    const [name, author, desc, dl, upd, src, inPack, manual] = r;
    const pressed = o.selected === i;
    return `<button type="button" class="result" aria-pressed="${pressed}"${W.attrs(go(""))}>${W.tile(name, "lg")}<span><span class="result__name">${name}</span> <span class="t-xs t-3">por ${author}</span><span class="result__desc" style="display:block">${desc}</span>
      <span class="result__meta">${W.source(o.onlyModrinth ? "modrinth" : src)}<span>${dl} downloads</span><span>atualizado ${upd}</span>${manual ? W.tag("Download manual", "warn", { icon: "download" }) : ""}</span></span>
      ${inPack ? W.tag("Já no pack", "primary", { icon: "check" }) : ""}</button>`;
  }
  function preview(o = {}) {
    return `<aside class="panel panel--strong preview" aria-label="Pré-visualização">
      <div class="row row--gap-3">${W.tile("Sophisticated Backpacks", "xl")}<div><div class="t-display-lg">Sophisticated Backpacks</div><div class="t-xs t-3" style="margin-top:4px">por P3pp3rF1y · 48 mi downloads</div></div></div>
      <p class="t-sm t-2" style="margin-top:12px">Mochilas com melhorias, filtros e muito espaço. Dá para pendurar no cinto e usar sem abrir.</p>
      <div class="stack" style="margin-top:14px">
        ${W.select({ id: "pv-src", label: "Fonte", options: o.onlyModrinth ? ["Modrinth"] : ["Modrinth (recomendada)", "CurseForge"], hint: o.onlyModrinth ? null : "O mesmo arquivo existe nas duas. O Modrinth deixa os jogadores baixarem sozinhos." })}
        ${W.select({ id: "pv-ver", label: "Versão", options: ["3.20.17 (mais nova compatível)", "3.20.16", "3.20.11 (beta)"] })}
        <dl class="kv"><dt>Lado</dt><dd>Cliente e servidor <span class="t-3">(informado pelo Modrinth)</span></dd><dt>Precisa de</dt><dd>Sophisticated Core ${W.tag("Não está no pack", "warn")}</dd></dl>
        ${o.added ? W.alert({ kind: "ok", compact: true, title: "Já no pack.", text: "Adicionado com a Sophisticated Core." }) : B("Adicionar ao pack", "dependencias", { variant: "primary", icon: "plus", block: true })}
        <div class="row row--wrap t-sm">${B("Descrição completa", "", { variant: "link" })}<span class="t-3">·</span>${B("Galeria", "", { variant: "link" })}<span class="t-3">·</span>${B("Página no Modrinth", "", { variant: "link" })}</div></div></aside>`;
  }
  function addContent(state, o = {}) {
    const omni = { empty: "search", link: "link", invalid: "invalid", loading: "loading" }[o.omni] || "search";
    const value = o.omni === "link" ? "https://modrinth.com/mod/sophisticated-backpacks" : state === "vazio" ? "mochila voadora" : "backpack";
    const statusText = o.onlyModrinth || state === "erro" ? "32 resultados do Modrinth para “backpack”" : "48 resultados para “backpack”, do Modrinth e da CurseForge";
    const top = `<div class="add-top">${W.select({ bare: true, ariaLabel: "Tipo", options: ["Tipo: Mods", "Tipo: Resource packs", "Tipo: Shaders"] })}${W.omnibox({ id: "add-q", value, state: state === "carregando" ? "loading" : state === "vazio" ? "search" : omni, statusText: state === "vazio" ? "Nenhum resultado" : statusText })}${B("Escolher arquivo do computador…", "", { icon: "file-plus" })}</div>`;
    const filters = `<div class="filters-line" style="margin-top:4px">${W.icon("lock", "icon--sm")}<span>Só aparece o que funciona em <b class="t-2">Minecraft 1.20.1 com Forge</b>.</span>${B("Mostrar também os sem versão compatível", "", { variant: "link" })}<span aria-hidden="true">·</span>${B("Mais filtros", "", { variant: "link", iconEnd: "chevron-down" })}</div>`;
    const notes = [];
    if (o.onlyModrinth) notes.push(W.alert({ kind: "info", title: "Mostrando só o Modrinth.", text: "Para buscar também na CurseForge, informe sua chave em Configurações. Nenhuma busca vai para a CurseForge sem chave.", actions: B("Abrir Configurações", "config-app", { size: "sm" }) }));
    if (state === "erro") notes.push(W.alert({ kind: "warn", title: "A CurseForge não respondeu.", text: "Mostrando só os resultados do Modrinth. O Warden tenta de novo na próxima busca.", actions: B("Tentar de novo", "adicionar", { size: "sm", icon: "refresh-cw" }) }));
    let main;
    if (state === "carregando") main = `<div class="results" aria-busy="true">${[0, 1, 2, 3].map(() => `<div class="result" aria-hidden="true"><span class="skeleton" style="width:40px;height:40px"></span><span><span class="skeleton skeleton--line" style="--w:45%;height:12px"></span><span class="skeleton skeleton--line" style="--w:80%"></span><span class="skeleton skeleton--line" style="--w:35%"></span></span><span></span></div>`).join("")}</div><div></div>`;
    else if (state === "vazio") main = W.empty({ glyph: "search", artKind: "muted", title: "Nada encontrado para “mochila voadora”", text: "Só aparecem mods que funcionam em Minecraft 1.20.1 com Forge. Tente outro nome, em inglês, ou cole o link do mod.", actions: B("Mostrar também os sem versão compatível", "", { variant: "link" }), compact: true }) + "<div></div>";
    else if (o.omni === "link") main = `<div>${W.alert({ kind: "ok", title: "Link do Modrinth reconhecido.", text: "É o Sophisticated Backpacks, versão 3.20.17 para Minecraft 1.20.1 com Forge. Confira ao lado e adicione." })}</div>${preview()}`;
    else {
      const list = (o.onlyModrinth || state === "erro" ? D.SEARCH.filter((r) => r[5] !== "curseforge") : D.SEARCH).map((r, i) => result(r, i, { selected: 0, onlyModrinth: o.onlyModrinth || state === "erro" })).join("");
      main = `<div class="results" role="group" aria-label="Resultados">${list}<p class="t-xs t-3" style="padding:8px 0">Rolar carrega mais 20.</p></div>${preview({ onlyModrinth: o.onlyModrinth || state === "erro", added: o.added })}`;
    }
    return pageHead("Adicionar ao pack", null, null, { back: ["Voltar para Mods", "mods"] }) + top + filters + (notes.length ? `<div class="stack-2" style="margin-top:12px">${notes.join("")}</div>` : "") + `<div class="add-grid">${main}</div>`;
  }
  const addSim = () => window.P.sim("Ver outros casos:", [["colar um link do Modrinth", "adicionar-link"], ["sem chave da CurseForge", "adicionar-semchave"]]);
  def("adicionar", { group: "pack", title: "Adicionar (busca combinada)", spec: "T08", render: (s) => packShell("mods", addContent(s) + (s === "normal" ? addSim() : "")), states: { carregando: "Buscando", vazio: "Nenhum resultado", erro: "CurseForge fora do ar" } });
  def("adicionar-link", { group: "pack", title: "Adicionar: link colado", spec: "T08", hidden: true, render: () => packShell("mods", addContent("normal", { omni: "link" })) });
  def("adicionar-semchave", { group: "pack", title: "Adicionar sem chave da CurseForge", spec: "T08", hidden: true, render: () => packShell("mods", addContent("normal", { onlyModrinth: true })) });
  def("adicionado", { group: "pack", title: "Adicionado: busca continua aberta", spec: "T08, T09", hidden: true, render: () => packShell("mods", addContent("normal", { added: true }), { unsaved: 7 }),
    after: () => window.WardenUI.toast(W.toast({ kind: "ok", title: "2 itens adicionados ao pack", text: "Sophisticated Backpacks e Sophisticated Core.", actions: B("Desfazer", "adicionar", { size: "sm", variant: "ghost" }) }), 9000) });

  function depsDialog() {
    return W.dialog({ esc: "adicionar", title: "Adicionar Sophisticated Backpacks", sub: "Nada foi gravado ainda. Confira o que vai entrar no pack.", body: `
      <div><div class="t-caps t-3" style="margin-bottom:6px">O que você escolheu</div>${W.check({ checked: true, disabled: true, label: "<b>Sophisticated Backpacks</b> <span class='t-mono t-sm'>3.20.17</span>" })}</div>
      <div><div class="t-caps t-3" style="margin-bottom:6px">Obrigatórias</div>${W.check({ checked: true, disabled: true, label: "<b>Sophisticated Core</b> <span class='t-mono t-sm'>0.6.26</span>", desc: "Sem ela o jogo não abre. Não dá para desmarcar." })}</div>
      <div><div class="t-caps t-3" style="margin-bottom:6px">Opcionais</div>${W.check({ label: "Sophisticated Backpacks Create Integration", desc: "Mochilas que se ligam às máquinas do Create." })}</div>
      ${W.alert({ kind: "ok", compact: true, title: "Nenhuma incompatibilidade declarada", text: "com os 128 itens que já estão no pack." })}`,
      foot: B("Cancelar", "adicionar", { variant: "ghost" }) + B("Adicionar 2 itens", "adicionado", { variant: "primary", icon: "plus" }) });
  }
  def("dependencias", { group: "pack", title: "Dependências ao adicionar (diálogo)", spec: "T09", render: () => packShell("mods", addContent("normal"), { overlay: depsDialog() }) });

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
      : `<button type="button" role="treeitem" class="tree__item" aria-expanded="true">${W.icon("folder-open")}config/ <span class="t-3">(86)</span></button><div role="group"><button type="button" role="treeitem" class="tree__item ${o.form ? "" : "tree__item--dirty"}" aria-selected="true">${W.icon("file-text")}create-common.toml</button><button type="button" role="treeitem" class="tree__item">${W.icon("file-text")}embeddium-options.json</button><button type="button" role="treeitem" class="tree__item">${W.icon("file-text")}waystones-common.toml</button><button type="button" role="treeitem" class="tree__item">${W.icon("file-text")}xaerominimap.txt</button><button type="button" role="treeitem" class="tree__item" aria-expanded="false">${W.icon("folder")}jei/</button><div role="group" hidden></div><span class="tree__item t-3" style="cursor:default">… mais 81 arquivos</span></div>
         <button type="button" role="treeitem" class="tree__item" aria-expanded="false">${W.icon("folder")}defaultconfigs/ <span class="t-3">(2)</span></button><div role="group" hidden></div><button type="button" role="treeitem" class="tree__item" aria-expanded="false">${W.icon("folder")}kubejs/</button><div role="group" hidden></div><button type="button" role="treeitem" class="tree__item tree__item--friendly">${W.icon("file-text")}Opções do jogo (options.txt)</button>`;
    const head = pageHead("Configs", "Arquivos de ajuste dos mods e do jogo. Comentários e formatação são preservados.");
    if (state === "vazio") return head + W.empty({ glyph: "dots", title: "Este pack ainda não tem configs", text: "Os mods criam os arquivos de config na primeira vez que o jogo abre. Teste o pack uma vez e eles aparecem aqui.", actions: B("Testar", "teste-checagem", { variant: "primary", icon: "play" }) });
    const instBanner = inst ? `<div style="margin-bottom:12px">${W.alert({ kind: "warn", title: "Você está editando a instância de teste, não o pack.", text: "Vale só para este teste. Quando o jogo fechar, a mudança aparece em “O que mudou durante o teste” e você decide se traz para o pack.", actions: B("Voltar ao teste", "teste-jogo", { size: "sm", icon: "terminal" }) })}</div>` : "";
    const ext = state === "erro" ? `<div style="margin-bottom:12px">${W.alert({ kind: "danger", title: "Este arquivo foi alterado fora do Warden.", text: "Outro programa mudou config/create-common.toml depois que você abriu. Escolha o que fazer antes de salvar.", actions: B("Recarregar", "configs", { size: "sm" }) + B("Ver diferenças", "configs-diff", { size: "sm" }) + B("Sobrescrever", "", { size: "sm", variant: "danger-ghost" }) })}</div>` : "";
    const file = inst ? "config/embeddium-options.json" : "config/create-common.toml";
    const lines = inst ? ["{", '  "quality": {', '    "weather_quality": "FANCY",', '    "leaves_quality": "FAST",', '    "enable_vignette": false', "  },", '  "performance": {', '    "chunk_builder_threads": 4', "  }", "}"] : D.CONFIG_FILE;
    const changed = inst ? 7 : 9;
    let editorBody;
    if (state === "carregando") editorBody = `<div class="editor__lines" aria-busy="true" style="padding:16px">${[60, 40, 75, 30, 55, 68, 20, 50].map((w) => `<span class="skeleton skeleton--line" style="--w:${w}%"></span>`).join("")}</div>`;
    else if (o.form) {
      const row = (key, desc, ctl, ch) => `<div class="cfgform__row ${ch ? "cfgform__row--changed" : ""}"><div><div class="cfgform__key">${key}</div><div class="cfgform__desc">${desc}</div></div>${ctl}<div>${ch ? B("Desfazer " + key, "", { variant: "ghost", size: "sm", iconOnly: true, icon: "undo-2" }) : ""}</div></div>`;
      editorBody = `<div class="editor__lines" style="font-family:var(--font-ui)"><div class="cfgform"><div class="cfgform__group">[worldgen]</div>${row("disableWorldGen", "Desliga a geração de minérios do Create. Padrão: false.", W.switchCtl({ label: "disableWorldGen", checked: false }).replace('<label', '<label class="sr-only"'), false)}
        <div class="cfgform__group">[kinetics]</div>${row("maxRotationSpeed", "Velocidade máxima de rotação. Faixa: acima de 64. Padrão: 256.", W.input({ bare: true, value: "512", size: "sm", ariaLabel: "maxRotationSpeed" }), true)}${row("stressMultiplier", "Multiplicador de stress. Faixa: 0,0 a 10,0. Padrão: 1,0.", W.input({ bare: true, value: "1.0", size: "sm", ariaLabel: "stressMultiplier" }), false)}
        <div class="cfgform__group">[fluids]</div>${row("mechanicalPumpRange", "Quantos blocos o cano leva o líquido. Faixa: 1 a 256. Padrão: 16.", W.input({ bare: true, value: "16", size: "sm", ariaLabel: "mechanicalPumpRange" }), false)}</div></div>`;
    } else editorBody = `<div class="editor__lines" role="textbox" aria-multiline="true" aria-label="Conteúdo de ${file}" tabindex="0">${lines.map((l, i) => `<div class="editor__line ${i === changed ? "editor__line--changed" : ""}">${highlight(l) || " "}</div>`).join("")}</div>`;
    const kindTag = inst ? W.tag("Instância de teste", "warn", { icon: "terminal" }) : W.tag("Config do pack", "plain", { title: "Sempre substitui a do jogador ao atualizar" });
    const editor = `<section class="editor" aria-label="Editor"><div class="editor__bar"><span class="editor__file">${file}</span>${kindTag}<span class="grow"></span>
        ${W.segmented([["form", "Formulário"], ["text", "Texto"]], o.form ? "form" : "text", "Modo do editor")}${B("Buscar e substituir", "", { size: "sm", variant: "ghost", icon: "search" })}${B("Salvar", inst ? "teste-jogo" : o.form ? "configs-diff" : "configs-diff", { size: "sm", variant: "primary", icon: "save" })}</div>
      ${inst || o.form ? "" : `<div style="padding:8px 12px;border-bottom:1px solid var(--color-border)" class="t-xs t-3">${W.icon("info", "icon--sm")} Arquivo do Forge: o jogo pode reescrever este arquivo e apagar comentários que você adicionar.</div>`}
      ${editorBody}<div class="editor__foot"><span class="t-warn">1 alteração não salva neste arquivo</span><span class="grow"></span><span>${inst ? "JSON" : "TOML"} · UTF-8 · linha ${changed + 1}, coluna 24</span></div></section>`;
    return head + instBanner + ext + `<div class="cfg"><div class="cfg__side">${W.select({ bare: true, ariaLabel: "Origem dos arquivos", options: inst ? ["Mostrando: instância de teste (jogo aberto)", "Mostrando: arquivos do pack"] : ["Mostrando: arquivos do pack", "Mostrando: instância de teste"] })}${W.input({ bare: true, icon: "search", placeholder: "Buscar arquivo", ariaLabel: "Buscar arquivo de config", size: "sm" })}<div role="tree" aria-label="Arquivos" class="tree">${tree}</div></div>${editor}</div>`;
  }
  window.P.configsContent = configsContent;
  def("configs", { group: "pack", title: "Configs (editor em texto)", spec: "T12", render: (s) => packShell("configs", configsContent(s)), states: { carregando: "Abrindo o arquivo", vazio: "Pack nunca testado: sem configs", erro: "Arquivo alterado fora do Warden" },
    after: () => document.querySelector('.segmented [data-value="form"]')?.setAttribute("data-go", "configs-formulario") });
  def("configs-formulario", { group: "pack", title: "Configs (modo formulário)", spec: "T12", hidden: true, render: () => packShell("configs", configsContent("normal", { form: true })),
    after: () => document.querySelector('.segmented [data-value="text"]')?.setAttribute("data-go", "configs") });
  function cfgDiffDialog() {
    return W.dialog({ esc: "configs", size: "lg", title: "Salvar config/create-common.toml?", sub: "Só esta linha muda. O resto do arquivo fica igual, inclusive os comentários.",
      body: W.diff({ file: "config/create-common.toml", lines: [["fold", "", "", "… 6 linhas iguais"], ["ctx", 7, 7, "[kinetics]"], ["ctx", 8, 8, "\t#Velocidade máxima de rotação"], ["ctx", 9, 9, "\t#Range: > 64"], ["del", 10, "", "\tmaxRotationSpeed = 128"], ["add", "", 10, "\tmaxRotationSpeed = 256"], ["ctx", 11, 11, "\t#Multiplicador de stress"], ["fold", "", "", "… 7 linhas iguais"]] }) + `<p class="t-sm t-3">Dá para desligar esta confirmação em Configurações.</p>`,
      foot: B("Continuar editando", "configs", { variant: "ghost" }) + B("Salvar", "configs", { variant: "primary", icon: "save" }) });
  }
  def("configs-diff", { group: "pack", title: "Salvar config: diferenças", spec: "T12", hidden: true, render: () => packShell("configs", configsContent("normal"), { overlay: cfgDiffDialog() }) });

  // ======================= PROBLEMAS =======================
  function problemsContent(state) {
    if (state === "vazio") return pageHead("Problemas", "Última verificação há 3 minutos.", B("Verificar agora", "problemas~carregando", { icon: "refresh-cw" })) + W.empty({ kind: "ok", glyph: "check", title: "Nenhum problema encontrado", text: "Dependências, versões, Java, duplicatas e conflitos conhecidos foram conferidos nos 128 itens. A verificação também roda sozinha ao clicar em Testar e antes de exportar." });
    if (state === "carregando") return pageHead("Problemas", "Verificando…", W.btn("Verificando…", { busy: true, disabled: true })) + `<div class="panel" style="max-width:720px">${W.progress({ label: "Conferindo 128 itens", value: 64, meta: "Dependências, versões, Java, duplicatas e conflitos conhecidos" })}</div>`;
    return pageHead("Problemas", "3 encontrados · última verificação há 3 minutos. Também roda sozinha ao clicar em Testar e antes de exportar.", B("Verificar agora", "problemas~carregando", { icon: "refresh-cw" })) +
      `<h2 class="group-title" style="margin-top:0">Erros <span class="t-3">impedem o teste, a menos que você escolha testar mesmo assim</span></h2>
      ${W.issue({ kind: "danger", id: "pb-1", title: "O Waystones precisa do Balm, que não está no pack", text: "Sem o Balm, o jogo para na tela de carregamento.", evidence: "O arquivo META-INF/mods.toml do Waystones 14.1.6 declara a dependência obrigatória “balm” na faixa [7.3.0,).", actions: B("Adicionar Balm", "dependencias", { variant: "primary", size: "sm", icon: "plus" }) + B("Ver Waystones", "mods", { variant: "ghost", size: "sm" }) + B("Ignorar neste pack", "", { variant: "ghost", size: "sm" }) + W.badge("p1", "P1") })}
      ${W.issue({ kind: "danger", id: "pb-2", title: "Dois mods de renderização no pack: Embeddium e Rubidium", text: "Os dois fazem a mesma coisa e travam o jogo quando estão juntos. Mantenha só um. O Embeddium é o mais novo.", evidence: "Lista de conflitos conhecidos do Warden, categoria “renderizador”.", actions: B("Remover Rubidium", "", { size: "sm", icon: "trash-2" }) + B("Remover Embeddium", "", { size: "sm", variant: "ghost" }) })}
      <h2 class="group-title">Avisos</h2>
      ${W.issue({ kind: "warn", id: "pb-3", title: "Xaero's Minimap está como Cliente e servidor, mas só funciona no cliente", text: "Num servidor, ele impede o servidor de abrir.", evidence: "O Modrinth informa client_side: required, server_side: unsupported.", actions: B("Mudar lado para Só cliente", "", { size: "sm" }) })}
      <h2 class="group-title">Último travamento</h2>
      <div class="panel row row--between row--wrap"><span class="t-sm">${W.status("danger", "Teste de hoje às 14:40 travou com 48 s de jogo.")} <span class="t-3">A verificação achou a causa: falta o Balm.</span></span><span class="row">${B("Ver o que causou", "teste-travou", { size: "sm" })}${B("Analisar com IA", "ia", { size: "sm", variant: "ghost", icon: "sparkles", iconCls: "icon--ai" })}</span></div>`;
  }
  def("problemas", { group: "pack", title: "Problemas", spec: "T14", render: (s) => packShell("problemas", problemsContent(s), s === "vazio" ? { problems: 0 } : {}), states: { carregando: "Verificando agora", vazio: "Nenhum problema" } });

  // ======================= DIAGNÓSTICO COM IA =======================
  function iaContent(state) {
    const head = pageHead(`${W.icon("sparkles", "icon--lg icon--ai")} Diagnóstico com IA`, "A IA (Google Gemini) lê os logs e sugere a causa de um travamento. Antes de enviar, você vê exatamente o que vai.");
    if (state === "vazio") return head + W.empty({ artKind: "ai", glyph: "dots", title: "Falta a chave do Gemini", text: "Para usar a IA, informe a sua chave do Gemini em Configurações. A chave é sua: o Google pode cobrar pelo uso, conforme o seu plano.", actions: B("Abrir Configurações", "config-app", { variant: "primary", icon: "key-round" }) + B("Como conseguir uma chave", "", { variant: "link" }) });
    return head + `<section class="panel panel--strong" aria-labelledby="ia-what"><h2 class="panel__title panel__title--sans" id="ia-what">O que analisar</h2><fieldset style="border:0;padding:0;margin:12px 0 0"><legend class="sr-only">O que analisar</legend><div class="choice-list">
        ${W.choice({ name: "ia-src", title: "Último travamento", desc: "Hoje às 14:40, 48 s de jogo. A verificação automática achou: falta o mod Balm.", checked: true })}
        ${W.choice({ name: "ia-src", title: "Outro teste", desc: "Escolha um dos testes guardados.", extra: `<div style="margin-top:6px;max-width:320px">${W.select({ bare: true, size: "sm", ariaLabel: "Teste", options: ["30/09 às 21:14 · travou", "28/09 às 19:02 · travou"] })}</div>` })}
        ${W.choice({ name: "ia-src", title: "Um log ou crash report do computador", desc: "Por exemplo, um arquivo que um jogador mandou.", extra: `<div style="margin-top:6px">${B("Escolher arquivo…", "", { size: "sm", icon: "file-plus" })}</div>` })}</div></fieldset>
      <div style="margin-top:14px">${W.textarea({ id: "ia-note", label: "Contar algo à IA", optional: true, placeholder: "Ex.: trava quando entro no Nether", rows: 2 })}</div>
      <div class="row row--gap-3" style="margin-top:14px">${B("Preparar envio", "ia-consent", { variant: "primary", icon: "sparkles" })}<span class="t-sm t-3">Próximo passo: ver o texto exato e confirmar. Nada é enviado antes disso.</span></div></section>
      <h2 class="group-title">Respostas anteriores</h2>
      <div class="tablewrap"><table class="table"><thead><tr><th>Quando</th><th>O que foi analisado</th><th>Causa provável</th><th>Confiança</th><th class="shrink"><span class="sr-only">Abrir</span></th></tr></thead><tbody>
        <tr><td>30/09 21:20</td><td>Teste de 30/09 às 21:14</td><td>Embeddium e Rubidium juntos</td><td>${W.meter("média")}</td><td class="shrink">${B("Ver resposta", "ia-resposta", { size: "sm", variant: "ghost" })}</td></tr>
        <tr><td>22/09 18:02</td><td class="path">crash-2026-09-22.txt</td><td>Falta de memória</td><td>${W.meter("alta")}</td><td class="shrink">${B("Ver resposta", "", { size: "sm", variant: "ghost" })}</td></tr></tbody></table></div>
      <p class="t-xs t-3" style="margin-top:8px">As respostas ficam nos dados do Warden, neste computador. Nunca vão para a pasta do pack nem para o GitHub.</p>
      <div class="panel row row--between" style="margin-top:16px"><span class="t-sm">${W.status("ok", "Chave do Gemini configurada")} <span class="t-3">· modelo gemini-2.5-flash</span></span>${B("Trocar em Configurações", "config-app", { variant: "link" })}</div>`;
  }
  function consentDialog(cancel) {
    return W.dialog({ esc: cancel, size: "lg", ai: true, title: "Enviar o log para a IA?", sub: "Este é o texto exato que será enviado. Nome de usuário do Windows, nome do jogador, IPs, e-mails e chaves foram trocados por marcadores.",
      body: `<pre class="code code--scroll" tabindex="0" aria-label="Texto que será enviado">Minecraft 1.20.1 · Forge 47.3.0 · Java 17.0.12 · 126 mods
Verificação automática: falta o mod Balm (exigido pelo Waystones)
[14:40:31] [main/ERROR] [fml/ModLoader]: Missing or unsupported mandatory dependencies:
	Mod ID: 'balm', Requested by: 'waystones', Expected range: '[7.3.0,)', Actual version: '[MISSING]'
Caminho: C:\\Users\\<mark>[usuário]</mark>\\AppData\\Roaming\\dev.kriticales.warden\\instances\\vale-sereno
--username <mark>[jogador]</mark> --uuid <mark>[uuid]</mark>
…(mais 412 linhas)</pre>
      <dl class="kv"><dt>Tamanho</dt><dd>18 KB</dd><dt>Para</dt><dd>Google Gemini (gemini-2.5-flash), com a sua chave</dd><dt>Custo</dt><dd>Pode ter limite e custo, conforme o seu plano. No plano gratuito, o Google pode usar o conteúdo para melhorar os produtos dele.</dd></dl>`,
      foot: B("Cancelar", cancel, { variant: "ghost" }) + B("Enviar", "ia-carregando", { variant: "primary", icon: "sparkles" }) });
  }
  window.P.consentDialog = consentDialog;
  def("ia", { group: "pack", title: "Diagnóstico com IA", spec: "T14", render: (s) => packShell("ia", iaContent(s)), states: { vazio: "Sem chave do Gemini" } });
  def("ia-consent", { group: "pack", title: "IA: consentimento (pela seção)", spec: "T14", hidden: true, render: () => packShell("ia", iaContent("normal"), { overlay: consentDialog("ia") }) });
  function answer(state) {
    const head = pageHead("Resposta da IA", state === "carregando" ? "Último travamento · enviado agora · Google Gemini (gemini-2.5-flash)" : "Teste de 30/09 às 21:14 · enviado às 21:20 · Google Gemini (gemini-2.5-flash)", null, { back: ["Voltar ao Diagnóstico com IA", "ia"] });
    if (state === "carregando") return head + `<div style="max-width:880px">${W.aiBlock({ loading: true, meta: "Google Gemini" })}</div>`;
    if (state === "erro") return head + `<div style="max-width:880px">${W.aiBlock({ error: "A IA não respondeu a tempo.", errorText: "O Google demorou mais de 60 segundos. Nada foi alterado no pack e o texto enviado continua guardado.", meta: "21:20", actions: B("Tentar de novo", "ia-carregando", { size: "sm", icon: "refresh-cw" }) })}</div>`;
    return head + `<div style="max-width:880px">${W.aiBlock({ meta: "gemini-2.5-flash · 21:20", cause: "Dois mods de renderização, Embeddium e Rubidium, carregados juntos. O log mostra os dois tentando substituir o mesmo código de desenho dos blocos logo depois de abrir o mundo.", confidence: "média",
      mods: [B("Embeddium", "mods", { variant: "link" }), B("Rubidium", "mods", { variant: "link" })],
      steps: [["Remover o Rubidium. O Embeddium é o substituto mais novo e já está no pack.", B("Remover Rubidium", "", { size: "sm", variant: "danger-ghost", icon: "trash-2" })], ["Testar de novo.", B("Testar", "teste-checagem", { size: "sm", icon: "play" })]] })}
      <p class="t-xs t-3" style="margin-top:8px">A verificação automática do Warden também aponta esse conflito em Problemas.</p></div>`;
  }
  def("ia-carregando", { group: "pack", title: "IA: esperando a resposta", spec: "T14", hidden: true, render: () => packShell("ia", answer("carregando")) });
  def("ia-resposta", { group: "pack", title: "Resposta da IA", spec: "T14", render: (s) => packShell("ia", answer(s)), states: { carregando: "Esperando a IA", erro: "A IA não respondeu" } });

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
      <details class="issue__evidence"><summary>Como o número é escolhido</summary><ul class="stack-2 t-sm" style="margin-top:8px"><li><span class="t-mono">1.4.2 → 1.4.3</span> só atualizações e ajustes de config</li><li><span class="t-mono">1.4.2 → 1.5.0</span> mods novos ou mudanças de equilíbrio</li><li><span class="t-mono">1.4.2 → 2.0.0</span> saiu um mod com blocos nos mundos: os mundos dos jogadores podem perder coisas</li></ul></details>
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
  function exportContent(state) {
    const head = pageHead("Exportar", "Gera o pack para quem vai jogar, só com o necessário. O pack no Warden não muda.");
    const checks = `<section class="panel" aria-labelledby="ex-1"><h2 class="panel__title panel__title--sans" id="ex-1">1. Antes de exportar</h2><ul class="checklist" style="margin-top:10px">
      <li class="is-ok">${W.icon("circle-check")}<span>Nenhum arquivo que não deveria ir para quem joga (registros, cópias de segurança, caches).</span></li>
      <li class="is-bad">${W.icon("circle-x")}<span>O pack tem 2 erros. ${B("Ver problemas", "problemas", { variant: "link" })}</span></li>
      <li class="is-warn">${W.icon("triangle-alert")}<span>Há 5 alterações não salvas: a exportação usa o estado de agora. ${B("Salvar versão antes", "salvar", { variant: "link" })}</span></li></ul></section>`;
    const fmt = `<section class="panel" aria-labelledby="ex-2"><h2 class="panel__title panel__title--sans" id="ex-2">2. Formato</h2><fieldset style="border:0;padding:0;margin:10px 0 0"><legend class="sr-only">Formato</legend><div class="choice-list choice-list--2">
      ${W.choice({ name: "ex-f", title: "Pasta packwiz", desc: "Para hospedar em outro lugar. Para o GitHub, use Publicar versão no Histórico.", checked: true })}
      ${W.choice({ name: "ex-f", title: "Arquivo .zip do pack packwiz", desc: "A mesma coisa, num arquivo só." })}
      ${W.choice({ name: "ex-f", title: ".mrpack", badge: " " + W.badge("p1", "P1"), desc: "Para o app do Modrinth e launchers compatíveis." })}
      ${W.choice({ name: "ex-f", title: ".zip da CurseForge", badge: " " + W.badge("p1", "P1"), desc: "Para o app da CurseForge. Não guarda o lado dos mods." })}</div></fieldset></section>`;
    const what = `<section class="panel" aria-labelledby="ex-3"><h2 class="panel__title panel__title--sans" id="ex-3">3. O que vai no pack</h2><div class="export-tree" style="margin-top:10px">
      <b>pack.toml</b> · <b>index.toml</b><br>${W.icon("chevron-right", "icon--sm")} <b>mods/</b> 124 referências <span class="t-3">(os .jar não vão: quem joga baixa)</span><br>${W.icon("chevron-right", "icon--sm")} <b>resourcepacks/</b> 3 referências · <b>shaderpacks/</b> 1 referência<br>
      ${W.icon("chevron-right", "icon--sm")} <b>config/</b> 86 arquivos, 412 KB<br>${W.icon("chevron-right", "icon--sm")} <b>defaultconfigs/</b> 2 arquivos<br><b>options.txt</b> ${W.tag("Substitui as preferências de quem já joga", "warn")} ${B("Não substituir se o jogador já tiver", "", { variant: "link" })} ${W.badge("p1", "P1")}</div>
      <p class="t-sm" style="margin-top:10px">Algo aqui não deveria ir? ${B("Excluir do pack…", "", { variant: "link" })}</p></section>`;
    let foot;
    if (state === "carregando") foot = `<div class="panel" style="margin-top:16px">${W.progress({ label: "Exportando", value: 72, meta: "156 de 216 arquivos · conferindo com o packwiz no fim" })}<div style="margin-top:10px">${B("Cancelar", "exportar", { variant: "ghost", size: "sm" })}</div></div>`;
    else foot = `${state === "erro" ? `<div style="margin-top:16px">${W.alert({ kind: "danger", title: "Não foi possível exportar: o disco D: está cheio.", text: "Faltam 1,2 MB. Nada foi gravado pela metade. Libere espaço ou escolha outra pasta.", actions: B("Escolher outra pasta", "", { size: "sm", icon: "folder-open" }) })}</div>` : ""}
      <div class="export-foot"><div class="field grow"><label class="field__label" for="ex-dir">Pasta de destino</label>${W.input({ id: "ex-dir", bare: true, value: state === "erro" ? "D:\\Exportados\\vale-sereno-1.4.2" : "Documents\\Warden\\exportados\\vale-sereno-1.4.2", mono: true, attrs: state === "erro" ? { "aria-invalid": "true" } : {} })}</div>${B("Escolher pasta…", "", { icon: "folder-open" })}${B("Exportar", "exportado", { variant: "primary", size: "lg", icon: "package" })}</div>`;
    return head + `<div class="stack" style="max-width:920px">${checks}${fmt}${what}</div><div style="max-width:920px">${foot}</div>`;
  }
  def("exportar", { group: "pack", title: "Exportar", spec: "T19", render: (s) => packShell("exportar", exportContent(s)), states: { carregando: "Exportando", erro: "Disco cheio" } });
  def("exportado", { group: "pack", title: "Exportado", spec: "T19", hidden: true, render: () => packShell("exportar", exportContent("normal"), { overlay: W.dialog({ esc: "exportar", size: "sm", title: "Pack exportado", body: `<p>216 arquivos, 1,2 MB, em <span class="path">Documents\\Warden\\exportados\\vale-sereno-1.4.2</span>.</p>${W.alert({ kind: "ok", compact: true, title: "Conferido com o packwiz.", text: "O pack exportado é exatamente o que o packwiz geraria." })}`, foot: B("Fechar", "exportar", { variant: "ghost" }) + B("Abrir pasta", "exportar", { variant: "primary", icon: "folder-open" }) }) }) });

  // ======================= INFORMAÇÕES DO PACK =======================
  def("info", { group: "pack", title: "Informações do pack (diálogo)", spec: "T11", render: () => packShell("mods", modsContent("normal"), {
    overlay: W.dialog({ esc: "mods", title: "Informações do pack", body: `${W.input({ id: "if-name", label: "Nome", value: "Vale Sereno" })}${W.input({ id: "if-author", label: "Autor", value: "Kriticales" })}${W.textarea({ id: "if-desc", label: "Descrição", optional: true, placeholder: "Uma frase sobre o pack", rows: 2 })}
      ${W.select({ id: "if-forge", label: "Versão do Forge", options: ["47.3.0 (recomendada)", "47.2.20", "47.1.106"], hint: "Minecraft e loader não mudam aqui: mudar a versão do Minecraft é outro pack." })}
      <hr class="sep" /><div class="btn-row">${B("Mostrar pasta do pack", "", { size: "sm", icon: "folder-open" })}${B("Apagar pack…", "", { size: "sm", variant: "danger-ghost", icon: "trash-2" })}${W.badge("p1", "P1")}</div>`,
      foot: B("Cancelar", "mods", { variant: "ghost" }) + B("Salvar", "mods", { variant: "primary" }) }) }) });
})();
