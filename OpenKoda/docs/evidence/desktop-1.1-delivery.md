# Desktop 1.1 — verificação de entrega

Data: 14 de novembro de 2026. As evidências locais citadas abaixo ficam em
`OpenKoda/app/.local/`, ignoradas pelo Git. Não contêm chaves de API.

## Referências observadas e decisões

Foram examinadas seis folhas de quadros extraídos a cada seis segundos das
três gravações fornecidas (110935, 111526 e 111932). Isso permite examinar os
estados visuais registrados; não equivale a uma medição de todas as animações.

Nos quadros: navegação lateral estreita; configurações agrupadas à esquerda e
formulários compactos à direita; perfil e consumo separados; entrada que leva
ao navegador e volta ao aplicativo; menus de modelos, raciocínio e ferramentas
próximos ao compositor. As referências preservam bastante espaço para trabalhar.

Decisões próprias para o OpenKoda: superfícies grafite/minerais, acento da marca,
tipografia e densidade de desktop, seções de conta separadas, transições breves
com redução de movimento, estado inicial vinculado ao carregamento real.
Não foram incorporadas marcas, capturas ou textos dos produtos de referência.

## Causas encontradas

- A apresentação do site e o código do app haviam avançado, mas o download
  ainda apontava ao instalador 1.0.0. Atualizar o site não recompila um desktop.
- O callback nativo passava primeiro pelo resgate de sessão web, que usava
  outro verificador PKCE. Uma falha nesse caminho impedia o retorno ao host.
- A simulação anterior registrava uma intenção, sem conceder os recursos de
  teste. A nova concessão está persistida e limitada a testadores autorizados.
- Catálogo e ajuste de raciocínio exigiam separar modalidade, parâmetros
  suportados e política de custo. Ter uma entrada no catálogo não garante
  disponibilidade de inferência ou inclusão em um plano.

## Testes já verificados

- Núcleo Rust: **679 aprovados, zero falhas, quatro ignorados**, 545,42 segundos.
- Após as últimas alterações, testes focais do atualizador: **cinco aprovados**;
  assinatura oficial: **dois aprovados**, incluindo corrupção, versão falsa,
  downgrade e falha de conexão. O teste de artefato exige seleção explícita.
- Nova regressão de skill com chave pessoal: aprovada. Troca da conta ou
  expiração bloqueia antes de reservar ou enviar; o provedor é simulado.
- PostgreSQL local real: autorização/RLS, cota, fila, simulação e 12 cliques
  concorrentes com o mesmo identificador. Nenhum crédito financeiro criado.
- Edge: 23 testes aprovados. Worker de sessão: sete aprovados. Frontend:
  typechecks e 84 testes unitários aprovados. A última rodada das telas afetadas
  aprovou 34 testes E2E, incluindo expiração de acesso e coexistência de plano
  financeiro com simulação. Os recibos de interface distinguem
  a rodada ampla das repetições focais; não se presume que todos os fluxos de
  navegador sejam equivalentes ao instalador.
- Supabase remoto: migração `20261114151900_openkoda_internal_simulation`,
  função Edge versão 11. Testador autorizado por sete dias. Duas ativações com
  o mesmo UUID produziram **um evento**, a segunda com `replay: true`.
  O digest do estado financeiro antes/depois permaneceu idêntico; skills e
  raciocínio de teste ativos, `hostedInference: false`.
- OpenRouter real pelo transporte Rust de produção: uma chamada, esforço
  `low`, teto de 128 tokens, 82 tokens de entrada e 15 de saída. Resposta
  concluída. **US$ 0,00000456** medidos no recibo do provedor. O recibo não estava
  disponível imediatamente: a asserção inicial falhou nessa consulta e uma
  consulta posterior reconciliou o custo. A inferência não foi repetida.
  O ensaio opt-in passou a repetir apenas a leitura do recibo por tempo limitado.

## Limites e próximos gates

O login Google de uma pessoa no instalador requer sua sessão pessoal. Os testes
automatizados de callback usam Auth controlado, listener local real, state,
PKCE, cancelamento, expiração e replay. O app precisa permanecer aberto durante
o login; reabrir um app fechado por protocolo não foi implementado.

A simulação permite skills e raciocínio com chave própria, sem representar
pagamento ou crédito gerenciado. Asaas, domínio, remetente de produção,
certificado Authenticode e backup portátil da chave de atualização permanecem
etapas separadas de lançamento. O cofre local de assinatura está fora do checkout,
criptografado para o usuário do Windows; não deve ser compartilhado ou apagado.
O mesmo segredo foi configurado por stdin no environment `desktop-release` do
GitHub, sem passar por argumentos, arquivos de texto ou logs. O workflow de
candidato certificado ainda requer o certificado Windows antes de executar.

Projetos e conversas locais pertencem ao perfil deste dispositivo e continuam
disponíveis no modo local. Trocar a conta comercial não transfere esses dados
para outra conta na nuvem; os testes de troca de conta cobrem identidade,
permissões, skills e caches comerciais, não perfis Windows de pessoas diferentes.

As versões anteriores a 1.1 precisam instalar esta versão uma vez. O updater
não pode aparecer retroativamente em um executável que não o continha.
