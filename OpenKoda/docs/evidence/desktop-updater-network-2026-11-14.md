# Atualização 1.1.0 → 1.1.1 — diagnóstico de rede

Data: 14 de novembro de 2026. Este registro distingue a falha de transporte
da preservação dos dados e não declara uma instalação concluída.

## Baseline privado antes de abrir o aplicativo

O executável instalado é 1.1.0, em `AppData/Local/OpenKoda/openkoda.exe`.
A base do produto continua em `%USERPROFILE%/.openkoda`.

Uma cópia privada dos 11 SQLite foi criada pela SQLite Online Backup API,
incluindo dados presentes no WAL. Cada cópia passou por `quick_check`; schema,
contagens e hashes de linhas por tabela foram comparados com a origem.
Uma segunda leitura confirmou que os dados não mudaram durante o backup.
Os metadados locais relevantes também foram copiados e verificados.

Baseline: **10 projetos, 31 avaliações/conversas, 35 mensagens e 21 solicitações**.
IDs são ofuscados no relatório privado; textos, credenciais e conteúdo de cofres
não foram publicados. O backup inclui ciphertext para recuperação no mesmo
perfil Windows e permanece em `.local/`, ignorado pelo Git.

Após as tentativas frustradas, uma nova comparação preservou exatamente o
estado principal, mensagens, solicitações e metadados. Somente o banco
`commercial-account.sqlite3` mudou durante a retomada da sessão; isso foi
registrado separadamente, sem declarar igualdade integral de todos os bancos.

## Sintoma e reprodução

A interface 1.1.0 apresentou falha ao consultar o GitHub. Uma nova consulta
conseguiu responder enquanto o canal estava pausado, mas a tentativa seguinte
voltou a falhar. O canal retornou a `rolloutPercent: 0`; não se deve interpretar
uma oferta pausada como inexistência da release 1.1.1.

O endpoint público verificado foi o manifesto `latest.json` da release
`desktop-updates` no repositório oficial. Nenhuma consulta baixou o instalador,
alterou credenciais ou consumiu uma API de IA.

| Cliente / condição | Resultado observado |
|---|---|
| curl padrão, HTTP/1.1 | Redirect 302 seguido de timeout em 25,66 s, sem conteúdo do manifesto |
| curl com IPv4 | Mesmo redirect seguido de timeout em 20,20 s |
| curl com IPv6 | Sem resolução IPv6 utilizável de `github.com` nesta rede |
| GitHub CLI `release download` | Timeout de leitura no CDN oficial, embora tivesse funcionado em consulta anterior |
| Outro IP anunciado pelo DNS oficial, final `.111.133` | Conexão reiniciada após 19,90 s |
| Outro IP anunciado pelo DNS oficial, final `.108.133` | Timeout após 20,23 s |
| Nova consulta padrão, minutos depois | Timeout após 20,38 s, sem conteúdo |
| Plugin oficial Tauri, configuração de produção e timeout de 20 s | `Reqwest { kind: Request, source: TimedOut }` após 20.015 ms |

Os erros ocorreram em `release-assets.githubusercontent.com`, após o redirect
do GitHub. As consultas a outros endereços usaram somente IPs retornados pelo
DNS oficial, limitados à requisição do diagnóstico, preservando hostname e
validação TLS. Não foram alterados DNS, proxy, hosts, certificados ou variáveis
do aplicativo instalado. URLs temporárias assinadas foram omitidas deste registro.

## Conclusão delimitada

A falha não foi exclusiva do OpenKoda: curl e o cliente oficial GitHub também
falharam no mesmo caminho de download. Há evidência de indisponibilidade ou
intermitência da rota para o CDN nesta máquina/rede; os testes não localizam
o responsável entre rede local, provedor de Internet e infraestrutura remota.

O teste nativo de atualização precisa ser retomado quando o endpoint público
responder normalmente. Aumentar o rollout não corrige a conectividade. Uma
instalação manual, um manifesto injetado ou uma mudança de DNS não substituiriam
a validação solicitada do fluxo do próprio aplicativo.

O teste manual `updater_probe_production_feed`, ignorado por padrão em
`OpenKoda/app/src-tauri/tests/updater_signatures.rs`, consulta o mesmo endpoint
usando o plugin oficial e o timeout de 20 s, sem baixar um instalador ou abrir
o perfil real. A reprodução falhou com timeout de transporte, sem erro de
interpretação do manifesto. O diagnóstico remove URLs dos detalhes do erro.

Comando executado em `OpenKoda/app/src-tauri`:

```powershell
cargo --config ../.local/cargo-transport.toml test --offline --test updater_signatures updater_probe_production_feed -- --ignored --nocapture
```

A configuração local acima apenas seleciona o cache Cargo já utilizado neste
computador; não integra o aplicativo nem seus requests de atualização. Em uma
máquina com o registro Cargo padrão, omita `--config`. A execução exige acesso
à rede. O resultado observado foi **1 teste falhou por timeout, nenhum passou**;
isso é a reprodução do problema externo, não uma aprovação da atualização.

A execução normal do binário compilado de `updater_signatures` aprovou os dois
testes offline existentes (assinatura/corrupção/versão e falha de conexão
recuperável), mantendo os dois ensaios opt-in ignorados. Não houve mudança
especulativa no transporte do produto, no instalador ou nas proteções TLS.

Estado final desta investigação: canal restaurado a **0%**, aplicativo ainda
na versão **1.1.0**, backup privado íntegro e dados principais preservados.
O ensaio real 1.1.0 → 1.1.1 continua pendente; não foi substituído por instalação
manual nem por um teste com endpoint injetado.

Uma última sondagem, após o restante da validação de interface, executou somente
o teste opt-in já compilado, com o mesmo plugin oficial e timeout de 20 segundos.
O resultado permaneceu `Reqwest { kind: Request, source: TimedOut }` após
**20.031 ms**: nenhum teste passou e um falhou. A consulta não baixou instalador,
não alterou o canal e não abriu o perfil real. As tentativas de rede foram
encerradas após essa confirmação; a atualização nativa continua pendente.
