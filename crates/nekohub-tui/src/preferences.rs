use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Pink,
    Blue,
    Red,
    Purple,
    Custom,
}

impl Theme {
    pub const ALL: [Self; 4] = [Self::Pink, Self::Blue, Self::Red, Self::Purple];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Pink => "Sakura",
            Self::Blue => "Ocean",
            Self::Red => "Ember",
            Self::Purple => "Violet",
            Self::Custom => "Custom",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Pink => Self::Blue,
            Self::Blue => Self::Red,
            Self::Red => Self::Purple,
            Self::Purple => Self::Pink,
            Self::Custom => Self::Pink,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::Pink => Self::Purple,
            Self::Blue => Self::Pink,
            Self::Red => Self::Blue,
            Self::Purple => Self::Red,
            Self::Custom => Self::Purple,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomTheme {
    pub name: String,
    pub primary: String,
    pub secondary: String,
    pub tertiary: String,
    pub success: String,
    pub background: String,
    pub surface: String,
    pub border: String,
    pub text: String,
    pub muted: String,
}

impl CustomTheme {
    fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() || self.name.chars().count() > 32 {
            return Err("Theme name must contain 1–32 characters.".into());
        }
        for color in [
            &self.primary,
            &self.secondary,
            &self.tertiary,
            &self.success,
            &self.background,
            &self.surface,
            &self.border,
            &self.text,
            &self.muted,
        ] {
            if color.len() != 7
                || !color.starts_with('#')
                || !color[1..]
                    .chars()
                    .all(|character| character.is_ascii_hexdigit())
            {
                return Err(format!("Invalid theme color {color}; use #RRGGBB."));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontProfile {
    #[default]
    Rounded,
    Compact,
    Ascii,
}

impl FontProfile {
    pub const ALL: [Self; 3] = [Self::Rounded, Self::Compact, Self::Ascii];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Rounded => "Rounded",
            Self::Compact => "Compact",
            Self::Ascii => "ASCII",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Rounded => Self::Compact,
            Self::Compact => Self::Ascii,
            Self::Ascii => Self::Rounded,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::Rounded => Self::Ascii,
            Self::Compact => Self::Rounded,
            Self::Ascii => Self::Compact,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default = "background_enabled_by_default")]
    pub background_enabled: bool,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub font_profile: FontProfile,
    #[serde(default)]
    pub custom_theme: Option<CustomTheme>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            background_enabled: background_enabled_by_default(),
            theme: Theme::default(),
            font_profile: FontProfile::default(),
            custom_theme: None,
        }
    }
}

pub fn import_theme(path: &Path) -> Result<CustomTheme, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|error| format!("Could not read theme file: {error}"))?;
    let theme: CustomTheme =
        serde_json::from_str(&contents).map_err(|error| format!("Invalid theme JSON: {error}"))?;
    theme.validate()?;
    Ok(theme)
}

pub fn state_path() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .unwrap_or_else(|| PathBuf::from(".config"))
        .join("nekohub")
        .join("preferences.json")
}

pub fn load(path: &Path) -> Preferences {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Preferences::default();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

pub async fn save(path: &Path, preferences: Preferences) -> Result<(), String> {
    let contents = serde_json::to_vec_pretty(&preferences)
        .map_err(|error| format!("Could not encode preferences: {error}"))?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|error| format!("Could not create the settings folder: {error}"))?;
    }
    tokio::fs::write(path, contents)
        .await
        .map_err(|error| format!("Could not save preferences: {error}"))
}

const fn background_enabled_by_default() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn preferences_round_trip_through_disk() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        let expected = Preferences {
            background_enabled: false,
            theme: Theme::Purple,
            font_profile: FontProfile::Ascii,
            custom_theme: None,
        };

        save(&path, expected).await.unwrap();

        assert_eq!(load(&path), expected);
    }

    #[test]
    fn imports_a_valid_community_theme() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("theme.json");
        std::fs::write(
            &path,
            r##"{"name":"Moon","primary":"#ff88bb","secondary":"#ffaa88","tertiary":"#aa88ff","success":"#77ddaa","background":"#101018","surface":"#181824","border":"#34344a","text":"#eeeeff","muted":"#888899"}"##,
        )
        .unwrap();

        let theme = import_theme(&path).unwrap();

        assert_eq!(theme.name, "Moon");
    }
}
