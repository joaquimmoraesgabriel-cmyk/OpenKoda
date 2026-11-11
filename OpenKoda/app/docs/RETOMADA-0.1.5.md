# Retomada da 0.1.5 — dados e referências de credenciais

Checkpoint final de 2026-11-08. **A 0.1.5 é o marco local validado nos limites deste registro**, com instalador sem assinatura e binários [preservados](../releases/0.1.5-local-20261108/release.json). Passaram Rust372, frontend55/185, nativo61, Kali real11, NVDA7 delimitado e substituição final do instalador. O upgrade original 0.1.4 → 0.1.5 e a substituição posterior entre candidatos estão diferenciados abaixo. **O OpenKoda completo permanece em construção** (`completeRoadmap:false`); os requisitos restantes constam no [mapa de aceitação](ACCEPTANCE-STATUS.md). A [0.1.4](../releases/0.1.4-local-20261108/release.json) continua preservada como marco anterior.

## Escopo implementado

A 0.1.5 acrescenta política de dados por projeto, adoção explícita por avaliação, classificação de sensibilidade e origem, referências descritivas de credenciais e leitura por janelas confirmadas. IA, transferência de evidência ao Kali, relatório e backup passam por preparação e conferência do pacote antes da admissão. O [guia de dados](DATA-POLICY.md) descreve os fluxos humanos e os limites.

O núcleo mantém `Evidence` e hashes de planos, escopo e provas históricos. `Snapshot` recebe metadados aditivos; o schema principal passa de 5 para 6 sem reescrever o JSON legado na abertura sem reconciliação. As gerações persistentes de subjects canônicos impedem que mudar e retornar ao mesmo conteúdo torne uma classificação ou revisão antiga válida novamente. O expurgo inclui essas gerações e o restante do grafo de política.

A interface usa `SnapshotView`/`EvidenceView`: metadados, hashes distintos e prévias limitadas, sem `Evidence.content` no DTO. A resposta de mutação preserva os mesmos metadados precisos de redação/omissão da evidência exibida no snapshot, sem substituí-los pelo marcador genérico da resposta. O host aplica redação antes de limitar conteúdo, fecha a exceção genérica para campos chamados “controle” e mantém o modo `metadata_only` quando não consegue verificar a proteção da credencial. Referências de alvo permanecem somente metadados, sem estado de conexão do provedor ou autenticação de adaptadores.

Relatório e backup possuem revisões próprias. O relatório preserva seleção, omissões, categorias reais e distinção entre origem factual e associação humana; remediações mantêm a whitelist histórica sem before/after/predicado. O backup reconstrói bancos lógicos novos, examina registros vivos e exclui vault, teste de conexão e ledger global. Replay consulta o recibo durável antes de reconstruir o pacote. Restauração e ativação desativam políticas prospectivas, preservando histórico.

O bloqueio de relatório vale também para a prévia: `report.prepare` com `canExport=false` devolve `dataReview` e manifest nulos, Markdown vazio e listas de artefatos/omissões vazias. A interface descarta esses conteúdos defensivamente caso uma resposta negada antiga ainda os contenha, mostrando motivos sem confirmação sensível nem botão de exportar. Essa correção R9 fecha a exposição antes do commit; o fluxo permitido mantém a revisão dos bytes exatos.

O fallback público de `ai.context` sem operação foi fechado com `data_review_required`. O construtor interno continua privado; testes puros de seleção usam um helper exclusivo de testes, sem reabrir a rota IPC.

## Evidências observadas

Os números abaixo conservam rodadas históricas e finais separadas. A aceitação final usa Rust372 com os 36 arquivos Rust inalterados, frontend55/185 e os recibos do binário corrigido identificados na seção de gates finais; as contagens intermediárias não são somadas a esses resultados.

| Verificação | Resultado registrado | Evidência e alcance |
|---|---|---|
| Frontend antes da correção visual: TypeScript/Vite | Build aprovado; 1.648 módulos; JS 606,43 kB | [Log após correção da prévia bloqueada](../.local/frontend-data-policy-blocked-report-build.log), 16,51 s. Aviso de chunk acima de 500 kB permanece. |
| Frontend antes da correção visual: unidades | 55/55 | [Log após correção da prévia bloqueada](../.local/frontend-data-policy-blocked-report-unit.log), 13 arquivos. |
| Frontend antes da correção visual: E2E completo | **181/181; 0 inesperados, flaky ou ignorados** | [Resultado](../.local/frontend-data-policy-final-full-results.json), [log](../.local/frontend-data-policy-final-full.log), **161,015 s**. Fonte/dist conferidos antes e depois; Edge headless, IPC sintético. |
| Frontend corrigido: TypeScript/Vite e unidades | Build aprovado; 55/55 unidades | [Build](../.local/frontend-data-policy-adoption-summary-build.log), 8,26 s, 1.648 módulos, JS 609,81 kB; [unidades](../.local/frontend-data-policy-adoption-summary-unit.log), 13 arquivos. Aviso de chunk >500 kB permanece. |
| Frontend corrigido: adoção/projeções | 23/23 E2E dirigidos; 0 inesperados, flaky ou ignorados | [Resultado](../.local/frontend-data-policy-adoption-summary-focused-results.json), [log](../.local/frontend-data-policy-adoption-summary-focused.log), 35,522 s. |
| Frontend corrigido: suíte completa | **185/185; 0 inesperados, flaky ou ignorados** | [Resultado](../.local/frontend-data-policy-adoption-summary-full-results.json), [log](../.local/frontend-data-policy-adoption-summary-full.log), **212,288 s**. IPC sintético; fonte c1f93943…. |
| Frontend: E2E completo | 176/176; 0 inesperados, flaky ou ignorados | [Resultado](../.local/frontend-data-policy-e2e-results.json), [log](../.local/frontend-data-policy-e2e-full.log), 150,966 s. IPC sintético, Edge headless. |
| Ajuste do contrato de orçamento | Build e 21/21 E2E dirigidos | [Resultado](../.local/frontend-data-policy-budget-contract-results.json), [log](../.local/frontend-data-policy-budget-contract.log). Investigação envia somente `id/digest` ao orçamento; run conserva a revisão de dados. |
| Frontend: digests de política | 61/61 E2E focados; 0 inesperados, flaky ou ignorados | [Resultado](../.local/frontend-data-policy-digest-focused-results.json), [log](../.local/frontend-data-policy-digest-focused.log), 65,345 s. Dados, orçamento, escopo e avaliações combinadas; inclui três regressões novas. Rodada anterior à correção da prévia bloqueada. |
| Frontend: relatório bloqueado | 19/19 E2E de dados; 0 inesperados, flaky ou ignorados | [Resultado](../.local/frontend-data-policy-blocked-report-focused-results.json), [log](../.local/frontend-data-policy-blocked-report-focused.log), 23,546 s. Inclui resposta negada vazia e resposta negada antiga com canários; [reprodução anterior](../.local/frontend-data-policy-blocked-report-red.log) falhou como esperado. |
| Rust: primeira suíte completa | 367 aprovados, 1 falha; 0 ignorados | [Log](../.local/data-policy-first-full-suite.log), 70,80 s. Falha na apresentação da origem no relatório. |
| Rust: segunda suíte completa | 368 aprovados, 1 falha; 0 ignorados | [Log](../.local/data-policy-second-full-suite.log), 72,68 s. Inclui bloqueio novo do contexto legado; a mesma asserção de origem ainda falhava. |
| Rust: correção dirigida de origem | 1/1 aprovado; 368 filtrados | [Log](../.local/data-report-origin-final-test.log), 2,12 s. Identificador de entrada apresentado sem alterar seu valor e associação humana explicitada. Não é uma nova suíte completa. |
| Rust: terceira suíte completa | 368 aprovados, 1 falha; 0 ignorados | [Log](../.local/data-policy-third-full-suite.log), 117,58 s. Fixture HTTP recebeu `WouldBlock` no stream aceito; corrigida somente a fixture. |
| Rust: quarta suíte completa | **369/369; 0 falhas ou ignorados** | [Log](../.local/data-policy-fourth-full-suite.log), 138,48 s. Inclui a regressão do contexto legado; executável principal e doc-tests possuem zero testes. Precede as correções de coerência do DTO e da prévia negada. |
| Rust: coerência de `EvidenceView` | 2/2 dirigidos; 367 filtrados | [Log](../.local/data-evidence-dto-consistency-test.log), 0,07 s. Mutação/snapshot coerentes, conteúdo canônico ausente e MCP sem bypass de leitura bruta. |
| Rust: relatório negado, R9 | Falha reproduzida; depois 8/8 dirigidos, 362 filtrados | [Reprodução](../.local/data-denied-report-preview-red.log), [correção](../.local/data-denied-report-preview-green.log), 0,75 s. O canário não sai pela prévia negada; relatórios permitidos e recuperação revisada permanecem cobertos. |
| Rust: suíte completa após R9 | 370/370; 0 falhas ou ignorados | [Log](../.local/data-policy-final-full-suite.log), 77,97 s. Inclui coerência do DTO e relatório negado. Precede a correção de retenção novo/legado. |
| Rust: retenção novo/legado | 19/19 dirigidos; 353 filtrados | [Log](../.local/data-retention-report-green.log), 1,98 s. Reconhecimento fechado de manifest, inventário e seleção por hashes exatos. |
| Rust: fonte congelada após retenção | **372/372; 0 falhas ou ignorados** | [Log](../.local/data-policy-release-full-suite.log), **104,47 s**. Executável principal e doc-tests possuem zero testes; build debug e recibos nativos finais continuam separados. |
| Helper de ambiente Windows | 33/33 | [Log](../.local/environment-helper-0.1.5-tests.log), 0,750 s. Não é instalação/manutenção real. |
| Helper MCP Windows | 22 executados; 8 ignorados | [Log](../.local/kali-helper-0.1.5-windows-tests.log), 0,010 s. Os oito casos exigem Linux. |
| Helper Linux | 30/30; zero ignorados; exit 0 | [Recibo](../.local/linux-helper-tests-a378be09-3190-4100-8655-a209406fce77/result.json), [log](../.local/linux-helper-tests-a378be09-3190-4100-8655-a209406fce77/tests.log), 1,895 s. Hashes dos arquivos registrados. Não é ensaio de transferência pelo bridge nem nova instalação. |
| Debug nativo com interface embutida | Build concluído; 11 warnings | [Log](../.local/data-policy-native-debug-build.log), 57,47 s. Compilar não certifica execução nativa ou instalador. |
| Smoke nativo de dados, estágio de integração | 6/6 cenários aprovados | [Recibo](../.local/native-data-policy-8dc84899-3896-48b9-b196-d92861a8c76f/result.json), 2026-11-08T16:35:10.829Z. Perfil exclusivo, IPC real, janelas/replay, política/classificação/origem, relatório exato, backup/replay/restore inativo e revogação/reinício. |
| Smoke nativo HTTP, mesmo estágio | 6/6 cenários aprovados | [Recibo](../.local/native-http-33f7fc86-9c28-491d-a721-ef861ca2ce52/result.json), 2026-11-08T16:36:15.089Z. Baseline, revisão humana, duplicidade de cabeçalho, reteste, relatório e reinício. |
| Smoke nativo de escopo, mesmo estágio | 9/9 cenários aprovados | [Recibo](../.local/native-scope-60200c54-0b90-4a5f-a8ef-57483c42030e/result.json), 2026-11-08T16:37:26.572Z. Migração legada, revisão, limites de método/janela, derivação, revogação e histórico. |
| Smoke nativo combinado, mesmo estágio | 10/10 cenários aprovados | [Recibo](../.local/native-combined-ac7d385b-bdf1-4292-928f-e932dee09cc2/result.json), 2026-11-08T16:38:14.731Z. Migração 4 → 6, catálogo, fontes/exclusões, HTTP/TCP em loopback próprio, exportação, backup e reinício. |
| Revisão independente e reconciliação | Sete achados independentes, mais R8 identificado pelo coordenador | [Revisão](../.local/data-policy-0.1.5-independent-review.md). Correções por inspeção e resultados da suíte do coordenador registrados separadamente; o revisor não executou Cargo, UI, provedor ou WSL. Não é aprovação de release. |

O [checkpoint do frontend](../.local/FRONTEND-DATA-POLICY-0.1.5-CHECKPOINT.md) registra os limites, a captura de 390 px e as rodadas. O [manifesto da rodada de 181 casos](../.local/frontend-data-policy-pre-adoption-summary-source-hashes.json) preserva o digest `b707a61251a4227ab74d1cd53d57e37fcc1c012baf907cc722770292ee1ee536`, conferido antes e depois daquela suíte. A correção visual posterior tem [manifesto próprio](../.local/frontend-data-policy-adoption-summary-source-hashes.json), digest `c1f9394303d2c5be0ea0ca9ce76d00116413b7532196d3f0da4d5b87776df8fe`. A regressão de 176 casos também permanece no [manifesto daquela rodada](../.local/frontend-data-policy-full-run-source-hashes.json). Os 21, 61, 19 e 23 casos dirigidos não são somados como casos distintos às suítes completas.

As suítes completas Rust372 e E2E181 passaram antes da correção visual. A [quarta tentativa geral nativa](../.local/data-policy-native-general-fourth.log) passou 21 gates e falhou na retenção: o inventário reconhecia o relatório legado, mas não o novo formato revisado. A correção preserva o reconhecimento fechado por categoria, schema e identidade e admite os formatos suportados; passou nos 19 testes dirigidos e depois na suíte372. A regressão geral passou posteriormente no candidato 4fcf4900… abaixo; esse resultado não certifica o próximo binário com frontend corrigido.

O [smoke de política de dados](../scripts/native-data-policy-smoke.mjs) foi promovido para `scripts/` e integrado ao [gate de fundação](../scripts/check-foundation.ps1). O gate calcula o SHA-256 do debug e o exige nos smokes geral, dados, HTTP, escopo e combinado. A presença do script não é evidência de execução bem-sucedida.

O baseline da revisão é [source-0.1.4.zip](../releases/0.1.4-local-20261108/source-0.1.4.zip), SHA-256 `6b1db0655dba1c6646aab081b3d1d9436945d79c3ec69ce7963e8df9927f8010`. Não foi substituído pelo trabalho da 0.1.5.

Os quatro grupos nativos acima usam o debug de 33.328.128 bytes, SHA-256 `34dcd0680ddfa51074d1c74389937419a4436f21b9fe493e2cbca9aaa3bec652`. Esse hash identifica **um estágio**, não o artefato final. A [primeira tentativa do smoke de dados](../.local/native-data-policy-d873e9b2-3ce1-448b-98af-8b7a2837b118/result.json) passou em cinco cenários e falhou ao enviar para revogação o digest de estado com avaliação; o harness passou a consultar o estado do projeto, e a segunda tentativa concluiu os seis.

A divergência análoga da reativação na interface foi corrigida: mutações de projeto consultam seu digest próprio; adoção usa o digest da avaliação. A interface invalida revisões e respostas pendentes quando o estado muda. Os três novos E2E reproduzem o ciclo entre projeto e avaliação e duas alterações durante preparação/commit; estão incluídos nos 61 dirigidos aprovados. O candidato 4fcf4900… incorporou essas correções; a alteração visual posterior recebeu uma nova rodada completa, registrada nos gates finais.

A terceira suíte Rust falhou porque o stream aceito pela fixture HTTP herdou modo não bloqueante e retornou `WouldBlock` sob carga. A fixture agora chama `set_nonblocking(false)` antes de configurar o timeout de leitura. Essa correção de teste não altera o transporte de produção; a quarta suíte completa passou com 369 testes.

## Candidato anterior à correção visual

O [manifesto preservado em `.local`](../.local/0.1.5-before-visual-fix/release.json) identifica o estágio `candidate_requiring_visual_fix`, sem declarar release concluída. O debug tem SHA-256 `4fcf4900d6d383ed7f25a233e46185de2a8a5891245f44c952764b3b0e3a7eaa`; o executável release, `265654aa9db0cae60e95626881b80e925e5c8bd5c937bde6cb75fd7121ea1e39`; o instalador sem assinatura de 5.780.403 bytes, `64f94bd90f5d0e4be67727a1e59d86d54b23b65b8e685a096a1023acdea0a54e`. Esses artefatos são anteriores à correção visual e não representam a entrega final 0.1.5.

| Prova desse candidato | Resultado | Alcance |
|---|---|---|
| Regressão nativa agregada | **61 gates aprovados** | [Agregado preservado](../.local/0.1.5-before-visual-fix/native-receipts.json): geral27, dados6, HTTP6, escopo9, combinado10 e origem3, todos vinculados ao debug 4fcf4900…. |
| Bridge Kali real | **11/11** | [Recibo](../.local/native-combined-kali-640835ba-4bb1-4580-aa40-0119cfa75f7a/result.json): diagnóstico, hash/análise de 888 bytes sintéticos, revisão de transferência/origens, negativas de sensibilidade/segredo/consentimento, revogação e reinício. Três cleanups confirmados; fixture arquivada, escopo revogado e outros dados preservados. Zero chamadas de provedor da fixture; histórico global não auditado. |
| Upgrade original 0.1.4 → candidato 0.1.5 | **Concluído/aprovado; schema 5 → 6** | [Recibo](../.local/upgrade-smoke-11977423-c78f-4faf-85aa-e1bf5a19292d/result.json): duas aberturas/seis áreas por versão, dados preexistentes preservados, sem adição estrutural além da mudança observada de `user_version`; desinstalação própria preservou os dados. Vale para o instalador 64f94bd9… e release 265654aa…, não para o próximo pacote. |
| Visual/leitor de tela | **Reprovado** | [Recibo](../.local/screen-reader-data-ed47c831-46bc-479a-9a2e-4fce7d6f944d/result.json): cinco observações passaram, mas a prévia de adoção não exibia destinos/classes/categorias/fontes antes de confirmar. Domínio/fontes preservados e processo próprio encerrado; nenhuma confirmação persistente foi enviada. |

Antes dos 27 gates gerais aprovados, uma [tentativa](../.local/native-smoke-fdce295a-edbd-4697-916d-1ff4c2a26d32/result.json) falhou em `activation.plan`: uma leitura iniciada pela interface ainda mantinha uma lease nativa. O harness passou a acompanhar o ciclo IPC, desmontar painéis com atualização e aguardar leituras pendentes antes de pedir ativação uma vez. Foi corrigida a sincronização do ensaio, sem mudar produção nem repetir mutação automaticamente.

A falha visual foi observada com NVDA 2026.3, Speech Viewer e síntese silenciosa, usando mouse, rolagem e teclado. Alguns campos UIA estavam desatualizados; as conclusões usaram também a tela e o Speech Viewer. O ensaio não é avaliação auditiva, percurso completo só por teclado ou certificação WCAG. Os nomes internos do aviso de omissão também foram encaminhados para apresentação em linguagem comum.

## Correção visual e nova rodada

`DataPolicy` agora apresenta a revisão exata a adotar, identidade/digest, estados permitidos ou bloqueados, classes, categorias, fontes, permissão de bytes originais ao Kali, leitura local e backup. A confirmação exige que o resumo corresponda à revisão retornada. `ProjectionNotice`, em `DataAccess`, apresenta rótulos humanos para omissões. Build, 55 unidades, 23 E2E dirigidos e **185 E2E completos em 212,288 s** passaram. A [revisão estática da correção](../.local/data-policy-visual-fix-review.md) não confirmou achado P1/P2 em `DataPolicy`/`DataAccess`; o revisor conferiu código e evidências de outros agentes, sem executar testes, app ou leitor de tela. A nova verificação visual nativa passou nas sete observações delimitadas abaixo.

O coordenador conferiu os [36 hashes Rust preservados](../.local/0.1.5-before-visual-fix/rust-source-hashes.json): o núcleo não mudou nesta correção. A suíte372 permanece vinculada a essa fonte. O novo debug incorpora o frontend corrigido; seus resultados finais estão separados abaixo.

## Artefato corrigido — gates finais concluídos

O novo debug tem **33.332.224 bytes**, SHA-256 **`a4fddba9b0a858bea8541fe04e867d68763894ee58e8bef726be1056faddb5c9`**. O frontend usa `index-BuCOrfgi.js`, SHA-256 `037d859e66129d301f8d75bba815c425ca6257244f21de4404706c2ad0b01af0`, vinculado à fonte `c1f9394303d2c5be0ea0ca9ce76d00116413b7532196d3f0da4d5b87776df8fe`.

| Gate do artefato final | Estado comprovado neste checkpoint | Recibo e alcance |
|---|---|---|
| Regressão nativa agregada | **61/61 aprovado** | [Agregado final](../.local/data-policy-final-native-receipts.json): geral27, dados6, HTTP6, escopo9, combinado10 e origem3, todos no debug a4fddba9…. |
| Geral nativo | 27/27 aprovado | [Recibo](../.local/native-smoke-a5135e21-c6ad-4c7e-a329-6f58ab1169b1/result.json), debug a4fddba9…. |
| Dados | 6/6 aprovado | [Recibo](../.local/native-data-policy-0d90aea8-c449-48e4-8c38-1e199634a049/result.json), mesmo debug. |
| HTTP | 6/6 aprovado | [Recibo](../.local/native-http-0841a57f-bcb7-43d2-8eff-4b8ac986a4c9/result.json), mesmo debug. |
| Escopo | 9/9 aprovado | [Recibo](../.local/native-scope-b7145d2b-085b-4787-9aed-3c306b41abd3/result.json), mesmo debug. |
| Combinado e origem externa | 10/10 e 3/3 aprovados | [Combinado](../.local/native-combined-0c522241-b1df-4141-93d2-828943539be0/result.json) e [origem externa](../.local/origin-smoke-afb6b922-debf-43e9-b432-2aca96dd576c/result.json), mesmo debug a4fddba9…. |
| Kali real | **11/11 aprovado no novo hash** | [Recibo final](../.local/native-combined-kali-1996231c-7472-4210-8338-9f57ab2ad044/result.json), [log](../.local/data-policy-visual-final-kali.log), debug a4fddba9…. Três cleanups confirmados; app próprio encerrado, fixture arquivada e escopo revogado. |
| Visual/leitor de tela | **7 observações aprovadas** | [Recibo final](../.local/screen-reader-data-a3e7ce17-090b-455b-a346-cc6a192ed7d3/result.json): resumo completo da adoção e anúncios, referências descritivas, aviso de projeção legível, origem/prévia e janela preparada. Domínio/fontes preservados, zero provider/vault, app próprio encerrado. |
| Pacote Windows corrigido | Compilado e preservado; `NotSigned` | [Log](../.local/package-windows-0.1.5-visual.log), [manifesto preservado](../releases/0.1.5-local-20261108/release.json). Instalador **5.780.532 bytes**, SHA-256 **`54239bede74b56d7b0a4f949984ed3d817e861d9adf53a6c036511dd6bcf35cc`**. Release executável SHA-256 `c30c66ce6e993f2a659d7090ee37d737d06c8d97c26b9ad3db27f19304b2709a`. Cópias dos três binários conferidas por hash pelo coordenador. |
| Substituição do instalador | **Concluída/aprovada** | [Recibo](../.local/upgrade-smoke-1a6f8112-6160-4aea-bf5c-e1d0af8e95c0/result.json), [log](../.local/upgrade-0.1.5-visual-rebuild.log), `testKind=same_version_rebuild`. Duas aberturas/seis áreas por artefato, dados preservados, sem adições estruturais ou mudança de schema; desinstalação própria exit0. Não é outra migração5→6. |
| Marco local | **0.1.5 validada nos limites verificados** | Binários em [releases/0.1.5-local-20261108](../releases/0.1.5-local-20261108/release.json), sem sobrescrever os marcos anteriores; `completeRoadmap:false`. |

O ensaio final com **NVDA 2026.3** confirmou destinos/classes/categorias e duas fontes nomeadas no resumo da adoção, com identidade/digest da revisão e condições de leitura/backup. O leitor anunciou o título e **Confirmar adoção prospectiva**; a revisão foi fechada sem adotar. A janela **0–654 de 654 bytes** foi preparada; o foco visível e o anúncio de **Confirmar leitura desta janela** foram conferidos sem acionar a leitura. O aviso de omissão usa linguagem comum. O coordenador encerrou o app e o leitor próprios após a inspeção.

Essa aprovação é delimitada: Speech Viewer com síntese silenciosa, navegação mista de mouse/rolagem/teclado e foco UIA desatualizado em `RootWebArea`; a observação usou contorno visível e anúncios. Não prova avaliação auditiva, percurso completo só por teclado, todos os estados ou conformidade WCAG. A falha do candidato anterior continua preservada como histórico, encerrada nesse recorte pela correção e pelo novo ensaio.

O perfil migrou para schema 6 no upgrade original. O binário antigo de schema 5 recusa esse perfil corretamente; a migração não foi repetida. O ensaio final usou `-AllowSameVersionRebuild` e substituiu **um candidato 0.1.5 por outro candidato 0.1.5**. Seu resultado passou, com os hashes finais de release/instalador conferidos e `existingDataPreserved=true`. O [autoteste prévio do harness](../.local/upgrade-rebuild-harness-selftest.log) passou 20 casos PowerShell e três Python, sem iniciar instalador nem ler o perfil. São evidências distintas: migração5→6 original, autoteste do harness e substituição real sem mudança de schema. Nenhuma delas é upgrade interrompido, rollback de binário ou matriz Windows limpo.

O Kali final repetiu o fluxo de política de dados no debug a4fddba9…, sem reutilizar a aprovação do hash anterior. Diagnóstico, transferência revisada/hash/análise lexical, negativas de sensibilidade/segredo/consentimento, origens, revogação e reinício passaram nos 11 gates. O registro confirma encerramento dos processos próprios, três cleanups, arquivamento da fixture e revogação do escopo. Não é execução genérica de ferramentas, sandbox universal ou prova de segurança dos alvos; o alcance permanece o dos dados sintéticos selecionados.

## Correções e regressões que precisam permanecer

| Fronteira | Correção presente no código | Regressão associada |
|---|---|---|
| Projeção de controles | Redação de credencial antecede preservação; valores fechados em `metadata_only` | `control_named_text_never_bypasses_managed_redaction_or_metadata_only` |
| Coerência da evidência projetada | A mutação conserva os metadados de redação/omissão do snapshot, sem substituir o DTO por um marcador genérico | `nested_mutations_and_snapshot_never_return_canonical_content`; `mcp_has_no_raw_read_argument_or_fallback` |
| Conteúdo do backup | Detector gerenciado antes do parse, além de chaves JSON, strings e números | `managed_material_in_json_keys_nested_strings_and_numbers_is_rejected` |
| Categoria de captura de fontes | Relatório exige a categoria real do conteúdo selecionado | `source_capture_cannot_be_exported_as_generic_evidence_category` |
| Metadados HTTP implícitos | Seção factual adicional exige identificação de alvos e resumo de execuções | `finding_only_report_omits_unselected_http_observation_categories` |
| Origem desconhecida | Saídas de conteúdo exigem origem conhecida ou associação humana válida; recuperação integral tem regra própria | `classified_evidence_without_origin_requires_explicit_association_for_report` |
| Mudança e retorno ao mesmo valor | Geração prospectiva persistente invalida classificações e reviews | `finding_aba_invalidates_classification_and_pending_report_without_rewriting_history` |
| Replay de backup | Consulta do recibo durável antes de reconstruir o pacote | `backup_replay_recovers_durable_receipt_without_rebuilding_or_writing` |
| Contexto legado | IPC sem operação não devolve conteúdo por fora da revisão | `legacy_context_cannot_bypass_explicit_data_review_or_local_read` |
| Prévia de relatório negado, R9 | Resposta bloqueada não devolve conteúdo do pacote nem revisão reutilizável; a interface também descarta conteúdo negado | `denied_report_preview_cannot_disclose_full_content_without_a_data_grant` |
| Retenção de relatórios | Reconhece formatos suportados novo/legado por categoria, schema e identidade; seleciona os arquivos exatos | `reviewed_report_is_inventoried_and_retained_by_exact_file_hashes`; `manifest_recognition_is_closed_by_category_schema_and_identity` |
| Exportação de remediação | Mantém metadados/hashes, omitindo conteúdo completo e valor do predicado | `report_omits_source_and_predicate_bytes_while_restore_keeps_local_history` |

As fixtures antigas de ativação e remediação foram migradas para os helpers de revisão pública; os asserts de integridade e preservação foram mantidos. O [registro da migração](../.local/data-recovery-fixture-migration.md) distingue a correção da fixture de uma mudança de produção.

## Fechamento do marco e limites

Os gates delimitados da 0.1.5 foram concluídos no artefato corrigido: fonte Rust372 sem mudança, frontend55/23/185, nativo61, Kali11, NVDA7 e substituição do instalador. Os binários finais foram copiados e conferidos sem sobrescrever releases anteriores. O [plano de dados](../.local/data-policy-native-plan.md) e o [handoff de escopo/combinado](../.local/data-scope-smoke-migration.md) preservam o escopo dos ensaios. O manifesto registra a identidade dos artefatos; não há hash de ZIP de fontes antecipado neste checkpoint.

O Kali11 final comprova transferência delimitada de dados sintéticos no novo hash. Nenhuma evidência desta rodada demonstra nova chamada real à OpenAI, instalação limpa do Windows, upgrade interrompido, rollback de binário ou resistência a todas as falhas físicas. NVDA7 não amplia a acessibilidade aos percursos não amostrados. Esses limites permanecem na avaliação do produto completo.

## Próximos blocos do produto

Concluir este recorte não conclui o OpenKoda. O [mapa de aceitação](ACCEPTANCE-STATUS.md) ainda exige acompanhamento iterativo de planos e dependências, fila durável e spool/cursor para as operações longas correspondentes, validações/remediações técnicas das demais capacidades, recuperação gerida da distribuição e a matriz de instalação/upgrade interrompido/rollback/Windows. Acessibilidade dos percursos não amostrados também permanece aberta.

O desenho e os critérios de 0.1.5 estão no [recorte original](../.local/data-policy-0.1.5-design.md) e no [contrato de integração](../.local/data-policy-0.1.5-contract.md). Eles são histórico de planejamento; o comportamento final deve ser conferido no [guia](DATA-POLICY.md), no código e nos recibos atuais. Extensões como multiusuário, execução remota, plugins externos e catálogo Kali completo continuam distintas dos bloqueios obrigatórios descritos na matriz.
