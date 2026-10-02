# ADR-0045 — Funções da 1.1 encaixadas na estrutura aprovada

- **Status:** aceita · **Data:** 2026-10-02 · **Origem:** decisão do dono (tarefa D5) · **Complementa:** ADR-0036

## Contexto

As seis funções do Warden 1.1 ([ADR-0039](0039-warden-1-1-profissional.md)) precisam de lugar na interface. A estrutura aprovada ([ADR-0026](0026-estrutura-de-navegacao.md), complementada pela [ADR-0036](0036-funcoes-avancadas-na-estrutura.md)) tem dois níveis, 6 seções no pack e nenhuma aba, e o dono manteve essas regras. Quatro das seis funções tratam de coisas que podem dar errado no pack, e Problemas não pode virar uma página interminável.

## Decisão

- **Nenhuma seção nova, nenhuma aba.** As funções entram como páginas de detalhe, modos de exibição, filtros, painéis e diálogos, conforme `docs/design/ESTRUTURA.md` §14. Mudanças aprovadas (ESTRUTURA §14.3, P1 a P6):
  - **P1:** Problemas ganha o painel **Verificações do pack** (Segurança, Manutenção, Itens repetidos), cada linha levando a uma página de detalhe com "← Problemas".
  - **P2:** Problemas → **Travamentos** aceita travamentos de jogadores, com o botão "Analisar travamento de um jogador…" e a coluna Origem.
  - **P3:** Mods ganha **Agrupar por** e o filtro **Grupo**; o painel de detalhes ganha **Nota e grupos** no topo.
  - **P4:** **Procurar substituto** troca o conteúdo do próprio painel de detalhes do mod, com "← Detalhes".
  - **P5:** Histórico ganha o bloco e a página **Desempenho entre versões**.
  - **P6:** o diálogo Publicar versão ganha **Checagens obrigatórias** (segurança e manutenção).
- O menu ▾ do Testar **não muda**.

## Alternativas consideradas

- Sétima seção "Segurança" ou "Verificações": tiraria de Problemas assuntos que são problemas do pack e exigiria nova decisão sobre a ADR-0026.
- Página separada para travamentos de jogadores: duplicaria a lista e a análise; um travamento é um travamento, venha de onde vier.
- Diálogo para os substitutos: seria camada sobre o painel; o padrão de trocar o conteúdo do painel já existe no raio-x.
- Desempenho entre versões na tela do teste: o que se compara são versões salvas, e elas moram no Histórico.

## Consequências

- SPEC T28 a T33 e as telas da v1 que mudam ganham a etiqueta "(1.1)"; o protótipo mostra as telas novas.
- Os três pontos de design pendentes da D4 (DESIGN-SYSTEM §10) continuam como estão.
