# Configurações do produto

Todos os defaults são propostas de planejamento, sujeitas aos ensaios de [FASES-E-VALIDACAO](FASES-E-VALIDACAO.md). Não representam valores aplicados neste computador.

O usuário confirmou **Tauri** como framework desktop e aprovou o [painel](conceitos/painel-openkoda-v2.png) e as [cinco telas adicionais](conceitos/TELAS-ADICIONAIS.md). Tauri 2 e React/TypeScript seguem como propostas de implementação. A aprovação das imagens confirma organização/aparência, sem comprovar conexão de IA, proteção de chave, prontidão do Kali ou aplicação de permissões.

## Mapa visual de configurações e autoridade

O acesso fica na área secundária de Configurações, com **Geral, IA e modelos, Permissões, Dados e privacidade, Atualizações**. Preservar esse menu e revelar detalhes conforme a seleção, sem criar destinos primários para cada controle técnico. Conversa e terminal permanecem sob demanda no contexto da Avaliação, sem ocupar a página de configurações por padrão.

| Área/controle aprovado | O que deverá representar no produto | Estado/erro necessário |
|---|---|---|
| Provedor, endpoint e modelo | Configuração selecionada e capacidades compatíveis, com revisão | Não configurado, formato inválido, incompatível, alterado sem salvar ou teste desatualizado |
| Chave de API mascarada | Referência/estado de cadastro no vault host, sem recuperar o segredo na WebView | Ausente, revogada, expirada quando conhecido ou falha do mecanismo de proteção |
| Testar conexão | Diagnóstico explícito e mínimo pelo serviço de inferência | Testando, cancelado, timeout, TLS/auth/modelo/capacidade indisponível e sucesso datado por revisão |
| Modo padrão e limite por avaliação | Preferências/budgets aplicáveis a novos trabalhos, dentro das políticas superiores | Valor não suportado, conflito de origem ou efeito só no próximo trabalho |
| Revisar política de dados | Classes e fontes permitidas ao provider para o Projeto, com valor efetivo | Alterações pendentes, envio bloqueado ou política exigida antes do uso |
| Permissões | Perfil, origem, escopo e limites desejados/efetivos, com ação de revisão | Capacidade não comprovada, decisão pendente, recusa, expiração ou revogação |
| Salvar alterações | Persistir uma revisão validada no backend e exibir seu efeito | Conflito de revisão, falha de persistência ou confirmação de salvo |

O conceito [05-configuracoes-v1.png](conceitos/05-configuracoes-v1.png) contém valores fictícios. “Conexão verificada”, “Armazenada com proteção local”, “Kali pronto” e “Ambiente saudável” não ficam verdes só por selecionar uma opção ou desenhar um controle. Na implementação, cada estado terá evidência e alcance próprios; conexão do provider não significa autorização de execução ou compatibilidade de todas as ferramentas.

O campo mascarado é apresentação de referência/estado, não uma leitura do valor guardado. Cadastro e troca de segredo requerem fluxo dedicado a definir e testar; a imagem não especifica um mecanismo de proteção e a máscara não é criptografia. O backend não oferece ao frontend uma ação genérica de ler o vault. O teste usa a credencial somente pelo serviço correspondente, sem registrar seu valor nem enviar arquivos de Projeto por padrão.

Testar alterações ainda não salvas usa uma revisão candidata explicitamente identificada; o resultado não marca como verificada a configuração persistida diferente. Salvar não inicia Avaliação ou shell. Mudanças de endpoint, credencial ou modelo invalidam o teste anterior e valem para próximas avaliações; trabalho existente conserva seu snapshot, salvo revogação/redução de autoridade com os efeitos definidos no mapa.

A integração Tauri da interface deverá usar commands/permissões explícitos e validar o payload no núcleo. Capabilities da WebView não são aprovação da Operação no Kali; o Executor continua aplicando política, revisão de início e grant técnico. O contrato e os cuidados de app commands estão em [PERMISSOES-E-SEGURANCA.md](PERMISSOES-E-SEGURANCA.md#tauri-e-a-superfície-de-autoridade-da-interface), com [referência oficial de capabilities](https://v2.tauri.app/security/capabilities/).

## 1. Precedência e registro

Configuração efetiva = capacidades demonstradas do ambiente + política da instalação + restrições do projeto + revisão de escopo + ajustes da avaliação + decisões de autorização válidas. Uma camada mais baixa não amplia autoridade negada por uma superior. O executor valida o resultado concreto, não apenas um formulário salvo.

Preferências visuais mudam imediatamente. Alterações de modelo, parser, ferramenta, ambiente ou capacidade criam revisão/snapshot para o próximo trabalho. Uma avaliação ativa conserva versões/regras relevantes; revogação e redução de autoridade podem ser aplicadas imediatamente, com reconciliação dos efeitos.

O usuário vê valor global, override de projeto, valor efetivo, motivo de bloqueio e reinício/revalidação necessários. Credenciais têm campo próprio de vault e nunca aparecem como texto em “configuração efetiva”.

## 2. Geral e interface

| Grupo | Opções propostas | Default proposto |
|---|---|---|
| Idioma e tempo | Idioma da UI/relatório, fuso, formato de data | Português e fuso do Windows; armazenamento UTC |
| Aparência | Claro/escuro/sistema, densidade e fonte monoespaçada | Sistema, densidade confortável |
| Acessibilidade | Contraste, foco, escala, reduzir movimento, atalhos | Teclado completo e sem depender de cor |
| Navegação | Projeto recente, filtros salvos, painéis e colunas | Retomar projeto sem iniciar execução |
| Fechamento | Encerrar supervisionado ou continuar com serviço validado | Perguntar ao fechar com trabalho ativo, sem escolher por timeout |
| Notificações | Local/som, falha, aprovação, custo e conclusão | Avisos locais para ação necessária/conclusão; sem envio externo automático |
| Diagnóstico | Nível, retenção, pacote de suporte | Redigido e local; exportação revisada |

## 3. IA e orçamento

| Opção | Regra proposta |
|---|---|
| Provedor/endpoint | Adaptador configurável; endpoint explícito, teste de conectividade e capacidades |
| Modelo | Escolha da lista do provedor ou identificador validado; sem modelo universal hardcoded |
| Chave | Referência no vault host, uso mínimo pelo adaptador de IA |
| Contexto | Limite por requisição/avaliação, fontes permitidas, redaction e orçamento de leitura |
| Custo | Teto por avaliação e instalação, moeda de exibição e estimativa identificada |
| Tempo de raciocínio/esforço | Somente quando suportado, sem inventar parâmetro no provedor |
| Agentes | Limite de workers/investigações independentes por avaliação |
| Autonomia | Assistido ou política prévia delimitada; sem ampliação por autoavaliação da IA |
| Falha de provedor | Parar novos passos de IA e preservar trabalho/checkpoints |
| Troca de provedor/modelo | Explícita; reavaliar política de dados, custos e compatibilidade, sem fallback silencioso |
| Modelo local | Opção futura quando houver API/capacidade compatível e recursos medidos |

Custos são estimativas quando uso/preço não forem medidos. Atingir teto interrompe novos passos; chamadas já em voo podem gerar custo residual. O app mostra essa condição e não promete um teto absoluto de faturamento imposto por um provedor externo.

## 4. Ambiente Kali e pacotes

Identidade da distro gerida; local do disco e workspace; versão WSL/kernel; saúde do executor; usuário Linux; modo de rede; proxies; caches; origem de imagens/repositórios; conjuntos de pacotes; versões e atualização; armazenamento reservado; backup/export; política de reinício; terminal e locale; conectividade; diagnóstico e recuperação.

O usuário pode escolher base/headless, default e grupos por área, consultando o catálogo efetivo. Selecionar everything fica disponível como escolha de instalação, com dependências, espaço e limitações visíveis; não é default nem garantia de executar todas as ferramentas. Compatibilidade de hardware/permissões é por capacidade.

Não aplicar ajustes globais de WSL ou reiniciar outras distros silenciosamente. Adoção de ambiente existente requer fluxo próprio, consentimento e backup. Alterar usuário/root e reinstalar a distro não são “preferências simples”. [Detalhes operacionais](KALI-E-AMBIENTES.md).

## 5. Perfis e alvos

Configurar perfil offline/código, ativo limitado, avançado e manutenção. Cada perfil declara capacidades de leitura/escrita, execução, rede, elevação, artefatos, duração e contenção comprovada. A UI expõe indisponibilidade e motivo se o backend não satisfizer o contrato.

Alvos e escopo têm tipos, endereços/nomes, portas, protocolos, paths, exclusões, redirecionamentos, credenciais, métodos permitidos, janela e taxa. Importar uma lista não autoriza automaticamente cada item; a normalização e a revisão precedem execução. Credenciais de produção nunca são inferidas a partir de arquivos pessoais.

Root é negado por padrão às execuções de avaliação. Uma operação que exige privilégio solicita capacidade específica; manutenção de pacotes usa autorização separada. Não existe toggle que faça o modelo herdar senha sudo ou credenciais do Windows.

## 6. Limites iniciais para ensaiar

| Limite | Valor inicial proposto | Comportamento ao atingir |
|---|---|---|
| Avaliações ativas | 1 | Demais avaliações entram em fila |
| Operações simultâneas globais | 2 | Fila; respeitar restrições mais baixas |
| Operações dinâmicas no mesmo alvo | 1 | Evitar concorrência que invalide estado/limites |
| Workers de IA | Até 3 por avaliação | Coordenador mantém orçamento total, sem delegação ilimitada |
| Timeout padrão de operação | 10 min | Cancelamento supervisionado; ferramenta pode exigir limite próprio aprovado |
| Validade inicial de aprovação de plano | 15 min para iniciar, limitada pela janela do escopo | Revalidar conteúdo e autoridade após expirar, sem renovar por texto da IA |
| Grant técnico de início | 60 s, de uso único | Emitido após prontidão e consumido no dispatch; pode ser reemitido pelo host se a decisão subjacente continuar válida |
| Contexto de um resultado paginado | 64 KiB por página | Cursor e referência ao bruto, sem truncamento invisível |
| Preview de saída na UI | 4 MiB por execução | Virtualizar/paginar; não apagar o bruto já capturado |
| Captura bruta por operação | 256 MiB agregados entre tentativas | Parar/limitar conforme perfil, marcar cap e cobertura parcial; retry não reinicia o orçamento |
| Janela do heartbeat | 5 s, suspeita após 3 perdas | Suspender dispatch e reconciliar, sem declarar kill confirmado |
| Retenção inicial de bruto | 90 dias | Aviso e política de expurgo, respeitando referências/retention lock |
| Retenção de diagnóstico | 14 dias | Rotação local com redaction |

Esses valores não substituem limites específicos de ferramenta/alvo. Taxa de rede, CPU, memória, disco, duração e orçamento são definidos pelo perfil/host; nenhum número de requests/s é seguro para todo alvo. O perfil ativo exige uma taxa concreta antes de executar. Desabilitar cap, timeout ou supervisão é mudança de autoridade/recursos, não mero ajuste visual.

Aprovação humana, grant técnico e lease de processo são objetos distintos. Um grant expirado não cancela por si só um processo já iniciado; a supervisão usa lease/heartbeat e política de término próprios. Reemitir grant não renova aprovação nem amplia escopo. O tempo máximo de uma execução pode ser maior que a validade de início, quando estiver explicitamente autorizado.

Se um cap interromper captura, o hash vale pelos bytes preservados e o manifest indica a lacuna. A avaliação não pode usar esse resultado para afirmar ausência de problema. Aumentar o limite para nova tentativa exige orçamento e decisão coerentes; não se recuperam bytes já descartados.

## 7. Evidências, privacidade e exportação

Local do object store, quotas/alertas, criptografia suportada, retenção por classe, preservação legal/manual quando aplicável, redaction, inclusão de fontes no contexto, acesso a bruto, expurgo e backups. A escolha de criptografia e a implementação de vault exigem validação própria; “arquivos locais” não implica dados criptografados.

Formato/idioma do relatório, filtros de achados, público, inclusão de bruto, revisão de segredos, assinatura quando suportada, pacotes de reprodução e integrações. Exportar/enviar cria uma ação com destino/conteúdo explícitos. Publicar em GitHub/Jira ou enviar mensagens não acontece por concluir uma avaliação.

## 8. MCP e extensões

Gateway interno habilitado, revisão de protocolo negociada, catálogo/perfis, timeout, tamanho, compatibilidade e diagnostics. A proposta inicial não expõe endpoint de rede. Clientes externos e servidores MCP adicionais são opções posteriores com identidade, privilégios mínimos, inspeção do catálogo e aprovação própria.

Instalar ferramenta Kali, habilitar adaptador e habilitar um servidor MCP são ações diferentes. Plugins de terceiros executam código com risco próprio; não recebem acesso ao vault e ao broker Windows por estarem no catálogo. Assinatura/proveniência de plugin não prova comportamento seguro.

## 9. Atualização e recuperação

Canais do app/executor/catálogo/parsers, checagem automática, janela de manutenção, backup prévio, compatibilidade de protocolos, rollback suportado e estado degradado. Atualização do ambiente trava novos jobs na distro; não muda ferramentas de uma avaliação ativa sem encerrar/reconciliar.

Antes de desinstalar, apresentar separadamente aplicativo, distro gerida, dados, evidências, vault e backups. Nenhuma remoção usa somente o nome “Kali” para escolher uma distribuição. Não executar `wsl --unregister` como limpeza automática de uma avaliação.
