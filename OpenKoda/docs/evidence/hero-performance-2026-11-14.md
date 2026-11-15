# Hero: ajuste de fluidez — 14/11/2026

Escopo: somente a apresentação inicial do site. R2, domínio de atualizações,
desktop e autenticação não foram alterados.

- Órbita desktop aproximada discretamente: raios 0,66/0,69 → 0,63/0,66;
  máscara central 0,38/0,28 → 0,36/0,27. Ajuste menor também no modo compacto.
- Painter descarta marcas transparentes e fora do canvas, mantendo simulação,
  ordem e geometria das marcas visíveis; buffers reutilizados na passagem de cor.
- Weight Wave mantém molas 260/22 e foco 150/22. Duas faces fixas por letra
  compartilham a mesma célula; opacidade interpola a aparência de peso, evitando
  refazer os glifos e suas larguras a cada frame. Trata-se de aproximação visual
  do peso, com o mesmo ritmo, sem depender de fonte variável instalada.

39 testes do site passaram. Novos testes comparam geometria, opacidade e ordem
visíveis em vários instantes, incluindo marcas parcialmente visíveis nas bordas.

Comparação local pelo navegador IAB, 1366×768, DPR aproximadamente 1, amostras
de 12 segundos após aquecimento de 1 segundo, sem interação. Instrumentação
temporária servida apenas localmente (não incluída no pacote publicado):

| Amostra | Intervalo mediano | p95 | Intervalos acima de 33,4 ms |
| --- | --- | --- | --- |
| anterior, arquivos do commit 0e13f72 | 90,9 ms | 236,3 ms | 108/116 |
| corrigida | 6,2 ms | 18,2 ms | 0/1266 |

Esses números são intervalos de requestAnimationFrame neste computador, não
tempo de GPU nem garantia de FPS em outros aparelhos. A amostra compacta
558×667 teve mediana 12 ms, p95 18,2 ms e 0/1178 intervalos acima de 33,4 ms.
Inspeção publicada também em 390×844: sem overflow horizontal; botão da hero
abriu /download/ e a navegação de retorno funcionou.

Publicado no Sites como versão 19, fonte
`d3e39c9b86801fd63443a8efdd79e3cf3bc5ecb6`, deployment
`appgdep_6af915699c788191a3c63859827022f3`. Novo markup conferido em
https://openglitch.com.br/ . Captura local: `app/.local/hero-smooth-published.png`.
