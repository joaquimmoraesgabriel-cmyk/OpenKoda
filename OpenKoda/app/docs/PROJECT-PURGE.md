# Expurgo explícito do histórico de um projeto

Este fluxo remove logicamente o grafo de um projeto **arquivado** do perfil ativo. A seleção é revisada antes da confirmação. Não existe exclusão automática por prazo, no reinício ou durante diagnóstico.

O escopo inclui o projeto, avaliações com evidências/eventos, achados, validações, planos, execuções e as linhas vinculadas dos bancos de IA, correções e MCP. Backups de bytes dentro de `remediation_plans` fazem parte do histórico selecionado. A credencial DPAPI, configuração, testes globais de conexão e dados de outros projetos permanecem fora do escopo.

Arquivos fonte/alvos, Kali, diretórios de backups/exportações, outros perfis, cópias externas e dados mantidos pelo provedor não são removidos. A exclusão não é apagamento seguro: não garante eliminação de páginas SQLite livres, WAL, caches ou cópias. Excluir histórico de IA não cancela solicitações nem cobranças.

## Contrato nativo

| Ação | Entrada | Resultado |
|---|---|---|
| `projectPurge.status` | `{}` | `journals`, `hasUnresolved`, `hasPartialHistory`, limites e frase exigida |
| `projectPurge.get` | `{id}` | Um plano/recibo existente |
| `projectPurge.plan` | `{projectId}` | Plano `prepared`, contagens, IDs/hashes e digest; não exclui dados |
| `projectPurge.apply` | `{id,digest,confirmation:"EXCLUIR PROJETO E HISTÓRICO"}` | Inicia ou continua explicitamente o mesmo escopo revisado |
| `projectPurge.reconcile` | `{id}` | Inspeciona um expurgo interrompido; não executa exclusões |
| `projectPurge.closePartial` | `{id,digest,note,acknowledgePartialDeletion:true}` | Encerra sem continuar, preservando dados restantes e aviso de histórico incompleto |

O plano vence em quinze minutos antes do primeiro efeito. Uma revisão ainda não utilizada é revogada no reinício. Um expurgo iniciado exige decisão explícita após interrupção; seu prazo inicial não autoriza efeitos novos nem modifica o escopo. A continuação só pode remover as mesmas linhas com os mesmos valores e IDs revisados.

O journal `project-purge.sqlite3` registra IDs, hashes, contagens, estados e mensagens fixas. Não copia conteúdo de evidências, conversas, patches, backups ou credenciais. A justificativa de encerramento parcial deve ter de 1 a 4000 bytes; somente seu hash e tamanho são conservados.

## Vínculo e conferência

O plano vincula a identidade Windows do diretório do perfil e dos bancos, um hash do caminho do perfil, schema esperado e hashes das linhas selecionadas. Copiar/restaurar um banco ou perfil não transfere autorização de exclusão. Journals encerrados continuam consultáveis em cópias; uma operação incompleta copiada continua bloqueada até inspeção/encerramento consciente, sem usar identidades antigas para excluir arquivos novos.

As consultas e exclusões usam somente tabelas/colunas e nomes de bancos fixos. SQL, nomes de tabelas e caminhos livres não são aceitos do WebView. Schema, versão, tabelas, colunas, constraints, views, triggers ou índices desconhecidos bloqueiam o fluxo. As linhas auxiliares usam SHA-256 de valores SQLite tipados, incluindo `rowid`; o grafo principal usa a serialização canônica das entidades do domínio. O plano contém também todas as identidades de avaliação/achado para detectar linhas movidas para outro projeto.

O inventário completo do escopo é reconferido antes da primeira exclusão. Linhas novas, modificadas, movidas, bancos substituídos e alterações do projeto invalidam a revisão. Operações ativas ou não reconciliadas de avaliações, IA, correções e MCP bloqueiam o expurgo mesmo quando o projeto está arquivado. Requisição IA `unknown` só é elegível após recibo válido de reconhecimento humano; ela é contada separadamente e seu consumo continua desconhecido.

Há limites de 10.000 linhas selecionadas, 128 MiB de conteúdo lógico selecionado e 100 journals. Excedê-los falha sem autorizar exclusão parcial automática. O inventário global de atividades auxiliares também é limitado.

## Etapas e falhas

As etapas ocorrem nesta ordem:

1. `assistant.sqlite3`: mensagens, requisições e investigações selecionadas, numa transação SQLite.
2. `remediation.sqlite3`: planos e backups internos selecionados, numa transação.
3. `kali-bridge.sqlite3`: sessões selecionadas, numa transação.
4. `openkoda.sqlite3`: remoção do grafo via `Store::mutate`, preservando invariantes do domínio.

Transações `IMMEDIATE` protegem os bancos auxiliares durante a conferência inicial e até sua etapa. O journal registra intenção antes de cada commit e confirmação depois. O coordenador mantém uma exclusão global durante planejamento/aplicação/inspeção; mutações normais ficam bloqueadas enquanto houver journal não resolvido. Leituras podem continuar com aviso de histórico parcial.

Não existe atomicidade distribuída entre os bancos. Uma falha pode confirmar etapas anteriores e preservar as posteriores. Nenhuma falha provoca restauração automática ou reexecução. `reconcile` compara o escopo atual e registra contagens presentes, ausentes, alteradas, novas ou não observadas. Ausência após uma etapa interrompida é apresentada como **observada**, sem atribuir a causa ou inventar uma contagem de exclusões confirmadas.

Uma linha que reapareça após etapa já concluída é conflito, mesmo que seu ID e conteúdo sejam iguais. Ela não é excluída novamente por esse plano. Se o escopo continuar compatível, `apply` com a mesma confirmação pode executar as etapas restantes. Se houver conflito ou o usuário preferir parar, `closePartial` libera o bloqueio mantendo o estado terminal `abandoned_partial`, o recibo e o aviso de histórico possivelmente incompleto. Esse plano não pode ser retomado.

## Verificação

Os testes sintéticos cobrem seleção completa e preservação de outro projeto, credencial/cópias/fontes excluídas do escopo, confirmação/digest, alterações entre revisão e aplicação, schema desconhecido, atividade pendente, consumo IA desconhecido reconhecido, interrupções antes/depois de commits, continuação explícita, inspeção sem replay, encerramento parcial, recriação de linhas, revogação após reinício e identidade de bancos/perfis copiados. As evidências de execução ficam no registro geral de implementação; a existência dos testes não substitui sua execução nem demonstra apagamento físico seguro.
