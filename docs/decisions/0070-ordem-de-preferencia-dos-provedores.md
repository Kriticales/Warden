# ADR-0070 — Ordem de preferência dos provedores do Sculk e Configurações com menu lateral

- **Status:** aceita · **Data:** 2026-10-25 · **Origem:** decisão do dono (relatório de bugs nº 1, P12; esboço aprovado `design/bug-01/configuracoes.html`, versão 2 · menu lateral) · **Complementa:** [ADR-0060](0060-ia-com-varios-provedores.md)

## Contexto

A ADR-0060 deixava a pessoa escolher **um** provedor de IA para as conversas novas (`aiProvider` no `ai/settings.json`). Com uma chave recusada ou sem chave, o Sensor Sculk (o nome novo do ✦ Diagnóstico com IA) parava e pedia a chave, mesmo havendo outro provedor com chave funcionando. No relatório de bugs nº 1 o dono aprovou um esboço de Configurações em que os provedores ficam numa **ordem de preferência**, e em que Configurações deixa de ser uma página única e ganha um **menu lateral** com uma página para cada assunto.

## Decisão

### Ordem de preferência

- `ai/settings.json` ganha `aiOrder`, a ordem dos três provedores. A pessoa muda a ordem em Configurações → Sensor Sculk, arrastando a linha, pelas setas ou com Alt+↑/↓ na alça da linha.
- **A conversa nova usa o 1º da ordem com a chave funcionando:** com chave no cofre e sem recusa no último **Testar** (ADR-0069). Os outros com chave são **Reserva**; sem chave, **Pulado: sem chave**; com a chave recusada no último teste, **Pulado: chave recusada**, com um aviso e o botão **Substituir a chave**. Sem nenhum utilizável, vale o 1º da ordem, e a conversa pede a chave dele, como antes.
- **Uma conversa já começada continua no provedor dela** (ADR-0060), mesmo que a ordem mude.
- **Migração do formato antigo:** sem `aiOrder`, o `aiProvider` antigo vem em primeiro e os outros na ordem da tela (Gemini, OpenAI, Claude). `aiProvider` continua sendo gravado com o 1º da ordem, para uma versão anterior do Warden ler.
- O passo da IA das boas-vindas (escolher um provedor) põe esse provedor em primeiro na ordem (`ai_provider_set`).
- Remover a chave do provedor em uso avisa quem assume as conversas novas (ou que nenhum outro tem chave funcionando).
- Ao lado da ordem, **só para consulta**, o modelo que cada modo usa, vindo da lista de modelos da conta: na OpenAI e no Claude, "Procurar rápido" (o modelo mais econômico) e "Procurar a fundo" (o mais capaz); no Gemini, sem escolha de modo: o Flash, com a troca sozinha para o Flash-Lite no limite diário (complementos da ADR-0060). O modo é escolhido a cada pergunta, no campo de pergunta do Sensor Sculk (tarefa T1); a página mostra uma prévia desse campo. Nomes de modelo nunca quebram no meio.

### Configurações com menu lateral

- Configurações passa a ter um menu à esquerda com 8 páginas, cada uma com o seu endereço (`/configuracoes/<página>`): **Geral**, **Chaves e contas**, **Sensor Sculk**, **Teste**, **Editor de configs**, **Privacidade e registros**, **Armazenamento** e **Sobre o Warden** (sempre a última). `/configuracoes` abre a Geral. O menu marca a página atual e mostra um dado curto onde o esboço mostra: o número de chaves recusadas, o provedor das conversas novas, o espaço usado e a versão; o rodapé diz onde as chaves ficam.
- A página usa a largura toda da janela, como no esboço (não a coluna de leitura de 880 px).
- Isso substitui a regra antiga da SPEC T21 ("página única, sem submenu"). Continua valendo a regra de "sem abas" da estrutura: o menu lateral é de páginas, como o menu de seções do pack.

## Alternativas consideradas

- Manter um provedor só e mostrar um aviso quando a chave falha: a pessoa leiga teria de trocar à mão a cada falha.
- Trocar de provedor no meio da conversa: quebra a regra da ADR-0060 (o histórico de cada API tem partes que só aquela API entende).
- Configurações numa página só, com âncoras: o dono escolheu o menu lateral no esboço.

## Consequências

- Backend: `aiOrder` com a migração do `aiProvider`, o comando `ai_order_set`, o `ai_models_of` (os modelos de um provedor qualquer, para a tabela) e o `ai_settings_get` com a ordem e a recusa de cada chave (`keyRefused`).
- SPEC: T21 (menu lateral; Sensor Sculk; CA-T21-06), T23 (Sobre com o commit do packwiz embutido) e T14 (o provedor da conversa nova é o 1º da ordem com chave funcionando).
