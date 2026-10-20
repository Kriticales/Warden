---
version: 1
slug: "apps-desktop-src-features-diagnostics-ai"
primary_target: "apps/desktop/src/features/diagnostics/ai"
related_targets: []
---

# Sensor Sculk (seção do pack; antes "✦ Diagnóstico com IA")

Escopo: a seção Sensor Sculk do pack aberto (lista de conversas + conversa + nova conversa + consentimento + Detalhes). Modo: **Operate**. Mundo herdado: Deep Dark (design/system/tokens.css e components.css); nada de mundo novo.

Nome (decisão do dono, 20/10/2026): a seção se chama **Sensor Sculk** e a IA, dentro do app, se chama **Sculk** (masculino: "o Sculk"). Rótulos: "✦ Sculk" para quem responde, "O Sculk consultou N coisas", "Enviado ao Sculk", "O Sculk pode errar", "O que o Sculk poderá consultar", "O Sculk pode ler". O ✦ continua marcando o que é IA, e só isso. Provedor e modelo (Claude · Opus, Gemini · Flash) continuam visíveis no painel Detalhes e no consentimento, para ficar claro qual serviço o Sculk está usando.

Público e tarefa: o dono (leigo, exigente), janela maximizada 1920×1080. Abre a seção depois de um travamento, começa ou continua uma conversa, confere o que foi enviado e aplica (ou descarta) uma proposta com a diferença à vista. Frequência: algumas vezes por semana, sempre em momento de frustração com o jogo travado.

Restrições: consentimento uma vez por conversa; cada consulta do Sculk visível byte a byte; nada muda sem **Aplicar**; "O Sculk pode errar. Confira antes de mudar o pack." fixo; ✦ só para IA; sem abas; seletor "Procurar rápido / Procurar a fundo" só com Claude e OpenAI (o Gemini não tem).

## Direction contract

THESIS: uma conversa de trabalho auditável, não um chat de IA. Recusa o arranjo de "assistente mágico" (balões arredondados centralizados, tela vazia com sugestões, raciocínio escondido): o que foi enviado, o que voltou e o que muda no pack ficam no mesmo fio, com o peso de um console.

OWN-WORLD: abismo #050d12, painéis em azul-petróleo (surface-1/2), osso #cfe3da só para o que é IA (fio ai-line, fundo ai-soft, ✦), ciano #19d3e0 só para ação principal e seleção, foco em osso. Blocos retos com degrau de pixel nos botões e selos, slots afundados nos campos e no item selecionado, moldura dupla em diálogo e painel. Pixelify só em títulos curtos; Manrope no texto; Plex Mono em versões, tamanhos, caminhos e logs.

STORY: o dono entende por que travou (afirmação + selo de evidência), acredita porque vê exatamente o que o Sculk leu (blocos "Enviado ao Sculk" abertos byte a byte) e age clicando Aplicar num cartão com a diferença; depois Testa de novo.

FIRST VIEWPORT: barra de título fina sem marca; cabeçalho do pack (ícone, Zoonicraft, meta com versões em mono; Salvar versão · 16 alterações; ▶ Testar ▾ à direita); trilho de 56 px com as 6 seções (Sensor Sculk aceso, ícone ✦); coluna Conversas de 300 px (busca, grupos Hoje / Esta semana, selos de estado); conversa no resto, título em uma linha com reticências e Detalhes à direita, mensagens numa coluna de leitura de 820 px centralizada; compositor fixo embaixo com Enviar como ação principal.

FORM: Clássico de desktop, posição 1 de 6 da 2ª rodada. seed:e1e63ab2c9403a01. Interação-assinatura: o grupo "✦ O Sculk consultou 9 coisas · 14 KB enviados" que se abre em camadas até o byte enviado, e o selo de evidência que abre o trecho certo; movimento só de estado (painel entra 24 px da direita, 320 ms ease-out; "Sculk lendo…" com o carregador de 4 blocos).

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Decisões em aberto

- Painel Detalhes: empurra a conversa (escolhido no protótipo) em vez de cobrir.
- Botão do consentimento: "Enviar e começar" (pedido desta rodada) diverge do texto da SPEC ("✦ Começar conversa").
- A SPEC, o glossário (QUALITY §8.2) e os textos em `i18n/pt-BR/ia.ts` ainda dizem "Diagnóstico com IA" e "Enviado à IA"; precisam ser atualizados para Sensor Sculk / Sculk na implementação.
