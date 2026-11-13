# OpenKoda desktop

**0.2.2 — revisão estática de código de uma pasta ou repositório público do GitHub.** O OpenKoda inicia a revisão com o provedor e o modelo escolhidos. A conversa reúne o progresso, os candidatos encontrados e os trechos que sustentam cada um. [Implementação e testes](IMPLEMENTACAO.md) · [Validação anterior da 0.2.1](docs/security/0.2.1/VALIDACAO.md)

## Usar

1. Abra **Configurações → IA e modelos → Adicionar provedor**. Salve a chave, consulte o catálogo, selecione os modelos e clique em **Ativar provedor**. A credencial fica no cofre DPAPI do Windows.
2. No Início, clique em **Criar projeto**. Escolha **Pasta local** e use **Escolher pasta**, ou escolha **GitHub** e cole o link de um repositório público. O OpenKoda usa o nome da pasta ou do repositório para o projeto.
3. Escolha **Todo o código** ou **Uma pasta ou arquivo**, selecione um modelo de texto ativo e descreva o que quer verificar. O caminho específico deve ficar dentro do projeto. **Contexto adicional** permite informar vetores de ataque, áreas de foco e contexto de segurança.
4. Em **Opções avançadas**, ajuste o título, o prazo para iniciar operações e, se quiser, ative **Personalizar limite de custo**. Sem personalização, a análise usa o teto estimado padrão de **US$ 5**. Uma chamada em andamento pode ultrapassar esse valor; ele não garante a fatura do provedor.
5. Revise os avisos de leitura e envio. **Criar projeto** baixa a cópia, quando necessário, e inicia a análise. Acompanhe as atualizações e abra **Cobertura da análise** ou **Relatório da análise**. O cancelamento fica disponível durante a execução. Reabrir o aplicativo consulta o registro salvo e não inicia outra análise automaticamente.

**Provedores.** OpenAI, OpenRouter, Anthropic, Google Gemini, Groq, DeepSeek e xAI usam a credencial cadastrada para o provedor selecionado. A análise precisa de um modelo de texto capaz de devolver resultados estruturados. Para um modelo direto sem tarifa conhecida, cadastre os preços em **Configurações → Histórico de uso → Análises → Gerenciar limites e preços**. O OpenRouter fornece o preço pelo catálogo.

**Isolamento.** O leitor do Windows captura texto dentro do alvo selecionado. Ignora links, pontos de reanálise, pastas de credenciais e binários. O modelo recebe trechos dessa cópia em memória. A revisão é estática e não executa comandos nem o código do repositório. A cobertura do relatório identifica as exclusões e os lotes examinados. O executor de comandos do SDK usado na 0.2.1 está fora deste fluxo.

## Conversas e opções do projeto

O **+** ao lado do projeto abre um rascunho. A primeira mensagem admitida dá nome à conversa e a inclui na barra lateral. Enter envia e Shift + Enter insere uma linha. A preferência Ctrl + Enter continua disponível em **Geral**. O campo é liberado após a admissão. O texto público da resposta aparece conforme chega e a leitura continua na posição escolhida ao rolar para cima.

Para mudar o título, use **Renomear conversa**. Para revisar os arquivos, escreva o objetivo e escolha **Adicionar à mensagem → Analisar código do projeto**. Em uma conversa existente, a análise abre outra conversa e conserva a seleção de arquivos e as autorizações da origem. O relatório registra as leituras e buscas feitas sobre a cópia autorizada.

O botão **Projeto** reúne **Visão geral** e **Configurações**, com tabelas de conversas, execuções, achados e uso, busca, ordenação e paginação. **Mostrar ao lado da conversa** mantém o chat disponível. Navegação principal, lista de projetos, categorias de configurações e conteúdo têm rolagem independente.

Em **Projeto → Configurações**, você pode ativar ou editar **Contexto adicional** e **Opções avançadas** depois da criação. Essas preferências valem para os próximos trabalhos; não reescrevem avaliações iniciadas, não alteram seu escopo congelado e não renovam autorizações revogadas. Perfil, aparência, provedores e histórico de uso conservam os componentes existentes.

Repetir uma solicitação de criação ou consultar um download recupera a intenção e seus recibos. Cancelar o download impede o início da análise, mesmo se a cópia terminar naquele instante. Após uma falha, interrupção ou cancelamento confirmados, **Tentar novo download** cria outra tentativa explícita. Se a resposta se perder e o estado permanecer desconhecido, a mesma intenção é consultada sem iniciar outro clone.

## Dados e escopo desta edição

Esta edição trabalha com código. O link do GitHub fornece uma cópia local com origem e revisão registradas. Análises de rede, Web/API por URL e operações Kali/WSL permanecem retiradas da interface e bloqueadas na entrada nativa. Filas antigas não são retomadas automaticamente. Módulos e formatos históricos permanecem para leitura, recuperação e regressões delimitadas; sua presença não libera essas operações.

O relatório informa a cobertura efetiva e registra possíveis vulnerabilidades como candidatos para validação. Em **Configurações → Histórico de uso**, **Histórico** mostra os tokens informados e **Análises** reúne os registros de custo e os controles de orçamento. Medições incompletas permanecem identificadas.

Os dados ficam em `%USERPROFILE%\.openkoda`. Backups SQLite preservam os relatórios incorporados, mas não incluem credenciais, cópias de repositórios importados nem a árvore de trabalho em `security-scans/`. Esses materiais também ficam fora da exclusão lógica do projeto. Para voltar à 0.1.18, use um backup anterior à abertura do perfil nas edições 0.2.x; não conte com compatibilidade reversa do banco atualizado.

## Desenvolvimento e validação

O desenvolvimento para Windows x64 usa Node 24, Python 3.11+, Git para Windows, Rust 1.90+ com MSVC, ferramentas C++ do Visual Studio e WebView2. O importador de repositórios públicos usa Git para Windows. A distribuição ainda inclui os componentes do runtime da versão anterior.

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

A análise na 0.2.2 usa o leitor e o transporte de IA do núcleo Rust. A distribuição também inclui o runtime do SDK `@openai/codex-security` 0.10.0 com lockfile próprio. A preparação restaura as dependências fixadas e o empacotamento inclui os avisos de licença. [Runtime e protocolo das versões anteriores](security-runtime/README.md).

A 0.2.1 passou em 79 testes unitários de interface, 101 casos Edge, 638 testes Rust e 20 testes offline do runtime. Três testes opt-in de host/rede ficaram separados; a importação GitHub real passou em seu ensaio próprio. O executável nativo final preservou duas conversas após reabertura e recusou 14 rotas antigas. Instalação, duas aberturas, persistência de configurações e desinstalação passaram em Windows Server 2022, Server 2025 e Windows 11 Arm com o aplicativo x64. Os recibos identificam cada binário e o pacote entregue. O ensaio real encontrou recusa OpenAI por limite/saldo; duas respostas reais OpenRouter foram verificadas nas conversas. Isso não substitui uma análise completa de segurança da versão. O MCP Deep também ficou bloqueado antes de ler arquivos; a revisão manual do código possui inventário separado. Consulte a [matriz da 0.2.1](docs/RETOMADA-0.2.1.md) para distinguir essas provas.

O instalador local é preparado sem assinatura digital de código. Isso se refere à identificação do publicador do executável, não a assinatura comercial ou planos de uso. Empacotar não comprova instalar, atualizar ou desinstalar; a aprovação para publicação pública permanece pendente.
