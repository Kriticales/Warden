/* ==========================================================================
   Warden — componentes do design system em funções que devolvem HTML.
   Usado pela galeria (design/system/index.html) e pelo protótipo final.
   No app real cada função vira um componente React (ver docs/design/HANDOFF.md);
   os nomes das opções aqui viram as props de lá.
   Depende de icons.js (window.WARDEN_ICONS).
   ========================================================================== */
(function () {
  "use strict";
  const ICONS = window.WARDEN_ICONS || {};

  const esc = (s) => String(s ?? "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  const attrs = (o) => (o ? Object.entries(o).filter(([, v]) => v !== false && v != null).map(([k, v]) => (v === true ? ` ${k}` : ` ${k}="${esc(v)}"`)).join("") : "");
  let uid = 0;
  const id = (p) => `${p || "w"}-${++uid}`;

  // ---------- Ícones ----------
  function icon(name, cls) {
    const body = ICONS[name];
    if (!body) return `<!-- ícone ${name} não existe -->`;
    return `<svg class="icon ${cls || ""}" viewBox="0 0 24 24" aria-hidden="true" focusable="false">${body}</svg>`;
  }

  // ---------- Pixel art original ----------
  function hash(s) {
    let h = 2166136261;
    for (const c of String(s)) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
    return h >>> 0;
  }
  // Ícone provisório 8x8 simétrico a partir do nome (só quando a API não tem ícone).
  function tileSvg(seed) {
    const h = hash(seed);
    const hue = h % 360;
    const bg = `hsl(${hue} 34% 18%)`;
    const fg = `hsl(${hue} 46% 52%)`;
    const hi = `hsl(${(hue + 30) % 360} 60% 72%)`;
    let rects = "";
    let bits = h;
    for (let y = 1; y < 7; y++) {
      for (let x = 1; x < 4; x++) {
        bits = Math.imul(bits ^ (bits >>> 13), 0x5bd1e995) >>> 0;
        const v = bits % 5;
        if (v < 2) continue;
        const col = v === 4 ? hi : fg;
        rects += `<rect x="${x}" y="${y}" width="1" height="1" fill="${col}"/><rect x="${7 - x}" y="${y}" width="1" height="1" fill="${col}"/>`;
      }
    }
    return `<svg viewBox="0 0 8 8" shape-rendering="crispEdges" aria-hidden="true"><rect width="8" height="8" fill="${bg}"/>${rects}</svg>`;
  }
  function tile(seed, size, label) {
    return `<span class="tile ${size ? "tile--" + size : ""}"${label ? ` role="img" aria-label="${esc(label)}"` : ' aria-hidden="true"'}>${tileSvg(seed)}</span>`;
  }

  // Marca do Warden: bloco escuro com duas antenas acesas e um núcleo de alma. Desenho próprio, 16x16.
  function brandMark(cls) {
    const r = (x, y, w, h, f, o) => `<rect x="${x}" y="${y}" width="${w}" height="${h}" fill="${f}"${o ? ` fill-opacity="${o}"` : ""}/>`;
    return `<svg class="${cls || "brand__mark"}" viewBox="0 0 16 16" shape-rendering="crispEdges" aria-hidden="true">
      ${r(2, 5, 12, 11, "#010507")}${r(3, 6, 10, 9, "#0e2630")}${r(3, 6, 10, 1, "#1e4552")}${r(3, 14, 10, 1, "#0a1a21")}
      ${r(4, 1, 2, 5, "#19d3e0")}${r(10, 1, 2, 5, "#19d3e0")}${r(4, 0, 2, 1, "#7ff3f0")}${r(10, 0, 2, 1, "#7ff3f0")}
      ${r(3, 2, 1, 1, "#0fb5c2")}${r(12, 2, 1, 1, "#0fb5c2")}
      ${r(7, 8, 2, 5, "#19d3e0")}${r(6, 9, 4, 3, "#19d3e0")}${r(7, 9, 2, 2, "#7ff3f0")}
      ${r(4, 12, 1, 1, "#19d3e0", ".6")}${r(11, 8, 1, 1, "#19d3e0", ".5")}${r(11, 13, 1, 1, "#7ff3f0", ".5")}
    </svg>`;
  }

  // Ilustração de estado vazio: um bloco de frente com um símbolo em pixel. 18x18.
  const GLYPHS = {
    plus: ["..XX..", "..XX..", "XXXXXX", "XXXXXX", "..XX..", "..XX.."],
    check: [".....X", "....XX", "X..XX.", "XXXX..", ".XX...", "......"],
    x: ["XX..XX", ".XXXX.", "..XX..", "..XX..", ".XXXX.", "XX..XX"],
    search: [".XXX..", "X...X.", "X...X.", ".XXXX.", "....XX", ".....X"],
    dots: ["......", "......", "XX.XX.", "XX.XX.", "......", "......"],
    box: ["XXXXXX", "X....X", "X.XX.X", "X.XX.X", "X....X", "XXXXXX"],
    clock: [".XXXX.", "X..X.X", "X..X.X", "X..XXX", "X....X", ".XXXX."],
    link: ["XXX...", "X.X...", "XXXXXX", "...X.X", "...XXX", "......"],
  };
  function art(kind, glyph, cls) {
    const color = { ok: "#3ddc84", danger: "#ff5c5c", warn: "#f2b33d", muted: "#8aacaa", ai: "#cfe3da" }[kind] || "#19d3e0";
    const r = (x, y, w, h, f) => `<rect x="${x}" y="${y}" width="${w}" height="${h}" fill="${f}"/>`;
    let g = "";
    (GLYPHS[glyph] || GLYPHS.box).forEach((row, y) => [...row].forEach((c, x) => { if (c === "X") g += r(6 + x, 8 + y, 1, 1, color); }));
    return `<svg class="${cls || "empty__art"}" viewBox="0 0 18 18" shape-rendering="crispEdges" aria-hidden="true">
      ${r(2, 3, 14, 14, "#010507")}${r(3, 4, 12, 3, "#1e4552")}${r(3, 7, 12, 9, "#0e2630")}${r(3, 15, 12, 1, "#0a1a21")}
      ${r(4, 4, 3, 1, "#2b5a68")}${r(10, 5, 2, 1, "#2b5a68")}
      ${r(3, 13, 2, 1, "#19d3e0")}${r(13, 9, 2, 1, "#0fb5c2")}${r(12, 15, 2, 1, "#19d3e0")}
      ${g}
    </svg>`;
  }

  // ---------- Botões ----------
  // variant: primary | secondary | ghost | danger | danger-ghost | link
  function btn(label, o = {}) {
    const cls = ["btn", o.variant ? "btn--" + o.variant : "", o.size ? "btn--" + o.size : "", o.iconOnly ? "btn--icon" : "", o.block ? "btn--block" : "", o.cls || ""].filter(Boolean).join(" ");
    const busy = o.busy ? `<span class="loader" aria-hidden="true"><i></i><i></i><i></i><i></i></span>` : "";
    const ic = o.icon ? icon(o.icon, o.iconCls) : "";
    const end = o.iconEnd ? icon(o.iconEnd) : "";
    const text = o.iconOnly ? `<span class="sr-only">${label}</span>` : `<span>${label}</span>`;
    const count = o.count != null ? `<span class="btn__count">${o.count}</span>` : "";
    const a = Object.assign({ type: o.href ? null : "button", class: cls, disabled: o.disabled && !o.href ? true : null, "aria-disabled": o.disabled && o.href ? "true" : null, "aria-busy": o.busy ? "true" : null, "data-tip": o.tip || (o.iconOnly ? label.replace(/<[^>]+>/g, "") : null) }, o.attrs || {});
    const tag = o.href ? "a" : "button";
    if (o.href) a.href = o.href;
    return `<${tag}${attrs(a)}>${busy}${ic}${text}${count}${end}</${tag}>`;
  }

  // Botão Testar. state: ready | preparing | running | disabled
  function testButton(o = {}) {
    const st = o.state || "ready";
    const label = { ready: "Testar", preparing: "Testando… ver progresso", running: "Jogo aberto: ver teste", disabled: "Testar" }[st];
    const lead = st === "running" ? `<span class="live" aria-hidden="true"></span>` : st === "preparing" ? `<span class="loader" aria-hidden="true"><i></i><i></i><i></i><i></i></span>` : icon("play", "icon--fill");
    const prog = st === "preparing" ? `<span class="testbtn__progress" aria-hidden="true"><span style="--p:${o.progress ?? 38}%"></span></span>` : "";
    const menuId = o.menuId || "menu-testar";
    return `<div class="testbtn testbtn--${st}">
      <span class="testbtn__antenna" aria-hidden="true"><i></i><i></i><i></i><i></i></span>
      <button type="button" class="btn btn--primary testbtn__main"${attrs(Object.assign({ disabled: st === "disabled" || null }, o.mainAttrs || {}))}>${lead}<span>${label}</span>${prog}</button>
      <button type="button" class="btn btn--primary testbtn__more" aria-haspopup="menu" aria-expanded="${o.open ? "true" : "false"}" aria-controls="${menuId}"${attrs(Object.assign({ disabled: st === "disabled" || null }, o.moreAttrs || {}))} data-tip="Mais opções do teste">${icon("chevron-down")}<span class="sr-only">Mais opções do teste</span></button>
    </div>`;
  }

  // ---------- Campos ----------
  function field(o, control) {
    const hint = o.hint ? `<div class="field__hint" id="${o.id}-hint">${o.hint}</div>` : "";
    const err = o.error ? `<div class="field__error" id="${o.id}-err">${icon("circle-alert", "icon--sm")}<span>${o.error}</span></div>` : "";
    const label = o.label ? `<label class="field__label" for="${o.id}">${o.label}${o.optional ? ' <span class="opt">(opcional)</span>' : ""}</label>` : "";
    return `<div class="field ${o.inline ? "field--inline" : ""} ${o.cls || ""}">${label}${control}${err}${hint}</div>`;
  }
  function describedBy(o) {
    return [o.error ? o.id + "-err" : "", o.hint ? o.id + "-hint" : ""].filter(Boolean).join(" ") || null;
  }
  function input(o = {}) {
    o.id = o.id || id("in");
    const a = { id: o.id, class: `input ${o.mono ? "input--mono" : ""} ${o.size === "sm" ? "input--sm" : ""} ${o.state ? "is-" + o.state : ""}`, type: o.type || "text", value: o.value, placeholder: o.placeholder, disabled: o.disabled, readonly: o.readonly, "aria-invalid": o.error ? "true" : null, "aria-describedby": describedBy(o), "aria-label": !o.label ? o.ariaLabel || o.placeholder : null, autocomplete: "off", spellcheck: "false" };
    Object.assign(a, o.attrs || {});
    let ctl = `<input${attrs(a)} />`;
    if (o.icon || o.end) {
      ctl = `<div class="inputwrap ${o.end ? "inputwrap--end" : ""} ${o.loading ? "inputwrap--loading" : ""}">${o.icon ? icon(o.icon) : ""}${o.loading ? '<span class="loader" aria-hidden="true"><i></i><i></i><i></i><i></i></span>' : ""}${ctl}${o.end ? `<span class="inputwrap__end">${o.end}</span>` : ""}</div>`;
    }
    return o.bare ? ctl : field(o, ctl);
  }
  function textarea(o = {}) {
    o.id = o.id || id("ta");
    return field(o, `<textarea${attrs({ id: o.id, class: "textarea", placeholder: o.placeholder, rows: o.rows || 3, disabled: o.disabled, "aria-describedby": describedBy(o), "aria-invalid": o.error ? "true" : null })}>${esc(o.value || "")}</textarea>`);
  }
  function select(o = {}) {
    o.id = o.id || id("sel");
    const opts = (o.options || []).map((op) => {
      const [v, l] = Array.isArray(op) ? op : [op, op];
      return `<option value="${esc(v)}"${String(v) === String(o.value) ? " selected" : ""}>${esc(l)}</option>`;
    }).join("");
    const ctl = `<select${attrs(Object.assign({ id: o.id, class: `select ${o.size === "sm" ? "select--sm" : ""} ${o.state ? "is-" + o.state : ""}`, disabled: o.disabled, "aria-label": !o.label ? o.ariaLabel : null, "aria-invalid": o.error ? "true" : null, "aria-describedby": describedBy(o) }, o.attrs || {}))}>${opts}</select>`;
    return o.bare ? ctl : field(o, ctl);
  }
  // Busca ou link. state: empty | search | link | invalid | loading
  function omnibox(o = {}) {
    const st = o.state || "empty";
    const fid = o.id || "omni";
    const ic = st === "link" ? "link" : "search";
    const status = {
      empty: `Busca no Modrinth e na CurseForge ao mesmo tempo. Também aceita um link colado.`,
      search: o.statusText || `48 resultados para “${esc(o.value)}”`,
      link: `${icon("circle-check", "icon--sm t-ok")}<span><b class="t-ok">Link do ${o.linkSource || "Modrinth"} reconhecido.</b> Mostrando a pré-visualização ao lado.</span>`,
      invalid: `${icon("circle-alert", "icon--sm t-danger")}<span class="t-danger">Este link não é do Modrinth, da CurseForge nem um arquivo .jar ou .zip. Confira o endereço.</span>`,
      loading: `Buscando…`,
    }[st];
    const a = { id: fid, class: "input", type: "search", value: o.value || null, placeholder: "Buscar pelo nome, ou colar um link do Modrinth, da CurseForge ou direto", "aria-describedby": fid + "-st", "aria-invalid": st === "invalid" ? "true" : null, "aria-label": "Buscar ou colar link", autocomplete: "off", spellcheck: "false" };
    return `<div class="omnibox ${st === "link" ? "omnibox--link" : ""}">
      <div class="grow"><div class="inputwrap ${st === "loading" ? "inputwrap--loading" : ""}">${icon(ic)}${st === "loading" ? '<span class="loader" aria-hidden="true"><i></i><i></i><i></i><i></i></span>' : ""}<input${attrs(a)} /></div>
      <div class="omnibox__status" id="${fid}-st" role="status">${status}</div></div>
      ${o.after || ""}
    </div>`;
  }
  function switchCtl(o = {}) {
    const sid = o.id || id("sw");
    const on = !!o.checked;
    const stateText = !o.stateText ? "" : `<span class="switch__state" data-on="${esc(o.onText || "Ligado")}" data-off="${esc(o.offText || "Desligado")}">${on ? o.onText || "Ligado" : o.offText || "Desligado"}</span>`;
    return `<span class="switch ${o.disabled ? "switch--disabled" : ""} ${o.state ? "is-" + o.state : ""}">
      <button type="button" role="switch" class="switch__track ${o.state === "focus" ? "is-focus" : ""}" id="${sid}" aria-checked="${on}" aria-labelledby="${sid}-l"${o.disabled ? " disabled" : ""}></button>
      <label id="${sid}-l" for="${sid}">${o.label}</label>${stateText}</span>`;
  }
  function check(o = {}) {
    const cid = o.id || id("cb");
    const a = { type: o.type || "checkbox", id: cid, name: o.name, value: o.value, checked: o.checked, disabled: o.disabled, class: o.state === "focus" ? "is-focus" : null, "data-indeterminate": o.indeterminate ? "true" : null, "aria-label": o.ariaLabel || null };
    Object.assign(a, o.attrs || {});
    const text = o.label ? `<span class="check__text"><span>${o.label}</span>${o.desc ? `<span class="check__desc">${o.desc}</span>` : ""}</span>` : "";
    return `<label class="check ${o.state && o.state !== "focus" ? "is-" + o.state : ""}"><input${attrs(a)} />${text}</label>`;
  }
  function choice(o = {}) {
    const cid = o.id || id("ch");
    return `<label class="choice ${o.state ? "is-" + o.state : ""}" for="${cid}">
      <span class="check"><input type="${o.type || "radio"}" id="${cid}" name="${esc(o.name || "choice")}"${o.checked ? " checked" : ""}${o.disabled ? " disabled" : ""} /></span>
      <span class="grow"><span class="choice__title">${o.title}</span>${o.badge || ""}<span class="choice__desc" style="display:block">${o.desc || ""}</span>${o.extra || ""}</span></label>`;
  }
  function segmented(options, value, label) {
    return `<div class="segmented" role="radiogroup" aria-label="${esc(label)}">${options.map((op) => {
      const [v, l, dis] = Array.isArray(op) ? op : [op, op];
      return `<button type="button" role="radio" aria-checked="${v === value}" tabindex="${v === value ? 0 : -1}"${dis ? " disabled" : ""} data-value="${esc(v)}">${l}</button>`;
    }).join("")}</div>`;
  }
  // Memória em GB. recommended: [de, até]
  function memory(o = {}) {
    const min = o.min ?? 2, max = o.max ?? 16, v = o.value ?? 6;
    const pct = (x) => ((x - min) / (max - min)) * 100;
    const [rf, rt] = o.recommended || [6, 8];
    const mid = o.id || id("mem");
    const warn = v > (o.high ?? 10) || v < (o.low ?? 4);
    const ticks = [2, 4, 6, 8, 10, 12, 14, 16].filter((t) => t >= min && t <= max);
    return `<div class="memory ${warn ? "memory--warn" : ""}">
      <div class="memory__top"><label class="field__label" for="${mid}">${o.label || "Memória para o jogo"}</label><output class="memory__value" for="${mid}" id="${mid}-out">${v}<small>GB</small></output></div>
      <div class="memory__rail" style="--zone-from:${pct(rf)}%;--zone-to:${pct(rt)}%"><span class="memory__zone" aria-hidden="true"></span>
        <input type="range" class="range" id="${mid}" min="${min}" max="${max}" step="1" value="${v}" style="--val:${pct(v)}" aria-describedby="${mid}-hint" aria-valuetext="${v} GB"${o.disabled ? " disabled" : ""} data-recommended="${rf}-${rt}" /></div>
      <div class="memory__scale" aria-hidden="true">${ticks.map((t) => `<span>${t}</span>`).join("")}</div>
      <div class="field__hint" id="${mid}-hint">${warn ? `<span class="t-warn">${v > (o.high ?? 10) ? "Acima de 10 GB o jogo costuma ficar mais lento, não mais rápido." : "Menos de 4 GB costuma travar packs com muitos mods."}</span> ` : ""}Faixa verde: recomendado para ${o.mods || 128} mods (${rf} a ${rt} GB). Este computador tem ${o.ram || 32} GB.</div>
    </div>`;
  }

  // ---------- Selos ----------
  function tag(text, kind, o = {}) {
    return `<span class="tag ${kind ? "tag--" + kind : ""}"${o.title ? ` title="${esc(o.title)}"` : ""}>${o.icon ? icon(o.icon) : ""}${text}</span>`;
  }
  const SOURCES = { modrinth: "Modrinth", curseforge: "CurseForge", local: "Arquivo local", url: "Link direto" };
  function source(src) {
    if (src === "both") return `<span class="tag"><span class="tag__mark tag__mark--modrinth" aria-hidden="true"></span><span class="tag__mark tag__mark--curseforge" aria-hidden="true"></span>Modrinth e CurseForge</span>`;
    return `<span class="tag tag--${src}"><span class="tag__mark" aria-hidden="true"></span>${SOURCES[src] || src}</span>`;
  }
  const SIDES = { both: ["Cliente e servidor", "layers"], client: ["Só cliente", "monitor"], server: ["Só servidor", "server"], unknown: ["Lado desconhecido", "circle-help"] };
  function side(s) {
    const [l, ic] = SIDES[s] || SIDES.unknown;
    return `<span class="tag tag--plain ${s === "unknown" ? "tag--warn" : ""}">${icon(ic)}${l}</span>`;
  }
  const BADGE_ICONS = { danger: "circle-x", warn: "triangle-alert", ok: "circle-check", info: "info", neutral: null, p1: null };
  function badge(kind, text) {
    const ic = BADGE_ICONS[kind];
    return `<span class="badge badge--${kind}">${ic ? icon(ic) : ""}${text}</span>`;
  }
  function count(n, kind, label) {
    return `<span class="count ${kind ? "count--" + kind : ""}"${label ? ` aria-label="${esc(label)}"` : ""}>${n}</span>`;
  }
  function status(kind, text) {
    const ic = { ok: "circle-check", danger: "circle-x", warn: "triangle-alert", muted: "circle-help" }[kind];
    return `<span class="status status--${kind}">${ic ? icon(ic) : ""}${text}</span>`;
  }

  // ---------- Retorno ----------
  const ALERT_ICONS = { info: "info", ok: "circle-check", warn: "triangle-alert", danger: "circle-alert", neutral: "info", ai: "sparkles" };
  function alert(o = {}) {
    const k = o.kind || "info";
    const role = o.role || (k === "danger" ? "alert" : null);
    const actions = o.actions ? `<div class="alert__actions ${o.actionsBelow ? "alert__actions--below" : ""}">${o.actions}</div>` : "";
    return `<div class="alert alert--${k} ${o.compact ? "alert--compact" : ""} ${o.cls || ""}"${role ? ` role="${role}"` : ""}>${icon(o.icon || ALERT_ICONS[k])}
      <div>${o.title ? `<div class="alert__title">${o.title}</div>` : ""}${o.text ? `<div class="alert__text">${o.text}</div>` : ""}${o.more || ""}</div>${actions}</div>`;
  }
  function issue(o = {}) {
    const k = o.kind || "danger";
    const lbl = { danger: "Erro", warn: "Aviso", info: "Informação" }[k];
    return `<article class="issue issue--${k}" aria-labelledby="${o.id || ""}">
      <div class="issue__head">${badge(k, lbl)}<h3 class="issue__title" ${o.id ? `id="${o.id}"` : ""}>${o.title}</h3></div>
      ${o.text ? `<p class="issue__text">${o.text}</p>` : ""}
      ${o.evidence ? `<details class="issue__evidence"${o.open ? " open" : ""}><summary>Como sabemos</summary><div style="margin-top:6px">${o.evidence}</div></details>` : ""}
      ${o.actions ? `<div class="issue__actions">${o.actions}</div>` : ""}</article>`;
  }
  function empty(o = {}) {
    return `<div class="empty ${o.kind ? "empty--" + o.kind : ""} ${o.compact ? "empty--compact" : ""}">${art(o.artKind || o.kind, o.glyph || "box")}
      <h2 class="empty__title">${o.title}</h2>${o.text ? `<p class="empty__text">${o.text}</p>` : ""}
      ${o.actions ? `<div class="empty__actions">${o.actions}</div>` : ""}${o.hint ? `<p class="empty__hint">${o.hint}</p>` : ""}</div>`;
  }
  function toast(o = {}) {
    const k = o.kind || "info";
    const ic = { info: "info", ok: "circle-check", danger: "circle-alert", warn: "triangle-alert" }[k];
    return `<div class="toast toast--${k}" role="${k === "danger" ? "alert" : "status"}">${icon(ic)}<div class="toast__text">${o.title ? `<strong>${o.title}</strong>` : ""}${o.text || ""}</div>
      <div class="toast__actions">${o.actions || ""}${btn("Fechar aviso", { variant: "ghost", size: "sm", iconOnly: true, icon: "x", attrs: { "data-dismiss": "toast" } })}</div></div>`;
  }
  function skeletonRows(n, cols) {
    let out = "";
    for (let i = 0; i < n; i++) {
      out += `<tr aria-hidden="true"><td class="modrow__check"><span class="skeleton" style="width:18px;height:18px"></span></td><td><div class="modrow__name"><span class="skeleton skeleton--tile"></span><div class="grow"><span class="skeleton skeleton--line" style="--w:${40 + ((i * 37) % 40)}%"></span><span class="skeleton skeleton--line" style="--w:${60 + ((i * 23) % 30)}%;height:8px"></span></div></div></td>${Array.from({ length: (cols || 6) - 2 }, (_, j) => `<td><span class="skeleton skeleton--line" style="--w:${50 + ((i + j) * 13) % 40}%"></span></td>`).join("")}</tr>`;
    }
    return out;
  }

  // ---------- Progresso e teste ----------
  // value: 0 a 100, ou null para indeterminado. state: running | waiting | error | done
  function progress(o = {}) {
    const ind = o.value == null;
    const st = o.state && o.state !== "running" ? "progress--" + o.state : "";
    const pid = o.id || id("pg");
    return `<div class="progress ${ind ? "progress--indeterminate" : ""} ${st} ${o.size === "sm" ? "progress--sm" : ""}">
      ${o.label || !ind ? `<div class="progress__top"><span class="progress__label" id="${pid}-l">${o.label || ""}</span>${!ind && o.showValue !== false ? `<span class="progress__value">${Math.round(o.value)}%</span>` : ""}</div>` : ""}
      <div class="progress__track" role="progressbar" aria-labelledby="${pid}-l"${ind ? "" : ` aria-valuenow="${Math.round(o.value)}" aria-valuemin="0" aria-valuemax="100"`}${o.valueText ? ` aria-valuetext="${esc(o.valueText)}"` : ""}><div class="progress__fill" style="--p:${ind ? 0 : o.value}%"></div></div>
      ${o.meta ? `<div class="progress__meta">${o.meta}</div>` : ""}</div>`;
  }
  // items: nomes; current: índice; o.waiting / o.error: índice com esse estado
  function steps(items, current, o = {}) {
    return `<ol class="steps" aria-label="${esc(o.label || "Etapas")}">${items.map((name, i) => {
      let cls = "", mark = i + 1, cur = "", sr = "";
      if (i < current) { cls = "steps__item--done"; mark = icon("check"); sr = " (concluída)"; }
      if (i === current) { cur = ' aria-current="step"'; sr = " (agora)"; }
      if (i === o.waiting) { cls = "steps__item--waiting"; mark = "!"; sr = " (esperando você)"; }
      if (i === o.error) { cls = "steps__item--error"; mark = icon("x"); sr = " (com erro)"; }
      return `<li class="steps__item ${cls}"${cur}><span class="steps__mark" aria-hidden="true">${mark}</span><span class="steps__label">${name}<span class="sr-only">${sr}</span></span></li>`;
    }).join("")}</ol>`;
  }
  // lines: [nivel, hora, origem, mensagem]. nivel: info | warn | error | warden
  function consoleLines(lines) {
    return lines.map(([lvl, time, src, msg]) => `<div class="console__line console__line--${lvl}"><span class="console__time">${time}</span><span class="console__lvl">${{ info: "INFO", warn: "WARN", error: "ERROR", warden: "WARDEN" }[lvl]}</span><span class="console__src" title="${esc(src)}">${esc(src)}</span><span class="console__msg">${esc(msg)}</span></div>`).join("");
  }
  function consoleBox(o = {}) {
    const st = o.state || "live";
    const stTxt = { live: '<span class="live" aria-hidden="true"></span>Ao vivo', paused: `${icon("pause", "icon--sm")}Rolagem pausada`, ended: `${icon("square", "icon--sm")}Jogo fechado`, waiting: '<span class="loader" aria-hidden="true"><i></i><i></i><i></i><i></i></span>Esperando o jogo abrir' }[st];
    const body = o.lines && o.lines.length ? consoleLines(o.lines) : `<div class="console__empty">Nenhuma linha ainda. A saída do jogo aparece aqui assim que ele começar a abrir.</div>`;
    return `<section class="console" aria-label="${esc(o.label || "Console do jogo")}" style="${o.height ? `height:${o.height}px` : ""}">
      <div class="console__bar"><span class="console__state">${stTxt}</span>
        ${select({ bare: true, size: "sm", ariaLabel: "Nível das linhas", options: [["all", "Todas as linhas"], ["warn", "Só avisos e erros"], ["error", "Só erros"]], value: o.level || "all" })}
        ${input({ bare: true, size: "sm", icon: "search", placeholder: "Buscar no console", ariaLabel: "Buscar no console", value: o.query })}
        <span class="grow"></span>
        ${btn(st === "paused" ? "Retomar rolagem" : "Pausar rolagem", { size: "sm", variant: "ghost", icon: st === "paused" ? "play" : "pause" })}
        ${btn("Copiar tudo", { size: "sm", variant: "ghost", icon: "copy" })}
        ${btn("Salvar em arquivo…", { size: "sm", variant: "ghost", icon: "download" })}</div>
      <div class="console__log" role="log" aria-live="off" tabindex="0">${body}</div>
      ${st === "paused" ? `<div class="console__paused"><span>${o.newLines || 23} linhas novas desde que você pausou.</span>${btn("Ir para o fim", { size: "sm", icon: "chevron-down" })}</div>` : ""}
    </section>`;
  }
  // lines: [tipo, numAntigo, numNovo, texto]. tipo: ctx | add | del | fold
  function diff(o = {}) {
    const add = o.lines.filter((l) => l[0] === "add").length, del = o.lines.filter((l) => l[0] === "del").length;
    const body = o.lines.map(([t, a, b, code]) => (t === "fold" ? `<div class="diff__fold">${code}</div>` :
      `<div class="diff__line diff__line--${t}"><span class="diff__num">${a || ""}</span><span class="diff__num">${b || ""}</span><span class="diff__sign" aria-hidden="true">${t === "add" ? "+" : t === "del" ? "−" : ""}</span><span class="diff__code">${t === "add" ? '<span class="sr-only">Adicionada: </span>' : t === "del" ? '<span class="sr-only">Removida: </span>' : ""}${esc(code)}</span></div>`)).join("");
    return `<figure class="diff"><figcaption class="diff__head"><span class="path">${esc(o.file)}</span><span class="diff__stats"><span class="add">+${add}</span> <span class="del">−${del}</span></span></figcaption><div class="diff__body">${body}</div></figure>`;
  }

  // ---------- Camadas ----------
  function dialog(o = {}) {
    const did = o.id || id("dlg");
    return `<div class="scrim" data-layer="dialog"${o.static ? " data-static" : ""}${o.esc ? ` data-esc="${esc(o.esc)}"` : ""}><div class="dialog ${o.size ? "dialog--" + o.size : ""}" role="${o.alert ? "alertdialog" : "dialog"}" aria-modal="true" aria-labelledby="${did}-t"${o.sub ? ` aria-describedby="${did}-d"` : ""}>
      <div class="dialog__head"><div><h2 class="dialog__title" id="${did}-t">${o.ai ? icon("sparkles", "icon--lg icon--ai") : ""}${o.title}</h2>${o.sub ? `<p class="dialog__sub" id="${did}-d">${o.sub}</p>` : ""}</div>
        ${o.noClose ? "" : btn("Fechar", { variant: "ghost", size: "sm", iconOnly: true, icon: "x", attrs: Object.assign({ "data-close": "" }, o.closeAttrs || {}) })}</div>
      <div class="dialog__body">${o.body || ""}</div>
      ${o.foot ? `<div class="dialog__foot ${o.footSplit ? "dialog__foot--split" : ""}">${o.foot}</div>` : ""}</div></div>`;
  }
  function drawer(o = {}) {
    const did = o.id || id("drw");
    return `<div class="scrim scrim--drawer" data-layer="drawer"${o.static ? " data-static" : ""}${o.esc ? ` data-esc="${esc(o.esc)}"` : ""}><div class="drawer" role="dialog" aria-modal="true" aria-labelledby="${did}-t">
      <div class="drawer__head"><h2 class="t-title" id="${did}-t">${o.title}</h2>${btn("Fechar painel", { variant: "ghost", size: "sm", iconOnly: true, icon: "x", attrs: Object.assign({ "data-close": "" }, o.closeAttrs || {}) })}</div>
      <div class="drawer__body">${o.body || ""}</div>${o.foot ? `<div class="drawer__foot">${o.foot}</div>` : ""}</div></div>`;
  }
  // items: { label, desc, icon, danger, disabled, attrs } | "sep" | { group: "Título" }
  function menu(items, o = {}) {
    return `<div class="menu" role="menu" id="${o.id || "menu"}"${o.static ? " data-static" : ""}${o.labelledby ? ` aria-labelledby="${o.labelledby}"` : ` aria-label="${esc(o.label || "Menu")}"`} style="${o.style || ""}">${items.map((it) => {
      if (it === "sep") return `<div class="menu__sep" role="separator"></div>`;
      if (it.group) return `<div class="menu__label" role="presentation">${it.group}</div>`;
      return `<button type="button" role="menuitem" class="menu__item ${it.danger ? "menu__item--danger" : ""} ${it.state ? "is-" + it.state : ""}" tabindex="-1"${it.disabled ? ' aria-disabled="true"' : ""}${attrs(it.attrs)}>${it.icon ? icon(it.icon) : "<span></span>"}<span>${it.label}</span>${it.end || "<span></span>"}${it.desc ? `<span class="menu__desc">${it.desc}</span>` : ""}</button>`;
    }).join("")}</div>`;
  }

  // ---------- IA e Histórico ----------
  function meter(level) {
    const n = { baixa: 1, média: 2, alta: 3 }[level] || 0;
    return `<span class="meter"><span class="meter__blocks" aria-hidden="true">${[1, 2, 3].map((i) => `<i class="${i <= n ? "on" : ""}"></i>`).join("")}</span><span>Confiança ${level}</span></span>`;
  }
  function aiBlock(o = {}) {
    if (o.loading) {
      return `<div class="ai-block ai-block--loading" aria-busy="true"><div class="ai-block__head">${icon("sparkles")}<span class="ai-block__title">A IA está lendo o log</span><span class="ai-block__meta">${o.meta || "Google Gemini"}</span></div>
        <div class="ai-block__body"><div class="row"><span class="loader" aria-hidden="true"><i></i><i></i><i></i><i></i></span><span>Isso costuma levar de 10 a 30 segundos. Você pode sair desta tela: a resposta fica guardada aqui.</span></div>
        <div><span class="skeleton skeleton--line" style="--w:70%"></span><span class="skeleton skeleton--line" style="--w:55%"></span><span class="skeleton skeleton--line" style="--w:62%"></span></div></div></div>`;
    }
    if (o.error) {
      return `<div class="ai-block"><div class="ai-block__head">${icon("sparkles")}<span class="ai-block__title">A IA não respondeu</span><span class="ai-block__meta">${o.meta || ""}</span></div>
        <div class="ai-block__body">${alert({ kind: "danger", title: o.error, text: o.errorText || "Nada foi alterado no pack.", actions: o.actions || "" })}</div></div>`;
    }
    return `<section class="ai-block" aria-labelledby="ai-cause">
      <div class="ai-block__head">${icon("sparkles")}<span class="ai-block__title">Resposta da IA</span><span class="ai-block__meta">${o.meta || ""}</span></div>
      <div class="ai-block__body">
        ${alert({ kind: "neutral", compact: true, icon: "info", title: "A IA pode errar.", text: "Confira antes de mudar o pack. Os botões abaixo passam pelas confirmações normais do Warden." })}
        <div><div class="ai-block__label" id="ai-cause">Causa provável</div><p>${o.cause}</p></div>
        <div class="row row--wrap row--gap-4"><div><div class="ai-block__label">Confiança</div>${meter(o.confidence || "média")}</div>
          <div><div class="ai-block__label">Mods envolvidos</div><div class="row row--wrap">${(o.mods || []).join("")}</div></div></div>
        <div><div class="ai-block__label">Passos sugeridos</div><ol class="ai-steps">${(o.steps || []).map(([t, a]) => `<li><span>${t}</span>${a || "<span></span>"}</li>`).join("")}</ol></div>
      </div>
      <div class="ai-block__foot">${o.foot || "A IA não muda nada sozinha."}</div></section>`;
  }
  // kind: unsaved | saved | final | published
  function tlItem(o = {}) {
    const stateTag = { unsaved: tag("Não salvas", "warn"), saved: tag("Só salva", "plain"), final: tag("Versão final · não publicada", "primary", { icon: "flag" }), published: tag("Publicada", "ok", { icon: "globe" }) }[o.kind];
    return `<li class="tl-item tl-item--${o.kind} ${o.open ? "tl-item--open" : ""}"><span class="tl-node" aria-hidden="true"></span>
      <div class="tl-body"><div class="tl-head"><span class="tl-version">${o.version}</span>${stateTag}<span class="tl-date">${o.date || ""}</span><span class="tl-actions">${o.actions || ""}</span></div>
      ${o.summary ? `<p class="tl-summary">${o.summary}</p>` : ""}${o.detail ? `<div class="tl-detail">${o.detail}</div>` : ""}</div></li>`;
  }


  // ---------- Estrutura do app ----------
  // Barra do nível do app. o.back: [rótulo, attrs]; o.where: texto de onde você está; o.end: HTML à direita
  function topbar(o = {}) {
    const back = o.back ? btn(o.back[0], { variant: "ghost", size: "sm", icon: "arrow-left", attrs: o.back[1] }) + '<span class="topbar__sep" aria-hidden="true"></span>' : "";
    const settings = o.noSettings ? "" : btn("Configurações", { variant: "ghost", icon: "settings", attrs: o.settingsAttrs });
    return `<header class="topbar">${back}<span class="brand">${brandMark()}<span class="brand__name">Warden</span></span>
      ${o.where ? `<span class="topbar__sep" aria-hidden="true"></span><span class="topbar__where">${o.where}</span>` : ""}
      <span class="topbar__end">${o.end || ""}${settings}</span></header>`;
  }
  // Rodapé com o indicador de Tarefas. tasks.state: idle | busy | error
  function statusbar(o = {}) {
    const t = o.tasks || {};
    const st = t.state || "idle";
    const lead = st === "busy" ? '<span class="loader" aria-hidden="true"><i></i><i></i><i></i><i></i></span>' : st === "error" ? icon("circle-alert", "icon--sm") : icon("list", "icon--sm");
    const text = t.text || "Nenhuma tarefa em andamento";
    return `<footer class="statusbar"><button type="button" class="statusbar__tasks statusbar__tasks--${st}"${attrs(Object.assign({ "aria-haspopup": "dialog" }, t.attrs || {}))}>${lead}<span>${text}</span></button>
      <span class="statusbar__end">${o.end || "Warden 0.1.0"}</span></footer>`;
  }
  // Cabeçalho fixo do pack
  function packHeader(o = {}) {
    const alerts = (o.alerts || []).map((a) => `<button type="button" class="headalert headalert--${a.kind || "warn"}"${attrs(a.attrs)}>${icon(a.icon || "triangle-alert", "icon--sm")}${a.text}</button>`).join("");
    const n = o.unsaved;
    const save = n
      ? btn("Salvar versão", { icon: "save", count: `${n}<span class="btn__word">&nbsp;${n === 1 ? "alteração" : "alterações"}</span>`, attrs: Object.assign({ "aria-label": `Salvar versão, ${n} ${n === 1 ? "alteração não salva" : "alterações não salvas"}` }, o.saveAttrs || {}) })
      : btn("Salvar versão", { icon: "save", disabled: true, tip: "Nada para salvar: o pack está igual à última versão salva" });
    return `<header class="packhead" aria-label="Pack aberto"><div class="packhead__row">
      ${btn("Meus packs", { variant: "ghost", size: "sm", icon: "arrow-left", attrs: o.backAttrs })}
      <span class="packhead__sep" aria-hidden="true"></span>
      <div class="packhead__id">${tile(o.name, "lg")}<div class="packhead__titles">
        <div class="packhead__name"><h1>${esc(o.name)}</h1>${btn("Editar informações", { variant: "ghost", size: "sm", iconOnly: true, icon: "pencil", attrs: o.editAttrs })}</div>
        <div class="packhead__meta"><span>Minecraft ${o.mc}</span><span class="dot" aria-hidden="true"></span><span>${o.loader}</span><span class="dot" aria-hidden="true"></span><span>versão ${o.version}</span></div></div></div>
      <div class="packhead__actions">${save}${testButton(Object.assign({ menuId: o.menuId || "menu-testar" }, o.test || {}))}</div></div>
      ${alerts ? `<div class="packhead__alerts" role="status">${alerts}</div>` : ""}
    </header>`;
  }
  // Menu de seções. items: { id, name, desc, icon, count, countKind, countLabel, ai, attrs }
  function sectionMenu(items, active, o = {}) {
    return `<nav class="secmenu" aria-label="${esc(o.label || "Seções do pack")}"><ul>${items.map((it) => {
      const cur = it.id === active;
      const cnt = it.count != null && it.count !== "" ? count(it.count, it.countKind, it.countLabel) : "";
      return `<li><a class="secmenu__item ${it.ai ? "secmenu__item--ai" : ""} ${it.state ? "is-" + it.state : ""}" href="${it.href || "#"}"${cur ? ' aria-current="page"' : ""}${attrs(it.attrs)}>${icon(it.icon)}<span class="secmenu__name">${it.name}</span>${cnt}<span class="secmenu__desc">${it.desc}</span></a></li>`;
    }).join("")}</ul>${o.after || ""}</nav>`;
  }
  // Linha de mod. m: { name, desc, ver, src, side, update, flags[], selected, kind(error|warn|invalid|new|outside), id }
  function modRow(m, o = {}) {
    const cls = ["modrow", m.kind ? "modrow--" + m.kind : "", m.state ? "is-" + m.state : ""].join(" ");
    const sideSel = m.kind === "invalid" ? "" : select({ bare: true, size: "sm", ariaLabel: `Lado de ${m.name}`, options: [["both", "Cliente e servidor"], ["client", "Só cliente"], ["server", "Só servidor"]], value: m.side || "both" });
    const upd = m.update === "available" ? `<span class="modrow__update modrow__update--available">${icon("arrow-right", "icon--sm")}<span class="sr-only">atualização disponível:</span>${m.updateTo}</span>` : m.update === "checking" ? '<span class="modrow__update t-3"><span class="loader" aria-hidden="true"><i></i><i></i><i></i><i></i></span> verificando</span>' : "";
    const title = m.kind === "invalid" ? `<span class="modrow__title">${esc(m.file)}</span>` : `<button type="button" class="modrow__title"${attrs(m.attrs)}>${esc(m.name)}</button>`;
    return `<tr class="${cls}"${m.selected ? ' aria-selected="true"' : ""}>
      <td class="modrow__check">${check({ checked: m.selected, ariaLabel: `Selecionar ${m.name || m.file}`, attrs: { "data-row": m.id || null } })}</td>
      <td><div class="modrow__name">${m.kind === "invalid" ? `<span class="tile" aria-hidden="true" style="display:grid;place-items:center;color:var(--color-danger-text)">${icon("file-code")}</span>` : tile(m.name)}<div class="modrow__titles"><span class="modrow__line">${title}${(m.flags || []).join("")}</span><span class="modrow__desc">${esc(m.desc || "")}</span></div></div></td>
      <td><span class="modrow__ver">${esc(m.ver || (m.kind === "invalid" ? "" : "—"))}</span>${upd}</td>
      <td>${m.src ? source(m.src) : ""}</td>
      <td>${sideSel}</td>
      ${o.rowEnd ? `<td class="shrink">${o.rowEnd}</td>` : ""}</tr>`;
  }
  const MOD_HEAD = `<thead><tr><th class="modrow__check"><span class="sr-only">Selecionar</span></th><th><button type="button" class="sort">Nome ${'$I'}</button></th><th>Versão</th><th>Fonte</th><th><span data-tip="Onde o mod precisa estar instalado: no jogo de quem joga (cliente), no servidor ou nos dois" tabindex="0" style="border-bottom:1px dotted var(--color-text-3)">Lado</span></th></tr></thead>`;
  function modTableHead() { return MOD_HEAD.replace("$I", icon("arrow-up-down", "icon--sm")); }

  // Linha de pack (Meus packs). p: { name, mc, loader, version, test:[tipo, texto], unsaved, when, kind(missing|unreadable), path }
  function packRow(p, o = {}) {
    if (p.kind === "missing" || p.kind === "unreadable") {
      const msg = p.kind === "missing" ? `Pasta não encontrada: ${esc(p.path)}` : "Não foi possível ler este pack";
      const why = p.kind === "missing" ? "Foi movida ou apagada fora do Warden." : "O pack.toml tem um erro na linha 4.";
      const acts = p.kind === "missing" ? btn("Localizar…", { size: "sm", attrs: o.locateAttrs }) + btn("Remover da lista", { size: "sm", variant: "ghost", attrs: o.removeAttrs }) : btn("Ver detalhes", { size: "sm", attrs: o.detailAttrs }) + btn("Remover da lista", { size: "sm", variant: "ghost", attrs: o.removeAttrs });
      return `<tr class="packrow packrow--missing"><td><div class="packrow__name">${tile(p.name, "lg")}<div><span class="t-strong">${esc(p.name)}</span><div class="t-xs t-danger">${msg}</div></div></div></td><td colspan="4" class="t-3">${why}</td><td class="shrink"><div class="row">${acts}</div></td></tr>`;
    }
    return `<tr class="packrow"><td><div class="packrow__name">${tile(p.name, "lg")}<div><button type="button" class="packrow__title"${attrs(o.openAttrs)}>${esc(p.name)}</button><div class="t-xs t-3">Minecraft ${p.mc} · ${p.loader}</div></div></div></td>
      <td class="t-mono">${p.version}</td><td>${status(p.test[0], p.test[1])}</td><td class="num">${p.unsaved ? `<span class="t-warn">${p.unsaved}</span>` : '<span class="t-3">0</span>'}</td><td class="t-3">${p.when}</td>
      <td class="shrink"><div class="row">${btn("Abrir", { size: "sm", variant: "primary", attrs: Object.assign({ "aria-label": `Abrir ${p.name}` }, o.openAttrs || {}) })}${btn(`Mais ações para ${p.name}`, { size: "sm", variant: "ghost", iconOnly: true, icon: "ellipsis", attrs: o.menuAttrs })}</div></td></tr>`;
  }
  const PACK_HEAD = `<thead><tr><th>Pack</th><th>Versão</th><th>Último teste</th><th class="num">Não salvas</th><th>Alterado</th><th class="shrink"><span class="sr-only">Ações</span></th></tr></thead>`;
  function packTableHead() { return PACK_HEAD; }

  window.W = { esc, attrs, id, icon, tile, tileSvg, brandMark, art, btn, testButton, input, textarea, select, omnibox, switchCtl, check, choice, segmented, memory, tag, source, side, badge, count, status, alert, issue, empty, toast, skeletonRows, progress, steps, consoleLines, consoleBox, diff, dialog, drawer, menu, meter, aiBlock, tlItem, topbar, statusbar, packHeader, sectionMenu, modRow, modTableHead, packRow, packTableHead, SOURCES, SIDES };
})();
