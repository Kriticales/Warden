/* Telas do nível do APP: primeira execução, Meus packs, criar e abrir pack, Configurações, Tarefas. */
(function () {
  "use strict";
  const W = window.W, D = window.DATA;
  const { def, go, B, appShell, pageHead } = window.P;

  // ---------- Primeira execução ----------
  const WELCOME = ["Aviso", "Nome do jogador", "Pasta dos packs", "Chaves (opcional)"];
  function welcome(step, state) {
    let body = "", foot = "";
    if (step === 0) {
      body = `<h1 class="t-display-2xl" tabindex="-1" data-title>Boas-vindas ao <span class="t-hl">Warden</span></h1>
        <p class="t-2 measure" style="margin-top:12px">O Warden cria, testa e versiona modpacks no formato packwiz. Para testar, ele abre o Minecraft em modo offline, só neste computador.</p>
        <div class="legal" style="margin-top:20px">NÃO É UM PRODUTO OFICIAL DO MINECRAFT. NÃO É APROVADO PELA MOJANG OU PELA MICROSOFT NEM ASSOCIADO A ELAS.</div>
        <p class="t-sm t-2" style="margin-top:12px">Você precisa possuir o Minecraft: Java Edition. O Warden não pede login da Mojang nem da Microsoft.</p>`;
      foot = `<span></span>${B("Entendi", "boas-vindas-2", { variant: "primary" })}`;
    } else if (step === 1) {
      const bad = state === "erro";
      body = `<h1 class="pagehead__title" tabindex="-1" data-title>Nome do jogador nos testes</h1><p class="pagehead__sub">Aparece no jogo e no chat do mundo de teste. Não é a sua conta: o teste é offline.</p>
        <div style="margin-top:20px;max-width:420px">${W.input({ id: "wl-name", label: "Nome do jogador", value: bad ? "Jogador Teste!" : "Jogador", error: bad ? "Use só letras, números e _ (sem espaço), de 3 a 16 caracteres." : null, hint: bad ? null : "Letras, números e _, de 3 a 16 caracteres." })}</div>`;
      foot = `${B("Voltar", "boas-vindas", { variant: "ghost" })}${B("Próximo", bad ? "" : "boas-vindas-3", { variant: "primary", disabled: bad })}`;
    } else if (step === 2) {
      body = `<h1 class="pagehead__title" tabindex="-1" data-title>Pasta dos packs</h1><p class="pagehead__sub">Cada pack vira uma pasta aqui dentro. Dá para mudar depois em Configurações.</p>
        <div style="margin-top:20px" class="field"><label class="field__label" for="wl-dir">Pasta</label><div class="row">${W.input({ id: "wl-dir", bare: true, value: "C:\\Users\\[você]\\Documents\\Warden", mono: true })}${B("Escolher…", "", { icon: "folder-open" })}</div>
        <p class="field__hint">Os dados do próprio Warden (instâncias de teste, Java, cache) ficam em outra pasta, fora dos packs.</p></div>`;
      foot = `${B("Voltar", "boas-vindas-2", { variant: "ghost" })}${B("Próximo", "boas-vindas-4", { variant: "primary" })}`;
    } else {
      body = `<h1 class="pagehead__title" tabindex="-1" data-title>Chaves</h1><p class="pagehead__sub">Todas são opcionais. Sem elas o Warden funciona; só algumas partes ficam de fora.</p>
        <div class="stack" style="margin-top:20px">
          ${W.input({ id: "wl-cf", label: "Chave da CurseForge", type: "password", placeholder: "Cole a chave", hint: "Sem ela, a busca mostra só o Modrinth." })}
          ${W.input({ id: "wl-gm", label: "Chave do Gemini", type: "password", placeholder: "Cole a chave", hint: "Usada só no Diagnóstico com IA, e sempre com a sua confirmação antes de cada envio." })}
          ${W.input({ id: "wl-gh", label: "Token do GitHub", type: "password", placeholder: "Cole o token", hint: "Usado para publicar versões para os jogadores." })}
          <fieldset class="field" style="border:0;padding:0;margin:0"><legend class="field__label" style="margin-bottom:6px">Onde guardar as chaves</legend><div class="choice-list choice-list--2">
            ${W.choice({ name: "wl-store", title: "Cofre do Windows", badge: " " + W.tag("Recomendado", "ok"), desc: "Protegido pelo seu usuário do Windows.", checked: true })}
            ${W.choice({ name: "wl-store", title: "Arquivo .env", desc: "Texto comum na pasta de dados do Warden. Outros programas conseguem ler." })}</div></fieldset></div>`;
      foot = `${B("Voltar", "boas-vindas-3", { variant: "ghost" })}<div class="row">${B("Pular, configuro depois", "packs-vazio", { variant: "ghost" })}${B("Concluir", "packs-vazio", { variant: "primary" })}</div>`;
    }
    const aside = `<aside class="welcome__aside" aria-label="Onde ficam as coisas"><div class="panel__title" style="font-size:var(--text-md)">Onde ficam as coisas</div><dl class="kv" style="margin-top:12px;grid-template-columns:1fr"><dt>Seus packs</dt><dd class="path">Documents\\Warden</dd><dt>Dados do Warden</dt><dd class="path">%APPDATA%\\dev.kriticales.warden</dd><dt>Chaves</dt><dd>Cofre do Windows (ou .env, se você escolher)</dd></dl><p class="t-xs t-3" style="margin-top:12px">Nada disso vai para o GitHub sem você publicar uma versão.</p></aside>`;
    return appShell(`<div class="welcome"><div class="wizard">${W.steps(WELCOME, step, { label: "Etapas da primeira execução" })}${body}<div class="wizard__foot">${foot}</div></div>${aside}</div>`, { where: "Primeira execução", noSettings: true });
  }
  def("boas-vindas", { group: "app", title: "Primeira execução: aviso", spec: "T01", render: () => welcome(0) });
  def("boas-vindas-2", { group: "app", title: "Primeira execução: nome do jogador", spec: "T01", hidden: true, render: (s) => welcome(1, s), states: { erro: "Nome com espaço e símbolo" } });
  def("boas-vindas-3", { group: "app", title: "Primeira execução: pasta", spec: "T01", hidden: true, render: () => welcome(2) });
  def("boas-vindas-4", { group: "app", title: "Primeira execução: chaves", spec: "T01", hidden: true, render: () => welcome(3) });

  // ---------- Meus packs ----------
  function packsContent(state, o = {}) {
    const actions = B("Abrir ou importar…", "abrir-existente", { icon: "folder-open", tip: "Pack packwiz, .mrpack, zip da CurseForge ou instância do Prism" }) + B("Criar pack", "criar-1", { variant: "primary", icon: "plus" });
    if (state === "vazio") {
      return pageHead("Meus packs", null) + W.empty({ title: "Você ainda não tem packs", text: "Crie um pack do zero, abra um pack packwiz que já está no computador ou importe um modpack de outro app (.mrpack, zip da CurseForge, instância do Prism).", actions: B("Criar pack", "criar-1", { variant: "primary", icon: "plus" }) + B("Abrir ou importar…", "abrir-existente", { icon: "folder-open" }), glyph: "plus" });
    }
    const head = pageHead("Meus packs", state === "carregando" ? "Lendo a pasta Documents\\Warden…" : `${D.PACKS.length + 1} packs na pasta Documents\\Warden`, actions);
    const tools = `<div class="packs-tools">${W.input({ bare: true, icon: "search", placeholder: "Buscar pack", ariaLabel: "Buscar pack" })}${W.select({ bare: true, ariaLabel: "Ordenar", options: ["Ordenar: alterados recentemente", "Ordenar: nome", "Ordenar: último teste"] })}</div>`;
    if (state === "carregando") return head + tools + `<div class="tablewrap"><table class="table" aria-busy="true">${W.packTableHead({ withHealth: true })}<tbody>${[0, 1, 2].map((i) => `<tr aria-hidden="true"><td><div class="row row--gap-3"><span class="skeleton skeleton--tile" style="width:40px;height:40px"></span><div class="grow"><span class="skeleton skeleton--line" style="--w:${50 + i * 10}%;height:12px"></span><span class="skeleton skeleton--line" style="--w:70%"></span></div></div></td>${[60, 80, 40, 30, 50].map((w) => `<td><span class="skeleton skeleton--line" style="--w:${w}%"></span></td>`).join("")}<td><span class="skeleton skeleton--btn" style="--w:90px"></span></td></tr>`).join("")}</tbody></table></div>`;
    const rows = D.PACKS.map((p, i) => W.packRow(p, { openAttrs: go(p.go || ""), menuAttrs: i === 0 ? { "aria-haspopup": "menu", "aria-expanded": o.menu ? "true" : "false", "aria-controls": "menu-pack" } : go("") })).join("") +
      W.packRow({ name: "Antigo Survival", kind: "missing", path: "D:\\Packs\\antigo-survival" }, { locateAttrs: go(""), removeAttrs: go(""), withHealth: true }) +
      (state === "erro" ? W.packRow({ name: "Pack do Lucas", kind: "unreadable" }, { detailAttrs: go(""), removeAttrs: go(""), withHealth: true }) : "");
    const menu = W.menu([{ label: "Mostrar na pasta", icon: "folder-open", attrs: go("") }, { label: "Remover da lista", desc: "Não apaga nenhum arquivo", icon: "x", attrs: go("") }, "sep", { label: "Apagar pack…", desc: "Vai para a lixeira, com o histórico junto", icon: "trash-2", danger: true, end: W.badge("p1", "P1"), attrs: go("") }], { id: "menu-pack", label: "Ações do pack Vale Sereno", style: "right:8px;top:96px" });
    const err = state === "erro" ? W.alert({ kind: "danger", title: "Não foi possível ler 1 pack.", text: "O pack.toml do “Pack do Lucas” tem um erro na linha 4. Os outros packs não são afetados.", cls: "mods-banner" }) : "";
    return head + err + tools + `<div style="position:relative"><div class="tablewrap"><table class="table">${W.packTableHead({ withHealth: true })}<tbody>${rows}</tbody></table></div>${menu}</div>`;
  }
  def("packs", { group: "app", title: "Meus packs", spec: "T02", render: (s) => appShell(packsContent(s) + (s === "normal" ? window.P.sim("Abrir ou importar…:", [["escolher uma pasta packwiz", "abrir-existente"], ["escolher um .mrpack", "importar"]]) : "")), states: { carregando: "Lendo a pasta dos packs", vazio: "Nenhum pack ainda", erro: "Um pack ilegível na lista" } });
  def("packs-vazio", { group: "app", title: "Meus packs (vazio)", spec: "T02", hidden: true, render: () => appShell(packsContent("vazio")) });
  def("packs-menu", { group: "app", title: "Meus packs: menu ⋯ de um pack", spec: "T02", hidden: true, render: () => appShell(packsContent("normal", { menu: true })) });

  // ---------- Criar pack ----------
  const CREATE = ["Nome e pasta", "Versão do Minecraft", "Loader", "Mods iniciais", "Resumo"];
  function create(step, state) {
    let body = "", next = "criar-" + (step + 2), nextLabel = "Próximo", nextDisabled = false;
    if (step === 0) {
      const bad = state === "erro";
      body = `<div class="stack" style="max-width:560px">${W.input({ id: "cr-name", label: "Nome do pack", value: "Vale Sereno" })}${W.input({ id: "cr-author", label: "Autor", value: "Kriticales" })}
        ${W.input({ id: "cr-desc", label: "Descrição", optional: true, placeholder: "Uma frase sobre o pack" })}
        <div class="field"><label class="field__label" for="cr-dir">Pasta</label><div class="row">${W.input({ id: "cr-dir", bare: true, value: "Documents\\Warden\\vale-sereno", mono: true, attrs: bad ? { "aria-invalid": "true", "aria-describedby": "cr-dir-err" } : {} })}${B("Escolher…", "", { icon: "folder-open" })}</div>
        ${bad ? `<div class="field__error" id="cr-dir-err">${W.icon("circle-alert", "icon--sm")}<span>Esta pasta já tem arquivos. Escolha uma pasta vazia ou mude o nome do pack.</span></div>` : '<p class="field__hint">Criada agora, vazia. O Warden não grava nada fora dela.</p>'}</div></div>`;
      nextDisabled = bad;
    } else if (step === 1) {
      if (state === "carregando") body = `<div style="max-width:560px">${W.progress({ label: "Carregando a lista de versões do Minecraft", value: null })}</div>`;
      else if (state === "erro") body = W.empty({ kind: "error", glyph: "x", title: "Não foi possível carregar as versões do Minecraft", text: "Sem internet e sem uma cópia guardada da lista. Verifique a conexão e tente de novo.", actions: B("Tentar de novo", "criar-2", { icon: "refresh-cw" }), compact: true });
      else body = `<div style="max-width:560px"><div class="row" style="margin-bottom:8px">${W.input({ bare: true, icon: "search", placeholder: "Buscar versão (ex.: 1.20)", ariaLabel: "Buscar versão" })}${W.switchCtl({ label: "Mostrar snapshots" })}</div>
        <fieldset style="border:0;padding:0;margin:0"><legend class="sr-only">Versão do Minecraft</legend><div class="verlist">${D.MC_VERSIONS.map((v) => W.check({ type: "radio", name: "cr-mc", label: `<span class="t-mono">${v}</span>`, checked: v === "1.20.1", desc: v === "1.20.1" ? "Forge, NeoForge e Fabric disponíveis" : v === "1.7.10" ? "Só Forge · Java 8" : v === "1.12.2" ? "Só Forge · Java 8" : "" })).join("")}
          <div class="t-xs t-3" style="padding:8px 12px">Versões anteriores a 1.7.10 aparecem com “melhor esforço”: podem não abrir.</div></div></fieldset>
        <p class="field__hint" style="margin-top:8px">Ordem de lançamento, da mais nova para a mais antiga. Mudar a versão do Minecraft depois é criar outro pack.</p></div>`;
      nextDisabled = state === "erro" || state === "carregando";
    } else if (step === 2) {
      body = `<div style="max-width:620px"><p class="t-sm t-2" style="margin-bottom:12px">Só aparecem os loaders que existem para o Minecraft 1.20.1.</p><fieldset style="border:0;padding:0;margin:0"><legend class="sr-only">Loader</legend><div class="choice-list">
        ${W.choice({ name: "cr-ld", title: "Forge", badge: ` <span class="t-mono t-sm t-2">47.3.0</span> ${W.tag("Recomendada", "ok")}`, desc: "O loader com mais mods para o 1.20.1.", checked: true, extra: `<div style="margin-top:6px">${B("Escolher outra versão do Forge", "", { variant: "link" })}</div>` })}
        ${W.choice({ name: "cr-ld", title: "NeoForge", badge: ' <span class="t-mono t-sm t-2">47.1.106</span>', desc: "Continuação do Forge. Existe a partir do Minecraft 1.20.1." })}
        ${W.choice({ name: "cr-ld", title: "Fabric", badge: ' <span class="t-mono t-sm t-2">0.16.5</span>', desc: "Leve, com mods diferentes dos do Forge." })}
        ${W.choice({ name: "cr-ld", title: "Nenhum (vanilla)", desc: "Só resource packs e shaders." })}
        ${W.choice({ name: "cr-ld", title: "Quilt", desc: "O Warden ainda não suporta o Quilt.", disabled: true })}</div></fieldset>
        <p class="field__hint" style="margin-top:10px">${W.icon("coffee", "icon--sm")} O teste vai usar o Java 17, o mais novo que o Minecraft 1.20.1 com Forge aceita.</p></div>`;
    } else if (step === 3) {
      const old = state === "antiga";
      const mc = old ? "1.12.2" : "1.20.1", ld = old ? "Forge 14.23.5.2860" : "Forge 47.3.0";
      body = `<div style="max-width:680px" class="stack"><p class="t-sm t-2">Mods que ajudam você a testar e ajudam quem joga quando algo dá errado. Vêm marcados; desmarque o que não quiser. Tudo entra pelo diálogo de dependências, como qualquer mod.</p>
        <fieldset style="border:0;padding:0;margin:0"><legend class="field__label" style="margin-bottom:8px">Recomendados para Minecraft ${mc} com ${ld}</legend><div class="dlist" role="list">
          <div class="drow drow--compact" role="listitem"><span class="drow__box">${W.check({ checked: true, ariaLabel: "spark" })}</span><span class="drow__main"><span class="drow__line"><b>spark</b> <span class="t-mono t-xs t-3">${old ? "1.6.3" : "1.10.187"}</span></span><span class="drow__desc">Mede o que deixa o jogo lento. Lado: cliente e servidor.</span>${old ? `<span class="drow__meta">${W.tag("Versão antiga do spark, sem atualizações", "warn", { icon: "triangle-alert" })}</span>` : ""}</span><span class="drow__end">${W.source(old ? "curseforge" : "modrinth")}</span></div>
          <div class="drow drow--compact" role="listitem"><span class="drow__box">${W.check({ checked: true, ariaLabel: "Crash Assistant" })}</span><span class="drow__main"><span class="drow__line"><b>Crash Assistant</b> <span class="t-mono t-xs t-3">1.11.14</span></span><span class="drow__desc">Mostra uma janela clara para o jogador quando o jogo trava. O envio de dados ao autor do mod vem desligado. Fica fora dos seus testes normais.</span></span><span class="drow__end">${W.source("modrinth")}</span></div></div></fieldset>
        <fieldset style="border:0;padding:0;margin:0"><legend class="field__label" style="margin-bottom:8px">Kit de desempenho <span class="opt">(opcional)</span></legend>
          <div class="drow drow--compact"><span class="drow__box">${W.check({ ariaLabel: "Adicionar o kit de desempenho" })}</span><span class="drow__main"><span class="drow__line"><b>${old ? "Desempenho (Forge 1.12.2)" : "Desempenho (Forge 1.20.1)"}</b></span><span class="drow__desc">${old ? "VintageFix, FoamFix, Phosphor e mais 1. Mods que deixam o jogo mais leve sem mudar como ele é jogado." : "Embeddium, FerriteCore, ModernFix e mais 2. Mods que deixam o jogo mais leve sem mudar como ele é jogado."}</span></span><span class="drow__end">${B("Ver o que vem no kit", "", { variant: "link" })}</span></div></fieldset>
        <p class="field__hint">Você confirma cada item no diálogo de dependências ao criar o pack.</p></div>`;
    } else {
      body = `<div style="max-width:620px" class="stack"><div class="packcard">${W.tile("Vale Sereno", "xl")}<div class="packcard__name">Vale Sereno</div><div class="packcard__meta">Minecraft 1.20.1 · Forge 47.3.0 · por Kriticales</div>
        <dl class="packcard__facts"><div><dt>Versão inicial</dt><dd>0.1.0</dd></div><div><dt>Java do teste</dt><dd>17 (baixado no primeiro teste)</dd></div><div><dt>Pasta</dt><dd class="path">Documents\\Warden\\vale-sereno</dd></div></dl></div>
        <div class="panel"><div class="panel__title panel__title--sans">Arquivos que o Warden vai criar</div><ul class="checklist" style="margin-top:8px">${["pack.toml e index.toml (o pack em si, no formato packwiz)", ".gitattributes (impede o Windows de mudar finais de linha e quebrar o pack)", ".packwizignore (só entra no pack o que é conteúdo)", "Mods iniciais: spark e Crash Assistant, com a config do Crash Assistant sem envio de dados ao autor", "Primeira versão salva no histórico"].map((t) => `<li class="is-ok">${W.icon("check")}<span>${t}</span></li>`).join("")}</ul></div></div>`;
      next = "pack-novo"; nextLabel = "Criar pack";
    }
    const prev = step === 0 ? B("Cancelar", "packs", { variant: "ghost" }) : B("Voltar", step === 1 ? "criar-1" : "criar-" + step, { variant: "ghost" });
    return appShell(`<div class="wizard">${pageHead("Criar pack", null)}${W.steps(CREATE, step, { label: "Etapas de criar pack" })}${body}<div class="wizard__foot">${prev}${B(nextLabel, nextDisabled ? "" : next, { variant: "primary", disabled: nextDisabled, icon: step === 4 ? "plus" : null })}</div></div>`, { back: ["Meus packs", "packs"], where: "Criar pack" });
  }
  def("criar-1", { group: "app", title: "Criar pack: nome e pasta", spec: "T03", render: (s) => create(0, s), states: { erro: "Pasta de destino não vazia" } });
  def("criar-2", { group: "app", title: "Criar pack: versão do Minecraft", spec: "T03", hidden: true, render: (s) => create(1, s), states: { carregando: "Carregando versões", erro: "Sem internet e sem cache" } });
  def("criar-3", { group: "app", title: "Criar pack: loader", spec: "T03", hidden: true, render: () => create(2) });
  def("criar-4", { group: "app", title: "Criar pack: mods iniciais", spec: "T03", d4: true, render: (s) => create(3, s), states: { antiga: "Minecraft 1.12.2: spark antigo" } });
  def("criar-5", { group: "app", title: "Criar pack: resumo", spec: "T03", hidden: true, render: () => create(4) });

  // ---------- Abrir pack existente ----------
  function openExisting(state) {
    const head = pageHead("Abrir pack", "Pasta packwiz em D:\\Packs\\meu-pack-antigo", null);
    if (state === "carregando") return appShell(head + `<div style="max-width:620px">${W.progress({ label: "Lendo o pack", value: null, meta: "pack.toml, index.toml e 92 arquivos do índice" })}</div>`, { back: ["Meus packs", "packs"], where: "Abrir ou importar" });
    if (state === "erro") return appShell(head + `<div style="max-width:720px">${W.alert({ kind: "danger", title: "Este pack usa Quilt, que o Warden ainda não suporta.", text: "Nada foi alterado na pasta. Packs Forge, NeoForge e Fabric abrem normalmente.", actions: B("Escolher outra pasta", "", { size: "sm", icon: "folder-open" }) })}</div>`, { back: ["Meus packs", "packs"], where: "Abrir ou importar" });
    const files = [["config/create-common.toml.bak", "4 KB", "cópia de segurança"], ["packwiz-installer-bootstrap.jar", "58 KB", "ferramenta, não conteúdo"], [".packwiz.toml", "1 KB", "arquivo de outro programa"]];
    return appShell(head + `<div class="stack" style="max-width:820px">
      <div class="packcard">${W.tile("Meu pack antigo", "xl")}<div class="packcard__name">Meu pack antigo</div><div class="packcard__meta">Minecraft 1.20.1 · NeoForge 47.1.106</div>
        <dl class="packcard__facts"><div><dt>Itens</dt><dd>86 mods · 2 resource packs</dd></div><div><dt>Histórico</dt><dd>12 versões, a mais nova é 1.2.0</dd></div><div><dt>Loader</dt><dd>${W.status("ok", "suportado")}</dd></div></dl></div>
      <div class="panel panel--strong"><div class="panel__head"><h2 class="panel__title panel__title--sans">3 arquivos não deveriam ir para quem joga o pack</h2></div>
        <div class="tablewrap"><table class="table table--plain"><thead><tr><th class="shrink"><span class="sr-only">Limpar</span></th><th>Arquivo</th><th class="num">Tamanho</th><th>Motivo</th></tr></thead><tbody>${files.map(([f, sz, why]) => `<tr><td>${W.check({ checked: true, ariaLabel: "Limpar " + f })}</td><td class="path">${f}</td><td class="num">${sz}</td><td class="t-2">${why}</td></tr>`).join("")}</tbody></table></div>
        <p class="t-sm t-3" style="margin-top:10px">Antes de limpar, o Warden guarda um ponto de segurança. Nada some sem volta.</p>
        <div class="btn-row btn-row--end" style="margin-top:12px">${B("Agora não", "mods", { variant: "ghost" })}${B("Limpar 3 arquivos e abrir", "mods", { variant: "primary" })}</div></div></div>`, { back: ["Meus packs", "packs"], where: "Abrir ou importar" });
  }
  // ---------- Importar modpack de outro app (D4) ----------
  function importPage(state) {
    const sh = (c) => appShell(c, { back: ["Meus packs", "packs"], where: "Abrir ou importar" });
    const head = pageHead("Importar modpack", "Vira um pack packwiz novo. O arquivo original não muda.", null);
    if (state === "carregando") return sh(head + `<div style="max-width:620px">${W.progress({ label: "Lendo Create+ 6.0.0.mrpack", value: 46, meta: "Identificando 168 arquivos no Modrinth e na CurseForge pelo hash" })}</div>`);
    if (state === "erro") return sh(pageHead("Importar modpack", "Cozy Create 2.3.0.zip · zip da CurseForge", null) + `<div style="max-width:760px">${W.alert({ kind: "warn", icon: "key-round", title: "Para importar um zip da CurseForge, o Warden precisa da sua chave da CurseForge.", text: "O zip só traz os números dos mods; a chave é usada para saber quais mods são. Nada foi gravado.", actions: B("Abrir Configurações", "config-app", { size: "sm" }) + B("Escolher outro arquivo", "", { size: "sm", variant: "ghost" }) })}</div>`);
    const rows = [
      ["circle-check", "ok", "154", "viram referência ao Modrinth", "Baixados pelo jogador direto do Modrinth, como num pack feito no Warden."],
      ["circle-check", "ok", "9", "jars de overrides/mods identificados pelo hash", "Também viram referência (8 do Modrinth, 1 da CurseForge)."],
      ["triangle-alert", "warn", "5", "jars ficam como arquivo local", "Não foram achados nas lojas. Vão dentro do pack; confira se a licença de cada um permite."],
      ["triangle-alert", "warn", "3", "mods com lado desconhecido", "O arquivo diz “unknown”. Entram como Cliente e servidor e ficam marcados para você conferir."],
      ["triangle-alert", "warn", "2", "arquivos em client-overrides/", "O packwiz não tem esse conceito: entram no pack para todos, com aviso."],
    ];
    return sh(pageHead("Importar modpack", "Create+ 6.0.0.mrpack · Minecraft 1.21.1 · NeoForge 21.1.252", null) + `<div class="stack" style="max-width:860px">
      ${W.alert({ kind: "info", icon: "scroll-text", title: "Licença do modpack: All Rights Reserved.", text: "Os mods são referências livres. As configs e os arquivos do modpack são trabalho do autor: use para você, não publique como se fosse seu." })}
      <div class="field"><label class="field__label" for="im-dir">Pasta do pack novo</label><div class="row">${W.input({ id: "im-dir", bare: true, value: "Documents\\Warden\\create-plus", mono: true })}${B("Escolher…", "", { icon: "folder-open" })}</div></div>
      <section class="panel panel--strong" aria-labelledby="im-h"><h2 class="panel__title panel__title--sans" id="im-h">O que acontece com cada arquivo</h2>
        <div class="tablewrap" style="margin-top:10px"><table class="table table--plain"><tbody>${rows.map(([ic, k, n, what, why]) => `<tr><td class="shrink">${W.icon(ic, "t-" + k)}</td><td class="num t-strong shrink">${n}</td><td><b>${what}</b><div class="t-xs t-3">${why}</div></td></tr>`).join("")}</tbody></table></div></section>
      <section class="panel" aria-labelledby="im-lixo"><h2 class="panel__title panel__title--sans" id="im-lixo">Lixo que não entra no pack</h2>
        <ul class="checklist" style="margin-top:8px"><li class="is-ok">${W.icon("x")}<span><span class="path">overrides/.mixin.out/</span> · 812 arquivos de depuração de mixin</span></li><li class="is-ok">${W.icon("x")}<span><span class="path">xmcl.json</span> · arquivo de outro launcher</span></li><li class="is-ok">${W.icon("x")}<span><span class="path">mods/.connector/temp/</span> · 1 arquivo de cache</span></li></ul></section>
      <div class="btn-row btn-row--end">${B("Cancelar", "packs", { variant: "ghost" })}${B("Importar como pack novo", "importar-pronto", { variant: "primary", icon: "import" })}</div></div>`);
  }
  def("importar", { group: "app", title: "Importar modpack (.mrpack)", spec: "T04", d4: true, render: importPage, states: { carregando: "Lendo o arquivo", erro: "Zip da CurseForge sem chave" } });
  def("importar-pronto", { group: "app", title: "Importado: o que revisar", spec: "T04", d4: true, hidden: true, render: () => importPage("normal"),
    after: () => window.WardenUI.toast(W.toast({ kind: "ok", title: "Create+ importado como pack novo", text: "8 itens marcados para revisar aparecem em Problemas.", actions: B("Abrir o pack", "mods", { size: "sm" }) }), 9000) });

  def("abrir-existente", { group: "app", title: "Abrir pack (pasta packwiz)", spec: "T04", render: openExisting, states: { carregando: "Lendo o pack", erro: "Pack Quilt (não suportado)" } });

  // ---------- Configurações ----------
  function settings(o = {}) {
    const keyRow = (name, configured, hint) => `<div class="keyrow"><span class="t-strong">${name}</span>${configured ? `<span class="row"><span class="t-mono t-sm t-2">••••••••${configured}</span>${W.status("ok", "funcionando")}</span>` : W.input({ bare: true, type: "password", placeholder: "Cole a chave", ariaLabel: "Chave " + name })}
      <span class="row">${configured ? B("Testar", "", { size: "sm" }) + B("Substituir", "", { size: "sm" }) + B("Remover", "", { size: "sm", variant: "danger-ghost" }) : B("Salvar", "", { size: "sm", variant: "primary" }) + B(hint, "", { size: "sm", variant: "link" })}</span></div>`;
    const env = !!o.env;
    const javaTable = `<div class="tablewrap" style="margin-top:8px"><table class="table"><thead><tr><th>Java</th><th>Usado por</th><th>Por que esta versão</th></tr></thead><tbody>${D.JAVAS.map(([j, who, why]) => `<tr><td class="t-strong">${j}</td><td class="${who.startsWith("nenhum") ? "t-3" : ""}">${who}</td><td class="t-2">${why}</td></tr>`).join("")}</tbody></table></div>`;
    return appShell(`<div class="settings">${pageHead("Configurações", "Valem para o Warden inteiro. O que é de um pack fica dentro do pack.")}
      <section class="panel" aria-labelledby="st-geral"><h2 class="panel__title" id="st-geral">Geral</h2><div class="stack" style="margin-top:12px">
        <div class="field"><label class="field__label" for="st-dir">Pasta dos packs</label><div class="row">${W.input({ id: "st-dir", bare: true, value: "Documents\\Warden", mono: true })}${B("Escolher…", "", { icon: "folder-open" })}</div></div>
        ${W.input({ id: "st-player", label: "Nome do jogador nos testes", value: "Jogador", hint: "Aparece no jogo de teste. O teste é offline." })}
        <div>${B("Rever boas-vindas", "boas-vindas", { variant: "link" })}</div></div></section>
      <section class="panel" aria-labelledby="st-keys"><h2 class="panel__title" id="st-keys">Chaves e contas</h2><p class="t-sm t-2" style="margin-top:4px">Depois de salvas, as chaves não aparecem de novo: só os últimos 4 caracteres.</p>
        <div style="margin-top:8px">${keyRow("CurseForge", "3f2a")}${keyRow("Gemini", "7b40")}${keyRow("GitHub", "9c1e")}</div>
        <fieldset style="border:0;padding:0;margin:16px 0 0"><legend class="field__label" style="margin-bottom:8px">Onde guardar as chaves</legend><div class="choice-list choice-list--2">
          <span ${env ? `data-go="config-app"` : ""}>${W.choice({ name: "st-store", title: "Cofre do Windows", badge: " " + W.tag("Recomendado", "ok"), desc: "Gerenciador de Credenciais, protegido pelo seu usuário do Windows.", checked: !env })}</span>
          <span ${env ? "" : `data-go="chaves-env"`}>${W.choice({ name: "st-store", title: "Arquivo .env", desc: "%APPDATA%\\dev.kriticales.warden\\.env, fora dos packs e do GitHub.", checked: env })}</span></div></fieldset>
        ${env ? `<div style="margin-top:12px">${W.alert({ kind: "warn", title: "As chaves estão num arquivo de texto.", text: "Qualquer programa no seu computador, e qualquer backup ou sincronização dessa pasta, consegue ler o .env.", actions: B("Voltar para o cofre", "config-app", { size: "sm", icon: "shield" }) })}</div>` : ""}</section>
      <section class="panel" aria-labelledby="st-test"><h2 class="panel__title" id="st-test">Teste</h2><div class="stack" style="margin-top:12px">
        ${W.select({ id: "st-mem", label: "Memória padrão", options: ["Automático (pelo número de mods)", "4 GB", "6 GB", "8 GB"], hint: "Cada pack pode ter o seu ajuste no menu ▾ do Testar." })}
        <div><div class="field__label">Java</div><p class="t-sm t-2" style="margin-top:4px">O Warden usa sempre o Java mais novo que cada versão do Minecraft aceita, com a atualização mais recente. Quando não é o mais novo de todos, a tabela explica o porquê.</p>${javaTable}
          <div class="btn-row" style="margin-top:8px">${B("Procurar atualizações do Java", "", { size: "sm", icon: "refresh-cw" })}${B("Remover Javas sem uso", "", { size: "sm", variant: "ghost" })}</div></div>
        ${W.switchCtl({ label: "Mostrar versões beta e alpha dos mods" })}
        ${W.select({ id: "st-upd", label: "Verificar atualizações dos mods", options: ["Ao abrir o pack", "A cada 24 horas", "Só quando eu pedir"] })}</div></section>
      <section class="panel" aria-labelledby="st-ed"><h2 class="panel__title" id="st-ed">Editor de configs</h2><div style="margin-top:12px">${W.check({ label: "Mostrar as diferenças antes de salvar", checked: true })}</div></section>
      <section class="panel" aria-labelledby="st-look"><h2 class="panel__title" id="st-look">Aparência</h2><div style="margin-top:12px">${W.switchCtl({ label: "Menos movimento", stateText: true, offText: "Seguindo o Windows" })}<p class="field__hint" style="margin-top:6px">Para o pulso do Testar e as animações. Por padrão segue a opção de animações do Windows.</p></div></section>
      <section class="panel" aria-labelledby="st-priv"><h2 class="panel__title" id="st-priv">Privacidade e registros</h2><div class="row row--wrap" style="margin-top:12px">${W.select({ id: "st-log", label: "Nível de detalhe dos registros", options: ["Normal", "Detalhado (para relatar um problema)"] })}<div style="align-self:end">${B("Abrir pasta de registros", "", { icon: "folder-open" })}</div></div>
        <p class="field__hint" style="margin-top:8px">Os registros do Warden nunca guardam chaves. Antes de qualquer envio à IA, você vê o texto exato.</p></section>
      <section class="panel" aria-labelledby="st-disk"><h2 class="panel__title" id="st-disk">Armazenamento ${W.badge("p1", "P1")}</h2><dl class="kv" style="margin-top:12px"><dt>Cache de downloads</dt><dd>1,8 GB</dd><dt>Minecraft e loaders</dt><dd>2,4 GB</dd><dt>Javas</dt><dd>610 MB</dd><dt>Instâncias de teste</dt><dd>3,1 GB</dd></dl><div style="margin-top:10px">${B("Limpar cache", "", { size: "sm" })}</div></section>
      <section class="panel" aria-labelledby="st-about"><h2 class="panel__title" id="st-about">Sobre o Warden</h2><dl class="kv" style="margin-top:12px"><dt>Versão</dt><dd>0.1.0</dd><dt>packwiz embutido</dt><dd class="t-mono">9066bf8</dd></dl>
        <div class="legal" style="margin-top:12px">NÃO É UM PRODUTO OFICIAL DO MINECRAFT. NÃO É APROVADO PELA MOJANG OU PELA MICROSOFT NEM ASSOCIADO A ELAS.</div>
        <div class="btn-row" style="margin-top:10px">${B("Licenças de terceiros", "", { variant: "link" })}${B("Apoiar o Forge", "", { variant: "link" })}</div></section></div>`,
      { back: ["Meus packs", "packs"], where: "Configurações", noSettings: true, overlay: o.overlay });
  }
  const envDialog = () => W.dialog({ esc: "config-app", title: "Guardar as chaves num arquivo .env?", body: `<p>As chaves da CurseForge, do Gemini e do GitHub saem do cofre do Windows e vão para o arquivo <span class="path">%APPDATA%\\dev.kriticales.warden\\.env</span>.</p>
      ${W.alert({ kind: "warn", title: "O .env é um arquivo de texto comum.", text: "Qualquer programa no seu computador, e qualquer backup ou sincronização dessa pasta, consegue ler as chaves. O cofre do Windows protege as chaves com o seu usuário." })}
      <p class="t-sm t-3">O arquivo fica fora dos packs e nunca vai para o GitHub. Dá para voltar ao cofre quando quiser: as chaves voltam e o .env é apagado.</p>`, foot: B("Manter no cofre", "config-app", { variant: "ghost" }) + B("Usar arquivo .env", "config-env", { variant: "primary" }) });
  def("config-app", { group: "app", title: "Configurações (com Sobre)", spec: "T21, T23", render: () => settings() });
  def("chaves-env", { group: "app", title: "Chaves: trocar para .env", spec: "T21", hidden: true, render: () => settings({ overlay: envDialog() }) });
  def("config-env", { group: "app", title: "Configurações com chaves no .env", spec: "T21", hidden: true, render: () => settings({ env: true }) });

  // ---------- Tarefas (gaveta do rodapé) ----------
  function tasksDrawer(esc) {
    return W.drawer({ esc, title: "Tarefas", body: `<div class="t-caps t-3">Em andamento</div>
      <div class="panel">${W.progress({ label: "Baixando o Minecraft 1.20.1", value: 38, size: "sm", meta: "Vale Sereno · 160 de 420 MB · cerca de 2 minutos" })}<div style="margin-top:8px">${B("Cancelar", "", { size: "sm", variant: "ghost" })}</div></div>
      <div class="t-caps t-3">Concluídas</div>
      <ul class="checklist"><li class="is-ok">${W.icon("circle-check")}<span>Verificar atualizações · Leve e Bonito · 14:01</span></li><li class="is-ok">${W.icon("circle-check")}<span>Exportar · Vale Sereno · ontem, 21:40</span></li><li class="is-bad">${W.icon("circle-x")}<span>Publicar versão · Técnico Clássico · sem conexão com o GitHub. ${B("Ver detalhes", "", { variant: "link" })}</span></li></ul>
      <p class="t-xs t-3">Mostra as últimas 50 tarefas.</p>` });
  }
  def("tarefas", { group: "app", title: "Tarefas (gaveta do rodapé)", spec: "T22", render: () => appShell(packsContent("normal"), { overlay: tasksDrawer("packs"), tasks: { state: "busy", text: "1 tarefa em andamento" } }) });
  window.P.tasksDrawer = tasksDrawer;
})();
