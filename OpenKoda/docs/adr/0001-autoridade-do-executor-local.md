---
status: proposed
scope: OpenKoda desktop de cibersegurança
---
# Autoridade operacional no executor

Propõe-se que toda operação gerida atravesse o motor de política e seja revalidada pelo executor com escopo, revisão, limites e autorização concretos. Isso permite automação previamente delimitada sem depender de pop-ups por comando e mantém a autoridade fora do texto do modelo ou de um toggle da interface. A alternativa de expor host exec genérico pela UI/MCP tornaria mais difícil impor o mesmo contrato; clientes futuros terão identidades próprias. Esta proposta é local à subpasta e não altera a autoridade do servidor definida no projeto pai.
