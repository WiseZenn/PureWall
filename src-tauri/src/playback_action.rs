#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackAction {
    Next,
    Like,
    Dislike,
    TogglePause,
}

impl PlaybackAction {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "next" => Some(Self::Next),
            "like" => Some(Self::Like),
            "dislike" => Some(Self::Dislike),
            "pause" | "toggle_pause" => Some(Self::TogglePause),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Next => "Next wallpaper",
            Self::Like => "Like current wallpaper",
            Self::Dislike => "Dislike current wallpaper",
            Self::TogglePause => "Toggle rotation pause",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PlaybackActionOutcome {
    pub action: PlaybackAction,
    pub current_wallpaper_path: Option<String>,
    pub rating: Option<i32>,
    pub paused: Option<bool>,
}

pub fn dispatch_with<Next, Rate, TogglePause>(
    action: PlaybackAction,
    next: Next,
    rate: Rate,
    toggle_pause: TogglePause,
) -> Result<PlaybackActionOutcome, String>
where
    Next: FnOnce() -> Result<String, String>,
    Rate: FnOnce(i32) -> Result<(String, i32), String>,
    TogglePause: FnOnce() -> Result<bool, String>,
{
    match action {
        PlaybackAction::Next => Ok(PlaybackActionOutcome {
            action,
            current_wallpaper_path: Some(next()?),
            rating: None,
            paused: None,
        }),
        PlaybackAction::Like | PlaybackAction::Dislike => {
            let rating_value = if action == PlaybackAction::Like {
                1
            } else {
                -1
            };
            let (current_wallpaper_path, rating) = rate(rating_value)?;
            Ok(PlaybackActionOutcome {
                action,
                current_wallpaper_path: Some(current_wallpaper_path),
                rating: Some(rating),
                paused: None,
            })
        }
        PlaybackAction::TogglePause => Ok(PlaybackActionOutcome {
            action,
            current_wallpaper_path: None,
            rating: None,
            paused: Some(toggle_pause()?),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{dispatch_with, PlaybackAction};

    #[test]
    fn action_names_parse_to_the_stable_playback_vocabulary() {
        assert_eq!(PlaybackAction::parse("next"), Some(PlaybackAction::Next));
        assert_eq!(PlaybackAction::parse("like"), Some(PlaybackAction::Like));
        assert_eq!(
            PlaybackAction::parse("dislike"),
            Some(PlaybackAction::Dislike)
        );
        assert_eq!(
            PlaybackAction::parse("pause"),
            Some(PlaybackAction::TogglePause)
        );
        assert_eq!(
            PlaybackAction::parse("toggle_pause"),
            Some(PlaybackAction::TogglePause)
        );
        assert_eq!(PlaybackAction::parse("unknown"), None);
    }

    #[test]
    fn pause_aliases_normalize_to_the_same_action() {
        assert_eq!(
            PlaybackAction::parse("pause"),
            PlaybackAction::parse("toggle_pause")
        );
    }

    #[test]
    fn every_action_has_a_user_facing_label() {
        for action in [
            PlaybackAction::Next,
            PlaybackAction::Like,
            PlaybackAction::Dislike,
            PlaybackAction::TogglePause,
        ] {
            assert!(!action.label().trim().is_empty());
        }
    }

    #[test]
    fn dispatcher_returns_action_specific_outcomes() {
        let next = dispatch_with(
            PlaybackAction::Next,
            || Ok("D:/walls/next.jpg".to_string()),
            |_| panic!("rating closure must not run"),
            || panic!("pause closure must not run"),
        )
        .expect("next should succeed");
        assert_eq!(
            next.current_wallpaper_path.as_deref(),
            Some("D:/walls/next.jpg")
        );
        assert_eq!(next.rating, None);
        assert_eq!(next.paused, None);

        let liked = dispatch_with(
            PlaybackAction::Like,
            || panic!("next closure must not run"),
            |rating| Ok(("D:/walls/current.jpg".to_string(), rating)),
            || panic!("pause closure must not run"),
        )
        .expect("like should succeed");
        assert_eq!(liked.rating, Some(1));

        let paused = dispatch_with(
            PlaybackAction::TogglePause,
            || panic!("next closure must not run"),
            |_| panic!("rating closure must not run"),
            || Ok(true),
        )
        .expect("pause should succeed");
        assert_eq!(paused.paused, Some(true));
    }

    #[test]
    fn dispatcher_preserves_the_underlying_failure() {
        let error = dispatch_with(
            PlaybackAction::Next,
            || Err("No existing wallpaper files found".to_string()),
            |_| unreachable!(),
            || unreachable!(),
        )
        .expect_err("empty playback must fail");
        assert_eq!(error, "No existing wallpaper files found");
    }
}
