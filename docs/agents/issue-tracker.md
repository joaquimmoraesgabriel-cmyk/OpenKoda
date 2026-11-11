# Acompanhamento de tarefas

As tarefas e especificações vivem no GitHub Issues de joaquimmoraesgabriel-cmyk/OpenKoda.

## Operações

Use o conector GitHub disponível; os comandos gh abaixo também documentam o fluxo das skills. Antes da primeira escrita da sessão, confira o destino pelo conector GitHub (consulta do repositório) ou por gh repo view --json nameWithOwner dentro desta pasta. O resultado deve ser joaquimmoraesgabriel-cmyk/OpenKoda. Um destino diferente exige esclarecer o repositório antes de publicar.

- Criar: gh issue create --title "..." --body-file <arquivo>.
- Ler: gh issue view <numero> --json number,title,body,labels,comments.
- Listar: gh issue list --state open --json number,title,body,labels.
- Classificar: gh issue edit <numero> --add-label <rotulo> ou --remove-label <rotulo>.
- Comentar, somente quando solicitado pelo usuário ou workflow explicitamente invocado: gh issue comment <numero> --body-file <arquivo>.
- Encerrar, quando autorizado pelo trabalho: registrar a conclusão e fechar a tarefa.

Use arquivo temporário para corpos multilinha; preserve quebras de linha e texto literal. Segredos e dados de conexão nunca entram em tarefas, comentários ou anexos.

Use dependências nativas quando disponíveis, confirmando o suporte do CLI pela ajuda. Como alternativa, indique Blocked by: #<numero> no início do corpo. Uma tarefa só está livre quando todos os bloqueios abertos forem resolvidos. Relacione subtarefas ao pai por vínculo nativo ou Part of #<numero>.

## Pull requests as a triage surface

PRs as a request surface: no.

## Contratos das skills

Publicar no issue tracker significa criar uma issue neste repositório, dentro do escopo autorizado. Buscar ticket significa ler a issue indicada. Esta configuração não cria um backlog nem autoriza publicação irrestrita por si só.
