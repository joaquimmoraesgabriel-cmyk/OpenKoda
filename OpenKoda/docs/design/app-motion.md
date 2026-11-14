# Movimento da interface do OpenKoda

## Contrato

Manter a composição do aplicativo: navegação fina, lista de projetos ajustável, conversa no centro. Movimento deve explicar uma mudança real de estado, sem simular execução de IA, alterar permissões ou atrasar a chegada do texto. Usar cores, fontes, raios e preferências de aparência já presentes no aplicativo.

- Habilidades: painel compacto sobre o compositor, linhas com ícone, nome, função e origem. `/` filtra o catálogo autorizado da conta; menu permite busca; setas, Enter e Escape funcionam sem enviar o comando como uma mensagem paga. Detalhes continuam mostrando conteúdo, origem e licença do catálogo.
- Resposta: revelar somente palavras recém-recebidas. Não acrescentar caracteres por um relógio artificial. Markdown continua limitado a nós de texto escapados, com os blocos de código existentes.
- Etapas: apresentar o estado da requisição. Eventos da avaliação ficam explicitamente separados como histórico recente da conversa, pois o contrato atual não os vincula à requisição corrente. Não inventar tarefas concluídas ou percentuais.
- Aprovação: apresentar claramente estado, alvo e efeitos; preservar o vínculo e os botões da aprovação real.
- Planos: seletor Plus/Pro/Ultra para comparar as opções recebidas pelo servidor. Alterar a comparação não muda assinatura nem inicia cobrança. Todos os cartões e seus limites continuam disponíveis.
- Copiar: confirmar somente depois de a área de transferência aceitar a escrita. Falha mantém a instrução de cópia manual.
- Lista de projetos: recolher preserva os projetos e conversas montados, mas impede foco e interação no conteúdo oculto. Largura ajustada continua sendo uma preferência local.
- OTP, tags e cartão: componentes preparados, sem criar novos fluxos. OTP recebe o resultado da autenticação de fora; completar seis dígitos não confirma o código. A prévia de cartão não aceita números nem efetua pagamento. Não existe hoje um campo de regras em lista livre apropriado para ligar tags sem mudar o domínio.

## Referências e implementação

As fontes indicadas pelo usuário foram lidas em 14/11/2026 antes da implementação. A implementação usa React, CSS e Web Animations já disponíveis, sem dependência nova. Os scripts demonstrativos/autoplay das referências não foram incorporados.

- [Design Bookmark: arquivos compartilhados](https://designbookmark.com/motion-src/shared/tokens.css): soft 260/24, lead 620/36, trail 190/19, pop 520/16 e fade 420/32 (rigidez/amortecimento).
- [Plan switch](https://designbookmark.com/motion-src/plan-switch/index.html): duas extremidades do marcador com lead/trail.
- [Copy button](https://designbookmark.com/motion-src/copy-button/index.html): rótulo 260/20, ícone 420/18, largura 300/24, retorno após 1,8 s.
- [Streaming reply](https://designbookmark.com/motion-src/streaming-reply/index.html): entrada de palavras 340/26; conteúdo e ritmo vêm da resposta real.
- [Sidebar collapse](https://designbookmark.com/motion-src/sidebar-collapse/index.html): largura 260/24.
- [OTP code](https://designbookmark.com/motion-src/otp-code/index.html): dígitos 520/16, resultado controlado externamente.
- [Tag input](https://designbookmark.com/motion-src/tag-input/index.html): entrada 420/16, valores controlados pelo formulário.
- [Kokonut AI Loading](https://kokonutui.com/docs/ai/ai-loading), [Tool Approval](https://kokonutui.com/docs/ai/ai-tool-approval), [Card Field](https://kokonutui.com/docs/inputs/card-field): referências MIT, adaptadas ao estilo existente; entradas de 0,2–0,36 s, curva de aprovação/etapas `(0.23, 1, 0.32, 1)`.

`motion/spring.ts` amostra as molas em no máximo dois segundos. Não há loop global permanente. Preferências de movimento reduzido do sistema ou do aplicativo cancelam inclusive animações em andamento. O texto e os controles permanecem disponíveis sem animação.

## Ensaio local

Com o Vite em execução, `/.local/motion-qa.html` abre componentes reais com dados sintéticos, sem API externa. Os arquivos `.local` são ignorados pelo Git e não entram no instalador. A ponte do ensaio permite apenas leitura do catálogo sintético e da conta sintética; demais ações falham explicitamente.

Word roll, weight wave, fan cards e fundos generativos pertencem à reformulação do site. O aplicativo mantém leitura estável em conteúdo de trabalho.

## Verificação

O teste `motion.test.tsx` confere estabilidade das molas, texto recebido escapado, ausência de confirmação automática de OTP e preparação de pagamento sem coleta. A verificação visual final deve cobrir habilidades por teclado e clique, retorno do foco, temas claro/escuro, movimento reduzido, resposta longa, rolagem, copiar com sucesso/falha, troca rápida de planos e recolhimento da lista.

Os E2E de habilidades também cobrem composição IME, Shift+Enter, modificadores e busca sem resultado, sem despacho pago. O E2E de streaming verifica uma URL contínua de 500 caracteres e a separação entre histórico da conversa e estado atual.
