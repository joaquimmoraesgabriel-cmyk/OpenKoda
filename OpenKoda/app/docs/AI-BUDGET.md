# Orçamento durável da IA

O orçamento controla a admissão de novas solicitações pagas por avaliação e por instalação. Usa USD, tarifas por modelo informadas e revisadas pela pessoa e reservas persistidas antes do envio. Ele começa desativado, sem preço ou limite inventado. Ativar exige configuração e reconhecimento explícitos.

O orçamento é uma proteção local de despacho. Não impõe um teto absoluto à fatura da OpenAI. A estimativa não usa o tokenizer do modelo; tarifas incorretas, diferenças de uso, chamadas em voo e custos externos podem produzir cobrança residual. O uso informado pelo provedor e o valor calculado pelas tarifas manuais não equivalem a uma fatura verificada.

## Revisão e envio

As quatro rotas pagas são `ai.ask`, `ai.test`, `ai.operation.propose` e `ai.investigation.run`. Quando o orçamento está ativo, cada uma exige `budgetReview:{id,digest}` produzido por `ai.budget.preview` e revisado pela pessoa. A autorização de contexto/envio existente continua necessária. A prévia é local e não chama o provedor.

| Operação da prévia | `payload` |
|---|---|
| `ask` | `assessmentId`, `evidenceIds`, `contextDigest`, `message` |
| `test` | Objeto vazio |
| `operation_proposal` | Campos de `ask`, mais `allowedAdapters` |
| `investigation` | `id` e `digest` do plano de investigação preparado |

Esses payloads não recebem `allowRemote` nem outra `budgetReview`. São usados os mesmos textos e instruções do envio real. A revisão dura 15 minutos e vincula os fingerprints de contexto, mensagem, instruções, modelo, endpoint, revisão de credencial, limite de saída e política de custo. Alterações exigem outra revisão. Cada slot pode ser consumido uma única vez; investigadores usam de um a três slots serialmente.

`ai.budget.preview` retorna `{enabled,review,notice}`. `review` é null com o orçamento desativado. Quando ativo, contém `id`, `digest`, `createdAt`, `expiresAt`, `epochId`, `policyRevision`, `model`, `endpoint`, `priceRevision`, `rates`, `estimatedInputTokens`, `maxOutputTokens`, `requests`, `reservedUsd`, `installationRemainingUsd`, `assessmentRemainingUsd`, `contextDigest` e `notice`.

A reserva estima a entrada como bytes UTF-8 do prompt e das instruções, mais 1.024 de margem; a saída usa o `max_output_tokens` solicitado. As duas quantidades são multiplicadas pelas tarifas manuais de entrada/saída por milhão de tokens. O cálculo usa inteiros e arredonda para cima ao microdólar, sem soma financeira em ponto flutuante. Valores monetários no IPC são strings decimais USD com até seis casas, ou null quando não atribuíveis.

## Política, saldo e histórico

`ai.budget.status` recebe `{}` ou `{assessmentId}`. Retorna política/revisão/época, totais da instalação e avaliação, estado `degraded`, histórico legado conhecido e recibos globais. Cada total distingue `pricedUsageUsd`, `heldUsd`, `assumedChargeUsd`, `remainingUsd` e `unresolvedCount`. `unknownCharges` e `recentCharges` mostram no máximo 100 itens cada, com suas contagens totais explícitas. As cobranças desconhecidas incluem `chargeId`, `heldUsd`, modelo e data, permitindo revisão mesmo depois do expurgo de um projeto.

`ai.budget.configure` recebe `expectedRevision`, `enabled`, `currency:"USD"`, `installationLimitUsd`, `assessmentLimitUsd`, `prices`, `acknowledgeEstimates`, `acknowledgeLegacyExcluded`, `legacyReviewDigest` e `acknowledgeUnbounded`. Os limites são positivos quando ativo; o limite por avaliação não excede o global. `prices` contém até 100 modelos distintos com `model`, `inputUsdPerMillion`, `outputUsdPerMillion` e `sourceNote`. Preços devem ser positivos: preço ausente ou zero não é interpretado como gratuidade. Valores são limitados a USD 1.000.000 por campo; esse limite de validação não é uma recomendação de gasto.

O primeiro salvamento explícito de política inicia uma época. Alterar tarifas, aumentar/diminuir limites, desativar ou reativar não reinicia gastos nem reprecifica recibos anteriores. Desativar após ativação exige reconhecimento de envio sem bloqueio por custo. As solicitações continuam registradas globalmente e são precificadas quando existe tarifa. Se faltar tarifa, o valor permanece null.

Histórico anterior sem preço e períodos desativados sem tarifa exigem reconhecimento explícito da exclusão ao configurar/reativar. Eles não são convertidos em custo zero. A contagem de legado cobre os registros conhecidos no perfil atual e no ledger da instalação; não afirma ter encontrado todos os perfis históricos ou toda a fatura externa.

Uso informado acima da reserva individual é preservado. O recibo indica a diferença; atingir o teto total ou não possuir saldo para a próxima reserva bloqueia novos envios. Isso não descarta respostas anteriores nem altera o status de achados ou execuções. O limite existente de 100 solicitações por avaliação continua aplicado, e o ledger global também conserva essa contagem entre cópias de perfil.

## Persistência, falhas e cobranças desconhecidas

`ai-budget.sqlite3` e `ai-budget.installation.json` pertencem à raiz estável da instalação, fora dos perfis restauráveis. O banco contém `budget_meta`, `budget_policies`, `budget_reviews` e `budget_charges`. O marcador contém `schema:"openkoda.ai-budget-installation.v1"` e `installationId`, que deve corresponder à identidade no banco. A primeira criação grava o marcador com `create_new` e sincronização antes de criar o banco; inicialização parcial, perda de um dos arquivos, marcador malformado ou identidades divergentes bloqueiam novos envios, sem recriar saldo vazio.

Totais são derivados exclusivamente dos recibos desse ledger. Restore, rollback, troca de perfil e expurgo de projeto não substituem esses dois arquivos nem renovam saldo. O expurgo preserva recibos mínimos de custo e a chave pseudônima da avaliação; não preserva ali textos, caminhos de fontes ou credenciais. Prévias contêm somente fingerprints e metadados monetários.

Antes do HTTP: persistir reserva global, persistir a intenção real em `requests.details.budget` no perfil e marcar despacho no ledger. Os dois bancos não constituem uma única transação. Uma interrupção entre seus commits mantém reserva/cobrança desconhecida, sem replay. Reiniciar transforma `reserved`/`dispatched` em `unknown`. Uma falha local comprovadamente anterior ao HTTP pode marcar `not_dispatched` e liberar a reserva; um timeout de rede não oferece essa prova.

A resposta informa uso de entrada/saída? O ledger calcula o valor pela tarifa congelada. Uso ausente, negativo, inválido ou impossível de representar mantém uma cobrança desconhecida. Uma falha ao gravar histórico do perfil não apaga o consumo já registrado no ledger global. Ledger indisponível/corrompido bloqueia inferência, preserva os arquivos e permite consultar o histórico local; não é recriado como saldo vazio. O construtor de migração `AiService::new` não cria autoridade monetária em uma cópia: somente `new_with_budget_root` conecta o ledger global.

Cobranças desconhecidas bloqueiam novos envios enquanto o orçamento está ativo. `ai.request.reconcile` reconhece o histórico do request, mas não libera uma reserva financeira. Para isso, `ai.budget.resolveUnknown` recebe `chargeId`, `assumedChargeUsd`, `note` e `acknowledgeUnknownCharge:true`. O débito deve ser positivo e pelo menos igual à reserva. A resolução converte o hold em débito conservador; o uso e a fatura continuam desconhecidos. A nota é redigida e preservada. Não há consulta externa, retry, promessa de estorno ou cancelamento do provedor.

## Verificação

As fixtures de `ai_budget.rs` usam somente diretórios temporários, credenciais sintéticas e provedor simulado. Cobrem as quatro rotas, revisão consumida/vencida/alterada, preços e épocas, limites globais e por avaliação, reinício nas fronteiras de persistência, falhas SQLite antes/depois do despacho, consumo desconhecido, reconhecimento separado, exclusão do histórico do perfil, corrupção do ledger, contagem durável e ausência de prompts/segredos nos recibos globais. A validação nativa de expurgo, ativação e rollback pertence à suíte coordenada do aplicativo.
