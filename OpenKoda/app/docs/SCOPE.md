# Escopo versionado — 0.1.4 local

O escopo revisado limita objetivo, métodos e janela para iniciar operações de uma avaliação. Não concede aprovação de plano, envio à IA, orçamento, execução ou escrita. O formato simples da 0.1.3 conserva modalidade e alvo iguais aos do projeto. A 0.1.4 acrescenta o formato combinado descrito em [Alvos e origens](COMBINED-SCOPE.md), com seleção explícita de entradas e exclusões. A passagem de uma avaliação simples ao formato combinado exige outra avaliação; o histórico anterior não é reinterpretado.

## Estados e persistência

O banco principal usa `user_version=5` na 0.1.4; a 0.1.3 usa 4. `Snapshot.scopeRevisions`, `assessmentScopes` e `scopeReceipts`, `Assessment.scopeId`, `ExecutionPlan.scopeRef` e `Execution.scopeAdmission` são aditivos. Campos opcionais ausentes e vetores vazios não são serializados no histórico legado. Digests de plano sem `scopeRef` conservam o cálculo anterior. A migração não autoriza trabalho, não reinterpreta provas e não realiza I/O de alvo. A passagem 4→5 preserva o JSON existente quando não há reconciliação de trabalho pendente; as novas estruturas só aparecem ao serem utilizadas.

Uma avaliação criada sem `scope` começa `unreviewed`; um formulário de criação pode fornecer a revisão explícita atomicamente, passando a `draft`. Uma derivação também começa `unreviewed`, com proposta preenchida e sem revisão humana. Revisões em rascunho são anexadas, com identidade, número, predecessor e digest. O primeiro uso congela a revisão de forma durável. Remover um plano ou evidência não desfaz o congelamento.

`frozen` e `revoked` não podem ser editados. A alternativa é derivar outra avaliação. A revogação impede novas admissões e preserva provas, registros e operações já iniciadas; ela não solicita cancelamento automaticamente. Cancelar/revogar um plano ou reconciliar uma interrupção continua sendo uma ação separada.

Legado significa ausência de `Assessment.scopeId`, não um rascunho novo sem revisão. A adoção exige decisão humana prospectiva; preserva o objetivo legado, cria a primeira revisão e congela imediatamente. Provas, digests e aprovações anteriores não recebem vínculo retroativo. Planos antigos podem ser consultados/revogados/reconciliados, mas uma nova execução exige outro plano preparado após a adoção.

Uma exceção estreita preserva correções textuais já aplicadas: uma correção legada `applied`/`retested` pode fazer uma nova leitura/reteste após adoção, com método e janela atuais. O novo recibo possui `scopeRef`, e `RemediationPlan.retestScopeRef` apresenta esse vínculo. Confirmar exige esse reteste novo, seu hash, revisão atual e releitura do arquivo. Aprovar/reaplicar plano legado permanece bloqueado; seu digest e bytes anteriores são preservados.

## Catálogo fechado

| Método | Modalidade | Aplicação da janela |
| --- | --- | --- |
| `local.inventory.v1` | code/files | Prazo local cooperativo |
| `source.rules.v1` | code/files | Prazo local cooperativo |
| `source.capture.v1` | code/files | Admissão; depende de source.rules |
| `http.headers.v1` | web | Prazo local cooperativo |
| `network.tcp-connect.v1` | network | Prazo local cooperativo |
| `kali.runtime-diagnostics.v1` | todas | Prazo local cooperativo |
| `kali.hash-evidence.v1` | todas | Prazo local cooperativo |
| `kali.rg.signals.v1` | todas | Prazo local cooperativo |
| `ai.remote-review.v1` | todas | Admissão de cada pedido/perspectiva |
| `local.text-remediation.v1` | code/files | Admissão; escrita/reteste/confirmação separados |

Lista vazia permite revisão local de registros, não operações do catálogo. Método desconhecido, duplicado, incompatível ou dependência ausente é recusado. A lista limita os adaptadores existentes; não habilita shell, ferramentas MCP arbitrárias ou novos destinos.

Datas RFC3339 são normalizadas em UTC. Início é inclusivo e fim exclusivo; limites nulos são abertos. O estado público distingue `unbounded`, `not_yet_open`, `open` e `expired`. Não existe agendamento automático. Preparação e dispatch são revalidados; nos adaptadores cooperativos o prazo usa `Instant` limitado pelo restante da janela. O recibo `Execution.scopeAdmission` conserva `scopeRef`, `admittedAt`, `configuredTimeoutMs`, `effectiveTimeoutMs` e `limitingFactor` (`scope_window_end` ou `plan_timeout`).

O prazo local não garante término remoto exato: uma requisição já enviada pode continuar, e cleanup/reconciliação podem ultrapassar a janela. O host não reapresenta automaticamente operações com resultado desconhecido. No Kali o deadline de dispatch também é limitado ao fim da janela em segundos Unix; sua aproximação é conservadora.

## Contrato IPC

Todos os inputs são objetos estritos, com limite de 16 KiB nas ações de escopo simples e 64 KiB nas ações combinadas. Não há autorização calculada pelo frontend. A tabela abaixo conserva o contrato do formato simples; o contrato combinado fica no documento próprio.

| Ação | Entrada principal | Resultado |
| --- | --- | --- |
| `assessment.scopeOptions` | projectId | Catálogo nativo, projectDigest, modalidades, proposta vazia, limites |
| `assessment.create` | Campos anteriores + scope opcional | Criação atômica da avaliação e revisão explícita |
| `assessment.scopeStatus` | assessmentId | phase, stateDigest, revisão, congelamento/revogação/origem, workGates, actions |
| `assessment.scopePreview` | assessmentId, expectedStateDigest, objective, methods, window | Proposta normalizada, reviewDigest, commitAction, canCommit, reasons |
| `assessment.scopeSave` / `scopeAdopt` | Mesmo corpo + reviewDigest, idempotencyKey, acknowledgeScope | saved, replayed, revisionId, scopeStatus |
| `assessment.derivePreview` | assessmentId, expectedStateDigest, title, copyFields, campos não copiados | Proposta no mesmo projeto, reviewDigest e canCreate |
| `assessment.deriveCreate` | Mesmo corpo + reviewDigest, idempotencyKey, acknowledgeNewAssessment | created, replayed, assessmentId, scopeStatus |
| `assessment.scopeRevoke` | assessmentId, expectedStateDigest, idempotencyKey, rationale, acknowledgeRevocation | revoked, replayed, scopeStatus, admittedWork, cancellationRequested:false |

`create.scope` contém `methods`, `window`, `projectDigest`, `catalogVersion:"openkoda.scope-methods.v1"` e `acknowledgeScope:true`. O catálogo é obtido antes da criação por `scopeOptions`.

`copyFields` admite somente objective/methods/window; um campo copiado não pode ser repetido explicitamente. Todo campo não copiado deve ser fornecido. A derivação copia apenas configuração escolhida, sem evidências, achados, execuções, aprovações, orçamento, consentimento ou pedidos. Um legado sem revisão pode copiar o objetivo, mas não inventa métodos ou janela anteriores.

`actions` inclui canSave/canAdopt/canDerive/canRevoke/canPrepareWork, canReviewStoredEvidence e canImportEvidence. `workGates` mostra a elegibilidade de cada método. Esses campos não substituem verificação nativa nem indicam disponibilidade de ambiente/budget. Revisão de fatos já armazenados permanece possível após revogação/expiração; novo input por `evidence.import` exige revisão ativa, inclusive após adoção de legado.

`expectedStateDigest` vincula avaliação, projeto, revisão, estado e trabalho pendente. `reviewDigest` também vincula a proposta normalizada. A mesma chave idempotente e mesmo corpo retornam o resultado original, sem repetir efeito; reutilizar a chave com outro corpo é recusado. Limites: 100 revisões e 1.000 recibos por avaliação, além dos limites existentes de armazenamento.

## Fronteiras e histórico

Guards vivem nos serviços antes de leitura de alvo, DNS/socket, lançamento Kali, reserva/envio IA ou escrita. O roteador também bloqueia novas mutações durante expurgo/ativação pendentes. `scopeRevoke` usa lease de leitura e redator sem lease de inferência; revogar não espera o término de um provider. Falha de leitura da credencial não impede retirada de autorização; a justificativa ainda recebe redação genérica.

A admissão usa um mutex curto junto ao congelamento e registro do serviço. O mutex não permanece preso durante I/O. Há verificação final antes do efeito. Revogação e dispatch não formam uma transação distribuída com o SO/provedor; a corrida residual após a última checagem é tratada como trabalho já admitido, sem alegar cancelamento ou reversão.

Validação estrutural de snapshots e provas históricas não depende do relógio atual nem da revogação atual. Relatórios Markdown/JSON apresentam revisões e estado do escopo separadamente das observações. Metadados exportados continuam sujeitos à redação e escape existentes.

O expurgo revisa as coleções novas: `scopeRevisions` por revisionId, `assessmentScopes` por bindingId e `scopeReceipts` agrupados por assessmentId. A exclusão de escopos e avaliações ocorre na mesma transação principal; vínculos de derivação ficam no mesmo projeto. O formato antigo do journal e suas contagens não mudam, permitindo retomar expurgo legado interrompido sem recalcular autorização sobre linhas novas.

## Verificação

As fixtures cobrem criação atômica, rascunho sem revisão, catálogo inválido, digests desatualizados, replay idempotente, congelamento durável, derivação sem autoridade herdada, janela inclusiva/exclusiva, timeout efetivo, revogação antes de efeitos, migração sem reescrever JSON legado, continuidade readonly de correção aplicada e expurgo interrompido. São perfis temporários e recursos locais próprios, sem API paga, Kali real ou perfil do usuário. A validação nativa e a embalagem pertencem ao gate de integração da versão; resultados ficam nos logs de aceitação.
