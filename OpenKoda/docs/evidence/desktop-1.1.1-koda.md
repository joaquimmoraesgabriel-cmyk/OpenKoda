# OpenKoda 1.1.1 — presença e acesso à Koda

Data: 14 de novembro de 2026.

## Correção

Koda é uma conexão gerenciada, identificada internamente por `openkoda`.
Sua presença na interface não depende da permissão de inferência paga.
Ela aparece em IA e modelos, Adicionar provedor, seletor do chat e criação
de projeto. Não há cadastro, teste ou remoção de chave pessoal nessa entrada.

O estado distingue entrada na conta, plano gratuito, simulação interna,
assinatura com inferência habilitada, serviço indisponível e conta offline.
Contas autenticadas podem consultar o catálogo; modelos ficam bloqueados
quando não há permissão de uso. A simulação não concede saldo financeiro.
Os controles financeiros e de autorização do servidor não foram alterados.

Os botões abrem Conta e segurança ou Plano e assinatura dentro do aplicativo.
O projeto continua montado e os rascunhos do chat e da criação são preservados.
O nome visível do provedor é Koda; o aplicativo continua OpenKoda.

## Catálogo e descrição dos planos

Consulta pública gratuita em 2026-11-14, 17:27 UTC: 482 entradas, das quais
60 modelos distintos elegíveis sob a política vigente de modalidades, tarifas
e capacidades. O número varia e não é uma promessa contratual. São modelos,
não 482 provedores. Aumentar apenas o teto de preços não torna todas as entradas
compatíveis. Por isso, o texto dos planos informa o acesso ao catálogo disponível
na Koda, sem prometer mais de 200 opções utilizáveis.

## Verificação da correção

- TypeScript sem erros; 84 testes unitários aprovados.
- 12 novos cenários de interface cobrem os seis estados, presença da Koda,
  bloqueio de seleção paga, ausência de operações BYOK e navegação com rascunhos.
- 32 cenários de interface afetados têm aprovação, incluindo entrada/sessão e
  raciocínio. A primeira execução teve quatro timeouts de inicialização do Vite;
  a segunda teve dois. Os testes correspondentes passaram na repetição focal
  com maior prazo de ação, sem alteração das verificações funcionais.
- Capturas sintéticas de gratuito e simulação, nas configurações e no seletor,
  revisadas visualmente. Não contêm dados de contas reais.
- Revisão estática independente sem falha concreta identificada no patch.
- Site: compilação e 22 testes aprovados, incluindo sessão, navegação e download.

Nenhuma chamada paga de IA foi necessária nesta correção: US$ 0.
A evidência de instalação e atualização será registrada após o ensaio do pacote
assinado 1.1.1; os testes de renderer não comprovam esse fluxo nativo.
