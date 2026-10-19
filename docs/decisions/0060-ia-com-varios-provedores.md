# ADR-0060 — IA com vários provedores: Gemini, OpenAI e Claude

- **Status:** aceita · **Data:** 2026-10-18 · **Origem:** decisão do dono ("Adicione também IA com vários provedores para sair com a 1.0") · **Complementa:** [ADR-0014](0014-diagnostico-e-ia.md), [ADR-0030](0030-ia-com-ferramentas-e-conversas.md) e [ADR-0055](0055-ia-modelo-padrao-e-resposta-final.md)

## Contexto

O ✦ Diagnóstico com IA da v1 (D-04 e D-14) fala só com o Gemini: o laço monta o corpo do `generateContent`, guarda o `contents` da API como veio e decide as novas tentativas pelos códigos do Google. O dono pediu, para a 1.0, que a pessoa possa usar também a OpenAI e o Claude (Anthropic), com a chave dela, sem perder nada do que já vale para o Gemini. Cada API tem o seu formato de mensagens, de ferramentas, de erros e de lista de modelos, e cada uma exige devolver intacta uma parte diferente da resposta (as *thought signatures* do Gemini 3; os blocos de raciocínio da Anthropic).

## Decisão

- **Três provedores**, todos pela **API REST oficial**, com a chave do próprio usuário: **Gemini** (Google, o **padrão**, porque tem plano gratuito), **OpenAI** e **Claude** (Anthropic). Nada de "compatível com OpenAI", agregador ou IA local nesta versão.
- **Sem SDK:** cada provedor é um tradutor em Rust sobre a `warden-http`, como o Gemini já era. A chave vai só no cabeçalho de autenticação de cada API, nunca na URL nem em log.
- **Chave do usuário** no cofre do sistema ou no `.env` da pasta de dados (ADR-0025 e ADR-0052): `GEMINI_API_KEY`, `OPENAI_API_KEY` e `ANTHROPIC_API_KEY`, nas contas `gemini-api-key`, `openai-api-key` e `anthropic-api-key`.
- **Um núcleo comum** na `warden-ai` (`providers/`): um trait de provedor assíncrono e um formato comum de mensagens (sistema, usuário, assistente com texto e chamadas de ferramenta, resultado de ferramenta), com um **campo opaco por mensagem** para o que o provedor exige devolver igual. O laço (`engine.rs`) só conhece esse formato; cada tradutor converte o pedido para a sua API e a resposta de volta, e classifica os erros em categorias comuns (temporário, limite por minuto, cota esgotada, chave inválida, resposta malformada, cobrança, recusa). O Gemini migra para o trait **sem mudar um byte** do que sai para a API.
- **Conversa presa ao provedor em que começou:** a conversa guarda o provedor e o modelo; trocar o provedor ativo não muda as conversas antigas, que continuam abrindo, mas para perguntar de novo o Warden usa a chave do provedor da conversa e, sem ela, pede essa chave (`AI_PROVIDER_KEY_MISSING`). As conversas gravadas antes desta decisão (formato 1, só Gemini) são convertidas ao abrir.
- **Continua valendo para todos os provedores**, sem exceção: consentimento uma vez por conversa (ADR-0030), agora com o nome do provedor e o modelo; cada envio visível byte a byte no bloco "Enviado à IA"; nada muda no pack sem **Aplicar**; a conferência das evidências; a resposta final pela ferramenta `responder` forçada, com a reserva sem ferramentas e com resposta em JSON pelo esquema (ADR-0055); no máximo 8 rodadas de ferramenta por pergunta e o teto de tokens por conversa; **nenhum nome de modelo fixo no código** (cada provedor escolhe pela lista de modelos da conta).
- **Passa a ser por provedor:** o formato da API; quais respostas são temporárias e quanto esperar (o laço aplica os números da ADR-0055 a partir da classificação comum); os limites e o tempo-limite; a lista de modelos e a regra do modelo padrão e da "Análise mais cuidadosa"; o aviso de plano gratuito (só no Gemini: OpenAI e Anthropic não têm plano gratuito pela API).
- **Configurações:** `ai/settings.json` ganha `aiProvider` (`gemini` padrão, `openai`, `anthropic`) e a escolha do modelo **por provedor**; o arquivo antigo, sem o campo, continua valendo como Gemini.

## Alternativas consideradas

- **SDKs oficiais** (`async-openai`, SDKs da Anthropic e do Google): recusados. São dependências grandes, cada uma com o seu cliente HTTP, e dão menos controle sobre o que sai, o que o Warden precisa para mostrar cada envio byte a byte e para tirar a chave dos erros.
- **Um agregador como o OpenRouter** (uma chave e uma API para todos): recusado pelo dono nesta versão. Põe mais um serviço entre o pack e o provedor e muda a conta do usuário.
- **Formato "compatível com OpenAI" para todos:** as APIs do Gemini e da Anthropic têm modos compatíveis, mas eles perdem justamente o que o laço precisa (as *thought signatures*, os blocos de raciocínio, a escolha de ferramenta entre um conjunto de nomes).

## Consequências

- A D-15 faz o núcleo, migra o Gemini, cria o formato 2 das conversas e as chaves e configurações no backend; a D-16 (OpenAI) e a D-17 (Anthropic) escrevem só os seus tradutores; a D-18 faz as telas e os E2E por provedor (ROADMAP fase 4).
- **Log sem pack aberto por link (D-19):** colar um link do mclo.gs ou do pastebin faz o Warden pedir **só o texto ao site do link** (`api.mclo.gs/1/raw/<id>`, `pastebin.com/raw/<id>`), pelo Rust, antes do consentimento, para mostrar o texto inicial exato. Nada vai ao provedor de IA além do que já ia sem pack (a lista de modelos e a contagem de tokens sem o log); o próprio link não vai à IA, e o texto redigido só sai depois de **✦ Começar conversa**, pelas mesmas regras de todos os provedores.
- Os testes de rede (`rede_*`) são um por provedor e ficam ignorados sem a chave daquele provedor.
- O Gemini continua com o servidor simulado que recusa histórico alterado; a OpenAI e a Anthropic ganham os seus servidores simulados (`apps/desktop/e2e/mock-server/`).

## Complemento de 19/10/2026: o modelo do Gemini

- **Origem:** pedido do dono, pelo orquestrador, na revisão da D-15. **Muda** o modelo padrão do Gemini da [ADR-0055](0055-ia-modelo-padrao-e-resposta-final.md) (o Flash-Lite).
- **As duas opções do Gemini** ("Mais econômico" e "Análise mais cuidadosa") usam **o modelo mais forte que o plano gratuito da API permite**. Pela tabela oficial de preços do Google (ai.google.dev/gemini-api/docs/pricing, consultada em 19/10/2026), os modelos de texto com plano gratuito são as famílias Flash e Flash-Lite; o Pro (`gemini-3.1-pro-preview`, `gemini-2.5-pro`) e o `gemini-4-argon` não têm. O mais forte é, então, o **Flash estável mais novo**, reconhecido pelo `models.list` sem nome fixo: `gemini-<versão>-flash` com `generateContent`, a maior versão numérica, sem `-preview`, `-exp`, `-latest`, revisão ou apelido (em 19/10/2026, `gemini-3.8-flash`).
- **Cota diária esgotada:** quando o 429 diário do Flash chega (`AI_QUOTA_EXCEEDED`), o laço repete a mesma requisição, sem espera e sem mexer no histórico, com o **Flash-Lite estável mais novo**, **só naquela pergunta**; a pergunta guarda o modelo usado (`fallbackModel` no turno da conversa) e a tela mostra "O limite diário do modelo mais forte acabou; esta resposta usou o mais leve." A pergunta seguinte volta ao Flash. Acabando também a cota do Flash-Lite, vale a mensagem de sempre, sem horário de volta. Sem Flash estável na conta, as duas opções usam o Flash-Lite, sem troca.
- **Por provedor:** o mais leve vem da regra de modelos de cada provedor (`Provider::models`, campo `lighter`); só o Gemini o preenche. A OpenAI e a Anthropic (D-16 e D-17) seguem a sua regra própria e não trocam de modelo.
- **Risco aceito:** a conversa passa a gastar a cota do Flash, menor que a do Flash-Lite no plano gratuito; a troca automática cobre o dia em que ela acaba.
