//! CLI Command definitions and parsing
//!
//! Parses user input into structured commands for the engine.

use thiserror::Error;

/// All supported CLI commands
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Consult a specific persona: /persona <name>
    Persona { name: String },

    /// Start a meeting with multiple personas: /reuniao <p1> <p2> [p3...] -- <topic>
    Reuniao { personas: Vec<String>, topic: String },

    /// Record a decision: /decisao <description>
    Decisao { description: String },

    /// Display decision history: /historico
    Historico,

    /// Display current context: /contexto
    Contexto,

    /// Display session quality report: /qualidade
    Qualidade,

    /// Display behavioral consistency report: /consistencia [persona_id]
    Consistencia { persona_id: Option<String> },

    /// Display help: /ajuda or /help
    Help,

    /// Send a message to the current active persona
    Message { content: String },

    /// Exit the CLI: /sair or /exit
    Exit,
}

/// Errors during command parsing
#[derive(Debug, Error)]
pub enum CommandError {
    #[error("Comando desconhecido: '{0}'. Digite /ajuda para ver os comandos.")]
    UnknownCommand(String),

    #[error("Argumento ausente para /{command}. Uso: {usage}")]
    MissingArgument { command: String, usage: String },

    #[error("Participantes insuficientes. Mínimo de 2 requiredos.")]
    TooFewPersonas,
}

/// Parse a raw user input string into a Command
pub fn parse_command(input: &str) -> Result<Command, CommandError> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return Ok(Command::Message {
            content: String::new(),
        });
    }

    // Check if it's a command (starts with /)
    if !trimmed.starts_with('/') {
        return Ok(Command::Message {
            content: trimmed.to_string(),
        });
    }

    let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();
    let cmd = parts[0].to_lowercase();
    let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match cmd.as_str() {
        "/persona" => {
            if args.is_empty() {
                Err(CommandError::MissingArgument {
                    command: "persona".to_string(),
                    usage: "/persona <nome_do_expert>".to_string(),
                })
            } else {
                Ok(Command::Persona {
                    name: args.to_string(),
                })
            }
        }
        "/reuniao" | "/reunião" => {
            if args.is_empty() {
                Err(CommandError::MissingArgument {
                    command: "reuniao".to_string(),
                    usage: "/reuniao <persona1> <persona2> [persona3...] -- <topic>".to_string(),
                })
            } else {
                // Split on "--" to separate personas from topic
                let (persona_part, topic) = if let Some(idx) = args.find("--") {
                    let p = args[..idx].trim();
                    let t = args[idx + 2..].trim();
                    (p, t.to_string())
                } else {
                    (args, "General discussion".to_string())
                };
                let personas: Vec<String> =
                    persona_part.split_whitespace().map(|s| s.to_string()).collect();
                if personas.len() < 2 {
                    Err(CommandError::TooFewPersonas)
                } else {
                    Ok(Command::Reuniao { personas, topic })
                }
            }
        }
        "/decisao" | "/decisão" => {
            if args.is_empty() {
                Err(CommandError::MissingArgument {
                    command: "decisao".to_string(),
                    usage: "/decisao <descrição da decisão>".to_string(),
                })
            } else {
                Ok(Command::Decisao {
                    description: args.to_string(),
                })
            }
        }
        "/historico" | "/histórico" => Ok(Command::Historico),
        "/contexto" => Ok(Command::Contexto),
        "/qualidade" => Ok(Command::Qualidade),
        "/consistencia" | "/consistência" => {
            let persona_id = if args.is_empty() {
                None
            } else {
                Some(args.to_string())
            };
            Ok(Command::Consistencia { persona_id })
        }
        "/ajuda" | "/help" => Ok(Command::Help),
        "/sair" | "/exit" | "/quit" => Ok(Command::Exit),
        _ => Err(CommandError::UnknownCommand(cmd)),
    }
}

/// Generate help text for all available commands
pub fn help_text() -> String {
    r#"
╔══════════════════════════════════════════════════════════╗
║           Terminais de Especialistas de Domínio          ║
╠══════════════════════════════════════════════════════════╣
║                                                          ║
║  /persona <nome>      Consultar um especialista          ║
║  /reuniao <p1> <p2>   Iniciar reunião com especialistas  ║
║     -- <tópico>       (tópico oppcional da discussão)    ║
║  /decisao <desc>      Registrar uma decisão de projeto   ║
║  /historico           Mostrar histórico de decisões      ║
║  /contexto            Mostrar contexto de cenário atual  ║
║  /qualidade           Exibir relatório de qualidade      ║
║  /consistencia [id]   Exibir consistência do modelo      ║
║  /ajuda               Exibir esta ajuda                  ║
║  /sair                Sair do sistema com segurança      ║
║                                                          ║
║  Durante uma consulta, digite sua mensagem diretamente   ║
║  para continuar a conversa com o especialista ativo.     ║
║                                                          ║
╚══════════════════════════════════════════════════════════╝
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_persona_command() {
        let result = parse_command("/persona mechanic").unwrap();
        assert_eq!(
            result,
            Command::Persona {
                name: "mechanic".to_string()
            }
        );
    }

    #[test]
    fn test_parse_reuniao_command() {
        let result = parse_command("/reuniao mechanic attendant").unwrap();
        assert_eq!(
            result,
            Command::Reuniao {
                personas: vec!["mechanic".to_string(), "attendant".to_string()],
                topic: "General discussion".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_reuniao_with_topic() {
        let result = parse_command("/reuniao mechanic attendant -- Repair cost discussion").unwrap();
        assert_eq!(
            result,
            Command::Reuniao {
                personas: vec!["mechanic".to_string(), "attendant".to_string()],
                topic: "Repair cost discussion".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_qualidade_command() {
        let result = parse_command("/qualidade").unwrap();
        assert_eq!(result, Command::Qualidade);
    }

    #[test]
    fn test_parse_consistencia_command() {
        let result = parse_command("/consistencia mechanic").unwrap();
        assert_eq!(
            result,
            Command::Consistencia {
                persona_id: Some("mechanic".to_string())
            }
        );
    }

    #[test]
    fn test_parse_consistencia_no_persona() {
        let result = parse_command("/consistencia").unwrap();
        assert_eq!(
            result,
            Command::Consistencia { persona_id: None }
        );
    }

    #[test]
    fn test_parse_decisao_command() {
        let result = parse_command("/decisao Replace the brake pads").unwrap();
        assert_eq!(
            result,
            Command::Decisao {
                description: "Replace the brake pads".to_string()
            }
        );
    }

    #[test]
    fn test_parse_historico() {
        let result = parse_command("/historico").unwrap();
        assert_eq!(result, Command::Historico);
    }

    #[test]
    fn test_parse_contexto() {
        let result = parse_command("/contexto").unwrap();
        assert_eq!(result, Command::Contexto);
    }

    #[test]
    fn test_parse_help() {
        let result = parse_command("/ajuda").unwrap();
        assert_eq!(result, Command::Help);
    }

    #[test]
    fn test_parse_exit() {
        let result = parse_command("/sair").unwrap();
        assert_eq!(result, Command::Exit);
    }

    #[test]
    fn test_parse_message() {
        let result = parse_command("What is wrong with the engine?").unwrap();
        assert_eq!(
            result,
            Command::Message {
                content: "What is wrong with the engine?".to_string()
            }
        );
    }

    #[test]
    fn test_parse_unknown_command() {
        let result = parse_command("/unknown");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_persona_missing_name() {
        let result = parse_command("/persona");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_reuniao_too_few_personas() {
        let result = parse_command("/reuniao mechanic");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_case_insensitive() {
        let result = parse_command("/PERSONA mechanic").unwrap();
        assert_eq!(
            result,
            Command::Persona {
                name: "mechanic".to_string()
            }
        );
    }
}
