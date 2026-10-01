// Warden — protótipo de design: núcleo (estado, rotas, componentes, diálogos).
// As telas ficam em screens-*.js e se registram em W.routes / W.actions.
// Sem dependências externas: a página publicada não pode carregar scripts de fora.
(function () {
  const D = window.DATA;

  const store = {
    get(k, d) {
      try {
        const v = localStorage.getItem("warden-proto:" + k);
        return v === null ? d : JSON.parse(v);
      } catch (_) {
        return d;
      }
    },
    set(k, v) {
      try {
        localStorage.setItem("warden-proto:" + k, JSON.stringify(v));
      } catch (_) {
        /* armazenamento indisponível: só não lembra a escolha */
      }
    },
  };

  const clone = (x) => JSON.parse(JSON.stringify(x));

  // ---------------------------------------------------------------------------
  // Estado do protótipo (tudo em memória; recarregar a página volta ao início)
  // ---------------------------------------------------------------------------
  const S = {
    dir: store.get("dir", "grafite"),
    motion: store.get("motion", "auto"),
    view: "normal", // normal | carregando | vazio | erro (seletor da barra do protótipo)
    route: "inicio",
    pack: clone(D.packs[0]),
    mods: clone(D.mods),
    balmAdded: false,
    unsaved: 5,
    crash: true,
    // editor
    modFilter: "todos",
    modQuery: "",
    selected: new Set(),
    // criar pack
    np: { name: "", mc: "1.21.1", loader: "NeoForge", lv: "21.1.209", tpl: "vazio", tried: false },
    // adicionar
    add: { tab: "buscar", kind: "mods", query: "", sel: "cookingforblockheads", ver: null, srcM: true, srcC: true, opt: {}, link: "https://www.curseforge.com/minecraft/mc-mods/framed-blocks", linkState: "idle", added: new Set() },
    // configs
    cfg: { file: "defaultconfigs/create-server.toml", mode: "form", editing: false, values: null, open: { config: true, jade: false, defaultconfigs: true } },
    // testar
    test: { mode: "trabalho", quick: true, checked: false, resolved: new Set(), prep: null, prepDone: false, startedAt: 0, consoleFilter: "tudo", consoleQuery: "", autoscroll: true, lines: [], detected: 0 },
    changes: null,
    changeSel: "ch1",
    // diagnóstico
    diag: { ai: "idle", fixed: null },
    // versões
    ver: { bump: "minor", channel: "Estável", title: "Conforto", push: true, changelog: null },
    // exportar
    exp: { excludeFingerprint: true, source: "atual", open: { "mods/": false }, done: false },
    // configurações
    set: { cfKey: "••••••••••••••••••••••••••••••••••••••••••••••••7Qe", showCf: false, gemKey: "", showGem: false, mem: 6, player: "Construtor42", javaAuto: true, textSize: "normal" },
  };

  // ---------------------------------------------------------------------------
  // Utilitários
  // ---------------------------------------------------------------------------
  const esc = (s) =>
    String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);

  function hash(s) {
    let h = 2166136261;
    for (const c of String(s)) {
      h ^= c.charCodeAt(0);
      h = Math.imul(h, 16777619);
    }
    return h >>> 0;
  }

  // Ícone provisório de mod/pack: monograma (Grafite/Calcita) ou pixel art (Ardósia).
  function ticon(name, size) {
    const hv = hash(name);
    const hue = hv % 360;
    const cls = `ticon${size ? " " + size : ""}`;
    if (S.dir === "ardosia") {
      let rects = "";
      let bits = hv;
      for (let y = 0; y < 6; y++) {
        for (let x = 0; x < 3; x++) {
          const on = bits & 1;
          bits = (bits >>> 1) | ((bits & 1) << 31);
          if (on || (y > 0 && y < 5 && x === 2 && hv % 3 === 0)) {
            rects += `<rect class="px" x="${x + 1}" y="${y + 1}" width="1" height="1"/><rect class="px" x="${6 - x}" y="${y + 1}" width="1" height="1"/>`;
          }
        }
      }
      return `<span class="${cls}" style="--h:${hue}" aria-hidden="true"><svg viewBox="0 0 8 8" shape-rendering="crispEdges"><rect width="8" height="8" class="pxbg"/>${rects}</svg></span>`;
    }
    const words = String(name).replace(/[^\p{L}\p{N} ]/gu, " ").split(/\s+/).filter((w) => w && !/^(e|de|do|da|of|the|n)$/i.test(w));
    const ini = ((words[0] || "?")[0] + (words[1] ? words[1][0] : (words[0] || "")[1] || "")).toUpperCase();
    return `<span class="${cls}" style="--h:${hue}" aria-hidden="true">${esc(ini)}</span>`;
  }

  const kindIcon = { ok: "ok", warn: "warn", danger: "error", info: "info", neutral: "clock" };
  const pill = (kind, text, ic) => `<span class="pill-status ${kind}">${icon(ic || kindIcon[kind])}${esc(text)}</span>`;
  const status = (kind, text, ic) => `<span class="status ${kind}">${icon(ic || kindIcon[kind])}${esc(text)}</span>`;
  const srcNames = { modrinth: "Modrinth", curseforge: "CurseForge", local: "Arquivo local" };
  const src = (s) => `<span class="src ${s}"><span class="dot" aria-hidden="true"></span>${srcNames[s]}</span>`;
  const sideNames = { cliente: "Cliente", servidor: "Servidor", ambos: "Ambos", "?": "Não identificado" };
  const sideIcons = { cliente: "monitor", servidor: "server", ambos: "layers", "?": "alert" };
  const chip = (text, ic, cls) => `<span class="chip${cls ? " " + cls : ""}">${ic ? icon(ic, "sm") : ""}${esc(text)}</span>`;

  // ---------------------------------------------------------------------------
  // Temporizadores ligados à tela atual (limpos ao trocar de tela)
  // ---------------------------------------------------------------------------
  let timers = [];
  const every = (ms, fn) => timers.push(setInterval(fn, ms));
  const after = (ms, fn) => timers.push(setTimeout(fn, ms));
  const clearTimers = () => {
    timers.forEach((t) => {
      clearInterval(t);
      clearTimeout(t);
    });
    timers = [];
  };
  const reducedMotion = () =>
    S.motion === "reduced" || (S.motion === "auto" && window.matchMedia("(prefers-reduced-motion: reduce)").matches);

  // ---------------------------------------------------------------------------
  // Avisos rápidos (toasts) — anunciados por leitor de tela (aria-live)
  // ---------------------------------------------------------------------------
  function toast(text, kind = "ok") {
    const box = document.getElementById("toasts");
    const el = document.createElement("div");
    el.className = "toast " + kind;
    el.innerHTML = `${icon(kind === "ok" ? "ok" : kind === "warn" ? "warn" : "info")}<div>${text}</div>`;
    box.appendChild(el);
    setTimeout(() => el.remove(), 5200);
  }

  // ---------------------------------------------------------------------------
  // Diálogos e gaveta: foco preso dentro, Esc fecha, foco volta para quem abriu
  // ---------------------------------------------------------------------------
  let dlg = null;
  function dialog({ title, sub = "", body, foot = "", wide = false, drawer = false, lead = "", onClose = null, initial = null }) {
    const returnTo = dlg ? dlg.returnTo : document.activeElement;
    if (dlg) closeDialog(true);
    const layer = document.getElementById("layer");
    const ov = document.createElement("div");
    ov.className = "overlay" + (drawer ? " drawer-overlay" : "");
    ov.innerHTML = `<div class="${drawer ? "drawer" : "dialog" + (wide ? " wide" : "")}" role="dialog" aria-modal="true" aria-labelledby="dlg-title"${sub ? ' aria-describedby="dlg-sub"' : ""}>
      <div class="dhead">${lead}<div class="grow"><h2 id="dlg-title">${title}</h2>${sub ? `<p id="dlg-sub" class="section-sub">${sub}</p>` : ""}</div>
        <button class="btn btn-ghost btn-icon x" data-action="dlg-close" aria-label="Fechar">${icon("x")}</button></div>
      <div class="dbody">${body}</div>
      ${foot ? `<div class="dfoot">${foot}</div>` : ""}
    </div>`;
    layer.appendChild(ov);
    document.getElementById("app").inert = true;
    document.getElementById("protobar").inert = true;
    ov.addEventListener("mousedown", (e) => {
      if (e.target === ov) closeDialog();
    });
    ov.addEventListener("keydown", (e) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        closeDialog();
      }
      if (e.key === "Tab") {
        const f = [...ov.querySelectorAll('button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea, [tabindex]:not([tabindex="-1"])')].filter((x) => x.offsetParent !== null);
        if (!f.length) return;
        const first = f[0];
        const last = f[f.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    });
    dlg = { ov, returnTo, onClose };
    const target = (initial && ov.querySelector(initial)) || ov.querySelector(".dfoot .btn-primary, .dfoot .btn-danger") || ov.querySelector(".dbody button, .dbody input, .dbody select, .dbody textarea") || ov.querySelector(".x");
    target && target.focus();
    return ov;
  }
  function closeDialog(silent) {
    if (!dlg) return;
    const { ov, returnTo, onClose } = dlg;
    ov.remove();
    dlg = null;
    document.getElementById("app").inert = false;
    document.getElementById("protobar").inert = false;
    if (!silent) {
      onClose && onClose();
      if (returnTo && document.contains(returnTo)) returnTo.focus();
    }
  }

  // ---------------------------------------------------------------------------
  // Rotas
  // ---------------------------------------------------------------------------
  const routes = {}; // chave: "pack/mods" etc. { title, group, nav, states, render, enter, after }
  const actions = {};

  function currentKey() {
    const h = location.hash.replace(/^#\/?/, "");
    return routes[h] ? h : "inicio";
  }

  function go(key) {
    if (location.hash === "#/" + key) onRoute();
    else location.hash = "#/" + key;
  }

  function sidebar(def) {
    const p = S.pack;
    const cur = (n) => (def.nav === n ? ' aria-current="page"' : "");
    const item = (href, ic, txt, n, extra = "") =>
      `<a href="${href}"${cur(n)} title="${esc(txt)}">${icon(ic, "lg")}<span class="txt">${esc(txt)}</span>${extra}</a>`;
    return `<aside class="sidebar" aria-label="Navegação principal">
      <a class="brand" href="#/inicio" aria-label="Warden — início">${wardenMark()}<span class="name">Warden</span></a>
      <nav class="nav" aria-label="Geral">
        ${item("#/inicio", "house", "Meus modpacks", "inicio")}
      </nav>
      <div class="nav-label" id="nav-pack-label">Pack aberto</div>
      <div class="pack-switch">${ticon(p.name)}<span class="pname">${esc(p.name)}</span><span class="pmeta">${esc(p.mc)} · ${esc(p.loader)}</span></div>
      <nav class="nav" aria-labelledby="nav-pack-label">
        ${item("#/pack/mods", "package", "Conteúdo", "conteudo", `<span class="count">${S.mods.length}</span>`)}
        ${item("#/pack/configs", "sliders", "Configs", "configs")}
        ${item("#/testar", "play", "Testar", "testar")}
        ${item("#/diagnostico", "activity", "Diagnóstico", "diagnostico", S.crash ? `<span class="count danger">1<span class="sr-only"> travamento para revisar</span></span>` : "")}
        ${item("#/versoes", "history", "Versões", "versoes", S.unsaved ? `<span class="count" title="Alterações não salvas">${S.unsaved}<span class="sr-only"> alterações não salvas</span></span>` : "")}
        ${item("#/exportar", "upload", "Exportar", "exportar")}
      </nav>
      <div class="bottom">
        <nav class="nav" aria-label="Aplicativo">${item("#/ajustes", "settings", "Configurações", "ajustes")}</nav>
        <div class="status-line" title="packwiz embutido e pronto">${status("ok", "", "ok")}<span class="txt">Tudo pronto para usar</span></div>
      </div>
    </aside>`;
  }

  function syncProtobar(key) {
    const def = routes[key];
    document.querySelectorAll("#protobar [data-dir]").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.dir === S.dir)));
    document.querySelectorAll("#protobar [data-view]").forEach((b) => {
      const ok = (def.states || ["normal"]).includes(b.dataset.view);
      b.disabled = !ok;
      b.title = ok ? "" : "Este estado não se aplica a esta tela";
      b.setAttribute("aria-pressed", String(b.dataset.view === S.view));
    });
    const sel = document.getElementById("proto-screen");
    if (sel) sel.value = key;
  }

  function render() {
    const key = S.route;
    const def = routes[key];
    // guarda foco e posição do cursor para restaurar depois de redesenhar
    const a = document.activeElement;
    const fk = a && a.closest && a.closest("[data-fk]") ? a.closest("[data-fk]").dataset.fk : null;
    const caret = a && typeof a.selectionStart === "number" ? [a.selectionStart, a.selectionEnd] : null;
    const app = document.getElementById("app");
    app.innerHTML = sidebar(def) + `<main id="main" tabindex="-1">${def.render()}</main>`;
    if (fk) {
      const el = app.querySelector(`[data-fk="${CSS.escape(fk)}"]`);
      if (el) {
        el.focus({ preventScroll: true });
        if (caret && typeof el.setSelectionRange === "function") {
          try {
            el.setSelectionRange(caret[0], caret[1]);
          } catch (_) {
            /* campo sem seleção de texto */
          }
        }
      }
    }
    def.after && def.after();
    syncProtobar(key);
    document.title = `${def.title} · Warden (protótipo)`;
  }

  function onRoute() {
    clearTimers();
    if (dlg) closeDialog(true);
    const key = currentKey();
    S.route = key;
    S.view = "normal";
    const def = routes[key];
    def.enter && def.enter();
    render();
    window.scrollTo(0, 0);
    const h = document.getElementById("page-title");
    (h || document.getElementById("main")).focus({ preventScroll: true });
  }

  function setView(v) {
    S.view = v;
    clearTimers();
    const def = routes[S.route];
    def.enter && def.enter();
    render();
  }

  function setDir(d) {
    S.dir = d;
    store.set("dir", d);
    document.documentElement.dataset.direction = d;
    render();
  }

  // ---------------------------------------------------------------------------
  // Delegação de eventos: data-action (clique), data-change, data-input
  // ---------------------------------------------------------------------------
  document.addEventListener("click", (e) => {
    const el = e.target.closest("[data-action]");
    if (!el || el.disabled || el.getAttribute("aria-disabled") === "true") return;
    const fn = actions[el.dataset.action];
    if (!fn) return;
    if (!(el instanceof HTMLInputElement)) e.preventDefault();
    fn(el, e);
  });
  document.addEventListener("change", (e) => {
    const el = e.target.closest("[data-change]");
    if (el && actions[el.dataset.change]) actions[el.dataset.change](el, e);
  });
  document.addEventListener("input", (e) => {
    const el = e.target.closest("[data-input]");
    if (el && actions[el.dataset.input]) actions[el.dataset.input](el, e);
  });

  // Teclado: abas com setas (padrão WAI-ARIA) e árvore de arquivos
  document.addEventListener("keydown", (e) => {
    const tab = e.target.closest && e.target.closest('[role="tab"]');
    if (tab && ["ArrowLeft", "ArrowRight", "Home", "End"].includes(e.key)) {
      const list = [...tab.closest('[role="tablist"]').querySelectorAll('[role="tab"]:not([disabled])')];
      let i = list.indexOf(tab);
      if (e.key === "ArrowLeft") i = (i - 1 + list.length) % list.length;
      if (e.key === "ArrowRight") i = (i + 1) % list.length;
      if (e.key === "Home") i = 0;
      if (e.key === "End") i = list.length - 1;
      e.preventDefault();
      list[i].click();
      const again = document.querySelector(`[role="tab"][data-fk="${CSS.escape(list[i].dataset.fk)}"]`);
      again && again.focus();
      return;
    }
    const item = e.target.closest && e.target.closest('[role="tree"] [role="treeitem"]');
    if (item && e.target === item && ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End", "Enter", " "].includes(e.key)) {
      e.preventDefault();
      if (e.key === "Enter" || e.key === " ") return item.click();
      const items = [...item.closest('[role="tree"]').querySelectorAll('[role="treeitem"]')].filter((n) => n.offsetParent !== null);
      let i = items.indexOf(item);
      const expanded = item.getAttribute("aria-expanded");
      if (e.key === "ArrowDown") i = Math.min(items.length - 1, i + 1);
      else if (e.key === "ArrowUp") i = Math.max(0, i - 1);
      else if (e.key === "Home") i = 0;
      else if (e.key === "End") i = items.length - 1;
      else if (e.key === "ArrowRight" && expanded === "false") return item.click();
      else if (e.key === "ArrowLeft" && expanded === "true") return item.click();
      else if (e.key === "ArrowLeft") {
        const parent = item.parentElement.closest('[role="treeitem"]');
        if (parent) parent.focus();
        return;
      } else return;
      items.forEach((n) => n.setAttribute("tabindex", "-1"));
      items[i].setAttribute("tabindex", "0");
      items[i].focus();
    }
  });

  actions["dlg-close"] = () => closeDialog();
  actions["skip"] = () => {
    const h = document.getElementById("page-title");
    (h || document.getElementById("main")).focus();
  };
  actions["go"] = (el) => go(el.dataset.to);
  actions["soon"] = (el) => toast(el.dataset.msg || "No protótipo esta ação só mostra o caminho; ela será construída no app.", "info");

  // ---------------------------------------------------------------------------
  // Barra do protótipo
  // ---------------------------------------------------------------------------
  function bootProtobar() {
    const pb = document.getElementById("protobar");
    pb.addEventListener("click", (e) => {
      const d = e.target.closest("[data-dir]");
      if (d) setDir(d.dataset.dir);
      const v = e.target.closest("[data-view]");
      if (v && !v.disabled) setView(v.dataset.view);
    });
    const sel = document.getElementById("proto-screen");
    sel.innerHTML = W.screenList.map(([k, t]) => `<option value="${k}">${esc(t)}</option>`).join("");
    sel.addEventListener("change", () => go(sel.value));
  }

  const W = (window.W = {
    D, S, store, clone, esc, hash, ticon, pill, status, src, srcNames, sideNames, sideIcons, chip,
    every, after, clearTimers, reducedMotion, toast, dialog, closeDialog, routes, actions, go, render, setView,
    screenList: [],
  });

  window.addEventListener("hashchange", onRoute);
  window.addEventListener("DOMContentLoaded", () => {
    document.documentElement.dataset.direction = S.dir;
    if (S.motion === "reduced") document.documentElement.dataset.motion = "reduced";
    bootProtobar();
    onRoute();
  });
})();
