# Validação local do OpenKoda 0.1.10

O instalador Windows x64 sem assinatura atualizou a 0.1.9 e preservou os dados sintéticos durante a desinstalação. Os pacotes compilados no GitHub a partir da mesma revisão abriram e reabriram um projeto em duas máquinas descartáveis. [Guia de uso](GUIA-0.1.10.md) · [Ensaios de instalação no GitHub](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/actions/runs/43028695936)

| Percurso | Resultado |
| --- | --- |
| Instalação neste PC | Extração, conferência do executável e dos cinco recursos, instalação da 0.1.9, atualização para 0.1.10 e desinstalação. Registro e atalhos conferidos antes e depois. |
| Abertura em Windows limpo | As imagens Windows Server 2022 e 2025 instalaram seus próprios pacotes, abriram a interface, criaram um projeto, reencontraram seu UUID na segunda abertura e conservaram o banco na desinstalação. Os hashes dos instaladores de CI e do instalador local são registrados separadamente. |
| Migração e cópia | Na regressão entre os executáveis de depuração preservados, o banco principal manteve o esquema 8 e os dados do projeto. O banco de ambiente passou de 1 para 2 e conservou o estado existente. A nova tabela de retiradas começou vazia. A restauração validou o histórico das versões anteriores. |
| Solicitação repetida | Uma criação, um replay sequencial e dois simultâneos deixaram uma única avaliação nova e um recibo para a chave. A alteração do conteúdo com a mesma chave retornou conflito. |
| AC-25, autorização por janela | Duas janelas WebView2 abriram a mesma página local em um binário de laboratório da 0.1.10. A principal criou e consultou um projeto. A janela secundária recebeu duas recusas explícitas de `main-window` para `snapshot` e `project.create`. O SQLite conservou o mesmo estado inclusive após fechar o processo. |
| Preparação do Windows | O diagnóstico local começou por ação explícita. A reabertura encontrou o mesmo checkpoint e verificou novamente os requisitos do WSL. O teste de mudança de boot usou uma simulação. |
| Retirada do Kali | O núcleo e a interface conferem identidade, confirmação, retirada, preservação da distribuição, colisão de nome ou caminho e retomada do histórico. Os backups retêm os recibos. |
| Regressões | 582 testes Rust no Windows, um teste adicional no Kali gerido, 66 unitários da interface, 249 de navegador e 74 grupos nativos. |
| OpenAI | No executável de depuração da 0.1.9, o comando das configurações gravou uma credencial temporária, consultou 96 modelos no catálogo oficial e removeu a credencial. A consulta não enviou dados de projeto nem fez inferência. |

O pacote local tem **7,299,718 bytes** e SHA-256 `d0e37ffebf6b1a24e36fc35173d5984f563958ff199fa388ae02ad7d556b1f9b`. Os resultados do Windows hospedado pertencem à revisão [`6bf8377`](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/commit/6bf8377961de53cb9f8fe317bdc705816900afbd). No checkout usado para gerar o pacote, o [manifesto local](../releases/0.1.10-local-20261111/release.json) reúne o instalador, o ZIP dos fontes e os hashes dos testes.

O instalador e o código de produção do ZIP correspondem ao commit `6bf8377`. O ZIP inclui também a documentação desta validação. O executável da suíte Rust usa o commit `78c716b`, com uma correção exclusivamente no módulo de testes do Windows de `ai_budget.rs`. O manifesto registra os dois commits, o arquivo de teste e seus hashes. [Correção do teste no GitHub](https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/commit/78c716bdae421cc8d8e7a2d09e0c87960a343fd6)

## Retirada em WSL 2 real

Um piloto sem interface gráfica usou os módulos de produção para importar um clone descartável, confirmar GUID, nome, caminho e marcador, desativar o executor e recusar a identidade anterior. Repetir a confirmação devolveu o recibo original. O operador do ensaio removeu apenas o clone. A reconciliação confirmou a ausência do registro e da pasta. As três distribuições anteriores, a padrão e os dois arquivos TAR foram preservados.

## Alcance

Os testes exercitam os adaptadores disponíveis de arquivos, análise lexical, HTTP e TCP, o executor MCP/Kali e os percursos de IA implementados. Os ensaios de plataforma ainda deixam abertas a instalação em Windows cliente sem WebView2, a queda física durante manutenção ou atualização e o esgotamento real do volume da distro durante APT. A acessibilidade foi amostrada em percursos específicos. [Matriz de aceitação](ACCEPTANCE-STATUS.md) · [Planejamento e capacidades condicionais](ROADMAP.md)
