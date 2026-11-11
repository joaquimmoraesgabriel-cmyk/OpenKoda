# Adaptadores Kali limitados

`kali.rg.signals.v1` analisa uma evidência UTF-8 já pertencente à avaliação,
selecionada explicitamente pelo usuário, de no máximo 32 KiB e sem NUL. A ação
`kali.mcp.scanEvidence` recebe somente `assessmentId`, `evidenceId` e
`acknowledgeLocalTransfer: true`. O aceite confirma transferência local desses
bytes ao usuário `openkoda` da distro gerida. Não concede acesso a caminhos,
rede, shell, credenciais do provedor ou execução do conteúdo. O hash original
é conferido no host e novamente no executor. Não há reenvio automático.

O catálogo MCP contém `runtime_diagnostics`, `hash_bytes` e `scan_bytes`.
Os dois últimos recebem apenas `base64` e `expectedSha256`; o host compara
os esquemas completos antes de chamar qualquer ferramenta. A entrada nunca
seleciona executável, argumentos, regex, diretório ou arquivo.

## Regras e resultado

As regras `2026-11-07.1` identificam marcadores de chave privada, chamadas
lexicais `eval`/`exec` e identificadores comuns de atribuição de credencial.
São sinais candidatos que podem ser falsos positivos. Não há análise de fluxo,
confirmação automática de vulnerabilidade, execução de código ou promessa de
cobertura SAST completa. Ausência de sinais não prova segurança.

`/usr/bin/rg` é aberto sem seguir symlink. O descritor deve apontar a arquivo
regular executável pertencente a root, sem escrita por grupo/outros, setuid,
setgid ou capabilities. Seu SHA-256 e metadados são verificados, e o hash é
reconferido antes de cada despacho. Os dois filhos executam o mesmo descritor
fixado: primeiro `--version`, depois padrões constantes com `--no-config`,
`--color never`, `--line-number`, `--only-matching`, `--threads 1` e stdin `-`.
O ambiente do programa contém somente LANG, LC_ALL e PATH fixos.

A resposta usa o envelope existente `{run, result, cleanupConfirmed}`.
`run.sourceEvidenceId` identifica a entrada; `run.evidenceId` identifica o
registro de saída gerado. A evidência de saída também preserva esses vínculos,
a distro, sessão, SHA-256 do código do executor e protocolo MCP.

`result` contém:

- `operation: "scan_bytes"`, `adapter: "kali.rg.signals.v1"`, `rulesVersion`;
- `environmentId`, `sessionId`, `assessmentId`, `inputSha256`, `bytes`;
- `tool: {name: "ripgrep", version, binarySha256}`;
- até 16 `candidates: [{ruleId, line, label, status: "candidate"}]`;
- `counts` por regra, `observedSignals`, `omittedCandidates`, `countsComplete`;
- `partial`, `termination`, `effects` e `limitations` constantes.

Nenhum candidato contém trecho, nome arbitrário de variável ou valor de
credencial. `line` se refere aos bytes da evidência, não a um arquivo do alvo.
A saída interna do parser é limitada a 64 KiB e nunca é persistida ou exibida.
`termination` pode ser `completed`, `canceled`, `timed_out`, `output_limited`
ou `parser_error`. Contagens de uma execução interrompida refletem somente
linhas completas observadas; `countsComplete` fica falso. `partial` também
fica verdadeiro quando candidatos foram omitidos pelo limite de 16 itens.
Um `run.status: completed` indica resposta recebida e limpeza confirmada;
é preciso conferir `result.termination` e `partial` para a cobertura do parser.

## Supervisão e recuperação

Uma sessão permite uma análise e no máximo dois filhos. Antes de `fork`, grava
e sincroniza a intenção; o filho cria grupo/sessão próprios, instala
`PR_SET_PDEATHSIG=SIGKILL`, confere o PID do pai e espera um token por pipe.
O pai só envia esse token após gravar e sincronizar PID, início e prazo do
filho. EOF, prazo expirado ou troca de pai impedem `execve`. O journal e seu
diretório são sincronizados antes de liberar a execução.

Cada filho tem limite de 2 segundos de CPU, 128 MiB de espaço de endereçamento,
32 descritores, nenhum arquivo de saída/core dump, watchdog de 9 segundos e
supervisão de 8 segundos no pai. Descritores herdados são fechados, exceto
stdin/stdout/stderr e o descritor do executável até `execve`; stderr vai para
`/dev/null`. O parser não recebe caminhos do alvo. O supervisor monitora EOF
do transporte, timeout e volume de saída com IO não bloqueante.

Cancelamento/limite exige SIGKILL do grupo/PID e `waitpid`. Antes de confirmar
limpeza, inspeciona também `/proc` para verificar ausência do grupo/sessão e
dos PIDs vinculados. A notificação de encerramento inclui
`childrenQuiescent: true`; o host rejeita ACK sem essa confirmação.
Matar `wsl.exe` não comprova morte dos processos Linux. Perda de comunicação
ou limpeza inconclusiva continua como `unknown` e bloqueia novas chamadas.

A reconciliação não executa o parser novamente. Journals antigos, sem campo
`children`, continuam legíveis. Journals novos preservam intenção e identidade
dos filhos; falta de PID exige expiração da janela e inventário íntegro sem
processo da sessão. Mudança de boot comprova que processos daquele boot
terminaram, sem alegar que a análise interrompida foi concluída.

Esses controles supervisionam um programa do sistema confiável com padrões
fixos. Não formam uma sandbox para código hostil, nem isolam a memória contra
root ou contra outro processo já comprometido do mesmo usuário Linux.

## Verificação

`python scripts/test_kali_executor.py` executa testes de protocolo, schemas,
redação, cotas e reconciliação em qualquer plataforma. Oito testes adicionais
usam `fork`, pipes, sinais, limites e o `/usr/bin/rg` real somente em Linux:
entrada sintética, timeout, EOF, limite de saída, ambiente/descritores,
falha do journal antes de autorização, gate expirado/EOF e morte do pai.
No Windows esses oito ficam explicitamente skipped. O coordenador executa
esses fixtures na distro gerida antes de considerar a supervisão validada.
Os testes Rust usam transportes falsos para consentimento, vínculos, schemas,
contagens, saída sem valores sensíveis e compatibilidade de journals.
