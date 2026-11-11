# Dados e referências de credenciais — 0.1.5

Estado em 2026-11-08: **marco local 0.1.5 validado nos limites registrados**. O [manifesto](../releases/0.1.5-local-20261108/release.json) preserva os binários sem assinatura; evidências, histórico e limitações estão em [RETOMADA-0.1.5](RETOMADA-0.1.5.md). Isso não conclui o roteiro completo nem certifica todos os percursos possíveis.

A política de dados define quais materiais você pode consultar, enviar à IA, transferir ao Kali ou incluir em um relatório e backup. Você continua revisando separadamente o escopo operacional, a execução e, quando aplicável, o orçamento de IA. Cadastrar uma fonte ou classificar uma evidência não autoriza uma ferramenta a acessar o alvo.

## Configurar uma política

1. Abra **Projetos**, escolha o projeto e entre em **Configurações → Política de dados do projeto**. Para consultar também projetos arquivados, use **Configurações → Dados e privacidade → Dados por projeto**.
2. Escolha os destinos necessários: **Provedor de IA**, **Kali local** ou **Relatório local**. Para cada destino habilitado, selecione as classes, categorias de dados e fontes permitidas. A transferência de bytes originais ao Kali possui uma opção adicional.
3. Revise separadamente **Leitura local por janela** e **Backup de recuperação**. Os destinos de saída e o backup começam desabilitados na proposta inicial; a leitura local começa habilitada para as quatro classes.
4. Selecione **Revisar alterações da política**, confira a revisão e confirme em **Salvar política revisada**. Essa gravação não envia o conteúdo do projeto.
5. Na avaliação, abra **Resumo → Política de dados desta avaliação** e escolha **Revisar adoção da política nesta avaliação**. Confira a revisão identificada, os destinos permitidos ou bloqueados, classes, categorias, fontes e as condições de leitura local/backup antes de **Confirmar adoção prospectiva**. Para Kali, confira também a permissão de bytes originais. Seleções mostradas em um destino bloqueado não autorizam seu uso.

A avaliação não adota automaticamente a política do projeto. O vínculo pode ser alterado antes do primeiro uso admitido; depois fica congelado. A regra aplicável é a interseção entre a revisão vinculada e a política corrente do projeto. Restringir a política reduz as permissões futuras; ampliar a política não amplia um vínculo já congelado. Para uma ampliação, derive outra avaliação e revise seu escopo e sua adoção.

**Desativar novas saídas** torna a política inativa e bloqueia novos usos sujeitos a ela, inclusive a leitura por janela quando existe uma política inativa. A ação preserva os recibos e os efeitos já ocorridos. **Revisar reativação da política** exige outra confirmação e não altera silenciosamente o corpo da revisão.

Se os dados ou a política mudarem enquanto você revisa uma alteração, a interface descarta a revisão pendente. Consulte o estado atual e prepare outra revisão antes de confirmar, inclusive ao reativar pela avaliação.

## Classificar conteúdo e origem

Na avaliação, abra **Resumo → Classificação e origem dos dados**. Selecione o dado, confira sua categoria e sua classe efetiva, escolha **Nova classificação**, informe o motivo e revise a decisão antes de confirmá-la. Não coloque senhas ou tokens no motivo.

| Classe | Significado para os controles |
|---|---|
| Não classificado | Falta uma decisão válida sobre o dado ou um contribuinte. Não é autorização para saída. |
| Público | Decisão de sensibilidade; não comprova a veracidade da informação. |
| Interno | Requer permissão dessa classe no destino escolhido. |
| Sensível | Requer permissão dessa classe e confirmação específica no pacote ou na janela. |

As categorias distinguem texto humano, identificação de fontes e alvos, resumo de execuções, trechos de evidências, trechos de código e arquivos, descrição/validação de achados e registro de atividades. Permitir trechos de evidências não permite automaticamente capturas de código.

A classe efetiva considera o próprio registro e seus contribuintes. Um contribuinte sem classificação válida mantém o resultado **Não classificado**; com todos classificados, prevalece a maior sensibilidade. Redigir um valor não reduz automaticamente essa classe. Rebaixar uma decisão exige motivo e confirmação adicional.

Para material legado ou de origem desconhecida, você pode declarar a associação às fontes do escopo atual. Essa associação permanece identificada como declaração do usuário, com `sourceVerified=false`. Ela não substitui uma origem conhecida, não modifica o conteúdo original e não valida a alegação ou a autoridade operacional. Relatório, IA e transferência de evidência ao Kali recusam conteúdo cuja origem necessária continue desconhecida. O backup integral de recuperação possui uma finalidade e uma regra próprias.

Uma classificação vale para a versão exata do dado. Uma mudança e posterior retorno aos mesmos valores não restauram uma classificação ou prévia anterior. O núcleo mantém gerações prospectivas dos registros canônicos mutáveis, sem reescrever hashes históricos de evidências, planos ou escopo.

## Consultar uma evidência ou registro derivado

A listagem apresenta metadados e uma prévia de até 2 KiB após redação. O hash `rawSha256` identifica o original preservado; `projectionSha256` identifica a representação exibida. Uma prévia parcial não substitui a evidência inteira.

Importar uma evidência e consultar o snapshot preservam a mesma descrição da projeção: hash original, tamanho e informações de redação/omissão. Esses metadados não são substituídos por uma descrição genérica da resposta. Os avisos de omissão usam rótulos de conteúdo para você identificar o que não foi exibido.

1. Abra a evidência ou o registro derivado e selecione **Revisar trecho armazenado**.
2. Confira a sensibilidade, o intervalo e o tamanho. A preparação não devolve o texto da janela.
3. Quando o material for sensível, marque a confirmação específica. Use **Confirmar leitura desta janela**.
4. Para continuar, selecione **Ler próximo trecho** e confirme a próxima janela. Cada janela tem até 32.768 bytes UTF-8 e possui revisão e recibo próprios.

Os intervalos referem-se à representação completa após redação. O redator é aplicado antes do corte para que uma credencial não escape por atravessar a divisão entre janelas. Repetir uma confirmação já consumida devolve o recibo, sem devolver novamente os bytes. Troca de avaliação, alteração de dados/política ou expiração exige nova revisão; a interface limpa o conteúdo pendente.

Sem política cadastrada, você pode confirmar uma leitura local para inspecionar e classificar material ainda não classificado. Isso não cria permissão de IA, Kali, relatório ou backup. A adoção de uma política invalida a leitura preparada no estado anterior. Uma política existente que nega a leitura ou a classe prevalece.

O MCP `evidence_read` retorna a projeção de evidência. Ele não oferece confirmação de leitura bruta nem aceita um argumento de consentimento como substituto do fluxo humano nativo.

## Cadastrar referências de credenciais

Abra **Referências de credenciais** nas configurações do projeto ou em **Dados por projeto**. Informe o nome, a finalidade e o tipo; depois selecione **Criar referência**.

| Tipo | O que você associa | Capacidade |
|---|---|---|
| Provedor de IA | O slot local da OpenAI | Identifica a credencial usada em novos pacotes; cada uso fixa também sua revisão. |
| Alvo — somente metadados | Um alvo ativo do catálogo | Registra finalidade e vínculo. Não configura login, cabeçalhos, cookies ou autenticação em adaptadores. |

O cadastro de referências não recebe o valor de uma chave, senha ou token. A chave da OpenAI é salva separadamente em **Configurações → IA e modelos**, no fluxo protegido existente. A referência não devolve a chave nem seu hash. Referências de alvo não recebem o estado de conexão do provedor.

Editar ou revogar uma referência preserva suas revisões anteriores. Rotação da credencial ou revogação da referência invalida revisões de envio pendentes; resultados já recebidos continuam históricos. O estado de conexão testada não substitui a revisão do pacote ou do orçamento.

## Revisar saídas

**IA.** Conversa, sugestão de operação e investigação usam o mesmo princípio: escolha evidências, fontes, categorias, sensibilidade da mensagem/pacote e referência do provedor; confira o pacote final, as instruções, o destino e o orçamento antes de confirmar. Editar a seleção ou a mensagem exige nova revisão. A investigação admite o conjunto revisado uma vez e revalida cada envio subsequente; seu contexto admitido permanece apenas na memória do host. Uma resposta ou sugestão não aprova uma ferramenta. O formato legado de `ai.context` sem operação/revisão é recusado.

**Kali.** Hash e análise de evidência exigem uma revisão de transferência separada e permissão para os bytes originais. O host confere a identidade do ambiente e os bytes antes da transferência. Segredo reconhecido nos bytes originais bloqueia essa operação: o aplicativo não redige o texto e apresenta o resultado como hash do original. Diagnóstico comum do ambiente continua separado de transferência de evidências.

**Relatório.** Em **Resumo → Cobertura e relatório**, escolha as entradas, categorias, representações de evidências, registros derivados e sensibilidade. Nenhuma evidência é selecionada por padrão. Use **Revisar pacote de relatório**, confira Markdown, manifest, hashes, tamanhos e omissões e depois **Exportar pacote revisado**. A preparação não cria arquivos. `partial=true` descreve cobertura/seleção parcial; o sucesso da gravação exige `completed=true`. O relatório mantém a omissão de conteúdo completo de antes/depois e do predicado de remediação. Metadados factuais HTTP adicionais exigem as categorias de identificação de alvos e resumo de execuções.

Se a política bloquear a exportação, a preparação mostra os motivos e mantém o conteúdo do pacote oculto. Não devolve texto Markdown, manifest, artefatos, omissões nem uma revisão de dados utilizável. A interface também descarta esses conteúdos caso os receba em uma resposta bloqueada e não oferece confirmação sensível ou exportação. Resolva o motivo do bloqueio e prepare outra revisão para consultar um pacote permitido.

**Backup.** Em **Configurações → Dados e privacidade**, escolha a sensibilidade e use **Revisar backup de recuperação**. Confira todos os projetos, inclusive arquivados, inventário, classes e exclusões antes de **Criar backup revisado**. A classificação do pacote não substitui a classificação de um dado e não libera segredo reconhecido. Esta versão não oferece backup parcial por projeto.

O backup reconstrói bancos novos a partir de registros permitidos, em vez de copiar páginas SQLite liberadas. Examina as colunas copiadas, incluindo chaves e números JSON, em busca de material reconhecido. Credenciais gerenciadas, confirmação de conexão e orçamento global ficam excluídos. A distribuição Kali, fontes externas e relatórios já exportados não integram essa cópia. A detecção de segredos tem limites e não identifica automaticamente todo dado sensível.

Uma confirmação repetida do mesmo backup recupera o recibo durável sem criar outra cópia, inclusive após reiniciar o serviço. Um recibo existente registra a tentativa anterior; ele não transforma uma falha em sucesso. Erros de gravação preservam o destino incompleto para inspeção, sem repetição automática.

## Restauração e indisponibilidade da proteção

A restauração continua sendo preparada em outra pasta, seguida do fluxo separado de ativação. Políticas restauradas ficam **inativas**. Revisões, classificações e recibos históricos permanecem, mas revisões efêmeras e contextos de envio não são restaurados. Revise a política antes de reativá-la. Configure a credencial novamente e revalide o ambiente Kali conforme o fluxo de recuperação; o orçamento monetário permanece na instalação.

Quando a proteção da credencial não pode ser verificada, o modo `metadata_only` omite texto livre e recusa bytes de conteúdo. Metadados de controle validados, consulta de estado, revogação e manutenção continuam disponíveis nos limites de cada ação. Esse modo não significa que o material foi apagado ou que uma credencial desconhecida pode ser enviada. Confira o armazenamento protegido e prepare uma nova revisão depois de resolver a indisponibilidade.

## Referência de implementação e compatibilidade

O banco principal passa da versão de schema 5 para 6. A abertura sem reconciliação preserva o JSON legado; as coleções de dados são aditivas e vazias por padrão. `Evidence`, planos e referências/provas de escopo preservam seus formatos históricos.

| Contrato | Rotas principais |
|---|---|
| Política e adoção | `data.policy.status/prepare/commit/revoke`, `data.policy.reactivate.prepare/commit`, `data.binding.prepare/commit` |
| Classificação | `data.classification.subjects/prepare/commit` |
| Referências | `credential.reference.list/create/update/revoke` |
| Leitura | `data.session.select`, `evidence.access.prepare/read`, `data.access.prepare/read` |
| Saídas | `ai.context`, `kali.data.prepare`, `report.prepare/commit`, `backup.prepare/commit` |

`report.export`, `report.bundle` e `backup.create` antigos recusam novas saídas com `data_review_required`. Clientes devem migrar para preparação e confirmação, sem reconstruir consentimentos a partir de flags legadas. Reviews normais expiram em cinco minutos e não sobrevivem ao reinício. O digest cobre seleção, versões, destino e bytes finais; IDs e instantes usados para reconstruir o pacote não são regenerados.

As coleções canônicas incluem políticas/revisões, vínculos, classificações, referências de credenciais, recibos de uso, metadados do slot, recibos idempotentes de mutação e gerações de subjects. O DTO público não devolve essas coleções completas. O expurgo inclui os metadados pertencentes ao projeto e explicita recibos de backup compartilhados; não apaga cópias externas nem o ledger monetário global.

Os limites atuais incluem 100 revisões de política por projeto, 100 identidades de referência por projeto com até 20 revisões cada, 20 classificações por subject, 10.000 recibos de uso por projeto e 64 MiB de estado canônico. Excesso é recusado; não há descarte silencioso de histórico.

Consulte o código de [política](../src-tauri/src/data_policy.rs), [projeções](../src-tauri/src/data_projection.rs), [referências](../src-tauri/src/credential_refs.rs), [adaptação dos journals](../src-tauri/src/data_host.rs), [exportação revisada](../src-tauri/src/recovery_review.rs) e [backup lógico](../src-tauri/src/recovery_data.rs). O [contrato de integração](../.local/data-policy-0.1.5-contract.md) conserva o acordo de implementação; correções posteriores e evidências ficam no [checkpoint](RETOMADA-0.1.5.md).

## Estado de validação

O checkpoint registra **372/372 testes Rust**, 104,47 s, após a correção de retenção dos relatórios novo/legado. O candidato anterior à correção visual passou **55/55 testes unitários e 181/181 E2E completos**, além de 61 gates nativos, 11 do bridge Kali real e upgrade original 5 → 6. A validação visual reprovou o resumo incompleto de adoção; por isso essas provas não liberaram a candidata.

A correção do resumo de adoção e dos rótulos de projeção possui build aprovado, **55 testes unitários, 23 E2E dirigidos e 185 E2E completos**. O coordenador conferiu os 36 hashes Rust e confirmou que o núcleo permanece inalterado. O novo debug passou **61 gates nativos e sete observações com NVDA**, incluindo as condições completas da adoção e a preparação de janela sem confirmação. O ensaio usou Speech Viewer/síntese silenciosa e navegação mista; não certifica WCAG nem um percurso inteiro só por teclado. Os helpers passaram em 33 casos de ambiente Windows, 22 de MCP Windows (oito dependem de Linux) e 30 de Linux. As evidências e diferenças entre candidatos estão no [checkpoint](RETOMADA-0.1.5.md).

O Kali real passou 11/11 no novo hash. A substituição do instalador passou com duas aberturas/seis áreas por artefato, dados preservados, sem adições estruturais ou mudança de schema; a desinstalação própria encerrou com sucesso. Esse ensaio compara candidatos 0.1.5 da mesma versão e não constitui nova prova de migração 5 → 6 do binário final. A 0.1.5 é o marco local validado, com os binários preservados e `completeRoadmap:false`; persistem os trabalhos obrigatórios da matriz geral.
