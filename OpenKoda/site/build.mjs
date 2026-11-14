import { mkdir, readFile, writeFile, copyFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { productChapters, publicPages } from './product-pages.mjs';
import { home, launch, pricing, screenshot, editorialPages } from './editorial.mjs';

import { walkthrough, connections, readingCards, connectionPages } from './showcase.mjs';
import { menu, footer } from './navigation.mjs';

const output = fileURLToPath(new URL('./dist/', import.meta.url));
const contact = 'joaquimmoraesgabriel@gmail.com';
const mail = (subject, label) => '<a href="mailto:' + contact + '?subject=' + encodeURIComponent(subject) + '">' + label + '</a>';
const mark = '<svg viewBox="0 0 26 24" width="26" height="24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="7" stroke-linecap="round" stroke-linejoin="round"><path d="M3.5 3.5v17M21.5 3.5l-9 8.5 9 8.5"/></svg>';
const brand = '<a class="brand" href="/" aria-label="OpenKoda, início">'+mark+'<span>OpenKoda</span></a>';
const download = '<a class="primary" href="/download/">Baixar para Windows</a>';
const compatibility = '<section class="resource-shelf" data-motion><div><h2>Escolha a conexão da sua IA.</h2><p>Selecione o modelo adequado ao tipo de trabalho. O catálogo disponível depende do provedor e da sua credencial.</p><a href="/guias/primeiro-projeto/#conexao">Configurar uma conexão</a></div><dl class="capabilities"><dt>Conversa</dt><dd>Um modelo de texto pode explicar o código e responder perguntas sobre os trechos autorizados.</dd><dt>Análise de código</dt><dd>Respostas estruturadas permitem organizar candidatos, arquivos e linhas no relatório.</dd><dt>Imagens</dt><dd>Para conversar sobre uma captura de tela, escolha um modelo que receba entrada visual.</dd></dl></section>';
const intro = (title, lead) => '<h1>' + title + '</h1><p class="lead">' + lead + '</p>';
const pages = {
 ...publicPages, ...editorialPages, ...connectionPages,
 '': ['Revisão de código para Windows', home(walkthrough, connections, readingCards)],
 produto: ['Funcionalidades', intro('Do projeto ao relatório', 'Use a revisão para investigar uma pergunta concreta. Escolha a entrada de dados, acompanhe os trechos citados e consulte o histórico quando retomar o projeto.') + walkthrough + '<section class="project-feature" data-motion><div><h2>Dentro do aplicativo</h2><p>O projeto reúne as conversas e o material autorizado na investigação. Abra a captura para ver o chat e os arquivos citados em detalhe.</p>'+download+'</div>'+screenshot+'</section>'+compatibility],
 download: ['Download', intro('OpenKoda para Windows', 'Revise código e converse sobre um projeto com uma chave de IA própria.') + launch],
 precos: ['Preços', intro('Escolha como usar a IA', 'Use sua própria chave na edição local ou conheça as cotas previstas para os planos da Koda.') + pricing],
 suporte: ['Suporte', intro('Fale com a Koda.', 'Conte em qual tela o problema aconteceu e o que você esperava ver. Para relatar um teste bem-sucedido, informe o modelo e o recurso usado.') + '<h2>Enviar um relato</h2><p>Inclua a versão do OpenKoda e passos para reproduzir. Revise o texto e remova chaves, senhas e código confidencial antes de enviar.</p><p><a href="/conta/?acao=suporte">Abrir chamado na conta</a> ou '+mail('Suporte OpenKoda','escrever por e-mail')+'.</p><h2>Privacidade e segurança</h2><p>Para solicitar seus dados, exclusão, informar uma falha de segurança ou retirar seu consentimento de marketing, escreva para '+mail('Privacidade OpenKoda',contact)+'. Para verificar a titularidade, podemos pedir uma confirmação pelo endereço da conta.</p><p>A Koda atende no Brasil durante o pré-lançamento. Não há prazo de resposta garantido.</p>'],
 privacidade: ['Privacidade', intro('Sua privacidade.', 'Informações do pré-lançamento da Koda no Brasil. Versão de 13 de novembro de 2026.') + '<h2>Dados e destinos</h2><p>A versão gratuita guarda projetos, conversas e histórico no dispositivo. Ao escolher um provedor de IA, os trechos e anexos autorizados seguem para esse provedor. Revise as condições dele antes de enviar dados de terceiros.</p><p>Os serviços de conta usam e-mail, identificador da conta e registros de consentimento. Chamados contêm o texto que você envia. O checkout demonstrativo não solicita cartão. A IA gerenciada prevê roteamento pelo OpenRouter aos fornecedores dos modelos selecionados, sujeito às condições de privacidade da rota. O lançamento pago depende de validar restrições de treinamento e retenção.</p><h2>Cookies e marketing</h2><p>O site usa armazenamento necessário para a sessão e para lembrar a escolha sobre cookies. Não instalamos ferramentas de publicidade ou analytics. Marketing é opcional, separado do aceite dos termos. Solicite o descadastro em '+mail('Cancelar marketing',contact)+'.</p><h2>Conservação e direitos</h2><p>Você controla os dados locais no computador. Para os serviços hospedados, o banco prevê remoção de chamados encerrados após 30 dias e de simulações de checkout após 90 dias. Os identificadores de rede e de geração têm retenção de até 30 dias após o marco aplicável. Registros de uso já conciliados têm retenção de até 90 dias. Valores pendentes ficam reservados até a conciliação. Prazos de backups e obrigações fiscais serão informados antes das vendas. Solicite acesso, correção, exportação ou exclusão pelo suporte. A exclusão da conta na nuvem não remove arquivos do computador.</p><p>Contas nesta etapa destinam-se a maiores de 18 anos. A Koda não oferece cadastro infantil neste lançamento. Um responsável pode pedir a remoção de dados enviados por engano.</p><p><a href="/suboperadores/">Consultar fornecedores e finalidades</a></p>'],
 termos: ['Termos de uso', intro('Termos de uso.', 'Condições de pré-lançamento. Versão de 13 de novembro de 2026.') + '<h2>Uso autorizado</h2><p>Use o aplicativo com código e dados que você tem autorização para analisar e compartilhar com os provedores escolhidos. Mantenha as credenciais pessoais em segurança. A conta comercial destina-se a maiores de 18 anos.</p><h2>Resultados de IA</h2><p>O relatório indica candidatos a problemas. Confira os trechos, o contexto de execução e as permissões antes de aplicar correções. Resultados podem ser incompletos ou incorretos.</p><h2>Disponibilidade e contratação</h2><p>O serviço pode sofrer interrupções. Não há compromisso percentual de disponibilidade no pré-lançamento. Esta versão do checkout não realiza cobranças. Preços, cotas e renovação deverão ser apresentados no momento de uma futura contratação. Os direitos aplicáveis do consumidor são preservados.</p><h2>Licenças e contato</h2><p>O código aberto mantém suas licenças. Os avisos de componentes acompanham o aplicativo. O catálogo de skills inclui material de Matt Pocock distribuído sob licença MIT. Para questões de direitos autorais, informe a obra, a localização do conteúdo e um contato para retorno. '+mail('Direitos autorais OpenKoda',contact)+'.</p>'],
 reembolso: ['Cancelamento e reembolso', intro('Cancele com clareza.', 'O checkout de pré-lançamento não cobra e não cria uma assinatura financeira.') + '<h2>Simulação</h2><p>Abra Minha conta para consultar ou encerrar uma simulação. Nenhum cartão é necessário.</p><h2>Após a abertura das vendas</h2><p>O checkout deverá informar preço total, prazo, renovação e forma de cancelar antes da confirmação. Em contratações à distância, a Koda respeitará o direito de arrependimento previsto no art. 49 do Código de Defesa do Consumidor quando aplicável. Um pedido de reembolso pode ser enviado diretamente ao suporte. Guarde o protocolo ou uma cópia do e-mail.</p><p>'+mail('Cancelamento ou reembolso','Solicitar atendimento')+'</p>'],
 suboperadores: ['Fornecedores', intro('Quem participa do serviço.', 'Fornecedores previstos ou utilizados nesta edição. A lista será atualizada antes de novas integrações em produção.') + '<dl><dt>Supabase</dt><dd>Autenticação e banco de dados da plataforma. O projeto Koda está na região de São Paulo.</dd><dt>Sites</dt><dd>Hospedagem desta apresentação pública.</dd><dt>Resend</dt><dd>Envio previsto de mensagens transacionais após a verificação do domínio remetente.</dd><dt>Google</dt><dd>Autenticação quando você escolhe o login Google. O suporte indicado usa Gmail.</dd><dt>Cloudflare</dt><dd>Turnstile para proteção dos formulários públicos.</dd><dt>OpenRouter e provedores dos modelos</dt><dd>Inferência de IA. O provedor efetivo depende do modelo e da política de roteamento. A versão gratuita envia solicitações ao provedor escolhido pelo usuário.</dd><dt>GitHub</dt><dd>Distribuição do instalador e do código-fonte.</dd><dt>Asaas</dt><dd>Integração de pagamento prevista. A simulação não envia informações ao Asaas.</dd></dl>'],
 conta: ['Minha conta', '<div class="account-shell"><div id="account" aria-busy="true">Carregando a área de conta…</div><noscript>Ative JavaScript para entrar.</noscript></div>'],
 'conta/retorno': ['Retorno à conta', '<div class="account-shell"><div id="account" aria-busy="true">Validando retorno…</div></div>']
};

pages.produto[1]+=productChapters;
pages.suporte[1]+='<section class="reading-section"><h2>Encontre uma orientação</h2><p><a href="/guias/primeiro-projeto/#conexao">Configurar um provedor</a>, <a href="/guias/relatorios/#cobertura">entender uma análise parcial</a> ou <a href="/guias/uso/">conferir custos e limites</a>.</p><p><a href="/perguntas/">Perguntas frequentes</a></p></section>';

const policySections = {
 privacidade: ['dados-e-destinos','cookies-e-marketing','conservacao-e-direitos'],
 termos: ['uso-autorizado','resultados','contratacao','licencas'],
 reembolso: ['simulacao','vendas']
};
for (const [route, ids] of Object.entries(policySections)) {
 let index=0;
 const links=[];
 const content=pages[route][1].replace(/<h2>([^<]+)<\/h2>/g,(_,title)=>{
  const id=ids[index++];
  if(!id)throw Error('Seção legal sem destino em '+route);
  links.push('<li><a href="#'+id+'">'+title+'</a></li>');
  return '<h2 id="'+id+'">'+title+'</h2>';
 });
 const start=content.indexOf('<h2');
 pages[route][1]='<a class="back-link" href="/politicas/">Central de políticas</a>'+content.slice(0,start)+
  '<div class="policy-layout"><nav class="lesson-index" aria-label="Nesta página"><h2>Nesta página</h2><ul>'+links.join('')+
  '</ul></nav><div class="policy-text">'+content.slice(start)+'</div></div>';
}
pages.suboperadores[1]='<a class="back-link" href="/politicas/">Central de políticas</a>'+pages.suboperadores[1];

for (const [route,[name,body]] of Object.entries(pages)) {
 const directory=output+route;
 await mkdir(directory,{recursive:true});
 const layout=route.startsWith('conta')?'account-page':route in policySections?'policies inner':route==='novidades'||route==='politicas'||route==='exemplos'?'collection inner':route.startsWith('novidades/')||route.startsWith('exemplos/')?'document story inner':route==='produto'?'product inner':route==='precos'?'pricing inner':
  route==='conexoes'?'product inner':route.startsWith('conexoes/')?'document story inner':route==='guias' || route.startsWith('guias/')?'guides inner':route?'document inner':'home';
 const html=`<!doctype html><html lang="pt-BR"><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="referrer" content="no-referrer"><meta http-equiv="Content-Security-Policy" content="default-src 'self'; script-src 'self' https://challenges.cloudflare.com; frame-src https://challenges.cloudflare.com; style-src 'self'; img-src 'self'; connect-src 'self' https://ynjcyvnrfxsrijrwpaye.supabase.co https://challenges.cloudflare.com; object-src 'none'; base-uri 'none'; form-action 'self'"><title>${name} | OpenKoda</title><link rel="stylesheet" href="/style.css"><link rel="stylesheet" href="/tema.css"><script src="/tema.js" defer></script><body><a class="skip" href="#main">Pular para o conteúdo</a><header class="site-header"><div class="header-inner">${brand}${menu(route)}</div></header><main id="main" tabindex="-1" class="${layout}">${body}</main>${footer(brand,route)}<aside id="cookie-box" hidden aria-label="Cookies e armazenamento"><p>Usamos cookies essenciais para sua sessão e a verificação de segurança. Consulte a <a href="/privacidade/#cookies-e-marketing">política de privacidade</a>.</p><button type="button" id="accept-essential">Entendi</button></aside><script type="module" src="/site.js"></script>${route.startsWith('conta')?'<script type="module" src="/account.js"></script>':''}</body></html>`;
 await writeFile(directory+'/index.html',html.replace('<title>', '<link rel="icon" type="image/svg+xml" href="/favicon.svg"><title>'));
}

// Serve the same reviewed files through a Worker so security headers also
// protect direct visits to the OAuth callback and account forms.
const files = [...Object.keys(pages).map(route=>(route?route+'/':'')+'index.html'),
 'style.css','tema.css','tema.js','account.js','site.js','cloud.js','painel.png','conversa.png','projeto.png','relatorio.png','favicon.svg'];
await writeFile(output+'favicon.svg',mark.replace('<svg ', '<svg xmlns="http://www.w3.org/2000/svg" ').replace('currentColor', '#171719'));
const assets = {};
for (const file of files) {
 const type = file.endsWith('.html')?'text/html; charset=utf-8':file.endsWith('.css')?'text/css; charset=utf-8':
  file.endsWith('.png')?'image/png':file.endsWith('.svg')?'image/svg+xml':'text/javascript; charset=utf-8';
 assets['/'+file] = {type,body:(await readFile(output+file)).toString(file.endsWith('.png')?'base64':'utf8')};
}
assets['/favicon.ico'] = assets['/favicon.svg'];
const handler = await readFile(new URL('./worker.mjs',import.meta.url),'utf8');
await mkdir(output+'server',{recursive:true});
await mkdir(output+'.openai',{recursive:true});
await copyFile(new URL('./.openai/hosting.json',import.meta.url),output+'.openai/hosting.json');
await writeFile(output+'server/index.js','const websiteAssets = '+JSON.stringify(assets)+';\n'+handler);
