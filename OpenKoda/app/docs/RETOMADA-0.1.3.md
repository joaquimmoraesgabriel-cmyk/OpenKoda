# Marco local testado — 0.1.3

Iniciada em 2026-11-08 após preservar e testar a 0.1.2. A 0.1.3 passou regressão, ensaios nativos, NVDA delimitado e upgrade positivo e foi preservada para uso local sem assinatura. O objetivo completo continua o roteiro do produto; este incremento trata uma parte das lacunas.

## Base preservada

[Manifesto 0.1.2](../releases/0.1.2-local-20261108/release.json), instalador, executáveis release/debug e fonte arquivados. SHA-256 da fonte ZIP: `46c87002603657a889926b676c7efd8e2646b69ed5fd6f0ac9acfcb41a4973d5`; 205 arquivos, incluindo testes, scripts, documentos e configuração de build. Não inclui dados pessoais, perfis, credenciais ou diretórios de dependências/build. Os hashes individuais estão dentro do ZIP.

A 0.1.2 passou Rust267, UI31+123, nativo27, HTTP6, origem externa3, helper de manutenção33 e MCP Linux30. Upgrade0.1.1→0.1.2 preservou registros e alterou somente user_version2→3. O [checkpoint anterior](RETOMADA-0.1.2.md) conserva os recibos e limites.

## Incremento

Revisar objetivo, métodos e janela; congelar antes do trabalho; revogar novas admissões; adotar prospectivamente avaliações legadas; criar avaliação derivada no mesmo projeto e alvo, sem copiar aprovações/provas/consumo. Fonte e destino combinados, multialvos, exclusões, política de sensibilidade, dependências, fila e spool permanecem etapas seguintes.

Desenhos preservados: [backend](../.local/scope-backend-0.1.3-design.md) e [interface](../.local/scope-ui-0.1.3-design.md). O contrato implementado está em [SCOPE.md](SCOPE.md), com regras antes dos efeitos nos serviços de IA, captura, bridge e remediação. Rótulo da UI não concede autoridade. Históricos e provas antigas permanecem verificáveis após expiração/revogação. Desenhos não substituem os recibos de execução abaixo.

Versões de package.json, package-lock, Cargo.toml, Cargo.lock e tauri.conf.json avançadas para 0.1.3 sem mudança de dependências. Migração 3→4 implementada e testada sem backfill de consentimento nem reescrita de JSON legado. Novas operações exigem referência válida; nenhuma autorização histórica é reconstruída automaticamente.

## Plano inicial e fechamento

1. Concluído: núcleo e consumidores com congelamento, revisão, janela, revogação e migração; fixtures positivas usam escopo explícito, sem bypass de teste.
2. Concluído: interface e contratos de preview/nativo alinhados; regressões unitárias, focais e completas.
3. Concluído no recorte: testes de serviço e IPC negam efeitos fora de escopo; histórico, idempotência, derivação e recuperação verificados.
4. Concluído: build incorporado, smokes geral/HTTP/escopo/origem com dados isolados e revisão independente.
5. Concluído: instalador local sem assinatura, upgrade 0.1.2→0.1.3 e preservação dos binários/hashes.
6. Concluído no recorte: NVDA 0.1.3, guia congelado e links conferidos. O coordenador identificará o arquivo de fontes separadamente no manifesto.

No registro inicial ainda não havia testes da0.1.3 aprovados; os resultados posteriores estão discriminados abaixo. Não sobrepor Cargo; não usar o perfil real ou chamadas pagas para fixtures. A chave original permanece protegida. O controle visual foi autorizado novamente e pode ser usado nos testes pertinentes.

## Resultado atual

A [regressão Rust final](../.local/rust-0.1.3-final-tests.log) passou **294/294**, zero falhas/ignorados, em9,61s; inclui os ajustes finais de revisão. O [frontend congelado](../.local/frontend-round142-checks.json) passou **38/38 unidades +142/142 E2E**, zero falhas/skips/flaky, em145,85s no navegador. Build aprovado: `index-I76Wyjao.js`, SHA-256 `744d188a79fa7a5695f15a075df9cb066144e0b82444a05256f2953665eaddb3`. CSS global permaneceu idêntico ao marco123. A captura de reflow a512px foi inspecionada; testes de navegador usam IPC simulado e não certificam o executável nativo.

As revisões [IA](../.local/scope-ai-0.1.3-review.md) e [núcleo/recuperação](../.local/scope-core-0.1.3-review.md) ficaram sem P1/P2 aberto no recorte final. Foram corrigidos quatro achados: revogação bloqueada por inferência/vault, whitelist de investigação no expurgo e revalidação MCP depois da negociação. O [RED do expurgo](../.local/scope-purge-schema-red.log) e o [RED MCP](../.local/scope-kali-revoke-red.log) preservam as reproduções. O [GREEN focal MCP](../.local/scope-kali-revoke-green.log) passou1/1; ambos os casos passam na regressão294. O contrato final está em [SCOPE.md](SCOPE.md).

**Binários preservados:** [manifesto](../releases/0.1.3-local-20261108/release.json), [instalador](../releases/0.1.3-local-20261108/OpenKoda_0.1.3_x64-setup.exe), [release](../releases/0.1.3-local-20261108/openkoda.exe) e [debug testado](../releases/0.1.3-local-20261108/openkoda-debug.exe). Cópias novas, sem sobrescrita de releases anteriores, conferidas por SHA-256:

| Artefato | Bytes | SHA-256 |
|---|---:|---|
| Instalador 0.1.3, NotSigned | 5.030.885 | 12e2811e2d521c0f8a7b281a8bfc80d84958a5d7ea0d778de25d35f900494b0d |
| Executável release | 19.168.768 | 58f7d7b5b588037560ea23b71a65e39710470abc0864800ad51e1be52de45c59 |
| Debug dos ensaios | 28.061.696 | 792ed0a06a7c2bdbdb522de1f74aefdde6d9f4b35513319224c9b40a50d42d90 |

O manifesto mantém **completeRoadmap:false** e identifica o [guia congelado](../releases/0.1.3-local-20261108/GUIA-USO-LOCAL.md). Sem assinatura por escolha explícita para uso local, sem declaração de confiança de publicador ou release pública certificada. O coordenador identificará o arquivo de fontes separadamente no manifesto; esta rodada documental preservou os três binários e o guia.

| Ensaio real com o debug acima | Resultado | Limite |
|---|---|---|
| [Escopo](../.local/native-scope-a0476f23-338f-4510-88dd-ceaf510b6202/result.json) | 9/9 gates, 30 recusas; um HEAD próprio. Legado pela API 0.1.2, migração/relatórios, métodos/janelas, revisão, idempotência, revogação, derivação e reinício | Sem grants inseridos por SQL; revogação não declara cancelamento de efeito já admitido; sem multialvos ou scheduler |
| [HTTP](../.local/native-http-1d6dc774-cff1-4002-a367-a40b6db0ad04/result.json) | 6/6, três HEADs, 18 recusas, nove hashes, relatório/reinício; [confirmação final](../.local/native-http-1d6dc774-cff1-4002-a367-a40b6db0ad04/http-final-confirmation.png) inspecionada | Servidor próprio; operador alterou a resposta. Sem remediação remota, exploração ou prova do site inteiro |
| [Geral](../.local/native-smoke-77a3277d-0696-4c74-bffa-3e5dc1730b82/result.json) | 27/27; 33 rotas bloqueadas por expurgo e 37 após ativação agendada; uma conexão TCP com zero bytes de aplicação; [rascunho real](../.local/native-smoke-77a3277d-0696-4c74-bffa-3e5dc1730b82/02-native-draft.png) inspecionado | Perfis e fontes sintéticos; não comprova falha física ou rollback de executável |
| [Origem externa](../.local/origin-smoke-b6dd796c-e57d-4850-8862-b16d846cb932/result.json) | 3/3; quatro recusas explícitas no IPC/fallback postMessage reais; hash SQLite preservado | Uma origem loopback e dois commands benignos; sem command em janela secundária nem afirmação universal de isolamento |

Os **45 gates** passaram na primeira execução, sem ajustes durante os ensaios. Zero chamada ao provedor/operação WSL, arquivos/listeners/perfis próprios, processos e servidores encerrados. As recusas verificam hashes e ausência de novo tráfego no alcance declarado; não observam toda syscall negada nem demonstram dez repetições/CI.

**Upgrade 0.1.2→0.1.3:** [recibo real](../.local/upgrade-smoke-4ac89b63-51ae-44ac-8603-10a434c00b8d/result.json), passed:true/stage:completed, 2026-11-08T13:39:12.0087447Z. Duas aberturas/seis áreas em cada versão, configuração protegida e seis requests anteriores preservados, sem nova API. JSON/linhas anteriores permaneceram iguais; somente user_version de openkoda.sqlite3 mudou 3→4 na inicialização. Desinstalação preservou hashes físicos. Não cobre upgrade interrompido, rollback binário, Windows limpo nem migrações futuras.

**NVDA 0.1.3:** [recibo](../.local/screen-reader-scope-699c06c3-1ba4-4154-abdb-245c35652c81/result.json), passed:true, cinco verificações delimitadas de navegação de escopo, objetivo/métodos, janela, reconhecimento da prévia e revogação sem confirmar. NVDA 2026.3, Speech Viewer e sintetizador silence; mouse junto de Tab/Enter. Domínio e arquivo sintético preservados, user_version4, zero provider/vault, processo do aplicativo encerrado. Hash do domínio: bdd30db8f1a3c7497bca39ad8d272408d5a747b8988c4b758fd9b0d51614fa7d. O aviso e o estado inválido foram vistos, mas seu anúncio automático não foi estabelecido. Não cobre percurso completo só por teclado, avaliação auditiva, todos os painéis ou conformidade WCAG; nenhuma confirmação de salvar/revogar/derivar/executar foi enviada.

## Histórico de integração e revisão

O cluster de IA agora vincula contexto, investigação e recibos de solicitações à referência imutável do escopo. A revisão de custo continua válida quando o mesmo escopo é apenas congelado; uma revisão diferente invalida o contexto. Cada perspectiva verifica novamente a autorização antes do envio. Histórico e consumo de chamadas já iniciadas permanecem registrados.

A [revisão estática independente de IA](../.local/scope-ai-0.1.3-review.md) encontrou dois problemas no caminho de revogação: o mutex de inferência bloqueava a retirada de autorização durante uma chamada, e falha no vault também podia impedi-la. Ambos foram corrigidos no código e receberam fixtures de regressão. A revisão inspecionou os testes, mas não os executou; depois eles passaram na suite Rust294. As provas nativas acima permanecem distintas da revisão e das fixtures de provedor.

Revisar evidências já armazenadas, inclusive registrar candidatos factuais, continua possível depois de revogar um escopo congelado. Isso não permite importar dados novos, adquirir evidências de alvo ou enviar conteúdo ao provedor. Revogação não promete cancelar nem estornar uma requisição externa já enviada.

Os helpers nativos geral/HTTP foram adaptados para criação com métodos explicitamente selecionados. O novo `native-scope-smoke.mjs` usa legado criado pelo binário preservado 0.1.2 e um HEAD loopback próprio. A sintaxe passou antes dos nove gates reais acima. O orquestrador `check-foundation.ps1` passou validação sintática e inclui o novo gate; essa verificação isolada de sintaxe não certifica o produto.

A [primeira compilação](../.local/scope-first-compile.log) revelou sete erros de integração, corrigidos antes da rodada seguinte. Durante a edição, um script com codificação inadequada truncou `scope.rs`; o arquivo foi restaurado pelo replay determinístico dos patches originais e deltas da sessão, sem reconstrução manual. A conferência independente do trecho de handlers recuperado identificou somente as duas flags de status adicionadas deliberadamente. Releases preservadas e dados do usuário não foram alterados por esse incidente.

A [primeira suíte completa do núcleo](../.local/scope-first-suite.log) compilou a fonte restaurada em 50,63 s e executou 275 testes: 269 passaram e seis falharam, em 7,65 s. Passaram as regressões de revogação durante inferência e ponte Kali, vault ilegível, preservação da revisão de custo ao congelar e leitura de investigação legada. Duas fixtures tentavam revogar rascunho; foram corrigidas para preparar/congelar antes da revogação. As outras quatro falhas envolviam expectativa de versão, digest legado e fixtures de remediação. A [rodada291](../.local/scope-second-suite.log) passou depois, mas antecedeu os REDs de expurgo/MCP. Somente a suite294 e o build final incorporam todos esses ajustes; resultados anteriores permanecem históricos.

O frontend focal existente começou com 35 aprovados/uma falha: uma fixture de plano HTTP novo omitia ScopeRef. Após corrigir a fixture, focais 28 e 49 passaram, seguidos da suite completa142; recibos constam no checkpoint do frontend. O bundle final de 501,75 kB emitiu aviso informativo de tamanho do Vite, sem falha de build e sem orçamento de bundle definido no projeto.

Os testes Python do ambiente passaram33/33 em1,144s ([log](../.local/environment-helper-0.1.3-tests.log)). O helper MCP no Windows executou22 casos e ignorou8 dependentes de Linux em0,109s ([log](../.local/kali-helper-0.1.3-windows-tests.log)); não se declara30 casos executados nesse host Windows. A prova Linux anterior permanece no checkpoint0.1.2, distinta desta rodada.
