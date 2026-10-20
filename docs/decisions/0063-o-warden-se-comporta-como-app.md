# ADR-0063 — O Warden se comporta como app, não como página

- **Status:** aceita · **Data:** 2026-10-20 · **Origem:** decisão técnica do orquestrador (tarefa B1-C, problema 11 do dono) · **Complementa:** [ADR-0002](0002-stack-tauri-rust-react.md)

## Contexto

A interface do Warden roda num WebView (WebView2 no Windows, WebKitGTK no Linux), e o dono notou que o app se comportava como uma página de navegador: o menu de contexto do Edge ("Voltar", "Recarregar", "Salvar como", "Imprimir", "Inspecionar"); F5 e Ctrl+R recarregavam e perdiam o estado da tela; Ctrl+P, Ctrl+F, F3, Ctrl+S (salvar a página), Alt+← e Alt+→ abriam funções do navegador; os botões voltar e avançar do mouse navegavam; o preenchimento automático oferecia valores antigos nos campos; quase toda a interface se selecionava como texto; e imagens e links se arrastavam.

## Decisão

- **No Windows, logo depois de criar a janela** (`apps/desktop/src-tauri/src/webview_behavior/`), pelo `with_webview` do Tauri e a `webview2-com` (a mesma versão que o `wry` já usa), com o `unsafe` isolado em `windows.rs`:
  - `ICoreWebView2Settings3::SetAreBrowserAcceleratorKeysEnabled(false)`: desliga os atalhos de navegador e mantém os de edição (Ctrl+C, Ctrl+V, Ctrl+Z…). A página continua recebendo as teclas, então o Ctrl+S do editor de configs e os atalhos do app funcionam;
  - `ContextMenuRequested` (`ICoreWebView2_11`): fora de campo editável e sem seleção, o menu não aparece; num campo editável ou sobre texto selecionado, só desfazer, refazer, recortar, copiar, colar e selecionar tudo. A escolha é uma função pura (`kept_entries`), com teste unitário;
  - preenchimento automático e oferta de salvar senhas desligados (`general_autofill_enabled(false)` no construtor da janela e `ICoreWebView2Settings4`).
- **Na interface, em qualquer WebView** (`apps/desktop/src/app/app-behavior.ts` e `styles/warden.css`):
  - `user-select: none` no `body`, com exceção para campos (`input`, `textarea`, `[contenteditable]`), o editor de configs (`.cm-editor`), o console e os logs (`.console__log`, `pre`, `code`, `.code`), caminhos (`.path`), mensagens de erro (`.alert--danger`) e a classe `.selectable`;
  - `img, a { -webkit-user-drag: none }`;
  - um listener global de `contextmenu` que cancela o menu fora de campo editável e sem seleção (a proteção do Linux e a reserva do Windows);
  - os botões 3 e 4 do mouse (voltar e avançar) não navegam (`mousedown`, `mouseup` e `auxclick` cancelados).
  - Alt+←, Alt+→ e as teclas "voltar" e "avançar" de alguns teclados não navegam fora de um campo editável (reserva do atalho do WebView2).
- O clique direito na barra de título própria continua abrindo o menu da janela do Windows: a barra é região não-cliente (`app-region: drag`), tratada pelo sistema, fora do DOM e do `ContextMenuRequested`.

## Alternativas consideradas

- **Só o `default_context_menus` do `wry` (desligar o menu do Edge inteiro):** some também o recortar, copiar e colar dos campos, que o dono usa. O Tauri não expõe a opção.
- **Bloquear as teclas no JavaScript (`keydown` com `preventDefault`):** o WebView2 trata parte dos atalhos antes da página, e a lista teria de acompanhar cada versão do Edge; a opção do WebView2 é a forma suportada.
- **Só o listener de `contextmenu` na página, também no Windows:** não dá para mostrar um menu nativo filtrado; a interface teria de desenhar o próprio menu de edição.

## Consequências

- A seleção de texto passa a ser exceção: um texto novo que o usuário precise copiar (um código, um caminho, uma mensagem) usa uma das classes acima ou `.selectable`.
- As DevTools (F12, Ctrl+Shift+I) também deixam de abrir pelo teclado, inclusive no build de debug do Windows. Para depurar a interface ali, use o `edge://inspect` com a depuração remota do WebView2, a mesma do E2E (QUALITY §13.5).
- O E2E `e2e/app-nao-pagina.e2e.ts` confere que F5 e Ctrl+R não recarregam e que o clique direito em área vazia não abre menu (num campo de texto, abre). No Linux, onde o WebKitGTK não tem atalhos de navegador, a parte das teclas passa com ou sem a mudança; ela vale de verdade no E2E do Windows (noturno ou local).
