pub mod anime;
pub mod character;
pub mod help;
pub mod input_validation;
pub mod manga;
pub mod ping;
pub mod recommend;
pub mod register;
pub mod response;
pub mod search;
pub mod settings;
pub mod songs;
pub mod studio;
pub mod traits;
pub mod unregister;
pub mod whoami;

use serenity::{builder::CreateCommand, client::Context, model::application::CommandInteraction};

/// A slash command known to both Discord registration and runtime dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BotCommand {
    Ping,
    Help,
    Songs,
    Manga,
    Anime,
    Search,
    Recommend,
    Character,
    Studio,
    Register,
    Unregister,
    Whoami,
    Settings,
}

impl BotCommand {
    pub const ALL: [Self; 13] = [
        Self::Ping,
        Self::Help,
        Self::Songs,
        Self::Manga,
        Self::Anime,
        Self::Search,
        Self::Recommend,
        Self::Character,
        Self::Studio,
        Self::Register,
        Self::Unregister,
        Self::Whoami,
        Self::Settings,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Ping => "ping",
            Self::Help => "help",
            Self::Songs => "songs",
            Self::Manga => "manga",
            Self::Anime => "anime",
            Self::Search => "search",
            Self::Recommend => "recommend",
            Self::Character => "character",
            Self::Studio => "studio",
            Self::Register => "register",
            Self::Unregister => "unregister",
            Self::Whoami => "whoami",
            Self::Settings => "settings",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|command| command.name() == name)
    }

    pub fn definition(self) -> CreateCommand {
        match self {
            Self::Ping => ping::register(),
            Self::Help => help::register(),
            Self::Songs => songs::command::register(),
            Self::Manga => manga::command::register(),
            Self::Anime => anime::command::register(),
            Self::Search => search::command::register(),
            Self::Recommend => recommend::command::register(),
            Self::Character => character::command::register(),
            Self::Studio => studio::command::register(),
            Self::Register => register::command::register(),
            Self::Unregister => unregister::register(),
            Self::Whoami => whoami::register(),
            Self::Settings => settings::register(),
        }
    }

    pub async fn run(self, ctx: &Context, interaction: &mut CommandInteraction) {
        match self {
            Self::Ping => ping::run(ctx, interaction).await,
            Self::Help => help::run(ctx, interaction).await,
            Self::Songs => songs::command::run(ctx, interaction).await,
            Self::Manga => manga::command::run(ctx, interaction).await,
            Self::Anime => anime::command::run(ctx, interaction).await,
            Self::Search => search::command::run(ctx, interaction).await,
            Self::Recommend => recommend::command::run(ctx, interaction).await,
            Self::Character => character::command::run(ctx, interaction).await,
            Self::Studio => studio::command::run(ctx, interaction).await,
            Self::Register => register::command::run(ctx, interaction).await,
            Self::Unregister => unregister::run(ctx, interaction).await,
            Self::Whoami => whoami::run(ctx, interaction).await,
            Self::Settings => settings::run(ctx, interaction).await,
        }
    }
}

pub fn command_definitions() -> Vec<CreateCommand> {
    BotCommand::ALL
        .into_iter()
        .map(BotCommand::definition)
        .collect()
}

#[cfg(test)]
mod catalog_tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn command_names_are_unique_and_match_definitions() {
        let mut names = HashSet::new();

        for command in BotCommand::ALL {
            assert!(names.insert(command.name()), "duplicate command name");

            let definition = serde_json::to_value(command.definition())
                .expect("command definition should serialize");
            assert_eq!(definition["name"], command.name());
            assert_eq!(BotCommand::from_name(command.name()), Some(command));
        }
    }

    #[test]
    fn unknown_command_is_not_routable() {
        assert_eq!(BotCommand::from_name("does-not-exist"), None);
    }
}
