# Sugestão de próxima operação pela IA

`ai.operation.propose` pede uma única sugestão à OpenAI dentro de opções escolhidas pela pessoa e verificadas pelo núcleo. A sugestão contém o nome do adaptador, justificativa e uma pergunta opcional. Ela não cria plano de ação, aprovação, execução ou candidato. Nenhuma ferramenta é oferecida ao modelo.

## Fluxo de revisão

1. Obter `ai.context` com a avaliação e as evidências selecionadas e mostrar sua prévia redigida.
2. A pessoa escolhe as opções permitidas e revisa objetivo, contexto, modelo configurado e base do provedor `https://api.openai.com/v1` (campo `endpoint`). O envio usa a rota `/responses` dessa base.
3. Enviar `ai.operation.propose` com consentimento explícito. Uma única chamada pode consumir API; não existe retry automático.
4. Mostrar a sugestão e sua origem. Somente uma nova ação humana pode preparar o plano atual no fluxo normal do adaptador. Portas, arquivos, transferência ao Kali e demais parâmetros continuam sendo escolhidos/revisados nesse fluxo, e a aprovação permanece independente.

Um registro histórico pode ser lido sem repetir a solicitação. Seu contexto pode estar desatualizado; a pessoa sempre prepara um plano novo a partir do estado atual. O backend não aceita ID de sugestão como autorização.

## Contrato nativo

Entrada de `ai.operation.propose`:

```json
{
  "assessmentId": "id-da-avaliacao",
  "evidenceIds": ["id-da-evidencia"],
  "contextDigest": "digest-da-previa",
  "message": "Objetivo para a próxima observação",
  "allowRemote": true,
  "allowedAdapters": ["http.headers.v1"]
}
```

Os campos são fechados: campos extras, nomes desconhecidos, opções repetidas, evidências repetidas ou de outra avaliação e opções incompatíveis com a modalidade são recusados antes de acessar o provedor. Não há filtragem silenciosa de opções inválidas.

| Adaptador | Modalidades | Condição adicional |
|---|---|---|
| `local.inventory.v1` | `code`, `files` | Alvo e prontidão serão verificados no plano local |
| `source.rules.v1` | `code`, `files` | Arquivos serão selecionados/revisados no plano local |
| `http.headers.v1` | `web` | Plano revisará a URL exata do alvo |
| `network.tcp-connect.v1` | `network` | A pessoa ainda deve informar/revisar portas |
| `kali.rg.signals.v1` | Todas as quatro modalidades | Pelo menos uma evidência selecionada íntegra, sem NUL e de até 32 KiB; escolha da evidência e confirmação da transferência continuam humanas |

O resultado contém `id`, `requestId`, `responseId`, `assessmentId`, `contextDigest`, `adapter`, `rationale`, `question` (string ou null), `model`, `endpoint`, `usage`, `notice`, `toolsExecuted:false`, `requiresHumanReview:true` e `availabilityVerified:false`. A disponibilidade real do adaptador não é prometida pela sugestão.

O modelo deve retornar somente JSON com `adapter`, `rationale` e `question` opcional. Campos extras como `command`, `target`, `path`, `ports` e argumentos são recusados. O adaptador deve pertencer à lista humana. JSON inválido, campos duplicados, blocos Markdown, justificativa vazia ou resposta acima do limite não geram proposta aceita. Justificativa e pergunta continuam sendo texto não confiável para exibição, nunca comandos.

## Limites, persistência e concorrência

- Até 20 evidências selecionadas, com SHA-256 conferido; prévia limitada pelo serviço de contexto existente a 48 KiB.
- Objetivo de até 8.000 bytes; resposta estruturada de até 12 KiB; justificativa de até 4.000 bytes e pergunta de até 1.000 bytes.
- Uma chamada, `max_output_tokens:2048`, timeout de 90 segundos e limite compartilhado de 100 solicitações por avaliação, incluindo registros desconhecidos. Esse limite não é um teto financeiro.
- A lease de inferência impede troca de modelo/configuração, remoção/rotação de credencial ou outra inferência durante a solicitação.
- O contexto é conferido antes e depois da chamada. Mudanças durante a espera tornam a sugestão `stale`; a resposta e o consumo permanecem registrados.

Não há tabela nova. O ledger existente preserva `requests.details.operationProposal`, exposto em `ai.history.requests[].operationProposal`. O objeto inclui os IDs, avaliação, digest, opções humanas, modelo, endpoint, datas, estado e, quando aceita, `proposal` com o resultado normalizado. Estados: `pending`, `accepted`, `rejected`, `stale`, `provider_error`, `interrupted`.

O estado do pedido ao provedor e o estado da sugestão são distintos. Uma resposta paga pode constar como request `completed` e proposta `rejected` por violar o esquema. Consumo ausente permanece desconhecido, nunca zero presumido. O histórico de mensagens preserva a resposta redigida mesmo quando ela é recusada como sugestão.

Ao reiniciar, pedidos pendentes passam a desconhecidos pelo fluxo existente e propostas `pending` passam a `interrupted`. Não há replay ou conclusão por inferência. Uma nova solicitação exige outra ação e consentimento da pessoa. O registro existente de reconciliação de consumo desconhecido continua aplicável.

## Verificação

As fixtures de `src-tauri/src/ai_planning.rs` usam provedor simulado, credencial sintética e diretórios temporários. Verificam consentimento e escopo antes do envio, modalidades, schema fechado, ausência de mutações no domínio operacional, histórico e consumo em respostas recusadas, mudança de contexto durante a espera, bloqueio de troca de credencial/configuração, redação, limite compartilhado e reinício sem replay. Não fazem chamada real à OpenAI ou ao Kali e não medem a qualidade ou a ausência de viés do modelo remoto.
