# ADR-0059 — Envio da publicação pelo protocolo "smart HTTP" do git sobre a `warden-http`

- **Status:** proposta (V-03; o orquestrador confirma na revisão) · **Data:** 2026-10-13 · **Origem:** decisão técnica da V-03 · **Complementa:** [ADR-0015](0015-versionamento-sobre-git.md) e [ADR-0028](0028-publicacao-no-github.md)

## Contexto

A ARCHITECTURE §11.1 previa o envio ao GitHub pelo `push` da `git2` com o recurso `https` ligado, credencial por callback. Ao implementar a V-03:

- o `https` da `git2` compila a libgit2 com **OpenSSL** no Linux (`openssl-sys`) e com **WinHTTP** no Windows: duas pilhas de TLS fora da `warden-http`, que já usa rustls, a verificação de certificados do sistema, o User-Agent, os limites e as mensagens de erro do Warden (ARCHITECTURE §17);
- o callback de credencial da libgit2 entrega o token a código C e a uma configuração de transporte que o Warden não controla;
- a troca protegida ("substituir o GitHub", §11.1 item 5) teria uma janela de corrida, porque o `git2` não oferece o `--force-with-lease`.

## Decisão

- A libgit2 continua montando tudo localmente: a árvore publicada, o commit da linha de publicação, a tag anotada e o **pacote de objetos** (`git2::PackBuilder`). O recurso `https` da `git2` continua desligado.
- O envio é o mesmo pedido HTTP do git oficial (`git-receive-pack`, protocolo "smart HTTP" v0), feito pela `warden-http`: `GET <repo>.git/info/refs?service=git-receive-pack` e `POST <repo>.git/git-receive-pack` com `<antigo> <novo> <ref>` de cada referência, o pacote e `report-status`. Código em `crates/warden-versioning/src/publish/transport.rs`.
- O token vai só no cabeçalho `Authorization: Basic base64("x-access-token:<token>")`, marcado como sensível na `warden-http` (nunca registrado, nunca repassado a outro site num redirecionamento, nunca em URL nem em `.git/config`).
- O valor antigo de cada referência vai no pedido e o servidor só atualiza se ele ainda for o atual: a substituição protegida passa a ser atômica no próprio servidor, sem a janela de corrida citada na §11.1. Quando o servidor anuncia `atomic` (o GitHub anuncia), a `main` e a tag mudam juntas ou nenhuma muda.
- O motivo de uma recusa (`ng <ref> <motivo>`) é texto livre do servidor (o `git-receive-pack` diz só "failed to update ref" ou "atomic transaction failed"): para saber se foi divergência, o Warden relê as referências e compara com os valores antigos que mandou.
- Se o envio chegou ao servidor e só a resposta se perdeu, a próxima tentativa encontra lá a `main` e a tag que ela mesma montou (objetos que existem aqui, sobre a ponta da linha de publicação): registra a publicação e segue para a Release, sem enviar de novo nem acusar divergência.
- A `warden-http` ganha `HttpClient::post_bytes` (corpo binário com `Content-Type` próprio, nunca repetido sozinho) e `Request::total_timeout` (o envio do pacote pode passar dos 30 s das chamadas de API).

## Alternativas consideradas

- `git2` com `https`: descartada pelos motivos do contexto.
- Transporte próprio registrado na libgit2 (`git2::transport::register`): exige `unsafe` (proibido no workspace) e um registro global por processo.
- Chamar o `git` instalado: o computador do dono não precisa ter git (ADR-0015).

## Consequências

- Nenhuma biblioteca de TLS nova; o envio passa pelo mesmo proxy e pelas mesmas regras de rede do resto do app.
- Os testes usam um servidor git "smart HTTP" local (sobre um repositório nu) que confere o valor antigo, aceita `atomic` e recusa com as mesmas respostas do `git-receive-pack` real, a API de Releases simulada e o `raw.githubusercontent.com` simulado (`crates/warden-versioning/tests/publicacao/`); os E2E usam o próprio `git receive-pack --stateless-rpc`.
- Só o protocolo v0 com `report-status` é usado, que o GitHub aceita; o protocolo v2 não tem envio.
