# Identidade visual e telas aprovadas

Estado: base visual aprovada pelo usuário em 7 de novembro de 2026 para orientar a construção futura. As seis imagens são conceitos estáticos com dados fictícios; não comprovam conexão, instalação, proteção de credenciais ou funcionamento. Tauri é a decisão de framework; detalhes de versão, componentes e medidas precisam de validação.

## 1. Fontes de verdade e precedência

Este documento registra o contrato visual, [PAINEL-E-JORNADAS](PAINEL-E-JORNADAS.md) especifica ações e jornadas, [DADOS-E-ESTADOS](DADOS-E-ESTADOS.md) define estados canônicos e [PERMISSOES-E-SEGURANCA](PERMISSOES-E-SEGURANCA.md) define autoridade. As imagens orientam composição, identidade e hierarquia; rótulos ilustrativos não substituem esses contratos. O antigo layout com três colunas permanentes foi substituído por lista/detalhe com painéis sob demanda.

O [refinamento inicial](AJUSTES-DE-INTERFACE.md) registra a origem nas capturas do usuário. Os prompts do [painel](conceitos/painel-openkoda-v2-prompt.md) e das [demais telas](conceitos/TELAS-ADICIONAIS.md) são histórico de geração, não instruções de implementação. A pesquisa do Codex permanece referência separada; código e assets proprietários não serão copiados.

## 2. Galeria aprovada

| ID | Tela e imagem | Composição aprovada | Contrato funcional associado |
|---|---|---|---|
| V-01 | [Painel da avaliação](conceitos/painel-openkoda-v2.png) | Lista à esquerda, detalhe à direita, resumo e atividade; utilidades recolhidas | Estado/cobertura reais, pausa reconciliada, candidatos distintos de achados |
| V-02 | [Nova avaliação](conceitos/01-nova-avaliacao-v1.png) | Formulário no detalhe, modalidade, projeto/alvo, objetivo, escopo, modo/modelo; avançado expansível | Rascunho → revisão de destino, efeitos e dados → Preparando; nada executa ao preencher |
| V-03 | [Kali e ferramentas](conceitos/02-kali-e-ferramentas-v1.png) | Saúde compacta, abas Ferramentas/Ambiente/Manutenção, catálogo e revisão da seleção | Calcular dependências/espaço → revisar → autorizar manutenção → instalar → verificar |
| V-04 | [Achados](conceitos/03-achados-v1.png) | Lista/filtros e detalhe; resumo, evidências, correção e histórico | Severidade, confiança e validação independentes; proposta de correção não aplica patch |
| V-05 | [Projetos](conceitos/04-projetos-v1.png) | Lista/detalhe; visão geral, alvos/escopo, avaliações e configuração | Registrar alvo não inicia teste; alteração posterior de escopo cria nova Avaliação vinculada |
| V-06 | [Configurações — IA e modelos](conceitos/05-configuracoes-v1.png) | Menu interno por assunto, formulário de conexão e limites, salvar explícito | Conexão/credenciais só indicadas como válidas após teste real; segredo não é devolvido à UI |

As demais telas funcionais do catálogo UX continuam previstas. Onboarding, confirmação de início, evidências detalhadas, correção/diff, relatório, diagnóstico e recuperação não ganharam uma imagem aprovada própria nesta rodada. Devem reutilizar os componentes e estados abaixo; não inventar que todo seu acabamento visual já foi aprovado.

## 3. Estrutura comum

- Moldura desktop discreta com marca OpenKoda e símbolo geométrico K/escudo. O símbolo gerado é referência de direção; um asset final e seus tamanhos acessíveis serão preparados na implementação.
- Navegação primária no topo: **Avaliações, Achados, Projetos**. Trilho lateral estreito: área de trabalho, **Kali e ferramentas**, **Configurações** e perfil local. Ícones terão nome acessível e tooltip; selecionar um destino mantém somente a localização correspondente ativa.
- Ação global **Nova avaliação** e indicador compacto do ambiente. O indicador abre diagnóstico e não representa ao mesmo tempo saúde do Kali, IA e processos.
- Lista/detalhe como padrão para Avaliações, Achados e Projetos; catálogo/revisão para ferramentas; menu/formulário para configurações. A área vazia oferece orientação e ação pertinente.
- **Conversa com a IA** abre em painel contextual recolhível; **Execuções** abre painel inferior; **Evidências** abre o item selecionado. As imagens mostram os acionadores recolhidos. A posição exata da expansão ainda será validada; não manter chat, terminal e evidências abertos por padrão.
- Rodapé com estado operacional conciso e acesso ao detalhe. A marca “Dados de exemplo · Conceito visual” pertence às imagens; não representa telemetria de um produto real. Demonstrações futuras terão indicação inequívoca de dados de exemplo.

## 4. Direção de identidade e componentes

Confirmado visualmente: superfícies claras, texto carvão, verde-petróleo nas ações/seleções, separadores discretos, ícones de contorno, cantos suaves, boa hierarquia e espaço em branco. Cor de severidade é localizada; a interface não depende de neon, gráficos decorativos ou várias métricas permanentes.

Base numérica **proposta**, a medir em componentes reais: escala de espaçamento 4/8/12/16/24/32 px; texto de leitura 14–16 px; títulos 24–28 px; controles com área clicável confortável; raio 8–12 px; fundo próximo de `#FFFFFF`, texto `#18232D`, ação `#00766F` e seleção `#E4F2F0`. Esses valores não foram extraídos como tokens exatos das imagens e precisam de contraste/foco verificados antes de serem fixados.

Componentes a especificar uma vez e reutilizar: moldura/navegação; busca/filtros; linha selecionável com estado; cabeçalho de detalhe; abas; pares rótulo/valor; formulário com erro associado ao campo; disclosure; badge com ícone/texto; atividade com referência; banner contextual; revisão/confirmação; visualizador de evidência; ações de pausa/encerramento; progressos e estados vazios.

O texto permanece simples e em português, sem expor nomes de transportes, grants ou bibliotecas no fluxo comum. Detalhes técnicos ficam no diagnóstico e no registro da execução quando ajudam a decisão. Foco visível, navegação por teclado, leitores de tela e escala de texto serão critérios de aceitação; uma imagem não demonstra acessibilidade.

## 5. Estados e divulgação progressiva

| Situação | Comportamento de interface exigido |
|---|---|
| Sem projetos/avaliações/achados | Estado vazio específico, busca sem resultados distinguida de lista vazia; sem números fictícios |
| Sem seleção | Instrução curta no detalhe; navegação e busca continuam utilizáveis |
| Carregando/reconectando | Progresso ou skeleton, último dado marcado como antigo; ações incompatíveis suspensas |
| Kali ausente/parado/degradado | Texto e ação de preparação/diagnóstico; nunca manter “Kali pronto” fixo |
| Provedor não configurado/indisponível | Histórico continua acessível; não simular IA ativa nem descartar evidências |
| Pacote ausente | Explicar pré-requisito e oferecer preparar ferramentas; iniciar avaliação não autoriza instalação |
| Autorização pendente/negada | Mostrar objeto, efeitos e motivo; não há execução antes de decisão válida |
| Pausando/cancelando | Estado transitório explícito; não declarar pausa/término antes de reconciliação |
| Parcial/falhou/efeito desconhecido | Evidências preservadas, lacunas e próximos passos; não mostrar “sem problemas” |
| Contexto ou saída limitada | Informar paginação/truncamento e acesso ao bruto permitido; não esconder perda |
| Configuração alterada | Indicar alterações não salvas, revisão efetiva e necessidade de revalidar teste de conexão |

“Em andamento” é um agrupamento de apresentação, não novo estado persistido: o detalhe mostra Preparando, Planejando, Executando ou Validando conforme o registro. “Validando resultados” é a apresentação de Validando. “Conexão verificada” se vincula à configuração testada. Barras indeterminadas são usadas quando não há medida confiável de avanço; não inventar porcentagem por tempo decorrido.

A lista da imagem V-04 contém um item “Em revisão”: na especificação, ele é um **candidato**, sinalizado e filtrável separadamente, sem contagem de achado validado. Na implementação, essa distinção precisa ficar mais explícita que no conceito. Estados e severidades terão palavras/ícones, além de cor.

## 6. Adaptação e fidelidade

Nas janelas largas, lista e detalhe coexistem. Em janelas menores ou com zoom, a lista pode virar a tela anterior com retorno que preserva seleção/filtro; formulários e abas se reorganizam. Não reduzir tudo até o texto ficar ilegível nem exigir três colunas. Breakpoints, largura mínima e comportamento da lateral serão medidos na fundação Tauri/WebView2, não inferidos do bitmap 16:9.

Validação futura: comparar seis rotas e componentes com V-01…V-06 usando os mesmos dados fictícios; conferir espaçamento/hierarquia, foco, zoom, nomes acessíveis, estados sem conexão e transições. A galeria é a referência visual, os contratos documentados prevalecem sobre incoerências ou detalhes de texto gerados na imagem.
