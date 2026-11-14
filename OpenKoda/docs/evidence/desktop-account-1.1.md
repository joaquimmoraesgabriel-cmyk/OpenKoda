# Conta desktop e simulação interna — 1.1

Validação local em 14 de novembro de 2026. A publicação e a aceitação no instalador são verificadas separadamente pela tarefa de entrega.

## Causas e mudança de comportamento

O retorno nativo do Google tentava primeiro resgatar o código no Worker do site, que possui seus próprios verificadores PKCE. Só um erro específico fazia o código seguir para o listener do desktop. Uma indisponibilidade ou outra recusa no caminho web impedia a entrega nativa. A regressão reproduziu essa falha com um `503` no exchange web antes da correção.

Agora o retorno identifica a tentativa nativa vigente antes de qualquer exchange web. A página oferece **Abrir aplicativo**, que envia somente o código de autorização e o state ao listener `127.0.0.1` da tentativa. O host possui o verificador PKCE, troca o código com o Auth e grava a sessão no cofre DPAPI. O site remove o código da barra de endereço e consome sua referência à tentativa. O app ganha foco após uma autenticação concluída.

O antigo checkout apenas registrava uma intenção em `checkouts`; ele não provisionava assinatura nem permissões. A simulação agora possui estado persistido próprio, uma lista administrativa de testadores com validade e um interruptor desativado por padrão. Ela concede skills e ajuste de raciocínio com chave pessoal enquanto estiver ativa. As tabelas financeiras, a admissão de inferência gerenciada e a cota de produção permanecem independentes.

Skills explícitas também podem compor conversas com chave pessoal. A revisão vincula separadamente a credencial do provedor e a sessão comercial que concedeu a skill. O host consulta o acesso vigente antes de preparar, reservar e despachar; troca da conta, expiração ou retirada do acesso bloqueiam o pacote anterior. O identificador dessa sessão permanece no registro de autorização local e não é enviado ao provedor. A seleção automática permanece restrita ao provedor gerenciado.

## Contratos

- `account.status`: restaura/renova e verifica a identidade; retorna `signedOut`, `authenticated`, `offline` ou `expired`, usuário e conta coerentes na mesma operação nativa. Uma falha transitória preserva a credencial criptografada, mas não devolve permissões antigas. Uma rejeição definitiva remove a sessão, preservando projetos e conversas locais.
- `account.google`: mantém o fluxo PKCE e o listener temporário por até três minutos. `account.loginCancel` encerra a espera; um retorno posterior não cria sessão.
- `account.simulation`: recebe plano, identificador UUID idempotente e resultado desejado (`activate`, `cancel`, `expire`, `fail`). A identidade é obtida do bearer validado no Edge, nunca do payload. A simulação ativa dura até sete dias e só vale enquanto a autorização de teste também estiver vigente.
- `account.overview`: distingue plano financeiro, `simulation` e `entitlements`. O site e o app consultam o mesmo estado. `hostedInference` não é concedido pela simulação.

## Nota de segurança

Fronteiras: navegador externo → listener local → Auth fixo HTTPS → cofre privado do perfil; renderer → IPC nativo; Edge autenticado → RPCs exclusivas do serviço → tabelas privadas com RLS.

Ataques considerados: troca de código entre sessões web/nativa, callback com Host ou state incorreto, repetição/expiração, login cancelado, revogação confundida com queda de rede, mistura de contas, metadados de usuário usados como autoridade, ativação de plano por usuário não autorizado e cliques concorrentes. O listener exige rota/método/Host/state exatos e limites de tamanho; PKCE vincula o código ao host. Nome/avatar são apenas apresentação; avatar aceita somente os hosts Google enumerados no CSP. Tokens e verificadores nunca são retornados ao renderer ou registrados em logs.

Eventos locais de autenticação são gravados sem tokens, e-mail ou motivos internos expostos ao usuário, com limite de 500 eventos e limpeza após 30 dias. Operações de simulação são um histórico transacional idempotente, exportável pela própria conta; eventos com mais de 90 dias são removidos na próxima operação. A simulação não invoca OpenRouter ou Asaas. Desativar o interruptor ou vencer/remover a autorização impede novas concessões e invalida o acesso lógico de teste.

## Evidências locais

- `supabase/tests/commercial.py --local`: suíte PostgreSQL real e isolada passou, incluindo RLS, quotas, fila e a nova simulação. Doze solicitações simultâneas idênticas criaram um único evento; repetição, conflito, usuário não autorizado, interruptor desligado, falha simulada, cancelamento e expiração foram exercitados. Nenhuma assinatura financeira ou crédito de produção foi criado. O banco temporário é removido ao finalizar.
- `node --test supabase/functions/tests/openkoda.test.mjs`: 20 testes passaram, incluindo identidade derivada do bearer, rejeição de payload que tenta escolher outro usuário, estados inválidos e ausência de chamadas pagas.
- `node --test OpenKoda/site/tests/worker-session.test.mjs`: 7 testes passaram, cobrindo cookies HttpOnly, renovação, origem/CSRF, PKCE e falhas transitórias.
- `site-account.spec.ts`: os 20 cenários da execução completa passaram após o ajuste, incluindo retorno PKCE nativo, persistência após F5, site sincronizado com simulação, cancelamento de sessão, recuperação, consentimento, teclado e diferentes larguras. Regressões adicionais cobrem retorno consumido e expirado.
- Testes Rust em `commercial::tests` cobrem cofre DPAPI e reabertura, recuperação offline versus revogação, renovação única, troca de identidade recusada, metadados sem autoridade e listener real com state inválido, cancelamento, expiração e consumo único. A execução destes testes deve constar na verificação nativa da entrega.
- O teste Rust `personal_key_skill_is_reviewed_and_account_change_or_expiry_blocks_before_reservation` exercita o pacote de skill completo com uma chave pessoal fictícia e um provedor local falso. O acesso ativo inclui a skill no pacote revisado; troca da sessão ou perda do entitlement devem resultar em zero chamadas e zero reservas. A fixture de conta é compilada somente em testes.

## Limites da evidência

Os testes de OAuth usam respostas controladas do provedor para exercitar falhas sem autenticar uma conta pessoal. O login real Google no instalador requer a conta do usuário. O listener exige que o app permaneça aberto; fechar o app invalida a tentativa e exige começar novamente. Não foi registrado um protocolo que reabra o app fechado com tokens na URL. Asaas e pagamentos reais continuam fora desta etapa.

Nenhuma chamada paga de IA foi feita nesta frente.
