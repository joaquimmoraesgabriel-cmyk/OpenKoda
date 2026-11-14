# Uma resposta cobrada pela cota da assinatura

Em 13 de novembro de 2026, uma conta descartável com plano Plus entrou pelo Supabase Auth do projeto Koda. O ensaio usou o mesmo endpoint hospedado de chat e consulta de uso do aplicativo. A assinatura foi provisionada para a conta de teste, sem checkout. O teto global temporário foi de US$ 0,000900.

| Etapa | Resultado |
| --- | --- |
| Entrada na fila | Uma reserva de capacidade de US$ 0,000001 ocupou a única vaga. A solicitação real recebeu HTTP 429 e posição 1. A consulta da conta confirmou zero de uso liquidado enquanto aguardava. |
| Liberação | A reserva artificial passou a `not_sent`, sem débito. O mesmo tíquete passou pela verificação de cota e reservou no máximo US$ 0,000248 para a chamada real. |
| Resposta | HTTP 200. A geração informou 40 tokens de entrada e 3 de saída. A chave do OpenRouter apresentou variação de **US$ 0,000006450**. O recibo da geração confirmou o mesmo valor. |
| Liquidação | O banco registra custos em milionésimos de dólar e arredonda cada geração para cima. A consulta da conta retornou **US$ 0,000007** de uso liquidado, uma vez na janela de sete dias e no acumulado do ciclo. Ambas representam a mesma geração. O saldo reservado voltou a zero. |
| Reenvio | O mesmo pedido, com o mesmo tíquete, recebeu HTTP 409 e `request_replayed`, com status `completed`. Uma nova consulta da conta mostrou os mesmos valores. O banco registrou exatamente uma geração com recibo do provedor. |

O acesso temporário à credencial foi restrito à sessão descartável e a um único identificador de solicitação. A função obteve o valor apenas em memória após receber o credenciamento cifrado. O fluxo normal verificou o usuário no Supabase Auth, o modelo, os limites de custo, a fila, o despacho e a liquidação. Ao encerrar o ensaio, o servidor voltou à implementação normal, com os orçamentos diário e mensal em zero e a inferência comercial desativada.

A exclusão da conta pelo fluxo normal do aplicativo recebeu HTTP 200. Usar a mesma sessão em seguida recebeu HTTP 401. No banco hospedado, a contagem final foi zero para o usuário descartável, suas sessões, perfil e assinatura. A fila e as reservas ativas também estavam vazias. A credencial temporária de sessão foi removida do computador.

Impressão SHA-256 da solicitação: `2cfdb12b1afa3ab13a04a3feff2af4caede959f553830abe985af0aaa7b33d21`. Impressão SHA-256 do identificador de geração: `0964874b3a977e07e4294d25375cd4a82b3529408cd3ab954ecd5f09397f6660`.
