# Cópias do ambiente Kali

Em **Kali e ferramentas → Manutenção → Cópias do Kali**, escolha **Revisar exportação do Kali**. Confira a origem, o formato TAR, o destino e o limite de tamanho. Ao confirmar, o OpenKoda encerra a distribuição gerida e usa o exportador do WSL para gravar a cópia. O aplicativo confere os bytes gravados e o SHA-256 antes de disponibilizá-la para restauração. Na instalação padrão, o arquivo fica em `%USERPROFILE%\.openkoda\kali-backups`.

O TAR guarda os arquivos que o WSL exporta da distribuição, incluindo ferramentas, configurações, registros de sessões e evidências presentes. A confirmação abrange todos esses arquivos. A cópia é local, sem redação ou criptografia adicional. O OpenKoda inspeciona as capturas que ele próprio gerencia. Se houver bytes de captura pendentes, abra **Configurações → Dados e privacidade** e revise sua retenção antes de exportar. Recibos terminais podem permanecer entre os arquivos copiados.

Avaliações e projetos usam o backup de dados em Configurações. Para recuperar esses dados e o ambiente Kali, guarde os dois backups. A restauração do Kali pela interface exige o arquivo exportado e seu recibo local, registrado em `environment-archives.sqlite3`.

## Restaurar uma cópia

1. Use **Consultar cópias do Kali** e escolha **Revisar restauração desta cópia**. O aplicativo verifica novamente o arquivo registrado e apresenta o formato, tamanho, SHA-256, nome da nova distribuição WSL e destino. Os recibos antigos de disco aparecem como **VHDX**. Esse formato inclui o disco virtual inteiro, inclusive espaço livre que pode conter dados excluídos.
2. Confirme a importação. O aplicativo cria uma distribuição WSL 2 separada, confere o registro e atribui uma nova identidade à cópia. Após verificar a parada, o histórico mostra **Cópia restaurada e inativa**.
3. Abra **Acessar os arquivos recuperados** para ver o comando PowerShell que inicia essa cópia como o usuário Linux comum `openkoda`. Use `exit` ao concluir.

A restauração usa o recibo e o arquivo da cópia mesmo que a origem já esteja indisponível. Quando o WSL não tem nenhuma distribuição registrada, a revisão informa que a primeira distribuição importada pode se tornar a padrão do Windows. A distribuição de recuperação tem nome próprio. A troca do executor exige uma confirmação separada.

## Usar uma cópia restaurada como executor

1. Em **Consultar cópias do Kali**, localize a importação verificada. O histórico mostra até 128 tentativas por página, das mais recentes às mais antigas. Use **Tentativas mais antigas** e **Tentativas mais recentes** para percorrer as páginas. O OpenKoda conserva os recibos e os arquivos das páginas anteriores.
2. Selecione **Revisar uso desta cópia como executor**. Compare os nomes e as identidades do executor atual e do proposto. Confira o local da cópia, o registro WSL, os efeitos e a validade da revisão. Abrir a revisão consulta o registro do Windows.
3. Para prosseguir, marque a autorização e use **Confirmar troca de executor**. O OpenKoda inicia a cópia para verificar sua identidade e só então atualiza o vínculo. A troca preserva as distribuições, os arquivos, a distribuição padrão do Windows e as evidências. Planos e permissões anteriores continuam ligados à identidade que os recebeu.
4. O histórico identifica a **Cópia vinculada ao executor**. Abra **Ambiente** para consultar o diagnóstico da distribuição agora vinculada. Uma cópia anteriormente usada aparece como **Vínculo anterior do executor**. Para obter uma nova identidade a partir de um arquivo verificado, faça uma nova restauração.

Se a revisão expirar, abra outra e confira os detalhes. Se a resposta da confirmação se perder, consulte o histórico para descobrir qual cópia está vinculada antes de decidir a próxima ação.

## Falha e retomada

Se o encerramento da origem perder confirmação, abra **Consultar cópias do Kali → Conferir se a origem parou**. Essa inspeção consulta o registro WSL e as distribuições em execução. A operação anterior permanece incompleta. Uma nova exportação exige nova revisão.

Falhas de gravação, hash, importação ou identidade deixam a tentativa identificada no histórico. Uma cópia incompleta não é oferecida para importação. Em caso de registro incerto do clone, confira o nome e o local registrados antes de tratar os arquivos da tentativa. O aplicativo preserva a origem e os resíduos para diagnóstico.

O limite de exportação é 64 GiB por TAR. Reserve espaço para o arquivo no destino e, na restauração, para a nova distribuição. O fluxo requer Windows com WSL 2. A restauração de VHDX anterior requer suporte a `wsl --import --vhd`.
