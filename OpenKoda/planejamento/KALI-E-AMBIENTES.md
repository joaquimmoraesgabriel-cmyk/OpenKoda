# Kali, WSL 2 e ambientes do OpenKoda

Mapa de produto e operação, de 7 de novembro de 2026. **Somente especificação: nenhuma distribuição, ferramenta, serviço ou aplicação foi instalado ou executado nesta etapa.** O requisito confirmado pelo usuário é usar Kali Linux no WSL 2. As escolhas de organização abaixo são propostas para construir e validar depois.

O usuário também confirmou Tauri para o desktop e aprovou a direção visual de [Kali e ferramentas](conceitos/02-kali-e-ferramentas-v1.png), registrada em [TELAS-ADICIONAIS](conceitos/TELAS-ADICIONAIS.md) e [AJUSTES-DE-INTERFACE](AJUSTES-DE-INTERFACE.md). A imagem usa estados e dados fictícios: não demonstra um ambiente instalado, conectado ou saudável neste computador.

## 1. Papel do ambiente

O OpenKoda deve transformar uma Avaliação em operações rastreáveis sobre alvos autorizados. O Kali fornece ferramentas Linux; o OpenKoda fornece planejamento, política, supervisão, evidências e interface. Uma ferramenta instalada pode estar indisponível para determinada Avaliação por incompatibilidade, falta de recurso ou permissão.

**Proposta base:** uma distribuição Kali dedicada, identificada como gerida pelo OpenKoda; um usuário Linux comum para atividades usuais; um executor Linux supervisionado; e um broker Windows que inicia, autentica e acompanha esse executor. A distribuição pessoal existente do usuário continua separada. Tauri está confirmado para a camada desktop; sua versão, configuração e bibliotecas complementares exigem validação. A linguagem do executor Kali continua uma proposta a decidir, não uma consequência dessa escolha.

O WSL integra Linux e Windows, mas não oferece uma fronteira de segurança entre a distribuição, o host e outras distribuições do mesmo usuário. Root Linux controla a distribuição e não equivale a administrador Windows. Desabilitar automount ou interop reduz caminhos de integração, sem transformar o WSL em sandbox. Esses são fatos do fornecedor que limitam as promessas do produto. [Modelo de segurança WSL](https://wsl.dev/technical-documentation/security/).

Consequência de projeto: o painel deve mostrar **qual ambiente executa, quais controles foram verificados e quais garantias não estão disponíveis**. Não pode apresentar um ícone verde de “isolado” apenas porque uma pasta ou distro separada foi criada.

## 2. Componentes e responsabilidades propostas

| Componente | Responsabilidade | Autoridade que não recebe implicitamente |
|---|---|---|
| Painel Windows | Mostrar diagnóstico, plano, aprovações, estado, saída e recuperação | Executar qualquer texto recebido da IA |
| Broker Windows | Resolver identidade da instalação/ambiente, iniciar processo correto, vincular sessão, supervisionar ciclo de vida | Acesso administrativo permanente ou execução genérica fora do WSL |
| Gerenciador de ambientes | Inventário, provisionamento, manutenção, backup e remoção controlada de ambientes geridos | Alterar ou apagar uma distro pessoal por coincidência de nome |
| Executor Linux | Validar operação concreta, iniciar ferramenta, impor controles disponíveis, capturar resultado | Aceitar mudança de escopo enviada pela própria ferramenta |
| Serviço de manutenção | Aplicar mudanças aprovadas de pacotes/configuração com elevação temporária | Virar shell root geral da IA ou reusar uma aprovação para outra mudança |
| Armazenamento de evidências | Preservar saídas, identidade do ambiente e vínculo com a Execução | Alterar artefatos brutos para fazer o resumo parecer consistente |

MCP é o contrato de capacidades, não a barreira de contenção. A conexão local deve ser vinculada a uma instância do executor, identidade do ambiente e sessão; não basta aceitar um servidor com nome conhecido. A especificação das ferramentas e seu ciclo aparece em [FERRAMENTAS-E-OPERACAO.md](FERRAMENTAS-E-OPERACAO.md).

### 2.1 Relação com a tela aprovada

“Kali e ferramentas” é um destino secundário da navegação, acessível pelo ícone de ferramentas. O resumo compacto mostra o ambiente efetivamente selecionado, sua disponibilidade e acesso a **Ver diagnóstico**. Os rótulos exemplificados na imagem — distro, executor, sessão de usuário e pacotes — devem vir do inventário/diagnóstico, com data e motivo de indisponibilidade; não são valores fixos da interface.

| Área visível | Dados e ações previstos | Condição operacional |
|---|---|---|
| Resumo do ambiente | Identidade da distro, versão/modo WSL, usuário, conexão do executor e situação dos pacotes | “Conectado” não significa que toda ferramenta ou todo perfil esteja pronto |
| Ferramentas | Catálogo pesquisável, grupos/metapacotes, instalados, seleção e preparação de instalação | Seleção não instala; cálculo, revisão e aplicação são etapas distintas |
| Ambiente | Inventário, controles efetivos, recursos, integração, origem, conectividade e diagnóstico | Preferências desejadas e valores efetivos aparecem separadamente; limitações do WSL permanecem visíveis |
| Manutenção | Planos, autorizações, fila exclusiva, progresso, histórico, backups e recuperação | Não adquirir lease de manutenção com processo não reconciliado nem alterar pacotes de avaliação ativa |
| Ver diagnóstico | Motivos do estado, verificações realizadas, versões e próximos passos | Um teste controlado necessário deve ter finalidade/efeitos descritos; abrir o diagnóstico não autoriza reparar ou reinstalar |

O indicador global “Kali pronto” e o rodapé “Ambiente saudável” são exemplos do estado positivo. Devem mudar com a informação real e permitir abrir o contexto. O estado positivo informa prontidão para capacidades verificadas, sem prometer isolamento do host ou suporte a todo o catálogo.

| Estado apresentado | O que o usuário entende | Ação disponível proposta |
|---|---|---|
| Indisponível | WSL/Kali/executor ausente, incompatível ou não alcançável | Abrir diagnóstico e o fluxo de preparação pertinente; não iniciar manutenção implícita |
| Checando | Há verificação em andamento ou estado efetivo ainda não confirmado | Mostrar progresso/última observação; não reutilizar um selo antigo de prontidão |
| Pronto por perfil | Dependências e controles daquele perfil foram verificados | Permitir somente as capacidades compatíveis e manter detalhes consultáveis |
| Bloqueado por manutenção | Há mudança exclusiva ou necessidade de aguardar reconciliação | Consultar manutenção; enfileirar ou impedir novos trabalhos na distro afetada |
| Erro / recuperação | Provisionamento, manutenção ou verificação deixou falha/estado parcial | Consultar evidências e revisar recuperação como plano próprio; não apagar a distro para ocultar o erro |

“Pacotes atualizados” exige verificação efetiva e data de referência. Se a informação estiver ausente ou antiga, usar estado desconhecido/ação de verificar, sem afirmar atualização. Quantidades, tamanhos e progresso futuros são medidos ou explicitamente estimados; os exemplos visuais não são limites técnicos nem resultados de diagnóstico.

### 2.2 Impacto de Tauri neste fluxo

O Tauri separa conteúdo do WebView e funcionalidade do núcleo, com permissões/capabilities para delimitar a exposição ao frontend. Isso orienta o desenho de uma superfície estreita, mas não substitui autorização de domínio ou controles no executor Linux. [Arquitetura Tauri](https://v2.tauri.app/concept/architecture/), [capabilities](https://v2.tauri.app/security/capabilities/).

Proposta para o OpenKoda: a tela solicita ações de domínio como consultar ambiente, calcular um plano, apresentar uma decisão ou acompanhar manutenção. O núcleo/broker valida identidade, parâmetros, revisão e autoridade; não fornece à tela um comando genérico para montar argumentos de `wsl.exe`, APT ou shell. Conteúdo de logs, descrições de pacotes e ajuda não recebe a autoridade da janela principal só porque é exibido no mesmo produto.

Recarregar/fechar o WebView não pode significar sucesso, cancelamento ou reexecução de manutenção. A UI reabre pelo estado durável e pelos eventos do trabalho existente. Tauri também não instala WSL/Kali automaticamente, não determina a linguagem do executor e não transforma uma distro em sandbox; esses contratos continuam nos componentes operacionais deste mapa.

## 3. Inventário do Ambiente

Cada Ambiente deve ter registro próprio, inclusive quando estiver parado ou indisponível:

- Identificador estável do OpenKoda, nome WSL efetivo, propriedade **gerido**, **externo** ou **importado para análise** e caminho resolvido do armazenamento.
- Identidade do usuário Windows associado; arquitetura; versão/build do Windows; versão do WSL, kernel e modo WSL 1/2 detectado.
- Imagem de origem, canal, publicador, URL de aquisição, data, tamanho, digest e resultado da verificação de origem/assinatura disponível.
- Usuário Linux normal, versão do executor, versão do protocolo, capabilities anunciadas e testes de readiness efetivamente aprovados.
- Configuração de integração, mounts, rede, proxy, recursos e desvios aceitos; não registrar apenas o valor desejado sem a leitura do valor efetivo.
- Inventário de pacotes, versões dos binários realmente resolvidos, adapters/parsers compatíveis, bases de assinaturas/templates e data de atualização.
- Último diagnóstico, operação de manutenção, backup verificado, bloqueio de uso, lease de manutenção e operações em andamento.

Nome da distro, PID ou caminho aparente não é identidade suficiente para uma ação destrutiva. O broker deve conferir seu registro, caminhos finais e ownership antes de manutenção, restauração ou remoção.

## 4. Onboarding e instalação como fluxo retomável

O instalador futuro não deve fazer uma sequência cega de comandos. Cada etapa grava estado, resultado, evidência mínima e próximo passo; repetir a etapa não deve duplicar distro nem apagar a anterior.

| Etapa | Verificação e resultado visível | Falha/retomada prevista |
|---|---|---|
| Descobrir host | Windows/arquitetura, virtualização, WSL disponível e versão, espaço e distros existentes | Mostrar requisito ausente e ação concreta; não iniciar download incompatível |
| Escolher destino | Local de armazenamento, proposta de distro gerida e tamanho disponível | Se nome/caminho já existir, identificar e solicitar escolha de reutilizar explicitamente ou usar novo destino |
| Preparar WSL | Plano separado de instalação/atualização de componentes Windows quando necessários | Elevação ou política da organização pode bloquear; registrar recusa sem simular sucesso |
| Aguardar reboot | Mostrar por que reiniciar, checkpoint e botão de retomar | Reabrir o app revalida o host antes de continuar; nunca declarar etapa concluída só pela volta do Windows |
| Obter Kali | Selecionar canal oficial compatível, verificar proveniência e bytes adquiridos | Download parcial retomável quando suportado; digest/assinatura inválida impede importação |
| Criar distro | Registrar a distro gerida e confirmar que ela é WSL 2 | Importação interrompida vira estado de reparo ou resíduo identificado, não ambiente pronto |
| Configurar base | Usuário normal, executor, diretórios geridos e integração proposta | Provisionamento root é etapa de manutenção delimitada; o processo comum posterior volta a usuário normal |
| Instalar conjuntos | Resolver e aplicar os metapacotes escolhidos por um plano de pacotes | Locks, falta de disco e dependências vão para manutenção/recuperação |
| Diagnosticar | Handshake do executor, ferramenta de teste inofensiva, evidência, cancelamento e política efetiva | Separar “WSL funciona”, “Kali funciona” e “OpenKoda pode executar com este perfil” |
| Concluir | Resumo das capacidades disponíveis, limitações e próximo passo do Projeto | Permitir trabalhar apenas nas modalidades prontas; não liberar tudo por uma única verificação positiva |

A documentação Microsoft distingue instalação de WSL, seleção da distribuição e conversão de WSL 1/2, e o caminho padrão de instalação pode requerer administrador e reinicialização. O catálogo real do host e os métodos suportados devem ser consultados na futura implementação. [Instalação WSL](https://learn.microsoft.com/en-us/windows/wsl/install).

A Kali documenta Store, rootfs e formatos modernos de distribuição; alguns trechos são históricos. A seleção do OpenKoda deve considerar versão do host e disponibilidade atual, sem promover uma instrução prerelease antiga a requisito permanente. [Kali WSL](https://www.kali.org/docs/wsl/wsl-preparations/).

### Proveniência antes de importar

O plano de aquisição deve distinguir hash calculado localmente, hash publicado autenticado, identidade do publicador e assinatura realmente verificada. Um hash obtido junto de uma imagem por uma origem não autenticada não prova autoria. O mecanismo de importação WSL não deve ser tratado como verificador universal da imagem.

Quando o canal fornecer assinatura, a confiança deve estar ligada à chave/publicador esperado, não a uma chave encontrada dentro do próprio download. Se o formato/canal escolhido não oferecer uma cadeia verificável prevista no catálogo, o produto deve indicar o nível de verificação e impedir a classificação “origem verificada” até haver uma alternativa aprovada. A rotação de chave deve ser atualizável por uma cadeia de confiança documentada, sem botão automático de ignorar erro.

### Reutilizar uma distro existente

É alternativa avançada e explícita, não o padrão proposto. O usuário vê que configurações, pacotes e dados pessoais podem coexistir. O app faz diagnóstico somente leitura primeiro; distingue alterações necessárias e pede aprovação do plano concreto antes de instalar executor ou pacotes. O registro permanece **externo**, salvo migração de ownership expressa e verificável.

Não desinstalar, restaurar por cima, redefinir usuário padrão, converter versão, trocar repositórios ou alterar integração de uma distro externa como parte de “corrigir automaticamente”. O modo externo pode ter menos garantias e menos automações disponíveis; o diagnóstico deve dizer quais.

## 5. Configuração e efeito sobre outras distribuições

`wsl.conf` configura a distribuição; `.wslconfig` configura o WSL 2 de forma global. O OpenKoda deve preferir configurações por ambiente, ler o estado atual, preservar opções não geridas e mostrar diff de qualquer mudança. Limites globais não devem ser apresentados como limites exclusivos de uma avaliação. [Configuração WSL](https://learn.microsoft.com/en-us/windows/wsl/wsl-config).

**Proposta para a distro dedicada:** evitar montagem ampla de discos Windows e herança do PATH Windows; importar entradas de Projeto para diretórios geridos e exportar evidências por um canal delimitado. Interop só é habilitado quando uma capacidade específica justificar, com o impacto visível. Isso reduz exposição acidental e não oferece uma fronteira host/guest.

A manutenção comum deve parar somente a distro gerida quando necessário. O comando global de shutdown afeta todas as distribuições; unregister remove dados da distribuição. Por isso, ações globais ou destrutivas não podem ser atalhos de reparo em background. [Comandos WSL](https://learn.microsoft.com/en-us/windows/wsl/basic-commands).

Não alterar `.wslconfig`, distro padrão ou configurações globais sem um fluxo separado que mostre o efeito sobre outros usos de WSL e permita recusa. Na primeira entrega proposta, tais alterações ficam no diagnóstico orientado ao usuário, sem aplicação automática.

## 6. Classes de execução e contenção

Os nomes finais dos perfis são consolidados no mapa de permissões. Esta tabela define suas necessidades operacionais, sem criar uma segunda escala de severidade.

| Classe | O que pode fazer no ambiente proposto | Condição para disponibilidade |
|---|---|---|
| Offline/código | Ler cópia de entrada autorizada, gerar evidências e analisar sem executar o projeto | Perfil de arquivos/rede verificado e ferramentas/parser compatíveis |
| Ativo limitado | Operações de adapters sobre alvos autorizados, com orçamento e efeitos conhecidos | Validação de alvo no executor e imposição de rede/recursos adequada ao que a interface promete |
| Avançado/shell | Usar ferramentas não adaptadas ou roteiro explícito de shell | Plano e autoridade mais amplos visíveis, supervisão e controles efetivos; não herdar a garantia de um adapter tipado |
| Manutenção | Pacotes, executor, configuração e backup do ambiente | Ambiente drenado, lease exclusivo e elevação restrita ao plano aprovado |
| Contenção separada | Trabalho que exige executar conteúdo não confiável com separação do host | Backend de isolamento adequado realmente configurado e testado; não satisfazer esse requisito com outra pasta/distro WSL |

Um shell amplo consegue invocar outros programas, bibliotecas, redirecionamentos e destinos. Validar apenas seu texto não demonstra confinamento. Se o app não conseguir impor a restrição de destino/rede/arquivos exigida, a operação deve ficar indisponível nesse perfil. Um modo supervisionado com limitações explícitas é uma proposta distinta; não pode reutilizar o rótulo de garantia técnica do modo limitado.

Para tarefas que exijam separação contra o Windows, o mapa contempla um backend futuro de VM separadamente gerida, com integrações reduzidas e evidências exportadas por um canal próprio. Não é requisito da primeira entrega nem substituição da integração Kali/WSL pedida; é a alternativa para uma garantia que o WSL não fornece. Containers dentro do WSL não devem receber automaticamente essa mesma classificação. [Modelo de segurança WSL](https://wsl.dev/technical-documentation/security/).

### Privilégios Linux

O executor comum não inicia como root. Operações que precisem de capabilities Linux, dispositivos ou root têm requisitos declarados no adapter e revisão antes da execução. Instalar pacotes é manutenção, não uma autorização reutilizável para qualquer ação ativa.

Um helper elevado não aceita shell arbitrário por um parâmetro textual. Ele deve receber uma operação administrativa tipada e vinculada ao plano, conferir novamente argumentos/artefatos e expirar a autorização. Se a capacidade desejada exigir uma sessão root geral, ela pertence ao modo avançado explicitamente aprovado e deve ser avaliada com a contenção necessária; não ocorre por fallback silencioso.

## 7. Rede, montagens e credenciais

Há três planos independentes: inferência do modelo no host, download/manutenção de pacotes e tráfego ativo da ferramenta Linux. Aprovar um não libera os outros. A operação registra qual plano usa e qual política foi aplicada.

O diagnóstico deve identificar NAT/mirrored, proxy/VPN/DNS, alcance de loopback e controles de firewall disponíveis. A topologia varia; um IP privado ou loopback não prova que um serviço só seja alcançável pelo componente pretendido. [Rede WSL](https://learn.microsoft.com/en-us/windows/wsl/networking).

Requisitos propostos:

- Não expor o executor em interface de rede por conveniência. A rota local preferida é um processo explicitamente selecionado e canal supervisionado; uma rota de rede futura precisa autenticação, autorização e lifecycle próprios.
- Não montar automaticamente todo o diretório do usuário Windows. Cada importação/mount tem propósito, modo de acesso e lifetime; entradas de análise preferem cópias geridas.
- Resolver e validar alvos, redirects e dependências no momento de uso. Conteúdo obtido do alvo não amplia o escopo da Avaliação.
- Segredos do provedor de IA permanecem fora do ambiente de ferramentas por padrão. Credenciais do alvo são referências com escopo e TTL, entregues somente à operação pertinente, sem aparecer em argv, logs ou relatórios.
- Uma ferramenta pode acessar sockets, arquivos e serviços de maneiras não previstas por seu adapter. O cadastro de capabilities não substitui imposição e verificação no executor.

## 8. Recursos e concorrência

As cotas numéricas são propostas centrais do mapa e devem ser validadas depois; este documento não fixa números concorrentes. O executor deve medir e impor os limites suportados para CPU, memória, processos, armazenamento de saída, duração, tráfego e arquivos de trabalho.

A interface distingue limite global do WSL, limite do ambiente e limite da Execução. Se o host/kernel não oferecer o controle exigido, não exibir “limite aplicado”. Registrar capability ausente e a decisão operacional correspondente.

Proposta de coordenação: fila por instalação/ambiente, slots globais e por alvo, exclusão entre manutenção e avaliações. O scheduler não pode iniciar duas operações incompatíveis no mesmo destino apenas porque vieram de agentes diferentes. Cada aquisição de slot é durável; leases expiram e são reconciliados após crash.

## 9. Backup, restauração e desinstalação

Três coisas precisam de políticas diferentes: evidências da Avaliação, banco/configuração do OpenKoda e estado do Kali. Backup de uma delas não implica recuperação das demais. Uma cópia de VHD em uso não deve ser rotulada como snapshot consistente.

O WSL oferece export/import de distribuição e formatos específicos conforme versão. O fluxo proposto interrompe de forma controlada as operações da distro gerida, confirma quiescência, exporta para destino novo, calcula hash e testa a capacidade de leitura/importação em ambiente de verificação antes de declarar o backup utilizável. O registro guarda os limites de consistência e não promete capturar memória/processos vivos. [Comandos WSL](https://learn.microsoft.com/en-us/windows/wsl/basic-commands).

Restauração deve criar identidade de ambiente nova ou destino de recuperação, validar e só então permitir a troca de referência; não sobrescrever imediatamente a única cópia saudável. Evidências finalizadas continuam ligadas ao ambiente original e ao seu digest, mesmo após restaurar um backup.

Remoção do app e remoção do Kali são opções distintas. Antes de apagar um ambiente gerido, conferir identidade/ownership, mostrar dados afetados, exigir confirmação específica e oferecer exportação. Ambiente externo nunca é removido pelo desinstalador padrão. Uma falha parcial de remoção precisa deixar um resíduo identificável e instruções de recuperação.

## 10. Falhas e recuperação

| Evento | Estado e ação proposta | O que não pode acontecer |
|---|---|---|
| Host sem requisito/virtualização bloqueada | Ambiente não disponível; diagnóstico orientado | Tentar contornar política administrativa ou declarar modo ativo pronto |
| Reboot pendente | Checkpoint retomável, sem novas avaliações | Repetir registro/importação automaticamente |
| Executor incompatível | Bloquear novas operações e oferecer atualização compatível | Usar shell livre como substituto invisível de protocolo |
| Queda da UI | Execuções com identidade durável continuam ou são canceladas conforme política configurada | Duplicar a operação ao reabrir o painel |
| Queda do executor/WSL | Execuções ficam com resultado desconhecido até reconciliação; preservar saída parcial | Inferir sucesso pela ausência de processo |
| Disco cheio | Suspender admissão, marcar evidência parcial e acionar limpeza gerida | Descartar bruto silenciosamente ou apagar fonte do usuário |
| Pacotes inconsistentes | Ambiente em manutenção/reparo, ativo indisponível | Rodar avaliação com inventário anterior apresentado como atual |
| Suspeita de comprometimento do ambiente | Quarentena operacional; preservar evidências e reconstruir destino limpo | Reutilizar credenciais, conceder mais privilégios ou chamar o ambiente de confiável só porque reiniciou |
| Controle obrigatório indisponível | Perfil/operacão indisponível; alternativa com garantias adequadas | Rebaixar o controle e manter o mesmo selo de segurança |

O OpenKoda deve explicar diferença entre **tentar novamente**, **continuar coleta**, **reexecutar operação** e **restaurar ambiente**. Ações com efeito externo não recebem retry automático por timeout ambíguo.

## 11. Critérios verificáveis antes da construção de confiança

1. Instalação interrompida em cada etapa retoma sem criar distro duplicada nem tocar em uma distro pessoal.
2. O painel identifica usuário, versão, ambiente e capabilities efetivos; configuração desejada divergente não passa como pronta.
3. Operação recusada por perfil/escopo não cria processo; alvo permitido funciona no mesmo perfil quando os controles estão disponíveis.
4. Perda de UI e retorno não duplica operações; processo não reconciliado aparece como desconhecido.
5. Manutenção exige exclusão, preserva evidências e produz inventário novo; avaliações não atravessam essa mudança sem registrar a nova identidade.
6. Cancelamento de uma Execução não encerra distros pessoais nem usa shutdown global.
7. Backup é testado; restauração preserva a origem e não muda o vínculo de evidências antigas.
8. Um requisito de contenção forte não pode ser satisfeito apenas desativando automount/interop ou adicionando um container no WSL.

As validações acima são trabalho futuro. O documento descreve o que deve ser demonstrado, não resultados já obtidos no computador atual.
