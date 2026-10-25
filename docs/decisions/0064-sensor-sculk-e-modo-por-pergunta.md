# ADR-0064 — Sensor Sculk: o nome, a tela nova, o modo por pergunta e o desempenho para a IA

- **Status:** aceita · **Data:** 2026-10-25 · **Origem:** relatório de bugs nº 1 do dono (telas aprovadas em `design/bug-01/`, protótipo `sensor-sculk.html`, versão 2; decisão P12) · **Substitui em parte:** [ADR-0055](0055-ia-modelo-padrao-e-resposta-final.md) (a escolha "Mais econômico" / "Análise mais cuidadosa" em Configurações) · **Complementa:** [ADR-0030](0030-ia-com-ferramentas-e-conversas.md) e [ADR-0060](0060-ia-com-varios-provedores.md)

## Contexto

O dono usou o ✦ Diagnóstico com IA e pediu, no relatório de bugs nº 1:

- um nome com a cara do Warden: a seção passa a se chamar **Sensor Sculk** e a IA, dentro do app, **Sculk**;
- uma tela de desktop de verdade, aprovada como esboço navegável: conversas à esquerda, conversa à direita e uma gaveta **Detalhes** recolhível, com a resposta do Sculk sem caixa (só o fio à esquerda), o consentimento com **Enviar e começar** e o campo de pergunta sem a linha "modelo · pedidos · Enter envia";
- escolher o esforço **a cada pergunta**, não uma vez em Configurações (P12): uma dúvida simples não precisa do modelo mais caro, e um travamento difícil não deveria esperar a pessoa ir a Configurações trocar a opção;
- que o Sculk leia o desempenho que o Warden já mede em cada teste, para responder perguntas como "por que o mundo demora tanto para carregar".

A ADR-0055 pôs a escolha do modelo em Configurações, por provedor ("Mais econômico", padrão, ou "Análise mais cuidadosa"), e a ADR-0060 manteve isso para os três provedores. No Gemini, as duas opções já usavam o mesmo modelo (o Flash estável mais novo), com a troca sozinha para o Flash-Lite no limite diário.

## Decisão

1. **Nomes.** A seção é **Sensor Sculk** (menu, títulos, glossário da QUALITY §8.2); a IA é **Sculk** em todos os textos da interface ("Perguntar ao Sculk", "Conversar com o Sculk", "O Sculk pode errar. Confira antes de mudar o pack.", "Enviado ao Sculk: …"). "IA" fica só onde se fala do serviço ("provedores de IA"). O ícone ✦ continua marcando só o que é IA.
2. **Tela.** A do protótipo aprovado, usando a área inteira do pack com o menu só nos ícones (o mesmo menu recolhido da página de descoberta): coluna de conversas (busca, grupos por data, selos Lendo / Respondida / Esperando você / Aplicada / Parou com erro / Sem resposta, rodapé do provedor), a conversa ou a Nova conversa, e a gaveta Detalhes (serviço e uso, o que o Sculk pode ler, tudo o que foi enviado com o total, avisos da conversa, apagar com confirmação no rodapé). "Esperando você" é a conversa com proposta para decidir; "Aplicada", a que teve proposta aplicada e nenhuma esperando (o resumo da lista ganhou `appliedProposals`). A página "Analisar um log" (sem pack, D-19) usa os mesmos componentes. As rotas continuam as mesmas (`/packs/$packId/ia` e `/ia/$conversaId`; `/analisar-log` e `/analisar-log/$conversaId`), mas a página é uma só: a coluna de conversas fica no lugar ao trocar de conversa.
3. **Modo por pergunta** (`SearchMode`, no lugar de `ModelTier`): **Procurar rápido** (`quick`) usa o modelo estável mais econômico do provedor e no máximo **4** rodadas de consulta; **Procurar a fundo** (`deep`) usa o modelo estável mais forte da faixa barata e no máximo **8** (o limite de sempre da ARCHITECTURE §9.5). Os modelos continuam saindo da lista de modelos da chave, pela regra de cada provedor (ADR-0055 e ADR-0060): no Claude, Sonnet e Opus mais novos; na OpenAI, `gpt-N-luna` e `gpt-N-sol`. A escolha aparece na Nova conversa e na barra do campo de pergunta, **só no Claude e na OpenAI**; o padrão da tela é "Procurar a fundo", e o campo propõe o modo da pergunta anterior. **No Gemini não há escolha**: toda pergunta usa o Flash estável mais novo, com a troca para o Flash-Lite no limite diário ou no tempo-limite (ADR-0060, complemento de 19/10/2026; D-15b), e o modo pedido é ignorado.
4. **A conversa grava o modo e o modelo de cada pergunta** (`ConversationTurn.mode` e `ConversationTurn.model`, opcionais). A primeira pergunta sai com o modo e o modelo da prévia; as seguintes guardam o modo no envio e o modelo é resolvido ao rodar, pela lista de modelos da chave **do provedor da conversa** (que continua presa ao provedor em que começou, ADR-0060). O laço usa o modelo da troca, senão o da pergunta, senão o da conversa. Detalhes mostra o modo (ou "Não se aplica ao Gemini").
5. **Configurações perde a escolha do modelo.** Sai o comando `ai_settings_set` e o campo `tier` de `ai_settings_get`; a tela só informa os modelos de cada modo (o redesenho de Configurações é de outra tarefa). **Migração:** um `ai/settings.json` com `tier`/`tiers` continua abrindo (os campos são ignorados) e perde a escolha fixa na próxima gravação; as conversas antigas, sem modo nem modelo por pergunta, abrem iguais e continuam no modelo em que começaram.
6. **Desempenho para o Sculk.** Duas ferramentas novas de leitura, no mesmo esquema das outras (declaradas desde a primeira rodada, cada item com `id` e `texto` para virar evidência, redação, bloco "Enviado ao Sculk" byte a byte): `get_test_performance` (as medidas de cada teste em `perf/<pack-id>.jsonl` — tempo para abrir e para entrar no mundo, pico de RAM, memória do jogo depois das coletas, perfil do teste, a impressão do computador reduzida a um código, primeira abertura, a versão salva pela árvore testada — e a comparação entre versões pelo critério de "mais pesada" da SPEC T33) e `get_performance_profile` (o resultado do "Testar com perfil de desempenho", T13: tempo de carregamento por mod e o resumo do spark). Não pedem permissão a mais: entram no consentimento como "Medidas de desempenho dos testes", com a marca "Sempre", porque são só números do próprio Warden e o computador vai só como código. O *system prompt* ganhou uma regra curta: para lentidão ou carregamento demorado, consultar o desempenho antes de concluir.

## Alternativas consideradas

- **Manter o modelo em Configurações e só renomear as opções:** recusada pelo dono (P12); a escolha é de cada pergunta.
- **Três modos (rápido, normal, a fundo):** mais uma coisa para o leigo entender, sem ganho claro; dois modos cobrem o "dúvida simples" e o "problema difícil".
- **Um modo por conversa:** a mesma conversa costuma ter uma pergunta difícil e várias de acompanhamento; por pergunta custa menos e o Detalhes mostra o que foi usado.
- **Trocar o Claude ou a OpenAI para um modelo mais leve quando o mais forte demora** (o esboço mostra esse aviso também no Claude): fica para depois; hoje só o Gemini tem um modelo mais leve escolhido pela regra, e o aviso de troca aparece onde ela acontece.
- **Permissão própria para o desempenho:** recusada; os dados são números do próprio Warden, sem nada pessoal, como a lista de mods.

## Consequências

- O que a ADR-0055 dizia sobre a escolha "Mais econômico" / "Análise mais cuidadosa" em Configurações deixa de valer; o resto dela (resposta final pela `responder`, reserva, novas tentativas) continua.
- `SearchMode` substitui `ModelTier` nos bindings; `ai_conversation_send` e `ai_standalone_send` recebem o modo; a prévia recebe e devolve o modo.
- A SPEC T14 descreve a tela nova, o modo por pergunta e o desempenho (CA-T14-31 e CA-T14-32); o glossário da QUALITY §8.2 tem os nomes novos.
- O aviso de troca para o modelo mais leve no Claude e na OpenAI e o "Desfazer" ao apagar uma conversa (o esboço mostra os dois) não entram agora: o primeiro precisa de uma regra de modelo mais leve nesses provedores, e o segundo, de guardar a conversa apagada para devolver.
