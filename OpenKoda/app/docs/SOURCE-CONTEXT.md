# Captura local revisada de fontes

Uma avaliação de código ou arquivos pode guardar uma cópia redigida de arquivos explicitamente selecionados no manifest de um plano `source.rules.v1`. Isso permite que a pessoa selecione depois essa evidência no contexto da IA. A captura não executa código, não cria candidatos e não envia conteúdo à OpenAI. O consentimento para envio permanece separado.

## Uso

1. Preparar o plano de análise estática existente. Esse plano contém caminhos relativos, tamanhos, SHA-256 e identidades Windows do alvo e dos arquivos.
2. Escolher de um a oito caminhos exatos desse manifest. Nenhum arquivo vem selecionado implicitamente pelo backend.
3. Pedir a prévia local e revisar os textos redigidos e os hashes apresentados.
4. Confirmar a gravação da cópia local. O núcleo relê os arquivos escolhidos e confere identidade, tamanho, hash e digest da prévia antes de persistir.
5. Selecionar a evidência capturada na prévia de `ai.context`; somente outra confirmação autoriza o envio à OpenAI.

O plano de execução não precisa ser aprovado para preparar a captura. Sua aprovação não autoriza captura nem envio. Um plano já aprovado deve ser revogado ou encerrado antes da captura. O manifest histórico pode ser usado mesmo após sua validade de execução, desde que a avaliação e seus arquivos continuem elegíveis e sejam novamente conferidos. Isso não renova o plano nem permite executá-lo.

## Contrato

`evidence.sourcePreview` aceita somente:

```json
{
  "assessmentId": "id-da-avaliacao",
  "planId": "id-do-plano-estatico",
  "paths": ["src/example.rs"]
}
```

Retorna `assessmentId`, `planId`, `planDigest`, `manifestSha256`, `paths` ordenados, `totalSourceBytes`, `digest`, `sourceFilesIncluded:true`, `sentToProvider:false`, `notice` e `files`. Cada arquivo contém:

```json
{
  "path": "src/example.rs",
  "bytes": 20,
  "sourceSha256": "hash-dos-bytes-originais",
  "redactedSha256": "hash-do-texto-redigido",
  "text": "texto revisável",
  "redactionChanged": true
}
```

`evidence.sourceCapture` recebe os mesmos três campos de entrada, além de `digest` da prévia e `acknowledgeLocalCapture:true`. Retorna uma `Evidence` normal: `id`, `name`, `content`, `sha256`, `createdAt`. A captura é idempotente para o mesmo plano, seleção e conteúdo redigido; a ordem de seleção é normalizada. Confirmar novamente retorna a mesma evidência, sem outro evento.

O `content` da evidência é JSON com `schema:"openkoda.source-context.v1"`, `origin:"host_selected_source_redacted_copy"`, `captureDigest` e `source`. `source` contém os IDs da avaliação/plano, digest do plano, hash do manifest, total de bytes de origem e os arquivos redigidos. `captureDigest` vincula esse conjunto; `Evidence.sha256` cobre o documento persistido inteiro. Não são adicionados campos ao modelo Evidence nem tabelas ao banco.

`ai.context.sourceFilesIncluded` é um campo informativo do resultado da prévia de IA. Ele será `true` somente quando uma captura selecionada tiver envelope, hashes e vínculo ao plano coerentes. Evidência manual com um marcador semelhante não é classificada como captura do núcleo. Essa indicação não autoriza leitura ou execução, e não consulta os arquivos originais.

## Restrições e integridade

- Apenas avaliações `code`/`files`, no alvo atual do projeto, com projeto não arquivado e avaliação sem estado ativo ou desconhecido. `awaiting_approval` estacionária é permitida.
- Qualquer plano `approved`, `running`, `canceling` ou `unknown` da avaliação impede a captura. O plano escolhido deve estar `proposed`, `completed`, `failed`, `canceled` ou `revoked` e manter digest, avaliação, alvo e manifest íntegros.
- De um a oito arquivos, somando até 32 KiB dos bytes originais. Somente UTF-8 sem NUL; não há conversão de encoding nem truncamento silencioso.
- Caminhos devem corresponder exatamente ao manifest. Caminhos novos, duplicados, absolutos, navegação relativa e campos extras são recusados. Arquivos não selecionados não são abertos, relidos ou descobertos durante a captura.
- Bloqueios e identidades de `source_scan` são reutilizados. Substituição do alvo ou de um arquivo, links, junctions, hardlinks, reparse points e streams alternativos são recusados. Os handles permanecem abertos até o fim da gravação local.
- Os bytes efetivamente capturados devem reproduzir tamanho e hash do manifest. Uma alteração entre a prévia e a confirmação exige nova revisão. O domínio é conferido novamente dentro da transação.
- A rota nativa mantém exclusão de operações, manutenção, backup, expurgo e troca agendada de perfil durante a leitura/gravação. A lease de IA impede rotação da credencial enquanto a cópia é redigida.

## Redação e limites de compartilhamento

Somente a cópia redigida é persistida. A redação aplica os padrões existentes de `ai::redact` e remove ocorrências literais da chave atual do cofre, sem entregar essa chave ao módulo de captura. Caminhos cuja redação mudaria são recusados para não registrar proveniência ambígua. O arquivo original nunca é alterado.

A redação é conservadora e não identifica todos os segredos. Ela também normaliza separadores/final de linhas conforme o redator existente; `redactionChanged` indica qualquer diferença do texto original, não uma contagem de segredos detectados. A pessoa deve revisar o conteúdo mostrado.

Os 32 KiB limitam bytes de arquivos, não o tamanho do JSON derivado. Escapes, proveniência e outras evidências aumentam o contexto. O limite independente de 48 KiB de `ai.context` continua em vigor e pode exigir uma seleção menor; não há corte automático de fontes para caber em um envio.

## Verificação

Os testes `source_context::tests` usam diretórios temporários e credencial sintética. Cobrem seleção exclusiva, prévia sem persistência, captura idempotente, proveniência e metadado de contexto, alteração de bytes/identidade/alvo, caminhos fora do manifest, outra avaliação, campos extras, confirmação, limites, UTF-8/NUL, hardlinks, streams, junctions, redação, estados incompatíveis e locks durante o callback de persistência. O histórico do provedor permanece vazio; nenhum teste chama OpenAI ou Kali.
