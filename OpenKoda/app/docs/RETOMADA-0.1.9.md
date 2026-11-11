# Validação local da 0.1.9

O instalador x64 sem assinatura passou na extração e no ciclo local de instalação da 0.1.8, atualização para 0.1.9 e desinstalação. Os dois percursos conferiram os hashes do executável e dos cinco recursos fornecidos. Os dados de teste e o estado de registro e atalhos foram preservados. [Manifesto](../releases/0.1.9-local-20261111/release.json) · [Guia de uso](GUIA-0.1.9.md)

## Resultados

| Percurso | Resultado observado |
| --- | --- |
| Núcleo Rust no Windows | 575 aprovados. O teste adicional opt-in passou no Kali gerido, com duas capturas e verificação posterior de liberação de espaço. |
| Interface | Build de produção, 66 testes unitários e 246 testes do Playwright aprovados. |
| Aplicativo nativo | 72 grupos em sete scripts com o binário debug e WebView2. Incluem migração de dados sintéticos da 0.1.8, backup e restauração, fechamento ocupado e ocioso, e gravação idempotente dos bytes revisados do pacote de suporte. |
| Repetição da solicitação | O teste nativo partiu de duas avaliações. A criação e três repetições, duas delas simultâneas, terminaram com três avaliações e um único recibo para a chave. As repetições devolveram o mesmo ID e revisão. Mudar o título mantendo a chave retornou `scope_idempotency_conflict`. Na regressão Rust, o recibo também foi recuperado após reabrir o SQLite enquanto o executor mantinha uma trava compartilhada. |
| Helpers | A manutenção, o executor e a restauração do Kali passaram pelas suítes de Python no Windows e no Linux. O manifesto registra os casos específicos de Linux que o Windows ignora. |
| Histórico de cópias | Paginação em blocos de 128, acesso aos recibos antigos e recuperação após interrupção nas páginas anteriores. |
| Executor recuperado | Dois registros WSL exclusivos. A troca recusou uma autorização emitida para a outra identidade antes de abrir uma execução. O diagnóstico MCP e o scan no segundo clone passaram e a captura foi selada. Ao terminar, o inventário de registros e a distro padrão voltaram ao estado inicial. |
| Pré-requisito de análise | Quando o ripgrep falta, a fila informa o pré-requisito e bloqueia o scan antes do envio da evidência. O programa de verificação também rodou no Kali gerido como usuário comum. |
| Recuperação da instalação | 21 regressões de recuperação. O ensaio com os instaladores 0.1.7 e 0.1.8 confirmou reparo, recusa do retorno com dados alterados e retomada entre desinstalar o binário novo e reinstalar o anterior. Os seis processos NSIS terminaram com código zero. |
| Acessibilidade | Cinco percursos com teclado e zoom nativo de 200% no Edge. No debug intermediário 0.1.9, o NVDA 2026.3 apresentou a prévia, a confirmação, a disponibilidade do botão e o anúncio de sucesso da gravação de um arquivo de suporte de 267 bytes. O teste usou o Visualizador da Fala com sintetizador silencioso. |

## Comparação visual

As referências V-01 a V-06 foram comparadas com capturas em 1440 × 810. Os projetos, avaliações e achados usaram títulos e modalidades do laboratório ilustrado. Painel, formulário, achados e projetos preservam seleção à esquerda e detalhe à direita. O painel real inclui escopo e política de dados antes do resumo. Kali apresenta saúde, abas e catálogo com o estado desconectado da prévia. As configurações mantêm o menu por assunto, a conexão e o orçamento. Na prévia, ações que exigem um provedor ou executor aparecem indisponíveis. As seis telas couberam na largura medida. [Referências originais](../../planejamento/VISUAL-E-TELAS.md)

## Limites da entrega

Os testes nativos usaram perfis sintéticos. O executável de produção instalado ainda precisa ser aberto e reaberto em outra conta Windows ou VM. Também estão pendentes uma instalação limpa sem WebView2, a matriz de versões Windows/WSL e falhas físicas de reboot e pouco disco. A interrupção do instalador foi simulada entre dois processos reais do NSIS. O teste de acessibilidade cobre os percursos registrados. O reteste desta versão não incluiu nova chamada paga à OpenAI.

O instalador tem 7,284,584 bytes e SHA-256 `e004aad8451a1d82467c96d4d7738efc5fa0378325d41061b3795587755a715d`. Os estados dos critérios de aceitação e os cenários externos podem ser consultados na [matriz](ACCEPTANCE-STATUS.md) e no [roadmap](ROADMAP.md).
