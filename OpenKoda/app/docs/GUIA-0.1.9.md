# Usar o OpenKoda 0.1.9 no Windows

## Instalar ou atualizar

Para instalar, execute `OpenKoda_0.1.9_x64-setup.exe` na pasta da entrega. O pacote é local, x64 e sem assinatura. Se já usa uma versão anterior, feche os trabalhos em andamento e guarde uma cópia em **Configurações → Dados e privacidade**. A abertura de um perfil antigo pode atualizar seus bancos. Para acompanhar a atualização com possibilidade de retomar uma interrupção ou voltar ao binário anterior, prepare o plano antes do upgrade. O procedimento, as condições de retorno e os comandos estão no [guia de recuperação do Windows](RECUPERACAO-WINDOWS.md).

## Retomar o trabalho

Use **Projetos** para registrar o alvo e **Nova avaliação** para definir o objetivo e o escopo. Abra **Kali e ferramentas → Ambiente** para conferir o executor. Em **Configurações → IA e modelos**, informe sua chave e teste a conexão quando quiser habilitar o provedor. O teste de conexão faz uma requisição ao provedor. As avaliações, a fila e a revisão das operações estão no [guia de uso](GUIA-0.1.6.md). O [guia da 0.1.8](GUIA-0.1.8.md) descreve propostas de revisão de plano e reteste de configurações JSON.

## Usar uma cópia do Kali

Em **Kali e ferramentas → Ambiente**, consulte as cópias, selecione a restauração e escolha **Revisar uso desta cópia como executor**. A tela apresenta a origem e o destino proposto. Ao confirmar, o núcleo verifica o registro do Windows e o usuário Linux da cópia. Os planos preparados para outra identidade exigem nova revisão. Para consultar um recibo antigo, use os controles de página na lista de tentativas. [Cópias, restauração e troca de executor](KALI-BACKUP.md)

## Fechar a janela com trabalho em andamento

Ao clicar em **X** durante uma operação em curso, o OpenKoda mantém a janela aberta. Escolha **Manter aberto** para continuar onde estava ou use o botão de revisão para ver a fila, a avaliação ou a operação indicada. Na fila, confira se a operação oferece pausa ou cancelamento e aguarde o resultado. Quando o trabalho terminar, você pode fechar a janela. Após uma queda do processo ou do Windows, consulte o estado de recuperação antes de iniciar outra tentativa.

## Gerar um arquivo de suporte

1. Abra **Configurações → Dados e privacidade → Diagnóstico e suporte**.
2. Selecione **Preparar prévia do pacote de suporte**. Leia o JSON com versões, plataforma, arquitetura e contagens de arquivos de banco presentes, ausentes ou indisponíveis. A tela também mostra o tamanho, o destino e o SHA-256.
3. Para gravar os bytes apresentados, marque a confirmação e use **Salvar pacote de suporte**. A prévia vale por dez minutos. Você também pode descartá-la.
4. Localize o arquivo `suporte-….json` na pasta `exports` do perfil ativo. O perfil inicial usa `%USERPROFILE%\.openkoda`. Perfis restaurados usam `%USERPROFILE%\.openkoda\profiles\<id>`. Compartilhar o arquivo com alguém é uma ação manual. A telemetria do OpenKoda vem desativada.

O [registro da versão](RETOMADA-0.1.9.md) identifica os testes executados e as condições de uso verificadas.
