# Fases da edição comercial do OpenKoda

O aplicativo gratuito continua funcionando com a credencial do usuário. Uma conta comercial concede acesso a serviços hospedados. O acesso ao computador, aos arquivos e à execução local continua dependendo do escopo autorizado naquela instalação.

Resultado mais recente dos e-mails e bloqueios para a abertura pública: [validação de e-mail e lançamento](./VALIDACAO-EMAIL-E-LANCAMENTO.md).

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

## Resultados do pré-lançamento de 13 de novembro de 2026

| Ensaio no desktop | Evidência observada |
| --- | --- |
| Gemini 2.5 Flash-Lite em dois arquivos e dois lotes | Quatro requisições completaram a revisão. O relatório ligou a entrada HTTP em `api.py:6` à execução em `math_worker.py:2`. O teste também encontrou um erro no formato intermediário do relatório. A repetição passou após a correção. |
| Imagem com Gemini 2.5 Flash-Lite | O modelo leu seis dígitos sorteados que apareciam somente no PNG e o histórico registrou o anexo. O recibo informou US$ 0,00010920. |
| Cancelamento de Ministral durante a resposta | O aplicativo parou após 55 caracteres e preservou uma única requisição sem mensagem final. O provedor faturou US$ 0,00009009 e marcou a geração como `cancelled: false`. O encerramento remoto não foi comprovado. |

O aumento medido no consumo da chave foi de **US$ 0,001709520** nessa bateria, incluindo a tentativa que revelou o erro. Os totais arredondados do histórico local servem para consulta. A diferença no consumo da chave foi usada para o total da bateria.

Um ensaio do serviço comercial executou seu código real em laboratório com autenticação e operações de banco simuladas. O Ministral entregou nove fragmentos no chat e encontrou o `eval` em `calculator.py:2` na análise. Repetir o identificador da requisição não repetiu o envio. Os dois recibos e o consumo total da chave concordaram em **US$ 0,000057150** adicionais.

No ensaio seguinte, o Supabase Auth (GoTrue) local criou e autenticou uma conta de teste. O PostgreSQL 17 executou a reserva, o despacho e a liquidação reais por meio de um adaptador SQL parametrizado. Duas solicitações simultâneas com a mesma chave geraram respostas HTTP 200 e 409. A repetição posterior recebeu 409. O único recibo do OpenRouter registrou 41 tokens de entrada, 5 de saída e **US$ 0,000006900**, valor confirmado pela variação no consumo da chave. O gasto total desta sessão foi **US$ 0,001773570**. A inferência paga hospedada permaneceu desligada.

No Supabase Koda, duas contas descartáveis confirmadas entraram pela API de autenticação. Acesso anônimo e token forjado receberam HTTP 401. O servidor aceitou a origem do site, recusou uma origem alheia e restringiu os chamados e a exportação ao titular. As contas foram excluídas após os ensaios. A simulação de checkout não habilitou inferência. A autenticação com Google chegou à tela oficial de seleção, com a URL do projeto como retorno. O responsável confirmou o acesso completo em sua conta. O Turnstile apresentou sucesso no cadastro e na recuperação, e um token inválido recebeu `captcha_failed` do servidor. A entrega e o resgate de links de confirmação e recuperação por um usuário externo ainda precisam de teste.

As migrações cobrem contas, catálogo, permissões, limites, alertas, checkout e consentimento inicial. Dois logins simultâneos preservaram o primeiro aceite. Um novo login após o descadastro preservou a opção de marketing. Apenas `public` está exposto pela Data API. As funções privilegiadas ali exigem o papel de serviço. As tabelas de negócio ficam no esquema privado.

O banco registra a passagem por 80% e 95% dos tetos e picos fora do padrão recente. O operador pode consultá-los em `openkoda_budget_health`. Para receber os avisos por e-mail, configurar o remetente verificado, a chave do Resend e `KODA_BUDGET_EMAIL_ENABLED=true`. Uma falha temporária deixa o alerta disponível para nova tentativa quando houver atividade de inferência. O teste de reenvio preservou a chave de idempotência e não gerou outra cobrança de IA.

O painel de suporte recebe os chamados. Os avisos por e-mail requerem um remetente verificado em `KODA_SUPPORT_FROM` e `RESEND_API_KEY` nos segredos do servidor. A resposta do operador fica no chamado do titular. As opções de marketing começam desmarcadas e o descadastro pode ser feito na conta ou por e-mail. Antes de enviar campanhas, ligar a lista de destinatários ao consentimento vigente e configurar o descadastro em cada mensagem.

A operação paga depende de credenciais próprias de produção, tetos definidos pelo operador, homologação de cancelamento e estorno no Asaas, SMTP, recuperação de conta e restauração de backup. O checkout de homologação aceita exclusivamente o ambiente sandbox e não libera crédito de produção. O catálogo comercial exige uma rota compatível com o preço, os recursos e as restrições de uso de dados. Modelos elegíveis variam com essas condições.

Para o lançamento brasileiro, preencher a identificação jurídica e o endereço de atendimento após a formalização. Verificar se as atividades efetivamente exercidas se enquadram na lista de ocupações permitidas antes de escolher MEI. [Portal do Empreendedor](https://www.gov.br/empresas-e-negocios/pt-br/empreendedor/quero-ser-mei/atividades-permitidas)

Registrar os acordos de tratamento efetivamente aceitos com os fornecedores, suas regiões, prazos de backup e responsáveis. Se houver interesse em invocar as proteções da seção 512 nos EUA, avaliar se o serviço é elegível e, quando aplicável, designar e manter atualizado o agente. O pré-lançamento fornece um contato de direitos autorais. [U.S. Copyright Office](https://www.copyright.gov/dmca-directory/faq.html)

### Entrega verificada

O instalador x64 0.2.3 publicado tem 181.712.345 bytes e SHA-256 `5e07db838fee737c683204aabd70c524101656c644abfe076bb58bdc7a4df35b`. O digest do GitHub coincide com o do arquivo extraído do artefato de CI. Os testes de instalação e abertura passaram em Windows 11 Arm64 e Windows Server 2022 e 2025. [Versão e instalador](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/releases/tag/desktop-v0.2.3)

A bateria local completou 662 testes nativos (3 ignorados), 79 testes unitários de interface e 110 cenários de interface do desktop. Os três cenários de conta no navegador, os seis testes do servidor do site, os 17 testes da função de conta e IA e os dez ensaios offline do Asaas também passaram.

Na versão 5 do site publicado, o servidor enviou o cookie de autorização com os atributos HttpOnly, Secure e SameSite. Uma requisição de outra origem recebeu 403. Um retorno sem cookie recebeu 409 e o mesmo retorno sintético, associado ao cookie, chegou à verificação do Supabase e recebeu 404 (código de autenticação desconhecido). O instalador e o painel de demonstração estão disponíveis no site. [OpenKoda](https://openkoda-desktop.steelyrowan1.chatgpt.site)

### Reavaliação do pedido de lançamento

O instalador x64 0.2.4 do commit `07f3f3980a46604d0ffe92b2eef83f92342f8d3e` passou pelos ensaios de instalação, abertura e desinstalação em Windows Server 2022 e 2025 e em Windows 11 Arm64. O arquivo tem 181.839.038 bytes e SHA-256 `5c31eab76ff591254c7616d5b34969d12dc10e0a201216aef3fa54bd75f88414`. A suíte completa de navegador passou nos 121 cenários depois do ajuste de animação. [Windows Server](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/actions/runs/43530646259) · [Windows 11](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/actions/runs/43530646078)

A versão 7 do site está publicada com 20 páginas, seis guias, demonstração de relatório, chat e projeto, perguntas frequentes e controles de movimento que respeitam a preferência do navegador. A verificação do endereço público percorreu as 20 páginas, conferiu o destino do instalador 0.2.4 e confirmou a recusa de duas tentativas de iniciar o Google com o CAPTCHA ausente ou inválido. [Guias do aplicativo](https://openkoda-desktop.steelyrowan1.chatgpt.site/guias/) · [Download](https://openkoda-desktop.steelyrowan1.chatgpt.site/download/)

Na versão 0.2.4, quatro requisições adicionais no aplicativo Windows custaram **US$ 0,000590020**, verificados pelos recibos de geração e pela diferença no consumo da chave. Duas respostas comuns terminaram naturalmente. Na terceira modalidade, o modelo parou exatamente nos 32 tokens autorizados para o ensaio. O histórico preservou o texto parcial ao reabrir, e preparar a continuação manteve a contagem em uma única requisição. No cancelamento, o usuário já via 335 caracteres. A geração teve cancelamento confirmado pelo provedor e gerou custo de US$ 0,000155600. Somando os ensaios anteriores de relatório, imagem e execução da assinatura, o total medido foi **US$ 0,002363590**.

Com 30 contas de assinatura concorrendo por quatro vagas em PostgreSQL real, quatro solicitações foram admitidas e 26 ficaram na fila. A fila conserva uma posição por conta, tem 64 posições no total e tempo máximo de dois minutos. O aplicativo mostra a posição e pode cancelar a espera. A admissão revalida os limites e as cotas antes de reservar o custo. O teste de carga usou um provedor simulado e não gerou trinta cobranças.

A migração `20261114002437` e a versão 4 da função `openkoda` foram publicadas no projeto Koda. O servidor tem dez vagas de execução e permite uma execução por conta. A tabela de espera está protegida por RLS. Somente o papel de serviço pode executar a autorização de cancelamento no banco. A conta autenticada pede o cancelamento pela função hospedada. Os controles de inferência e os orçamentos de produção permanecem desligados até a homologação comercial. Passaram os 18 testes locais da função e os dez testes locais do Asaas em simulação.

A compilação do desktop passou, assim como os 79 testes unitários de interface e 666 testes nativos. Três verificações opcionais ficaram fora da execução nativa padrão (rede real do Windows, importação pública de uma fixture do GitHub e execução no Kali gerenciado). `npm audit` não encontrou avisos nas dependências de desenvolvimento e de produção do desktop. Uma busca por formatos conhecidos de chaves dos serviços integrados em 1.642 objetos de arquivo alcançáveis pelo histórico Git não encontrou correspondências. Esses controles têm o escopo das dependências, referências e formatos examinados.

| Área | Implementado ou verificado | Pendente para a operação pública paga |
| --- | --- | --- |
| Conta e sessão | Supabase Auth, Google, recuperação, Turnstile, cookies de sessão protegidos e consentimentos separados. Após a correção do endereço de retorno do Google, o responsável entrou no site pelo celular e confirmou que permaneceu na conta ao atualizar a página. Os seis cenários de navegador passaram dez vezes cada. O servidor do site recusou iniciar o Google com token de CAPTCHA falso. | Cadastro e recuperação foram recebidos e resgatados no Gmail com autenticação descartável e remetente de teste. Falta homologar pelo site hospedado com SMTP e domínio de produção e verificar a classificação de spam. |
| Revisão e isolamento | Seleção explícita de arquivos, revisão estática de cópias de texto e isolamento entre contas. O teste percorreu arquivos e lotes, abriu o relatório salvo e verificou um anexo visual. | Ampliar a cobertura de projetos e de modelos usados no campo. A confirmação remota de cancelamento depende da rota do fornecedor. |
| Assinatura e uso | Uma conta Plus de teste passou pela fila hospedada, produziu uma resposta real e teve um único débito nas cotas semanal e mensal. O servidor recusou o reenvio já concluído. A simulação de checkout, os limites, a reserva e os alertas também passaram pelos ensaios locais. [Recibo do teste hospedado](./ENSAIO-FILA-COTA-HOSPEDADA.md) | Homologar o Asaas, verificar que um checkout pago ativa a assinatura correta e, depois, habilitar a cobrança e a inferência comercial. O alerta por e-mail requer um remetente transacional. |
| Operação e dados | Registro privado de chamados, direitos de acesso, exportação e exclusão, políticas de retenção, descadastro e restrições de coleta. O pré-lançamento só aceita contas de adultos. | Verificar o domínio remetente, entregabilidade, temporização dos logs e backups dos fornecedores, restauração e resposta a incidentes. Campanhas futuras precisam respeitar o descadastro em cada mensagem. |
| Segurança e distribuição | Limites de formatos e tamanhos no envio, imagem limitada em dimensões, isolamento da revisão, controle de acesso e rotas protegidas. O serviço recebe anexos na requisição e dispensa armazenamento público e URLs de download de anexos. O site distribui o instalador por HTTPS e publica o hash. | Assinar o executável para distribuição comercial, revisar dependências e licenças a cada versão e validar limites da infraestrutura hospedada sob a carga real prevista. |
| Publicação e identificação | Demonstração, preço e periodicidade, informação sobre renovação, privacidade, termos, reembolso, suboperadores e canal de contato do responsável. | Preencher a identificação comercial e o endereço, registrar os acordos de tratamento aplicáveis e concluir as verificações de retenção, disponibilidade e direitos autorais antes das vendas. |
