# Revisão e cobertura do mapa

Esta entrega especifica o produto proposto e suas principais condições, sem declarar implementação existente. Produto/experiência, execução Kali e segurança foram mapeados por especialistas em paralelo; o coordenador integrou arquitetura, estados, configurações, glossário, decisões e critérios de validação.

Revisão consolidada após o usuário confirmar **Tauri** e aprovar seis conceitos visuais. As três frentes revisaram produto/jornadas, ambiente/ferramentas e segurança/IA/configuração. O coordenador incorporou os resultados, registrou o ADR aceito, centralizou o contrato visual e atualizou arquitetura, estados, fases e diagramas. As imagens aprovadas foram preservadas sem regeneração.

## Cobertura do pedido

| Área solicitada | Onde foi mapeada |
|---|---|
| Para que serve e para quem | PRODUTO-E-ESCOPO e README |
| Como funcionará do começo ao relatório | README, PAINEL-E-JORNADAS, diagrama03 |
| Como será o painel | VISUAL-E-TELAS, seis imagens aprovadas, 17 telas em PAINEL-E-JORNADAS e diagrama09 revisado |
| Tauri e integração Windows/WSL | ADR-0004 aceito, ARQUITETURA, diagrama02, SEC-23 e AC-25 |
| O que a IA conseguirá fazer | IA-MCP-E-DADOS e catálogo de capacidades |
| Código, web/API, rede e offline | PRODUTO-E-ESCOPO, ferramentas e perfis |
| Instalação de WSL/Kali e conjuntos de pacotes | KALI-E-AMBIENTES, diagramas07/10 |
| Uso de ferramentas e terminal avançado | FERRAMENTAS-E-OPERACAO, IA-MCP-E-DADOS |
| Comunicação entre módulos | ARQUITETURA, diagrama02 e contratos MCP |
| Autorização, autonomia e limites | PERMISSOES-E-SEGURANCA e diagrama04 |
| Dados, resultados e estados | DADOS-E-ESTADOS, diagramas05/06/08 |
| Configurações e defaults | CONFIGURACOES |
| Credenciais, contexto enviado e retenção | IA-MCP-E-DADOS, segurança e dados |
| Processos longos, falha, pausa e cancelamento | DADOS-E-ESTADOS e operação Kali |
| Correções, patches, reteste e histórico | Produto, jornadas e fases |
| Atualização, manutenção, backup e desinstalação | Kali, configurações e fases |
| O que construir primeiro e como saber que funciona | FASES-E-VALIDACAO, 28 cenários transversais e 23 requisitos SEC |
| Escolhas confirmadas e ainda abertas | DECISOES, ADR-0004 aceito e ADRs 0001–0003 propostos |

## Harmonizações realizadas na revisão

- Escopo congela ao iniciar Preparando. Mudança posterior cria nova Avaliação vinculada, preservando a anterior; revisar plano dentro do escopo continua possível com revalidação.
- Os estados de Avaliação são canônicos e separados dos estados de Execução, IA e Ambiente. Pausa não implica suspensão POSIX do processo.
- Aprovação humana de início, grant técnico de uso único e lease de execução são distintos.
- Captura por Operação agrega tentativas; retries não multiplicam silenciosamente o orçamento.
- Workspace da Avaliação possui subdiretórios por tentativa; manutenção não entra com processos não reconciliados.
- Perfis WSL, shell avançado, rede e backend reforçado não prometem controles sem prova técnica.
- Tauri escolhido é distinto da proposta de Tauri 2/React; IPC da UI e MCP do executor são fronteiras separadas, sem shell genérico exposto ao renderer.
- Navegação Avaliações/Achados/Projetos e acessos secundários substituem três colunas permanentes. Conversa e execuções abrem sob demanda; fechar um painel não pausa uma operação.
- Labels dos conceitos derivam dos estados canônicos; candidatos “Em revisão”, progresso, saúde e teste de conexão não viram confirmações por aparência.
- Instalação de pacotes tem cálculo/revisão/autoridade próprios; credencial mascarada não comprova vault; teste de conexão se vincula à revisão.

As frentes de produto, backend e segurança revisaram os contratos cruzados. Não restaram conflitos materiais relatados após as correções; isso é revisão do mapa, não uma validação do produto em execução.

## Verificação de artefatos

Os dez diagramas passaram pela validação do MCP OpenFlowKit: [resultado de compilação](referencias/validacao-diagramas.json). São mantidos em fonte, documento editável e SVG. Os mapas02 e09 foram atualizados no mesmo documento/frame após leitura das edições existentes, sem descartes. O validador [validar_mapa.py](referencias/validar_mapa.py) confere links locais, JSON/XML, identidade SVG/documento, preservação da fonte DSL, ADRs propostos/aceito, os seis PNGs e seus IDs visuais, 28 cenários AC e 23 requisitos SEC. Dimensões e hashes das imagens são registrados para rastreabilidade, sem avaliar automaticamente qualidade ou fidelidade. Resultado corrente: [validacao.json](referencias/validacao.json).

Prévia visual dos mapas atualizados é uma inspeção dos SVGs, sem construir UI executável. As seis imagens aprovadas continuam como referência de acabamento; as figuras OpenFlowKit explicam composição/arquitetura e não substituem a galeria. Tabelas e contratos registram condições omitidas por legibilidade.

## Limites da completude

O mapa cobre as áreas de produto e os contratos necessários para orientar o desenvolvimento. Bibliotecas, modelos, versões suportadas finais, desempenho medido, controles de SO, formatos completos de cada adapter, distribuição e isolamento reforçado dependem das decisões/provas futuras descritas nas fases. Capacidade proposta não aparece como capacidade já instalada.

Nenhum pacote Kali, ambiente WSL, scanner, alvo, instalador ou aplicação OpenKoda foi executado/construído nesta etapa. Scripts de validação e prévias estáticas servem apenas para conferir a documentação.
