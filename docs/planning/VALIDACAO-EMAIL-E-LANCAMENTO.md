# Prontidão do OpenKoda para lançamento

O download de pré-lançamento para Windows e a abertura geral de assinaturas são etapas diferentes. O fluxo de pagamento público ainda é uma simulação.

| Verificação dos e-mails | Resultado observado |
| --- | --- |
| Suporte no serviço publicado | O responsável abriu um chamado pelo site. O banco registrou uma ocorrência às 23:14:21 de 13/11/2026 (Brasília). Às 23:14:22, o Gmail do suporte recebeu um aviso do Resend na caixa de entrada. SPF, DKIM e DMARC passaram. O link HTTPS leva ao editor do projeto Koda no Supabase e exige acesso de operador. A resposta ao cliente fica no chamado. |
| Cadastro com senha | O Supabase Auth (GoTrue) enviou a mensagem em português por SMTP para uma caixa local. O link confirmou a conta descartável e não funcionou uma segunda vez. O ensaio não alterou a conta Google do responsável. |
| Recuperação | A mesma pilha enviou o segundo e-mail. O retorno exigiu o navegador que iniciou o pedido, a nova senha permitiu entrar e a anterior foi recusada. O código e o link usados não puderam ser reaproveitados. |
| Primeira tentativa externa de cadastro | O Gmail recebeu a mensagem do remetente de teste do Resend na pasta Spam; SPF, DKIM e DMARC passaram. O processo descartável encerrou antes do resgate. Essa tentativa comprova entrega na internet, mas não comprova o retorno do link. Os segredos da função de suporte não configuram o SMTP de autenticação. |
| Cadastro recebido no Gmail, segundo ensaio | Às 23:42:15, a confirmação chegou em Spam. O link extraído da mensagem recebida coincidiu com o emitido pelo GoTrue descartável. Seu resgate confirmou a conta; uma segunda tentativa foi recusada. |
| Recuperação recebida no Gmail | Às 23:43:10, a recuperação chegou à caixa de entrada. O link também coincidiu e só funcionou uma vez. A senha anterior foi recusada e a nova permitiu entrar. SPF, DKIM e DMARC passaram nas duas mensagens. |

Os dois links externos apontavam para o ambiente descartável no computador do ensaio. A conta pessoal no projeto hospedado não teve senha alterada. A entrega pelo Gmail e o resgate dos links estão confirmados para esse ensaio. A configuração do SMTP de autenticação e dos modelos no Supabase hospedado, com o domínio de produção, ainda exige homologação pelo site público. O ensaio durável está em [supabase/tests/gmail](../../supabase/tests/gmail/README.md).

## Antes de abrir o cadastro por e-mail ao público

1. Verificar no provedor um domínio que a Koda controle. O remetente de teste do Resend atende somente o endereço cadastrado da própria conta; ele serve ao aviso de suporte do pré-lançamento.
2. Configurar no Supabase Auth o remetente, o SMTP transacional e os modelos de cadastro e recuperação. Manter a confirmação de e-mail, o CAPTCHA e a lista de retornos autorizados. Desativar o rastreamento de cliques nos links de autenticação.
3. Fazer cadastro e recuperação no site publicado com uma identidade descartável, em provedores externos. Conferir chegada, remetente, retorno HTTPS, expiração, tentativa de reutilização e troca efetiva da senha. Medir rejeições e atrasos.

## Antes de abrir vendas

- Homologar o pagamento no Asaas e o ciclo de ativação, renovação, cancelamento e estorno. Depois habilitar inferência paga com catálogo permitido, orçamento definido e alerta testado. A fila e o débito único já têm um [ensaio hospedado](./ENSAIO-FILA-COTA-HOSPEDADA.md).
- Concluir a identificação comercial, o endereço e os acordos aplicáveis com fornecedores. Validar prazos de retenção, recuperação de backup e atendimento de solicitações sobre dados.
- Assinar o instalador para distribuição comercial e testar a versão assinada num Windows limpo. O instalador de pré-lançamento tem hash verificável, mas não assinatura comercial.

As evidências e pendências de custos, isolamento e operação estão no [plano da edição comercial](./EDICAO-COMERCIAL.md).
