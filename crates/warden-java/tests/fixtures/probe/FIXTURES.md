# Saídas reais de `java -XshowSettings:properties -version`

Gravadas em 2026-10-05 no Windows 11 (code page 1252, idioma pt-BR) com os JREs Temurin
baixados do Adoptium (zip conferido pelo SHA-256 da API), extraídos numa pasta temporária e
apagados depois. É a saída de erro do programa (onde o Java imprime as propriedades), seguida
da saída padrão.

| Arquivo | JRE |
|---|---|
| `temurin-8-windows-x64.txt` | `OpenJDK8U-jre_x64_windows_hotspot_8u504b01.zip` (`jdk8u504-b01`) |
| `temurin-17-windows-x64.txt` | `OpenJDK17U-jre_x64_windows_hotspot_17.0.20.1_1.zip` (`jdk-17.0.20.1+1`) |
| `temurin-21-windows-x64.txt` | `OpenJDK21U-jre_x64_windows_hotspot_21.0.12.1_1.zip` (`jdk-21.0.12.1+1`) |
| `temurin-25-windows-x64.txt` | `OpenJDK25U-jre_x64_windows_hotspot_25.0.4.1_1.zip` (`jdk-25.0.4.1+1`) |

Anonimização (nenhum dado pessoal no repositório): a pasta temporária virou `C:\exemplo`, o
nome do usuário virou `usuario` e o `java.library.path` (que traz o PATH inteiro do Windows)
ficou só com a primeira linha, seguida de `<PATH removido>`. O resto é a saída original, com o
fim de linha trocado de CRLF para LF (o teste confere as duas formas).
