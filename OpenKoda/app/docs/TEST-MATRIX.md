# Evidências de teste — fundação E1

Os testes E2E usam a prévia Vite no Microsoft Edge. Cada teste possui contexto e armazenamento próprios. Dados fictícios são criados pelo fluxo sob teste; não reutilizam projetos de outro teste. A criação por formulário pertence ao caso CRUD; navegação e teclado usam o estado vazio. Não há sleeps ou retries automáticos.

## Execução

`npm run test:e2e` inicia/reutiliza Vite em 127.0.0.1:1420. `npm run test:e2e -- --repeat-each=10` verifica repetibilidade. Resultado JSON e screenshots estão em test-results; relatório em playwright-report. Falhas retêm trace, screenshot e log de console/requisições/erros. A configuração usa Edge instalado; não instala navegador adicional nem conecta Kali.

## Matriz

| Teste | Contrato coberto | Limite |
|---|---|---|
| Navegação e estados verdadeiros | V-01…V-06, AC-26; nenhuma afirmação falsa de Kali/IA pronto | UI preview; não prova estado nativo |
| Projeto → avaliação rascunho → reload | Continuidade do fluxo e armazenamento da preview | Não prova SQLite/arquivo nativo, que exige testes Rust e restart real |
| Campos obrigatórios | Integração formulário/validação visível | Validação de domínio/escopo pertence ao núcleo |
| Início sem executor | Erro explícito e rascunho preservado; sem falso progresso | Não prova bloqueios de processo nativo |
| Configurações → salvar → reload | Valores persistidos e ausência de conexão implícita; parte de AC-27 | Não testa vault nem provedor real |
| Teclado e janela 1024 px | Parte de AC-20 e AC-26 | Não equivale a auditoria de leitor de tela ou conformidade total |

## Gate nativo separado

- Compilar Rust/Tauri e pacote Windows.
- Abrir a janela real e verificar requests através de IPC.
- Criar projeto/rascunho, fechar aplicativo e abrir novamente usando dados isolados de teste.
- Rejeitar action desconhecida, campos inválidos e ID/revisão fora de contexto (testes núcleo).
- Garantir que corrupção não apaga o arquivo/banco original.
- Verificar UI sem acesso a shell/vault e origem/capability restritas (AC-25).
- Gerar relatório com incompletude e evidências sem inventar resultado.

Os 28 AC e 23 SEC da especificação permanecem critérios do produto inteiro. Os testes desta etapa não validam WSL, ferramentas, escopo de rede, modelo de IA, chaves de API, cancelamento de processos ou assinatura do instalador. Execuções e resultados reais serão acrescentados abaixo após o frontend estar disponível.

## Harness nativo

Após compilar `src-tauri/target/debug/openkoda.exe` e disponibilizar a UI, executar `node scripts/native-smoke.mjs`. Em build debug que usa devUrl, manter `npm run dev` em execução. O script usa `OPENKODA_DATA_DIR` exclusivamente no processo debug para criar banco/dados isolados em `.local/native-smoke-<uuid>`. A configuração de WebView2/CDP também vale somente para esse processo, em porta temporária loopback; não é configuração de produção.

O harness chama o IPC real do WebView para criar Projeto/Avaliação, rejeitar ação desconhecida, campo de chave proibido e divergência de escopo, exportar relatório, terminar seu próprio processo e abrir novamente. O restart testa sobrevivência à interrupção do processo; não afirma desligamento limpo. Retém screenshots, traces, resultado JSON e diagnóstico. Não instala WSL, pacotes ou credenciais.

O transporte CDP tem limites de fidelidade em relação ao transporte nativo do Playwright: a aceitação nativa aqui cobre smoke funcional/IPC, não toda acessibilidade nem segurança da WebView. Referências: [Playwright connectOverCDP](https://playwright.dev/docs/api/class-browsertype#browser-type-connect-over-cdp) e [depuração WebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/how-to/remote-debugging-desktop).
