# A-07: revisão das funções avançadas (D4)

Revisão das telas das funções avançadas antes da versão 1, com olhar de revisor: textos contra o glossário (QUALITY §8.2), estados (vazio, carregando, erro), foco visível, contraste e o design system Deep Dark. Este arquivo foi escrito **antes** de qualquer mudança em componente ou texto: a lista da seção "O que será ajustado" é tudo o que a A-07 muda fora do E2E e do roteiro.

## Como foi feita

- **Textos:** varredura dos namespaces de i18n das telas da D4 (`busca-com-servidor`, `busca-do-culpado`, `ia`, `ia-propostas`, `raio-x`, `grafo`, `saude`, `console`, `memoria`, `perfil-desempenho`, `desempenho`, `servidor`, `perfis`, `jogador`, `configs-formulario`, `configs-padrao`, `scripts`, `opcionais`, `trocar-versao`, `modpacks`, `importar`, `exportar-formatos`, `pacote-servidor`, `instancia-prism`, `armazenamento`, `descoberta`, `travamentos`) procurando cada termo da coluna "Não usar" do glossário, português europeu, exclamação e marketing; cada ocorrência foi lida no contexto e comparada com a SPEC e o protótipo.
- **Estados:** cada bloco que lê dados (raio-x, grafo, saúde, armazenamento, opcionais, trocar versão, descoberta, modpacks, importar, exportar) foi conferido no código: carregando com texto próprio, erro pelo componente comum de erro (`appErrorMessage`) e vazio com frase própria.
- **Foco, contraste e `axe`:** os 5 fluxos avançados do novo `apps/desktop/e2e/advanced-flows.e2e.ts` rodam o `axe` (violações sérias e críticas) em cada tela por onde passam, inclusive as que nenhum E2E media: a página de descoberta (início, busca, seleção e pré-visualização), o painel de mods de um modpack, o aviso "O pack foi alterado fora do Warden", o diálogo de dependências de vários itens, o raio-x nos detalhes do mod, os formatos Prism e `.mrpack` de Exportar e Configurações → Armazenamento. O menu ▾ do Testar e a página de descoberta são percorridos só pelo teclado, conferindo o foco visível (`:focus-visible` com contorno desenhado) e a volta do foco ao ▾ quando o diálogo fecha.
- **Telas já cobertas por outros E2E com `axe`** (não repetidas aqui): busca do culpado com e sem servidor (`bisect.e2e.ts`, `bisect-server.e2e.ts`), IA e propostas (`ai.e2e.ts`, `ai-propostas.e2e.ts`), grafo e saúde (`grafo.e2e.ts`, `saude.e2e.ts`), memória ao vivo e perfil de desempenho (`heap.e2e.ts`, `perf-profile.e2e.ts`), servidor local (`server.e2e.ts`), testar como o jogador recebe (`player-test.e2e.ts`), configs em formulário, padrão e scripts (`configs.e2e.ts`, `configs-busca.e2e.ts`, `scripts.e2e.ts`), importar (`import.e2e.ts`), exportar `.mrpack`, zip da CurseForge e pacote para servidor (`export-formats.e2e.ts`, `server-pack.e2e.ts`) e o aviso de arquivo alterado fora do Warden nas configs (`configs.e2e.ts`).

## O que está certo e fica como está

- **"Este mod não altera o código do jogo por mixins."** (`raio-x.ts`): o glossário evita "mixin" na interface, mas a frase é a da SPEC (T07), escrita assim de propósito para quem conhece o termo. Fica.
- **"Arquivo fora das lojas: o Warden não copia jars de pacotes de terceiros"** (`modpacks.ts`): "lojas" fala do Modrinth e da CurseForge, não da página de descoberta; a frase é a da SPEC (T08) e do protótipo. Fica.
- **"Perfil do spark"** e **"Testar com perfil de desempenho"** (`perfil-desempenho.ts`): são o arquivo de perfil do spark e o nome do teste na SPEC (T26), não o "perfil do teste" dos ajustes. Fica.
- **"chat do jogo"** (`scripts.ts`, `ia.ts`): o chat dentro do Minecraft, não a conversa com a IA. Fica.
- **"só backup"** (`pacote-servidor.ts`): é o nome da opção do repositório privado em Publicar versão (`publicar.ts`). Fica.
- Aspas retas e curvas convivem nos textos da D4; a QUALITY não fixa uma forma e trocar todas mudaria dezenas de testes sem ganho para quem usa. Fica.
- Estados: os blocos conferidos têm carregando, erro (pelo componente comum) e vazio próprios; nenhum ajuste.

## O que será ajustado

| # | Arquivo | Texto ou componente | Motivo |
|---|---|---|---|
| 1 | `apps/desktop/src/i18n/pt-BR/servidor.ts` (`opcoes.poucaTexto`) | "…ou use um perfil com menos memória." → "…ou use um perfil do teste com menos memória." | Glossário (QUALITY §8.2): o conjunto de ajustes com nome é sempre "perfil do teste"; o mesmo aviso da busca do culpado com servidor (`busca-com-servidor.ts`) já diz assim. Teste de componente em `features/test/server/Server.test.tsx`. |

Defeitos que os fluxos de ponta a ponta encontrarem entram no relatório da tarefa: os pequenos, consertados com teste; os grandes ou fora da posse, registrados com a tela, os passos e a gravidade.
