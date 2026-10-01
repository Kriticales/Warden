// Telas 1–5: Início, Criar modpack, Editor do pack, Adicionar conteúdo, Configs.
(function () {
  const { D, S, esc, ticon, pill, status, src, sideNames, sideIcons, chip, toast, dialog, closeDialog, routes, actions, go, render } = W;

  W.screenList.push(
    ["inicio", "1. Início (meus modpacks)"],
    ["novo", "2. Criar modpack"],
    ["pack/mods", "3. Editor do pack — Mods"],
    ["pack/resourcepacks", "3b. Editor — Resource packs"],
    ["pack/shaders", "3c. Editor — Shaders"],
    ["pack/arquivos", "3d. Editor — Arquivos do pack"],
    ["adicionar", "4. Adicionar conteúdo"],
    ["pack/configs", "5. Editor de configs"],
  );

  // ---------------------------------------------------------------------------
  // Partes compartilhadas
  // ---------------------------------------------------------------------------
  function loadingSr(text) {
    return `<p class="sr-only" role="status">${esc(text)}</p>`;
  }
  function emptyState({ art = "package", title, body, actions: acts = "", error = false }) {
    return `<div class="empty${error ? " error" : ""}">
      <span class="art" aria-hidden="true">${blockArt(art, error)}</span>
      <h2>${title}</h2><p>${body}</p>${acts ? `<div class="row wrap">${acts}</div>` : ""}</div>`;
  }
  // Ilustração simples (bloco isométrico) — sem marcas de terceiros
  // Ilustração em pixel art original: um bloco de pedra escura com veias e um
  // ícone no centro. Sem nenhuma textura de terceiros.
  const ART = [
    "................",
    "...##########...",
    "..#bbbbbbbbbb#..",
    ".#bbvbbbbbbvbb#.",
    ".#bvvbbbbbbbvb#.",
    ".#bbbbbbbbbbbb#.",
    ".#bbbbbbbbbbbb#.",
    ".#bbbbbbbbbbbb#.",
    ".#bbbbbbbbbbbb#.",
    ".#bbbbbbbbbbbb#.",
    ".#bvbbbbbbbbbb#.",
    ".#bvvbbbbbbvvb#.",
    ".#dddddddddddd#.",
    "..############..",
  ];
  function blockArt(kind, error) {
    const col = { "#": "var(--c-bg-sunken)", b: "var(--c-surface-3)", d: "var(--c-surface-2)", v: error ? "var(--c-danger)" : "var(--c-accent)" };
    let r = "";
    ART.forEach((row, y) => [...row].forEach((ch, x) => col[ch] && (r += `<rect x="${x}" y="${y}" width="1" height="1" fill="${col[ch]}"${ch === "v" ? ' fill-opacity=".6"' : ""}/>`)));
    const glyph = error ? icon("alert") : icon(kind);
    return `<span class="art-wrap"><svg viewBox="0 0 16 15" width="80" height="75" shape-rendering="crispEdges" aria-hidden="true">${r}</svg><span class="art-glyph" style="color:${error ? "var(--c-danger-text)" : "var(--c-accent-text)"}">${glyph}</span></span>`;
  }
  W.emptyState = emptyState;
  W.loadingSr = loadingSr;

  function packHead() {
    const p = S.pack;
    return `<header class="pack-head">
      ${ticon(p.name, "lg")}
      <div class="titles">
        <h1 id="page-title" tabindex="-1">${esc(p.name)}</h1>
        <div class="meta">
          ${chip("Minecraft " + p.mc)} ${chip(p.loader + " " + p.loaderVersion)} ${chip("Versão " + p.version, "tag")}
          ${S.unsaved ? `<a class="unsaved" href="#/versoes">${icon("edit", "sm")}${S.unsaved} alterações não salvas</a>` : `<span class="status ok">${icon("ok", "sm")}Tudo salvo</span>`}
        </div>
      </div>
      <div class="page-actions">
        <a class="btn" href="#/versoes">${icon("save")}Salvar versão</a>
        <a class="btn btn-primary btn-test" href="#/testar">${icon("play")}Testar</a>
      </div>
    </header>`;
  }
  W.packHead = packHead;

  function packTabs(cur) {
    const t = (k, label, n) =>
      `<a href="#/pack/${k}"${cur === k ? ' aria-current="page"' : ""}>${label}${n !== undefined ? ` <span class="count">${n}</span>` : ""}</a>`;
    return `<nav class="tabs" aria-label="Partes do pack">
      ${t("mods", "Mods", S.mods.length)}${t("resourcepacks", "Resource packs", D.resourcepacks.length)}${t("shaders", "Shaders", D.shaders.length)}${t("configs", "Configs")}${t("arquivos", "Arquivos")}
    </nav>`;
  }
  W.packTabs = packTabs;

  // ---------------------------------------------------------------------------
  // 1. Início
  // ---------------------------------------------------------------------------
  routes["inicio"] = {
    title: "Meus modpacks",
    nav: "inicio",
    states: ["normal", "carregando", "vazio", "erro"],
    render() {
      const head = `<div class="page-head">
        <div class="titles"><h1 id="page-title" class="page-title" tabindex="-1">Meus <span class="hl">modpacks</span></h1>
          <p class="page-sub">Cada modpack é uma pasta no formato do packwiz. Abra um para editar, testar e salvar versões.</p></div>
        <div class="page-actions">
          <button class="btn" data-action="soon" data-msg="Abre um pack packwiz que já existe no computador (pasta com pack.toml).">${icon("folderOpen")}Abrir pack existente</button>
          <a class="btn btn-primary" href="#/novo">${icon("plus")}Novo modpack</a>
        </div></div>`;
      if (S.view === "carregando") {
        const sk = Array.from({ length: 6 }, () => `<div class="card" aria-hidden="true"><div class="pack-card"><div class="top"><span class="skel" style="width:48px;height:48px"></span><div class="grow stack-sm"><span class="skel" style="height:16px;width:70%"></span><span class="skel" style="height:12px;width:45%"></span></div></div><div class="facts"><span class="skel" style="height:28px"></span><span class="skel" style="height:28px"></span></div><div class="foot"><span class="skel" style="height:20px;width:40%"></span></div></div></div>`).join("");
        return `<div class="page">${head}${loadingSr("Carregando seus modpacks…")}<div class="pack-grid">${sk}</div></div>`;
      }
      if (S.view === "vazio") {
        return `<div class="page">${head}${emptyState({
          art: "package",
          title: "Nenhum modpack ainda",
          body: "Crie o seu primeiro: você escolhe a versão do Minecraft e o loader, e o Warden prepara a pasta no formato do packwiz para você.",
          acts: `<a class="btn btn-primary" href="#/novo">${icon("plus")}Criar meu primeiro modpack</a><button class="btn" data-action="soon">${icon("folderOpen")}Abrir pack existente</button>`,
        })}</div>`;
      }
      if (S.view === "erro") {
        return `<div class="page">${head}${emptyState({
          error: true,
          title: "Não conseguimos abrir a pasta de modpacks",
          body: "A pasta <code>D:\\Modpacks</code> não foi encontrada. Talvez o disco tenha sido desconectado ou a pasta mudou de lugar.",
          acts: `<button class="btn btn-primary" data-action="retry-home">${icon("refresh")}Tentar de novo</button><a class="btn" href="#/ajustes">${icon("folder")}Escolher outra pasta</a>`,
        })}</div>`;
      }
      const hq = (S.homeQ || "").trim().toLowerCase();
      const shown = D.packs.filter((p) => !hq || p.name.toLowerCase().includes(hq));
      const cards = shown
        .map((p) => {
          const href = "#/pack/mods";
          return `<article class="card pack-card">
          <div class="top">${ticon(p.name, "lg")}
            <div class="grow"><h2><a href="${href}" data-action="open-pack" data-id="${p.id}">${esc(p.name)}</a></h2>
              <div class="chips">${chip("Minecraft " + p.mc)}${chip(p.loader)}</div></div></div>
          <dl class="facts">
            <div class="fact"><dt class="k">Mods</dt><dd class="v num" style="margin:0">${p.mods}</dd></div>
            <div class="fact"><dt class="k">Última versão salva</dt><dd class="v" style="margin:0">${p.version ? `<span class="mono">${p.version}</span> <span class="muted xs">· ${esc(p.savedAgo)}</span>` : `<span class="muted">Nenhuma ainda</span>`}</dd></div>
          </dl>
          <div class="foot">
            <div class="stack-sm"><span class="xs muted" style="display:block">Último teste${p.test.when ? ` · ${esc(p.test.when)}` : ""}</span>${pill(p.test.kind, p.test.text)}</div>
            ${p.test.kind === "danger" ? `<a class="btn btn-sm" href="#/diagnostico">${icon("activity", "sm")}Ver diagnóstico</a>` : `<a class="btn btn-sm" href="#/testar" aria-label="Testar ${esc(p.name)}">${icon("play", "sm")}Testar</a>`}
          </div>
        </article>`;
        })
        .join("");
      return `<div class="page">${head}
        <div class="toolbar"><div class="input-icon">${icon("search")}<input class="input" type="search" placeholder="Filtrar por nome" aria-label="Filtrar modpacks por nome" data-fk="home-q" data-input="home-q" value="${esc(S.homeQ || "")}"></div>
          <label class="sr-only" for="home-sort">Ordenar</label>
          <select id="home-sort" class="select" style="width:auto"><option>Editados recentemente</option><option>Nome (A–Z)</option><option>Versão do Minecraft</option></select></div>
        ${cards ? `<div class="pack-grid">${cards}</div>` : `<p class="text-2" style="padding:var(--space-8) 0">Nenhum modpack com “${esc(S.homeQ)}” no nome.</p>`}</div>`;
    },
  };
  actions["home-q"] = (el) => {
    S.homeQ = el.value;
    render();
  };
  actions["retry-home"] = () => W.setView("carregando");
  actions["open-pack"] = (el) => {
    if (el.dataset.id !== "vale") toast("No protótipo, todo pack abre o exemplo “Vale das Engrenagens”.", "info");
    go("pack/mods");
  };

  // ---------------------------------------------------------------------------
  // 2. Criar modpack
  // ---------------------------------------------------------------------------
  function mcIndex(v) {
    const all = D.mcVersions.flatMap((g) => g.items);
    return all.indexOf(v);
  }
  // compara pela ordem do manifesto (nunca por texto): índice menor = mais novo
  const atLeast = (v, min) => mcIndex(v) <= mcIndex(min);
  function loaderRule(loader, mc) {
    if (loader === "NeoForge" && !atLeast(mc, "1.20.1")) return "O NeoForge existe a partir do Minecraft 1.20.1.";
    if (loader === "Fabric" && !atLeast(mc, "1.14.4")) return "O Fabric existe a partir do Minecraft 1.14.";
    return null;
  }
  function javaFor(mc) {
    if (atLeast(mc, "26.1")) return 25;
    if (atLeast(mc, "1.20.6")) return 21;
    if (atLeast(mc, "1.18.2")) return 17;
    if (atLeast(mc, "1.17.1")) return 17;
    return 8;
  }
  W.javaFor = javaFor;
  function loaderVersions(loader, mc) {
    return D.loaderVersions[`${loader}|${mc}`] || D.loaderVersions[`${loader}|*`] || [{ v: loader === "Forge" ? "(mais recente para " + mc + ")" : "(mais recente)", rec: true }];
  }

  routes["novo"] = {
    title: "Criar modpack",
    nav: "inicio",
    states: ["normal"],
    render() {
      const n = S.np;
      const legacy = mcIndex(n.mc) > mcIndex("1.7.10");
      const nameErr = n.tried && !n.name.trim();
      const loaders = [
        { k: "Forge", d: "O loader mais antigo. Funciona em todas as versões, inclusive 1.7.10 e 1.12.2." },
        { k: "NeoForge", d: "Continuação moderna do Forge. Escolha comum para packs novos de 1.20.1 em diante." },
        { k: "Fabric", d: "Leve e rápido de atualizar. Muito usado em packs de otimização." },
      ];
      const lvs = loaderVersions(n.loader, n.mc);
      if (!lvs.some((x) => x.v === n.lv)) n.lv = (lvs.find((x) => x.rec) || lvs[0]).v;
      const templates = [
        { k: "vazio", t: "Começar vazio", d: "Só o loader. Você adiciona tudo." },
        { k: "otimizacao", t: "Otimização", d: "Mods de desempenho que combinam com a versão e o loader escolhidos." },
        { k: "tecnologia", t: "Tecnologia", d: "Base com Create, JEI e utilidades comuns." },
      ];
      const java = javaFor(n.mc);
      return `<div class="page">
        <div class="page-head"><div class="titles">
          <nav class="crumbs" aria-label="Você está em"><a href="#/inicio">Meus modpacks</a>${icon("chevronRight", "sm")}<span>Novo modpack</span></nav>
          <h1 id="page-title" class="page-title" tabindex="-1">Criar <span class="hl">modpack</span></h1>
          <p class="page-sub">Dá para mudar quase tudo depois, menos a combinação de versão do Minecraft e loader, que define quais mods funcionam.</p></div></div>
        <div class="split split-main-side">
          <form class="card" onsubmit="return false" novalidate aria-labelledby="page-title">
            <div class="card-body">
              <div class="field">
                <label class="label" for="np-name">Nome do modpack</label>
                <input id="np-name" class="input" data-fk="np-name" data-input="np-name" value="${esc(n.name)}" placeholder="Ex.: Vale das Engrenagens" aria-describedby="np-name-hint${nameErr ? " np-name-err" : ""}" ${nameErr ? 'aria-invalid="true"' : ""} autocomplete="off">
                <span class="hint" id="np-name-hint">Aparece para quem jogar. A pasta é criada com um nome simplificado: <code>${esc(slug(n.name) || "meu-modpack")}</code></span>
                ${nameErr ? `<span class="error-text" id="np-name-err">${icon("alert", "sm")}Dê um nome ao modpack para continuar.</span>` : ""}
              </div>
              <div class="field">
                <label class="label" for="np-mc">Versão do Minecraft</label>
                <select id="np-mc" class="select" data-change="np-mc" data-fk="np-mc" aria-describedby="np-mc-hint">
                  ${D.mcVersions.map((g) => `<optgroup label="${esc(g.group)}">${g.items.map((v) => `<option ${v === n.mc ? "selected" : ""}>${v}</option>`).join("")}</optgroup>`).join("")}
                </select>
                <span class="hint" id="np-mc-hint">De 1.7.10 até a mais nova com suporte completo. Versões mais antigas funcionam “no melhor esforço”.</span>
                ${legacy ? `<div class="alert warn" style="margin-top:var(--space-2)">${icon("warn")}<div><div class="atitle">Suporte limitado</div><div class="abody">Versões anteriores à 1.7.10 podem não abrir no teste. O Warden tenta, mas não garante.</div></div></div>` : ""}
              </div>
              <fieldset class="field" style="border:0;padding:0;margin-top:var(--space-5)">
                <legend class="label" style="margin-bottom:var(--space-2)">Loader</legend>
                <div class="choice-grid" style="grid-template-columns:repeat(3,minmax(0,1fr))">
                  ${loaders
                    .map((l) => {
                      const why = loaderRule(l.k, n.mc);
                      return `<label class="choice"><input type="radio" name="np-loader" value="${l.k}" data-change="np-loader" ${n.loader === l.k ? "checked" : ""} ${why ? "disabled" : ""} ${why ? `aria-describedby="why-${l.k}"` : ""}>
                    <span class="ctitle">${esc(l.k)}<span class="radio-dot" aria-hidden="true"></span></span>
                    <span class="cdesc">${esc(l.d)}</span>
                    ${why ? `<span class="creason" id="why-${l.k}">${icon("lock", "sm")}${esc(why)}</span>` : ""}</label>`;
                    })
                    .join("")}
                </div>
              </fieldset>
              <div class="field" style="margin-top:var(--space-5)">
                <label class="label" for="np-lv">Versão do ${esc(n.loader)}</label>
                <select id="np-lv" class="select" data-change="np-lv" data-fk="np-lv">
                  ${lvs.map((x) => `<option value="${x.v}" ${x.v === n.lv ? "selected" : ""} ${x.bad ? "disabled" : ""}>${x.v}${x.rec ? " — recomendada" : ""}${x.beta ? " — beta" : ""}${x.bad ? " — " + x.bad : ""}</option>`).join("")}
                </select>
                <span class="hint">A recomendada é a mais nova considerada estável. Versões com problema conhecido aparecem bloqueadas.</span>
              </div>
              <fieldset class="field" style="border:0;padding:0;margin-top:var(--space-5)">
                <legend class="label" style="margin-bottom:var(--space-2)">Começar a partir de <span class="opt">(opcional)</span></legend>
                <div class="choice-grid" style="grid-template-columns:repeat(3,minmax(0,1fr))">
                  ${templates
                    .map(
                      (t) => `<label class="choice"><input type="radio" name="np-tpl" value="${t.k}" data-change="np-tpl" ${n.tpl === t.k ? "checked" : ""}>
                    <span class="ctitle">${esc(t.t)}<span class="radio-dot" aria-hidden="true"></span></span><span class="cdesc">${esc(t.d)}</span></label>`,
                    )
                    .join("")}
                </div>
              </fieldset>
            </div>
            <div class="card-foot"><a class="btn" href="#/inicio">Cancelar</a><button class="btn btn-primary" data-action="np-create">${icon("plus")}Criar modpack</button></div>
          </form>
          <aside class="card card-pad" aria-labelledby="np-sum" style="align-self:start">
            <h2 class="section-title" id="np-sum">O que vai ser criado</h2>
            <dl class="kv" style="margin-top:var(--space-4)">
              <dt>Pasta</dt><dd><code>D:\\Modpacks\\${esc(slug(n.name) || "meu-modpack")}</code></dd>
              <dt>Jogo</dt><dd>Minecraft ${esc(n.mc)}</dd>
              <dt>Loader</dt><dd>${esc(n.loader)} ${esc(n.lv)}</dd>
              <dt>Java</dt><dd>Java ${java} <span class="muted">(baixado sozinho no primeiro teste)</span></dd>
              <dt>Formato</dt><dd>packwiz (<code>pack.toml</code>, <code>index.toml</code>)</dd>
            </dl>
            <hr class="hr">
            <p class="small text-2">Junto vão regras para que caches, logs e mundos de teste <strong>nunca</strong> entrem no pack.</p>
          </aside>
        </div></div>`;
    },
  };
  function slug(s) {
    return String(s).normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
  }
  actions["np-name"] = (el) => {
    S.np.name = el.value;
    render();
  };
  actions["np-mc"] = (el) => {
    S.np.mc = el.value;
    if (loaderRule(S.np.loader, S.np.mc)) S.np.loader = "Forge";
    render();
  };
  actions["np-loader"] = (el) => {
    S.np.loader = el.value;
    render();
    document.querySelector(`input[name="np-loader"][value="${el.value}"]`).focus();
  };
  actions["np-lv"] = (el) => {
    S.np.lv = el.value;
    render();
  };
  actions["np-tpl"] = (el) => {
    S.np.tpl = el.value;
    render();
    document.querySelector(`input[name="np-tpl"][value="${el.value}"]`).focus();
  };
  actions["np-create"] = () => {
    S.np.tried = true;
    if (!S.np.name.trim()) {
      render();
      document.getElementById("np-name").focus();
      return;
    }
    toast(`“${esc(S.np.name)}” criado. No protótipo, abrimos o exemplo “Vale das Engrenagens”.`);
    go("pack/mods");
  };

  // ---------------------------------------------------------------------------
  // 3. Editor do pack — Mods
  // ---------------------------------------------------------------------------
  function visibleMods() {
    const q = S.modQuery.trim().toLowerCase();
    return S.mods.filter((m) => {
      if (q && !(m.name.toLowerCase().includes(q) || m.author.toLowerCase().includes(q))) return false;
      if (S.modFilter === "problemas") return !!issueOf(m);
      if (S.modFilter === "atualizacoes") return !!m.upd;
      if (S.modFilter === "cliente") return m.side === "cliente";
      if (S.modFilter === "naosalvos") return !!m.changed;
      return true;
    });
  }
  function issueOf(m) {
    if (m.id === "waystones" && S.balmAdded) return null;
    if (m.id === "packutils" && m.side !== "?") return null;
    return m.issue || null;
  }
  W.issueOf = issueOf;

  function modRow(m) {
    const iss = issueOf(m);
    const sel = S.selected.has(m.id);
    return `<tr class="${sel ? "selected" : ""}">
      <td class="col-check"><input type="checkbox" class="check-input" aria-label="Selecionar ${esc(m.name)}" data-change="mod-select" data-id="${m.id}" data-fk="sel-${m.id}" ${sel ? "checked" : ""} style="width:16px;height:16px;accent-color:var(--c-accent)"></td>
      <td><div class="modname">${ticon(m.name)}<div style="min-width:0"><button class="nm" data-action="mod-open" data-id="${m.id}" data-fk="open-${m.id}">${esc(m.name)}</button>
        <div class="sub">${esc(m.author)}${m.lib ? " · biblioteca" : ""}${m.changed ? ` · <span style="color:var(--c-warn-text)">${esc(m.changed)}</span>` : ""}</div></div></div></td>
      <td><span class="ver">${esc(m.ver)}</span>${m.upd ? `<span class="upd" title="Atualização disponível">${icon("arrowUp", "sm")}${esc(m.upd)}<span class="sr-only"> disponível</span></span>` : ""}</td>
      <td>${src(m.src)}</td>
      <td><label class="sr-only" for="side-${m.id}">Lado de ${esc(m.name)}</label>
        <select id="side-${m.id}" class="select side-select" data-change="mod-side" data-id="${m.id}" data-fk="side-${m.id}">
          ${["ambos", "cliente", "servidor"].map((s) => `<option value="${s}" ${m.side === s ? "selected" : ""}>${sideNames[s]}</option>`).join("")}
          ${m.side === "?" ? `<option value="?" selected>Não identificado</option>` : ""}
        </select></td>
      <td>${iss ? status(iss.kind, iss.text) : `<span class="muted xs">—</span>`}</td>
      <td style="text-align:right"><button class="btn btn-ghost btn-icon btn-sm" aria-label="Mais ações para ${esc(m.name)}" data-action="mod-open" data-id="${m.id}">${icon("more")}</button></td>
    </tr>`;
  }

  routes["pack/mods"] = {
    title: "Editor do pack — Mods",
    nav: "conteudo",
    states: ["normal", "carregando", "vazio", "erro"],
    render() {
      let body;
      const addBtn = `<a class="btn btn-primary" href="#/adicionar">${icon("plus")}Adicionar conteúdo</a>`;
      if (S.view === "carregando") {
        const rows = Array.from({ length: 9 }, () => `<tr aria-hidden="true"><td class="col-check"><span class="skel" style="width:16px;height:16px"></span></td><td><div class="modname"><span class="skel" style="width:36px;height:36px"></span><div class="grow stack-sm"><span class="skel" style="height:12px;width:60%"></span><span class="skel" style="height:10px;width:35%"></span></div></div></td><td><span class="skel" style="height:12px;width:70px"></span></td><td><span class="skel" style="height:12px;width:80px"></span></td><td><span class="skel" style="height:24px;width:90px"></span></td><td><span class="skel" style="height:12px;width:100px"></span></td><td></td></tr>`).join("");
        body = `${loadingSr("Lendo os mods do pack…")}<div class="loading-line" style="margin-bottom:var(--space-4)"><span class="spinner"></span>Lendo os arquivos do pack e buscando nomes e ícones…</div><div class="table-wrap"><table class="table">${tableHead(false)}<tbody>${rows}</tbody></table></div>`;
      } else if (S.view === "vazio") {
        body = emptyState({
          art: "puzzle",
          title: "Este pack ainda não tem mods",
          body: "Busque no Modrinth e no CurseForge de uma vez só. Os resultados já vêm filtrados para Minecraft " + S.pack.mc + " e " + S.pack.loader + ".",
          acts: addBtn + `<a class="btn" href="#/adicionar" data-action="add-tab" data-tab="arquivo">${icon("upload")}Adicionar arquivo .jar</a>`,
        });
      } else {
        const vis = visibleMods();
        const updates = S.mods.filter((m) => m.upd).length;
        const probs = S.mods.filter(issueOf).length;
        const f = (k, label, n) => `<button aria-pressed="${S.modFilter === k}" data-action="mod-filter" data-k="${k}" data-fk="mf-${k}">${label}${n !== undefined ? ` <span class="n">${n}</span>` : ""}</button>`;
        const allSel = vis.length && vis.every((m) => S.selected.has(m.id));
        const someSel = vis.some((m) => S.selected.has(m.id));
        const errRows =
          S.view === "erro"
            ? `<tr class="row-error"><td class="col-check"></td><td colspan="4"><div class="modname"><span class="ticon" aria-hidden="true">${icon("alert")}</span><div><strong class="mono small">mods/sodium-extra.pw.toml</strong><div class="sub" style="color:var(--c-danger-text)">Não foi possível ler: aspas não fechadas na linha 4.</div></div></div></td><td>${status("danger", "Arquivo com erro")}</td><td style="text-align:right;white-space:nowrap"><button class="btn btn-sm" data-action="soon">${icon("code", "sm")}Abrir arquivo</button></td></tr>`
            : "";
        body = `
          ${S.view === "erro" ? `<div class="alert danger" style="margin-bottom:var(--space-4)">${icon("error")}<div><div class="atitle">1 arquivo do pack não pôde ser lido</div><div class="abody">O resto do pack está normal. O item com problema aparece no topo da lista, com o motivo.</div></div></div>` : ""}
          ${updates ? `<div class="alert info" style="margin-bottom:var(--space-4)">${icon("update")}<div><div class="atitle">${updates} atualizações disponíveis</div><div class="abody">Antes de aplicar, você vê o que muda em cada mod.</div></div><div class="aactions"><button class="btn btn-sm" data-action="review-updates">Revisar atualizações</button></div></div>` : ""}
          <div class="toolbar" style="margin-bottom:var(--space-3)">
            <div class="input-icon">${icon("search")}<input class="input" type="search" placeholder="Filtrar mods" aria-label="Filtrar mods por nome ou autor" value="${esc(S.modQuery)}" data-input="mod-q" data-fk="mod-q"></div>
            <span class="grow"></span>
            <div class="segmented" role="group" aria-label="Ver como">
              <button aria-pressed="${S.modView === "cartoes"}" data-action="mod-view" data-v="cartoes" data-fk="mv-c">${icon("layers", "sm")}Cartões</button>
              <button aria-pressed="${S.modView !== "cartoes"}" data-action="mod-view" data-v="tabela" data-fk="mv-t">${icon("list", "sm")}Tabela</button>
            </div>
            ${addBtn}
          </div>
          <div class="filter-chips" role="group" aria-label="Mostrar" style="margin-bottom:var(--space-4)">
            ${f("todos", "Todos", S.mods.length)}${f("problemas", "Com problemas", probs)}${f("atualizacoes", "Com atualização", updates)}${f("cliente", "Só no cliente")}${f("naosalvos", "Não salvos", S.mods.filter((m) => m.changed).length)}
          </div>
          ${S.modView === "cartoes" ? modCards(vis) : `<div class="table-wrap"><table class="table" aria-label="Mods do pack">
            ${tableHead(true, allSel, someSel && !allSel)}
            <tbody>${errRows}${vis.map(modRow).join("") || `<tr><td colspan="7"><div style="padding:var(--space-8);text-align:center" class="text-2">Nenhum mod corresponde a “${esc(S.modQuery)}”${S.modFilter !== "todos" ? " neste filtro" : ""}. <button class="btn btn-sm btn-ghost" data-action="mod-clear">Limpar filtros</button></div></td></tr>`}</tbody>
          </table></div>`}
          <p class="xs muted" style="margin-top:var(--space-2)">Mostrando ${vis.length} de ${S.mods.length}. O lado diz onde o mod precisa estar: no jogo do jogador (cliente), no servidor ou nos dois.</p>
          ${S.selected.size ? bulkBar() : ""}`;
      }
      return `${packHead()}${packTabs("mods")}<div class="page wide">${body}</div>`;
    },
  };
  function tableHead(withCheck, all, mixed) {
    return `<thead><tr>
      <th class="col-check">${!withCheck ? `<span class="sr-only">Seleção</span>` : ""}${withCheck ? `<input type="checkbox" aria-label="Selecionar todos os mods visíveis" data-change="mod-select-all" data-fk="sel-all" ${all ? "checked" : ""} ${mixed ? 'data-mixed="1"' : ""} style="width:16px;height:16px;accent-color:var(--c-accent)">` : ""}</th>
      <th>Nome</th><th>Versão</th><th>Fonte</th><th>Lado</th><th>Avisos</th><th><span class="sr-only">Ações</span></th></tr></thead>`;
  }
  function bulkBar() {
    const n = S.selected.size;
    const upd = [...S.selected].filter((id) => S.mods.find((m) => m.id === id)?.upd).length;
    return `<div class="bulkbar" role="region" aria-label="Ações para os mods selecionados">
      <span class="count">${n} ${n === 1 ? "selecionado" : "selecionados"}</span>
      <label class="sr-only" for="bulk-side">Mudar lado dos selecionados</label>
      <select id="bulk-side" class="select side-select" data-change="bulk-side" data-fk="bulk-side"><option value="">Mudar lado…</option><option value="ambos">Ambos</option><option value="cliente">Só cliente</option><option value="servidor">Só servidor</option></select>
      <button class="btn btn-sm" data-action="bulk-update" ${upd ? "" : "disabled"}>${icon("arrowUp", "sm")}Atualizar${upd ? ` (${upd})` : ""}</button>
      <button class="btn btn-sm" data-action="soon" data-msg="Desativar mantém o mod no pack, mas ele não é carregado (arquivo .disabled).">${icon("pause", "sm")}Desativar</button>
      <button class="btn btn-sm btn-ghost danger" data-action="bulk-remove">${icon("trash", "sm")}Remover</button>
      <span class="grow"></span>
      <button class="btn btn-sm btn-ghost" data-action="bulk-clear">Limpar seleção</button>
    </div>`;
  }
  // Cartão de conteúdo: ícone + nome + descrição de 2 linhas + interruptor (ativo/desativado)
  function contentCard(m, opts = {}) {
    const iss = opts.noIssue ? null : issueOf(m);
    const on = !m.off;
    return `<li class="mod-card${on ? "" : " off"}${iss ? " has-issue " + iss.kind : ""}">
      ${ticon(m.name, "lg")}
      <div class="mc-main">
        <div class="mc-title"><button class="nm" data-action="${opts.open || "mod-open"}" data-id="${m.id}" data-fk="card-${m.id}">${esc(m.name)}</button>${m.upd ? `<span class="upd" title="Atualização disponível">${icon("arrowUp", "sm")}${esc(m.upd)}<span class="sr-only"> disponível</span></span>` : ""}</div>
        <p class="mc-desc">${esc(m.desc || "")}</p>
        <div class="mc-meta">${src(m.src)}<span class="chip">${icon(sideIcons[m.side], "sm")}${sideNames[m.side]}</span><span class="ver">${esc(m.ver)}</span>${m.changed ? `<span class="xs" style="color:var(--c-warn-text)">${icon("edit", "sm")} não salvo</span>` : ""}</div>
        ${iss ? `<div class="mc-issue">${status(iss.kind, iss.text)}</div>` : ""}
      </div>
      <label class="switch mc-toggle" title="${on ? "Ativo" : "Desativado"}"><input type="checkbox" role="switch" ${on ? "checked" : ""} data-change="${opts.toggle || "mod-toggle"}" data-id="${m.id}" data-fk="tg-${m.id}" aria-label="${esc(m.name)} ativo"></label>
    </li>`;
  }
  W.contentCard = contentCard;
  function modCards(vis) {
    const err = S.view === "erro" ? `<li class="mod-card has-issue danger"><span class="ticon lg" aria-hidden="true">${icon("alert")}</span><div class="mc-main"><div class="mc-title"><strong class="mono small">mods/sodium-extra.pw.toml</strong></div><p class="mc-desc" style="color:var(--c-danger-text)">Não foi possível ler: aspas não fechadas na linha 4.</p><div class="mc-meta"><button class="btn btn-sm" data-action="soon">${icon("code", "sm")}Abrir arquivo</button></div></div></li>` : "";
    if (!vis.length && !err) return `<div class="card card-pad text-2" style="text-align:center">Nenhum mod corresponde a “${esc(S.modQuery)}”${S.modFilter !== "todos" ? " neste filtro" : ""}. <button class="btn btn-sm btn-ghost" data-action="mod-clear">Limpar filtros</button></div>`;
    return `<ul class="mod-grid" aria-label="Mods do pack">${err}${vis.map((m) => contentCard(m)).join("")}</ul>`;
  }
  actions["mod-view"] = (el) => {
    S.modView = el.dataset.v;
    render();
  };
  actions["mod-toggle"] = (el) => {
    const m = S.mods.find((x) => x.id === el.dataset.id);
    m.off = !el.checked;
    m.changed = m.changed || (m.off ? "Desativado (não salvo)" : "Ativado (não salvo)");
    S.unsaved++;
    render();
    toast(`<strong>${esc(m.name)}</strong> ${m.off ? "desativado: continua no pack, mas não carrega no jogo" : "ativado"}.`, "info");
  };
  W.routes["pack/mods"].after = () => {
    const all = document.querySelector('[data-change="mod-select-all"]');
    if (all && all.dataset.mixed) all.indeterminate = true;
  };
  actions["mod-q"] = (el) => {
    S.modQuery = el.value;
    render();
  };
  actions["mod-filter"] = (el) => {
    S.modFilter = el.dataset.k;
    render();
  };
  actions["mod-clear"] = () => {
    S.modFilter = "todos";
    S.modQuery = "";
    render();
  };
  actions["mod-select"] = (el) => {
    el.checked ? S.selected.add(el.dataset.id) : S.selected.delete(el.dataset.id);
    render();
  };
  actions["mod-select-all"] = (el) => {
    visibleMods().forEach((m) => (el.checked ? S.selected.add(m.id) : S.selected.delete(m.id)));
    render();
  };
  actions["bulk-clear"] = () => {
    S.selected.clear();
    render();
    document.querySelector('[data-fk="sel-all"]')?.focus();
  };
  actions["mod-side"] = (el) => {
    const m = S.mods.find((x) => x.id === el.dataset.id);
    m.side = el.value;
    m.changed = m.changed || "Lado alterado (não salvo)";
    S.unsaved++;
    render();
    toast(`Lado de <strong>${esc(m.name)}</strong>: ${sideNames[m.side]}.`);
  };
  actions["bulk-side"] = (el) => {
    if (!el.value) return;
    S.selected.forEach((id) => {
      const m = S.mods.find((x) => x.id === id);
      m.side = el.value;
    });
    toast(`${S.selected.size} mods marcados como “${sideNames[el.value]}”.`);
    S.unsaved++;
    render();
  };
  function applyUpdates(ids) {
    let n = 0;
    ids.forEach((id) => {
      const m = S.mods.find((x) => x.id === id);
      if (m && m.upd) {
        m.changed = `Atualizado de ${m.ver} (não salvo)`;
        m.ver = m.upd;
        delete m.upd;
        n++;
      }
    });
    S.unsaved += n;
    return n;
  }
  actions["bulk-update"] = () => {
    const n = applyUpdates([...S.selected]);
    toast(`${n} ${n === 1 ? "mod atualizado" : "mods atualizados"}. Lembre de testar antes de salvar a versão.`);
    render();
  };
  actions["bulk-remove"] = () => {
    const names = [...S.selected].map((id) => S.mods.find((m) => m.id === id).name);
    dialog({
      title: `Remover ${names.length} ${names.length === 1 ? "mod" : "mods"} do pack?`,
      body: `<p class="text-2">Estes mods saem do pack:</p><ul style="margin:var(--space-3) 0;list-style:disc;padding-left:var(--space-5)">${names.map((n) => `<li>${esc(n)}</li>`).join("")}</ul>
        <div class="alert warn">${icon("warn")}<div><div class="atitle">Mundos que usam blocos desses mods podem perder coisas</div><div class="abody">Dá para desfazer voltando a uma versão salva.</div></div></div>`,
      foot: `<button class="btn" data-action="dlg-close">Manter mods</button><button class="btn btn-danger" data-action="bulk-remove-ok">${icon("trash")}Remover ${names.length === 1 ? "mod" : "mods"}</button>`,
    });
  };
  actions["bulk-remove-ok"] = () => {
    const n = S.selected.size;
    S.mods = S.mods.filter((m) => !S.selected.has(m.id));
    S.selected.clear();
    S.unsaved++;
    closeDialog(true);
    render();
    toast(`${n} ${n === 1 ? "mod removido" : "mods removidos"} do pack.`);
    document.querySelector('[data-fk="mod-q"]')?.focus();
  };
  actions["review-updates"] = () => {
    const ups = S.mods.filter((m) => m.upd);
    dialog({
      title: "Revisar atualizações",
      sub: "Desmarque o que você não quer atualizar agora.",
      wide: true,
      body: `<ul class="dep-list">${ups
        .map(
          (m) => `<li><input type="checkbox" id="up-${m.id}" checked style="width:16px;height:16px;margin-top:3px;accent-color:var(--c-accent)" data-upd="${m.id}">
        <label for="up-${m.id}" class="grow" style="cursor:pointer"><span class="row">${ticon(m.name, "sm")}<strong>${esc(m.name)}</strong><span class="ver mono small text-2">${esc(m.ver)}</span>${icon("arrowRight", "sm")}<span class="mono small" style="color:var(--c-info-text)">${esc(m.upd)}</span></span>
        <span class="small text-2" style="display:block;margin-top:var(--space-1)">${updNote(m.id)}</span></label>${src(m.src)}</li>`,
        )
        .join("")}</ul>`,
      foot: `<button class="btn" data-action="dlg-close">Agora não</button><button class="btn btn-primary" data-action="updates-ok">${icon("arrowUp")}Atualizar selecionados</button>`,
    });
  };
  function updNote(id) {
    return (
      {
        createaddition: "Corrige compatibilidade com o Create 6.0.8.",
        iris: "Correções de sombras e de desempenho com shaders.",
        jade: "Pequenas correções na dica que aparece ao olhar para um bloco.",
        sophisticatedbackpacks: "Corrige itens sumindo ao trocar de dimensão.",
      }[id] || "Correções gerais."
    );
  }
  actions["updates-ok"] = () => {
    const ids = [...document.querySelectorAll("[data-upd]")].filter((c) => c.checked).map((c) => c.dataset.upd);
    closeDialog(true);
    const n = applyUpdates(ids);
    render();
    toast(`${n} mods atualizados. Rode um teste antes de salvar a versão.`);
    document.getElementById("page-title")?.focus();
  };

  // Gaveta de detalhes do mod
  actions["mod-open"] = (el) => {
    const m = S.mods.find((x) => x.id === el.dataset.id);
    const iss = issueOf(m);
    const toml = pwToml(m);
    dialog({
      drawer: true,
      lead: ticon(m.name, "lg"),
      title: esc(m.name),
      sub: `${esc(m.author)} · ${W.srcNames[m.src]}`,
      body: `<div class="stack">
        ${iss ? `<div class="alert ${iss.kind}">${icon(iss.kind === "danger" ? "error" : "warn")}<div><div class="atitle">${esc(iss.text)}</div><div class="abody">Veja os detalhes e a correção sugerida na checagem do <a href="#/testar/checagem">Testar</a>.</div></div></div>` : ""}
        <div class="field"><label class="label" for="dv">Versão</label>
          <select id="dv" class="select">${[m.upd, m.ver].filter(Boolean).map((v, i) => `<option ${v === m.ver ? "selected" : ""}>${esc(v)}${i === 0 && m.upd ? " — mais nova" : v === m.ver ? " — no pack" : ""}</option>`).join("")}<option>Ver todas as versões…</option></select></div>
        <fieldset class="field" style="border:0;padding:0;margin:0"><legend class="label">Lado</legend>
          <div class="segmented" role="group" aria-label="Lado" style="margin-top:var(--space-1)">${["cliente", "servidor", "ambos"].map((s) => `<button aria-pressed="${m.side === s}" data-action="drawer-side" data-id="${m.id}" data-side="${s}">${icon(sideIcons[s], "sm")}${sideNames[s]}</button>`).join("")}</div>
          <span class="hint" style="margin-top:var(--space-1)">${m.src === "modrinth" ? "Detectado automaticamente pelas informações do Modrinth." : m.src === "local" ? "Arquivo local sem essa informação: escolha você." : "O CurseForge não informa o lado; confira se precisar de servidor."}</span></fieldset>
        <label class="switch"><input type="checkbox" role="switch"><span><span class="label">Opcional para o jogador</span><span class="hint" style="display:block">Quem instala pode escolher não baixar este mod.</span></span></label>
        <label class="switch"><input type="checkbox" role="switch"><span><span class="label">Fixar nesta versão</span><span class="hint" style="display:block">Fica fora de “atualizar todos”.</span></span></label>
        <details><summary class="small" style="cursor:pointer;color:var(--c-text-2)">Ver o arquivo do packwiz (<code>mods/${esc(m.id)}.pw.toml</code>)</summary>
          <div class="code" style="margin-top:var(--space-2)"><table>${toml.map((l, i) => `<tr><td class="ln">${i + 1}</td><td>${hl(l)}</td></tr>`).join("")}</table></div></details>
      </div>`,
      foot: `<button class="btn btn-ghost danger left" data-action="soon" data-msg="Abriria a confirmação de remoção, como na ação em lote.">${icon("trash")}Remover do pack</button>${m.src !== "local" ? `<button class="btn" data-action="soon" data-msg="Abre a página do mod no navegador.">${icon("external")}Abrir página</button>` : ""}<button class="btn btn-primary" data-action="dlg-close">Concluir</button>`,
    });
  };
  actions["drawer-side"] = (el) => {
    const m = S.mods.find((x) => x.id === el.dataset.id);
    m.side = el.dataset.side;
    el.parentElement.querySelectorAll("button").forEach((b) => b.setAttribute("aria-pressed", String(b === el)));
    render();
  };
  function pwToml(m) {
    if (m.src === "local") return [`# arquivo local: o próprio .jar fica em mods/${m.file}`, `# e é listado no index.toml com hash`];
    const side = { cliente: "client", servidor: "server", ambos: "both", "?": "both" }[m.side];
    const fn = `${m.id}-${S.pack.mc}-${m.ver}.jar`;
    if (m.src === "curseforge")
      return [`name = "${m.name}"`, `filename = "${fn}"`, `side = "${side}"`, ``, `[download]`, `hash-format = "sha1"`, `hash = "9c1e…"`, `mode = "metadata:curseforge"`, ``, `[update]`, `[update.curseforge]`, `file-id = 6108240`, `project-id = 238222`];
    return [`name = "${m.name}"`, `filename = "${fn}"`, `side = "${side}"`, ``, `[download]`, `url = "https://cdn.modrinth.com/data/…/${fn}"`, `hash-format = "sha512"`, `hash = "3b1f…"`, ``, `[update]`, `[update.modrinth]`, `mod-id = "…"`, `version = "…"`];
  }
  // realce mínimo de TOML/props
  function hl(line) {
    const l = esc(line);
    if (/^\s*#/.test(line)) return `<span class="cm">${l}</span>`;
    if (/^\s*\[.*\]\s*$/.test(line)) return `<span class="sec">${l}</span>`;
    const m = l.match(/^(\s*)([\w.-]+)(\s*=\s*)(.*)$/);
    if (m) return `${m[1]}<span class="key">${m[2]}</span>${m[3]}<span class="val">${m[4]}</span>`;
    return l;
  }
  W.hl = hl;

  // ---------------------------------------------------------------------------
  // 3b/3c. Resource packs e Shaders
  // ---------------------------------------------------------------------------
  function simpleList(items, kind) {
    if (S.view === "vazio")
      return emptyState({
        art: kind === "shaders" ? "sun" : "image",
        title: kind === "shaders" ? "Nenhum shader no pack" : "Nenhum resource pack no pack",
        body: kind === "shaders" ? "Shaders mudam a luz e as sombras do jogo. Para funcionar, o pack precisa do Iris (ou do OptiFine em versões antigas)." : "Resource packs trocam texturas, sons e a interface do jogo.",
        acts: `<a class="btn btn-primary" href="#/adicionar">${icon("plus")}Adicionar ${kind === "shaders" ? "shader" : "resource pack"}</a>`,
      });
    return `${kind === "shaders" ? `<div class="alert ok" style="margin-bottom:var(--space-4)">${icon("ok")}<div><div class="atitle">O Iris já está no pack</div><div class="abody">Os shaders abaixo funcionam. O jogador escolhe qual usar no menu de vídeo.</div></div></div>` : ""}
      <div class="toolbar"><p class="small text-2">${kind === "shaders" ? "O jogador escolhe qual shader usar no menu de vídeo. Desativar tira do pack sem apagar." : "Desativar tira do pack sem apagar. A ordem de prioridade fica nas opções do jogo (Configs → options.txt)."}</p><span class="grow"></span><a class="btn btn-primary" href="#/adicionar">${icon("plus")}Adicionar</a></div>
      <ul class="mod-grid">${items.map((r) => W.contentCard(r, { noIssue: !r.issue, open: "soon", toggle: "rp-toggle" })).join("")}</ul>
      <details class="passed" style="margin-top:var(--space-4);border:0"><summary>${icon("chevronRight", "sm")}Ver como tabela</summary>
      <div class="table-wrap"><table class="table"><thead><tr><th>Nome</th><th>Versão</th><th>Fonte</th><th>Tamanho no pack</th><th>Avisos</th></tr></thead><tbody>
      ${items
        .map(
          (r) => `<tr><td><div class="modname">${ticon(r.name)}<div><strong>${esc(r.name)}</strong><div class="sub">${esc(r.author)}</div></div></div></td>
        <td><span class="ver">${esc(r.ver)}</span></td><td>${src(r.src)}</td>
        <td class="small text-2">${r.size ? `${esc(r.size)} (arquivo inteiro)` : "só a referência (menos de 1 KB)"}</td>
        <td>${r.issue ? `${status(r.issue.kind, r.issue.text)} <button class="btn btn-sm" data-action="soon" data-msg="Adicionaria Entity Model Features e Entity Texture Features.">Adicionar os 2 mods</button>` : `<span class="muted xs">—</span>`}</td></tr>`,
        )
        .join("")}</tbody></table></div></details>`;
  }
  actions["rp-toggle"] = (el) => {
    const r = [...D.resourcepacks, ...D.shaders].find((x) => x.id === el.dataset.id);
    r.off = !el.checked;
    S.unsaved++;
    render();
  };
  routes["pack/resourcepacks"] = {
    title: "Editor do pack — Resource packs",
    nav: "conteudo",
    states: ["normal", "vazio"],
    render: () => `${packHead()}${packTabs("resourcepacks")}<div class="page wide">${simpleList(D.resourcepacks, "rp")}</div>`,
  };
  routes["pack/shaders"] = {
    title: "Editor do pack — Shaders",
    nav: "conteudo",
    states: ["normal", "vazio"],
    render: () => `${packHead()}${packTabs("shaders")}<div class="page wide">${simpleList(D.shaders, "shaders")}</div>`,
  };

  // ---------------------------------------------------------------------------
  // 3d. Arquivos do pack (o que entra / regras)
  // ---------------------------------------------------------------------------
  const folders = [
    { p: "mods/", d: "Mods (referências e arquivos locais)", n: "25 itens", on: true, fixed: true },
    { p: "resourcepacks/", d: "Resource packs", n: "3 itens", on: true },
    { p: "shaderpacks/", d: "Shaders", n: "2 itens", on: true },
    { p: "config/", d: "Configs dos mods", n: "31 arquivos", on: true },
    { p: "defaultconfigs/", d: "Configs de servidor padrão (valem para mundos novos)", n: "3 arquivos", on: true },
    { p: "kubejs/", d: "Scripts do KubeJS", n: "não existe", on: false, missing: true },
    { p: "options.txt", d: "Opções do jogo (só as chaves escolhidas, na primeira execução)", n: "3 chaves", on: true },
  ];
  routes["pack/arquivos"] = {
    title: "Editor do pack — Arquivos",
    nav: "conteudo",
    states: ["normal"],
    render() {
      const packToml = [`name = "${S.pack.name}"`, `author = "${S.set.player}"`, `version = "${S.pack.version}"`, `pack-format = "packwiz:1.1.0"`, ``, `[index]`, `file = "index.toml"`, `hash-format = "sha256"`, `hash = "9f2c…"`, ``, `[versions]`, `minecraft = "${S.pack.mc}"`, `neoforge = "${S.pack.loaderVersion}"`];
      return `${packHead()}${packTabs("arquivos")}<div class="page wide">
        <div class="split split-main-side">
          <section class="card" aria-labelledby="inc-t">
            <div class="card-head"><div><h2 class="section-title" id="inc-t">O que faz parte do pack</h2><p class="section-sub">Só o que estiver ligado aqui entra no pack e na exportação. Todo o resto (logs, mundos, caches) fica de fora sempre.</p></div></div>
            <ul>${folders
              .map(
                (f) => `<li class="set-row" style="padding:var(--space-3) var(--space-5)"><div class="row">${icon(f.p.endsWith("/") ? "folder" : "fileText", "lg")}<div><div class="mono small"><strong>${esc(f.p)}</strong></div><div class="small text-2">${esc(f.d)} · <span class="muted">${esc(f.n)}</span></div></div></div>
              ${f.fixed ? `<span class="chip locked">${icon("lock", "sm")}Sempre incluído</span>` : f.missing ? `<button class="btn btn-sm" data-action="soon">${icon("plus", "sm")}Criar pasta</button>` : `<label class="switch"><input type="checkbox" role="switch" ${f.on ? "checked" : ""} aria-label="Incluir ${esc(f.p)} no pack"></label>`}</li>`,
              )
              .join("")}</ul>
          </section>
          <aside class="stack">
            <section class="card" aria-labelledby="pw-t"><div class="card-head"><h2 class="section-title" id="pw-t">Arquivos do packwiz</h2></div>
              <div class="card-body stack-sm">
                <p class="small text-2">Gerados e mantidos pelo Warden. É isso que torna o pack 100% packwiz.</p>
                <div class="code"><table>${packToml.map((l, i) => `<tr><td class="ln">${i + 1}</td><td>${hl(l)}</td></tr>`).join("")}</table></div>
                <p class="xs muted"><code>pack.toml</code> · também: <code>index.toml</code> (68 arquivos) e <code>.packwizignore</code></p>
              </div></section>
            <section class="card card-pad"><h2 class="section-title">Pastas</h2>
              <dl class="kv" style="margin-top:var(--space-3)"><dt>Projeto do pack</dt><dd><code>D:\\Modpacks\\vale-das-engrenagens</code></dd><dt>Instância de teste</dt><dd><code>%APPDATA%\\Warden\\instances\\vale-das-engrenagens</code></dd></dl>
              <p class="xs muted" style="margin-top:var(--space-3)">São pastas separadas de propósito: testar nunca suja o pack.</p>
              <div class="row" style="margin-top:var(--space-3)"><button class="btn btn-sm" data-action="soon">${icon("folderOpen", "sm")}Abrir pasta do pack</button></div></section>
          </aside>
        </div></div>`;
    },
  };

  // ---------------------------------------------------------------------------
  // 4. Adicionar conteúdo
  // ---------------------------------------------------------------------------
  const popular = [
    { id: "cookingforblockheads", name: "Cooking for Blockheads", author: "BlayTheNinth", desc: "Uma cozinha de verdade: livro de receitas, geladeira, fogão e balcões que mostram o que dá para cozinhar com o que você tem.", dl: "48 mi", updated: "18 set 2026", srcs: ["modrinth", "curseforge"] },
    { id: "steamrails", name: "Create: Steam 'n' Rails", author: "Layers of Railways", desc: "Mais trilhos, sinais, engates e estilos de locomotiva para os trens do Create.", dl: "9,1 mi", updated: "3 set 2026", srcs: ["modrinth", "curseforge"] },
    { id: "sophisticatedstorage", name: "Sophisticated Storage", author: "P3pp3rF1y", desc: "Baús, barris e caixas que podem ser melhorados, com filtros e ordenação.", dl: "62 mi", updated: "26 set 2026", srcs: ["curseforge"] },
    { id: "embeddium", name: "Embeddium", author: "embeddedt", desc: "Motor de renderização alternativo para Forge e NeoForge.", dl: "31 mi", updated: "2 jun 2026", srcs: ["modrinth", "curseforge"], conflict: true },
    { id: "farmersdelight", name: "Farmer's Delight", author: "vectorwing", desc: "Agricultura e culinária com panelas, frigideiras e tábuas de corte.", dl: "71 mi", updated: "9 set 2026", srcs: ["modrinth", "curseforge"], inpack: true },
    { id: "createdeco", name: "Create Deco", author: "MRH0", desc: "Blocos decorativos industriais: grades, portas, placas e luzes no estilo do Create.", dl: "11 mi", updated: "30 jul 2026", srcs: ["modrinth", "curseforge"] },
    { id: "naturescompass", name: "Nature's Compass", author: "Chaosyr", desc: "Uma bússola que aponta para o bioma que você escolher.", dl: "40 mi", updated: "14 ago 2026", srcs: ["modrinth", "curseforge"] },
  ];
  const detail = {
    cookingforblockheads: {
      versions: [{ v: "21.1.15", t: "Estável", d: "18 set 2026", rec: true }, { v: "21.1.14", t: "Estável", d: "2 set 2026" }, { v: "21.1.16-beta", t: "Beta", d: "27 set 2026" }],
      side: "Ambos (cliente e servidor)",
      req: [{ id: "balm", name: "Balm", note: "Biblioteca do mesmo autor. Não está no pack — será adicionada junto.", v: "21.0.31" }],
      opt: [{ id: "jei", name: "Just Enough Items (JEI)", note: "Mostra as receitas da cozinha no JEI.", inPack: true }],
    },
    steamrails: {
      versions: [{ v: "1.7.1", t: "Estável", d: "3 set 2026", rec: true }, { v: "1.7.0", t: "Estável", d: "14 ago 2026" }],
      side: "Ambos (cliente e servidor)",
      req: [{ id: "create", name: "Create", note: "Já está no pack (6.0.8).", inPack: true }],
      opt: [],
    },
    sophisticatedstorage: {
      versions: [{ v: "1.4.12", t: "Estável", d: "26 set 2026", rec: true }],
      side: "Ambos (o CurseForge não informa; vamos usar “Ambos”)",
      req: [{ id: "sophisticatedcore", name: "Sophisticated Core", note: "Já está no pack (1.2.31).", inPack: true }],
      opt: [],
    },
    embeddium: { versions: [{ v: "1.0.15", t: "Estável", d: "2 jun 2026", rec: true }], side: "Cliente", req: [], opt: [], conflict: "Embeddium e Sodium fazem a mesma coisa e não funcionam juntos. O Sodium já está no pack." },
  };
  function results() {
    const q = S.add.query.trim().toLowerCase();
    let r = popular.filter((x) => !q || x.name.toLowerCase().includes(q) || x.desc.toLowerCase().includes(q));
    if (!S.add.srcC) r = r.filter((x) => x.srcs.includes("modrinth"));
    if (!S.add.srcM) r = r.filter((x) => x.srcs.includes("curseforge"));
    if (S.view === "erro") r = r.filter((x) => x.srcs.includes("modrinth"));
    if (S.view === "vazio") r = [];
    return r;
  }
  routes["adicionar"] = {
    title: "Adicionar conteúdo",
    nav: "conteudo",
    states: ["normal", "carregando", "vazio", "erro"],
    render() {
      const t = S.add.tab;
      const tab = (k, ic, label) => `<button role="tab" id="tab-${k}" aria-controls="panel-${k}" aria-selected="${t === k}" tabindex="${t === k ? 0 : -1}" data-action="add-tab" data-tab="${k}" data-fk="addtab-${k}">${icon(ic)}${label}</button>`;
      return `<div class="page wide">
        <div class="page-head"><div class="titles">
          <nav class="crumbs" aria-label="Você está em"><a href="#/pack/mods">${esc(S.pack.name)}</a>${icon("chevronRight", "sm")}<a href="#/pack/mods">Conteúdo</a>${icon("chevronRight", "sm")}<span>Adicionar</span></nav>
          <h1 id="page-title" class="page-title" tabindex="-1">Adicionar <span class="hl">conteúdo</span></h1>
          <p class="page-sub">Tudo o que aparece aqui já foi filtrado para <strong>Minecraft ${esc(S.pack.mc)}</strong> e <strong>${esc(S.pack.loader)}</strong>, então funciona no seu pack.</p></div>
          <div class="page-actions"><a class="btn" href="#/pack/mods">${icon("arrowLeft")}Voltar ao pack</a></div></div>
        <div class="tabs inline" role="tablist" aria-label="Como adicionar" style="margin-bottom:var(--space-5);border-bottom:1px solid var(--c-border)">
          ${tab("buscar", "search", "Buscar")}${tab("link", "link", "Colar link")}${tab("arquivo", "upload", "Arquivo local ou URL")}
        </div>
        <div role="tabpanel" id="panel-${t}" aria-labelledby="tab-${t}">${t === "buscar" ? searchPanel() : t === "link" ? linkPanel() : filePanel()}</div>
      </div>`;
    },
  };
  function searchPanel() {
    const a = S.add;
    const r = results();
    const kind = (k, label) => `<button aria-pressed="${a.kind === k}" data-action="add-kind" data-k="${k}" data-fk="kind-${k}">${label}</button>`;
    let list;
    if (S.view === "carregando") {
      list = `${W.loadingSr("Buscando no Modrinth e no CurseForge…")}${Array.from({ length: 5 }, () => `<div class="result" aria-hidden="true"><span class="skel" style="width:48px;height:48px"></span><div class="stack-sm"><span class="skel" style="height:14px;width:50%"></span><span class="skel" style="height:10px;width:90%"></span><span class="skel" style="height:10px;width:70%"></span></div><span></span></div>`).join("")}`;
    } else if (!r.length) {
      list = `<div style="padding:var(--space-10) var(--space-6);text-align:center" class="stack-sm"><p><strong>Nada encontrado${a.query ? ` para “${esc(a.query)}”` : ""}</strong></p><p class="small text-2">Procuramos em Minecraft ${esc(S.pack.mc)} + ${esc(S.pack.loader)}. Tente outro nome, o nome em inglês, ou verifique se o mod existe para esta versão.</p>${a.query ? `<button class="btn btn-sm" data-action="add-clear">Limpar busca</button>` : ""}</div>`;
    } else {
      list = r
        .map((x) => {
          const added = a.added.has(x.id);
          const state = x.inpack || added ? pill("ok", added ? "Adicionado" : "No pack") : x.conflict ? pill("danger", "Incompatível") : "";
          return `<button class="result" role="option" aria-selected="${a.sel === x.id}" data-action="add-sel" data-id="${x.id}" data-fk="res-${x.id}">
            ${ticon(x.name, "lg")}<span style="min-width:0"><span class="rname">${esc(x.name)}</span> <span class="xs muted">por ${esc(x.author)}</span>
            <span class="rdesc">${esc(x.desc)}</span>
            <span class="rmeta"><span>${icon("download", "sm")} ${esc(x.dl)}</span><span>Atualizado em ${esc(x.updated)}</span></span></span>
            <span class="rright">${state}<span class="row" style="gap:var(--space-2)">${x.srcs.map((s) => `<span class="src ${s}" title="${W.srcNames[s]}"><span class="dot" aria-hidden="true"></span><span class="sr-only">${W.srcNames[s]}</span></span>`).join("")}</span></span>
          </button>`;
        })
        .join("");
    }
    const sel = popular.find((x) => x.id === a.sel) || r[0];
    return `
      ${S.view === "erro" ? `<div class="alert warn" style="margin-bottom:var(--space-4)">${icon("wifiOff")}<div><div class="atitle">O CurseForge não respondeu</div><div class="abody">Mostrando só resultados do Modrinth. O resto do Warden continua funcionando.</div></div><div class="aactions"><button class="btn btn-sm" data-action="retry-add">${icon("refresh", "sm")}Tentar de novo</button></div></div>` : ""}
      <div class="toolbar" style="margin-bottom:var(--space-3)">
        <div class="input-icon grow" style="max-width:560px">${icon("search")}<input class="input" type="search" placeholder="Buscar por nome ou assunto" aria-label="Buscar mods" value="${esc(a.query)}" data-input="add-q" data-fk="add-q"></div>
        <div class="segmented" role="group" aria-label="Tipo de conteúdo">${kind("mods", "Mods")}${kind("rp", "Resource packs")}${kind("shaders", "Shaders")}</div>
      </div>
      <div class="toolbar filters-row" role="group" aria-label="Filtros">
        <span class="xs muted">Sempre filtrado por</span>
        <span class="chip locked" title="Travado na versão do pack">${icon("lock", "sm")}Minecraft ${esc(S.pack.mc)}</span>
        <span class="chip locked" title="Travado no loader do pack">${icon("lock", "sm")}${esc(S.pack.loader)}</span>
        <span class="vsep" aria-hidden="true"></span>
        <span class="xs muted">Fontes</span>
        <label class="check"><input type="checkbox" data-change="add-src" data-k="srcM" ${a.srcM ? "checked" : ""}>${src("modrinth")}</label>
        <label class="check"><input type="checkbox" data-change="add-src" data-k="srcC" ${a.srcC ? "checked" : ""}>${src("curseforge")}</label>
        <span class="grow"></span>
        <label class="sr-only" for="add-sort">Ordenar por</label>
        <select id="add-sort" class="select side-select"><option>Mais relevantes</option><option>Mais baixados</option><option>Atualizados recentemente</option></select>
      </div>
      <div class="split" style="grid-template-columns:minmax(0,1fr) minmax(360px,440px);align-items:start">
        <section class="card" aria-labelledby="res-t">
          <div class="card-head"><h2 class="section-title" id="res-t">${a.query ? `Resultados para “${esc(a.query)}”` : `Populares para ${esc(S.pack.loader)} ${esc(S.pack.mc)}`}</h2><span class="xs muted">${S.view === "carregando" ? "" : r.length + " resultados"}</span></div>
          <div class="results"${r.length && S.view !== "carregando" ? ' role="listbox" aria-labelledby="res-t"' : ""}>${list}</div>
        </section>
        ${S.view === "carregando" || !r.length ? `<section class="card card-pad hide-narrow"><p class="text-2 small">Escolha um resultado para ver detalhes, versões e dependências.</p></section>` : detailPanel(sel)}
      </div>`;
  }
  function detailPanel(x) {
    const a = S.add;
    const d = detail[x.id] || { versions: [{ v: "—", t: "Estável", d: x.updated, rec: true }], side: "Ambos", req: [], opt: [] };
    const ver = a.ver && d.versions.some((v) => v.v === a.ver) ? a.ver : d.versions[0].v;
    const reqNew = d.req.filter((r) => !r.inPack && !(r.id === "balm" && S.balmAdded));
    const optChecked = d.opt.filter((o) => !o.inPack && a.opt[o.id]);
    const total = 1 + reqNew.length + optChecked.length;
    const inPack = x.inpack || a.added.has(x.id);
    return `<section class="card" aria-labelledby="det-t" style="position:sticky;top:calc(var(--protobar-h) + var(--space-4))">
      <div class="card-body stack">
        <div class="row" style="align-items:flex-start;gap:var(--space-3)">${ticon(x.name, "lg")}<div class="grow"><h2 class="section-title" id="det-t">${esc(x.name)}</h2><p class="small text-2">por ${esc(x.author)} · ${esc(x.dl)} downloads</p></div></div>
        <p class="small text-2">${esc(x.desc)}</p>
        ${d.conflict ? `<div class="alert danger">${icon("error")}<div><div class="atitle">Incompatível com o seu pack</div><div class="abody">${esc(d.conflict)}</div></div></div>` : ""}
        ${x.srcs.length > 1 ? `<div class="field"><span class="label" id="srcpick">Baixar de</span><div class="segmented" role="group" aria-labelledby="srcpick">${x.srcs.map((sname) => `<button aria-pressed="${(a.pick || "modrinth") === sname}" data-action="add-pick" data-s="${sname}" data-fk="pick-${sname}">${src(sname)}</button>`).join("")}</div><span class="hint">Está nas duas fontes. Usamos uma só, para não duplicar o mod.</span></div>` : `<div class="small">Fonte: ${src(x.srcs[0])}</div>`}
        <div class="field"><label class="label" for="det-ver">Versão</label>
          <select id="det-ver" class="select" data-change="add-ver" data-fk="det-ver">${d.versions.map((v) => `<option value="${v.v}" ${v.v === ver ? "selected" : ""}>${v.v} · ${v.t} · ${v.d}${v.rec ? " — recomendada" : ""}</option>`).join("")}</select>
          ${/beta/i.test(ver) ? `<span class="error-text" style="color:var(--c-warn-text)">${icon("warn", "sm")}Versão beta: pode ter erros.</span>` : `<span class="hint">Lado: ${esc(d.side)}</span>`}</div>
        ${
          d.req.length || d.opt.length
            ? `<div><h3 class="label">Dependências</h3><ul class="dep-list" style="margin-top:var(--space-1)">
          ${d.req.map((r) => {
            const have = r.inPack || (r.id === "balm" && S.balmAdded);
            return `<li><input type="checkbox" checked disabled aria-label="${esc(r.name)} (obrigatória)" style="width:16px;height:16px;margin-top:3px;accent-color:var(--c-accent)"><div class="grow"><div class="row"><strong>${esc(r.name)}</strong>${pill(have ? "ok" : "info", have ? "Já no pack" : "Obrigatória", have ? "ok" : "lock")}</div><div class="small text-2">${esc(have ? "Já está no pack." : r.note)}</div></div></li>`;
          }).join("")}
          ${d.opt.map((o) => `<li><input type="checkbox" id="opt-${o.id}" ${o.inPack ? "checked disabled" : a.opt[o.id] ? "checked" : ""} data-change="add-opt" data-id="${o.id}" style="width:16px;height:16px;margin-top:3px;accent-color:var(--c-accent)"><label for="opt-${o.id}" class="grow"><div class="row"><strong>${esc(o.name)}</strong>${pill(o.inPack ? "ok" : "neutral", o.inPack ? "Já no pack" : "Opcional", o.inPack ? "ok" : "plus")}</div><div class="small text-2">${esc(o.note)}</div></label></li>`).join("")}
        </ul></div>`
            : ""
        }
      </div>
      <div class="card-foot" style="justify-content:space-between">
        <span class="small text-2">${inPack ? "Já faz parte do pack." : d.conflict ? "Não recomendado." : `${total} ${total === 1 ? "item" : "itens"} entram no pack`}</span>
        ${inPack ? `<a class="btn" href="#/pack/mods">Ver no pack</a>` : d.conflict ? `<button class="btn" data-action="add-do" data-id="${x.id}" data-n="1">Adicionar mesmo assim</button>` : `<button class="btn btn-primary" data-action="add-do" data-id="${x.id}" data-n="${total}">${icon("plus")}Adicionar${total > 1 ? ` ${total} itens` : ""}</button>`}
      </div>
    </section>`;
  }
  function linkPanel() {
    const a = S.add;
    const ok = a.linkState === "ok";
    const bad = a.linkState === "bad";
    return `<div class="split split-main-side"><section class="card card-pad stack">
      <div class="field"><label class="label" for="link-in">Link do mod</label>
        <div class="input-affix"><input id="link-in" class="input" value="${esc(a.link)}" data-input="link-in" data-fk="link-in" placeholder="https://modrinth.com/mod/… ou https://www.curseforge.com/minecraft/mc-mods/…" aria-describedby="link-hint${bad ? " link-err" : ""}" ${bad ? 'aria-invalid="true"' : ""}><button class="btn" data-action="link-check">${icon("search")}Verificar</button></div>
        <span class="hint" id="link-hint">Funciona com links de páginas do Modrinth e do CurseForge. Links do CurseForge são resolvidos pelo packwiz, o mesmo motor que monta o pack.</span>
        ${bad ? `<span class="error-text" id="link-err">${icon("alert", "sm")}Não reconhecemos este link. Cole o endereço da página do mod (começa com https://modrinth.com/ ou https://www.curseforge.com/).</span>` : ""}
      </div>
      ${a.linkState === "checking" ? `<div class="loading-line" role="status"><span class="spinner"></span>Consultando o CurseForge…</div>` : ""}
      ${ok ? `<div class="card" style="box-shadow:none"><div class="card-body row" style="align-items:flex-start;gap:var(--space-3)">${ticon("FramedBlocks", "lg")}<div class="grow stack-sm"><div class="row wrap"><strong>FramedBlocks</strong>${src("curseforge")}${pill("ok", "Compatível com o pack")}</div><p class="small text-2">Versão 10.3.1 para NeoForge 1.21.1 · lado: ambos (não informado pelo CurseForge)</p><p class="small text-2">Sem dependências obrigatórias.</p></div></div><div class="card-foot"><button class="btn btn-primary" data-action="link-add">${icon("plus")}Adicionar ao pack</button></div></div>` : ""}
    </section>
    <aside class="card card-pad stack-sm"><h2 class="section-title">Quando usar</h2><p class="small text-2">Se você já está com a página do mod aberta no navegador. O Warden faz as mesmas checagens da busca: versão, loader, dependências e duplicatas.</p></aside></div>`;
  }
  function filePanel() {
    return `<div class="split split-main-side"><section class="stack">
      <div class="dropzone">${icon("upload", "xl")}<strong>Arraste arquivos .jar ou .zip para cá</strong><span class="small">ou</span><button class="btn" data-action="soon" data-msg="Abriria o seletor de arquivos do Windows.">Escolher arquivos…</button></div>
      <section class="card" aria-labelledby="files-t"><div class="card-head"><h2 class="section-title" id="files-t">Arquivos analisados</h2></div>
        <ul>
          <li class="finding"><span class="sev ok">${icon("ok")}</span><div><div class="ftitle mono small">meu-ajuste-de-receitas-1.2.jar</div><div class="fbody small">É um mod para NeoForge 1.21.1 (lido de dentro do arquivo). Não está no Modrinth nem no CurseForge, então o arquivo inteiro vai dentro do pack (84 KB).</div>
            <div class="fix"><label class="sr-only" for="lf-side">Lado</label><select id="lf-side" class="select side-select"><option>Lado: escolha…</option><option>Ambos</option><option>Só cliente</option><option>Só servidor</option></select><button class="btn btn-sm btn-primary" data-action="soon" data-msg="Adicionaria o arquivo ao pack.">${icon("plus", "sm")}Adicionar</button></div></div></li>
          <li class="finding"><span class="sev info">${icon("info")}</span><div><div class="ftitle mono small">jei-1.21.1-neoforge-19.21.0.247.jar</div><div class="fbody small">Encontramos este arquivo no CurseForge. Ele já está no pack — nada a fazer.</div></div></li>
          <li class="finding"><span class="sev danger">${icon("error")}</span><div><div class="ftitle mono small">sodium-fabric-0.6.13+mc1.21.1.jar</div><div class="fbody small">Este arquivo é para Fabric. Seu pack usa NeoForge, então ele não funcionaria. Procure a versão para NeoForge na busca.</div></div></li>
        </ul></section>
      </section>
      <aside class="card card-pad stack"><h2 class="section-title">Por URL direta</h2>
        <div class="field"><label class="label" for="url-in">Endereço do arquivo</label><input id="url-in" class="input" placeholder="https://exemplo.com/arquivo.jar"><span class="hint">O Warden baixa uma vez para calcular o hash, e o pack guarda só o link.</span></div>
        <button class="btn" data-action="soon">${icon("download")}Verificar e adicionar</button></aside></div>`;
  }
  actions["add-tab"] = (el) => {
    S.add.tab = el.dataset.tab;
    if (S.route !== "adicionar") go("adicionar");
    else render();
  };
  actions["add-kind"] = (el) => {
    S.add.kind = el.dataset.k;
    if (el.dataset.k !== "mods") toast("No protótipo só a busca de mods tem resultados de exemplo.", "info");
    render();
  };
  actions["add-q"] = (el) => {
    S.add.query = el.value;
    render();
  };
  actions["add-clear"] = () => {
    S.add.query = "";
    render();
    document.querySelector('[data-fk="add-q"]')?.focus();
  };
  actions["add-src"] = (el) => {
    S.add[el.dataset.k] = el.checked;
    render();
  };
  actions["add-sel"] = (el) => {
    S.add.sel = el.dataset.id;
    S.add.ver = null;
    S.add.pick = null;
    render();
  };
  actions["add-pick"] = (el) => {
    S.add.pick = el.dataset.s;
    render();
  };
  actions["add-ver"] = (el) => {
    S.add.ver = el.value;
    render();
  };
  actions["add-opt"] = (el) => {
    S.add.opt[el.dataset.id] = el.checked;
    render();
  };
  actions["retry-add"] = () => W.setView("normal");
  actions["add-do"] = (el) => {
    const x = popular.find((p) => p.id === el.dataset.id);
    S.add.added.add(x.id);
    const extra = [];
    if (x.id === "cookingforblockheads" && !S.balmAdded) {
      S.balmAdded = true;
      S.mods.push({ id: "balm", name: "Balm", author: "BlayTheNinth", ver: "21.0.31", src: "modrinth", side: "ambos", lib: true, changed: "Adicionado (não salvo)" });
      extra.push("Balm");
    }
    S.mods.push({ id: x.id, name: x.name, author: x.author, ver: (detail[x.id]?.versions[0].v) || "1.0.0", src: x.srcs[0], side: "ambos", changed: "Adicionado (não salvo)" });
    S.unsaved += 1 + extra.length;
    render();
    toast(`<strong>${esc(x.name)}</strong>${extra.length ? " e <strong>Balm</strong>" : ""} adicionados ao pack.${extra.length ? " Isso também resolveu o aviso do Waystones." : ""}`);
  };
  actions["link-in"] = (el) => {
    S.add.link = el.value;
    if (S.add.linkState !== "idle") {
      S.add.linkState = "idle";
      render();
    }
  };
  actions["link-check"] = () => {
    const v = S.add.link.trim();
    if (!/^https:\/\/(www\.)?(modrinth\.com|curseforge\.com)\//.test(v)) {
      S.add.linkState = "bad";
      render();
      document.getElementById("link-in").focus();
      return;
    }
    S.add.linkState = "checking";
    render();
    W.after(900, () => {
      S.add.linkState = "ok";
      render();
    });
  };
  actions["link-add"] = () => {
    S.mods.push({ id: "framedblocks", name: "FramedBlocks", author: "XFactHD", ver: "10.3.1", src: "curseforge", side: "ambos", changed: "Adicionado (não salvo)" });
    S.unsaved++;
    S.add.linkState = "idle";
    render();
    toast("<strong>FramedBlocks</strong> adicionado ao pack.");
  };

  // ---------------------------------------------------------------------------
  // 5. Editor de configs
  // ---------------------------------------------------------------------------
  function cfgValues() {
    if (!S.cfg.values) {
      S.cfg.values = {};
      D.createServerToml.forEach((s) => s.fields.forEach((f) => (S.cfg.values[f.key] = f.value)));
    }
    return S.cfg.values;
  }
  function cfgChanges() {
    const v = cfgValues();
    const out = [];
    D.createServerToml.forEach((s) => s.fields.forEach((f) => v[f.key] !== f.orig && out.push({ f, s, now: v[f.key] })));
    return out;
  }
  function tomlLines() {
    const v = cfgValues();
    const lines = [];
    D.createServerToml.forEach((s, i) => {
      if (i) lines.push("");
      lines.push(s.title);
      s.fields.forEach((f) => {
        lines.push(`\t#${f.desc}`);
        if (f.min !== undefined) lines.push(`\t#Range: ${f.min} ~ ${f.max}`);
        lines.push(`\t${f.key} = ${v[f.key]}`);
      });
    });
    return lines;
  }
  function treeHtml(nodes, path = "", depth = 0) {
    return nodes
      .map((n) => {
        const p = path + n.name + (n.dir ? "/" : "");
        if (n.dir) {
          const open = S.cfg.open[n.name] !== undefined ? S.cfg.open[n.name] : n.open;
          return `<li role="treeitem" aria-expanded="${open}" tabindex="-1" data-action="cfg-toggle" data-name="${esc(n.name)}" data-fk="tree-${esc(p)}"><span class="node">${icon(open ? "chevronDown" : "chevronRight", "sm")}${icon(open ? "folderOpen" : "folder", "sm")}${esc(n.name)}</span>${open ? `<ul role="group">${treeHtml(n.children, p, depth + 1)}</ul>` : ""}</li>`;
        }
        const sel = S.cfg.file === p;
        const changed = (p === "defaultconfigs/create-server.toml" && cfgChanges().length) || n.changed;
        return `<li role="treeitem" aria-selected="${sel}" tabindex="${sel ? 0 : -1}" data-action="cfg-file" data-path="${esc(p)}" data-fk="tree-${esc(p)}"><span class="node">${icon("fileText", "sm")}<span class="grow" style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${esc(n.name)}</span>${changed ? `<span class="badge-mod" title="Alterado"></span><span class="sr-only"> (alterado)</span>` : ""}</span></li>`;
      })
      .join("");
  }
  routes["pack/configs"] = {
    title: "Editor de configs",
    nav: "configs",
    states: ["normal", "carregando", "vazio", "erro"],
    render() {
      const tree = `<section class="card" aria-labelledby="tree-t" style="align-self:start;position:sticky;top:calc(var(--protobar-h) + var(--space-4))">
        <div class="card-head" style="padding:var(--space-3) var(--space-4)"><h2 class="label" id="tree-t">Arquivos de config</h2><button class="btn btn-ghost btn-icon btn-sm" aria-label="Recarregar lista" data-action="soon">${icon("refresh", "sm")}</button></div>
        <div style="padding:var(--space-2) var(--space-3) var(--space-3)"><div class="input-icon" style="margin-bottom:var(--space-2)">${icon("search")}<input class="input" style="height:var(--control-h-sm)" placeholder="Filtrar arquivos" aria-label="Filtrar arquivos de config"></div>
        <ul class="tree" role="tree" aria-labelledby="tree-t">${treeHtml(D.configTree)}</ul></div></section>`;
      return `${packHead()}${packTabs("configs")}<div class="page wide"><div class="split split-side-main">${tree}<div>${editorPane()}</div></div></div>`;
    },
  };
  function editorPane() {
    const f = S.cfg.file;
    if (S.view === "carregando") return `<section class="card card-pad">${W.loadingSr("Abrindo o arquivo…")}<div class="loading-line"><span class="spinner"></span>Abrindo <code>${esc(f)}</code>…</div><div class="stack-sm" style="margin-top:var(--space-5)" aria-hidden="true">${Array.from({ length: 6 }, (_, i) => `<span class="skel" style="height:14px;width:${90 - i * 9}%"></span>`).join("")}</div></section>`;
    if (S.view === "vazio") return emptyState({ art: "fileText", title: "Escolha um arquivo à esquerda", body: "Arquivos TOML, JSON e options.txt abrem como formulário. Os outros abrem como texto. Os comentários do arquivo são preservados." });
    const isCreate = f === "defaultconfigs/create-server.toml";
    const isOptions = f === "options.txt";
    const fmt = f.endsWith(".toml") ? "TOML" : f.endsWith(".json") ? "JSON" : "Texto";
    const formable = isCreate || isOptions;
    const mode = formable ? S.cfg.mode : "texto";
    const ch = isCreate ? cfgChanges() : [];
    const kindNote = f.startsWith("defaultconfigs/")
      ? `${icon("server", "sm")}Config de servidor padrão: vale para todo mundo novo criado com o pack.`
      : isOptions
        ? `${icon("user", "sm")}Primeira execução: o jogador recebe isso na primeira vez e depois pode mudar à vontade.`
        : `${icon("package", "sm")}Config do pack: sobrescreve a do jogador a cada atualização do pack.`;
    const tabs = formable
      ? `<div class="segmented" role="tablist" aria-label="Modo de edição">
          <button role="tab" aria-selected="${mode === "form"}" tabindex="${mode === "form" ? 0 : -1}" data-action="cfg-mode" data-mode="form" data-fk="cfgm-form">${icon("list", "sm")}Formulário</button>
          <button role="tab" aria-selected="${mode === "texto"}" tabindex="${mode === "texto" ? 0 : -1}" data-action="cfg-mode" data-mode="texto" data-fk="cfgm-texto">${icon("code", "sm")}Texto</button></div>`
      : `<span class="chip">${icon("code", "sm")}Só modo texto</span>`;
    let content;
    if (S.view === "erro" && isCreate) {
      content = `<div class="alert danger">${icon("error")}<div><div class="atitle">Não conseguimos interpretar este arquivo</div><div class="abody">Linha 14: falta o sinal “=” depois de <code>maxChassisRange</code>. Abrimos em modo texto para você corrigir. Nada foi alterado.</div></div></div>
        <div class="code" tabindex="0" role="region" aria-label="Conteúdo do arquivo" style="margin-top:var(--space-4)"><table>${tomlLines().map((l, i) => `<tr class="${i === 13 ? "hl" : ""}"><td class="ln">${i + 1}</td><td>${W.hl(i === 13 ? "\tmaxChassisRange 16" : l)}</td></tr>`).join("")}</table></div>`;
    } else if (mode === "form" && isCreate) {
      const v = cfgValues();
      content = D.createServerToml
        .map(
          (s) => `<div class="cfg-section"><h3 class="mono small" style="color:var(--c-accent-text)">${esc(s.title)}</h3>
        ${s.fields
          .map((fl) => {
            const changed = v[fl.key] !== fl.orig;
            const id = "cf-" + fl.key;
            let ctrl;
            if (fl.type === "bool") ctrl = `<label class="switch"><input id="${id}" type="checkbox" role="switch" ${v[fl.key] ? "checked" : ""} data-change="cfg-val" data-key="${fl.key}" data-type="bool" data-fk="${id}"><span>${v[fl.key] ? "Sim (true)" : "Não (false)"}</span></label>`;
            else ctrl = `<input id="${id}" class="input" type="number" min="${fl.min}" max="${fl.max}" step="${fl.type === "float" ? "0.01" : "1"}" value="${v[fl.key]}" data-change="cfg-val" data-key="${fl.key}" data-type="${fl.type}" data-fk="${id}" aria-describedby="${id}-d ${id}-m">`;
            return `<div class="cfg-field${changed ? " changed" : ""}"><div><label class="fname" for="${id}">${esc(fl.key)}</label>
            <p class="fdesc" id="${id}-d">${esc(fl.desc)}</p><p class="fmeta" id="${id}-m">Padrão do mod: ${fl.def}${fl.min !== undefined ? ` · aceita de ${fl.min} a ${fl.max}` : ""}${changed ? ` · antes: ${fl.orig}` : ""}</p></div>
            <div class="ctrl">${ctrl}${changed ? `<button class="btn btn-ghost btn-icon btn-sm" aria-label="Desfazer mudança em ${fl.key}" title="Desfazer" data-action="cfg-reset" data-key="${fl.key}">${icon("rotateCcw", "sm")}</button>` : ""}</div></div>`;
          })
          .join("")}</div>`,
        )
        .join("");
      content += `<p class="xs muted">As descrições vêm do próprio arquivo (escritas pelo autor do mod, em inglês) e continuam no arquivo ao salvar.</p>`;
    } else if (mode === "form" && isOptions) {
      content = `<div class="cfg-section">
        <div class="cfg-field"><div><label class="fname" for="o-rd">Distância de renderização</label><p class="fdesc">Quantos chunks ao redor do jogador são desenhados. Mais = mais bonito e mais pesado.</p><p class="fmeta">Chave <code>renderDistance</code></p></div><div class="ctrl"><input id="o-rd" type="range" min="2" max="32" value="10" aria-valuetext="10 chunks"><span class="num small" style="min-width:3ch">10</span></div></div>
        <div class="cfg-field"><div><label class="fname" for="o-gui">Escala da interface</label><p class="fdesc">Tamanho de menus e textos.</p><p class="fmeta">Chave <code>guiScale</code></p></div><div class="ctrl"><select id="o-gui" class="select"><option>Automática</option><option>1</option><option>2</option><option selected>3</option><option>4</option></select></div></div>
        <div class="cfg-field"><div><label class="fname" for="o-lang">Idioma</label><p class="fdesc">Idioma do jogo na primeira vez.</p><p class="fmeta">Chave <code>lang</code></p></div><div class="ctrl"><select id="o-lang" class="select"><option selected>Português (Brasil)</option><option>English (US)</option></select></div></div>
        <div class="cfg-field"><div><span class="fname">Resource packs ativos</span><p class="fdesc">Na ordem em que são aplicados (o de cima vence).</p><p class="fmeta">Chave <code>resourcePacks</code></p></div><div class="ctrl" style="flex-direction:column;align-items:stretch"><span class="chip">Texturas do Vale</span><span class="chip">Fresh Animations</span></div></div>
      </div><p class="xs muted">O Warden guarda só as opções que você mudou, não o arquivo inteiro.</p>`;
    } else {
      const lines = isCreate ? tomlLines() : isOptions ? ["version:3955", "renderDistance:10", "guiScale:3", "lang:pt_br", 'resourcePacks:["vanilla","mod_resources","file/Texturas do Vale.zip"]'] : ["# conteúdo de exemplo", `# ${f}`, "", "exemplo = true"];
      content = S.cfg.editing
        ? `<label class="sr-only" for="cfg-text">Conteúdo de ${esc(f)}</label><div class="code"><textarea id="cfg-text" class="code-editable" spellcheck="false" data-fk="cfg-text">${esc(lines.join("\n"))}</textarea></div><p class="xs muted" style="margin-top:var(--space-2)">No app, este campo vira um editor de código com cores, números de linha e busca.</p>`
        : `<div class="code" tabindex="0" role="region" aria-label="Conteúdo do arquivo"><table>${lines.map((l, i) => `<tr><td class="ln">${i + 1}</td><td>${W.hl(l.replace(/^(\w+):/, "$1 = "))}</td></tr>`).join("")}</table></div>
           <div class="row" style="margin-top:var(--space-3)"><button class="btn btn-sm" data-action="cfg-edit">${icon("edit", "sm")}Editar como texto</button></div>`;
    }
    return `<section class="card" aria-labelledby="cfg-t">
      <div class="card-head" style="flex-wrap:wrap">
        <div style="min-width:0"><h2 class="mono" id="cfg-t" tabindex="-1" style="font-size:var(--text-md)">${esc(f)}</h2><p class="small text-2 row" style="margin-top:var(--space-1)">${kindNote}</p></div>
        <div class="row">${chip(fmt)}${tabs}</div>
      </div>
      <div class="card-body">${content}</div>
      <div class="card-foot" style="justify-content:space-between;position:sticky;bottom:0;background:var(--c-surface);border-radius:0 0 var(--radius-lg) var(--radius-lg)">
        <span class="small ${ch.length ? "" : "muted"}" role="status">${ch.length ? `${icon("edit", "sm")} ${ch.length} ${ch.length === 1 ? "mudança não salva" : "mudanças não salvas"}` : "Sem mudanças"}</span>
        <div class="row"><button class="btn" data-action="cfg-discard" ${ch.length ? "" : "disabled"}>Descartar</button><button class="btn btn-primary" data-action="cfg-review" ${ch.length ? "" : "disabled"}>${icon("compare")}Revisar e salvar</button></div>
      </div></section>`;
  }
  actions["cfg-toggle"] = (el) => {
    const n = el.dataset.name;
    const cur = S.cfg.open[n] !== undefined ? S.cfg.open[n] : true;
    S.cfg.open[n] = !cur;
    render();
  };
  actions["cfg-file"] = (el) => {
    S.cfg.file = el.dataset.path;
    S.cfg.editing = false;
    render();
  };
  actions["cfg-mode"] = (el) => {
    S.cfg.mode = el.dataset.mode;
    render();
  };
  actions["cfg-edit"] = () => {
    S.cfg.editing = true;
    render();
    document.getElementById("cfg-text")?.focus();
  };
  actions["cfg-val"] = (el) => {
    const v = cfgValues();
    const t = el.dataset.type;
    v[el.dataset.key] = t === "bool" ? el.checked : t === "float" ? parseFloat(el.value) : parseInt(el.value, 10);
    render();
  };
  actions["cfg-reset"] = (el) => {
    const f = D.createServerToml.flatMap((s) => s.fields).find((x) => x.key === el.dataset.key);
    cfgValues()[f.key] = f.orig;
    render();
    document.getElementById("cf-" + f.key)?.focus();
  };
  actions["cfg-discard"] = () => {
    S.cfg.values = null;
    D.createServerToml.forEach((s) => s.fields.forEach((f) => (f.value = f.orig)));
    render();
    toast("Mudanças descartadas.", "info");
  };
  actions["cfg-review"] = () => {
    const ch = cfgChanges();
    const rows = [];
    ch.forEach(({ f, s }) => {
      rows.push([" ", s.title]);
      rows.push([" ", `\t#${f.desc}`]);
      rows.push(["-", `\t${f.key} = ${f.orig}`]);
      rows.push(["+", `\t${f.key} = ${cfgValues()[f.key]}`]);
    });
    dialog({
      wide: true,
      title: "Revisar mudanças antes de salvar",
      sub: `<code>${esc(S.cfg.file)}</code> · comentários e a ordem do arquivo continuam iguais`,
      body: `<div class="code" tabindex="0" role="region" aria-label="Diferenças"><table>${rows.map((r) => `<tr class="${r[0] === "+" ? "add" : r[0] === "-" ? "del" : ""}"><td class="sign">${r[0] === " " ? "" : r[0]}<span class="sr-only">${r[0] === "+" ? "adicionado" : r[0] === "-" ? "removido" : ""}</span></td><td>${esc(r[1])}</td></tr>`).join("")}</table></div>
        <p class="small text-2" style="margin-top:var(--space-3)">Linhas em vermelho saem; em verde entram. Depois de salvar, a mudança aparece em “alterações não salvas” até você salvar uma versão do pack.</p>`,
      foot: `<button class="btn" data-action="dlg-close">Continuar editando</button><button class="btn btn-primary" data-action="cfg-save">${icon("save")}Salvar no pack</button>`,
    });
  };
  actions["cfg-save"] = () => {
    const v = cfgValues();
    D.createServerToml.forEach((s) => s.fields.forEach((f) => (f.orig = v[f.key])));
    S.unsaved++;
    closeDialog(true);
    render();
    toast("Config salva no pack.");
    document.getElementById("cfg-t")?.focus();
  };
})();
