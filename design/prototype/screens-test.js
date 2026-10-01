// Telas 6–7: Testar (início, checagem, preparação, jogo aberto, o que mudou) e Diagnóstico.
// O Warden abre o Minecraft sozinho (motor de launcher embutido, invisível): nenhum
// outro launcher aparece em nenhum momento. Perfil offline, sem login.
(function () {
  const { D, S, esc, ticon, pill, status, chip, toast, dialog, closeDialog, routes, actions, go, render, every, after } = W;

  W.screenList.push(
    ["testar", "6. Testar — início"],
    ["testar/checagem", "6a. Testar — checagem antes de abrir"],
    ["testar/preparando", "6b. Testar — preparando o jogo"],
    ["testar/jogo", "6c. Testar — jogo aberto (console)"],
    ["testar/mudancas", "6d. Testar — o que mudou durante o teste"],
    ["diagnostico", "7. Diagnóstico de crash"],
  );

  const STEPS = [
    { k: "checagem", t: "Checar" },
    { k: "preparando", t: "Preparar" },
    { k: "jogo", t: "Jogar" },
    { k: "mudancas", t: "Revisar mudanças" },
  ];
  function stepper(cur) {
    const i = STEPS.findIndex((s) => s.k === cur);
    return `<ol class="stepper" aria-label="Etapas do teste">${STEPS.map((s, j) => `<li class="${j < i ? "done" : ""}" ${j === i ? 'aria-current="step"' : ""}><span class="n" aria-hidden="true">${j < i ? icon("check", "sm") : j + 1}</span>${s.t}${j < i ? '<span class="sr-only"> (concluída)</span>' : ""}</li>`).join("")}</ol>`;
  }
  function testHead(title, sub, cur, right = "") {
    return `<div class="page-head" style="margin-bottom:var(--space-5)"><div class="titles">
      <nav class="crumbs" aria-label="Você está em"><a href="#/pack/mods">${esc(S.pack.name)}</a>${icon("chevronRight", "sm")}<a href="#/testar">Testar</a></nav>
      <h1 id="page-title" class="page-title" tabindex="-1">${title}</h1>${sub ? `<p class="page-sub">${sub}</p>` : ""}</div>
      <div class="page-actions" style="flex-direction:column;align-items:flex-end;gap:var(--space-3)">${cur ? stepper(cur) : ""}${right}</div></div>`;
  }
  const fmtMB = (n) => (n >= 1000 ? (n / 1000).toFixed(2).replace(".", ",") + " GB" : Math.round(n) + " MB");
  const fmtInt = (n) => Math.round(n).toLocaleString("pt-BR");
  const clock = (s) => `${String(Math.floor(s / 60)).padStart(2, "0")}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

  // ---------------------------------------------------------------------------
  // 6. Testar — início
  // ---------------------------------------------------------------------------
  routes["testar"] = {
    title: "Testar",
    nav: "testar",
    states: ["normal"],
    render() {
      const t = S.test;
      const journey = [
        ["shield", "Checar", "Procura problemas conhecidos antes de abrir: mods faltando, repetidos ou incompatíveis."],
        ["download", "Preparar", "Baixa o que faltar (Java, Minecraft, loader, mods) e monta a instância de teste."],
        ["play", "Jogar", "Abre o Minecraft com o seu pack. Você joga e mexe em configs à vontade."],
        ["compare", "Revisar", "Quando você fecha o jogo, mostramos o que mudou para você trazer ao pack."],
      ];
      return `<div class="page">
        ${testHead('Testar o <span class="hl">pack</span>', "", "")}
        <section class="card test-hero" aria-labelledby="hero-t">
          <div class="hero-main">
            <div class="row" style="gap:var(--space-4);align-items:flex-start">${ticon(S.pack.name, "lg")}
              <div><h2 id="hero-t" class="hero-title">${esc(S.pack.name)}</h2>
              <p class="text-2" style="margin-top:var(--space-1)">Minecraft ${esc(S.pack.mc)} · ${esc(S.pack.loader)} ${esc(S.pack.loaderVersion)} · ${S.mods.length} mods</p></div></div>
            <p class="text-2" style="margin-top:var(--space-5);max-width:60ch">O Warden abre o Minecraft por conta própria, numa pasta separada do pack. Nada no seu pack muda sem você revisar.</p>
            <dl class="hero-facts">
              <div><dt>Java</dt><dd>${icon("coffee", "sm")}Java 21 <span class="muted">· instalado</span></dd></div>
              <div><dt>Memória</dt><dd>${icon("memory", "sm")}${S.set.mem} GB <a href="#/ajustes" class="xs">mudar</a></dd></div>
              <div><dt>Jogador</dt><dd>${icon("user", "sm")}${esc(S.set.player)} <span class="muted">· offline</span></dd></div>
            </dl>
          </div>
          <div class="hero-side">
            <button class="btn btn-primary btn-lg btn-launch" data-action="test-start" data-fk="test-start"><span class="tendrils" aria-hidden="true"><i></i><i></i><i></i><i></i></span>${icon("play", "lg")}Testar</button>
            <span class="xs muted">Atalho: <kbd>F5</kbd></span>
          </div>
        </section>
        <div class="split split-2" style="margin-top:var(--space-5)">
          <section class="card card-pad" aria-labelledby="how-t">
            <h2 class="section-title" id="how-t">Como testar</h2>
            <div class="choice-grid" style="grid-template-columns:1fr 1fr;margin-top:var(--space-3)">
              <label class="choice"><input type="radio" name="tmode" value="trabalho" data-change="test-mode" ${t.mode === "trabalho" ? "checked" : ""}><span class="ctitle">Instância de trabalho<span class="radio-dot" aria-hidden="true"></span></span><span class="cdesc">Rápida: reaproveita downloads e o mundo de teste. Para o dia a dia.</span></label>
              <label class="choice"><input type="radio" name="tmode" value="limpa" data-change="test-mode" ${t.mode === "limpa" ? "checked" : ""}><span class="ctitle">Como o jogador vai receber<span class="radio-dot" aria-hidden="true"></span></span><span class="cdesc">Do zero, igual a quem baixar o pack. Use antes de lançar uma versão.</span></label>
            </div>
            <label class="switch" style="margin-top:var(--space-4)"><input type="checkbox" role="switch" ${t.quick ? "checked" : ""} data-change="test-quick"><span><span class="label">Entrar direto no mundo de teste</span><span class="hint" style="display:block">Pula o menu principal e abre o mundo “Teste”.</span></span></label>
          </section>
          <section class="card card-pad" aria-labelledby="last-t">
            <h2 class="section-title" id="last-t">Último teste</h2>
            <div class="row wrap" style="margin-top:var(--space-3)">${pill("warn", "Passou com avisos")}<span class="small text-2">ontem, 21:14 · jogou por 6 min</span></div>
            <ul class="small text-2 stack-sm" style="margin-top:var(--space-4)">
              <li class="row">${icon("ok", "sm")}3 arquivos trazidos de volta para o pack</li>
              <li class="row">${icon("warn", "sm")}2 avisos na checagem (resolvidos depois)</li>
            </ul>
          </section>
        </div>
        <section aria-labelledby="journey-t" style="margin-top:var(--space-8)">
          <h2 class="section-title" id="journey-t">O que acontece quando você clica em Testar</h2>
          <ol class="journey">${journey.map(([ic, t1, d], i) => `<li><span class="jn">${icon(ic, "lg")}</span><div><div class="jt"><span class="muted num">${i + 1}.</span> ${t1}</div><p class="small text-2">${d}</p></div></li>`).join("")}</ol>
        </section>
      </div>`;
    },
  };
  actions["test-mode"] = (el) => {
    S.test.mode = el.value;
    render();
    document.querySelector(`input[name="tmode"][value="${el.value}"]`)?.focus();
  };
  actions["test-quick"] = (el) => (S.test.quick = el.checked);
  actions["test-start"] = () => {
    S.test.checked = false;
    go("testar/checagem");
  };
  document.addEventListener("keydown", (e) => {
    if (e.key === "F5" && S.route === "testar") {
      e.preventDefault();
      actions["test-start"]();
    }
  });

  // ---------------------------------------------------------------------------
  // 6a. Checagem
  // ---------------------------------------------------------------------------
  function isResolved(c) {
    if (c.id === "c2" && S.balmAdded) return true;
    if (c.id === "c3" && S.mods.find((m) => m.id === "packutils")?.side !== "?") return true;
    if (c.id === "c1" && !S.mods.find((m) => m.id === "embeddium")) return true;
    return S.test.resolved.has(c.id);
  }
  routes["testar/checagem"] = {
    title: "Testar — checagem",
    nav: "testar",
    states: ["normal"],
    enter() {
      if (!S.test.checked) {
        S.test.checking = 0;
        every(60, () => {
          S.test.checking += W.reducedMotion() ? 25 : 4;
          const bar = document.getElementById("chk-bar");
          if (bar) {
            bar.style.width = Math.min(100, S.test.checking) + "%";
            bar.parentElement.setAttribute("aria-valuenow", Math.min(100, S.test.checking));
            document.getElementById("chk-what").textContent = checkingLabel(S.test.checking);
          }
          if (S.test.checking >= 100) {
            S.test.checked = true;
            W.clearTimers();
            render();
            document.getElementById("chk-summary")?.focus();
          }
        });
      }
    },
    render() {
      if (!S.test.checked) {
        return `<div class="page">${testHead('Checando o <span class="hl">pack</span>', "Isso leva só alguns segundos e evita abrir um jogo que não vai funcionar.", "checagem")}
          <section class="card card-pad" aria-busy="true"><div class="row" style="gap:var(--space-4)"><span class="spinner lg" aria-hidden="true"></span><div class="grow"><p><strong id="chk-what">Lendo os mods do pack…</strong></p>
          <div class="progress" role="progressbar" aria-label="Checagem" aria-valuemin="0" aria-valuemax="100" aria-valuenow="0" style="margin-top:var(--space-3)"><span id="chk-bar" style="width:0%"></span></div></div></div></section></div>`;
      }
      const list = D.checks.map((c) => ({ ...c, done: isResolved(c) }));
      const errors = list.filter((c) => c.sev === "danger" && !c.done).length;
      const warns = list.filter((c) => c.sev === "warn" && !c.done).length;
      const sevName = { danger: "Erro", warn: "Aviso", info: "Informação" };
      return `<div class="page">
        ${testHead('<span class="hl">Checagem</span> antes de abrir o jogo', "", "checagem")}
        <section class="card" aria-labelledby="chk-summary">
          <div class="card-head" style="flex-wrap:wrap">
            <h2 class="section-title" id="chk-summary" tabindex="-1">${errors ? `Encontramos ${errors} ${errors === 1 ? "problema que impede" : "problemas que impedem"} o jogo de abrir` : warns ? "Pronto para testar, com avisos" : "Tudo certo para testar"}</h2>
            <div class="row wrap">${errors ? pill("danger", `${errors} ${errors === 1 ? "erro" : "erros"}`) : ""}${warns ? pill("warn", `${warns} ${warns === 1 ? "aviso" : "avisos"}`) : ""}${pill("ok", `${D.passedChecks.length} checagens ok`)}</div>
          </div>
          <ul>${list
            .map(
              (c) => `<li class="finding${c.done ? " resolved" : ""}">
            <span class="sev ${c.done ? "ok" : c.sev}">${icon(c.done ? "ok" : c.sev === "danger" ? "error" : c.sev, "lg")}</span>
            <div><div class="row wrap"><span class="sr-only">${c.done ? "Resolvido" : sevName[c.sev]}: </span><span class="ftitle">${c.title}</span>${c.done ? pill("ok", "Resolvido") : ""}</div>
              ${c.done ? "" : `<p class="fbody">${c.body}</p><p class="fev">${icon("search", "sm")} Como sabemos: ${c.ev}</p>
              ${c.fixes.length ? `<div class="fix">${c.fixes.map((f, i) => `<button class="btn btn-sm${f.primary ? " btn-primary" : ""}" data-action="chk-fix" data-id="${c.id}" data-i="${i}" data-fk="fix-${c.id}-${i}">${f.primary ? icon("wrench", "sm") : ""}${esc(f.label)}</button>`).join("")}</div>` : ""}`}
            </div></li>`,
            )
            .join("")}</ul>
          <details class="passed"><summary>${icon("chevronRight", "sm")}Ver as ${D.passedChecks.length} checagens que passaram</summary>
            <ul class="passed-list">${D.passedChecks.map((p) => `<li>${icon("check", "sm")}${esc(p)}</li>`).join("")}</ul></details>
        </section>
        <div class="action-bar">
          <p class="small ${errors ? "" : "muted"}">${errors ? `${icon("info", "sm")} Corrija os erros para testar com segurança, ou teste mesmo assim para ver o que acontece.` : "As correções entram no pack como alterações não salvas."}</p>
          <div class="row"><a class="btn" href="#/testar">Voltar</a>
            ${errors ? `<button class="btn" data-action="test-anyway">${icon("warn")}Testar mesmo assim</button>` : ""}
            <button class="btn btn-primary" data-action="test-go" ${errors ? "disabled" : ""} data-fk="test-go">${icon("play")}Testar</button></div>
        </div>
      </div>`;
    },
  };
  function checkingLabel(p) {
    if (p < 25) return "Lendo os mods do pack…";
    if (p < 50) return "Conferindo versões e loader de 24 mods…";
    if (p < 75) return "Procurando dependências faltando e mods repetidos…";
    return "Conferindo Java, memória e conflitos conhecidos…";
  }
  actions["chk-fix"] = (el) => {
    const c = D.checks.find((x) => x.id === el.dataset.id);
    const f = c.fixes[+el.dataset.i];
    let msg = "";
    if (c.id === "c1") {
      const rm = f.label.includes("Embeddium") ? "embeddium" : "sodium";
      S.mods = S.mods.filter((m) => m.id !== rm);
      msg = `${rm === "embeddium" ? "Embeddium" : "Sodium"} removido do pack.`;
    } else if (c.id === "c2") {
      S.balmAdded = true;
      S.mods.push({ id: "balm", name: "Balm", author: "BlayTheNinth", ver: "21.0.31", src: "modrinth", side: "ambos", lib: true, changed: "Adicionado (não salvo)" });
      msg = "Balm 21.0.31 adicionado ao pack.";
    } else if (c.id === "c3") {
      if (f.label.startsWith("Escolher")) {
        toast("Abriria a gaveta do mod para escolher o lado.", "info");
        return;
      }
      S.mods.find((m) => m.id === "packutils").side = "ambos";
      msg = "“Utilitários do Vale” marcado como Ambos.";
    } else {
      S.test.resolved.add(c.id);
      msg = f.label === "Ignorar" ? "Aviso ignorado neste teste." : "Entity Model Features e Entity Texture Features adicionados.";
    }
    S.unsaved++;
    render();
    toast(msg);
    const next = document.querySelector(".finding:not(.resolved) .fix .btn") || document.querySelector('[data-fk="test-go"]');
    next && next.focus();
  };
  actions["test-anyway"] = () => {
    dialog({
      title: "Testar mesmo com erros?",
      body: `<p class="text-2">O jogo provavelmente vai parar na tela de carregamento. Se isso acontecer, o Warden mostra o diagnóstico com o motivo.</p>`,
      foot: `<button class="btn" data-action="dlg-close">Voltar e corrigir</button><button class="btn btn-primary" data-action="test-go">${icon("play")}Testar mesmo assim</button>`,
    });
  };
  actions["test-go"] = () => {
    closeDialog(true);
    S.test.prep = null;
    go("testar/preparando");
  };

  // ---------------------------------------------------------------------------
  // 6b. Preparação — progresso detalhado
  // ---------------------------------------------------------------------------
  const PREP = [
    { id: "java", title: "Java 21", sub: "Eclipse Temurin 21.0.8 · 64 bits", w: 0, mb: 0, unit: "", total: 1, already: "Já instalado" },
    { id: "mc", title: "Minecraft 1.21.1", sub: "Jogo, bibliotecas, sons e texturas", w: 0.58, mb: 520, unit: "arquivos", total: 3412 },
    { id: "loader", title: "NeoForge 21.1.209", sub: "Instalando o loader para esta versão", w: 0.2, mb: 118, unit: "etapas", total: 10 },
    { id: "mods", title: "Mods e arquivos do pack", sub: "24 mods · 3 resource packs · 2 shaders · 34 configs", w: 0.17, mb: 52, unit: "arquivos", total: 63 },
    { id: "sync", title: "Montar a instância de teste", sub: "Copiar configs e anotar o “antes” para comparar depois", w: 0.05, mb: 0, unit: "", total: 1 },
  ];
  const TOTAL_MB = PREP.reduce((a, s) => a + s.mb, 0);
  const FILES = {
    mc: ["libraries/org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar", "assets/objects/3f/3f5a9c…  (som: ambient/cave)", "libraries/com/mojang/datafixerupper/8.0.16/datafixerupper-8.0.16.jar", "assets/objects/b1/b14e02…  (textura)", "versions/1.21.1/1.21.1.jar"],
    loader: ["Baixando o instalador do NeoForge", "Processando: aplicando mapeamentos do jogo", "Processando: gerando o cliente modificado", "Conferindo os arquivos gerados (SHA-1)"],
    mods: ["create-1.21.1-6.0.8.jar", "sodium-neoforge-0.6.13+mc1.21.1.jar", "jei-1.21.1-neoforge-19.21.0.247.jar", "Complementary Reimagined r5.5.1.zip", "config/create-client.toml"],
    sync: ["Guardando o estado inicial de config/, options.txt e defaultconfigs/"],
  };
  function prepState() {
    if (!S.test.prep) S.test.prep = { i: 1, frac: 0, done: false, failed: false, file: FILES.mc[0] };
    return S.test.prep;
  }
  function prepOverall(p) {
    let w = 0;
    PREP.forEach((s, j) => {
      if (j < p.i) w += s.w;
      else if (j === p.i) w += s.w * p.frac;
    });
    return p.done ? 1 : w;
  }
  function prepMB(p) {
    let mb = 0;
    PREP.forEach((s, j) => {
      if (j < p.i) mb += s.mb;
      else if (j === p.i) mb += s.mb * p.frac;
    });
    return p.done ? TOTAL_MB : mb;
  }
  function stepHtml(s, j, p) {
    const state = p.failed && j === p.i ? "fail" : j < p.i || p.done || s.already ? "ok" : j === p.i ? "run" : "wait";
    const cur = j === p.i && !p.done ? p.frac : state === "ok" ? 1 : 0;
    const right =
      state === "ok"
        ? `<span class="small" style="color:var(--c-ok-text)">${s.already || "Pronto"}</span>`
        : state === "fail"
          ? `<span class="small" style="color:var(--c-danger-text)">Falhou</span>`
          : state === "run" && s.unit
            ? `<span class="small num text-2">${fmtInt(cur * s.total)} de ${fmtInt(s.total)} ${s.unit}</span>`
            : state === "run"
              ? `<span class="small text-2">Em andamento</span>`
              : `<span class="small muted">Na fila</span>`;
    const st = { ok: icon("check"), run: `<span class="spinner" aria-hidden="true"></span>`, wait: icon("clock", "sm"), fail: icon("x") }[state];
    return `<li class="prep-step ${state}" id="ps-${s.id}">
      <span class="st ${state === "ok" ? "ok" : state === "run" ? "run" : state === "fail" ? "fail" : ""}" aria-hidden="true">${st}</span>
      <div style="min-width:0"><div class="row between"><strong>${esc(s.title)}</strong>${right}</div>
        <div class="small text-2">${esc(s.sub)}${s.mb ? ` · ${fmtMB(s.mb)}` : ""}</div>
        ${state === "run" ? `<div class="progress" style="margin-top:var(--space-2)" aria-hidden="true"><span style="width:${(cur * 100).toFixed(1)}%"></span></div><div class="xs muted mono now-file" style="margin-top:var(--space-1);white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${esc(p.file)}</div>` : ""}
        ${state === "fail" ? `<div class="small" style="color:var(--c-danger-text);margin-top:var(--space-1)">3 arquivos não baixaram depois de 3 tentativas.</div>` : ""}
      </div><span class="sr-only">${state === "ok" ? "concluído" : state === "run" ? "em andamento" : state === "fail" ? "falhou" : "aguardando"}</span></li>`;
  }
  function prepLive() {
    const p = prepState();
    const pct = Math.round(prepOverall(p) * 100);
    const mb = prepMB(p);
    const remaining = p.done ? 0 : Math.max(1, Math.round(((1 - prepOverall(p)) * 11)));
    return `<div class="prep-hero">
        <div class="prep-ring" aria-hidden="true" style="--p:${pct}"><span class="display">${pct}<small>%</small></span></div>
        <div class="grow" style="min-width:0">
          <h2 class="hero-title">${p.failed ? "A preparação parou" : p.done ? "Tudo pronto. Abrindo o Minecraft…" : PREP[p.i].id === "mc" ? "Baixando o Minecraft 1.21.1" : PREP[p.i].id === "loader" ? "Instalando o NeoForge" : PREP[p.i].id === "mods" ? "Baixando os mods do pack" : "Montando a instância de teste"}</h2>
          <p class="text-2" style="margin-top:var(--space-1)">${p.failed ? "Sem internet ou servidor fora do ar. O que já foi baixado fica guardado." : "Primeira vez deste pack: baixando o que falta. Nas próximas vezes abre em segundos."}</p>
          <div class="progress lg" role="progressbar" aria-label="Preparação do jogo" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${pct}" aria-valuetext="${pct}%" style="margin-top:var(--space-4)"><span style="width:${pct}%"></span></div>
          <dl class="prep-stats">
            <div><dt>Baixado</dt><dd class="num">${fmtMB(mb)} <span class="muted">de ${fmtMB(TOTAL_MB)}</span></dd></div>
            <div><dt>Velocidade</dt><dd class="num">${p.done || p.failed ? "—" : (16 + (Math.round(mb) % 7) * 0.6).toFixed(1).replace(".", ",") + " MB/s"}</dd></div>
            <div><dt>Falta</dt><dd class="num">${p.failed ? "—" : p.done ? "nada" : `cerca de ${remaining} s`}</dd></div>
          </dl>
        </div>
      </div>
      <ol class="prep-steps">${PREP.map((s, j) => stepHtml(s, j, p)).join("")}</ol>`;
  }
  routes["testar/preparando"] = {
    title: "Testar — preparando",
    nav: "testar",
    states: ["normal", "erro"],
    enter() {
      S.test.prep = null;
      const p = prepState();
      if (S.view === "erro") {
        p.i = 1;
        p.frac = 0.38;
        p.failed = true;
        return;
      }
      const speed = W.reducedMotion() ? 3 : 1;
      let lastStep = p.i;
      every(90, () => {
        const s = PREP[p.i];
        const dur = { mc: 5.2, loader: 2.2, mods: 1.6, sync: 0.6 }[s.id] || 1;
        p.frac += (0.09 / dur) * speed;
        const pool = FILES[s.id] || [""];
        p.file = pool[Math.floor(p.frac * pool.length * 3) % pool.length];
        if (p.frac >= 1) {
          p.frac = 0;
          p.i++;
          if (p.i >= PREP.length) {
            p.done = true;
            p.i = PREP.length - 1;
          }
        }
        const live = document.getElementById("prep-live");
        if (live) live.innerHTML = prepLive();
        if (p.i !== lastStep || p.done) {
          const ann = document.getElementById("prep-announce");
          if (ann) ann.textContent = p.done ? "Preparação concluída. Abrindo o Minecraft." : `${PREP[lastStep].title}: concluído. Agora: ${PREP[p.i].title}.`;
          lastStep = p.i;
        }
        if (p.done) {
          W.clearTimers();
          after(900, () => go("testar/jogo"));
        }
      });
    },
    render() {
      const p = prepState();
      return `<div class="page">
        ${testHead('Preparando o <span class="hl">jogo</span>', "", "preparando")}
        ${p.failed ? `<div class="alert danger" style="margin-bottom:var(--space-4)">${icon("wifiOff")}<div><div class="atitle">Não conseguimos baixar 3 arquivos do Minecraft</div><div class="abody">Confira sua internet e tente de novo. O Warden continua de onde parou, sem baixar tudo outra vez.</div></div><div class="aactions"><button class="btn btn-sm btn-primary" data-action="prep-retry">${icon("refresh", "sm")}Tentar de novo</button></div></div>` : ""}
        <div class="split" style="grid-template-columns:minmax(0,1fr) 300px;align-items:start">
          <section class="card prep-card" aria-labelledby="prep-h">
            <h2 class="sr-only" id="prep-h">Progresso</h2>
            <div id="prep-live">${prepLive()}</div>
            <p class="sr-only" role="status" aria-live="polite" id="prep-announce"></p>
            <div class="card-foot" style="justify-content:space-between"><details class="tech"><summary class="small">${icon("terminal", "sm")}Detalhes técnicos</summary><p class="xs muted mono" style="margin-top:var(--space-2)">Pasta compartilhada: %APPDATA%\\Warden\\shared · instância: instances\\vale-das-engrenagens\\minecraft · downloads conferidos por SHA-1/SHA-512</p></details>
              <button class="btn" data-action="prep-cancel">${icon("x")}Cancelar</button></div>
          </section>
          <aside class="stack">
            <section class="card card-pad stack-sm"><h2 class="label">${icon("info", "sm")} Enquanto isso</h2>
              <p class="small text-2">O Java e o Minecraft ficam guardados numa pasta do Warden e são reaproveitados por todos os seus packs.</p>
              <p class="small text-2">Nada disso entra no pack: ele continua só com mods, configs e o que você adicionou.</p></section>
            <section class="card card-pad stack-sm"><h2 class="label">${icon("hardDrive", "sm")} Espaço usado</h2>
              <p class="small text-2">Este teste usa cerca de <strong>${fmtMB(TOTAL_MB)}</strong>. Dá para liberar espaço em <a href="#/ajustes">Configurações</a>.</p></section>
          </aside>
        </div></div>`;
    },
  };
  actions["prep-cancel"] = () => {
    W.clearTimers();
    go("testar");
    toast("Preparação cancelada. O que já foi baixado fica guardado.", "info");
  };
  actions["prep-retry"] = () => W.setView("normal");

  // ---------------------------------------------------------------------------
  // 6c. Jogo aberto — console ao vivo
  // ---------------------------------------------------------------------------
  const LIVE_POOL = [
    ["INFO", "[Server thread/INFO] [net.minecraft.server.MinecraftServer/]: Saving chunks for level 'ServerLevel[Teste]'/minecraft:overworld"],
    ["INFO", "[Render thread/INFO] [snownee.jade.Jade/]: Saving config: config/jade/plugins.json", "change"],
    ["INFO", "[Render thread/INFO] [net.minecraft.client.gui.components.ChatComponent/]: [CHAT] Você abriu o livro de receitas"],
    ["INFO", "[Render thread/INFO] [net.irisshaders.iris.Iris/]: Shaderpack changed to Complementary Reimagined r5.5.1"],
    ["WARN", "[Server thread/WARN] [net.minecraft.server.MinecraftServer/]: Can't keep up! Is the server overloaded? Running 2140ms or 42 ticks behind"],
    ["INFO", "[Render thread/INFO] [net.minecraft.client.Options/]: Saving options to options.txt", "change"],
    ["INFO", "[Server thread/INFO] [net.neoforged.neoforge.common.ModConfigSpec/]: Saved serverconfig create-server.toml", "change"],
    ["INFO", "[Render thread/INFO] [net.minecraft.client.renderer.LevelRenderer/]: Reloading renderer: render distance 10"],
    ["INFO", "[Server thread/INFO] [com.simibubi.create.Create/]: Train schedule updated: Estação Norte → Vila"],
  ];
  function lineHtml([t, lv, msg]) {
    const m = msg.match(/^(\[[^\]]+\]\s*\[[^\]]+\]:\s*)(.*)$/);
    let srcPart = m ? m[1] : "";
    if (m && !S.test.fullLines) {
      // forma curta: "[Render thread] SoundEngine: " (a linha original fica no log e com "Linhas completas")
      const thread = (srcPart.match(/^\[([^/\]]+)/) || [])[1] || "";
      const logger = ((srcPart.match(/\]\s*\[([^\]]+)\]/) || [])[1] || "").replace(/\/.*$/, "").split(".").pop();
      srcPart = `[${thread}]${logger ? " " + logger : ""}: `;
    }
    const body = m ? m[2] : msg;
    return `<span class="ln ${lv}"><span class="t">${t}</span><span class="lv ${lv}">${lv === "INFO" ? "info" : lv === "WARN" ? "aviso" : "erro"}</span><span class="msg"><span class="srcp">${esc(srcPart)}</span>${esc(body)}</span></span>`;
  }
  function consoleFilterOk(l) {
    const f = S.test.consoleFilter;
    if (f === "avisos" && l[1] !== "WARN") return false;
    if (f === "erros" && l[1] !== "ERROR") return false;
    const q = S.test.consoleQuery.trim().toLowerCase();
    return !q || l[2].toLowerCase().includes(q);
  }
  routes["testar/jogo"] = {
    title: "Testar — jogo aberto",
    nav: "testar",
    states: ["normal"],
    enter() {
      const t = S.test;
      if (!t.lines.length) t.lines = D.consoleLines.slice();
      if (!t.startedAt) t.startedAt = Date.now() - 41000;
      t.detected = t.detected || 0;
      let k = 0;
      every(1000, () => {
        const el = document.getElementById("game-time");
        if (el) el.textContent = clock((Date.now() - t.startedAt) / 1000);
        const mem = document.getElementById("game-mem");
        if (mem) {
          const gb = 2.6 + ((Date.now() / 1000) % 9) * 0.07;
          mem.querySelector(".fill").style.width = (gb / S.set.mem) * 100 + "%";
          mem.querySelector(".v").textContent = gb.toFixed(1).replace(".", ",") + " GB";
        }
      });
      every(W.reducedMotion() ? 2400 : 1300, () => {
        const [lv, msg, change] = LIVE_POOL[k++ % LIVE_POOL.length];
        // relógio fictício contínuo a partir da última linha do log de exemplo (21:12:42)
        const secs = 21 * 3600 + 12 * 60 + 42 + Math.round((Date.now() - t.startedAt) / 1000) - 41;
        const hh = (n) => String(n).padStart(2, "0");
        const l = [`${hh(Math.floor(secs / 3600) % 24)}:${hh(Math.floor(secs / 60) % 60)}:${hh(secs % 60)}`, lv, msg];
        t.lines.push(l);
        if (change && t.detected < 5) {
          t.detected++;
          const c = document.getElementById("det-count");
          if (c) c.textContent = t.detected;
          const cl = document.getElementById("det-list");
          if (cl) cl.innerHTML = detectedList();
        }
        const box = document.getElementById("console");
        if (box && consoleFilterOk(l)) {
          box.insertAdjacentHTML("beforeend", lineHtml(l));
          if (t.autoscroll) box.scrollTop = box.scrollHeight;
        }
        const cnt = document.getElementById("console-count");
        if (cnt) cnt.textContent = `${fmtInt(1180 + t.lines.length)} linhas`;
        const w = document.getElementById("warn-n");
        if (w) w.textContent = t.lines.filter((x) => x[1] === "WARN").length;
      });
    },
    after() {
      const box = document.getElementById("console");
      if (box) box.scrollTop = box.scrollHeight;
    },
    render() {
      const t = S.test;
      const warns = t.lines.filter((x) => x[1] === "WARN").length;
      const errs = t.lines.filter((x) => x[1] === "ERROR").length;
      const f = (k, label, n, id) => `<button aria-pressed="${t.consoleFilter === k}" data-action="con-filter" data-k="${k}" data-fk="cf-${k}">${label}${n !== undefined ? ` <span class="num muted"${id ? ` id="${id}"` : ""}>${n}</span>` : ""}</button>`;
      const shown = t.lines.filter(consoleFilterOk);
      return `<div class="page wide">
        ${testHead('Jogo <span class="hl">aberto</span>', "", "jogo")}
        <section class="session" aria-label="Sessão de teste">
          <div class="session-main">
            <span class="live-dot" aria-hidden="true"></span>
            <div><div class="session-title">Minecraft está aberto</div><div class="small text-2">${esc(S.pack.name)} · ${esc(S.pack.mc)} · ${esc(S.pack.loader)} · instância de ${S.test.mode === "limpa" ? "teste limpa" : "trabalho"}</div>
              <div class="session-actions"><button class="btn btn-sm" data-action="soon" data-msg="Abriria a pasta da instância de teste no Explorador de Arquivos.">${icon("folderOpen", "sm")}Abrir pasta do jogo</button>
              <button class="btn btn-sm btn-danger-outline" data-action="game-stop">${icon("stop", "sm")}Parar o jogo</button></div></div>
          </div>
          <dl class="session-stats">
            <div><dt>Tempo de jogo</dt><dd class="num display" id="game-time">${clock((Date.now() - (t.startedAt || Date.now())) / 1000)}</dd></div>
            <div class="mem"><dt>Memória em uso</dt><dd><div class="meter" id="game-mem"><div class="row between"><span class="v num">2,8 GB</span><span class="xs muted">de ${S.set.mem} GB</span></div><div class="bar" aria-hidden="true"><span class="fill" style="width:46%"></span></div></div></dd></div>
            <div><dt>Jogador</dt><dd>${esc(S.set.player)} <span class="muted xs">offline</span></dd></div>
          </dl>
        </section>
        <div class="split" style="grid-template-columns:minmax(0,1fr) 320px;align-items:start;margin-top:var(--space-5)">
          <section class="card console-card" aria-labelledby="con-t">
            <div class="card-head console-head">
              <div class="row" style="gap:var(--space-3)"><h2 class="label" id="con-t">${icon("terminal", "sm")} Console</h2><span class="xs muted num" id="console-count">${fmtInt(1180 + t.lines.length)} linhas</span></div>
              <div class="row"><button class="btn btn-sm btn-ghost" data-action="con-copy">${icon("copy", "sm")}Copiar</button><button class="btn btn-sm btn-ghost" data-action="soon" data-msg="Abriria o logs/latest.log no editor de texto.">${icon("fileText", "sm")}Abrir latest.log</button></div>
            </div>
            <div class="console-tools">
              <div class="segmented" role="group" aria-label="Filtrar linhas">${f("tudo", "Tudo")}${f("avisos", "Avisos", warns, "warn-n")}${f("erros", "Erros", errs)}</div>
              <div class="input-icon">${icon("search")}<input class="input" style="height:var(--control-h-sm);width:200px" type="search" placeholder="Buscar no console" aria-label="Buscar no console" value="${esc(t.consoleQuery)}" data-input="con-q" data-fk="con-q"></div>
            </div>
            <div class="console" id="console" role="log" aria-label="Saída do jogo" tabindex="0">${shown.map(lineHtml).join("") || `<span class="ln"><span class="t">—</span><span></span><span class="msg">Nenhuma linha com esse filtro.</span></span>`}</div>
            <div class="console-foot small">
              <div class="row" style="gap:var(--space-5)"><label class="switch"><input type="checkbox" role="switch" ${t.autoscroll ? "checked" : ""} data-change="con-auto">Rolar sozinho</label>
              <label class="switch"><input type="checkbox" role="switch" ${t.fullLines ? "checked" : ""} data-change="con-full">Linhas completas</label></div>
              <span class="xs muted">Texto exatamente como o jogo escreve, sem tradução.</span></div>
          </section>
          <aside class="stack">
            <section class="card card-pad stack-sm" aria-labelledby="det-t">
              <div class="row between"><h2 class="label" id="det-t">${icon("compare", "sm")} Mudanças detectadas</h2><span class="det-count num" id="det-count" aria-live="polite">${t.detected}</span></div>
              <p class="small text-2">Mexa à vontade em opções, configs dos mods e resource packs. Ao fechar o jogo você escolhe o que trazer para o pack.</p>
              <ul class="det-list" id="det-list">${detectedList()}</ul>
            </section>
            <div class="proto-sim" role="group" aria-label="Simulação do protótipo"><strong>Protótipo:</strong> simular o fim do teste
              <button data-action="game-end" data-how="ok">Fechar o jogo normalmente</button><button data-action="game-end" data-how="crash">Simular travamento</button></div>
          </aside>
        </div>
      </div>`;
    },
  };
  function detectedList() {
    const all = ["config/jade/plugins.json", "options.txt", "serverconfig/create-server.toml", "config/sodium-options.json", "config/xaerominimap.txt"];
    const n = S.test.detected;
    if (!n) return `<li class="xs muted">Nada ainda.</li>`;
    return all.slice(0, n).map((p) => `<li class="mono xs">${icon("edit", "sm")}${esc(p)}</li>`).join("");
  }
  actions["con-filter"] = (el) => {
    S.test.consoleFilter = el.dataset.k;
    render();
  };
  actions["con-q"] = (el) => {
    S.test.consoleQuery = el.value;
    render();
  };
  actions["con-auto"] = (el) => (S.test.autoscroll = el.checked);
  actions["con-full"] = (el) => {
    S.test.fullLines = el.checked;
    render();
  };
  actions["con-copy"] = () => toast("Console copiado (o nome de usuário do Windows continua no texto copiado).", "info");
  actions["game-stop"] = () =>
    dialog({
      title: "Parar o jogo agora?",
      body: `<p class="text-2">É como fechar o Minecraft à força. O que não foi salvo no mundo de teste se perde. As mudanças em configs já feitas continuam sendo detectadas.</p>`,
      foot: `<button class="btn" data-action="dlg-close">Continuar jogando</button><button class="btn btn-danger" data-action="game-end" data-how="ok">${icon("stop")}Parar o jogo</button>`,
    });
  actions["game-end"] = (el) => {
    closeDialog(true);
    S.test.startedAt = 0;
    S.test.lines = [];
    S.test.detected = 0;
    if (el.dataset.how === "crash") {
      S.crash = true;
      S.diag.ai = "idle";
      go("diagnostico");
    } else {
      S.changes = null;
      go("testar/mudancas");
    }
  };

  // ---------------------------------------------------------------------------
  // 6d. O que mudou durante o teste
  // ---------------------------------------------------------------------------
  function changes() {
    if (!S.changes) S.changes = W.clone(D.changes);
    return S.changes;
  }
  routes["testar/mudancas"] = {
    title: "Testar — o que mudou",
    nav: "testar",
    states: ["normal", "vazio"],
    render() {
      if (S.view === "vazio") {
        return `<div class="page">${testHead('O que <span class="hl">mudou</span> durante o teste', "", "mudancas")}${W.emptyState({
          art: "ok",
          title: "Nada mudou durante o teste",
          body: "Você jogou sem mexer em configs, opções ou resource packs. O pack continua exatamente como estava.",
          acts: `<a class="btn btn-primary" href="#/pack/mods">Voltar ao pack</a><a class="btn" href="#/testar">Testar de novo</a>`,
        })}</div>`;
      }
      const ch = changes();
      const sel = ch.find((c) => c.id === S.changeSel) || ch[0];
      const picked = ch.filter((c) => c.pick);
      const keysPicked = (c) => (c.keys ? c.keys.filter((k) => k.pick).length : 1);
      const nChanges = picked.reduce((a, c) => a + keysPicked(c), 0);
      const suggested = ch.filter((c) => ["ch1", "ch2", "ch3"].includes(c.id));
      const personal = ch.filter((c) => !["ch1", "ch2", "ch3"].includes(c.id));
      const row = (c) => `<li><div class="change${c.id === sel.id ? " active" : ""}" data-action="chg-sel" data-id="${c.id}">
          <input type="checkbox" ${c.pick ? "checked" : ""} data-change="chg-pick" data-id="${c.id}" aria-label="Trazer ${esc(c.path)} para o pack" data-fk="pick-${c.id}" style="width:16px;height:16px;margin-top:3px;accent-color:var(--c-accent)">
          <div style="min-width:0"><button class="path linklike" data-action="chg-sel" data-id="${c.id}" data-fk="chg-${c.id}" aria-current="${c.id === sel.id}">${esc(c.path).replace(/\//g, "/<wbr>")}</button>
            <p class="why">${esc(c.why)}</p></div>
          ${pill(c.kind === "Novo" ? "info" : "neutral", c.kind, c.kind === "Novo" ? "plus" : "edit")}
        </div></li>`;
      return `<div class="page wide">
        ${testHead('O que <span class="hl">mudou</span> durante o teste', "", "mudancas")}
        <section class="after-hero">
          <div>${pill("ok", "O jogo fechou normalmente")}<span class="small text-2" style="margin-left:var(--space-2)">jogou por 6 min 12 s</span>
            <p class="text-2" style="margin-top:var(--space-3);max-width:70ch">Encontramos <strong>${ch.length} arquivos</strong> diferentes do pack. Já marcamos o que parece fazer parte dele. <strong>Nada é copiado até você confirmar.</strong></p></div>
          <dl class="tiles">
            <div class="tile"><dt>Sugeridos para trazer</dt><dd class="num display">${suggested.length}</dd></div>
            <div class="tile"><dt>Parecem pessoais</dt><dd class="num display">${personal.length}</dd></div>
            <div class="tile"><dt>Ignorados sozinhos</dt><dd class="num display">${D.ignored.length}</dd></div>
          </dl>
        </section>
        <div class="split" style="grid-template-columns:minmax(320px,420px) minmax(0,1fr);align-items:start;margin-top:var(--space-5)">
          <section class="card" aria-labelledby="chg-list-t">
            <h2 class="sr-only" id="chg-list-t">Arquivos alterados</h2>
            <h3 class="group-title">${icon("ok", "sm")} Sugerimos trazer</h3><ul class="change-list">${suggested.map(row).join("")}</ul>
            <h3 class="group-title">${icon("user", "sm")} Parecem preferência pessoal ou automáticos</h3><ul class="change-list">${personal.map(row).join("")}</ul>
            <details class="passed"><summary>${icon("chevronRight", "sm")}${D.ignored.length} ignorados automaticamente</summary>
              <p class="xs muted" style="padding:0 var(--space-5)">Logs, mundos, capturas de tela e caches nunca vão para o pack.</p>
              <ul class="passed-list mono">${D.ignored.map((p) => `<li>${icon("minus", "sm")}${esc(p)}</li>`).join("")}</ul></details>
          </section>
          <section class="card" aria-labelledby="diff-t">${diffPanel(sel)}</section>
        </div>
        <div class="action-bar">
          <p class="small"><strong>${picked.length} ${picked.length === 1 ? "arquivo" : "arquivos"}</strong> (${nChanges} ${nChanges === 1 ? "mudança" : "mudanças"}) ${picked.length === 1 ? "vai" : "vão"} para o pack.</p>
          <div class="row"><button class="btn" data-action="chg-discard">Descartar tudo</button><button class="btn btn-primary" data-action="chg-apply" ${picked.length ? "" : "disabled"}>${icon("download")}Trazer para o pack</button></div>
        </div>
      </div>`;
    },
  };
  function diffPanel(c) {
    const destPath = c.path.startsWith("saves/") ? "defaultconfigs/create-server.toml" : c.path;
    const head = `<div class="card-head" style="align-items:flex-start;flex-wrap:wrap">
      <div style="min-width:0"><h2 class="mono" id="diff-t" style="font-size:var(--text-md)">${esc(c.path).replace(/\//g, "/<wbr>")}</h2>
        <p class="route small">${icon("monitor", "sm")}<span>instância de teste</span>${icon("arrowRight", "sm")}${icon("package", "sm")}<span>pack: <code>${esc(destPath)}</code></span></p></div>
      <label class="switch"><input type="checkbox" role="switch" ${c.pick ? "checked" : ""} data-change="chg-pick" data-id="${c.id}" data-fk="pick2-${c.id}"><span>Trazer</span></label></div>`;
    const dest = `<div class="alert ${c.pick ? "accent" : ""}" style="margin-bottom:var(--space-4)">${icon("info")}<div><div class="abody" style="margin:0">${esc(c.dest)}</div></div></div>`;
    if (c.keys) {
      return `${head}<div class="card-body">${dest}
        <p class="small text-2" style="margin-bottom:var(--space-2)">O <code>options.txt</code> é reescrito inteiro pelo jogo. Por isso trazemos só as opções que você escolher:</p>
        <ul class="keydiff">${c.keys
          .map(
            (k, i) => `<li><input type="checkbox" id="k-${i}" ${k.pick ? "checked" : ""} data-change="chg-key" data-i="${i}" data-fk="key-${i}" style="width:16px;height:16px;margin-top:3px;accent-color:var(--c-accent)">
          <label for="k-${i}"><strong>${esc(k.label)}</strong> <code class="xs muted">${esc(k.k)}</code>${k.note ? ` ${pill("neutral", k.note, "user")}` : ""}
            <span class="small" style="display:block;margin-top:var(--space-1);font-family:var(--font-mono)"><span class="old">${esc(k.old)}</span> → <span class="new">${esc(k.nw)}</span></span></label></li>`,
          )
          .join("")}</ul></div>`;
    }
    return `${head}<div class="card-body">${dest}
      <div class="code" tabindex="0" role="region" aria-label="Diferenças"><table>${c.diff.map((r) => `<tr class="${r[0] === "+" ? "add" : r[0] === "-" ? "del" : ""}"><td class="sign">${r[0] === " " ? "" : r[0]}<span class="sr-only">${r[0] === "+" ? "adicionado" : r[0] === "-" ? "removido" : ""}</span></td><td>${esc(r[1])}</td></tr>`).join("")}</table></div></div>`;
  }
  actions["chg-sel"] = (el, e) => {
    if (e.target.closest("input")) return;
    S.changeSel = el.dataset.id;
    render();
  };
  actions["chg-pick"] = (el) => {
    const c = changes().find((x) => x.id === el.dataset.id);
    c.pick = el.checked;
    render();
  };
  actions["chg-key"] = (el) => {
    const c = changes().find((x) => x.id === "ch1");
    c.keys[+el.dataset.i].pick = el.checked;
    c.pick = c.keys.some((k) => k.pick);
    render();
  };
  actions["chg-discard"] = () =>
    dialog({
      title: "Descartar todas as mudanças do teste?",
      body: `<p class="text-2">O pack continua como estava. As mudanças ficam só na instância de teste e somem no próximo teste limpo.</p>`,
      foot: `<button class="btn" data-action="dlg-close">Voltar</button><button class="btn btn-danger" data-action="chg-discard-ok">Descartar tudo</button>`,
    });
  actions["chg-discard-ok"] = () => {
    closeDialog(true);
    go("pack/mods");
    toast("Mudanças do teste descartadas. O pack não mudou.", "info");
  };
  actions["chg-apply"] = () => {
    const n = changes().filter((c) => c.pick).length;
    S.unsaved += n;
    S.pack.test = { kind: "ok", text: "Passou" };
    go("pack/configs");
    toast(`${n} ${n === 1 ? "arquivo trazido" : "arquivos trazidos"} para o pack. Quando estiver satisfeito, <a href="#/versoes">salve uma versão</a>.`);
  };

  // ---------------------------------------------------------------------------
  // 7. Diagnóstico de crash
  // ---------------------------------------------------------------------------
  routes["diagnostico"] = {
    title: "Diagnóstico de crash",
    nav: "diagnostico",
    states: ["normal", "carregando", "erro"],
    render() {
      const fixed = S.diag.fixed;
      return `<div class="page">
        <div class="page-head"><div class="titles">
          <nav class="crumbs" aria-label="Você está em"><a href="#/pack/mods">${esc(S.pack.name)}</a>${icon("chevronRight", "sm")}<span>Diagnóstico</span></nav>
          <h1 id="page-title" class="page-title" tabindex="-1">O jogo <span class="hl danger">travou</span> ao carregar</h1>
          <p class="page-sub row wrap">${pill("danger", "Travou")}<span>hoje, 21:43 · 18 segundos depois de abrir · ainda na tela de carregamento</span></p></div>
          <div class="page-actions"><a class="btn" href="#/testar/jogo" data-action="soon" data-msg="Abriria o console completo da última sessão.">${icon("terminal")}Ver console</a><a class="btn btn-primary" href="#/testar">${icon("play")}Testar de novo</a></div></div>
        ${fixed ? `<div class="alert ok" style="margin-bottom:var(--space-5)">${icon("ok")}<div><div class="atitle">${esc(fixed)}</div><div class="abody">Teste de novo para confirmar que resolveu.</div></div><div class="aactions"><a class="btn btn-sm btn-primary" href="#/testar">${icon("play", "sm")}Testar de novo</a></div></div>` : ""}
        <div class="split split-main-side" style="align-items:start">
          <div class="stack">
            <section class="card diag-summary" aria-labelledby="what-t">
              <div class="card-body">
                <p class="eyebrow">${icon("activity", "sm")} O que aconteceu</p>
                <h2 class="hero-title" id="what-t" style="margin-top:var(--space-2)">O mod <em>Create Crafts &amp; Additions</em> não combina com a versão do Create que está no pack</h2>
                <p class="text-2" style="margin-top:var(--space-3)">A versão 1.3.2 dele tentou usar uma parte do Create que mudou na 6.0.8. Por isso o jogo parou antes de chegar ao menu. Seu mundo e o pack não foram afetados.</p>
                <div class="confidence">${icon("shield", "sm")}<span><strong>Certeza alta.</strong> A mensagem de erro cita o mod pelo nome. A checagem antes do teste não pegou isso porque o mod não informa com quais versões do Create funciona.</span></div>
              </div>
            </section>
            <section class="card" aria-labelledby="fix-t">
              <div class="card-head"><h2 class="section-title" id="fix-t">Como resolver</h2><span class="xs muted">um clique, e você pode desfazer</span></div>
              <ol class="fix-list">
                <li><div class="grow"><div class="row wrap"><strong>Atualizar Create Crafts &amp; Additions para 1.3.4</strong>${pill("ok", "Recomendado", "sparkles")}</div><p class="small text-2">A versão 1.3.4 foi feita para o Create 6.0.8.</p></div><button class="btn btn-primary" data-action="diag-fix" data-msg="Create Crafts & Additions atualizado para 1.3.4.">${icon("arrowUp")}Atualizar</button></li>
                <li><div class="grow"><strong>Desativar Create Crafts &amp; Additions por enquanto</strong><p class="small text-2">O pack abre, mas os fios, motores e baterias desse mod somem do mundo de teste.</p></div><button class="btn" data-action="diag-fix" data-msg="Create Crafts & Additions desativado.">${icon("pause")}Desativar</button></li>
                <li><div class="grow"><strong>Voltar o Create para 6.0.6</strong><p class="small text-2">Era a versão que funcionava na versão 1.4.0 do pack.</p></div><button class="btn" data-action="diag-fix" data-msg="Create voltou para 6.0.6.">${icon("rotateCcw")}Voltar versão</button></li>
              </ol>
            </section>
            <section class="card" aria-labelledby="ev-t">
              <div class="card-head"><div><h2 class="section-title" id="ev-t">A prova no log</h2><p class="section-sub"><code>logs/latest.log</code>, linhas 1827 a 1833. As linhas marcadas são as que explicam o problema.</p></div>
                <div class="row"><button class="btn btn-sm" data-action="soon" data-msg="Copiaria o trecho do log.">${icon("copy", "sm")}Copiar</button><button class="btn btn-sm" data-action="soon" data-msg="Abriria o log completo.">${icon("fileText", "sm")}Log completo</button></div></div>
              <div class="card-body"><div class="code log" tabindex="0" role="region" aria-label="Trecho do log"><table>${D.crashLog.map(([n, l, h]) => `<tr class="${h ? "hl" : ""}"><td class="ln">${n}</td><td>${esc(l)}${h ? '<span class="sr-only"> (linha importante)</span>' : ""}</td></tr>`).join("")}</table></div></div>
            </section>
          </div>
          <aside class="stack">
            <section class="card card-pad stack-sm" aria-labelledby="sus-t">
              <h2 class="label" id="sus-t">Mod suspeito</h2>
              <div class="row" style="gap:var(--space-3)">${ticon("Create Crafts & Additions", "lg")}<div><strong>Create Crafts &amp; Additions</strong><div class="small text-2 mono">1.3.2 · createaddition</div></div></div>
              <dl class="kv" style="margin-top:var(--space-2)"><dt>Fonte</dt><dd>${W.src("modrinth")}</dd><dt>Atualização</dt><dd><span class="upd" style="margin:0">${icon("arrowUp", "sm")}1.3.4 disponível</span></dd><dt>Depende de</dt><dd>Create (6.0.8 no pack)</dd></dl>
              <a class="btn btn-sm" href="#/pack/mods">${icon("package", "sm")}Ver no pack</a>
            </section>
            ${aiCard()}
          </aside>
        </div></div>`;
    },
  };
  function aiCard() {
    if (S.view === "erro")
      return `<section class="card card-pad stack-sm ai-card" aria-labelledby="ai-t"><h2 class="label" id="ai-t">${icon("sparkles", "sm")} Segunda opinião da IA</h2>
        <div class="alert warn">${icon("key")}<div><div class="atitle">Falta a chave do Gemini</div><div class="abody">Para usar a IA, cole a sua chave do Gemini em Configurações. Ela fica guardada no cofre de senhas do Windows.</div></div></div>
        <a class="btn" href="#/ajustes">${icon("settings")}Abrir Configurações</a></section>`;
    if (S.view === "carregando" || S.diag.ai === "loading")
      return `<section class="card card-pad stack-sm ai-card" aria-labelledby="ai-t" aria-busy="true"><h2 class="label" id="ai-t">${icon("sparkles", "sm")} Segunda opinião da IA</h2>
        <div class="loading-line" role="status"><span class="spinner"></span>O Gemini está lendo o log…</div><p class="xs muted">Costuma levar de 5 a 20 segundos.</p>
        <button class="btn btn-sm" data-action="ai-cancel">Cancelar</button></section>`;
    if (S.diag.ai === "done")
      return `<section class="card card-pad stack-sm ai-card" aria-labelledby="ai-t"><h2 class="label" id="ai-t">${icon("sparkles", "sm")} Resposta da IA</h2>
        <p class="small">Concordo com a checagem automática: o <strong>createaddition 1.3.2</strong> chama um método do Create que não existe mais na 6.0.8. Atualizar para a <strong>1.3.4</strong> deve resolver. Não vi outros problemas no trecho enviado.</p>
        <p class="xs muted">${icon("info", "sm")} Resposta gerada por IA. Confira antes de aplicar. Enviado: lista de mods e 31 linhas do log, sem dados pessoais.</p></section>`;
    return `<section class="card card-pad stack-sm ai-card" aria-labelledby="ai-t"><h2 class="label" id="ai-t">${icon("sparkles", "sm")} Ainda com dúvida?</h2>
      <p class="small text-2">Peça uma segunda opinião à IA (Gemini). Antes de enviar, você vê exatamente o que vai e o que será apagado.</p>
      <button class="btn" data-action="ai-ask">${icon("sparkles")}Analisar com IA…</button></section>`;
  }
  actions["diag-fix"] = (el) => {
    S.diag.fixed = el.dataset.msg;
    const m = S.mods.find((x) => x.id === "createaddition");
    if (m && el.dataset.msg.includes("1.3.4")) {
      m.ver = "1.3.4";
      delete m.upd;
      m.changed = "Atualizado de 1.3.2 (não salvo)";
    }
    S.unsaved++;
    S.crash = false;
    render();
    document.getElementById("page-title")?.focus();
  };
  actions["ai-ask"] = () => {
    const prev = D.aiPreview
      .map((l) =>
        esc(l)
          .replace("{{JOGADOR}}", '<span class="redact">[JOGADOR]</span>')
          .replace("{{PASTA}}", '<span class="redact">C:\\Users\\[USUÁRIO]</span>')
          .replace("{{IP}}", '<span class="redact">[IP]</span>'),
      )
      .map((l, i) => `<tr><td class="ln">${i + 1}</td><td style="white-space:pre-wrap">${l}</td></tr>`)
      .join("");
    dialog({
      wide: true,
      lead: `<span class="ticon" style="--h:200" aria-hidden="true">${icon("sparkles")}</span>`,
      title: "Enviar para a IA (Gemini)?",
      sub: "Nada sai do seu computador antes de você confirmar. O envio usa a sua chave do Gemini.",
      body: `<div class="split split-2" style="gap:var(--space-6)">
        <div class="stack">
          <fieldset style="border:0;padding:0;margin:0"><legend class="label" style="margin-bottom:var(--space-2)">O que será enviado</legend>
            <div class="stack-sm">
              <label class="check"><input type="checkbox" checked><span>Lista de mods e versões <span class="muted">(${S.mods.length})</span></span></label>
              <label class="check"><input type="checkbox" checked><span>Resultado da checagem automática</span></label>
              <label class="check"><input type="checkbox" checked><span>Trecho do log <span class="muted">(31 de 4.210 linhas)</span></span></label>
              <label class="check"><input type="checkbox"><span>Configs do pack <span class="muted">(normalmente não precisa)</span></span></label>
            </div></fieldset>
          <div><h3 class="label">Dados pessoais apagados antes do envio</h3>
            <ul class="redact-list">
              <li>${icon("user", "sm")}<span>Seu nome de usuário do Windows nas pastas</span><span class="redact">C:\\Users\\[USUÁRIO]</span></li>
              <li>${icon("user", "sm")}<span>Nome do jogador</span><span class="redact">[JOGADOR]</span></li>
              <li>${icon("server", "sm")}<span>Endereços IP</span><span class="redact">[IP]</span></li>
              <li>${icon("key", "sm")}<span>Chaves e senhas que aparecerem</span><span class="redact">removidas</span></li>
            </ul></div>
        </div>
        <div><h3 class="label" style="margin-bottom:var(--space-2)">Prévia do que vai</h3><div class="code" tabindex="0" role="region" aria-label="Prévia do envio" style="max-height:280px"><table>${prev}</table></div>
          <p class="xs muted" style="margin-top:var(--space-2)">Os trechos destacados foram trocados. O Google processa o envio conforme os termos da sua conta do Gemini.</p></div>
      </div>`,
      foot: `<button class="btn" data-action="dlg-close">Não enviar</button><button class="btn btn-primary" data-action="ai-send">${icon("send")}Enviar para a IA</button>`,
    });
  };
  actions["ai-send"] = () => {
    closeDialog(true);
    S.diag.ai = "loading";
    render();
    after(W.reducedMotion() ? 600 : 2200, () => {
      S.diag.ai = "done";
      if (S.route === "diagnostico") render();
      toast("A IA respondeu. Veja ao lado do diagnóstico.", "info");
    });
  };
  actions["ai-cancel"] = () => {
    W.clearTimers();
    S.diag.ai = "idle";
    if (S.view === "carregando") W.setView("normal");
    else render();
  };
})();
