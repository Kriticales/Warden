# Versão de teste do Warden no Windows

> Para o dono do projeto. Tarefa F0-04, [ADR-0048](decisions/0048-desenvolvimento-no-windows.md).

A cada entrega você pode instalar a versão mais recente do Warden neste computador para testar. Instalar de novo **atualiza** o Warden e **não apaga** seus packs, configurações nem chaves.

## Jeito mais fácil: duplo clique

1. Abra a pasta do projeto: `C:\Users\solel\orca\projects\Warden\scripts\`.
2. Dê **duplo clique** em `versao-de-teste.cmd`.
3. Uma janela preta abre e mostra o que está acontecendo:
   - procura no GitHub a versão mais recente da `main` que passou em todos os testes;
   - mostra a **versão** e o **commit** que vão ser instalados (e a versão que está instalada agora);
   - se o Warden estiver aberto, pergunta `Fechar o Warden agora? [s/N]`. Digite `s` e Enter para continuar; qualquer outra resposta cancela sem instalar nada;
   - instala só para o seu usuário, sem pedir senha de administrador;
   - confere que os dados do app ficaram intactos e abre o Warden.
4. No fim aparece `Tudo certo. Pode fechar esta janela.` Aperte qualquer tecla.

No Warden, a tela **Sobre** mostra a versão e o commit. Eles devem ser os mesmos que a janela preta mostrou.

A primeira vez demora um pouco mais, porque o programa que faz a instalação (o `xtask`) é preparado antes. Nas outras vezes leva menos de um minuto, dependendo da internet.

### Se aparecer uma mensagem de erro

A janela não fecha sozinha: leia a última frase, que diz o que fazer. As mais comuns:

| Mensagem | O que fazer |
|---|---|
| O GitHub CLI (gh) não está conectado à sua conta | Abra o PowerShell, rode `gh auth login`, siga as instruções e tente de novo. |
| Nenhuma execução da CI terminada com sucesso... | A versão nova ainda não ficou pronta (a CI leva uns 40 minutos) ou falhou. Espere, ou avise o orquestrador. |
| O Warden não fechou em 20 s | O Warden pode estar perguntando algo na janela dele. Responda, feche o Warden e tente de novo. |

Em todos esses casos **nada foi instalado** e nada mudou no computador.

## Gerar a versão neste computador

Sem esperar a CI, a partir do código que está na pasta. Abra o PowerShell na pasta do projeto e rode:

```powershell
cargo xtask preview
```

Leva alguns minutos (compila o app inteiro). Faz o resto igual ao duplo clique. A janela informa de que branch saiu a versão e se havia alterações ainda sem commit.

Outras formas:

```powershell
cargo xtask preview --from-ci                        # o mesmo que o duplo clique
cargo xtask preview --from-ci --branch <nome>         # a versão da CI de outra branch
```

A versão de outra branch só existe se a CI do Windows rodou nela: na `main` ela roda sempre; nas outras, só quando o orquestrador pede.

## O aviso "O Windows protegeu o computador"

O instalador do Warden **não tem assinatura digital**. A assinatura é um certificado pago que prova quem fez o programa, e a decisão D4 do projeto foi não comprar um, porque o Warden é só para você. Por isso o Windows pode desconfiar dele.

- **Pelo duplo clique ou pelo `cargo xtask preview`**, o aviso normalmente **não aparece**: o instalador é baixado pelo `gh` (ou gerado aqui), e o Windows só desconfia de arquivos baixados pelo navegador.
- **Se você baixar o instalador pelo navegador** (por exemplo, da página da CI no GitHub) e der duplo clique nele, aparece uma tela azul **"O Windows protegeu o computador"**. Para instalar mesmo assim:
  1. clique em **Mais informações**;
  2. confira que o aplicativo é `Warden_..._x64-setup.exe` e o fornecedor aparece como **Fornecedor desconhecido**;
  3. clique em **Executar assim mesmo**.

O antivírus também pode estranhar o `warden-app.exe` ou o `packwiz.exe` pelo mesmo motivo. **Não desligue o antivírus nem crie exceções** por conta própria: tire um print do aviso e mande ao orquestrador.

## Onde ficam as coisas

| O quê | Onde |
|---|---|
| O programa | `%LOCALAPPDATA%\Warden\` (com o `warden-app.exe` e o `packwiz.exe`) |
| Configurações | `%APPDATA%\dev.kriticales.warden\` |
| Dados locais, cache e registros | `%LOCALAPPDATA%\dev.kriticales.warden\` (os registros ficam em `logs\`) |
| Seus packs | `Documentos\Warden\` |
| Chaves (CurseForge, Gemini, GitHub) | Gerenciador de Credenciais do Windows, ou o arquivo `.env` nas configurações, se você escolheu essa opção no app |

Para abrir uma dessas pastas, copie o caminho (por exemplo `%APPDATA%\dev.kriticales.warden`) e cole na barra de endereço do Explorador de Arquivos.

Instalar de novo troca só o programa: o comando confere, antes e depois da instalação, que nenhum arquivo das pastas de configurações e de dados locais mudou.

## Como desinstalar

1. Abra **Configurações do Windows** → **Aplicativos** → **Aplicativos instalados**.
2. Procure **Warden**, clique nos três pontos e em **Desinstalar**.
3. A janela do desinstalador pode estar em inglês. Ela tem a caixa **Delete the application data** ("apagar os dados do aplicativo"):
   - **desmarcada** (o padrão): remove só o programa; configurações, registros e chaves continuam, e voltam quando você instalar de novo;
   - **marcada**: apaga também as pastas de configurações e de dados locais.

Seus packs em `Documentos\Warden\` e as chaves no Gerenciador de Credenciais **nunca** são apagados pelo desinstalador.

## O que mandar ao orquestrador quando algo der errado

1. **O texto da janela preta inteiro**: clique com o botão direito na janela, **Selecionar tudo**, Enter para copiar, e cole na conversa. Ou um print, se for curto.
2. **A versão e o commit** da tela Sobre do Warden (se ele chegou a abrir).
3. **O que você estava fazendo** quando deu errado, em poucas palavras.
4. Se o Warden abriu e o problema foi dentro dele: o arquivo de registro mais recente de `%LOCALAPPDATA%\dev.kriticales.warden\logs\`. Os registros não guardam suas chaves.

## Para os agentes

- O comando é o `cargo xtask preview` (`xtask/src/preview.rs`). Com `--from-ci`, escolhe a execução mais recente da branch que **terminou com sucesso** e ainda tem o artefato `warden-windows-<sha>` do `windows.yml` (válido por 30 dias), baixa o instalador para `<target>/preview/warden-windows-<sha>/` e instala. Sem a opção, roda `cargo xtask build-packwiz` e `cargo tauri build --bundles nsis` e usa o instalador de `<target>/release/bundle/nsis/`.
- A instalação é `Warden_<versão>_x64-setup.exe /S`, só para o usuário atual. `--install-dir <pasta>` instala em outra pasta (`/D=`), útil para testar sem tocar na instalação do dono; desinstale depois com `<pasta>\uninstall.exe /S`, senão a próxima instalação sem `--install-dir` reaproveita a pasta registrada.
- Instalar o Warden nesta máquina é só para o dono ou o orquestrador (QUALITY §13.1), ou para a tarefa que pedir isso explicitamente.
- O instalador do Tauri, em modo silencioso, fecha sozinho qualquer `warden-app.exe` do usuário, inclusive o de `cargo xtask dev`. Por isso o `preview` pergunta antes e pede para a janela fechar normalmente (sem forçar).
