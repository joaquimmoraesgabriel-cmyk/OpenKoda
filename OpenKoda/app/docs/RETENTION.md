# Retenção local revisável

O expurgo desta versão cobre somente backups em `backups/<UUID>`, pacotes de relatório em `exports/report-<UUID>` e exportações Markdown geradas pelo OpenKoda. A interface solicita IDs do inventário; não aceita um caminho de arquivo. O prazo configurado de retenção não dispara seleção nem remoção automática.

Arquivar um projeto continua sendo uma alteração lógica. O expurgo do grafo de projeto está indisponível: dados de avaliações, conversas, correções e sessões MCP vivem em bancos WAL distintos. Removê-los parcialmente quebraria referências e trilhas. Esta entrega não declara que essas informações foram apagadas.

## Contrato nativo

| Ação | Entrada | Resultado |
| --- | --- | --- |
| `retention.inventory` | `{}` | Itens elegíveis, itens recusados, limites, planos anteriores e bloqueios |
| `retention.plan` | `{items:[{category,id}]}` | Plano de 15 minutos com arquivos, bytes, SHA-256, identidades do sistema de arquivos e digest |
| `retention.apply` | `{id,digest,confirmation}` | Plano atualizado com resultados por arquivo; a frase é `EXCLUIR PERMANENTEMENTE` |
| `retention.reconcile` | `{id}` | Inspeção do plano interrompido, sem repetir qualquer exclusão |

As categorias permitidas são `backup`, `report` e `markdown`. Os hashes do inventário são vazios; a preparação do plano lê e calcula os hashes completos. O lote aceita no máximo 20 artefatos, 256 arquivos e 1 GiB; cada arquivo tem limite de 128 MiB.

O coordenador nativo deve manter o bloqueio exclusivo compartilhado com manutenção, execução, exportação, backup e ativação durante as alterações. Além da verificação local de avaliações, ele verifica sessões MCP, correções, manutenção e ativação pendentes. Inventário e reconciliação permanecem acessíveis para recuperar estados desconhecidos.

## Validação e falhas

Todos os itens são revalidados antes da primeira exclusão. Mudança de bytes, identidade, composição ou destino rejeita o lote inteiro nessa pré-verificação. Links, junctions, reparse points, hardlinks, streams alternativos e arquivos não reconhecidos são recusados. Diretórios ancestrais ficam protegidos contra substituição durante a operação.

No Windows, a remoção usa `SetFileInformationByHandle` com `FileDispositionInfo` no handle verificado, aberto com acesso DELETE. Não há `remove_dir_all`, shell de remoção ou nova resolução do caminho para apagar um arquivo. A [documentação Microsoft](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle) descreve a remoção quando o handle é fechado. Outras plataformas não executam o expurgo nesta versão.

O lote não é uma transação atômica do sistema de arquivos. Cada intenção é persistida com SQLite `synchronous=FULL` antes do efeito, seguida do resultado. Falha intermediária produz estado parcial; o manifest é removido por último para que os remanescentes possam receber uma nova prévia. Um diretório vazio remanescente só é reconhecido pela identidade preservada no histórico anterior.

Ao abrir o aplicativo, planos preparados são revogados e operações interrompidas ficam desconhecidas. A reconciliação registra se cada identidade original está presente, ausente ou foi substituída. Objetos substitutos não são apagados. Ela não repete o lote; remanescentes exigem seleção, plano e confirmação novos.

`retention.sqlite3` conserva referências, hashes, planos e resultados; não armazena conteúdo dos artefatos. Incluí-lo em backups preserva a trilha do expurgo. A restauração não reativa aprovações nem executa exclusões.

## Limites de apagamento

Excluir uma cópia gerida não remove projetos originais, cópias externas, outros perfis, restaurações, fontes, dados enviados ao provedor, memória ou caches do sistema operacional. A operação não implementa apagamento seguro de mídia. Os hashes do plano identificam os bytes selecionados; não certificam que um backup é restaurável. Não há remoção silenciosa da última cópia nem escolha automática baseada em idade.
