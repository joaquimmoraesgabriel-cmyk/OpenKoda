# OpenKoda 1.0 — interface e demonstração

Data: 14 de novembro de 2026. Esta entrega continua em pré-lançamento: a numeração 1.0 não ativa cobrança ou promete disponibilidade comercial.

## Aplicativo Windows

- Navegação compacta por ícones, com nomes no foco e no hover.
- Lista de projetos e conversas redimensionável por arraste ou teclado; largura lembrada neste computador.
- Indicador de uso acima de Configurações. O resumo abre por hover/foco e o clique leva ao Histórico de uso.
- Resumo por provedor e modelo, grade de 21 dias (3 × 7), botões roláveis com transição suave nas bordas.
- O anel usa a cota efetiva da assinatura quando a conexão é Koda. Para chave própria, usa o orçamento local de todos os provedores, nunca presume o saldo da conta externa.
- Aviso vermelho e borda animada no compositor quando resta no máximo 15%; preferência de movimento reduzido respeitada.
- Assinatura ativa permite raciocínio com chave própria em modelos compatíveis das conexões OpenRouter e OpenAI. Automático continua disponível. A elegibilidade é conferida no host antes da execução e novamente por lote. O consumo continua vinculado à chave própria.
- As páginas solicitadas anteriormente já estão disponíveis: Perfil/conta/plano, Histórico de uso, Relatar um erro, Diagnóstico e suporte, Dados e privacidade, Aparência e demais preferências.

## Site

- Demonstração interativa sob o título da página inicial, dentro de um card com rolagem própria.
- Projetos, conversas, relatório e configurações fictícios. Caminhos e URLs de projeto são exemplos; nenhuma URL digitada é consultada. Respostas são preparadas, progressivas, canceláveis e explicitamente demonstrativas. Não há chamadas de IA, cadastro ou armazenamento persistente na demonstração.
- Guias ampliados com capturas do aplicativo, legendas, passos e pequenos controles interativos.
- Texto explicativo maior, revelação suave ao rolar e formação gradual de títulos curtos. Conteúdo acessível por teclado, sem JavaScript e com movimento reduzido.
- Download por rota do próprio site, com transferência em fluxo e suporte a retomada. O link direto do GitHub continua como alternativa. Não há falsa confirmação de download concluído.
- Versões anteriores preservadas no GitHub. Nenhuma remoção de arquivos pessoais ou releases antigos.

## Validação

- Build TypeScript/Vite: aprovado.
- 84 testes unitários de frontend: aprovados.
- 120 testes de interface desktop: aprovados. Testes focados adicionais cobrem orçamento, grade, celular, zoom, raciocínio com um único nível, renovação semanal, retomada após suspensão e falhas sem repetição automática.
- 668 testes nativos Rust: aprovados, 3 ignorados pelo projeto. Dois testes de raciocínio repetidos após ampliar as verificações: aprovados.
- 19 testes de interface do site: aprovados. Regressão adicional de teclado mobile da demonstração: aprovada.
- 11 testes Node de links, sessão, CAPTCHA e transferência do instalador: aprovados.
- As verificações locais usam fixtures/simulações. Não comprovam nova chamada paga de raciocínio com assinatura real.

## Custos e próximo trabalho

US$ 0 de chamadas de IA nesta entrega. Nenhum pagamento, domínio, alteração de banco ou configuração Asaas foi executado.

Próximas etapas, mantidas fora deste pedido: domínio e remetente próprio de e-mail; finalizar identificação legal e condições comerciais; ativar e testar pagamentos Asaas em sandbox; validar raciocínio BYOK ponta a ponta com assinatura real e chamadas pequenas antes de abrir vendas.
