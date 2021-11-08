use serde::{Deserialize, Deserializer};
use time::{macros::format_description, Date};

pub mod time_entry {
    use super::*;

    #[derive(Deserialize, Debug, Clone)]
    pub struct User {
        pub id: u64,
        pub name: String,
    }

    #[derive(Deserialize, Debug, Clone)]
    pub struct Issue {
        pub id: u64,
    }

    #[derive(Deserialize, Debug)]
    pub struct TimeEntry {
        pub id: u64,
        pub hours: f64,
        pub comments: String,

        pub user: User,
        pub issue: Issue,

        #[serde(deserialize_with = "super::deserialize_date")]
        pub spent_on: Date,
    }
}

pub mod issue {
    use super::*;

    #[derive(Deserialize, Debug)]
    pub struct Issue {
        pub id: u64,
        pub subject: String,
    }
}

fn deserialize_date<'de, D>(deserializer: D) -> Result<Date, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let format = format_description!("[year]-[month]-[day]");

    Date::parse(&s, &format).map_err(serde::de::Error::custom)
}
