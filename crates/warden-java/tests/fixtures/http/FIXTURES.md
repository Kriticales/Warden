# Respostas reais gravadas (warden-java)

Gravadas em 2026-10-05 com `curl`, sem chave (as duas APIs são públicas) e sem dado pessoal.
Servem aos testes com servidor simulado (`wiremock`) e aos testes de leitura dos modelos.

| Arquivo | Pedido |
|---|---|
| `2026-10-05-adoptium-available_releases.json` | `GET https://api.adoptium.net/v3/info/available_releases` |
| `2026-10-05-adoptium-latest-<major>-<os>-x64-jre.json` | `GET https://api.adoptium.net/v3/assets/latest/<major>/hotspot?architecture=x64&image_type=jre&os=<os>&vendor=eclipse` (major 8, 17, 21 e 25; `windows` e `linux`) |
| `2026-10-05-adoptium-version-8-ate-312-<os>-x64-jre.json` | `GET https://api.adoptium.net/v3/assets/version/[8.0.0,8.0.313)?architecture=x64&heap_size=normal&image_type=jre&jvm_impl=hotspot&os=<os>&page_size=20&project=jdk&release_type=ga&sort_method=DEFAULT&sort_order=DESC&vendor=eclipse` |
| `2026-10-05-mojang-java-runtime-all.json` | `GET https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json` |
| `2026-10-05-mojang-jre-legacy-linux-manifest.json` | manifesto do `jre-legacy` do Linux (8u202), `https://piston-meta.mojang.com/v1/packages/c529f68a6febc042e71835a579f44d55a3717c46/manifest.json` (tem `type: link`) |
| `2026-10-05-mojang-epsilon-windows-x64-manifest.json` | manifesto do `java-runtime-epsilon` do Windows (25.0.1), `https://piston-meta.mojang.com/v1/packages/b374544c680d965fb5535977d7cb04c6befe1930/manifest.json` |

Observação registrada na gravação: no endpoint `assets/version`, o limite superior inclusivo
`[1.8.0,1.8.0_312]` devolve o 8u302, porque a API compara pela versão semântica com o build
(`8.0.312+7` > `8.0.312`). Por isso o Warden pede `[8.0.0,8.0.313)` e filtra pelo número da
atualização.
