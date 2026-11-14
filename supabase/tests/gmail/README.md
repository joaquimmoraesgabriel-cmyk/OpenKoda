# Entrega de cadastro e recuperação no Gmail

O ensaio inicia um Supabase Auth e um PostgreSQL descartáveis. A conta criada é exclusiva desse banco. O GoTrue usa os modelos de `supabase/templates` e entrega as duas mensagens por SMTP ao Mailpit. O processo de teste confere o destinatário e retransmite o MIME original pelo SMTP do Resend com STARTTLS. O endereço de destino deve pertencer ao operador da conta Resend enquanto o remetente for `onboarding@resend.dev`. [Documentação de SMTP do Resend](https://resend.com/docs/send-with-smtp)

No Windows com Docker Desktop e Node.js 22 ou posterior, rode primeiro o ensaio fechado:

```powershell
node supabase/tests/gmail/verify.mjs ensaio@gmail.com --offline
```

O modo offline usa a caixa SMTP local. As imagens de contêiner indicadas no compose precisam estar disponíveis localmente. O ensaio básico em `supabase/tests/mail` pode baixá-las.

Para o teste externo, execute o mesmo comando com o Gmail cadastrado no Resend e retire `--offline`. A janela pede uma chave temporária de envio do Resend. Ela segue por um pipe privado em memória para o cliente SMTP Python. Revogue a chave temporária no Resend depois do teste.

O programa imprime o assunto único da mensagem, a origem local esperada e o caminho `receipt` de um arquivo temporário. Pela integração Gmail, leia a mensagem HTML recebida e confira destinatário, assunto, remetente e origem esperada em `127.0.0.1`. Calcule o SHA-256 do endereço do botão após decodificar as entidades HTML e grave apenas os 64 dígitos hexadecimais minúsculos no arquivo indicado usando UTF-8 sem BOM. O teste só resgata o link se a impressão digital coincidir exatamente. Confirmação e recuperação usam códigos de uso único e PKCE. O ensaio ainda verifica que a senha anterior foi recusada e a nova permitiu entrar. Os arquivos de recibo são apagados e os links nunca devem ser registrados nem compartilhados.

O servidor de teste escuta apenas na interface local do computador durante o ensaio. Quando o programa termina, os contêineres, a base, o registro de teste e as credenciais locais são removidos. Para a abertura comercial ainda é preciso configurar o SMTP e conferir a entrega pelo site e pelo domínio do remetente em produção.

Em 13 de novembro de 2026, o Gmail recebeu a confirmação às 23h42 de Brasília na pasta de spam e a recuperação às 23h43 na caixa de entrada. Nas duas mensagens, o Gmail indicou SPF, DKIM e DMARC aprovados. Os links recebidos coincidiram com os emitidos pelo GoTrue, completaram o resgate e recusaram a segunda tentativa. Após a troca, o login aceitou a nova senha e recusou a anterior. Uma tentativa preliminar também entregou uma confirmação em spam antes de a sessão de ensaio ser interrompida.
