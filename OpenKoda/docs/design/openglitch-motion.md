# OpenGlitch: matéria em movimento

Atualização de 14 de novembro de 2026. Implementação local preparada para a
validação visual integrada; este documento não afirma publicação ou aprovação
em hardware móvel real.

## Contrato visual

A abertura mantém a família de abertura modular: um portal de módulos de
grafite, com área de leitura calma e revelação violeta nas próprias marcas.
O encerramento usa grupos de pontos como composição complementar, em contraste
mais baixo. As duas cenas ocupam a largura completa de suas seções; formulário,
preços e demonstração continuam em HTML estável.

O campo agora evolui sem ponteiro. Ondas espaciais de baixa frequência deformam
a posição dos módulos e fazem a cobertura variar suavemente entre 80% e 120% da
área base. A fase espacial é determinística, sem sorteio de ruído por frame.
Não há troca abrupta de ocupação ou um disco colorido sobreposto ao ponteiro.

Cada cena mantém um relógio ativo. O ponteiro colore os módulos da abertura e
desloca os pontos do encerramento em relação à base atual; ao sair, apenas esse
estado transitório desaparece. O campo continua vivo. A margem de simulação
cobre a soma dos deslocamentos máximos, evitando uma borda vazia na interação.

## Controles e estados

- Botões **Pausar animação** e **Retomar animação** em cada cena controlam as duas
  cenas. A escolha é persistida em `openglitch.motion-paused`, com funcionamento
  local preservado se o armazenamento estiver bloqueado.
- A preferência de movimento reduzido congela a composição, desabilita o botão
  com o texto **Movimento reduzido** e não altera a escolha manual armazenada.
- Toque mantém o movimento autônomo com menor densidade. Não há interceptação
  de `touchmove` ou `wheel`; o gesto de rolagem permanece nativo.
- Cenas fora da tela e abas ocultas param o relógio e o agendamento. A retomada
  não acumula o tempo em segundo plano. Mudanças de tamanho/tema preservam o
  tempo ativo; o tamanho das células continua isotrópico.
- O renderer libera frames, observadores e listeners em `pagehide`; o retorno
  de BFCache remonta os recursos. O campo estático existe antes da inicialização
  e fica visível quando Canvas falha.

O botão tem contraste de texto de aproximadamente 5,90:1 no tema claro e
8,14:1 no escuro, pelas cores `--muted`/`--paper` existentes. Seu estado usa texto
e `aria-pressed`, além do símbolo. Arte decorativa permanece sem foco e sem
interceptar cliques. DPR é limitado a 1,5; há no máximo 2.400 marcas em desktop
e 950 em composição compacta, com redução de densidade em renderizações lentas.

## Arquivos

- `site/dist/matter-field.js`: base viva, interação, coordenador e pausa.
- `site/dist/openglitch.js`: fallback vetorial recomposto para ambas as cenas.
- `site/dist/openglitch.css`: apresentação do controle de pausa.
- `site/editorial.mjs`: dois controles nas cenas existentes.
- `site/dist/brand/openglitch-aperture-*.svg`: quatro fallbacks regenerados.
- `site/tests/matter-field.test.mjs`: comportamento e ciclo de vida.

Os SVGs usam `renderMatterSvg` do mesmo motor: desktop 1440 × 800, mobile
390 × 832; claro `#202124`/`#6942e7`, escuro `#f5f4ef`/`#b8a4ff`. São arte
procedural original, sem fontes ou bibliotecas adicionais.

## Evidência automatizada

`node --test tests/matter-field.test.mjs` aprova 16 testes. Além das verificações
anteriores, cobre quadros de 0, 3 e 8 segundos sem entrada; buffers estáveis;
retorno à base viva após ponteiro estacionário; independência de frequência de
frames; limite de delta; pausa persistente e entre abas; armazenamento bloqueado;
toque autônomo; movimento reduzido; dez ciclos de montagem/limpeza; falha e
restauração do contexto. Testes antigos que exigiam ausência de animação ambiente
e imobilidade em toque foram atualizados para o comportamento agora solicitado.

Esses testes comprovam os estados do motor, não a fluidez percebida. A revisão
integrada no navegador deve observar 10 segundos sem entrada, pointer parado,
saída rápida, pausa/retomada, teclado, leitura, fallback e tamanhos do contrato
da skill. Medição de frames deve ser identificada como amostra do navegador e
dispositivo realmente usados. Fluxos de conta, demonstração e download continuam
sob a validação do coordenador.

## Componentes de interface

Referências lidas integralmente, incluindo HTML, CSS, JS e arquivos compartilhados:
[Plan switch](https://designbookmark.com/motion-src/plan-switch/index.html),
[Copy button](https://designbookmark.com/motion-src/copy-button/index.html),
[Word roll](https://designbookmark.com/motion-src/word-roll/index.html),
[Weight wave](https://designbookmark.com/motion-src/weight-wave/index.html) e
[Fan cards](https://designbookmark.com/motion-src/fan-cards/index.html).
O código de integração é próprio. Conserva os pares de rigidez/amortecimento,
sem importar fontes remotas, beacons, branding, mensagens entre frames ou a
simulação automática de contratação das demonstrações.

- **Preços:** seletor Plus/Pro/Ultra com duas bordas de mola, rolagem vertical dos
  dígitos e tablist acessível por setas, Home e End. Valores existentes preservados:
  R$ 219, R$ 999 e R$ 1.999 por mês; cotas US$ 20/100/200. A tabela completa continua
  acessível abaixo. Sem JavaScript, todos os planos permanecem visíveis. No checkout
  já existente, o mesmo destaque acompanha os rádios nativos; não muda o envio,
  os dados nem a validação de pagamento.
- **Copiar:** snippets `pre > code` recebem botão com texto e ícone animados. A
  confirmação ocorre somente quando a escrita na área de transferência resolve;
  rejeição mostra instrução para seleção manual. Pedidos simultâneos são ignorados
  e a desmontagem descarta feedback assíncrono antigo.
- **Apresentação:** uma frase acessível estável acompanha as palavras decorativas
  perguntas/arquivos/evidências, com troca a cada 2,4 segundos. O título tem onda de
  peso de 4,6 segundos e reação ao ponteiro. Usa Segoe UI Variable quando instalada;
  outros sistemas mantêm a família local disponível, podendo variar o peso em
  passos discretos. Não há download de fonte ou dependência externa.
- **Guias:** três cards levam a páginas reais de projeto, conversa e relatório.
  O leque usa passos de 0,75 segundo e repouso de 1,4 segundo. Ponteiro e foco
  interrompem a sequência. Em área compacta os cards viram lista estável e deixam
  de agendar frames contínuos.

As animações compartilham a pausa persistida com o fundo, respeitam movimento
reduzido, suspendem atualização fora da tela e em aba oculta e liberam recursos
ao sair da página. O retorno de BFCache remonta os componentes. Nenhum fluxo de
OTP ou coleta de cartão foi ativado.

Arquivos novos: `motion-ui.css`, `motion-ui.js`, `motion-core.js`,
`motion-controls.js` e `motion-editorial.js`, incluídos na lista explícita de
assets do build. `account.js` apenas solicita o aprimoramento depois de renderizar
seu conteúdo. Seis testes novos cobrem retarget de mola, movimento reduzido,
confirmação/falha de cópia, feedback obsoleto, pausa, suspensão e limpeza. A suíte
anterior de 28 testes passou antes dessa integração; o coordenador realiza a
inspeção visual e a regressão integrada final.

## Demonstração recompilada — 14/11/2026

A entrada pública foi reconstruída pelo pipeline existente
`tsc --noEmit -p demo/tsconfig.json` e `vite build --config vite.demo.config.ts`,
depois dos ajustes finais de habilidades, progresso e resposta do app. A versão
permanece 1.1.1. Os assets finais usam `index-BwNf_Gq-.js`,
`mount-TAggl8SD.js` e `mount-CeSF5DLy.css`; ficam rastreados no Git, ao contrário
do worker e HTML das páginas gerados pelo build do site.

O teste de criação revelou uma fixture desatualizada: a conta fictícia indicava
assinatura ativa, mas serviço Koda indisponível. Em `app/demo/api.ts`, somente
`controls.inferenceEnabled` passou a representar disponibilidade simulada.
Pagamentos continuam desligados; transporte de produção, CSP e conta real não
mudaram. Esse valor libera apenas as respostas preparadas do adaptador em memória.

Validação: TypeScript e Vite aprovados; 34 testes do site aprovados; teste de
cabeçalhos repetido após a correção aprovado; os **19 E2E da demo passaram** em
Edge headless. Cobrem isolamento de origem, cookies e armazenamento, bloqueio de
rede e submissão nativa, reset, chat/cancelamento, criação de projeto, modelos,
raciocínio, histórico, configurações, anexos fictícios e larguras de 320 a 1440 px.
As evidências locais ficam em `app/.local/site-demo-motion-fixed-results`.
Nenhuma API de IA foi chamada. O aviso de bundle acima de 500 kB permanece:
o módulo principal mede aproximadamente 757 kB, ou 225 kB comprimido.

A configuração temporária dos E2E dispensa Vite porque os testes interceptam
todas as rotas; a porta de desenvolvimento já estava ocupada. Edge precisou de
execução fora da sandbox de processos após encerrar antes de abrir páginas.
Essa limitação do ambiente não alterou o sandbox do iframe testado.
