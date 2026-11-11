# Recuperação da instalação local no Windows

A ferramenta recupera o executável, os recursos e o cadastro da instalação NSIS do usuário atual. Ela usa os instaladores x64 preservados do OpenKoda. O aplicativo pode estar indisponível durante todo o procedimento.

## Antes da atualização

1. No aplicativo estável, abra **Backup → Revisar backup de recuperação**. Revise as inclusões e exclusões, conclua a cópia lógica e guarde-a identificada com a versão e a data. Esta é a cópia dos dados que poderá ser restaurada. O diário da ferramenta guarda somente uma impressão digital para comparar os dados.
2. Preserve as pastas de ambas as versões, com o instalador, o arquivo `release.json` e `receipts\packageExtraction.json`. Registre o SHA-256 de cada manifesto a partir da entrega confiável. O instalador local é sem assinatura. Guarde os pacotes no mesmo caminho até concluir a atualização e eventual retorno.
3. Copie `windows_release_recovery.py` e este guia para uma pasta de trabalho **fora da instalação do programa**. O instalador pode remover e recriar a pasta de programa. Também é possível usar a cópia externa fornecida ao lado do instalador.
4. Use Windows x64 com WebView2 já instalado e Python 3.10 ou superior disponível como `python` ou `py -3`. Feche o OpenKoda e aguarde o fim dos instaladores. Se o Kali mantiver arquivos do perfil em uso, encerre os trabalhos e pare a distribuição gerida antes de tentar novamente.
5. Para acompanhar um upgrade, a versão anterior deve ter aberto e fechado com sucesso pelo menos uma vez, criando o bloqueio nativo de perfis. A ferramenta recusa arquivos inacessíveis e pontos de reanálise. Ler todos os arquivos da base pode demorar quando ela contém um disco Kali grande.

## Revisar e aplicar

Exemplo PowerShell para uma atualização de 0.1.7 para 0.1.8. Substitua a pasta das entregas, o destino e os dois hashes conforme suas cópias de confiança. Se o Windows indicar divergência entre o destino e o registro, confira o caminho da instalação atual.

~~~powershell
$anterior = 'D:\Pacotes\OpenKoda\0.1.7-local-20261110\release.json'
$nova = 'D:\Pacotes\OpenKoda\0.1.8-local-20261110\release.json'
$hashAnterior = '95797357b4c78df6852bf1cf58342ef325fe746727bafa4c71aae983c7d40284'
$hashNova = '6adf1db72b4ec486cf21e1067976bd6462019437dd4649a8577a7e3f4b5c67b5'
$destino = "$env:LOCALAPPDATA\OpenKoda"
python .\windows_release_recovery.py plan upgrade --from-manifest $anterior --from-sha256 $hashAnterior --to-manifest $nova --to-sha256 $hashNova --destination $destino
~~~

Confira `operation`, `fromVersion`, `toVersion`, `destination` e os hashes dos instaladores no resultado. O comando seguinte executa o plano exato e também serve para retomá-lo após interrupção. Copie o valor completo de `id`.

~~~powershell
python .\windows_release_recovery.py apply --confirm ID_COMPLETO_DO_PLANO
python .\windows_release_recovery.py status
~~~

A ferramenta compara o executável empacotado, a versão registrada, o destino e os dados do usuário antes de emitir o recibo. O diário fica em `%LOCALAPPDATA%\OpenKoda.ReleaseRecovery`. O script utiliza o modo silencioso com preservação das informações de atualização. Não inicia o aplicativo ao terminar. Abra o executável da pasta revisada quando estiver pronto.

| Situação | Operação a revisar |
| --- | --- |
| Primeira instalação acompanhada | Use `plan install` com o mesmo manifesto e hash de ambos os lados. O destino padrão deve estar vazio. A retomada do plano funciona mesmo que a primeira instalação pare antes de registrar a versão. |
| Programa instalado com executável ou recurso faltando | Use `plan repair` com a versão atualmente registrada nos dois lados. O NSIS reinstala os arquivos no destino revisado. |
| Instalação manual interrompida antes de registrar | O reparo reconhece o executável exato do pacote no destino padrão. Quando ele também falta ou diverge, retome o diário da instalação acompanhada, se existir. Uma pasta sem identidade verificável é preservada para inspeção. |
| Retorno após atualização acompanhada | Use `plan rollback`, com a versão mais nova em `--from-manifest` e a anterior em `--to-manifest`, e os hashes correspondentes. O instalador anterior só é executado após remoção verificada da versão mais nova. |
| Interrupção após aplicar | Consulte `status`, encerre processos de instalação ainda ativos e repita `apply --confirm` com o mesmo identificador. A ferramenta consulta o cadastro e retoma a fase persistida. A conclusão exige a releitura do executável instalado. |
| Interrupção seguida de alteração do perfil | Prepare `repair` usando a **mais nova** das duas versões nos dois lados e revise o novo identificador. Esse reparo pode substituir o diário pendente e recuperar a instalação compatível com os dados atuais. |

O último upgrade acompanhado cria o ponto de comparação para aquele destino e aquele par de pacotes. A ferramenta confere toda a base `%USERPROFILE%\.openkoda` e a base legada `%APPDATA%\com.openkoda.desktop`, incluindo bancos, arquivos auxiliares e diretórios. O arquivo de exclusão mútua `profiles.lock` fica reservado durante a operação. Entradas, remoções ou alterações de conteúdo bloqueiam o retorno. Arquivos pessoais estranhos na pasta do programa também interrompem a operação. O mecanismo atende aos pacotes locais de schema 8 das versões 0.1.6 a 0.1.9 com recibo compatível. A instalação padrão do OpenKoda continua com a proteção contra downgrade.

## Quando a versão nova já gravou dados

Continue com o binário mais novo ou repare-o. Faça **outra cópia lógica** no aplicativo mais novo e identifique-a como posterior ao upgrade. Guarde o backup anterior separadamente. Nunca sobrescreva o anterior com a cópia posterior.

Para consultar o conteúdo anterior **na mesma conta Windows**, abra a recuperação da versão mais nova. Na cópia anterior, selecione **Verificar backup** e **Preparar restauração**. Isso cria um perfil inativo. Revise os dados e as políticas antes de usar **Agendar ativação revisada** para a próxima abertura do aplicativo. O perfil atual e a cópia feita depois do upgrade ficam preservados. Esse procedimento utiliza o binário mais novo em ambos os perfis.

Para executar **o binário anterior** com o backup anterior após os dados terem mudado, use outra conta Windows sem instalação do OpenKoda ou uma VM descartável. Instale lá a versão que criou a cópia e abra-a uma vez para preparar a base. Feche o aplicativo. No perfil inicial dessa conta nova, copie a pasta completa do backup pré-upgrade para `%USERPROFILE%\.openkoda\backups\<id-do-backup>`. Mantenha o identificador da pasta e seu `manifest.json`, sem substituir outra cópia. Ao reabrir, use **Verificar backup** e **Preparar restauração**. O aplicativo confere identificador, versão do formato, integridade e conteúdo dos bancos antes de criar o perfil inativo. Os dados pós-upgrade permanecem com a versão mais nova na conta original. Sem outro ambiente disponível, mantenha a versão mais nova e consulte a cópia anterior em perfil separado.

Credenciais protegidas pelo Windows, orçamento global, arquivos externos e distribuição Kali têm recuperação própria. A cópia lógica exibe as exclusões durante a revisão. Quando a versão tiver exportação de Kali, preserve também o TAR e seu manifesto conforme o guia do ambiente. Não troque nomes de pastas de perfil, bancos SQLite ou discos registrados no WSL para forçar um retorno. Uma restauração lógica gera um novo perfil e não substitui o requisito de igualdade dos dados para o rollback do binário.

## Verificação da ferramenta

A suíte `scripts/test_windows_release_recovery.py` cria exclusivamente perfis, arquivos NSIS simulados e registros em arquivos descartáveis. Execute `python -B scripts/test_windows_release_recovery.py` na árvore de fontes. O ensaio separado `scripts/windows_recovery_nsis_smoke.py` usa instaladores reais, destino exclusivo e dados sintéticos. Ele exige ausência de registro, atalho e inicialização pré-existentes, confere o conteúdo instalado e nunca abre o OpenKoda release. O ensaio distingue a interrupção coordenada entre operações de uma queda do Windows durante a escrita interna do processo NSIS.

