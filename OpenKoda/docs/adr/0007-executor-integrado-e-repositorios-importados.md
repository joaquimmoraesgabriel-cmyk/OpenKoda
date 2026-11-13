---
status: accepted
scope: OpenKoda desktop, edição local de análise de código
---
# Executor integrado e repositórios importados

A edição local usa a interface e a autoridade de admissão do OpenKoda para controlar o SDK Codex Security. O usuário escolheu integrar esse motor em lugar das modalidades de rede, Web por URL e Kali. Uma análise é admitida com projeto, alvo, política, modelo e orçamento determinados; recibos, evidências e custos permanecem no domínio local. Comandos internos pertencem ao executor integrado, portanto esta integração não implementa a proposta de revalidar cada comando externo descrita no ADR 0001 nem reativa o executor anterior.

O projeto pode partir de uma pasta ou de uma cópia obtida de um repositório público do GitHub. A cópia recebe uma identidade própria e uma revisão de origem; o recibo permite repetir a solicitação sem repetir o download ou a avaliação. Cancelamento não autoriza uma tentativa nova. Um download que terminou em falha ou cancelamento exige uma ação explícita para iniciar outra tentativa. As cópias e os artefatos do executor são materiais separados do backup SQLite e da exclusão lógica do projeto; o relatório incorporado continua sendo evidência do projeto.

A escolha local evita exigir um serviço de análise próprio, mas tem um limite material: o perfil do SDK 0.10.0 permite leitura ampla do sistema acessível à conta, não apenas da pasta escolhida. Alvos e caminhos orientam a análise; não constituem isolamento de leitura no sistema operacional. A autorização de escopo não é ampliada por essa capacidade técnica. A interface informa esse limite, e a fábrica impede concessões adicionais com approval_policy=never. MXC é preferido quando disponível, preservando o fallback elevado e a proteção das credenciais do SDK.

Uma garantia forte de exposição apenas do código selecionado requer um executor descartável ou hospedado com somente esse material disponível. Essa alternativa ainda não foi implementada e precisa ser resolvida antes de afirmar isolamento por repositório. O cofre DPAPI, a projeção de dados e os testes desta versão não substituem esse isolamento. Esta decisão registra o funcionamento da edição local; não aprova publicação pública nem acesso a dados fora do escopo autorizado.

Na versão fixada do SDK, o limite de custo não funciona com os IDs canônicos do OpenRouter. Novas análises admitem OpenAI; os demais provedores continuam nas conversas. A admissão precisa ser revista quando o SDK oferecer uma forma verificável de estimar esse custo, mantendo o limite e os recibos históricos. O preflight e o inventário de revisão da 0.2.1 documentam as provas e as limitações em app/docs/RETOMADA-0.2.1.md.

## Revisão para a 0.2.2

As novas análises de código precisam funcionar com os provedores cadastrados e limitar a leitura aos arquivos selecionados. O desktop captura texto com handles do Windows, mantém o material em memória e envia somente os trechos admitidos à API. O provedor não recebe acesso ao host ou ao sistema de arquivos. A revisão identifica candidatos estáticos e registra a cobertura por lote. Os recibos de análises anteriores continuam acessíveis. Executar a análise completa do SDK com comandos exige um ambiente externo isolado e uma credencial separada do processo que executa esses comandos.
