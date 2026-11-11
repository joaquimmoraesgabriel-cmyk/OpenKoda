# Marco 0.1.2 preservado e continuidade do roteiro

Atualizado em 2026-11-08 após regressão final, smokes nativos e upgrade real aprovados. **A distribuição local 0.1.2 está preservada e testada no alcance abaixo; o OpenKoda completo continua pendente.** O P2 de relógio foi corrigido e integrado aos binários finais. A próxima 0.1.3 está apenas em desenho; não se transfere este resultado a fontes futuras.

## Marco preservado e verificado

- [Manifesto 0.1.2](../releases/0.1.2-local-20261108/release.json) e [setup local](../releases/0.1.2-local-20261108/OpenKoda_0.1.2_x64-setup.exe): **4.883.480 bytes**, SHA-256 **8940c712f1c2977758b31c2a0d3b7137ef4490734f068b9db8593df23bc0eb72**, NotSigned conforme preferência local. Release preservado SHA-256 225c941db5503687e7e55c8bdc01cbe570f9200458ceebd023696d3dc9445edc; debug SHA-256 287f7e79e29b2af325fd3a2a40d778192923e7bd17964a3cbd9d4d78e438dcc0, 27.115.520 bytes; bundle index-DoZ1-Of1.js. Manifesto mantém completeRoadmap:false.
- [Upgrade real 0.1.1 → 0.1.2](../.local/upgrade-smoke-298a5ab0-a295-418f-b8df-546fdb1592bd/result.json): completed/passed:true, duas aberturas/seis áreas por versão, executáveis/registro conferidos e desinstalação da cópia temporária preservando os hashes físicos do perfil. JSON, linhas e hash lógico anteriores ficaram iguais; apenas user_version de openkoda.sqlite3 mudou 2→3 na primeira abertura. Zero nova chamada à API. Não prova interrupção de upgrade, rollback de binário, Windows limpo ou migrações futuras.
- [Manifesto 0.1.1](../releases/0.1.1-local-20261107/release.json) e [instalador local sem assinatura](../releases/0.1.1-local-20261107/OpenKoda_0.1.1_x64-setup.exe): SHA-256 fed95c0882c90deb6e4ed56d0f610c0df924c285d52d838be252a11fe25e965d, 4.760.616 bytes. Binários release e debug também preservados nessa pasta.
- Rust 250/250; frontend 28 unidades e 107 E2E, seguidos de uma alteração textual com teste focal aprovado; nativo 27/27. Os recibos e as distinções estão em [IMPLEMENTACAO](../IMPLEMENTACAO.md).
- [Atualização real 0.1.0 → 0.1.1](../.local/upgrade-smoke-919057ca-4cd6-4527-b546-a0059d97567a/result.json): passou, com duas aberturas por versão e desinstalação da cópia temporária. Linhas anteriores, configuração protegida e domínio preservados. Apenas o registro e o marcador do orçamento foram adicionados na inicialização. Não prova interrupção de upgrade, rollback de binários ou Windows limpo.
- [Ensaio posterior de origem externa](../.local/origin-smoke-df04aa55-9c67-4206-8f01-208af0a700e2/result.json): 3/3 gates, quatro recusas ACL explícitas pela ponte real Tauri, SQLite preservado, zero requisições à IA. Usa o debug 0.1.1 preservado; não somar aos 27 gates como se fosse a mesma execução. Processo e servidor próprios encerrados. Janela secundária invocando commands permanece fora dessa prova.

## Configuração do trabalho concluída

O [recibo do setup Matt Pocock](../.local/setup-matt-pocock-result.json) confirma os arquivos na raiz do repositório: [AGENTS.md](../../../AGENTS.md), [issue-tracker](../../../docs/agents/issue-tracker.md), [domain](../../../docs/agents/domain.md) e [triage-labels](../../../docs/agents/triage-labels.md). Os cinco rótulos existentes foram verificados: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human` e `wontfix`. Isso configura o fluxo de trabalho; não é prova de aceitação do aplicativo.

## Fonte e provas da 0.1.2

Versão de package.json, lockfiles, Cargo.toml e tauri.conf.json no marco: 0.1.2. Os recibos abaixo vinculam-se ao artefato preservado, independentemente de futuras mudanças na árvore de trabalho.

Backend: novo `src-tauri/src/http_validation.rs`; alterações em core.rs, executor.rs, remediation.rs e project_purge.rs. HEAD v2 preserva todas as ocorrências de headers e o peer. A alegação tipada é limitada à ausência de nosniff na resposta observada; não comprova exploração. O fluxo propõe candidato host, validação humana, nova execução aprovada, reteste e confirmação. Schema 3 impede binário anterior de interpretar a nova prova de fixed. lib.rs já declara o módulo e aplica guards de expurgo/ativação às rotas HTTP.

A [compilação intermediária](../.local/http-validation-compile.log) passou com **zero testes HTTP executados e 251 filtrados**; fica preservada como histórico, não como validação da fonte final. Depois, [14/14 testes HTTP](../.local/http-validation-tests.log) passaram em 2,31 s e [4/4 testes de migração](../.local/http-validation-migration-tests.log) passaram em 1,50 s, incluindo preservação do domínio 2 → 3 e retomada explícita do journal de expurgo. A [regressão Rust completa](../.local/rust-0.1.2-full-tests.log) passou **266/266**, zero falhas/ignorados, em 19,83 s. Essa rodada precede a correção de relógio indicada abaixo.

Depois da correção, a [regressão Rust final](../.local/rust-0.1.2-final-tests.log) passou **267/267**, zero falhas/ignorados, em **13,68 s**. O debug e o release preservados foram recompilados após essa correção; não são os binários intermediários anteriores.

Frontend: `src/http-validation.ts`, `HttpObservationView.tsx`, `HttpCandidateReview.tsx`, `HttpFindingPanel.tsx` e integração em App.tsx, Assessments.tsx e Findings.tsx estão implementados com o contrato final. O [checkpoint da interface](../.local/frontend-round123-checks.json) registra **31 testes unitários e 123/123 E2E**, zero inesperados, ignorados ou flaky; a rodada E2E levou 127,12 s. Build final aprovado: `index-DoZ1-Of1.js`, SHA-256 `326509bd66a3ba2239fd84a5a40ca5aa46c72fc5433e899c5b2f444bca0fb0d8`. São fixtures de IPC e preview de navegador; não validam o transporte HEAD nativo nem os painéis novos com NVDA.

Histórico da interface preservado: a primeira rodada focal teve 14/15 por seletor ambíguo; a primeira rodada completa teve 122/123 porque chaves React repetidas mantinham o formulário de interrupção após estado parcial. O seletor foi delimitado, as chaves dos componentes foram corrigidas e houve prova focal de 27 casos antes dos 123 finais. Logs/traces estão relacionados no mesmo checkpoint; não tratar essas falhas como tentativas silenciosamente descartadas.

O contrato vigente e os limites estão em [HTTP-VALIDATION](HTTP-VALIDATION.md), incluindo fatos no relatório e checksums. O [desenho backend](../.local/http-validation-design.md) e o [desenho da interface](../.local/http-validation-ui-design.md) continuam disponíveis como histórico.

A [revisão independente](../.local/http-0.1.2-review.md) encontrou um **P2, resolvido**: escolher a execução mais recente pelo relógio de parede permitia confirmar um reteste antigo após uma HEAD posterior com horário menor. O [RED](../.local/http-clock-rollback-red.log) reproduziu a falha com digest atual. A seleção passou a usar a ordem canônica persistida; [GREEN 15/15 HTTP](../.local/http-clock-rollback-green-local.log) e Rust267 passaram. A fixture cobre completed/failed/canceled posteriores, e a correção foi novamente conferida pelo revisor. Nenhum P1/P2 permanece aberto nesse escopo; não constitui certificação geral.

O [harness HTTP nativo](../scripts/native-http-smoke.mjs) passou **6/6 gates**, [recibo final](../.local/native-http-816730da-8575-4f89-9dd2-89e41d6cad43/result.json), exigindo SHA do debug acima. Perfil exclusivo e servidor loopback próprio: exatamente **três HEADs**, **18 recusas sem efeitos**, candidato/validação, duplicata inconclusiva, reteste nosniff e confirmação humana separada. Nove hashes, relatório e persistência após reinício conferidos; [UI real final](../.local/native-http-816730da-8575-4f89-9dd2-89e41d6cad43/http-final-confirmation.png) inspecionada. Zero requests/vault; processos/servidor encerrados. O operador do fixture alterou a resposta; o aplicativo não aplicou correção remota nem comprovou exploração.

Falhas anteriores do harness preservadas: [mensagem legítima de replay recusado fora da regex](../.local/native-http-517c4372-6463-40b7-b0db-e9dc95ed4a4c/result.json), depois [prefixo canônico Windows não normalizado no guard de paths](../.local/native-http-c92182cc-d169-4556-84c7-16ca77b692b1/result.json). As duas correções ficaram no harness; não houve alteração do produto para fazer essas asserções passarem.

O [smoke nativo geral](../scripts/native-smoke.mjs) passou **27/27 gates**, [recibo 0.1.2](../.local/native-smoke-1567a8b8-e4b5-4861-8fa3-8faea3e6d97d/result.json), com **29 rotas bloqueadas durante expurgo e 34 após ativação agendada**. Inclui seis rotas HTTP no expurgo, quatro mutações HTTP na ativação e `finding.httpStatus` permitido; regressão de orçamento/recuperação e demais fluxos em perfil sintético, sem nova inferência real. Não somar essas contagens aos 27 gates históricos da 0.1.1.

A [origem externa no debug 0.1.2](../.local/origin-smoke-d3def5bf-dee2-4f3e-a09c-64f4103b1cb3/result.json) passou **3/3 gates**, quatro recusas ACL explícitas pelo protocolo IPC e fallback postMessage reais, domínio preservado e zero requests/vault. Uma origem loopback própria; janela secundária e matriz ampliada de origens continuam fora da prova.

Helpers repetidos: [ambiente Windows 33/33](../.local/environment-helper-0.1.2-tests.log), 0,932 s; [MCP/executor Linux 30/30](../.local/linux-helper-tests-6cb60193-3df1-4266-9004-f932f41d5b7c/result.json), [log](../.local/linux-helper-tests-6cb60193-3df1-4266-9004-f932f41d5b7c/tests.log), 1,576 s. A [rodada MCP Windows](../.local/kali-helper-0.1.2-tests.log) executou 22 e ignorou oito dependentes de Linux; não equivale a 30 executados no Windows.

## Continuidade após o marco

1. Preservar binários, manifesto, fontes/documentação e recibos da 0.1.2 antes de liberar alterações da 0.1.3; não reatribuir hashes/provas a arquivos posteriores.
2. Detalhar o próximo contrato de trabalho e manter aprovação, proveniência e estados desconhecidos ao ampliar acompanhamento de planos/dependências e escopo. A 0.1.3 permanece em desenho neste checkpoint.
3. Continuar as lacunas da [matriz completa](ACCEPTANCE-STATUS.md): escopo versionado/combinação de fontes, fila e resultados paginados, política de dados, validações/remediações por modalidade, recuperação/atualização e matriz de acessibilidade/Windows.
4. Manter upgrade interrompido, rollback de binário, Windows limpo e leitor de tela dos painéis novos como provas pendentes. Upgrade positivo e NVDA delimitado não substituem esses cenários.

## Leitor de tela: interrupção preservada e retomada concluída

NVDA 2026.3 foi baixado da [origem oficial](https://www.nvaccess.org/post/nvda-2026-3/), conferido pelo hash publicado e assinatura válida de NV Access Limited. Cópia portátil em `.local/nvda-2026.3`, sem instalação no sistema. As opções seguem o [guia oficial](https://download.nvaccess.org/documentation/en/userGuide.html) e o [schema da versão](https://raw.githubusercontent.com/nvaccess/nvda/release-2026.3/source/config/configSpec.py).

`scripts/screen-reader-fixture.mjs` preparou somente dados sintéticos no debug 0.1.1. A primeira chamada Computer Use foi interrompida por Escape antes de iniciar o leitor; nenhum controle de interface foi acionado. O aplicativo de teste foi encerrado pelo próprio helper. O [recibo histórico](../.local/screen-reader-a307d978-325c-4178-a119-ff94e5a3091c/result.json) contém `readerStarted:false`, `interruptedBy:user-escape` e `passed:false`; a asserção por zero observações reflete a interrupção, não uma falha demonstrada do produto.

Após autorização para retomar, o [novo ensaio delimitado](../.local/screen-reader-e6df8aaf-9887-498a-bffa-8015b7eba31d/result.json) **passou em seis áreas**, com `readerStarted:true` e `domainPreserved:true`: Avaliações, Achados, Projetos, Configurações, Nova avaliação e Kali e ferramentas. Conferiu anúncios no Speech Viewer do NVDA com sintetizador `silence`, incluindo navegação, campo Endpoint, alerta de objetivo obrigatório e título de Kali. Aplicativo sintético e leitor foram encerrados.

Essa prova pertence ao debug **0.1.1** preservado e observa visualmente os anúncios, sem avaliação auditiva. Não certifica WCAG, todas as tarefas, abertura do candidato com leitor ou os novos painéis HTTP da 0.1.2. O foco/valor atrasado da árvore UIA não foi tomado como prova: os anúncios foram conferidos nas imagens atuais do Speech Viewer.

Nenhuma nova chamada paga à OpenAI foi realizada nos testes de orçamento, atualização, origem externa ou leitor de tela descritos aqui. A chave original permanece protegida e não deve ser impressa nem copiada para relatórios.
