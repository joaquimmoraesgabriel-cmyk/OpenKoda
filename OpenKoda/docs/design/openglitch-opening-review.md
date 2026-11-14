# OpenGlitch — abertura representativa

Preparado em 14 de novembro de 2026. **Protótipo isolado; não publicado e ainda
sem revisão visual no navegador.**

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

Para uma revisão futura, servir `prototypes/` e `dist/` como diretórios irmãos,
com as rotas públicas, incluindo `/demo-app/`, apontando para `dist/`.
Nenhum servidor de prévia novo foi aberto após a interrupção do controle de
janelas. O protótipo não está na lista de recursos publicados pelo Worker.

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

Essas verificações são estáticas e de comportamento isolado. Não comprovam a
composição final, fluidez real, frequência de frames ou experiência por toque.

## Antes de integrar às páginas existentes

1. Observar a abertura em navegador real, nos tamanhos desktop, notebook, tablet
   e celular; testar ponteiro rápido, bordas, teclado, orientação e resize.
2. Conferir aparência clara/escura, movimento reduzido, módulo indisponível,
   falha do Canvas, leitura, seleção de texto e rolagem natural.
3. Ajustar a seção representativa após essa observação; só então aplicar o
   sistema às páginas públicas, guias, preços, notícias e políticas.
4. Preservar os contratos existentes de navegação (`primary-nav`, `menu-toggle`,
   menus), âncoras, conta e sessão, preferência `koda.appearance`, cookies,
   demonstração, download e formulários. A conta clona o SVG inline da marca:
   a integração final precisa conservar esse contrato ou adaptá-lo e testá-lo.
5. Acrescentar somente os recursos aprovados à lista explícita de assets do
   Worker e executar os testes de regressão antes da publicação.

O controle do computador foi interrompido por Esc durante a preparação. A
revisão visual continua pendente até ser possível retomar essa etapa. O site
em produção conserva seu visual atual.

O domínio está preparado separadamente em
[DOMINIO-OPENGLITCH.md](../DOMINIO-OPENGLITCH.md); DNS, HTTPS e validação dos
fluxos de conta sob a nova origem continuam pendentes.
