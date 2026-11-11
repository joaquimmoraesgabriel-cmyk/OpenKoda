# Preparar o Windows e retirar um executor Kali no OpenKoda 0.1.10

## Preparar o Windows

1. Abra **Kali e ferramentas → Ambiente**. Em **Preparar o Windows para o WSL**, selecione **Verificar requisitos do Windows**. O OpenKoda salva o início do acompanhamento e consulta o Windows. A tela mostra a arquitetura, o build, a versão do WSL, as informações de virtualização, a quantidade de distribuições registradas para o usuário e o espaço livre no volume de destino.
2. Siga a orientação exibida. Esta edição usa uma imagem Kali x64. O fluxo guiado de instalação do WSL requer Windows 10 build 19041 ou posterior, ou Windows 11. Um campo **Não verificado** indica que a consulta falhou ou que o Windows não forneceu a informação.
3. Se o painel indicar que é preciso preparar o WSL, um administrador pode executar `wsl --install --no-distribution` em um PowerShell elevado. Esse comando prepara o WSL sem instalar uma distribuição padrão. Se a orientação for atualizar o WSL, `wsl --update` atualiza o componente compartilhado pelas distribuições da máquina. A execução desses comandos e o reinício solicitado pelo Windows são manuais. [Instalação do WSL](https://learn.microsoft.com/pt-br/windows/wsl/install) · [Comandos do WSL](https://learn.microsoft.com/pt-br/windows/wsl/basic-commands)
4. Se já houver uma distribuição pessoal, preserve seu registro. Consulte `wsl --status` para verificar o estado do WSL e `wsl --list --verbose` para consultar os nomes, estados e versões das distribuições. Se a **Plataforma de Máquina Virtual** estiver desabilitada, peça ao administrador para conferir o recurso do Windows. [Comandos de consulta da Microsoft](https://learn.microsoft.com/pt-br/windows/wsl/basic-commands)
5. Após retornar ao Windows, abra novamente **Kali e ferramentas → Ambiente**. Um acompanhamento já iniciado faz nova consulta. Confira **Verificação atual**, **Reinício observado desde o início** e a orientação para o próximo passo. Use **Verificar novamente os requisitos** depois de qualquer ajuste.
6. Quando o painel indicar suporte à instalação dedicada por nome e destino, use **Revisar preparação do Kali**. Confira a origem, o destino e os efeitos em **Manutenção**, marque a autorização e selecione **Executar operação revisada** para iniciar a preparação. Se já existir uma distro com o nome reservado sem vínculo com o OpenKoda, preserve-a e confira a identidade em **Ambiente**.

## Preservar os dados da distribuição

O registro WSL identifica uma distribuição e seu armazenamento Linux. A remoção desse registro pelo Windows pode apagar os arquivos, pacotes e configurações da distribuição selecionada. Os projetos e avaliações do OpenKoda têm um backup próprio em **Configurações → Dados e privacidade**.

Antes de desativar um executor que contenha arquivos a guardar, vá a **Kali e ferramentas → Manutenção → Cópias do Kali** e selecione **Revisar exportação do Kali**. Confira o destino, o tamanho permitido e a distribuição de origem. A confirmação encerra a distribuição gerida e grava uma cópia no Windows. Guarde o arquivo e seu recibo. Para conferir um arquivo importante, restaure uma cópia separada e abra **Acessar os arquivos recuperados**. [Como exportar e restaurar o Kali](KALI-BACKUP.md)

## Retirar o executor selecionado

1. Conclua ou reconcilie os trabalhos ativos de avaliação e manutenção. Em **Kali e ferramentas → Manutenção → Retirar a distribuição Kali**, selecione **Revisar retirada do Kali**. O OpenKoda confere o registro WSL e pode iniciar a distribuição para verificar o marcador e o usuário gerido.
2. Compare **Distribuição**, **Identidade do ambiente**, **Registro WSL** e **Local registrado** com o executor que deseja retirar. A revisão pode apontar para o Kali original ou para uma cópia restaurada que você selecionou como executor. Se o destino estiver incorreto, use **Descartar revisão**.
3. Digite exatamente o nome exibido, marque a confirmação do efeito sobre os dados Linux e selecione **Desativar este executor**. O aplicativo registra a retirada e bloqueia novas operações nessa identidade. Você pode fechar e reabrir o painel e usar **Consultar retirada registrada** para recuperar o recibo.

### Manter a distribuição no Windows

Abra **Preservar a distribuição**, marque **Quero concluir a retirada local e preservar o que existe no Windows** e selecione **Encerrar vínculo local**. Use essa opção também se outra distribuição estiver ocupando o nome ou o local anterior. O recibo registra o encerramento local. A distribuição presente e seus arquivos ficam sob controle do usuário do Windows. O OpenKoda usa uma identidade nova em uma futura preparação. Se o nome ou a pasta dedicada continuar ocupado, a preparação informa o conflito e preserva o conteúdo existente.

### Excluir a distribuição pelo Windows

1. Selecione **Conferir registro no Windows**. Quando a identidade conferir, o painel exibirá uma consulta PowerShell ao GUID revisado. Execute-a na mesma conta do Windows e confira os valores atuais de `DistributionName` e `BasePath`. Execute o comando de remoção exibido somente se os dois valores ainda corresponderem à distribuição da revisão e você tiver decidido dispensar seus dados Linux. [Efeito de cancelar o registro de uma distribuição](https://learn.microsoft.com/pt-br/windows/wsl/basic-commands#unregister-or-uninstall-a-linux-distribution)
2. Volte ao OpenKoda e use **Conferir registro no Windows**. O recibo distingue um registro ainda presente, uma identidade divergente e a ausência do registro verificada no inventário. Também indica se o caminho anterior ainda existia na conferência. Preserve qualquer distribuição com nome, GUID ou caminho divergente. Se restarem arquivos, identifique-os no Windows antes de decidir seu destino.

O guia da versão anterior descreve a atualização do aplicativo Windows, o suporte local e a proteção de trabalho ativo ao fechar a janela. [Guia da 0.1.9](GUIA-0.1.9.md)
