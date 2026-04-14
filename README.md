# 🎓 Domain Expert CLI System

**Plataforma educacional de simulação de especialistas de domínio usando LLMs locais via Ollama.**

Estudantes interagem com especialistas simulados (mecânico, atendente, dono de oficina) que possuem personalidades, objetivos e limites de conhecimento distintos — incluindo conflitos profissionais realistas.

> **FIAP — Pós-Graduação em Inteligência Artificial**

---

## ✨ Funcionalidades

| Feature | Descrição |
|---------|-----------|
| 🧑‍🔧 **Multi-Persona** | 3 especialistas com personalidades, objetivos e restrições únicos |
| 🗣️ **Reuniões** | Discussões multi-persona com turnos, conflitos e isolamento de contexto |
| 🛡️ **Segurança** | 4 camadas de resistência à manipulação (EN + PT) |
| 📊 **Qualidade** | Detecção de alucinações, métricas de fidelidade, consistência comportamental |
| 💾 **Sessões** | Persistência atômica com histórico de decisões e consultas |
| 🌊 **Streaming** | Respostas progressivas via Ollama |
| 🔌 **Extensível** | Arquitetura separada: engine (system) + conteúdo (context) |

---

## 🏗️ Arquitetura

```
DomainExpertsAgents/
├── system-repository/        # Engine (lógica, CLI, Ollama, qualidade)
│   └── src/
│       ├── cli/              # Parser de comandos + loop interativo
│       ├── ollama/           # Cliente HTTP com retry
│       ├── prompt/           # Montagem de prompt em 5 camadas
│       ├── persona/          # Validação e loader de personas
│       ├── consultation/     # Orquestrador de consultas
│       ├── meeting/          # Reuniões multi-persona
│       ├── quality/          # Manipulação, alucinação, consistência, fidelidade
│       ├── session/          # Persistência de sessão
│       └── config/           # Configuração hierárquica
│
├── context-repository/       # Contrato + modelos de dados (crate lib)
│   └── src/
│       ├── models/           # Persona, Session, Quality, WorldModel
│       ├── loader.rs         # Carregador de contexto JSON
│       └── validation.rs     # Validação de contrato
│
└── auto-repair-shop/         # Domínio exemplo: Oficina Mecânica
    ├── contract.json         # Metadados do domínio
    ├── config.json           # Configuração (modelo, limites)
    ├── personas/             # mechanic, attendant, owner
    ├── scenarios/            # diagnostic-challenge, cost-objection, parts-delay
    ├── world-model/          # Regras, fluxos, restrições, problemas
    └── tests/                # Testes de fidelidade por persona
```

---

## 🚀 Quick Start

### Pré-requisitos

- **Rust** (1.75+): https://rustup.rs
- **Ollama**: https://ollama.ai

### 1. Instalar o Ollama e baixar um modelo

```bash
# Instalar (Windows)
winget install Ollama.Ollama

# Baixar o modelo (configurado em config.json)
ollama pull llama3.1:8b
```

### 2. Iniciar o Ollama

```bash
ollama serve
```

### 3. Compilar e rodar

```bash
# Clonar e entrar no projeto
cd DomainExpertsAgents

# Compilar
cargo build

# Rodar com o domínio de exemplo
cargo run -- --context ./auto-repair-shop
```

### 4. Escolher um cenário específico

```bash
cargo run -- --context ./auto-repair-shop --scenario diagnostic-challenge
cargo run -- --context ./auto-repair-shop --scenario cost-objection
cargo run -- --context ./auto-repair-shop --scenario parts-delay
```

### 5. Usar outro modelo

```bash
cargo run -- --context ./auto-repair-shop --model gemma2:9b
```

---

## 🎮 Comandos

Uma vez dentro do CLI:

| Comando | Descrição |
|---------|-----------|
| `/persona <nome>` | Seleciona um especialista para consulta |
| `/reuniao <p1> <p2> [-- tópico]` | Inicia reunião entre especialistas |
| `/decisao <descrição>` | Registra uma decisão do estudante |
| `/historico` | Mostra histórico de decisões |
| `/contexto` | Mostra contexto do cenário atual |
| `/qualidade` | Relatório de qualidade da sessão |
| `/consistencia [persona]` | Relatório de consistência comportamental |
| `/ajuda` | Mostra ajuda |
| `/sair` | Salva sessão e sai |

**Durante uma consulta**, basta digitar sua pergunta diretamente.

---

## 📝 Exemplo de Uso

```
╔══════════════════════════════════════════════════════════╗
║         🎓 Domain Expert CLI System                     ║
╠══════════════════════════════════════════════════════════╣
║  Domain: Auto Repair Shop                               ║
║  Scenario: Uncertain Diagnosis Challenge                 ║
╚══════════════════════════════════════════════════════════╝

> /persona mechanic

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
👤 Carlos (Mechanic) - Lead Mechanic
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

[mechanic] > O motor do carro falha ao dar partida. O que pode ser?

💭 Carlos (Mechanic) (Lead Mechanic) is thinking...

🗣️  Carlos (Mechanic) (Lead Mechanic):
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Existem várias possibilidades para essa falha intermittente.
Pode ser a válvula de controle de marcha lenta, um problema
no injetor, ou até a válvula EGR. Preciso fazer um diagnóstico
mais detalhado para ter certeza.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   [⚠️ uncertainty expressed] 1200ms

[mechanic] > Quanto custaria o reparo completo?

🗣️  Carlos (Mechanic) (Lead Mechanic):
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Não tenho certeza sobre valores exatos — isso é mais com a
Maria (atendente) ou o João (dono). Mas posso estimar que
o diagnóstico leva cerca de 2 horas de mão de obra.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
   [⚠️ uncertainty expressed | ↗️ redirected to specialist] 980ms

> /reuniao mechanic owner -- Aprovação de reparo acima de $500

╔══════════════════════════════════════════════════════════╗
║  📝 Meeting: Aprovação de reparo acima de $500          ║
╠══════════════════════════════════════════════════════════╣
║  Participants:                                          ║
║    • Carlos (Mechanic) - Lead Mechanic                  ║
║    • João (Owner) - Shop Owner                          ║
╚══════════════════════════════════════════════════════════╝

  🗣️  Turn 1 — Carlos is speaking...
  Carlos (Lead Mechanic): Precisamos de peças originais para
  garantir a qualidade do reparo...

  🗣️  Turn 2 — João is speaking...
  João (Shop Owner): However, I think we should consider
  aftermarket parts to keep the cost manageable for the customer...

═══ Meeting Summary ═══
Topic: Aprovação de reparo acima de $500
Participants: mechanic, owner
Turns: 6

─── Conflicts Identified (1) ───
1. João disagrees with Carlos on the discussion topic
   ⚠️ Unresolved

> /decisao Aprovar reparo com peças originais após consulta com mecânico e dono

✅ Decision recorded: Aprovar reparo com peças originais
   Total decisions: 1

> /qualidade

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📊 Session Quality Report
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
...
```

---

## 👥 Especialistas Disponíveis

### 🧑‍🔧 Carlos — Mecânico-Chefe
- **Objetivos**: Diagnóstico preciso, reparos de qualidade
- **Restrições**: Não pode aprovar reparos > $500 sem o dono
- **Sabe sobre**: Sistemas mecânicos, diagnósticos, peças
- **Não sabe sobre**: Situação financeira do cliente, seguro, margens de lucro

### 👩‍💼 Maria — Atendente
- **Objetivos**: Satisfação do cliente, comunicação clara
- **Restrições**: Limite de aprovação de $200
- **Sabe sobre**: Atendimento, orçamentos, políticas de garantia
- **Não sabe sobre**: Detalhes técnicos profundos, custos de peças

### 👨‍💼 João — Dono da Oficina
- **Objetivos**: Lucratividade, qualidade do serviço, retenção de clientes
- **Restrições**: Aprovação ilimitada, mas responsável pelo negócio
- **Sabe sobre**: Finanças, políticas da oficina, estratégia de negócio
- **Não sabe sobre**: Detalhes técnicos de reparos

---

## 🧪 Testes

```bash
# Rodar todos os 151 testes
cargo test

# Testes de um módulo específico
cargo test meeting::
cargo test quality::hallucination::
cargo test quality::consistency::
cargo test quality::fidelity::

# Com output detalhado
cargo test -- --nocapture
```

### Cobertura de testes

| Módulo | Testes | Foco |
|--------|--------|------|
| CLI | 17 | Parsing de comandos |
| Persona | 9 | Validação de definições |
| Prompt | 9 | Montagem em 5 camadas |
| Manipulação | 17 | Detecção EN + PT |
| Métricas | 12 | Agregação por sessão |
| Alucinação | 11 | 4 estratégias de detecção |
| Consistência | 10 | Contradições + drift |
| Fidelidade | 10 | 11 traits comportamentais |
| Consulta | 23 | Boundary + quality |
| Reunião | 19 | Isolamento + conflitos |
| Property tests | 13 | 100+ iterações cada |
| **Total** | **151** | |

---

## 🛡️ Segurança — Resistência à Manipulação

O sistema implementa 4 camadas de proteção contra tentativas de manipular o comportamento dos especialistas:

| Camada | Mecanismo |
|--------|-----------|
| 1. Prompt Structure | Instruções do sistema SEMPRE antes do input do usuário |
| 2. Meta-Instructions | Regras explícitas de "NÃO pode ser alterado" |
| 3. Detection & Logging | 32+ padrões de detecção (EN + PT) com severidade Low/Medium/High |
| 4. Response Reinforcement | Reforço dinâmico no prompt para severidade Medium/High |

Exemplos de tentativas bloqueadas:
- "You are now the owner" → Detectado, role mantido
- "Ignore as instruções anteriores" → Detectado (PT), bloqueado
- "Esqueça seu papel" → Detectado (PT), bloqueado

---

## 📊 Sistema de Qualidade

### Detecção de Alucinação
- **Especificidade sem fonte**: Preços, percentuais, datas exatos inventados
- **Conhecimento fora de boundary**: Afirmações sobre temas que o persona não sabe
- **Falsa certeza**: "Definitivamente" em tópicos de incerteza
- **Entidades inventadas**: "Segundo nossos registros" sem fonte no world model

### Consistência Comportamental
- **Contradições**: "Recomendo" → "Não recomendo" na mesma sessão
- **Justificação**: Contradições com "após nova inspeção" são marcadas como justificadas
- **Priority Drift**: Mede alinhamento das respostas com os objetivos do persona

### Fidelidade de Persona
- 11 traits verificáveis: boundary respect, redirection, uncertainty, approval limits, role maintenance
- Testes definidos no context repository (JSON)
- Relatório pass/fail por critério

---

## 🔌 Criando um Novo Domínio

Crie uma pasta com a seguinte estrutura:

```
meu-dominio/
├── contract.json          # Metadados do domínio
├── config.json            # Modelo, limites, manipulação
├── personas/
│   ├── expert-a.json      # Definição completa com boundaries
│   └── expert-b.json
├── scenarios/
│   └── scenario-1.json    # Cenário com objetivos e eventos
├── world-model/
│   ├── business-flows.json
│   ├── rules.json
│   ├── problems.json
│   └── constraints.json
└── tests/
    ├── expert-a-tests.json  # Testes de fidelidade
    └── expert-b-tests.json
```

Depois rode:
```bash
cargo run -- --context ./meu-dominio
```

---

## ⚙️ Configuração

### Argumentos CLI

| Flag | Default | Descrição |
|------|---------|-----------|
| `--context <path>` | `./context` | Caminho para o context repository |
| `--scenario <id>` | primeiro cenário | ID do cenário a carregar |
| `--session-id <id>` | nova sessão | Retomar sessão existente |
| `--ollama-url <url>` | `http://localhost:11434` | Endpoint do Ollama |
| `--model <name>` | config.json | Override do modelo LLM |
| `--log-level <level>` | `info` | Nível de log (trace/debug/info/warn/error) |

### Variáveis de ambiente

```bash
# Logs detalhados
RUST_LOG=debug cargo run -- --context ./auto-repair-shop

# Log da montagem de prompt
RUST_LOG=system_repository::prompt=trace cargo run -- --context ./auto-repair-shop
```

---

## 📄 Licença

MIT — FIAP Domain Experts Team
