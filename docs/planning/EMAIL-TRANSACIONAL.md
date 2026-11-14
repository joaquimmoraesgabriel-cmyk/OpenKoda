# E-mails da conta e do suporte

O Supabase Auth envia a confirmação do cadastro e a recuperação de senha pelo SMTP configurado em Authentication → Emails → SMTP Settings. Os segredos da Edge Function são uma configuração separada. O formulário de suporte grava o chamado no banco e a função `openkoda` avisa o operador pelo Resend.

## Remetente público

1. Para habilitar o envio público pelo Resend, escolha um domínio que você administra. Adicione o domínio ou subdomínio em **Domains** no Resend, copie para o DNS os registros indicados e aguarde o status **Verified**. O subdomínio de hospedagem do site só serve para envio quando o operador também controla os registros DNS exigidos.
2. Crie uma chave do Resend com permissão de envio, restrita ao domínio quando disponível. No SMTP do Supabase, informe o remetente desse domínio, o nome OpenKoda, o host `smtp.resend.com`, a porta 465, o usuário `resend` e a chave no campo de senha. Desative o rastreamento de cliques. Um sistema que reescreve links pode invalidar a confirmação.
3. Em **Authentication → URL Configuration**, inclua o endereço HTTPS de retorno do site. Na implantação atual ele é `https://openkoda-desktop.steelyrowan1.chatgpt.site/conta/retorno/`. Mantenha a confirmação do endereço obrigatória e verifique a validade do OTP em **Authentication → Providers → Email**. A tentativa no navegador vale uma hora. Os testes com o coletor SMTP local utilizam a mesma ponte de sessão da implantação.
4. Em **Authentication → Emails → Templates**, use [o modelo de confirmação](../../supabase/templates/confirmation.html) com o assunto “Confirme seu cadastro no OpenKoda” e [o modelo de recuperação](../../supabase/templates/recovery.html) com “Redefina sua senha do OpenKoda”. O Supabase gera o endereço de verificação em `{{ .ConfirmationURL }}`.
5. Para o suporte, em **Edge Functions → Secrets**, salve `RESEND_API_KEY` e `KODA_SUPPORT_FROM` com o remetente autorizado. A função envia a notificação para o Gmail do operador. A caixa de entrada recebe o endereço do painel administrativo. O assunto do chamado, a descrição, a identidade de quem o abriu e a resposta ficam no painel autenticado.

O remetente de teste `onboarding@resend.dev` só envia para o endereço cadastrado da própria conta Resend. Como o operador é o destinatário dos avisos de suporte, o canal de suporte pode ser testado nesse modo. Para mensagens de cadastro e recuperação destinadas ao público, verifique um domínio no Resend ou configure outro remetente SMTP transacional que aceite os destinatários.

## Verificação e operação

Com Docker e Node instalados, execute na raiz:

```powershell
node supabase/tests/mail/auth-mail.mjs
```

O teste inicia PostgreSQL, GoTrue e uma caixa Mailpit temporários. Lê os dois e-mails efetivamente entregues por SMTP, verifica a origem e o retorno dos links, confirma o cadastro, redefine a senha e tenta repetir cada link. Verifica a recusa do acesso sem cookie PKCE e com a senha anterior. A ponte usa endereços reservados para teste, e os serviços expõem somente portas de loopback. O próprio teste remove os recursos da execução.

Na hospedagem, faça o cadastro por um alias de teste no navegador. Abra o link da mensagem no mesmo perfil de navegador, saia da conta e solicite a recuperação. Verifique a entrada com a nova senha e a recusa da anterior. Para o suporte, abra um chamado sintético, confira a saída do Resend e a mensagem recebida no Gmail do operador. A resposta ao usuário fica disponível em **Minha conta → Suporte → Meus chamados e respostas**.

Cada novo chamado começa pendente. Somente a função do servidor pode reservar até quatro avisos por chamada à fila. O bloqueio da linha impede que duas execuções reivindiquem o mesmo aviso. O reenvio usa a mesma chave de idempotência no provedor. Diante de falha de rede, resposta 429 ou falha temporária, há uma segunda tentativa em segundo plano. Aberturas da lista e novos chamados retomam os pendentes. Uma indisponibilidade prolongada sem atividade de suporte deixa os avisos aguardando a próxima consulta. Uma rejeição definitiva do provedor ou quatro tentativas encerra o envio automático. Na tabela privada `tickets`, consulte `notice_sent_at`, `notice_failed_at`, `notice_pending` e `notice_attempts`. Um aviso marcado como enviado indica aceitação pelo provedor; a caixa do destinatário confirma a entrega. Chamados anteriores à instalação da fila preservam o histórico, sem reenvio em massa.

A autorização do operador continua no painel Supabase. O link de cadastro ou recuperação autentica só a combinação de código de uso único e navegador com o verificador PKCE. A ponte elimina os códigos da barra de endereço, restringe a origem dos POSTs e armazena a sessão em cookies Secure e HttpOnly. Eventos de autenticação podem ser consultados em Auth Logs. Nunca copie o link integral, a senha SMTP, o corpo de um chamado ou um cookie para o Git e para os registros da aplicação.

Consulte a [documentação SMTP do Supabase](https://supabase.com/docs/guides/auth/auth-smtp), os [modelos de autenticação](https://supabase.com/docs/guides/auth/auth-email-templates), a [integração Resend e Supabase](https://resend.com/docs/send-with-supabase-smtp) e as [restrições do remetente de teste](https://resend.com/docs/knowledge-base/403-error-resend-dev-domain).
