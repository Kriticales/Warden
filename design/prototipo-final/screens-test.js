/* Testar: o modo aberto pelo botão do cabeçalho. Não é seção do menu (nenhum item acende). */
(function () {
  "use strict";
  const W = window.W, D = window.DATA;
  const { def, go, B, packShell, pageHead, modsContent, configsContent, consentDialog, tasksDrawer } = window.P;
  const STEPS = ["Verificar o pack", "Preparar o Minecraft", "Copiar o pack para o teste", "Verificação final", "Abrir o jogo"];
  const sim = window.P.sim;

  function testHead(title, sub, actions, o = {}) {
    return pageHead(title, sub, actions, { sans: o.sans }) + (o.steps === false ? "" : `<div class="test-steps">${W.steps(STEPS, o.step ?? 0, { label: "Etapas do teste", waiting: o.waiting, error: o.error })}</div>`);
  }
  const cancel = B("Cancelar teste", "mods", { variant: "ghost", icon: "x" });

  // ---------- Menu ▾ e ajustes ----------
  def("testar-menu", { group: "teste", title: "Menu do Testar (▾) com outros testes e perfis", spec: "T11, T13", d4: true, render: () => packShell("mods", modsContent("normal"), { menuOpen: true }) });
  def("perfil-ativo", { group: "teste", title: "Perfil “PC fraco” ativo no botão Testar", spec: "T11, T13", d4: true, hidden: true, render: () => packShell("mods", modsContent("normal"), { profile: "PC fraco" }) });
  function settingsDialog() {
    return W.dialog({ esc: "mods", size: "lg", title: "Ajustes do teste neste computador", sub: "Não fazem parte do pack e não contam como alteração não salva.", body: `
      <div class="field"><label class="field__label" for="aj-prof">Perfil do teste</label><div class="row row--wrap">${W.select({ bare: true, id: "aj-prof", options: ["Padrão", "PC fraco", "Shaders"] })}${B("Novo perfil", "", { size: "sm", icon: "plus" })}${B("Renomear", "", { size: "sm", variant: "ghost" })}${B("Apagar", "", { size: "sm", variant: "danger-ghost" })}</div>
        <p class="field__hint">Cada perfil guarda memória, Java, argumentos e como o jogo abre. Troque rápido pelo menu ▾ do Testar.</p></div>
      ${W.memory({ id: "aj-mem", value: 6, recommended: [6, 8], mods: 128, ram: 32 })}
      <div>${W.select({ id: "aj-java", label: "Java", options: ["Automático: Java 17", "Java 21 (por sua conta)", "Java 8 (não abre o 1.20.1)", "Escolher um Java do computador…"], hint: "É o mais novo que o Minecraft 1.20.1 com Forge aceita, sempre com a atualização mais recente (17.0.12)." })}
        <details class="disclosure" style="margin-top:8px"><summary>Por que não o Java 25?</summary><p class="t-sm t-2" style="margin-top:6px">O Minecraft 1.18 a 1.20.4 foi feito para o Java 17. Forge e mods dessa época não são garantidos em Java mais novo: o jogo pode não abrir. Você pode escolher outro Java acima, por sua conta.</p></details></div>
      ${W.select({ id: "aj-open", label: "Ao abrir o jogo", options: ["Ir para o menu principal", "Entrar direto no Mundo de teste (Minecraft 1.20 ou mais novo)"], hint: "Entrar direto pula o menu e a escolha do mundo. Usa o Quick Play do próprio jogo." })}
      ${W.input({ id: "aj-jvm", label: "Argumentos extras da JVM", optional: true, placeholder: "Deixe vazio se não souber", mono: true, hint: "O Warden avisa se algum argumento desligar a leitura da memória do jogo." })}
      <div><div class="field__label">Mods opcionais ligados no teste ${W.badge("p1", "P1")}</div><p class="t-sm t-3">Nenhum mod opcional neste pack.</p></div>
      <hr class="sep" /><div class="row">${B("Recriar instância de teste…", "", { size: "sm", icon: "rotate-ccw" })}<span class="t-sm t-3">Pergunta antes se mantém os mundos de teste.</span></div>`,
      foot: B("Cancelar", "mods", { variant: "ghost" }) + B("Salvar ajustes", "mods", { variant: "primary" }) });
  }
  def("ajustes-teste", { group: "teste", title: "Ajustes do teste (memória, Java)", spec: "T11", render: () => packShell("mods", modsContent("normal"), { overlay: settingsDialog() }) });

  // ---------- 1. Checagem ----------
  function checkContent() {
    return testHead("Testando o pack", "Etapa 1 de 5 · começou às 14:18", cancel, { step: 0 }) +
      `<div class="panel" style="max-width:760px">${W.progress({ label: "Verificando 128 itens", value: null, meta: "Dependências, versões, Java, duplicatas e conflitos conhecidos, com os dados do pack e das APIs." })}</div>`;
  }
  function checkDialog() {
    return W.dialog({ esc: "problemas", title: "2 erros podem impedir o jogo de abrir", sub: "Dá para corrigir agora, aqui mesmo, ou testar assim e ver o que acontece.", body: `
      ${W.issue({ kind: "danger", title: "O Waystones precisa do Balm, que não está no pack", actions: B("Adicionar Balm", "dependencias", { size: "sm", variant: "primary", icon: "plus" }) })}
      ${W.issue({ kind: "danger", title: "Dois mods de renderização: Embeddium e Rubidium", actions: B("Remover Rubidium", "", { size: "sm", icon: "trash-2" }) })}
      <p class="t-sm t-3">2 avisos também foram encontrados. Eles não impedem o teste e ficam em Problemas.</p>`,
      foot: B("Cancelar e ver Problemas", "problemas", { variant: "ghost" }) + B("Testar mesmo assim", "teste-preparo", { icon: "play" }) });
  }
  def("teste-checagem", { group: "teste", title: "1. Checagem antes do teste", spec: "T13, T14", render: (s) => packShell("teste", checkContent(), { game: "preparing", progress: 8, prepTarget: "teste-checagem", overlay: s === "vazio" ? "" : checkDialog() }), states: { vazio: "Sem erros: segue sozinho" },
    after: (s) => { if (s === "vazio") document.querySelector(".app__main .content").insertAdjacentHTML("beforeend", sim("Sem erros, o teste segue sozinho para a etapa 2.", [["Ir para a etapa 2", "teste-preparo"]])); } });

  // ---------- 2. Preparação ----------
  function prepContent(state) {
    const err = state === "erro";
    const left = `<div class="panel panel--strong stack">${W.progress({ label: "Baixando o Minecraft 1.20.1, o Forge 47.3.0 e o Java 17", value: 38, state: err ? "error" : "running", meta: err ? "Parou em 812 de 2.140 arquivos." : "812 de 2.140 arquivos · 160 de 420 MB · 4,2 MB/s · cerca de 2 minutos", valueText: "38%, cerca de 2 minutos" })}
      ${err ? W.alert({ kind: "danger", title: "O download parou: a conexão caiu.", text: "O que já foi baixado fica guardado. Ao tentar de novo, o Warden continua de onde parou.", actions: B("Tentar de novo", "teste-preparo", { size: "sm", icon: "refresh-cw" }) }) : ""}
      <ul class="substeps"><li>${W.icon("circle-check")}<span>Java 17.0.12 (Temurin) baixado e conferido</span></li><li class="is-now"><span class="loader t-primary" aria-hidden="true"><i></i><i></i><i></i><i></i></span><span>Bibliotecas e arquivos do jogo (812 de 2.140)</span></li><li class="is-next">${W.icon("circle-help", "t-3")}<span>Instalar o Forge 47.3.0 (uma vez por versão)</span></li></ul>
      <p class="t-sm t-3">Na primeira vez isso pode levar alguns minutos. Nos próximos testes, só o que mudou é baixado.</p></div>`;
    const right = `<aside class="panel" aria-label="Este teste"><div class="panel__title panel__title--sans" style="margin-bottom:10px">Este teste</div><dl class="kv"><dt>Minecraft</dt><dd>1.20.1</dd><dt>Loader</dt><dd>Forge 47.3.0</dd><dt>Java</dt><dd>17 (automático)</dd><dt>Memória</dt><dd>6 GB</dd><dt>Jogador</dt><dd>Jogador (offline)</dd><dt>Mundo</dt><dd>Mundo de teste</dd></dl><div style="margin-top:10px">${B("Ajustes do teste…", "ajustes-teste", { variant: "link" })}</div></aside>`;
    return testHead("Testando o pack", "Etapa 2 de 5 · começou às 14:18", cancel, { step: 1, error: err ? 1 : undefined }) + `<div class="prep">${left}${right}</div>` + sim("Avançar a preparação:", [["etapa 3 pede download manual", "teste-manual"], ["pular para o jogo aberto", "teste-jogo"]]);
  }
  def("teste-preparo", { group: "teste", title: "2. Preparação (download)", spec: "T13", render: (s) => packShell("teste", prepContent(s), { game: "preparing", progress: 38 }), states: { erro: "A conexão caiu" } });

  // ---------- 3. Download manual ----------
  function manualContent() {
    return testHead("Testando o pack", "Etapa 3 de 5 · esperando você", cancel, { step: 2, waiting: 2 }) + `<div class="stack" style="max-width:980px">
      ${W.alert({ kind: "warn", icon: "download", title: "2 mods da CurseForge precisam ser baixados à mão", text: "O autor não permite que outros apps baixem estes arquivos. Baixe pelo site e mostre ao Warden onde está. O Warden também procura sozinho na pasta Downloads." })}
      <div class="tablewrap"><table class="table"><thead><tr><th>Mod</th><th>Arquivo esperado</th><th>Situação</th><th><span class="sr-only">Ações</span></th></tr></thead><tbody>
        <tr><td class="t-strong">Xaero's World Map</td><td class="path">XaerosWorldMap_1.39.0_Forge_1.20.jar</td><td>${W.status("ok", "Encontrado em Downloads")}</td><td></td></tr>
        <tr><td class="t-strong">Epic Fight</td><td class="path">epicfight-forge-20.9.4-1.20.1.jar</td><td><span class="status status--warn"><span class="loader" aria-hidden="true"><i></i><i></i><i></i><i></i></span>Aguardando</span></td><td><div class="row row--wrap">${B("Abrir página de download", "", { size: "sm", icon: "external-link" })}${B("Selecionar arquivo baixado…", "teste-jogo", { size: "sm", variant: "primary", icon: "file-plus" })}</div></td></tr></tbody></table></div>
      <p class="t-sm t-3">Quando todos forem encontrados, o teste continua sozinho. Quem jogar o pack também vai precisar baixar esses mods à mão (o aviso aparece ao publicar).</p></div>`;
  }
  def("teste-manual", { group: "teste", title: "3. Download manual da CurseForge", spec: "T20", render: () => packShell("teste", manualContent(), { game: "preparing", progress: 71, prepTarget: "teste-manual" }) });

  // ---------- 4. Jogo aberto ----------
  function perf(state) {
    if (state === "memoria") return W.perfStrip({ state: "warn", mem: ["5,9", 6], series: D.MEM_SERIES_HIGH, seriesLabel: "Memória nos últimos 5 minutos: subiu de 4,8 para 5,9 GB", rss: "7,1", gc: "1.902", gcSub: "18,4 s parado em coletas", loaded: "1 min 42 s", loadedSub: "18 s a mais que na 1.4.2" });
    if (state === "semdados") return W.perfStrip({ state: "nodata", nodataWhy: "Os argumentos do teste têm -XX:+PerfDisableSharedMem.", rss: "4,2", gc: "—", gcSub: "sem leitura", loaded: "1 min 42 s", loadedSub: "18 s a mais que na 1.4.2" });
    return W.perfStrip({ mem: ["3,1", 6], series: D.MEM_SERIES, seriesLabel: "Memória nos últimos 5 minutos: entre 2,4 e 3,2 GB", rss: "4,2", gc: "214", gcSub: "1,2 s parado em coletas", loaded: "1 min 42 s", loadedSub: "18 s a mais que na 1.4.2" });
  }
  function runContent(state) {
    const grouped = state === "agrupado";
    const memAlert = state === "memoria" ? W.alert({ kind: "warn", title: "O jogo está quase sem memória.", text: "Depois de limpar a memória, ele continua usando mais de 90% dos 6 GB há 2 minutos. Aumente para 8 GB em Ajustes do teste.", actions: B("Ajustes do teste…", "ajustes-teste", { size: "sm", icon: "settings" }) }) : state === "semdados" ? W.alert({ kind: "neutral", compact: true, icon: "info", title: "A memória do jogo não pode ser lida neste teste.", text: "O argumento -XX:+PerfDisableSharedMem, comum nas “flags do Aikar”, desliga a leitura. A RAM do processo continua aparecendo.", actions: B("Ajustes do teste…", "ajustes-teste", { size: "sm" }) }) : "";
    const cons = W.consoleBox({ id: "run", show: grouped ? "grouped" : "lines", groups: D.GROUPS, lines: D.LOG, showAttrs: { "data-show": "" } });
    return testHead("Jogo aberto", "Começou às 14:20 · Minecraft 1.20.1 com Forge 47.3.0", B("Recarregar scripts", "", { icon: "refresh-cw", tip: "O pack tem KubeJS 6: o Warden mostra o comando para colar no jogo" }) + B("Editar configs do teste", "configs-instancia", { icon: "file-code" }) + B("Abrir pasta da instância", "", { variant: "ghost", icon: "folder-open", iconOnly: true }) + B("Parar jogo", "parar", { variant: "danger-ghost", icon: "square" }), { step: 5 }) +
      `<div class="stack-3">${perf(state)}${memAlert}<div class="run-console">${cons}</div></div>` + sim("Simular:", [["o jogo fechou normalmente", "teste-fechou"], ["o jogo travou", "teste-travou"], ["clicar em Testar em outro pack", "outro-pack"]]);
  }
  def("teste-jogo", { group: "teste", title: "4. Jogo aberto (desempenho e console)", spec: "T13", d4: true, render: (s) => packShell("teste", runContent(s), { game: "running" }), states: { agrupado: "Console agrupado por mod", memoria: "Memória alta", semdados: "Sem leitura de memória" },
    after: () => { const log = document.querySelector(".console__log"); if (log && log.getAttribute("role") === "log") log.scrollTop = log.scrollHeight; } });
  // Seletor "Mostrar" do console leva ao estado correspondente
  document.addEventListener("change", (e) => { if (e.target.matches && e.target.matches("[data-show]")) { const v = e.target.value; window.P.navigate(window.location.hash.slice(1).split("~")[0], v === "grouped" ? "agrupado" : "normal"); } });
  def("parar", { group: "teste", title: "Parar jogo (confirmação)", spec: "T13", hidden: true, render: () => packShell("teste", runContent("normal"), { game: "running",
    overlay: W.dialog({ esc: "teste-jogo", size: "sm", alert: true, title: "Parar o jogo?", body: `<p>O progresso não salvo do mundo de teste pode ser perdido.</p><p class="t-sm t-3">Para sair salvando, feche pelo menu do próprio jogo.</p>`, foot: B("Continuar jogando", "teste-jogo", { variant: "ghost" }) + B("Parar jogo", "teste-fechou", { variant: "danger", icon: "square" }) }) }) });
  def("configs-instancia", { group: "teste", title: "Editar configs durante o teste", spec: "T12, T13", render: () => packShell("configs", configsContent("normal", { instance: true }), { game: "running" }) });

  // ---------- 5. O jogo fechou: o que mudou ----------
  function closedContent(state) {
    const conflict = state === "conflito";
    const item = (checked, label, body, end) => `<div class="change-item">${W.check({ checked, ariaLabel: label })}<div><div class="t-sm t-strong">${label}</div>${body ? `<div style="margin-top:4px">${body}</div>` : ""}</div><div>${end || ""}</div></div>`;
    const conflictBlock = `<section class="change-group"><h3>${W.icon("git-compare", "t-warn")}O mesmo arquivo mudou no pack e no jogo <span class="t-3">(1)</span></h3>
      <div class="change-item" style="grid-template-columns:minmax(0,1fr)"><div class="t-sm t-strong path" style="color:var(--color-text)">config/create-common.toml</div>
        <div class="grid-2" style="margin-top:8px">${W.diff({ file: "No pack (você editou depois que o teste começou)", lines: [["del", 10, "", "\tmaxRotationSpeed = 128"], ["add", "", 10, "\tmaxRotationSpeed = 256"]] })}${W.diff({ file: "No jogo (mudou durante o teste)", lines: [["del", 13, "", "\tstressMultiplier = 1.0"], ["add", "", 13, "\tstressMultiplier = 2.0"]] })}</div>
        <fieldset style="border:0;padding:0;margin:10px 0 0"><legend class="sr-only">O que fazer com config/create-common.toml</legend><div class="choice-list choice-list--2">${W.choice({ name: "cf", title: "Manter a do pack", desc: "Descarta a mudança do jogo." })}${W.choice({ name: "cf", title: "Usar a do jogo", desc: "Descarta a sua edição no pack." })}${W.choice({ name: "cf", title: "Juntar as duas", desc: "As linhas mudadas são diferentes: dá para ficar com as duas.", checked: true })}${W.choice({ name: "cf", title: "Editar e combinar", desc: "Abre o editor com as três versões." })}</div></fieldset></div></section>`;
    const perfRes = state === "desempenho" ? `<section class="panel panel--strong" aria-labelledby="perf-h" style="margin-bottom:16px"><div class="panel__head"><div><h2 class="panel__title" id="perf-h">Desempenho</h2><p class="t-sm t-3" style="margin-top:2px">Teste com perfil de desempenho · abriu em 1 min 42 s, 18 s a mais que na 1.4.2</p></div></div>
        <div class="grid-2"><div><h3 class="field__label">O que mais demorou para carregar</h3><div class="tablewrap" style="margin-top:6px"><table class="table table--plain"><tbody>${D.LOAD_TIMES.map(([m, t]) => `<tr><td>${m}</td><td class="num t-mono">${t}</td></tr>`).join("")}</tbody></table></div>
          <p class="t-xs t-3" style="margin-top:6px">Mede o código de cada mod nas fases do Forge. Não entram as texturas e os modelos, que costumam ser a maior parte do tempo em packs grandes.</p></div>
          <div><h3 class="field__label">Perfil do spark salvo no jogo às 14:28</h3><dl class="kv" style="margin-top:6px"><dt>Quadros por segundo</dt><dd>58 em média, 21 no pior momento</dd><dt>Mais processamento</dt><dd>Create (31%), Epic Fight (12%), Supplementaries (6%)</dd></dl>
            <div class="row row--wrap" style="margin-top:8px">${B("Abrir no visualizador do spark", "", { size: "sm", icon: "external-link" })}</div><p class="t-xs t-3" style="margin-top:6px">Abre o arquivo deste computador no navegador. Nada é enviado: o spark lê o arquivo aí mesmo.</p></div></div></section>` : "";
    return testHead(`${W.icon("circle-check", "icon--lg t-ok")} O jogo fechou normalmente`, `Teste de hoje, 14:20 a 14:32 · 12 min de jogo · abriu em 1 min 42 s`, B("Ver console", "teste-jogo", { variant: "ghost", icon: "terminal" }) + B("Testar de novo", "teste-checagem", { icon: "play" }), { sans: true, steps: false }) + perfRes +
      `<section class="panel panel--strong" aria-labelledby="chg-h"><div class="panel__head"><div><h2 class="panel__title" id="chg-h">O que mudou durante o teste</h2><p class="t-sm t-3" style="margin-top:2px">${conflict ? "6" : "5"} arquivos · só mudanças de verdade, sem a reformatação que o jogo faz sozinho. Nada vai para o pack sem você marcar.</p></div></div>
      <div class="changes">${conflict ? conflictBlock : ""}
        <section class="change-group"><h3>${W.icon("file-code", "t-3")}Configs do pack que você mudou no jogo <span class="t-3">(1)</span></h3>${item(true, "config/embeddium-options.json", `<div class="kvdiff"><span>chunk_builder_threads</span><span class="old">2</span>${W.icon("arrow-right", "icon--sm")}<span class="new">4</span><span></span></div>`, B("Ver diferença", "", { variant: "link" }))}</section>
        <section class="change-group"><h3>${W.icon("settings", "t-3")}Opções do jogo (options.txt) <span class="t-3">2 de 31 mudanças</span></h3>${item(true, "options.txt", `<div class="kvdiff"><span>renderDistance</span><span class="old">12</span>${W.icon("arrow-right", "icon--sm")}<span class="new">16</span><span></span><span>guiScale</span><span class="old">0</span>${W.icon("arrow-right", "icon--sm")}<span class="new">3</span><span></span></div><p class="t-xs t-3" style="margin-top:4px">As 29 mudanças automáticas do jogo (posição da janela, último servidor) ficam escondidas.</p>`, B("Mostrar todas", "", { variant: "link" }))}</section>
        <section class="change-group"><h3>${W.icon("globe", "t-3")}Config de mundo <span class="t-3">(1)</span></h3>${item(false, "saves/Mundo de teste/serverconfig/create-server.toml", `<span class="t-xs t-2">Se trouxer, vai para ${W.tag("defaultconfigs/", "plain")} e vale para todo mundo novo dos jogadores.</span>`)}</section>
        <section class="change-group"><h3>${W.icon("puzzle", "t-3")}Mod colocado à mão na pasta da instância <span class="t-3">(1)</span></h3><div class="change-item" style="grid-template-columns:minmax(0,1fr) auto"><div class="t-sm"><span class="path">jei-1.20.1-forge-15.20.0.112.jar</span> é o JEI do Modrinth, numa versão mais nova. O arquivo não é copiado.</div>${B("Adicionar ao pack pelo Modrinth", "adicionar", { size: "sm", icon: "plus" })}</div></section>
        <details class="disclosure"><summary>Arquivos novos criados pelos mods (14) · desmarcados: em geral não precisam ir para o pack</summary><p class="path" style="margin-top:6px">config/jade/plugins.json, config/supplementaries-client.toml, … e mais 12</p></details>
      </div>
      <div class="dialog__foot" style="margin:16px -20px -16px;justify-content:space-between">${B("Descartar tudo", "", { variant: "danger-ghost" })}<div class="row">${B("Decidir depois", "pack-revisar", { variant: "ghost" })}${B("Trazer 3 selecionados para o pack", "trazido", { variant: "primary", icon: "download" })}</div></div></section>`;
  }
  def("teste-fechou", { group: "teste", title: "5. Jogo fechou: o que mudou", spec: "T13, T15", render: (s) => packShell("teste", closedContent(s)), states: { conflito: "O mesmo arquivo mudou no pack e no jogo", desempenho: "Teste com perfil de desempenho" } });
  def("trazido", { group: "teste", title: "Mudanças trazidas para o pack", spec: "T15, T16", hidden: true, render: () => packShell("teste", closedContent("normal"), { unsaved: 8,
    overlay: W.dialog({ esc: "mods", size: "sm", title: "3 mudanças trazidas para o pack", body: `<p>Agora elas fazem parte do pack e aparecem nas alterações não salvas, junto com o resto do que mudou desde a versão 1.4.2.</p>`, foot: B("Continuar editando", "mods", { variant: "ghost" }) + B("Salvar versão", "salvar", { variant: "primary", icon: "save" }) }) }) });

  // ---------- 5b. O jogo travou ----------
  function crashContent(state) {
    const noCause = state === "semcausa";
    const head = testHead(`${W.icon("circle-x", "icon--lg t-danger")} O jogo travou`, noCause ? "Teste de 30/09, 21:10 · fechou com 3 min 20 s de jogo · 3ª vez com a mesma causa" : "Teste de hoje, 14:40 · fechou com 48 s de jogo", B("Abrir crash report", "", { variant: "ghost", icon: "file-text" }) + B("Ver console", "teste-jogo", { variant: "ghost", icon: "terminal" }) + B("Testar de novo", "teste-checagem", { icon: "play" }), { sans: true, steps: false });
    const ai = W.alert({ kind: "ai", title: noCause ? "Ou converse com a IA sobre este travamento." : "Não parece ser isso?", text: "Ela consulta o log, o pack e os testes. Antes de começar, você vê o que ela poderá consultar; nada é enviado sem a sua confirmação.", actions: B("Pedir ajuda à IA", "teste-ia", { variant: "secondary", icon: "sparkles" }) + W.badge("p1", "P1") });
    const culprit = W.alert({ kind: "neutral", icon: "target", title: "Encontrar o mod culpado", text: "Este travamento já aconteceu 3 vezes. O Warden pode abrir o jogo em rodadas, com partes do pack, até achar o mod. Leva cerca de 30 minutos; dá para pausar e cancelar.", actions: B("Encontrar o mod culpado…", "culpado-config", { variant: "primary", icon: "target" }) + W.badge("p1", "P1") });
    if (noCause) {
      return head + `<div class="stack" style="max-width:980px">${W.alert({ kind: "neutral", icon: "circle-help", title: "Não encontramos a causa automaticamente.", text: "O log não tem nenhum dos padrões que o Warden conhece (dependência faltando, mod duplicado, Java errado, falta de memória). As últimas linhas estão abaixo." })}
        ${W.consoleBox({ lines: [["info", "21:13:41", "minecraft/Minecraft", "Loaded 4 advancements"], ["error", "21:14:02", "minecraft/CrashReport", "Ticking entity: java.lang.NullPointerException: Cannot invoke \"net.minecraft.world.entity.Entity.getX()\""], ["warden", "21:14:04", "Warden", "O jogo fechou com o código 255 depois de 3 min 20 s."]], state: "ended", height: 190, label: "Últimas linhas do console" })}
        ${culprit}${ai}</div>`;
    }
    return head + `<div class="stack" style="max-width:980px">
      <section class="panel panel--strong" aria-labelledby="why-h"><h2 class="panel__title" id="why-h">Por que travou</h2><div style="margin-top:12px">
        ${W.issue({ kind: "danger", title: "Falta o mod Balm, exigido pelo Waystones", text: "O Forge parou de carregar porque o Waystones precisa do Balm 7.3.0 ou mais novo.", evidence: `<pre class="code">latest.log, linha 1.204:\nMissing or unsupported mandatory dependencies:\n\tMod ID: 'balm', Requested by: 'waystones', Expected range: '[7.3.0,)', Actual version: '[MISSING]'</pre>`, open: true, actions: B("Adicionar Balm", "dependencias", { variant: "primary", icon: "plus" }) + B("Remover Waystones", "", { variant: "ghost", icon: "trash-2" }) })}</div></section>
      ${ai}
      <div class="panel row row--between"><span class="t-sm">O jogo também mudou 1 arquivo antes de travar.</span>${B("Ver o que mudou", "teste-fechou", { variant: "link" })}</div></div>`;
  }
  def("teste-travou", { group: "teste", title: "5b. Jogo travou: por que travou", spec: "T13, T14", render: (s) => packShell("teste", crashContent(s)), states: { semcausa: "Sem conclusão automática" } });
  def("teste-ia", { group: "teste", title: "Pedir ajuda à IA (consentimento)", spec: "T14", render: () => packShell("teste", crashContent("normal"), { overlay: consentDialog("teste-travou") }) });

  // ======================= Busca do culpado (D4) =======================
  // Modo da tela do teste: rodadas no lugar das 5 etapas. O pack, a instância de teste e os mundos não mudam.
  function culpritConfig(o = {}) {
    const old = o.old;
    return W.dialog({ esc: "teste-travou~semcausa", size: "lg", title: "Encontrar o mod culpado", sub: "O Warden abre o jogo várias vezes com partes do pack, numa cópia separada. Seu pack, a instância de teste e seus mundos não mudam.", body: `
      <fieldset style="border:0;padding:0;margin:0"><legend class="field__label" style="margin-bottom:6px">O que acontece no jogo?</legend><div class="choice-list choice-list--2">
        ${W.choice({ name: "bs-sym", title: "Trava ao abrir o jogo", desc: "Fecha antes do menu principal." })}
        ${W.choice({ name: "bs-sym", title: "Trava ao entrar no mundo", desc: "Igual ao travamento de 30/09 (Ticking entity), que aconteceu 3 vezes.", checked: true })}
        ${W.choice({ name: "bs-sym", title: "Aparece uma mensagem no log", desc: "Você escolhe a linha no console e o Warden procura por ela." })}
        ${W.choice({ name: "bs-sym", title: "Outra coisa: eu digo se aconteceu", desc: "A cada rodada você joga e responde. Para problemas que precisam de uma ação no jogo." })}</div></fieldset>
      ${old ? W.alert({ kind: "info", icon: "server", title: "Para este problema, o Warden precisa abrir um servidor neste computador.", text: "No Minecraft 1.12.2 o jogo não entra direto num mundo. Em cada rodada o Warden abre um servidor só para você (127.0.0.1) e entra nele; o servidor fecha sozinho ao terminar. Ele pede antes de abrir.", actions: B("Entendi, pode abrir", "", { size: "sm" }) }) : ""}
      <div class="panel"><div class="t-caps t-3">Estimativa</div><div class="t-strong" style="margin-top:4px;font-size:var(--text-md)">11 a 17 rodadas · cerca de 22 a 35 minutos</div>
        <ul class="checklist" style="margin-top:12px"><li class="is-ok">${W.icon("check")}<span>118 mods entram na busca. O Forge fica sempre ligado.</span></li><li class="is-ok">${W.icon("check")}<span>Cada mod vai junto com as bibliotecas que ele exige, para não aparecer um erro diferente.</span></li><li class="is-ok">${W.icon("check")}<span>spark e Crash Assistant ficam desligados durante a busca, para não atrapalhar.</span></li><li class="is-ok">${W.icon("check")}<span>Cada rodada entra numa cópia do “Mundo de teste”, apagada no fim.</span></li></ul></div>
      <p class="t-sm t-2">Enquanto a busca roda, não dá para testar outra coisa. Você pode pausar entre rodadas e cancelar quando quiser; ao cancelar, o Warden mostra até onde chegou.</p>`,
      foot: B("Cancelar", "teste-travou~semcausa", { variant: "ghost" }) + B("Começar a busca", "culpado", { variant: "primary", icon: "target" }) });
  }
  def("culpado-config", { group: "teste", title: "Encontrar o mod culpado: configurar", spec: "T13", d4: true, render: (s) => packShell("teste", crashContent("semcausa"), { overlay: culpritConfig({ old: s === "velho" }) }), states: { velho: "Versão antiga: precisa de servidor" } });

  function roundsTable(list) {
    const word = { same: ["danger", "Travou igual"], pass: ["ok", "Passou"], now: ["muted", "Em andamento"], diff: ["warn", "Travou diferente"], paused: ["muted", "Pausada antes desta rodada"], ask: ["warn", "Esperando a sua resposta"], todo: ["muted", "Não feita"] };
    return `<div class="tablewrap"><table class="table"><thead><tr><th class="num">Rodada</th><th>Mods ligados</th><th>Resultado</th><th>Tempo</th><th class="shrink"><span class="sr-only">Log</span></th></tr></thead><tbody>${list.map((r) => `<tr><td class="num">${r.n}</td><td>${r.what} <span class="t-3">(${r.mods})</span></td><td>${r.result === "now" ? `<span class="status status--muted"><span class="loader" aria-hidden="true"><i></i><i></i><i></i><i></i></span>Em andamento</span>` : W.status(word[r.result][0], word[r.result][1])} <span class="t-xs t-3">${r.note}</span></td><td class="t-2">${r.time}</td><td class="shrink">${r.result === "now" ? "" : B(`Log da rodada ${r.n}`, "", { size: "sm", variant: "ghost", iconOnly: true, icon: "file-text" })}</td></tr>`).join("")}</tbody></table></div>`;
  }
  function bisectContent(state) {
    const paused = state === "pausada", ask = state === "assistido", cancelled = state === "cancelada";
    const list = D.ROUNDS.map((r) => Object.assign({}, r, r.result === "now" ? { result: paused ? "paused" : ask ? "ask" : cancelled ? "todo" : "now" } : {}));
    const track = [...list, ...[7, 8, 9, 10, 11].map((n) => ({ n, result: "todo" }))];
    const actions = cancelled ? B("Começar de novo", "culpado-config", { icon: "rotate-ccw" }) : paused ? B("Continuar a busca", "culpado", { variant: "primary", icon: "play" }) + B("Cancelar busca", "culpado~cancelada", { variant: "danger-ghost", icon: "x" }) : B("Pausar depois desta rodada", "culpado~pausada", { icon: "pause" }) + B("Cancelar busca", "culpado~cancelada", { variant: "danger-ghost", icon: "x" });
    const sub = cancelled ? "Cancelada na rodada 6 · 12 min" : paused ? "Pausada antes da rodada 6 · 7 mods ainda suspeitos · 11 min até agora" : "Rodada 6 de 11 a 17 · 7 mods ainda suspeitos · 12 min até agora";
    let now;
    if (cancelled) now = W.alert({ kind: "warn", icon: "target", title: "O culpado está entre estes 7 mods.", text: "A busca parou antes do fim, mas já descartou 111 mods. Você pode conversar com a IA sobre estes 7 ou começar a busca de novo depois.", more: `<p class="t-sm" style="margin-top:6px">Epic Fight · Farmer's Delight · Jade · Sophisticated Core · Supplementaries · Waystones · Xaero's World Map</p>`, actions: B("Conversar com a IA", "ia-consent", { size: "sm", icon: "sparkles", iconCls: "icon--ai" }), actionsBelow: true });
    else if (ask) now = `<div class="panel panel--selected"><div class="t-caps t-3">Rodada 6 · modo assistido</div><p style="margin-top:6px">O jogo está aberto com 69 mods. Faça no jogo o que causa o problema e diga o que aconteceu.</p><div class="btn-row" style="margin-top:12px">${B("Aconteceu", "culpado", { variant: "primary", icon: "circle-x" })}${B("Não aconteceu", "culpado", { icon: "circle-check" })}${B("Não deu para testar", "culpado", { variant: "ghost", icon: "circle-minus" })}</div><p class="t-xs t-3" style="margin-top:8px">Se o jogo travar, a rodada é marcada sozinha.</p></div>`;
    else if (paused) now = W.alert({ kind: "neutral", icon: "pause", title: "Busca pausada.", text: "O estado fica guardado: dá para continuar agora ou outro dia. Nenhum jogo está aberto." });
    else now = `<div class="panel panel--selected"><div class="row row--between"><div class="t-caps t-3">Agora: rodada 6</div><span class="t-xs t-3">48 s</span></div><p style="margin-top:6px"><b>Testando os primeiros 69 mods</b> numa cópia do Mundo de teste.</p>${W.progress({ label: "Abrindo o jogo", value: null, size: "sm", meta: "Espera entrar no mundo e ficar 20 s sem travar. A rodada 5 passou com 66 mods; se esta travar, o culpado está entre os mods 67 e 69." })}</div>`;
    const side = `<aside class="stack-3"><div class="panel">${W.suspects(cancelled || paused ? 7 : 7, 118, { from: 22 })}<details class="disclosure" style="margin-top:8px"><summary>Ver os 7 mods suspeitos</summary><p class="t-sm" style="margin-top:4px">Epic Fight, Farmer's Delight, Jade, Sophisticated Core, Supplementaries, Waystones, Xaero's World Map</p></details></div>
      <div class="panel"><div class="t-caps t-3">Sempre ligados</div><p class="t-sm t-2" style="margin-top:4px">Forge 47.3.0</p><div class="t-caps t-3" style="margin-top:8px">Desligados na busca</div><p class="t-sm t-2" style="margin-top:4px">spark, Crash Assistant</p></div>
      <p class="t-xs t-3">O que o Warden faz: abre o jogo com uma parte dos mods. Se travar igual, o culpado está nessa parte; se não travar, está no resto. A cada rodada sobra metade dos suspeitos.</p></aside>`;
    return pageHead("Buscando o mod culpado", sub, actions, { sans: true }) +
      `<div class="test-steps">${W.rounds(track)}</div><div class="bisect"><div class="bisect__now stack">${now}<h2 class="group-title" style="margin:8px 0 0">Rodadas</h2>${roundsTable(cancelled ? list.filter((r) => r.result !== "todo") : list)}</div>${side}</div>` +
      sim("Ver outros estados:", [["a busca terminou", "culpado-resultado"], ["modo assistido", "culpado~assistido"]]);
  }
  def("culpado", { group: "teste", title: "Busca do culpado: em andamento", spec: "T13", d4: true, render: (s) => packShell("teste", bisectContent(s), { game: s === "cancelada" || s === "pausada" ? "ready" : "bisect", progress: 44, tasks: s === "cancelada" ? null : { state: "busy", text: s === "pausada" ? "Busca do culpado pausada antes da rodada 6" : "Buscando o culpado · rodada 6" } }), states: { pausada: "Pausada entre rodadas", assistido: "Modo assistido (você responde)", cancelada: "Cancelada: resultado parcial" } });

  function bisectResult() {
    const done = [...D.ROUNDS.slice(0, 6), { n: 6, mods: 69, result: "same", what: "Primeiros 69 mods", time: "1 min 12 s", note: "Travou com a mesma mensagem" }, { n: 7, mods: 67, result: "pass", what: "Primeiros 67 mods", time: "1 min 8 s", note: "Passou" }, { n: 8, mods: 68, result: "same", what: "Primeiros 68 mods (com o Epic Fight)", time: "1 min 10 s", note: "Travou: o Epic Fight é o último que falta" }];
    const track = [...done, ...[9, 10, 11, 12, 13].map((n) => ({ n, result: n === 11 ? "pass" : "same" })), { n: 14, result: "pass" }];
    return pageHead(`${W.icon("target", "icon--lg t-primary")} Culpado encontrado: Epic Fight (junto com o Supplementaries)`, "Busca de hoje, 15:02 a 15:33 · 15 rodadas · 31 min", B("Testar de novo", "teste-checagem", { icon: "play" }), { sans: true }) +
      `<div class="stack" style="max-width:980px"><section class="panel panel--strong" aria-labelledby="br-h"><h2 class="sr-only" id="br-h">Resultado</h2>
        <p>O jogo trava ao entrar no mundo quando o Epic Fight está ligado junto com o Supplementaries. Sem o Epic Fight, entrou e ficou estável; só com o Epic Fight e o Supplementaries, travou com a mesma mensagem.</p>
        <div class="row row--wrap row--gap-4" style="margin-top:12px"><div><div class="t-caps t-3">Confiança</div><div style="margin-top:2px">${W.status("ok", "Alta: confirmado em 2 testes de controle")}</div></div><div><div class="t-caps t-3">Também ligado</div><p class="t-sm">Forge 47.3.0</p></div></div>
        <div class="btn-row" style="margin-top:14px">${B("Ver o que o Epic Fight altera no jogo", "mods-raio-x", { icon: "scan-search" })}${B("Remover Epic Fight do pack", "", { variant: "danger-ghost", icon: "trash-2" })}${B("Perguntar à IA", "ia-consent", { variant: "ghost", icon: "sparkles", iconCls: "icon--ai" })}</div></section>
        ${W.alert({ kind: "info", title: "Os dois mods alteram o mesmo ponto do jogo.", text: "O raio-x mostra que Epic Fight e Supplementaries trocam a mesma chamada no movimento das criaturas. Atualizar o Supplementaries para a 2.8.21 pode resolver sem remover nada.", actions: B("Ver atualização", "atualizar", { size: "sm" }) })}
        <details class="disclosure"><summary>Ver as 15 rodadas</summary><div style="margin-top:8px">${W.rounds(track, { legend: false })}<div style="margin-top:10px">${roundsTable(done)}</div><p class="t-xs t-3" style="margin-top:6px">… e mais 6 rodadas para achar o par e confirmar.</p></div></details>
        <p class="t-xs t-3">O resultado fica em Problemas → Travamentos, junto com o travamento de 30/09.</p></div>`;
  }
  def("culpado-resultado", { group: "teste", title: "Busca do culpado: resultado", spec: "T13", d4: true, render: () => packShell("teste", bisectResult()) });

  // ======================= Servidor local e Testar como servidor (D4) =======================
  def("servidor-eula", { group: "teste", title: "Testar como servidor: aceitar a EULA", spec: "T13", d4: true, render: () => packShell("mods", modsContent("normal"), {
    overlay: W.dialog({ esc: "mods", size: "sm", title: "Aceitar a EULA do Minecraft?", body: `<p>Para abrir um servidor do Minecraft, a Mojang exige que você aceite a EULA, a licença de uso do jogo.</p><p>${B("Ler a EULA (aka.ms/MinecraftEULA)", "", { variant: "link", iconEnd: "external-link" })}</p><p class="t-sm t-2">O Warden pergunta só uma vez e lembra a sua resposta. Dá para mudar em Configurações → Teste.</p>`,
      foot: B("Agora não", "mods", { variant: "ghost" }) + B("Aceito a EULA", "servidor-opcoes", { variant: "primary" }) }) }) });
  function serverOptions(low) {
    return W.dialog({ esc: "mods", title: "Testar como servidor", sub: "Abre um servidor do Minecraft neste computador, só para você. Ele não faz parte do Testar normal.", body: `
      <fieldset style="border:0;padding:0;margin:0"><legend class="sr-only">Como testar</legend><div class="choice-list">
        ${W.choice({ name: "sv-how", title: "Abrir o servidor e entrar nele com o jogo", badge: " " + W.tag("Recomendado", "ok"), desc: "Testa os dois lados: o servidor com os mods e o jogo conectado a ele.", checked: true })}
        ${W.choice({ name: "sv-how", title: "Só o servidor", desc: "Mais leve. Confere se o servidor abre com estes mods." })}</div></fieldset>
      <dl class="kv"><dt>Memória</dt><dd>servidor 4 GB + jogo 6 GB = 10 GB</dd><dt>Livre agora</dt><dd${low ? ' class="t-warn"' : ""}>${low ? "7 GB" : "13 GB"}</dd></dl>
      ${low ? W.alert({ kind: "warn", title: "Pode faltar memória.", text: "Com 7 GB livres, o computador pode ficar lento ou o jogo pode fechar. Feche outros programas, escolha “Só o servidor” ou use um perfil com menos memória." }) : ""}
      <ul class="checklist"><li class="is-ok">${W.icon("shield")}<span>Só aceita conexões deste computador (127.0.0.1) e fecha sozinho quando o teste acabar.</span></li><li class="is-ok">${W.icon("monitor")}<span>8 mods só de cliente ficam de fora: Embeddium, Rubidium, Jade e mais 5.</span></li><li class="is-ok">${W.icon("folder")}<span>Usa uma pasta própria, separada da instância de teste. O pack não muda.</span></li></ul>`,
      foot: B("Cancelar", "mods", { variant: "ghost" }) + B("Abrir servidor e testar", "teste-servidor", { variant: "primary", icon: "server" }) });
  }
  def("servidor-opcoes", { group: "teste", title: "Testar como servidor: opções", spec: "T13", d4: true, render: (s) => packShell("mods", modsContent("normal"), { overlay: serverOptions(s === "pouca") }), states: { pouca: "Pouca memória livre" } });
  function serverRun(state) {
    const game = state === "jogo";
    const SSTEPS = ["Verificar o pack", "Preparar o servidor", "Preparar o Minecraft", "Copiar o pack para o teste", "Abrir o servidor e o jogo"];
    const cons = W.consoleBox({ id: "srv", show: "lines", lines: game ? D.LOG : D.SERVER_LOG, source: { value: game ? "game" : "server", options: [["server", "Servidor"], ["game", "Jogo"]] }, command: !game, label: game ? "Console do jogo" : "Console do servidor" });
    return pageHead("Testando como servidor", "Servidor em 127.0.0.1, porta 25566 · Forge 47.3.0 · o jogo entrou nele às 15:11", B("Recarregar scripts", "", { icon: "refresh-cw", tip: "Envia kubejs reload server_scripts ao servidor" }) + B("Parar jogo e servidor", "", { variant: "danger-ghost", icon: "square" }), { sans: true }) +
      `<div class="test-steps">${W.steps(SSTEPS, 5, { label: "Etapas do teste com servidor" })}</div><div class="stack-3">${W.perfStrip({ label: "Desempenho do servidor", memLabel: "Memória do servidor", mem: ["2,4", 4], series: D.MEM_SERIES.map((v) => v * 0.75), seriesLabel: "Memória do servidor nos últimos 5 minutos", rss: "3,3", gc: "88", gcSub: "0,4 s parado em coletas", loaded: "29 s", loadedSub: "servidor pronto (Done)" })}<div class="run-console">${cons}</div></div>` +
      sim("Ver também:", [["o console do jogo", "teste-servidor~jogo"], ["recarregar scripts pelo editor", "scripts~servidor"]]);
  }
  def("teste-servidor", { group: "teste", title: "Testando como servidor (dois consoles)", spec: "T13", d4: true, render: (s) => packShell("teste", serverRun(s), { game: "running", runTarget: "teste-servidor" }), states: { jogo: "Mostrando o console do jogo" },
    after: (s) => { document.querySelectorAll('.console .segmented [role=radio]').forEach((b) => { if (b.getAttribute("aria-checked") !== "true") b.setAttribute("data-go", b.dataset.value === "game" ? "teste-servidor~jogo" : "teste-servidor"); }); const log = document.querySelector(".console__log"); if (log) log.scrollTop = log.scrollHeight; void s; } });

  // ---------- Um jogo por vez ----------
  def("outro-pack", { group: "teste", title: "Testar com o jogo aberto em outro pack", spec: "T13", hidden: true,
    render: () => window.P.appShell(`<div class="pagehead"><div><h1 class="pagehead__title" tabindex="-1" data-title>Meus packs</h1></div></div>`, { tasks: { state: "busy", text: "Jogo aberto: Vale Sereno" },
      overlay: W.dialog({ esc: "packs", size: "sm", title: "O Vale Sereno está com o jogo aberto", body: `<p>Só dá para testar um pack por vez. Feche o jogo do Vale Sereno ou volte para o teste dele.</p>`, foot: B("Cancelar", "packs", { variant: "ghost" }) + B("Ir para o teste", "teste-jogo", { variant: "primary", icon: "terminal" }) }) }) });

  def("tarefas-pack", { group: "teste", title: "Tarefas dentro do pack", spec: "T22", hidden: true, render: () => packShell("mods", modsContent("normal"), { tasks: { state: "busy", text: "1 tarefa em andamento" }, overlay: tasksDrawer("mods") }) });
})();
