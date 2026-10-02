/* Warden 1.1 "Profissional" (tarefa D5): travamento de um jogador (T30), notas e grupos de mods (T31)
   e desempenho entre versões (T33). Lugares definidos em docs/design/ESTRUTURA.md §14:
   jogador em Problemas (← Problemas), notas e grupos em Mods, desempenho no Histórico (← Histórico). */
(function () {
  "use strict";
  const W = window.W;
  const { def, go, B, sim, packShell, pageHead, navigate } = window.P;
  const MODEL = "gemini-3.8-flash";
  const esc = W.esc;

  // ======================= TRAVAMENTO DE UM JOGADOR (T30) =======================
  const LINK = "https://mclo.gs/aB3dE9f";
  const ORIGIN = "mclo.gs/aB3dE9f · recebido hoje às 11:02";
  const backProblemas = { back: ["Problemas", "problemas"], sans: true };

  function playerEntry(state) {
    const loading = state === "carregando", err = state === "erro";
    const src = W.logSource({
      id: "pro-o-log",
      state: loading ? "loading" : err ? "invalid" : "empty",
      value: loading ? LINK : err ? "https://drive.google.com/file/d/1aB7xQ/view" : "",
      service: "mclo.gs",
      label: "Link ou texto do log do jogador",
      error: "O Warden não sabe ler links deste site. Peça o arquivo ao jogador ou cole o texto aqui.",
      fileAttrs: go(""),
    });
    const progress = loading ? `<div class="pro-o-progress">${W.progress({ label: "Etapa 1 de 2: baixar o log", value: 42, meta: "1,9 MB de 4,6 MB, só texto, por HTTPS. Depois o Warden tira os dados pessoais e compara com as 5 versões salvas." })}</div>` : "";
    const analyze = loading
      ? W.btn("Analisando…", { variant: "primary", busy: true, disabled: true }) + B("Cancelar", "jogador", { variant: "ghost" })
      : B("Analisar", "jogador-resultado", { variant: "primary", icon: "scan-search" });
    const form = `<section class="panel panel--strong pro-o-src" aria-labelledby="pj-h"><h2 class="panel__title panel__title--sans" id="pj-h">O que o jogador mandou</h2>
      ${src}
      <p class="t-sm t-2">Peça ao jogador o link que o Crash Assistant mostra quando o jogo trava, ou o arquivo <span class="t-mono">crash-reports/crash-….txt</span>. Se ele mandar também o <span class="t-mono">packwiz.json</span> da pasta do jogo, o Warden acha a versão exata.</p>
      <p class="pro-o-services">Links aceitos: mclo.gs, Pastebin, paste.ee, Gist do GitHub, 0x0.st, hst.sh e paste.gg. Arquivos: latest.log, debug.log, crash report e packwiz.json, vários de uma vez.</p>
      ${progress}
      <div class="pro-o-actions">${analyze}</div></section>`;
    const privacy = W.alert({ kind: "neutral", compact: true, icon: "eye-off", title: "Dados pessoais", text: "Antes de mostrar ou guardar, o Warden tira dados pessoais: nome do jogador, nome de usuário do Windows, IPs. Só a cópia sem dados pessoais fica guardada, neste computador." });
    const after = `<p class="t-xs t-3">Cada análise vira uma linha em Problemas → Travamentos, com a origem Jogador, junto com os travamentos dos seus testes que tiveram a mesma causa.</p>`;
    const simBox = state === "normal" ? sim("Simular o que acontece ao colar:", [["um link do mclo.gs", "jogador~carregando"], ["um link de outro site", "jogador~erro"]])
      : loading ? sim("O download termina em alguns segundos.", [["Ver o resultado", "jogador-resultado"]]) : "";
    return pageHead("Travamento de um jogador", "Descubra qual versão do pack o jogador usa, por que o jogo dele travou e o que fazer.", null, backProblemas) +
      `<div class="stack pro-o-page">${form}${privacy}${after}${simBox}</div>`;
  }
  def("jogador", { group: "pack", title: "Travamento de um jogador: colar o log", spec: "T30", v11: true,
    render: (s) => packShell("problemas", playerEntry(s)),
    states: { carregando: "Baixando o log do mclo.gs", erro: "Link de um site que o Warden não sabe ler" } });

  // ---------- Resultado ----------
  const codeLines = (title, lines) => `<div class="t-xs t-3" style="margin-bottom:4px">${title}</div><pre class="code" aria-label="${esc(title)}">${lines.map(([n, t]) => `<span class="t-3">${String(n).padStart(3, " ")}</span>  ${esc(t)}`).join("\n")}</pre>`;
  const EV_TICK = codeLines("Crash report do jogador, linha 41 (cópia sem dados pessoais)", [
    [7, "Description: Ticking entity"],
    [9, "java.lang.NullPointerException: Cannot invoke \"net.minecraft.world.entity.Entity.getX()\""],
    [10, "\tat net.minecraft.world.entity.LivingEntity.travel(LivingEntity.java:2108)"],
    [41, "Mixins in Stacktrace:"],
    [42, "\tnet.minecraft.world.entity.LivingEntity:"],
    [43, "\t\tepicfight.mixins.json:MixinLivingEntity"],
    [44, "\t\tsupplementaries-common.mixins.json:LivingEntityMixin"],
    [57, "\tEntity Name: <jogador>"],
    [63, "\tGame Directory: C:\\Users\\<usuário>\\curseforge\\minecraft\\Instances\\<instância>"],
  ]);
  const EV_OOM = (xmx) => codeLines("Crash report do jogador, linha 9 (cópia sem dados pessoais)", [
    [7, "Description: Exception in server tick loop"],
    [9, "java.lang.OutOfMemoryError: Java heap space"],
    [52, "\tPlayer Count: 1 / 8; [ServerPlayer['<jogador>'/312, l='ServerLevel[Novo mundo]']]"],
    [70, `\tJVM Flags: 2 total; -Xmx${xmx} -Xms256M`],
    [74, "\tGame Directory: C:\\Users\\<usuário>\\AppData\\Roaming\\.minecraft"],
  ]);
  const EV_OPTIFINE = codeLines("Crash report do jogador, linha 38 (cópia sem dados pessoais)", [
    [7, "Description: Rendering overlay"],
    [9, "java.lang.RuntimeException: Mixin transformation of net.minecraft.client.renderer.LevelRenderer failed"],
    [38, "Caused by: org.spongepowered.asm.mixin.injection.throwables.InvalidInjectionException: embeddium.mixins.json:core.WorldRendererMixin"],
    [39, "\t... target was modified by net.optifine.reflect.Reflector"],
    [61, "\tGame Directory: C:\\Users\\<usuário>\\curseforge\\minecraft\\Instances\\<instância>"],
  ]);

  function playerResult(state) {
    const ai = W.alert({ kind: "ai", title: state === "proxima" || state === "antiga" || state === "desconhecido" ? "Ainda com dúvida? Converse com a IA." : "Não parece ser isso? Converse com a IA.", text: "Ela recebe o log sem dados pessoais e a versão do pack do jogador, e pode comparar com o pack atual. Antes de começar, você vê o que ela poderá consultar.", actions: B("Conversar com a IA", "jogador-ia", { variant: "secondary", icon: "sparkles", iconCls: "icon--ai" }) });
    const culprit = W.alert({ kind: "neutral", icon: "target", title: "Encontrar o mod culpado", text: "Abre o jogo neste computador, em rodadas, com partes do pack atual, até achar o mod. Leva cerca de 30 minutos; dá para pausar e cancelar.", actions: B("Encontrar o mod culpado…", "culpado-config", { icon: "target" }) });
    let match, top = "", issues, showCulprit = true;
    const tickIssue = (o = {}) => W.issue({ kind: "warn", id: "pj-i1", title: "Provável causa: Epic Fight e Supplementaries alteram o mesmo ponto do jogo",
      text: "O jogo do jogador travou no movimento de uma criatura, e os dois mods trocam essa mesma chamada (LivingEntity#travel). " + (o.low ? "Parece a mesma causa de 3 travamentos dos seus testes, mas o log cortado não deixa confirmar." : "É a mesma causa de 3 travamentos dos seus testes."),
      evidence: EV_TICK, open: true, actions: B("Ver o que o Epic Fight altera", "mods-raio-x", { size: "sm", icon: "scan-search" }) });
    if (state === "proxima") {
      match = W.versionMatch({ id: "pj-vm", level: 2, variant: "near", version: "1.4.0", matched: 127, total: 128, open: true,
        diffs: { extra: [["OptiFine", "HD U I6"], ["Entity Culling", "1.6.6"]], changed: [["Supplementaries", "2.8.17", "2.8.11"]] } });
      issues = W.issue({ kind: "danger", id: "pj-i1", title: "O OptiFine não funciona junto com o Embeddium", text: "O jogador pôs o OptiFine por conta própria. O Embeddium, que já está no pack, faz o mesmo trabalho, e os dois juntos travam o jogo ao desenhar o mundo. Peça ao jogador para tirar o OptiFine da pasta mods.", evidence: EV_OPTIFINE, open: true }) +
        W.issue({ kind: "warn", id: "pj-i2", title: "O jogador tem 2 mods que não são do pack: OptiFine HD U I6 e Entity Culling 1.6.6", text: "Mods que o jogador acrescenta costumam ser a causa. Se ele reinstalar pelo link do pack, fica só com o que o pack tem.", evidence: "Lista de mods do crash report do jogador (linhas 112 a 240), comparada com a versão publicada 1.4.0." });
      showCulprit = false;
    } else if (state === "antiga") {
      match = W.versionMatch({ id: "pj-vm", level: 2, variant: "exact", version: "1.3.0", matched: 128, total: 128, title: "O jogador está na 1.3.0; a versão publicada agora é a 1.4.0", text: "A lista de mods do log é igual à da versão 1.3.0, publicada em 02/09/2026. Confirmado pelo packwiz.json da pasta do jogo." });
      top = W.alert({ kind: "ok", icon: "circle-check", title: "Isso parece já corrigido na 1.4.1, que ainda não foi publicada.", text: "A mesma causa aparece em Travamentos como “não voltou a acontecer desde a 1.4.1”. Publique uma versão nova: quem instalou pelo link do pack recebe sozinho ao abrir o jogo." });
      issues = W.issue({ kind: "danger", id: "pj-i1", title: "O jogo ficou sem memória", text: "O jogo do jogador usou toda a memória que tinha (4 GB) e fechou. É a mesma causa do travamento dos seus testes de 22/09, na 1.4.0.", evidence: EV_OOM("4G"), open: true });
      showCulprit = false;
    } else if (state === "desconhecido") {
      match = W.versionMatch({ id: "pj-vm", level: 2, variant: "unknown", matched: 12, total: 128, text: "Este log não parece ser deste pack (só 12 de 128 mods conferem). O Warden não escolheu uma versão; a análise do travamento continua, sem os dados do pack." });
      issues = W.issue({ kind: "danger", id: "pj-i1", title: "O jogo ficou sem memória", text: "O jogador deu só 2 GB ao jogo. Packs com muitos mods costumam precisar de 6 a 8 GB. Como o log não é deste pack, o Warden não sugere mudanças no pack.", evidence: EV_OOM("2G"), open: true });
      showCulprit = false;
    } else if (state === "incompleto") {
      match = W.versionMatch({ id: "pj-vm", level: 2, variant: "incomplete", text: "Não deu para saber a versão do pack que o jogador usa. Peça a ele o arquivo latest.log inteiro, que fica na pasta logs do jogo." });
      issues = `<div class="pro-o-conf">${W.meter("baixa")}<span class="t-sm t-2">O log veio cortado. A análise usa só o que chegou.</span></div>` + tickIssue({ low: true });
    } else {
      match = W.versionMatch({ id: "pj-vm", level: 2, variant: "exact", version: "1.4.0", date: "20/09/2026", matched: 128, total: 128, text: "A lista de mods do log é igual à da versão 1.4.0, publicada em 20/09/2026. Confirmado pelo packwiz.json da pasta do jogo: é exatamente esta versão." });
      issues = tickIssue();
    }
    const why = `<section class="panel panel--strong" aria-labelledby="pj-why"><h2 class="panel__title" id="pj-why">Por que travou</h2><div class="stack-3" style="margin-top:12px">${issues}</div></section>`;
    const actions = B("Ver o log", "", { variant: "ghost", icon: "file-text" }) + B("Apagar esta análise", "", { variant: "danger-ghost", icon: "trash-2" });
    const foot = `<p class="t-xs t-3">${state === "desconhecido" ? "Esta análise aparece em Problemas → Travamentos, com a origem Jogador." : "Esta análise aparece em Problemas → Travamentos, com a origem Jogador. “Ver o log” mostra a cópia guardada, sem dados pessoais, com as linhas da evidência marcadas."}</p>`;
    return pageHead("Travamento de um jogador", ORIGIN, actions, backProblemas) +
      `<div class="stack pro-o-page">${match}${top}${why}${ai}${showCulprit ? culprit : ""}${foot}</div>`;
  }
  def("jogador-resultado", { group: "pack", title: "Travamento de um jogador: resultado", spec: "T30, T14", v11: true,
    render: (s) => packShell("problemas", playerResult(s)),
    states: { proxima: "Versão próxima, com mods a mais no jogador", antiga: "Jogador numa versão antiga (já corrigido)", desconhecido: "Log de outro pack", incompleto: "Log sem a lista de mods" } });

  // ---------- Consentimento da IA (mesma regra do T14, texto do travamento do jogador) ----------
  function playerConsent(cancel) {
    const item = (label, desc, o = {}) => `<li class="row row--top" style="gap:10px">${o.fixed ? `<span style="padding-top:2px">${W.icon("check", "t-ok")}</span>` : ""}${o.fixed ? `<span><b>${label}</b><span class="check__desc" style="display:block">${desc}</span></span>` : W.check({ checked: true, label: `<b>${label}</b>`, desc })}</li>`;
    const m = (s) => `<mark>${esc(s)}</mark>`;
    return W.dialog({ esc: cancel, size: "lg", ai: true, title: "Começar uma conversa com a IA?", sub: "Pedido uma vez para esta conversa. Cada coisa que a IA consultar aparece na conversa, exatamente como foi enviada. Nada muda no pack sem você clicar em Aplicar.",
      body: `<div><div class="field__label" style="margin-bottom:8px">Durante esta conversa, a IA pode consultar</div><ul class="stack-2">
          ${item("Informações do pack e a lista de mods", "Versões do Minecraft, do Forge e do Java; nome, versão, lado e nota de cada mod.", { fixed: true })}
          ${item("A versão do pack que o jogador usa", "A 1.4.0, identificada pelo packwiz.json, para comparar com o pack atual.", { fixed: true })}
          ${item("Os problemas que o Warden já encontrou", "Os achados de Problemas, com as evidências.", { fixed: true })}
          ${item("O log do jogador e os logs dos seus testes", "Sempre sem dados pessoais: nome do jogador, nome de usuário do Windows, IPs, e-mails e chaves viram marcadores.", { fixed: true })}
          ${item("Configs do pack", "Arquivos de config/ e defaultconfigs/. Desmarque se não quiser.")}
          ${item("Procurar problemas parecidos nas páginas dos mods no GitHub", "Envia ao GitHub só o nome do mod e algumas palavras do erro. Nunca o log.")}</ul></div>
        <div><div class="field__label" style="margin-bottom:6px">Texto inicial, exatamente como vai</div><pre class="code code--scroll" tabindex="0" aria-label="Texto inicial que será enviado">${esc("Sintoma: travamento de um jogador (log do mclo.gs, recebido hoje às 11:02)\nVersão do pack do jogador: 1.4.0 (128 de 128 mods conferem)\nPack atual: Minecraft 1.20.1 · Forge 47.3.0 · Java 17.0.12 · 128 itens\nAchados do Warden: 4 (2 erros, 2 avisos), entre eles \"Epic Fight e Supplementaries alteram o mesmo ponto do jogo\"\nCrash report do jogador:\nDescription: Ticking entity\njava.lang.NullPointerException: Cannot invoke \"net.minecraft.world.entity.Entity.getX()\"\nEntity Name: ")}${m("<jogador>")}${esc("\nGame Directory: C:\\Users\\")}${m("<usuário>")}${esc("\\curseforge\\minecraft\\Instances\\")}${m("<instância>")}${esc("\n…(mais 72 linhas)")}</pre></div>
        <dl class="kv"><dt>Para</dt><dd>Google Gemini (${MODEL}), com a sua chave</dd><dt>Tamanho inicial</dt><dd>cerca de 28 mil tokens (a unidade que o Google usa para medir e cobrar)</dd><dt>Custo</dt><dd>Cerca de US$ 0,20 numa conversa típica no plano pago. No plano gratuito não há custo, mas o Google pode usar o conteúdo para melhorar os produtos dele, e pessoas podem ler.</dd></dl>`,
      foot: B("Cancelar", cancel, { variant: "ghost" }) + B("Começar conversa", "ia-conversa", { variant: "primary", icon: "sparkles" }) });
  }
  def("jogador-ia", { group: "pack", title: "Travamento de um jogador: conversar com a IA (consentimento)", spec: "T30, T14", v11: true, hidden: true,
    render: () => packShell("problemas", playerResult("normal"), { overlay: playerConsent("jogador-resultado") }) });

  // ======================= NOTAS E GRUPOS (T31) =======================
  // [nome, descrição, versão, fonte, lado, nota, grupos, extras]
  const GMODS = {
    Embeddium: ["Deixa o jogo mais leve e com mais quadros por segundo.", "0.3.31", "modrinth", "client", "Deixa o jogo mais leve; ficou no lugar do Rubidium", ["Performance"]],
    FerriteCore: ["Diminui a memória usada pelos blocos e modelos.", "6.0.1", "modrinth", "both", "Usa menos memória. Ajuda quem joga com 4 GB", ["Performance"]],
    ModernFix: ["Correções que deixam o jogo abrir mais rápido.", "5.19.4", "modrinth", "both", "Abre o jogo mais rápido", ["Performance"]],
    Terralith: ["Mais de 95 biomas novos, só com blocos do jogo.", "2.5.4", "modrinth", "server", "Biomas novos só com blocos do jogo", ["Geração de mundo"]],
    Tectonic: ["Montanhas mais altas, vales e ilhas.", "2.4.1", "modrinth", "server", "Montanhas mais altas e vales mais fundos", ["Geração de mundo"]],
    "YUNG's Better Dungeons": ["Masmorras maiores e mais variadas.", "4.0.4", "curseforge", "server", "", ["Geração de mundo"]],
    "Just Enough Items (JEI)": ["Mostra todos os itens e receitas do jogo.", "15.20.0.106", "curseforge", "both", "Receitas à mão: pedido dos jogadores", ["Qualidade de vida"], { update: "available", updateTo: "15.20.0.112" }],
    Jade: ["Mostra o que você está olhando, no topo da tela.", "11.12.3", "modrinth", "client", "", ["Qualidade de vida"]],
    "Mouse Tweaks": ["Arrastar e mover itens com o mouse no inventário.", "2.25.1", "curseforge", "client", "Arrastar itens com o mouse no inventário", ["Qualidade de vida"]],
    "Sophisticated Backpacks": ["Mochilas com melhorias, filtros e muito espaço.", "3.20.17", "modrinth", "both", "Mochilas para o começo do jogo", ["Qualidade de vida", "Exploração"], { kind: "new", flags: [W.tag("Não salvo", "warn")] }],
    "Xaero's Minimap": ["Minimapa no canto da tela.", "24.6.1", "curseforge", "both", "Minimapa; usar junto com o World Map", ["Exploração"]],
    Waystones: ["Pedras de teleporte entre lugares que você já visitou.", "14.1.6", "modrinth", "both", "Viagem rápida entre as vilas", ["Exploração"], { kind: "error", flags: [W.badge("danger", "Erro")] }],
  };
  const GROUPS = [["Performance", 3], ["Geração de mundo", 3], ["Qualidade de vida", 4], ["Exploração", 3], ["Decoração", 0]];
  const SEL = "Sophisticated Backpacks";
  // Na lista agrupada, o chip do próprio grupo não se repete em cada linha: só os outros grupos do mod.
  function gRow(name, inGroup) {
    const [desc, ver, src, side, note, allGroups, extra] = GMODS[name];
    const groups = (allGroups || []).filter((g) => g !== inGroup);
    const dest = name === "Embeddium" ? "mods-nota" : name === "Waystones" ? "problemas" : name.startsWith("Just") ? "mods-detalhe" : "";
    return W.modRow(Object.assign({ name, desc, ver, src, side, note, groups, update: "ok", selected: name === SEL, attrs: go(dest) }, extra || {}));
  }
  function gGroup(id, title, count, names, o = {}) {
    const open = !o.closed;
    return `<section class="listgroup pro-o-gl" aria-labelledby="${id}-h"><div class="listgroup__head"><button type="button" class="listgroup__toggle" id="${id}-h" aria-expanded="${open}" aria-controls="${id}-b">${W.icon("chevron-down")}${title}</button><span class="t-3 t-sm">${count}</span></div>
      <div id="${id}-b"${open ? "" : " hidden"}><div class="tablewrap"><table class="table">${W.modTableHead()}<tbody>${o.rows || names.map((n) => gRow(n, title)).join("")}</tbody></table></div></div></section>`;
  }
  function putPopover(groups) {
    return `<span class="pro-o-pop">${W.btn("Pôr no grupo", { size: "sm", icon: "tag", iconEnd: "chevron-down", attrs: { id: "pro-o-putbtn", "aria-expanded": "false", "aria-controls": "pro-o-put", "data-pro-o-pop": "pro-o-put" } })}
      <div class="pro-o-pop__panel" id="pro-o-put" hidden>${W.groupPicker({ id: "pro-o-gp", framed: true, title: `Pôr ${SEL} no grupo`, groups, newAttrs: go("grupos") })}</div></span>`;
  }
  function groupsToolbar(o = {}) {
    const agrupar = W.select({ bare: true, id: "pro-o-agrupar", ariaLabel: "Agrupar por", options: [["tipo", "Agrupar por: tipo"], ["grupo", "Agrupar por: grupo"], ["nenhum", "Agrupar por: nenhum"]], value: "grupo" });
    const filtro = W.select({ bare: true, id: "pro-o-fgrupo", ariaLabel: "Grupo", options: [["", "Grupo: todos"], ...(o.noGroups ? [] : GROUPS.map(([g]) => [g, "Grupo: " + g])), ["sem", "Grupo: sem grupo"]], value: o.filter || "" });
    return `<div class="toolbar"><span class="row"><span class="t-xs t-3">Ver como</span><span data-viewas>${W.segmented([["list", "Lista"], ["graph", "Grafo"]], "list", "Ver como")}</span></span>${W.input({ bare: true, icon: "search", placeholder: "Buscar no pack e nas notas", ariaLabel: "Buscar no pack e nas notas" })}${agrupar}${filtro}${W.select({ bare: true, ariaLabel: "Lado", options: ["Lado: todos", "Cliente e servidor", "Só cliente", "Só servidor"] })}${W.select({ bare: true, ariaLabel: "Mostrar", options: ["Mostrar: tudo", "Com problemas (8)", "Com atualização (5)", "Não salvos (2)", "Sem manutenção (4)"] })}${B("Grupos do pack…", "grupos", { variant: "link" })}</div>`;
  }
  function modsGroupsContent(state) {
    const head = pageHead("Mods", "128 itens: 124 mods, 3 resource packs e 1 shader. Agrupados pelos seus grupos; um mod em dois grupos aparece nos dois.", B("Verificar atualizações", "", { icon: "refresh-cw" }) + B("Adicionar", "adicionar", { variant: "primary", icon: "plus" }));
    if (state === "semgrupo") {
      return head + groupsToolbar({ noGroups: true }) +
        `<div class="pro-o-emptylist">${W.empty({ compact: true, glyph: "plus", title: "Nenhum grupo ainda", text: "Crie grupos para juntar mods pelo assunto, como Performance ou Geração de mundo.", actions: B("Novo grupo", "grupos~vazio", { variant: "primary", icon: "plus" }) })}</div>` +
        gGroup("pg-none", "Sem grupo", "128 itens", [], { closed: true, rows: "" });
    }
    if (state === "vazio") {
      return head + groupsToolbar({ filter: "Decoração" }) +
        `<div class="pro-o-emptylist">${W.empty({ compact: true, glyph: "search", title: "Nenhum mod no grupo Decoração.", text: "O grupo foi criado, mas nenhum mod foi posto nele ainda. Selecione mods na lista e use “Pôr no grupo”.", actions: B("Limpar filtro", "mods-grupos", { icon: "x" }) })}</div>`;
    }
    const sel = `<div class="selbar" style="margin-bottom:12px" role="region" aria-label="Ações para os selecionados"><span class="selbar__count">1 selecionado</span>${putPopover([["Performance", false, 3], ["Geração de mundo", false, 3], ["Qualidade de vida", true, 4], ["Exploração", true, 3], ["Decoração", false, 0]])}${B("Alterar lado", "", { size: "sm" })}${B("Atualizar", "", { size: "sm", icon: "refresh-cw" })}${B("Remover", "", { size: "sm", variant: "danger-ghost", icon: "trash-2" })}<span class="grow"></span>${B("Limpar seleção", "", { size: "sm", variant: "ghost" })}</div>`;
    return head + groupsToolbar() + sel +
      gGroup("pg-perf", "Performance", "3 mods", ["Embeddium", "FerriteCore", "ModernFix"]) +
      gGroup("pg-world", "Geração de mundo", "3 mods", ["Terralith", "Tectonic", "YUNG's Better Dungeons"]) +
      gGroup("pg-qol", "Qualidade de vida", "4 mods", ["Just Enough Items (JEI)", "Jade", "Mouse Tweaks", "Sophisticated Backpacks"]) +
      gGroup("pg-expl", "Exploração", "3 mods", ["Sophisticated Backpacks", "Xaero's Minimap", "Waystones"]) +
      gGroup("pg-none", "Sem grupo", "116 itens", [], { closed: true, rows: "" });
  }
  def("mods-grupos", { group: "pack", title: "Mods agrupados por grupo, com notas", spec: "T06, T31", v11: true,
    render: (s) => packShell("mods", modsGroupsContent(s)),
    states: { semgrupo: "Nenhum grupo criado ainda", vazio: "Filtro de grupo sem resultado" },
    after: () => document.querySelectorAll("#stage [data-viewas] [role=radio]").forEach((b) => { if (b.getAttribute("aria-checked") !== "true") b.setAttribute("data-go", "mods-grafo"); }) });

  // ---------- Detalhes do mod com "Nota e grupos" no topo ----------
  function noteDrawer(state) {
    const empty = state === "vazio", err = state === "erro";
    const name = empty ? "Sophisticated Core" : "Embeddium";
    const by = empty ? "P3pp3rF1y" : "embeddedt";
    const top = err
      ? W.alert({ kind: "warn", title: "Não foi possível ler as notas e os grupos do pack.", text: "O arquivo .warden/mods.toml tem um erro na linha 7. A lista e os detalhes funcionam sem as notas e os grupos até o arquivo ser corrigido.", actionsBelow: true, actions: B("Ver detalhes", "", { size: "sm" }) })
      : empty
        ? W.noteBlock({ id: "pro-o-note", state: "empty", groups: [] }) + `<p class="pro-o-why">${W.icon("info", "icon--sm")}<span><b>Por que está no pack:</b> exigido pelo Sophisticated Backpacks. O Warden adicionou junto, em 01/10/2026.</span></p>`
        : W.noteBlock({ id: "pro-o-note", state: "filled", note: "Deixa o jogo mais leve; substitui o Rubidium", groups: ["Performance"] });
    const sec = `<section aria-labelledby="pro-o-sec-h"><h3 class="field__label" id="pro-o-sec-h">Segurança do arquivo</h3><div class="stack-2" style="margin-top:6px">${W.secStatus("oficial", "Confere com o arquivo oficial do Modrinth")}
      <p class="t-xs t-3">Conferido hoje às 14:38. Nenhum sinal de programa malicioso conhecido.</p>
      <details class="disclosure"><summary>Detalhes técnicos</summary><dl class="kv" style="margin-top:6px"><dt>SHA-512</dt><dd class="t-mono" style="word-break:break-all">${empty ? "4c1e9a07…b2d3" : "9f2b6e41…7a0c"}</dd><dt>Conferido com</dt><dd>api.modrinth.com (só o hash é enviado)</dd></dl></details></div></section>`;
    const info = empty
      ? `<dl class="kv"><dt>Versão instalada</dt><dd class="t-mono">0.6.26</dd><dt>Para</dt><dd>Minecraft 1.20.1 · Forge</dd><dt>Arquivo</dt><dd><span class="path">sophisticatedcore-1.20.1-0.6.26.jar</span> <span class="t-3">· 1,9 MB</span></dd><dt>Usado por</dt><dd>${B("Sophisticated Backpacks", "", { variant: "link" })} <span class="t-3">· obrigatória</span></dd></dl>`
      : `<dl class="kv"><dt>Versão instalada</dt><dd class="t-mono">0.3.31</dd><dt>Para</dt><dd>Minecraft 1.20.1 · Forge</dd><dt>Arquivo</dt><dd><span class="path">embeddium-0.3.31+mc1.20.1.jar</span> <span class="t-3">· 1,1 MB</span></dd><dt>Depende de</dt><dd class="t-3">Nenhum mod (só o Forge)</dd><dt>Por que está no pack</dt><dd>Você adicionou em 28/09/2026</dd></dl>`;
    const sideSel = W.select({ id: "pro-o-side", label: "Lado", options: [["both", "Cliente e servidor"], ["client", "Só cliente"], ["server", "Só servidor"]], value: empty ? "both" : "client", hint: "Onde o mod precisa estar instalado. Informado pelo Modrinth." });
    return W.drawer({ esc: "mods", id: "pro-o-drw", title: "Detalhes do mod",
      body: `<div class="row row--gap-3">${W.tile(name, "xl")}<div><div class="t-display-lg">${name}</div><div class="t-xs t-3 row" style="margin-top:4px">por ${by} · ${W.source("modrinth")} ${B("Abrir página", "", { variant: "link", iconEnd: "external-link" })}</div></div></div>
        ${top}${sec}${info}${sideSel}`,
      foot: B("Trocar de versão…", "", { variant: "ghost" }) + B("Remover", "remover", { variant: "danger-ghost", icon: "trash-2" }) });
  }
  function modsBehind(state) {
    const top = state === "erro" ? `<div class="mods-banner">${W.alert({ kind: "warn", compact: true, title: "Não foi possível ler as notas e os grupos do pack.", text: "A lista funciona sem eles.", actions: B("Ver detalhes", "", { size: "sm" }) })}</div>` : "";
    return window.P.modsContent("normal", { top });
  }
  def("mods-nota", { group: "pack", title: "Detalhes do mod: nota e grupos", spec: "T07, T31", v11: true,
    render: (s) => packShell("mods", modsBehind(s), { overlay: noteDrawer(s) }),
    states: { vazio: "Sem nota (mod trazido como dependência)", erro: ".warden/mods.toml ilegível" } });

  // ---------- Diálogo "Grupos do pack" ----------
  function groupsDialog(state) {
    const none = state === "vazio";
    const list = none
      ? `<p class="t-sm t-2"><b>Nenhum grupo ainda.</b> Crie grupos para juntar mods pelo assunto, como Performance ou Geração de mundo.</p>`
      : `<ul class="pro-o-glist">${GROUPS.map(([g, n]) => `<li class="pro-o-grow">${W.groupChip(g)}<span class="t-sm t-3">${n} ${n === 1 ? "mod" : "mods"}</span><span class="grow"></span>${B("Renomear", "", { size: "sm", variant: "ghost", icon: "pencil", attrs: { "aria-label": `Renomear o grupo ${g}` } })}${B("Apagar", "", { size: "sm", variant: "danger-ghost", icon: "trash-2", attrs: { "aria-label": `Apagar o grupo ${g}` } })}</li>`).join("")}</ul>
        <p class="t-xs t-3">Apagar um grupo não remove nenhum mod: os mods continuam no pack; só saem do grupo.</p>`;
    const add = `<div class="field pro-o-newgroup"><label class="field__label" for="pro-o-newg">Novo grupo</label><div class="row">${W.input({ id: "pro-o-newg", bare: true, placeholder: "Ex.: Magia", attrs: { maxlength: 32, "aria-describedby": "pro-o-newg-hint", "aria-label": null } })}${B("Criar grupo", "", { icon: "plus" })}</div><div class="field__hint" id="pro-o-newg-hint">Até 32 caracteres, sem repetir o nome de outro grupo.</div></div>`;
    return W.dialog({ esc: "mods-grupos", id: "pro-o-gdlg", title: "Grupos do pack", sub: "Grupos juntam mods pelo assunto. Um mod pode estar em vários grupos. Os grupos e as notas são só seus: ficam no pack, mas não vão para os jogadores.",
      body: `<div class="stack-3">${list}</div>${add}`, foot: B("Fechar", "mods-grupos", { variant: "ghost" }) });
  }
  def("grupos", { group: "pack", title: "Grupos do pack (diálogo)", spec: "T31", v11: true,
    render: (s) => packShell("mods", modsGroupsContent(s === "vazio" ? "semgrupo" : "normal"), { overlay: groupsDialog(s) }),
    states: { vazio: "Nenhum grupo criado" } });

  // ======================= DESEMPENHO ENTRE VERSÕES (T33) =======================
  const PERF = {
    time: { unit: "s", label: "Tempo para abrir", values: [["1.2.0", "18/08/2026", 84, 2], ["1.3.0", "02/09/2026", 91, 3], ["1.4.0", "20/09/2026", 98, 4], ["1.4.1", "24/09/2026", null, 0], ["1.4.2", "28/09/2026", 104, 3]] },
    memory: { unit: "GB", label: "Memória máxima", values: [["1.2.0", "18/08/2026", 4.1, 2], ["1.3.0", "02/09/2026", 4.4, 3], ["1.4.0", "20/09/2026", 4.6, 4], ["1.4.1", "24/09/2026", null, 0], ["1.4.2", "28/09/2026", 4.8, 3]] },
    tick: { unit: "ms", label: "Tempo por tick", values: [["1.2.0", "18/08/2026", null, 0], ["1.3.0", "02/09/2026", null, 0], ["1.4.0", "20/09/2026", 31, 2], ["1.4.1", "24/09/2026", null, 0], ["1.4.2", "28/09/2026", 33, 2]] },
  };
  const toVals = (arr) => arr.map(([version, date, value, tests]) => ({ version, date, value, tests }));
  function perfControls(o = {}) {
    const show = `<span class="row"><span class="t-sm t-2" id="pro-o-show-l">Mostrar</span><span data-pro-o-show>${W.segmented([["time", "Tempo para abrir"], ["memory", "Memória máxima"], ["tick", "Tempo por tick"]], "time", "Mostrar")}</span></span>`;
    const prof = `<span class="row"><label class="t-sm t-2" for="pro-o-perfil">Perfil do teste</label>${W.select({ bare: true, id: "pro-o-perfil", options: [["padrao", "Padrão (6 GB · Java automático)"], ["fraco", "PC fraco (4 GB)"], ["shaders", "Shaders (8 GB)"]], value: o.profile || "padrao" })}</span>`;
    return `<div class="pro-o-controls" role="group" aria-label="Opções do gráfico">${show}${prof}</div>`;
  }
  function charts(sets, o = {}) {
    return Object.keys(PERF).map((k) => {
      const p = PERF[k];
      const vals = toVals((sets && sets[k]) || p.values);
      return `<div data-pro-o-metric="${k}"${k === "time" ? "" : " hidden"}>${W.versionChart({ values: vals, unit: p.unit, label: p.label, highlight: k === "time" ? o.highlight : undefined, caption: k === "tick" ? "Mediana dos testes em que você salvou um perfil do spark. As barras começam do zero." : "Mediana dos testes comparáveis de cada versão salva. As barras começam do zero." })}</div>`;
    }).join("");
  }
  const LEFT_OUT = `<details class="disclosure"><summary>5 testes ficaram de fora · Ver por quê</summary><ul class="stack-2 t-sm" style="margin-top:6px">
      <li><b>2 testes</b> com o perfil PC fraco <span class="t-3">· outro perfil: a memória e os ajustes mudam o resultado</span></li>
      <li><b>1 teste</b> foi a primeira abertura depois de recriar a instância de teste <span class="t-3">· a primeira abertura é sempre mais lenta</span></li>
      <li><b>1 teste</b> travou antes de carregar</li>
      <li><b>1 teste</b> com perfil de desempenho <span class="t-3">· o registro detalhado deixa o carregamento mais lento</span></li></ul></details>`;
  const PRIVACY = `<p class="pro-o-why t-xs">${W.icon("lock", "icon--sm")}<span>Os números vêm só dos seus testes neste computador. Nada disso sai do seu computador.</span></p>`;
  function perfContent(state) {
    const head = pageHead("Desempenho entre versões", "Quanto cada versão salva demora para abrir e quanta memória usa, com os seus testes normais neste computador.", null, { back: ["Histórico", "historico"], sans: true });
    if (state === "vazio") {
      return head + `<div class="stack pro-o-page">${W.empty({ glyph: "clock", title: "Ainda não há testes comparáveis", text: "Cada teste normal deste pack entra aqui.", actions: B("Testar", "teste-checagem", { variant: "primary", icon: "play" }) })}${PRIVACY}</div>`;
    }
    if (state === "perfil") {
      return head + `<div class="stack pro-o-page">${perfControls({ profile: "fraco" })}${W.empty({ compact: true, glyph: "search", title: "Nenhum teste com o perfil PC fraco neste computador.", text: "Só entram testes com o mesmo perfil, porque a memória e os ajustes mudam o resultado. Escolha o perfil PC fraco no menu ▾ do Testar e teste o pack.", actions: B("Voltar para o perfil Padrão", "desempenho", { icon: "arrow-left" }) })}${PRIVACY}</div>`;
    }
    if (state === "carregando") {
      return head + `<div class="stack pro-o-page">${perfControls()}${W.versionChart({ loading: true })}${PRIVACY}</div>`;
    }
    let notice, body, left = LEFT_OUT;
    if (state === "poucos") {
      const only = (k, v, n) => PERF[k].values.map(([ver, d], i) => [ver, d, i === 4 ? v : null, i === 4 ? n : 0]);
      body = charts({ time: only("time", 104, 2), memory: only("memory", 4.8, 2), tick: only("tick", 33, 1) });
      notice = W.alert({ kind: "info", title: "Salve outra versão e teste para comparar.", text: "Por enquanto só a 1.4.2 tem testes comparáveis neste computador. Agora (não salvo) tem 1 teste: poucos testes para comparar. Teste de novo para confirmar.", actions: B("Testar", "teste-checagem", { size: "sm", icon: "play" }) });
      left = `<p class="t-sm t-3">Nenhum teste ficou de fora.</p>`;
    } else if (state === "pesada") {
      const add = (k, v, n) => [...PERF[k].values, ["1.5.0", "02/10/2026", v, n]];
      body = charts({ time: add("time", 126, 3), memory: add("memory", 5.1, 3), tick: add("tick", 34, 2) }, { highlight: 5 });
      notice = W.alert({ kind: "warn", icon: "trending-up", title: "A versão 1.5.0 está mais pesada", text: "Abre em 2 min 6 s, 21% mais lenta que a 1.4.2 (1 min 44 s), comparando 3 testes de cada versão com o perfil Padrão neste computador.", actions: B("Testar com perfil de desempenho", "teste-fechou~desempenho", { size: "sm", icon: "gauge" }) });
    } else {
      body = charts();
      notice = W.alert({ kind: "warn", icon: "trending-up", title: "Agora (não salvo) abre em 2 min 6 s, 21% mais lenta que a 1.4.2", text: "Mediana de 2 testes das alterações não salvas, contra 3 testes da 1.4.2 (1 min 44 s), com o perfil Padrão neste computador. Testes de alterações não salvas não viram uma barra: entram no gráfico quando você salvar a versão.", actions: B("Testar com perfil de desempenho", "teste-fechou~desempenho", { size: "sm", icon: "gauge" }) });
    }
    return head + `<div class="stack pro-o-page">${perfControls()}${notice}${body}${left}${PRIVACY}</div>`;
  }
  def("desempenho", { group: "pack", title: "Desempenho entre versões", spec: "T33", v11: true,
    render: (s) => packShell("historico", perfContent(s), s === "pesada" ? { unsaved: 0, version: "1.5.0" } : {}),
    states: { carregando: "Lendo os testes", vazio: "Nenhum teste comparável", poucos: "Só uma versão com testes", perfil: "Perfil PC fraco sem testes", pesada: "Versão salva mais pesada" } });

  // ---------- Comportamento local (só nestas telas) ----------
  // "Mostrar": troca o gráfico (modo de exibição, não aba)
  document.addEventListener("warden:segment", (e) => {
    const box = e.target.closest("[data-pro-o-show]");
    if (!box) return;
    document.querySelectorAll("#stage [data-pro-o-metric]").forEach((el) => { el.hidden = el.dataset.proOMetric !== e.detail.value; });
  });
  document.addEventListener("change", (e) => {
    const t = e.target;
    if (!t.closest || !t.closest("#stage")) return;
    if (t.id === "pro-o-agrupar") navigate(t.value === "grupo" ? "mods-grupos" : "mods");
    if (t.id === "pro-o-fgrupo") { if (t.value === "Decoração") navigate("mods-grupos", "vazio"); else if (t.value === "") navigate("mods-grupos"); }
    if (t.id === "pro-o-perfil") navigate("desempenho", t.value === "fraco" ? "perfil" : "normal");
  });
  // "Pôr no grupo ▾": abre e fecha o seletor de grupos
  function closePop(refocus) {
    const p = document.getElementById("pro-o-put");
    if (!p || p.hidden) return;
    p.hidden = true;
    const b = document.getElementById("pro-o-putbtn");
    if (b) { b.setAttribute("aria-expanded", "false"); if (refocus) b.focus(); }
  }
  document.addEventListener("click", (e) => {
    const b = e.target.closest && e.target.closest("[data-pro-o-pop]");
    if (b) {
      const p = document.getElementById(b.dataset.proOPop);
      if (p) { const open = p.hidden; p.hidden = !open; b.setAttribute("aria-expanded", String(open)); if (open) { const f = p.querySelector("input"); if (f) f.focus(); } }
      return;
    }
    if (!(e.target.closest && e.target.closest("#pro-o-put"))) closePop(false);
  });
  document.addEventListener("keydown", (e) => { if (e.key === "Escape" && e.target.closest && e.target.closest("#pro-o-put")) { e.stopPropagation(); closePop(true); } });
})();
