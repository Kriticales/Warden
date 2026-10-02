# Contraste dos tokens (WCAG 2.1)

Gerado por `node design/system/tools/contraste.mjs` a partir de `tokens.json`. Cores translúcidas são compostas sobre o fundo indicado antes do cálculo. Mínimo 4,5:1 para texto (1.4.3) e 3:1 para contornos, ícones e marcadores (1.4.11).

| Frente | Fundo | Contraste | Mínimo | Nível | Uso |
|---|---|---|---|---|---|
| `text` | `bg` | 17.12:1 | 4.5:1 | AAA | Texto principal |
| `text-2` | `bg` | 11.86:1 | 4.5:1 | AAA | Texto secundário |
| `text-3` | `bg` | 7.98:1 | 4.5:1 | AAA | Texto apagado / placeholder |
| `primary-text` | `bg` | 12.76:1 | 4.5:1 | AAA | Link e texto em ciano |
| `bone` | `bg` | 14.59:1 | 4.5:1 | AAA | Texto em osso / IA |
| `border-control` | `bg` | 5.05:1 | 3:1 | AA (3:1) | Contorno de campo |
| `focus` | `bg` | 14.59:1 | 3:1 | AA (3:1) | Anel de foco |
| `primary` | `bg` | 10.66:1 | 3:1 | AA (3:1) | Ciano como marcador (barra, progresso, borda) |
| `text` | `bg-sunken` | 17.52:1 | 4.5:1 | AAA | Texto principal |
| `text-2` | `bg-sunken` | 12.13:1 | 4.5:1 | AAA | Texto secundário |
| `text-3` | `bg-sunken` | 8.17:1 | 4.5:1 | AAA | Texto apagado / placeholder |
| `primary-text` | `bg-sunken` | 13.06:1 | 4.5:1 | AAA | Link e texto em ciano |
| `bone` | `bg-sunken` | 14.93:1 | 4.5:1 | AAA | Texto em osso / IA |
| `border-control` | `bg-sunken` | 5.17:1 | 3:1 | AA (3:1) | Contorno de campo |
| `focus` | `bg-sunken` | 14.93:1 | 3:1 | AA (3:1) | Anel de foco |
| `primary` | `bg-sunken` | 10.91:1 | 3:1 | AA (3:1) | Ciano como marcador (barra, progresso, borda) |
| `text` | `surface-1` | 15.53:1 | 4.5:1 | AAA | Texto principal |
| `text-2` | `surface-1` | 10.75:1 | 4.5:1 | AAA | Texto secundário |
| `text-3` | `surface-1` | 7.24:1 | 4.5:1 | AAA | Texto apagado / placeholder |
| `primary-text` | `surface-1` | 11.57:1 | 4.5:1 | AAA | Link e texto em ciano |
| `bone` | `surface-1` | 13.24:1 | 4.5:1 | AAA | Texto em osso / IA |
| `border-control` | `surface-1` | 4.58:1 | 3:1 | AA (3:1) | Contorno de campo |
| `focus` | `surface-1` | 13.24:1 | 3:1 | AA (3:1) | Anel de foco |
| `primary` | `surface-1` | 9.67:1 | 3:1 | AA (3:1) | Ciano como marcador (barra, progresso, borda) |
| `text` | `surface-2` | 13.72:1 | 4.5:1 | AAA | Texto principal |
| `text-2` | `surface-2` | 9.50:1 | 4.5:1 | AAA | Texto secundário |
| `text-3` | `surface-2` | 6.40:1 | 4.5:1 | AA | Texto apagado / placeholder |
| `primary-text` | `surface-2` | 10.22:1 | 4.5:1 | AAA | Link e texto em ciano |
| `bone` | `surface-2` | 11.69:1 | 4.5:1 | AAA | Texto em osso / IA |
| `border-control` | `surface-2` | 4.05:1 | 3:1 | AA (3:1) | Contorno de campo |
| `focus` | `surface-2` | 11.69:1 | 3:1 | AA (3:1) | Anel de foco |
| `primary` | `surface-2` | 8.54:1 | 3:1 | AA (3:1) | Ciano como marcador (barra, progresso, borda) |
| `text` | `surface-3` | 12.00:1 | 4.5:1 | AAA | Texto principal |
| `text-2` | `surface-3` | 8.31:1 | 4.5:1 | AAA | Texto secundário |
| `text-3` | `surface-3` | 5.60:1 | 4.5:1 | AA | Texto apagado / placeholder |
| `primary-text` | `surface-3` | 8.94:1 | 4.5:1 | AAA | Link e texto em ciano |
| `bone` | `surface-3` | 10.23:1 | 4.5:1 | AAA | Texto em osso / IA |
| `border-control` | `surface-3` | 3.54:1 | 3:1 | AA (3:1) | Contorno de campo |
| `focus` | `surface-3` | 10.23:1 | 3:1 | AA (3:1) | Anel de foco |
| `primary` | `surface-3` | 7.47:1 | 3:1 | AA (3:1) | Ciano como marcador (barra, progresso, borda) |
| `ok-text` | `bg` | 12.66:1 | 4.5:1 | AAA | Texto de estado ok |
| `ok-text` | `surface-1` | 11.48:1 | 4.5:1 | AAA | Texto de estado ok |
| `ok-text` | `surface-2` | 10.14:1 | 4.5:1 | AAA | Texto de estado ok |
| `ok-text` | `surface-3` | 8.87:1 | 4.5:1 | AAA | Texto de estado ok |
| `ok-text` | `ok-soft sobre surface-1` | 9.03:1 | 4.5:1 | AAA | Texto ok sobre o fundo suave do alerta |
| `text` | `ok-soft sobre surface-1` | 12.21:1 | 4.5:1 | AAA | Texto principal dentro do alerta ok |
| `ok` | `surface-1` | 9.95:1 | 3:1 | AA (3:1) | Ícone/barra ok |
| `warn-text` | `bg` | 13.04:1 | 4.5:1 | AAA | Texto de estado warn |
| `warn-text` | `surface-1` | 11.83:1 | 4.5:1 | AAA | Texto de estado warn |
| `warn-text` | `surface-2` | 10.45:1 | 4.5:1 | AAA | Texto de estado warn |
| `warn-text` | `surface-3` | 9.14:1 | 4.5:1 | AAA | Texto de estado warn |
| `warn-text` | `warn-soft sobre surface-1` | 9.49:1 | 4.5:1 | AAA | Texto warn sobre o fundo suave do alerta |
| `text` | `warn-soft sobre surface-1` | 12.46:1 | 4.5:1 | AAA | Texto principal dentro do alerta warn |
| `warn` | `surface-1` | 9.55:1 | 3:1 | AA (3:1) | Ícone/barra warn |
| `danger-text` | `bg` | 8.87:1 | 4.5:1 | AAA | Texto de estado danger |
| `danger-text` | `surface-1` | 8.04:1 | 4.5:1 | AAA | Texto de estado danger |
| `danger-text` | `surface-2` | 7.11:1 | 4.5:1 | AAA | Texto de estado danger |
| `danger-text` | `surface-3` | 6.22:1 | 4.5:1 | AA | Texto de estado danger |
| `danger-text` | `danger-soft sobre surface-1` | 6.97:1 | 4.5:1 | AA | Texto danger sobre o fundo suave do alerta |
| `text` | `danger-soft sobre surface-1` | 13.46:1 | 4.5:1 | AAA | Texto principal dentro do alerta danger |
| `danger` | `surface-1` | 5.87:1 | 3:1 | AA (3:1) | Ícone/barra danger |
| `info-text` | `bg` | 11.22:1 | 4.5:1 | AAA | Texto de estado info |
| `info-text` | `surface-1` | 10.18:1 | 4.5:1 | AAA | Texto de estado info |
| `info-text` | `surface-2` | 8.99:1 | 4.5:1 | AAA | Texto de estado info |
| `info-text` | `surface-3` | 7.87:1 | 4.5:1 | AAA | Texto de estado info |
| `info-text` | `info-soft sobre surface-1` | 8.23:1 | 4.5:1 | AAA | Texto info sobre o fundo suave do alerta |
| `text` | `info-soft sobre surface-1` | 12.55:1 | 4.5:1 | AAA | Texto principal dentro do alerta info |
| `info` | `surface-1` | 8.30:1 | 3:1 | AA (3:1) | Ícone/barra info |
| `on-primary` | `primary` | 10.25:1 | 4.5:1 | AAA | Texto do botão primário e do Testar |
| `on-primary` | `primary-hover` | 7.54:1 | 4.5:1 | AAA | Botão primário com o mouse em cima |
| `on-primary` | `primary-press` | 4.85:1 | 4.5:1 | AA | Botão primário pressionado |
| `on-danger` | `danger-solid` | 5.52:1 | 4.5:1 | AA | Botão de perigo |
| `text` | `primary-soft sobre surface-1` | 12.26:1 | 4.5:1 | AAA | Texto de item selecionado |
| `text-3` | `primary-soft sobre surface-1` | 5.71:1 | 4.5:1 | AA | Metadado de item selecionado |
| `primary-text` | `primary-soft sobre surface-1` | 9.13:1 | 4.5:1 | AAA | Ciano sobre item selecionado |
| `text` | `ai-soft sobre surface-1` | 13.29:1 | 4.5:1 | AAA | Texto do bloco de resposta da IA |
| `ai` | `ai-soft sobre surface-1` | 11.33:1 | 4.5:1 | AAA | Rótulo em osso no bloco da IA |
| `text` | `diff-add sobre bg-sunken` | 13.95:1 | 4.5:1 | AAA | Linha adicionada no diff |
| `text` | `diff-del sobre bg-sunken` | 15.30:1 | 4.5:1 | AAA | Linha removida no diff |
| `ok-text` | `diff-add sobre bg-sunken` | 10.32:1 | 4.5:1 | AAA | Sinal + no diff |
| `danger-text` | `diff-del sobre bg-sunken` | 7.92:1 | 4.5:1 | AAA | Sinal − no diff |
| `console-text` | `console-bg` | 15.49:1 | 4.5:1 | AAA | Console: texto |
| `console-muted` | `console-bg` | 7.95:1 | 4.5:1 | AAA | Console: hora e origem |
| `warn-text` | `console-bg` | 13.49:1 | 4.5:1 | AAA | Console: aviso |
| `danger-text` | `console-bg` | 9.17:1 | 4.5:1 | AAA | Console: erro |
| `src-modrinth` | `surface-1` | 9.95:1 | 3:1 | AA (3:1) | Marcador Modrinth |
| `src-curseforge` | `surface-1` | 7.26:1 | 3:1 | AA (3:1) | Marcador CurseForge |
| `src-local` | `surface-1` | 7.24:1 | 3:1 | AA (3:1) | Marcador arquivo local |
| `ok-text` | `bg-sunken` | 12.95:1 | 4.5:1 | AAA | Chip de evidência conferida; string no editor de scripts |
| `ok` | `bg-sunken` | 11.23:1 | 3:1 | AA (3:1) | Contorno do chip de evidência |
| `info-text` | `bg-sunken` | 11.48:1 | 4.5:1 | AAA | Palavra-chave no editor de scripts |
| `text` | `ai-soft sobre bg` | 15.12:1 | 4.5:1 | AAA | Texto da mensagem da IA na conversa |
| `text-2` | `ai-soft sobre bg` | 10.47:1 | 4.5:1 | AAA | Texto secundário na mensagem da IA |
| `ai` | `ai-soft sobre bg` | 12.89:1 | 4.5:1 | AAA | Rótulo em osso na mensagem da IA |
| `text` | `danger-soft sobre console-bg` | 15.73:1 | 4.5:1 | AAA | Console agrupado: linha de erro |
| `danger-text` | `danger-soft sobre console-bg` | 8.14:1 | 4.5:1 | AAA | Console agrupado: nível de erro |
| `text` | `warn-soft sobre console-bg` | 14.96:1 | 4.5:1 | AAA | Stack trace: primeira linha de mod |
| `warn` | `surface-3` | 7.38:1 | 3:1 | AA (3:1) | Faixa de suspeitos e blocos de memória alta |
| `danger` | `danger-soft sobre surface-2` | 4.50:1 | 3:1 | AA (3:1) | Contorno da rodada que travou |
| `ok` | `ok-soft sobre surface-2` | 6.79:1 | 3:1 | AA (3:1) | Contorno da rodada que passou |
| `warn-text` | `warn-soft sobre surface-1` | 9.49:1 | 4.5:1 | AAA | Memória alta na faixa de desempenho |
| `primary` | `surface-3` | 7.47:1 | 3:1 | AA (3:1) | Blocos do mini-gráfico de memória |

**102 pares conferidos, todos aprovados.**
