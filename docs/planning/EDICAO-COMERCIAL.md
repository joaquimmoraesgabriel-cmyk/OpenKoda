# Fases da edição comercial do OpenKoda

O aplicativo gratuito continua funcionando com a credencial do usuário. Uma conta comercial concede acesso a serviços hospedados. O acesso ao computador, aos arquivos e à execução local continua dependendo do escopo autorizado naquela instalação.

## 1. Verificação da execução existente

Usar fixtures sintéticas e perfis descartáveis para verificar um fluxo que atravessa arquivos e lotes, uma imagem com informação ausente do prompt e a interrupção depois do primeiro texto real. Conferir fonte e trecho, histórico após reabertura, unicidade da solicitação e cobrança do provedor. Uma interrupção pode gerar cobrança mesmo sem mensagem final. Reexecutar as regressões afetadas antes de prosseguir.

## 2. Conta, privacidade e admissão comercial

Validar localmente a migração e a separação entre dois usuários antes de aplicá-la ao projeto Supabase existente. Incluir cadastro, confirmação, login Google, recuperação, saída, exportação, exclusão e registro versionado da aceitação. O consentimento de marketing é opcional e revogável. O suporte precisa de acesso restrito aos chamados e de um canal que permita ao usuário acompanhar a resposta. Confirmar entregabilidade de e-mail e callbacks OAuth com contas de teste.

## 3. Assinaturas, modelos e proteção de gastos

Para o checkout de teste no Brasil, os preços mensais propostos são R$ 219,00 (Plus), R$ 999,00 (Pro) e R$ 1.999,00 (Ultra). A tabela usa como referência o câmbio de venda de R$ 5,6518 por dólar em 13/11/2026. [Banco Central](https://ptax.bcb.gov.br/ptax_internet/consultarUltimaCotacaoDolar.do)

| Plano | Preço mensal em reais | Cota mensal em dólares | Receita menos inferência em reais |
| --- | ---: | ---: | ---: |
| Plus | 219 | 20 | 105,96 |
| Pro | 999 | 100 | 433,82 |
| Ultra | 1.999 | 200 | 868,64 |

A diferença financia processamento de pagamento, tributos, câmbio, infraestrutura, suporte e reembolsos. Ainda é necessário medir essas despesas para estimar a margem líquida. Antes de abrir vendas no Brasil, apresentar o preço final em reais, periodicidade, data de renovação, regras de cancelamento e reembolso. O acesso antecipado do Pro e as versões experimentais do Ultra dependem da existência de recursos disponíveis.

Identificação de pré-lançamento fornecida pelo responsável: marca Koda, Brasil, contato joaquimmoraesgabriel@gmail.com. A razão social, o CNPJ e o endereço comercial ainda não foram constituídos ou informados. Conferir a ocupação permitida e o enquadramento com um profissional antes de registrar a atividade e receber assinaturas.

O servidor deve aplicar simultaneamente o teto do ciclo e a cota de sete dias. Para evitar que o último dia de um ciclo consuma outra semana inteira, repartir o teto pelo número de dias do ciclo. Cada janela recebe a proporção dos dias que ocupa nesse ciclo. Exibir saldo, próxima liberação e data final na conta. Administradores não devem poder atribuir um plano por meio de metadados graváveis pelo próprio usuário.

Antes de enviar uma chamada paga, reservar uma estimativa conservadora em transação com chave de idempotência, teto da plataforma e limite de simultaneidade. Concluir pela geração do provedor e manter a reserva em caso de resultado desconhecido. Retomar uma operação interrompida não pode cobrar a mesma etapa duas vezes. O servidor seleciona os modelos, preços, limites de raciocínio e tokens de saída, incluindo restrições de multimodalidade e tratamento de dados. Cada modelo disponível precisa ter preço e capacidades verificáveis. Avisar quando um modelo não aceitar o nível solicitado.

O checkout desta etapa funciona em simulação identificada. Uma simulação não concede crédito de produção. A integração externa com Asaas sandbox e a ativação de cobrança real serão feitas em etapas posteriores, a pedido do responsável.

## 4. Desktop e skills

Mostrar plano e cota no perfil e no histórico, com distinção entre gasto da assinatura e uso de chave própria. Oferecer controle de raciocínio no seletor de modelos de projetos e conversas elegíveis. Carregar o catálogo de skills da nuvem depois de verificar a assinatura. Preservar licença, revisão, descrição, recursos relativos e modalidade de invocação. A presença de uma skill não concede acesso a comandos, segredos, arquivos adicionais ou publicação externa.

Fazer compactação em um ponto seguro entre etapas do trabalho. Manter o objetivo original, fatos confirmados, escopo e revogações, referências de evidência e ações pendentes. Salvar o resumo e o cursor atomicamente antes de continuar. Se falhar, preservar o histórico e informar o usuário. Testar interrupção, reabertura e reenvio.

## 5. Site e lançamento

Publicar apresentação, demonstração identificada do aplicativo, download verificável, funcionalidades, preços, suporte e páginas contratuais. Usar o nome OpenKoda para o serviço e identificar o intermediário e os fornecedores de inferência nas informações de privacidade. Documentar retenção e exclusão efetivamente configuradas, inclusive backups de terceiros. Conferir as obrigações aplicáveis à jurisdição, verificar acordos de tratamento e impedir verificações de cartão ou cobranças no ambiente demonstrativo.

Antes da abertura ao público, verificar acessibilidade, licenças, origem dos arquivos, consentimentos, proteção contra abuso, faturamento, concorrência e restauração. Registrar a distinção entre controles comprovados, configuração externa pendente e condições comerciais propostas.
