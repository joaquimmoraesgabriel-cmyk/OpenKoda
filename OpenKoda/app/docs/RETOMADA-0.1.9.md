# Validação da 0.1.9

O instalador x64 sem assinatura passou na extração do executável e dos cinco recursos distribuídos. [Manifesto](../releases/0.1.9-local-20261111/release.json) · [Guia de uso](GUIA-0.1.9.md)

## Instalador e abertura em perfil limpo

O instalador local tem 7.284.584 bytes e SHA-256 `e004aad8451a1d82467c96d4d7738efc5fa0378325d41061b3795587755a715d`. O ensaio instalou a 0.1.8, atualizou para esse pacote e desinstalou a 0.1.9. Conferiu o executável e os cinco recursos, além dos dados sintéticos, do registro e dos atalhos ao término. [Registro do ensaio local](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/blob/b00c083dbbfb4feab4901c7f8bcc93f34c60f136/OpenKoda/app/docs/RETOMADA-0.1.9.md)

O GitHub Actions compilou o commit `bda2bd73e5587a528b5be131f21b8e73f2802be6` no Windows Server 2025. O instalador dessa compilação tem SHA-256 `4fe0849ef9ae7b3557c1dfe1fa564d002172cebfccdf7e9d7420ec93afd610e7`. Em um perfil limpo, o teste abriu o programa instalado, criou um projeto pela interface e abriu o programa novamente. A interface mostrou o projeto salvo e o SQLite confirmou o mesmo UUID nas duas aberturas. A desinstalação preservou o banco. O recibo JSON tem SHA-256 `d38ef8367e8e92e21db15018c62d172d92ee69fa547f2d7bb1805128649de404`. [Execução aprovada e recibo](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/actions/runs/43028109086)

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

A integração com a OpenAI foi verificada no debug preservado desta versão. Em um perfil temporário, o aplicativo guardou a credencial pelo mesmo comando das configurações, consultou o catálogo oficial e recebeu 96 modelos. A chave foi removida ao final. O teste fez uma consulta e zero inferências.

## Comparação visual

As referências V-01 a V-06 foram comparadas com capturas em 1440 × 810. Os projetos, avaliações e achados usaram títulos e modalidades do laboratório ilustrado. Painel, formulário, achados e projetos preservam seleção à esquerda e detalhe à direita. O painel real inclui escopo e política de dados antes do resumo. Kali apresenta saúde, abas e catálogo com o estado desconectado da prévia. As configurações mantêm o menu por assunto, a conexão e o orçamento. Na prévia, ações que exigem um provedor ou executor aparecem indisponíveis. As seis telas couberam na largura medida. [Referências originais](../../planejamento/VISUAL-E-TELAS.md)

## Limites da entrega

Os testes nativos usaram perfis sintéticos. Estão pendentes uma instalação limpa sem WebView2, a matriz de versões Windows/WSL e falhas físicas de reboot e pouco disco. A interrupção do instalador foi simulada entre dois processos reais do NSIS. O teste de acessibilidade cobre os percursos registrados. O reteste desta versão não incluiu nova chamada paga à OpenAI.

Os estados dos critérios de aceitação e os cenários externos podem ser consultados na [matriz](ACCEPTANCE-STATUS.md) e no [roadmap](ROADMAP.md).
