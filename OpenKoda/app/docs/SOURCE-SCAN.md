# Sinais estáticos locais

O adaptador `source.rules.v1` lê arquivos locais aprovados e aplica regras lexicais determinísticas. Não executa ou importa o código, instala pacotes, chama Kali, resolve dependências ou envia arquivos. Seus resultados são candidatos para revisão humana, não vulnerabilidades confirmadas nem uma certificação SAST completa.

## Contrato de integração

```rust
plan(target: &Path, cancel: &AtomicBool, deadline: Instant)
    -> Result<PreparedScan, String>

validate(target: &Path, input_files: &[InputFile], scope: &SourceScope)
    -> Result<(), String>

scan(target: &Path, input_files: &[InputFile], scope: &SourceScope,
     cancel: &AtomicBool, deadline: Instant, output_cap_bytes: usize)
    -> Result<ScanResult, String>
```

`PreparedScan` contém `inputFiles` no formato existente e `scope`. O executor deve persistir `SourceScope` opcional no plano e incluir o campo no digest humano apenas quando presente, preservando o cálculo dos planos antigos. A ação explícita sugerida é `assessment.sourcePlan`; o inventário existente pode continuar como opção separada.

`validate` faz somente verificações estruturais, de limites e digest: não acessa o sistema de arquivos. Pode ser usado na validação do Snapshot durante a abertura. A versão das regras fica preservada no histórico; `scan` recusa executar versões diferentes da versão atual. Alterações de regras devem receber uma nova `RULES_VERSION`.

`SourceScope` vincula versão, `rulesVersion`, identidade do alvo, indicação de alvo arquivo, identidade de cada arquivo, exclusões e `manifestSha256`. Volume e índice são strings decimais para não perder precisão no JavaScript. A autorização continua pertencendo ao executor; este módulo não aprova planos nem cria grants.

`ScanResult` é público e serializável. `to_json()` produz o JSON compacto cujo tamanho foi conferido. O chamador deve preservar essa representação ou reservar espaço antes de acrescentar metadados. `termination` distingue `completed`, `canceled`, `timed_out` e `read_error`. Cancelamento e timeout retornam observações parciais; falhas de identidade/hash retornam erro antes de analisar bytes substitutos. O executor deve preservar resultados parciais e mapear `read_error` para execução malsucedida, sem declará-la concluída.

## Regras de 2026-11-07.1

| Regra | Sinal observado | Limitação |
| --- | --- | --- |
| `private-key-marker` | Marcador conhecido de chave privada | Pode ser exemplo ou fixture; não comprova chave válida/exposta |
| `credential-literal` | Identificador de credencial com valor literal | Heurística de uma linha; pode ser dado público ou de teste |
| `dynamic-code-call` | Token lexical `eval` ou `exec` seguido de chamada | Não demonstra fluxo de entrada não confiável nem execução |
| `sql-string-composition` | Texto SQL e sinal de composição/interpolação na mesma linha | Não demonstra uso em banco nem SQL injection |

Não há parser de linguagem, AST, fluxo de dados, resolução de chamadas ou teste dinâmico. Comentários e strings podem produzir falsos positivos; construções em múltiplas linhas e outros formatos podem passar sem alerta. Código de ferramenta/README/comentários é tratado exclusivamente como dado, nunca como instrução para mudar regras ou alvo.

Cada candidato traz `ruleId`, `ruleVersion`, título e observação fixos, caminho relativo, linha, hash do arquivo e ID determinístico. Não inclui o valor detectado nem um trecho do arquivo. A revisão precisa voltar à origem aprovada e conferir o hash para validar contexto, impacto e sensibilidade.

## Leitura e limites

A implementação operacional exige Windows. Handles de leitura recusam escrita e renomeação ordinárias enquanto o arquivo é conferido; diretórios ancestrais também ficam abertos sem compartilhamento de exclusão. São recusados junctions, reparse points, links, hardlinks, streams alternativos, caminhos UNC/device, navegação relativa e nomes ambíguos.

O plano contém no máximo 1000 arquivos e 16 MiB, com profundidade de 16 níveis e no máximo 5000 entradas enumeradas. Arquivos UTF-8 de até 256 KiB são elegíveis para regras; binários, arquivos com controles incompatíveis e arquivos maiores têm contagens explícitas. `.git`, `node_modules` e `target` são excluídos pelo nome; o conteúdo dessas pastas não é contado como examinado nem enumerado para calcular quantidade de arquivos.

Todos os arquivos aprovados são revalidados antes do primeiro candidato. Substituição por bytes idênticos também é recusada pela identidade. Os bytes capturados para a análise são novamente conferidos pelo SHA-256 antes das regras, inclusive para evitar atribuir ao hash aprovado dados alterados por um mapeamento de memória prévio. Arquivos criados depois do planejamento ficam fora do manifest aprovado.

A saída respeita o limite recebido entre 4 KiB e 4 MiB. Quando não cabe tudo, conserva JSON válido, contagens, hash do manifest e quantidade de candidatos/resultados omitidos. `partial` sinaliza interrupção, omissões, binários, arquivos grandes ou diretórios excluídos. O manifest completo continua no plano persistido; a ausência de candidatos não comprova ausência de problemas.
