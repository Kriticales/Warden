# ADR-0053 — Isolamento do app de desenvolvimento: cofre de teste automático e identificador próprio

- **Status:** aceita · **Data:** 2026-10-04 · **Origem:** decisão técnica (tarefa F0-05; registrada na D7) · **Complementa:** [ADR-0048](0048-desenvolvimento-no-windows.md)

## Contexto

A [ADR-0048](0048-desenvolvimento-no-windows.md) manda o app de desenvolvimento usar `WARDEN_DATA_ROOT` e `WARDEN_SECRET_BACKEND=file:<pasta>` para nunca tocar nos dados e no cofre reais do dono, e restringir a instância única a cópias com a mesma pasta. Na F0-05 apareceram dois buracos: quem definisse só `WARDEN_DATA_ROOT` (esquecendo a segunda variável) cairia no cofre real; e o `tauri-plugin-single-instance` identifica a instância pelo `identifier` do app, então o app de desenvolvimento e o Warden instalado se enxergavam.

## Decisão

- **Cofre de teste automático:** só em build de debug, com `WARDEN_DATA_ROOT` definida e sem `WARDEN_SECRET_BACKEND`, o Warden usa o cofre de teste em arquivo em `<WARDEN_DATA_ROOT>/cofre-de-teste`. `WARDEN_SECRET_BACKEND` com valor diferente de `file:<pasta>` faz o app recusar abrir (melhor não abrir do que cair no cofre real por engano).
- **Identificador de desenvolvimento:** só em build de debug, com `WARDEN_DATA_ROOT` definida, o identificador do app passa a ser `dev.kriticales.warden.dev-<hash>`, em que `<hash>` é o FNV-1a de 64 bits (16 dígitos hexadecimais) do caminho normalizado da pasta (no Windows, sem diferença de maiúsculas e de barras). A instância única vale só entre cópias com a mesma pasta, e o app de desenvolvimento nunca foca o Warden instalado.
- Em build de release nada disso existe.

## Alternativas consideradas

- **Exigir as duas variáveis:** depende de memória humana; o erro cai justamente no cofre do dono.
- **Desligar a instância única em debug:** permitiria duas cópias de desenvolvimento escrevendo na mesma pasta.

## Consequências

- O perfil do WebView2 também fica dentro da pasta de dados (`AppPaths::webview_dir`, ARCHITECTURE §13), então o app de desenvolvimento não compartilha cookies nem cache do WebView com o instalado.
- Os testes da `warden-app` conferem o hash para caminhos equivalentes no Windows.
