import {desktopVersion} from './editorial.mjs';
// The embedded build mounts the desktop App with a browser-local service adapter.
export const interactiveDemo = `<section class="koda-tour" id="experimente-openkoda" aria-labelledby="tour-title">
  <div class="tour-heading"><div><p class="section-name">Experimente no navegador</p><h2 id="tour-title">Entre no projeto.</h2></div><p>Esta é a interface do OpenKoda. Escreva uma pergunta, abra o relatório ou crie um projeto de exemplo.</p></div>
  <div class="koda-demo" data-koda-demo>
    <div class="kd-topline"><span class="kd-wordmark">OpenKoda ${desktopVersion} <span>· demonstração</span></span><div class="kd-toolbar"><a href="#depois-da-demonstracao" class="kd-skip">Pular demonstração</a><button type="button" class="kd-reset" data-demo-reset>Recomeçar</button></div></div>
    <div class="kd-frame-wrap"><iframe src="/demo-app/" title="Demonstração interativa do aplicativo OpenKoda" sandbox="allow-scripts allow-forms" referrerpolicy="no-referrer" loading="lazy"></iframe></div>
    <div class="kd-disclosure"><span>Dados fictícios · respostas preparadas · sem consumo de IA</span><span class="kd-status" role="status" aria-live="polite"></span></div>
    <noscript><p>Explore o aplicativo nos <a href="/guias/">guias ilustrados</a>.</p></noscript>
  </div>
  <div class="tour-footnote" id="depois-da-demonstracao" tabindex="-1"><p>A interface é a mesma do aplicativo. Nesta janela, pastas, arquivos, conexões e ações usam exemplos locais. Reiniciar restaura todos os exemplos.</p><a href="/guias/primeiro-projeto/">Como começar com seu código</a></div>
</section>`;
