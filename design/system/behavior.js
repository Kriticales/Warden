/* ==========================================================================
   Warden — comportamento acessível dos componentes (sem dependências).
   No app real isso vem do Radix (via shadcn/ui); aqui reproduz o mesmo contrato
   de teclado para a galeria e o protótipo poderem ser testados de verdade.
     Menus: Enter/Espaço/↓ abrem; ↑ ↓ Home End navegam; Esc fecha e devolve o foco.
     Diálogo e painel: o foco entra, Tab fica preso, Esc fecha, o foco volta para quem abriu.
     Tooltip: aparece no hover e no foco, some com Esc.
     Seletor de modo e grupos de radio: setas trocam.
   Eventos para quem usa (o protótipo escuta):
     "warden:close"  disparado no .scrim quando o usuário pede para fechar (Esc, ✕, clique fora)
     "warden:menu-close" disparado no .menu quando fecha
   ========================================================================== */
(function () {
  "use strict";
  const FOCUSABLE = 'a[href], button:not([disabled]), input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"]), summary';
  const visible = (el) => !!(el.offsetWidth || el.offsetHeight || el.getClientRects().length);
  const focusables = (root) => [...root.querySelectorAll(FOCUSABLE)].filter((el) => visible(el) && !el.closest("[hidden]") && !el.closest("[inert]"));

  // ---------- Camadas (diálogo / painel lateral) ----------
  function topLayer() {
    const layers = [...document.querySelectorAll(".scrim:not([data-static])")];
    return layers[layers.length - 1] || null;
  }
  function requestClose(scrim, reason) {
    const ev = new CustomEvent("warden:close", { bubbles: true, cancelable: true, detail: { reason } });
    if (scrim.dispatchEvent(ev)) {
      scrim.remove();
      const back = scrim.__returnFocus;
      if (back && document.contains(back)) back.focus();
    }
  }
  function openLayer(scrim, returnFocus) {
    scrim.__returnFocus = returnFocus || document.activeElement;
    // O foco entra no título (o leitor de tela anuncia o diálogo e o Tab segue a ordem);
    // [data-autofocus] força outro alvo, por exemplo o campo principal.
    const target = scrim.querySelector("[data-autofocus]") || scrim.querySelector(".dialog__title, .drawer__head h2") || focusables(scrim)[0];
    if (target) target.focus({ preventScroll: true });
  }

  // ---------- Menus ----------
  function menuItems(menu) { return [...menu.querySelectorAll('[role="menuitem"]')].filter(visible); }
  function openMenu(trigger, menu, focusLast) {
    menu.hidden = false;
    trigger.setAttribute("aria-expanded", "true");
    menu.__trigger = trigger;
    const items = menuItems(menu);
    const t = focusLast ? items[items.length - 1] : items.find((i) => i.getAttribute("aria-disabled") !== "true") || items[0];
    if (t) t.focus();
  }
  function closeMenu(menu, refocus) {
    if (!menu || menu.hidden) return;
    const ev = new CustomEvent("warden:menu-close", { bubbles: true, cancelable: true });
    if (!menu.dispatchEvent(ev)) return;
    menu.hidden = true;
    const trig = menu.__trigger || document.querySelector(`[aria-controls="${menu.id}"]`);
    if (trig) {
      trig.setAttribute("aria-expanded", "false");
      if (refocus) trig.focus();
    }
  }

  // ---------- Tooltip ----------
  let tipEl = null, tipFor = null;
  function showTip(el) {
    const text = el.getAttribute("data-tip");
    if (!text) return;
    hideTip();
    tipEl = document.createElement("div");
    tipEl.className = "tooltip";
    tipEl.setAttribute("role", "tooltip");
    tipEl.id = "tip-" + Math.random().toString(36).slice(2, 8);
    tipEl.textContent = text;
    (el.closest('[role="dialog"], [role="alertdialog"], header, nav, main, footer, aside, [role="region"]') || document.body).appendChild(tipEl);
    const r = el.getBoundingClientRect();
    const tw = tipEl.offsetWidth, th = tipEl.offsetHeight;
    let left = r.left + r.width / 2 - tw / 2 + window.scrollX;
    left = Math.max(8, Math.min(left, window.scrollX + document.documentElement.clientWidth - tw - 8));
    let top = r.bottom + 8 + window.scrollY;
    if (r.bottom + th + 16 > window.innerHeight) top = r.top - th - 8 + window.scrollY;
    // Posição fixa na tela, qualquer que seja o contêiner
    tipEl.style.position = "fixed";
    tipEl.style.left = left - window.scrollX + "px";
    tipEl.style.top = top - window.scrollY + "px";
    // Só descreve quando o texto acrescenta algo ao nome acessível
    const name = (el.getAttribute("aria-label") || el.textContent || "").trim();
    if (name !== text) el.setAttribute("aria-describedby", tipEl.id);
    tipFor = el;
  }
  function hideTip() {
    if (tipEl) tipEl.remove();
    if (tipFor && tipFor.getAttribute("aria-describedby")?.startsWith("tip-")) tipFor.removeAttribute("aria-describedby");
    tipEl = tipFor = null;
  }

  // ---------- Inicialização por raiz ----------
  function bind(root) {
    root = root || document;
    root.querySelectorAll('input[data-indeterminate="true"]').forEach((i) => { i.indeterminate = true; });
    root.querySelectorAll('.menu[role="menu"]').forEach((m) => {
      const trig = document.querySelector(`[aria-controls="${m.id}"]`);
      if (trig && trig.getAttribute("aria-expanded") !== "true" && !m.hasAttribute("data-static")) m.hidden = true;
    });
    const layer = root.querySelector ? root.querySelector(".scrim:not([data-static])") : null;
    if (layer) openLayer(layer);
  }

  // Delegação global (vale para conteúdo renderizado depois)
  document.addEventListener("click", (e) => {
    const t = e.target;
    // Fechar camada
    const closeBtn = t.closest("[data-close]");
    if (closeBtn) { const s = closeBtn.closest(".scrim:not([data-static])"); if (s) { e.preventDefault(); requestClose(s, "button"); return; } }
    if (t.classList && t.classList.contains("scrim") && !t.hasAttribute("data-static") && t === e.target) { requestClose(t, "backdrop"); return; }
    // Toast
    const dis = t.closest('[data-dismiss="toast"]');
    if (dis) { dis.closest(".toast")?.remove(); return; }
    // Menu
    const trig = t.closest('[aria-haspopup="menu"][aria-controls]');
    if (trig) {
      const m = document.getElementById(trig.getAttribute("aria-controls"));
      if (m) { e.preventDefault(); if (m.hidden) openMenu(trig, m); else closeMenu(m, true); return; }
    }
    document.querySelectorAll('.menu[role="menu"]:not([hidden]):not([data-static])').forEach((m) => { if (!m.contains(t)) closeMenu(m, false); });
    // Toggle
    const sw = t.closest(".switch__track");
    if (sw && !sw.disabled) {
      const on = sw.getAttribute("aria-checked") !== "true";
      sw.setAttribute("aria-checked", String(on));
      const st = sw.parentElement.querySelector(".switch__state");
      if (st) st.textContent = on ? st.dataset.on : st.dataset.off;
      return;
    }
    // Seletor de modo
    const seg = t.closest('.segmented [role="radio"]');
    if (seg && !seg.disabled) {
      seg.parentElement.querySelectorAll('[role="radio"]').forEach((b) => { b.setAttribute("aria-checked", String(b === seg)); b.tabIndex = b === seg ? 0 : -1; });
      seg.dispatchEvent(new CustomEvent("warden:segment", { bubbles: true, detail: { value: seg.dataset.value } }));
      return;
    }
    // Grupo recolhível
    const gt = t.closest(".listgroup__toggle[aria-controls]");
    if (gt) {
      const open = gt.getAttribute("aria-expanded") !== "true";
      gt.setAttribute("aria-expanded", String(open));
      const body = document.getElementById(gt.getAttribute("aria-controls"));
      if (body) body.hidden = !open;
    }
    // Árvore
    const ti = t.closest(".tree__item");
    if (ti && ti.getAttribute("aria-expanded") != null) {
      const open = ti.getAttribute("aria-expanded") !== "true";
      ti.setAttribute("aria-expanded", String(open));
      const grp = ti.nextElementSibling;
      if (grp && grp.getAttribute("role") === "group") grp.hidden = !open;
    }
  });

  document.addEventListener("keydown", (e) => {
    // Menu aberto
    const openMenuEl = e.target.closest && e.target.closest('.menu[role="menu"]');
    if (openMenuEl) {
      const items = menuItems(openMenuEl);
      const i = items.indexOf(document.activeElement);
      if (e.key === "ArrowDown") { e.preventDefault(); items[(i + 1) % items.length]?.focus(); return; }
      if (e.key === "ArrowUp") { e.preventDefault(); items[(i - 1 + items.length) % items.length]?.focus(); return; }
      if (e.key === "Home") { e.preventDefault(); items[0]?.focus(); return; }
      if (e.key === "End") { e.preventDefault(); items[items.length - 1]?.focus(); return; }
      if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); closeMenu(openMenuEl, true); return; }
      if (e.key === "Tab") { closeMenu(openMenuEl, false); return; }
    }
    const trig = e.target.closest && e.target.closest('[aria-haspopup="menu"][aria-controls]');
    if (trig && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
      const m = document.getElementById(trig.getAttribute("aria-controls"));
      if (m) { e.preventDefault(); openMenu(trig, m, e.key === "ArrowUp"); return; }
    }
    if (e.key === "Escape" && tipEl) { hideTip(); return; }
    // Camada
    const layer = topLayer();
    if (layer) {
      if (e.key === "Escape") { e.preventDefault(); requestClose(layer, "escape"); return; }
      if (e.key === "Tab") {
        const f = focusables(layer);
        if (!f.length) return;
        const first = f[0], last = f[f.length - 1];
        if (!layer.contains(document.activeElement) || !f.includes(document.activeElement)) { e.preventDefault(); (e.shiftKey ? last : first).focus(); }
        else if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
        else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
      }
    }
    // Seletor de modo e grupo de radios custom
    const seg = e.target.closest && e.target.closest('.segmented [role="radio"]');
    if (seg && ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(e.key)) {
      e.preventDefault();
      const all = [...seg.parentElement.querySelectorAll('[role="radio"]:not([disabled])')];
      const n = all[(all.indexOf(seg) + (e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 1) + all.length) % all.length];
      n.click(); n.focus();
    }
    // Árvore: setas para cima/baixo
    const ti = e.target.closest && e.target.closest(".tree__item");
    if (ti && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
      e.preventDefault();
      const tree = ti.closest('[role="tree"]');
      const all = [...tree.querySelectorAll(".tree__item")].filter(visible);
      const n = all[all.indexOf(ti) + (e.key === "ArrowDown" ? 1 : -1)];
      if (n) n.focus();
    }
  });

  document.addEventListener("input", (e) => {
    const r = e.target;
    if (r.classList && r.classList.contains("range")) {
      const min = +r.min, max = +r.max, v = +r.value;
      r.style.setProperty("--val", ((v - min) / (max - min)) * 100);
      r.setAttribute("aria-valuetext", v + " GB");
      const out = document.getElementById(r.id + "-out");
      if (out) out.innerHTML = `${v}<small>GB</small>`;
      const wrap = r.closest(".memory");
      if (wrap) {
        const warn = v > 10 || v < 4;
        wrap.classList.toggle("memory--warn", warn);
        const hint = document.getElementById(r.id + "-hint");
        if (hint) {
          const [rf, rt] = (r.dataset.recommended || "6-8").split("-");
          const base = hint.dataset.base || hint.innerHTML.replace(/^<span class="t-warn">.*?<\/span> /, "");
          hint.dataset.base = base;
          hint.innerHTML = (warn ? `<span class="t-warn">${v > 10 ? "Acima de 10 GB o jogo costuma ficar mais lento, não mais rápido." : "Menos de 4 GB costuma travar packs com muitos mods."}</span> ` : "") + base;
          void rf; void rt;
        }
      }
    }
  });

  document.addEventListener("mouseover", (e) => { const el = e.target.closest && e.target.closest("[data-tip]"); if (el && el !== tipFor) showTip(el); });
  document.addEventListener("mouseout", (e) => { const el = e.target.closest && e.target.closest("[data-tip]"); if (el && !el.contains(e.relatedTarget)) hideTip(); });
  document.addEventListener("focusin", (e) => { const el = e.target.closest && e.target.closest("[data-tip]"); if (el && el.matches(":focus-visible")) showTip(el); });
  document.addEventListener("focusout", (e) => { if (e.target === tipFor) hideTip(); });
  window.addEventListener("scroll", hideTip, true);

  function toast(html, ms) {
    let host = document.querySelector(".toasts");
    if (!host) { host = document.createElement("div"); host.className = "toasts"; document.body.appendChild(host); }
    const wrap = document.createElement("div");
    wrap.innerHTML = html;
    const el = wrap.firstElementChild;
    host.appendChild(el);
    let timer = setTimeout(() => el.remove(), ms || 6000);
    el.addEventListener("mouseenter", () => clearTimeout(timer));
    el.addEventListener("mouseleave", () => { timer = setTimeout(() => el.remove(), 3000); });
    el.addEventListener("focusin", () => clearTimeout(timer));
    return el;
  }

  window.WardenUI = { bind, openLayer, requestClose, openMenu, closeMenu, toast, focusables, hideTip };
})();
