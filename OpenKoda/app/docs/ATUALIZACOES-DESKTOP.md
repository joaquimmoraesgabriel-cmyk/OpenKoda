# Atualização do desktop pelo GitHub

O identificador público continua `com.openkoda.desktop`. Os dados continuam em
`%USERPROFILE%\.openkoda`, fora da pasta substituída pelo instalador NSIS.
Alterar nome/identificador/diretório entre releases quebra essa continuidade.

## Fluxo implementado

O app consulta o canal ao iniciar e em **Configurações → Atualizações**. O usuário
escolhe baixar, adiar e instalar/reiniciar. Não existe instalação silenciosa ao
fechar. As falhas de rede deixam o aplicativo utilizável e permitem tentar de novo.

O renderer chama somente `desktop_update` com `status`, `check`, `download`,
`install` ou `defer`. Não fornece endpoint, caminho, comando de instalação ou
chave. Somente a janela local `main` recebe essa permissão; os comandos genéricos
do plugin updater não recebem permissão no frontend.

O canal estável é:
`https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/releases/download/desktop-updates/latest.json`.
A release dedicada evita confundir o canal com a definição de “latest” do GitHub,
que pode excluir pré-lançamentos. Os instaladores têm tags imutáveis de versão.

A consulta usa TLS e timeout de 20 s. O download aceita somente releases deste
repositório, segue apenas redirects HTTPS dos hosts oficiais GitHub, tem limite
de 384 MiB e prazo de 10 min. A assinatura é validada antes de salvar e novamente
antes de instalar. `requireSignedVersion: true` exige a versão no comentário
autenticado do minisign; editar somente a versão do JSON não transforma um
instalador antigo em atualização nova. Isso requer CLI ≥2.12 e plugin com suporte
à validação de versão; as dependências efetivas ficam no lockfile.

Referências: [updater Tauri](https://v2.tauri.app/plugin/updater/),
[CLI e assinatura de versão](https://v2.tauri.app/release/@tauri-apps/cli/).

## Preservação e recuperação

Na instalação o núcleo fecha atomicamente a entrada de novas operações, exige
que os trabalhos estejam quiescentes e adquire a trava de manutenção. Enquanto
isso cria snapshots dos bancos SQLite do perfil ativo e da instalação pela Online
Backup API, incluindo dados ainda presentes no WAL, e verifica `quick_check`.
Os backups ficam em `.openkoda/update-backups/<id>/` com manifesto e hashes.
Falha de backup ou assinatura impede iniciar o instalador. Falha ao iniciar NSIS
solta a trava e mantém o app utilizável.

Essa cópia é local e privada, inclusive com credenciais já criptografadas por
DPAPI; não é um pacote de suporte nem um export para compartilhar. Não é enviada
à nuvem. Repositórios, anexos e perfis inativos permanecem na pasta de dados,
sem exclusão nem migração em massa. Backups anteriores não são apagados
automaticamente nesta versão. O limite atual do backup de cada SQLite é 128 MiB;
um banco maior bloqueia a instalação e requer revisão dessa política antes de
atualizar. Não copie um `.sqlite3` aberto por conta própria sem considerar WAL.

O rollback de código deve ser uma nova release com versão maior e código da
última versão estável. O updater recusa downgrade. Uma restauração de dados
exige fechar o app, guardar o estado atual e restaurar uma cópia verificada; não
é feita automaticamente por uma falha genérica. A compatibilidade do schema
precisa ser verificada antes de republicar código antigo.

## Chaves e build

`scripts/init-update-signing.ps1 -Initialize` cria a chave uma única vez. O segredo
fica criptografado para o usuário do Windows via DPAPI em
`%USERPROFILE%/.openkoda-release-signing/production/private-key.dpapi`, com ACL do
usuário e SYSTEM, fora do checkout e da virtualização de AppData por processos
MSIX; somente a chave pública entra em
`tauri.conf.json`. Limpar artefatos do projeto não apaga esse cofre. Uma cópia do arquivo DPAPI **não substitui um backup
portátil**: mantenha a conta do Windows e prepare exportação protegida para um
gerenciador de segredos antes de depender dessa máquina em produção. Não cole
essa chave em issues, chats, logs ou frontend.

Os scripts ainda leem os cofres legados em AppData ou `.local/update-signing`
quando o cofre estável não existe. A migração preservou essas cópias criptografadas;
não gerou outra chave nem invalidou a chave pública distribuída.

`package-windows.ps1` carrega o segredo apenas no processo do build e restaura o
ambiente ao terminar. No CI, configure `TAURI_SIGNING_PRIVATE_KEY` e a senha
correspondente nos secrets do environment `desktop-release`; não em variáveis
públicas. O workflow `windows-update-candidate.yml` exige também certificado de
assinatura Windows e produz um candidato para revisão, sem publicar o canal.
Assinatura Tauri e Authenticode são controles distintos; ter `.sig` não elimina
o aviso do Windows para um executável sem certificado. Certificado comercial
Windows e backup portátil da chave são etapas externas de lançamento.

Durante o pré-lançamento, `prepare-update-manifest.mjs --prerelease` permite um
instalador com assinatura Tauri e sem Authenticode, identificado explicitamente
no manifesto e nas notas do GitHub como pré-release. Esse modo não certifica
confiança do Windows nem remove seus avisos. A publicação estável certificada
continua exigindo Authenticode válido. `--internal-test` é reservado a builds
isolados, com outra chave e outro canal; eles nunca entram no canal do produto.

## Publicação em etapas

1. Compilar, testar frontend/Rust e instalar o candidato em um perfil de ensaio.
2. Conferir receipt, SHA-256, assinatura Windows, `.sig` e versão do binário.
3. Publicar o instalador e `.sig` na tag `desktop-vX.Y.Z` sem substituir artefatos
   de versões anteriores.
4. Gerar `latest.json` com `prepare-update-manifest.mjs --rollout 1` e publicar
   esse arquivo na release `desktop-updates` após a revisão.
5. Observar falhas reais de atualização/crash antes de passar para 10% e 100%.
   O app tem coorte local determinística; nenhum identificador é enviado. Não há
   telemetria de saúde nem promoção automática implementada.
6. Para pausar, publicar `rolloutPercent: 0`. Isso impede novas ofertas; uma
   atualização já baixada continua disponível à pessoa que a aceitou. Para
   revogar um artefato comprometido, remova o arquivo e publique instruções e
   uma versão corretiva; pausa de coorte não é revogação criptográfica.

## Ensaios isolados

`build-update-rehearsal.ps1` prepara versões 1.0.9 → 1.1.0 com nome/identificador
`OpenKodaUpdateTest`, dados de teste definidos **na compilação** e chave separada
`.local/update-test-signing`. O feature `updater-test-harness` não está no build
público. O canal de ensaio é `desktop-updater-rehearsal`, nunca `desktop-updates`.
O script não publica nem instala. O `rehearsal.json` informa todos os caminhos.
O gerador de manifesto recusa encaminhar um build isolado para publicação normal.

Versões antigas sem updater (incluindo 1.0.0) precisam de uma instalação inicial
da primeira versão com este recurso. Depois disso, as próximas versões podem
ser baixadas e aplicadas dentro do app.
