# Serviço de atualizações OpenKoda

Worker ESM sem dependências para entregar os instaladores assinados pelo Tauri a partir de um bucket R2 **privado**. O cliente nativo continua verificando a assinatura e o vínculo da versão antes de instalar. Este serviço não assina instaladores, não recebe chaves privadas, não edita projetos e não concede créditos de IA.

## Contrato HTTP

Origem HTTPS fixa em `PUBLIC_ORIGIN`. Para a preparação atual: `https://openkoda-updates.openglitch-br.workers.dev`. O endereço de produção deve coincidir com o host permitido pelo cliente nativo; não substituir hosts ou redirecionar o Bearer dinamicamente.

| Rota | Resultado |
| --- | --- |
| `GET /health` | Estado do serviço e indicação de distribuição ligada/desligada, sem segredos. |
| `GET /v1/updates/{channel}/{target}/{arch}/{current_version}` | JSON Tauri dinâmico, ou `204` quando não houver atualização elegível. |
| `GET /v1/downloads/{channel}/{version}/{target}/{arch}/{sha256}/{filename}` | Corpo do instalador do release atual, em fluxo. |
| `HEAD` nas mesmas rotas | Mesma autorização; download consulta metadados sem ler o instalador. |

Exemplo de resposta do manifesto:

```json
{
  "version": "1.1.2",
  "notes": "Notas do release publicado.",
  "pub_date": "2026-11-14T12:00:00Z",
  "url": "https://updates.example.invalid/v1/downloads/stable/1.1.2/windows/x86_64/HASH/OpenKoda_1.1.2_x64-setup.exe",
  "signature": "ASSINATURA_TAURI_ORIGINAL",
  "rolloutPercent": 100
}
```

O servidor já aplicou o rollout; `rolloutPercent:100` evita uma segunda amostragem no cliente. A versão usa SemVer, incluindo prereleases; versão igual/mais antiga nunca é oferecida. `stable` não aceita release prerelease. Plataformas permitidas são Windows/Linux/macOS (`windows`, `linux`, `darwin`), arquiteturas `x86_64`/`aarch64`; somente entradas realmente presentes no release têm instalador. Hoje publicar apenas a plataforma efetivamente construída/testada.

Parâmetros de query não são aceitos. O cliente não escolhe chave R2, URL remota, plano, ID de usuário ou percentual. A origem retornada é a configuração do servidor, nunca `Host`/`X-Forwarded-Host` refletido.

## Acesso aos canais

| Canal | Autorização |
| --- | --- |
| `stable` | Público, para uso gratuito e todos os planos. Não precisa enviar JWT. |
| `early` | Conta autenticada com plano financeiro **Pro ou Ultra ativo**. |
| `experimental` | Conta autenticada com plano financeiro **Ultra ativo** e `X-Koda-Experimental: true`, somente após escolha explícita no aplicativo. |

Nos canais privados, enviar `Authorization: Bearer <access_token>` no manifesto **e em cada download/HEAD/Range**. A sessão fica no núcleo nativo, não no renderer. Não há credencial de download na URL, cookie, redirecionamento ou cache de autorização.

O Worker faz `POST {"action":"account"}` ao endpoint existente `SUPABASE_URL/functions/v1/openkoda`, com o Bearer e a chave publicável configurada. Esse endpoint valida o JWT no Supabase Auth `/auth/v1/user`, exige conta confirmada e consulta o SQL financeiro. `plan` e `subscription` são derivados de `openkoda_private.period`; a simulação fica em campo separado. O Worker ignora `simulation`, `entitlements`, claims, metadados de usuário e cabeçalhos com plano; confere status e início/fim do período ao concluir a consulta.

O Worker não precisa de `service_role`. A indisponibilidade do Supabase nega a autorização temporariamente. O mesmo token é revalidado em cada nova requisição; uma transferência já admitida pode terminar após expiração ou pausa. Logout/revogação imediata além da validade do JWT depende da política do Supabase Auth existente: a API atual não consulta diretamente `auth.sessions`. Não afirmar revogação instantânea de JWTs já emitidos.

`401` significa autenticação necessária; `403`, canal indisponível. Essas respostas não revelam plano, dados da conta nem metadados do instalador. Quando renovar a sessão, refazer a consulta e assegurar que a identidade não mudou. Nunca usar fallback GitHub para canais privados.

## Armazenamento e publicação

```text
channels/stable.json
channels/early.json
channels/experimental.json
releases/<sha256-dos-bytes-do-json>.json
artifacts/<sha256-do-instalador>/<filename>
```

Somente os pequenos ponteiros `channels/*.json` são mutáveis. Releases e instaladores são imutáveis, endereçados por conteúdo. O hash do JSON é recalculado em toda leitura; não reformatar o JSON após calcular o nome. A assinatura deve ser exatamente a gerada pelo pipeline Tauri, sem alterar o comentário assinado que vincula a versão. O Worker verifica formato/limites, não substitui a verificação criptográfica do cliente.

Cada objeto de instalador deve conter `customMetadata.sha256` com o digest completo. No upload, usar checksum SHA-256 da API R2/S3 além desse metadado, criação condicional sem sobrescrita e locks R2 nos prefixos `releases/` e `artifacts/`. Configurar a credencial de publicação com menor escopo e nunca oferecê-la ao Worker ou ao aplicativo. O Worker não possui rotas de upload, exclusão ou listagem.

1. Construir e verificar instalador + `.sig` pelo pipeline existente.
2. Gerar o JSON de release e o ponteiro inicialmente pausado. O script `scripts/prepare-release.mjs` é mantido pelo coordenador da publicação.
3. Enviar o instalador com checksum e metadados; enviar o JSON imutável com o nome baseado em seu hash.
4. Conferir downloads e assinaturas no ambiente de QA, sem trocar a chave confiável de produção.
5. Publicar o ponteiro do canal, inicialmente `enabled:false`, `rolloutPercent:0`.
6. Após validação, habilitar a distribuição e aumentar o percentual conservando `rolloutSeed`. Trocar a seed muda a amostra.

Preparar um pacote verificado, a partir de `OpenKoda/updates/`:

```powershell
node scripts/prepare-release.mjs --receipt ../app/.local/package-windows.json --channel stable --output ../app/.local/r2-prepared
```

O preparo confere hashes antes/depois, chama a verificação nativa offline da assinatura/versão oficial e produz o JSON imutável, controle pausado e plano de upload. Não faz upload nem habilita canal. Ao enviar os arquivos, conferir novamente os checksums do plano; o caminho local de origem não é uma cópia imutável.

Se a máquina usa um cache Cargo transportado, informar explicitamente `--cargo-config ../app/.local/cargo-transport.toml` com uma configuração local confiável. A opção não dispensa a verificação da assinatura; o teste nativo precisa executar e passar, e uma seleção de zero testes é rejeitada.

Exemplos em `examples/` são deliberadamente desabilitados/inválidos para publicação acidental. O bucket precisa permanecer sem acesso público via `r2.dev` **e sem domínio público R2**; usar apenas o host do Worker. Configurar IAM/locks no deploy, pois endereçamento por hash sozinho não impede um administrador de substituir objetos.

O rollout parcial usa `X-Koda-Installation-Id`, UUID persistido localmente pelo aplicativo. Sem UUID válido, não participa de rollout parcial. É distribuição gradual, não controle de licença: uma pessoa pode reinstalar/mudar seu identificador, mas isso nunca substitui a autorização de plano dos canais privados.

Uma promoção troca o ponteiro atomicamente. Se mudar antes de um novo download, o URL antigo pode retornar `410`; se o objeto mudar entre metadados/leitura, retorna `409`. Refazer a consulta do manifesto, sem repetir indefinidamente o mesmo URL. Uma transferência já aberta fica presa ao objeto lido por ETag e pode concluir; a assinatura continua obrigatória.

## HTTP, limites e pausa

- Instalador de até **384 MiB**, manifesto até **64 KiB**, controle até **8 KiB**, resposta da conta até **64 KiB**; notas até 8.000 bytes. Os JSONs são contados durante a leitura. Instaladores são transmitidos sem carregar o arquivo inteiro na memória.
- `Range` único: prefixo, aberto ou sufixo; `206` com `Content-Range`. Múltiplos intervalos e intervalos inválidos retornam `416`. `HEAD` ignora `Range`. `If-Range` suporta ETag forte; outros valores recebem corpo completo. `304` por ETag também passa pela autorização.
- `Cache-Control: no-store, private` em todas as respostas. Não habilitar Cache Rules/Cache API que contornem o Worker para esses caminhos.
- `REQUEST_LIMITER` obrigatório enquanto a distribuição está ligada; sugestão inicial: 60 requisições por IP a cada 60 segundos, usando a identificação de rede de Cloudflare. A chave é hash do IP. O binding tem o comportamento de consistência/localidade da plataforma, não é um teto financeiro global.
- `UPDATES_ENABLED=false` para emergência: manifesta `204` com `X-Koda-Updates-Paused:true`; downloads novos recebem `503`. O cliente não deve interpretar `204` como falha nem buscar outro canal/fonte. A pausa funciona antes de configurar bucket/limiter.
- Canal ausente, desligado ou com 0% também não oferece atualização. Erros de configuração/serviço retornam `503`; cliente deve usar espera antes de tentar novamente, nunca loop rápido.
- Cancelar assinatura impede novos downloads privados após a consulta refletir o fim do direito. Não remove um instalador já obtido nem promete esconder recursos compilados no binário.

Versões legadas que só consultam o manifesto GitHub não conhecem a pausa do Worker. Enquanto a ponte GitHub existir, uma emergência também precisa pausar seu manifesto. Nunca publicar builds privados como assets públicos do GitHub.

## Deploy e observabilidade

`deployment.example.json` é inventário de bindings/variáveis, **não** configuração Wrangler. Não há pacote, instalação de CLI nem recurso remoto criado por esta pasta. Usar a CLI/API Cloudflare vigente do projeto para configurar o Worker, gerar tipos reais de bindings se adotar TypeScript e validar o esquema do deploy.

O upload ESM precisa incluir **`worker.mjs` e `policy.mjs`**, com `worker.mjs` como módulo principal. Não enviar somente o entrypoint e deixar a importação relativa sem módulo.

Usar compatibilidade `2026-11-14`, sem `nodejs_compat` (o Worker só usa APIs web). Habilitar Logs **e** Traces com amostragem, retenção curta e filtros de credenciais. Logs de aplicação incluem apenas códigos estáveis; nunca URL completa, Bearer, chave, corpo do Supabase, ID pessoal ou stack de dependência. Antes de ativar observabilidade externa, confirmar retenção e coleta automática da conta. Confirmar região real do Supabase e escolha de placement; não inferir região a partir do hostname. Não publicar `SUPABASE_PUBLISHABLE_KEY` junto de dados de sessão; nunca configurar chave de serviço, segredo OAuth ou chave de assinatura privada no Worker.

O Worker foi publicado **pausado** em `https://openkoda-updates.openglitch-br.workers.dev`, no plano gratuito. O runtime real retornou saúde 200 e pausa 204 nos três canais. O bucket privado `openkoda-desktop-updates`, locks de release/artefato, Rate Limiting e conexão Supabase estão configurados. Os três controles pausados foram gravados e lidos do R2. Não há instalador distribuído pelo serviço ainda. Evidência: [preparação de 14/11/2026](../docs/evidence/r2-updates-preparation-2026-11-14.md).

Ainda faltam testes ativos de Rate Limiting e imutabilidade, domínio definitivo/HTTPS, download Windows completo e plano financeiro de conta real. Testes Node usam fakes do R2 e do Supabase; não certificam esses fluxos. Não foi gasto crédito de IA.

## Testes locais

```powershell
node --test OpenKoda/updates/tests/*.test.mjs
```

Cobertura: manifesto Tauri, versões/rollback, canais por plano real, simulação sem privilégio, ausência/expiração de autenticação delegada, opt-in, revogação entre check/download, concorrência de identidades, tamper de release/artefato, limitação de tamanho, pausa, amostragem, host/rotas/query, HEAD/Range/ETag, rate limit e falha fechada.

Fontes consultadas: [R2 Workers API](https://developers.cloudflare.com/r2/api/workers/workers-api-reference/), [Tauri updater](https://v2.tauri.app/plugin/updater/), [Supabase getUser](https://supabase.com/docs/reference/javascript/auth-getuser) e [changelog Supabase](https://supabase.com/changelog.md). Contrato financeiro conferido nas migrations e em `supabase/functions/openkoda/handler.mjs` deste repositório.
