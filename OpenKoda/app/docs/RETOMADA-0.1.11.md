# Validação da versão local 0.1.11

Em **Projetos → Avaliações**, é possível comparar duas avaliações do mesmo projeto. O núcleo verifica a qual projeto ambas pertencem e consulta suas identidades de ambiente no histórico completo do Kali. Em **Configurações → Geral → Internet**, o aplicativo mostra o indicador do Windows e a hora da consulta. [Guia da versão](GUIA-0.1.11.md)

## Instalador e dados

O instalador local x64 sem assinatura tem 7,319,137 bytes e SHA-256 `b47f3fb5095dc76c94e4514e03d5cf7e176de5b6012448790290421b5165fb55`. O ensaio conferiu o executável e os cinco recursos distribuídos, instalou a 0.1.10, atualizou para a 0.1.11 e desinstalou. O banco sintético foi preservado. Registro e atalhos foram conferidos ao terminar. O teste entre executáveis preservou os dados, o esquema 8 do núcleo e o esquema 2 do ambiente.

Os runners Windows Server 2022 e 2025 recompilaram a revisão `0c4594201f49b55a5cc14985d9f0afdf30b25be5`. Cada um instalou seu pacote, abriu o aplicativo, criou um projeto pela interface e reencontrou o mesmo UUID ao reabrir o programa. A desinstalação preservou o SQLite. [Resultado das duas instalações](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/actions/runs/43034524778)

No Windows 11 Enterprise ARM64, o instalador x64 do CI abriu o aplicativo por emulação. O projeto conservou o UUID entre duas aberturas, e a desinstalação preservou o banco. O WebView2 156.0.4314.58 já estava presente. A árvore dos fontes do aplicativo é idêntica à usada no pacote local. [Ensaio no Windows 11](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/actions/runs/43036089162)

O teste nativo geral abriu a página Projetos antes de passar dos painéis de avaliação e retenção aos ensaios exclusivos de orçamento e backup. Essa sequência permite terminar as consultas periódicas iniciadas pela interface. O roteiro executado está no commit `3b6246a7adb2bc3a8adb81145b7e4d0b327dd43e`. Os executáveis correspondem à revisão de produção indicada no manifesto.

## Testes de regressão

| Percurso | Resultado |
| --- | --- |
| Derivação de avaliação combinada | Uma criação, uma repetição sequencial e duas simultâneas deixaram uma avaliação nova e um recibo. A mesma chave com conteúdo alterado foi recusada. |
| Comparação | O ensaio entre duas avaliações do mesmo projeto preservou o estado do núcleo, recusou o outro projeto e deixou a fila intacta. O teste Rust incluiu registros anteriores aos 50 mais recentes. |
| Conectividade | A interface consultou o Windows pelo IPC, exibiu a hora e o resultado, e recusou parâmetros adicionais. A consulta deixou os dados do provedor e do executor independentes. |
| Núcleo Windows | 587 testes passaram na suíte padrão. Os dois testes opt-in passaram separadamente, um consultando o Windows e o outro produzindo e removendo duas capturas no Kali gerido. |
| Interface | 66 testes unitários, 254 testes no Edge e 76 grupos em sete ensaios nativos. |
| Auxiliares | 72 testes executados no Windows e 123 no Linux. |
| Leitor de tela | NVDA 2026.3 portátil apresentou sete anúncios nos dois novos percursos. O leitor anunciou o resultado da consulta da Internet e distinguiu alvos iguais de métodos diferentes nas avaliações sintéticas. |

Em um contêiner Kali descartável, o APT recebeu ENOSPC ao escrever os índices em um volume de 2 MiB. O OpenKoda registrou o resultado como desconhecido. A reconciliação executou `dpkg --audit`, preservou a falha registrada e fez zero novas chamadas ao APT ou ao repositório. [Ensaio com o volume cheio](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/actions/runs/43033604838)

Os dados do clone WSL descartável, do teste de autorização por janela e da consulta real ao catálogo da OpenAI constam dos registros anteriores. [Validação 0.1.10](RETOMADA-0.1.10.md) · [Validação 0.1.9](RETOMADA-0.1.9.md)

O desligamento físico durante uma atualização e a preparação de um Windows cliente x64 inicialmente sem WebView2 permanecem sem comprovação de ponta a ponta. Os percursos de leitor de tela e zoom têm cobertura amostral. [Matriz de aceitação](ACCEPTANCE-STATUS.md)

Na 0.1.11, a tela **Nova avaliação → Um alvo** ainda envia a criação sem chave de idempotência. Se o núcleo gravar e a atualização da tela falhar, uma nova confirmação pode criar uma segunda avaliação. O recibo de derivação combinada cobre outro comando.
