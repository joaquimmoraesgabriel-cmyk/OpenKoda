# OpenKoda — painel e jornadas

**Estado:** especificação de experiência, revisão 0.2, 7 de novembro de 2026. O usuário confirmou Tauri e aprovou seis referências visuais. A organização visual abaixo incorpora essas decisões; fluxos, validações, acessibilidade e recuperação continuam contratos para implementação e teste futuros. As imagens usam dados fictícios e não são um protótipo executável ou uma interface implementada. O escopo e as hipóteses estão em [PRODUTO-E-ESCOPO.md](PRODUTO-E-ESCOPO.md).

## 1. Estrutura do painel

As referências aprovadas e [AJUSTES-DE-INTERFACE.md](AJUSTES-DE-INTERFACE.md) orientam o painel: navegação primária **Avaliações, Achados, Projetos**, acesso secundário a **Kali e ferramentas** e **Configurações**, lista à esquerda e detalhe da seleção à direita. A lateral estreita contém acessos secundários; não transforma cada função técnica em um destino primário.

A referência funcional continua sendo um workbench de segurança. A pessoa localiza um Projeto, abre uma Avaliação, acompanha o trabalho, examina Achados e volta às Evidências. Sem seleção, o detalhe mostra uma instrução curta; selecionar um item revela seu contexto. O chat não é necessário para navegar nem para entender o estado do trabalho.

```text
OPENKODA | Avaliações | Achados | Projetos | estado compacto do Kali | Nova avaliação
----------------------------------------------------------------------------------
Trilho       Lista e busca                 Detalhe do item selecionado
secundário   da área atual                 Resumo, estado, alvo e ações pertinentes
             Avaliações OU                Plano / resultados / atividade / arquivos
Kali e       Achados OU                    Cobertura, correções e histórico no contexto
ferramentas  Projetos
                                          Conversa com a IA [recolhida]
Configurações                             Execuções [recolhidas]
                                          Evidências [referência contextual]
----------------------------------------------------------------------------------
Indicador operacional discreto | acesso à atividade/diagnóstico | contexto selecionado
```

**Conversa com a IA** abre sob demanda em uma lateral recolhível. **Execuções/terminal** abre sob demanda em um painel inferior recolhível. As seis figuras mostram esses recursos fechados ou fora de exibição; não há exigência de manter três painéis abertos. A abertura das áreas opcionais não exige que a outra também abra. Evidências são abertas a partir da referência selecionada, e não como um terceiro painel permanente.

Fechar essas áreas só muda a apresentação, sem pausar processos, revogar autoridade ou perder o histórico. As ações aplicáveis de pausar/parar e os pedidos de aprovação continuam acessíveis no detalhe/indicadores, sem depender de manter o chat aberto. Em janela estreita, lista e detalhe podem alternar por navegação de retorno; adaptação de largura, painéis e foco ainda exige desenho e teste. O wireframe09 foi atualizado para essa composição; [VISUAL-E-TELAS](VISUAL-E-TELAS.md) consolida a identidade, as seis referências e a precedência dos contratos.

### 1.1. As seis referências e seu limite

| Referência aprovada | Direção visual preservada | Comportamento que a imagem não demonstra |
|---|---|---|
| [Painel de avaliações](conceitos/painel-openkoda-v2.png) | Lista/detalhe, busca, resumo/progresso, atividade recente e áreas recolhidas | Estado real, cálculo de progresso, pausa, parada, atualização da lista e abertura dos painéis |
| [Nova avaliação](conceitos/01-nova-avaliacao-v1.png) | Formulário no detalhe, quatro modalidades, Projeto/alvo, objetivo, escopo, modo/modelo e opções avançadas | Validação, autorização, preparação e início de operações |
| [Kali e ferramentas](conceitos/02-kali-e-ferramentas-v1.png) | Acesso secundário, saúde compacta, catálogo e resumo de seleção | Ambiente instalado, diagnóstico real, dependências calculadas, instalação e manutenção |
| [Achados](conceitos/03-achados-v1.png) | Lista/detalhe, severidade e confiança separadas, abas de evidência/correção/histórico | Existência de vulnerabilidade, método de validação, aplicação de correção ou reteste |
| [Projetos](conceitos/04-projetos-v1.png) | Lista/detalhe, alvos, escopo padrão, últimas avaliações e ação contextual | Persistência, autoria, autorização de alvos ou mudança de avaliação já iniciada |
| [Configurações](conceitos/05-configuracoes-v1.png) | Menu secundário, IA/modelos, conexão, limites e política de dados | Conexão testada, provedor compatível, vault implementado ou limites aplicados |

Os valores e indicadores são fictícios. “Kali pronto”, “Conexão verificada”, “Validado”, a quantidade de ferramentas, a barra de progresso e a nota de proteção local não são resultados de testes. Rótulos amigáveis como “Em andamento” devem derivar dos estados canônicos, sem criar estados de domínio novos. Posição e estilo de um botão aprovado não definem em quais estados ele estará habilitado. O registro dos cinco conceitos adicionais e de sua geração está em [TELAS-ADICIONAIS.md](conceitos/TELAS-ADICIONAIS.md).

### 1.2. Relação entre jornadas e telas

| Jornada | Entrada e percurso na navegação aprovada | Referência visual e partes ainda não desenhadas |
|---|---|---|
| A — primeira configuração | Acesso secundário a Kali e ferramentas e Configurações | [Kali](conceitos/02-kali-e-ferramentas-v1.png) e [Configurações](conceitos/05-configuracoes-v1.png); onboarding e falhas ainda precisam de desenho |
| B — Projeto e escopo | Projetos → selecionar/adicionar → Alvos e escopo | [Projetos](conceitos/04-projetos-v1.png); formulário de cadastro e revisão de escopo ainda serão detalhados |
| C — avaliação completa | Nova avaliação → formulário no detalhe → revisão → Avaliações, lista/detalhe | [Nova avaliação](conceitos/01-nova-avaliacao-v1.png) e [Painel](conceitos/painel-openkoda-v2.png); validações e confirmação operacional continuam especificação |
| D — plano e autorização | Detalhe da Avaliação → pedido contextual de decisão | O [Painel](conceitos/painel-openkoda-v2.png) orienta o contexto; revisão de plano/aprovação não tem tela visual própria aprovada |
| E — acompanhar e interromper | Avaliações → item selecionado → ações, conversa opcional e Execuções opcionais | [Painel](conceitos/painel-openkoda-v2.png); painéis abertos, pausa e erro ainda precisam de desenho/teste |
| F — candidato/achado | Achados → lista e detalhe → evidência/histórico | [Achados](conceitos/03-achados-v1.png); candidatos continuam explicitamente separados de achados sustentados |
| G — correção e reteste | Achados → Correção → nova Avaliação de reteste vinculada; falha operacional → Kali e ferramentas | [Achados](conceitos/03-achados-v1.png) e [Kali](conceitos/02-kali-e-ferramentas-v1.png); diff, autorização e resultado do reparo/reteste não foram desenhados |
| H — evidência e relatório | Evidências/Arquivos no detalhe de Avaliação ou Achado → seleção → exportação | [Painel](conceitos/painel-openkoda-v2.png) e [Achados](conceitos/03-achados-v1.png); viewer completo e prévia de exportação ainda serão detalhados |

## 2. Regras de compreensão da interface

- O detalhe da seleção identifica Projeto, Avaliação, alvo/revisão em uso e Perfil de execução, com resumo compacto e acesso aos dados completos. Esses campos não precisam dominar a barra global.
- Estados de IA, Ambiente e Executor são separados. “IA desconectada” não significa “nenhum processo ativo”.
- Plano, autorização, operação iniciada, saída recebida e conclusão validada têm apresentações distintas.
- Candidatos e Achados têm contadores separados. Severidade, confiança, revisão humana e remediação não compartilham um único status.
- Percentual de cobertura só aparece quando existe denominador e método definidos. Caso contrário, a tela mostra etapas/itens revisados e pendentes.
- A pessoa consegue navegar de conclusão → evidência → Execução → parâmetros/versão → revisão de plano e autorização aplicáveis.
- Ações de manutenção, alteração de alvo e envio externo não ficam escondidas em um botão genérico “continuar”.
- O modo avançado revela parâmetros e saída bruta; ele não remove os limites do Executor.
- Busca/filtros pertencem à área atual. Ao trocar entre Avaliações, Achados e Projetos, a seleção e o retorno precisam ser previsíveis, sem misturar resultados de áreas distintas.
- Custo, provedor e diagnóstico completo ficam disponíveis no contexto. Indicadores compactos não podem esconder uma decisão pendente ou fingir saúde de um componente que falhou.

## 3. Inventário das telas

Os identificadores são referências de planejamento, não rotas ou componentes já implementados.

| ID / tela | Informação principal | Ações principais | Estados que precisam de desenho |
|---|---|---|---|
| UX-01 Boas-vindas e diagnóstico | Objetivo do app, pré-requisitos detectados e etapas faltantes | Iniciar configuração, abrir documentação, repetir diagnóstico | Primeira abertura, diagnóstico em curso, incompatibilidade, necessidade de ação humana/reinício, retomar configuração |
| UX-02 Provedor e dados de IA | Provedor/modelo selecionados, credencial referenciada, classes de dados e orçamento | Configurar credencial, testar conexão, selecionar modelo, definir limites | Sem credencial, inválida/expirada, limite/cota, indisponibilidade, configuração local sem provedor hospedado |
| UX-03 Ambientes | Distribuições reconhecidas, Ambiente do app, saúde, versões, espaço e tarefas em andamento | Preparar Ambiente, selecionar existente, diagnosticar, manutenção, recuperação | WSL ausente, virtualização indisponível, distribuição ausente, indisponível ou incompatível, download interrompido, reboot pendente |
| UX-04 Ferramentas e conjuntos | Catálogo por capacidade, versão, dependências, estimativa resolvida e compatibilidade | Selecionar conjunto, revisar alteração, instalar/atualizar/remover futuramente | Catálogo indisponível, metadados antigos, instalado mas não pronto, download/lock APT, falha parcial, capacidade não suportada |
| UX-05 Navegação e listas | Listas/busca de Avaliações, Achados e Projetos, seleção e indicadores de atividade | Selecionar item, criar Projeto/Avaliação, retomar trabalho e abrir resultado | Lista vazia, filtro sem resultado, sem seleção, carregando, falha de consulta e trabalho recuperável |
| UX-06 Projeto | Alvos e fontes, contexto, escopo versionado, referências de credenciais, histórico | Editar contexto/escopo, adicionar fonte, iniciar Avaliação, arquivar Projeto | Fonte removida, alvo alterado, configuração incompleta, permissão expirada, sem avaliações |
| UX-07 Nova Avaliação | Modalidade, objetivo, alvos, exclusões, método, Ambiente e orçamento | Configurar, validar prontidão, salvar rascunho, revisar plano | Alvo inválido, método indisponível, conflito de limites, credencial necessária, estimativa desconhecida |
| UX-08 Workbench da Avaliação | Plano/revisão, progresso, processos, custos, cobertura, mensagens e resultados | Acompanhar, inspecionar Operação, responder pedido, pausar, parar, encerrar com parcialidade | Estados canônicos da Avaliação definidos em DADOS-E-ESTADOS; conexão da IA, prontidão do Ambiente e estados de Execução apresentados separadamente |
| UX-09 Revisão e autorização | Operações propostas, alvos/efeitos, capacidades, dados envolvidos e validade | Aprovar revisão exata, aprovar envelope delimitado quando suportado, negar, pedir ajuste | Plano mudou, autorização expirada/revogada, efeito adicional, capacidade indisponível, decisão registrada |
| UX-10 Candidatos e achados | Listas separadas, severidade/confiança, validação, revisão e cobertura associada | Filtrar, abrir, agrupar, validar, descartar com motivo, priorizar | Sem candidatos, validação pendente/inconclusiva, duplicata sugerida, erro de parsing, dados insuficientes |
| UX-11 Detalhe do Achado | Alegação, impacto, alcance, método, evidências, contraevidência e histórico | Revisar conclusão, corrigir anotação, propor correção, iniciar reteste, exportar seleção | Evidência faltante, alvo mudou, achado contestado, correção proposta/aplicada/não verificada, reteste inconclusivo |
| UX-12 Biblioteca de Evidências | Origem, digest, Execução, bruto/normalizado, sensibilidade e retenção | Visualizar, buscar, correlacionar, selecionar/redigir para exportação, gerenciar retenção | Arquivo indisponível, truncado, formato sem viewer, parser falhou, integridade divergente, original retido fora do app |
| UX-13 Correção e reteste | Plano de correção, patch/configuração proposta, revisão de base, resultados comparados | Revisar diff, autorizar aplicação quando suportada, registrar correção externa, revalidar | Base mudou, conflito, aplicação falhou, reversão indisponível, reteste bloqueado, correção verificada em versão específica |
| UX-14 Relatórios e exportação | Resumo, cobertura, resultados, pendências e prévia do pacote | Gerar rascunho, ajustar público, escolher/redigir evidências, exportar arquivo | Avaliação em andamento, dados incompletos, falha de exportação, destino sem espaço/permissão, conteúdo sensível selecionado |
| UX-15 Histórico e comparação | Avaliações, planos, versões, mudanças de método/alvo e revisões | Comparar, abrir estado anterior, duplicar configuração para nova Avaliação | Métodos incompatíveis, dados retidos parcialmente, alvo não comparável, ausência de reteste confirmado |
| UX-16 Configurações | IA, execução, dados, interface, armazenamento, manutenção e integrações | Revisar mudanças, salvar, restaurar configuração aplicável, revogar delegação | Alteração pendente, conflito com trabalho ativo, configuração inválida, provedor/integração indisponível |
| UX-17 Central de atividades e recuperação | Processos, fila, downloads/manutenção, incidentes operacionais e órfãos possíveis | Abrir trabalho, interromper owned process, reconciliar, coletar diagnóstico, recuperar | Encerramento não confirmado, host reiniciado, worker caiu, lease vencido, disco cheio, múltiplas tarefas afetadas |

O catálogo inteiro aparece no planejamento. Na aplicação construída, uma capacidade indisponível deve informar seu requisito ou estágio de disponibilidade; não simular uma execução nem prometer que um pacote resolverá toda incompatibilidade.

Os 17 identificadores descrevem superfícies e funções, não 17 abas primárias. UX-05/07/08 vivem em Avaliações; UX-06 em Projetos; UX-10/11 em Achados; UX-03/04 em Kali e ferramentas; UX-02/16 em Configurações. Autorização, evidência, correção, relatório, comparação e central de atividade abrem no contexto de origem. O onboarding pode usar os acessos secundários sem alterar essa hierarquia.

### 3.1. Estados complementares das seis telas

São comportamentos especificados para desenho/teste futuros, não estados já comprovados pelos conceitos aprovados.

| Tela | Vazio, falha e recuperação | Pausa e preservação de contexto | Teclado, foco e leitura |
|---|---|---|---|
| Avaliações | Sem avaliações, oferecer Nova avaliação; filtro sem resultado oferece limpar filtros; sem seleção, instrução curta; falha de consulta mantém o erro distinto de lista vazia | Pausada/Cancelando/Parcial e demais estados vêm do contrato; recolher conversa/Execuções não muda estado; restaurar seleção sem redisparar trabalho | Navegar da lista ao detalhe e voltar ao item de origem; ações de interrupção acessíveis com painéis fechados |
| Nova avaliação | Sem Projeto, caminho contextual para cadastro; entradas inválidas indicadas no campo; pacote/credencial ausente preserva rascunho e oferece preparo contextual, sem instalar ao salvar | Rascunho não é trabalho em pausa; iniciar Preparando congela o escopo; semântica de Cancelar/guardar alterações precisa ser definida e testada | Modalidades e expansões operáveis por teclado; foco no primeiro erro; limites essenciais não dependem de abrir toda opção avançada |
| Kali e ferramentas | Ambiente ausente/incompatível ou catálogo indisponível têm mensagens próprias; lock de manutenção e instalação parcial não aparecem como “Instalado e pronto” | Pausar/retomar manutenção só quando seu contrato suportar; não interromper transação de pacotes para imitar pausa de Avaliação; preservar seleção de conjuntos | Checkbox com nome do conjunto, estados textuais e progresso anunciado sem roubar foco |
| Achados | Sem resultados, distinguir nenhum resultado na cobertura de falha da avaliação; sem seleção, instrução curta; evidência ausente/divergente permanece indicada | Pausa da Avaliação de origem é visível no vínculo, sem inventar estado “achado pausado”; candidato não vira validado por seleção | Filtros nomeados; foco retorna ao achado ao fechar evidência; severidade, confiança e revisão não dependem só de cor |
| Projetos | Primeiro uso oferece Adicionar projeto; falha de carga não simula lista vazia; fonte indisponível mantém histórico e explica efeito sobre trabalho futuro | Avaliação vinculada pode estar Pausada, sem pausar o Projeto; editar padrão não reescreve escopo congelado | Lista/detalhe e abas internas com navegação consistente; mudanças não descartam rascunho sem informar |
| Configurações | Sem provedor/credencial, erro de conexão ou configuração inválida são estados distintos; “verificada” só após teste real correspondente | Modelo padrão novo vale para futuras Avaliações; trabalhos ativos/pausados conservam revisões fixadas; alteração aplicável não é inferida de Salvar | Campos e controles nomeados, chave mascarada e resultado do teste anunciado; teclado alcança menu interno e retorna ao contexto anterior |

## 4. Jornada A — primeira configuração

**Entrada:** pessoa abre o OpenKoda pela primeira vez. **Saída:** existe um Ambiente utilizável e um provedor configurado, ou um caminho claro para usar o que está disponível sem eles.

Os conceitos de [Kali e ferramentas](conceitos/02-kali-e-ferramentas-v1.png) e [Configurações](conceitos/05-configuracoes-v1.png) orientam a aparência dessas etapas. Não existe ainda uma tela de onboarding implementada nem evidência de que os estados verdes mostrados já tenham sido alcançados.

| Etapa | Experiência | Exceção e recuperação |
|---|---|---|
| 1. Entender o produto | Mostrar o ciclo Projeto → Avaliação → Evidência → Achado → correção/reteste, com exemplo de laboratório | A pessoa pode consultar o painel e documentação antes de configurar tudo |
| 2. Diagnosticar o computador | Listar requisitos detectados com resultado e próxima ação | Privilégio Windows, reboot ou virtualização exigem ação específica; não apresentar botão que aparenta resolver tudo sem intervenção |
| 3. Escolher Ambiente | Distinguir Ambiente gerido pelo app de distribuição já existente | Descoberta não adota/modifica distro automaticamente; explicar propriedade e impacto da manutenção |
| 4. Preparar Kali | Exibir etapas, origem, integridade/versão quando disponíveis, espaço e progresso | Download interrompido pode ser retomado/verificado; instalação parcial fica identificada e não recebe estado “pronto” |
| 5. Selecionar ferramentas | Oferecer conjunto mínimo para o laboratório e catálogo por área | Estimativa vem da resolução efetiva; ferramenta instalada mas incompatível mostra motivo |
| 6. Configurar IA | Informar provedor, modelo, credencial e classes de dados que podem sair da máquina | Falha de conexão não apaga configuração; sem IA, o painel local continua consultável |
| 7. Conferir prontidão | Resumir Ambiente, ferramentas, acesso a dados e limite de orçamento | Componentes indisponíveis bloqueiam somente as operações dependentes, com motivo |
| 8. Abrir laboratório | Criar/selecionar Projeto de exemplo controlado quando a primeira entrega o suportar | Laboratório e alvo devem ser identificados; não iniciar avaliação de um destino externo implícito |

Etapas que podem levar muito tempo devem sobreviver ao fechamento da janela segundo o contrato de tarefas de manutenção. Se houver reboot, o onboarding retorna ao último estado comprovado e reconcilia o restante; não repete ações de instalação às cegas.

## 5. Jornada B — criar Projeto e cadastrar o escopo

A entrada é **Projetos**, seguindo a [lista e o detalhe aprovados](conceitos/04-projetos-v1.png). Os Projetos e alvos mostrados na imagem são exemplos, sem cadastro real. A busca e Adicionar projeto pertencem à lista; alvos, escopo e avaliações anteriores ficam no detalhe selecionado.

1. Informar nome e objetivo do Projeto.
2. Adicionar fontes e alvos com tipo explícito: código/diff, web/API, rede/infra ou arquivos offline.
3. Definir o que está incluído e excluído, limites de operação e contexto de autorização.
4. Referenciar credenciais necessárias por função, sem mostrar segredos no cartão do alvo ou na conversa.
5. Selecionar contexto útil: arquitetura conhecida, finalidade dos serviços, ambiente de teste, restrições e janela de trabalho.
6. Salvar uma revisão e ver o resumo do escopo antes de abrir uma Avaliação.

**Problemas previstos:** caminho deixou de existir, repositório mudou, documento importado é inválido, um host resolve para destinos fora do envelope, redirect aponta para outro alvo, escopo conflita com exclusão ou a janela terminou. O painel mantém o rascunho, indica o item afetado e pede uma alteração concreta; não aumenta o escopo automaticamente.

Alterar o escopo do Projeto não reescreve Avaliações já iniciadas. Cada Avaliação pode ajustar seu vínculo ao escopo enquanto Rascunho; ao iniciar Preparando, esse vínculo congela. Qualquer mudança posterior do escopo cria uma nova Avaliação vinculada à anterior, com autoridade revalidada e sem herdar visualmente a aprovação antiga. Revisões de plano no mesmo escopo continuam possíveis.

Cadastro de um alvo não executa descoberta de rede nem prova, sozinho, uma autorização externa. O produto registra a declaração e as regras que o Executor consegue verificar; não se apresenta como verificador universal de propriedade de ativos.

## 6. Jornada C — avaliação assistida do início ao fim

**Exemplo de intenção:** “Avalie esta aplicação do meu laboratório e me explique os riscos que conseguir comprovar”. Os valores reais do alvo são escolhidos no Projeto, não inferidos de um exemplo da interface.

**Entrada Nova avaliação:** a ação leva à área Avaliações e abre o [formulário aprovado](conceitos/01-nova-avaliacao-v1.png) no painel de detalhe, mantendo a lista contextual. Modalidades Código, Web/API, Rede e Arquivos aparecem juntas. Projeto/alvo, objetivo, escopo, modo e modelo vêm primeiro; profundidade/contexto e limites avançados ficam em expansões. “Assistido” e “Modelo configurado” são valores ilustrativos, cuja ligação aos perfis e capacidades depende da configuração real.

“Revisar e iniciar” deve levar à conferência de destino, efeitos, dados compartilhados e política aplicável antes do despacho autorizado. Não significa executar diretamente tudo que o formulário permitir digitar. Pacote ausente oferece abrir Kali e ferramentas e voltar ao rascunho preservado; nenhuma instalação ocorre silenciosamente como efeito de criar a Avaliação. Após o início, o [painel de avaliações](conceitos/painel-openkoda-v2.png) orienta o detalhe do trabalho selecionado.

| Passo | Área que a pessoa utiliza | Registro que deve resultar |
|---|---|---|
| Objetivo | Nova Avaliação / conversa contextual | Objetivo, modalidade, contexto e fonte do pedido |
| Escopo e método | Configuração da Avaliação | Revisão de alvos/exclusões, perfil, Ambiente e orçamento |
| Planejamento | Aba Plano | Plano de ação com operações, dependências, efeitos e resultado esperado |
| Revisão | Tela de autorização quando necessária | Aprovação ou política aplicável à revisão e ao envelope preciso |
| Execução | Aba Progresso / central de atividades | Execuções identificadas, parâmetros/versões, tempos, estados e saída |
| Investigação complementar | Plano e conversa | Próxima operação no mesmo escopo com revisão de plano/autoridade quando necessária; mudança de escopo após Preparando cria nova Avaliação vinculada |
| Candidatos | Lista de resultados | Sinais extraídos com origem, sem promoção automática por aparência |
| Validação | Detalhe do candidato e evidências | Método, evidência, contraevidência e resultado da validação |
| Conclusão | Resumo e cobertura | Achados, inconclusivos, falhas, exclusões e o que não foi avaliado |
| Relatório | Prévia de exportação | Resultado coerente com o registro, com seleção/redação de conteúdo |

### Estados da Avaliação que aparecem ao usuário

`Rascunho → Preparando → Planejando → Aguardando aprovação → Na fila → Executando → Validando → Concluída` é o caminho de referência. O ramo de Aguardando aprovação pode ser omitido quando uma política válida cobre o plano. Os demais estados canônicos da Avaliação são `Pausando`, `Pausada`, `Bloqueada`, `Cancelando`, `Cancelada`, `Efeito desconhecido`, `Parcial` e `Falhou`. As transições permitidas são as de [DADOS-E-ESTADOS.md](DADOS-E-ESTADOS.md), sem transições inventadas a partir de uma frase da IA.

Aguardando aprovação informa a decisão concreta pendente. Bloqueada informa a causa, como pacote, credencial, orçamento, compatibilidade ou conexão. Uma falha de etapa pode resultar em Parcial depois da validação, desde que existam resultados utilizáveis e lacunas explícitas. Preparando para retomar descreve a transição de Pausada para Preparando com checkpoint, sem renovar autorização automaticamente.

“IA reconectando” e “Ambiente indisponível” são sinais dos componentes, não estados adicionais da Avaliação. “Cancelamento solicitado” e “Estado desconhecido” pertencem ao ciclo de uma Execução. O painel mostra esses estados subordinados junto do estado canônico da Avaliação, sem tratá-los como sinônimos.

## 7. Jornada D — revisar um plano e delegar trabalho

O cartão de revisão aparece no detalhe quando há decisão pendente e mostra primeiro o que muda para a pessoa; parâmetros completos ficam disponíveis em uma seção expandida. A navegação aprovada não exige manter uma coluna permanente de aprovações nem abrir a conversa para decidir. A tela específica desse cartão continua pendente de desenho.

```text
Plano revisão 3 — Avaliação X
Objetivo e alvos: ...
Vai realizar: ...
Efeitos previstos: leitura / escrita / tráfego / privilégios / persistência
Ambiente e perfil: ...
Limites: destinos, caminhos, duração, volume, orçamento e validade
Dados que poderão ir à IA: ...
Operações: lista e dependências — detalhes completos expansíveis
Mudanças desde a revisão aprovada: ...

[Aprovar esta revisão] [Ajustar plano] [Negar]
```

Quando houver delegação prévia, o cartão informa qual política cobre a ação e a opção de revogar. A política não se confunde com “aprovar qualquer comando futuro”. Se o plano mudar, o cartão anterior permanece no histórico e a nova revisão exibe as diferenças.

O ajuste do plano pode permanecer na mesma Avaliação se respeitar o escopo congelado e revalidar seus efeitos e autoridade. Se exigir mudança desse escopo depois de Preparando, a tela oferece uma nova Avaliação vinculada; o registro e as decisões da Avaliação original permanecem preservados.

Uma negativa preserva o trabalho já concluído e permite ajustar o plano. Uma autorização expirada ou revogada não vira erro genérico de ferramenta. Um pedido repetido com a mesma identidade deve mostrar a decisão já registrada ou explicar por que a operação é diferente, evitando decisões duplas ambíguas.

## 8. Jornada E — acompanhar e interromper trabalho longo

O detalhe prioriza resumo/progresso, estado e próximo passo, conforme o [painel aprovado](conceitos/painel-openkoda-v2.png). Etapa/operação, processos, fila, tempo, orçamento e saída limitada/paginada ficam acessíveis nas seções pertinentes, sem exigir exposição simultânea. Mensagens de progresso não devem fabricar percentuais quando a ferramenta não fornece estimativa confiável; a barra ilustrativa da imagem não define um cálculo de cobertura.

Conversa com a IA abre na lateral e Execuções/terminal no painel inferior, cada um sob demanda. Fechar uma dessas áreas não para o trabalho. Seleção, posição de leitura e referências devem ser preservadas quando a área reabrir, com retorno de foco ao controle que a abriu. Esse comportamento e a composição dos painéis abertos ainda precisam de desenho e teste.

| Ação | Semântica percebida |
|---|---|
| Pausar novas operações | Impede novos despachos no trabalho; processos em andamento seguem conforme a política e a tela informa isso |
| Cancelar uma operação | Solicita encerramento do processo/filhos geridos daquela Execução e registra o resultado de cleanup |
| Parar a Avaliação | Interrompe fila e solicita término das Execuções geridas; preserva evidências e cobertura parcial |
| Revogar delegação | Retira autoridade para operações futuras e afeta as ativas conforme a política anunciada |
| Fechar a janela | Segue uma preferência explícita e informa se haverá tarefa gerida em segundo plano ou interrupção |

A Avaliação usa Cancelando enquanto o encerramento é supervisionado e Cancelada somente após a reconciliação; se seus efeitos não puderem ser confirmados, usa Efeito desconhecido conforme o contrato. Cada Execução mostra separadamente Cancelamento solicitado, Estado desconhecido ou o encerramento efetivamente observado. O botão de parar não deve encerrar uma distribuição WSL inteira compartilhada nem processos alheios como atalho silencioso.

Se efeitos já ocorreram no alvo, cancelar não promete desfazê-los. A UI informa o que foi observado e se existe recuperação/reversão suportada. Operações de rede e mudanças no alvo não devem receber um rótulo genérico “revertido” sem prova.

## 9. Jornada F — revisar candidato e achado

A entrada é **Achados**, com busca/filtros, lista e detalhe como na [referência aprovada](conceitos/03-achados-v1.png). Resumo, Evidências, Correção e Histórico ficam no detalhe. Candidatos precisam permanecer identificáveis por filtro/seção e origem, sem serem promovidos pela seleção. A conversa segue opcional e recolhida; severidade e confiança continuam independentes.

O detalhe reúne:

- alegação em linguagem clara e alcance exato;
- estado técnico: candidato, validado, inconclusivo ou descartado, com método;
- severidade sugerida e sua justificativa;
- confiança e lacunas;
- evidências primárias e contraevidência;
- referências a fontes, Execuções e versões;
- histórico de revisão humana, sem apagar o resultado técnico original;
- recomendações, correção e reteste vinculados.

O usuário pode contestar a alegação, corrigir contexto, marcar duplicata ou descartar com motivo. A comparação/agrupamento não apaga instâncias com alvos, versões ou causas diferentes. Mudança de severidade ou estado fica no histórico com autoria e justificativa.

Uma conclusão deve ser apresentada como estática, dinâmica controlada, inferida ou inconclusiva conforme a evidência. A pessoa não deve precisar abrir todos os logs para descobrir que um “achado” era apenas um alerta do scanner.

## 10. Jornada G — propor correção, aplicar e retestar

1. A IA propõe uma recomendação e explica a ligação com o Achado.
2. Quando suportado, apresenta patch ou alteração estruturada contra uma base identificada; a pessoa pode comparar e ajustar.
3. O produto distingue salvar a proposta, autorizar aplicação e executar a alteração.
4. Se a base mudou ou existe conflito, interrompe a aplicação e solicita revisão; não força substituição.
5. Uma correção feita fora do app pode ser registrada com a nova revisão/versão, sem fingir que o OpenKoda a aplicou.
6. O reteste cria trabalho vinculado ao Achado e à versão corrigida, reutilizando apenas a configuração ainda válida.
7. O resultado pode ser correção verificada, problema ainda presente ou inconclusivo, com evidências e cobertura do reteste.

Comparar “antes/depois” não exige substituir a Avaliação anterior. Reversão só é oferecida quando o método preservou material suficiente e o estado atual permite executá-la. Deploy, publicação e envio de patch são integrações futuras distintas da correção local.

### Reparo de falha operacional pela IA

Corrigir um Achado e reparar ferramenta/Ambiente são percursos distintos. Uma falha operacional abre diagnóstico com logs e prontidão no contexto da Avaliação ou em Kali e ferramentas. A IA pode propor um plano de reparo/pacote; aplicar manutenção depende da autoridade e dos locks pertinentes. Depois, é necessário verificar saúde e decidir como retomar, preservando o resultado da tentativa que falhou.

O [conceito Kali e ferramentas](conceitos/02-kali-e-ferramentas-v1.png) fornece a referência visual, mas não comprova diagnóstico, reparo ou instalação funcionando. Nem retorno zero de comando nem indicador verde de exemplo bastam para marcar reparo como verificado. Falha sem solução permanece Bloqueada, inconclusiva ou encerrada com a limitação correspondente.

## 11. Jornada H — conferir o bruto e exportar um relatório

O viewer oferece abas de resultado normalizado, resumo e bruto, com origem, hash, versão de ferramenta/parser e indicação de limites. Se o bruto não tiver viewer seguro, a tela explica o formato e oferece uma ação explícita de exportação/abertura apropriada; não executa o arquivo para mostrar uma prévia.

Evidências e Arquivos abrem no contexto da Avaliação ou do Achado selecionado. Relatórios e exportação também partem desse trabalho, sem exigir novos destinos primários na navegação. As imagens aprovadas orientam esses acessos, mas não mostram o viewer completo nem a prévia de exportação; ambos ainda precisam de desenho e implementação.

Na exportação, o usuário escolhe público e pacote: resumo executivo, detalhes técnicos, cobertura/limitações e evidências selecionadas. A prévia inclui as redações aplicadas e os metadados que permanecerão. Um pacote parcial recebe essa indicação; uma evidência perdida não é substituída por texto inventado da IA.

O aplicativo salva a exportação no destino escolhido e informa formato, estado e identidade da Avaliação. Envio externo, ticket ou publicação tem ação própria e contexto revisável. A existência de um conector configurado não deve transformar “gerar relatório” em “enviar relatório”.

## 12. Jornadas de recuperação

| Evento | O que a pessoa vê | Próxima ação permitida |
|---|---|---|
| Queda da IA ou cota esgotada | Execuções e orçamento conhecidos continuam visíveis; próxima inferência está pausada | Reconectar/trocar configuração autorizada, ajustar orçamento ou encerrar parcialmente |
| Ferramenta falha | Execução com Falha de início ou encerramento com erro, saída/exit e impacto sobre a etapa | Corrigir causa, repetir como nova tentativa autorizada no mesmo escopo ou concluir a Avaliação como Parcial quando houver resultado utilizável |
| Saída muito grande | Limite atingido, parte disponível e localização/estado do bruto | Paginar, usar parser, gerar artefato ou repetir com operação ajustada; não ocultar o truncamento |
| Processo perdeu vínculo | Estado desconhecido da Execução, último sinal e identidade conhecida; Avaliação segue o estado reconciliado de seu contrato | Reconciliar com Executor; não repetir ação de efeito só porque a UI não recebeu retorno |
| App/host reinicia | Lista de trabalhos recuperáveis e estado reconciliado | Retomar passos idempotentes ou revisar ações de efeito conforme contrato |
| WSL/Ambiente para | Novos despachos dependentes bloqueados, Execuções em reconciliação e evidências preservadas | Diagnosticar/reiniciar Ambiente e decidir retomada sem repetir efeitos silenciosamente |
| Disco fica cheio | Gravação afetada e evidências potencialmente incompletas identificadas | Liberar espaço por ação revisável, mudar destino suportado ou encerrar com parcialidade |
| Pacote/versão muda | Ambiente mudou e avaliações históricas conservam sua identidade | Revalidar prontidão; se mudar a revisão de Ambiente fixada, preparar nova Avaliação vinculada em vez de reescrever a original |
| Corrupção/integridade divergente | Item não confiável e conclusões dependentes sinalizadas | Recuperar backup/original, preservar incidente e recalcular derivação quando suportado |
| Fonte/alvo muda durante trabalho | Mudança detectada e ponto de comparação registrado | Se mudar o escopo ou a revisão de fonte fixados após Preparando, criar nova Avaliação vinculada; a original preserva estado/resultado e pode ser encerrada com limitação explícita |
| Autorização expira | Operações afetadas aguardam nova decisão, sem autoridade presumida | Revalidar autorização e plano dentro do mesmo escopo e janela; encerrar se não houver autoridade aplicável |
| Janela do escopo termina | Novas operações fora da janela ficam impedidas e resultados existentes são preservados | Encerrar a original conforme a cobertura; uma janela diferente exige nova Avaliação vinculada, sem alterar o escopo congelado |
| Duas tarefas disputam manutenção | Fila/conflito de recurso, sem falsa falha de avaliação | Aguardar ou cancelar a tarefa gerida correspondente |

Retomar uma Avaliação não significa reenviar tudo ao modelo ou repetir todos os processos. A tela precisa explicar qual parte será retomada e qual decisão/ambiente é exigido agora.

## 13. Privacidade e contexto enviados à IA

Na configuração e na Avaliação, a pessoa deve compreender que classes de dados poderão ser processadas pelo provedor: objetivo, contexto, trechos de código, saída selecionada de ferramentas, conteúdo de evidências e metadados pertinentes. O conteúdo efetivamente enviado deve ser rastreável no nível definido pela política de retenção, sem expor chaves na UI ou no histórico comum.

Uma prévia pode resumir classes/itens selecionados em vez de exigir confirmação por fragmento. Se houver regra de redação ou exclusão, o painel indica o efeito e as limitações da análise resultante. “Não enviar ao modelo” e “apagar do disco” são decisões distintas.

A referência de [Configurações](conceitos/05-configuracoes-v1.png) comunica que a mudança do modelo padrão vale para próximas Avaliações. Salvar esse padrão não troca silenciosamente o provedor/modelo de uma Avaliação ativa ou Pausada. Qualquer mudança de revisão fixada segue [DADOS-E-ESTADOS.md](DADOS-E-ESTADOS.md), com vínculo preservado e nova Avaliação quando exigida. Compatibilidade de ferramentas/contexto, efeito sobre dados e orçamento continuam verificações futuras, sem promessa de continuidade idêntica entre modelos.

## 14. Acessibilidade e uso prolongado

- Fluxos principais por teclado, ordem de foco estável e retorno ao elemento de origem ao fechar diálogo.
- Tabelas com cabeçalhos, filtros nomeados e atalhos anunciados; seleção preservada quando chegam novos eventos.
- Status por texto/ícone, além de cor. Severidade e estado de execução não usam apenas vermelho/verde.
- Atualizações de progresso anunciadas sem interromper continuamente leitura ou navegação.
- Logs e saída em áreas paginadas, com pausa de rolagem automática e indicação de novos eventos.
- Tamanho de texto ajustável, contraste, temas e layouts que suportam zoom sem esconder interrupção/estado.
- Diálogos de decisão com objetivo, efeito e botões nomeados; nenhum “Sim” sem contexto.
- Gráficos complementam listas/tabelas acessíveis. Densidade de painel não pode esconder alvos ou pendências.
- Ícones do trilho secundário têm nomes acessíveis para Kali e ferramentas e Configurações; avatar ilustrativo não implica um fluxo de conta/login já definido.
- Recolher/reabrir lateral e painel inferior preserva seleção e devolve foco; lista/detalhe em janela estreita precisa de um caminho de retorno, sem depender de gesto de mouse.

Os critérios precisam de validação real por teclado e tecnologia assistiva quando houver interface construída. Este mapa não equivale a uma declaração de conformidade.

## 15. Configurações e manutenção na experiência

Configurações devem separar uso do produto de administração do Ambiente. Instalar/atualizar pacotes, mudar uma distro gerida, remover dados e alterar credenciais têm impacto e ciclo próprios. A UI informa escopo da mudança, estimativa quando conhecida, trabalhos afetados, progresso e recuperação.

**Kali e ferramentas** fica no acesso secundário, com as áreas Ferramentas, Ambiente e Manutenção da [referência aprovada](conceitos/02-kali-e-ferramentas-v1.png). Selecionar um conjunto prepara a revisão: calcular dependências/espaço vem antes de aplicar mudanças. Valores “A calcular” não serão substituídos por números fixos de demonstração. Um pacote instalado não comprova hardware, adaptador ou parser compatível.

**Configurações** também fica no acesso secundário. O [conceito aprovado](conceitos/05-configuracoes-v1.png) organiza Geral, IA e modelos, Permissões, Dados e privacidade e Atualizações. A chave mascarada e a nota de proteção indicam a intenção de produto, não um vault já implementado; “Conexão verificada” só poderá vir de teste real com resultado/validade claros. O catálogo completo de ajustes pode ser organizado dentro dessas áreas sem multiplicar a navegação primária.

Excluir um Projeto não remove automaticamente a distribuição Kali, fontes originais ou todos os relatórios exportados. A pessoa vê o que será removido, o que é compartilhado e o que permanece. A retenção pode remover bruto pesado mantendo metadados, mas o produto precisa mostrar que a verificabilidade da evidência foi reduzida; não tratar o hash remanescente como substituto dos bytes.

Atualizações do app, do protocolo MCP, de adaptadores e de ferramentas são versões diferentes. Um pedido de atualização deve indicar qual componente muda e se existe avaliação ativa dependente dele. A política técnica de compatibilidade fica na arquitetura; o painel a traduz para disponibilidade e ação compreensível.

## 16. Cenários de aceitação da experiência

Estes cenários serão executados futuramente contra a fatia real definida em [FASES-E-VALIDACAO.md](FASES-E-VALIDACAO.md). Não são testes já realizados.

| ID | Dado / quando | Resultado que uma pessoa deve observar |
|---|---|---|
| UX-A01 | Primeira abertura com WSL ausente | Diagnóstico correto, etapa necessária e retorno ao onboarding; nenhuma tela de Avaliação indica prontidão falsa |
| UX-A02 | Ambiente pronto, sem IA configurada | Consulta local funciona; iniciar investigação dependente de IA explica requisito sem descartar Projeto |
| UX-A03 | Pessoa cria Projeto com código e alvo de laboratório | Fontes/escopo/revisão são visíveis; nenhuma verificação externa começa ao salvar |
| UX-A04 | IA propõe operação fora do envelope | Pedido mostra diferença concreta e distingue revisão de plano de nova Avaliação por mudança de escopo; nenhuma execução começa antes da decisão aplicável |
| UX-A05 | Pessoa aprova plano e ele muda | No mesmo escopo, o histórico preserva revisões e revalida autoridade; se mudar o escopo após Preparando, cria nova Avaliação vinculada sem reescrever a original nem herdar visualmente sua aprovação |
| UX-A06 | Ferramenta devolve sinal sem validação | Contagem aumenta em candidatos, sem criar achado validado ou sugerir confirmação inexistente |
| UX-A07 | Evidências sustentam uma conclusão | Pessoa navega do Achado ao método, bruto e Execução que o sustentam |
| UX-A08 | Processo longo e pessoa pede parada | UI continua operável; pedido, término confirmado e cobertura parcial são distinguíveis |
| UX-A09 | App cai depois de uma ação com efeito | Recuperação mostra estado reconciliado/indeterminado e não repete silenciosamente a ação |
| UX-A10 | Pessoa aplica correção externa e retesta | Novo alvo/revisão e reteste ficam vinculados; Avaliação antiga não é reescrita |
| UX-A11 | Pessoa exporta relatório com evidência sensível | Prévia e redação correspondem ao arquivo gerado; nenhum envio externo automático |
| UX-A12 | Pessoa usa só teclado e zoom | Consegue navegar por Avaliações/Achados/Projetos e acessos secundários, criar/acompanhar/interromper trabalho e recolher/reabrir áreas opcionais sem perder seleção, foco ou contexto |
| UX-A13 | Ferramenta existe no Kali mas não tem adaptador/capacidade suportada | Tela distingue instalado de utilizável e explica a dependência; não promete suporte por presença do pacote |
| UX-A14 | Alvo/método muda entre duas Avaliações | Comparação evidencia a mudança e não conclui “corrigido” apenas por ausência do alerta |

## 17. Visual aprovado e validações pendentes

As seis referências aprovadas já definem a direção visual: linguagem clara e discreta, navegação primária compacta, lista/detalhe e detalhes sob demanda. A localização opcional da conversa na lateral e de Execuções na área inferior também orienta o desenho. Não é necessário voltar à premissa de três painéis sempre abertos.

Ainda precisam de desenho/teste: áreas opcionais abertas, janelas estreitas e zoom, vazio/sem seleção, carregamento/falha, Pausada/Cancelando/Efeito desconhecido, revisão de plano, acessibilidade, evidência extensa, manutenção parcial e exportação. Dimensões/proporções ilustrativas não substituem testes de conteúdo real e interação; os rótulos/indicadores fictícios não definem novos estados ou resultados técnicos.

**Tauri está confirmado como framework desktop.** Biblioteca de componentes, rotas, integração do frontend com o núcleo, implementação dos estados e comportamento técnico dos workers ainda precisam de detalhamento e validação na arquitetura. Tauri e a aprovação das imagens não comprovam segurança, isolamento, desempenho ou compatibilidade. Os estados e vínculos de domínio continuam definidos em [DADOS-E-ESTADOS.md](DADOS-E-ESTADOS.md). Este documento atualiza o planejamento, sem construir a interface.
