# ADR-0020 — Bibliotecas do frontend

- **Status:** aceita · **Data:** 2026-10-01 · **Origem:** decisão técnica (A1)

## Contexto

A interface tem muitas telas de lista e formulário, um editor de configs, um console de alto volume e precisa ser acessível e fácil de manter por vários agentes em paralelo.

## Decisão

| Uso | Escolha |
|---|---|
| Rotas | TanStack Router, rotas por arquivo (`routeTree.gen.ts` gerado, não versionado) |
| Dados do backend | TanStack Query, invalidado pelo evento `pack-changed` |
| Estado de interface | Zustand (só estado local de UI) |
| Componentes | shadcn/ui (Radix UI) + Tailwind CSS 4 + lucide-react |
| Formulários | React Hook Form + Zod |
| Editor de configs e diferenças | CodeMirror 6 + `@codemirror/merge` |
| Listas longas | TanStack Virtual |
| Textos | i18next + react-i18next |
| Conteúdo remoto | react-markdown + rehype-sanitize; DOMPurify |
| Testes | Vitest + Testing Library + `mockIPC` + vitest-axe; WebdriverIO + tauri-driver |

## Alternativas consideradas

- React Router: parâmetros menos tipados; rotas centralizadas geram conflito entre agentes.
- Redux: cerimônia desnecessária com TanStack Query cuidando dos dados.
- MUI/Mantine: mais pesados e menos controláveis que componentes copiados para o repositório.
- Monaco: pesado e exige *web workers* no WebView.
- Playwright: não dirige o WebView do Tauri de forma oficial nas duas plataformas.

## Consequências

- Primitivos de UI ficam no repositório (`components/ui/`) e podem ser ajustados.
- O E2E roda contra o app real em Linux e Windows.
