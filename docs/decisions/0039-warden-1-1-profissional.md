# ADR-0039 — Warden 1.1 "Profissional": versão nova e ganchos na v1

- **Status:** aceita · **Data:** 2026-10-02 · **Origem:** decisão do dono + técnica (tarefa D5, decisão D33)

## Contexto

Depois da D4, o dono perguntou o que faltava para o Warden ser um gerenciador de mods profissional e aprovou seis funções (D27 a D32): segurança dos mods, manutenção e substitutos, travamento de um jogador, notas e grupos, itens repetidos entre mods e desempenho entre versões. A v1 já ficou grande com a D4 (ADR-0036). Algumas das funções novas dependem de dados que só existem se forem gravados desde o primeiro dia: as métricas de testes antigos, por exemplo, não podem ser recuperadas depois.

## Decisão

- As seis funções **não aumentam o escopo da v1**. Elas formam o **Warden 1.1 "Profissional"**, construído depois da v1, no marco **M6** do ROADMAP. Na SPEC, aparecem com a etiqueta **"1.1"**, do mesmo jeito que "P1". SPEC, ARCHITECTURE e ROADMAP sobem para a versão 1.3 do documento.
- A v1 deixa só **ganchos de custo mínimo**, para a 1.1 não exigir refatoração nem perder dados:
  1. **L-03 / P1-03:** o índice do cache de downloads (`cache/downloads/index.sqlite`) guarda, para cada jar, sha1, sha512, sha256 e a impressão murmur2 da CurseForge, mais a origem (fonte, projeto, versão, URL), calculados durante o download.
  2. **P1-19:** jars vindos de um pack importado ficam marcados com a origem da importação em `.warden/project.toml`.
  3. **L-04 / L-10:** cada sessão grava também a assinatura do perfil do teste, a impressão do computador e a marca de primeira abertura, e acrescenta uma linha de métricas em `perf/<pack-id>.jsonl` (dados locais), que a poda das sessões não apaga.
  4. **P1-07:** o leitor de `.warden/` preserva arquivos e tabelas que não conhece (o nome `.warden/mods.toml` fica reservado). **P1-08:** cada item tem uma chave estável (projeto do Modrinth ou da CurseForge, ou o caminho do metafile) e o agrupamento da lista de Mods é feito por uma função genérica.
  5. **D-02:** a análise pós-crash aceita qualquer texto de log (já necessário para a IA com um log do computador) e a redação aceita regras extras. **D-06:** o registro de travamento tem o campo `origin` (teste ou jogador).
  6. **V-03:** as conferências do `publish_plan` são uma lista de checagens plugáveis.
  7. **P1-03 / P1-12:** o cache do Modrinth guarda `status`, `updated` e `game_versions` dos projetos, que já vêm na resposta.

## Alternativas consideradas

- Incluir as seis funções na v1: atrasa a v1, que já tem as funções avançadas da D4.
- Não deixar ganchos: a 1.1 teria de refatorar contratos já usados e perderia dados históricos, como as métricas dos testes feitos antes dela.

## Consequências

- Nenhuma tarefa da v1 muda de prioridade; os ganchos entram nas entregas das tarefas citadas, cada um com o seu teste.
- As funções e seus porquês ficam nas ADR-0040 a ADR-0047; onde cada uma mora, na `docs/design/ESTRUTURA.md` §14 ([ADR-0045](0045-funcoes-1-1-na-estrutura.md)).
- O ROADMAP ganha o marco M6 com as tarefas da 1.1, todas depois dos marcos da v1.
