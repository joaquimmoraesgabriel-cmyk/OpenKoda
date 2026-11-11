# OpenKoda 0.1.8

A candidata local reúne sugestões de revisão do plano pela IA, exportação e restauração do sistema de arquivos do Kali gerido e um reteste de configuração JSON com evidência vinculada ao achado. O [guia de uso](GUIA-0.1.8.md) descreve os controles de cada percurso. A [versão 0.1.7](../releases/0.1.7-local-20261110/release.json) conserva instalador, executáveis, fontes e recibos próprios.

## Correções verificadas

- O núcleo aceita a prova do reteste JSON ao confirmar o achado. A mesma lista de métodos é usada na leitura da evidência e na validação da confirmação. Os métodos de texto e SHA-256 continuam disponíveis.
- A sugestão de revisão do plano verifica as cotas de tentativa e captura ao propor, comparar e salvar a revisão. Uma revisão concluída que já consumiu sua cota conserva o histórico.
- Inspecionar a parada do Kali, exportar ou revisar uma restauração deixou de disparar o diagnóstico que reiniciava a origem. Uma cópia já verificada pode ser restaurada mesmo se a origem desaparecer depois de uma tentativa posterior de exportação interrompida.
- Quando a credencial só permite uma projeção de metadados, o histórico de cópias continua legível. Uma revisão cujos detalhes estão ocultos exige que a pessoa volte a preparar a operação com os detalhes visíveis.

## Ensaio em WSL

O laboratório partiu de uma imagem oficial Kali verificada e criou uma distribuição e um perfil exclusivos. O VHDX de origem continuou em uso depois da parada informada pelo WSL. A implementação passou a exportar os arquivos por `wsl --export` no formato TAR. O gravador impõe o limite revisado durante a transferência e verifica o hash do arquivo por releitura.

O ensaio real exportou 764.846.080 bytes, importou uma distribuição distinta e confirmou o novo marcador e a parada. Após o ensaio, a lista de registros e a distribuição padrão do Windows correspondiam ao estado inicial. [Recibo do laboratório](../.local/vhd-lab-0.1.8/run-db7c1bc5-ccae-47bb-8586-af6d86a6e0de/result.json)

## Alcance da distribuição

O canal escolhido é local e sem assinatura de código. A abertura do executável instalado em um Windows separado, a instalação com WebView2 ausente e as falhas durante reinício ou atualização continuam sem ensaio nesta máquina. O teste de abertura e persistência em Windows Sandbox tem preparação própria. A versão Home instalada aqui não oferece esse recurso.

As chamadas reais à OpenAI constam do histórico das versões anteriores. Os novos percursos de resposta do modelo usam respostas controladas para reproduzir propostas válidas, custos, rejeições e alterações concorrentes. A avaliação com NVDA disponível no histórico cobre cinco cenários da 0.1.6.

## Validações das fontes finais

No Windows, 556 testes Rust passaram. O único teste reservado ao Kali passou separadamente no executável da mesma compilação, com duas capturas sintéticas, sua remoção e nova inspeção da ausência de cada diretório. Os três conjuntos de helpers somaram 122 testes no Linux. No Windows, passaram 71 e outros 51 dependem do sistema Linux.

O frontend passou na compilação TypeScript/Vite e em 66 testes unitários. A suíte de navegador com exportação TAR passou em 233 casos. Após a tradução do estado de exportação em andamento, os 31 casos do arquivo de contrato afetado passaram. Sete ensaios com WebView2 e perfis sintéticos passaram pelo executável desktop. A migração da 0.1.7 preservou o histórico da avaliação, e a restauração manteve as capturas.

Uma segunda exportação real gerou 764.958.720 bytes. O clone importado preservou o arquivo de teste e recebeu um marcador próprio, ambos lidos como usuário comum. Com a distribuição de origem removida, o mesmo TAR e o recibo permitiram recuperar novamente o arquivo em outro clone. [Ensaio final](../.local/vhd-lab-0.1.8/run-1beb0484-82a6-4a4c-b3ea-6ba9a6693661/result.json) · [Restauração sem a origem](../.local/vhd-lab-0.1.8/run-1beb0484-82a6-4a4c-b3ea-6ba9a6693661/after-loss/result.json)

Os avisos do pacote cobrem as 247 entradas do inventário de distribuição. Os textos do SDK WebView2 e do componente NSIS correspondem aos binários usados. Os três arquivos de avisos extraídos do instalador corresponderam aos hashes das fontes. [Inventário e avisos](avisos/AVISOS.txt)

O instalador passou na extração e no ciclo 0.1.7 → 0.1.8 → desinstalação em pasta própria. Os recibos confirmam a identidade dos executáveis e a preservação dos dados sintéticos e do registro anterior do Windows. [Manifesto e recibos](../releases/0.1.8-local-20261110/release.json)
