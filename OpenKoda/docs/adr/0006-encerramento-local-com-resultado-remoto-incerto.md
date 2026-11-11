---
status: accepted
scope: OpenKoda desktop de cibersegurança
---
# Encerramento local com resultado remoto incerto

Uma requisição IA pode perder a conexão sem deixar uma resposta remota recuperável, bloqueando indefinidamente o recurso local. Decidimos oferecer um encerramento local explícito, condicionado à prova de ausência do worker local e à identidade exata da tentativa, preservando como desconhecidos o resultado remoto e os custos pendentes. Essa decisão evita tanto um bloqueio permanente quanto tratar reconhecimento humano de incerteza como prova de conclusão remota.

O encerramento precisa de revisão e confirmação humanas próprias, recibo durável e liberação local no mesmo commit; não pode repetir a tentativa, concluir suas dependências nem acordar automaticamente trabalhos já admitidos. Novos trabalhos exigem preparação e admissão posteriores, com a incerteza anterior visível. Reinício, restauração, manutenção e exclusão de histórico devem conservar essa distinção e o registro financeiro; nenhuma disposição local pode declarar cancelamento no provedor ou cobrança zero.

Decisão aceita para a implementação em andamento da 0.1.6. A disponibilidade depende dos testes de falha, concorrência, reinício e restauração; este registro não declara a função pronta. Consultar uma resposta remota identificada é uma operação separada e não faz parte desta decisão.
