/* ==========================================================================
   Warden — protótipo final: núcleo (visualizador, rotas, cascas)
   Telas em screens-app.js, screens-pack.js e screens-test.js.
   Tudo é montado com design/system/components.js (W) e behavior.js (WardenUI).
   ========================================================================== */
(function () {
  "use strict";
  const W = window.W, D = window.DATA;
  const SCREENS = {};
  const ORDER = [];
  const GROUPS = { app: "Nível do app", pack: "Nível do pack", teste: "Testar" };
  const STATE_LABELS = { normal: "Normal", carregando: "Carregando", vazio: "Vazio", erro: "Erro", conflito: "Conflito", semcausa: "Sem conclusão",
    agrupado: "Agrupado por mod", memoria: "Memória alta", semdados: "Sem leitura", desempenho: "Desempenho", pausada: "Pausada", assistido: "Assistido",
    cancelada: "Cancelada", aplicada: "Aplicada", mudou: "Pack mudou", antiga: "Versão antiga", pouca: "Pouca memória", jogo: "Console do jogo", jogoaberto: "Com jogo", servidor: "Com servidor", semchave: "Sem chave", todas: "Todas as alterações", velho: "Versão antiga",
    suspeito: "Sinal conhecido", naoconfere: "Não confere", bloqueado: "Bloqueado", proxima: "Versão próxima", desconhecido: "Pack desconhecido", incompleto: "Log incompleto",
    unificado: "Já unificado", poucos: "Poucos testes", perfil: "Outro perfil", pesada: "Versão mais pesada", grupos: "Agrupado por grupo", semgrupo: "Sem grupos", sinais: "Lista antiga", confiado: "Confiado", removido: "Arquivo removido" };

  // ---------- Registro ----------
  // o: { group, title, spec, render(state), states: { vazio: "descrição", ... }, hidden, note, d4 }
  // d4: tela ou estado novo da tarefa D4 (funções avançadas); aparece marcado no seletor e no mapa.
  // v11: tela nova do Warden 1.1 "Profissional" (tarefa D5); aparece marcada como (1.1).
  function def(id, o) { SCREENS[id] = Object.assign({ id, states: {} }, o); ORDER.push(id); }

  // ---------- Atalhos de componentes com navegação ----------
  const go = (id, extra) => Object.assign({ "data-go": id || "" }, extra || {});
  function B(label, target, o = {}) { return W.btn(label, Object.assign({}, o, { attrs: Object.assign(go(target), o.attrs || {}) })); }

  // ---------- Cascas ----------
  function tasksText(t) { return t || { state: "idle", text: "Nenhuma tarefa em andamento", attrs: go("tarefas") }; }
  function appShell(content, o = {}) {
    return `<div class="app">${W.topbar({ back: o.back ? [o.back[0], go(o.back[1])] : null, where: o.where, noSettings: o.noSettings, settingsAttrs: go("config-app") })}
      <div class="app__body"><main class="app__main ${o.narrow ? "app__main--narrow" : ""}" id="conteudo"><div class="content">${content}</div></main></div>
      ${W.statusbar({ tasks: Object.assign(tasksText(o.tasks), { attrs: go(o.tasksGo || "tarefas") }) })}</div>${o.overlay || ""}`;
  }

  const SECTIONS = [
    { id: "mods", name: "Mods", desc: "Mods, resource packs e shaders", icon: "puzzle" },
    { id: "configs", name: "Configs", desc: "Arquivos de ajuste e scripts do pack", icon: "file-code" },
    { id: "problemas", name: "Problemas", desc: "Saúde do pack, problemas e travamentos", icon: "triangle-alert" },
    { id: "ia", name: "Diagnóstico com IA", desc: "Conversar com a IA sobre um problema do pack", icon: "sparkles", ai: true },
    { id: "historico", name: "Histórico", desc: "Versões salvas e publicação", icon: "history" },
    { id: "exportar", name: "Exportar", desc: "Gerar o pack para quem vai jogar", icon: "package" },
  ];
  // section: id da seção acesa ("teste" não acende nenhuma)
  // o.compact: menu lateral recolhido (página de descoberta). o.profile: perfil do teste ativo (≠ Padrão).
  // o.game: ready | preparing | running | bisect
  function testMenu(game, testTarget, o = {}) {
    const p1 = W.badge("p1", "P1");
    const prof = o.profile || "Padrão";
    return W.menu([
      { label: "Ver último teste", desc: "Hoje, 14:40 · travou", icon: "history", attrs: go("teste-travou") },
      { group: "Outros testes" },
      { label: "Testar como o jogador recebe", desc: "Instala pelo link do pack, como um jogador", icon: "user", end: p1, attrs: go("") },
      { label: "Testar como servidor…", desc: "Abre um servidor só neste computador e entra nele", icon: "server", end: p1, attrs: go("servidor-eula") },
      { label: "Testar com perfil de desempenho", desc: "Mede o que mais pesa para carregar e rodar", icon: "gauge", end: p1, attrs: go("teste-fechou~desempenho") },
      { label: "Encontrar o mod culpado…", desc: "Abre o jogo em rodadas até achar o mod", icon: "target", end: p1, attrs: go("culpado-config") },
      { group: "Perfil do teste" },
      ...D.PROFILES.map(([id, name, desc]) => ({ radio: "perfil", label: name, checked: name === prof, end: `<span class="t-xs t-3">${desc}</span>`, attrs: go(id === "fraco" ? "perfil-ativo" : id === "padrao" ? "mods" : "") })),
      { label: "Ajustes do teste neste computador…", desc: "Memória, Java, mundo e perfis. Não entram no pack", icon: "settings", attrs: go("ajustes-teste") },
      { group: "Instância de teste" },
      { label: "Abrir pasta da instância de teste", icon: "folder-open", attrs: go("") },
      { label: "Apagar mundos de teste…", icon: "trash-2", attrs: go("") },
      { label: "Recriar instância de teste…", icon: "rotate-ccw", attrs: go("") },
    ], { id: "menu-testar", label: "Mais opções do teste", style: "right:20px;top:72px" });
  }
  function packShell(section, content, o = {}) {
    const unsaved = o.unsaved ?? D.PACK.unsaved;
    const problems = o.problems ?? 6;
    const counts = {
      mods: { count: o.empty ? "" : 128, countLabel: "128 itens" },
      problemas: problems ? { count: problems, countKind: o.problemsKind || "danger", countLabel: `${problems} problemas` } : {},
      historico: unsaved ? { count: unsaved, countKind: "warn", countLabel: `${unsaved} alterações não salvas` } : {},
    };
    const items = SECTIONS.map((s) => Object.assign({}, s, counts[s.id] || {}, { href: "#" + s.id }));
    const game = o.game || "ready";
    const testTarget = game === "running" ? (o.runTarget || "teste-jogo") : game === "bisect" ? "culpado" : game === "preparing" ? (o.prepTarget || "teste-preparo") : "teste-checagem";
    const header = W.packHeader({
      name: D.PACK.name, mc: D.PACK.mc, loader: D.PACK.loader, version: o.version || D.PACK.version, unsaved,
      alerts: o.alerts || [], backAttrs: go("packs"), editAttrs: go("info"), saveAttrs: go("salvar"), menuId: "menu-testar",
      test: { state: game, progress: o.progress, profile: o.profile, open: !!o.menuOpen, mainAttrs: go(testTarget), moreAttrs: {} },
    });
    return `<div class="app app--pack">${header.replace(/<\/header>\s*$/, testMenu(game, testTarget, o) + "</header>")}
      <div class="packbody">${W.sectionMenu(items, section, { compact: o.compact })}
        <main class="app__main ${o.mainCls || ""}" id="conteudo"><div class="content">${content}</div></main></div>
      ${W.statusbar({ tasks: Object.assign(tasksText(o.tasks), { attrs: go(o.tasksGo || "tarefas-pack") }) })}</div>${o.overlay || ""}`;
  }
  // Cabeçalho de página dentro das cascas
  function pageHead(title, sub, actions, o = {}) {
    return `<div class="pagehead"><div>${o.back ? `<div class="back-link">${B(o.back[0], o.back[1], { variant: "ghost", size: "sm", icon: "arrow-left" })}</div>` : ""}
      <h1 class="pagehead__title ${o.sans ? "pagehead__title--sans" : ""}" tabindex="-1" data-title>${title}</h1>${sub ? `<p class="pagehead__sub">${sub}</p>` : ""}</div>
      ${actions ? `<div class="pagehead__actions">${actions}</div>` : ""}</div>`;
  }

  // Caixa de simulação: controle do protótipo dentro da tela (não existe no app).
  const sim = (label, links) => `<div class="proto-sim" role="note"><span class="proto-sim__tag">Protótipo</span><span>${label}</span>${links.map(([t, g]) => `<button type="button" data-go="${g}">${t}</button>`).join("")}</div>`;

  // ---------- Visualizador ----------
  const stage = () => document.getElementById("stage");
  function parseHash() {
    const raw = decodeURIComponent(location.hash.replace(/^#/, ""));
    const [id, state] = raw.split("~");
    return { id: SCREENS[id] ? id : "packs", state: state || "normal" };
  }
  function navigate(id, state) { location.hash = id + (state && state !== "normal" ? "~" + state : ""); }

  function protobar(cur) {
    const opts = Object.keys(GROUPS).map((g) => `<optgroup label="${GROUPS[g]}">${ORDER.filter((i) => SCREENS[i].group === g).map((i) => `<option value="${i}"${i === cur.id ? " selected" : ""}>${SCREENS[i].hidden ? "· " : ""}${SCREENS[i].title}${SCREENS[i].v11 ? " (1.1)" : SCREENS[i].d4 ? " (D4)" : ""}</option>`).join("")}</optgroup>`).join("");
    const s = SCREENS[cur.id];
    const st = ["normal", ...Object.keys(s.states)];
    const seg = st.length > 1 ? `<span class="proto-bar__lbl" id="pb-st">Estado</span><div class="segmented" role="radiogroup" aria-labelledby="pb-st">${st.map((k) => `<button type="button" role="radio" aria-checked="${k === cur.state}" tabindex="${k === cur.state ? 0 : -1}" data-state="${k}" title="${W.esc(s.states[k] || "")}">${STATE_LABELS[k] || k}</button>`).join("")}</div>` : `<span class="proto-bar__lbl t-3">Esta tela não tem outros estados</span>`;
    return `<div class="proto-bar" role="region" aria-label="Controles do protótipo (não fazem parte do app)"><span class="proto-bar__tag">Protótipo</span>
      <label class="proto-bar__lbl" for="pb-go">Tela</label><select class="select select--sm" id="pb-go">${opts}</select>${seg}
      <span class="grow"></span><span class="proto-bar__spec t-3">${s.spec ? "SPEC " + s.spec : ""}</span>
      <button type="button" class="btn btn--sm btn--ghost" id="pb-map">${W.icon("list")}<span>Mapa e fluxos</span></button>
      <label class="proto-bar__lbl check" style="align-items:center"><input type="checkbox" id="pb-motion"${document.documentElement.dataset.motion === "reduced" ? " checked" : ""} /><span>Menos movimento</span></label></div>`;
  }

  function render() {
    const cur = parseHash();
    const s = SCREENS[cur.id];
    if (!s.states[cur.state]) cur.state = "normal";
    document.title = `${s.title} · Warden — Protótipo final`;
    window.WardenUI.hideTip();
    document.getElementById("bar").innerHTML = protobar(cur);
    stage().innerHTML = s.render(cur.state);
    document.querySelectorAll('.toasts').forEach((t) => t.remove());
    window.WardenUI.bind(stage());
    if (s.after) s.after(cur.state);
    // Telas do Warden 1.1: selo "1.1" ao lado do título da página (como o selo P1 nas funções P1)
    if (s.v11) { const h = stage().querySelector(".pagehead__title[data-title]"); if (h && !h.querySelector(".badge")) h.insertAdjacentHTML("beforeend", " " + W.badge("v11", "1.1")); }
    const layer = stage().querySelector(".scrim:not([data-static])");
    const openMenu = stage().querySelector('[aria-expanded="true"][aria-controls]');
    if (openMenu) { const m = document.getElementById(openMenu.getAttribute("aria-controls")); if (m) window.WardenUI.openMenu(openMenu, m); }
    else if (!layer) { const t = stage().querySelector("[data-title]"); if (t) t.focus({ preventScroll: true }); }
    const main = stage().querySelector(".app__main");
    if (main) main.scrollTop = 0;
    window.scrollTo(0, 0);
  }

  function mapDialog() {
    const flows = [
      ["1. Primeiro uso: criar o primeiro pack", ["boas-vindas", "boas-vindas-2", "boas-vindas-3", "boas-vindas-4", "packs-vazio", "criar-1", "criar-2", "criar-3", "criar-4", "criar-5", "pack-novo"]],
      ["2. Adicionar um mod", ["packs", "mods", "adicionar", "adicionar-busca", "dependencias", "adicionado"]],
      ["3. Ajustar configs, testar e trazer o que mudou", ["configs", "configs-diff", "teste-checagem", "teste-preparo", "teste-manual", "teste-jogo", "configs-instancia", "teste-fechou", "trazido"]],
      ["4. O jogo travou: entender e corrigir", ["teste-travou", "dependencias", "teste-ia", "ia-conversa"]],
      ["5. Salvar a versão final e publicar", ["salvar", "salvo", "publicar", "publicar-repo", "publicado"]],
      ["6. Descobrir e adicionar vários mods de uma vez (D4)", ["mods", "adicionar", "adicionar-busca", "adicionar-detalhe", "dependencias-varios", "adicionado"]],
      ["7. Trazer mods de outro modpack (D4)", ["adicionar", "adicionar-modpacks", "modpack", "dependencias-modpack", "adicionado"]],
      ["8. Travou sem causa: encontrar o culpado e conversar com a IA (D4)", ["problemas", "teste-travou~semcausa", "culpado-config", "culpado", "culpado-resultado", "mods-raio-x", "ia-consent", "ia-conversa"]],
      ["9. Testar como servidor e gerar o pacote para servidor (D4)", ["testar-menu", "servidor-eula", "servidor-opcoes", "teste-servidor", "exportar-servidor"]],
      ["10. Importar um modpack de outro app (D4)", ["packs", "importar", "importar-pronto"]],
      ["11. Antes de publicar: segurança e mods removidos (1.1)", ["problemas", "seguranca", "seguranca-confiar", "manutencao~removido", "substitutos", "publicar~bloqueado", "publicar"]],
      ["12. Analisar o travamento de um jogador (1.1)", ["problemas", "jogador", "jogador-resultado", "jogador-ia", "ia-conversa"]],
      ["13. Organizar o pack e acompanhar o desempenho (1.1)", ["mods-grupos", "mods-nota", "grupos", "salvar", "historico", "desempenho", "teste-fechou~pesada", "repetidos"]],
    ];
    const link = (i) => `<a href="#${i}" data-map-link>${SCREENS[i].title}</a>`;
    const body = `<h3 class="t-strong">Fluxos principais</h3>${flows.map(([t, ids]) => `<div style="margin-top:10px"><div class="t-sm t-2">${t}</div><div class="row row--wrap t-sm" style="margin-top:4px">${ids.map(link).join(`<span class="t-3" aria-hidden="true">→</span>`)}</div></div>`).join("")}
      <h3 class="t-strong" style="margin-top:20px">Todas as telas</h3><div class="grid-2" style="margin-top:8px">${Object.keys(GROUPS).map((g) => `<div><div class="t-caps t-3">${GROUPS[g]}</div><ul class="stack-2 t-sm" style="margin-top:6px">${ORDER.filter((i) => SCREENS[i].group === g).map((i) => `<li>${link(i)}${SCREENS[i].v11 ? " " + W.tag("1.1", "plain", { title: "Warden 1.1 \"Profissional\" (tarefa D5)" }) : SCREENS[i].d4 ? " " + W.tag("D4", "plain", { title: "Novo na tarefa D4" }) : ""}${Object.keys(SCREENS[i].states).length ? ` <span class="t-3">· ${Object.keys(SCREENS[i].states).map((k) => STATE_LABELS[k] || k).join(", ")}</span>` : ""}</li>`).join("")}</ul></div>`).join("")}</div>`;
    return W.dialog({ title: "Mapa e fluxos do protótipo", sub: `${ORDER.length} telas e diálogos: ${ORDER.filter((i) => SCREENS[i].d4).length} da D4 (funções avançadas) e ${ORDER.filter((i) => SCREENS[i].v11).length} do Warden 1.1 "Profissional". Os estados (carregando, vazio, erro) ficam no seletor Estado da barra de cima.`, body, size: "lg", id: "map" });
  }

  // ---------- Eventos ----------
  document.addEventListener("click", (e) => {
    const t = e.target;
    // "Pular para o conteúdo": a rota usa o hash, então o atalho move o foco por script
    if (t.closest(".skip-link")) { e.preventDefault(); const m = document.getElementById("conteudo"); if (m) { m.setAttribute("tabindex", "-1"); (m.querySelector("[data-title]") || m).focus(); } return; }
    if (t.closest("#pb-map")) { const w = document.createElement("div"); w.innerHTML = mapDialog(); const l = w.firstElementChild; l.id = "map-layer"; document.body.appendChild(l); window.WardenUI.openLayer(l, t.closest("#pb-map")); return; }
    if (t.closest("[data-map-link]")) { document.getElementById("map-layer")?.remove(); return; }
    const sb = t.closest(".proto-bar [data-state]");
    if (sb) { navigate(parseHash().id, sb.dataset.state); return; }
    const g = t.closest("[data-go]");
    if (g && !g.disabled && g.getAttribute("aria-disabled") !== "true") {
      e.preventDefault();
      const target = g.getAttribute("data-go");
      if (target) { const [id, st] = target.split("~"); navigate(id, st); }
      else window.WardenUI.toast(W.toast({ kind: "info", text: "Esta ação não faz parte do protótipo." }), 2600);
      return;
    }
    const sec = t.closest(".secmenu__item");
    if (sec) { e.preventDefault(); navigate(sec.getAttribute("href").slice(1)); }
  });
  document.addEventListener("change", (e) => {
    if (e.target.id === "pb-go") navigate(e.target.value);
    if (e.target.id === "pb-motion") { if (e.target.checked) document.documentElement.dataset.motion = "reduced"; else delete document.documentElement.dataset.motion; try { localStorage.setItem("warden-motion", e.target.checked ? "reduced" : ""); } catch (_) { /* sem armazenamento */ } }
  });
  // Fechar diálogo/painel = ir para a tela de fundo (data-esc)
  document.addEventListener("warden:close", (e) => {
    const target = e.target.getAttribute("data-esc");
    if (target != null && e.target.closest("#stage")) { e.preventDefault(); navigate(target); }
  });
  document.addEventListener("warden:menu-close", (e) => {
    if (e.target.id === "menu-testar" && parseHash().id === "testar-menu") { e.preventDefault(); navigate("mods"); }
    if (e.target.id === "menu-pack" && parseHash().id === "packs-menu") { e.preventDefault(); navigate("packs"); }
  });
  window.addEventListener("hashchange", render);

  try { if (localStorage.getItem("warden-motion") === "reduced") document.documentElement.dataset.motion = "reduced"; } catch (_) { /* sem armazenamento */ }

  window.P = { def, go, B, sim, appShell, packShell, pageHead, render, navigate, SCREENS };
  window.addEventListener("DOMContentLoaded", () => { if (!location.hash) history.replaceState(null, "", "#packs"); render(); });
})();
