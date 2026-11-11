# IA, MCP e fluxo de dados do OpenKoda

Estado: contrato proposto de produto/arquitetura, 7 de novembro de 2026. Não é uma API implementada. Os exemplos de nomes e envelopes são especificação para revisão; não constituem comandos de exploração nem prometem compatibilidade com uma biblioteca ainda não escolhida.

O framework desktop **Tauri** e os seis conceitos visuais foram confirmados pelo usuário. A escolha de Tauri 2 e React/TypeScript permanece proposta. O [refinamento da interface](AJUSTES-DE-INTERFACE.md) e as [telas adicionais aprovadas](conceitos/TELAS-ADICIONAIS.md) organizam a apresentação; seus valores e estados ilustrativos não são respostas reais de provider, vault, Kali ou Executor.

O usuário confirmou que a IA usará Kali/WSL 2 por MCP. API de inferência e MCP podem ser usados juntos: o host envia contexto ao modelo, interpreta a chamada de ferramenta e a despacha ao Executor. O protocolo das ferramentas não concede autoridade por si. As regras de escopo e aprovação estão em [PERMISSOES-E-SEGURANCA.md](PERMISSOES-E-SEGURANCA.md).

## Componentes lógicos

| Componente | Responsabilidade | O que não recebe por padrão |
|---|---|---|
| Interface/workbench | Projetos, alvos, conversa, plano, progresso, achados, evidências e configurações | Chave de API em texto recuperável pelo renderer; execução arbitrária no host |
| Orquestrador de Avaliação | Estado, agenda, dependências, pausas, budgets, checkpoints e retomada | Permissão de aumentar o próprio escopo |
| Serviço de contexto/inferência | Selecionar material permitido, montar requests, receber chamadas e medir uso | Credenciais dos alvos em massa; leitura irrestrita do Windows |
| Cliente/servidor MCP OpenKoda | Publicar schemas e transportar propostas/resultados | Autoridade implícita derivada de nome/annotation da ferramenta |
| Motor de política/broker host | Identidade, autorização, vault, grants e ciclo do Ambiente | Linguagem de shell como API genérica de administração Windows |
| Executor Linux | Validar grant e iniciar/supervisionar processo no Ambiente | Todos os segredos da instalação ou token administrativo Windows |
| Adaptadores | Traduzir operação tipada para ferramenta/argv/artefatos | Poder de autorizar novos alvos ou conceder privilégios |
| Parsers/normalizadores | Converter bruto em dados estruturados e resumo com vínculos | Poder de promover conteúdo do alvo a instrução ou aprovação |
| Repositório de Evidência | Bytes, metadados, integridade, versões, acesso e retenção | Garantia de imutabilidade contra administrador local sem mecanismo adicional |

São módulos e fronteiras de autoridade. Ainda não determinam quantos processos/linguagens o aplicativo terá. Separar parser, renderer e processo privilegiado deve ser preservado na escolha da implementação.

### Relação entre interface aprovada e fluxo de dados

| Elemento visual | Contrato proposto de dados/ação |
|---|---|
| Indicador compacto do Kali/provedor | Projeção de diagnóstico datado e capacidades efetivas; indisponível/desatualizado quando não há confirmação |
| Configurações → IA e modelos | Exibe referência da conexão, endpoint/modelo selecionados e estado de teste; não devolve chave guardada pelo backend |
| Testar conexão | Pedido de diagnóstico ao serviço de inferência com dados mínimos, sem enviar fontes/evidências da Avaliação como teste implícito |
| Dados enviados à IA → Revisar política | Abre classes, fontes permitidas e revisão efetiva do Projeto; alteração não reescreve envios anteriores |
| Permissões | Apresenta perfil/política desejados e efetivos; salvar não permite ao frontend emitir grants |
| Conversa com a IA | Lateral sob demanda que usa o mesmo Projeto/Avaliação e ciclo de política, sem autoridade própria |
| Execuções/terminal | Painel sob demanda para saída e interação aplicável; abrir o painel não inicia um shell genérico |
| Evidências e achados | Navega por referências autorizadas e histórico; texto de alvo é dado, não evento de autorização |

O Tauri IPC será a ponte de ações tipadas da WebView para o núcleo, não um substituto do MCP do Executor. A lista de commands/permissões da UI deve ser mínima e explícita; a aplicação continua verificando a autoridade de domínio. [Modelo oficial de permissions Tauri 2](https://v2.tauri.app/security/permissions/).

## O que a IA consegue fazer

A IA pode compreender o objetivo, pedir contexto que falte, inventariar o Projeto permitido, propor um Plano de ação, escolher adaptadores, iniciar Operações cobertas por política/aprovação, acompanhar Execuções longas, ler resultados paginados, correlacionar evidências, produzir candidatos, investigar a validação, propor remediação e preparar relatório/reteste. Pode operar ferramentas Kali sem adaptador por uma rota avançada controlada e verificar se faltam pacotes, versões ou capacidades.

Não pode decidir que possui autorização sobre um alvo; alterar a própria política; criar aprovação humana; ler segredo fora da Operação; instalar pacote ou mudar a distro como efeito oculto de uma análise; confirmar um achado só porque a ferramenta imprimiu um alerta; enviar relatório externamente apenas porque terminou; nem mascarar falha/truncamento como cobertura completa.

O conjunto efetivo de ferramentas depende do Projeto, Perfil de execução, Ambiente e versão do catálogo. Um modelo com tool calling pode usar a interface MCP mediada pelo host. Um provider sem recursos suficientes pode ficar restrito a conversa/plano/leitura já preparada; não haverá emulação silenciosa que execute texto arbitrário como chamada.

## Ciclo de inferência e execução

1. O usuário escolhe Projeto/Avaliação e informa objetivo. O Orquestrador carrega escopo, estado e política de dados dessa revisão.
2. O serviço de contexto seleciona instruções do produto, objetivo humano, resumo do Projeto, capacidades disponíveis e evidências pertinentes. A política de compartilhamento filtra o material antes do envio.
3. O provider recebe esse pacote e devolve conteúdo ou uma proposta de chamada com argumentos. O endpoint, modelo, IDs de request e uso/custo ficam registrados quando disponíveis.
4. O host valida schema, identidade e limites; associa contexto confiável do trabalho. IDs fornecidos pela IA não substituem a vinculação do host.
5. A Operação passa pela política/approval/grant, conforme [PERMISSOES-E-SEGURANCA.md](PERMISSOES-E-SEGURANCA.md). O Executor faz a última verificação antes de iniciar.
6. O processo produz bytes/artefatos e eventos de ciclo de vida. O bruto é preservado conforme quota e política; parser e normalizador produzem projeções vinculadas.
7. O modelo recebe um resultado limitado e verificável, com estado, campos essenciais e referências para leitura posterior. Se pedir mais, a política de acesso/envio é aplicada novamente.
8. Candidatos e Achados recebem estados distintos. A avaliação só é fechada com cobertura, falhas, pendências e referências, inclusive quando parcial.

O loop tem budgets independentes de tokens/custo, tempo de trabalho, número de chamadas, volume de dados e intensidade no alvo. Esgotar um budget pausa/reduz a tarefa segundo regra explícita; não seleciona outro provider ou amplia rede sem decisão.

## Catálogo MCP proposto

Os nomes abaixo são candidatos para a implementação, sem compromisso de versão. Ferramentas de autoridade humana/manutenção são separadas do toolset que o modelo pode invocar.

| Grupo | Exemplos de ferramentas propostas | Resultado principal |
|---|---|---|
| Contexto e Ambiente | `project.describe`, `environment.inspect`, `tool.catalog.list` | Estado permitido, versões/capacidades e limitações |
| Plano | `plan.propose`, `plan.inspect` | Plano versionado e decisões ainda necessárias |
| Preparação | `operation.prepare` | Operação normalizada e requisito de autoridade |
| Execução | `operation.start`, `execution.status`, `execution.cancel` | ID da Execução e transições de estado |
| Entrada interativa | `execution.write_input` | Resultado de stdin aprovado; nova entrada pode mudar efeitos e exigir decisão |
| Evidência | `evidence.list`, `evidence.read_page`, `execution.output.read` | Páginas limitadas, proveniência e estado de truncamento |
| Candidatos/Achados | `candidate.record`, `finding.propose_validation`, `finding.read` | Rascunho com evidência e validação, sem autoelevação de certeza |
| Relatório | `report.prepare`, `report.inspect` | Documento local e cobertura; envio externo é outra Operação |
| Avançado | `shell.prepare` e a mesma rota de início autorizada | Programa/argv/digest/perfil/limites explícitos |

Não há uma ferramenta MCP `approval.accept` utilizável pela IA. O modelo pode preparar um pedido; o evento de decisão entra por um canal humano autenticado. `environment.package_plan` pode propor manutenção, mas executar a instalação pertence ao fluxo de manutenção autorizado. Ferramentas de host não aparecem simplesmente porque um servidor MCP anuncia `readOnlyHint`.

## Envelope da operação, separado do protocolo

O MCP usa JSON-RPC; o contrato OpenKoda fica dentro dos argumentos/resultados. Uma chamada ilustrativa de preparação pode conter apenas referências:

```json
{
  "jsonrpc": "2.0",
  "id": "request-demo",
  "method": "tools/call",
  "params": {
    "name": "operation.prepare",
    "arguments": {
      "planId": "plan-demo",
      "planVersion": 3,
      "operationRef": "op-demo"
    }
  }
}
```

O host associa internamente Instalação, Projeto, Avaliação, revisão do escopo, ator, sessão e catálogo efetivos. O modelo não envia um bearer/grant administrativo. O plano só descreve a intenção até o Executor compilar/verificar a representação concreta.

O registro interno de uma Operação deve conter: IDs/versões; finalidade; adaptador/ferramenta/binário; comando estruturado ou digest do programa de shell; argv; cwd; input refs; read/write roots; destinos e política DNS/redirect; refs de credencial; efeito/idempotência; limites; capacidades; Ambiente; requisito de contenção e digest canônico. O grant é um objeto de controle separado, vinculado a esse digest e à política vigente, com expiração/revisão/revogação.

Segredos não entram no digest como texto; entra a identidade/versão do segredo e o escopo concedido. Se a implementação usar material cifrado ou um processo auxiliar, isso não dispensa controlar onde o valor se torna acessível à ferramenta.

## Transporte e identidade

Proposta inicial: cliente MCP controlado pelo broker, servidor lançado como processo próprio e transporte stdio sobre um canal de processo gerido. O bridge Windows↔Linux tem bootstrap, handshake e identidade do Ambiente explícitos. A escolha concreta entre pipes/streams de processo e serviço local precisa validar peer, lifecycle e proteção contra conexão cruzada; não será resolvida por um token gravado no workspace.

A especificação MCP 2025-11-25 fornece transporte stdio e Streamable HTTP. Stdio troca mensagens UTF-8 delimitadas por linhas, reservando stdout para protocolo e stderr para diagnóstico. HTTP exige cuidados próprios; a versão e as capabilities efetivamente negociadas precisam ser verificadas no handshake. [Transportes MCP](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports).

O produto deve negociar versão compatível, anunciar toolset/schema versions, inventariar tool names e validar mudanças de catálogo. Uma sessão é ligada ao Projeto/Avaliação/ambiente e ao emissor do host, com capabilities mínimas. Replays, sessão expirada e IDs cruzados são recusados. As annotations são pistas de apresentação, nunca o mecanismo de autorização. [Ferramentas MCP](https://modelcontextprotocol.io/specification/2025-11-25/server/tools).

Não haverá listener em `0.0.0.0` por padrão nem endpoint WSL aberto à LAN. Se Streamable HTTP entrar futuramente: bind escolhido explícito; autenticação de toda conexão; validação de Origin e Host; proteção DNS rebinding; sessão curta; limite de corpo; nenhuma credencial em URL; e TLS nos trechos que saírem da máquina. Loopback é endereço, não autenticação. A integração remota não é parte confirmada desta primeira proposta.

MCP cancellation é um pedido de cancelamento, não prova de término de processo. A UI deve receber confirmação do Executor e estado de cleanup. A reconexão recupera estado por IDs/checkpoints e cursor, sem repetir automaticamente a Operação.

## Contratos de resultado e eventos

Um resultado normalizado deve incluir pelo menos:

| Área | Campos semânticos |
|---|---|
| Correlação | Projeto, Avaliação, Operação, Execução, tentativa e tool call |
| Estado | Estado da Operação separado da Execução: preparação/autorização/fila; início/processo/término; resultado final ou desconhecido |
| Processo | Início/fim, exit code/signal quando disponíveis, backend, ferramenta/versão e confirmação de cleanup |
| Observações | Dados estruturados por schema, avisos, candidatos e evidências que sustentam cada observação |
| Saída | Raw refs, bytes/captura, encoding, stdout/stderr, limites, intervalo preservado e omitted ranges quando conhecidos |
| Transformação | Parser/normalizador/versões, digest dos inputs e dos resultados, falhas de parse e confiança da interpretação |
| Cobertura | Alvos efetivamente visitados, arquivos/rotas observados, falhas, exclusões e pendências |
| Consumo | Duração, tráfego/volume, tokens/custo quando atribuíveis e estado dos budgets |

Há dois planos de eventos: eventos duráveis do trabalho e fragmentos transitórios de saída. Transições críticas e decisões são persistidas antes de serem exibidas como concluídas. Os eventos têm sequência por Execução e timestamp; lacunas são detectáveis. O histórico não depende de o renderer ficar aberto.

Pausar a Avaliação impede novo despacho; não promete suspender com segurança um processo de rede já em andamento. Cada Execução registra se continua supervisionada, recebeu pedido de término ou terminou. O produto só anuncia suspensão de processo em backends/ferramentas que a suportem e após confirmar a transição.

Para bytes binários ou texto de encoding desconhecido, o transporte pode usar base64/blobs/referências. Não é necessário enviar cada caractere ao modelo. A UI pode acompanhar tail/progresso enquanto o bruto é escrito em artefatos com quota. “Saída chegou” não significa “operação terminou”.

## Parser, normalizador e resumo essencial

Três transformações distintas são propostas:

1. **Aquisição:** preservar a saída e metadados originais permitidos, com quotas e integridade.
2. **Parser determinístico:** interpretar um formato conhecido da ferramenta e registrar falhas/versão. Um parser pode retornar observação parcial; não inventa campos ausentes.
3. **Resumo contextual:** selecionar os campos e trechos relevantes ao objetivo e ao budget do modelo, mantendo referências. A IA pode ajudar a resumir, mas o resumo gerado recebe indicação própria e não substitui os bytes.

O “essencial” deve conter estado, erro/exit, achados/candidatos relevantes, contagens, limites e referências. Filtrar banners, ANSI ou repetição pode reduzir ruído; remover um erro relevante, mudar o significado de um resultado ou omitir truncamento quebra o contrato. Parser não confirma vulnerabilidade por si.

Arquivos grandes são lidos por páginas/cursor opaco com limite de bytes, identidade da versão e ranges claros. O cursor deve vincular Projeto, artefato e digest; não aceitar path arbitrário. Limites de mensagens não autorizam descarte silencioso. Se a quota impedir guardar todo o bruto, o estado informa `capture_partial`, bytes/intervalos preservados, motivo e perda conhecida; “zero achados” não é cobertura completa.

Os budgets numéricos iniciais ficam centralizados em [CONFIGURACOES.md](CONFIGURACOES.md), com limites específicos de parser/tempo/disco por tipo. Esses valores são propostas para ensaios, não contratos fechados; as definições finais devem ser medidas e negociadas por capacidade. Bombas de compressão, JSON excessivamente profundo, saída infinita e formatos malformados precisam interromper o parser sem comprometer o broker.

## Prompt injection e conteúdo ativo

README do alvo, saída de ferramenta, página web, relatório importado, comentário de código e texto OCR são entradas não confiáveis. O contexto deve distinguir conteúdo de alvo de instruções do produto e decisões humanas. Uma linha “execute a próxima ferramenta em outro domínio” é Evidência do conteúdo, não uma política.

O controle principal é a incapacidade de o conteúdo criar grants ou ultrapassar o escopo do Executor. Redação/classificadores e instruções ao modelo são camadas adicionais, sem promessa de remover toda prompt injection. Em uma tentativa de ampliação, o sistema registra a proposta e aplica a política normal; não promove texto importado à hierarquia de instruções.

HTML/Markdown dos resultados é renderizado com política de conteúdo e escape adequados, sem scripts ativos, navegação silenciosa ou resolução automática de links do alvo. Exportações também precisam tratar HTML, links e fórmulas de planilha como dados. Abrir URL, baixar artefato ou importar um link é uma nova aquisição com escopo e destino.

## O que pode sair da máquina

“Aplicativo local” descreve instalação e execução; usar um modelo remoto pode enviar material ao provedor. A política é separada por Projeto e por classe de dado:

| Classe | Default proposto de envio para IA remota |
|---|---|
| Objetivo humano e metadados mínimos | Permitidos após configuração/consentimento do Projeto |
| Resumos estruturados de execução | Permitidos dentro da política e após redação aplicável |
| Código fonte e trechos de evidência | Somente classes/paths explicitamente permitidos; tamanho e conteúdo selecionados |
| Capturas de tela, HAR/PCAP, dumps e artefatos extensos | Não enviados integralmente por padrão; extratos selecionados e rastreados |
| Credenciais, chaves API, cookies e tokens de alvo | Negados por padrão; referência/metadata em vez do valor |
| Informações de outro Projeto | Negadas pela vinculação do contexto |

Credenciais necessárias à conexão com o próprio provedor são usadas pelo serviço de inferência como autenticação e não inseridas no prompt. Algumas avaliações autenticadas exigirão dados sensíveis na ferramenta: o produto deve separar entrega ao processo do envio à IA.

O painel precisa mostrar provider/endpoint/modelo efetivos, classes permitidas, última revisão de política e uma prévia consultável do pacote de contexto. Cada envio registra classes/ref/digests/quantidades e IDs do request quando disponíveis; a inspeção local do pacote completo segue a política de retenção para não criar uma segunda cópia desnecessária de segredos.

A mensagem visual “Conexão verificada” deverá corresponder ao endpoint, referência/versão da credencial, modelo e capacidades efetivamente testados, com data e revisão. Alterar esses campos invalida a indicação anterior. HTTP bem-sucedido, autenticação, disponibilidade do modelo, tool calling e streaming são resultados distintos. O teste não comprova o isolamento do Kali nem autoriza enviar conteúdo do Projeto. As regras visíveis de edição/salvamento ficam centralizadas em [CONFIGURACOES.md](CONFIGURACOES.md).

Não há fallback silencioso para outro endpoint/modelo ou upload de arquivos como resposta a limite de contexto. Provedores locais são opção de integração quando suportados, não uma decisão tecnológica já confirmada nem garantia automática de isolamento. Políticas de retenção/uso do provedor escolhido devem ser exibidas por configuração, sem prometer controle do OpenKoda sobre dados já enviados.

## Credenciais e configurações

Configurações separadas: interface/preferências; conexão de inferência; política de dados; escopo/política operacional; toolset/ambiente; manutenção; e retenção/exportação. Precedência proposta: requisitos obrigatórios da instalação → capacidade real do Ambiente → política do Projeto → autorização da Avaliação → grant da Operação. Camada inferior pode restringir, não ampliar a superior.

Segredos usam refs de vault com acesso específico, rotação e revogação. O Executor recebe apenas o mínimo para a Operação. Environment inheritance por default é uma allowlist de variáveis funcionais, sem copiar todo o ambiente do broker. Shell history, dump/core, logs de debug e crash report não recebem segredos deliberadamente. Uma falha de limpeza marca a credencial/artefato como potencialmente exposto, sem afirmar apagamento garantido.

Trust de certificados e proxy são configurações explícitas. Uma avaliação de TLS não desativa globalmente a validação HTTPS do provedor, do catálogo de pacotes ou do update. Opções necessárias para observar um alvo devem ser delimitadas à Operação e registradas como condição de validação.

## Persistência, retenção e exclusão

O estado durável compreende identidade do Projeto/Avaliação, planos/revisões, escopos, políticas, decisões, execuções, cobertura, candidatos/achados e índices de Evidência. Os bytes grandes ficam em armazenamento de artefatos com ACLs e identidade, vinculados ao banco; escolher SQLite ou outro mecanismo é decisão de implementação futura.

Proposta inicial: workspace mantém projetos/relatórios até exclusão humana; temporários de processo têm limpeza ao fim confirmado; capturas sensíveis e bruto têm prazo/quota configurável por Projeto e semântica visível. Expurgo possui preview de alcance e registra quais referências deixam de estar disponíveis. Relatório retido pode preservar hash/metadata de evidência eliminada, mas deve mostrar que ela não pode mais ser inspecionada.

Excluir do armazenamento vivo não remove cópias exportadas, backups, cache do SO, memória de processo ou dados do provedor. “Apagamento seguro” só pode ser anunciado com mecanismo e ameaça explicitados. A política deve diferenciar exclusão lógica, coleta de artefatos não referenciados, retenção por investigação e remoção de backups geridos.

Exportações têm manifest de conteúdo, proveniência e redação; importações validam formato, tamanho, paths e integridade antes de indexar. Hashes ajudam a detectar alteração de bytes, mas não provam a veracidade do conteúdo nem imutabilidade contra o administrador local.

## Compatibilidade e critérios antes de implementar

O futuro contrato precisa resolver/validar: dialect/schema MCP; tool calling e streaming por provider; armazenamento de estado e artefatos; conexão autenticada host↔WSL; tokens/grants internos; adapter/plugin signing e versionamento; negociação de limites; proveniência dos resumos; e isolamento de parsers. Nada disso exige copiar o protocolo privado do Codex.

As fontes de referência são a investigação local [PROTOCOLS.md](../docs/PROTOCOLS.md), os [schemas instalados do Codex](../docs/INSTALLED-SCHEMAS.md) e a especificação MCP fixada acima. O que foi observado no Codex é inspiração e evidência de fronteiras úteis; os métodos propostos OpenKoda continuam um contrato novo a testar. Este documento não registra scan, instalação ou execução do futuro produto.
