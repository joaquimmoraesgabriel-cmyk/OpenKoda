# Preparação R2 e publicação do site — 14/11/2026

## Resultado verificado

- Sites: versão pública **18**, fonte `867b48f6ea0102c6afb44ed4322cf2ead4aeeb2f`, publicação `appgdep_6af9037322f88191af27cf0481e6dd11` concluída.
- URL preservada: https://openkoda-desktop.steelyrowan1.chatgpt.site/ . O trabalho do domínio principal permanece separado, com Dot.
- Hero central OpenKoda, Weight Wave no subtítulo, campo orbital horário com máscara central, Scramble na introdução e Goo Morph na seção de guias.
- Removidos os botões de pausa, faixa da edição e linhas decorativas solicitadas. Movimento reduzido do sistema, suspensão fora da tela e em aba oculta continuam disponíveis.
- Aviso do download distingue a ausência de Authenticode reconhecido pelo Windows da assinatura obrigatória do atualizador. Sem orientação para desligar antivírus.
- Site: **37 testes passaram**. Inspeção visual em 1280×720 claro/escuro, 1366×768 e 390×844. Na largura móvel não foi observado overflow horizontal; a mensagem da demonstração recebeu a resposta preparada, sem API.
- Capturas locais em `app/.local/site-orbit-published-20261114.png` e `app/.local/site-download-warning-20261114.png`.

## Cloudflare

- Criado o subdomínio Workers `openglitch-br` e publicado o Worker `openkoda-updates`, módulos `worker.mjs` e `policy.mjs`.
- Origem de preparação: https://openkoda-updates.openglitch-br.workers.dev . Cliente de produção continuará exigindo `https://updates.openglitch.com.br` quando a migração for ativada.
- Upload confirmado em 2026-11-15T01:00:20Z, deployment `89c3ea86859440699cf493dda9bfb6db`, compatibility date `2026-11-14`.
- Plano Workers gratuito preservado. A API não permite limite customizado de CPU nesse plano; a publicação usa os limites nativos do Free, sem contratação de plano pago.
- Logs/traces amostrados em 0,1, sem persistência e sem logs automáticos de invocação; query strings removidas. Nenhum segredo ou token de usuário configurado.
- `UPDATES_ENABLED=false`. O endpoint permanece em preparação; os bindings foram acrescentados após o usuário ativar o R2, conforme a seção seguinte.

Verificação HTTPS real (15/11/2026 UTC):

| Requisição | Resultado |
| --- | --- |
| `/health` | 200, `status:ok`, `updatesEnabled:false` |
| `/v1/updates/stable/windows/x86_64/1.1.1` | 204, `X-Koda-Updates-Paused:true` |
| `/v1/updates/early/windows/x86_64/1.1.1` | 204, `X-Koda-Updates-Paused:true` |
| `/v1/updates/experimental/windows/x86_64/1.1.1` | 204, `X-Koda-Updates-Paused:true` |

Isso prova carregamento dos módulos no runtime real e comportamento de pausa; **não** prova download via R2 ou autorização financeira real.

## R2 ativado e configurado pelo coordenador

Após a confirmação do usuário, a consulta R2 passou a responder com sucesso. Criado o bucket `openkoda-desktop-updates` (Standard, localização retornada `ENAM`) em 2026-11-15T01:16:24Z. Acesso `r2.dev` desativado e lista de domínios públicos vazia, ambos conferidos pela API.

Locks por prazo indeterminado nos prefixos `artifacts/` e `releases/` foram salvos e lidos de volta. Os controles `channels/stable.json`, `channels/early.json` e `channels/experimental.json` foram gravados e lidos do R2 real: todos com `enabled:false`, `rolloutPercent:0` e `release:null`.

Worker atualizado com bucket privado, Rate Limiting de 60 requisições/IP/60 segundos e endpoint/chave **publicável** Supabase. Nenhuma chave `service_role` nem token pessoal. Deployment `87bdf925302b4931b0ec279b68452839`; `/health` permaneceu 200 com `updatesEnabled:false`.

A revisão automática rejeitou a ativação temporária do serviço antes de validar o instalador e enviar os artefatos. A alternativa executada manteve `UPDATES_ENABLED=false` e configurou somente bindings e controles pausados. Não houve tentativa de contornar a rejeição.

## Limites e próximos passos

O candidato desktop **1.1.2** foi empacotado com 182.274.291 bytes. O preparador
executou a validação criptográfica nativa com o plugin Tauri e a chave pública
de produção, aceitando assinatura e versão. O SHA-256 do instalador é
`4d085dd87210a232bc06a2ae0aede5d5ce1b3448b1f57ce169017294e7a31bca`.
O manifesto preparado tem SHA-256
`450b490d5a26565d3e0e2440381010d7a2c135c65b5d4697f2c46ec9af3c94d2`.
O plano de upload está em `app/.local/r2-prepared/`; foi gerado sem ativação
e sem upload dos binários. Authenticode continua `NotSigned`.

- O bloqueio inicial R2 HTTP 403/código 10042 foi resolvido pela ativação feita pelo usuário. O coordenador não aceitou termos nem informou dados de pagamento em seu nome.
- Ainda falta enviar os artefatos com checksum/metadados, validar GET/HEAD/Range com o serviço ativo e revalidar acesso de cada plano com contas de teste. Os bindings configurados não equivalem a esses testes concluídos.
- Validar `updates.openglitch.com.br` com HTTPS, só então habilitar a origem Cloudflare no cliente e repetir a atualização instalada com comparação de projetos/conversas.
- O download público permanece em **1.1.1**; o pacote 1.1.2 está preparado para rascunho, sem promoção ou substituição de versão já publicada.
- Testes locais do servidor/preparo: **43 passaram**; R2 e Supabase são simulados nessa suíte. Sem uso de API de IA e sem Asaas nesta tarefa.
- Revisão independente somente de leitura do Worker, cliente nativo e preparador não identificou falhas concretas novas em autenticação, redirects, fallback ou assinatura. Essa revisão não substitui a validação instalada/financeira real.
