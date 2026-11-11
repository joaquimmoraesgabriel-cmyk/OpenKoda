# Usar o OpenKoda 0.1.6 local

A 0.1.6 acrescenta planos com dependências, fila de trabalho, revisão humana, capturas consultáveis e encerramento local de chamadas de IA cujo resultado remoto permanece incerto. Este guia descreve esses fluxos e os controles anteriores de que eles dependem. O [estado de validação da versão](RETOMADA-0.1.6.md) identifica os artefatos e os testes realizados.

O instalador local foi compilado sem assinatura, conforme a escolha do usuário. A [extração dos pacotes](../.local/installer-workflow-8c86f49a-3731-4bcc-87c1-91dfcea77562/result.json) conferiu seus executáveis. O [ensaio do instalador](../.local/installer-workflow-f71e7255-9bdf-4958-b221-e9581ee8740e/result.json) passou pela instalação da 0.1.5, substituição pela 0.1.6 e desinstalação em uma pasta própria, sem abrir o aplicativo release nem o perfil pessoal. Os dados sintéticos e o estado compartilhado conferido permaneceram preservados.

A migração do armazenamento do schema 6 para o 8 foi validada separadamente, com executáveis debug e dados sintéticos. A abertura do aplicativo release instalado ainda não foi ensaiada. O OpenKoda completo ainda possui etapas pendentes. Preserve a versão anterior e suas cópias de recuperação; não abra um perfil migrado para o schema 8 com o executável antigo de schema 6.

O [guia preservado da 0.1.5](../releases/0.1.5-local-20261108/GUIA-USO-LOCAL.md) continua disponível para os percursos anteriores. As referências de [escopo](SCOPE.md), [fontes combinadas](COMBINED-SCOPE.md), [política de dados](DATA-POLICY.md), [orçamento da IA](AI-BUDGET.md) e [validação HTTP](HTTP-VALIDATION.md) detalham seus contratos.

## Preparar o projeto

1. Em **Configurações → IA e modelos**, confira modelo e credencial. Insira a chave no campo próprio; o aplicativo não a exibe de volta depois de salvar. **Testar conexão** envia uma chamada pequena ao provedor e pode consumir créditos. Salvar configurações não faz esse teste automaticamente.
2. Em **Kali e ferramentas**, confira o ambiente gerido e seus diagnósticos. Preparação e manutenção têm revisões próprias. Instalar um pacote não cria um adaptador para usá-lo; a integração atual não oferece comandos livres ao modelo. O Kali em WSL não é apresentado como isolamento forte contra o Windows.
3. Crie um Projeto e uma Avaliação. Escolha objetivo, alvos, métodos e, quando necessário, o período autorizado. Revise o escopo antes de salvar. Uma avaliação salva apenas como rascunho ainda precisa dessa revisão.
4. Confira a política de dados do projeto e sua adoção na avaliação. Classifique o material e associe suas fontes. Para usar IA ou Kali, a política precisa permitir o destino e os dados selecionados. A classificação não autoriza uma operação sobre o alvo.

Nas avaliações combinadas, selecione cada entrada e seus métodos explicitamente. Cadastrar um alvo no projeto não o inclui no escopo. Depois que o escopo é usado e fica congelado, uma mudança exige uma avaliação derivada com revisão própria. Revogar o escopo bloqueia novas operações; o trabalho iniciado e seus efeitos continuam no histórico.

## Montar um plano com dependências

Abra a avaliação em **Plano e fila** e escolha **Criar plano de ação**. Informe o objetivo do plano e os limites totais de tentativas e de bytes de captura. Esses limites podem ser menores que os máximos aceitos pelo núcleo.

Escolha o **Tipo do próximo nó** e use **Adicionar nó**. Um nó representa uma etapa do plano. Preencha sua finalidade, a observação esperada e as dependências. Se uma etapa usa o resultado de outra, selecione também essa entrada em **Resultados para revisar**; a dependência, sozinha, não escolhe o conteúdo.

| Tipo de etapa | O que você seleciona | O que ainda precisa revisar |
| --- | --- | --- |
| Operação nativa | Método e entrada de escopo; portas no caso TCP | Plano concreto, destinos, limites e autorização da tentativa |
| Revisão humana | Evidências existentes ou resultados dos predecessores | Conteúdo, decisão e nota da revisão |
| Evidência no Kali | Uma evidência existente ou produzida por predecessor; hash ou análise de padrões | Ambiente, dados exatos e transferência local |
| Investigação de IA | Evidências e perspectivas de análise | Contexto, dados, modelo, orçamento e autorização de envio |

Uma investigação de IA produz conteúdo derivado. Você pode encaminhá-lo a uma revisão humana; ele não se torna evidência factual ou achado por aparecer no plano.

Escolha **Revisar plano de ação**. Confira a revisão proposta, os nós, as dependências, os limites, os resultados reutilizados e os nós pendentes que serão substituídos. Marque o reconhecimento e use **Salvar revisão do plano**. Salvar organiza o trabalho: cada operação continua exigindo sua preparação e confirmação.

Para mudar um plano, use **Preparar nova revisão do plano**. Nós já despachados permanecem no histórico. Se já houve despacho, monte os novos passos e selecione explicitamente os resultados que deseja reutilizar. Uma revisão antiga não concede autorização para outra tentativa.

## Preparar e acompanhar cada etapa

Em **Fila e tentativas**, confira separadamente **Controle do plano** e **Resultado dos nós**. Um plano ativo pode ter nós aguardando dependências, preparação ou revisão. Concluir seus nós não conclui a avaliação nem comprova ausência de falhas.

Nos nós disponíveis, use **Preparar operação do nó** ou **Preparar revisão humana**. Se já existe uma preparação, o controle aparece como **Reabrir revisão do nó**. A preparação pode expirar ou perder validade quando o escopo, as entradas, a política ou a revisão mudam; consulte o estado e prepare novamente quando solicitado.

Para uma operação nativa, confira o plano concreto e os limites efetivos. Depois de marcar o reconhecimento, **Autorizar e enfileirar esta tentativa** registra uma tentativa elegível para despacho. A fila executa uma tentativa por vez na instalação; estar na fila não é uma confirmação de início ou término. Use **Atualizar fila** e consulte **Tentativas registradas** e os eventos.

Os adaptadores nativos atuais fazem inventário/SHA-256 local, análise lexical local, observação HTTP com HEAD e conexão TCP aos endereços e portas revisados. HEAD não navega pelo site nem autentica sessões; TCP connect não analisa banners ou vulnerabilidades. Sinais de regras e interpretações do modelo continuam exigindo validação.

Se uma resposta de confirmação se perder, use o controle de consulta da **mesma intenção** ou do **mesmo recibo**, quando oferecido. Ele recupera o registro correspondente sem pedir uma nova tentativa. **Preparar nova tentativa**, quando disponível, é uma ação separada, sujeita a nova revisão e ao estado anterior.

## Registrar uma revisão humana

Abra **Preparar revisão humana** e confira as entradas identificadas. Use os controles de leitura revisada quando o conteúdo não estiver disponível na projeção. Escolha uma das decisões:

| Decisão | Resultado da etapa |
| --- | --- |
| **Continuar após este resultado** | Conclui esta revisão e permite que dependências satisfeitas avancem à próxima preparação |
| **Precisa de mais trabalho** | Registra resultado parcial; não libera o caminho como concluído |
| **Interromper este caminho** | Cancela esta etapa, preservando o registro |

Escreva a nota, confira o reconhecimento e use **Registrar revisão humana**. Essa decisão não valida um achado nem autoriza o próximo nó. A próxima operação ainda passa por sua própria preparação. Fechar a revisão sem confirmar não registra uma decisão; ao reabrir, confira novamente os campos.

## Usar Kali e IA na fila

No nó Kali, escolha **Verificar hash da evidência** ou **Analisar padrões na evidência** ao montar o plano. Depois de preparar a operação, confira **Revisar transferência para o Kali**: ambiente dedicado, identidade da evidência, tamanho e SHA-256. Selecione a sensibilidade e use **Revisar dados da transferência do nó**. Confira os motivos de bloqueio ou permissões, marque os reconhecimentos aplicáveis e use **Autorizar e enfileirar nó Kali**.

Essa confirmação vale para a evidência, a operação e o ambiente mostrados. O parser recebe os bytes da evidência selecionada; você não fornece caminhos ou comandos de shell. Na análise, o produtor preserva uma saída limitada no Kali e o aplicativo coleta essa captura. A reconciliação consulta a execução conhecida; não inicia outra análise para reconstruir uma saída perdida.

No nó de investigação IA, confira as evidências selecionadas e as perspectivas. O envio mantém as revisões de dados e orçamento. As perspectivas são processadas sequencialmente; resultados parciais e consumo já ocorrido permanecem registrados se o trabalho parar. Uma resposta do modelo não autoriza outra ferramenta, e o plano não amplia o escopo da avaliação.

## Ler uma captura

Abra **Tentativas registradas → Capturas preservadas** e use **Consultar capturas da tentativa**. A consulta apresenta metadados: estado, bytes preservados, total observado quando conhecido, origem, hashes e motivos de perda. Ela não concede leitura do conteúdo.

Quando a captura estiver encerrada ou interrompida e a leitura for permitida, escolha **Revisar janela da captura**. Confira sensibilidade e intervalo, reconheça dados sensíveis quando solicitado e use **Confirmar leitura da captura**. Cada janela contém até 32.768 bytes da representação preparada. **Revisar próximo trecho da captura** exige outra revisão; o cursor apenas indica a continuação.

O SHA-256 dos bytes preservados identifica o original capturado. O SHA-256 da representação identifica o conteúdo preparado para leitura, que pode conter ocultações. A redação considera o fluxo completo antes de recortar a janela. Se a codificação não puder ser confirmada como UTF-8, a leitura textual fica indisponível.

Uma captura parcial conserva somente os bytes que puderam ser preservados. Confira **Captura limitada**, as faixas omitidas e o total desconhecido antes de interpretar resultados. Fim da representação preservada não significa fim de toda a saída produzida.

## Pausar, cancelar ou reconciliar

| Controle | Efeito que você confirma |
| --- | --- |
| **Pausar novos despachos** | Impede novos inícios; a tentativa em execução continua |
| **Revisar retomada** | Mostra nós elegíveis e nós que precisam de nova revisão; não renova aprovações vencidas |
| **Solicitar cancelamento do nó** | Pede o cancelamento da etapa; aguarde confirmação de término |
| **Cancelar plano** | Solicita o encerramento do caminho do plano, mantendo histórico e resultados |
| **Revisar reconciliação desta tentativa** | Consulta o trabalho conhecido em estado desconhecido, com justificativa, sem repetir a operação |

Um pedido de cancelamento não desfaz efeitos já enviados. Uma tentativa com efeito desconhecido pode continuar bloqueando novos trabalhos enquanto o núcleo não consegue provar sua situação. Reiniciar o aplicativo não transforma incerteza em sucesso nem restaura autorizações expiradas.

## Encerrar somente a posse local de uma chamada de IA

Use **Encerramento local de IA** em **Plano e fila** ou em **Configurações → IA e modelos**. **Consultar encerramentos locais** verifica a situação no núcleo. Somente operações consideradas elegíveis podem abrir **Revisar encerramento desta operação**. Uma lista vazia não autoriza encerrar outra operação por fora desse fluxo.

Na revisão, confira a operação, as chamadas com resultado remoto incerto, as filas a pausar e as preparações a invalidar. Informe uma justificativa de até 256 caracteres e 1.024 bytes UTF-8. Leia os três reconhecimentos antes de **Confirmar encerramento local**.

O recibo registra o encerramento da posse local. O resultado remoto e a cobrança continuam desconhecidos. A ação não consulta nem cancela no provedor, não repete a chamada e não faz estorno. O nó encerrado localmente não libera dependências nem pode ser repetido como se tivesse terminado normalmente. As filas anteriores são pausadas; novos trabalhos exigem novas preparações e admissões.

O controle financeiro permanece separado. Uma reserva ou custo desconhecido não desaparece com o encerramento local; consulte [Orçamento da IA](AI-BUDGET.md) para a revisão financeira apropriada. Após restaurar um perfil, um recibo histórico não prova que a operação está liberada no perfil atual: a elegibilidade depende de nova inspeção local. A [decisão de domínio](../../docs/adr/0006-encerramento-local-com-resultado-remoto-incerto.md) explica essa distinção.

## Excluir bytes de capturas com retenção revisada

Em **Configurações → Dados e privacidade → Revisão de retenção e exclusão**, use **Atualizar arquivos e histórico**. Selecione até 20 itens e escolha **Revisar exclusão selecionada**. **Captura local de operação** e **Captura no Kali** são cópias distintas: selecione cada uma que pretende remover.

Confira identidade, tamanho, hashes, ambiente e validade do plano. Marque o reconhecimento, digite a frase exibida e use **Excluir itens revisados permanentemente**. O conteúdo e suas janelas de leitura deixam de estar disponíveis; manifesto, origem e hashes históricos permanecem. Excluir a captura não exclui automaticamente o projeto nem as demais evidências.

Se o resultado ficar desconhecido, abra o plano no histórico e use **Inspecionar resultado sem excluir** quando disponível. Essa inspeção não repete a exclusão. Quando ela confirmar bytes restantes de uma captura Kali, outra seleção prepara um novo plano com referência ao recibo parcial anterior; a remoção do restante exige nova confirmação. Trabalho ativo ou desconhecido pode impedir a retenção até sua resolução.

O prazo em dias é uma referência para seleção manual. Não há coleta automática de lixo. Na 0.1.6, os registros de capturas já purgadas no Kali ainda contam no limite de **256 entradas** do armazenamento temporário. Por isso, ciclos repetidos de captura e purga podem esgotar a capacidade mesmo depois de remover os bytes. A recuperação dessa capacidade por retenção de metadados é uma etapa posterior; não remova manualmente os registros para contornar o limite, pois eles sustentam identidade e recuperação.

## Preservar e restaurar

Use **Revisar backup de recuperação** para conferir os projetos, os dados e as exclusões antes de criar a cópia lógica. O backup inclui o histórico de trabalho e as capturas locais abrangidas pela revisão; não é uma imagem da distro Kali. Credenciais, disco Kali, fontes externas e o orçamento global têm tratamento separado.

A restauração prepara uma cópia inativa. Revise políticas, credencial e ambiente antes de reativar trabalho. Restaurar não inicia a fila, não renova orçamento nem torna válida uma antiga autorização de envio. Uma captura restaurada em outro perfil não concede, por si só, propriedade sobre a cópia existente no Kali.

O expurgo de um projeto arquivado é outro fluxo. Capturas geridas no Kali que ainda dependem de retenção ou reconciliação podem bloqueá-lo; conclua os controles exibidos antes de tentar excluir o histórico.

## Limites e alcance dos testes

Os máximos do plano são 32 nós, 64 dependências, 20 revisões e 100 tentativas; o plano pode estabelecer limites menores. A fila admite uma tentativa em execução por vez. As capturas locais estão sujeitas ao teto de 16 MiB por perfil e aos limites efetivos da operação. A análise de padrões no Kali preserva até 64 KiB de saída; atingir o limite produz cobertura parcial. O [checkpoint](RETOMADA-0.1.6.md) registra os testes de limites, recuperação e compatibilidade realizados.

O percurso com NVDA 2026.3 verificou cinco cenários delimitados: navegação da fila, revisão nativa, revisão humana, controles de pausa/cancelamento e consulta vazia de encerramento local. Usou o Visualizador da Fala com síntese silenciosa, mouse e teclado, sem confirmar operações. Não avaliou a voz, toda a aplicação só por teclado, leitura paginada de capturas nem confirmação de encerramento elegível; não constitui auditoria WCAG completa.

Os testes de IA desta rodada usam provedor simulado; eles não comprovam uma nova chamada real à OpenAI. O ensaio real no Kali usou entrada sintética. Reinício de processo não equivale a queda de energia. O ensaio positivo de instalação, substituição e desinstalação não cobre Windows limpo, interrupção ou rollback; a migração debug separada não comprova a abertura nem a migração pelo aplicativo release instalado. As demais capacidades e verificações continuam acompanhadas em [IMPLEMENTACAO](../IMPLEMENTACAO.md) e no [planejamento de fases](../../planejamento/FASES-E-VALIDACAO.md).
