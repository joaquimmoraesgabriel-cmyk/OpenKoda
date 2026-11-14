const groups = [
 {title:'Produto', pages:[
  ['produto','Visão geral','Conheça o aplicativo para Windows.'],
  ['exemplos','Exemplos de revisão','Siga um caso do código ao relatório.'],
  ['conexoes','Conexões de IA','Compare credencial pessoal e assinatura.'],
  ['download','Download','Baixe e confira o instalador.']]},
 {title:'Guias',pages:[
  ['guias','Todos os guias','Encontre a etapa em que está.'],
  ['guias/primeiro-projeto','Primeiro projeto','Escolha a pasta, o modelo e o objetivo.'],
  ['guias/relatorios','Ler o relatório','Confira arquivos, linhas e cobertura.'],
  ['guias/conversas','Conversa e contexto','Use anexos e retome uma resposta.'],
  ['guias/uso','Histórico de uso','Consulte tokens, custos e janelas.']]},
 {title:'Ajuda',pages:[
  ['suporte','Falar com o suporte','Envie um chamado ou relate um problema.'],
  ['perguntas','Perguntas frequentes','Consulte respostas sobre o aplicativo.'],
  ['seguranca','Segurança dos projetos','Veja os limites da leitura de código.'],
  ['politicas','Central de políticas','Consulte privacidade, termos e reembolso.']]}
];
function link(route,path,label,description) {
 return `<a href="/${path}/"${route===path?' aria-current="page"':''}>${label}${description?`<small>${description}</small>`:''}</a>`;
}
function disclosure(route,group) {
 return `<details class="nav-group"><summary>${group.title}</summary><div class="nav-flyout">${group.pages.map(([path,label,desc])=>link(route,path,label,desc)).join('')}</div></details>`;
}
export function menu(route) {
 return `<nav id="primary-nav" aria-label="Principal">${disclosure(route,groups[0])}${disclosure(route,groups[1])}${link(route,'precos','Preços')}${link(route,'novidades','Novidades')}${disclosure(route,groups[2])}</nav><div class="nav-actions"><a class="login" href="/conta/"${route==='conta'?' aria-current="page"':''}>Minha conta</a><button id="menu-toggle" type="button" aria-label="Menu principal" aria-expanded="false" aria-controls="primary-nav">Menu</button></div>`;
}
export function footer(brand,route) {
 const blocks=[
  ['Aplicativo',[['produto','Funcionalidades'],['download','Baixar para Windows'],['novidades','Notas de versão'],['precos','Planos e cotas']]],
  ['Aprender',[['guias','Guias do OpenKoda'],['exemplos','Revisões de exemplo'],['conexoes','Escolher uma conexão'],['seguranca','Segurança dos projetos']]],
  ['Atendimento',[['suporte','Contato e suporte'],['perguntas','Perguntas frequentes'],['guias/conta','Conta e chamados']]],
  ['Privacidade e termos',[['politicas','Todas as políticas'],['privacidade','Privacidade'],['termos','Termos de uso'],['reembolso','Cancelamento e reembolso'],['suboperadores','Fornecedores']]]
 ];
 return `<footer class="site-footer"><div class="footer-inner"><div class="footer-brand">${brand}<small>Koda, Brasil</small></div><div class="footer-groups">${blocks.map(([title,items])=>`<nav aria-label="${title}"><strong>${title}</strong>${items.map(([path,label])=>link(route,path,label)).join('')}</nav>`).join('')}</div><div class="footer-tools"><button id="cookies" type="button">Preferências de cookies</button><div class="site-appearance"><label for="site-appearance">Aparência</label><select id="site-appearance"><option value="system">Do sistema</option><option value="light">Clara</option><option value="dark">Escura</option></select></div></div></div></footer>`;
}
