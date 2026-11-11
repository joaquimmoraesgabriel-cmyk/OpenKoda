# Decisões de implementação

## Fundação E1

- Tauri 2 implementa o desktop escolhido. React/TypeScript cuida da apresentação e Rust da autoridade local.
- SQLite com WAL, synchronous FULL e transação de snapshot JSON versionado oferece persistência inicial. O schema é simples, mas não representa ainda o armazenamento paginado de grandes evidências previsto no mapa; essa evolução terá migração e testes antes de ampliar limites.
- Commands de aplicação restritos por manifesto/capability e checagem da janela. A rota request escolhe apenas ações de domínio conhecidas; não aceita comandos de terminal.
- O preview web usa dados separados e indica que é uma prévia; o gate nativo verifica IPC e SQLite reais.
- Dependências Node auditadas. Vitest inicial foi atualizado para 5.1.0 após auditoria detectar versões vulneráveis; primeira instalação corrigida retornou zero vulnerabilidades.
- Provedor confirmado pelo usuário: API da OpenAI. Modelo será escolhido nas configurações. Chaves não são lidas de arquivos pessoais da pasta pai automaticamente.

## Pré-requisitos observados

Node 24.19.0, npm 11.17.0, Rust/Cargo 1.99.1 e Build Tools MSVC disponíveis. WSL 2 já instalado; distribuições existentes docker-desktop e Ubuntu-24.04 preservadas. OpenKoda-Kali ainda ausente na inspeção inicial. Acesso WSL do sandbox retornou E_ACCESSDENIED; consulta fora do sandbox confirmou o estado real.

## Fontes técnicas

- [Pré-requisitos Tauri](https://v2.tauri.app/start/prerequisites/).
- [Capabilities](https://v2.tauri.app/security/capabilities/) e [commands](https://v2.tauri.app/develop/calling-rust/).
- [Kali WSL](https://www.kali.org/docs/wsl/wsl-preparations/) e [comandos WSL](https://learn.microsoft.com/en-us/windows/wsl/basic-commands).
- [OpenAI Responses](https://developers.openai.com/api/docs/guides/text) e [catálogo de modelos](https://developers.openai.com/api/reference/resources/models/methods/list).

O ensaio de conexão não transmite fontes reais nem pressupõe que um modelo listado suporte todas as ferramentas. A evidência de cada gate será registrada em IMPLEMENTACAO.md.

## Retomada da implementação

- O schema SQLite do núcleo passou a v2. A migração v1 valida o estado antes de atualizar JSON e versão na mesma transação; estados inválidos ou versões futuras não são resetados.
- Uma instância Windows mantém handle exclusivo de `instance.lock`. Reabrir após interrupção invalida aprovações pendentes e conserva execuções desconhecidas sem repeti-las. A reconciliação das operações nativas registra explicitamente os efeitos desconhecidos.
- Os primeiros adaptadores implementados são observações nativas delimitadas: inventário de arquivos/hashes e HTTP HEAD com DNS fixado, sem proxy herdado ou redirects. Eles não substituem o executor Kali, uma análise de vulnerabilidades completa, nem uma contenção de código hostil.
- A conversa OpenAI usa Responses, endpoint oficial fixo, HTTPS, limite de resposta e `store:false`. Isso não é uma declaração de retenção zero do provedor. Texto do modelo não recebe acesso a aprovação ou shell.
- Credenciais usam DPAPI por usuário; troca/remoção compartilham o limite de despacho e invalidam a conexão testada. Consumo e resposta/histórico são persistidos atomicamente; incompletude e timeout não viram sucesso. O contexto exige revisão explícita e pertence a uma única avaliação.
- Backup usa a API Online Backup do SQLite sob exclusão de operações do aplicativo. Histórico de IA pode ser preservado, mas vault e resultado do teste da conexão são removidos antes de gravar a cópia. Restaurar prepara uma pasta separada. A ativação revisada é agendada para o próximo início, valida o estado antes de trocar um ponteiro e preserva o perfil anterior; rollback prepara outra cópia. Credenciais não são restauradas.
- O instalador NSIS está configurado para o usuário atual. A configuração de empacotamento não comprova assinatura, atualização ou validação do instalador; esses gates continuam distintos.
- Em falhas locais do transporte Cargo, ferramentas temporárias podem buscar os índices oficiais pelo GitHub e os arquivos de crates.io, preservando os checksums do lockfile. O produto não incorpora esse servidor de desenvolvimento.
- A execução real pelo Codex mostrou virtualização do AppData para o cache do pacote MSIX que iniciou o processo. Por isso o domínio nativo usa a pasta `.openkoda` do perfil do usuário. Uma importação inicial copia os bancos anteriores com a API SQLite, valida-os e instala a pasta por rename, preservando a origem. Não há importação sobre pasta já existente. A imagem Kali nova fica nessa área estável.
- A chave indicada pelo usuário foi cadastrada por IPC no vault DPAPI, e uma solicitação Responses mínima confirmou o modelo `gpt-5-mini`. O arquivo original continua preservado e excluído do Git. A configuração é revalidada após reiniciar sem gerar nova chamada paga.

Cobertura, testes executados e lacunas ficam discriminados em [ACCEPTANCE-STATUS](ACCEPTANCE-STATUS.md). A matriz conserva as lacunas do mapa original e identifica a versão e a extensão real de cada prova.

## Integrações e recuperação verificadas

- O Kali dedicado foi instalado de imagem oficial conferida por SHA-256. A configuração efetiva exige usuário `openkoda` não root, interop desabilitado e ausência de mounts Windows; isso não constitui sandbox para código hostil. Nenhuma distribuição pessoal foi adotada ou removida.
- A manutenção APT tem canal root separado, versões fixadas pela simulação, locks e journal durável. Um timeout real bloqueou o ambiente; o plano de reparação encerrou apenas a distro gerida, preservou o journal e auditou o dpkg sem repetir APT. Uma nova atualização explícita e a instalação do pacote revisado `ripgrep=15.2.0-1` passaram depois.
- As chamadas de atualização geridas desabilitam apenas os índices Contents do apt-file e traduções, que o catálogo não utiliza. A validação das assinaturas e `--error-on=any` continuam ativas. [Configuração documentada pelo apt-file](https://sources.debian.org/src/apt-file/3.4/examples-apt-conf/60disable-contents-fetching.conf).
- O adaptador TCP nativo aceita IP/CIDR literal e portas revisadas: até 16 hosts, 16 portas e 64 conexões, sem DNS ou payload. Cancelamento/timeout preservam observações parciais. O plano é validado novamente antes de consumir a aprovação.
- A análise estática local fixa identidades Windows e hashes no plano, bloqueia caminhos ambíguos/links e não executa código. Regras lexicais versionadas produzem candidatos com arquivo/linha/hash, sem publicar trechos ou valores encontrados. Binários, arquivos grandes, exclusões e omissões constam da cobertura; isso não substitui análise de fluxo nem validação de vulnerabilidade.
- A investigação OpenAI utiliza uma a três perspectivas fixas, sequenciais e independentes: cada uma recebe somente o mesmo contexto revisado. Consentimento, modelo, revisão da credencial e orçamento pertencem ao digest. Uso desconhecido interrompe a sequência e conserva o histórico. Duas perspectivas passaram na API real com conteúdo sintético; não receberam ferramentas ou autoridade para aprovar operações.
- Retenção seletiva remove apenas cópias geridas expressamente selecionadas por ID, após revisão de hashes/identidades e confirmação. Um journal registra cada item; interrupção não repete exclusão. O expurgo do grafo de projeto é um fluxo separado, limitado a projetos arquivados e com etapas duráveis por banco; interrupções exigem revisão explícita. Nenhum dos fluxos promete apagamento seguro ou remoção de cópias externas.
- A sugestão de próxima operação pela IA usa schema fechado e uma lista humana de adaptadores compatíveis. A resposta não cria plano, aprovação ou execução. A primeira chamada real, com fixture sintética e apenas `source.rules.v1` permitido, preservou o domínio sem alterações e registrou 1.602 tokens.
- O instalador local foi solicitado sem assinatura pelo usuário. O NSIS é por usuário e recusa downgrade; o pacote precisa de novo build e de instalação/remoção verificados a cada fechamento. Provas de perfis/SQLite não são provas de rollback de binários ou restauração da distro.

- O reparo revisado da distro pode ser preparado mesmo com sessão MCP de término desconhecido: a degradação do Kali pode impedir o próprio reconciliador MCP. O gate exclusivo continua impedindo concorrência com uma chamada ativa; o reparo encerra somente a distro gerida e mantém os journals MCP desconhecidos para inspeção posterior. Atualização/instalação de pacotes continua bloqueada por sessões não reconciliadas.
