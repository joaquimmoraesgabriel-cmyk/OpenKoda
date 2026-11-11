# OpenKoda

Vocabulário do aplicativo de assistência e automação no servidor do usuário. As definições distinguem identidade, conversa e autoridade.

## Linguagem

**Instalação**:
Ambiente OpenKoda pertencente ao usuário, com identidade própria, serviços, dados e política de acesso.
_Evitar_: Conta, computador.

**Perfil local**:
Identidade de uso do aplicativo no dispositivo, associada a uma instalação ativa na primeira versão.
_Evitar_: Conta da plataforma.

**Conta da plataforma**:
Identidade usada nos serviços hospedados e na edição comercial, distinta da autorização sobre uma instalação.
_Evitar_: Conta do servidor, proprietário automático.

**Dispositivo autorizado**:
Computador ou celular reconhecido por uma instalação, com permissões próprias e vínculo revogável.
_Evitar_: Login compartilhado.

**Pareamento**:
Associação confirmada entre uma instalação e um novo dispositivo.
_Evitar_: Conexão de rede, login Google.

**Convite de pareamento**:
Permissão temporária para solicitar o cadastro de um dispositivo, que ainda depende de confirmação do administrador autorizado.
_Evitar_: Senha permanente, autorização de execução.

**Líder**:
Bot principal fixo responsável pela conversa inicial e coordenação dos especialistas e equipes.
_Evitar_: Administrador do servidor.

**Especialista**:
Bot criado pelo líder com função, escopo e capacidades delimitados.
_Evitar_: Novo serviço instalado, usuário administrativo.

**Equipe**:
Grupo de bots com conversa compartilhada na qual pessoas autorizadas podem participar.
_Evitar_: Organização comercial.

**Incidente**:
Ocorrência correlacionada que exige acompanhamento, explicação ou possível intervenção.
_Evitar_: Qualquer mensagem, aprovação.

**Plano de ação**:
Proposta versionada de operações, com alvo, efeito, risco, verificação e recuperação identificados.
_Evitar_: Comando livre, aprovação genérica.

**Aprovação**:
Decisão humana autenticada que permite executar um escopo específico de um plano válido.
_Evitar_: Resposta do bot, comentário, Outra coisa.

**Política de automação**:
Autorização humana prévia e delimitada para uma categoria explícita de operações determinísticas.
_Evitar_: Liberdade da IA, ação simples autodeclarada.

**Executor**:
Autoridade operacional que verifica a aprovação ou política aplicável antes de realizar uma operação gerida.
_Evitar_: Líder, prompt.

**Relay de notificações**:
Intermediário de envio de avisos mínimos para os dispositivos associados a uma instalação.
_Evitar_: Canal de chat, relay remoto.

**Relay remoto**:
Intermediário de conectividade para o acesso do cliente à instalação quando a rota direta não é suficiente.
_Evitar_: Firebase, aprovação humana.

**Revogação**:
Retirada de uma autorização ou vínculo, com efeitos sobre acessos e decisões ainda válidos.
_Evitar_: Desinstalação, exclusão de histórico.

**Edição aberta**:
Distribuição do núcleo OpenKoda com código aberto e operação da instalação independente de assinatura comercial.
_Evitar_: Teste gratuito da edição comercial.

**Edição comercial**:
Distribuição com conta da plataforma e direitos de serviços comerciais, preservando a autoridade da instalação.
_Evitar_: Proprietário dos servidores dos clientes.
