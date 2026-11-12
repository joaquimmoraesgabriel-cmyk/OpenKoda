# OpenKoda desktop

**0.2.1 — análise de código de pasta local ou repositório público do GitHub.** O OpenKoda cria o projeto, inicia a análise e reúne a conversa, a atividade pública e o relatório. Esta é uma versão local para testes. O instalador passou por instalação, duas aberturas e desinstalação nos runners Windows; a análise automatizada completa da 0.2.1 no próprio código ainda não foi concluída. [Estado da versão](docs/RETOMADA-0.2.1.md) · [Implementação e testes](IMPLEMENTACAO.md) · [Validação e limites](docs/security/0.2.1/VALIDACAO.md) · [Interface](docs/interface/README.md)

## Usar

1. Abra **Configurações → IA e modelos → Adicionar provedor**. Salve a chave, consulte o catálogo, selecione os modelos e clique em **Ativar provedor**. A credencial fica no cofre DPAPI do Windows.
2. No Início, clique em **Criar projeto**. Escolha **Pasta local** e use **Escolher pasta**, ou escolha **GitHub** e cole o link de um repositório público. O OpenKoda usa o nome da pasta ou do repositório para o projeto.
3. Escolha **Todo o código** ou **Uma pasta ou arquivo**, selecione um modelo OpenAI e descreva o que quer verificar. O caminho específico deve ficar dentro do projeto. **Contexto adicional** permite informar vetores de ataque, áreas de foco e contexto de segurança.
4. Em **Opções avançadas**, ajuste o título, o prazo para iniciar operações e, se quiser, ative **Personalizar limite de custo**. Sem personalização, a análise usa o teto estimado padrão de **US$ 5**. Uma chamada em andamento pode ultrapassar esse valor; ele não garante a fatura do provedor.
5. Revise os avisos de leitura e envio. **Criar projeto** baixa a cópia, quando necessário, e inicia a análise. Acompanhe as atualizações e abra **Cobertura da análise** ou **Relatório da análise**. O cancelamento fica disponível durante a execução. Reabrir o aplicativo consulta o registro salvo e não inicia outra análise automaticamente.

**A análise de código usa OpenAI nesta versão. Os demais provedores continuam disponíveis nas conversas.** O catálogo compartilhado permite trocar provedor e modelo no compositor. O SDK fixado não consegue verificar o preço dos IDs OpenRouter usados no preflight com limite de custo; o OpenKoda recusa essa análise antes de baixar ou criar o projeto. Não há troca silenciosa de modelo nem remoção do teto para contornar a recusa.

**Os comandos da análise podem ler outros arquivos acessíveis à sua conta Windows. Use apenas código em que você confia nesta versão local.** A seleção de pasta, arquivo ou repositório orienta o trabalho, mas não estabelece isolamento de leitura por projeto no sistema operacional. Consulte os [limites do executor local](docs/RETOMADA-0.2.1.md#limites-do-executor-local) antes de analisar código de terceiros.

## Conversas e opções do projeto

Enter envia e Shift + Enter insere uma linha; a preferência Ctrl + Enter continua disponível em **Geral**. O campo é limpo quando a mensagem é admitida pelo aplicativo. Uma recusa anterior à admissão mantém o rascunho. O botão **+** adiciona anexos e o compositor reúne seleção de modelo e cancelamento. A atividade mostra fases, operações e mensagens públicas recebidas do executor, respeitando a política de dados; não exibe raciocínio privado.

O botão **Projeto** reúne **Visão geral** e **Configurações**, com tabelas de conversas, execuções, achados e uso, busca, ordenação e paginação. **Mostrar ao lado da conversa** mantém o chat disponível. Navegação principal, lista de projetos, categorias de configurações e conteúdo têm rolagem independente.

Em **Projeto → Configurações**, você pode ativar ou editar **Contexto adicional** e **Opções avançadas** depois da criação. Essas preferências valem para os próximos trabalhos; não reescrevem avaliações iniciadas, não alteram seu escopo congelado e não renovam autorizações revogadas. Perfil, aparência, provedores e histórico de uso conservam os componentes existentes.

Repetir uma solicitação de criação ou consultar um download recupera a intenção e seus recibos. Cancelar o download impede o início da análise, mesmo se a cópia terminar naquele instante. Após uma falha, interrupção ou cancelamento confirmados, **Tentar novo download** cria outra tentativa explícita. Se a resposta se perder e o estado permanecer desconhecido, a mesma intenção é consultada sem iniciar outro clone.

## Dados e escopo desta edição

Esta edição trabalha com código. O link do GitHub fornece uma cópia local com origem e revisão registradas. Análises de rede, Web/API por URL e operações Kali/WSL permanecem retiradas da interface e bloqueadas na entrada nativa. Filas antigas não são retomadas automaticamente. Módulos e formatos históricos permanecem para leitura, recuperação e regressões delimitadas; sua presença não libera essas operações.

O relatório informa a cobertura efetiva. Uma análise concluída ou sem achados não comprova leitura de todos os arquivos nem ausência de vulnerabilidades. Resultados importados entram como candidatos vinculados ao relatório e não como validações humanas automáticas. O OpenKoda não aplica correções do scanner nesta edição. A estimativa de custo da análise é registrada separadamente das conversas; tokens ou valores não informados continuam desconhecidos.

Os dados ficam em `%USERPROFILE%\.openkoda`. Backups SQLite preservam os relatórios incorporados, mas não incluem credenciais, cópias de repositórios importados nem a árvore de trabalho em `security-scans/`. Esses materiais também ficam fora da exclusão lógica do projeto. Para voltar à 0.1.18, use um backup anterior à abertura do perfil nas edições 0.2.x; não conte com compatibilidade reversa do banco atualizado.

## Desenvolvimento e validação

Pré-requisitos Windows x64: Node 24, Python 3.11+, Git para Windows, Rust 1.90+ com MSVC, ferramentas C++ do Visual Studio e WebView2. O runtime distribuído inclui Node, Python e Codex; Git para Windows continua sendo um requisito local verificado pelo diagnóstico.

```powershell
npm ci
pwsh -NoProfile -File scripts/prepare-security-runtime.ps1
npm run tauri dev
```

O preview `npm run dev` usa dados de demonstração identificados. A análise precisa do núcleo Tauri, de credencial e de acesso ao provedor. Confira os componentes em **Configurações → Diagnóstico e suporte**.

```powershell
pwsh -NoProfile -File scripts/check-foundation.ps1
pwsh -NoProfile -File scripts/package-windows.ps1
```

A integração usa o SDK oficial `@openai/codex-security` 0.10.0, com lockfile independente em `security-runtime/`. A preparação restaura as dependências fixadas e o empacotamento inclui os avisos de licença. Os binários portáteis e `node_modules` não são versionados. [Motor e protocolo](security-runtime/README.md).

A 0.2.1 passou em 79 testes unitários de interface, 101 casos Edge, 638 testes Rust e 20 testes offline do runtime. Três testes opt-in de host/rede ficaram separados; a importação GitHub real passou em seu ensaio próprio. O executável nativo final preservou duas conversas após reabertura e recusou 14 rotas antigas. Instalação, duas aberturas, persistência de configurações e desinstalação passaram em Windows Server 2022, Server 2025 e Windows 11 Arm com o aplicativo x64. Os recibos identificam cada binário e o pacote entregue. O ensaio real encontrou recusa OpenAI por limite/saldo; duas respostas reais OpenRouter foram verificadas nas conversas. Isso não substitui uma análise completa de segurança da versão. O MCP Deep também ficou bloqueado antes de ler arquivos; a revisão manual do código possui inventário separado. Consulte a [matriz da 0.2.1](docs/RETOMADA-0.2.1.md) para distinguir essas provas.

O instalador local é preparado sem assinatura digital de código. Isso se refere à identificação do publicador do executável, não a assinatura comercial ou planos de uso. Empacotar não comprova instalar, atualizar ou desinstalar; a aprovação para publicação pública permanece pendente.
