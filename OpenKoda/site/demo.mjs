// The preview is a closed, browser-local product tour. No user input leaves it.
export const interactiveDemo = `<section class="koda-tour" id="experimente-openkoda" aria-labelledby="tour-title">
  <div class="tour-heading"><div><p class="section-name">Entre no projeto</p><h2 id="tour-title">Experimente o OpenKoda.</h2></div><p>Crie um projeto, converse e explore as configurações. Tudo acontece nesta demonstração.</p></div>
  <div class="koda-demo" data-koda-demo>
    <div class="kd-topline"><span class="kd-wordmark"><svg viewBox="0 0 26 24" width="18" height="18" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="7" stroke-linecap="round" stroke-linejoin="round"><path d="M3.5 3.5v17M21.5 3.5l-9 8.5 9 8.5"/></svg>OpenKoda 1.0</span><span class="kd-mode-label">Ambiente de demonstração</span><button type="button" class="kd-reset" data-demo-reset>Recomeçar</button></div>
    <div class="kd-workspace"><nav class="kd-rail" aria-label="Navegação da demonstração"></nav><aside class="kd-tree" aria-label="Projetos da demonstração"></aside><div class="kd-main" tabindex="-1"></div></div>
    <div class="kd-disclosure"><span class="kd-demo-dot" aria-hidden="true"></span>Demonstração interativa · dados fictícios · sem uso de IA</div>
    <div class="kd-announcer kd-sr" role="status" aria-live="polite"></div>
    <noscript><p class="kd-no-script">Ative JavaScript para explorar a demonstração. Você também pode conhecer o aplicativo nos <a href="/guias/">guias ilustrados</a>.</p></noscript>
  </div>
  <p class="tour-footnote">Os arquivos e as respostas são exemplos preparados. Nenhuma pasta, chave ou URL digitada aqui é acessada ou enviada.</p>
</section>`;
