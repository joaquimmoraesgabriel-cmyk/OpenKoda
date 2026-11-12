# OpenKoda — cibersegurança local

Vocabulário do desktop de segurança, incluindo modalidades históricas. Reutiliza as definições de instalação, plano de ação, aprovação, política de automação, executor e revogação do glossário da pasta pai. Os termos abaixo pertencem ao produto desta pasta.

## Trabalho e autoridade

**Projeto**:
Conjunto duradouro de alvos, avaliações, regras e evidências de um trabalho de segurança.
_Evitar_: Avaliação, pasta, conta.

**Repositório importado**:
Cópia de um repositório obtida de uma origem remota identificada para compor um projeto.
_Evitar_: Conexão contínua, autorização sobre todos os repositórios de uma conta.

**Opções de análise do projeto**:
Preferências para preparar os próximos trabalhos de um projeto, sem reescrever avaliações já iniciadas.
_Evitar_: Escopo já adotado, aprovação retroativa.

**Alvo**:
Recurso identificado que pode ser avaliado dentro de um escopo autorizado.
_Evitar_: Qualquer destino descoberto, toda a Internet.

**Escopo autorizado**:
Delimitação versionada dos alvos, métodos, condições e período permitidos para um trabalho.
_Evitar_: Declaração genérica de propriedade, permissão irrestrita.

**Avaliação**:
Trabalho de segurança com objetivo, escopo e resultado próprios, que pode reunir diversas operações e execuções.
_Evitar_: Processo, comando, achado.

**Catálogo de alvos**:
Conjunto de recursos registrados em um projeto para possível seleção em avaliações. Estar no catálogo não concede autorização para acessar o recurso.
_Evitar_: Escopo autorizado, descoberta automática.

**Entrada de escopo**:
Seleção de um alvo e dos métodos permitidos para ele dentro de uma revisão do escopo de uma avaliação.
_Evitar_: Projeto inteiro, permissão herdada de outro alvo.

**Avaliação combinada**:
Avaliação que reúne entradas de escopo explicitamente selecionadas, mantendo a autoridade e a cobertura de cada entrada distinguíveis.
_Evitar_: Varredura irrestrita, autorização de todo o catálogo.

**Exclusão de escopo**:
Restrição explícita que impede determinadas leituras ou conexões dentro do trabalho revisado.
_Evitar_: Filtro visual de resultados, recurso avaliado sem achados.

**Operação**:
Unidade de ação de um plano, com efeito, alvo e limites determinados antes de executar.
_Evitar_: Texto livre da IA, execução já realizada.

**Execução**:
Tentativa concreta de realizar uma operação, com início, término ou estado ainda não resolvido.
_Evitar_: Avaliação inteira, autorização.

**Encerramento local**:
Decisão explícita de encerrar a posse de um recurso local após confirmar que não há executor local ativo, mantendo registrado um resultado remoto ainda desconhecido.
_Evitar_: Conclusão remota, cancelamento no provedor, confirmação de cobrança zero.

**Perfil de execução**:
Conjunto de capacidades e limites técnicos escolhido para realizar uma classe de operações.
_Evitar_: Nível de confiança da IA, promessa de isolamento.

**Ambiente**:
Recurso operacional preparado para realizar execuções, com identidade e condições conhecidas.
_Evitar_: Garantia de sandbox, projeto.

## Resultados

**Evidência**:
Registro identificável de uma observação, com origem, integridade e vínculo ao trabalho que a produziu.
_Evitar_: Resumo sem origem, opinião da IA.

**Vínculo de origem**:
Relação entre uma evidência e as entradas de escopo que contribuíram para ela. Verificar essa relação não confirma a veracidade do conteúdo fornecido por uma pessoa.
_Evitar_: Validação do achado, autorização para novo acesso.

**Candidato**:
Alegação de possível problema que ainda precisa de validação suficiente para ser tratada como achado.
_Evitar_: Vulnerabilidade confirmada, resultado garantido.

**Achado**:
Problema sustentado por evidência e resultado de validação, com impacto, confiança e limites declarados.
_Evitar_: Qualquer alerta de ferramenta, hipótese sem avaliação.

**Cobertura**:
Registro do que foi examinado, excluído, tentado ou deixado pendente em uma avaliação.
_Evitar_: Número de ferramentas instaladas, garantia de ausência de falhas.

**Reteste**:
Nova avaliação delimitada da condição relacionada a um achado após uma mudança.
_Evitar_: Marcar como corrigido sem nova evidência, repetir automaticamente tudo.

**Relatório**:
Apresentação versionada de resultados, evidências e cobertura de uma avaliação ou conjunto delimitado delas.
_Evitar_: Fonte única de verdade substituindo as evidências.
