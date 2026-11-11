# Correções locais e reconciliação

O fluxo normal prepara um patch de um arquivo UTF-8 de até 256 KiB, exige aprovação do digest, aplica a alteração, registra um reteste delimitado e exige confirmação humana separada. O reteste de texto/SHA-256 não demonstra ausência de outras vulnerabilidades. Antes/depois e backup ficam no armazenamento local; não são enviados automaticamente à IA.

A escrita usa um único handle exclusivo Windows e mantém os ancestrais protegidos contra substituição. Não é substituição atômica: interrupções podem deixar bytes parciais. O aplicativo não reaplica patches no reinício. Operações interrompidas em `applying`, `retesting` ou `confirming` tornam-se `unknown`, conservando a fase interrompida quando disponível.

## Reconciliação explícita

`remediation.reconcile` recebe:

```json
{
  "id": "UUID do plano",
  "note": "Justificativa humana de 1 a 4000 bytes",
  "acknowledgeUnknownEffects": true
}
```

A reconciliação aceita somente operações interrompidas/desconhecidas. Ela observa o caminho original, registra evidência e encerra o plano em `reconciled`. Não escreve no alvo, não restaura backup, não repete o patch, não executa o reteste e não altera o status do achado. Uma confirmação humana que tenha sido persistida antes da interrupção permanece registrada. O plano reconciliado não pode voltar a `apply`, `retest` ou `confirm`; outra alteração precisa de um novo plano e aprovação.

O recibo `reconciliation` preserva a justificativa, autor local, fase anterior, data e evidência vinculada. Sua `observation` distingue:

| Estado | Observação |
|---|---|
| `before` | Os bytes atuais correspondem ao SHA-256 anterior. Isso não prova que nenhuma escrita ocorreu ou que uma operação foi desfeita. |
| `after` | Os bytes atuais correspondem ao conteúdo proposto. Isso não confirma que o patch foi a causa nem que o achado foi corrigido. |
| `other` | Os bytes atuais, inclusive bytes parciais ou não UTF-8, têm outro hash. |
| `missing` | O Windows retornou `FILE_NOT_FOUND` para o arquivo, depois de proteger todos os ancestrais. A causa e o momento da ausência continuam desconhecidos. |
| `unavailable` | A inspeção não pôde classificar os bytes: acesso/bloqueio, reparse point, hardlinks, ancestral ausente, tamanho excedido ou outra falha. Não equivale a arquivo ausente. |

`identityMatches` é separado da igualdade de hash. Um arquivo substituído por outro com os mesmos bytes pode ser `before` ou `after`, com identidade original não correspondente. As observações são pontuais, não garantias sobre alterações futuras. A leitura é limitada a 256 KiB, usa somente o caminho registrado, recusa destinos ambíguos e não segue links/reparse points.

Mesmo uma observação indisponível pode ser encerrada mediante reconhecimento explícito de que os efeitos históricos continuam desconhecidos. O recibo conserva essa limitação. Isso permite liberar retenção e troca de perfil sem apresentar uma recuperação não comprovada como bem-sucedida.

## Persistência e repetição

O recibo é persistido antes de acrescentar a evidência e o evento ao domínio. Se houver falha entre os dois bancos SQLite, o plano continua `unknown` com o recibo pendente. Uma nova chamada humana explícita, com a mesma justificativa, conclui esse recibo sem repetir a inspeção ou duplicar evidência. A data da observação original permanece visível. Justificativa diferente é recusada para preservar o registro em andamento.

Uma repetição com a mesma justificativa após `reconciled` retorna o registro existente. O reinício não conclui a reconciliação automaticamente. Backups e dados do patch são preservados. O relatório de correções inclui os metadados e hash da justificativa, sem os bytes de antes/depois ou do backup; uma exportação que inclua evidências também pode incluir a justificativa humana registrada na evidência.

O coordenador nativo deve manter a exclusão de operações concorrentes durante a reconciliação. O serviço também serializa suas próprias operações e reconfere os vínculos de projeto, avaliação e achado. Não há reconstrução automática de um alvo que tenha mudado de escopo.
