# OpenGlitch — abertura representativa

Preparado em 14 de novembro de 2026. **Revisado no navegador, integrado e
publicado na versão 16 do site.** A autorização para retomar a revisão foi
recebida após a interrupção anterior. Evidências e limites estão em
[openglitch-site-2026-11-14.md](../evidence/openglitch-site-2026-11-14.md).

## Identidade e experiência

A empresa usa OpenGlitch; o produto continua OpenKoda e o provedor da assinatura
continua Koda. O símbolo e os arquivos estáticos estão documentados em
[openglitch-brand.md](openglitch-brand.md).

A abertura usa papel suave, grafite e um acento violeta localizado. A composição
de módulos percorre a largura da seção e deixa uma área calma para a leitura.
O conteúdo apresenta o trabalho concreto: revisar arquivos autorizados,
conversar com IA e conferir evidências. A demonstração real ocupa uma seção
própria, sem distorção ou efeitos sobre a interface.

O motor possui duas composições determinísticas: abertura modular (`aperture`)
e relações entre grupos de pontos (`constellation`). Este protótipo usa apenas
a primeira. A cor e o deslocamento pertencem às próprias marcas; não há um
círculo colorido sobreposto acompanhando o ponteiro.

## Arquivos

- [Abertura HTML](../../site/prototypes/openglitch-opening.html).
- [Estilos](../../site/prototypes/openglitch-opening.css) e
  [comportamento externo](../../site/prototypes/openglitch-opening.js).
- [Motor de matéria](../../site/dist/matter-field.js).
- Quatro SVGs `openglitch-aperture-*` junto do protótipo, para os estados
  desktop/mobile e claro/escuro. Derivam da mesma composição do Canvas.

O protótipo foi servido com `prototypes/` e `dist/` como diretórios irmãos,
com as rotas públicas, incluindo `/demo-app/`, apontando para `dist/`. Depois
da revisão, a integração foi feita em `editorial.mjs`, `navigation.mjs` e
`build.mjs`, com estilos e comportamento em `dist/openglitch.css` e
`dist/openglitch.js`. Os quatro SVGs aprovados estão em `dist/brand/` e na lista
explícita de recursos do Worker; o protótipo isolado não foi publicado.

## Verificações concluídas

Os dez testes do motor aprovam determinismo, limites de quantidade e deslocamento,
retorno ao repouso, pausa fora da tela, limpeza de listeners, tratamento de
redimensionamento, alteração de tema, movimento reduzido e falha do Canvas.
A densidade e a resolução do Canvas são limitadas; há um coordenador de frames
e nenhuma animação ambiente contínua quando a interação termina.

Foram verificadas sintaxe, referências locais, IDs, HTML/SVG e geração idêntica
dos fallbacks. A demonstração conserva `sandbox="allow-scripts allow-forms"`,
sem `allow-same-origin`. O protótipo não altera as preferências persistidas do
site ou do aplicativo. O contraste mínimo calculado para seus textos é 5,54:1.

Além das verificações isoladas, a composição foi observada no navegador em
larguras de 320 a 1920 px. Foram conferidos ponteiro, menus, teclado, temas,
cookies, preços, guias e a demonstração interativa. O ajuste para celular
mantém a textura abaixo da área de leitura. A seção final também usa a largura
completa sem transformar o Canvas em uma coluna do conteúdo.

## Integração concluída e limites

Os contratos de navegação (`primary-nav`, `menu-toggle`, menus), âncoras,
preferência `koda.appearance`, cookies, demonstração e download foram
preservados. A conta continua recebendo um SVG inline da marca, necessário
para sua montagem. Em produção, a tela de entrada renderizou e o Turnstile
mostrou sucesso. Não foi realizado um novo login na conta pessoal do usuário.

A regressão automatizada aprovou 22 testes. Após o último ajuste do contêiner
da seção final, os 11 testes de links e do motor foram repetidos e aprovados.
Uma amostra local de 2,2 segundos de intervalos de frames durante a interação
registrou mediana de 8 ms, p95 de 16,1 ms e nenhum intervalo acima de 50 ms.
Isso não comprova desempenho em todos os dispositivos. Movimento reduzido e
toque são cobertos por testes do motor; faltam ensaios em hardware móvel real
e com a preferência de movimento reduzido do sistema operacional.

A imagem estática foi conferida no navegador com os módulos de animação
removidos de uma página de teste local. A página permaneceu legível, com a
composição estática. Esses arquivos de ensaio não foram publicados.

O domínio está preparado separadamente em
[DOMINIO-OPENGLITCH.md](../DOMINIO-OPENGLITCH.md); DNS, HTTPS e validação dos
fluxos de conta sob a nova origem continuam pendentes.
