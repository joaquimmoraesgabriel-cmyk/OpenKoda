# Decisões, propostas e pontos de escolha

Este mapa é uma proposta revisável. “Recomendado” significa base de planejamento; não é compra, instalação, implementação ou confirmação do usuário. Requisitos explícitos permanecem em [BASE-E-PREMISSAS](referencias/BASE-E-PREMISSAS.md).

## Decisões de produto confirmadas

Nome OpenKoda; desktop de cibersegurança; workbench inspirado na finalidade do Codex Security; ferramentas Kali no WSL 2; escolha de conjuntos de pacotes; IA capaz de planejar/utilizar essas ferramentas; framework desktop Tauri confirmado pelo usuário em 7 de novembro de 2026; agora somente mapa e imagens conceituais. O produto não se resume a um terminal embutido: o objetivo é ligar trabalho, evidência, resultado e tratamento em uma interface.

Também foram aprovados o painel e os cinco conceitos adicionais: seis referências em [VISUAL-E-TELAS](VISUAL-E-TELAS.md). Fixam identidade, navegação e hierarquia, preservando a especificação funcional e os estados reais. O [ADR-0004](../docs/adr/0004-tauri-como-framework-desktop.md) formaliza Tauri; nenhuma aprovação visual promove controles propostos a controles já demonstrados.

## Propostas técnicas e critérios

| ID | Tema | Base recomendada para o mapa | Alternativas e condição para decidir |
|---|---|---|---|
| D-01 | Implantação inicial | Windows, um usuário local, núcleo e dados locais | Multiusuário/servidor remoto acrescentam autenticação, operação e autoridade próprias |
| D-02 | Distro | Kali dedicada/gerida com identidade exclusiva | Distro externa explicitamente adotada pode ser suportada, sem prometer controle equivalente |
| D-03 | Interface | Tauri confirmado pelo usuário; proposta de Tauri 2 + React/TypeScript, com núcleo/broker Rust e executor Kali separado | Validar versão, bibliotecas, WSL, IPC, supervisão, acessibilidade, WebView2, atualização e distribuição; a escolha de Tauri não implica implementação iniciada |
| D-04 | Núcleo/broker/executor | Contratos separados por autoridade e módulo | Linguagens/processos serão escolhidos após prova de IPC, supervisão e distribuição |
| D-05 | IA | Adaptador de provedor, ferramentas tipadas, orçamento e contexto explícitos | Endpoint/modelo concreto escolhido por capacidade, custo, privacidade e qualidade medida |
| D-06 | Ferramentas | Adaptadores preferenciais e shell avançado com política | Shell sozinho acelera cobertura, mas perde contratos específicos e exige controles extras |
| D-07 | Transporte interno | MCP stdio supervisionado, IPC local da UI separado | HTTP local/remoto só com necessidade, autenticação e superfície de rede revisada |
| D-08 | Autoridade | Executor valida escopo/plano/perfil e grants concretos | Decisão apenas na UI ou prompt não satisfaz o contrato |
| D-09 | Persistência | Banco relacional local + blobs imutáveis + journal/outbox | Avaliar SQLite em falhas, concorrência e migração antes de fixar biblioteca |
| D-10 | Fonte Windows | Snapshot/staging selecionado para Linux | Mount de projeto vivo pode ser opção explícita, com risco e autoridade de escrita próprios |
| D-11 | Isolamento | Declarar perfil realmente suportado no WSL e bloquear capacidades incompatíveis | Backend separado para tarefas hostis/privilegiadas quando contenção forte for necessária |
| D-12 | Atualizações | Componentes versionados, sem atualizar ambiente durante avaliação | Canal/manual/automático depende de assinatura, rollback e política de manutenção demonstrados |
| D-13 | Plugins | Catálogo próprio de adaptadores no início | Plugins terceiros/MCP externo entram após contrato de identidade, autoridade e supply chain |
| D-14 | Relatórios | Canônicos estruturados + projeções humanas | Formatos extras entram por requisito de uso, sem tornar PDF fonte de verdade |
| D-15 | Operação com app fechado | Encerramento supervisionado como base | Continuação exige serviço independente demonstrado e opção explícita |

Nenhuma escolha de framework autoriza usar generic shell plugins diretamente na interface. Uma UI nativa também precisa do mesmo contrato de autoridade. Não será copiado código ou asset proprietário do Codex para montar o painel.

## Pontos a resolver antes de construir cada parte

1. **Perfis ativos WSL:** quais controles conseguem impor destino, rede, diretórios, subprocessos e quotas no host real? Uma demonstração negativa fora de escopo é critério de entrada, não tarefa pós-lançamento.
2. **Backend reforçado:** qual tecnologia atende às tarefas que não cabem no perfil WSL? Manter opção futura não permite declarar isolamento inexistente na primeira versão.
3. **Stack desktop:** com Tauri escolhido, testar IPC, acessibilidade, logs, atualizador, assinatura e empacotamento do executor em Windows suportado; validar versão e bibliotecas complementares.
4. **Provedor/modelo:** confirmar tool calling, streaming, contexto, uso/custo, limites e política de dados. “Compatível com uma API” não prova todas as capacidades.
5. **Distribuição/licença:** decidir modelo aberto/comercial, obrigações de redistribuição de dependências/imagens e canais de atualização antes de publicar. Não se presume licença do produto pela inspiração no Codex.
6. **Clientes externos MCP:** decidir se o usuário quer usar OpenKoda como ferramenta de outros apps. O executor pode manter o mesmo núcleo de política, com novas identidades e grants.
7. **Alvos de produção:** definir quais efeitos serão suportados inicialmente, requisitos de comprovação de escopo e operação de recuperação/reteste.
8. **Retenção/criptografia:** confirmar threat model, armazenamento, recuperação de chave e limites reais de backup/exclusão.

Esses pontos são decisões futuras do projeto. Não exigem responder a um questionário para compreender o mapa; estão explícitos para não virarem suposições invisíveis durante implementação.

## Registros de arquitetura

Os ADRs desta subpasta têm escopo no novo desktop. 0001–0003 permanecem **proposed**; 0004 é **accepted** para a escolha de Tauri. Não substituem os ADRs do projeto pai.

- [Autoridade do executor](../docs/adr/0001-autoridade-do-executor-local.md).
- [Kali dedicado e perfis de contenção](../docs/adr/0002-kali-dedicado-e-limites-de-isolamento.md).
- [Bruto preservado e projeções](../docs/adr/0003-evidencia-bruta-e-projecoes.md).
- [Tauri como framework desktop — aceito](../docs/adr/0004-tauri-como-framework-desktop.md).
