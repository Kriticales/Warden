// Telas 8–10: Salvar versão e histórico, Exportar, Configurações.
(function () {
  const { D, S, esc, ticon, pill, status, chip, toast, dialog, closeDialog, routes, actions, go, render } = W;

  W.screenList.push(["versoes", "8. Salvar versão e histórico"], ["exportar", "9. Exportar"], ["ajustes", "10. Configurações"]);

  const plural = (n, one, many) => `${n} ${n === 1 ? one : many}`;
  function crumbs(last) {
    return `<nav class="crumbs" aria-label="Você está em"><a href="#/pack/mods">${esc(S.pack.name)}</a>${icon("chevronRight", "sm")}<span>${last}</span></nav>`;
  }

  // ---------------------------------------------------------------------------
  // 8. Versões
  // ---------------------------------------------------------------------------
  function nextVersion() {
    const [a, b, c] = S.pack.version.split(".").map(Number);
    const base = S.ver.bump === "major" ? `${a + 1}.0.0` : S.ver.bump === "minor" ? `${a}.${b + 1}.0` : `${a}.${b}.${c + 1}`;
    return S.ver.channel === "Estável" ? base : `${base}-${S.ver.channel === "Beta" ? "beta" : "alpha"}.1`;
  }
  function autoChangelog() {
    const added = S.mods.filter((m) => /^Adicionado/.test(m.changed || "")).map((m) => `${m.name} ${m.ver}`);
    const updated = S.mods.filter((m) => /^Atualizado/.test(m.changed || "")).map((m) => `${m.name} ${m.changed.match(/de (\S+)/)[1]} → ${m.ver}`);
    const removed = D.mods.filter((m) => !S.mods.find((x) => x.id === m.id)).map((m) => m.name);
    const L = [];
    if (added.length) L.push("## Adicionados", ...added.map((x) => "- " + x), "");
    if (updated.length) L.push("## Atualizados", ...updated.map((x) => "- " + x), "");
    if (removed.length) L.push("## Removidos", ...removed.map((x) => "- " + x), "");
    L.push("## Configs", ...D.pending.configs.map((x) => "- " + x));
    return { text: L.join("\n"), added, updated, removed };
  }
  routes["versoes"] = {
    title: "Versões",
    nav: "versoes",
    states: ["normal", "vazio", "erro"],
    render() {
      const cl = autoChangelog();
      if (S.ver.changelog === null) S.ver.changelog = cl.text;
      const v = nextVersion();
      const [a, b, c] = S.pack.version.split(".").map(Number);
      const bumps = [
        { k: "patch", t: "Correção", n: `${a}.${b}.${c + 1}`, d: "Só atualizações de mods e ajustes de config. Quem já joga atualiza sem medo." },
        { k: "minor", t: "Novidade", n: `${a}.${b + 1}.0`, d: "Mods novos ou mudanças de equilíbrio. Mundos antigos continuam funcionando." },
        { k: "major", t: "Grande mudança", n: `${a + 1}.0.0`, d: "Trocou a versão do Minecraft ou do loader, ou removeu mods que têm blocos no mundo. Peça backup do mundo." },
      ];
      const empty = S.view === "vazio";
      const timeline = empty
        ? `<div class="empty" style="padding:var(--space-8) var(--space-5)"><span class="art" aria-hidden="true">${icon("history", "xl")}</span><h2>Nenhuma versão salva ainda</h2><p>Salve a primeira quando o pack estiver jogável. Ela vira o ponto de partida do histórico, e você sempre pode voltar a ela.</p></div>`
        : `<ol class="timeline">
          <li><span class="node draft" aria-hidden="true">${icon("edit", "sm")}</span><div><div class="vhead"><span class="vtitle">Alterações não salvas</span>${pill("warn", S.unsaved + " mudanças", "edit")}</div><p class="small text-2" style="margin-top:var(--space-1)">Desde a ${esc(S.pack.version)}. Salve ao lado para virar a ${esc(v)}.</p></div></li>
          ${D.versions
            .map(
              (x) => `<li><span class="node${x.current ? " current" : ""}" aria-hidden="true">${icon(x.current ? "check" : "commit", "sm")}</span><div>
            <div class="vhead"><span class="vnum">${x.v}</span><span class="vtitle">${esc(x.title)}</span>${x.current ? chip("Atual", "", "accent") : ""}${x.channel ? chip(x.channel) : ""}</div>
            <div class="vsum"><span>${x.date}</span><span class="num">+${x.added} mods</span><span class="num">↑${x.updated} atualizados</span>${x.removed ? `<span class="num">−${x.removed} removido</span>` : ""}${x.configs ? `<span class="num">${x.configs} configs</span>` : ""}</div>
            <div class="vsum">${status(x.test, x.testText)}${x.pushed ? status("ok", "No GitHub", "cloudUp") : status("neutral", "Só no computador", "hardDrive")}</div>
            <div class="vacts"><button class="btn btn-sm" data-action="ver-see" data-v="${x.v}">${icon("fileText", "sm")}Ver mudanças</button>${x.current ? "" : `<button class="btn btn-sm btn-ghost" data-action="ver-back" data-v="${x.v}">${icon("rotateCcw", "sm")}Voltar para esta versão</button>`}</div></div></li>`,
            )
            .join("")}</ol>`;
      return `<div class="page wide">
        <div class="page-head"><div class="titles">${crumbs("Versões")}<h1 id="page-title" class="page-title" tabindex="-1">Versões do pack</h1>
          <p class="page-sub">Cada versão é uma foto do pack que você pode compartilhar e para a qual sempre pode voltar.</p></div></div>
        ${S.view === "erro" ? `<div class="alert danger" style="margin-bottom:var(--space-5)">${icon("wifiOff")}<div><div class="atitle">A versão 1.4.0 foi salva no computador, mas não chegou ao GitHub</div><div class="abody">Sem conexão com a internet. Nada se perdeu: tentamos enviar de novo sozinhos quando a conexão voltar.</div></div><div class="aactions"><button class="btn btn-sm" data-action="ver-retry">${icon("refresh", "sm")}Tentar enviar agora</button></div></div>` : ""}
        <div class="split" style="grid-template-columns:minmax(0,1.1fr) minmax(0,1fr);align-items:start">
          <section class="card" aria-labelledby="save-t">
            <div class="card-head"><div><h2 class="section-title" id="save-t">Salvar nova versão</h2><p class="section-sub">Desde a ${esc(S.pack.version)}: ${plural(cl.added.length, "mod adicionado", "mods adicionados")}, ${plural(cl.updated.length, "atualizado", "atualizados")}, ${plural(cl.removed.length, "removido", "removidos")} e ${plural(D.pending.configs.length, "config", "configs")}.</p></div></div>
            <div class="card-body stack">
              <fieldset style="border:0;padding:0;margin:0"><legend class="label" style="margin-bottom:var(--space-2)">Que tipo de mudança é essa?</legend>
                <div class="choice-grid" style="grid-template-columns:repeat(3,minmax(0,1fr))">
                  ${bumps
                    .map(
                      (x) => `<label class="choice"><input type="radio" name="bump" value="${x.k}" data-change="ver-bump" ${S.ver.bump === x.k ? "checked" : ""}>
                    <span class="ctitle">${x.t}<span class="radio-dot" aria-hidden="true"></span></span>
                    <span class="bump-n mono">${x.n}</span><span class="cdesc">${x.d}</span>${x.k === "minor" ? `<span class="tag-rec">${pill("info", "Sugerido", "sparkles")}</span>` : ""}</label>`,
                    )
                    .join("")}
                </div></fieldset>
              <div class="alert info">${icon("sparkles")}<div><div class="atitle">Por que sugerimos “Novidade”?</div><div class="abody">Você adicionou ${cl.added.length} mods (${esc(cl.added.map((x) => x.replace(/ [\d.]+$/, "")).join(", "))}). Nenhum mod com blocos no mundo foi removido${cl.removed.length ? ` (o ${esc(cl.removed.join(", "))} não tem blocos)` : ""}, então mundos antigos continuam funcionando.</div></div></div>
              <div class="row wrap" style="gap:var(--space-5);align-items:flex-end">
                <div class="field"><span class="label" id="ch-l">Canal</span><div class="segmented" role="group" aria-labelledby="ch-l">${["Estável", "Beta", "Alfa"].map((c) => `<button aria-pressed="${S.ver.channel === c}" data-action="ver-channel" data-c="${c}" data-fk="chan-${c}">${c}</button>`).join("")}</div></div>
                <div class="field grow" style="margin:0"><label class="label" for="ver-title">Título <span class="opt">(opcional)</span></label><input id="ver-title" class="input" value="${esc(S.ver.title)}" data-input="ver-title" data-fk="ver-title"></div>
              </div>
              <div class="field"><div class="row between"><label class="label" for="ver-cl">Notas da versão (changelog)</label><button class="btn btn-ghost btn-sm" data-action="ver-regen">${icon("refresh", "sm")}Gerar de novo</button></div>
                <textarea id="ver-cl" class="textarea mono" rows="11" data-input="ver-cl" data-fk="ver-cl" aria-describedby="ver-cl-h">${esc(S.ver.changelog)}</textarea>
                <span class="hint" id="ver-cl-h">Gerado a partir das mudanças reais do pack. Pode editar à vontade.</span></div>
              <label class="check"><input type="checkbox" ${S.ver.push ? "checked" : ""} data-change="ver-push"><span>Enviar para o GitHub depois de salvar <span class="muted">(repositório privado <code>vale-das-engrenagens</code>)</span></span></label>
            </div>
            <div class="card-foot" style="justify-content:space-between"><span class="small text-2">Vai ficar: <strong class="mono">${esc(v)}</strong>${S.ver.title ? ` — ${esc(S.ver.title)}` : ""}</span><button class="btn btn-primary" data-action="ver-save" ${S.unsaved ? "" : "disabled"}>${icon("save")}Salvar versão ${esc(v)}</button></div>
          </section>
          <section class="card" aria-labelledby="hist-t"><div class="card-head"><h2 class="section-title" id="hist-t">Histórico</h2><span class="xs muted">${empty ? "" : D.versions.length + " versões"}</span></div><div class="card-body">${timeline}</div></section>
        </div></div>`;
    },
  };
  actions["ver-bump"] = (el) => {
    S.ver.bump = el.value;
    render();
    document.querySelector(`input[name="bump"][value="${el.value}"]`)?.focus();
  };
  actions["ver-channel"] = (el) => {
    S.ver.channel = el.dataset.c;
    render();
  };
  actions["ver-title"] = (el) => (S.ver.title = el.value);
  actions["ver-cl"] = (el) => (S.ver.changelog = el.value);
  actions["ver-push"] = (el) => (S.ver.push = el.checked);
  actions["ver-regen"] = () => {
    S.ver.changelog = autoChangelog().text;
    render();
  };
  actions["ver-retry"] = () => {
    W.setView("normal");
    toast("Versão 1.4.0 enviada ao GitHub.");
  };
  actions["ver-save"] = () => {
    const v = nextVersion();
    D.versions.forEach((x) => (x.current = false));
    const cl = autoChangelog();
    D.versions.unshift({ v, title: S.ver.title || "Sem título", date: "1 out 2026", added: cl.added.length, updated: cl.updated.length, removed: cl.removed.length, configs: 2, test: "ok", testText: "Passou", pushed: S.ver.push, current: true, channel: S.ver.channel === "Estável" ? "" : S.ver.channel });
    S.pack.version = v;
    S.unsaved = 0;
    S.mods.forEach((m) => delete m.changed);
    S.ver.changelog = null;
    S.ver.title = "";
    render();
    toast(`Versão <strong>${esc(v)}</strong> salva${S.ver.push ? " e enviada ao GitHub" : ""}.`);
    document.getElementById("hist-t")?.setAttribute("tabindex", "-1");
    document.getElementById("hist-t")?.focus();
  };
  actions["ver-see"] = (el) => {
    const x = D.versions.find((y) => y.v === el.dataset.v);
    dialog({
      title: `Versão ${esc(x.v)} — ${esc(x.title)}`,
      sub: `${x.date} · ${x.testText}`,
      body: `<div class="code" style="padding:var(--space-3) var(--space-4);white-space:pre-wrap">## Adicionados\n- (${x.added} mods)\n\n## Atualizados\n- (${x.updated} mods)\n\n## Configs\n- (${x.configs} arquivos)</div><p class="xs muted" style="margin-top:var(--space-2)">No app aparecem os nomes e versões reais, como no changelog ao lado.</p>`,
      foot: `<button class="btn btn-primary" data-action="dlg-close">Fechar</button>`,
    });
  };
  actions["ver-back"] = (el) => {
    const v = el.dataset.v;
    dialog({
      title: `Voltar para a versão ${esc(v)}?`,
      body: `<p class="text-2">O pack fica exatamente como estava na ${esc(v)}: mods, configs e arquivos.</p>
        <ul class="stack-sm small" style="margin-top:var(--space-4)">
          <li class="row" style="align-items:flex-start">${icon("ok", "sm")}<span>Suas ${S.unsaved} alterações não salvas são guardadas num rascunho, para você recuperar se quiser.</span></li>
          <li class="row" style="align-items:flex-start">${icon("ok", "sm")}<span>As versões mais novas continuam no histórico. Nada é apagado.</span></li>
          <li class="row" style="align-items:flex-start">${icon("warn", "sm")}<span>Mundos de teste criados depois da ${esc(v)} podem perder blocos de mods que não existiam nela.</span></li>
        </ul>`,
      foot: `<button class="btn" data-action="dlg-close">Cancelar</button><button class="btn btn-primary" data-action="ver-back-ok" data-v="${esc(v)}">${icon("rotateCcw")}Voltar para ${esc(v)}</button>`,
    });
  };
  actions["ver-back-ok"] = (el) => {
    closeDialog();
    toast(`O pack voltou para a versão ${esc(el.dataset.v)}. Suas alterações anteriores estão no rascunho.`, "info");
  };

  // ---------------------------------------------------------------------------
  // 9. Exportar
  // ---------------------------------------------------------------------------
  const fmtKB = (kb) => (kb >= 1000 ? (kb / 1024).toFixed(1).replace(".", ",") + " MB" : (kb < 10 ? kb.toFixed(1).replace(".", ",") : Math.round(kb)) + " KB");
  const children = {
    "mods/": [["create.pw.toml", 0.5], ["createaddition.pw.toml", 0.5], ["sodium.pw.toml", 0.4], ["…mais 21 referências", 10.4], ["vale-utils-1.0.0.jar", 84, "local"]],
    "resourcepacks/": [["fresh-animations.pw.toml", 0.5], ["default-dark-mode.pw.toml", 0.6], ["Texturas do Vale.zip", 2150, "local"]],
    "config/": [["create-client.toml", 6.1], ["jade/plugins.json", 1.2], ["…mais 29 arquivos", 278.7]],
  };
  routes["exportar"] = {
    title: "Exportar",
    nav: "exportar",
    states: ["normal", "carregando", "erro"],
    render() {
      const tree = D.exportTree;
      const total = tree.reduce((a, x) => a + x.size, 0);
      const max = Math.max(...tree.map((x) => x.size));
      let preview;
      if (S.view === "carregando") {
        preview = `<div class="card-body">${W.loadingSr("Montando a pré-visualização…")}<div class="loading-line"><span class="spinner"></span>Conferindo o pack com o packwiz e calculando tamanhos…</div><div class="stack-sm" style="margin-top:var(--space-5)" aria-hidden="true">${tree.map(() => `<span class="skel" style="height:22px"></span>`).join("")}</div></div>`;
      } else if (S.view === "erro") {
        preview = `<div class="card-body">${W.emptyState({ error: true, title: "O packwiz encontrou um problema", body: "O <code>index.toml</code> está desatualizado: 2 arquivos de config mudaram depois do último refresh. Dá para corrigir sozinho, sem mexer no conteúdo.", acts: `<button class="btn btn-primary" data-action="exp-fix">${icon("wrench")}Corrigir e continuar</button><button class="btn" data-action="soon">${icon("terminal")}Ver saída do packwiz</button>` })}</div>`;
      } else {
        const rows = tree
          .map((x) => {
            const kids = children[x.name];
            const open = S.exp.open[x.name];
            const kidRows = kids && open ? kids.map(([n, s, local]) => `<tr class="child"><td><span class="row" style="padding-left:var(--space-8)">${icon(local ? "file" : "link", "sm")}<span class="mono small">${esc(n)}</span>${local ? chip("arquivo inteiro") : ""}</span></td><td><div class="sizebar"><div class="track" aria-hidden="true"><span style="width:${(s / max) * 100}%"></span></div><span class="v">${fmtKB(s)}</span></div></td></tr>`).join("") : "";
            return `<tr><td><span class="row">${kids ? `<button class="btn btn-ghost btn-icon btn-sm" aria-expanded="${!!open}" aria-label="${open ? "Recolher" : "Expandir"} ${esc(x.name)}" data-action="exp-toggle" data-n="${esc(x.name)}" data-fk="exp-${esc(x.name)}">${icon(open ? "chevronDown" : "chevronRight", "sm")}</button>` : `<span style="width:var(--control-h-sm);display:inline-block"></span>`}${icon(x.kind === "dir" ? "folder" : "fileText", "sm")}<span><span class="mono small"><strong>${esc(x.name)}</strong></span><span class="xs muted" style="display:block">${esc(x.note)}</span></span></span></td>
            <td style="width:42%"><div class="sizebar"><div class="track" aria-hidden="true"><span style="width:${(x.size / max) * 100}%"></span></div><span class="v">${fmtKB(x.size)}</span></div></td></tr>${kidRows}`;
          })
          .join("");
        preview = `<div class="table-wrap" style="border:0;border-radius:0"><table class="table export-table" aria-label="Arquivos que vão no pack"><thead><tr><th>Arquivo ou pasta</th><th>Tamanho no pack</th></tr></thead><tbody>${rows}</tbody>
          <tfoot><tr><td><strong>Total</strong> <span class="xs muted">· 112 arquivos</span></td><td><span class="num"><strong>${fmtKB(total)}</strong></span></td></tr></tfoot></table></div>`;
      }
      return `<div class="page wide">
        <div class="page-head"><div class="titles">${crumbs("Exportar")}<h1 id="page-title" class="page-title" tabindex="-1">Exportar o pack</h1>
          <p class="page-sub">O resultado é uma pasta no formato do packwiz, só com o necessário: mods, resource packs, shaders, configs e o que você adicionou.</p></div></div>
        ${S.exp.done ? `<div class="alert ok" style="margin-bottom:var(--space-5)">${icon("ok")}<div><div class="atitle">Pack exportado</div><div class="abody">Em <code>D:\\Modpacks\\Exportados\\vale-das-engrenagens-1.4.0</code> · 2,5 MB · 112 arquivos.</div></div><div class="aactions"><button class="btn btn-sm" data-action="soon">${icon("folderOpen", "sm")}Abrir pasta</button></div></div>` : ""}
        <div class="split split-main-side" style="align-items:start">
          <div class="stack">
            <section class="card" aria-labelledby="prev-t"><div class="card-head"><div><h2 class="section-title" id="prev-t">Exatamente o que vai no pack</h2><p class="section-sub">Referências (.pw.toml) ocupam menos de 1 KB: quem instala baixa o mod da fonte original.</p></div></div>${preview}</section>
            ${S.view === "normal" ? `<section class="card" aria-labelledby="alerts-t"><div class="card-head"><h2 class="section-title" id="alerts-t">Antes de exportar</h2></div><ul>
              <li class="finding"><span class="sev warn">${icon("warn", "lg")}</span><div><div class="ftitle mono small">config/sodium-fingerprint.json</div><p class="fbody small">Arquivo gerado pelo Sodium que identifica este computador. Não serve para outros jogadores.</p>
                <div class="fix"><label class="switch small"><input type="checkbox" role="switch" ${S.exp.excludeFingerprint ? "checked" : ""} data-change="exp-fp">Deixar de fora</label></div></div></li>
              <li class="finding"><span class="sev info">${icon("info", "lg")}</span><div><div class="ftitle mono small">resourcepacks/Texturas do Vale.zip · 2,1 MB</div><p class="fbody small">Não vem do Modrinth nem do CurseForge, então vai copiado inteiro. É o maior item do pack.</p></div></li>
              <li class="finding"><span class="sev info">${icon("info", "lg")}</span><div><div class="ftitle mono small">options.txt</div><p class="fbody small">Não vai inteiro: só as 3 opções que você escolheu, aplicadas na primeira vez que o jogador abrir.</p></div></li>
              <li class="finding"><span class="sev ok">${icon("shield", "lg")}</span><div><div class="ftitle">Conferido com o packwiz</div><p class="fbody small">O pack exportado é exatamente o que o packwiz geraria. Nenhum arquivo fora da lista.</p></div></li>
            </ul>
            <details class="passed"><summary>${icon("chevronRight", "sm")}Deixados de fora automaticamente</summary><ul class="passed-list">${D.exportExcluded.map((x) => `<li>${icon("minus", "sm")}${esc(x)}</li>`).join("")}</ul></details></section>` : ""}
          </div>
          <aside class="card" aria-labelledby="dest-t" style="position:sticky;top:calc(var(--protobar-h) + var(--space-4))">
            <div class="card-head"><h2 class="section-title" id="dest-t">Destino</h2></div>
            <div class="card-body stack">
              <fieldset style="border:0;padding:0;margin:0"><legend class="label" style="margin-bottom:var(--space-2)">Formato</legend>
                <div class="stack-sm">
                  <label class="choice"><input type="radio" name="fmt" checked><span class="ctitle">Pasta do pack (packwiz)<span class="radio-dot" aria-hidden="true"></span></span><span class="cdesc">Para publicar no GitHub ou instalar com o packwiz-installer.</span></label>
                  <label class="choice"><input type="radio" name="fmt" disabled aria-describedby="soon-1"><span class="ctitle">Arquivo .mrpack (Modrinth)</span><span class="creason" id="soon-1">${icon("clock", "sm")}Em breve</span></label>
                  <label class="choice"><input type="radio" name="fmt" disabled aria-describedby="soon-2"><span class="ctitle">Arquivo .zip (CurseForge)</span><span class="creason" id="soon-2">${icon("clock", "sm")}Em breve</span></label>
                </div></fieldset>
              <fieldset style="border:0;padding:0;margin:0"><legend class="label" style="margin-bottom:var(--space-2)">O que exportar</legend>
                <div class="stack-sm">
                  <label class="check"><input type="radio" name="src" value="salva" ${S.exp.source === "salva" ? "checked" : ""} data-change="exp-src"><span>A versão salva ${esc(S.pack.version)}</span></label>
                  <label class="check"><input type="radio" name="src" value="atual" ${S.exp.source === "atual" ? "checked" : ""} data-change="exp-src"><span>O pack como está agora <span class="muted">(com ${S.unsaved} alterações não salvas)</span></span></label>
                </div></fieldset>
              <div class="field"><label class="label" for="exp-dest">Pasta de destino</label>
                <div class="input-affix"><input id="exp-dest" class="input mono small" value="D:\\Modpacks\\Exportados"><button class="btn" data-action="soon" data-msg="Abriria o seletor de pastas do Windows.">Escolher…</button></div>
                <span class="hint">Será criada a pasta <code>vale-das-engrenagens-${esc(S.pack.version)}${S.exp.source === "atual" && S.unsaved ? "-rascunho" : ""}</code>.</span></div>
              <label class="check"><input type="checkbox" checked><span>Abrir a pasta quando terminar</span></label>
            </div>
            <div class="card-foot"><button class="btn btn-primary" data-action="exp-go" ${S.view === "normal" ? "" : "disabled"}>${icon("upload")}Exportar pack</button></div>
          </aside>
        </div></div>`;
    },
  };
  actions["exp-toggle"] = (el) => {
    S.exp.open[el.dataset.n] = !S.exp.open[el.dataset.n];
    render();
  };
  actions["exp-fp"] = (el) => (S.exp.excludeFingerprint = el.checked);
  actions["exp-src"] = (el) => {
    S.exp.source = el.value;
    render();
  };
  actions["exp-fix"] = () => {
    W.setView("normal");
    toast("index.toml atualizado pelo packwiz.");
  };
  actions["exp-go"] = (el) => {
    el.disabled = true;
    el.innerHTML = `<span class="spinner" aria-hidden="true"></span>Exportando…`;
    W.after(W.reducedMotion() ? 300 : 1400, () => {
      S.exp.done = true;
      render();
      toast("Pack exportado.");
      document.getElementById("page-title")?.focus();
    });
  };

  // ---------------------------------------------------------------------------
  // 10. Configurações
  // ---------------------------------------------------------------------------
  routes["ajustes"] = {
    title: "Configurações",
    nav: "ajustes",
    states: ["normal"],
    render() {
      const s = S.set;
      const sec = [["chaves", "Chaves de acesso"], ["jogo", "Jogo e jogador"], ["java", "Java e memória"], ["pastas", "Pastas e espaço"], ["github", "GitHub"], ["aparencia", "Aparência"], ["sobre", "Sobre"]];
      const ram = 16;
      const dirs = [["grafite", "Grafite", "Escuro e sóbrio"], ["ardosia", "Ardósia", "Escuro com toque de bloco"], ["calcita", "Calcita", "Claro e limpo"]];
      return `<div class="page">
        <div class="page-head"><div class="titles"><h1 id="page-title" class="page-title" tabindex="-1">Configurações</h1><p class="page-sub">Valem para todos os modpacks.</p></div></div>
        <div class="settings">
          <nav aria-label="Seções das configurações"><ul class="nav">${sec.map(([k, t]) => `<li><a href="#/ajustes" data-action="set-jump" data-k="${k}">${t}</a></li>`).join("")}</ul></nav>
          <div>
            <section id="s-chaves" aria-labelledby="h-chaves"><h2 class="section-title" id="h-chaves">Chaves de acesso</h2>
              <p class="section-sub">Ficam guardadas no cofre de senhas do Windows, nunca em arquivos do pack ou do Warden.</p>
              <div class="card card-pad" style="margin-top:var(--space-4)">
                <div class="field"><label class="label" for="k-cf">Chave da API do CurseForge</label>
                  <div class="input-affix"><input id="k-cf" class="input mono" type="${s.showCf ? "text" : "password"}" value="${s.showCf ? "chave-de-exemplo-0000-0000-0000-7Qe" : s.cfKey}" autocomplete="off" aria-describedby="k-cf-h k-cf-s" data-fk="k-cf">
                    <button class="btn" data-action="key-show" data-k="showCf" aria-pressed="${s.showCf}" aria-label="${s.showCf ? "Ocultar" : "Mostrar"} chave do CurseForge">${icon(s.showCf ? "eyeOff" : "eye")}</button>
                    <button class="btn" data-action="key-test" data-k="cf">Testar</button></div>
                  <span class="hint" id="k-cf-h">Libera a busca completa no CurseForge.</span>
                  <span class="status ok" id="k-cf-s">${icon("ok", "sm")}Chave válida · conferida hoje, 20:58</span></div>
                <div class="field"><label class="label" for="k-gem">Chave da API do Gemini <span class="opt">(opcional)</span></label>
                  <div class="input-affix"><input id="k-gem" class="input mono" type="${s.showGem ? "text" : "password"}" placeholder="Cole sua chave aqui" value="${esc(s.gemKey)}" autocomplete="off" aria-describedby="k-gem-h" data-input="gem-in" data-fk="k-gem">
                    <button class="btn" data-action="key-show" data-k="showGem" aria-pressed="${s.showGem}" aria-label="${s.showGem ? "Ocultar" : "Mostrar"} chave do Gemini">${icon(s.showGem ? "eyeOff" : "eye")}</button>
                    <button class="btn" data-action="key-test" data-k="gem" ${s.gemKey ? "" : "disabled"}>Testar</button></div>
                  <span class="hint" id="k-gem-h">Usada só quando você pede uma análise de crash e confirma o envio. <a href="#/ajustes" data-action="soon" data-msg="Abriria a página do Google AI Studio no navegador.">Como conseguir uma chave</a></span></div>
              </div></section>
            <section id="s-jogo" aria-labelledby="h-jogo"><h2 class="section-title" id="h-jogo">Jogo e jogador</h2>
              <div class="card card-pad" style="margin-top:var(--space-4)">
                <div class="field"><label class="label" for="pl">Nome do jogador nos testes</label><input id="pl" class="input" style="max-width:280px" value="${esc(s.player)}" maxlength="16" aria-describedby="pl-h" data-input="pl-in" data-fk="pl">
                  <span class="hint" id="pl-h">De 3 a 16 letras, números ou _. Os testes usam modo offline: não precisa de conta, e servidores públicos não aceitam esse nome.</span></div>
              </div></section>
            <section id="s-java" aria-labelledby="h-java"><h2 class="section-title" id="h-java">Java e memória</h2>
              <div class="card card-pad" style="margin-top:var(--space-4)">
                <div class="set-row"><div><div class="label">Escolher o Java sozinho</div><p class="small text-2">Cada versão do Minecraft pede um Java diferente. O Warden baixa e usa o certo.</p></div><label class="switch"><input type="checkbox" role="switch" ${s.javaAuto ? "checked" : ""} aria-label="Escolher o Java sozinho"></label></div>
                <table class="table" style="margin-top:var(--space-2)"><thead><tr><th>Java</th><th>Usado para</th><th>Situação</th></tr></thead><tbody>
                  <tr><td>Java 8</td><td class="small text-2">Minecraft 1.7.10 a 1.16.5</td><td>${status("ok", "Instalado")}</td></tr>
                  <tr><td>Java 17</td><td class="small text-2">Minecraft 1.17 a 1.20.4</td><td>${status("neutral", "Baixa quando precisar", "download")}</td></tr>
                  <tr><td>Java 21</td><td class="small text-2">Minecraft 1.20.5 a 1.21.x</td><td>${status("ok", "Instalado")}</td></tr>
                  <tr><td>Java 25</td><td class="small text-2">Minecraft 26.1 em diante</td><td>${status("neutral", "Baixa quando precisar", "download")}</td></tr>
                </tbody></table>
                <hr class="hr">
                <div class="field"><label class="label" for="mem">Memória para o jogo: <span class="num">${s.mem} GB</span></label>
                  <input id="mem" type="range" min="2" max="${ram}" step="1" value="${s.mem}" data-input="mem-in" data-fk="mem" aria-describedby="mem-h" aria-valuetext="${s.mem} gigabytes">
                  <div class="meter mem-zone" aria-hidden="true"><div class="bar"><span class="zone" style="left:${((4 - 2) / (ram - 2)) * 100}%;width:${(6 / (ram - 2)) * 100}%"></span></div><div class="scale"><span>2 GB</span><span>Faixa recomendada: 4–10 GB</span><span>${ram} GB (seu computador)</span></div></div>
                  <span class="hint" id="mem-h">${s.mem > 10 ? `<span style="color:var(--c-warn-text)">${icon("warn", "sm")} Mais de 10 GB costuma deixar o jogo mais lento, não mais rápido.</span>` : s.mem < 4 ? `<span style="color:var(--c-warn-text)">${icon("warn", "sm")} Packs com muitos mods podem travar com menos de 4 GB.</span>` : "Bom para a maioria dos packs. Packs grandes (mais de 200 mods) podem pedir 8–10 GB."}</span></div>
                <details style="margin-top:var(--space-4)"><summary class="small" style="cursor:pointer">Argumentos extras do Java (avançado)</summary>
                  <div class="field" style="margin-top:var(--space-2)"><label class="sr-only" for="jvm">Argumentos extras</label><input id="jvm" class="input mono small" placeholder="-XX:+UseG1GC" aria-describedby="jvm-h"><span class="hint" id="jvm-h">Conferimos se cada argumento existe na versão do Java usada antes de abrir o jogo.</span></div></details>
              </div></section>
            <section id="s-pastas" aria-labelledby="h-pastas"><h2 class="section-title" id="h-pastas">Pastas e espaço</h2>
              <div class="card card-pad" style="margin-top:var(--space-4)">
                <div class="field"><label class="label" for="p1">Pasta dos modpacks</label><div class="input-affix"><input id="p1" class="input mono small" value="D:\\Modpacks"><button class="btn" data-action="soon">Escolher…</button></div></div>
                <div class="field"><label class="label" for="p2">Pasta de dados do Warden <span class="opt">(Java, Minecraft, instâncias de teste)</span></label><div class="input-affix"><input id="p2" class="input mono small" value="%APPDATA%\\Warden"><button class="btn" data-action="soon">Escolher…</button></div></div>
                <div class="set-row"><div><div class="label">Espaço usado pelo Warden</div><p class="small text-2">4,2 GB · downloads que podem ser baixados de novo se precisar</p></div><button class="btn" data-action="soon" data-msg="Mostraria o que pode ser apagado com segurança antes de apagar.">${icon("trash")}Liberar espaço…</button></div>
              </div></section>
            <section id="s-github" aria-labelledby="h-github"><h2 class="section-title" id="h-github">GitHub</h2>
              <div class="card card-pad" style="margin-top:var(--space-4)"><div class="set-row"><div><div class="label">${status("ok", "Conectado")}</div><p class="small text-2">Usa o login do GitHub já salvo no Windows. O Warden não guarda senha nem token próprio.</p></div><button class="btn" data-action="soon">Trocar conta</button></div></div></section>
            <section id="s-aparencia" aria-labelledby="h-aparencia"><h2 class="section-title" id="h-aparencia">Aparência</h2>
              <div class="card card-pad" style="margin-top:var(--space-4)">
                <fieldset style="border:0;padding:0;margin:0"><legend class="label" style="margin-bottom:var(--space-2)">Estilo</legend>
                  <div class="choice-grid">${dirs.map(([k, t, d]) => `<label class="choice"><input type="radio" name="dir" value="${k}" data-change="set-dir" ${S.dir === k ? "checked" : ""}><span class="swatch swatch-${k}" aria-hidden="true"><i></i><i></i><i></i></span><span class="ctitle">${t}<span class="radio-dot" aria-hidden="true"></span></span><span class="cdesc">${d}</span></label>`).join("")}</div></fieldset>
                <div class="set-row" style="margin-top:var(--space-3)"><div><div class="label">Reduzir animações</div><p class="small text-2">Tira movimentos e transições. Por padrão segue o Windows.</p></div><label class="switch"><input type="checkbox" role="switch" ${S.motion === "reduced" ? "checked" : ""} data-change="set-motion" aria-label="Reduzir animações"></label></div>
              </div></section>
            <section id="s-sobre" aria-labelledby="h-sobre"><h2 class="section-title" id="h-sobre">Sobre</h2>
              <div class="card card-pad" style="margin-top:var(--space-4)"><dl class="kv"><dt>Warden</dt><dd>0.1.0 (protótipo)</dd><dt>packwiz embutido</dt><dd class="mono small">commit 9066bf8</dd><dt>Licenças de terceiros</dt><dd><a href="#/ajustes" data-action="soon">Ver lista</a></dd></dl></div></section>
          </div>
        </div></div>`;
    },
  };
  actions["set-jump"] = (el) => {
    const t = document.getElementById("s-" + el.dataset.k);
    t.scrollIntoView({ behavior: W.reducedMotion() ? "auto" : "smooth", block: "start" });
    const h = t.querySelector("h2");
    h.setAttribute("tabindex", "-1");
    h.focus({ preventScroll: true });
  };
  actions["key-show"] = (el) => {
    S.set[el.dataset.k] = !S.set[el.dataset.k];
    render();
  };
  actions["key-test"] = (el) => {
    el.innerHTML = `<span class="spinner" aria-hidden="true"></span>Testando`;
    W.after(800, () => {
      render();
      toast(el.dataset.k === "cf" ? "Chave do CurseForge funcionando." : "Chave do Gemini funcionando.");
    });
  };
  actions["gem-in"] = (el) => {
    const had = !!S.set.gemKey;
    S.set.gemKey = el.value;
    if (had !== !!el.value) render();
  };
  actions["pl-in"] = (el) => (S.set.player = el.value);
  actions["mem-in"] = (el) => {
    S.set.mem = +el.value;
    render();
  };
  actions["set-dir"] = (el) => {
    S.dir = el.value;
    W.store.set("dir", el.value);
    document.documentElement.dataset.direction = el.value;
    render();
    document.querySelector(`input[name="dir"][value="${el.value}"]`)?.focus();
  };
  actions["set-motion"] = (el) => {
    S.motion = el.checked ? "reduced" : "auto";
    W.store.set("motion", S.motion);
    if (el.checked) document.documentElement.dataset.motion = "reduced";
    else delete document.documentElement.dataset.motion;
  };
})();
