---
status: accepted
scope: OpenKoda desktop de cibersegurança
date: 2026-11-07
---
# Tauri como framework desktop

O usuário escolheu explicitamente Tauri após a comparação com Electron, antes de iniciar a construção. O mapa passa a usar uma interface WebView e uma camada nativa Tauri para separar apresentação de serviços locais; a arquitetura favorece a proposta de núcleo Rust e evita incluir o Chromium do Electron na distribuição da UI. Essa é uma escolha de integração e distribuição, não uma garantia de menor consumo ou de isolamento do Kali.

A decisão aceita é **usar Tauri**. Tauri 2, React/TypeScript, bibliotecas, empacotamento, comandos IPC e a linguagem do executor Linux continuam sujeitos à validação nas fases futuras. O desktop deve preservar a autoridade do núcleo/executor e a verificação de escopo; o renderer não ganha shell genérico nem leitura do vault pela escolha do framework. Os seis conceitos aprovados orientam o acabamento segundo [VISUAL-E-TELAS](../../planejamento/VISUAL-E-TELAS.md). Nenhuma aplicação é implementada por este registro.
