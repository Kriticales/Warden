# Telas aprovadas do relatório de bugs nº 1 (bug-01)

Esboços navegáveis aprovados pelo dono em 19 e 20/10/2026, feitos com a skill impeccable no visual Deep Dark. São a referência visual da implementação: siga a estrutura, os textos, os estados e o nível de acabamento de cada um. Cada arquivo é um fragmento HTML (sem `<!doctype>`) e abre direto no navegador.

| Arquivo | Tela | O que foi aprovado |
|---|---|---|
| `sensor-sculk.html` | Sensor Sculk (seção de IA; a IA se chama **Sculk**) | Protótipo inteiro, versão 2: "Clássico de desktop" (conversas à esquerda, conversa à direita, gaveta Detalhes), botão do consentimento **Enviar e começar**, resposta do Sculk sem caixa (fio à esquerda), "Procurar rápido \| Procurar a fundo" na barra do campo (some com o Gemini), sem a linha de modelo/pedidos/Enter. |
| `problemas-e-sculk.html` | Problemas | **Esboço 1 · Separadas**: Problemas e Sensor Sculk continuam seções separadas, ligadas por botões. O design da guia Problemas desse esboço foi aprovado como está, com o bloco **Desempenho do último teste**. Os esboços 2 e 3 foram recusados. |
| `configs.html` | Configs (P9 e P7) | **Opção 1 · Três colunas**, com o seletor **Do pack \| Do teste local \| Comparar** (o "Comparar" vem da opção 3: teste e pack lado a lado, setas → e ← por chave). Grupos No pack / Só no teste / Só neste computador. |
| `adicionar.html` | Adicionar ao pack (P15) | **Opção 4 · As duas, com seletor**: "Lista \| Cartões" ao lado da busca (Lista = tabela densa da opção 1; Cartões = grade da opção 2), filtros à direita da opção 1 nos dois modos, seleção mantida ao trocar, o app lembra o último modo. |
| `adicionar-itens.html` | Diálogo "Adicionar itens" (P1, T09) | **Opção 4 · Árvore + detalhe**: árvore por mod à esquerda (com "Em comum"), painel "por quê" à direita, conta igual no título, no botão e no rodapé. |
| `configuracoes.html` | Configurações (P12) | **Versão 2 · menu lateral** com as 8 páginas; chaves **sempre no Cofre do Windows** (sai a opção `.env`); ordem de preferência dos provedores do Sculk (o 1º com chave funcionando abre as conversas novas; chave recusada é pulada com aviso); modelos só informativos. |

Contexto de produto: `PRODUCT.md` na raiz. Decisões e causas: `docs/decisions/` (ADRs novas da implementação) e o relatório do dono (fora do repositório).
