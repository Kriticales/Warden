# ADR-0049 — Cópia fixa do packwiz com patches nossos para cada defeito encontrado

- **Status:** aceita · **Data:** 2026-10-04 · **Origem:** decisão do dono (política da cópia) + decisão técnica (tarefa D7) · **Complementa:** [ADR-0007](0007-sidecar-packwiz.md)

## Contexto

A [ADR-0007](0007-sidecar-packwiz.md) fixou o packwiz num commit e criou o primeiro patch (a chave da CurseForge em tempo de execução). A F0-03 confirmou que o commit `ef87d96` é a ponta da única branch do packwiz, sem tags nem releases, e que pull requests úteis ao Warden estão parados lá (`third_party/packwiz/README.md`). A pesquisa R7 (`docs/research/08-concorrencia-e-mercado.md` §8.3, risco R2) registrou manutenção fraca do packwiz e do packwiz-installer (este sem commit desde 06/2024).

A F0-03 e a P1-01 já acharam um defeito do packwiz original: com `--pack-file` absoluto, todo `add` falha depois de gravar o metadado da primeira dependência (exige `--meta-folder-base` absoluto), e os padrões ancorados do `.packwizignore` deixam de valer. O Warden contornou chamando o packwiz de outro jeito, mas a pergunta de fundo ficou: quando o packwiz tiver um defeito que não dá para contornar, o que fazemos?

Em 04/10/2026 o dono decidiu manter a política "cópia fixa + patches nossos" e não fazer fork do packwiz-installer agora (ADR-0051).

## Decisão

- O Warden continua usando **uma cópia do packwiz fixada por commit** em `third_party/packwiz/COMMIT`, compilada pelo `cargo xtask build-packwiz`, sem fork publicado no GitHub.
- **Todo defeito do packwiz que o Warden encontrar vira um registro em `third_party/packwiz/`**:
  - se o Warden contorna o defeito chamando o packwiz de outro jeito, o contorno e o teste que mostra o defeito ficam descritos no `third_party/packwiz/README.md` (seção "Cuidado ao chamar o packwiz") e na ARCHITECTURE §6.3;
  - se não há contorno seguro, vira um patch novo e mínimo em `third_party/packwiz/patches/NNNN-<assunto>.patch`, com o parágrafo em português explicando o porquê, um teste que falha sem o patch e passa com ele, e a entrada no `README.md` da pasta.
- Um patch só entra pela tarefa dona da chamada ao packwiz (hoje a P1-02) ou por uma tarefa própria do orquestrador, nunca como efeito colateral de outra tarefa.
- Quando um defeito for corrigido no packwiz oficial, a atualização do commit (tarefa própria, ADR-0007) remove o patch ou o contorno correspondente.

## Alternativas consideradas

- **Fork do packwiz no GitHub:** facilita publicar correções, mas cria um projeto a manter e não muda o que o Warden embute (o mesmo binário compilado de um commit). Fica para quando houver muitos patches.
- **Mandar as correções ao packwiz e esperar:** os pull requests abertos mostram que pode não haver revisão; o Warden não pode depender disso.
- **Contornar tudo em Rust:** já é a regra para leitura e escrita (ADR-0006); refresh, validação e exportação continuam no packwiz para garantir o formato.

## Consequências

- `third_party/packwiz/README.md` é o lugar único para saber o que o Warden muda ou evita no packwiz.
- Cada patch novo exige rodar a suíte de integração com o packwiz real (como a atualização do commit).
- O primeiro registro desta política é o contorno de `--pack-file` (ARCHITECTURE §6.3).
