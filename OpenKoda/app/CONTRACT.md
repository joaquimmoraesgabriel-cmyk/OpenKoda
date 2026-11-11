# Contrato inicial — implementação

Tauri 2, React/TypeScript, núcleo Rust. API nativa única `request`, argumento `request: { action: string, payload?: object }`. Ações de domínio fechadas; nenhuma execução shell genérica. Erro rejeita Promise com mensagem legível. Respostas JSON camelCase. Um mutex transacional guarda o store no app_data_dir; persistência deve sobreviver reinício e rejeitar corrupção sem apagar dados.

## Modelo compartilhado

Snapshot: `{projects: Project[], assessments: Assessment[], findings: Finding[], settings: Settings}`.

Project: `{id,name,kind,target,description,createdAt}`. kind: code | web | network | files. Alvo de code/files é diretório/arquivo selecionado explicitamente; web é URL; network é endereço/rede.

Assessment: `{id,projectId,title,kind,target,objective,status,createdAt,updatedAt,events: Event[],evidence: Evidence[],coverage: string[],summary:string}`. status: draft | preparing | planning | awaiting_approval | queued | running | pausing | paused | validating | completed | partial | canceling | canceled | blocked | failed | unknown. Event: `{id,at,message,kind}`. Evidence: `{id,name,content,sha256,createdAt}`.

Finding: `{id,assessmentId,title,severity,confidence,status,description,recommendation,evidenceIds:string[],createdAt}`. status: candidate | validated | dismissed | fixed. Severidade: low | medium | high | critical.

Settings: `{provider,endpoint,model,mode,theme,language,retentionDays,timeoutSeconds,outputLimitKiB,hasApiKey:boolean}`. Chave nunca no Snapshot.

## Etapa 1 — ações obrigatórias

- `snapshot` -> Snapshot.
- `project.create` payload `{name,kind,target,description}` -> Project; `project.archive` `{id}` -> `{ok:true}` (não perder histórico).
- No aplicativo Windows, `project.pickTarget` recebe `{"selection":"folder"}` ou `{"selection":"file"}`. O diálogo pertence à janela principal e devolve o caminho escolhido, ou `null` ao cancelar. Registrar o caminho no projeto e autorizar a avaliação são ações posteriores.
- `assessment.create` `{projectId,title,kind,target,objective}` -> Assessment draft. Escopo validado igual ao alvo do projeto nessa etapa.
- `assessment.start` `{id}` -> bloqueado com motivo claro até executor pronto; nunca fingir execução.
- `assessment.cancel` `{id}`, `assessment.pause` `{id}` -> Assessment; transições verificadas.
- `settings.save` payload campos Settings permitidos -> Settings; bloquear chave via esta rota.
- `finding.update` `{id,status}` -> Finding; validar evidência antes de validated/fixed, não permitir promover vazio.
- `report.export` `{id}` -> `{path}`. Markdown em app_data_dir/exports, nome derivado de ID; inclui cobertura/incompletude/evidências.
- `environment.status` -> `{status:'unavailable'|'ready'|'degraded',distro:'OpenKoda-Kali',message:string,tools:[]}`. Etapa1 indisponível sem sondagem feita.

## Etapas seguintes reservadas

`environment.*`, `packages.*`, `assessment.run`, `evidence.*`, `ai.*`, `patch.*` serão contratos tipados adicionais. Não liberar comandos ausentes como operações bem-sucedidas.

Em preview web, mostrar aviso explícito e persistência local de demonstração separada; nunca afirmar Kali/IA conectado. UI trata erros, estados vazios e carregamento, não tem dados inventados por padrão. Dados de exemplo só por ação explícita, identificados.
## Provedores e escolha do modelo

O host aceita `openai`, `openrouter` e `groq`. Resolve o destino HTTPS a partir do provedor e envia a chave somente ao domínio correspondente. O catálogo usa `/models` na OpenAI e na Groq e `/models/user` no OpenRouter, com as preferências da chave. Inferências usam Responses na OpenAI e Chat Completions nos outros dois serviços. O catálogo inclui modalidades que podem ser incompatíveis com geração de texto. Modelos com busca própria podem consultar fontes no provedor. A escolha de modelo não habilita ferramentas locais.

- `ai.key.save` aceita `{key, provider?}`. `ai.status`, `ai.models` e `ai.key.remove` aceitam `{provider?}`. O padrão é o provedor da instalação. Cada chave possui um slot DPAPI e revisão próprios. Listar modelos consulta o catálogo do provedor e devolve `models` com os identificadores completos.
- `settings.save` define o padrão com `provider`, `endpoint` e `model`. `assessment.create`, `assessment.combinedPreview` e `assessment.combinedCreate` aceitam `aiSelection` opcional com `{provider, model}`. A seleção participa da identidade da criação. Avaliações anteriores usam o padrão da instalação.
- `ai.selection.save` aceita `{assessmentId, aiSelection}`. `null` restaura o padrão. A conversa, a proposta de operação e a investigação resolvem a configuração da avaliação ao revisar e despachar cada envio. Trocar o destino, o modelo ou a revisão da credencial exige nova revisão.
- A tarifa manual identifica o par provedor/modelo. Nas revisões gravadas antes da 0.1.14 e nos campos de preço com `provider` omitido, o provedor é OpenAI. O histórico de uso e as reservas registram o endpoint usado.
- Ao configurar o segundo provedor, o host amplia o vault de `assistant.sqlite3` em uma transação. A chave e revisão OpenAI existentes mantêm o slot original. O backup revisado conserva o histórico e exclui as chaves e as confirmações de conexão de todos os slots.
