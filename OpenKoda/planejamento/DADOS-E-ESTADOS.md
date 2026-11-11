# Dados, estados e recuperação

Proposta de contrato de domínio e persistência. Os campos abaixo são uma especificação lógica, sem criar banco, schema executável ou código de aplicativo. [Vocabulário](../GLOSSARY.md).

## Apresentação Tauri e projeções das seis telas

[VISUAL-E-TELAS](VISUAL-E-TELAS.md) define V-01…V-06 e sua tradução de estados. O frontend Tauri não é dono das entidades nem cria estados persistidos por conveniência visual: listas, filtros, seleção, abas e painéis recolhidos são estado de apresentação. Reload/reconexão reidrata o canônico pelo núcleo e cursor de eventos. O label “Em andamento” agrupa estados ativos, mas o detalhe conserva o estado específico e a origem/atualidade do dado.

Rascunho de nova avaliação pode ser salvo sem executar; a ação Revisar e iniciar valida a revisão antes de Preparando. Configuração de conexão testada, plano de instalação calculado e permissões efetivas precisam carregar identidade/revisão dos dados utilizados. Alterar entradas invalida a confirmação/teste anterior. Uma linha de resultado “Em revisão” permanece Candidato mesmo quando apresentada no destino Achados. Nenhum mockup cria registros reais ou promove candidatos a confirmações.

## 1. Entidades e relações

| Entidade | Dados essenciais | Relações/invariantes |
|---|---|---|
| Instalação | id, versão, identidade local, configurações e estado de bloqueio | Dono do estado local e das sessões confiáveis |
| Projeto | id, nome, objetivo, classificação, diretórios e regras | Contém alvos e avaliações; arquivar não apaga evidências |
| Alvo | id, tipo, identificador canônico e metadados de origem | Pode ter várias revisões e participação em mais de uma avaliação |
| Revisão de escopo | id/revisão/digest, inclusões, exclusões, métodos, janela e condições | Imutável depois de usada; descoberta não a altera |
| Ambiente | id, dono, distro/backend, versão, health e manifest | Uma execução registra exatamente qual revisão utilizou |
| Perfil de execução | id/revisão, capacidades requeridas, controles comprovados e limites | Indisponível se o backend não demonstrar controles exigidos |
| Avaliação | id, projeto, objetivo, modo, escopo, estado e budgets | Vínculo fixo às revisões de escopo, configuração e fontes |
| Plano de ação | id/revisão, hipóteses, operações, efeitos e dependências | Mudança material cria revisão, não edita aprovação antiga |
| Operação | id/revisão/digest, tipo, alvo, efeito, inputs, limites e pré-condições | É o objeto autorizado; não confundir com processo |
| Decisão de autorização | id, origem humana/política, objeto vinculado, prazo e estado | Vale para escopo concreto; pode ser consumida/expirada/revogada |
| Execução | id, operação, tentativa, executor, estado, tempos e status de efeito | Retry é novo id; PID isolado não identifica trabalho após restart |
| Evento | streamId, seq, id, hora, tipo, correlationId e payload | Ordenação garantida por stream, sem inventar ordem global entre máquinas |
| Evidência | id, origem, hash, tamanho, conteúdo, intervalo e disponibilidade | Deduplicar bytes não perde as origens de observação |
| Observação | id, evidências, parser/versão, campos e limites | Reprocessável sem alterar evidência original |
| Candidato | id, hipótese, origem, evidências, testes e pendências | Não aparece como achado confirmado só porque um scanner o emitiu |
| Achado | id/revisão, impacto, confiança, validação, localizações e evidências | Separar severidade, confiança e prioridade de correção |
| Remediação/reteste | id, achado, patch/mudança, autorização, base e resultado | “Aplicado” e “verificado” são fatos distintos |
| Relatório | id/revisão, seleção, manifest, hashes e redaction | Projeção dos canônicos, não substitui fonte/evidência |
| Referência de credencial | id, finalidade, versão e política de uso | Valor fica no vault e não é campo comum de projeto |

Chaves externas de ferramenta, URL ou nome de arquivo são dados. IDs internos são emitidos pelo núcleo. Um resultado importado precisa de origem declarada e nunca pode se passar por execução própria usando um id escolhido no conteúdo.

Enquanto Rascunho, a avaliação pode ajustar sua revisão de escopo. Ao iniciar Preparando, o vínculo usado pela avaliação é congelado. Depois disso, ampliar ou mudar o escopo cria uma **nova Avaliação vinculada à anterior**, copiando somente configurações escolhidas e revalidando autorização. A avaliação original conserva seu resultado/estado; pausar e clicar em novo escopo não muda retroativamente suas execuções. Revisões de plano dentro do mesmo escopo são possíveis, com nova verificação de efeitos e autoridade.

## 2. Estados da avaliação

| Estado canônico | Significado | Saídas possíveis |
|---|---|---|
| Rascunho | Objetivo/alvos ainda podem ser ajustados | Preparando, Arquivada |
| Preparando | Validar fontes, ambiente, provedor, escopo e recursos | Planejando, Bloqueada, Cancelando, Falhou |
| Planejando | Produzir plano e cobertura esperada | Aguardando aprovação, Na fila, Bloqueada, Falhou |
| Aguardando aprovação | Decisão concreta pendente | Na fila, Planejando, Cancelando |
| Na fila | Plano elegível, aguardando recursos | Executando, Aguardando aprovação, Bloqueada, Cancelando |
| Executando | Há operações/investigações ativas | Pausando, Validando, Bloqueada, Cancelando, Falhou |
| Pausando | Não iniciar novas operações e estabilizar trabalho ativo | Pausada, Bloqueada, Cancelando |
| Pausada | Checkpoint estável ou estado não pausável claramente descrito | Preparando para retomar, Cancelando |
| Validando | Consolidar evidências, candidatos e cobertura | Concluída, Parcial, Bloqueada, Falhou |
| Cancelando | Revogar novos dispatches e supervisionar encerramento | Cancelada, Efeito desconhecido |
| Bloqueada | Falta ambiente, credencial, orçamento, compatibilidade ou decisão | Preparando, Planejando, Cancelando |
| Concluída | Critérios e cobertura planejada atendidos, resultados persistidos | Arquivada, nova avaliação/reteste |
| Parcial | Há resultado utilizável e lacunas explícitas | Arquivada, nova avaliação/continuação autorizada |
| Cancelada | Encerramento reconciliado, resultados parciais mantidos | Arquivada, nova avaliação |
| Efeito desconhecido | Não foi possível confirmar execução/encerramento/efeitos | Reconciliação, Parcial com restrições, intervenção do usuário |
| Falhou | Não há conclusão confiável para a avaliação | Arquivada, nova tentativa com causa tratada |

“Preparando para retomar” é a transição de Pausada para Preparando com checkpoint anterior, não uma autorização renovada automática. “Sem achados” só descreve resultados na cobertura efetiva. Concluída não significa que o alvo seja seguro.

## 3. Estados da operação e da execução

Operação: Proposta → Validada → Autorizada → Agendada → Despachada, ou Recusada/Expirada/Revogada. A autorização é reavaliada ao sair da fila; mudança de alvo efetivo, ambiente, comando, inputs ou política pode invalidar a decisão anterior.

Execução: Reservada → Iniciando → Em execução → Coletando saída → Encerrada. Caminhos alternativos: Falha de início, Cancelamento solicitado, Encerramento forçado, Timeout, Limite atingido e Estado desconhecido. Uma execução encerrada registra código/sinal, duração, status de drenagem de I/O e status dos efeitos separadamente.

Três situações exigem interpretação diferente:

- Não iniciada: o executor comprovou que nenhum processo foi criado.
- Processo encerrado: o supervisor confirmou o fim, mas alterações remotas podem persistir.
- Desconhecida: perdeu-se confirmação; não repetir ação com efeitos automaticamente.

Cancelamento é um pedido, não sucesso instantâneo. Não existe promessa de desfazer tráfego já enviado, pacotes instalados ou escrita remota consumada. A UI mostra o marco observado e as ações ainda necessárias.

## 4. Estados do resultado

Candidato: Novo → Em validação → Sustentado, Suprimido, Não aplicável ou Inconclusivo. Promover a Achado exige evidência suficiente e motivo de validação. Resultado de validação estática, observação passiva e reprodução dinâmica têm rótulos próprios.

Achado possui dois eixos independentes:

- Validade: sustentado, contestado ou inconclusivo, preservando o histórico de revisão.
- Tratamento: aberto, aceito como risco, correção proposta, alteração aplicada, aguardando reteste, corrigido no reteste ou reaberto.

Uma revisão humana pode mudar classificação com motivo e evidência. Não apaga a versão anterior. Duplicatas têm vínculo explícito; semelhança de título não basta para unir problemas de alvos/instâncias diferentes.

## 5. Contrato de eventos

Toda mudança durável relevante produz evento com versão, id, stream, sequência monotônica local, entidade/revisão, request/operation/execution IDs quando aplicáveis, tempos de observação e ingestão, origem e payload validado. Tempos UTC são armazenados; apresentação segue o fuso configurado.

Eventos de progresso podem ser agregados para a UI. Eventos de autorização, início, término, revogação, promoção de evidência e finalização não podem ser descartados como ruído. A reconexão pede eventos após o último cursor persistido e aceita duplicatas pelo id.

A proposta é estado relacional transacional com journal/outbox e blobs imutáveis, não event sourcing obrigatório. Comandos que alteram estado usam revisão esperada/idempotency key; a UI recebe conflito se a base mudou.

## 6. Persistência e integridade

Proposta inicial: SQLite para estado e índice, object store de arquivos para evidências e spool Linux para execuções ativas. Chaves, credenciais e grants sensíveis ficam em mecanismos próprios. O banco não registra tokens de API em texto.

Promoção de evidência: receber em área temporária → impor limites e validar nome/tipo → verificar tamanho/hash/manifest → fsync/rename conforme plataforma → transação de registro e origem → publicar evento. Um crash entre arquivo e transação produz objeto órfão reconciliável, não evidência completa fictícia.

Uma revisão final de avaliação sela o manifest, cobertura e seleção de resultados com hashes. Hash local oferece integridade verificável dentro desse modelo; não é assinatura independente ou prova contra um invasor com controle do usuário Windows. Exportações têm novo manifest e política de redaction.

Blobs são imutáveis. Redaction cria derivado com referência ao original, regras e versão. Reparse points/symlinks e paths vindos de ferramentas não podem escapar das áreas permitidas. Nenhuma limpeza usa um caminho bruto retornado pela IA como alvo recursivo.

## 7. Checkpoints e reconciliação

| Interrupção | Recuperação proposta |
|---|---|
| UI recarrega | Reabrir estado e eventos após cursor, sem repetir dispatch |
| Provedor cai | Manter execuções em política predefinida, bloquear novos passos de IA e permitir inspeção/manual |
| Núcleo reinicia | Reconciliar leases e execuções com executor usando instalação+boot+execution id |
| Executor perde conexão | Marcar comunicação perdida; investigar processos e efeitos antes de retry |
| Distro reinicia | Novo boot invalida handles antigos e grants de sessão |
| Windows reinicia | Reconciliar estado, inputs, versão e autorização antes de retomar |
| Disco enche | Parar ingestão/novos jobs, preservar checkpoints, marcar cobertura parcial e oferecer liberação controlada |
| Parser falha | Preservar bruto e registrar falha; candidato pode continuar pendente |
| Atualização incompatível | Recusar trabalho novo e abrir diagnóstico/migração ou rollback do app quando suportado |

Leases e heartbeats detectam perda de supervisão; não provam ausência de um processo remoto órfão. Um watchdog técnico precisa ter comportamento de fail-close validado. O produto não confunde “sem heartbeat” com “todos os efeitos pararam”.

## 8. Idempotência e concorrência

Requests de criar avaliação, solicitar execução e registrar resultado possuem chaves idempotentes com escopo. Repetir a mesma requisição devolve a identidade existente; não dispara novo processo. Trocar conteúdo usando a mesma chave produz conflito.

Isso não promete exactly once sobre um sistema externo. A operação pode ter começado antes de cair a conexão. Reexecutar uma ação com efeitos exige reconciliação e nova decisão quando o resultado é desconhecido.

Escritas no mesmo projeto/fonte e instalação de pacotes usam locks específicos. Atualização de APT bloqueia novas avaliações na distro afetada. Uma pausa não libera automaticamente reservas de execução nem permite adquirir lease de manutenção enquanto houver processo não reconciliado. A manutenção não muda versões usadas por trabalho ainda ativo.

## 9. Retenção, exportação e exclusão

Retenção diferencia bruto, derivados, conversa, diagnósticos e vault. Expurgar bruto torna referências indisponíveis e afeta o que pode ser revalidado; relatórios preservam esse fato. Exportar não é apagar, e arquivar projeto não é excluir dados.

O fluxo de exclusão calcula dependências e inclui índice, blobs sem referências, spool, caches e backups sob controle do app. Logs externos, backups do usuário e dados já enviados ao provedor não desaparecem por excluir um projeto. O produto não promete apagamento físico irrecuperável em SSD.

## 10. Pacote de reprodução

Manifest de relatório referencia escopo, plano, configuração efetiva, versões ambiente/ferramentas/parsers/modelo, fontes/snapshot, execuções, evidências, redactions, cobertura e lacunas. Credenciais são substituídas por referências/descrições. A possibilidade de repetir depende da disponibilidade do alvo e das versões, e é declarada em vez de presumida.
