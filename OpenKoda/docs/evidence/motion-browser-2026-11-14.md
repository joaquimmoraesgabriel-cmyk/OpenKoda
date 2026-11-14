# Movimento — verificação de 14/11/2026

## Site

Ensaio no navegador integrado do Codex, com a versão local do site e instrumentação temporária excluída da publicação. Não representa teste em um telefone físico.

- Fundo de abertura carregou com `data-matter-ready=true`, sem erros no console.
- Em 20 segundos sem interação, os checksums dos pixels foram diferentes em 0, 3, 8 e 20 segundos: há evolução autônoma, não apenas movimento ao passar o mouse.
- Medição após aquecimento: 1.199 intervalos, mediana 18,1 ms, p95 24,3 ms e um intervalo acima de 50 ms. São observações desta execução; não constituem garantia de desempenho em outros dispositivos.
- Com pausa, os quatro checksums ficaram idênticos durante outros 20 segundos. Retomar atualizou os controles das duas cenas. A preferência permaneceu coerente ao recarregar outra aba.
- Apresentação inspecionada em larguras de 320, 390, 768 e 1.366 CSS px. Não houve overflow horizontal do documento. O menu móvel abriu e fechou; temas claro e escuro mantiveram os controles legíveis.
- Selecionar Ultra apresentou R$ 1.999 e US$ 200 por ciclo; seta para a esquerda selecionou Pro. A seleção apenas compara planos.
- A inspeção encontrou a herança de `subgrid` do cartão gratuito, que empurrava seu conteúdo para baixo. O override local da nova comparação corrigiu a posição; verificado novamente no navegador.
- Copiar trecho retornou a confirmação após a promessa da área de transferência. O botão de próximo exemplo mudou a palavra visível e os cartões permaneceram links para guias reais.

Os testes automatizados cobrem movimento reduzido, pausa, ocultação, desmontagem e recursos liberados; não devem ser confundidos com observação manual de todas essas situações. Não foi feita medição em hardware móvel de baixo desempenho, leitor de tela ou zoom de texto de 200% nesta rodada.

## Aplicativo e demonstração

Os componentes React reais foram observados no ensaio local com catálogo e conta sintéticos. A lista de habilidades apresenta nome, descrição e origem; setas/Enter selecionam uma habilidade e devolvem o foco ao compositor. Busca sem resultado manteve o texto e não selecionou uma habilidade. A resposta preparada chegou em trechos e terminou com o conteúdo esperado, inclusive com movimento reduzido selecionado.

A demonstração embutida do site foi recompilada com o frontend novo. Recolher e reabrir a lista de projetos preservou exatamente o rascunho digitado. Digitar `/` abriu o novo seletor dentro do iframe. Isso valida o frontend; não prova que o executável instalado recebeu uma atualização.

A revisão separada encontrou e corrigiu dois problemas antes da integração: captura indevida de Shift+Enter/IME no seletor e apresentação de eventos antigos como etapas atuais. O histórico agora é identificado como histórico; atalhos com modificadores e composição de texto são preservados. O teste de URL longa durante streaming também levou a ajustar a quebra de palavras.

Build final do app aprovado, 84 testes unitários em 20 arquivos, suíte de 38 E2E aprovada e dois casos focados repetidos após as correções da revisão. A suíte do site aprovou 34 testes de componentes, autenticação, downloads, links e isolamento. Nenhuma dessas aprovações substitui o teste pendente do instalador real.

Os 19 E2E da demonstração também passaram após corrigir sua disponibilidade fictícia de IA. A fixture continua em memória, com rede e serviços reais bloqueados. A demonstração é identificada como prévia; estas alterações de frontend ainda precisam de um novo pacote desktop, sem substituir os artefatos já publicados de 1.1.1.

## Escopo da distribuição

Troca de plano e copiar foram integrados aos controles existentes. Palavras, onda tipográfica e cartões pertencem à apresentação do site. Recolhimento, resposta progressiva, atividade e aprovação pertencem aos fluxos do aplicativo. OTP, tags e cartão são componentes preparados, sem ativação de autenticação ou pagamento novos.

Não houve chamada paga à IA nesta rodada. O teste do atualizador é registrado separadamente em `desktop-updater-network-2026-11-14.md` e permanece pendente enquanto o CDN oficial não responder.
