use std::fmt::Display;

use anyhow::{Context, Result};
use log::{debug, info};
use serde::{de::DeserializeOwned, Deserialize};
use time::{macros::format_description, Date};
#[cfg(feature = "trace")]
use tracing::instrument;
pub use types::{issue::Issue, time_entry::TimeEntry};
pub mod types;

const AUTHORIZATION_HEADER: &str = "X-Redmine-API-Key";
const LIMIT: usize = 100;

pub struct Redmine {
    site: reqwest::Url,
    api_key: String,
}

impl std::fmt::Debug for Redmine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Redmine")
            .field("site", &self.site)
            .field("api_key", &"PRIVATE")
            .finish()
    }
}

impl Redmine {
    pub fn new(site: reqwest::Url, api_key: String) -> Self {
        Self { site, api_key }
    }

    fn concat<T: Display>(list: &[T], separator: &str) -> String {
        list.iter()
            .map(|elem| format!("{}", elem))
            .collect::<Vec<String>>()
            .join(separator)
    }

    #[cfg_attr(feature = "trace", instrument)]
    async fn get_api<'de, T, I, K, V>(&self, endpoint: &str, options: I, offset: usize) -> Result<T>
    where
        T: DeserializeOwned + std::fmt::Debug,
        I: Iterator<Item = (K, V)> + std::fmt::Debug,
        K: Display + std::fmt::Debug,
        V: Display + std::fmt::Debug,
    {
        let url = format!(
            "{}/{}.json?offset={}&limit={}&{}",
            self.site,
            endpoint,
            offset,
            LIMIT,
            options
                .map(|(name, value)| format!("{}={}", name, value))
                .collect::<Vec<String>>()
                .join("&")
        );

        debug!("try to call {}", url);
        reqwest::Client::new()
            .get(&url)
            .header(AUTHORIZATION_HEADER, &self.api_key)
            .send()
            .await
            .with_context(|| format!("get {} failed", url))?
            .json()
            .await
            .map_err(|err| err.into())
    }

    #[cfg_attr(feature = "trace", instrument)]
    pub async fn get_time_entries(
        &self,
        user_id: u64,
        from: Date,
        to: Date,
    ) -> Result<Vec<TimeEntry>> {
        #[derive(Deserialize, Debug)]
        struct BatchRequest {
            total_count: usize,
            time_entries: Vec<TimeEntry>,
        }

        let mut time_entries = Vec::new();
        let mut offset = 0;
        let mut total_count = usize::MAX;

        let user_id = user_id.to_string();
        let format = format_description!("[year]-[month]-[day]");
        let from = from.format(&format)?;
        let to = to.format(&format)?;

        while time_entries.len() < total_count {
            let time_entry_args = [("user_id", &user_id), ("from", &from), ("to", &to)];

            let mut res: BatchRequest = self
                .get_api("time_entries", time_entry_args.into_iter(), offset)
                .await
                .with_context(|| format!("get time_entries {:?} failed", time_entry_args))?;

            total_count = res.total_count;
            offset += LIMIT;

            info!(
                "Fetch time entries for user {}: {}/{}",
                user_id,
                time_entries.len(),
                total_count
            );

            time_entries.append(&mut res.time_entries);
        }

        Ok(time_entries)
    }

    #[cfg_attr(feature = "trace", instrument)]
    pub async fn get_issues(&self, issue_ids: Vec<u64>) -> Result<Vec<Issue>> {
        #[derive(Deserialize, Debug)]
        struct BatchRequest {
            issues: Vec<Issue>,
        }

        let whole_issues =
            futures::future::try_join_all(issue_ids.chunks(LIMIT).map(|issue_id| async {
                let issue_ids = Self::concat(issue_id, ",");

                let batch: BatchRequest = self
                    .get_api(
                        "issues",
                        [("issue_id", issue_ids.as_str()), ("status_id", "*")].into_iter(),
                        0,
                    )
                    .await
                    .with_context(|| format!("get issues {:?} failed", issue_ids))?;

                Ok::<Vec<Issue>, anyhow::Error>(batch.issues)
            }))
            .await?;

        Ok(whole_issues.into_iter().flatten().collect())
    }
}
