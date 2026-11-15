# Desktop 1.1.2 — preparação dos canais de atualização

Implementação local preparada em 14 de novembro de 2026. Este documento não
declara atualização instalada, publicação ou disponibilidade do R2.

## Limites de confiança

O renderer pode escolher somente um verbo e um canal enumerado: `stable`,
`early` ou `experimental`. Não recebe tokens e não fornece URLs, caminhos,
chaves públicas ou argumentos do instalador. O canal experimental exige uma
confirmação explícita. Alterar o canal não altera plano, assinatura ou cota.

A sessão existente do Supabase continua no cofre DPAPI do perfil. O cliente
nativo renova a sessão usando o fluxo já existente e vincula uma oferta privada
à identidade da sessão. Trocar de conta ou sair impede instalar um pacote
privado em cache sob outra conta. O Worker confirma a assinatura comercial
vigente; a interface e os claims de plano não concedem acesso.

## Contrato de distribuição

Origem planejada e fixada no código: `https://updates.openglitch.com.br`.

- Manifesto: `/v1/updates/{channel}/windows/x86_64/{current_version}`.
- Artefato: `/v1/downloads/{channel}/{version}/windows/x86_64/{sha256}/{filename}`.
- Estável funciona sem login. Antecipado exige Pro ou Ultra; experimental exige
  Ultra e `X-Koda-Experimental: true`.
- Um UUID aleatório persistido em `update-preferences.json` identifica a coorte
  de distribuição. O caminho do computador e conteúdo de projetos não são enviados.
- Bearer só é anexado pelo host nativo para canais privados, nessa mesma origem.
  Não há token em query strings. O cliente Cloudflare recusa redirects.
- HTTP 204 significa nenhuma oferta, inclusive pausa administrativa. Não ativa
  fallback. Erros 401/403, metadados inválidos ou excessivos também não o ativam.
- Somente o canal estável pode tentar o manifesto GitHub legado quando há falha
  de transporte ou erro 5xx. Canais privados nunca usam o fallback público.
- A leitura do manifesto tem limite de 128 KiB/20 segundos; o download tem
  limite de 384 MiB/10 minutos. Não há repetição automática do download.

O manifesto é restaurado com o plugin oficial Tauri, que exige assinatura com a
versão vinculada. O pacote Cloudflare também precisa corresponder ao SHA-256 da
URL. A assinatura minisign é verificada após o download e novamente antes de
instalar. A chave pública de produção e `requireSignedVersion` foram preservados.

Antes de instalar uma versão privada em cache, um HEAD autenticado confirma
novamente o acesso no Worker. Uma promoção, retirada ou pausa entre consulta e
download é recuperável por “Verificar novamente”. Nenhum downgrade é habilitado.
Voltar de um canal antecipado ao estável exige aguardar uma versão estável maior.

## Preservação local

O instalador mantém o identificador e diretório de dados existentes. A instalação
continua bloqueando novas operações, exigindo quiescência e ausência de manutenção
pendente. A cópia de segurança usa SQLite Online Backup API, verifica integridade
e preserva cofre cifrado e preferências. A preferência de atualização passa a
integrar os metadados copiados. O ensaio não modifica o perfil pessoal.

## Ponte para 1.1.0 e 1.1.1

Os clientes antigos conhecem somente o manifesto GitHub e as URLs oficiais de
release GitHub. Por isso a migração exige publicar **uma nova versão 1.1.2**,
com o mesmo par de assinatura, no caminho compatível. Não é seguro substituir
o conteúdo da release 1.1.1 ou mudar seu URL para R2: os clientes antigos
recusariam esse host. Uma falha de rota para o CDN GitHub ainda pode bloquear
essa primeira atualização; preparar R2 não altera o binário já instalado.

O script existente `prepare-update-manifest.mjs` prepara a ponte GitHub. A
preparação de objetos R2 é feita por `OpenKoda/updates/scripts/prepare-release.mjs`.
Preparar arquivos não publica nem libera uma coorte.

## Estado de ativação e verificações

A constante `CLOUD_UPDATES_ENABLED` permanece `false` enquanto a origem de
produção não estiver pronta. O bucket R2 privado foi criado pelo coordenador,
mas o domínio ainda está pendente. O pacote mantém a consulta GitHub existente.
A habilitação do transporte Cloudflare exige nova compilação após verificar o
domínio, os downloads assinados e os controles de acesso. O estado de infraestrutura
está registrado em [preparação R2](r2-updates-preparation-2026-11-14.md).

Build TypeScript/Vite e `cargo check --lib` concluíram sem erro. Os **nove testes
Rust do atualizador passaram**, incluindo preservação de SQLite/WAL, quiescência,
origens, credenciais, opt-in, persistência da coorte, bloqueio de operações
intercaladas e decisões de fallback. Os **três testes headless do renderer
passaram**: opt-in obrigatório sem mutação financeira/token no IPC, gate desativado
e possibilidade de reconsultar uma oferta em cache. Esses três testes usam
respostas sintéticas e não comprovam autenticação ou instalação real.

A cobertura fica versionada em `app/src-tauri/src/updater.rs` e
`app/e2e/updater-channels.spec.ts`. Para reproduzir:

```powershell
# Em OpenKoda/app; o arquivo Cargo local seleciona o cache disponível nesta máquina.
npm.cmd run build
npx.cmd playwright test e2e/updater-channels.spec.ts --project="Edição de código" --workers=1
cd src-tauri
cargo --config ../.local/cargo-transport.toml test --offline --locked --lib updater::tests --jobs 2 -- --nocapture
```

O relatório local de navegador está em `app/test-results/results.json` e
`app/playwright-report/index.html` (3 esperados, 0 inesperados, 0 instáveis).

`npm audit --omit=dev` encontrou zero vulnerabilidades nas dependências de
produção. O empacotamento local terminou com código 0 às
`2026-11-15T01:42:16Z` (14 de novembro no Brasil), com estes artefatos:

- Receipt: `app/.local/package-windows.json`, versão e FileVersion `1.1.2`.
- Instalador: `app/src-tauri/target/release/bundle/nsis/OpenKoda_1.1.2_x64-setup.exe`.
- Tamanho: **182.274.291 bytes** (173,83 MiB).
- SHA-256: `4d085dd87210a232bc06a2ae0aede5d5ce1b3448b1f57ce169017294e7a31bca`.
- Assinatura minisign gerada no mesmo caminho, com sufixo `.sig` (436 bytes).
- `updaterSigned=true`, `isolatedUpdateTest=false`, `publicReleaseApproved=false`.

O build de release Rust concluiu em 18m22s. O diagnóstico do runtime empacotado
retornou `available=true`, sem componentes faltantes. O preparador R2 executou
o teste nativo `updater_verify_packaged_candidate` usando o plugin Tauri e a
chave pública de produção: assinatura e versão aceitas, um teste passou. O
SHA-256 foi conferido antes e depois da verificação; o preparador retornou
`updaterSignatureVerified=true` e `activated=false`. Isso não comprova download
ou instalação. Nenhum instalador foi executado neste ensaio e nenhum dado do
perfil pessoal foi alterado.

O Windows informa **Authenticode: NotSigned**. Essa pendência é distinta da
assinatura minisign do atualizador. Este pacote não é uma release pública
certificada. Não houve chamadas pagas a modelos de IA nesta preparação.

Referências primárias: [updater Tauri](https://v2.tauri.app/plugin/updater/) e
[sessões Supabase](https://supabase.com/docs/guides/auth/sessions). A API
`restore_update` foi conferida no código do plugin 2.14.1 fixado pelo Cargo.lock.
