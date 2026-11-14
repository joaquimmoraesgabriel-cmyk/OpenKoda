// Render complete articles first. The browser adds tab controls as an enhancement.
export function switcher(id, label, items, render, initial = 0) {
  const controls = items.map(({key, tab}, index) =>
    `<button type="button" id="${id}-tab-${key}" data-panel="${id}-${key}" aria-controls="${id}-${key}" aria-selected="${index === initial}">${tab}</button>`
  ).join('');
  const panels = items.map((item, index) =>
    `<div class="show-panel" id="${id}-${item.key}" data-switch-panel>${render(item, index)}</div>`
  ).join('');
  return `<div class="switcher" data-switcher><div class="switch-nav" data-switch-nav aria-label="${label}">${controls}</div>${panels}</div>`;
}

const steps = [
  {key:'projeto', tab:'Projeto', title:'Investigar dois arquivos', image:'projeto.png',
   alt:'Tela Novo projeto com a pasta Calculadora, o alvo Todo o código e a descrição da investigação.',
   caption:'Preparação do projeto no aplicativo para Windows.',
   text:'Uma rota da calculadora recebe uma expressão digitada pelo usuário. Escolha a pasta do projeto e escreva a pergunta que guiará a análise.',
   evidence:'“Onde a fórmula recebida pela API passa a ser executada como código?”',
   href:'/guias/primeiro-projeto/#objetivo', link:'Definir o escopo de uma revisão'},
  {key:'relatorio', tab:'Relatório', title:'Da entrada HTTP ao eval()', image:'relatorio.png',
   alt:'Relatório da Calculadora com o candidato Expressão avaliada sem validação e o trecho que cita api.py e math_worker.py.',
   caption:'Relatório do projeto de demonstração na interface para Windows.',
   text:'O relatório aponta uma operação que merece investigação. Abra a linha indicada, acompanhe a origem do valor e confira quais arquivos entraram na revisão.',
   evidence:'Em math_worker.py, a linha 2 passa a expressão recebida para eval(). Quem controla esse valor?',
   href:'/guias/relatorios/#evidencia', link:'Ler e conferir a evidência'},
  {key:'chat', tab:'Chat', title:'De onde vem a fórmula?', image:'conversa.png',
   alt:'Projeto Calculadora aberto na conversa do OpenKoda com a explicação do caminho da fórmula entre dois arquivos.',
   caption:'Conversa com dados ilustrativos na interface do aplicativo.',
   text:'A conversa retoma os trechos do projeto. Neste caso, a resposta volta a api.py para explicar onde a requisição HTTP fornece a expressão.',
   evidence:'“Que trecho da rota chama compute_formula e o que preciso verificar antes de corrigir?”',
   href:'/exemplos/revisao-de-api/', link:'Seguir a investigação completa'}
];

export const walkthrough = `<section class="walkthrough" id="demonstracao" aria-labelledby="demo-title">
  <div class="feature-heading"><div><p class="section-name">Projeto de exemplo · Python</p><h2 id="demo-title">Acompanhe uma revisão</h2></div><p>Siga a fórmula de uma calculadora, da escolha da pasta à conversa sobre o resultado.</p></div>
  ${switcher('demo','Etapas da revisão',steps, (s,i)=>`<div class="walk-scene"><figure class="walk-capture"><a href="/${s.image}" aria-label="Ampliar captura de ${s.tab.toLowerCase()}"><img src="/${s.image}" width="1280" height="955" loading="lazy" decoding="async" alt="${s.alt}"></a><figcaption>${s.caption} Abra para ampliar.</figcaption></figure>
  <article class="walk-story"><p class="step-count">Etapa ${i+1} de ${steps.length}</p><h3>${s.title}</h3><p>${s.text}</p><blockquote>${s.evidence}</blockquote><a href="${s.href}">${s.link}</a></article></div>`)}
</section>`;

const options = [
  {key:'propria', tab:'Chave própria', name:'Use o provedor que você já paga.', intro:'Se você já tem acesso à API de um provedor, pode conectar essa conta à edição gratuita para Windows.',
   facts:[['Configuração','Abra IA e modelos, cadastre a chave e ative um modelo da sua conta. A credencial fica no cofre do Windows.'],['Quem paga','O provedor escolhido fatura o consumo na conta vinculada à chave.'],['Onde conferir','O Histórico de uso mostra os registros disponíveis. Consulte o extrato do provedor para o valor faturado.']],
   href:'/conexoes/chave-propria/', link:'Como conectar minha chave', chip:'Conta do provedor', ledger:'Histórico de uso e extrato'},
  {key:'assinatura', tab:'IA da assinatura', name:'Acompanhe uma cota na sua conta Koda.', intro:'Na modalidade gerenciada, o serviço autoriza cada envio pelo plano. O checkout público está em simulação durante o pré-lançamento.',
   facts:[['Configuração','Entre na conta Koda no aplicativo. Uma conta com acesso ativo recebe os modelos permitidos pelo serviço.'],['Quem paga','O uso autorizado consome a cota de IA da assinatura. A simulação pública do checkout não gera cobrança.'],['Onde conferir','Em Plano e uso, veja quanto foi consumido na janela de sete dias, o total do ciclo e a próxima liberação.']],
   href:'/conexoes/assinatura/', link:'Entender o uso da assinatura', chip:'Conta Koda', ledger:'Cota da janela e do ciclo'}
];
const pathCard = o => `<div class="route-note"><h4>Destino do uso</h4><dl><div><dt>Pedido enviado por</dt><dd>OpenKoda no Windows</dd></div><div><dt>Conta responsável</dt><dd>${o.chip}</dd></div><div><dt>Consulta do consumo</dt><dd>${o.ledger}</dd></div></dl></div>`;
export const connections = `<section id="conexoes" class="connection-lab" aria-labelledby="connection-title" data-motion>
  <div class="feature-heading"><div><p class="section-name">Conexões de IA</p><h2 id="connection-title">De qual conta sai o uso?</h2></div><p>Escolha uma conexão para ver o que muda no aplicativo, no custo e no histórico.</p></div>
  ${switcher('conexao','Comparar conexões de IA', options, o=>`<article class="connection-scene"><div><h3>${o.name}</h3><p>${o.intro}</p><dl class="fact-list">${o.facts.map(([k,v])=>`<div><dt>${k}</dt><dd>${v}</dd></div>`).join('')}</dl><a href="${o.href}">${o.link}</a></div>${pathCard(o)}</article>`)}
</section>`;

const stories = [
  {title:'Como uma linha vira candidata a problema',category:'Leitura de código',summary:'Acompanhe uma entrada de API entre dois arquivos e descubra o que a linha ainda precisa provar.',href:'/exemplos/revisao-de-api/',visual:'evidence'},
  {title:'O que realmente entrou na revisão?',category:'Escopo e cobertura',summary:'Use a cobertura para distinguir arquivos lidos, exclusões e interrupções antes de concluir uma análise.',href:'/guias/relatorios/#cobertura',visual:'coverage'},
  {title:'Quando uma resposta chega ao limite',category:'Conversas',summary:'Veja como guardar o trecho recebido, preparar a continuação e decidir se vale outro envio.',href:'/guias/conversas/#limite-de-resposta',visual:'chat'}
];
const previews = {
 evidence:'<span class="card-file">math_worker.py</span><code><span>1&nbsp; def compute_formula(x):</span><b>2&nbsp;&nbsp;&nbsp; return eval(x)</b></code>',
 coverage:'<span class="card-file">Escopo do exemplo</span><ul><li>api.py <span>Entrada</span></li><li>math_worker.py <span>Operação</span></li></ul>',
 chat:'<span class="card-file">Exemplo de conversa</span><p>Onde a fórmula entra?</p><p>Na rota em <b>api.py</b>.</p>'
};
export const readingCards = `<section class="reading-cards" data-motion><div class="feature-heading"><h2>Para investigar melhor</h2><a href="/guias/">Ver todos os guias</a></div><div class="story-grid">${stories.map(s=>`<a class="story-card" href="${s.href}"><div class="card-sketch ${s.visual}" aria-hidden="true">${previews[s.visual]}</div><div><span class="section-name">${s.category}</span><h3>${s.title}</h3><p>${s.summary}</p></div></a>`).join('')}</div></section>`;

export const connectionPages = {
 conexoes:['Conexões de IA',`<h1>Como conectar sua IA</h1><p class="lead">A mesma conversa pode usar uma conexão pessoal ou um modelo da assinatura. Antes de enviar, confira o destino e a conta que assumirá o custo.</p>${connections}`],
 'conexoes/chave-propria':['Conectar uma chave própria',`<a class="back-link" href="/conexoes/">Conexões de IA</a><h1>Conectar uma chave própria</h1><p class="lead">Use o OpenKoda gratuito com uma credencial do seu provedor. Você escolhe o modelo e acompanha a cobrança na conta em que gerou a chave.</p>
 <div class="story-body"><h2>Cadastre a conexão no Windows</h2><p>Em <strong>Configurações</strong>, abra <strong>IA e modelos</strong> e selecione <strong>Adicionar provedor</strong>. Salve a chave no aplicativo, consulte o catálogo e marque os modelos que quer disponibilizar. Ative o provedor. O Windows protege a credencial no cofre do seu perfil.</p><h2>Escolha o modelo pelo pedido</h2><p>Para conversar, selecione um modelo de texto. Para enviar uma captura, ele precisa aceitar imagens. A análise de código usa saídas estruturadas para reunir arquivos, linhas e candidatos no relatório. O catálogo e as permissões dependem da conta pessoal.</p><h2>Confira o primeiro envio</h2><p>Antes de autorizar, confira a conexão, a pergunta e os arquivos propostos. Depois da resposta, abra <strong>Histórico de uso</strong> para consultar a operação. As estimativas do aplicativo usam o preço configurado. A fatura do provedor informa o valor efetivo.</p><p><a href="/guias/primeiro-projeto/">Criar o primeiro projeto</a> · <a href="/guias/uso/#estimativas">Entender as estimativas</a></p></div>`],
 'conexoes/assinatura':['Usar a conexão da assinatura',`<a class="back-link" href="/conexoes/">Conexões de IA</a><h1>Usar a conexão da assinatura</h1><p class="lead">Uma conta Koda com direito ativo usa os modelos autorizados pelo plano. O checkout público ainda permite apenas simular a contratação.</p>
 <div class="story-body"><h2>O que muda no aplicativo</h2><p>Entre com sua conta Koda e escolha a conexão da assinatura. O seletor apresenta os modelos disponíveis para aquela conta. Nos modelos compatíveis, você pode ajustar o nível de raciocínio. Para consultar as skills, digite <strong>/</strong> no campo da mensagem.</p><h2>Quando há vários pedidos</h2><p>O serviço confere o plano e a cota antes de executar a solicitação paga. Se as vagas estiverem ocupadas, o pedido pode aguardar em fila. Você acompanha a posição e pode cancelar enquanto espera. Uma nova tentativa ainda precisa de cota disponível.</p><h2>Como ler o saldo</h2><p>Em <strong>Minha conta</strong>, abra <strong>Plano e uso</strong>. A janela mostra o consumo daquele período de até sete dias. O ciclo mensal reúne o consumo das janelas. A data da próxima liberação informa quando uma nova parcela fica disponível.</p><p><a href="/precos/">Comparar planos e cotas</a> · <a href="/guias/uso/#fila">Entender a espera na fila</a></p></div>`]
};
