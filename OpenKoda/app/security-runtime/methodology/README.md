# Guia de análise estática

O aplicativo incorpora o guia de avaliação de caminhos de ataque do pacote
`@openai/codex-security` 0.10.0 da OpenAI, sob a licença Apache 2.0. A cópia
de `attack-path-facts.md` preserva os bytes da distribuição npm. O arquivo
original fica em `_bundled_plugin/skills/attack-path-analysis/references/`.
O núcleo Rust inclui esse texto na instrução da revisão em português, antes
do protocolo de consulta à cópia autorizada.

A licença acompanha este diretório e a distribuição original do SDK. O teste
`tests/methodology.test.mjs` compara versão, licença e hashes com o pacote
instalado por meio do lockfile.
