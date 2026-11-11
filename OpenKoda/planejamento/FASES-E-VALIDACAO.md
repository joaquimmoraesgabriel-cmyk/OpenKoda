# Fases, dependências e critérios de validação

Sequência proposta de trabalho futuro, sem cronograma ou compromisso de entrega. Não foram criados tickets, escrito código de aplicação ou executados os ensaios abaixo. O catálogo completo continua descrito em [PRODUTO-E-ESCOPO](PRODUTO-E-ESCOPO.md).

## F0 — Fechar a especificação e as decisões que bloqueiam a fundação

Tauri e as seis referências visuais já foram aprovados. Esta revisão consolida [VISUAL-E-TELAS](VISUAL-E-TELAS.md), [ADR-0004](../docs/adr/0004-tauri-como-framework-desktop.md), requisitos, invariantes e matriz de aceitação. Permanecem detalhes de versão/frontend, escopo inicial de adaptadores, threat model, perfis, política de dados e distribuição. A escolha de Tauri não será reaberta como decisão pendente; as provas seguintes avaliam sua integração. Não é necessário escolher agora todo recurso futuro.

## F1 — Provar a fundação técnica em laboratório

Etapa futura de provas limitadas: fundação Tauri/WebView2, versão/frontend propostos, commands restritos e eventos UI/núcleo, identidade do executor, ciclo de processo, spool/eventos, vault, WSL/Kali gerido, perfil offline e enforcement de um perfil ativo limitado. Alvos de laboratório e dados de teste serão definidos antes dos ensaios. Acessibilidade, provisionamento WebView2, runtime usuário comum e viabilidade do instalador/assinatura devem ser avaliados cedo, embora a distribuição completa seja F5.

Critérios: instalação/reentrada após reboot; sem apropriar-se de distro pessoal; perda/reconexão sem execução duplicada; término real confirmado; recusa de capacidade incompatível; nenhuma chave do modelo no ambiente Kali; casos fora de escopo tecnicamente impedidos no perfil que afirma essa garantia. Se a imposição de rede/host falhar, resolver arquitetura/perfil antes de apresentar esse modo como disponível.

## F2 — Primeira fatia funcional completa

Um fluxo vertical utilizável: criar projeto e escopo → preparar Kali e provedor → analisar fontes selecionadas → elaborar plano → autorizar por política/aprovação → executar um adaptador dinâmico limitado em alvo de laboratório → preservar bruto/observações → validar candidato → apresentar achado e cobertura → exportar relatório revisado → retomar após falha.

Inclui a estrutura visual V-01…V-06: painel, nova avaliação, catálogo Kali, achados, projetos e configurações. Conversa, fila, processos, evidências, diagnóstico, cancelamento e budgets entram como capacidades contextuais, preservando lista/detalhe e utilidades recolhidas. Comparar a fatia com as imagens usando dados de exemplo, sem simular saúde/conexão/cobertura. A fatia prova o ciclo completo, mesmo com catálogo pequeno; demais modalidades/adaptadores continuam F3.

## F3 — Expandir modalidades e ferramentas

Adicionar adaptadores e parsers de web/API, rede e arquivos offline por prioridades de uso; incluir compatibilidade/versões, fixtures benignas, falhas conhecidas e política por efeitos. Ampliar shell avançado somente quando o perfil realmente suportar suas capacidades. Atualizar pacotes e catálogo com simulador/locks/recuperação.

Critérios: cada ferramenta nova informa pré-requisitos, argumentos, efeitos, origem do resultado, limites, cancelamento, coverage e manutenção. Um pacote instalado sem parser/adapter aparece com essa condição; não finge suporte completo.

## F4 — Remediação, profundidade e ciclo de tratamento

Patches propostos com base imutável; revisão de diff e autorização de escrita; teste/reteste e histórico de achados; avaliações profundas com investigadores independentes e redução de duplicatas. Não aplicar mudança em produção automaticamente por uma frase da IA.

Critérios: edição concorrente invalida patch; aplicado não significa corrigido; reteste identifica ambiente/base/condições; orçamento limita expansão de workers e hipóteses; resultados divergentes permanecem visíveis.

## F5 — Distribuição e extensões

Assinatura, instalador, atualizações/migrações/rollback do app, experiência de desinstalação e recuperação; depois plugins terceiros, clientes externos MCP, integrações de exportação, múltiplos usuários ou backend isolado/remoto quando decididos. Cada superfície exige identidade e política próprias.

Critérios: preservação/restauração de dados testada, recuperação de atualização interrompida, matriz Windows/WSL, telemetria e pacote de suporte explícitos, inventário de dependências/licenças, ausência de endpoint aberto por padrão e documentação de limites de isolamento.

## Matriz transversal de aceitação

| ID | Cenário a demonstrar | Resultado exigido |
|---|---|---|
| AC-01 | Primeira instalação e reboot no meio | Retomar estado salvo e diagnosticar sem reinstalar tudo |
| AC-02 | Distro Kali pessoal já existe | Não adotar/remover silenciosamente; criar gerida ou pedir escolha explícita |
| AC-03 | Alterar comando/inputs/alvo após aprovação | Decisão anterior não autoriza o novo conteúdo |
| AC-04 | IP/redirect/porta fora do escopo | Bloqueio real no perfil que promete esse limite, com motivo e evidência |
| AC-05 | Ferramenta tenta burlar proxy/usar filho | Controles abrangem subprocessos ou modo não fica disponível |
| AC-06 | Output contém instrução para obter segredo | Dado tratado como evidência; nenhuma autoridade/contexto ampliado |
| AC-07 | Modelo inventa grant/id de execução | Recusado; identidades emitidas pelo host não são confiadas ao texto |
| AC-08 | Revogar enquanto está na fila/iniciando | Não iniciar novo dispatch válido; corrida residual e efeitos registrados honestamente |
| AC-09 | Fechar UI ou cair pipe após iniciar | Reconciliar processo, não repetir ação com efeito desconhecido |
| AC-10 | Processo ignora cancelamento gracioso | Supervisor segue política de término e mostra confirmação/unknown |
| AC-11 | APT/disco/rede falha na manutenção | Estado degradado/reparável, sem falso rollback e sem soltar avaliações prematuramente |
| AC-12 | Cap de saída ou parser inválido | Bruto capturado mantido, perda e cobertura parcial visíveis |
| AC-13 | Timeout/erro de ferramenta sem achado | Não classificar como avaliação limpa/segura |
| AC-14 | Falha de IA ou teto de custo | Preservar trabalho e bloquear novos passos conforme política |
| AC-15 | Token API/credencial aparece em saída | Redaction de derivados/exportação e tratamento explícito de evidência sensível |
| AC-16 | Requisição repetida/fora de ordem | Idempotência/conflito/cursor evitam novo processo invisível |
| AC-17 | Atualizar parser/ferramenta durante avaliação | Avaliação conserva versão ou exige nova revisão/retomada |
| AC-18 | Aplicar patch sobre base alterada | Recusar/recalcular, sem sobrescrever trabalho do usuário |
| AC-19 | Excluir projeto/exportar/retomar backup | Dependências e destino explícitos; sem prometer apagar cópias externas |
| AC-20 | Navegação por teclado/contraste/leitor de tela | Ações, erros e progresso acessíveis, sem depender de cor/terminal visual |
| AC-21 | Distro/backend não suporta root/hardware requisitado | Bloqueio ou alternativa explícita, sem alegar capacidade só pelo pacote instalado |
| AC-22 | Cliente MCP não autorizado ou sessão antiga | Recusa antes de acessar executor/vault, com revogação e versão verificadas |
| AC-23 | Fonte Windows não selecionada | Não copiar/enviar por conveniência ao modelo ou à distro |
| AC-24 | Finalizar relatório com lacunas | Manifest registra cobertura, pendências, evidência ausente e limites |
| AC-25 | WebView/origem/janela não autorizada tenta command ou troca ID/revisão | Restrição explícita de app commands e autorização de objeto no núcleo/executor negam a ação; SEC-23 |
| AC-26 | Comparar as seis telas, zoom e janela estreita | Identidade e navegação V-01…V-06 preservadas; lista/detalhe adaptável, utilidades recolhidas, foco/escala acessíveis |
| AC-27 | Editar endpoint/modelo após teste ou ler chave existente pela UI | Teste antigo invalidado por revisão; segredo nunca retornado pelo vault à WebView e sem sucesso fictício |
| AC-28 | Pacote ausente ao iniciar avaliação ou catálogo mudou após cálculo | Oferecer preparação sem instalar automaticamente; revisão/autorização de manutenção separada e cálculo invalidado quando necessário |

Os requisitos SEC em [PERMISSOES-E-SEGURANCA](PERMISSOES-E-SEGURANCA.md) detalham os controles. A implementação futura deverá ligar cada requisito à prova correspondente; esta matriz não declara que testes já passaram.

## Bloqueios legítimos antes de liberar uma capacidade

Sem identidade confiável, grants e reconciliação: não liberar execução. Sem enforcement demonstrado: não prometer perfil restrito. Sem recuperação APT: não oferecer manutenção autônoma. Sem origem/bruto: não promover observação a confirmação forte. Sem armazenamento/privacidade definida: não enviar fontes sensíveis por padrão. Falta de recurso avançado pode limitar uma modalidade; não deve gerar um atalho silencioso para um modo mais amplo.
