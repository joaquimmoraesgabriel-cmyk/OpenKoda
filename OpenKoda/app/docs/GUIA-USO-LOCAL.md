# Usar a versão local do OpenKoda

Este guia acompanha a **0.1.5 local**, validada nos limites do [checkpoint](RETOMADA-0.1.5.md), com dados em `%USERPROFILE%\.openkoda`. O [instalador preservado](../releases/0.1.5-local-20261108/OpenKoda_0.1.5_x64-setup.exe) é local e sem assinatura, conforme a escolha do usuário; seu hash está no [manifesto](../releases/0.1.5-local-20261108/release.json). Os guias anteriores, incluindo o [da 0.1.4](../releases/0.1.4-local-20261108/GUIA-USO-LOCAL.md), continuam preservados.

A 0.1.5 passou Rust372, frontend55/185, nativo61, Kali real11, NVDA7 delimitado e a substituição do pacote corrigido com dados preservados. O upgrade original 0.1.4 → 0.1.5 comprovou schema 5 → 6 no candidato anterior. O ensaio final substituiu candidatos 0.1.5 da mesma versão, com duas aberturas/seis áreas em cada artefato, sem outra migração. Não abra o perfil já migrado com um binário antigo de schema 5. **Esse marco local não conclui o OpenKoda completo**; os limites e próximos blocos permanecem em [IMPLEMENTACAO](../IMPLEMENTACAO.md).

O **incremento local 0.1.4 foi concluído nos limites testados**: interface 50/162 com IPC simulado, Rust 332, 55 gates nativos em perfis isolados, nove gates do bridge Kali V2 real, 30 testes do helper Linux, NVDA 6 delimitado e upgrade positivo. Isso não conclui o produto completo. Os binários estão preservados. Guia e ZIP de fontes da 0.1.4 preservados; hashes registrados no manifesto da release.

Use `Ctrl` + `+` e `Ctrl` + `-` para ajustar o zoom, `Ctrl` + `0` para restaurar a escala e `Tab`/`Enter` para navegar e ativar controles. A verificação de zoom nativo não substitui testes com leitor de tela.

Na 0.1.5, o ensaio com NVDA conferiu condições da adoção, referências, avisos de projeção, origem/prévia e preparação da janela de leitura. A revisão foi fechada e a janela não foi confirmada; domínio/fontes permaneceram intactos. Usou Speech Viewer e síntese silenciosa, mouse/rolagem/teclado e foco visual/anunciado porque o campo UIA ficou desatualizado. O [recibo](../.local/screen-reader-data-a3e7ce17-090b-455b-a346-cc6a192ed7d3/result.json) não certifica avaliação auditiva, percurso inteiro só por teclado ou WCAG.

Na 0.1.4, seis observações de objetivo, entradas/métodos, exclusão, revisão, origem manual e contexto IA passaram com NVDA 2026.3/Speech Viewer e síntese silenciosa, usando mouse, rolagem e teclado. Campos UIA de foco ficaram desatualizados em capturas; o ensaio se baseou nas telas e no Speech Viewer. Não comprova anúncio automático de avisos, percurso inteiro só por teclado, avaliação auditiva ou WCAG. Os controles inválidos continuam desabilitados até corrigir os campos.

## Preparar

1. Abra **Configurações** e confira o modelo e o estado da credencial. A chave salva não é exibida de volta. **Testar conexão** faz uma chamada pequena e pode consumir créditos.
2. Abra **Kali e ferramentas** para verificar o ambiente gerido. A instalação de um pacote não significa que exista um adaptador para executá-lo. Preparação e manutenção exigem planos próprios.
3. Crie um Projeto com um alvo que você autoriza avaliar e, depois, uma Avaliação. As modalidades atuais são código, arquivos, web e rede. Projetos arquivados mantêm histórico, mas não aceitam novas operações.

## Revisar o escopo da avaliação

Este fluxo passou na regressão nativa da 0.1.4. Os guias das versões anteriores continuam preservados em suas pastas de release.

Ao criar a avaliação, preencha o objetivo, escolha os métodos permitidos e revise a janela de autorização. Os métodos começam desmarcados. Deixar a lista vazia permite somente trabalho local com dados armazenados, sem adquirir dados de alvo ou chamar o provedor. Deixar as datas vazias significa não estabelecer limite de data; não significa aprovar operações. Capturar fontes exige também o método de análise local. A revisão de IA autoriza esse tipo de trabalho no escopo, mas cada envio ainda exige seleção e consentimento de contexto, além da revisão financeira quando habilitada.

Use **Revisar avaliação**, confira objetivo, alvo, métodos e janela, marque o reconhecimento e escolha **Salvar avaliação com escopo revisado**. **Salvar rascunho sem revisar escopo** conserva a intenção sem autorizar trabalho; será necessário revisar o escopo antes de continuar. Nenhuma dessas ações executa o plano automaticamente.

Antes do primeiro trabalho, é possível revisar objetivo, métodos e janela. Preparar trabalho congela a revisão utilizada. Depois disso, use a derivação para abrir outra avaliação no mesmo projeto e alvo, escolhendo quais configurações copiar. O rascunho derivado precisa de revisão própria e não herda evidências, achados, aprovações nem saldo novo na instalação.

Avaliações criadas nas versões anteriores continuam com histórico intacto. A adoção de escopo registra condições para trabalho futuro, sem alterar objetivo, provas ou aprovações antigas. Prepare novos planos depois da adoção. Uma correção textual antiga já aplicada pode receber um novo reteste autorizado, sem reaplicar o patch.

Revogar impede novas admissões. A tela mantém visível o trabalho já iniciado; o cancelamento ou a reconciliação de cada operação continua separado. Resultados e consumo de uma chamada já enviada não são apagados. Histórico e revisão factual das evidências armazenadas permanecem acessíveis. Novas operações são recusadas antes da abertura da janela, após seu término ou quando o método necessário não foi selecionado. Consulte [Escopo da avaliação](SCOPE.md) para os limites de prazo de cada método; a janela não é um agendamento automático.

## Fontes combinadas

Em **Projetos**, abra **Fontes e alvos do projeto** para cadastrar entradas adicionais. O cadastro não as inclui em avaliações. Inativar impede novas seleções, mas não revoga um escopo já revisado; a retirada dessa autorização é feita na avaliação correspondente.

Na avaliação combinada, selecione explicitamente as entradas, os métodos de cada uma e as exclusões aplicáveis. O alvo mostrado como referência de apresentação não substitui essa escolha. Antes de preparar uma operação, escolha sua entrada; o plano conserva esse vínculo para revisão e aprovação.

Use exclusões relativas para arquivos ou subárvores de uma raiz local selecionada. Se houver uma raiz local inteira explicitamente excluída, operações locais são bloqueadas porque o aplicativo não pode confirmar que ela não é um alias sem acessar seus metadados. Isso não bloqueia, por si só, operações web. Exclusões de conexão usam IPs ou redes literais e portas opcionais.

Ao importar uma evidência, associe-a às entradas pertinentes. O rótulo de associação declarada não confirma a veracidade do texto. Na revisão por IA, escolha as entradas do contexto e as evidências; todas as origens contribuintes precisam estar incluídas e permitir esse método. Conferir a origem não confirma uma vulnerabilidade nem autoriza o envio. Uma sugestão de operação usa a entrada escolhida pela pessoa e passa depois pela preparação normal.

A cobertura aparece por entrada e método. Uma saída parcial, arquivo omitido ou tentativa sem resultado não significa avaliação completa. Revogar bloqueia novas admissões; para mudar um escopo congelado, derive outra avaliação e faça uma revisão própria. Os contratos e limites estão em [Alvos e origens](COMBINED-SCOPE.md).

## Preparar dados na 0.1.5

Esses controles complementam a revisão de escopo, a aprovação da operação e o orçamento. Uma classificação de sensibilidade ou uma origem associada não concede autoridade para acessar um alvo. O [guia de dados](DATA-POLICY.md) detalha cada estado e limite.

1. Em **Projetos → Configurações → Política de dados do projeto**, escolha os destinos, classes, categorias e fontes permitidas. Revise leitura local e backup separadamente e confirme a política revisada. Em **Configurações → Dados e privacidade → Dados por projeto**, você também encontra projetos arquivados.
2. Na avaliação, use **Resumo → Política de dados desta avaliação** para revisar a adoção. Confira a identidade da revisão, destinos permitidos/bloqueados, classes, categorias, fontes, permissão de bytes originais ao Kali, leitura local e backup antes de confirmar. Seleções de um destino bloqueado não autorizam uso. A adoção não é automática; depois do primeiro uso admitido, o vínculo congela. Restringir o projeto reduz as permissões futuras, e uma ampliação exige outra avaliação com adoção própria.
3. Em **Classificação e origem dos dados**, selecione o registro, informe sua classe e o motivo e confirme a revisão. Material não classificado não ganha permissão de saída. Associe fontes do escopo quando a origem for desconhecida; a associação continua declarada pelo usuário, sem comprovar origem factual ou veracidade.
4. Em **Referências de credenciais**, associe o slot local da OpenAI ou cadastre uma referência de alvo. Não digite o segredo na referência. A chave é configurada separadamente em **Configurações → IA e modelos**; referência de alvo registra somente metadados e não configura autenticação.

Para ler conteúdo armazenado, abra a evidência ou o registro derivado e selecione **Revisar trecho armazenado**. Confira a classe e o intervalo e use **Confirmar leitura desta janela**. Cada janela tem até 32.768 bytes UTF-8 após redação; **Ler próximo trecho** exige nova confirmação. A prévia da listagem é limitada a 2 KiB e não substitui essa leitura. Hash do original e hash da representação são distintos. Alteração, troca de avaliação ou expiração exige outra revisão.

Na conversa, sugestão ou investigação de IA, selecione evidências, fontes, categorias, sensibilidade e referência do provedor. Confira o pacote exato, instruções, destino e orçamento antes de confirmar. Dados sensíveis exigem reconhecimento específico. Mudar o pacote ou a credencial invalida a revisão; resposta do modelo não aprova execução. No Kali, confirme separadamente a transferência dos bytes originais e o plano da operação. A política precisa permitir esses bytes; material secreto reconhecido bloqueia a transferência. Diagnóstico do ambiente permanece um fluxo separado.

Em **Resumo → Cobertura e relatório**, selecione o conteúdo e use **Revisar pacote de relatório**. Confira Markdown, manifest, tamanhos, hashes e omissões antes de **Exportar pacote revisado**. Uma política que bloqueia o pacote também oculta sua prévia; prepare outra revisão após resolver os motivos. Uma exportação concluída pode continuar parcial em cobertura. Ela cria uma cópia local, sem publicação automática.

Em **Configurações → Dados e privacidade**, escolha a sensibilidade e use **Revisar backup de recuperação**. O pacote abrange todos os projetos, inclusive arquivados; confirme o inventário e as exclusões antes de criar a cópia. Vault, teste de conexão, orçamento global, disco Kali, fontes externas e relatórios já exportados ficam fora. A restauração prepara outra pasta e conserva o histórico, mas suas políticas ficam **inativas**. Revise-as para reativar; configure novamente a credencial e revalide o Kali. Restaurar não renova o orçamento da instalação.

Se a proteção da credencial ficar indisponível, a interface mantém metadados validados e bloqueia conteúdo no modo `metadata_only`. Resolva o armazenamento protegido e prepare novas revisões. Isso não apaga as evidências nem autoriza enviar segredos desconhecidos. Desativar a política preserva efeitos históricos e bloqueia novos usos, inclusive a leitura por janela sob uma política inativa.

## Controlar o orçamento da IA

Em **Configurações**, informe os limites em USD por instalação e avaliação e as tarifas de entrada/saída por milhão de tokens para cada modelo. Os preços são manuais; registre de onde vieram. Revise a política e o tratamento do histórico sem preço antes de salvar. O controle começa desativado e essa condição aparece nos fluxos de envio.

Quando ativado, revise a estimativa exibida em cada conversa, teste de conexão, proposta de operação ou investigação. A revisão vale por 15 minutos e perde validade se o conteúdo, a chave, o modelo ou a política mudar. Sem saldo suficiente, a solicitação é recusada antes do envio. As estimativas locais não garantem o valor máximo da fatura do provedor.

Uma interrupção pode deixar o custo desconhecido e reter a reserva. Em **Custos desconhecidos**, revise o registro e, quando apropriado, registre um débito conservador com justificativa; isso não declara conhecida a cobrança real. Reconhecer a interrupção no histórico da conversa não libera o orçamento. Restaurar um perfil ou excluir um projeto também não renova o saldo. Se o registro financeiro ficar indisponível, o histórico permanece consultável e novos envios são bloqueados. Consulte os limites em [Orçamento da IA](AI-BUDGET.md).

## Observar e analisar

No detalhe da Avaliação, prepare a operação correspondente e revise o plano antes de aprovar. Inventário e análise textual local leem o alvo selecionado; HTTP observa uma URL com HEAD; TCP conecta apenas aos IPs e portas revisados, sem payload. Nenhuma dessas verificações declara que um alvo está livre de vulnerabilidades.

Para disponibilizar código à IA:

1. Prepare um plano de análise local. Em **Capturar fontes selecionadas**, escolha explicitamente os arquivos do manifest: até oito arquivos UTF-8 e 32 KiB no total.
2. Gere a prévia e revise todo o conteúdo redigido. A redação automática pode deixar dados sensíveis; nenhuma alteração detectada não significa ausência de segredos.
3. Confirme **Capturar cópia local revisada**. Isso cria uma evidência local e não envia conteúdo ao provedor.
4. Na conversa, investigação ou sugestão de operação, selecione essa evidência, confira o contexto e autorize separadamente o envio ao modelo. Selecionar arquivos não concede autorização permanente para envios futuros.

**Sugerir próxima operação com IA** permite escolher quais adaptadores o modelo poderá sugerir. A resposta só recomenda uma opção; **Revisar operação sugerida** abre a preparação/revisão normal. Ainda é preciso conferir parâmetros e aprovar a execução. A investigação usa até três perspectivas independentes do mesmo contexto, com limites de tokens e registro de consumo; esses limites não constituem um teto financeiro de cobrança.

O parser Kali recebe somente os bytes da evidência escolhida após confirmação local. Suas regras fixas não recebem caminhos, comandos ou expressões regulares livres. Sinais lexicais permanecem candidatos; valide a condição e suas evidências antes de tratá-la como achado.

## Corrigir e registrar

Para uma avaliação web, execute a HEAD aprovada e abra **Revisar controle HTTP**. A regra atual verifica somente `X-Content-Type-Options: nosniff` na resposta registrada. Se o cabeçalho estiver ausente nas condições suportadas, revise os fatos e limites para criar o candidato. Em **Achados**, a validação exige justificativa e confirmação humana.

Depois de uma alteração feita no servidor pela pessoa responsável, prepare e aprove uma nova HEAD para a mesma URL. No achado, compare e registre o reteste. Uma única ocorrência válida de `nosniff`, com alvo e peer comparáveis, permite a confirmação humana separada. Duplicatas, redirects, falhas ou diferenças relevantes deixam o resultado inconclusivo. O aplicativo não altera a configuração do servidor nem comprova exploração; o resultado vale para a resposta e o instante observados. Consulte [Validação HTTP](HTTP-VALIDATION.md).

Para um achado validado em código/arquivos, a correção atual revisa um arquivo UTF-8 de até 256 KiB. Confira a base e o conteúdo proposto, aprove, aplique e reteste. O reteste textual/hash comprova somente a condição revisada. A confirmação de correção é uma ação humana separada; mudança concorrente do arquivo invalida a revisão.

Exporte o relatório e confira a cobertura parcial, limitações e dados sensíveis. Exportar cria uma cópia local; não publica nem envia o relatório automaticamente.

## Retomar e preservar

- Se uma operação terminar como desconhecida, inspecione/reconcilie seu registro. Não presuma que o efeito foi desfeito nem repita uma escrita automaticamente.
- Faça backup antes de mudanças relevantes. A restauração prepara outra pasta; a ativação é revisada e ocorre no reinício, preservando o perfil anterior. Credenciais e disco da distro Kali não fazem parte desse backup.
- A retenção de backups/relatórios exige seleção e confirmação próprias. Arquivar um projeto não apaga seu histórico. A exclusão definitiva de um projeto arquivado possui outro fluxo, com dependências e possibilidade de término parcial explicitadas.
- Desinstalar o programa preserva os dados e a distro gerida. Não há remoção implícita de fontes, cópias externas ou dados do provedor.

As capacidades ainda não entregues e os ensaios pendentes permanecem na [matriz de aceitação](ACCEPTANCE-STATUS.md). Não use a quantidade de testes como declaração de cobertura de todas as modalidades de segurança.
