# Retomada 0.2.1 — código local e GitHub

A 0.2.1 amplia a edição de código com importação de repositórios públicos do GitHub, opções persistentes do projeto e correções de admissão das mensagens. É uma versão local para testes, com recibos do executável e do instalador. **A análise real completa de segurança da 0.2.1 no próprio código ainda não foi concluída, e a publicação pública não está aprovada.**

O executor integrado permanece o SDK oficial `@openai/codex-security` 0.10.0. O OpenKoda controla a admissão, os vínculos com projeto e avaliação, a política de dados, a credencial, a supervisão do processo, os recibos e a apresentação. Os limites técnicos do SDK continuam relevantes mesmo quando a interface apresenta a análise com o nome OpenKoda. Consulte a [decisão do desktop](../../docs/adr/0007-executor-integrado-e-repositorios-importados.md) e o [protocolo do runtime](../security-runtime/README.md).

## Criar e acompanhar um projeto

**Criar projeto** oferece **Pasta local** e **GitHub**. A primeira usa o seletor nativo de pasta; a segunda recebe o link de um repositório público e obtém uma cópia gerenciada. A origem e a revisão Git ficam registradas. O nome da pasta ou do repositório identifica inicialmente o projeto.

O formulário reúne o alvo inteiro ou um caminho dentro dele, o seletor de modelo e a descrição do objetivo. **Contexto adicional** abre três campos: vetores de ataque, áreas de foco e contexto de segurança. **Opções avançadas** concentra título, prazo para iniciar operações e o controle opcional de custo. O formulário não oferece nível de raciocínio.

A análise de código exige um modelo OpenAI ativo nesta versão. O seletor continua compartilhado com o catálogo das configurações; outros provedores continuam disponíveis para conversas. O aviso explica a diferença e a criação fica bloqueada com um provedor incompatível antes da importação ou da admissão do projeto.

**Criar projeto** conclui a importação, quando necessária, admite o projeto e inicia a análise. A conversa reúne atualizações públicas e o resultado. **Projeto** abre Visão geral e Configurações no mesmo painel, com tabelas pesquisáveis e ordenáveis. O painel pode ocupar a área principal ou permanecer ao lado do chat. Navegação, lista de projetos, categorias de configurações e conteúdo têm rolagem independente.

As modalidades históricas de rede, Web/API por URL e Kali/WSL continuam retiradas da navegação e bloqueadas na entrada nativa. Um link GitHub é uma origem para obter código; não reativa os executores antigos. Filas históricas não são retomadas automaticamente.

## Repetição, cancelamento e recuperação

A importação recebe uma chave de idempotência e um recibo consultável. Repetir a mesma intenção recupera esse recibo. A criação e o início da análise também conservam seus recibos, evitando transformar uma resposta perdida em outra avaliação ou execução.

O cancelamento do download vence a intenção de analisar, inclusive se o host confirmar a cópia pronta enquanto o clique de cancelamento está em trânsito. Uma cópia já concluída pode permanecer no cache gerenciado; isso não autoriza iniciar a análise. Se o projeto já tiver sido admitido no momento do cancelamento, ele é preservado e o início da análise é bloqueado.

Quando o download chega a falha, cancelamento ou interrupção confirmados, o formulário oferece **Tentar novo download**. Só essa ação explícita abre uma intenção nova. Um estado ainda desconhecido ou uma resposta perdida mantêm a intenção anterior; não há retry silencioso com outra chave. Reabrir o aplicativo consulta a execução armazenada e não inicia outra automaticamente.

## Conversa e preferências futuras

O compositor limpa o texto e o anexo submetidos quando a mensagem é admitida pelo aplicativo. Erros anteriores à admissão mantêm o rascunho. Uma falha do provedor posterior à admissão permanece vinculada à solicitação já registrada e não torna o texto um novo envio pendente. Um rascunho novo digitado enquanto há resposta em andamento continua separado da mensagem aceita. Trocar o modelo ou reabrir o perfil não reenvia solicitações antigas.

Enter envia, Shift + Enter insere uma linha e a preferência Ctrl + Enter permanece em Geral. O seletor permite mudar entre provedores e modelos ativados nas configurações. A lista de modelos não é filtrada globalmente pela limitação do scanner.

**Projeto → Configurações → Opções de análise do projeto** permite ativar e editar contexto e opções avançadas depois da criação. O salvamento retorna um DTO com os três textos do contexto e aplica a política de projeção. As preferências compõem os próximos trabalhos; não reescrevem o objetivo, o alvo ou o prazo de uma avaliação iniciada, nem renovam uma origem ou autorização revogada. O escopo congelado de uma avaliação continua distinguível das preferências do projeto.

O campo de custo só aparece ao ativar **Personalizar limite de custo**, dentro de **Opções avançadas**. Valor personalizado ausente usa o padrão interno de **US$ 5 por análise**, sem exigir um campo no chat. Uma chamada em andamento pode ultrapassar o teto estimado. O orçamento não é uma garantia de fatura externa e o aplicativo não elimina o limite para tornar um modelo incompatível executável.

## Política de dados e atividade pública

A conversa mostra fases, contagens e operações observadas, além de mensagens públicas recebidas do executor. O transporte não publica mensagens de raciocínio privado nem inventa falas para preencher intervalos. Metadados de ferramentas são limitados; comandos brutos não viram texto de atividade para a interface.

Mensagens, eventos e erros podem conter dados de código mesmo antes da conclusão do relatório. A projeção da 0.2.1 aplica a política de dados a esses campos tanto nas consultas do scan quanto no snapshot. Recusas e revogações ocultam o conteúdo correspondente sem apagar os recibos canônicos ou inventar conclusão, cobertura ou cobrança zero. A classificação sensível do mesmo objeto não é diluída em não classificada apenas porque a atividade mudou; a reclassificação explícita e atual segue seu fluxo próprio.

A revisão encontrou e corrigiu a exposição de atividade sem resultado incorporado e uma regressão com classificação sensível desatualizada. Testes negativos e de liberação posterior cobrem essas condições. Essa projeção protege o que a interface recebe; não define quais arquivos as ferramentas do SDK conseguem ler no Windows.

## Limites do executor local

O perfil efetivo de arquivos do SDK 0.10.0 concede `:root=read` e `:workspace_roots=write`, além das raízes de leitura usadas pelo código e pelo plugin. A casa dedicada de credenciais do SDK recebe uma recusa explícita. O restante do sistema acessível à conta Windows não é confinado ao repositório por essa configuração. A leitura real de cada arquivo continua sujeita à conta, ao backend Windows e às ACLs existentes.

O SDK trata alvo e caminhos como entrada do fluxo de análise. Validar um caminho relativo, definir o diretório de trabalho, preparar um inventário ou adotar uma política no OpenKoda **não estabelece isolamento de leitura por repositório** para os subprocessos das ferramentas.

O executor prefere o backend Windows `prefer_mxc` quando disponível e mantém o fallback suportado. Usa `approval_policy=never` para negar solicitações de permissões adicionais. Isso não revoga a leitura já concedida pelo perfil do SDK. Não foi alterada a implementação do SDK distribuído para fingir confinamento por caminho.

O formulário e o início de uma análise já criada exibem: “Os comandos da análise podem ler outros arquivos acessíveis à sua conta Windows. Use apenas código em que você confia nesta versão local.” O download do GitHub não executa o projeto durante a obtenção; a análise posterior envolve ferramentas do executor e deve ser avaliada com esse limite em mente.

Uma garantia de exposição apenas do código escolhido exigiria um executor descartável ou hospedado com somente esse material disponível. Essa arquitetura ainda não foi implementada. A proteção DPAPI da chave, a projeção dos resultados e o backend MXC não comprovam essa garantia por si só.

## Provedores e ensaios reais

O preflight público offline do SDK recusou `openai/gpt-6-luna` no OpenRouter com teto de US$ 5: `pricing_unavailable`, **zero tentativas de rede**, sem credenciais e sem início de scan. O código de admissão e o formulário passaram a aceitar somente OpenAI para novas análises de código. Não há promessa de que todos os modelos do catálogo OpenAI terão preço reconhecido pelo SDK.

O ensaio nativo encontrou a recusa OpenAI por limite/saldo, identificada como HTTP 429. A mensagem foi admitida uma única vez e o compositor foi limpo; a recusa ficou registrada. Duas conversas reais seguintes com OpenRouter, usando `openai/gpt-4.1-nano`, terminaram com uma solicitação e uma resposta cada. A reabertura preservou os três contextos adicionais, as opções avançadas, os turnos registrados e as duas respostas. A seleção posterior de OpenAI persistiu sem reenviar a tentativa recusada. As credenciais de teste foram retiradas.

Essas respostas verificam o chat e a troca de provedor. **Não comprovam uma análise completa do código na 0.2.1.** O recibo da continuação registra ausência de novo scan. A recusa da OpenAI mantém essa análise real completa pendente; não foi contornada removendo o limite, trocando o provedor do scanner ou repetindo chamadas pagas indefinidamente.

A prova real da 0.2.0 continua [preservada](RETOMADA-0.2.0.md): 39 arquivos integralmente revisados entre 252 estimados, zero achados reportados e custo estimado de US$ 0,270424255. Ela tem cobertura parcial, antecede estas alterações e não se transfere para a 0.2.1.

## Revisão manual e tentativa de MCP Deep

O inventário preparado para a revisão ampliada registrou 574 arquivos, dos quais 389 classificados como código ou configuração, em um snapshot dedicado sem credenciais, perfis, recibos locais ou dependências instaladas. Isso é inventário, não cobertura obtida pelo scanner.

O MCP Deep ficou bloqueado no início da execução por uma falha do worker Windows, antes da leitura: **zero arquivos revisados pelo MCP**. Seus artefatos canônicos de bloqueio foram preservados; nenhum relatório completo ou cobertura fictícia foi criado para substituir o resultado.

A revisão local de engenharia percorreu o código próprio inventariado, incluindo frontend, núcleo nativo, testes, runtime, scripts, configuração e caminhos históricos. Os registros por arquivo distinguem leitura integral, hashes, exceções e revisão recebida de outro especialista. Módulos históricos lidos não foram certificados como percursos operacionais da edição atual. Binários e dependências de terceiros não são declarados integralmente auditados por esse inventário; a investigação específica do SDK tem escopo e fontes próprios.

Os achados corrigidos e as provas negativas ficam separados das limitações conhecidas do executor. O fechamento de cobertura registra 390 arquivos de código/configuração: 356 de código e testes lidos, 34 de dados/configuração validados e sete ativos adicionais. Inclui o novo teste do diagnóstico e os diffs finais, com hashes vinculados ao commit. A cobertura consta no [inventário manual sanitizado](security/0.2.1/manual-coverage.json). Revisão estática, testes automatizados, respostas reais de chat e análise automatizada completa continuam sendo verificações distintas.

## Checkpoints de validação

Os resultados abaixo têm recibos próprios. O executável nativo final foi reconstruído com os cinco arquivos do runtime conferidos contra as fontes. O instalador entregue é o mesmo artefato do CI que foi instalado no Windows 11 Arm; builds separados da mesma fonte também passaram em Server 2022 e Server 2025.

| Verificação | Evidência já obtida | Limite |
| --- | --- | --- |
| Build e unidades da interface | 79/79 unidades; compilação aprovada | Não chama APIs nem comprova núcleo nativo |
| Interface Edge | 101/101 casos | Transporte nativo simulado e prévias identificadas |
| Runtime integrado offline | 20/20 testes | Inclui SDK instalado e preflight offline, sem inferência paga |
| Ajuste final de provedores e aviso de leitura | 4/4 focais; 22/22 na suíte da análise; build aprovado | Confirma bloqueio antes de importação/criação e preserva Router no chat |
| Rust da candidata final | 638 aprovados, zero falhas, três opt-ins ignorados | O download GitHub real passou separadamente; Kali e host Windows opt-in históricos não foram reexecutados |
| Chat nativo com provedores reais | Duas respostas OpenRouter, reabertura e recusa OpenAI preservadas | Não houve conclusão da análise completa de código |
| MCP Deep | Bloqueado; zero arquivos lidos | Não é análise concluída |
| Revisão manual | Registros por arquivo separados | Não equivale a um resultado do MCP nem a certificação |
| Executável nativo final | Aprovado: reabertura, duas conversas, 14 recusas IPC e diagnóstico | Perfil sintético, sem nova inferência paga |
| Instalador final e Windows | Três runners aprovados: Server 2022, Server 2025 e Windows 11 Arm | Instalação limpa; não é teste de upgrade de um perfil pessoal; WebView2 já estava presente no cliente |

Os logs locais que sustentam os checkpoints são `021-check-release.log`, `021-edge-release.log`, `021-rust-release.log` e `021-platform-sdk-order-green.log`, sob `.local/`. A regressão focal está em `.local/021/provider-ui-focal-green.log`, `provider-ui-security-regression.log` e `provider-ui-build.log`. O RED anterior foi preservado em `provider-ui-red.log`.

O [relatório de validação](security/0.2.1/VALIDACAO.md) e os [resultados finais](security/0.2.1/test-results.json) recebem os vínculos sanitizados dos ensaios nativos, do pacote e dos gates finais. O [recibo Windows](security/0.2.1/windows-validation.json) separa os builds e registra a instalação do binário entregue; a [conferência do pacote](security/0.2.1/package-verification.json) registra os hashes, os recursos incorporados e o diagnóstico offline. Esses resultados não concluem a análise automatizada completa de código, que continua pendente.

## Dados, recuperação e distribuição

Backups SQLite preservam os relatórios incorporados ao domínio. Credenciais, cópias de repositórios importados e a árvore `security-scans/` permanecem fora desse backup. As cópias e os artefatos do executor também ficam fora da exclusão lógica do projeto. Não há promessa de apagamento seguro ou recuperação desses arquivos a partir do backup SQLite.

O retorno à 0.1.18 depende de um backup anterior à abertura do perfil nas edições 0.2.x. Os formatos históricos permanecem para leitura e recuperação delimitadas; a presença desse código não reativa suas operações.

A preparação do instalador inclui runtime, inventário e avisos de licença. A versão local sem assinatura digital foi solicitada pelo usuário. Essa condição descreve a identificação do publicador do executável, não a existência de um plano comercial. Um pacote gerado não aprova publicação pública nem substitui ensaio de instalação, atualização e desinstalação com o executável correspondente.
