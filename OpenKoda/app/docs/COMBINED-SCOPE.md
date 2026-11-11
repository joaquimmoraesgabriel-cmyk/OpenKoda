# Alvos combinados e origem — 0.1.4 local

Esta versão amplia a seleção de recursos sem ampliar os adaptadores disponíveis. Uma operação continua vinculada a um alvo determinado e exige o fluxo normal de preparação e aprovação. A evidência de cada verificação e o estado de entrega ficam em IMPLEMENTACAO.md; este documento descreve o contrato, não certifica um instalador.

## Catálogo e revisão

O projeto tem um alvo principal e até 63 alvos adicionais. O alvo principal permanece compatível com avaliações antigas. Adicionar um recurso ao catálogo não autoriza acesso; a avaliação combinada seleciona de uma a oito entradas, cada uma com métodos compatíveis. O diagnóstico do Kali é comum à avaliação e não representa cobertura dos recursos.

Uma entrada conserva a identidade do item de catálogo, modalidade, destino, rótulo e métodos na revisão. A referência da entrada contém identidade e digest. Alterar o rótulo ou inativar um item no catálogo não reescreve escopos existentes; inativar impede novas seleções. Remover exige que nenhuma revisão referencie o item. Para encerrar autoridade já revisada, revogue o escopo da avaliação.

Prévia e confirmação verificam o digest do catálogo, a revisão e a chave idempotente. Uma repetição exata retorna o resultado anterior; reutilizar a chave com outro corpo falha. Antes do primeiro trabalho, uma avaliação combinada pode revisar suas entradas. Depois do congelamento, derive outra avaliação, escolhendo objetivo, janela, alvos e exclusões a copiar. Ela recebe novas identidades de entrada e começa sem revisão. Evidências, achados, permissões e operações não são copiados.

O alvo principal da avaliação combinada serve à apresentação. Ele não fornece um destino implícito para operações nem autoriza todos os métodos reunidos nas outras entradas. Avaliações simples existentes conservam seu formato; sua conversão direta é recusada.

## Exclusões e identidade local

Exclusões locais pertencem a uma entrada selecionada e usam um caminho relativo exato ou uma subárvore. Não aceitam navegação, caminhos absolutos, fluxos alternativos, caracteres ambíguos ou expressões livres. São aplicadas antes da leitura do conteúdo e antes de percorrer diretórios excluídos. Os limites são 128 regras no conjunto e 32 por entrada.

Raízes locais selecionadas não podem se sobrepor. O preflight observa metadados das raízes necessárias e mantém identidades estáveis durante a operação. Isso não lê o conteúdo de outra entrada; o conteúdo só é adquirido para o alvo e método da operação. A captura usa exclusivamente arquivos já presentes no manifesto revisado, e a correção exige também que a entrada pertença à origem do achado.

Uma raiz local inteira explicitamente excluída pode ser um alias de outra raiz. Sem autorização para inspecionar seus metadados, o host não pode comprovar separação física. Por isso, operações locais nessa combinação são bloqueadas antes do preflight; use exclusões relativas ao trabalhar com raízes locais. Operações web continuam sujeitas às próprias regras. Não se interpreta um erro de acesso como prova de ausência de conteúdo.

As exclusões de conexão são IPs ou redes CIDR literais, com portas opcionais, comuns ao escopo. Não fazem resolução DNS. Aplicam-se a HTTP e TCP antes de abrir sockets. TCP conserva os limites positivos de 16 endereços, 16 portas e 64 combinações antes de subtrair os pares excluídos; uma exclusão não aumenta a quota. HTTP conserva a URL e os peers revisados, sem redirects.

## Evidências e contexto

O host resolve a origem das evidências a partir de planos, execuções e provas registradas. Conteúdo manual recebe um envelope com associação declarada às entradas escolhidas, mas seu texto não é decodificado como saída de ferramenta. Um JSON importado que se pareça com prova HTTP, Kali ou remediação continua texto fornecido pelo usuário.

Antes de uma revisão por IA, a pessoa seleciona as entradas do contexto e as evidências. Todas as entradas contribuintes de cada evidência precisam estar na seleção e permitir revisão remota. O contexto contém apenas metadados das entradas selecionadas; não inclui o catálogo inteiro, o alvo principal não escolhido ou a cobertura agregada da avaliação. O digest vincula essa seleção à prévia de custo e ao envio. Nenhuma dessas escolhas autoriza automaticamente outra operação.

Uma sugestão de operação recebe também a entrada operacional escolhida pela pessoa, que deve pertencer ao contexto. A resposta do modelo não escolhe destino, portas, caminho ou autorização. As referências devolvidas pelo host são conferidas pela interface antes de encaminhar à preparação normal.

Hash e análise de evidência no Kali herdam todas as entradas contribuintes; o host as revalida antes do lançamento e depois do handshake. Diagnóstico comum tem origem de recurso vazia. Uma sonda de reconciliação registra o término do trabalho anterior no journal e não produz cobertura nova.

`evidence.origins` é uma projeção somente de leitura. `verified:true` significa que o vínculo foi conferido; não confirma a veracidade de uma alegação, vulnerabilidade ou correção. `originKind:user_association` identifica a associação manual. Um registro não verificável permanece sem referências confiáveis na interface.

## Operações, cobertura e preservação

A preparação combinada recebe `scopeRef`, `targetRef` e um dos quatro adaptadores nativos. Portas são obrigatórias somente para TCP. As rotas simples continuam próprias das avaliações simples. Planos e recibos conservam as referências, exclusões aplicadas e preflight de identidade. Revogar impede novas admissões de todas as entradas, preservando trabalho anterior e permitindo sua reconciliação.

A validação HTTP e seu reteste comparam execuções da mesma entrada. Uma resposta posterior de outra URL não valida nem corrige o achado. A ordem persistida das execuções determina a mais recente, mesmo quando o relógio recua. Remediação local exige origem compatível, base imutável, escrita única e confirmação humana separada.

A cobertura apresenta entradas e métodos separadamente. IA, captura, diagnóstico e correção têm registros próprios; ausência de execução de alvo não significa ausência desses registros. Saída parcial, omissões e resultados inconclusivos não equivalem a avaliação completa. Relatórios incluem referências de origem, revisões, exclusões e limitações, sem exportar todo o catálogo do projeto.

Backup conserva o catálogo e o histórico do perfil, excluindo credenciais e o saldo global da instalação. Restauração cria uma nova cópia e exige nova configuração operacional. O expurgo continua limitado ao grafo revisado do projeto arquivado; avaliações simples e combinadas seguem os mesmos limites de preservação de cópias externas e fontes originais.

## Ações adicionadas

| Grupo | Ações |
| --- | --- |
| Catálogo | `project.targetOptions`, `targetAdd`, `targetUpdate`, `targetRemove` |
| Nova avaliação | `assessment.combinedOptions`, `combinedPreview`, `combinedCreate` |
| Revisão | `assessment.combinedScopePreview`, `combinedScopeSave` |
| Derivação | `assessment.combinedDerivePreview`, `combinedDeriveCreate` |
| Operação | `assessment.targetPlan` |
| Origem | `evidence.origins` |

`assessment.scopeStatus` acrescenta `targetOptions`, `commonWorkGates` e `targetCoverage`. `assessment.scopeRevoke` permanece comum aos dois formatos. Os contratos de IA recebem `contextTargetRefs`; a sugestão recebe também `operationTargetRef`. Esses campos são opcionais apenas no formato simples e não podem ser usados para converter sua autoridade.
