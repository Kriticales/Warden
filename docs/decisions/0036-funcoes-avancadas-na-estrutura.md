# ADR-0036 — Funções avançadas encaixadas na estrutura aprovada

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão do dono (tarefa D4) · **Complementa:** ADR-0026

## Contexto

Depois de aprovar a estrutura (ADR-0026), o dono achou o app "simplificado demais" e pediu uma ferramenta completa de criação, edição e debug (pesquisas R5A e R5B). A ADR-0026 exige nova decisão para uma sétima seção; o dono manteve a regra: nada de abas, menu do pack com 6 seções.

## Decisão

- **Nenhuma seção nova, nenhuma aba.** As funções novas entram como páginas de detalhe, modos de exibição, itens do menu ▾ do Testar, painéis e diálogos, conforme `docs/design/ESTRUTURA.md` §13.
- Mudanças aprovadas (ESTRUTURA N1 a N10): descrições novas de Problemas ("Saúde do pack, problemas e travamentos"), ✦ Diagnóstico com IA ("Conversar com a IA sobre um problema do pack") e Configs ("Arquivos de ajuste e scripts do pack"); "Abrir pack existente" vira "Abrir ou importar…"; Criar pack ganha a etapa "Mods iniciais"; a página Adicionar abre em tela cheia como página de descoberta (o menu lateral recolhe para ícones enquanto ela está aberta) e o seletor Tipo ganha "Modpacks"; o menu ▾ do Testar ganha grupos (Outros testes, Perfil do teste, Instância de teste); Mods ganha "Ver como: Lista · Grafo"; a tela do teste ganha a faixa de desempenho, o seletor "Mostrar" do console, o console do servidor e o modo "Busca do culpado"; Exportar ganha "Pacote para servidor".
- O botão ▶ Testar continua fazendo só o teste normal; servidor local, busca do culpado e perfil de desempenho só rodam quando o usuário pede.

## Alternativas consideradas

- Sétima seção "Explorar" para a descoberta: dividiria "Mods" em dois lugares para a mesma tarefa (R5B §7.6).
- Seção própria para "Desempenho" ou "Testes": o que é sobre um teste mora na tela do teste.

## Consequências

- SPEC §5, T02 a T08, T12 a T14 e T19 atualizadas; ARCHITECTURE §18 (rotas) e ROADMAP com as tarefas novas.
- O protótipo final mostra as telas novas (`design/prototipo-final/`).
