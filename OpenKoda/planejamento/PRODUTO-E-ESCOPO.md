# OpenKoda — produto e escopo

**Estado:** mapa de produto, revisão 0.2, 7 de novembro de 2026. A direção do produto, o framework desktop Tauri e seis referências visuais foram confirmados pelo usuário. Os comportamentos permanecem especificados para implementação e validação futuras; as imagens não são um aplicativo funcional. Este documento mantém a etapa de planejamento, sem construção, instalação de Kali ou execução de avaliações.

## 1. O resultado que o usuário deve obter

O OpenKoda reúne, em um aplicativo desktop, o trabalho de entender um alvo, planejar uma avaliação de segurança, usar ferramentas do Kali, conferir evidências e acompanhar correções. A pessoa informa o que quer avaliar e o que permite fazer; a IA ajuda a investigar e explicar; o aplicativo mostra o que foi executado, o que foi comprovado, o que permaneceu incerto e como repetir a verificação. O valor é chegar a uma conclusão rastreável, com menos troca manual entre chat, terminal, arquivos e relatórios.

O problema de produto é a fragmentação desse trabalho. Uma conversa pode perder o vínculo com o comando executado; uma saída de ferramenta pode parecer um achado confirmado; uma mudança no ambiente pode impedir reprodução; um relatório pode omitir a cobertura que faltou. O painel deve preservar esses vínculos durante toda a Avaliação, incluindo quando uma ferramenta falha ou a IA perde conexão.

**Base de evidência disponível:** intenção explícita do usuário e investigação local anterior. Não foram feitas entrevistas com outros usuários, medições de demanda ou testes de usabilidade. As metas e prioridades abaixo são hipóteses de produto para validação, não resultados medidos.

## 2. Confirmado, proposto e pendente

| Estado | Definição |
|---|---|
| Confirmado pelo usuário | Nome OpenKoda; foco em cibersegurança; aplicativo local; painel semelhante em função ao Codex Security; IA usando Kali Linux no WSL 2; opções para instalar conjuntos de ferramentas; Tauri como framework desktop; seis referências visuais aprovadas; mapear antes de construir |
| Proposto como base | Windows desktop, uso individual inicial, Projetos persistentes, avaliações de código/web/API/rede e evidências offline, execução por MCP mediada por um Executor, planejamento e histórico rastreáveis |
| Direção visual aprovada | Navegação primária Avaliações/Achados/Projetos, acesso secundário a Kali e ferramentas e Configurações, lista e detalhe, informação revelada pela seleção e controles discretos; conversa lateral opcional e execuções em painel inferior opcional, recolhidas nas figuras |
| Comportamento especificado, ainda não implementado | Autonomia configurável e delimitada; transições de estado, aprovações, execução, validação, revisão humana, recolhimento dos painéis, manutenção e recuperação |
| Pendente de decisão ou validação | Provedor/modelos de IA, arquitetura exata de contenção, componentes e contratos de integração do Tauri, distribuição/instalação, licenciamento, adaptadores da primeira entrega, hardware, formatos finais de exportação e comportamento responsivo/acessível das telas |

“Semelhante ao Codex Security” significa aproveitar os padrões de workbench, avaliação, cobertura, achados e remediação identificados na investigação. A direção visual própria do OpenKoda já tem referências aprovadas; os componentes funcionais ainda serão implementados. Uma tela parecida não implica o mesmo protocolo, o mesmo backend ou as mesmas garantias.

Este planejamento pertence à pasta separada `D:\OpenKoda\OpenKoda`. Os documentos do projeto anterior na pasta pai não são automaticamente compromissos deste produto de segurança local. Termos compartilhados como Plano de ação, Aprovação, Política de automação e Executor mantêm a separação entre conversa e autoridade operacional.

### Referências visuais aprovadas

| Referência | Direção de produto que ela fixa | O que continua sendo especificação futura |
|---|---|---|
| [Painel de avaliações](conceitos/painel-openkoda-v2.png) | Navegação principal, lista/detalhe, resumo de trabalho e seções recolhidas | Progresso real, transições, pausa/parada, aprovações e abertura dos painéis opcionais |
| [Nova avaliação](conceitos/01-nova-avaliacao-v1.png) | Formulário no detalhe, quatro modalidades, campos essenciais e opções avançadas por expansão | Validação de entradas, prontidão, revisão do plano e início autorizado |
| [Kali e ferramentas](conceitos/02-kali-e-ferramentas-v1.png) | Área secundária, saúde compacta, catálogo e resumo da seleção | Diagnóstico, resolução de dependências, estimativas, manutenção e recuperação |
| [Achados](conceitos/03-achados-v1.png) | Lista/detalhe, resumo, evidências, correção e histórico | Validação técnica, confiança, revisão e reteste sustentados por registros |
| [Projetos](conceitos/04-projetos-v1.png) | Lista de projetos, alvos, escopo padrão e avaliações vinculadas | Cadastro, persistência, edição e congelamento de escopo por Avaliação |
| [Configurações](conceitos/05-configuracoes-v1.png) | Acesso secundário e organização de IA/modelos, permissões, dados e atualização | Conexão real, armazenamento protegido, aplicação de limites e compatibilidade |

As seis imagens usam dados fictícios. Rótulos como “Kali pronto”, “Conexão verificada”, “Validado”, quantidades e barras de progresso não comprovam instalação, teste ou avaliação realizados. A aprovação da imagem fixa a direção visual, sem aprovar por implicação cada interação, cada valor de exemplo ou qualquer mudança nos contratos de domínio. O inventário dos conceitos está em [TELAS-ADICIONAIS.md](conceitos/TELAS-ADICIONAIS.md); a relação com as jornadas está em [PAINEL-E-JORNADAS.md](PAINEL-E-JORNADAS.md).

## 3. Pessoas e trabalhos atendidos

| Pessoa / contexto | Trabalho principal | Resultado útil |
|---|---|---|
| Desenvolvedor responsável por uma aplicação | Revisar código, dependências e configuração; relacionar risco ao comportamento real | Achados ligados a arquivos/versões e correções revisáveis |
| Analista de segurança com alvo autorizado | Planejar e conduzir avaliações de web/API/rede com ferramentas Linux | Execuções limitadas ao escopo, evidências e validação reproduzível |
| Responsável por infraestrutura própria | Inspecionar exposição e configuração de serviços permitidos | Inventário contextualizado e prioridades de correção |
| Investigador em laboratório | Analisar binários, capturas ou arquivos sem executar o objeto analisado quando o método permitir | Evidências de origem preservadas e conclusões com limites claros |
| Pessoa aprendendo em laboratório | Entender por que uma operação foi escolhida e o que o resultado significa | Explicação ligada à operação e aos bytes, sem “vulnerável” por aparência |

O uso individual inicial é uma proposta para limitar a primeira entrega. Equipes, papéis organizacionais, servidor remoto e colaboração simultânea são extensões do mapa, não dependências necessárias para usar o produto local.

## 4. Unidade de trabalho

O **Projeto** mantém os alvos, fontes, contexto, regras e histórico. Uma **Avaliação** é um trabalho delimitado dentro desse Projeto, com objetivo, escopo e orçamento próprios. A conversa pode continuar depois de uma Avaliação, mas não altera silenciosamente seu registro encerrado.

Cada Avaliação registra:

- objetivo e modalidade;
- alvos incluídos e exclusões, inclusive limites de caminhos, portas, contas e ambientes quando aplicáveis;
- revisão da autorização de escopo e das permissões;
- estado do código/artefato ou identificação do alvo no momento avaliado;
- Ambiente, versões de ferramentas e Perfil de execução;
- Plano de ação e suas revisões;
- Operações planejadas e respectivas Execuções;
- Evidências brutas e normalizadas, Candidatos, Achados e pendências;
- consumo, interrupções e cobertura efetiva;
- relatório e vínculos com remediação/reteste.

Enquanto estiver em Rascunho, a Avaliação pode ajustar sua revisão de escopo. Ao iniciar Preparando, o vínculo ao escopo é congelado. Depois disso, mudar alvos, objetivo, métodos, exclusões, janela ou outras condições do escopo exige uma **nova Avaliação vinculada à anterior**, preservando o registro e as autorizações da original. A nova Avaliação copia apenas as configurações escolhidas e revalida a autoridade aplicável.

Revisões de Plano de ação dentro do mesmo escopo continuam possíveis, com nova verificação dos efeitos, do perfil e da autorização. Uma aprovação anterior não deve aparecer na tela como aprovação de conteúdo diferente. Os estados e vínculos canônicos estão em [DADOS-E-ESTADOS.md](DADOS-E-ESTADOS.md).

## 5. Quatro modalidades com igual visibilidade

O mapa completo apresenta as quatro modalidades no seletor principal. A disponibilidade real depende dos adaptadores, do Ambiente e das permissões; a primeira versão não precisa entregar o catálogo inteiro.

| Modalidade | Entradas | Atividades possíveis | Saídas e limites |
|---|---|---|---|
| Código e configuração | Repositório, diretório selecionado, patch/diff, manifestos, contexto de arquitetura | Leitura estática, revisão de mudanças, dependências, configuração e modelo de ameaças | Arquivos/linhas/revisões, hipóteses e achados; leitura não equivale a execução ou comprovação dinâmica |
| Web e API | URLs e limites, documentação de API, ambiente de teste, sessão/credencial referenciada | Inspeção de superfície, testes controlados, comparação entre perfis, validação permitida | Requests/responses redigidos, passos e resultados; acesso a outra conta, host ou efeito adicional exige escopo correspondente |
| Rede e infraestrutura | Ativos próprios/autorizados, faixas/portas e janelas definidas, contexto de serviços | Descoberta permitida, inspeção de serviços e verificações específicas de configuração/exposição | Inventário temporal e evidências por ativo; disponibilidade de ferramenta não autoriza qualquer destino |
| Evidências offline | Capturas, logs, arquivos, imagens de disco, binários ou resultados importados | Parsing, análise estática, correlação e explicação, conforme adaptadores | Conclusões sobre os dados fornecidos; ausência em uma captura não prova ausência no sistema inteiro |

**Modo combinado:** uma Avaliação pode relacionar código a uma aplicação web ou inventário de rede, mas deve declarar cada fonte e destino. “Mesmo Projeto” não concede autorização automática a todos os seus alvos em cada Avaliação.

## 6. Catálogo funcional do produto completo

| Área | Capacidades previstas no mapa | Critério de valor |
|---|---|---|
| Preparação | Diagnóstico do Windows/WSL, seleção ou criação futura de Ambiente, escolha de ferramentas, provedor de IA | Pessoa entende por que o Ambiente está pronto ou indisponível |
| Projetos e alvos | Cadastro, agrupamento, fontes locais, escopo versionado, exclusões, referências de credenciais | Nenhuma operação perde o vínculo com o alvo e o contexto |
| Avaliações | Modalidades, objetivos, configurações reutilizáveis, execução única e comparações | Escopo ajustável em Rascunho, congelado ao iniciar Preparando, com cobertura honesta ao concluir |
| IA investigadora | Conversa contextual, esclarecimento do objetivo, plano, escolha de ferramentas, interpretação e investigação complementar | Sugestões e conclusões apontam para evidência ou são rotuladas como hipóteses |
| Orquestração | Fila, dependências, orçamento, paralelismo limitado, cancelamento e retomada | A pessoa acompanha trabalho longo sem perder controle do que está ativo |
| Ferramentas Kali | Catálogo por capacidade, pacotes/metapacotes, versões, requisitos e estado de instalação | “Instalada”, “compatível”, “configurada” e “permitida” são estados distintos |
| Execução MCP | Operações tipadas, parâmetros revisáveis, processos identificados, saída e artefatos | Há uma fronteira verificável entre a proposta da IA e a ação executada |
| Candidatos e achados | Deduplicação contextual, validação, severidade, confiança e revisão | Contagens não misturam alertas de ferramenta com achados validados |
| Evidências | Bruto, resumo, campos estruturados, origem, digest, versão do parser, classificação de sensibilidade | Resumos permitem voltar ao dado que sustenta a conclusão |
| Correção e reteste | Recomendação, patch proposto quando aplicável, aplicação controlada e revalidação | O reteste referencia o achado e a versão corrigida exatos |
| Relatórios | Resumo executivo, detalhes técnicos, cobertura, pendências e exportação selecionada | Outra pessoa consegue entender alcance e reproduzir o que for apropriado |
| Configuração e manutenção | IA, rede, privacidade, permissões, Ambiente, pacotes, armazenamento e recuperação | Alterações relevantes não surpreendem uma Avaliação ativa |
| Integrações futuras | MCPs adicionais, importação/exportação, rastreadores e execução remota | Conectores ampliam o produto sem assumir autoridade por sua presença |

O catálogo é um mapa do produto, não um compromisso de construir todos os recursos de uma vez. A sequência recomendada e as portas de validação ficam em [FASES-E-VALIDACAO.md](FASES-E-VALIDACAO.md).

## 7. O que a IA pode fazer

| Capacidade | Comportamento desejado | Condição operacional |
|---|---|---|
| Entender contexto | Ler as fontes escolhidas e fazer perguntas pertinentes | Acesso às fontes permitido; dados sensíveis e envio ao provedor tratados explicitamente |
| Planejar | Propor objetivo, passos, dependências, ferramentas e expectativas | Plano visível, versionado e ligado ao escopo |
| Investigar | Pedir operações de leitura/análise/teste e acompanhar resultados | Executor valida cada pedido contra a autorização ou política aplicável |
| Adaptar investigação | Propor próximo passo a partir de um resultado | Dentro do mesmo escopo, revisar plano/efeitos e revalidar autoridade; mudança do escopo após Preparando exige nova Avaliação vinculada |
| Usar ferramentas Kali | Solicitar adaptadores disponíveis e, se o perfil permitir, execução avançada | Compatibilidade, parâmetros, limites e privilégios efetivos verificados fora do prompt |
| Correlacionar | Relacionar código, ativos, execuções e evidências | Não transformar semelhança de texto em identidade de alvo ou prova |
| Validar | Testar uma hipótese por método estático ou controlado permitido | Registrar método, evidência, contraevidência e limitações |
| Redigir | Explicar achados, priorizar, propor correção e relatório | Separar fato observado, inferência, recomendação e desconhecido |
| Coordenar investigadores | Dividir uma Avaliação em subtarefas delimitadas | Proposta do produto completo, com orçamento/concorrência e autoridade herdada sem ampliação |

O usuário não precisa aprovar manualmente toda leitura repetitiva se já concedeu uma política delimitada. Autonomia significa trabalhar dentro desse envelope e saber parar ou solicitar revisão quando uma operação sai dele. O chat não deve conter uma instrução que permita à IA reescrever sozinha os limites que o Executor aplica.

O mapa prevê acesso amplo às capacidades Linux quando requerido pelo trabalho, sem usar “todo o Kali” como um perfil indefinido. Uso de root Linux, mudanças de pacotes, persistência, acesso ao host e operações de rede têm efeitos diferentes e precisam de capacidades explícitas. Nenhum desses nomes promete suporte a hardware ou isolamento que o Ambiente não fornece.

## 8. Plano, execução e validação são objetos distintos

Um Plano de ação pode estar pronto sem nenhuma Operação ter sido executada. Uma Operação pode ter várias Execuções por falha ou reteste. Uma Execução bem-sucedida significa que o processo terminou conforme seu contrato; não significa que a hipótese de vulnerabilidade foi confirmada.

O trabalho deve tornar explícitas estas situações:

| Situação | O que o produto pode dizer |
|---|---|
| Ferramenta não executou | “Não executado”, com motivo e efeito sobre cobertura |
| Processo terminou sem alerta | “Verificação concluída sem evidência nessa verificação”, com alcance |
| Parser extraiu sinal suspeito | “Candidato”, pendente de validação |
| Método e evidências sustentam a alegação | “Achado validado por [método]”, com confiança e limitações |
| Evidências insuficientes ou contraditórias | “Inconclusivo”, mantendo candidato/pendência e contraevidência |
| Pessoa reviu a conclusão | Estado de revisão humana separado da validação técnica |
| Correção foi aplicada | “Alteração aplicada”, ainda distinta de “correção verificada” |
| Reteste confirmou a versão corrigida | “Correção verificada no reteste [id]”, sem extrapolar para outras versões |

Severidade, confiança, estado de revisão e estado de remediação são eixos independentes. Um candidato de alto impacto potencial não deve aparecer como achado crítico confirmado apenas para preencher o painel.

## 9. Autonomia percebida pelo usuário

São propostas de experiência, a serem ligadas aos perfis técnicos definidos pela arquitetura:

| Experiência | O que a IA faz sozinha | O que a pessoa vê |
|---|---|---|
| Planejamento | Organiza o objetivo e propõe operações | Plano ainda não executado |
| Leitura/análise delegada | Executa leituras e análises permitidas no envelope selecionado | Progresso, fontes e gasto; solicitações fora do envelope ficam pendentes |
| Avaliação ativa controlada | Executa operações aprovadas ou abrangidas por política prévia | Alvos/efeitos, processos ativos, limites e interrupção |
| Investigação avançada | Usa capacidades adicionais explicitamente concedidas | Diferenças do perfil, escopo preciso, expiração e evidências de cada ação |

Os rótulos finais devem comunicar autoridade real, não “seguro” versus “liberado”. A pessoa poderá rever e revogar delegações; a revogação deve produzir efeito observável na fila e nas execuções segundo a política definida.

## 10. O painel como fonte de verdade do trabalho

O painel deve responder, sem depender da leitura integral do chat e sem exibir todas as funções ao mesmo tempo:

1. O que estou avaliando e em qual revisão de escopo?
2. Qual Ambiente, perfil e provedor estão sendo usados?
3. O que a IA pretende fazer, o que foi autorizado e o que já começou?
4. Quais processos ainda estão ativos, quais aguardam algo e quanto já foi consumido?
5. O que foi observado e qual evidência sustenta cada conclusão?
6. O que não foi avaliado, falhou, foi excluído ou ficou inconclusivo?
7. Qual correção foi proposta/aplicada e o que foi retestado?
8. O que pode ser exportado ou apagado sem perder a rastreabilidade necessária?

A navegação primária reúne Avaliações, Achados e Projetos. Kali e ferramentas e Configurações ficam no acesso secundário. Avaliações, Achados e Projetos usam lista e detalhe; evidências, relatórios, histórico e correções aparecem no contexto pertinente. Nenhuma dessas funções exige uma aba primária própria.

A conversa abre sob demanda em uma lateral recolhível e Execuções/terminal em uma área inferior recolhível. As referências aprovadas mostram essas áreas fechadas. Fechar uma área apenas muda a apresentação: não pausa a Avaliação, não interrompe um processo e não modifica permissões. Pedidos de decisão, estado e ações de interrupção aplicáveis continuam localizáveis no detalhe/indicadores mesmo com o chat e a saída recolhidos.

Chat, terminal e indicadores derivam do mesmo registro de Avaliação. O texto da IA não pode marcar um processo como encerrado, aprovar uma operação ou declarar uma correção verificada sem os registros correspondentes. A implementação das interações e sua validação permanecem futuras.

## 11. Configurações que o produto precisa expor

| Grupo | Ajustes previstos | Efeito visível |
|---|---|---|
| IA | Provedor, modelo disponível, credencial referenciada, orçamento, limites e política de contexto | Onde o conteúdo será processado e quando o trabalho pausará |
| Ambiente | Distribuição gerida ou existente, estado WSL, recursos, diretórios do Projeto, saúde e versões | Capacidade real disponível, compatibilidade e tarefas de manutenção |
| Ferramentas | Conjuntos, adaptadores, versões, requisitos extras e políticas de atualização | Diferença entre pacote instalado e operação utilizável |
| Execução | Perfis, concorrência, duração, saída, disco, rede e capacidades adicionais | Limites que valem para novas Operações e comportamento das ativas |
| Projeto | Alvos, exclusões, contexto, credenciais, janelas e fontes | Envelope de trabalho revisado e alterações pendentes |
| Privacidade | Regras de envio ao modelo, classificação, redação, retenção e telemetria opcional | Conteúdo enviado, preservado e exportável |
| Armazenamento | Local de dados, espaço, retenção de bruto, cache e backups | Espaço livre, política de limpeza e avaliação que depende de cada item |
| Relatórios | Idioma, público, campos, inclusão/redação de evidências e formatos | Prévia do material que sairá do aplicativo |
| Interface | Tema, densidade, tamanho de texto, idioma, atalhos e notificações | Preferências sem alterar permissões de execução |

Mudanças de configuração não devem ser aplicadas silenciosamente a trabalhos em andamento. A tela deve indicar se uma alteração vale imediatamente, só para novas Operações ou exige revisão do Plano de ação com revalidação. Se alterar o escopo já congelado ou uma revisão à qual a Avaliação está fixada, deve preparar uma nova Avaliação vinculada, preservando a original. A semântica canônica de vínculos e estados está em [DADOS-E-ESTADOS.md](DADOS-E-ESTADOS.md) e precisa aparecer na experiência.

## 12. Local, remoto e offline

“Aplicativo local” descreve onde estão o painel e os componentes instalados. Um provedor de IA hospedado ainda pode receber instruções e trechos de código/evidência. A seleção de um modelo local é uma possibilidade a avaliar, não uma capacidade já entregue nem garantia automática de fluxo totalmente offline.

Sem conexão a um provedor, o usuário deve poder abrir Projetos, ler achados/evidências, conferir histórico e consultar relatórios locais. Ferramentas que não precisam da IA ou de rede podem continuar conforme seu contrato. Um processo já iniciado não deixa de existir porque o chat caiu: seu estado e supervisão permanecem acessíveis.

O produto deve mostrar estados separados para provedor de IA, Internet, WSL/Ambiente e Executor. Um único indicador “online” esconderia causas e induziria a pessoa a repetir uma operação cujo resultado ainda é desconhecido.

## 13. Evidências, relatórios e saída para outros sistemas

Cada relatório precisa declarar objetivo, alvos/versões, método, cobertura, resultados, limitações e pendências. Evidências sensíveis devem poder ser selecionadas ou redigidas na exportação, preservando o original conforme a política de retenção. A prévia mostra exatamente o material escolhido, incluindo metadados que podem revelar caminhos, hosts ou identidades.

Exportação local e publicação/envio externo são ações diferentes. A criação de um relatório não cria automaticamente ticket, comentário, e-mail ou publicação. Integrações futuras devem preparar um resultado revisável e usar o vínculo explícito com Projeto/Avaliação/Achado.

Comparações entre Avaliações devem mostrar mudanças de alvo, fonte, Ambiente e método. “Achado não apareceu de novo” é informação diferente de “correção verificada”.

## 14. Não funcionais e critérios verificáveis de produto

As metas abaixo são propostas para experimentos de aceitação, sem baseline medido. Os limites quantitativos finais dependem de protótipos técnicos e hardware de referência, que ainda não foram construídos.

| Qualidade | Critério proposto |
|---|---|
| Clareza | Em um teste de cenário, a pessoa identifica alvo, perfil, processo ativo e motivo de pausa sem ler o chat inteiro |
| Rastreabilidade | Todo Achado exportado referencia método e ao menos uma Evidência íntegra; o relatório declara itens pendentes |
| Honestidade | Nenhuma falha/truncamento/ausência de ferramenta é contada como teste concluído ou correção verificada |
| Interrupção | O usuário localiza a ação de parar durante uma execução; a UI distingue pedido de cancelamento de término confirmado |
| Recuperação | Após reinício controlado, a pessoa entende quais operações terminaram, continuam, foram interrompidas ou têm estado desconhecido |
| Acessibilidade | Fluxos principais navegáveis por teclado, foco previsível, estados anunciados e informação não dependente apenas de cor |
| Contexto de IA | Prévia/registro permite entender classes de dados que podem ir ao provedor e referências consultadas pela análise |
| Custo | Orçamento e uso parcial são visíveis; estimativas não são apresentadas como preço final garantido |
| Manutenção | Mudança de ferramenta/Ambiente é registrada e não reescreve a identidade de uma Avaliação anterior |
| Responsividade | A UI permanece operável durante ferramentas longas e saída volumosa; limites mensuráveis serão definidos no hardware de referência |

## 15. Métricas a medir, sem inventar resultados

| Resultado | Métrica de validação proposta | Janela e responsável por papel |
|---|---|---|
| Ativação compreensível | Taxa de pessoas que concluem diagnóstico, configuram IA e chegam a uma avaliação de laboratório válida; causas de abandono | Piloto da primeira fatia; produto/UX |
| Trabalho rastreável | Percentual de achados amostrados com evidência recuperável e método explícito; meta de aceitação: 100% na amostra do piloto | Cada candidato a entrega; produto/QA |
| Controle do trabalho | Sucesso em cenários de pausar/cancelar/revogar sem duplicar operação ou interpretar estado errado | Testes de cenários da primeira fatia; UX/execução |
| Qualidade da conclusão | Taxas separadas de candidatos, validados, inconclusivos e descartados em corpus rotulado; divergências revisadas | Antes de expandir modalidade; segurança/QA |
| Valor da explicação | Pessoa consegue apontar o que foi observado e o que ainda não foi comprovado | Sessões de usabilidade do piloto; produto |
| Recuperação | Cenários de interrupção retomados com histórico preservado e sem repetição silenciosa de efeitos | Antes da primeira entrega; execução/QA |

Volume de comandos, número de ferramentas instaladas e quantidade bruta de alertas não são medidas suficientes de valor ou qualidade.

## 16. Catálogo completo versus primeira entrega

O mapa inteiro cobre código, web/API, rede e offline. Para demonstrar a proposta sem simular recursos, a primeira fatia recomendada deve percorrer um ciclo real em laboratório autorizado: preparar Kali, cadastrar um alvo de laboratório, ler código selecionado, executar um adaptador dinâmico simples, preservar evidência, validar uma conclusão e gerar relatório com cobertura. A seleção exata desse adaptador e o Ambiente de laboratório ainda precisam ser decididos.

Rede ampliada, múltiplos perfis autenticados de API, investigação profunda paralela, forense/engenharia reversa especializadas, colaboração e execução remota podem ampliar o produto depois de demonstrado o ciclo. Não se deve preencher o painel inicial com botões que aparentam funcionar sem contrato e implementação correspondente.

Esse sequenciamento não reduz a visão do OpenKoda a um scanner único. Ele cria evidência de que a cadeia completa funciona antes de multiplicar ferramentas e modos. Fases, dependências e critérios de passagem são consolidados em [FASES-E-VALIDACAO.md](FASES-E-VALIDACAO.md).

## 17. Decisões pendentes antes de construir

| Decisão | Responsável proposto | Quando precisa estar resolvida |
|---|---|---|
| Quem será o primeiro usuário e qual laboratório representará o uso real? | Usuário + produto | Antes de fechar a primeira fatia |
| Qual combinação de operações de código e dinâmicas prova o valor inicial? | Produto + segurança + execução | Antes de implementar adaptadores |
| Quais dados poderão ser enviados a cada provedor e onde ficam credenciais? | Usuário + arquitetura | Antes de integrar inferência |
| Qual Ambiente será gerido pelo app e qual contenção efetiva cada perfil oferece? | Arquitetura + segurança | Antes de qualquer execução gerida |
| Que compatibilidade será garantida para Windows/WSL/hardware? | Execução + QA | Antes de publicar requisitos mínimos |
| Como distribuir, atualizar e recuperar o app/Ambiente sem perder evidência? | Arquitetura + operação | Antes de distribuir uma versão instalável |
| Qual é o limite entre correção sugerida e alteração aplicada? | Produto + segurança | Antes da remediação gravar no alvo |
| Quais formatos e redações atendem o primeiro público do relatório? | Usuário + produto | Antes de exportação ser considerada pronta |

## 18. Referências de produto

- [Investigação do Codex Security local](../docs/CODEX-SECURITY.md): base dos padrões de avaliação, draft, cobertura, achados e remediação; não especifica o novo produto.
- [Implicações Kali/WSL/MCP](../docs/KALI-IMPLICATIONS.md): base de capacidades e limitações do ambiente; WSL e MCP não são tratados como garantia de contenção.
- [Ajustes de interface](AJUSTES-DE-INTERFACE.md): organização visual refinada a partir das referências do usuário.
- [Telas adicionais](conceitos/TELAS-ADICIONAIS.md): registro dos cinco conceitos que complementam o painel aprovado, com dados fictícios e limites.
- [Painel e jornadas](PAINEL-E-JORNADAS.md): telas, ações, estados e cenários desta proposta.

O documento descreve o produto a mapear, não promete equivalência integral com Codex Security, suporte a toda ferramenta Kali ou uma avaliação completa de qualquer alvo. Esses resultados dependem do escopo, dos adaptadores e da cobertura registrada em cada Avaliação.
