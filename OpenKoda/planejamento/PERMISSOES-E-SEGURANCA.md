# Permissões, confiança e segurança do OpenKoda

Estado: mapa atualizado após aprovação dos conceitos visuais, em 7 de novembro de 2026. O usuário confirmou o produto de cibersegurança, o uso de Kali/WSL 2 pela IA, o framework desktop **Tauri** e os conceitos do painel e das cinco telas adicionais. Tauri 2 e React/TypeScript continuam propostas de versão/frontend. Os controles e valores abaixo são requisitos de construção; não foram implementados ou demonstrados pelas imagens.

Este documento define a autoridade de execução. O fluxo de informação e o protocolo estão em [IA-MCP-E-DADOS.md](IA-MCP-E-DADOS.md). A operação dos ambientes pertence a [KALI-E-AMBIENTES.md](KALI-E-AMBIENTES.md). A especificação deve permitir revisão de código, avaliações web/API e de rede autorizada, investigação assistida e análise offline de evidências.

## O que o OpenKoda protege

Os ativos prioritários são os arquivos e credenciais do Windows; os dados de cada Projeto; a identidade e a configuração do Ambiente; a autorização sobre alvos; as evidências e o histórico; as chaves de inferência e o orçamento da IA. O alvo avaliado pode conter conteúdo malicioso e não controla a política do aplicativo.

A IA propõe ações e interpreta resultados. O Executor verifica a autoridade antes de iniciar cada Operação e a supervisão durante a Execução. A interface apresenta e recebe decisões; ela não é o único ponto de controle. Uma instrução no prompt, uma anotação MCP ou um botão desabilitado não substitui a verificação no Executor.

### O que a aprovação visual confirma

O [painel aprovado](conceitos/painel-openkoda-v2.png) e as [telas adicionais](conceitos/TELAS-ADICIONAIS.md) fixam direção visual e organização de interação: navegação enxuta, detalhes selecionados e conversa/execuções/evidências sob demanda. Indicadores como “Kali pronto”, “Conexão verificada”, “Validado” e “Armazenada com proteção local” são **dados de exemplo**. Não comprovam conexão, validação de Achado, criptografia, isolamento ou proteção do vault.

A ação “Revisar e iniciar” deve consultar novamente a revisão concreta de escopo, plano, perfil e configuração. Abrir um chat, expandir um terminal, selecionar um modelo ou salvar uma preferência não cria aprovação nem amplia o Perfil de execução. Em Permissões, a UI apresenta valor desejado, origem e valor efetivo; o estado aplicado só é confirmado depois de resposta do núcleo/Executor.

### Tauri e a superfície de autoridade da interface

Na documentação Tauri 2, capabilities associam permissões a janelas/webviews; permissões podem habilitar/negar commands e delimitar scopes. A exposição efetiva exige configuração explícita: a documentação alerta que app commands registrados por `invoke_handler` ficam acessíveis por padrão a todas as janelas/webviews, salvo a configuração apropriada desses comandos. [Capabilities](https://v2.tauri.app/security/capabilities/), [Permissions](https://v2.tauri.app/security/permissions/).

Contrato proposto para o OpenKoda: frontend WebView de apresentação, sem iniciar processos, sem shell Windows/Linux genérico e sem recuperar valores de segredos do vault. Seus commands IPC são ações de domínio estreitas, com allowlist explícita, identidade de janela/peer e validação de payload. Conteúdo do alvo e previews não recebem a mesma autoridade da UI principal. Permissões de plugins e app commands serão inventariadas separadamente, sem herdar um conjunto amplo por conveniência.

Essa camada limita a exposição do núcleo, mas não substitui a autorização de Projeto/Avaliação, o grant de início ou o enforcer do Executor. O núcleo precisa verificar cada chamada, mesmo se ela veio de uma janela habilitada no Tauri. Código nativo/Rust incorreto e checks de escopo ausentes continuam responsabilidade da aplicação, não garantias obtidas pela escolha do framework. Antes de construir, a versão escolhida deverá demonstrar essas fronteiras em testes de IPC e de execução.

O produto pretende reduzir erros de escopo, vazamento acidental, execução indevida e mistura de projetos. Não promete proteger todos os segredos contra um administrador do Windows, um processo hostil já executando como o mesmo usuário Windows, o kernel comprometido ou um provedor de inferência comprometido. Essa limitação deve aparecer na documentação e na escolha do Ambiente, sem transformar o próprio aplicativo em serviço administrativo permanente.

## Fronteiras de confiança

| Fronteira | Autoridade permitida | Controle proposto |
|---|---|---|
| Conteúdo do alvo → leitura/parsing | Fornecer dados, nunca instruções de sistema | Proveniência, limites, classificação de conteúdo não confiável e parsers fora do processo de autoridade |
| IA → Orquestrador | Propor Plano de ação e chamadas | Schemas fechados, quotas, contexto do Projeto e nenhum grant criado pelo modelo |
| Interface → broker Windows | Pedir operações do produto | IPC com identidade de peer, handlers específicos e validação equivalente à API |
| Broker Windows → Executor Linux | Delegar uma Operação delimitada | Identidade da instalação/ambiente, grant curto, vínculo ao plano e verificação no destino |
| Executor → processo Kali | Iniciar ferramentas e supervisionar filhos | Usuário comum, ambiente mínimo, cwd controlado, limites e backend compatível com o Perfil de execução |
| Processo Kali → rede/arquivos | Somente os recursos concedidos | Enforcer efetivo quando disponível; restrição semântica do adaptador rotulada como tal |
| Serviço de contexto → provedor da IA | Enviar apenas o pacote de contexto permitido | Política de egress de dados, referências de evidência, limites e trilha de envio |
| Manutenção → Linux root/Windows elevado | Alterar componentes aprovados | Ações estreitas de ciclo de vida; identidade e plano específicos; canal separado do terminal da IA |
| Exportação → arquivo/integração externa | Publicar o pacote escolhido | Destino concreto, seleção/redação e confirmação quando publicar/enviar |

## WSL 2: o limite que não pode ser prometido

A documentação Microsoft descreve WSL como integração com a conta Windows: uma distribuição não é fronteira forte contra o host nem contra outras distros do mesmo usuário. Root Linux controla a distro, mas não equivale a administrador Windows. Alterar automount/interop não torna a distro uma sandbox. Para código que precise ser isolado do host, a Microsoft aponta uma VM gerida separadamente. [Modelo de segurança WSL](https://wsl.dev/technical-documentation/security/).

Por isso, o Ambiente Kali/WSL do produto será apresentado como **ambiente integrado gerido**, com grau de contenção explicitamente informado. Distro dedicada, usuário sem root, mounts mínimos e namespaces reduzem exposição e interferência; não autorizam o rótulo “isolado do Windows”. Um container dentro de WSL também não muda sozinho essa garantia. O caminho de VM separada é uma extensão de contenção, e não substituição silenciosa do requisito Kali/WSL confirmado pelo usuário.

O modo WSL gerido é destinado a ferramentas instaladas e verificadas e a trabalhos que aceitem essa integração. Executar malware desconhecido, scripts hostis ou uma tarefa cujo requisito seja “não pode alcançar a conta Windows” exige um backend de isolamento adequado, com acesso ao host e à rede realmente limitado. Se esse backend não estiver disponível, a tarefa fica bloqueada por incompatibilidade de Ambiente; o aplicativo não troca a garantia por um aviso genérico.

## Perfis propostos

Os perfis não são uma escala de severidade de Achado. Eles descrevem os efeitos e recursos disponíveis a uma Operação. A seleção considera tarefa, ferramenta, alvos, credenciais e Ambiente real.

| Perfil de execução | Recursos propostos | Autoridade e limite |
|---|---|---|
| Código e evidências offline | Leitura dos inputs importados; escrita só em diretório da Avaliação; sem egress da ferramenta | Política prévia pode autorizar lotes determinísticos; envio para IA segue política de dados separada |
| Ativo limitado | Adaptadores aprovados para web/API/rede, alvos e orçamento de tráfego concretos | Só inicia se o caminho da ferramenta e os controles de destino forem compatíveis; nenhuma descoberta fora do escopo vira autorização |
| Avançado Kali | Ferramentas sem adaptador, shell, scripts de investigação e PTY | Plano explícito, usuário comum e grau de integração WSL visível; não promete conter um programa hostil usando só parsing do comando |
| Capacidades privilegiadas | Capacidade Linux mínima para a função, por exemplo uma necessidade justificada de rede bruta | Concessão temporária por Operação e ambiente compatível; não equivale a root geral |
| Manutenção do Ambiente | Instalar/atualizar pacote e preparar/restaurar ambiente | Plano de manutenção separado; não recebe alvo de pentest como script nem token do terminal |
| Isolamento forte | Backend separado com acesso ao host, egress e persistência controlados | Somente anunciado após teste de fronteiras; seu ciclo de vida não pode depender de uma distro WSL comum ser uma sandbox |

Usar “todo o Kali” significa disponibilizar o catálogo compatível e uma rota avançada para ferramentas ainda não adaptadas. Não significa que toda Operação tenha root, rede irrestrita, arquivos Windows ou autorização sobre qualquer alvo. Recursos de hardware, rádio, kernel e GUI dependem da compatibilidade real do Ambiente.

### Shell avançado e ferramentas de rede bruta

Uma ferramenta tipada pode validar flags e destinos antes de iniciar um binário conhecido. Isso reduz erros operacionais, mas não contém um binário malicioso. Um script arbitrário pode abrir sockets ou subprocessos que um validador de argumentos não vê.

O Executor deve distinguir: **escopo validado pelo adaptador**; **egress imposto por backend verificado**; e **sem isolamento técnico suficiente para essa garantia**. Um proxy HTTP só controla tráfego que efetivamente passa por ele; não é filtro universal de TCP, UDP, DNS, IPv6 ou pacotes brutos. Perfis que prometam egress estrito exigem que todo caminho de rede pertinente seja imposto e testado. Se a ferramenta não suportar esse caminho, o produto oferece Ambiente compatível, reduz a tarefa ou a deixa pendente, sem apresentar uma allowlist de UI como garantia de kernel.

No modo avançado integrado, o usuário vê e autoriza a classe de efeitos e o limite de contenção. A aplicação não libera automaticamente execução de código hostil por essa escolha. A política avançada deve continuar limitada ao Projeto, ao prazo, à área de dados e ao orçamento; permissões técnicas que não possam ser impostas ficam explicitadas como confiança no processo do mesmo usuário.

## O objeto de autorização de escopo

Cada Avaliação vincula uma revisão imutável do escopo. O objeto contém:

| Campo | Regra proposta |
|---|---|
| Identidade | Instalação, Projeto, Avaliação, autor humano e revisão do escopo |
| Finalidade | Modalidade da avaliação, objetivo e relação declarada do usuário com os alvos |
| Alvos | Referências normalizadas a repositório/snapshot, arquivo, hostname, URL, IP ou CIDR; inclusões e exclusões separadas |
| Rede | Protocolos, portas, métodos/classes de ação, profundidade, redirects, DNS/resolved endpoints e destinos auxiliares |
| Tempo | Início, expiração e janela permitida; fuso para apresentação e relógio UTC para registro |
| Intensidade | Concorrência, taxa, volume máximo, tentativa de autenticação e limites por alvo |
| Dados | Inputs permitidos, áreas de escrita, refs de credenciais e política de envio para IA |
| Efeitos | Leitura, escrita local, alteração de alvo, acesso privilegiado e operações persistentes |
| Ambiente | Identidade, ferramentas/versões relevantes e requisitos de contenção/capacidades |
| Exclusões | Hosts, redes, arquivos, endpoints ou classes de ação explicitamente negados |

Declarar propriedade/autorização do alvo registra o contexto de uso; não é verificação externa de titularidade. A semântica precisa ser concreta: `example.test` não inclui automaticamente `*.example.test`, outras portas, IPs de uma CDN, redirecionamentos nem serviços ligados por hyperlinks.

### Resolução de alvos

- IPv4 e IPv6 são normalizados por parser apropriado; formatos ambíguos são recusados. Endereços IPv4 mapeados em IPv6 passam pela mesma política. Zone IDs e CIDRs têm suporte explícito ou são recusados.
- Um hostname corresponde ao nome exato normalizado; wildcard exige entrada própria. IP/CIDR permitido não concede autoridade sobre todos os domínios hospedados nesse destino.
- Uma URL vincula esquema, hostname, porta, path/base path e classes de método. Segmentos percent-encoded, normalização de path e diferenças entre autoridade URL e header Host precisam de política definida.
- Cada redirect é uma nova decisão de destino. Sair da base path, trocar de origem, reduzir TLS, alcançar endereço privado ou enviar credenciais a outra autoridade não é herdado por padrão.
- DNS é resolvido no ponto de despacho e revalidado na conexão. O serviço de conexão deve usar os endpoints autorizados e preservar a autoridade TLS/HTTP adequada. Rebinding e novas respostas não podem mudar silenciosamente o destino do grant.
- IPs compartilhados/CDNs requerem escopo por autoridade da aplicação e, quando possível, imposição nessa camada. “Liberar o IP inteiro” não é uma equivalência segura de “avaliar esse site”.
- DNS, atualização de ferramentas, proxy, inferência e telemetria são destinos auxiliares distintos. Não devem ser misturados à allowlist de alvos avaliados.
- RFC1918, loopback, link-local, endpoints do host, metadata de nuvem e redes privadas precisam de inclusão deliberada quando forem alvos legítimos. Descoberta não libera automaticamente tráfego para eles.

## Da proposta ao processo

1. A IA ou o usuário cria um Plano de ação com versão, objetivo, Operações e efeitos esperados.
2. O Orquestrador compila a Operação para uma representação concreta: ferramenta/adaptador/versão, argv ou programa de shell, cwd, inputs, destinos, credenciais por referência, limites, capacidades e Ambiente.
3. O Executor normaliza novamente, verifica revisão do escopo, exclusões, política de automação aplicável, validade temporal e capacidade do backend.
4. Uma política prévia suficiente permite seguir sem diálogo adicional. Caso contrário, o produto pede Aprovação humana para os efeitos exatos. A IA não pode registrar a própria aprovação.
5. A decisão gera um grant curto e de uso delimitado, ligado ao digest canônico da Operação, plano, escopo e Ambiente. Esse grant não aparece em argumentos que o modelo possa inventar.
6. Antes de iniciar, o Executor revalida a política vigente, a revisão/revogação e os recursos efetivos. Mudanças invalidam a decisão antiga.
7. O processo e seus filhos são supervisionados, com limites, logs, heartbeat e registro de efeitos conhecidos. A saída entra na cadeia de Evidência.

O digest de aprovação deve cobrir também scripts/inputs executáveis por conteúdo, ambiente relevante, mount/área de escrita, credenciais por identidade/versão, portas/protocolos, perfil e limites. Não basta hashear a string exibida. A ferramenta não pode substituir executável, script ou artefato depois da aprovação sem nova verificação. Os mecanismos concretos de handle/cópia imutável/identity binding são decisões de implementação a validar.

### Política de automação e experiência de aprovação

Uma política de automação é criada por ação humana e delimita categorias de Operação, projetos/alvos, efeitos, limites, duração e condições. Por exemplo, uma avaliação pode delegar a execução de um conjunto conhecido de adaptadores de leitura dentro do escopo. Isso evita interrupção por comando e mantém o Executor responsável pela decisão concreta.

Uma nova classe de efeito, expansão de alvo, mudança de credencial/capacidade, atualização de script, troca de Ambiente ou aumento de orçamento provoca reavaliação. A política não pode ser ampliada pela IA ao descrever uma ação como segura. Um revisor de IA adicional pode aconselhar sobre risco; não substitui a aprovação humana ou uma política humana já suficiente.

## Revogação, cancelamento e efeitos desconhecidos

| Evento | Comportamento exigido |
|---|---|
| Grant expirado antes do spawn | Não iniciar; produzir resultado de autoridade expirada |
| Revogação antes do spawn | Verificação sincronizada com a transição para execução impede novo início |
| Revogação durante execução | Cortar novas operações/credenciais/egress quando aplicável; pedir término e escalar cleanup do grupo próprio |
| Aprovação recebida após cancelamento | Ignorar por estado/revisão, sem reabrir a execução |
| UI fecha ou IPC cai | Marcar conexão perdida; manter estado durável; aplicar a política de continuidade previamente selecionada |
| Executor perde heartbeat | Marcar estado desconhecido; não afirmar que o alvo não foi alterado nem repetir operação mutante |
| Timeout | Tentar término; registrar confirmação ou falha de cleanup; evidência parcial permanece identificada |
| Retry | Nova tentativa com ID próprio; revalidar escopo/política/recursos e idempotência; aumento de autoridade exige nova decisão |
| Atualização do ambiente durante avaliação | Bloquear/drain conforme política e invalidar plano incompatível; nunca trocar a ferramenta sob o mesmo digest |

Uma operação de rede pode produzir efeito antes de o cancelamento chegar. “Cancelada” descreve a decisão de controle; o produto deve registrar separadamente término confirmado, recursos restantes e efeitos desconhecidos. Mutação remota sem confirmação não recebe retry automático. Contenção e revogação não desfazem pacotes já enviados nem credenciais já entregues ao processo.

## Identidade local, root e bridge de manutenção

O perfil inicial é um usuário local Windows. A identidade da instalação e os IDs de Ambiente não são uma conta de provedor da IA. Um aplicativo pode usar vault/credenciais protegidas pelo SO e sessão local bloqueável, mas não deve prometer isolamento criptográfico universal de processos hostis da mesma conta.

O broker Windows funciona sem elevação contínua. Seus handlers são ações de produto, como verificar pré-requisitos, provisionar/atualizar um Ambiente e importar/exportar um pacote escolhido. Não existe `host.exec` genérico acessível ao MCP Kali. O Executor Linux não recebe um token administrativo Windows ou a capacidade irrestrita de chamar os handlers de manutenção.

A manutenção privilegiada valida a identidade da distro gerida, pacote/operação permitida, revisão do plano, locks e estado. O helper não aceita um script arbitrário, nome de distro pessoal ou path Windows escolhido pela IA como equivalente a uma ação permitida. Instalação APT executa código de pacotes como root Linux; portanto, assinatura/origem, resolução de dependências, lista de mudanças e registro de resultado fazem parte dessa fronteira. A IA pode propor uma instalação; a autorização vem do usuário/política de manutenção definida, separadamente do grant de investigação.

## Dados, credenciais e exportação

Chaves de inferência ficam no lado host/vault proposto e são usadas por um serviço de inferência específico. Não são herdadas por todo processo WSL. Credenciais de alvo são referenciadas por ID, com Projeto, autoridade, finalidade e validade; são materializadas somente para a Operação que as exige. O modelo recebe metadados mínimos, nunca o valor por padrão. Segredos em argv são evitados; um processo que necessita da credencial poderá vê-la em algum canal, e esse limite deve ser assumido.

O histórico usa referências/redação para impedir cópia de segredo em prompt, log, shell history, crash report e exportação. Falha do redator não deve bloquear a preservação da evidência bruta permitida, mas deve bloquear o envio/exportação automática do material suspeito. Revogar uma credencial impede novo uso; não apaga o conhecimento de um processo ou provedor que já a recebeu.

Exportar um relatório local não autoriza enviá-lo a GitHub, email ou outro serviço. Publicação é uma Operação com destino e conteúdo concretos. Backup, sincronização, crash report e telemetria têm políticas de dados próprias, opt-in quando levarem conteúdo sensível.

## Ameaças e requisitos verificáveis

Não são achados de vulnerabilidade do produto: o produto ainda não existe. São cenários de ameaça e requisitos de aceitação para a construção. P0 significa necessário antes de executar avaliações reais; P1 significa necessário antes de anunciar a capacidade correspondente.

| ID | Ameaça e consequência | Requisito proposto | Aceitação concreta | Prioridade |
|---|---|---|---|---|
| SEC-01 | Texto de alvo convence a IA a ampliar autoridade | Fonte não confiável não cria política/grant | Conteúdo de teste pedindo novo alvo não muda escopo nem dispara operação sem decisão válida | P0 |
| SEC-02 | UI ou cliente forja Projeto/Avaliação | Executor resolve identidade e vínculos no backend | Chamada com IDs cruzados é recusada antes do spawn e não revela dados do outro Projeto | P0 |
| SEC-03 | Aprovação genérica reutilizada para outra ação | Grant ligado à representação canônica e à revisão | Alterar argv, script, destino, cwd, credencial, perfil ou Ambiente invalida o grant | P0 |
| SEC-04 | Corrida entre aprovação, revogação e spawn | Transição de estado e revisão verificadas atomicamente | Teste concorrente de revogação não deixa operação ainda não iniciada começar com revisão antiga | P0 |
| SEC-05 | DNS/redirect envia tráfego fora do alvo | Destino verificado na conexão e a cada redirecionamento | Mudança DNS para destino negado e redirect cross-origin são bloqueados, sem encaminhar credencial | P0 |
| SEC-06 | IPv6, raw sockets ou subprocesso contorna filtro superficial | Garantia de egress ligada ao backend real | Perfil estrito passa matriz TCP/UDP/DNS/IPv4/IPv6/processos filhos ou a ferramenta fica indisponível nesse perfil | P0 |
| SEC-07 | Distro WSL anunciada como sandbox forte | Classe de Ambiente informa garantia real | UI, API e relatório mostram grau de contenção; tarefa que exige host-isolation não inicia em WSL comum | P0 |
| SEC-08 | Processo Kali acessa secrets Windows por bridge | Sem exec host genérico; handlers estreitos e autenticados | MCP de execução não possui rota para PowerShell/exec Windows nem para leitura arbitrária do vault | P0 |
| SEC-09 | Privilégio de pacote/root usado por terminal de avaliação | Grants e canais de manutenção separados | Grant de instalação não executa comando de investigação e grant de investigação não aceita alteração de pacote | P0 |
| SEC-10 | Filho escapa de cleanup ou continua sem controle | Grupo próprio, rastreio e confirmação de término | Cancelar árvore sintética encerra filhos suportados; falha fica `cleanup_incomplete`, não sucesso | P0 |
| SEC-11 | Processo ou parser exaure máquina/disco | Quotas, watchdog e limites independentes | Ferramenta que excede memória/tempo/disco/saída atinge estado limitado e mantém metadados/evidência parcial legível | P0 |
| SEC-12 | Prompt/log/export vaza segredo | Vault, materialização mínima e envio por política | Segredo sentinela não aparece em contexto padrão, log comum ou export padrão; bruto sensível exige acesso específico | P0 |
| SEC-13 | Repositório/path/link atravessa a área permitida | Resolver identidade e containment, rejeitar symlink/reparse inadequado | Inputs com escape via link/path e substituição após aprovação não alteram arquivo externo | P0 |
| SEC-14 | Retry duplica alteração remota | Classificação de idempotência e estado desconhecido | Perda de ACK após operação mutante não produz segunda execução automática | P0 |
| SEC-15 | Histórico alterado parece achado confirmado | Evidência com hash/proveniência e revisão de estado | Alterar artefato quebra integridade; candidato sem validação não vira Achado confirmado por parser | P0 |
| SEC-16 | Processo local/website chama API de autoridade | Peer autenticado, sessão curta e nenhum listener público padrão | Cliente não autorizado não enumera ferramentas sensíveis nem executa ação; rota HTTP opcional testa Origin/Host/auth | P0 |
| SEC-17 | Pacote/adaptador troca sem revisão | Catálogo fixado por versão/identidade e atualização separada | Mudança de binário/adapter invalida plano e registra proveniência; pacote de origem não permitida é recusado | P0 |
| SEC-18 | Export/relatório executa HTML ou fórmulas de alvo | Renderizar dados com escape e export seguro | Strings ativas em evidências são apresentadas como texto e não executam no painel/arquivo exportado | P0 |
| SEC-19 | Exclusão promete apagamento que não ocorreu | Política de retenção por classe e confirmação de alcance | Expurgo distingue dados vivos, backups, cópias exportadas e dados já enviados ao provedor | P1 |
| SEC-20 | Serviço comprometido ganha todos os projetos | Grants/session/toolset e armazenamento ligados ao Projeto | Sessão de um Projeto não lê artefato/credencial de outro nem amplia seu catálogo operacional | P0 |
| SEC-21 | Troca de provider/ferramenta altera destino de dados | Política de compartilhamento versionada e ligada ao trabalho | Trocar endpoint/provider reavalia o consentimento de envio; nenhum fallback silencioso recebe dados | P0 |
| SEC-22 | Validação estrutural é confundida com validação de achado | Separar schema, evidência, validação e revisão humana | Achado mostra o que foi observado, inferido, não reproduzido e quem decidiu o estado | P0 |
| SEC-23 | WebView ou conteúdo de resultado alcança app command amplo | Restringir explicitamente commands Tauri, janelas e handlers de domínio | Preview sem capability não chama handler sensível; chamada da UI principal sem autorização de domínio também é recusada; nenhum handler retorna segredo do vault ou executa shell host genérico | P0 |

Os ensaios de fronteira devem usar alvos e dados sintéticos controlados. O resultado esperado é uma evidência de bloqueio/permitido e estado durável, não apenas uma captura de tela de configuração.

## Defaults e pendências para decisão

Proposta de defaults: rede de ferramentas fechada até selecionar escopo; usuário Kali comum; root ausente do terminal padrão; leitura de inputs importados e escrita em área da Avaliação; nenhuma credencial de inferência no Kali; política de compartilhamento com IA explícita por Projeto; grants de execução com TTL curto; nenhuma continuidade de execução mutante após perda de supervisão sem política prévia. Os valores numéricos, inclusive TTL, ficam centralizados em [CONFIGURACOES.md](CONFIGURACOES.md).

Valores como taxa, timeout, memória e retenção dependem da classe de ferramenta. O usuário configura limites por Projeto dentro das capacidades verificadas do Ambiente; não existe um número universal seguro para toda rede ou ferramenta. A UI exibe limites efetivos e sua origem. A implementação deve recusar capacidade não suportada, em vez de aceitar um valor que o backend ignora.

Antes de construir, é preciso escolher quais garantias serão entregues no primeiro backend WSL e quais tarefas exigirão isolamento separado; o mecanismo de vault/cadastro de credenciais; as restrições efetivas dos app commands/IPC; o enforcer de egress por classe de ferramenta; a política de extensões; a autoridade da manutenção; e os valores iniciais de budgets/retention. O framework Tauri está confirmado. Versão principal/frontend, provedor, modelo e produto de virtualização continuam propostas ou pendências, sem decisão adicional inferida das imagens.
