//! Chat metadata stays aligned with the consuming parser.

use std::collections::BTreeSet;

#[test]
fn discovery_chat_flags_match_the_parser_tables() {
    let mut root = crate::workspace_commands::spec::command();
    root.build();
    let parser: BTreeSet<_> = super::FLAG_OPTIONS
        .iter()
        .map(|(name, _)| *name)
        .chain(super::VALUE_OPTIONS.iter().map(|(name, _)| *name))
        .chain(["--print", "-p", "--list-models"])
        .collect();
    let mut described = BTreeSet::new();
    for arg in root.get_arguments().filter(|arg| arg.get_long().is_some()) {
        described.insert(format!("--{}", arg.get_long().unwrap()));
        if let Some(short) = arg.get_short() {
            described.insert(format!("-{short}"));
        }
        // The chat parser accepts multi-letter short spellings, not clap clusters.
        for name in &parser {
            if name.starts_with('-')
                && !name.starts_with("--")
                && name.len() > 2
                && arg
                    .get_help()
                    .is_some_and(|help| help.to_string().contains(name))
            {
                described.insert((*name).to_owned());
            }
        }
    }
    assert_eq!(described, parser.into_iter().map(str::to_owned).collect());
    for (name, _) in super::VALUE_OPTIONS
        .iter()
        .filter(|(name, _)| name.starts_with("--"))
    {
        let arg = root
            .get_arguments()
            .find(|arg| arg.get_long() == Some(&name[2..]))
            .unwrap();
        assert!(arg.get_action().takes_values(), "{name} lost its value");
    }
}
