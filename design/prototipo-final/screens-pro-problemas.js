/* Warden 1.1 "Profissional" (tarefa D5): páginas de detalhe de Problemas.
   Segurança dos mods (T28), Manutenção dos mods e substitutos (T29), Itens repetidos entre mods (T32)
   e a etapa "Verificação final" do teste com erro de segurança (T13, T28). */
(function () {
  "use strict";
  const W = window.W, D = window.DATA;
  const { def, go, B, packShell, pageHead, modsContent } = window.P;
  const BACK = { back: ["Problemas", "problemas"] };

  // Hash encurtado para os detalhes técnicos (o texto completo fica no tooltip)
  const hash = (a, b, full) => `<span class="pro-p-hash" data-tip="${full || "SHA-512 completo no arquivo de relatório"}">${a}…${b}</span>`;
  // Linha "quando foi verificado" + ação
  const metaRow = (parts, action) => `<div class="pro-p-meta"><p class="pro-p-meta__txt">${parts.join('<span aria-hidden="true"> · </span>')}</p>${action || ""}</div>`;
  const gtitle = (id, text, n, sub) => `<h2 class="group-title" id="${id}">${text} <span class="t-3">${n}${sub ? " · " + sub : ""}</span></h2>`;

  // ======================= SEGURANÇA DOS MODS (T28) =======================
  const HONEST = W.alert({ kind: "neutral", icon: "shield", title: "O Warden não é um antivírus.", text: "Ele confere se cada arquivo é igual ao oficial e procura sinais de casos conhecidos de programas maliciosos em mods. Um mod malicioso novo pode passar sem ser notado. Baixe mods do Modrinth e da CurseForge sempre que puder." });

  // Arquivo sem achado: mod, arquivo, fonte, resultado e os detalhes técnicos recolhidos
  // f: [mod, arquivo, fonte, secKind, secText, detalhes(kv html)]
  function fileRow(f) {
    const [mod, file, src, kind, text, ev] = f;
    return `<li class="pro-p-file"><span class="pro-p-file__mod">${W.tile(mod, "sm")}<span class="pro-p-file__titles"><span class="pro-p-file__name">${mod}</span><span class="path">${file}</span></span></span>
      <span class="pro-p-file__src">${W.source(src)}</span><span class="pro-p-file__res">${W.secStatus(kind, text)}</span>
      ${ev ? `<details class="disclosure pro-p-file__ev"><summary>Detalhes técnicos</summary><dl class="kv pro-p-kv">${ev}</dl></details>` : ""}</li>`;
  }
  const evOficial = (plat, a, b, when) => `<dt>Impressão digital deste arquivo</dt><dd>${hash(a, b)}</dd><dt>Arquivo oficial do ${plat}</dt><dd>${hash(a, b)} <span class="t-3">igual</span></dd><dt>Consultado</dt><dd>${when || "hoje, 14:02"}</dd>`;
  const evLocal = (a, b) => `<dt>Impressão digital</dt><dd>${hash(a, b)}</dd><dt>No Modrinth e na CurseForge</dt><dd>nenhum arquivo com esta impressão digital</dd><dt>Sinais conhecidos</dt><dd>nenhum, em 1.412 classes lidas</dd>`;
  const OFICIAIS = [
    ["Create", "create-1.20.1-0.5.1.j.jar", "modrinth", "oficial", "Confere com o arquivo oficial do Modrinth", evOficial("Modrinth", "9be1c04d", "33a07f12")],
    ["Just Enough Items (JEI)", "jei-1.20.1-forge-15.20.0.106.jar", "curseforge", "oficial", "Confere com o arquivo oficial da CurseForge", evOficial("CurseForge", "5c0de8a1", "e47b90c3")],
    ["Embeddium", "embeddium-0.3.31+mc1.20.1.jar", "modrinth", "oficial", "Confere com o arquivo oficial do Modrinth", evOficial("Modrinth", "2b7d61f0", "c88e019a")],
    ["Waystones", "waystones-forge-1.20-14.1.6.jar", "modrinth", "oficial", "Confere com o arquivo oficial do Modrinth", evOficial("Modrinth", "70fa2e93", "0b1d55c6")],
    ["Xaero's Minimap", "Xaeros_Minimap_24.6.1_Forge_1.20.jar", "curseforge", "oficial", "Confere com o arquivo oficial da CurseForge", evOficial("CurseForge", "d31a7c20", "9f6e4b18")],
  ];
  const LOCAIS = [
    ["Rubidium", "rubidium-0.7.1.jar", "local", "escaneado", null, evLocal("41c9e2b7", "a05d3f88")],
    ["Vale Sereno UI", "vale-sereno-ui-1.0.0.zip", "local", "escaneado", null, evLocal("e6b2007a", "17c4d9e1")],
    ["Ajustes do Vale", "ajustes-do-vale-1.0.jar", "local", "escaneado", null, evLocal("8d3f6a52", "c2e19b04")],
  ];

  // Achados (W.issue): o que foi visto, onde, e as ações
  const facts = (src, file, sec) => `<span class="pro-p-facts">${W.source(src)}<span class="path">${file}</span>${sec}</span>`;
  function attentionIssue() {
    return W.issue({ kind: "warn", id: "sg-att", title: "FastChest Extra tem 2 pontos de atenção",
      text: `${facts("local", "fastchest-extra.jar", W.secStatus("atencao", "2 pontos de atenção"))}<span class="pro-p-why">Este arquivo não está no Modrinth nem na CurseForge, e faz coisas que mods comuns quase nunca fazem. Não quer dizer que seja malicioso. Se você não sabe de onde ele veio, o mais seguro é remover.</span>`,
      evidence: `<dl class="kv pro-p-kv"><dt>Ponto de atenção 1</dt><dd>Carrega código de um endereço da internet<br><span class="t-mono pro-p-code">com/fastchest/extra/Loader.class</span><br><span class="t-mono pro-p-code">new URL("http://85.217.144.130/c/update.bin")</span></dd>
        <dt>Ponto de atenção 2</dt><dd>IP fixo 85.217.144.130<br><span class="t-mono pro-p-code">com/fastchest/extra/net/Sync.class</span></dd>
        <dt>Impressão digital</dt><dd>${hash("f02c9b41", "6e8a1d37")}</dd><dt>No Modrinth e na CurseForge</dt><dd>nenhum arquivo com esta impressão digital (consultado hoje, 14:02)</dd><dt>Sinais conhecidos</dt><dd>nenhum, em 38 classes lidas</dd></dl>`,
      actions: B("Remover do pack", "", { size: "sm", variant: "danger-ghost", icon: "trash-2" }) + B("Confiar neste arquivo…", "seguranca-confiar", { size: "sm", variant: "ghost", icon: "shield-user" }) });
  }
  function suspectIssue() {
    return W.issue({ kind: "danger", id: "sg-sus", open: true, title: "Better Biomes tem um sinal de programa malicioso conhecido",
      text: `${facts("local", "betterbiomes-1.2.jar", W.secStatus("suspeito"))}<span class="pro-p-why">Este arquivo tem o mesmo código que o caso <b>fractureiser</b> (2023) colocou em mods para roubar senhas, a conta do Minecraft e as sessões do Discord e do navegador. Não abra o jogo com ele. O arquivo veio do modpack Aventuras do Vale 2.1, que você trouxe em 29/09.</span>`,
      evidence: `<dl class="kv pro-p-kv"><dt>Sinal</dt><dd>Código injetado do caso fractureiser (2023), estágio 0: cria um carregador de classes por reflexão e baixa código de um endereço fixo</dd>
        <dt>Onde</dt><dd><span class="t-mono pro-p-code">net/bb/Main.class</span>, no jar embutido <span class="t-mono pro-p-code">META-INF/jars/bbcore-1.0.jar</span></dd>
        <dt>Trecho</dt><dd><span class="t-mono pro-p-code">new String(new byte[]{104,116,116,112,58,47,47,…})</span></dd>
        <dt>Impressão digital</dt><dd>${hash("7f3a0e5d", "b21cc91d")}</dd><dt>Lista de sinais</dt><dd>de 15/09/2026, veio com o Warden 1.1.0</dd></dl>`,
      actions: B("Remover do pack", "", { size: "sm", variant: "danger", icon: "trash-2" }) + B("Confiar neste arquivo…", "seguranca-confiar", { size: "sm", variant: "ghost", icon: "shield-user" }) });
  }
  function mismatchIssue() {
    return W.issue({ kind: "danger", id: "sg-nc", open: true, title: "O arquivo do Embeddium 0.3.31 não é igual ao oficial",
      text: `${facts("modrinth", "embeddium-0.3.31+mc1.20.1.jar", W.secStatus("naoconfere"))}<span class="pro-p-why">O pack diz que este é o Embeddium 0.3.31 do Modrinth, mas o arquivo é diferente do que o Modrinth distribui para essa versão. Pode ter sido alterado por alguém ou baixado de outro site. O arquivo oficial da mesma versão existe: trocar não muda nada no jogo.</span>`,
      evidence: `<dl class="kv pro-p-kv"><dt>Impressão digital deste arquivo</dt><dd>${hash("9c1e4f07", "d7a244af")}</dd><dt>Arquivo oficial do Modrinth</dt><dd>${hash("2b7d61f0", "c88e019a")} <span class="t-danger">diferente</span></dd>
        <dt>O que o pack diz</dt><dd>Embeddium 0.3.31 para Forge 1.20.1, em <span class="path">mods/embeddium.pw.toml</span></dd><dt>Consultado</dt><dd>hoje, 14:02, no Modrinth</dd></dl>`,
      actions: B("Trocar pelo arquivo oficial", "", { size: "sm", variant: "primary", icon: "replace" }) + B("Remover do pack", "", { size: "sm", variant: "danger-ghost", icon: "trash-2" }) + B("Confiar neste arquivo…", "seguranca-confiar", { size: "sm", variant: "ghost", icon: "shield-user" }) });
  }
  function trustedRow() {
    return `<li class="pro-p-file pro-p-file--trusted"><span class="pro-p-file__mod">${W.tile("Better Biomes", "sm")}<span class="pro-p-file__titles"><span class="pro-p-file__name"><s>Better Biomes</s></span><span class="path">betterbiomes-1.2.jar</span></span></span>
      <span class="pro-p-file__src">${W.source("local")}</span><span class="pro-p-file__res">${W.secStatus("confiado")}</span>
      <p class="pro-p-file__note">Você confiou hoje, às 14:31. Vale só para este arquivo exato: um arquivo novo, mesmo do mesmo mod, volta a ser conferido. ${B("Desfazer confiança", "seguranca~suspeito", { variant: "link" })}</p></li>`;
  }

  // Grupo de arquivos sem achado; ok = escondido pelo filtro "Só os que pedem atenção"
  function fileGroup(id, title, n, rows, o = {}) {
    return `<section aria-labelledby="${id}"${o.ok ? " data-ok-group hidden" : ""}>${gtitle(id, title, n, o.sub)}<ul class="pro-p-files">${rows.map(fileRow).join("")}${o.more ? `<li class="pro-p-file pro-p-file--more">${o.more}</li>` : ""}</ul></section>`;
  }
  function filterBar(hiddenText) {
    return `<div class="pro-p-filter">${W.check({ id: "sg-filter", label: "Só os que pedem atenção", checked: true })}<p class="t-sm t-3" data-filter-note>${hiddenText}</p></div>`;
  }

  function securityContent(state) {
    const verify = B("Verificar agora", "seguranca~carregando", { size: "sm", icon: "refresh-cw" });
    const head = pageHead("Segurança dos mods", "Confere se os arquivos dos mods são os oficiais e se têm sinais de programas maliciosos já conhecidos. Nada é executado: os arquivos só são lidos.", null, BACK);
    const lista = state === "sinais" ? "Lista de sinais de 12/03/2026" : "Lista de sinais de 15/09/2026, veio com o Warden 1.1.0";
    const meta = (when) => metaRow([when || "Última verificação: hoje, 14:02", lista], verify);
    if (state === "vazio") return head + HONEST + W.empty({ title: "Nada para conferir ainda.", text: "O pack não tem mods. Quando você adicionar um, o Warden confere o arquivo na hora.", actions: B("Adicionar mods", "adicionar", { variant: "primary", icon: "plus" }) });
    if (state === "carregando") return head + HONEST + metaRow(["Verificando agora", lista], W.btn("Verificando…", { size: "sm", busy: true, disabled: true })) +
      `<div class="panel pro-p-narrow">${W.progress({ label: "Conferindo 54 de 128 arquivos", value: 42, valueText: "54 de 128 arquivos", meta: "Só a impressão digital de cada arquivo vai para o Modrinth e a CurseForge, nunca o arquivo. Os arquivos que não mudaram desde a última vez não são lidos de novo." })}</div>`;

    const moreOk = (n) => `… e mais ${n} que conferem com o arquivo oficial. ${B("Mostrar todos", "", { variant: "link" })}`;
    const okGroups = (nOf, nLoc, o = {}) => fileGroup("sg-of", "Confere com o arquivo oficial", nOf, OFICIAIS, { ok: true, more: moreOk(nOf - OFICIAIS.length) }) +
      fileGroup("sg-lo", "Arquivo do computador: nenhum sinal conhecido", nLoc, o.locals || LOCAIS, { ok: true, sub: "não estão em nenhuma plataforma" });

    if (state === "erro" || state === "semchave") {
      const net = state === "erro";
      const alertBox = net
        ? W.alert({ kind: "warn", title: "Sem internet: não deu para conferir com o Modrinth e a CurseForge.", text: "A busca de sinais conhecidos rodou nos 128 arquivos e não achou nenhum. A conferência com o arquivo oficial fica para quando a conexão voltar.", actions: B("Tentar de novo", "seguranca~carregando", { size: "sm", icon: "refresh-cw" }) })
        : W.alert({ kind: "warn", icon: "key-round", title: "Sem a chave da CurseForge, os 38 itens da CurseForge não foram conferidos.", text: "A busca de sinais conhecidos rodou neles e não achou nenhum. Os itens do Modrinth foram conferidos normalmente.", actions: B("Abrir Configurações", "config-app", { size: "sm", icon: "settings" }) });
      const pend = (net ? OFICIAIS : OFICIAIS.filter((f) => f[2] === "curseforge")).map((f) => [f[0], f[1], f[2], "semconferir", null, `<dt>Impressão digital</dt><dd>${hash("5c0de8a1", "e47b90c3")}</dd><dt>Sinais conhecidos</dt><dd>nenhum</dd><dt>Conferência</dt><dd>${net ? "o Modrinth e a CurseForge não responderam (sem conexão)" : "a CurseForge precisa de uma chave, que fica nas Configurações"}</dd>`]);
      const nPend = net ? 124 : 38;
      const pendGroup = `<section aria-labelledby="sg-pd">${gtitle("sg-pd", "Não deu para conferir", nPend, net ? "sem internet" : "itens da CurseForge")}<ul class="pro-p-files">${pend.map(fileRow).join("")}<li class="pro-p-file pro-p-file--more">… e mais ${nPend - pend.length}. ${B("Mostrar todos", "", { variant: "link" })}</li></ul></section>`;
      const okRest = net ? fileGroup("sg-lo", "Arquivo do computador: nenhum sinal conhecido", 3, LOCAIS, { ok: true, sub: "não estão em nenhuma plataforma" })
        : fileGroup("sg-of", "Confere com o arquivo oficial", 86, OFICIAIS.filter((f) => f[2] === "modrinth"), { ok: true, more: moreOk(86 - 3) }) + fileGroup("sg-lo", "Arquivo do computador: nenhum sinal conhecido", 3, LOCAIS, { ok: true, sub: "não estão em nenhuma plataforma" });
      return head + `<div class="stack-3">${HONEST}${meta(net ? "Última verificação: agora, sem internet" : null)}${alertBox}</div>` +
        filterBar(net ? "Escondidos pelo filtro: 3 arquivos do computador sem sinal conhecido." : "Escondidos pelo filtro: 86 que conferem com o arquivo oficial e 3 arquivos do computador sem sinal conhecido.") +
        `<section aria-labelledby="sg-at">${gtitle("sg-at", "Pontos de atenção", 1, "arquivos fora das plataformas")}${attentionIssue()}</section>` + pendGroup + okRest;
    }

    let top = "", groups = "", hidden = "Escondidos pelo filtro: 124 que conferem com o arquivo oficial e 3 arquivos do computador sem sinal conhecido.";
    const att = `<section aria-labelledby="sg-at">${gtitle("sg-at", "Pontos de atenção", 1, "arquivos fora das plataformas")}${attentionIssue()}</section>`;
    if (state === "suspeito") {
      top = W.alert({ kind: "danger", title: "1 arquivo tem um sinal de programa malicioso conhecido.", text: "O teste e a publicação ficam bloqueados até você remover o arquivo ou confiar nele." });
      groups = `<section aria-labelledby="sg-su">${gtitle("sg-su", "Sinal de programa malicioso conhecido", 1)}${suspectIssue()}</section>` + att +
        okGroups(124, 2, { locals: LOCAIS.slice(1) });
      hidden = "Escondidos pelo filtro: 124 que conferem com o arquivo oficial e 2 arquivos do computador sem sinal conhecido.";
    } else if (state === "naoconfere") {
      top = W.alert({ kind: "danger", title: "1 arquivo não confere com o arquivo oficial.", text: "O teste e a publicação ficam bloqueados até você trocar, remover ou confiar no arquivo." });
      groups = `<section aria-labelledby="sg-nc-h">${gtitle("sg-nc-h", "Não confere com o arquivo oficial", 1)}${mismatchIssue()}</section>` + att +
        fileGroup("sg-of", "Confere com o arquivo oficial", 123, OFICIAIS.filter((f) => f[0] !== "Embeddium"), { ok: true, more: moreOk(123 - 4) }) + fileGroup("sg-lo", "Arquivo do computador: nenhum sinal conhecido", 3, LOCAIS, { ok: true, sub: "não estão em nenhuma plataforma" });
      hidden = "Escondidos pelo filtro: 123 que conferem com o arquivo oficial e 3 arquivos do computador sem sinal conhecido.";
    } else if (state === "confiado") {
      top = W.alert({ kind: "ok", compact: true, title: "Você confiou no arquivo betterbiomes-1.2.jar.", text: "O teste e a publicação seguem. A confiança fica guardada no pack, em .warden/trust.toml (não vai para os jogadores)." });
      groups = att + `<section aria-labelledby="sg-tr">${gtitle("sg-tr", "Você confiou neste arquivo", 1)}<ul class="pro-p-files">${trustedRow()}</ul></section>` + okGroups(124, 2, { locals: LOCAIS.slice(1) });
      hidden = "Escondidos pelo filtro: 124 que conferem com o arquivo oficial e 2 arquivos do computador sem sinal conhecido.";
    } else {
      if (state === "sinais") top = W.alert({ kind: "info", compact: true, icon: "clock", title: "A lista de sinais tem mais de 6 meses. Atualize o Warden quando puder.", text: "Cada versão do Warden traz a lista mais nova. A conferência com o arquivo oficial não depende dela e continua em dia." });
      groups = att + okGroups(124, 3);
    }
    const summary = `<p class="pro-p-sum">${state === "suspeito" ? "128 arquivos: 1 com sinal de programa malicioso conhecido, 1 com pontos de atenção, 124 conferem com o oficial e 2 do computador sem sinal conhecido."
      : state === "naoconfere" ? "128 arquivos: 1 não confere com o oficial, 1 com pontos de atenção, 123 conferem com o oficial e 3 do computador sem sinal conhecido."
      : state === "confiado" ? "128 arquivos: 1 com pontos de atenção, 1 em que você confiou, 124 conferem com o oficial e 2 do computador sem sinal conhecido."
      : "128 arquivos: 124 conferem com o arquivo oficial do Modrinth ou da CurseForge, 3 são arquivos do computador sem sinal conhecido e 1 tem pontos de atenção."}</p>`;
    return head + `<div class="stack-3">${HONEST}${meta()}${top}${summary}</div>` + filterBar(hidden) + groups;
  }
  // Filtro "Só os que pedem atenção": mostra ou esconde os grupos sem achado
  function filterAfter() {
    const cb = document.getElementById("sg-filter");
    if (!cb) return;
    const apply = () => {
      document.querySelectorAll("[data-ok-group]").forEach((g) => { g.hidden = cb.checked; });
      const note = document.querySelector("[data-filter-note]");
      if (note) note.hidden = !cb.checked;
    };
    cb.addEventListener("change", apply);
    apply();
  }
  const secShell = (s, o = {}) => packShell("problemas", securityContent(s), Object.assign(s === "vazio" ? { problems: 0, empty: true } : s === "suspeito" || s === "naoconfere" ? { problems: 5 } : {}, o));
  def("seguranca", { group: "pack", title: "Segurança dos mods", spec: "T28", v11: true, render: (s) => secShell(s), after: filterAfter,
    states: { carregando: "Conferindo 54 de 128 arquivos", vazio: "Pack sem mods", erro: "Sem internet: só a busca de sinais rodou", semchave: "Sem a chave da CurseForge", suspeito: "Um arquivo com sinal de programa malicioso conhecido", naoconfere: "Um arquivo que não confere com o oficial", sinais: "Lista de sinais com mais de 6 meses", confiado: "Depois de confiar num arquivo" } });

  // ---------- Confiar neste arquivo (diálogo com o nome digitado) ----------
  const TRUST_NAME = "Better Biomes";
  function trustDialog() {
    return W.dialog({ id: "sg-trust", esc: "seguranca~suspeito", alert: true, title: "Confiar neste arquivo?", sub: "Se este arquivo tiver um programa malicioso, ele roda no seu computador quando o jogo abrir e no computador de quem jogar o pack.",
      body: `<dl class="kv"><dt>Arquivo</dt><dd><span class="path">betterbiomes-1.2.jar</span></dd><dt>Resultado</dt><dd>${W.secStatus("suspeito")}</dd><dt>Sinal</dt><dd>Código injetado do caso fractureiser (2023)</dd><dt>Impressão digital</dt><dd>${hash("7f3a0e5d", "b21cc91d")}</dd></dl>
        <p class="t-sm t-2">Confie só se você sabe de onde o arquivo veio e tem certeza de que o aviso está errado. A confiança vale só para este arquivo exato e fica guardada no pack (em <span class="t-mono pro-p-hash">.warden/trust.toml</span>, que não vai para os jogadores). Se o arquivo mudar, mesmo numa versão nova do mesmo mod, o Warden confere de novo.</p>
        ${W.input({ id: "sg-trust-name", label: `Para confirmar, digite o nome do mod: <b>${TRUST_NAME}</b>`, hint: "O botão Confiar neste arquivo é liberado quando o nome estiver igual." })}`,
      foot: B("Cancelar", "seguranca~suspeito", { variant: "ghost" }) + B("Confiar neste arquivo", "seguranca~confiado", { variant: "danger", icon: "shield-user", disabled: true, attrs: { id: "sg-trust-ok" } }) });
  }
  def("seguranca-confiar", { group: "pack", title: "Confiar neste arquivo (confirmação)", spec: "T28", v11: true, hidden: true, render: () => secShell("suspeito", { overlay: trustDialog() }),
    after: () => {
      filterAfter();
      const inp = document.getElementById("sg-trust-name"), ok = document.getElementById("sg-trust-ok");
      if (inp && ok) inp.addEventListener("input", () => { ok.disabled = inp.value.trim().toLowerCase() !== TRUST_NAME.toLowerCase(); });
    } });

  // ---------- Testar: Verificação final com erro de segurança (T13, T28) ----------
  const STEPS = ["Verificar o pack", "Preparar o Minecraft", "Copiar o pack para o teste", "Verificação final", "Abrir o jogo"];
  function testSecContent() {
    return pageHead("Testando o pack", "Etapa 4 de 5 · começou às 14:18", B("Cancelar teste", "mods", { variant: "ghost", icon: "x" })) +
      `<div class="test-steps">${W.steps(STEPS, 3, { label: "Etapas do teste", error: 3 })}</div>
      <div class="panel pro-p-narrow">${W.progress({ label: "Segurança dos arquivos da instância de teste", value: 100, state: "error", showValue: false, meta: "12 arquivos novos ou alterados foram lidos. Os outros 116 já estavam conferidos e não mudaram." })}</div>`;
  }
  function testSecDialog() {
    return W.dialog({ id: "ts-sec", esc: "mods", alert: true, title: "2 arquivos não passaram na verificação de segurança", sub: "O jogo não abre com eles. Um arquivo assim pode rodar um programa no seu computador assim que o jogo começar.", body: `
      ${W.issue({ kind: "danger", id: "ts-1", title: "Better Biomes: sinal de programa malicioso conhecido", text: `${facts("local", "betterbiomes-1.2.jar", "")}<span class="pro-p-why">O mesmo código do caso fractureiser (2023). Não existe arquivo oficial para trocar.</span>`, actions: B("Remover do pack", "", { size: "sm", variant: "danger", icon: "trash-2" }) + B("Confiar neste arquivo…", "seguranca-confiar", { size: "sm", variant: "ghost", icon: "shield-user" }) })}
      ${W.issue({ kind: "danger", id: "ts-2", title: "Embeddium 0.3.31: não confere com o arquivo oficial", text: `${facts("modrinth", "embeddium-0.3.31+mc1.20.1.jar", "")}<span class="pro-p-why">O arquivo oficial da mesma versão existe no Modrinth.</span>`, actions: B("Trocar pelo arquivo oficial", "", { size: "sm", variant: "primary", icon: "replace" }) + B("Remover do pack", "", { size: "sm", variant: "danger-ghost", icon: "trash-2" }) })}
      <p class="t-sm t-3">Por segurança, aqui não existe a opção de testar mesmo assim. Depois de resolver, clique em Testar de novo.</p>`,
      foot: B("Cancelar teste", "mods", { variant: "ghost" }) + B("Ver detalhes", "seguranca~suspeito", { icon: "shield" }) });
  }
  def("teste-seguranca", { group: "teste", title: "Verificação final com erro de segurança", spec: "T13, T28", v11: true, render: () => packShell("teste", testSecContent(), { game: "preparing", progress: 86, prepTarget: "teste-seguranca", overlay: testSecDialog() }) });

  // ======================= MANUTENÇÃO DOS MODS (T29) =======================
  // Linha de manutenção: mod, situação, último arquivo, evidência e ações
  function maintRow(o) {
    return `<li class="pro-p-file pro-p-file--${o.tone || "plain"}"><span class="pro-p-file__mod">${W.tile(o.mod, "sm")}<span class="pro-p-file__titles"><span class="pro-p-file__name">${o.mod} <span class="t-mono t-3 pro-p-ver">${o.ver}</span></span><span class="pro-p-file__sub">${o.last}</span></span></span>
      <span class="pro-p-file__src">${W.source(o.src)}</span><span class="pro-p-file__res">${o.status}</span>
      ${o.text ? `<p class="pro-p-file__note">${o.text}</p>` : ""}
      ${o.ev ? `<details class="disclosure pro-p-file__ev"><summary>Como sabemos</summary><p class="t-sm t-2">${o.ev}</p></details>` : ""}
      ${o.actions ? `<div class="pro-p-file__acts">${o.actions}</div>` : ""}</li>`;
  }
  const subst = (target) => B("Procurar substituto", target, { size: "sm", icon: "search" });
  const rmv = B("Remover do pack", "", { size: "sm", variant: "danger-ghost", icon: "trash-2" });
  const ignore = B("Ignorar neste pack", "", { size: "sm", variant: "ghost" }) + W.badge("p1", "P1");
  const STALE = [
    { mod: "Chunk Animator", ver: "1.20.1-1.3.7", src: "modrinth", last: "Último arquivo: 14/01/2025", status: W.maintTag("stale", "Sem atualização há 1 ano e 8 meses"), ev: "O Modrinth informa que a versão mais recente do projeto é de 14/01/2025, para qualquer Minecraft. Consultado hoje, às 14:02." },
    { mod: "Default Options", ver: "18.0.1", src: "modrinth", last: "Último arquivo: 20/09/2024", status: W.maintTag("stale", "Sem atualização há 2 anos"), ev: "O Modrinth informa que a versão mais recente do projeto é de 20/09/2024. Consultado hoje, às 14:02." },
    { mod: "Item Borders", ver: "1.2.2", src: "curseforge", last: "Último arquivo: 03/02/2025", status: W.maintTag("stale", "Sem atualização há 1 ano e 8 meses"), ev: "A CurseForge informa que o arquivo mais recente do projeto é de 03/02/2025. Consultado hoje, às 14:02." },
  ];
  const NONEWER = [["Create", "1.21.1"], ["Epic Fight", "1.20.1"], ["Waystones", "1.21.1"], ["Farmer's Delight", "1.21.1"], ["Supplementaries", "1.21.1"], ["Xaero's World Map", "1.21.8"]];
  function nonewerGroup() {
    return `<section aria-labelledby="mt-nn"><h2 class="group-title" id="mt-nn">Sem versão para o Minecraft mais novo <span class="t-3">14 · informação, não tira pontos</span></h2>
      <details class="disclosure pro-p-collapsed"><summary>Ver os 14 mods</summary><p class="t-sm t-2">A versão mais nova do Minecraft é a 1.21.9. Estes mods ainda não têm versão para ela. Serve para planejar uma mudança de versão do pack; para o pack de agora, nada muda.</p>
        <ul class="pro-p-mini">${NONEWER.map(([m, v]) => `<li><span class="t-strong">${m}</span>${W.maintTag("nonewer", `Vai até o ${v}`)}</li>`).join("")}<li class="t-3">… e mais 8 mods</li></ul></details></section>`;
  }
  function maintContent(state) {
    const verify = B("Verificar agora", "manutencao~carregando", { size: "sm", icon: "refresh-cw" });
    const head = pageHead("Manutenção dos mods", "Mods removidos, arquivados ou que pararam de ser atualizados no Modrinth e na CurseForge.", null, BACK);
    const support = `<p class="pro-p-sum">Um mod sem atualização não está necessariamente quebrado. Muitos mods antigos continuam funcionando.</p>`;
    const localNote = `<p class="t-xs t-3 pro-p-foot">Os 4 arquivos do computador não entram aqui: manutenção não se aplica a eles. A lista do Warden de mods obsoletos continua valendo em Problemas (por exemplo, o Rubidium).</p>`;
    if (state === "carregando") return head + metaRow(["Verificando agora · Modrinth e CurseForge"], W.btn("Verificando…", { size: "sm", busy: true, disabled: true })) +
      `<div class="panel pro-p-narrow">${W.progress({ label: "Consultando o Modrinth e a CurseForge sobre 124 mods", value: null, meta: "Poucas consultas, em lotes. Os arquivos não saem do computador." })}</div>`;
    if (state === "vazio") return head + metaRow(["Verificado ao abrir o pack, hoje às 14:02 · Modrinth e CurseForge"], verify) +
      W.empty({ kind: "ok", glyph: "check", title: "Nenhum mod com problema de manutenção.", text: "Os 124 mods continuam nas plataformas e foram atualizados nos últimos 18 meses." }) + localNote;

    const removed = maintRow({ tone: "danger", mod: "Xaero's Minimap", ver: "24.6.1", src: "curseforge", last: "Último arquivo do projeto: 24.6.2, de 18/09/2026", status: W.maintTag("filegone", "Arquivo removido da CurseForge"),
      text: `<b class="t-danger">Os jogadores não vão conseguir baixar este mod.</b> O arquivo da versão 24.6.1 saiu da CurseForge. O projeto continua no ar, com uma versão mais nova.`,
      ev: "A CurseForge não devolveu o arquivo Xaeros_Minimap_24.6.1_Forge_1.20.jar no lote de arquivos pedido hoje, às 14:02. O projeto respondeu normalmente, com a versão 24.6.2 como a mais nova para Forge 1.20.1.",
      actions: B("Atualizar para 24.6.2", "atualizar", { size: "sm", variant: "primary", icon: "refresh-cw" }) + subst("substitutos") + rmv });
    const archived = maintRow({ tone: "warn", mod: "Clumps", ver: "12.0.0.4", src: "curseforge", last: "Último arquivo: 05/03/2024", status: W.maintTag("archived"),
      text: "O autor parou de publicar versões novas. O arquivo do pack continua disponível para baixar.",
      ev: "A CurseForge informa o projeto Clumps como inativo. Consultado hoje, às 14:02.", actions: subst("") + rmv });
    const staleGroup = (rows, sub) => `<section aria-labelledby="mt-st">${gtitle("mt-st", "Sem atualização há mais de 18 meses", rows.length, sub || "informação")}<ul class="pro-p-files">${rows.map((r) => maintRow(Object.assign({ actions: subst("") + ignore }, r))).join("")}</ul></section>`;

    if (state === "erro" || state === "semchave") {
      const net = state === "erro";
      const box = net
        ? W.alert({ kind: "warn", title: "Sem internet. Mostrando o resultado do Modrinth de ontem, às 21:40.", text: "Os resultados da CurseForge não podem ser guardados de uma vez para outra, então os 38 mods da CurseForge ficam sem verificação agora.", actions: B("Tentar de novo", "manutencao~carregando", { size: "sm", icon: "refresh-cw" }) })
        : W.alert({ kind: "warn", icon: "key-round", title: "Sem a chave da CurseForge, os 38 mods da CurseForge não foram verificados.", text: "Os mods do Modrinth foram verificados normalmente.", actions: B("Abrir Configurações", "config-app", { size: "sm", icon: "settings" }) });
      const cfRows = [["Xaero's Minimap", "24.6.1"], ["Just Enough Items (JEI)", "15.20.0.106"], ["Clumps", "12.0.0.4"], ["Epic Fight", "20.9.4"]];
      const cf = `<section aria-labelledby="mt-cf">${gtitle("mt-cf", net ? "Não verificado agora" : "Não verificado", 38, "mods da CurseForge")}<ul class="pro-p-files">${cfRows.map(([m, v]) => maintRow({ mod: m, ver: v, src: "curseforge", last: net ? "Sem conexão com a CurseForge" : "Falta a chave da CurseForge", status: W.status("muted", net ? "Não verificado agora" : "Não verificado") })).join("")}<li class="pro-p-file pro-p-file--more">… e mais 34 mods da CurseForge.</li></ul></section>`;
      return head + metaRow([net ? "Modrinth: verificado ontem, às 21:40" : "Verificado ao abrir o pack, hoje às 14:02", net ? "CurseForge: não verificado agora" : "CurseForge: sem chave"], verify) + `<div class="stack-3">${box}${support}</div>` +
        staleGroup(STALE.filter((r) => r.src === "modrinth"), net ? "informação · resultado do Modrinth de ontem" : "informação") + cf + nonewerGroup() + localNote;
    }
    return head + metaRow(["Verificado ao abrir o pack, hoje às 14:02 · Modrinth e CurseForge"], verify) + support +
      (state === "removido" ? `<section aria-labelledby="mt-rm">${gtitle("mt-rm", "Arquivo removido", 1, "erro: impede publicar")}<ul class="pro-p-files">${removed}</ul></section>` : "") +
      `<section aria-labelledby="mt-ar">${gtitle("mt-ar", "Arquivado pelo autor", 1, "aviso")}<ul class="pro-p-files">${archived}</ul></section>` +
      staleGroup(STALE) + nonewerGroup() + localNote;
  }
  def("manutencao", { group: "pack", title: "Manutenção dos mods", spec: "T29", v11: true, render: (s) => packShell("problemas", maintContent(s)),
    states: { carregando: "Consultando as plataformas", vazio: "Tudo em dia", erro: "Sem internet: resultado do Modrinth de ontem", semchave: "Sem a chave da CurseForge", removido: "Um arquivo removido da plataforma (impede publicar)" } });

  // ---------- Substitutos (painel de detalhes do mod, "← Detalhes") ----------
  const CANDIDATES = [
    { name: "JourneyMap", author: "techbrew", src: "curseforge", downloads: "128 mi", updated: "há 9 dias", reasons: ["mesma categoria na CurseForge: Mapa e informação", "tem versão para Forge 1.20.1", "minimapa e mapa do mundo num mod só", "o mais baixado da categoria para Forge"] },
    { name: "FTB Chunks", author: "FTB", src: "curseforge", downloads: "52 mi", updated: "há 1 mês", reasons: ["mesma categoria na CurseForge: Mapa e informação", "tem versão para Forge 1.20.1", "precisa do FTB Library e do Architectury, que entram junto ao trocar"] },
    { name: "Map Atlases", author: "Pepperoni_Jabroni", src: "curseforge", downloads: "9 mi", updated: "há 2 meses", reasons: ["mesma categoria na CurseForge: Mapa e informação", "tem versão para Forge 1.20.1", "minimapa feito com os mapas do próprio jogo"] },
    { name: "VoxelMap-Updated", author: "fantahund", src: "curseforge", downloads: "6 mi", updated: "há 3 semanas", reasons: ["mesma categoria na CurseForge: Mapa e informação", "tem versão para Forge 1.20.1", "funciona só no cliente, como o Xaero's Minimap"] },
  ];
  function substDrawer(state) {
    const back = `<div class="back-link">${B("Detalhes", "mods-detalhe", { variant: "ghost", size: "sm", icon: "arrow-left" })}</div>`;
    const cur = `<div class="pro-p-cur">${W.tile("Xaero's Minimap", "lg")}<div><div class="t-strong">Xaero's Minimap <span class="t-mono t-3 pro-p-ver">24.6.1</span></div><div class="row row--wrap pro-p-cur__tags">${W.source("curseforge")}${W.maintTag("filegone", "Arquivo removido")}</div></div></div>`;
    const intro = `<p class="t-sm t-2">Mods da CurseForge parecidos com o Xaero's Minimap, com versão para Forge 1.20.1. Sem os arquivados e sem os que já estão no pack, do mais baixado para o menos.</p>`;
    const fixed = `<p class="t-xs t-3">Os substitutos são parecidos pela categoria; confira se fazem o que você precisa.</p>`;
    let list;
    if (state === "carregando") list = `<div aria-busy="true"><p class="sr-only" role="status">Buscando substitutos na CurseForge…</p><ul class="replist">${W.replacementRow({ state: "loading" }).repeat(3)}</ul></div>`;
    else if (state === "vazio") list = W.empty({ compact: true, glyph: "search", title: "Nenhum mod parecido com versão para Forge 1.20.1 na CurseForge.", text: "Você pode buscar outro mod de mapa na página de descoberta, no Modrinth e na CurseForge.", actions: B("Abrir a descoberta", "adicionar", { size: "sm", icon: "search" }) });
    else if (state === "erro") list = W.alert({ kind: "danger", title: "Não deu para buscar na CurseForge.", text: "Sem conexão. Os substitutos aparecem quando a conexão voltar.", actions: B("Tentar de novo", "substitutos~carregando", { size: "sm", icon: "refresh-cw" }) });
    else list = `<ul class="replist">${CANDIDATES.map((c, i) => W.replacementRow(Object.assign({ id: "rep-" + i, detailAttrs: go("adicionar-detalhe"), swapAttrs: go("dependencias") }, c))).join("")}</ul>`;
    return W.drawer({ id: "sb", esc: "mods", title: "Substitutos para Xaero's Minimap", body: `${back}${cur}${intro}${list}${state === "normal" || state === "carregando" ? fixed : ""}
      <p class="t-xs t-3">Trocar por este adiciona o mod novo, com as dependências, e remove o Xaero's Minimap de uma vez só. Antes, o Warden guarda um ponto de segurança.</p>` });
  }
  def("substitutos", { group: "pack", title: "Detalhes: procurar substituto", spec: "T29", v11: true, render: (s) => packShell("mods", modsContent("normal"), { overlay: substDrawer(s) }),
    states: { carregando: "Buscando candidatos", vazio: "Nenhum candidato", erro: "Sem conexão com a CurseForge" } });

  // ======================= ITENS REPETIDOS ENTRE MODS (T32) =======================
  const ADD_AU = go("adicionar-detalhe");
  const MATERIALS = [
    { material: "Chumbo", summary: "3 mods têm o próprio lingote · 2 geram minério", open: true, mods: [{ mod: "Mekanism", id: "mekanism:ingot_lead", ore: true }, { mod: "Thermal Foundation", id: "thermal:lead_ingot", ore: true }, { mod: "Immersive Engineering", id: "immersiveengineering:ingot_lead" }], evidence: "tag <span class=\"t-mono\">forge:ingots/lead</span> nos jars de 3 mods; minério em <span class=\"t-mono\">worldgen/configured_feature</span> do Mekanism e do Thermal · confiança: certa", text: "o AlmostUnified faz as receitas usarem um lingote só. Nesta versão, os 2 minérios continuam aparecendo: desligue um deles na config do mod." },
    { material: "Níquel", summary: "2 mods têm o próprio lingote · 2 geram minério", mods: [{ mod: "Thermal Foundation", id: "thermal:nickel_ingot", ore: true }, { mod: "Immersive Engineering", id: "immersiveengineering:ingot_nickel", ore: true }], evidence: "tag <span class=\"t-mono\">forge:ingots/nickel</span> nos jars de 2 mods; minério nos dois · confiança: certa", text: "o AlmostUnified faz as receitas usarem um lingote só. Nesta versão, os 2 minérios continuam aparecendo: desligue um deles na config do mod." },
    { material: "Estanho", state: "items", summary: "2 mods têm o próprio lingote · só 1 gera minério", mods: [{ mod: "Mekanism", id: "mekanism:ingot_tin", ore: true }, { mod: "Thermal Foundation", id: "thermal:tin_ingot" }], evidence: "tag <span class=\"t-mono\">forge:ingots/tin</span> nos jars de 2 mods · confiança: certa", text: "o AlmostUnified faz as receitas usarem um lingote só e esconde o outro no JEI." },
    { material: "Prata", state: "items", summary: "2 mods têm o próprio lingote · só 1 gera minério", mods: [{ mod: "Thermal Foundation", id: "thermal:silver_ingot" }, { mod: "Immersive Engineering", id: "immersiveengineering:ingot_silver", ore: true }], evidence: "tag <span class=\"t-mono\">forge:ingots/silver</span> nos jars de 2 mods · confiança: certa", text: "o AlmostUnified faz as receitas usarem um lingote só e esconde o outro no JEI." },
  ];
  // Pack antigo (1.12.2): leitura aproximada, pelo dicionário de minérios
  const LEGACY_PACK = { name: "Sky Factory do Zero", mc: "1.12.2", loader: "Forge 14.23.5.2860", loaderShort: "Forge", version: "1.0.0", unsaved: 0 };
  const LEGACY = [
    { material: "Cobre", summary: "3 mods têm o próprio lingote · leitura provável", mods: [{ mod: "Thermal Foundation", id: "thermalfoundation:material:128" }, { mod: "Mekanism", id: "mekanism:ingot:5" }, { mod: "IndustrialCraft 2", id: "ic2:ingot:2" }], evidence: "nome <span class=\"t-mono\">ingotCopper</span> nos arquivos de idioma de 3 mods · confiança: provável" },
    { material: "Estanho", summary: "3 mods têm o próprio lingote · leitura provável", mods: [{ mod: "Thermal Foundation", id: "thermalfoundation:material:129" }, { mod: "Mekanism", id: "mekanism:ingot:6" }, { mod: "IndustrialCraft 2", id: "ic2:ingot:6" }], evidence: "nome <span class=\"t-mono\">ingotTin</span> nos arquivos de idioma de 3 mods · confiança: provável" },
  ];

  function dupContent(state) {
    const verify = B("Verificar agora", "repetidos~carregando", { size: "sm", icon: "refresh-cw" });
    const legacy = state === "antiga";
    const head = pageHead("Itens repetidos entre mods", "Materiais que mais de um mod adiciona. É um conselho: nada vai travar por causa disso.", null, BACK);
    const why = `<p class="pro-p-sum">Com itens repetidos, o jogador acaba com lingotes que parecem iguais mas não empilham, e com veios de minério demais no mundo. Minério em dobro é difícil de desfazer depois que o mundo foi gerado.</p>`;
    if (state === "carregando") return head + metaRow(["Lendo os arquivos dos mods"], W.btn("Verificando…", { size: "sm", busy: true, disabled: true })) +
      `<div class="panel pro-p-narrow">${W.progress({ label: "Lendo as tags de material de 124 mods", value: 61, meta: "Só os arquivos dos mods são lidos. O jogo não é aberto." })}</div>`;
    if (state === "semdados") return head + why + W.empty({ glyph: "search", title: "Ainda não verificado.", text: "A verificação lê os arquivos dos mods. Faça um teste ou clique em Verificar agora.", actions: B("Verificar agora", "repetidos~carregando", { variant: "primary", icon: "refresh-cw" }) + B("Testar", "teste-checagem", { icon: "play" }) });
    if (state === "vazio") return head + metaRow(["Lido dos arquivos dos mods no último teste, hoje às 14:20"], verify) +
      W.empty({ kind: "ok", glyph: "check", title: "Nenhum material repetido entre os mods do pack.", text: "Lingotes, pepitas, pós, placas, minérios e blocos de material foram conferidos nos 124 mods." });

    let solution, list, health;
    if (legacy) {
      solution = `<section class="panel pro-p-sol" aria-labelledby="dp-sol"><h2 class="panel__title panel__title--sans" id="dp-sol">Solução para Forge 1.12.2</h2>
        <p class="t-sm t-2"><b>UniDict</b> faz as receitas e as máquinas usarem um item só, pelo dicionário de minérios.</p>
        <p class="t-sm t-warn pro-p-caveat">${W.icon("triangle-alert", "icon--sm")}<span>Está sem atualização desde 2021 e não mexe na geração de minério.</span></p>
        <div class="row row--wrap">${B("Adicionar UniDict", "adicionar-detalhe", { size: "sm", variant: "primary", icon: "plus" })}</div></section>`;
      list = LEGACY.map((m, i) => W.dupMaterial(Object.assign({ id: "dl-" + i, state: "legacy", mc: "Minecraft 1.12.2", solution: { name: "UniDict", text: "o UniDict faz o mesmo papel nesta versão.", attrs: ADD_AU } }, m))).join("");
      health = "Na saúde do pack: nada, porque nenhum destes materiais tem minério gerado por 2 ou mais mods.";
    } else if (state === "unificado") {
      solution = W.alert({ kind: "ok", icon: "merge", title: "O AlmostUnified 0.9.4 está no pack.", text: "Ele faz os mods usarem um item só para os 4 materiais abaixo. A config fica em config/almostunified/. Nesta versão ele não unifica a geração de minério.", actions: B("Abrir a config", "configs", { size: "sm", icon: "file-code" }) });
      list = MATERIALS.map((m, i) => W.dupMaterial(Object.assign({}, m, { id: "du-" + i, state: "solved", solvedBy: "AlmostUnified", open: false, solution: { text: m.state === "items" ? "As receitas usam um lingote só e o outro fica escondido no JEI." : "As receitas usam um lingote só. Os 2 minérios continuam aparecendo no mundo: desligue um na config do mod." } }))).join("");
      health = "Na saúde do pack: −2, porque chumbo e níquel ainda geram minério em 2 mods cada. Itens repetidos só como item não tiram pontos.";
    } else {
      solution = `<section class="panel pro-p-sol" aria-labelledby="dp-sol"><h2 class="panel__title panel__title--sans" id="dp-sol">Solução para Forge 1.20.1</h2>
        <p class="t-sm t-2"><b>AlmostUnified</b> faz os mods usarem um item só: reescreve as receitas e esconde os repetidos no JEI.</p>
        <p class="t-sm t-2 pro-p-caveat">${W.icon("info", "icon--sm")}<span>Nesta versão ele não unifica a geração de minério. Para não ter veios em dobro, desligue o minério repetido na config de um dos mods.</span></p>
        <div class="row row--wrap">${B("Adicionar AlmostUnified", "adicionar-detalhe", { size: "sm", variant: "primary", icon: "plus" })}</div></section>`;
      list = MATERIALS.map((m, i) => W.dupMaterial(Object.assign({}, m, { id: "dn-" + i, solution: { name: "AlmostUnified", text: m.text, attrs: ADD_AU } }))).join("");
      health = "Na saúde do pack: −2, porque chumbo e níquel geram minério em 2 mods cada. Itens repetidos só como item não tiram pontos.";
    }
    const legacyNote = legacy ? W.alert({ kind: "warn", title: "No Minecraft 1.12.2 a leitura é aproximada.", text: "Nesta versão os mods registram os materiais em código. O Warden lê os nomes do dicionário de minérios nos arquivos de idioma e os arquivos de geração de minério conhecidos. O que é feito só em código ou por scripts do CraftTweaker fica de fora." }) : "";
    const n = legacy ? LEGACY.length : MATERIALS.length;
    return head + metaRow([legacy ? "Lido dos arquivos dos mods no último teste, 03/09/2026" : "Lido dos arquivos dos mods no último teste, hoje às 14:20"], verify) +
      `<div class="stack-3">${legacyNote}${why}${solution}</div>
      <h2 class="group-title" id="dp-list">Materiais repetidos <span class="t-3">${n}</span></h2><ul class="dups" aria-labelledby="dp-list">${list}</ul>
      <p class="t-xs t-3 pro-p-foot">${health}</p>`;
  }
  // Troca o pack do cabeçalho só durante a montagem da tela (estado "pack antigo")
  function withPack(pack, fn) { const saved = D.PACK; D.PACK = pack; try { return fn(); } finally { D.PACK = saved; } }
  def("repetidos", { group: "pack", title: "Itens repetidos entre mods", spec: "T32", v11: true,
    render: (s) => s === "antiga" ? withPack(LEGACY_PACK, () => packShell("problemas", dupContent(s), { unsaved: 0, problems: 1, problemsKind: "warn", empty: true })) : packShell("problemas", dupContent(s)),
    states: { carregando: "Lendo os arquivos dos mods", vazio: "Nada repetido", semdados: "Antes do primeiro teste", unificado: "AlmostUnified já no pack", antiga: "Pack Forge 1.12.2" } });
})();
