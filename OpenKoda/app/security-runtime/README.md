# Análise de código do OpenKoda

O processo nativo chama este helper para analisar código com o SDK oficial `@openai/codex-security` 0.10.0. O lockfile fixa as dependências, incluindo Codex 0.167.1. O projeto usa a API `CodexSecurity.run` no modo Standard. [SDK oficial](https://github.com/openai/codex-security/blob/main/sdk/typescript/docs/sdk.md)

## Executar localmente

No Windows x64, a distribuição inclui `bin/node.exe`, `python/python.exe` e o executável Codex em `node_modules/@openai/codex-win32-x64`. O motor procura o Git para Windows em `%ProgramFiles%\Git\cmd\git.exe`. O diagnóstico lista `git` quando esse executável falta. Em desenvolvimento, para restaurar as dependências, execute `npm ci --ignore-scripts --no-audit` nesta pasta.

```powershell
.\bin\node.exe cli.mjs --check
.\bin\node.exe --test tests/*.test.mjs
```

A importação do SDK termina antes de armar os temporizadores das consultas de versão. Um controle offline reproduziu perda de stdout pelo timer do Node quando o event loop fica bloqueado, mesmo com saída zero e child.killed=false; a separação elimina a sobreposição conhecida sem ampliar os dez segundos nem repetir a consulta. O controle não atribui sozinho a causa de todos os atrasos observados. O primeiro comando informa as versões disponíveis e métricas seguras das consultas: categoria de falha, duração, bytes de saída e estado do processo. Não devolve caminhos, argumentos ou saída bruta. Um processo encerrado não é aceito como consulta de versão válida, mesmo se a biblioteca resolver a chamada com código zero. Os testes exercitam o protocolo, as projeções dos resultados e o preflight oficial offline com credencial sintética. O teste Windows de preflight cria uma pasta de saída temporária exclusiva sob o perfil e a remove após conferir sua identidade.

## Protocolo nativo

Para uma análise, inicie `bin/node.exe cli.mjs` sem argumentos adicionais. Envie uma linha JSON UTF-8 por stdin, limitada a 64 KiB. O processo chamador mantém stdin aberto até receber o evento terminal. A aplicação obtém a credencial do cofre e a inclui unicamente no campo `apiKey` da mensagem.

| Campo | Conteúdo |
| --- | --- |
| `operation` | `scan` |
| `repository` | Caminho absoluto da pasta autorizada |
| `outputDir` | Destino exclusivo da execução, novo ou vazio, fora do projeto, com um diretório pai existente |
| `target` | Lista opcional de até 64 caminhos relativos com barras, como `["src","tests"]`. Omissão seleciona o projeto inteiro |
| `model` e `provider` | ID do modelo escolhido e `openai` ou `openrouter` no protocolo; a admissão desktop 0.2.1 aceita novas análises somente com OpenAI |
| `apiKey` | Credencial da conexão selecionada |
| `prompt` | Foco adicional da análise, em até 8 KiB. Uma string vazia usa as instruções do SDK |
| `maxCostUsd` | Limite positivo de custo estimado para o SDK |

O SDK verifica permissões dos ancestrais do destino no Windows. Na máquina validada, aceitou uma nova pasta sob o diretório do usuário e recusou a árvore de `AppData/Local/Temp` com erro 5. Os artefatos oficiais ficam em `outputDir/artifacts`, o estado do SDK em `outputDir/state` e o diretório de configuração isolado em `outputDir/config`. O escopo com caminhos relativos orienta o scanner e conserva a interpretação de políticas e metadados da raiz do projeto.

As fases em stdout têm o formato `{type:"progress",stage,message}`. Para cancelar, envie `{"operation":"cancel"}`. A perda do canal de entrada também solicita cancelamento. O helper aguarda o encerramento pedido ao SDK.

Uma conclusão confirmada produz uma linha `{type:"result",status:"completed",scanId,report,findings,coverage,cost}` e código de saída zero. Uma interrupção solicitada retorna `status:"cancelled"` e código diferente de zero. Uma falha produz `{type:"error",code,message}` com mensagem sanitizada. Um processo filho isola a saída do SDK. As mensagens de progresso usam fases conhecidas, operações observadas e falas públicas do assistente. Raciocínio privado e comandos brutos não atravessam essa projeção. Os textos são limitados por caracteres Unicode completos e as credenciais conhecidas são mascaradas. No desktop, a política de dados controla todas as cópias desses textos e do relatório em get, cancel, replay e snapshot, inclusive antes do resultado e após mudanças na atividade.

O campo `report` contém até 256 KiB do Markdown. Cada uma das primeiras 100 entradas de `findings` contém `id`, `title`, `severity`, `description` e `location`. O campo `coverage` registra escopo, exclusões, número adicional de exclusões, contagem original de achados e eventual corte do relatório. O arquivo `artifacts/report.md` conserva o resultado completo. Cada linha de saída tem limite de 2 MiB.

O campo `cost` contém `estimatedUsd` quando o SDK dispõe de dados. A investigação recebe uma referência de tempo de 15 minutos e o limite de custo escolhido. O SDK compara estimativas e requisições em andamento podem elevar o total acima do limite. Para modelos sem preços reconhecidos na versão instalada, o SDK recusa a execução com limite e o helper devolve `pricing_unavailable`. O preflight oficial confirmou a recusa dos IDs canônicos OpenRouter com limite nesta versão. O desktop bloqueia essa combinação antes de reserva ou despacho; OpenRouter permanece disponível nas conversas. Não se remove o prefixo nem se aplica uma tabela de outro provedor para contornar a falta de preço. [Semântica de custo do SDK](https://github.com/openai/codex-security/blob/main/sdk/typescript/docs/sdk.md#progress-and-cost)

## Permissões e limites

A fábrica prefere MXC quando o Windows o suporta e mantém o fallback elevado do SDK; não seleciona o backend unelevated. A prova local offline leu um arquivo permitido e recusou o canário protegido. A política de aprovação é never: pedidos de permissões adicionais são recusados. Isso não reduz as leituras que o perfil do SDK já concede. [Configuração oficial de isolamento Windows](https://learn.chatgpt.com/docs/windows/windows-sandbox).

O SDK 0.10.0 documenta leitura ampla do filesystem acessível à conta e escrita nas raízes de trabalho. Selecionar um alvo ou caminhos relativos orienta o scanner e não cria isolamento de leitura apenas da pasta. A interface informa esse limite; só a configuração isolada do SDK recebe a negação própria do motor. Não há prova de negação de todo o restante do perfil OpenKoda, e um cofre DPAPI não equivale a isolamento de leitura por repositório. Um executor descartável/hospedado expondo somente um snapshot seria necessário para essa garantia; não foi implementado nesta versão local.

Artefatos e estado do executor podem conter código e continuam fora do backup SQLite e da exclusão lógica do projeto. O relatório incorporado é uma evidência preservada no banco. Uma conclusão do SDK, um preflight aprovado ou zero achados não comprovam revisão de todos os arquivos nem ausência de vulnerabilidades. Consultar os recibos e limites de app/docs/RETOMADA-0.2.1.md na distribuição de fontes.

## Avisos dos componentes distribuídos

O Node.js inclui licenças e atribuições em `bin/LICENSE.txt`. Os avisos do Python ficam em `python/LICENSE.txt`. O SDK `@openai/codex-security` inclui a licença Apache 2.0 em `node_modules/@openai/codex-security/LICENSE`. O `package-lock.json` identifica as versões e integridades npm. Os arquivos de licença e atribuição fornecidos pelos pacotes acompanham suas respectivas pastas em `node_modules`.

A distribuição Windows do Codex inclui bibliotecas nativas para áudio. Consulte `node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex-resources/voice/NOTICE.md` para os avisos de GStreamer, GLib, libintl, libffi, PCRE2, Opus, zlib e Microsoft Visual C++. A pasta `licenses` contém os textos que acompanham o pacote. `sources.json` identifica as versões, os endereços e os hashes das fontes correspondentes.
