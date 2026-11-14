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
- A primeira execução do workflow Windows 11 encontrou uma omissão na chamada
  de empacotamento do ensaio: faltava `-SkipUpdaterSigning`, já presente no
  workflow Windows Server. O teste de inicialização não precisa dos segredos
  de publicação. A chamada foi corrigida e uma nova execução iniciada; isso
  não desabilita a assinatura exigida no pacote distribuído nem no atualizador.
- A execução [Windows release startup 43615102082](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/actions/runs/43615102082)
  passou em **Windows Server 2022 e 2025**: instalador 1.1.0, duas inicializações,
  navegação por Windows UI Automation, entrada local explícita, preservação da
  preferência de tempo limite e desinstalação. Esses ambientes de CI não são
  apresentados como prova de login Google pessoal nem de atualização via canal.
- O workflow corrigido [Windows 11 client startup 43615842310](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/actions/runs/43615842310)
  também passou: build x64 e execução do ensaio instalado no cliente Windows 11.
  A falha anterior de configuração permanece registrada no histórico do CI.

## Pacote final e instalação real

[OpenKoda 1.1.0 no GitHub](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/releases/tag/desktop-v1.1.0)
foi publicado como pré-lançamento, preservando as versões anteriores. O pacote
de produção não contém o feature nem o caminho de dados do ensaio. Possui
182.251.856 bytes e SHA-256
`1cc2a36637032eb299e818fcfd5b74495d4dd87ae3111fd83e8d07449cf9ed30`.
A assinatura Tauri passou no verificador oficial com versão autenticada 1.1.0.
O certificado Authenticode continua ausente e isso está nas notas da release.

A instalação existente em `AppData/Local/OpenKoda` foi atualizada de **1.0.0
para 1.1.0**. O binário instalado corresponde ao compilado, admitindo somente
os três bytes do marcador NSIS documentado do Tauri. Antes de instalar foram
copiados onze SQLite pela API de backup e verificados com `quick_check`.

O teste abriu o executável instalado duas vezes e visitou Perfil, Conta e
segurança, Plano, Aparência, Modelos, Histórico e Atualizações, além de projetos
e conversas existentes. Não enviou prompts nem trocou a conta. A comparação
separada com o **backup anterior à instalação**, e não somente entre as duas
aberturas, preservou os hashes de todo o estado principal, todas as mensagens e
todos os registros de solicitações: **10 projetos, 31 avaliações/conversas,
35 mensagens e 21 solicitações**. As preferências também permaneceram iguais
durante as duas aberturas.

O site recebeu a versão 14, fonte `8b7583010f68fe4fc926bf4b16b63766abb1a9c5`,
com o retorno de autenticação do desktop e o download atualizado. A rota pública
`/download/windows` respondeu **206 em 1,84 s**, com os dois bytes `MZ` e
`Content-Range: bytes 0-1/182251856`. Isso comprova início e suporte ao intervalo;
não é apresentado como um segundo download integral dos 182 MB.

O [canal `desktop-updates`](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/releases/tag/desktop-updates)
foi inicializado com `rolloutPercent: 0`. O instalador manual está disponível;
a oferta automática de novos pacotes fica pausada até concluir o ensaio abaixo.

## Limites e próximos gates

O ensaio com o canal técnico real encontrou intermitência na entrega dos assets
do GitHub: consultas falharam por timeout; uma consulta seguinte identificou a
versão, abriu detalhes e permitiu adiar. O download real chegou a 159.577.729 de
182.241.536 bytes, mas o prazo global do **harness**, contado antes de uma
repetição, encerrou essa tentativa. O teste foi corrigido para uma janela por
tentativa, detecção imediata do erro nativo e quantidade limitada de repetições.
Não foi alterado o transporte do produto nem injetado um instalador local.

Na retomada, três consultas também falharam. As tentativas foram interrompidas:
não houve instalação automática completa para declarar como aprovada. Vinte
bancos SQLite do ensaio e onze do perfil real mantiveram hashes lógicos idênticos
entre as tentativas. Os receipts, versões e capturas de falha foram preservados
na pasta ignorada do ensaio; a publicação dos assets e o sucesso de testes de
assinatura não substituem esse teste completo.

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
