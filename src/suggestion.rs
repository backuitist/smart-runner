
use std::rc::Rc;
use command::{Command, Commands};
use std::collections::HashSet;

#[derive(Default)]
pub struct Suggestion {
    pub keywords: Vec<String>,
    pub commands: Vec<Rc<Command>>,
}


impl Suggestion {

    pub fn from_input(commands: &Commands,
                      input: &str,
                      validated_keywords: HashSet<&String>) -> Suggestion {
        let mut suggestion: Suggestion = Default::default();

        let validated_commands: HashSet<Rc<Command>> = if validated_keywords.is_empty() {
            commands.commands.iter().cloned().collect()
        } else {
            commands.commands.iter().cloned()
                .filter(|cmd| validated_keywords.iter()
                    .all(|kw| cmd.keywords.contains(kw)))
                .collect()
        };

        if input.is_empty() {
            suggestion.commands.extend(validated_commands);
        } else {
            for (kw, cmds) in &commands.kwd2cmd {
                if kw.starts_with(&input)
                    && !validated_keywords.contains(kw) {
                    suggestion.keywords.push(kw.clone());
                    suggestion.commands.extend(cmds.intersection(&validated_commands)
                        .cloned());
                }
            }
        }

        suggestion
    }
}

//#[test]
//fn
