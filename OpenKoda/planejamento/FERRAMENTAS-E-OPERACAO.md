# Ferramentas, pacotes e operação da IA

Especificação proposta para o OpenKoda, de 7 de novembro de 2026. Não é um catálogo garantido de ferramentas já integradas. Não houve instalação de pacotes, execução de Kali, scan ou implementação nesta etapa. Contexto do ambiente: [KALI-E-AMBIENTES.md](KALI-E-AMBIENTES.md).

A direção visual de [Kali e ferramentas](conceitos/02-kali-e-ferramentas-v1.png) foi aprovada pelo usuário para o desktop Tauri. O contrato abaixo liga seus controles ao comportamento proposto, conforme [TELAS-ADICIONAIS](conceitos/TELAS-ADICIONAIS.md) e [AJUSTES-DE-INTERFACE](AJUSTES-DE-INTERFACE.md). As seleções, mensagens de saúde e quaisquer valores das imagens são exemplos, sem comprovar instalação ou prontidão.

## 1. O significado de a IA usar o Kali

A IA pode escolher e combinar capacidades compatíveis com o Projeto, o Plano de ação e o Perfil de execução. O executor continua sendo a autoridade que admite uma Operação. Ter uma ferramenta no Kali não concede automaticamente ao modelo permissão para usá-la, nem comprova que ela funciona com o hardware/WSL desta instalação.

O produto deve ter três camadas de uso:

| Camada | Experiência | Condição |
|---|---|---|
| Adapter tipado | A IA solicita uma capacidade com campos de alvo, opções e limites conhecidos | Cadastro, validação, imposição e parser compatíveis com a ferramenta/versão |
| Operação composta | O plano encadeia adapters e validações com dependências e checkpoints | Cada operação conserva sua autoridade; resultado de uma etapa não autoriza a seguinte |
| Shell avançado | Um operador/IA propõe uma invocação ou roteiro explícito para ferramentas ainda não adaptadas | Perfil avançado aprovado e controles adequados; sem herdar a garantia do adapter limitado |

Essa composição atende à expansão para o conjunto amplo do Kali sem prometer centenas de integrações profundas na primeira entrega. O catálogo pode mostrar um pacote **instalável** antes de seu adapter estar disponível, mas a UI deve distinguir isso de **integrado** e **pronto para executar**.

## 2. Catálogo de capacidades

O catálogo organiza o trabalho pelo objetivo do usuário. Ferramentas concretas e versões são resolvidas no ambiente, sem embutir uma lista imutável no aplicativo.

| Família | Capacidade de produto | Entrada/saída esperada | Limites que o catálogo deve mostrar |
|---|---|---|---|
| Código e dependências | Ler repositório/diff, configurações, manifests e resultados de analisadores | Revisão, candidatos, arquivos/linhas e evidências | Ler código não autoriza executar build, hooks, testes ou binários do projeto |
| Web e APIs | Inventariar superfície autorizada, analisar respostas/configuração e executar verificações previstas no plano | Requests/observações redigidas, endpoints, candidatos e validações | Autenticação de alvo, efeitos persistentes, redirects e limitação de taxa |
| Rede e infraestrutura | Identificar e validar serviços/configurações dentro do escopo | Inventário de hosts/serviços, cobertura e evidência temporal | Rotas reais, privilégios de sockets, hosts fora de escopo e disponibilidade de controles |
| Evidências offline/forense | Inspecionar arquivos, imagens e capturas previamente selecionadas | Artefatos derivados com relação ao original | Não montar conteúdo não confiável com escrita nem executar payload por conveniência |
| Engenharia reversa | Extrair metadados, strings, imports e analisar artefatos com motores disponíveis | Localizações, pseudocódigo quando obtido e lacunas explícitas | Licença/engine, arquitetura, memória/tempo e diferença entre estático e observado |
| Auditoria de credenciais autorizada | Avaliar políticas, hashes ou amostras fornecidas no contexto autorizado | Resultado de auditoria e evidência minimizada | Material sensível, orçamento, destinos e exposição ao provedor do modelo |
| Protocolos e configuração | Analisar TLS, DNS, autenticação/configuração e fluxos capturados ou autorizados | Propriedades observadas e validações | Nenhuma inferência de proteção global a partir de uma única resposta |
| Relatórios e comparação | Deduplicar, correlacionar, comparar avaliações e montar relatório | Achados, coverage, deltas, recomendações e evidências | Comparabilidade depende de alvo/ambiente/escopo/versão, não apenas do nome do projeto |
| Hardware e rádio | Capacidades que dependem de USB, Wi-Fi, Bluetooth, SDR ou GPU | Disponibilidade detectada e resultados quando suportados | Pacote instalado não prova acesso ao dispositivo, modo necessário ou driver |

O catálogo Kali atual contém metapacotes por grupos, incluindo web, forense, engenharia reversa, rede e hardware. A associação de um grupo a uma capacidade OpenKoda é uma proposta de UX, não prova de compatibilidade de cada pacote. [Catálogo kali-meta](https://www.kali.org/tools/kali-meta/).

USB no WSL tem integração própria via USB/IP. Para cada capacidade de hardware, o diagnóstico deve testar disponibilidade e requisitos concretos, sem concluir que uma interface emulada substitui acesso físico. O mapa não inclui instalar drivers ou vincular dispositivos automaticamente. [Microsoft: USB no WSL](https://learn.microsoft.com/en-us/windows/wsl/connect-usb).

### 2.1 Catálogo na tela aprovada

A aba **Ferramentas** mostra busca e linhas de conjuntos à esquerda; à direita, o painel **Preparar instalação** mostra a seleção atual. As abas **Ambiente** e **Manutenção** permitem consultar, respectivamente, prontidão/configuração efetiva e os planos/trabalhos de mudança, sem criar destinos primários extras para cada função técnica.

| Controle/área | Significado no produto | O que não executa por si só |
|---|---|---|
| Pesquisar conjuntos | Filtrar o catálogo efetivo e mostrar critérios de compatibilidade | Instalar, atualizar índices ou executar ferramenta |
| Selecionar conjunto | Alterar a intenção de instalação no painel de preparação | Autorizar alteração do ambiente |
| Badge Instalado | Presença verificada naquele ambiente/inventário | Confirmar adapter compatível, permissão, hardware ou prontidão para avaliação |
| Ver todos os conjuntos | Abrir as opções adicionais, incluindo seleções ampliadas e everything | Trocar a seleção atual ou instalar todos os grupos |
| Calcular instalação | Resolver dependências, efeitos, download e espaço da seleção | Aplicar pacotes, conceder root ou dispensar revisão |
| Dependências / Verificar | Abrir a resolução e suas limitações | Aprovar automaticamente dependências novas/removidas |
| Ver diagnóstico | Abrir a informação real do ambiente e os motivos de bloqueio | Executar reparo, apagar distro ou converter seu ownership |

As primeiras linhas do conceito — base sem interface gráfica, seleção padrão, web/API, forense e engenharia reversa — são uma apresentação inicial do catálogo, não um limite definitivo de opções. Seus IDs de metapacotes podem ficar como texto secundário para identificar a escolha. O app preserva a distinção entre **instalável**, **instalado**, **integrado**, **compatível**, **pronto** e **bloqueado**.

Um conjunto instalado pode precisar de atualização ou ter apenas parte de suas capacidades pronta. A UI deve explicar esse motivo por ambiente/ferramenta, sem substituir todos os estados por um checkbox verde.

## 3. Seleção e instalação de pacotes

Metapacotes Kali agregam dependências e permitem selecionar conjuntos; seu tamanho individual não representa a soma dos downloads e do espaço instalado. O OpenKoda deve apresentar conjuntos como escolhas de intenção e resolver o efeito real no Ambiente escolhido. [Metapacotes Kali](https://www.kali.org/docs/general-use/metapackages/).

Propostas de apresentação: base WSL, headless, seleção padrão, ampliada, everything e grupos temáticos. O catálogo consultado anuncia, entre outros, `kali-linux-wsl`, `kali-linux-headless`, `kali-linux-default`, `kali-linux-large` e `kali-linux-everything`. O app deve mostrar versão e data da consulta, compatibilidade, custo estimado e itens não suportados. Não instalar tudo por padrão. [kali-meta](https://www.kali.org/tools/kali-meta/).

### Seleção → cálculo → revisão → aplicação

Antes do cálculo, o painel preserva a linguagem do conceito: **Download: A calcular**, **Espaço adicional: A calcular** e **Dependências: Verificar**. Não mostrar zeros como se significassem ausência de download ou custo. O botão **Calcular instalação** fica disponível apenas com seleção e ambiente compatível; estados indisponível, checando, bloqueado por manutenção ou em recuperação mostram o motivo e a ação pertinente.

O cálculo usa o inventário e os índices identificados no plano, com sua data/frescura. Se atualizar metadados ou obter informação de rede for necessário, essa suboperação deve estar declarada e autorizada pela política aplicável; não ocultá-la como instalação de pacotes ou como efeito de selecionar uma linha.

Depois do cálculo, apresentar uma revisão com ambiente/destino, conjuntos selecionados, pacotes novos/atualizados/removidos, versões, dependências e conflitos, download e espaço estimados, espaço disponível, origem/autenticação, privilégios/efeitos e recuperação. Detalhes extensos podem ser expandidos, mas efeitos relevantes e informação desconhecida não ficam escondidos. Só uma ação de aplicação claramente identificada, vinculada à revisão válida, permite solicitar a manutenção.

Trocar seleção, ambiente, inventário relevante ou resolução invalida o cálculo/aprovação anterior. Falha no cálculo não preserva um botão de aplicar um plano antigo como se continuasse atual. Reemitir um grant técnico só é possível sob decisão ainda válida; não renova a aprovação, a janela do escopo ou o orçamento.

**Nenhuma avaliação instala pacotes automaticamente por descobrir uma dependência ausente.** Ela pode ficar bloqueada e oferecer **Preparar ferramentas**, levando a esta área com a necessidade contextualizada. O usuário revisa o plano de manutenção separado; depois da instalação e verificação efetivas, a avaliação é revalidada antes de retomar, conservando a mudança de versões em seu histórico.

No desktop Tauri, consulta de catálogo, cálculo, autorização e acompanhamento são ações de domínio separadas. O frontend envia seleção/referências de revisão; núcleo/broker conferem esses dados antes de atuar. Isso não escolhe a linguagem do executor Linux nem concede shell/root à interface.

### Contrato de uma mudança de pacotes

Antes da aplicação, gerar um plano de manutenção com:

- Ambiente/identidade e inventário de origem; versões do gerenciador e índices/repositórios usados na resolução.
- Intenção solicitada, pacotes já presentes, pacotes novos, atualizados, removidos, retidos e conflitos.
- Tamanho de download e expansão estimados, espaço livre e margem necessária; marcar desconhecido quando a ferramenta não puder estimar.
- Origem e autenticação do repositório, licenças/interações previstas, serviços/configurações que podem mudar e eventual reinicialização.
- Backup/ponto de recuperação disponível, limites da reversão, efeito da manutenção nas avaliações e prazo de validade do plano.
- Identidade/digest do plano, autoridade necessária e decisão aplicável antes de qualquer mudança.

A simulação APT consulta o estado atual e não mantém o locking normal; mudanças concorrentes podem invalidar seu resultado. Por isso, a proposta inclui conferir novamente inventário/índices/solução antes de executar e exigir nova revisão se o efeito mudar materialmente. Simulação não é reserva de espaço, garantia de sucesso ou transação. [apt-get](https://manpages.debian.org/trixie/apt/apt-get.8.en.html).

APT verifica autenticação de metadados dos repositórios; isso deve permanecer habilitado. Erros de chave, origem ou metadados alterados não são resolvidos desabilitando autenticação ou aceitando repositório desconhecido automaticamente. A aprovação do usuário para instalar um conjunto não transforma um pacote em código confiável por definição. [apt-secure](https://manpages.debian.org/trixie/apt/apt-secure.8.en.html).

### Estados da manutenção

`Planejada → resolvida → aguardando autorização → aguardando exclusividade → baixando → aplicando → verificando → concluída`.

Ramos adicionais: `cancelada antes de aplicar`, `download interrompido`, `espaço insuficiente`, `lock externo`, `falha ao aplicar`, `reparo necessário`, `recuperação por backup` e `não reconciliada`. A conclusão exige inventário efetivo e readiness novos, não somente exit code do processo de download.

| Estado na UI | Informação e controle esperados |
|---|---|
| A calcular | Seleção preservada, valores desconhecidos explícitos e nenhuma aplicação iniciada |
| Checando / calculando | Progresso da consulta, origem e possibilidade de interromper a preparação; aplicar indisponível |
| Revisão pronta | Resolução completa ou lacunas explícitas, revisão identificada e ação de autorização pertinente |
| Aguardando exclusividade | Plano/decisão ainda sujeitos à validade; motivo da espera e cancelamento antes de aplicar |
| Bloqueado por manutenção | Trabalho exclusivo existente ou processo não reconciliado; abrir seu contexto sem criar manutenção duplicada |
| Baixando / aplicando / verificando | Etapa, quantidade medida ou estimada, logs redigidos sob demanda e sem promessa de cancelamento instantâneo durante aplicação |
| Erro / recuperação | Erro, efeito conhecido/desconhecido, evidência preservada e proposta de reparo separada |
| Concluída | Novo inventário e saúde verificados; capacidades disponíveis discriminadas, sem promover todo o Kali a pronto |

A aba Manutenção é a fonte visível para retomar/acompanhar esse trabalho. Fechar ou trocar de aba não repete a operação nem converte uma falha em sucesso. A interface conserva o checkpoint e a revisão, e reconcilia o estado real ao voltar.

| Situação | Comportamento especificado |
|---|---|
| Outro APT/dpkg em uso | Esperar com prazo/cancelamento ou orientar; não apagar lock nem matar processo desconhecido |
| Rede indisponível no download | Preservar estado/caches válidos; retry limitado e backoff, sem ampliar fontes |
| Índices/pacotes mudaram após aprovação | Invalidar plano antigo, apresentar resolução nova e preservar histórico |
| Cancelamento durante aplicação | Exibir que parada pode exigir reparo; não prometer interrupção instantânea e limpa |
| Processo de manutenção perdido | Marcar estado desconhecido, verificar processo/inventário antes de retry |
| Pacote solicita interação inesperada | Suspender/encaminhar pela UI ou falhar com contexto; nunca responder com entrada escolhida pelo modelo sem autorização pertinente |
| Falha de configuração de pacote | Modo reparo explícito e evidências; avaliações dependentes ficam bloqueadas |
| Restauração solicitada | Usar backup verificado ou reconstruir ambiente; remover pacote não é rollback completo |

Atualização de ferramenta, base de assinaturas, templates e parser são identidades distintas. Uma Avaliação deve continuar com versões fixadas ou registrar uma transição; não atualizar no meio para tentar obter um resultado diferente silenciosamente.

## 4. Contrato do adapter

Cada adapter de ferramenta precisa declarar, de maneira versionada:

| Campo conceitual | Obrigação |
|---|---|
| Identidade | Ferramenta, faixa de versões suportada, origem do pacote, adapter e parser |
| Capacidade | Objetivo oferecido, modalidades, efeitos conhecidos e limitações |
| Entrada | Schema de alvos/opções/arquivos/credenciais, defaults visíveis e campos rejeitados |
| Autoridade | Leitura/escrita/rede/privilégios/dispositivos e perfil compatível |
| Preparação | Resolução de executável e versão, caminhos finais, workspace e dependências |
| Invocação | Vetor argv para binário conhecido por padrão; shell apenas quando declarado e justificado |
| Controles | Timeout, orçamento, concorrência, rede, limites de output e condições de cancelamento |
| Saída | stdout/stderr/artefatos, formato de parser, exit codes e detecção de resultado parcial |
| Verificação | Como distinguir conclusão, cobertura, candidato e achado validado |
| Recuperação | Idempotência conhecida, retries permitidos, efeitos que tornam retry ambíguo e limpeza |

Um adapter não concatena campos não confiáveis em código de shell. Quando o programa necessita de arquivo de configuração, gerar um artefato com schema, validar e ligar seu digest à operação. Opções que habilitem execução arbitrária, plugins, scripts, destinos extras ou escrita fora do workspace entram na política da operação; não ficam escondidas como “parâmetros avançados”.

O processo deve usar ambiente mínimo e caminhos de executável resolvidos. Mudança de executável, config, versão ou alvo após a aprovação exige revalidação. Entradas vindas do repositório, página, capture, template ou resultado anterior continuam dados não confiáveis.

## 5. Operação concreta e Execução

A operação autorizável é diferente da tentativa de processo. Uma Operação pertence a uma revisão de Plano de ação e define finalidade, alvos, entradas, efeitos, requisitos, limites, validação e recuperação. Cada Execução registra a tentativa concreta, inclusive retry e falhas antes de iniciar.

Antes de criar processo, o executor deve conferir: vínculo da sessão; ambiente e revisão do plano; schema; alvo normalizado e seu escopo atual; artefatos; ferramenta/configuração efetivos; autorização válida; controles requeridos; slots e espaço. O fato de a IA ter escrito “aprovado” não satisfaz esse contrato.

Registro mínimo da Execução:

- `executionId`, Projeto, Avaliação, operação/revisão, tentativa, identidade do Ambiente/executor e correlação da chamada.
- Binário/configuração/parser efetivos, argv registrado de forma redigida, cwd resolvido, referências de entradas e segredos sem valor em claro.
- Perfil e decisão aplicada, recursos concedidos, controles verificados e limites; hora de admissão, início, fim e relógio de duração.
- Process group/session/cgroup ou mecanismo equivalente quando suportado; handles de streams, lease e identidade necessária para reconciliação.
- Exit code/sinal/timeout, último evento confirmado, status de cancelamento, saída parcial e evidências com digest.

Os nomes de campos acima expressam o contrato de dados proposto, não uma implementação pronta do protocolo.

## 6. Processos, terminais e encerramento

Preferir invocação não interativa com stdin fechado ou definido e streams separados. PTY é uma capacidade explícita para ferramentas que a exigem; ele pode mesclar saída e alterar o formato. Uma Execução PTY não recebe automaticamente o mesmo parser de stdout/stderr de uma execução comum.

Estados propostos: `na fila`, `validando`, `aguardando autorização`, `preparando`, `executando`, `aguardando entrada`, `cancelando`, `encerrada`, `falhou`, `timeout`, `interrompida` ou `desconhecida`. Uma resposta de admissão não é conclusão. Um MCP desconectado não é sinal de que o processo terminou.

O executor supervisiona o grupo de processos e filhos, com controle equivalente quando disponível. A política de cancelamento tenta encerramento cooperativo e, após grace period, encerramento forçado do grupo gerido; a verificação final determina quais filhos e artefatos permaneceram. Falta de controle sobre descendentes é limitação de capability, não sucesso de cancelamento.

Um botão de parada de emergência deve impedir novas admissões e acionar cancelamento das Execuções da instalação/ambiente selecionado. Encerrar toda a distro gerida é recurso final separado, com impacto visível sobre avaliações e evidências; shutdown global de WSL não é implementação aceitável do botão comum.

Ferramentas que deixam servidores, listeners, sessões remotas ou alterações no alvo precisam de plano de teardown verificado. Cancelar o processo local pode não desfazer seu efeito externo; o relatório deve manter essa pendência.

## 7. Saída bruta, parsing e conteúdo para o modelo

O fluxo proposto preserva três produtos distintos:

1. **Bruto:** bytes de stdout/stderr, sequência de blocos, streams, formato, timestamps e arquivos gerados. O produto informa limites/caps e erros de armazenamento.
2. **Estruturado:** parser determinístico versionado, registros extraídos, localizações/offsets e erros de parse, sempre apontando para o bruto.
3. **Contexto resumido:** seleção orientada ao orçamento de tokens do modelo, com origem, cobertura e links/cursors para buscar o restante.

O resumo não substitui a evidência. Se a saída exceder a cota, parar/capar conforme o plano e marcar incompletude; não apagar o início ou o fim sem registro. O modelo deve receber informação de truncamento, falha do parser ou falta de cobertura. “Nada encontrado” só cabe quando a execução e cobertura sustentam essa conclusão.

Cada artefato precisa de digest, tamanho, formato, dono lógico, classificação de sensibilidade, referência de origem e retenção. A sanitização para exibir HTML/terminal e a redação de segredos criam versões derivadas; o bruto pode ser mais restrito. Conteúdo perigoso de escape de terminal, HTML ou instruções de prompt não é executado pela UI, parser ou modelo como autoridade.

Quando o parser não conhece a versão/formato, o sistema mantém bruto e resultado **não interpretado/parcial**. A IA pode ajudar a examinar, mas uma interpretação livre não deve aparecer como resultado do parser validado.

## 8. Retries, duplicação e quedas

Cada pedido de início possui chave de idempotência ligada ao conteúdo canônico da operação. Repetir a mesma chave com conteúdo diferente é erro. O ledger do executor precisa retornar a Execução já admitida para uma repetição legítima, não criar outro processo.

Ao perder confirmação, consultar/reconciliar primeiro. Se não for possível provar se uma operação com efeito externo ocorreu, marcar resultado desconhecido e exigir decisão; não retry automático porque a IA ficou sem resposta. Leitura idempotente pode ter retry limitado, mas a evidência identifica cada tentativa e a mudança do alvo no tempo.

Eventos têm sequência por Execução e cursor durável de consumo. Reconectar reproduz eventos ainda não confirmados sem repetir a ação. O UI pode receber progresso em coalescência, mas a perda de renderização não deve apagar o log de execução. Destino lento sofre backpressure/spool limitado; não provocar uso ilimitado de memória ou quebrar o MCP por stdout misturado.

O executor não descobre órfãos somente por PID. Deve validar lease, início, ambiente e marcador de processo gerido antes de reassociar ou encerrar; PIDs podem ser reutilizados. Após reboot/crash, arquivo de estado “executando” é indício a reconciliar, não prova de atividade.

## 9. Limpeza, retenção e operação offline

O workspace da Avaliação possui um subdiretório por tentativa, contendo apenas entradas aprovadas, temporários e artefatos geridos. A limpeza exige containment de caminho final e ownership, lida com symlinks/mounts sem atravessar a fronteira prevista e preserva evidências retidas. Não limpar o diretório pessoal do Kali nem o repositório original para liberar espaço.

Retenção distingue temporários descartáveis, bruto protegido, artefatos derivados, banco de controle e relatórios exportados. Antes de remover conteúdo que sustenta um Achado, informar a perda de reprodutibilidade e atualizar o registro de disponibilidade; um hash sem bytes não equivale a evidência ainda acessível.

Offline tem readiness própria: ferramentas e bases necessárias presentes, modelo local disponível se escolhido, licença acessível quando exigida, entradas locais e execução sem rede demonstrável. Com provedor remoto sem conexão, permanecem consulta de histórico/evidências, manutenção de projetos e operações manuais autorizadas compatíveis; não prometer inferência offline sem um modelo local efetivamente configurado.

## 10. Critérios de aceitação para futura implementação

| Área | Demonstração exigida |
|---|---|
| Catálogo | Distingue disponível para instalar, instalado, integrado, compatível, pronto e bloqueado |
| Pacotes | Mostra mudanças/estimativa reais, revalida plano, respeita locks e detecta manutenção parcial |
| Autoridade | Operação fora do escopo não cria processo; shell avançado não herda limites apenas declarados pelo adapter |
| Conclusão | Admissão, saída parcial, exit e cobertura são estados separados |
| Processos | Cancelamento controla filhos geridos ou informa claramente a falha/limitação |
| Idempotência | Reenvio após perda de resposta não duplica processo ou efeito externo |
| Evidência | Parser/resumo aponta para bytes e versão; cap/erro não produz conclusão de ausência |
| Sensibilidade | Segredo não aparece em argv/log/contexto comum, e derivado redigido não é confundido com bruto |
| Queda | Reabertura reconcilia em vez de reexecutar; identidade não depende só de PID |
| Concorrência | Manutenção exclui avaliações; orçamento global e por alvo realmente governa a admissão |
| Offline | UI só promete capacidades cuja dependência está localmente pronta |

## 11. Decisões pendentes e caminho de expansão

Precisam de validação futura: método/canal de entrega Kali por versão Windows; IPC local exato; implementação dos controles Linux/rede por perfil; suporte a processos/PTY e sinais no bridge; esquema final de adapters; formatos de evidência por ferramenta; política de atualização e formato de backup; hardware suportado; backend separado de contenção forte.

A primeira entrega pode priorizar poucos adapters com contrato completo e um shell avançado explícito, enquanto o catálogo mostra a expansão prevista. Os grupos de pacotes continuam extensíveis sem fingir que integração profunda e compatibilidade de todo o Kali já foram demonstradas.
