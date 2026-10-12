# ADR-0059 — Relato enviado direto ao GitHub: login por device flow de um GitHub App

- **Status:** aceita · **Data:** 2026-10-12 · **Origem:** decisão do dono (pedido depois de usar a UI-02; escolhas 1A e 2A) · **Complementa:** [ADR-0025](0025-segredos-cofre-ou-env.md)

## Contexto

A UI-02 monta a issue do relato já preenchida e abre o navegador; o dono ainda precisa clicar em "Submit new issue" na página do GitHub e não consegue anexar prints pelo Warden. Ele usou e não gostou. Pediu que o botão **Enviar** crie a issue direto, sem abrir o navegador, com categoria e prints.

Criar issue pela API exige um token com permissão de escrever issues no repositório do Warden. Havia três caminhos:

1. o dono cola um token pessoal (PAT) em Configurações, como a ADR-0025 já prevê para o GitHub;
2. um OAuth App com o fluxo de dispositivo (*device flow*);
3. um **GitHub App** com o *device flow*.

O PAT exige que o dono entenda escopos, datas de validade e onde gerar o token, e um PAT clássico com `repo` dá acesso a todos os repositórios dele. O OAuth App só tem escopos largos (`public_repo` ou `repo`). O GitHub App tem permissões finas por recurso, só vale nos repositórios em que foi instalado e entrega tokens de usuário que expiram.

Para os prints, o dono escolheu guardá-los no próprio repositório (2A): a API do GitHub não tem upload de imagem para issues.

## Decisão

- **Login:** "Entrar com o GitHub" em Configurações → Chaves e contas, pelo *device flow* de um **GitHub App do dono**: o Warden pede o código (`POST https://github.com/login/device/code`), mostra o código de 8 letras, abre `https://github.com/login/device` no navegador e consulta `POST https://github.com/login/oauth/access_token` respeitando o `interval` e o `slow_down` até a aprovação, a recusa (`access_denied`) ou o fim do prazo (`expired_token`). O usuário pode cancelar a espera.
- **Permissões mínimas do GitHub App:** *Issues*: leitura e escrita; *Contents*: leitura e escrita (prints na branch `relatos-anexos`). *Metadata*: leitura é obrigatória e automática no GitHub. Nada além: sem webhook, sem permissões de organização nem de conta.
- **Instalação:** só no repositório `Kriticales/Warden` ("Only select repositories"); o app é privado do dono ("Only on this account"). Mesmo com o token, o Warden só fala com esse repositório: o endereço da API e o repositório são constantes no código, trocados só em build de debug pelo servidor simulado dos testes (`WARDEN_API_BASE_GITHUB`, `WARDEN_LOGIN_BASE_GITHUB`), como os demais `WARDEN_API_BASE_*` (ARCHITECTURE §17).
- **Client ID:** é público no *device flow* (não existe *client secret* no app instalado). Fica numa constante única, `warden_github::CLIENT_ID`. Enquanto estiver vazia, "Entrar com o GitHub" aparece indisponível com a explicação, e o relato continua pelo jeito da UI-02 ("Abrir no GitHub"). Em build de debug, `WARDEN_GITHUB_CLIENT_ID` troca o valor, só para os testes.
- **Tokens que expiram:** o GitHub App fica com "Expire user authorization tokens" ligado. O token de acesso vale 8 h e o *refresh token*, 6 meses. O Warden renova sozinho antes de usar quando faltam menos de 5 minutos (e uma vez depois de um 401), grava os dois tokens novos (o GitHub invalida os antigos na renovação) e, se a renovação for recusada, desconecta e pede para entrar de novo. Tokens do *device flow* renovam sem *client secret*.
- **Onde ficam:** o token de acesso fica no mesmo lugar do token do GitHub de hoje (`SecretKind::Github`, conta `github-token`, `GITHUB_TOKEN` no `.env`); o *refresh token* ganha um lugar próprio (`SecretKind::GithubRefresh`, conta `github-refresh-token`, `GITHUB_REFRESH_TOKEN` no `.env`), que segue o modo escolhido (cofre ou `.env`) e a troca de modo. O nome da conta e as datas de validade, que não são segredo, ficam em `github-login.json` na pasta de configuração.
- **Sair do GitHub** apaga os dois tokens do cofre e o `github-login.json`.
- **Segurança:** tokens, *refresh token* e `device_code` nunca vão para registros, mensagens de erro, eventos, o corpo do relato nem arquivos versionados. Os valores circulam como `SecretString`; a redação (`Redactor`) já aprende os valores do cofre, inclusive os dois do login, e os formatos `ghu_…` e `ghr_…`. O corpo do relato continua passando pela redação da UI-02.
- **Prints:** cada imagem vai pela Contents API (`PUT /repos/Kriticales/Warden/contents/anexos/<id>.<ext>`) para a branch `relatos-anexos`, criada, se não existir, a partir de um commit órfão (sem pai, com um `LEIA-ME.md`), sem tocar no `main`. O corpo da issue aponta para `https://github.com/Kriticales/Warden/blob/relatos-anexos/anexos/<id>.<ext>?raw=true`, que funciona com o repositório público ou privado; o `download_url` da API nunca é usado (no repositório privado ele leva um token temporário).

## O que muda na ADR-0025

- O token do GitHub pode vir de dois lugares: colado pelo usuário (como antes) ou gravado pelo login. Os dois ocupam o mesmo `github-token`; entrar com o GitHub substitui um token colado, e "Remover" o token na linha do GitHub tem o mesmo efeito que sair do login no envio direto.
- Passa a existir um quarto segredo, `github-refresh-token` (`GITHUB_REFRESH_TOKEN`), que não aparece como linha em Chaves e contas: ele só existe com o login.

## Consequências

- Nasce a crate `warden-github` (cliente da API do GitHub sobre a `warden-http`: *device flow*, renovação, usuário, labels, branch órfã, Contents API e issues), com o domínio de erros `github`.
- A **V-03 (publicar versão)** poderá reaproveitar o login. As permissões de que ela precisar (criar repositório, escrever conteúdo e Releases nos repositórios dos packs) serão decididas nela, com a instalação do GitHub App ajustada pelo dono; esta ADR não as concede.
- Os prints ficam públicos enquanto o repositório for público, como a issue. A tela avisa antes de enviar e mostra cada miniatura com "Remover".
- A branch `relatos-anexos` cresce com os prints (até 25 MB por relato). Se ficar grande, o dono pode apagá-la; o Warden a recria no próximo envio, e as issues antigas perdem as imagens.
- Criar o GitHub App é tarefa do dono, com o orquestrador, que depois preenche o `CLIENT_ID`.
