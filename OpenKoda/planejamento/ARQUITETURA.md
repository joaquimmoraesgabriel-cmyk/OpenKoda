# Arquitetura proposta do OpenKoda

Estado: mapa para revisão, sem implementação. A descrição de cada componente é um contrato desejado, não uma garantia já demonstrada. Base confirmada e premissas: [BASE-E-PREMISSAS](referencias/BASE-E-PREMISSAS.md). Vocabulário: [GLOSSARY](../GLOSSARY.md).

## 1. Forma do sistema

O OpenKoda será um workbench local de segurança. Ele reúne avaliações, conversa com IA, ferramentas Kali, evidências, achados e relatórios em uma interface. A IA pode explorar o trabalho dentro de uma política delimitada; o executor verifica a operação concreta antes de iniciar um processo. O histórico permanece útil mesmo sem provedor de IA disponível.

A implantação inicial proposta tem um desktop Windows, um núcleo local com persistência, um broker Windows com API estreita e um executor Linux dentro de uma distro Kali dedicada. O modelo fica atrás de um adaptador de provedor. Não é necessário um servidor comercial próprio para a operação local; serviços futuros não são requisito da primeira implantação.

**Tauri é o framework desktop escolhido pelo usuário**, registrado no [ADR-0004](../docs/adr/0004-tauri-como-framework-desktop.md). A direção visual está aprovada em [VISUAL-E-TELAS](VISUAL-E-TELAS.md). A fundação recomendada é Tauri 2, frontend React/TypeScript e núcleo/broker Rust; versão, bibliotecas e separação final de processos ainda dependem das provas F1. A linguagem do executor Kali não está decidida. Electron não é mais uma alternativa pendente neste mapa.

## 2. Componentes e responsabilidades

| Componente lógico | Responsabilidade | Contrato de limite |
|---|---|---|
| Painel desktop Tauri | Navegação aprovada, formulários, conversa opcional, planos, aprovações, processos, evidências e achados | WebView apresenta dados; envia ações de domínio ao núcleo, sem obter uma API universal de host exec |
| Núcleo da instalação | Projetos, avaliações, comandos da UI, versionamento, transações e eventos | Autoridade sobre o estado local e dono dos IDs |
| Coordenador de avaliação | Planejamento, seleção de investigadores, cobertura, orçamento e síntese | Pode propor operações e delegar trabalho dentro do mesmo escopo |
| Adaptador de IA | Provedor, modelo, streaming, limites de contexto, custo e retomada | Só envia contexto aprovado e nunca equivale a permissão de execução |
| Montador de contexto | Seleção de fontes, resumos, referências, redaction e páginas | Registra quais evidências entraram na chamada e o que foi omitido |
| Motor de política | Resolver perfil, escopo, grants, TTL, risco e revogação | Resultado obrigatório antes de cada dispatch ou retry |
| Gateway MCP interno | Catálogo de ferramentas e chamadas tipadas | Não aceita um grant inventado pelo modelo nem transforma annotations em autorização |
| Broker Windows | Descobrir/iniciar ambiente gerido, IPC, vault, staging e manutenção específica | Métodos estreitos e identidade de peer, sem shell arbitrário exposto à IA |
| Executor Kali | Materializar operação, ambiente, argv, processo e limites | Só usa o perfil verificado e devolve estado real do processo |
| Adaptadores de ferramentas | Validar argumentos, construir execução, conferir versão e interpretar saída | Compatibilidade e efeitos declarados, sem confiança automática no conteúdo produzido |
| Gerente de ambientes/pacotes | Instalar, diagnosticar, atualizar e recuperar ambiente gerido | Exclusão mútua com avaliações e autorização de manutenção separada |
| Coletores e parsers | Guardar bruto, normalizar, indexar e produzir observações | Hash, origem, versão e perda/truncamento explícitos |
| Validador de resultados | Relacionar candidato, evidência, alcance e contraevidência | Erro de ferramenta não vira ausência de falha; hipótese não vira confirmação |
| Persistência e auditoria | Estado transacional, blobs, eventos, checkpoints, integridade e migração | Fonte canônica local, com projeções reconstruíveis |
| Exportador | Relatório, pacote de evidências e formatos de intercâmbio | Exportação selecionada/revisada, com redaction e destino explícitos |

Esses nomes representam responsabilidades. Não obrigam um processo por componente nem microserviços. Um núcleo modular pode hospedar vários deles; a separação de autoridade entre Windows, IA, executor e manutenção continua necessária.

## 3. Processos e canais propostos

1. A UI Tauri conversa com o núcleo nativo por commands IPC tipados e restritos. Cada request tem versão, id de correlação e resultado/erro. Webviews de conteúdo não recebem o mesmo canal de confiança da UI principal.
2. O núcleo mantém um coordenador por avaliação ativa, com fila e orçamento. Workers não recebem credenciais ou autoridade maiores que a avaliação que os originou.
3. O gateway MCP inicia o serviço Linux por stdio supervisionado, por um launcher Windows/WSL de finalidade delimitada. Logs vão em canal separado das mensagens MCP. Não há porta LAN aberta por padrão.
4. O broker host fornece serviços que Linux não deve controlar diretamente: vault, escolha de diretório Windows, validação de manifests de ambiente e manutenção previamente autorizada.
5. O executor Linux mantém a supervisão de processos separada da conexão de UI. O fim do pipe não é interpretado como execução concluída ou cancelada.
6. Um protocolo de eventos próprio registra estado durável, seq, correlationId e referências de evidência. Progresso MCP pode alimentar a UI, mas não substitui o estado canônico de processos.

O contrato MCP e as mensagens estão em [IA-MCP-E-DADOS](IA-MCP-E-DADOS.md). Proposta inicial de transporte: stdio local. Um cliente externo MCP será uma superfície adicional com descoberta, pareamento/autorização e catálogo restrito; não reutilizará implicitamente a sessão administrativa do desktop.

### 3.1 Fundação Tauri e fronteira da interface

Tauri usa um processo nativo e WebViews; no Windows, o conteúdo visual utiliza WebView2. Commands podem devolver respostas serializáveis e erros ao frontend. Esses mecanismos são recursos do framework; os contratos de domínio e suas restrições abaixo são escolhas do OpenKoda. [Arquitetura Tauri](https://v2.tauri.app/concept/architecture/) · [commands](https://v2.tauri.app/develop/calling-rust/).

| Camada proposta | O que poderá fazer | O que precisa ser demonstrado em F1 |
|---|---|---|
| Frontend local no WebView | Navegar nas seis telas, editar rascunhos, selecionar arquivos por fluxo mediado e apresentar eventos/projeções | Conteúdo remoto/evidência não se torna UI confiável; teclado, escala e leitores de tela |
| Handlers nativos Tauri | Ações nomeadas de Projeto/Avaliação, leitura paginada de evidência, solicitação de manutenção e diagnóstico | Commands explicitamente restritos por origem/janela/capacidade e validação de objeto/revisão; erros sem segredos |
| Núcleo/broker Windows | Política, persistência, vault, staging e launcher delimitado de WSL | Não depender da UI para validar; sem bloquear o loop visual com subprocessos longos |
| Gateway e executor Kali | MCP e supervisão Linux com identidade, versões e grants | Interrupção/reconexão, cancelamento real e nenhum dispatch em nome da UI sem autoridade válida |

Commands de aplicação não ficam restritos automaticamente só por existir um arquivo de capabilities: o plano exige escopar explicitamente os commands registrados e testar chamadas não autorizadas. Capabilities do Tauri não substituem as verificações no núcleo/executor. Não expor plugins genéricos de shell/arquivos à UI por conveniência. [Capabilities oficiais](https://v2.tauri.app/security/capabilities/) · [controle SEC-23](PERMISSOES-E-SEGURANCA.md).

Ações de tela serão contratos como criar/revisar Avaliação, solicitar pausa, consultar Execução e calcular manutenção — nomes lógicos, sem fixar ainda assinaturas. Leituras retornam projeções limitadas, não o banco/vault inteiro. Eventos de UI têm cursor e podem ser coalescidos; após reload, o frontend lê o estado canônico e reconcilia. O canal visual não é o terminal nem a fonte durável de evidência.

Uma única janela principal é a base proposta; leitores de conteúdo não confiável não ganham as permissões dela. Não há requisito de servidor HTTP/WebSocket local para ligar UI e núcleo: usar o IPC do Tauri nessa fronteira e MCP stdio na fronteira de ferramentas. O detalhe de transporte de assets do framework não é uma API de execução aberta na rede. Superfícies externas futuras exigem decisão própria.

### 3.2 Distribuição e ciclo de vida a validar

Planejar instalador Windows Tauri, verificação/provisionamento de WebView2, assinatura, integridade e atualização compatível de app/executor/catálogo. O app permanece usuário comum; elevação do sistema, se necessária à preparação do WSL, é pontual e visível. Não transformar toda a aplicação em administrador. Versões suportadas, estratégia de instalação offline e mecanismos de atualização serão escolhidos antes de distribuir.

Na base proposta, núcleo/broker podem ser módulos do processo nativo; tarefas longas têm supervisão própria. Serviço persistente ou sidecar é alternativa a demonstrar, não pressuposto nem razão para prometer operação com o app fechado. A atualização não altera o ambiente de uma Avaliação ativa. A moldura nativa deverá oferecer fechar/maximizar/atalhos/acessibilidade preservando a identidade aprovada.

## 4. Onde cada dado reside

| Dado | Local lógico proposto | Observação |
|---|---|---|
| Estado de projeto/avaliação/política | Banco local gerido no Windows | SQLite é proposta inicial, ainda sujeita a prova de durabilidade/migração |
| Saída bruta ativa | Spool Linux por execução | Sequência de chunks, tamanho e digest em progresso |
| Evidência consolidada | Object store local gerido no Windows | Promoção só após verificar tamanho, manifest e hash |
| Workspace temporário | Diretório Linux por avaliação com subdiretório por tentativa | Apenas fontes/staging explicitamente selecionados |
| Repositório Windows original | Diretório escolhido pelo usuário | Snapshot de leitura por padrão, escrita apenas em fluxo de patch autorizado |
| Pacotes e caches Kali | Distro dedicada | Manutenção serializada, versões registradas por avaliação |
| Credenciais | Vault host com referências opacas | Segredos de modelo não vão para Linux; credenciais de alvo só quando estritamente necessárias |
| Contexto enviado ao modelo | Provedor configurado | Seleção/minimização registradas; aplicação local não significa processamento sempre local |

O banco referencia blobs por identidade, não aceita um caminho fornecido por conteúdo não confiável como destino irrestrito. Evidências podem existir em spool pendente e no object store; a transição é registrada para impedir que a UI mostre completude antes da cópia verificada.

## 5. Caminho principal de avaliação

O usuário escolhe projeto, alvo, objetivo, perfil e orçamento. O núcleo verifica ambiente e versões. O coordenador produz um plano de ação que identifica operações, pré-requisitos e efeitos. A política decide se o plano pode ser executado, requer aprovação concreta ou precisa mudar.

Cada operação é resolvida para um adaptador ou shell avançado. O executor confere o grant vinculado à revisão da operação, ambiente, alvo efetivo, janela e limites. Os processos geram saída, artefatos e eventos. Parsers preservam o bruto e geram observações. A IA recebe páginas selecionadas e propõe candidatos, próximos passos e síntese. Validação/cobertura decidem o que pode ser apresentado como achado. O relatório é uma projeção versionada desses registros.

Novos alvos descobertos entram como descobertas pendentes. Não ampliam o escopo automaticamente. Um redirect, novo IP, subdomínio, child process ou recurso sugerido pela IA precisa continuar dentro das condições permitidas.

## 6. Modos de trabalho

| Modalidade | Uso principal | Ambiente proposto |
|---|---|---|
| Código/repositório | Revisão estática, dependências, configuração e patches propostos | Snapshot, rede desabilitada para ferramentas de avaliação quando possível |
| Web/API | Observação e testes dinâmicos autorizados | Adaptadores e política de destinos, taxas e efeitos |
| Rede/infraestrutura | Inventário, configuração e validações autorizadas | Capacidade de rede explicitamente demonstrada no perfil |
| Evidência offline | Arquivos, capturas, logs, binários e resultados importados | Sem execução automática do artefato analisado |
| Investigação assistida | Conversa orientada a hipóteses, ferramentas e evidências | Mesmo ciclo de autorização, sem bypass por ser chat |
| Laboratório | Exercícios e validação da integração em alvos controlados | Destinos de teste explícitos, separados de ambientes reais |
| Remediação/reteste | Propor patch, aplicar mudança autorizada e conferir resultado | Escrita distinta de leitura e autorização separada para mudança de alvo |

É uma cobertura de produto proposta. Instalar todas as ferramentas não demonstra que todos esses fluxos estejam implementados ou compatíveis com WSL.

## 7. Autonomia e especialização

Proposta: um coordenador e especialistas sob demanda para código, web/API, rede e validação. Cada worker recebe a mesma revisão de escopo, subconjunto de fontes, orçamento, capacidade e resultado esperado. Descoberta independente e validação crítica podem ser separadas para reduzir confirmação automática de uma hipótese.

Uma avaliação profunda repete investigações independentes com orçamento explícito, consolida duplicatas e mantém divergências. Quantidade de agentes não é um score de cobertura. Workers não criam cadeia ilimitada de agentes nem executam fora do motor de política.

No modo assistido, o usuário acompanha e orienta. No modo autônomo delimitado, uma política permite categorias conhecidas sem intervenção por comando. Operações com novos efeitos, novos alvos, root, instalação, credenciais ou mudanças persistentes são reavaliadas conforme a política. Autonomia não depende de simular cliques no painel.

## 8. Isolamento: contrato honesto

O requisito confirmado é usar Kali/WSL. A proposta não vende a distro como barreira forte contra o Windows. Perfis só ficam disponíveis quando os controles necessários forem demonstrados no host. A documentação de fronteiras e critérios está em [PERMISSOES-E-SEGURANCA](PERMISSOES-E-SEGURANCA.md).

Para tarefas que exigem contenção de código hostil ou privilégios incompatíveis com esse perfil, o mapa prevê suporte futuro a um backend realmente separado, com política própria. Essa extensão mantém contratos de avaliação/execução, sem fingir que trocar um toggle no WSL cria isolamento forte. O produto precisa bloquear ou explicar a incompatibilidade antes da execução.

## 9. Atualização, execução e recuperação

App, protocolo, executor Linux, catálogo, parser, modelo e pacotes têm versões independentes. Um handshake detecta incompatibilidade antes de aceitar trabalho. Atualizações de ferramentas não acontecem no meio de uma avaliação; reparação de ambiente exige suspender a fila e reconciliar processos ativos.

Reiniciar a UI não mata silenciosamente trabalho. Reiniciar o núcleo exige reconciliar jobs com identidades do executor e checkpoints. Operações cujo efeito é desconhecido não são reexecutadas automaticamente. Continuação de um processo vivo é diferente de uma nova tentativa da operação.

Fechar o aplicativo oferece comportamento configurado e visível: cancelar/encerrar de forma supervisionada, ou continuar somente quando o serviço independente estiver instalado e testado. A proposta inicial não promete trabalho quando o PC estiver desligado, suspenso ou após reboot sem reconciliação.

## 10. Fronteiras de decisão

O mapa fixa Tauri e a direção das seis telas aprovadas. React/TypeScript, versão Tauri 2, bibliotecas, linguagem do executor, assinaturas IPC e distribuição detalhada continuam como propostas em [DECISOES](DECISOES.md). As provas de integração Windows/WSL, autoridade, acessibilidade, atualização e supervisão continuam necessárias; aprovar o visual não valida esses mecanismos.
