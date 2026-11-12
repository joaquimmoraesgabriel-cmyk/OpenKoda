# Retomada 0.2.0 — edição de código

O motor ativo passa a ser o SDK oficial `@openai/codex-security` 0.10.0, com Codex 0.167.1. O Tauri mantém a autoridade sobre escopo, credenciais, subprocessos, persistência e orçamento. A interface acompanha fases públicas, cancelamento e resultado. O pacote do SDK tem lockfile separado e o runtime portátil integra o instalador.

## Teste real no próprio OpenKoda

Em 12/11/2026, o aplicativo nativo criou um projeto apontando para uma cópia Git do código OpenKoda, iniciou o SDK com OpenAI e `gpt-6-luna` e recuperou o mesmo resultado após fechar e abrir o mesmo perfil. O reenvio da solicitação manteve uma única execução. A credencial foi removida no encerramento do teste. O limite informado foi US$ 5; o SDK estimou US$ 0,270424255.

- Revisão analisada: `58fa65e48886defed56dc6e9569b0531ffc04acf`, snapshot dedicado com `src`, `src-tauri/src`, manifests e código do helper. Não inclui arquivos TXT da raiz, credenciais, perfis ou recibos locais.
- Execução oficial: `6cbd978b-af83-4750-b2a4-171ba0ef6c95`, concluída com manifesto selado.
- Resultado: zero achados reportados. **Cobertura parcial: 39 arquivos integralmente revisados entre 252 estimados; 213 sem revisão.** Os scripts auxiliares anteriores, a configuração Tauri e os componentes internos do SDK não estavam no snapshot. Isso é um teste real da integração, não uma certificação do aplicativo inteiro ou de mudanças posteriores.
- [Relatório oficial](security/0.2.0/report.md), [manifesto](security/0.2.0/scan-manifest.json), [achados](security/0.2.0/findings.json) e [recibo sanitizado](security/0.2.0/integration-receipt.json).

A execução usou um binário de verificação 0.2.0 com SHA-256 `d61b48a2840345ed2944592ceb29bba71d65786ce6fc4dd4fa29844b7dff7fa0`. Depois desse resultado, foi ativado o corte dos executores anteriores. O binário de distribuição e sua regressão possuem provas próprias.

## Produto e dados

A criação de projetos agora oferece apenas código local. URL, rede e Kali foram retirados da navegação e bloqueados na entrada nativa. Não há retomada automática das filas antigas. O código de leitura dos formatos anteriores permanece para compatibilidade e recuperação. A retirada foi ativada somente depois da análise real concluída.

Cada execução tem um recibo durável e chave de idempotência. Interrupção de processo não vira conclusão, cobrança zero ou repetição automática. O relatório incorporado à evidência acompanha o backup SQLite. Artefatos brutos em `security-scans/` ficam fora do backup e do expurgo lógico. O retorno à 0.1.18 exige backup anterior à atualização.

O catálogo continua compartilhado entre configurações e compositor. O scanner exige um modelo aceito pelo SDK e com preços verificáveis; alguns modelos OpenAI e IDs OpenRouter são recusados antes da análise. Os demais provedores continuam na conversa. Não há troca silenciosa de provedor, modelo ou remoção do limite financeiro.

## Validação da edição

A suíte de referência 0.1.19 está no commit `01c1960`. Seus testes de TCP, HTTP por URL, Kali/APT/WSL, MCP, captura e executores antigos pertencem àquela edição. A suíte padrão 0.2.0 testa a nova análise de código e os recursos preservados, incluindo o bloqueio nativo das operações retiradas. Não há bypass do núcleo nativo para testes.

Os ensaios offline do helper usam credencial sintética. Os ensaios de interface simulam o transporte nativo. A abertura desktop e a chamada real do SDK são verificações separadas. Os recibos finais de build, testes e instalador são registrados em IMPLEMENTACAO.md.

## Referência visual

A interface mantém os provedores, configurações, perfil e componentes previamente aprovados com referência Cindy. O workspace adota a organização das referências fornecidas do Codex: trilho de ícones, projetos e conversas em lateral própria, conteúdo central, compositor compacto e um único painel Projeto com tabelas interativas.

O REA abriu o pacote instalado do Codex e reconheceu `app.asar`. Sua observação nativa nesta instalação retornou `capability_unavailable`, porque o leitor disponível exige macOS. A referência visual usa as capturas fornecidas e as evidências anteriores; não se afirma inspeção completa da interface Windows pelo REA.
