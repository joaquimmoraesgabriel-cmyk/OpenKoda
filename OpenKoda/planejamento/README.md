# OpenKoda — mapa do aplicativo de cibersegurança

**Estado: mapa consolidado antes da construção, em 7 de novembro de 2026. Nenhum aplicativo foi construído nesta etapa.** O nome, o objetivo, o uso do Kali/WSL pela IA, o framework **Tauri** e a direção visual das **seis telas aprovadas** são confirmados. Mecanismos ainda não demonstrados, valores iniciais, versão do framework e bibliotecas complementares continuam propostas revisáveis.

O OpenKoda será um ambiente local para organizar e realizar trabalhos de segurança com ajuda de IA. O painel reúne Projetos, Avaliações, planos, processos, evidências, candidatos, achados, correções e relatórios. O Kali fornece as ferramentas Linux; o OpenKoda fornece o contexto do trabalho, a supervisão, a autoridade de execução e a ligação entre resultado e evidência.

## Como ele funcionará para você

1. **Preparar uma vez:** o app verifica Windows/WSL, prepara um Kali dedicado, mostra os conjuntos de ferramentas e configura o provedor/modelo de IA. Downloads, espaço, dependências, necessidade de elevação e retomada após reboot ficam visíveis. A distro pessoal existente não será adotada ou apagada automaticamente.
2. **Criar um Projeto:** você informa o que pretende avaliar e adiciona repositórios, arquivos, aplicações, APIs ou redes autorizadas. Define inclusões, exclusões, credenciais necessárias, janela, intensidade e limites de dados.
3. **Abrir uma Avaliação:** escolhe o objetivo e a modalidade. A IA considera as fontes e ferramentas disponíveis e propõe um Plano de ação com operações, efeitos, pré-requisitos e orçamento.
4. **Autorizar o trabalho:** uma política já configurada pode permitir operações conhecidas. Quando houver novos efeitos ou autoridade adicional, o painel apresenta o pedido concreto. A IA não cria sua própria aprovação e o executor verifica os limites antes de iniciar.
5. **Acompanhar a execução:** a IA usa adaptadores MCP e, quando permitido, o terminal avançado do Kali. Você vê processos, comandos/argumentos aplicáveis, saída, custos, fila, cobertura e pedidos pendentes. Pode orientar, pausar novos passos ou solicitar encerramento.
6. **Conferir resultados:** o app preserva o bruto capturado, cria observações e permite navegar de cada conclusão até a execução que a originou. Candidatos ainda não validados ficam separados dos achados sustentados.
7. **Corrigir e retestar:** a IA pode propor patches ou recomendações. Aplicar uma mudança é uma operação própria; um reteste registra o que foi realmente corrigido, em qual versão e com quais limites.
8. **Gerar relatório:** você escolhe conteúdo e público, revisa dados sensíveis e exporta resultados/cobertura/evidências. Envio externo é uma ação separada. Uma avaliação incompleta continua útil, mas aparece como parcial.

### Exemplo de uso

Você cria um Projeto para uma aplicação sua de laboratório, inclui o repositório e o endereço de teste, limita os caminhos/portas e escolhe um perfil ativo compatível. A IA lê o código selecionado, propõe verificações e usa ferramentas Kali para reunir evidências do alvo autorizado. Se descobrir outro domínio, ele aparece como descoberta pendente. A conclusão pode conter um achado sustentado, candidatos inconclusivos e áreas não examinadas. Depois de uma correção, uma nova avaliação de reteste compara a condição relevante.

## O que estará no produto

| Área | Funções mapeadas |
|---|---|
| Projetos e alvos | Fontes, snapshots, contexto, escopo versionado, histórico e comparações |
| Código | Revisão completa, de alterações e profunda, configuração/dependências e patches propostos |
| Web/API | Observações e verificações dinâmicas autorizadas, sessões e credenciais selecionadas |
| Rede | Inventário e validações delimitadas, portas/protocolos/taxas e capacidades do ambiente |
| Offline | Capturas, logs, binários e arquivos, com parsers e ferramentas de análise compatíveis |
| IA | Conversa contextual, coordenador, especialistas, planejamento, investigação, síntese e orçamento |
| Kali | Catálogo de pacotes, grupos por área, adaptadores e terminal avançado sob perfil |
| Execução | Fila, processos longos, stdin/PTY quando suportado, eventos, limites e cancelamento |
| Evidência | Bruto, origem/hash, parsing, páginas, redaction, busca e cobertura |
| Resultados | Candidatos, validação, achados, severidade/confiança, tratamento, remediação e reteste |
| Relatórios | Projeções humanas, dados estruturados, evidências selecionadas e pacote de reprodução |
| Operação local | Configuração, vault, saúde, armazenamento, backups, manutenção, atualização e recuperação |

O catálogo completo é o destino do produto. A primeira entrega proposta será uma fatia inteira desse fluxo, com poucas ferramentas bem integradas. As fases não removem as demais áreas do mapa; organizam a ordem em que serão demonstradas.

## Como será o painel

A referência central agora é [Visual e telas aprovadas](VISUAL-E-TELAS.md), que incorpora as capturas e substitui a composição antiga de três painéis permanentes. Superfícies claras, verde-petróleo, texto carvão, ícones discretos e espaços confortáveis dão identidade própria ao OpenKoda.

A navegação principal tem **Avaliações, Achados e Projetos**. **Kali e ferramentas** e **Configurações** ficam no trilho secundário. Avaliações, Achados e Projetos usam lista à esquerda e detalhe à direita. Conversa, execuções/terminal e evidências são abertas quando necessárias. Estado do ambiente e ação Nova avaliação ficam compactos no topo; diagnóstico, custos e detalhes técnicos têm acesso contextual.

No detalhe de uma Avaliação, Resumo, Achados, Atividade e Arquivos orientam a primeira camada. Plano, cobertura, evidências, correções/retestes e relatórios continuam no mapa, acessíveis por seções contextuais e sem criar uma aba global para cada função. Uma pausa solicitada não aparece como término confirmado; estados de Kali, IA e processo continuam independentes.

| Referência aprovada | Imagem |
|---|---|
| V-01 Painel da avaliação | [Abrir](conceitos/painel-openkoda-v2.png) |
| V-02 Nova avaliação | [Abrir](conceitos/01-nova-avaliacao-v1.png) |
| V-03 Kali e ferramentas | [Abrir](conceitos/02-kali-e-ferramentas-v1.png) |
| V-04 Achados | [Abrir](conceitos/03-achados-v1.png) |
| V-05 Projetos | [Abrir](conceitos/04-projetos-v1.png) |
| V-06 Configurações | [Abrir](conceitos/05-configuracoes-v1.png) |

O catálogo funcional mantém **17 telas e oito jornadas**, com vazio, carregamento, erro e recuperação. As seis imagens aprovam composição/identidade, não implementam as 17 telas nem comprovam capacidade. O diagrama09 foi alinhado à nova organização e continua um esquema explicativo.

## Fundação tecnológica escolhida

**Tauri → núcleo/broker local → MCP → executor Kali no WSL 2.** O provedor de IA é ligado ao coordenador pelo adaptador de API; o MCP é o canal de ferramentas, separado da interface.

Recomendação técnica para demonstrar: Tauri 2 com React/TypeScript e núcleo Rust. O frontend pede ações de domínio por IPC restrito, sem shell genérico ou acesso ao vault; núcleo e executor verificam a autoridade. Versões, bibliotecas, linguagem do executor, empacotamento e controles reais serão ensaiados nas fases. [Arquitetura](ARQUITETURA.md) · [Decisão Tauri](../docs/adr/0004-tauri-como-framework-desktop.md).

## IA, API, MCP e Kali

A API de IA é o caminho para conversar com o modelo configurado. MCP é o contrato para descobrir e chamar ferramentas. Ambos podem participar do mesmo fluxo: contexto selecionado → modelo → proposta de ferramenta → política → executor Kali → evidência → resultado paginado → modelo.

O modelo pode planejar, selecionar ferramentas, operar dentro do que já foi autorizado, acompanhar tarefas longas, interpretar resultados e preparar conclusões. Ferramentas conhecidas terão adaptadores tipados e parsers. O modo avançado permitirá ferramentas sem adaptador, shell e interação de terminal quando o ambiente/perfil puderem suportar os efeitos requeridos.

“Usar todo o Kali” significa acesso ao catálogo compatível e uma rota avançada para capacidades ainda não integradas. Não significa que todo pacote funcione em qualquer WSL, que o modelo herde root ou que receba automaticamente arquivos/credenciais do Windows. Ferramentas que exigem hardware ou outra contenção mostram o requisito real.

## Autoridade, autonomia e isolamento

Aprovação não precisa acontecer por comando: políticas prévias podem delegar classes delimitadas de operações. Plano, escopo, entradas, ambiente, efeitos e validade ficam vinculados à decisão. Um novo alvo ou mudança de autoridade exige reavaliação. Depois de iniciada a preparação, mudar o escopo cria uma nova Avaliação vinculada à anterior; não altera retrospectivamente o trabalho já registrado.

O produto terá perfis para código/offline, ativo limitado, avançado Kali, capacidades privilegiadas e manutenção. Um perfil só pode prometer uma restrição que seu executor/backend realmente consiga impor. Parsing de argumentos, um proxy ou um botão da UI não serão apresentados como proteção universal contra qualquer programa.

A distro dedicada é uma proposta de organização e gerenciamento. WSL não fornece, por si só, a contenção forte do host necessária a todo código hostil. O mapa mantém essa distinção e prevê um backend separado para capacidades que exijam esse limite, sem trocar silenciosamente o requisito Kali/WSL por uma promessa que ele não cumpre. [Fonte Microsoft e consequências de desenho](PERMISSOES-E-SEGURANCA.md).

As chaves de inferência ficam no vault host e não são herdadas pelo Kali. Fontes/evidências enviadas ao modelo seguem política de dados; operação local não significa que nada saia ao provedor. As credenciais de alvo são disponibilizadas somente quando a operação exigir, com escopo e rastreabilidade.

## Configuração e comportamento em falhas

Você poderá configurar IA/modelo, dados compartilháveis, orçamento, autonomia, alvos/perfis, conjuntos Kali, limites de processos, retenção, relatórios, interface, atualização e diagnóstico. Valores iniciais estão centralizados em [CONFIGURACOES](CONFIGURACOES.md), para não existirem regras contraditórias em cada tela.

O mapa distingue pausar a Avaliação de suspender um processo Linux. Uma pausa impede novos passos e estabiliza o que estiver ativo; cancelamento só é confirmado após reconciliação. Se a IA cair, os dados e processos não desaparecem. Se o efeito de uma execução for desconhecido, ela não é repetida automaticamente.

Manutenção de APT/pacotes é separada da avaliação e usa locks. Falha no meio de atualização leva a diagnóstico/reparo ou restauração de backup comprovado; o app não promete desfazer uma instalação usando um rollback inexistente. Reiniciar o Windows/WSL exige revalidar identidade e estados antes da retomada.

## Documentação detalhada

| Documento | Pergunta que responde |
|---|---|
| [Produto e escopo](PRODUTO-E-ESCOPO.md) | Para quem serve, modalidades, funções e fronteiras da proposta |
| [Painel e jornadas](PAINEL-E-JORNADAS.md) | Como serão as telas, ações, estados e caminhos de uso |
| [Visual e telas aprovadas](VISUAL-E-TELAS.md) | Galeria das seis telas, navegação, componentes, estados e fidelidade visual |
| [Arquitetura](ARQUITETURA.md) | Quais módulos/processos existirão e como se comunicam |
| [Kali e ambientes](KALI-E-AMBIENTES.md) | Como instalar, diagnosticar, atualizar, recuperar e preservar ambientes |
| [Ferramentas e operação](FERRAMENTAS-E-OPERACAO.md) | Como usar catálogo, adaptadores, shell, processos, parsers e quotas |
| [IA, MCP e dados](IA-MCP-E-DADOS.md) | O que a IA recebe/faz e como propostas/resultados atravessam o protocolo |
| [Permissões e segurança](PERMISSOES-E-SEGURANCA.md) | Quem autoriza, como verificar escopo, revogar e proteger dados |
| [Dados e estados](DADOS-E-ESTADOS.md) | Entidades, ciclos de vida, consistência, evidências e recuperação |
| [Configurações](CONFIGURACOES.md) | Opções, precedência e defaults a validar |
| [Decisões](DECISOES.md) | O que já foi pedido, o que recomendamos e o que continua em aberto |
| [Fases e validação](FASES-E-VALIDACAO.md) | Ordem de demonstração e 28 cenários transversais de aceitação |
| [Revisão e cobertura](REVISAO-E-COBERTURA.md) | O que foi integrado, como foi conferido e os limites da entrega |
| [Glossário](../GLOSSARY.md) | Termos canônicos para não confundir plano, operação, execução e resultado |
| [Base e premissas](referencias/BASE-E-PREMISSAS.md) | Confirmações, fontes, escopo e relação com o projeto pai |

## Dez mapas editáveis

Cada mapa tem fonte `.ofk`, documento `.openflow.json` e exportação `.svg`. O documento editável pode ser aberto no OpenFlowKit. Os diagramas explicam as ideias; tabelas e contratos detalham condições e caminhos que não cabem em uma figura.

| Mapa | Consulta | Editável / fonte |
|---|---|---|
| 01 Visão do produto | [SVG](diagramas/01-visao-produto.svg) | [OpenFlow](diagramas/01-visao-produto.openflow.json) · [DSL](diagramas/01-visao-produto.ofk) |
| 02 Arquitetura | [SVG](diagramas/02-arquitetura.svg) | [OpenFlow](diagramas/02-arquitetura.openflow.json) · [DSL](diagramas/02-arquitetura.ofk) |
| 03 Jornada de avaliação | [SVG](diagramas/03-jornada-avaliacao.svg) | [OpenFlow](diagramas/03-jornada-avaliacao.openflow.json) · [DSL](diagramas/03-jornada-avaliacao.ofk) |
| 04 Autorização | [SVG](diagramas/04-autorizacao.svg) | [OpenFlow](diagramas/04-autorizacao.openflow.json) · [DSL](diagramas/04-autorizacao.ofk) |
| 05 Dados | [SVG](diagramas/05-dados.svg) | [OpenFlow](diagramas/05-dados.openflow.json) · [DSL](diagramas/05-dados.ofk) |
| 06 Estados da avaliação | [SVG](diagramas/06-estados-avaliacao.svg) | [OpenFlow](diagramas/06-estados-avaliacao.openflow.json) · [DSL](diagramas/06-estados-avaliacao.ofk) |
| 07 Instalação Kali | [SVG](diagramas/07-instalacao-kali.svg) | [OpenFlow](diagramas/07-instalacao-kali.openflow.json) · [DSL](diagramas/07-instalacao-kali.ofk) |
| 08 Evidência e achados | [SVG](diagramas/08-evidencia.svg) | [OpenFlow](diagramas/08-evidencia.openflow.json) · [DSL](diagramas/08-evidencia.ofk) |
| 09 Painel com lista/detalhe e utilidades recolhidas | [SVG](diagramas/09-painel.svg) | [OpenFlow](diagramas/09-painel.openflow.json) · [DSL](diagramas/09-painel.ofk) |
| 10 Pacotes e manutenção | [SVG](diagramas/10-pacotes-operacao.svg) | [OpenFlow](diagramas/10-pacotes-operacao.openflow.json) · [DSL](diagramas/10-pacotes-operacao.ofk) |

## O que ainda será decidido

Versão Tauri, frontend/bibliotecas complementares e linguagem do executor; provedor/modelo inicial; mecanismo de isolamento reforçado; controles de rede/arquivos demonstráveis; distribuição/licença; assinatura/atualização; extensões e operação externa/remota. Tauri e a direção visual já foram escolhidos. O mapa propõe interfaces e critérios para que os detalhes restantes não mudem o significado das permissões ou dos resultados.

Os ADRs em `../docs/adr/` têm escopo deste desktop: 0001–0003 seguem propostos e 0004 registra a escolha aceita de Tauri. O planejamento anterior do projeto pai foi preservado; este mapa não o substitui silenciosamente. A [pesquisa do Codex](../README.md) permanece como referência de arquitetura observada, distinta do produto proposto aqui.

## Ordem recomendada depois deste mapa

Revisar a especificação → demonstrar fundamentos em laboratório → construir uma fatia completa de avaliação/evidência/relatório → ampliar catálogo e modalidades → adicionar remediação/profundidade → distribuir e estender. [Dependências e critérios](FASES-E-VALIDACAO.md).

Nesta entrega foram produzidos somente documentos, contratos propostos, diagramas e verificações desses artefatos. Nenhum WSL/Kali foi instalado ou modificado, nenhum alvo foi avaliado e nenhuma parte executável do OpenKoda foi construída.
