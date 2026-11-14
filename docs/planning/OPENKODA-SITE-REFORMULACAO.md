# Reformulação do site — 14/11/2026

## Diagnóstico e direção

A demonstração anterior repetia a interface em HTML próprio; já divergia das barras e configurações do desktop. A conta reunia todas as tarefas numa grade. A apresentação repetia efeitos de entrada e fragmentava letras, sem relação suficiente com o assunto.

Mantivemos o símbolo K. A direção escolhida após a nova orientação do usuário é uma bancada de trabalho técnica: grafite, superfícies claras frias e destaque lima. Títulos, controles e corpo usam uma família sem serifa; nomes de arquivos e dados recebem monoespaçada. O aplicativo real mantém seus próprios estilos dentro da janela. Etapas, tabelas, registros e imagens têm composições distintas. Texto essencial permanece visível antes e durante o movimento.

## Referências observadas

- [OpenBot](https://openbot.run/pt): navegação da demonstração, configurações dentro da janela e rolagem externa; versão estreita observada. Aproveitamos o formato incorporado, com o OpenKoda como fonte visual.
- [OpenRouter](https://openrouter.ai/): página pública e modal de entrada examinados no navegador. A área autenticada não estava acessível nessa sessão; sua organização foi observada na gravação fornecida pelo usuário.
- [OpenAI](https://openai.com/pt-BR/): página inicial editorial e página de segurança, incluindo rolagem e diferentes composições por assunto. A gravação também mostrou artigos com índice lateral.

As gravações foram examinadas por quadros e comparadas às interações do navegador. As trilhas de áudio estavam silenciosas. Não foram republicadas gravações, textos ou materiais dessas marcas.

## Inventário da demonstração

| Área | Comportamento previsto |
|---|---|
| Projetos | Abrir, criar com pasta/URL fictícia, escolher modelo e iniciar análise preparada |
| Conversas | Histórico coerente, nova conversa, mensagem digitada, resposta progressiva e cancelamento |
| Relatório | Candidato, trechos, cobertura e relatório do exemplo Loja Aurora |
| IA e modelos | Seleção, gerenciamento de catálogo fictício e raciocínio nos modelos ilustrativos |
| Configurações | Navegação pelas categorias reais, aparência e histórico de uso |
| Perfil | Conta, plano e atendimento fictícios; sem login ou comunicação reais |
| Reinício | Restaura os exemplos e descarta alterações locais da janela |

O build monta `App.tsx` e seus componentes/estilos reais. `app/vite.demo.config.ts` substitui apenas o transporte `api.ts`; o desktop continua com seu transporte Tauri original. O adaptador vive em `app/demo/`. Arquivos de exemplo substituem seletores de arquivos reais; links e recursos nativos sem simulação recebem explicação local.

O iframe tem origem opaca (`sandbox allow-scripts allow-forms`). `allow-forms` libera o evento local React; `form-action 'none'` continua impedindo envio nativo. A política também impede conexões, subframes e workers. Não há acesso ao DOM/cookies do pai. Preferências usam Storage em memória instalado antes da importação da aplicação. CORS público aplica-se somente aos assets da demonstração, não às rotas da conta.

## Conta e movimento

A conta tem seis destinos: Visão geral, Plano, Uso e limites, Chamados, Preferências, Dados e acesso. Os formulários preservam rascunhos ao mudar de seção; ações têm feedback próximo, espera e proteção contra duplicação. Dados indisponíveis são identificados sem fabricar saldo. Google, CAPTCHA, cookies e PKCE mantêm o contrato existente.

| Composição | Gatilho e movimento | Celular/redução de movimento |
|---|---|---|
| Revisão em três momentos | Captura fixa acompanha a etapa lida na rolagem natural; botões escolhem a captura | Captura no fluxo e escolha por botões; texto sempre disponível |
| Percurso do consumo | Clique mostra autorização, resposta e registro de uso | Mesmos controles; estado final imediato com movimento reduzido |
| Cards de leitura | Foco/ponteiro destaca linha de código, cobertura ou conversa relacionada | Conteúdo continua legível sem hover |
| Índice dos guias | Marca a seção em leitura | Links normais e navegação por teclado |

## Reprodução

1. Em `OpenKoda/app`, executar `npx tsc --noEmit -p demo/tsconfig.json` e `npx vite build --config vite.demo.config.ts`.
2. Na raiz, executar `node OpenKoda/site/build.mjs`.
3. Rodar os testes Node em `OpenKoda/site/tests/` e os specs `site-*.spec.ts` do aplicativo. Os testes de conta usam serviços simulados; os testes da demonstração usam o Worker compilado e suas políticas reais.
4. O repositório do site recebe os assets compilados da demonstração, incluindo o HTML de entrada. A publicação é gerada desse estado, sem chaves ou dados pessoais.

## Limites desta entrega

Os exemplos não executam análise real nem consomem APIs. Não houve alteração de domínio, Asaas, banco ou configuração de autenticação. A nova navegação autenticada é verificada com fixtures de usuário; isso não substitui uma sessão pessoal de produção. Leitor de tela real não foi exercitado. O aplicativo é para Windows; a janela do site adapta sua interface para apresentação no celular.

## Verificação concluída

- 40 testes de interface passaram em execução integrada no Edge. Cobrem a conta, os aceites e marketing, restauração de sessão, PKCE, recuperação, aparência entre abas, menus, teclado, guias, exemplos interativos e a demonstração real.
- 12 testes Node passaram: origem/CSRF, cookies, CAPTCHA, recuperação, download com retomada, links e política de isolamento da demonstração.
- A janela foi verificada em 390, 768 e 1440 px. A roda dentro do histórico permanece na janela; fora dela, a página continua rolando. Reiniciar a demonstração preserva preferências e dados do site.
- Na conferência publicada, uma janela de 334 px revelou deslocamento lateral interno após enviar a mensagem. A correção ficou somente no CSS demonstrativo: os controles se reorganizam em linhas. Três novos casos em 320/334/350 px e o novo caso incorporado em 375 px passaram, medindo a conversa e os controles internos. Os cinco casos de isolamento/layout da janela foram repetidos; os dois estreitos foram confirmados no build final. Foram exercitados 44 casos distintos de interface no total.
- Revisão visual independente: nove capturas públicas e cinco da conta, nos temas claro/escuro. Sem bloqueadores identificados nas capturas examinadas.
- TypeScript e build da demonstração passaram. O build desktop e os 84 testes unitários existentes também passaram durante esta entrega; o transporte de produção permaneceu intacto.
- Nenhum serviço de inferência foi acionado: gasto de API nesta reformulação, US$ 0.

As animações incluem mudança da captura durante as etapas da revisão, destaque de evidência, percurso de um envio e atualização da cota. A preferência de movimento reduzido remove a transição; nenhuma explicação depende de aguardar letras ou caixas aparecerem.
