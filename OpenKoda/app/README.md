# OpenKoda desktop

**0.2.0 — análise de código com Codex Security.** O OpenKoda controla o SDK oficial pela interface desktop: selecionar uma pasta, iniciar, acompanhar as fases, cancelar e consultar o relatório salvo. [Instalador local x64](releases/0.2.0-local-20261112/OpenKoda_0.2.0_x64-setup.exe) · [Implementação e testes](IMPLEMENTACAO.md) · [Motor e protocolo](security-runtime/README.md) · [Interface](docs/interface/README.md)

## Usar

1. Abra **Configurações → IA e modelos → Adicionar provedor**. Salve a chave, consulte o catálogo, escolha os modelos e clique em **Ativar provedor**.
2. No Início, clique em **Criar projeto** e use **Escolher pasta**. O nome da pasta identifica o projeto. Escolha todo o código ou um caminho relativo específico, selecione um modelo e descreva o objetivo.
3. Revise o envio do código ao provedor e o limite estimado em dólares. Criar o projeto inicia a análise e abre a conversa. A chave fica no cofre DPAPI do Windows.
4. Acompanhe as fases e abra **Cobertura da análise** e **Relatório do Codex Security**. O cancelamento fica disponível durante a execução. Reabrir o aplicativo recupera o registro, sem iniciar outra análise.

O motor instalado aceita a conexão OpenAI e a configuração OpenRouter. O limite de custo exige um modelo com preços reconhecidos pelo SDK instalado; alguns IDs, especialmente os do OpenRouter, podem ser recusados. A varredura real desta versão usou OpenAI com `gpt-6-luna`. Não se deve interpretar o catálogo de modelos como garantia de compatibilidade com o scanner. Uma chamada em andamento pode ultrapassar o limite estimado.

O relatório mostra a estimativa de custo do SDK; o orçamento local a registra separadamente das chamadas de conversa. O histórico de tokens das conversas não detalha as chamadas internas do scanner. Tokens e valores não informados pelo motor permanecem desconhecidos.

A conversa mantém os provedores, modelos e componentes já cadastrados. Enter envia, Shift + Enter insere uma linha e a preferência Ctrl + Enter continua disponível em **Geral**. O botão **+** adiciona anexos. O seletor de modelo e o cancelamento ficam no compositor. A atividade da análise mostra fases públicas do SDK.

O botão **Projeto** reúne Visão geral e Configurações. As tabelas de conversas, execuções, achados e uso permitem busca, ordenação e paginação. **Mostrar ao lado da conversa** mantém o chat aberto. A navegação principal, a lista de projetos e os menus de Configurações têm rolagem independente. Perfil, aparência, provedores e histórico de uso conservam os componentes existentes.

## Escopo desta edição

A 0.2.0 trabalha com pastas locais de código. A criação de projetos por URL, rede, os executores anteriores e as operações Kali/WSL foram retirados da interface e bloqueados na entrada nativa. Filas antigas não são retomadas automaticamente. Módulos e formatos históricos continuam no código para leitura, recuperação e testes de persistência; sua presença não libera essas operações.

O relatório oficial informa os limites efetivos da revisão. Uma análise concluída ou sem achados não comprova revisão de todos os arquivos nem ausência de vulnerabilidades. Os achados importados entram como candidatos vinculados ao relatório; não se tornam validações humanas automaticamente. O OpenKoda não aplica correções do scanner nesta edição.

Os dados ficam em `%USERPROFILE%\.openkoda`. Backups SQLite preservam os relatórios incorporados, mas não incluem credenciais nem a árvore de trabalho do SDK em `security-scans/`. Essa árvore contém artefatos oficiais, estado e configurações isoladas por execução; também permanece fora da exclusão lógica de projetos. Para voltar à 0.1.18, use um backup feito antes de abrir o perfil na 0.2.0. Os instaladores locais da 0.1.18 e 0.1.17 continuam disponíveis para recuperação.

## Desenvolvimento

Pré-requisitos Windows x64: Node 24, Python 3.11+, Git para Windows, Rust 1.90+ com MSVC, ferramentas C++ do Visual Studio e WebView2. O motor distribuído inclui Node, Python e Codex; Git para Windows continua sendo um requisito local detectado pelo diagnóstico.

```powershell
npm ci
pwsh -NoProfile -File scripts/prepare-security-runtime.ps1
npm run tauri dev
```

O preview `npm run dev` é identificado e usa dados de demonstração. A análise oficial precisa do núcleo Tauri, da credencial e de acesso ao provedor. Confira o motor em **Configurações → Diagnóstico e suporte**.

```powershell
pwsh -NoProfile -File scripts/check-foundation.ps1
pwsh -NoProfile -File scripts/package-windows.ps1
```

A preparação do motor restaura o lockfile independente em `security-runtime/`. Os binários portáteis e `node_modules` não são versionados. O empacotamento inclui o runtime e seus avisos de licença. O instalador local não tem assinatura digital de código; isso não representa assinatura comercial ou planos de uso.

O gate de fundação executa build, testes unitários, Rust, a suíte Edge da edição atual, o protocolo offline do SDK e um ensaio desktop sem inferência paga. O teste real do SDK tem recibo separado. A suíte funcional da edição anterior continua reproduzível no commit `01c1960`; os percursos retirados foram substituídos por testes de bloqueio e de análise de código, conforme [a matriz de validação](docs/RETOMADA-0.2.0.md).
