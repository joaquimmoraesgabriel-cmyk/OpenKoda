# Recuperar espaço de captura no Kali

A versão 0.1.7 acrescenta a remoção revisada dos metadados deixados por uma captura. O Kali reserva até 256 entradas para capturas. Mesmo depois de remover os bytes de uma captura, seu diretório ocupa uma entrada até a consolidação dos metadados.

1. Abra **Configurações → Dados e privacidade** e atualize o inventário de retenção. Para remover a saída de uma operação do ambiente Linux, selecione a **Captura no Kali** correspondente, prepare o plano e confira a captura e o ambiente antes de confirmar.
2. Após a remoção dos bytes, atualize o inventário e selecione **Metadados de captura no Kali**. Abra a revisão para saber quantos arquivos e bytes ainda existem. A seção **Identidade dos metadados no Kali** apresenta o produtor, o manifesto e o recibo anterior.
3. Confira a lista de arquivos. Se a inspeção identificar uma gravação interrompida do produtor, a revisão mostra o temporário reconhecido, seu tamanho e sua identidade. Para prosseguir, marque o aceite, digite a frase mostrada na tela e confirme a exclusão. **Fechar revisão da exclusão** devolve o foco à preparação ou à atualização do inventário.
4. Consulte o resultado. Se aparecer **Metadados consolidados; slot liberado**, o diretório que ocupava aquela entrada foi removido. O recibo da operação permanece no histórico. Outras capturas e a cópia local no Windows têm seleções próprias.

Se a sessão for interrompida, abra o plano no **Histórico de exclusões** e use **Inspecionar resultado sem excluir**. Quando a inspeção encontra arquivos restantes, prepare um novo plano para eles. A inspeção também pode confirmar que a remoção anterior já terminou. Uma tentativa ainda desconhecida mantém o expurgo do projeto bloqueado.

A consolidação depende do vínculo com o perfil que criou a captura. Uma cópia restaurada em outro perfil conserva o histórico. A autoridade para operar aquele diretório continua no perfil de origem. O limite global de 10.000 registros do bridge permanece em vigor.

Para preservar a possibilidade de retornar à 0.1.6, guarde o backup anterior ao primeiro uso dos novos controles de retenção. A 0.1.6 antecede os registros de consolidação da 0.1.7.

Os demais percursos de avaliação, IA, fila, backup e recuperação estão no [guia da 0.1.6](GUIA-0.1.6.md). O estado da validação do pacote está no [registro da 0.1.7](RETOMADA-0.1.7.md).
