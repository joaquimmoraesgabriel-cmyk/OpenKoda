# Interações do OpenKoda 0.1.18

## Revisão do compositor na 0.1.19

Em 12/11/2026, a revisão usou as duas novas capturas fornecidas do Codex e uma observação externa da janela aberta. O texto ocupa a primeira área do compositor. O botão **+** e a aprovação ficam à esquerda no rodapé, com o modelo e o envio à direita. Os menus aparecem acima do controle e deixam a conversa visível. No OpenKoda, Enter envia por padrão e Shift + Enter insere uma linha. A preferência explícita por Ctrl + Enter continua disponível.

O servidor MCP local Craft Anything 0.6.0 comparou as capturas fornecidas, registrando seus hashes e a área comum em `ev_d48cbcfdb71c9ca08eacf9aa33a88b50134720d37b604f722762f41ec5795d77`. Essa comparação identifica as imagens de referência. A leitura de acessibilidade ao vivo ficou sem resposta e seu processo foi encerrado. A organização dos menus foi obtida da observação visual. [Evidência da comparação](codex-composer-evidence.json)

O OpenKoda reúne visão geral e configurações em **Projeto**, com abas e opção de manter o chat ao lado. Conversas, execuções, achados e uso de IA usam tabelas com busca, ordenação e paginação. Os filtros de estado e severidade pertencem à tabela de achados. Esses controles operam sobre os registros do projeto.

## Referências da 0.1.18

[Mapa das interfaces de referência](referencias.svg) · [Mapa aplicado ao OpenKoda](openkoda.svg)

Os mapas têm fontes Mermaid e documentos editáveis do OpenFlowKit nesta pasta. As observações abaixo descrevem controles visíveis no Windows e nas capturas de referência fornecidas para o projeto.

| Superfície observada | Comportamento visível | Aplicação no OpenKoda |
| --- | --- | --- |
| Conversas da Cindy e do Codex | Projetos agrupam conversas. A linha atual tem seleção persistente e a ação de criar conversa aparece junto ao projeto. | Árvore recolhível, seleção com marcador, ação acessível por foco, título do projeto e conversa atual no cabeçalho. |
| Chat da Cindy | Mensagem enviada alinhada à direita, resposta com parágrafos e listas, anexos pequenos. A volta às mensagens recentes aparece acima do campo. | Separar leitura, eventos de execução e composição. Permitir consultar o histórico durante uma resposta. |
| Compositor da Cindy | Texto na primeira linha, anexos e aprovação de um lado, modelo e envio do outro. Os menus abrem junto ao controle. | Compositor compacto, modelos habilitados por provedor, pesquisa e retorno ao campo de trabalho. |
| Codex em atividade | A árvore de acessibilidade expõe novo chat, nova conversa por projeto, modo de aprovação e Parar durante o trabalho. | Estados reais da execução, cancelamento e prévia dos dados quando a aprovação é necessária. |
| Geral, Aparência e Personalização da Cindy | Coluna de leitura limitada, títulos distinguem grupos, descrição próxima ao controle. A categoria ativa tem tratamento próprio. | Ritmo comum para linhas e seções, foco visível e conteúdo com rolagem independente. |
| Provedores, Uso e cobrança, Histórico de uso | Lista e detalhe dos provedores lado a lado. Filtros de período precedem totais e gráficos. Carga e disponibilidade são indicadas no grupo correspondente. | Organizar preferências de IA, orçamento, histórico e métricas existentes. |
| Entrada de voz, Bots de mensagens, Atalhos de teclado, Importação de sessões | O menu mantém posição entre categorias. A importação reserva espaço para linhas enquanto carrega. | Manter navegação estável e aproximar ação, estado e descrição no conteúdo. |
| Conexão remota, Compartilhar tarefa, Plugins, Ferramentas e Automação | Compartilhamento tem abas. Plugins distingue itens instalados do catálogo. Ferramentas identifica o projeto antes da lista. | Projeto identificado no painel, controles de inventário ao lado da ferramenta e área de detalhe apartada dos filtros. |
| Armazenamento, Ajuda e Sobre | A rolagem própria da navegação dá acesso às últimas categorias. | Painéis e categorias acessíveis em janelas mais baixas e com escala ampliada. |
| Perfil e uso nas capturas do Codex | Identidade, indicadores e atividade ocupam blocos de leitura distintos. As legendas pertencem aos respectivos gráficos. | Agrupar dados do perfil e métricas de uso locais, adaptar as colunas à largura disponível. |

Na janela da Cindy de 1536 × 816 px, a navegação ocupava cerca de 260 px. O compositor tinha cerca de 85 px de altura e o menu de modelo abria acima dele, com cerca de 460 × 510 px. São medidas aproximadas de uma janela. No OpenKoda, o espaço disponível e a escala do usuário determinam o tamanho final. As transições de 120 e 160 ms são escolhas desta versão.

As 18 categorias de Configurações da Cindy foram abertas. A comparação usou o chat já existente, os menus e a navegação. Envio de mensagens, compra, alteração de contas e fluxos autenticados internos das referências ficaram fora da observação ao vivo. No OpenKoda, o progresso apresentado na conversa corresponde aos eventos produzidos pela análise.

## Evidência visual

O servidor MCP Craft Anything 0.6.0 comparou a área comum das capturas com tolerância de 16 por canal. No Codex, ao rolar Uso e faturamento, a primeira coluna de pixels alterados ficou em x=709. A navegação à esquerda permaneceu idêntica dentro dessa tolerância. Na Cindy, preencher a chave de API alterou 0,66% da área comum, no retângulo que abrange o campo e o botão Próximo. Isso orientou a manutenção dos controles em posição estável quando o estado muda.

| Referência | Capturas de 11/11/2026 | Identificador da observação |
| --- | --- | --- |
| Codex | 14:43:01 e 14:43:17 | `ev_bbd1029eb23e0ebac7767380d7483737b007554e2f6145e674c8435b428af598` |
| Cindy | 17:20:09 e 17:20:24 | `ev_cb4e10f6b49065a56c5eff8124526d87eff1b7a64953148bf49d43fc831085c4` |
