# Base e premissas deste mapa

Estado: proposta de produto, em 7 de novembro de 2026. Nenhum requisito deste conjunto significa implementação existente.

## Confirmado pelo usuário

- Nome: OpenKoda.
- Aplicativo local de cibersegurança, com painel semelhante em finalidade e organização ao Codex Security.
- IA capaz de utilizar ferramentas do Kali Linux no WSL 2.
- Opções para instalar conjuntos de ferramentas/pacotes que não vêm na imagem mínima.
- Fluxo inspirado em avaliar, investigar, obter evidências e acompanhar resultados.
- Framework desktop **Tauri**, confirmado explicitamente pelo usuário.
- Identidade e composição das seis imagens em [VISUAL-E-TELAS](../VISUAL-E-TELAS.md), aprovadas pelo usuário: painel, nova avaliação, Kali/ferramentas, achados, projetos e configurações.
- Nesta etapa, mapear produto, funcionamento, telas, configuração e capacidades; construir depois.

## Base proposta para tornar o mapa concreto

- Desktop Windows, inicialmente um usuário local e um computador.
- Uma distribuição Kali dedicada e identificada, gerida pelo OpenKoda, sem apropriar-se da distro pessoal existente.
- Projetos com avaliações de código, web/API, rede autorizada e artefatos offline.
- IA por API configurável e ferramentas expostas por MCP; integração externa com outros clientes MCP em etapa posterior, com autorização própria.
- Painel, coordenador, executor, manutenção e evidências como responsabilidades separadas.
- Nenhum pop-up obrigatório por comando quando uma política válida já autoriza a ação.
- Valores de desempenho e limites neste mapa são defaults propostos para validar em protótipo futuro.

## Herança e limites do repositório pai

O glossário e os ADRs em `D:\OpenKoda` descrevem um planejamento anterior com servidor e dispositivos associados. Eles são preservados. A solicitação atual define um desktop de segurança nesta subpasta. O mapa reutiliza a ideia de que a instalação/executor é autoridade, mas não presume servidor remoto, app Android, pareamento, relay, assinatura ou disponibilidade com o PC desligado.

Essa diferença de implantação é explícita. As propostas desta pasta não substituem silenciosamente os ADRs anteriores. Eventual unificação dos produtos exigirá uma decisão posterior. O glossário local é contexto de planejamento; não migra o domínio ou código do projeto pai.

## Pesquisa utilizada

| Fonte | O que sustenta | Limite |
|---|---|---|
| [Investigação Codex](../../README.md) e seus relatórios | Padrões de workbench, execução, contratos, autoridade e persistência | Comportamento observado do Codex não é implementação nem licença de código do OpenKoda |
| [Modelo de segurança WSL](https://wsl.dev/technical-documentation/security/) | Fronteiras reais entre host, distro e usuário | Distro dedicada e desligar interop não criam por si só uma sandbox contra o host |
| [Metapacotes Kali](https://www.kali.org/docs/general-use/metapackages/) | Grupos de dependências e escolha de conjuntos | Tamanho e compatibilidade devem vir do catálogo/resolução efetivos |
| [Arquitetura Tauri](https://v2.tauri.app/concept/architecture/) e [commands](https://v2.tauri.app/develop/calling-rust/) | Fundação nativa/WebView e comunicação com Rust | Versão, frontend e contratos concretos são propostas a ensaiar |
| [Capabilities Tauri](https://v2.tauri.app/security/capabilities/) | Controle de acesso IPC e limites dos commands da aplicação | Requer configuração explícita; não substitui a autorização do núcleo/executor |
| [Galeria aprovada](../VISUAL-E-TELAS.md) | Identidade, navegação e hierarquia das seis telas | Imagens geradas com exemplos; não comprovam funcionalidades ou acessibilidade |
| [MCP 2025-11-25: transportes](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports) | Stdio/JSON-RPC e separação de mensagens e logs | Revisão explícita para avaliação de compatibilidade, sem afirmar que seja a última ou que forneça contenção |

Fontes consultadas em 7 de novembro de 2026. Afirmações de engenharia propostas, critérios de aceitação e defaults são escolhas deste mapa; as propriedades reais serão demonstradas nas etapas de validação futuras.

## O que continua aberto

Versão Tauri e bibliotecas/frontend complementares; linguagem do executor; distribuição comercial/licença; provedor/modelo e custos; backend exato de isolamento reforçado; mecanismos Linux capazes de impor rede/arquivos no host alvo; política de atualização; assinatura do instalador; extensões/plugins de terceiros; execução remota/múltiplos usuários. Tauri e a direção visual não permanecem em aberto. As escolhas restantes têm opções e critérios em DECISOES.md e FASES-E-VALIDACAO.md.
