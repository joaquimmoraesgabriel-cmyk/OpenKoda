# Usar a versão 0.1.8 do OpenKoda

## Comparar uma alteração de plano sugerida pela IA

1. Abra uma avaliação com um método de observação autorizado. Na sugestão de operação, selecione uma evidência, descreva o objetivo e escolha quais operações o modelo pode sugerir. Inventário local, regras de código e cabeçalhos HTTP admitem a alteração de plano.
2. Use **Revisar contexto da sugestão**. Confira os dados, o provedor, o modelo e o orçamento antes de solicitar a sugestão.
3. Quando a resposta oferecer uma alteração, use **Comparar alteração sugerida no plano**. Confira a evidência, o trabalho concluído e os dois novos passos. O primeiro pede uma decisão humana sobre a evidência. O segundo prepara a observação autorizada se essa decisão permitir prosseguir.
4. **Salvar alteração no plano** registra a revisão. Abra a fila para revisar cada passo. Mudanças na evidência, no escopo ou na revisão do plano exigem nova comparação. O histórico conserva o que já foi executado.

Se aparecer apenas uma sugestão de operação, você pode revisá-la pelos controles da avaliação. A opção de alterar o plano depende também da resposta do modelo, do estado do trabalho e das cotas de tentativas e captura.

## Retestar uma configuração JSON

Em um achado local já validado, abra uma proposta de correção. Escolha o arquivo dentro do alvo autorizado e selecione **Desativar o campo debug de uma configuração JSON**. O arquivo original precisa conter o campo booleano `"debug": true` na raiz de um objeto JSON. No conteúdo proposto, o valor deve ser `false`.

Confira a alteração e a identidade do arquivo antes de aprovar a gravação. Use **Verificar o campo debug no JSON** e confira a evidência. A confirmação humana final vincula o achado ao arquivo e ao hash observados. Para saber se um serviço em execução já carregou essa configuração, é preciso observar o próprio serviço.

## Recuperar dados

Para projetos, avaliações e evidências do aplicativo, use o backup em **Configurações**. Para arquivos da distribuição gerida, consulte o [guia de cópias do Kali](KALI-BACKUP.md). A retenção de capturas tem seu próprio [guia de recuperação de espaço](GUIA-0.1.7.md).
