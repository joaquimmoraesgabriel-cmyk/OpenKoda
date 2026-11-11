# Validação local do OpenKoda 0.1.12

Em **Nova avaliação → Um alvo**, repetir um envio após perder a resposta recuperou a avaliação já gravada. O teste passou tanto para um rascunho quanto para uma avaliação com escopo revisado. [Guia de uso](GUIA-0.1.12.md)

| Caminho testado | Resultado |
| --- | --- |
| Salvar rascunho | Uma avaliação e um recibo para a solicitação. Depois da perda da resposta, 2 reenvios simultâneos devolveram o mesmo ID. O registro foi recuperado após a reabertura. |
| Salvar com escopo revisado | Uma avaliação e um recibo. A reabertura devolveu o mesmo ID. Repetir a chave com conteúdo diferente retornou conflito. Uma nova solicitação permitiu criar outra avaliação. |
| Reenviar pela interface do Windows | No rascunho, o ensaio descartou a resposta da criação depois que o SQLite gravou a avaliação. No escopo revisado, descartou a atualização da tela que já continha o novo ID. Nos dois percursos, clicar em **Reenviar a mesma solicitação** recuperou o mesmo ID. O banco manteve uma avaliação e um recibo por solicitação. Os planos e as execuções permaneceram iguais. [Recibo do WebView2](../releases/0.1.12-local-20261111/receipts/singleAssessmentNativeUi.json) |
| Derivar avaliação combinada | Uma avaliação nova e um recibo após um reenvio sequencial e 2 simultâneos. |
| Atualizar o instalador | A atualização local da 0.1.11 para a 0.1.12 passou pela extração, instalação e desinstalação. Os dados sintéticos foram preservados. O teste entre executáveis preservou o esquema 8 do núcleo e o esquema 2 do ambiente. |

O instalador local x64 sem assinatura tem 7,312,298 bytes e SHA-256 `35d6709e946039529c4b803f97d746438f3d43e97b10c09674bf848bcb3a6456`. O manifesto local registra os hashes do ZIP de fontes, dos executáveis e dos recibos. [Manifesto](../releases/0.1.12-local-20261111/release.json) · [Revisão de produção `2977d47`](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/commit/2977d47f7dc22aaaf282b693da033e960e932395)

O CI compilou seus próprios instaladores. No Windows Server 2022, no Windows Server 2025 e no Windows 11 ARM64, o aplicativo instalado reencontrou o mesmo projeto após uma segunda abertura. [Windows Server](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/actions/runs/43042033174) · [Windows 11](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/actions/runs/43042033109)

## Cobertura e histórico

No Windows passaram 590 testes Rust na suíte principal e 1 teste separado do indicador de conectividade. O teste opt-in do Kali também passou. A interface teve 68 testes unitários, 259 testes de navegador e 77 grupos de integração nativa.

Nos testes dos auxiliares, passaram 72 casos aplicáveis ao Windows e 123 no Kali. As telas da 0.1.12 enviam uma chave por solicitação de criação. Integrações antigas que chamam `assessment.create` diretamente precisam reutilizar a mesma `idempotencyKey` para recuperar a avaliação em um reenvio. Cada chamada sem chave continua criando um registro independente.

Os percursos de comparação e conectividade com NVDA foram verificados na 0.1.11. O ensaio anterior de disco cheio no APT também foi reutilizado depois de conferir os hashes do workflow, do auxiliar e da fixture. A matriz registra os cenários de plataforma ainda pendentes. [Validação anterior](RETOMADA-0.1.11.md) · [Ensaio do APT](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/actions/runs/43033604838) · [Matriz de aceitação](ACCEPTANCE-STATUS.md)

O Windows 11 ARM64 do ensaio já tinha o WebView2 instalado. Permanecem sem teste de ponta a ponta a primeira instalação em Windows cliente x64 sem WebView2 e uma queda física de energia ou reinicialização forçada durante a atualização. O ensaio do instalador local usou um perfil sintético.
