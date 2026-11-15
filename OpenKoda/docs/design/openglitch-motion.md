# OpenGlitch / OpenKoda — contrato de movimento do site

Revisão de 14 de novembro de 2026. Este documento descreve a implementação local.
A publicação e a avaliação visual no navegador pertencem ao coordenador; testes
matemáticos não comprovam fluidez percebida ou compatibilidade com celulares reais.

## Abertura e composição

A abertura mostra **OpenKoda** centralizado em escala grande, o subtítulo literal
**Revise código. Converse com IA.** e uma única ação: **Baixar para Windows**.
A faixa de edição, o eyebrow, o segundo CTA e o texto auxiliar foram retirados.
Depois da abertura, o texto centralizado é:

> Abra um projeto, investigue uma pergunta e confira as evidências no código. Um espaço de trabalho para seguir a ideia até o arquivo.

A demonstração real continua abaixo, em seu iframe isolado. As seções de produto,
conexões, uso, guias, perguntas e notícias continuam presentes. Linhas horizontais
editoriais foram retiradas; bordas de controles, tabelas, painéis de navegação e da
demonstração permanecem para indicar estrutura funcional.

## Matéria orbital

A composição principal é uma órbita oval de quadrados de grafite, com acentos
violeta discretos. Cada marca possui fase de nascimento e posição angular
estáveis, determinadas na montagem. O campo cobre toda a seção, inclusive além
das bordas; as células mantêm dimensões iguais nos eixos.

- O tempo ativo avança continuamente sem ponteiro. A rotação é horária: 0,19 rad/s
  mais a contribuição da espiral. O ciclo radial avança 0,021 por segundo.
- Marcas surgem gradualmente no exterior, espiralam para dentro e desaparecem
  antes de reiniciar o ciclo. A emenda entre fim e começo ocorre com opacidade zero.
- Uma região central vazia, calculada no próprio campo, protege título, subtítulo
  e ação. Essa região é verificada novamente após o deslocamento por interação;
  não há uma caixa opaca cobrindo a arte.
- A gravidade local atrai os quadrados próximos e acrescenta movimento tangencial
  horário. A velocidade do ponteiro transmite impulso somente dentro de sua área
  de influência. Os deslocamentos são limitados a 68 px no desktop e 44 px compacto.
- Ao parar, a influência dissolve material próximo por um momento e perde força
  entre 1,2 e 3,6 segundos de imobilidade. Ao sair, as marcas retornam à base viva,
  não ao quadro inicial. Não há acúmulo, seguidores globais ou rastro ilimitado.
- O encerramento preserva a família complementar de pontos, mais calma e de
  contraste menor. Não compete com a nova abertura.

O motor limita DPR a 1,5 e quantidade a 2.400/950 marcas. Não aloca partículas
nem objetos por marca durante os frames. Cenas ocultas e fora da tela suspendem
trabalho; retorno, resize e troca de tema preservam o tempo ativo, com delta
limitado. Limpeza é idempotente e BFCache remonta os recursos.

## Tipografia e referências

Foram lidos integralmente os HTML, CSS e JS públicos das três referências e os
arquivos compartilhados `tokens.css`, `base.css` e `motion.js`:

- [Weight Wave](https://designbookmark.com/motion-src/weight-wave/index.html):
  letras com mola 260/22 e foco com mola 150/22; ciclo autônomo de 4,6 segundos.
  O subtítulo usa a fonte local do projeto e mantém contraste opaco, sem o
  esmaecimento da referência. A interpolação de peso depende de fonte variável
  disponível no sistema; não é feito download de fonte.
- [Scramble Text](https://designbookmark.com/motion-src/scramble-text/index.html):
  passo de glifo de 55 ms, assentamento sequencial de 35 ms, início de 180 ms e
  variação determinística de até 280 ms. O parágrafo decodifica uma vez ao entrar
  na tela; depois fica estável para leitura. Palavras mantêm sua geometria durante
  a transformação. A sentença original fica integral na árvore acessível.
- [Goo Morph](https://designbookmark.com/motion-src/goo-morph/index.html):
  fusão de 1,05 s, repouso de 1,6 s, curva suave e filtro alfa durante a fusão.
  Aplicado discretamente ao título dos guias, alternando pergunta/evidência/ideia;
  a versão acessível do título é estável e inclui essas três palavras.

A integração é própria e usa os tokens do produto. Não incorpora fontes remotas,
beacons, temas, mensagens entre frames, branding ou textos promocionais das
referências. Os componentes anteriores de planos, cópia, Word Roll e Fan Cards
continuam nos seus lugares, com seus parâmetros de mola preservados.

## Movimento reduzido e conteúdo acessível

Por solicitação explícita do usuário, todos os botões e textos de pausar animação
foram removidos do site. A preferência antiga de pausa não congela cenas públicas
que já não possuem esse controle. `prefers-reduced-motion` continua respeitado
antes da montagem e quando muda: o campo fica estático, a decodificação mostra a
frase final e o título Goo mantém uma palavra legível.

O HTML contém os textos reais antes de carregar JavaScript. Fragmentos visuais
animados são `aria-hidden`; a cópia acessível não muda por frame. Arte não recebe
foco, não intercepta cliques, não bloqueia seleção e não intercepta rolagem móvel.
Não é afirmada certificação geral de acessibilidade.

## Fluxos preservados

Login, conta, preços e checkout não foram alterados nesta revisão. A versão de
download continua 1.1.1. A página explica que o instalador ainda não possui
assinatura de editor Authenticode reconhecida pelo Windows e pode gerar aviso;
a assinatura própria do atualizador continua presente e não é confundida com o
certificado do sistema. Não há orientação para desativar proteções.

Não foram alterados domínio, R2, dados, APIs, backend ou a fixture da demonstração
nesta revisão visual. O iframe continua `allow-scripts allow-forms`, sem
`allow-same-origin`, com CSP bloqueando rede e envio nativo de formulários.

## Arquivos e validação

- `site/dist/orbit-field.js`: composição orbital, gravidade, impulso e dissolução.
- `site/dist/matter-field.js`: integração do campo e ciclo de vida existente.
- `site/dist/motion-type.js`: Scramble e Goo; `motion-editorial.js`: Wave acessível.
- `site/dist/openglitch.css`, `motion-ui.css`: hierarquia, composição e tipografia.
- `site/editorial.mjs`: abertura, introdução, guias e aviso do download.
- `site/build.mjs`: assets novos explicitamente registrados.
- `site/dist/brand/openglitch-aperture-*.svg`: quatro alternativas estáticas,
  geradas pelo mesmo motor para desktop/mobile claro/escuro.

25 testes direcionados aprovaram: t=0/3/8 sem entrada, buffers, sentido horário,
espiral para dentro, fade na emenda do ciclo, gravidade local, retorno após mouse
parado, limites de deslocamento, região central vazia, frequência de frames,
suspensão, redução de movimento, preferência legada, dez montagens/limpezas,
fallback, seleção de planos e confirmação de cópia. Sintaxe JS, parse de CSS e
`git diff --check` também passaram. A suíte completa do site aprovou **37 testes**,
incluindo rotas, assets, download, isolamento da demo e proteções da sessão. O
coordenador observou e aprovou as primeiras capturas em 1280 × 720, claro e
escuro: título/subtítulo/CTA legíveis, centro vazio e órbita reconhecível. A revisão
móvel e a observação temporal continuam sob sua validação; não há medição de FPS
ou alegação de teste em dispositivo móvel físico.

Na etapa anterior, após recompilar a demo com o frontend atualizado e corrigir
sua disponibilidade fictícia, passaram 19 E2E de isolamento/fluxos da demo e 34
testes do site. Essas evidências ficam em `app/.local/site-demo-motion-fixed-results`
e não substituem a revisão visual da abertura atual. Nenhuma API de IA foi chamada.
