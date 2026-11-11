# OpenKoda desktop

<!-- local-0.1.9 -->
Baixe o [instalador local OpenKoda 0.1.9 para Windows x64](releases/0.1.9-local-20261111/OpenKoda_0.1.9_x64-setup.exe). A versão permite ativar uma cópia recuperada do Kali, consultar todo o histórico de cópias, rever trabalho em curso antes de fechar a janela e salvar um diagnóstico local para suporte. A atualização da 0.1.8 preservou os dados de teste. [Guia](docs/GUIA-0.1.9.md) · [Recuperação do Windows](docs/RECUPERACAO-WINDOWS.md) · [Testes e limites](docs/RETOMADA-0.1.9.md)

Implementação Tauri do [mapa aprovado](../planejamento/README.md), com referências em [Visual e telas](../planejamento/VISUAL-E-TELAS.md). O andamento verificável está em [IMPLEMENTACAO](IMPLEMENTACAO.md); a presença de uma tela não significa que seu executor já está liberado.

A [candidata local 0.1.8 sem assinatura](releases/0.1.8-local-20261110/OpenKoda_0.1.8_x64-setup.exe) acrescenta revisão de uma proposta de plano da IA, backup e restauração de arquivos do Kali e reteste do campo booleano `debug` em JSON. O instalador passou na atualização a partir da 0.1.7 e na desinstalação com dados sintéticos. A abertura do executável instalado aguarda um ambiente Windows separado. [Guia de uso](docs/GUIA-0.1.8.md) · [Manifesto](releases/0.1.8-local-20261110/release.json) · [Resultados dos testes](docs/RETOMADA-0.1.8.md)

O [guia da 0.1.6](docs/GUIA-0.1.6.md) cobre avaliação, fila e aprovação. O [guia da 0.1.7](docs/GUIA-0.1.7.md) explica a retenção das capturas. Consulte o [roadmap](docs/ROADMAP.md) para os limites de plataforma e distribuição.

## Desenvolvimento

Pré-requisitos: Node 24+, Rust 1.90 ou superior com alvo MSVC, ferramentas C++ do Visual Studio e WebView2 no Windows. A retomada foi validada com Rust 1.99.1; o mínimo acompanha o Tauri fixado no lockfile.

```powershell
npm ci
npm run tauri dev
```

Preview da interface: `npm run dev`. O preview é identificado e não tem acesso ao núcleo Tauri. Os dados locais do preview são independentes dos dados nativos. Chaves, preparação do Kali e execuções são oferecidas somente pelo desktop.

O armazenamento nativo fica em `%USERPROFILE%\.openkoda`. Na primeira abertura, uma instalação anterior em AppData é importada por cópia SQLite verificada quando ainda não existe essa pasta. A origem, exportações e backups anteriores são preservados. Esse caminho evita que um aplicativo iniciado por outro pacote MSIX tenha os dados redirecionados ao cache do programa que o iniciou. Somente builds de desenvolvimento aceitam `OPENKODA_DATA_DIR` para testes isolados.

## Fluxos implementados

- Plano versionado com dependências e resultados explícitos, fila durável, revisão humana e operações nativas/IA/Kali no mesmo recurso de execução. Cada nó mantém preparação e autorização próprias; salvar o plano não executa operações.
- Capturas limitadas, consulta de metadados, leitura revisada por janelas, coleta do spool Kali sem repetir o produtor e retenção com reconciliação somente de leitura. Resultado parcial ou desconhecido mantém sua distinção.
- Encerramento local revisado para IA sem worker local ativo, preservando resultado remoto e cobrança desconhecidos. Pausa filas anteriores, invalida preparações e não libera dependências, repete a chamada nem cancela no provedor.
- Projetos, avaliações e configurações com SQLite, migração transacional e recuperação conservadora após interrupção.
- [Escopo versionado](docs/SCOPE.md), com revisão de objetivo, métodos e janela, revogação prospectiva e derivação sem herdar aprovações. [Fontes combinadas](docs/COMBINED-SCOPE.md) selecionam até oito entradas de um catálogo de até 64 alvos, com métodos e cobertura por entrada, exclusões locais/de conexão e origem das evidências. O alvo principal é uma referência de apresentação.
- Planos de observação com digest, aprovação e revogação: inventário local de arquivos/hashes, HTTP HEAD para a URL exata e conexões TCP em IPs/portas explícitos. A observação TCP aceita até 16 endereços, 16 portas e 64 combinações; não usa DNS, payloads ou subprocessos. Essas operações produzem cobertura parcial; não equivalem a uma auditoria de vulnerabilidades completa.
- Análise estática local por regras versionadas, com arquivo/linha/hash, revisão de identidade e limites explícitos para binários, arquivos grandes, exclusões e saída parcial. Valores encontrados e trechos não são incluídos nos candidatos.
- Captura local de conteúdo selecionado do manifest: até oito arquivos UTF-8 e 32 KiB, com prévia redigida, hashes, confirmação e deduplicação. A captura vira uma evidência; o envio à IA depende de outra seleção e consentimento.
- Candidatos e validação humana com método, comportamento esperado/observado e hashes das evidências utilizadas.
- [Validação factual HTTP](docs/HTTP-VALIDATION.md): ausência de `X-Content-Type-Options: nosniff` em uma resposta HEAD capturada pelo host, com candidato, revisão humana, nova execução aprovada, comparação e confirmação separada do reteste. Cabeçalhos repetidos ou respostas não comparáveis permanecem inconclusivos. A observação não comprova exploração nem aplica configuração remota.
- Preparação de `OpenKoda-Kali`, inventário e manutenção por planos, preservando distribuições pessoais. A disponibilidade da base Kali é separada da disponibilidade de um executor.
- Conexão OpenAI pelo núcleo, vault DPAPI do usuário Windows, revisão do contexto antes de cada envio e registro de consumo. A conversa e a investigação de até três perspectivas independentes exigem consentimento do contexto e registram uso conhecido/desconhecido; não executam automaticamente ferramentas nem aplicam alterações.
- [Orçamento da IA](docs/AI-BUDGET.md) com tarifas manuais em USD, limites por avaliação e instalação e revisão da estimativa antes do envio. Começa desativado. Reservas e débitos ficam fora dos perfis restauráveis; expurgo ou restauração não renovam o saldo. Uso desconhecido mantém uma reserva, e indisponibilidade do registro bloqueia novos envios.
- Sugestão da próxima operação pela IA, limitada aos adaptadores escolhidos pela pessoa e compatíveis com a modalidade. A proposta registra contexto e consumo; preparar o plano, revisar parâmetros e aprovar a execução continuam sendo ações separadas.
- Correção revisável de um arquivo UTF-8, aprovação vinculada aos bytes, bloqueio de edição concorrente, backup, reteste de conteúdo/hash e confirmação humana separada. Interrupções podem ser reconciliadas por inspeção explícita, preservando o arquivo atual e os efeitos históricos desconhecidos, sem reaplicar ou restaurar o patch.
- Relatórios Markdown/JSON, pacote de evidências redigidas, backups SQLite verificados e restauração preparada em outra pasta. A ativação ocorre no reinício, preservando o perfil anterior; voltar ao perfil anterior também prepara uma cópia nova. Credenciais e VHD do Kali não integram o backup.
- Inventário de cópias geridas e expurgo com seleção, prévia de hashes e confirmação específica. Arquivar projeto não apaga seu histórico; excluir uma cópia gerida não remove dados do provedor, fontes, outros perfis ou cópias externas.
- Exclusão explícita de projeto arquivado e dos seus históricos vinculados, com prévia das dependências e registro de cada etapa. Interrupções exigem inspeção e continuação ou encerramento parcial; não há promessa de transação entre bancos ou apagamento seguro.

Na 0.1.5, revise a política do projeto e sua adoção na avaliação antes das saídas. Classifique os dados e associe origens desconhecidas explicitamente; essa associação não confirma a veracidade do conteúdo. A listagem contém uma projeção limitada, e cada janela de conteúdo exige confirmação. A revisão de envio fixa o pacote e o destino; mudanças invalidam a revisão. Relatórios bloqueados não expõem a prévia, backups excluem credenciais e políticas restauradas ficam inativas. Referências de alvo são metadados, sem configurar autenticação. Consulte os passos no [guia de dados](docs/DATA-POLICY.md).

A ponte MCP stdio foi exercitada no Kali dedicado real: diagnóstico, hash de evidência selecionada e análise por regras fixas do ripgrep, com versão e hash do binário registrados e confirmação de término do processo. O parser recebe somente os bytes da evidência selecionada, após confirmação local; não recebe caminhos ou comandos livres. Terminal/PTY, ferramentas Kali genéricas, orquestração autônoma da IA e atualização de binários com recuperação continuam sujeitos aos gates registrados em IMPLEMENTACAO.md e na matriz de aceitação. O mapa completo ainda contém capacidades em construção.

## Verificação por etapa

```powershell
pwsh -NoProfile -File scripts/check-foundation.ps1
```

O script interrompe no primeiro gate que falha e registra os resultados em `.local/foundation-checks.json`. Não pula testes antigos ao integrar uma etapa nova. O teste de navegador não comprova IPC nativo; abertura nativa, WSL, credenciais e acesso externo precisam de resultados próprios.

O gate nativo compila em debug com a interface incorporada, calcula seu SHA-256 e passa esse hash aos smokes geral, dados, HTTP, escopo e combinado. Eles usam dados descartáveis em `.local`. Não exige Vite para esse binário. `-SkipNative` e `-SkipBrowser` registram explicitamente o que foi omitido.

O ensaio `scripts/native-data-policy-smoke.mjs --expected-sha256 <hash-do-debug>` verifica projeções, leitura por janelas, replay, política/classificação/origem, relatório e backup revisados, restauração inativa e revogação após reinício. O [script promovido](scripts/native-data-policy-smoke.mjs) integra `check-foundation.ps1`; sua presença não equivale a um recibo aprovado do candidato final.

O ensaio complementar `scripts/native-http-smoke.mjs --expected-sha256 <hash-do-debug>` usa três HEADs contra um servidor loopback próprio, verifica a cadeia de prova e preserva o resultado após reinício. Exige o hash do binário revisado e não usa alvos externos nem a API da OpenAI.

O ensaio `scripts/native-scope-smoke.mjs --expected-sha256 <hash-do-debug>` verifica escopo, revogação, derivação e migração a partir de dados criados pelo debug preservado 0.1.2. Usa perfil isolado, inventário local e um HEAD loopback próprio.

O ensaio `scripts/native-combined-smoke.mjs --expected-sha256 <hash-do-debug>` verifica migração a partir da 0.1.3, seleção de entradas, exclusões, captura e origens, cobertura parcial, backup/restauração e reinício. Seus arquivos sentinela e servidores HTTP/TCP são próprios. Os recibos da [0.1.4](docs/RETOMADA-0.1.4.md) correspondem ao executável fixado por hash; alterações posteriores exigem nova validação.

## Credencial e ambiente reais

No desktop, abra **Configurações → IA**, cadastre a chave, selecione um modelo e use **Testar conexão**. A chave não é retornada no snapshot nem herdada pelos processos Kali. O teste envia uma mensagem mínima e pode consumir créditos da API. Após configurar, cada conversa ainda exige revisar e autorizar seu contexto.

Na 0.1.5, associe uma referência ao slot do provedor, selecione classes/categorias/fontes permitidas e revise o pacote exato antes do envio. A referência não recebe o valor da chave; referências de alvo não materializam senhas, cookies ou cabeçalhos. O orçamento e a aprovação operacional continuam separados da política de dados.

Em **Kali e ferramentas**, revise o plano de preparação antes de executar. Nome ou destino já existentes não são adotados automaticamente. Diagnóstico, retomada e manutenção ficam separados; falhas não autorizam apagar ou reinstalar uma distribuição.

Os scripts `configure-openai.mjs` e `provision-kali.mjs` são auxiliares explícitos de integração nativa para desenvolvimento. O primeiro recebe um caminho de credencial indicado pelo usuário; o segundo só aplica o plano com `--apply`. Eles usam o perfil real do aplicativo, enquanto o smoke test usa dados isolados.

Se o catálogo WSL não puder baixar a imagem, o fluxo de imagem oficial consulta o manifesto oficial do Kali e confere SHA-256 antes da instalação. Uma imagem local selecionada também precisa corresponder ao manifesto oficial; escolher um arquivo não dispensa essa verificação.

## Pacote Windows

`pwsh -NoProfile -File scripts/package-windows.ps1` gera o instalador NSIS para o usuário atual. O recibo `.local/package-windows.json` registra hash e situação da assinatura. O ensaio `scripts/installer-smoke.ps1` instala uma cópia em `.local`, confere seus bytes, testa substituição pela mesma versão, abre o aplicativo duas vezes, desinstala essa cópia e verifica a preservação dos dados. Ele recusa um registro de instalação anterior para não substituir uma instalação do usuário. O script não equivale a uma prova executada: consulte o recibo da revisão testada.

O usuário solicitou a versão local sem assinatura Authenticode. O instalador recusa downgrade; a substituição pela mesma versão não comprova migração de uma versão futura. O teste neste computador não equivale a uma matriz de Windows limpo ou a distribuição pública assinada. A desinstalação normal preserva `%USERPROFILE%\.openkoda` e a distribuição `OpenKoda-Kali`; apagar esses dados exige uma decisão separada.

`scripts/upgrade-smoke.ps1` ensaia a atualização de um instalador preservado para uma versão maior, verifica os binários instalados e compara dados antes/depois de duas aberturas em cada versão. Inclui o registro e o marcador do orçamento. `-SelfTest` usa dados sintéticos; `-ValidateOnly` verifica os artefatos sem instalar. O resultado de uma execução real deve ser consultado em [IMPLEMENTACAO](IMPLEMENTACAO.md); o script sozinho não comprova a atualização.

## Transporte temporário de dependências

Se o Cargo/libcurl falhar no host enquanto Python HTTPS funcionar, `scripts/cargo-official-proxy.py` oferece um transporte exclusivamente em loopback para o índice oficial e os arquivos de crates.io. Os checksums continuam sendo conferidos pelo Cargo e permanecem no lockfile com a origem oficial. Esse auxiliar não pertence ao runtime do produto, não deve permanecer rodando após o build e não exige alterar a configuração global do Cargo. A opção `-CargoConfig` do gate permite indicar uma configuração temporária local.

## Organização

- `src/`: interface, componentes e contratos TypeScript.
- `src-tauri/`: aplicação nativa, autoridade e persistência Rust.
- `e2e/`: fluxos de navegação e regressão.
- `scripts/`: ferramentas locais de validação.
- `docs/`: registros da implementação e matriz de testes.
- `.local/`: resultados temporários locais, excluídos do Git.

Dados sensíveis e chaves não pertencem ao repositório. Não usar arquivos pessoais da pasta pai como entradas de teste. Projetos/alvos são selecionados explicitamente no aplicativo.
