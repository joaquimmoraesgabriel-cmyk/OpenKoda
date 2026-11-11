# Investigação com perspectivas delimitadas

O OpenKoda pode organizar até três análises sobre um contexto selecionado: evidências, contraevidências e remediação. Cada chamada recebe o mesmo contexto e objetivo humano; nenhuma recebe a resposta de outra perspectiva ou o histórico da conversa. As chamadas usam o mesmo modelo configurado. Essa separação de entradas não comprova independência estatística nem qualidade das conclusões.

O serviço não recebe um Executor, não oferece ferramentas ao modelo, não aprova operações e não aplica patches. Os resultados permanecem separados, incluindo alegações contraditórias. Não são criados candidatos automaticamente. A revisão e a validação humana continuam sendo operações próprias.

## Revisão e consentimento

Primeiro, `ai.context` produz a prévia redigida das evidências escolhidas. `ai.investigation.plan` exige seu digest, o objetivo, uma a três perspectivas distintas e limites explícitos. O plano inclui contexto completo, mensagem redigida, endpoint, modelo, revisão da credencial, digest e validade de 15 minutos. Criar uma prévia não faz chamada à API.

O início exige o ID e digest desse plano, junto de `allowRemote:true`. O serviço verifica novamente projeto, contexto, modelo, endpoint e revisão da credencial. Não existe fallback de provedor ou modelo. O plano é consumido uma única vez e não pode ser retomado para repetir solicitações.

| Ação | Entrada | Resultado |
|---|---|---|
| `ai.investigation.plan` | `assessmentId`, `evidenceIds`, `contextDigest`, `message`, `perspectives`, `maxOutputTokensPerRequest`, `maxTotalTokens` | Plano revisável com estado `prepared` |
| `ai.investigation.run` | `id`, `digest`, `allowRemote:true` | Plano encerrado com resultados individuais e consumo |
| `ai.investigation.cancel` | `id` | Cancela a prévia ou solicita interrupção dos próximos envios |
| `ai.investigation.list` | `assessmentId` | Até 500 planos e seus estados/resultados |
| `ai.investigation.reconcile` | `id`, `note`, `acknowledgeUnknownUsage:true` | Encerra localmente um registro `unknown`, sem consultar/repetir o provedor |

Perspectivas permitidas: `evidence`, `counterevidence`, `remediation`. Nomes de ferramentas, comandos ou novos investigadores não são aceitos nesses campos.

## Limites e consumo

- Uma chamada por vez e no máximo três chamadas por plano.
- Contexto de até 48 KiB; objetivo de até 8.000 bytes.
- Entre 256 e 2.048 tokens de saída solicitados por chamada, usando `max_output_tokens` na Responses API.
- Limiar total entre 1.024 e 300.000 tokens. Antes de enviar, o serviço reserva a quantidade de bytes UTF-8 do prompt e das instruções, mais 1.024 de margem e o limite de saída.
- O consumo informado pelo provedor é registrado no ledger existente. Cada chamada também conta para o limite geral de 100 solicitações por avaliação, inclusive tentativas desconhecidas ou incompletas.
- Cada chamada mantém o timeout de 90 segundos do serviço. Não há retry automático.

A reserva por bytes é conservadora, mas não usa o tokenizer do modelo. `maxTotalTokens` é um limiar de despacho e parada, **não um teto financeiro garantido**. Não há cálculo de preço ou promessa de estorno. Se o provedor informar consumo acima do limiar ou saída acima do limite solicitado, o serviço preserva esse consumo e não envia outra chamada. Se o uso não puder ser confirmado, o estado fica `unknown` e os próximos investigadores não são enviados.

`knownTokens` soma somente os registros cujo uso de entrada e saída está completo. `usageUnknown` indica lacunas; números ausentes não são convertidos em zero. Cada resultado contém perspectiva, ID de request, texto redigido, estado, uso, erro e datas. Respostas acima de 64 KiB são recusadas como análise completa; o uso continua registrado.

## Concorrência, cancelamento e recuperação

O serviço mantém uma lease de inferência durante todo o plano. Trocar/remover a credencial, salvar configurações ou iniciar outra inferência exige essa mesma lease. O roteador nativo deve enviar `settings.save` por `AiService::save_settings`. Listagem, cancelamento e reconciliação não exigem a lease; permanecem acessíveis durante uma chamada longa.

Cancelar uma investigação ativa registra `canceling` sem sobrescrever resultados que estejam sendo gravados. A resposta da chamada já enviada é aguardada e preservada; o próximo envio não acontece. Isso não comprova cancelamento no provedor nem ausência de cobrança. Uma mudança no contexto entre perspectivas interrompe o plano e conserva as respostas já recebidas com o vínculo ao contexto anterior.

Antes de enviar cada request, há uma intenção durável no plano e no ledger de requests. Interrupção durante uma atualização de estado produz `unknown`. Ao abrir o aplicativo, `running`/`canceling` passam a `unknown` e prévias `prepared` são revogadas. Não há replay. A reconciliação exige uma nota humana e reconhecimento do consumo/resultado remoto desconhecido; registra `reconciledFrom:"unknown"` e preserva os requests originais, inclusive seu estado desconhecido.

O banco `assistant.sqlite3` inclui `investigations(id TEXT PRIMARY KEY, assessment TEXT NOT NULL, data TEXT NOT NULL)`. O backup deve copiar essa tabela de histórico, continuando a excluir vault e teste da conexão. A ativação de outro perfil deve recusar investigações `running`, `canceling` ou `unknown` na origem até a reconciliação explícita. Abrir uma cópia restaurada não reativa prévias nem envios.

## Candidatos e conteúdo não confiável

`candidate.record` reutiliza um registro somente quando avaliação, título, descrição, severidade, recomendação e conjunto de IDs de evidência são exatamente iguais. O estado e ID existentes são preservados. Uma descrição ou interpretação diferente continua separada; similaridade textual não autoriza fusão. IDs de evidência repetidos são recusados.

Instruções recebidas em evidências continuam sendo dados, sem caminho para execução. Chaves são redigidas no objetivo, contexto, resposta e nota de reconciliação. As instruções dos investigadores também proíbem inferir risco por atributos pessoais irrelevantes. As fixtures demonstram os controles do serviço e a redação de sentinelas; não medem qualidade, ausência de viés ou resistência universal à injeção do modelo remoto.

## Evidência de verificação

Os testes em `src-tauri/src/investigation.rs` usam um provedor simulado e dados temporários. Cobrem independência das entradas, divergência preservada, consentimento, digest, escopo, expiração, mudança de modelo/credencial/contexto, limites, resposta parcial, consumo desconhecido, cancelamento concorrente, reinício sem replay, redação e deduplicação exata.

Nenhuma chamada real à OpenAI é executada por essa suíte. A validação de integração real e o registro final dos gates pertencem à matriz de aceitação do aplicativo.
