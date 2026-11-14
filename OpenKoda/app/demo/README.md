# Demonstração do aplicativo no site

Esta entrada separada monta `src/App.tsx` e seus componentes, ícones, estilos,
formulários e comportamento de teclado originais. Não é incluída na compilação
desktop. `vite.demo.config.ts` substitui somente a fronteira `src/api.ts` pelo
transporte em memória deste diretório.

## Telas e ações disponíveis

| Área | Comportamento do exemplo |
| --- | --- |
| Projetos | Loja Aurora inicial; selecionar, criar com pasta fictícia ou GitHub fictício; arquivar; arrastar divisor |
| Conversas | Criar, renomear, selecionar; histórico em memória; resposta predefinida progressiva, cancelamento, compactação demonstrativa |
| Análise | Dois arquivos fictícios; andamento temporizado, cancelamento e relatório com cobertura/limitações |
| Modelos | Catálogo ilustrativo, seleção de provedor/modelo e esforço de raciocínio |
| Anexos | Arquivo de exemplo em memória, sem seletor de arquivos reais |
| Configurações | Telas desktop reais, preferências de aparência e permissões, uso fictício, privacidade por projeto, suporte e versão |
| Perfil | Identidade local e conta Plus fictícia; chamado e preferências sem envio |

Operações nativas sem simulação (por exemplo, exportar backup) retornam uma
explicação explícita. Não são tratadas como concluídas. O relatório não representa
uma auditoria real. Valores de uso, conta, modelos e permissões são amostras.

## Isolamento

- O site incorpora a página com `sandbox="allow-scripts allow-forms"`, sem acesso
  à origem do pai. `allow-forms` permite que o React receba `submit`; CSP
  `form-action 'none'` impede submissões nativas e `connect-src 'none'` bloqueia rede.
- A entrada instala implementações de Storage em memória **antes** de importar
  App. Nenhuma preferência usa o armazenamento da conta real.
- O transporte não importa Tauri, não usa IPC nem acessa arquivos, contas ou APIs.
- Links externos e seleção de arquivos usam explicações/fixtures locais. A URL
  digitada no formulário GitHub nunca é consultada.
- Recarregar ou recomeçar o iframe restaura todos os registros fictícios.

## Compilar e verificar

Dentro de `OpenKoda/app`:

```sh
npx tsc --noEmit -p demo/tsconfig.json
npx vite build --config vite.demo.config.ts
node ../site/build.mjs
```

O Vite gera `../site/dist/demo-app/`. O build do site embute esses assets no
worker e define cabeçalhos específicos apenas para `/demo-app/`.
`e2e/site-real-demo.spec.ts` testa os fluxos reais com o worker compilado e
requisições interceptadas; `e2e/site-demo.spec.ts` verifica o isolamento do iframe.
