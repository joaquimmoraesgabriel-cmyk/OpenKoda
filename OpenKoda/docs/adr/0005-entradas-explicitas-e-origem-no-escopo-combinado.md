# Entradas explícitas e origem no escopo combinado

Uma avaliação combinada conserva entradas tipadas e métodos por entrada em uma revisão imutável, separada do catálogo do projeto. Operações escolhem uma entrada; envios de contexto à IA escolhem todas as entradas contribuintes, e a origem é resolvida a partir de registros do host. Isso evita que o alvo principal, a união dos métodos ou um conteúdo importado amplie a autoridade de uma operação.

O escopo de alvo único e seus hashes históricos permanecem inalterados; a passagem para o formato combinado exige derivar outra avaliação. A alternativa de converter avaliações existentes permitiria reinterpretar aprovações e evidências antigas. A migração do armazenamento acrescenta suporte ao novo formato sem reescrever o JSON legado quando não há reconciliação pendente.

Exclusões locais relativas são aplicadas antes de abrir conteúdo ou percorrer a árvore excluída; negativas de conexão são aplicadas antes de abrir sockets. Uma raiz local inteira explicitamente excluída pode ser um alias de outra raiz: como não há consentimento para inspecionar seus metadados, operações locais são recusadas nesse caso. A revisão pode usar exclusões relativas à raiz selecionada; operações web continuam sujeitas às próprias regras. Não se assume separação física a partir de nomes de caminhos.

Conteúdo manual recebe uma associação declarada a entradas revisadas, sem ser interpretado como prova produzida por ferramenta. Uma sonda de reconciliação do Kali confirma apenas o término de trabalho anterior e não gera evidência de cobertura de recursos. Relatórios mantêm a cobertura por entrada e a distinção entre registros verificados e alegações que ainda exigem revisão humana.
