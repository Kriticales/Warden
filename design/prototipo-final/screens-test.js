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
  def("testar-menu", { group: "teste", title: "Menu do Testar (▾)", spec: "T11, T13", render: () => packShell("mods", modsContent("normal"), { menuOpen: true }) });
  function settingsDialog() {
    return W.dialog({ esc: "mods", size: "lg", title: "Ajustes do teste neste computador", sub: "Não fazem parte do pack e não contam como alteração não salva.", body: `
      ${W.memory({ id: "aj-mem", value: 6, recommended: [6, 8], mods: 128, ram: 32 })}
      <div>${W.select({ id: "aj-java", label: "Java", options: ["Automático: Java 17", "Java 21 (por sua conta)", "Java 8 (não abre o 1.20.1)", "Escolher um Java do computador…"], hint: "É o mais novo que o Minecraft 1.20.1 com Forge aceita, sempre com a atualização mais recente (17.0.12)." })}
        <details class="disclosure" open style="margin-top:8px"><summary>Por que não o Java 25?</summary><p class="t-sm t-2" style="margin-top:6px">O Minecraft 1.18 a 1.20.4 foi feito para o Java 17. Forge e mods dessa época não são garantidos em Java mais novo: o jogo pode não abrir. Você pode escolher outro Java acima, por sua conta.</p></details></div>
      ${W.input({ id: "aj-jvm", label: "Argumentos extras da JVM", optional: true, placeholder: "Deixe vazio se não souber", mono: true })}
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
      <p class="t-sm t-3">1 aviso também foi encontrado. Ele não impede o teste e fica em Problemas.</p>`,
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
  function runContent() {
    return testHead("Jogo aberto", "Começou às 14:20 · Minecraft 1.20.1 com Forge 47.3.0", B("Editar configs do teste", "configs-instancia", { icon: "file-code" }) + B("Abrir pasta da instância", "", { variant: "ghost", icon: "folder-open" }) + B("Parar jogo", "parar", { variant: "danger-ghost", icon: "square" }), { step: 5 }) +
      `<dl class="stats"><div class="stat"><dt>Tempo de jogo</dt><dd>4:12</dd></div><div class="stat"><dt>Memória em uso</dt><dd>3,1 <span class="t-sm t-2" style="font-family:var(--font-ui);text-shadow:none">de 6 GB</span></dd></div><div class="stat"><dt>Jogador</dt><dd>Jogador</dd></div><div class="stat"><dt>Mudanças detectadas</dt><dd>3 <span class="t-sm t-2" style="font-family:var(--font-ui);text-shadow:none">arquivos</span></dd></div></dl>
      <div class="run-console">${W.consoleBox({ lines: D.LOG })}</div>` + sim("Simular:", [["o jogo fechou normalmente", "teste-fechou"], ["o jogo travou", "teste-travou"], ["clicar em Testar em outro pack", "outro-pack"]]);
  }
  def("teste-jogo", { group: "teste", title: "4. Jogo aberto (console)", spec: "T13", render: () => packShell("teste", runContent(), { game: "running" }),
    after: () => { const log = document.querySelector(".console__log"); if (log) log.scrollTop = log.scrollHeight; } });
  def("parar", { group: "teste", title: "Parar jogo (confirmação)", spec: "T13", hidden: true, render: () => packShell("teste", runContent(), { game: "running",
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
    return testHead(`${W.icon("circle-check", "icon--lg t-ok")} O jogo fechou normalmente`, "Teste de hoje, 14:20 a 14:32 · 12 min de jogo", B("Ver console", "teste-jogo", { variant: "ghost", icon: "terminal" }) + B("Testar de novo", "teste-checagem", { icon: "play" }), { sans: true, steps: false }) +
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
  def("teste-fechou", { group: "teste", title: "5. Jogo fechou: o que mudou", spec: "T13, T15", render: (s) => packShell("teste", closedContent(s)), states: { conflito: "O mesmo arquivo mudou no pack e no jogo" } });
  def("trazido", { group: "teste", title: "Mudanças trazidas para o pack", spec: "T15, T16", hidden: true, render: () => packShell("teste", closedContent("normal"), { unsaved: 8,
    overlay: W.dialog({ esc: "mods", size: "sm", title: "3 mudanças trazidas para o pack", body: `<p>Agora elas fazem parte do pack e aparecem nas alterações não salvas, junto com o resto do que mudou desde a versão 1.4.2.</p>`, foot: B("Continuar editando", "mods", { variant: "ghost" }) + B("Salvar versão", "salvar", { variant: "primary", icon: "save" }) }) }) });

  // ---------- 5b. O jogo travou ----------
  function crashContent(state) {
    const noCause = state === "semcausa";
    const head = testHead(`${W.icon("circle-x", "icon--lg t-danger")} O jogo travou`, "Teste de hoje, 14:40 · fechou com 48 s de jogo", B("Abrir crash report", "", { variant: "ghost", icon: "file-text" }) + B("Ver console", "teste-jogo", { variant: "ghost", icon: "terminal" }) + B("Testar de novo", "teste-checagem", { icon: "play" }), { sans: true, steps: false });
    const ai = W.alert({ kind: "ai", title: noCause ? "A IA pode ler o log e sugerir uma causa." : "Não parece ser isso?", text: "Antes de enviar, você vê o texto exato, já sem dados pessoais. Nada é enviado sem a sua confirmação.", actions: B("Pedir ajuda à IA", "teste-ia", { variant: noCause ? "primary" : "secondary", icon: "sparkles" }) + W.badge("p1", "P1") });
    if (noCause) {
      return head + `<div class="stack" style="max-width:980px">${W.alert({ kind: "neutral", icon: "circle-help", title: "Não encontramos a causa automaticamente.", text: "O log não tem nenhum dos padrões que o Warden conhece (dependência faltando, mod duplicado, Java errado, falta de memória). As últimas linhas estão abaixo." })}
        ${W.consoleBox({ lines: [["info", "14:40:41", "minecraft/Minecraft", "Loaded 4 advancements"], ["error", "14:40:48", "minecraft/CrashReport", "Ticking entity: java.lang.NullPointerException: Cannot invoke \"net.minecraft.world.entity.Entity.m_20185_()\""], ["warden", "14:40:50", "Warden", "O jogo fechou com o código 255 depois de 48 segundos."]], state: "ended", height: 190, label: "Últimas linhas do console" })}
        ${ai}</div>`;
    }
    return head + `<div class="stack" style="max-width:980px">
      <section class="panel panel--strong" aria-labelledby="why-h"><h2 class="panel__title" id="why-h">Por que travou</h2><div style="margin-top:12px">
        ${W.issue({ kind: "danger", title: "Falta o mod Balm, exigido pelo Waystones", text: "O Forge parou de carregar porque o Waystones precisa do Balm 7.3.0 ou mais novo.", evidence: `<pre class="code">latest.log, linha 1.204:\nMissing or unsupported mandatory dependencies:\n\tMod ID: 'balm', Requested by: 'waystones', Expected range: '[7.3.0,)', Actual version: '[MISSING]'</pre>`, open: true, actions: B("Adicionar Balm", "dependencias", { variant: "primary", icon: "plus" }) + B("Remover Waystones", "", { variant: "ghost", icon: "trash-2" }) })}</div></section>
      ${ai}
      <div class="panel row row--between"><span class="t-sm">O jogo também mudou 1 arquivo antes de travar.</span>${B("Ver o que mudou", "teste-fechou", { variant: "link" })}</div></div>`;
  }
  def("teste-travou", { group: "teste", title: "5b. Jogo travou: por que travou", spec: "T13, T14", render: (s) => packShell("teste", crashContent(s)), states: { semcausa: "Sem conclusão automática" } });
  def("teste-ia", { group: "teste", title: "Pedir ajuda à IA (consentimento)", spec: "T14", render: () => packShell("teste", crashContent("normal"), { overlay: consentDialog("teste-travou") }) });

  // ---------- Um jogo por vez ----------
  def("outro-pack", { group: "teste", title: "Testar com o jogo aberto em outro pack", spec: "T13", hidden: true,
    render: () => window.P.appShell(`<div class="pagehead"><div><h1 class="pagehead__title" tabindex="-1" data-title>Meus packs</h1></div></div>`, { tasks: { state: "busy", text: "Jogo aberto: Vale Sereno" },
      overlay: W.dialog({ esc: "packs", size: "sm", title: "O Vale Sereno está com o jogo aberto", body: `<p>Só dá para testar um pack por vez. Feche o jogo do Vale Sereno ou volte para o teste dele.</p>`, foot: B("Cancelar", "packs", { variant: "ghost" }) + B("Ir para o teste", "teste-jogo", { variant: "primary", icon: "terminal" }) }) }) });

  def("tarefas-pack", { group: "teste", title: "Tarefas dentro do pack", spec: "T22", hidden: true, render: () => packShell("mods", modsContent("normal"), { tasks: { state: "busy", text: "1 tarefa em andamento" }, overlay: tasksDrawer("mods") }) });
})();
