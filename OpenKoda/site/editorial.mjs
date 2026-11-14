const repository = 'https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda';
export const desktopVersion = '0.2.5';
const release = version => repository+'/releases/tag/desktop-v'+version;
const entry = version => '/novidades/openkoda-'+version.replaceAll('.','-')+'/';
const currentEntry = entry(desktopVersion);
const date = '<time datetime="2026-11-13">13 de novembro de 2026</time>';
const download = '<a class="primary" href="/download/">Baixar para Windows</a>';

const changes = [
  {version:'0.2.5', title:'Novos controles para abrir e fechar detalhes',
   summary:'A nova apresentação deixa mais fácil encontrar preferências, ler os detalhes de uma análise e acompanhar chamados.',
   paragraphs:[
    'No aplicativo, os controles de expansão receberam uma apresentação mais clara. Você pode abri-los pelo teclado, ler o conteúdo e voltar ao ponto em que estava.',
    'No site, perguntas frequentes, preferências da conta e chamados usam um controle de abrir e fechar com foco visível. As informações sobre versões, planos e privacidade também ganharam índices próprios.'
   ], more:'/guias/conta/',label:'Acompanhar meus chamados'},
  {version:'0.2.4', title:'Continue uma resposta a partir do trecho recebido',
   summary:'Uma resposta que alcança o limite de saída fica no histórico. Você decide quando enviar a continuação.',
   paragraphs:[
    'Durante uma conversa longa, o modelo pode alcançar seu limite de saída no meio de uma explicação. O OpenKoda guarda o texto recebido e sinaliza a interrupção. Na última resposta, Continuar resposta prepara um rascunho que você pode ajustar e enviar.',
    'A versão também permite acompanhar a posição de uma solicitação da assinatura quando todas as vagas de processamento estão ocupadas. Uma espera pode ser cancelada. O checkout público continua em simulação.'
   ], more:'/guias/conversas/#limite-de-resposta',label:'Entender o limite de resposta'},
  {version:'0.2.3', title:'Conta, chamados e histórico de uso no mesmo aplicativo',
   summary:'A conta Koda reúne preferências, atendimento e a simulação dos planos. O histórico ajuda a distinguir uso de chave própria e uso da assinatura.',
   paragraphs:[
    'A edição para Windows ganhou acesso à conta Koda. Você pode entrar com Google ou e-mail, consultar o plano e revisar as preferências de comunicação. O checkout demonstra o fluxo dos planos sem solicitar cartão.',
    'Se encontrar um problema, descreva a versão, a etapa e o resultado esperado no suporte. As respostas ficam disponíveis na sua conta.'
   ], more:'/guias/uso/',label:'Consultar o histórico de uso'}
];

function newsItems(items){
  return '<ul class="bulletin-list">'+items.map(item=>`<li><a href="${entry(item.version)}"><span class="bulletin-version">Versão ${item.version}</span><span><strong>${item.title}</strong><span class="blurb">${item.summary}</span></span></a></li>`).join('')+'</ul>';
}

const example = '/exemplos/revisao-de-api/';
export function home(demo){
  return `<section class="cover">
    <div class="cover-words"><h1>Revise seu código linha por linha.</h1><p>Escolha os arquivos do projeto no Windows. Converse com a IA e confira no código a origem de cada possível problema.</p>
    <div class="actions">${download}<a class="text-action" href="#demonstracao">Ver demonstração</a></div></div>
    <figure class="code-plate"><figcaption><span>Calculadora</span><span>Projeto de exemplo</span></figcaption>
    <div class="source-piece"><p>api.py</p><pre><code><span class="source-line">6</span> formula = request.args[<em>"formula"</em>]
<span class="source-line">7</span> <b>return</b> compute_formula(formula)</code></pre></div>
    <div class="source-piece"><p>math_worker.py</p><pre><code><span class="source-line">1</span> <b>def</b> compute_formula(formula):
<span class="source-line">2</span>     <mark>return eval(formula)</mark></code></pre></div>
    <p class="plate-note">A mesma entrada atravessa dois arquivos até a operação destacada.</p></figure>
  </section>
  <div class="edition-band"><a href="${currentEntry}">O que mudou no OpenKoda ${desktopVersion}</a><span>Pré-lançamento para Windows x64</span></div>
  ${demo}
  <section class="project-feature" data-motion><div><h2>A investigação continua na conversa.</h2><p>Pergunte de onde vem um valor, confira a linha no arquivo e retome o raciocínio no mesmo projeto.</p><a href="${example}">Ler o exemplo da calculadora</a></div>
  <figure class="screen-print"><a href="/produto/#aplicativo"><img src="/conversa.png" alt="Conversa de exemplo no OpenKoda em que a revisão liga api.py à operação em math_worker.py." width="1376" height="870" loading="lazy" decoding="async"></a><figcaption>Conversa ilustrativa na interface do OpenKoda para Windows.</figcaption></figure></section>
  <section class="home-news" data-motion><div><h2>Novidades do aplicativo</h2><p>Veja o que mudou nas versões para Windows e consulte os guias dos recursos.</p><a href="/novidades/">Todas as novidades</a></div>${newsItems(changes.slice(0,2))}</section>`;
}

export const screenshot = '<figure class="screen-print" id="aplicativo"><a href="/conversa.png" aria-label="Abrir captura do aplicativo no tamanho original"><img src="/conversa.png" width="1376" height="870" loading="lazy" decoding="async" alt="Projeto Calculadora no OpenKoda, com o histórico do chat e a explicação das duas linhas de código."></a><figcaption>Conversa ilustrativa no aplicativo. Abra a imagem para ver em tamanho original.</figcaption></figure>';

export const launch = `<div class="install-panel"><div><h2>Instalador x64</h2><p>OpenKoda ${desktopVersion} para Windows, em pré-lançamento.</p></div><a class="primary" href="${repository}/releases/download/desktop-v${desktopVersion}/OpenKoda_${desktopVersion}_x64-setup.exe">Baixar instalador x64</a></div>
  <div class="install-facts"><section><h2>Confira o arquivo</h2><p>Esta prévia é distribuída sem assinatura digital. Nas notas da versão, compare o SHA-256 com o do arquivo baixado antes de instalar.</p><a href="${release(desktopVersion)}">Verificar o instalador no GitHub</a></section>
  <section><h2>Prepare a primeira revisão</h2><p>Separe uma pasta pequena e uma chave do seu provedor de IA. Depois de instalar, abra <strong>Configurações</strong> e escolha <strong>IA e modelos</strong> para cadastrar sua conexão.</p><a href="/guias/primeiro-projeto/">Configurar um projeto</a></section></div>
  <p><a href="/novidades/">Histórico de versões</a> <span class="separate-links"><a href="${repository}">Código-fonte e licença</a></span></p>`;

const plans = [
 ['Plus','219','20','IA incluída na assinatura, níveis de raciocínio nos modelos compatíveis e acesso ao catálogo de skills.'],
 ['Pro','999','100','Tudo do Plus, com cinco vezes a cota mensal e acesso antecipado a novidades quando disponíveis.'],
 ['Ultra','1.999','200','Tudo do Pro, com dez vezes a cota mensal do Plus e participação opcional em versões experimentais.']
];
export const pricing = `<section class="local-edition"><div><h2>Com a sua chave</h2><p>O aplicativo é gratuito. Você escolhe um modelo compatível e paga o consumo diretamente ao provedor.</p></div><a href="/download/">Baixar a edição local</a></section>
  <section class="subscriptions"><h2>Planos com IA incluída</h2><p>Os preços abaixo são os previstos para o lançamento. Hoje você pode simular a escolha sem cartão ou cobrança.</p>
  <div class="plans">${plans.map(([name,price,quota,desc])=>`<article><h3>${name}</h3><p class="price"><strong>R$ ${price}</strong> por mês</p><p class="allowance">US$ ${quota} de uso de IA por ciclo mensal</p><p>${desc}</p><a href="/conta/?plano=${name.toLowerCase()}">Simular ${name}</a></article>`).join('')}</div></section>
  <section class="quota-example"><div><h2>Como a cota é distribuída?</h2><p>O limite do ciclo é dividido em janelas de até sete dias. Em um ciclo de 30 dias, são quatro janelas completas e uma última de dois dias. A cota disponível vence no fim de cada janela.</p><p>Na conta, você vê o consumo e a próxima liberação. Se os lugares estiverem ocupados, pode aguardar na fila ou cancelar a espera.</p></div>
  <table><caption>Exemplo para um ciclo de 30 dias</caption><thead><tr><th>Plano</th><th>Ciclo completo</th><th>Sete dias*</th></tr></thead><tbody>${plans.map(([n,,q])=>`<tr><th scope="row">${n}</th><td>US$ ${q}</td><td>US$ ${(Number(q)*7/30).toLocaleString('pt-BR',{minimumFractionDigits:2,maximumFractionDigits:2})}</td></tr>`).join('')}</tbody></table><p class="table-note">* Valores aproximados. A duração do ciclo determina a fração liberada em cada janela.</p></section>
  <section class="reading-section"><h2>Renovação e cancelamento</h2><p>A simulação pode ser encerrada em Minha conta. Quando as vendas abrirem, a contratação exibirá o valor total, a renovação mensal e a data da próxima cobrança.</p><p><a href="/reembolso/">Consultar cancelamento e reembolso</a></p></section>`;

const policyResources = [
 ['privacidade','Política de privacidade','Confira os dados usados pelo aplicativo e pela conta, as finalidades e os prazos de conservação.'],
 ['termos','Termos de uso','Entenda as condições do pré-lançamento, a autorização para revisar código e a interpretação dos resultados.'],
 ['reembolso','Cancelamento e reembolso','Saiba como encerrar uma simulação e onde solicitar atendimento sobre cobranças.'],
 ['suboperadores','Fornecedores do serviço','Consulte as empresas que participam da hospedagem, autenticação, distribuição e inferência.'],
 ['seguranca','Segurança dos projetos','Veja o que a revisão pode ler e como o aplicativo apresenta os dados antes de enviá-los.']
];
const directory = rows => '<ul class="guide-directory">'+rows.map(([slug,title,desc])=>`<li><a href="/${slug}/"><strong>${title}</strong><span>${desc}</span></a></li>`).join('')+'</ul>';
const news = Object.fromEntries(changes.map(item=>[`novidades/openkoda-${item.version.replaceAll('.','-')}`,[item.title,
  `<a class="back-link" href="/novidades/">Novidades do OpenKoda</a><p class="story-meta">OpenKoda ${item.version} <span>${date}</span></p><h1>${item.title}</h1><p class="lead">${item.summary}</p><div class="story-body">${item.paragraphs.map(text=>'<p>'+text+'</p>').join('')}<h2>Para usar esta versão</h2><p><a href="${item.more}">${item.label}</a></p><p><a href="${release(item.version)}">Ver o lançamento ${item.version} no GitHub</a></p></div>`]]));

export const editorialPages = {
  novidades:['Novidades',`<h1>Novidades do OpenKoda</h1><p class="lead">Acompanhe as mudanças nas versões para Windows. Cada nota descreve o que você pode fazer no aplicativo e onde encontrar o recurso.</p><section class="news-archive"><h2>Novembro de 2026</h2>${newsItems(changes)}</section>`],
  ...news,
  exemplos:['Exemplos de revisão','<h1>Exemplos de revisão</h1><p class="lead">Investigue um projeto pequeno com o mesmo roteiro que você pode aplicar ao seu próprio código.</p>'+directory([['exemplos/revisao-de-api','Uma fórmula recebida pela API','Examine como uma entrada HTTP chega a uma operação Python em outro arquivo e quais perguntas ainda precisam de resposta.']])],
  'exemplos/revisao-de-api':['Uma fórmula recebida pela API',`<a class="back-link" href="/exemplos/">Exemplos de revisão</a><h1>Uma fórmula recebida pela API</h1><p class="lead">Nesta calculadora de exemplo, uma rota recebe uma expressão de texto. A pergunta da revisão é onde esse texto passa a ser tratado como código.</p>
    <div class="story-body"><h2>Comece pela entrada</h2><p>O escopo contém <code>api.py</code> e <code>math_worker.py</code>. Na primeira leitura, procure a origem da fórmula e o nome da função que a recebe.</p><pre class="story-code"><code>expression = request.args["formula"]\nreturn str(compute_formula(expression))</code></pre>
    <h2>Siga a chamada entre arquivos</h2><p>A linha 6 de <code>api.py</code> lê o parâmetro. No segundo arquivo, <code>compute_formula</code> o passa a <code>eval()</code>. Essa sequência sustenta o candidato apresentado pelo relatório.</p>
    <h2>Confira o que falta provar</h2><p>Quem pode alcançar essa rota? Quais valores são aceitos antes da chamada? Há restrições em um arquivo que ficou fora da revisão? Conferir a cobertura ajuda a escolher o próximo trecho a investigar.</p>
    <h2>Continue no chat</h2><p>Peça a linha que sustenta cada etapa. Quando surgir uma hipótese sobre outro módulo, revise o escopo antes de autorizar um novo envio.</p><a href="/produto/#demonstracao">Explorar o projeto na demonstração</a></div>`],
  politicas:['Privacidade e condições de uso','<h1>Privacidade e condições de uso</h1><p class="lead">Encontre as regras da edição de pré-lançamento e as opções para cuidar dos seus dados.</p>'+directory(policyResources)+'<section class="reading-section"><h2>Pedidos sobre a sua conta</h2><p>Em Minha conta você pode exportar dados, solicitar a exclusão e retirar o consentimento de marketing. Para correções ou dúvidas sobre informações pessoais, entre em contato pelo suporte.</p><p><a href="/guias/conta/#dados">Consultar as opções da conta</a></p></section>']
};
