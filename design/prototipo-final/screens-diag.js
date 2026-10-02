/* ✦ Diagnóstico com IA (D4): conversas. Consentimento uma vez por conversa, cada envio visível
   no chat, evidências conferidas pelo Warden e propostas que só valem com o clique em Aplicar. */
(function () {
  "use strict";
  const W = window.W;
  const { def, go, B, packShell, pageHead } = window.P;
  const MODEL = "gemini-3.8-flash";

  // ---------- Seção: nova conversa e lista de conversas ----------
  function iaContent(state) {
    const head = pageHead(`${W.icon("sparkles", "icon--lg icon--ai")} Diagnóstico com IA`, "Converse com a IA (Google Gemini) sobre um problema do pack. Ela consulta o pack, os logs e os testes; você vê tudo o que é enviado, e nada muda no pack sem você clicar em Aplicar.");
    if (state === "vazio") return head + W.empty({ artKind: "ai", glyph: "dots", title: "Falta a chave do Gemini", text: "Para conversar com a IA, informe a sua chave do Gemini em Configurações. A chave é sua: o Google pode cobrar pelo uso, conforme o seu plano.", actions: B("Abrir Configurações", "config-app", { variant: "primary", icon: "key-round" }) + B("Como conseguir uma chave", "", { variant: "link" }) });
    const start = `<section class="panel panel--strong" aria-labelledby="ia-new"><h2 class="panel__title panel__title--sans" id="ia-new">Nova conversa</h2>
      <fieldset style="border:0;padding:0;margin:12px 0 0"><legend class="field__label" style="margin-bottom:6px">O que está acontecendo?</legend><div class="choice-list choice-list--2">
        ${W.choice({ name: "ia-sym", title: "Trava ao abrir o jogo", desc: "Fecha antes do menu principal." })}${W.choice({ name: "ia-sym", title: "Trava ao entrar no mundo", desc: "Abre, mas fecha ao carregar ou jogar.", checked: true })}
        ${W.choice({ name: "ia-sym", title: "Fica lento", desc: "Quadros por segundo baixos ou travadinhas." })}${W.choice({ name: "ia-sym", title: "Outra coisa", desc: "Conte no campo abaixo." })}</div></fieldset>
      <div class="grid-2" style="margin-top:12px"><div>${W.select({ id: "ia-src", label: "Começar pelo", options: ["Travamento de 30/09 às 21:14 (causa não encontrada)", "Último teste: hoje às 14:40 (travou)", "Um log ou crash report do computador…", "Só o pack, sem log"] })}</div>
        <div>${W.textarea({ id: "ia-note", label: "Contar algo à IA", optional: true, placeholder: "Ex.: trava quando entro no Nether", rows: 2 })}</div></div>
      <div class="row row--gap-3" style="margin-top:12px">${B("Começar conversa", "ia-consent", { variant: "primary", icon: "sparkles" })}<span class="t-sm t-3">Antes, você vê o que a IA poderá consultar e confirma. Isso é pedido uma vez por conversa.</span></div></section>`;
    const list = `<h2 class="group-title">Conversas</h2><div class="tablewrap"><table class="table"><thead><tr><th>Conversa</th><th>Quando</th><th>Situação</th><th class="shrink"><span class="sr-only">Ações</span></th></tr></thead><tbody>
      <tr><td><b>Trava ao entrar no mundo (Ticking entity)</b><div class="t-xs t-3">Travamento de 30/09 · ~182 mil tokens</div></td><td class="t-2">hoje, 15:40</td><td>${W.status("warn", "Aberta: 1 proposta esperando você")}</td><td class="shrink"><div class="row">${B("Continuar", "ia-conversa", { size: "sm" })}${B("Apagar conversa", "", { size: "sm", variant: "ghost", iconOnly: true, icon: "trash-2" })}</div></td></tr>
      <tr><td><b>Embeddium e Rubidium juntos</b><div class="t-xs t-3">Teste de 28/09</div></td><td class="t-2">28/09, 19:20</td><td>${W.status("ok", "Resolvida: Rubidium removido")}</td><td class="shrink"><div class="row">${B("Continuar", "ia-conversa~mudou", { size: "sm" })}${B("Apagar conversa", "", { size: "sm", variant: "ghost", iconOnly: true, icon: "trash-2" })}</div></td></tr>
      <tr><td><b>Falta de memória</b><div class="t-xs t-3 path">crash-2026-09-22.txt</div></td><td class="t-2">22/09, 18:02</td><td>${W.status("ok", "Resolvida: memória do teste em 8 GB")}</td><td class="shrink"><div class="row">${B("Continuar", "", { size: "sm" })}${B("Apagar conversa", "", { size: "sm", variant: "ghost", iconOnly: true, icon: "trash-2" })}</div></td></tr></tbody></table></div>
      <p class="t-xs t-3" style="margin-top:8px">As conversas ficam nos dados do Warden, neste computador. Nunca vão para a pasta do pack nem para o GitHub.</p>
      <div class="panel row row--between" style="margin-top:16px"><span class="t-sm">${W.status("ok", "Chave do Gemini configurada")} <span class="t-3">· modelo ${MODEL}</span></span>${B("Trocar em Configurações", "config-app", { variant: "link" })}</div>`;
    return head + start + list;
  }
  def("ia", { group: "pack", title: "Diagnóstico com IA: conversas", spec: "T14", d4: true, render: (s) => packShell("ia", iaContent(s)), states: { vazio: "Sem chave do Gemini" } });

  // ---------- Consentimento: uma vez por conversa ----------
  function consentDialog(cancel) {
    const item = (label, desc, o = {}) => `<li class="row row--top" style="gap:10px">${o.fixed ? `<span style="padding-top:2px">${W.icon("check", "t-ok")}</span>` : ""}${o.fixed ? `<span><b>${label}</b><span class="check__desc" style="display:block">${desc}</span></span>` : W.check({ checked: true, label: `<b>${label}</b>`, desc })}</li>`;
    return W.dialog({ esc: cancel, size: "lg", ai: true, title: "Começar uma conversa com a IA?", sub: "Pedido uma vez para esta conversa. Cada coisa que a IA consultar aparece na conversa, exatamente como foi enviada. Nada muda no pack sem você clicar em Aplicar.",
      body: `<div><div class="field__label" style="margin-bottom:8px">Durante esta conversa, a IA pode consultar</div><ul class="stack-2">
          ${item("Informações do pack e a lista de mods", "Versões do Minecraft, do Forge e do Java; nome, versão e lado de cada mod.", { fixed: true })}
          ${item("Os problemas que o Warden já encontrou", "Os achados de Problemas, com as evidências.", { fixed: true })}
          ${item("Logs e crash reports dos seus testes", "Sempre sem dados pessoais: nome de usuário do Windows, nome do jogador, IPs, e-mails e chaves viram marcadores.", { fixed: true })}
          ${item("Configs do pack", "Arquivos de config/ e defaultconfigs/. Desmarque se não quiser.")}
          ${item("Procurar problemas parecidos nas páginas dos mods no GitHub", "Envia ao GitHub só o nome do mod e algumas palavras do erro. Nunca o log.")}</ul></div>
        <div><div class="field__label" style="margin-bottom:6px">Texto inicial, exatamente como vai</div><pre class="code code--scroll" tabindex="0" aria-label="Texto inicial que será enviado">Sintoma: trava ao entrar no mundo
Pack: Minecraft 1.20.1 · Forge 47.3.0 · Java 17.0.12 · 128 itens
Achados do Warden: 4 (2 erros, 2 avisos), entre eles "Epic Fight e Supplementaries alteram o mesmo ponto do jogo"
Travamento de 30/09 21:14, 3ª vez com a mesma causa:
java.lang.NullPointerException: Cannot invoke "net.minecraft.world.entity.Entity.getX()"
Caminho: C:\\Users\\<mark>[usuário]</mark>\\AppData\\Local\\dev.kriticales.warden\\instances\\vale-sereno
…(mais 64 linhas)</pre></div>
        <dl class="kv"><dt>Para</dt><dd>Google Gemini (${MODEL}), com a sua chave</dd><dt>Tamanho inicial</dt><dd>cerca de 31 mil tokens (a unidade que o Google usa para medir e cobrar)</dd><dt>Custo</dt><dd>Cerca de US$ 0,20 numa conversa típica no plano pago. No plano gratuito não há custo, mas o Google pode usar o conteúdo para melhorar os produtos dele, e pessoas podem ler.</dd></dl>`,
      foot: B("Cancelar", cancel, { variant: "ghost" }) + B("Começar conversa", "ia-conversa", { variant: "primary", icon: "sparkles" }) });
  }
  window.P.consentDialog = consentDialog;
  def("ia-consent", { group: "pack", title: "IA: consentimento da conversa", spec: "T14", d4: true, render: () => packShell("ia", iaContent("normal"), { overlay: consentDialog("ia") }) });

  // ---------- A conversa ----------
  const PW_DIFF = W.diff({ file: "mods/supplementaries.pw.toml", lines: [["ctx", 1, 1, 'name = "Supplementaries"'], ["del", 2, "", 'filename = "supplementaries-1.20-2.8.17.jar"'], ["add", "", 2, 'filename = "supplementaries-1.20-2.8.21.jar"'], ["fold", "", "", "… e o hash e o endereço da versão nova"]] });
  function conversation(state) {
    const loading = state === "carregando", applied = state === "aplicada", err = state === "erro", changed = state === "mudou";
    const tools = [
      W.toolCall({ what: "visão geral do pack", size: "2,1 KB", sent: "Minecraft 1.20.1 · Forge 47.3.0 · Java 17.0.12 · memória 6 GB\n128 itens: 124 mods, 3 resource packs, 1 shader\nSaúde 33 (Crítico) · último teste: travou hoje às 14:40", back: "A IA recebeu o resumo acima." }),
      W.toolCall({ what: "crash report de 30/09 21:14, linhas 1 a 80 (sem dados pessoais)", size: "6,8 KB", sent: "---- Minecraft Crash Report ----\nDescription: Ticking entity\n\njava.lang.NullPointerException: Cannot invoke \"net.minecraft.world.entity.Entity.getX()\"\n\tat net.minecraft.world.entity.LivingEntity.travel(LivingEntity.java:2108)\n\tat yesman.epicfight.world.capabilities.entitypatch.LivingEntityPatch.onTravel(LivingEntityPatch.java:412)\n…\nMixins in Stacktrace:\n\tnet.minecraft.world.entity.LivingEntity:\n\t\tepicfight.mixins.json:MixinLivingEntity\n\t\tsupplementaries-common.mixins.json:LivingEntityMixin", back: "Linhas 1 a 80 do arquivo crash-2026-09-30_21.14.02-server.txt." }),
      W.toolCall({ what: "o que o Epic Fight altera no jogo", size: "1,4 KB", sent: "Epic Fight 20.9.4: 214 alterações em 3 configs.\nMesmo ponto que outro mod: LivingEntity#travel (@Redirect), também Supplementaries 2.8.17 (@Redirect). Risco alto." }),
      W.toolCall({ kind: "github", what: "busca de issues no Supplementaries com “NullPointerException travel”", size: "0,1 KB", sent: "GET https://api.github.com/search/issues?q=repo:MehVahdJukaar/Supplementaries+is:issue+NullPointerException+travel", back: "2 issues: <b>#3121</b> “NPE in LivingEntity.travel with Epic Fight” (fechada em 12/09, corrigida na 2.8.21) e <b>#2987</b> (outro erro, aberta)." }),
    ];
    const answer = `<div class="ai-block__label">Causa provável</div><ul class="claims">
        ${W.claim("O travamento acontece quando o Epic Fight e o Supplementaries trocam, ao mesmo tempo, a mesma chamada no movimento das criaturas.", [W.evidence("crash report #L41"), W.evidence("raio-x: LivingEntity#travel")])}
        ${W.claim("O autor do Supplementaries corrigiu isso na versão 2.8.21. O pack está com a 2.8.17.", [W.evidence("issue #3121 do Supplementaries"), W.evidence("changelog 2.8.21")])}
        ${W.claim("O problema também aparece sem o Epic Fight em servidores com muitos jogadores.", [W.evidence("", { unverified: true })], { unverified: true })}</ul>
      <div class="row row--wrap row--gap-4"><div><div class="ai-block__label">Confiança</div>${W.meter("alta")}</div><div><div class="ai-block__label">Mods envolvidos</div><div class="row">${B("Epic Fight", "mods-raio-x", { variant: "link" })}${B("Supplementaries", "mods-raio-x", { variant: "link" })}</div></div></div>
      ${W.alert({ kind: "neutral", compact: true, icon: "info", title: "A IA pode errar.", text: "Afirmações sem evidência conferida aparecem riscadas como “não verificado”. Confira antes de mudar o pack." })}`;
    const props = W.proposal({ level: 2, title: "Atualizar Supplementaries 2.8.17 → 2.8.21", text: "A versão nova corrige o travamento com o Epic Fight.", body: PW_DIFF, evidence: W.evidence("issue #3121") + W.evidence("changelog 2.8.21"), state: applied ? "applied" : "pending", appliedText: "Aplicada às 15:52 · ponto de segurança criado", appliedActions: B("Testar agora", "teste-checagem", { size: "sm", icon: "play" }) + B("Desfazer", "ia-conversa", { size: "sm", variant: "ghost", icon: "undo-2" }),
        actions: B("Aplicar", "ia-conversa~aplicada", { variant: "primary", icon: "check" }) + B("Descartar", "", { variant: "ghost" }) }) +
      W.proposal({ level: 2, icon: "target", title: "Encontrar o mod culpado para confirmar", text: "Se o travamento continuar depois de atualizar, a busca do culpado testa em rodadas e confirma quais mods causam o problema. Leva cerca de 30 minutos.", evidence: W.evidence("travou 3 vezes com a mesma causa"), actions: B("Começar a busca", "culpado-config", { icon: "target" }) + B("Descartar", "", { variant: "ghost" }) });
    const aiMsg = loading
      ? W.chatMsg({ from: "ai", meta: `${MODEL} · agora`, body: tools.slice(0, 2).join("") + W.toolCall({ state: "running", what: "o que o Epic Fight altera no jogo" }) + `<div class="row t-sm t-2"><span class="loader t-ai" aria-hidden="true"><i></i><i></i><i></i><i></i></span><span>A IA está lendo. Costuma levar de 10 a 40 segundos; você pode sair desta tela.</span></div>` })
      : err ? W.chatMsg({ from: "ai", meta: `${MODEL} · 15:41`, body: tools.slice(0, 2).join("") + W.alert({ kind: "danger", title: "O Google recusou: a cota da sua chave acabou.", text: "Nada foi alterado no pack. A conversa fica guardada; dá para continuar quando a cota voltar (no plano gratuito, ela renova todo dia).", actions: B("Tentar de novo", "ia-conversa", { size: "sm", icon: "refresh-cw" }) }) })
      : W.chatMsg({ from: "ai", meta: `${MODEL} · 15:41`, body: tools.join("") + answer + props });
    const changedAlert = changed ? W.alert({ kind: "warn", title: "O pack mudou desde esta conversa.", text: "3 mods foram atualizados e 1 removido depois de 28/09. As respostas podem não valer mais.", actions: B("Começar nova conversa", "ia-consent", { size: "sm", icon: "sparkles" }) + B("Continuar mesmo assim", "", { size: "sm", variant: "ghost" }) }) : "";
    const composer = `<div class="composer">${W.textarea({ id: "ia-more", label: "Perguntar mais", placeholder: "Ex.: e se eu remover o Epic Fight em vez de atualizar?", rows: 2 })}${B("Enviar", "", { variant: "primary", icon: "send", disabled: loading })}</div>`;
    const side = `<aside class="chatpage__side" aria-label="Sobre esta conversa"><div class="panel"><dl class="kv"><dt>Começou</dt><dd>hoje, 15:40</dd><dt>Modelo</dt><dd>${MODEL}</dd><dt>Esta conversa</dt><dd>~182 mil tokens</dd><dt>A IA pode ler</dt><dd>pack, achados, logs, configs e issues no GitHub</dd></dl></div>
      <p class="t-xs t-3">Tudo o que foi enviado está nos blocos “Enviado à IA” e “Enviado ao GitHub”, byte a byte.</p>${B("Apagar esta conversa", "ia", { size: "sm", variant: "danger-ghost", icon: "trash-2" })}</aside>`;
    return pageHead("Trava ao entrar no mundo (Ticking entity)", "Conversa sobre o travamento de 30/09 às 21:14", null, { back: ["Conversas", "ia"], sans: true }) +
      `<div class="chatpage"><div class="chat">${changedAlert}${W.chatMsg({ from: "user", meta: "15:40", body: "<p>Trava quando entro no mundo, às vezes logo, às vezes depois de alguns minutos. Já aconteceu 3 vezes.</p><p class='t-xs t-3'>Sintoma: trava ao entrar no mundo · começar pelo travamento de 30/09</p>" })}${aiMsg}${composer}</div>${side}</div>`;
  }
  def("ia-conversa", { group: "pack", title: "Conversa com a IA (ferramentas e propostas)", spec: "T14", d4: true, render: (s) => packShell("ia", conversation(s), s === "aplicada" ? { unsaved: 6 } : {}), states: { carregando: "A IA está consultando", aplicada: "Proposta aplicada", erro: "Cota da chave esgotada", mudou: "O pack mudou desde a conversa" } });
})();
