# ADR-0030 — IA "médico" com ferramentas, conversas por pack e consentimento por conversa

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono (tarefa D4, sobre a pesquisa R5A §4) · **Substitui em parte:** ADR-0014 (regra "antes de cada envio")

## Contexto

A ADR-0014 previa a IA (Gemini) recebendo um pacote fixo (resumo do pack, achados, até 200 KB de log) e devolvendo uma resposta, com o texto exato mostrado e aprovado **antes de cada envio**. O dono pediu um diagnóstico "mais avançado que o MCDoctor.ai". A pesquisa R5A §4 mostrou que o Gemini 3.x faz *function calling* (a IA pede ao app o que precisa: um trecho de log, os metadados de um jar, a comparação com a última versão que funcionava, issues do mod) e que, com isso, pedir confirmação a cada consulta tornaria a conversa inutilizável.

## Decisão

- A IA vira um **agente com ferramentas** dentro de **conversas por pack** (seção ✦ Diagnóstico com IA; ESTRUTURA §13).
- **Consentimento uma vez por conversa:** ao começar, um diálogo lista o que a IA poderá consultar (informações do pack e lista de mods; achados do Warden; logs e crash reports, sempre redigidos; configs do pack, desmarcável; busca de issues no GitHub, desmarcável) e mostra o texto inicial exato, o tamanho, o modelo e o aviso do plano gratuito. Sem opção "não perguntar de novo" entre conversas.
- **Cada envio fica visível no chat**, num bloco "Enviado à IA" que é byte a byte o que saiu (mantém o espírito do CA-T14-03 da regra antiga).
- **Nada muda no pack sem o clique em Aplicar.** Ferramentas de leitura respondem na hora; ferramentas de ação só criam uma **proposta**, que aparece como cartão com a diferença exata e passa pelos fluxos normais do app (dependências, diferença antes de salvar, ponto de segurança).
- **Busca em issues do GitHub** dos mods: a IA pode pedir; o Warden envia ao GitHub **só o nome do repositório do mod e palavras do erro**, nunca o log.
- **Evidência conferida por código:** a resposta final é JSON (`responseFormat`/`responseJsonSchema`) com afirmações e evidências; o Rust confere que cada evidência veio de uma ferramenta desta conversa e que a citação aparece literalmente no resultado. Afirmação sem evidência válida aparece marcada "não verificado". Mods citados precisam existir no pack ou na API.
- **Laço sem estado** no `generateContent` (o histórico é reenviado a cada rodada, preservando as *thought signatures* do Gemini 3), com no máximo 8 rodadas de ferramenta por pergunta; a Interactions API com `store: true` não é usada, porque guarda dados no servidor do Google.
- **Conversas guardadas** nos dados locais (`ai/<pack-id>/<ULID>.json`), nunca na pasta do pack, com as mensagens, os resultados como foram enviados, o modelo, os tokens e as propostas aplicadas.

## Alternativas consideradas

- Manter o pacote fixo com confirmação a cada envio (ADR-0014): seguro, mas a IA não consegue fazer perguntas de seguimento nem alcançar configs, histórico e issues.
- Confirmar cada consulta da IA: a conversa vira uma sequência de cliques (questão 3 da R5A, opção b, recusada pelo dono).
- Interactions API com estado: menos dados a reenviar, mas guarda o conteúdo no Google (1 dia no gratuito, 55 no pago).

## Consequências

- `warden-ai` ganha registro de ferramentas, laço, conferência de evidências e armazenamento de conversas (ARCHITECTURE §9.5); a SPEC T14 troca "Resposta da IA" por conversas (ESTRUTURA N2).
- O spike S-R5-4 valida o laço contra um servidor simulado e com uma chave real antes da implementação (ROADMAP).
- Texto vindo de logs, issues e changelogs é tratado como dado não confiável (injeção de instruções, OWASP LLM01): vai delimitado e nenhuma ferramenta tem efeito sem o clique do usuário.
- O resto da ADR-0014 continua: camada determinística primeiro, redação de dados pessoais antes de qualquer envio, chave do próprio usuário.
