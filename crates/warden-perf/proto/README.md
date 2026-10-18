# Formato dos perfis do spark

Os `.proto` desta pasta são os do [spark](https://github.com/lucko/spark) (`spark-common/src/main/proto/spark/`), copiados **sem alteração** do commit `3e8745a76eaabe9a4f70aad6283876fb518d2ae8` (2026-10-07). Licença: GPL-3.0-or-later (Copyright (c) lucko (Luck) e colaboradores), texto em [`LICENSE.txt`](LICENSE.txt); registro no `THIRD_PARTY.md` (seção "Trechos copiados"). O Warden é privado e nunca distribuído (ADR-0004).

| Arquivo | O que descreve |
|---|---|
| `spark/spark.proto` | Dados comuns: plataforma, estatísticas do sistema e do jogo, janelas de tempo (TPS e tempo por tick), mods. |
| `spark/spark_sampler.proto` | O perfil de processamento (`.sparkprofile`): `SamplerData` com as threads e a árvore de chamadas. |
| `spark/spark_heap.proto` | O resumo da memória (`.sparkheap`): `HeapData` com as classes, instâncias e bytes. |

O `spark_ws.proto` (canal ao vivo do visualizador) ficou de fora: o Warden só lê arquivos salvos.

## Código Rust gerado

`../src/spark/generated.rs` é a saída do `prost-build` 0.14 para os três arquivos, sem edição à mão. Para gerar de novo (depois de trocar os `.proto` por uma versão nova do spark), num projeto à parte, fora do workspace:

```toml
# Cargo.toml
[dependencies]
prost-build = "0.14"
protox = "0.9"   # compila os .proto em Rust puro, sem o protoc
```

```rust
// src/main.rs: cargo run -- <esta pasta> <pasta de saída>
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let files = ["spark/spark.proto", "spark/spark_sampler.proto", "spark/spark_heap.proto"];
    let descriptors = protox::compile(files, [&args[1]]).unwrap();
    let mut config = prost_build::Config::new();
    config.out_dir(&args[2]);
    config.compile_fds(descriptors).unwrap();
}
```

Depois, copie `spark.rs` da pasta de saída para `src/spark/generated.rs` (mantendo o cabeçalho) e rode `cargo nextest run -p warden-perf`: os golden tests com os perfis reais (`tests/spark/`) conferem que o formato continua lido.
