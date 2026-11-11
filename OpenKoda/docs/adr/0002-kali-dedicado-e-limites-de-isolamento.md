---
status: proposed
scope: OpenKoda desktop de cibersegurança
---
# Kali dedicado com perfis de execução verificáveis

O usuário escolheu Kali no WSL 2. Propõe-se uma distro dedicada e gerida para separar manutenção e dados pessoais, com usuário de avaliação sem root e perfis habilitados somente após provar os controles requeridos. A distro não será anunciada como sandbox forte contra o Windows. Tarefas que exigem outro limite de segurança deverão ser recusadas nesse perfil ou usar backend separado explicitamente suportado. Adoção de distro pessoal favorece conveniência, mas dificulta identidade, manutenção e recuperação; por isso não é a base automática.
