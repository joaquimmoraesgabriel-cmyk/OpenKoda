const repository = 'https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda';
export const desktopVersion = '1.0.0';
export const desktopInstaller={version:desktopVersion,filename:'OpenKoda_'+desktopVersion+'_x64-setup.exe',url:repository+'/releases/download/desktop-v'+desktopVersion+'/OpenKoda_'+desktopVersion+'_x64-setup.exe',bytes:181850126};
const release = version => repository+'/releases/tag/desktop-v'+version;
const entry = version => '/novidades/openkoda-'+version.replaceAll('.','-')+'/';
const currentEntry = entry(desktopVersion);
const date = version => version === '1.0.0' ? '<time datetime="2026-11-14">14 de novembro de 2026</time>' : '<time datetime="2026-11-13">13 de novembro de 2026</time>';
const download = '<a class="primary" href="/download/">Baixar para Windows</a>';

const changes = [
  {version:'1.0.0',title:'Mais espaço para trabalhar, mais contexto sobre o uso',
   summary:'A barra compacta devolve espaço ao projeto. Ajuste a largura da lista de conversas e consulte o consumo sem sair do chat.',
   paragraphs:[
    'A navegação mostra ícones com nomes ao passar o mouse ou usar o teclado. Arraste a divisória entre os projetos e a conversa para ajustar o espaço. O aplicativo guarda sua escolha neste dispositivo.',
    'O indicador acima de Configurações abre um resumo do uso. Consulte provedores, modelos e a atividade recente. Na conexão da assinatura, ele considera a cota da janela e do ciclo. Com chave própria, apresenta o orçamento local configurado.',
    'Assinantes também podem ajustar o raciocínio em modelos compatíveis usando sua própria chave. Esse consumo continua na conta do provedor. O modo Automático usa o comportamento padrão do modelo.',
    'Esta edição inaugura a numeração 1.0 e continua em pré-lançamento. O checkout público permanece em simulação. As versões anteriores seguem disponíveis no GitHub.'
   ],more:'/guias/uso/',label:'Entender o painel de uso'},
  {version:'0.2.6', title:'As cores do OpenKoda, do site à conversa',
   summary:'O aplicativo ganhou a paleta do site. Você pode escolher outra paleta e ver a sua escolha na área do projeto, no chat e nas configurações.',
   paragraphs:[
    'Em Configurações, abra Aparência e experimente a paleta OpenKoda nos temas claro e escuro. As outras paletas continuam disponíveis. Quem já personalizou fonte, densidade ou papel de parede pode manter essas escolhas.',
    'O modo de navegação por ícones passa a ser respeitado também nas janelas largas. Na lista de achados, alterne a prévia e a visualização em uma linha sem mudar o conteúdo da análise.'
   ],more:'/guias/aparencia/',label:'Ajustar a aparência no Windows'},
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

export function home(demo, connections, readingCards){
  return `<section class="cover interactive-cover">
    <div class="cover-words"><h1>Revise seu código linha por linha.</h1><p>Escolha os arquivos do projeto no Windows. Converse com a IA e confira no código a origem de cada possível problema.</p>
    <div class="actions">${download}<a class="text-action" href="#experimente-openkoda">Experimentar agora</a></div></div>
  </section>
  <div class="edition-band"><a href="${currentEntry}">O que mudou no OpenKoda ${desktopVersion}</a><span>Pré-lançamento para Windows x64</span></div>
  ${demo}
  ${connections}
  ${readingCards}
  <section class="home-news" data-motion><div><h2>Novidades do aplicativo</h2><p>Veja o que mudou nas versões para Windows e consulte os guias dos recursos.</p><a href="/novidades/">Todas as novidades</a></div>${newsItems(changes.slice(0,2))}</section>`;
}

export const screenshot = '<figure class="screen-print" id="aplicativo"><a href="/conversa.png" aria-label="Abrir captura do aplicativo no tamanho original"><img src="/conversa.png" width="1280" height="955" loading="lazy" decoding="async" alt="Projeto Calculadora no OpenKoda, com o histórico do chat e a explicação das duas linhas de código."></a><figcaption>Conversa ilustrativa no aplicativo. Abra a imagem para ver em tamanho original.</figcaption></figure>';

export const launch = `<div class="install-panel"><div><h2>Instalador x64</h2><p>OpenKoda ${desktopVersion} para Windows, em pré-lançamento.</p><p>${Math.ceil(desktopInstaller.bytes/1e6)} MB · Windows 10 e 11</p></div><a class="primary" data-installer download="${desktopInstaller.filename}" href="/download/windows">Baixar instalador x64</a></div>
  <div class="download-assistance"><p id="download-status" role="status">O arquivo será transferido pelo endereço do OpenKoda.</p><p>Se o download não começar, <a href="${desktopInstaller.url}" rel="noopener" target="_blank">baixar diretamente pelo GitHub</a> ou <a href="${release(desktopVersion)}" rel="noopener" target="_blank">abrir a página da versão</a>. Em navegadores dentro de outros aplicativos, abra esta página no seu navegador habitual.</p></div>
  <div class="install-facts"><section><h2>Confira o arquivo</h2><p>Esta prévia é distribuída sem assinatura digital. Nas notas da versão, compare o SHA-256 com o do arquivo baixado antes de instalar.</p><a href="${release(desktopVersion)}">Verificar o instalador no GitHub</a></section>
  <section><h2>Prepare a primeira revisão</h2><p>Separe uma pasta pequena e uma chave do seu provedor de IA. Depois de instalar, abra <strong>Configurações</strong> e escolha <strong>IA e modelos</strong> para cadastrar sua conexão.</p><a href="/guias/primeiro-projeto/">Configurar um projeto</a></section></div>
  <p><a href="/novidades/">Histórico de versões</a> <span class="separate-links"><a href="${repository}">Código-fonte e licença</a></span></p>`;

const planCards = [
 {name:'Gratuito', price:'0', audience:'Para quem já tem acesso à API de um provedor.', highlight:'Use sua própria chave', features:['Projetos e conversas no Windows','Análise dos arquivos autorizados','Histórico de uso e estimativas locais'], href:'/download/', action:'Baixar o aplicativo'},
 {name:'Plus', price:'219', quota:20, audience:'Para quem prefere uma cota incluída e quer trabalhar com skills.', highlight:'US$ 20 de IA por ciclo mensal', features:['Conexão gerenciada pela Koda','Skills da assinatura','Raciocínio nos modelos compatíveis'], href:'/conta/?plano=plus', action:'Simular Plus'},
 {name:'Pro', price:'999', quota:100, audience:'Para quem revisa projetos com mais frequência.', highlight:'US$ 100 de IA por ciclo mensal', features:['Todos os recursos do Plus','Cinco vezes a cota do Plus','Acesso antecipado a novidades disponíveis'], href:'/conta/?plano=pro', action:'Simular Pro'},
 {name:'Ultra', price:'1.999', quota:200, audience:'Para quem precisa de mais uso e quer experimentar versões em teste.', highlight:'US$ 200 de IA por ciclo mensal', features:['Todos os recursos do Pro','Dez vezes a cota do Plus','Participação opcional em versões experimentais'], href:'/conta/?plano=ultra', action:'Simular Ultra'}
];
const comparison = [
 ['Quem fornece a IA','Seu provedor','Koda','Koda','Koda'],
 ['Uso de IA incluído por ciclo','Pago ao seu provedor','US$ 20','US$ 100','US$ 200'],
 ['Projetos e relatórios locais','Incluídos','Incluídos','Incluídos','Incluídos'],
 ['Skills e ajuste de raciocínio¹','Sem acesso','Incluídos','Incluídos','Incluídos'],
 ['Novidades antecipadas','Sem acesso','Sem acesso','Quando disponíveis','Quando disponíveis'],
 ['Versões experimentais','Sem acesso','Sem acesso','Sem acesso','Participação opcional']
];
export const pricing = `<div class="pricing-intro"><strong>Pré-lançamento</strong><p>O download com chave própria está disponível. Os planos pagos têm preços previstos. Hoje você pode simular o checkout sem informar cartão.</p></div>
  <section class="offer-section" aria-label="Escolha do plano"><div class="offer-grid">${planCards.map((p,i)=>`<article class="offer"><h2>${p.name}</h2><p class="offer-audience">${p.audience}</p><p class="offer-price"><strong>R$ ${p.price}</strong> <span>/ mês</span></p><a class="${i===0?'primary':'plan-link'}" href="${p.href}">${p.action}</a><h3>${p.highlight}</h3><ul>${p.features.map(f=>'<li>'+f+'</li>').join('')}</ul></article>`).join('')}</div></section>
  <section class="comparison-section"><h2>Compare os recursos</h2><p class="small-screen-scroll">Deslize a tabela para consultar todos os planos.</p><div class="comparison-scroll" role="region" aria-label="Comparação dos quatro planos" tabindex="0"><table><caption class="sr-only">Recursos do Gratuito, Plus, Pro e Ultra</caption><thead><tr><th scope="col">Recurso</th>${planCards.map(p=>'<th scope="col">'+p.name+'</th>').join('')}</tr></thead><tbody>${comparison.map(([name,...cells])=>`<tr><th scope="row">${name}</th>${cells.map(c=>'<td>'+c+'</td>').join('')}</tr>`).join('')}</tbody></table></div><p class="table-note">¹ Assinantes também podem ajustar o raciocínio com chave própria em conexões e modelos compatíveis. Esse uso é cobrado pelo seu provedor e não desconta a cota de IA incluída.</p></section>
  <section class="quota-example"><div><h2>Como a cota é distribuída?</h2><p>O limite do ciclo é dividido em janelas de até sete dias. Em um ciclo de 30 dias, são quatro janelas completas e uma última de dois dias. A cota disponível vence no fim de cada janela.</p><p>Na conta, você vê o consumo e a próxima liberação. Se os lugares estiverem ocupados, pode aguardar na fila ou cancelar a espera.</p></div>
  <table><caption>Exemplo para um ciclo de 30 dias</caption><thead><tr><th>Plano</th><th>Ciclo completo</th><th>Sete dias*</th></tr></thead><tbody>${planCards.filter(p=>p.quota).map(({name:n,quota:q})=>`<tr><th scope="row">${n}</th><td>US$ ${q}</td><td>US$ ${(Number(q)*7/30).toLocaleString('pt-BR',{minimumFractionDigits:2,maximumFractionDigits:2})}</td></tr>`).join('')}</tbody></table><p class="table-note">* Valores aproximados. A duração do ciclo determina a fração liberada em cada janela.</p></section>
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
  `<a class="back-link" href="/novidades/">Novidades do OpenKoda</a><p class="story-meta">OpenKoda ${item.version} <span>${date(item.version)}</span></p><h1>${item.title}</h1><p class="lead">${item.summary}</p><div class="story-body">${item.paragraphs.map(text=>'<p>'+text+'</p>').join('')}<h2>Para usar esta versão</h2><p><a href="${item.more}">${item.label}</a></p><p><a href="${release(item.version)}">Ver o lançamento ${item.version} no GitHub</a></p></div>`]]));

export const editorialPages = {
  novidades:['Novidades',`<h1>Novidades do OpenKoda</h1><p class="lead">Acompanhe as mudanças nas versões para Windows. Cada nota descreve o que você pode fazer no aplicativo e onde encontrar o recurso.</p><section class="news-archive"><h2>Novembro de 2026</h2>${newsItems(changes)}</section>`],
  ...news,
  exemplos:['Exemplos de revisão','<h1>Exemplos de revisão</h1><p class="lead">Investigue um projeto pequeno com o mesmo roteiro que você pode aplicar ao seu próprio código.</p>'+directory([['exemplos/revisao-de-api','Uma fórmula recebida pela API','Examine como uma entrada HTTP chega a uma operação Python em outro arquivo e quais perguntas ainda precisam de resposta.']])],
  'exemplos/revisao-de-api':['Uma fórmula recebida pela API',`<a class="back-link" href="/exemplos/">Exemplos de revisão</a><h1>Uma fórmula recebida pela API</h1><p class="lead">Nesta calculadora de exemplo, uma rota recebe uma expressão de texto. A pergunta da revisão é onde esse texto passa a ser tratado como código.</p>
    <div class="story-body"><h2>Comece pela entrada</h2><p>O escopo contém <code>api.py</code> e <code>math_worker.py</code>. Na primeira leitura, procure a origem da fórmula e o nome da função que a recebe.</p><pre class="story-code"><code>expression = request.args["formula"]\nreturn str(compute_formula(expression))</code></pre>
    <h2>Siga a chamada entre arquivos</h2><p>A linha 4 de <code>api.py</code> lê o parâmetro. No segundo arquivo, <code>compute_formula</code> o passa a <code>eval()</code>. Essa sequência sustenta o candidato apresentado pelo relatório.</p>
    <h2>Confira o que falta provar</h2><p>Quem pode alcançar essa rota? Quais valores são aceitos antes da chamada? Há restrições em um arquivo que ficou fora da revisão? Conferir a cobertura ajuda a escolher o próximo trecho a investigar.</p>
    <h2>Continue no chat</h2><p>Peça a linha que sustenta cada etapa. Quando surgir uma hipótese sobre outro módulo, revise o escopo antes de autorizar um novo envio.</p><a href="/produto/#demonstracao">Explorar o projeto na demonstração</a></div>`],
  politicas:['Privacidade e condições de uso','<h1>Privacidade e condições de uso</h1><p class="lead">Encontre as regras da edição de pré-lançamento e as opções para cuidar dos seus dados.</p>'+directory(policyResources)+'<section class="reading-section"><h2>Pedidos sobre a sua conta</h2><p>Em Minha conta você pode exportar dados, solicitar a exclusão e retirar o consentimento de marketing. Para correções ou dúvidas sobre informações pessoais, entre em contato pelo suporte.</p><p><a href="/guias/conta/#dados">Consultar as opções da conta</a></p></section>']
};
