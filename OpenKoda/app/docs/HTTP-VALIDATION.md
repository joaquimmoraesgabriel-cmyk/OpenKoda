# Validação factual e reteste HTTP

O fluxo registra uma conclusão limitada a uma resposta HEAD: ausência de `X-Content-Type-Options` na observação inicial e, após uma nova execução aprovada, presença de uma única ocorrência `nosniff`. A revisão humana valida esse fato. Não demonstra exploração, impacto, comportamento do navegador, respostas GET, cobertura de outras URLs ou permanência da configuração remota.

Nenhuma ação `finding.http*` envia tráfego, aprova plano ou altera o servidor. Uma nova requisição continua exigindo `assessment.plan` → `plan.approve` → `plan.run`, com a URL e os destinos revisados. O relatório mantém a avaliação parcial e os limites da observação.

## Usar o fluxo

1. Na avaliação Web, prepare, revise, aprove e execute uma observação HEAD. Selecione essa execução no painel de controle HTTP e revise os fatos retornados pelo núcleo.
2. Quando a resposta for elegível e o header estiver ausente, registre o candidato factual com reconhecimento dos limites. A alegação, a regra e a origem são emitidas pelo host; o candidato ainda não está validado.
3. No achado, revise a baseline, justifique a conclusão e confirme a validação factual. Essa ação registra revisão humana ligada aos hashes da evidência e da alegação.
4. Se a configuração remota for alterada por um fluxo autorizado externo, prepare **um novo plano HEAD depois da validação**. Revise os destinos, aprove e execute normalmente. Abrir o reteste ou voltar à avaliação não executa esse plano.
5. Escolha a nova execução e revise a comparação. Registrar reteste preserva o resultado — inclusive inconclusivo ou ainda sem header — e não marca a correção automaticamente.
6. Somente o último reteste positivo, comparável e sem execução posterior não revisada permite confirmação humana. A confirmação vale para aquela resposta, URL, destinos, peer e instante; não afirma que o OpenKoda aplicou uma alteração ao servidor.

O fluxo de patch de arquivo local continua separado. Uma alegação HTTP tipada não pode ser validada ou reaberta pelo formulário genérico de texto livre. Descartar uma candidata/validada preserva as provas e o histórico.

## Regra e observação v2

Regra única inicial: `http.x-content-type-options-nosniff.v1`. O adaptador continua `http.headers.v1`; a evidência nova usa `schema:"openkoda.http-observation.v2"`.

| Resultado | Critério |
|---|---|
| `control_missing` | HEAD concluída, status 200–299, captura v2 completa, peer dentro dos destinos aprovados e nenhuma ocorrência do header |
| `control_present` | As mesmas condições e exatamente uma ocorrência ASCII textual cujo valor, sem SP/HTAB externos, é `nosniff`, sem distinguir maiúsculas/minúsculas |
| `inconclusive` | Captura legada/incompleta, falha/cancelamento, status fora de 2xx, redirect, peer desconhecido/fora do plano, header repetido, não textual, valor diferente ou limite de interpretação excedido |

Dois valores `nosniff` também são inconclusivos neste critério conservador. Um valor diferente não é convertido em “ausente”. Falha de integridade, schema inconsistente ou ID de outra avaliação é recusada, em vez de gerar uma prova válida com resultado inconclusivo.

A evidência conserva os campos legados `headers` com o primeiro valor, apenas para consulta. A autoridade da regra usa `headerValues`: as dez chaves do adaptador estão sempre presentes, com uma lista base64 de **todas** as ocorrências dos bytes de cada header. A lista vazia registra ausência. Inclui também `method:"HEAD"`, `captureComplete`, `remoteAddress`, status, URL, digest do plano e destinos aprovados.

São capturados `content-type`, `content-length`, `server`, `cache-control`, `location`, `strict-transport-security`, `content-security-policy`, `x-content-type-options`, `x-frame-options` e `referrer-policy`. O adaptador continua sem proxy, redirects, cookies, credencial ou corpo de resposta. O limite de saída do plano continua aplicado. O parser factual limita a interpretação a 1.024 ocorrências e a apresentação de um valor a 4.096 bytes; qualquer excesso é informado como inconclusivo, mantendo a evidência original.

## Proveniência e persistência

Um hash correto ou um texto contendo `adapter`/`schema` não concede origem de executor. A prova percorre a cadeia canônica: execução → plano aprovado → avaliação → evidência da execução. Confere digest, hashes, IDs, alvo, destinos, peer e cronologia. Uma evidência não pode ser compartilhada artificialmente por duas execuções para sustentar esse fluxo.

As provas são evidências tipadas criadas dentro de `Store::mutate`, na mesma transação que a atualização do achado e da revisão humana:

- `openkoda.http-claim.v1`: alegação fixa, findingId, regra e baseline recalculável.
- `openkoda.http-validation.v1`: reconhecimento e justificativa da revisão, estado revisado e registro humano.
- `openkoda.http-retest.v1`: nova execução, comparação, resultado e referências da baseline/validação.
- `openkoda.http-confirmation.v1`: reteste exato confirmado, reconhecimento, justificativa e registro humano.

Não há novo banco nem campo obrigatório em Finding. A importação humana usa seu próprio wrapper e não pode assumir esses schemas de raiz. Um candidato genérico com título parecido continua genérico. A remoção de um vínculo em `evidenceIds` não remove a natureza da alegação tipada, pois o invariant procura o claim por findingId no conjunto canônico de evidências.

A prova para `fixed` é uma união explícita: o fluxo local anterior mantém suas exigências de patch/reteste/confirmação, e o fluxo HTTP exige sua cadeia factual e confirmação humana. O novo ramo não aceita apenas um JSON que diga que o reteste passou.

Retestes exigem plano e execução diferentes, criados/aprovados/iniciados/encerrados depois da validação. Mudança de URL, conjunto de destinos ou peer impede a comparação positiva. Uma nova execução HTTP posterior ao último reteste impede sua confirmação até que o novo resultado seja revisado. Provas já confirmadas conservam o instante histórico: uma observação futura não apaga a conclusão anteriormente registrada nem declara o estado atual do servidor.

Para esse bloqueio, a execução mais recente é a última inserida pelo host no vetor canônico da avaliação, incluindo falha ou cancelamento. O maior timestamp não define a ordem: uma correção do relógio não pode ocultar uma execução posterior. As verificações de datas internas do plano, observação e validação continuam aplicáveis.

## Contratos IPC

Todos os payloads recusam campos desconhecidos, IDs inválidos e solicitações acima de 16 KiB. Justificativas contêm de 1 a 2.000 bytes UTF-8, sem controles inesperados, e passam pelo redator local antes de persistir. As rotas de leitura não acessam o vault.

| Ação | Payload | Resultado/efeito |
|---|---|---|
| `finding.httpPreview` | `{executionId,ruleId}` | Observação factual, `reviewDigest`, `canCreateCandidate`, razões e candidato existente; sem escrita |
| `finding.httpCandidate` | `{executionId,ruleId,reviewDigest,acknowledgeLimits:true}` | `{created,finding,status}`; candidato com alegação fixa; repetição exata reaproveita o registro |
| `finding.httpStatus` | `{findingId}` | `{supported:false,findingId}` para achado genérico; caso tipado retorna baseline, provas, ações e `stateDigest` |
| `finding.httpValidate` | `{findingId,stateDigest,rationale,acknowledgeLimits:true}` | Estado factual atualizado; revisão humana e promoção para validated |
| `finding.httpRetestPreview` | `{findingId,executionId}` | Comparação, resultado, `reviewDigest`, `canRecord`, `reasons` de impedimento e `factReasons` da conclusão |
| `finding.httpRetest` | `{findingId,executionId,reviewDigest}` | `{recorded,retest,status}`; preserva resultado sem promover a fixed |
| `finding.httpConfirm` | `{findingId,retestEvidenceId,stateDigest,rationale,acknowledgeLimits:true}` | Estado atualizado e confirmação humana de fixed para o reteste exato |

O status tipado contém `actions.canValidate`, `canRetest`, `canConfirm`, `confirmRetestEvidenceId` e razões. O frontend usa o ID emitido pelo host para confirmar, sem eleger uma prova por relógio ou texto. `availableExecutions` informa quais execuções podem ser revisadas; `retests` mantém a ordem canônica antiga→nova. Erro de leitura/integridade não é convertido em `supported:false`.

O digest de estado vincula Finding, claim, provas, registros humanos, estado da avaliação/projeto e planos/execuções HTTP dessa avaliação. Mudança exige nova revisão. O digest de reteste acrescenta a execução/plano/evidência selecionados, fatos e comparação. As mutações revalidam tudo dentro da transação e sob os bloqueios de expurgo/ativação/execução do host.

## Relatórios, backup e migração

`report.export` e `report.bundle` apresentam uma seção factual com regra, origem, hashes, URL, peer, instantes, resultados e confirmação humana. O manifest do bundle inclui `httpValidations`. O resumo exclui texto dos valores de headers; uma exportação de evidência, quando explicitamente selecionada, continua sendo um derivado redigido separado, com hashes e aviso de revisão humana.

As evidências/provas e registros humanos seguem o backup, a restauração e o expurgo do grafo existente. Não há arquivo remoto modificado ou removido pelo reteste.

O domínio usa `user_version=3`: a forma do JSON legado não muda, mas a semântica de `fixed` passa a incluir a nova prova HTTP. Um binário anterior deve recusar essa versão antes de interpretar seus registros. A migração 2→3 valida o domínio e preserva o documento canônico; `project_purge` usa a mesma constante de versão. O formato de journal, DDL e row digests do expurgo permanecem iguais, sem alterar as limitações textuais de planos anteriores.

## Verificação

Os recibos da rodada atual serão consolidados em [IMPLEMENTACAO](../IMPLEMENTACAO.md). Testes unitários, loopback real, smoke nativo, regressão de interface e instalador são evidências distintas; este documento não declara a conclusão antecipada dos ensaios de distribuição.

Checkpoint focal: [14/14 testes HTTP](../.local/http-validation-tests.log) passaram, zero falhas/ignorados, 2,31 s. O [filtro de migrações](../.local/http-validation-migration-tests.log) passou 4/4, 1,50 s, incluindo preservação do documento 2→3 e continuação explícita de journal de expurgo após migração. A primeira rodada teve 13/14 porque uma fixture chamou uma rota inexistente; o [log histórico](../.local/http-validation-first-tests.log) foi preservado e a fixture passou depois com `candidate.record`. Esses recibos não substituem a regressão completa, o smoke nativo ou o instalador da versão.

A matriz focal cobre headers ausentes/válidos/repetidos/não textuais, status/peer/legado, IDs e digests alterados, importação imitando proof, aprovação e cronologia, revisão humana, recusa de confirmação prematura/stale, preservação de baseline, backup/restauração, redação de justificativa e relatório com checksums. Um teste usa um listener loopback próprio e verifica HEAD, bytes repetidos e peer real; não consulta alvos externos.

A revisão independente encontrou uma falha na escolha da execução mais recente pelo relógio. A [regressão RED](../.local/http-clock-rollback-red.log) reproduziu confirmação indevida com estado atualizado após uma HEAD ausente inserida depois, mas com timestamp menor. A seleção agora segue a ordem canônica de inserção. A fixture também cobre execução posterior falha e cancelada. O [filtro HTTP atualizado](../.local/http-clock-rollback-green-local.log) passou **15/15**, zero falhas/ignorados, 1,54 s, com permissões para os testes locais Windows. A [rodada restrita anterior](../.local/http-clock-rollback-green.log) teve 13/15: o sandbox impediu backup e conexão loopback; os mesmos testes passaram sem alterar fixtures na repetição autorizada. Esse recibo exige nova integração/build do binário e não certifica um artefato compilado antes da correção.
