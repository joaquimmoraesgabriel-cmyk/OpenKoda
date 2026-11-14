# OpenKoda 1.1.1 — presença e acesso à Koda

Data: 14 de novembro de 2026.

## Correção

Koda é uma conexão gerenciada, identificada internamente por `openkoda`.
Sua presença na interface não depende da permissão de inferência paga.
Ela aparece em IA e modelos, Adicionar provedor, seletor do chat e criação
de projeto. Não há cadastro, teste ou remoção de chave pessoal nessa entrada.

O estado distingue entrada na conta, plano gratuito, simulação interna,
assinatura com inferência habilitada, serviço indisponível e conta offline.
Contas autenticadas podem consultar o catálogo; modelos ficam bloqueados
quando não há permissão de uso. A simulação não concede saldo financeiro.
Os controles financeiros e de autorização do servidor não foram alterados.

Os botões abrem Conta e segurança ou Plano e assinatura dentro do aplicativo.
O projeto continua montado e os rascunhos do chat e da criação são preservados.
O nome visível do provedor é Koda; o aplicativo continua OpenKoda.

## Catálogo e descrição dos planos

Consulta pública gratuita em 2026-11-14, 17:27 UTC: 482 entradas, das quais
60 modelos distintos elegíveis sob a política vigente de modalidades, tarifas
e capacidades. O número varia e não é uma promessa contratual. São modelos,
não 482 provedores. Aumentar apenas o teto de preços não torna todas as entradas
compatíveis. Por isso, o texto dos planos informa o acesso ao catálogo disponível
na Koda, sem prometer mais de 200 opções utilizáveis.

## Verificação da correção

- TypeScript sem erros; 84 testes unitários aprovados.
- 12 novos cenários de interface cobrem os seis estados, presença da Koda,
  bloqueio de seleção paga, ausência de operações BYOK e navegação com rascunhos.
- 32 cenários de interface afetados têm aprovação, incluindo entrada/sessão e
  raciocínio. A primeira execução teve quatro timeouts de inicialização do Vite;
  a segunda teve dois. Os testes correspondentes passaram na repetição focal
  com maior prazo de ação, sem alteração das verificações funcionais.
- Quatro regressões adicionais aprovadas: cadastro/retomada BYOK, cancelamento
  sem salvar chave, troca explícita do padrão e uso do seletor pelo teclado.
- Capturas sintéticas de gratuito e simulação, nas configurações e no seletor,
  revisadas visualmente. Não contêm dados de contas reais.
- Revisão estática independente sem falha concreta identificada no patch.
- Site: compilação e 22 testes aprovados, incluindo sessão, navegação e download.

Nenhuma chamada paga de IA foi necessária nesta correção: US$ 0.
Os testes de renderer não comprovam o fluxo nativo de atualização; suas
limitações estão registradas abaixo.

## Pacote assinado e publicação

O pacote de produção foi compilado do commit
`2a4eed9aa43c6a7fa7eeb43b5a762805c0fd387f`, com versão PE 1.1.1, endpoint e chave
pública de produção. Não contém os marcadores do build isolado de ensaio. O
inventário do runtime de segurança permaneceu igual ao da versão 1.1.0.

- Arquivo: `OpenKoda_1.1.1_x64-setup.exe`, 182.164.978 bytes.
- SHA-256: `dbda828b997888c37ae97611f91a9031e98bf78436d574b6422e46a6ac6c30f6`.
- Teste oficial `updater_verify_packaged_candidate`: 1 aprovado, 0 falhas;
  assinatura vinculada à chave de produção e à versão 1.1.1.
- [Release 1.1.1](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/releases/tag/desktop-v1.1.1)
  publicada como pré-lançamento com instalador, assinatura e `SHA256SUMS.txt`.
  O tamanho e o digest retornados pelo GitHub conferem com o arquivo local.
- Canal `desktop-updates`: versão 1.1.1, `rolloutPercent: 0`. Hash do manifesto
  publicado: `8cf4496655903ea2aaf6d78b84c8118363f362e9c82d5a27f9dcdad2c38a9d7b`.
- Certificado Authenticode do Windows continua ausente. A assinatura do
  atualizador não substitui esse certificado.

O instalador e a release 1.1.0 foram preservados. Os receipts detalhados locais
são `.local/package-windows-production-1.1.1.json` e
`.local/production-1.1.1-verification.json`. A compilação e a publicação não
instalam essa versão no computador do usuário.

O site existente foi publicado na versão 15 do Sites, a partir do commit de
fonte `03875081c92bc13daec30d60531446849f9bc9b9`, mantendo o público e o visual
anteriores. Foram enviados apenas os textos revisados e os metadados da release.
A proposta OpenGlitch continua fora da lista de recursos publicados.

Na verificação HTTP de 18:27 UTC, `/download/windows` respondeu `206` ao intervalo
`bytes=0-1`, entregou os bytes `MZ`, o nome do instalador 1.1.1 e o tamanho total
correto (182.164.978). Os primeiros bytes chegaram em 3,1 s nessa medição. Início,
download, notas 1.1.1 e preços responderam `200`. Essa consulta verifica o início
e o suporte a intervalos; não equivale a baixar e instalar o arquivo inteiro.

## Instalação em máquinas de teste do GitHub

O código do commit `2a4eed9aa43c6a7fa7eeb43b5a762805c0fd387f` passou pelos
dois workflows, com versão 1.1.1, duas aberturas, retenção das preferências,
desinstalação e preservação dos bancos locais do ensaio:

- [Windows Server 2022 e 2025](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/actions/runs/43622769480).
- [Windows 11 Enterprise, build 26200](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/actions/runs/43622769651).

O Windows 11 do ensaio é Arm64 executando o aplicativo x64. Isso não comprova
um pacote ARM nativo. Os instaladores do CI são builds de validação sem assinatura
do atualizador e têm hashes diferentes do pacote de distribuição assinado.
Os perfis são sintéticos; esses testes não comprovam atualização sobre os dados
pessoais do computador do usuário.

## Ensaio real ainda pendente

A consulta direta ao manifesto de releases do GitHub neste computador voltou a
falhar com reset de conexão. Além disso, a ferramenta de controle do computador
informou uma interrupção por Esc; novas operações de janelas foram suspensas.
O canal de atualização permanece pausado. Não foi executada instalação local
da versão 1.1.1 nem declarada preservação de dados por essa atualização.

O harness preparado usa agora o caminho canonicalizado real do Windows para
calcular a coorte. A representação foi comparada à saída de
`std::fs::canonicalize` em um helper Rust de leitura, sem abrir o app. No perfil
atual a coorte é 12, e não 43 como seria obtido omitindo o prefixo de namespace.
O diretório informado pelo backup é comparado por identidade de arquivo
(`samefile`), preservando as verificações de integridade e inventário.
