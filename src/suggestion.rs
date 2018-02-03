
use std::rc::Rc;
use command::{Command, Commands};
use std::collections::HashSet;

#[derive(Default, Debug)]
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
        suggestion.commands.sort_by(|c1, c2| c1.cmd.cmp(&c2.cmd));

        suggestion
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use hamcrest::prelude::*;

    struct TestData {
        commands: Commands,
        cmd_nix_env: Rc<Command>,
        cmd_nix_store: Rc<Command>,
        cmd_shutdown: Rc<Command>,
        kw: Keywords
    }

    macro_rules! keywords {
        ($($ks:ident),+) => {
            struct Keywords { $( $ks: String, )* }

            impl Keywords {
                fn new() -> Keywords {
                    Keywords { $( $ks: stringify!($ks).to_owned(), )* }
                }
            }
        }
    }

    macro_rules! vec_clone {
        ($($item:expr),+) => { vec![$( $item.clone(), )*] }
    }

    keywords! { store, nix, search, shutdown }

    impl TestData {
        fn new() -> TestData {
            let kw = Keywords::new();

            let cmd_nix_env = Rc::new(Command {
                cmd: "nix-env -q '.*{}.*'".to_owned(),
                description: Some("Search a Nix package by name".to_owned()),
                keywords: vec_clone![kw.nix, kw.search]
            });
            let cmd_nix_store = Rc::new(Command {
                cmd: "du -sh /nix/store".to_owned(),
                description: Some("Show the size of the Nix store".to_owned()),
                keywords: vec_clone![kw.nix, kw.store]
            });
            let cmd_shutdown = Rc::new(Command {
                cmd: "sudo shutdown -h now".to_owned(),
                description: Some("Shut the system down".to_owned()),
                keywords: vec_clone![kw.shutdown]
            });

            TestData {
                commands: Commands::new_rc(vec![
                    cmd_nix_env.clone(),
                    cmd_nix_store.clone(),
                    cmd_shutdown.clone()]),
                cmd_nix_env,
                cmd_nix_store,
                cmd_shutdown,
                kw
            }
        }
    }

    #[test]
    fn input_empty() {
        let t = TestData::new();
        let s = Suggestion::from_input(&t.commands, "", HashSet::new());

        // Playing around with the matchers...
        // They do not provide much value upon failure:
        //
        //    assert_that!(&s.keywords, is(of_len(0)));
        //
        // Will simply print something like `1 is not 0`.
        // In this case, you're better off with a simple
        //
        //    assert_eq!(s.keywords, empty_keywords());
        //
        assert_eq!(s.keywords, empty_keywords());
        assert_that!(s.commands, equal_to(vec![
            t.cmd_nix_store, t.cmd_nix_env, t.cmd_shutdown]));
    }

    #[test]
    fn input_matching_commands() {
        let t = TestData::new();
        let s = Suggestion::from_input(&t.commands, "ni", HashSet::new());
        assert_that!(s.keywords, equal_to(vec![t.kw.nix]));
        assert_that!(s.commands, equal_to(vec![
            t.cmd_nix_store, t.cmd_nix_env]));
    }

    #[test]
    fn input_not_matching_commands() {
        let t = TestData::new();
        let s = Suggestion::from_input(&t.commands, "xy", HashSet::new());
        assert_eq!(s.keywords, empty_keywords());
        assert_eq!(s.commands, Vec::<Rc<Command>>::new());
    }

    #[test]
    fn input_matching_commands_with_validated_keywords() {
        let t = TestData::new();
        let s = Suggestion::from_input(&t.commands, "ni", hashset!(&t.kw.store));
        assert_that!(s.keywords, equal_to(vec![t.kw.nix]));
        assert_that!(s.commands, equal_to(vec![t.cmd_nix_store]));
    }

    fn empty_keywords() -> Vec<String> {
        Vec::<String>::new()
    }

}
