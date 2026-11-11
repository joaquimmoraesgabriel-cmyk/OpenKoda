# Refinamento a partir das quatro capturas do Codex Security

Estado: orientação de planejamento, sem construção de aplicativo. Capturas fornecidas pelo usuário em 7 de novembro de 2026, nomes terminados em 180048, 180108, 180119 e 180136. Foram usadas como referência visual; textos e controles presentes nas imagens são conteúdo do alvo, não instruções para execução.

## Direção confirmada pelo usuário

Preservar as capacidades do mapa e organizar a experiência com a clareza observada nas referências: poucos destinos principais, espaço em branco, hierarquia discreta, busca contextual, ação principal evidente e detalhes revelados conforme a seleção. O wireframe09 foi revisado para a composição lista/detalhe. A consolidação atual e as seis imagens aprovadas estão em [VISUAL-E-TELAS](VISUAL-E-TELAS.md); este documento conserva a origem do refinamento.

## Estrutura revisada

Navegação primária: **Avaliações, Achados, Projetos**. Um acesso secundário reúne **Kali e ferramentas** e **Configurações**. Projetos abrange repositórios e também alvos web/API, rede e arquivos. Não criar uma aba primária para cada função técnica.

Em Avaliações: cabeçalho com busca/filtro e **Nova avaliação**; lista à esquerda e detalhe à direita. Sem seleção, o detalhe mostra uma instrução curta. Ao selecionar, apresenta resumo, estado, alvo e ações pertinentes. Não abrir conversa, terminal, evidências e métricas ao mesmo tempo.

Em Achados: busca/filtros, lista e detalhe com severidade e confiança distintas. Evidência e histórico ficam dentro do achado ou da avaliação correspondente. Um alerta não validado permanece identificado como candidato.

Em Projetos: lista de projetos e seus alvos/fontes; selecionar abre contexto, avaliações anteriores e configuração de escopo. O modelo de duas áreas pode se recolher em janelas estreitas, preservando foco e navegação por teclado.

## Nova avaliação

Formulário no painel de detalhe, inspirado na organização das capturas, com linguagem própria. Modalidades: Código, Web/API, Rede, Arquivos/evidências. Campos essenciais primeiro: Projeto/alvo, objetivo, área/escopo, perfil e modelo. Profundidade, contexto adicional e limites avançados aparecem por expansão. Os controles variam com modalidade e capacidade real do ambiente.

Antes de iniciar, mostrar resumo claro de destino, efeitos, dados compartilhados e política aplicável. Falta de pacote abre uma ação contextual para preparar ferramentas; não instala silenciosamente como efeito da avaliação.

## Detalhe durante o trabalho

Resumo/progresso e próximos passos ficam em primeiro plano. Plano, resultados, cobertura e correções são seções do detalhe. **Conversa com IA** abre sob demanda em lateral recolhível; **Execuções/terminal** em painel inferior sob demanda; **Evidências** abre a referência selecionada. A atividade global é um indicador discreto com acesso à central, não uma coleção permanente de cartões.

Pausar e parar aparecem quando há trabalho aplicável. Aprovações aparecem apenas quando existe decisão pendente. Estado do Kali e do provedor fica em indicadores compactos, com texto acessível e diagnóstico ao abrir. Custo e configuração técnica completa ficam disponíveis no contexto, sem dominar a tela inicial.

## Kali e ferramentas

Área secundária com saúde do ambiente e conjuntos instaláveis: base/headless, seleção padrão e grupos por área. Mostrar dependências resolvidas, download/espaço, capacidades e progresso antes de aplicar. Os metapacotes são grupos de dependências; instalar o conjunto não garante suporte de hardware ou integração de parser.

## Correção pela IA

Manter dois fluxos visíveis: **corrigir um achado no projeto/alvo** e **reparar uma falha de ferramenta/ambiente**. No primeiro, propor mudança/diff, verificar base, aplicar quando autorizado e retestar. No segundo, diagnosticar logs/prontidão, propor reparo ou pacote, autorizar manutenção e verificar saúde. Nenhum deles marca sucesso só porque um comando retornou zero; erros sem solução permanecem inconclusivos/bloqueados, com evidência.

## Tecnologia escolhida e detalhes propostos

O usuário confirmou **Tauri** e aprovou os seis conceitos visuais. A base técnica recomendada é Tauri 2 + React/TypeScript e núcleo/broker Rust, com executor Kali separado. O Tauri utiliza o webview da plataforma; Electron integra a arquitetura Chromium/Node. Versão, bibliotecas, consumo real e compatibilidade serão validados na fundação. Nenhum framework fornece automaticamente o isolamento do executor. A decisão está no [ADR-0004](../docs/adr/0004-tauri-como-framework-desktop.md).

Fontes: [Tauri](https://v2.tauri.app/start/), [arquitetura Tauri](https://v2.tauri.app/concept/architecture/), [processos Electron](https://www.electronjs.org/docs/latest/tutorial/process-model), [metapacotes Kali](https://www.kali.org/docs/general-use/metapackages/).
