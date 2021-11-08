use std::collections::{HashMap, HashSet};

use time::Date;
use tonic::Status;
#[cfg(feature = "trace")]
use tracing::instrument;

use crate::model::{self as redmine, Redmine};

pub struct Report {
    pub user_id: u64,
    pub report: String,
}

#[derive(Debug, PartialEq)]
struct Issue {
    id: u64,
    hours: f64,
    comments: String,
}

#[cfg_attr(feature = "trace", instrument)]
fn process_time_entries(time_entries: Vec<redmine::TimeEntry>) -> Vec<Issue> {
    type IssueID = u64;

    let time_entries: HashMap<IssueID, Vec<redmine::TimeEntry>> =
        time_entries
            .into_iter()
            .fold(HashMap::new(), |mut acc, time_entry| {
                let issues = acc.entry(time_entry.issue.id).or_default();
                issues.push(time_entry);
                acc
            });

    let mut issues = time_entries
        .into_iter()
        .map(|(issue_id, mut time_entries)| {
            time_entries
                .sort_by(|lhs, rhs| lhs.spent_on.cmp(&rhs.spent_on).then(lhs.id.cmp(&rhs.id)));

            let id = issue_id;
            let hours = time_entries.iter().map(|time_entry| time_entry.hours).sum();
            let comments = time_entries
                .into_iter()
                .fold(String::new(), |mut acc, time_entry| {
                    use std::fmt::Write;
                    writeln!(&mut acc, "  {}  ", time_entry.comments).unwrap();
                    acc
                });

            Issue {
                id,
                hours,
                comments,
            }
        })
        .collect::<Vec<Issue>>();

    issues.sort_by(|lhs, rhs| {
        rhs.hours
            .partial_cmp(&lhs.hours)
            .unwrap()
            .then(lhs.id.cmp(&rhs.id))
    });

    issues
}

#[cfg_attr(feature = "trace", instrument)]
fn generate_report_by_user(
    issue_time_entries: Vec<Issue>,
    issues: &HashMap<u64, redmine::Issue>,
) -> String {
    issue_time_entries
        .into_iter()
        .fold(String::new(), |mut report, issue| {
            use std::fmt::Write;

            write!(
                &mut report,
                "* **#{issue_id}: {subject}**\n\n{comments}\n",
                issue_id = issue.id,
                subject = issues[&issue.id].subject.trim(),
                comments = issue.comments
            )
            .unwrap();

            report
        })
}

#[cfg_attr(feature = "trace", instrument)]
fn extract_issues_from_time_entries(time_entries: &HashMap<u64, Vec<Issue>>) -> Vec<u64> {
    time_entries
        .iter()
        .flat_map(|(_, issues)| issues.iter())
        .map(|issue| issue.id)
        .collect::<HashSet<u64>>()
        .into_iter()
        .collect()
}

#[cfg_attr(feature = "trace", instrument)]
async fn fetch_issues(
    redmine: &Redmine,
    issues: Vec<u64>,
) -> Result<HashMap<u64, redmine::Issue>, Status> {
    Ok(redmine
        .get_issues(issues)
        .await
        .map_err(|err| Status::internal(format!("get issues: {}", err)))?
        .into_iter()
        .map(|issue| (issue.id, issue))
        .collect())
}

#[cfg_attr(feature = "trace", instrument)]
pub async fn aggregate_report(
    redmine: &Redmine,
    user_ids: &[u64],
    from: Date,
    to: Date,
) -> Result<Vec<Report>, Status> {
    let mut time_entries = HashMap::new();

    let collected = futures::future::try_join_all(user_ids.iter().map(|&user_id| async move {
        let time_entries = redmine
            .get_time_entries(user_id, from, to)
            .await
            .map_err(|err| Status::internal(format!("get time_entries: {}", err)))?;

        Ok::<(u64, Vec<redmine::TimeEntry>), Status>((user_id, time_entries))
    }))
    .await?;

    for (user_id, time_entry) in collected {
        time_entries.insert(user_id, process_time_entries(time_entry));
    }

    let issues = extract_issues_from_time_entries(&time_entries);
    let issues = fetch_issues(redmine, issues).await?;

    Ok(time_entries
        .into_iter()
        .map(|(user_id, time_entry)| {
            let report = generate_report_by_user(time_entry, &issues);

            Report { user_id, report }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use pretty_assertions::assert_eq;
    use time::{Date, Month};

    use super::*;

    fn get_processed_time_entries() -> Vec<Issue> {
        vec![
            Issue {
                id: 1,
                hours: 8.0,
                comments: "Issue 1/Note 1/Day 1".to_string(),
            },
            Issue {
                id: 1,
                hours: 8.0,
                comments: "Issue 1/Note 2/Day 2".to_string(),
            },
            Issue {
                id: 1,
                hours: 4.0,
                comments: "Issue 1/Note 3/Day 3".to_string(),
            },
            Issue {
                id: 2,
                hours: 4.0,
                comments: "Issue 2/Note 1/Day 3".to_string(),
            },
            Issue {
                id: 2,
                hours: 4.0,
                comments: "Issue 2/Note 2/Day 4".to_string(),
            },
            Issue {
                id: 3,
                hours: 4.0,
                comments: "Issue 3/Note 1/Day 4".to_string(),
            },
            Issue {
                id: 3,
                hours: 8.0,
                comments: "Issue 3/Note 2/Day 5".to_string(),
            },
        ]
    }

    fn get_raw_time_entries() -> Vec<redmine::TimeEntry> {
        use redmine::{
            types::time_entry::{Issue, User},
            TimeEntry,
        };

        let user_1 = User {
            id: 1,
            name: "User 1".to_string(),
        };

        let today = Date::from_calendar_date(2021, Month::January, 2).unwrap();
        let yesterday = Date::from_calendar_date(2021, Month::January, 1).unwrap();
        let issue_1 = Issue { id: 1 };
        let issue_2 = Issue { id: 2 };
        let issue_3 = Issue { id: 3 };
        let issue_4 = Issue { id: 4 };
        let issue_5 = Issue { id: 5 };
        let issue_6 = Issue { id: 6 };
        vec![
            TimeEntry {
                id: 5,
                hours: 1.,
                comments: "Note 5".to_string(),
                user: user_1.clone(),
                issue: issue_1.clone(),
                spent_on: today,
            },
            TimeEntry {
                id: 4,
                hours: 2.,
                comments: "Note 4".to_string(),
                user: user_1.clone(),
                issue: issue_1,
                spent_on: today,
            },
            TimeEntry {
                id: 3,
                hours: 5.,
                comments: "Note 3".to_string(),
                user: user_1.clone(),
                issue: issue_2,
                spent_on: yesterday,
            },
            TimeEntry {
                id: 2,
                hours: 8.,
                comments: "Note 2".to_string(),
                user: user_1.clone(),
                issue: issue_4.clone(),
                spent_on: yesterday,
            },
            TimeEntry {
                id: 1,
                hours: 8.,
                comments: "Note 1".to_string(),
                user: user_1.clone(),
                issue: issue_4,
                spent_on: today,
            },
            TimeEntry {
                id: 8,
                hours: 8.,
                comments: "Note 8".to_string(),
                user: user_1.clone(),
                issue: issue_3,
                spent_on: today,
            },
            TimeEntry {
                id: 9,
                hours: 8.,
                comments: "Note 9".to_string(),
                user: user_1.clone(),
                issue: issue_5,
                spent_on: today,
            },
            TimeEntry {
                id: 10,
                hours: 8.,
                comments: "Note 10".to_string(),
                user: user_1,
                issue: issue_6,
                spent_on: today,
            },
        ]
    }

    #[test]
    fn test_extract_issues() {
        let mut per_user_time_entries = HashMap::new();
        let time_entries = get_processed_time_entries();

        per_user_time_entries.insert(1, time_entries);

        let mut issues = extract_issues_from_time_entries(&per_user_time_entries);
        issues.sort_unstable();

        assert_eq!(vec![1, 2, 3], issues);
    }

    #[test]
    fn test_process_time_entries() {
        let raw_time_entries = get_raw_time_entries();
        let processed_time_entries = process_time_entries(raw_time_entries);

        assert_eq!(processed_time_entries.len(), 6); // 6 issues per user
        assert_eq!(
            processed_time_entries.iter().find(|i| i.id == 1u64),
            Some(&super::Issue {
                id: 1,
                hours: 3.,
                comments: "  Note 4  \n  Note 5  \n".to_string()
            })
        );
        assert_eq!(
            processed_time_entries.iter().find(|i| i.id == 2u64),
            Some(&super::Issue {
                id: 2,
                hours: 5.,
                comments: "  Note 3  \n".to_string()
            })
        );
        assert_eq!(
            processed_time_entries.iter().find(|i| i.id == 3u64),
            Some(&super::Issue {
                id: 3,
                hours: 8.,
                comments: "  Note 8  \n".to_string()
            })
        );
        assert_eq!(
            processed_time_entries.iter().find(|i| i.id == 4u64),
            Some(&super::Issue {
                id: 4,
                hours: 16.,
                comments: "  Note 2  \n  Note 1  \n".to_string()
            })
        );
        assert_eq!(
            processed_time_entries.iter().find(|i| i.id == 5u64),
            Some(&super::Issue {
                id: 5,
                hours: 8.,
                comments: "  Note 9  \n".to_string()
            })
        );
        assert_eq!(
            processed_time_entries.iter().find(|i| i.id == 6u64),
            Some(&super::Issue {
                id: 6,
                hours: 8.,
                comments: "  Note 10  \n".to_string()
            })
        );

        let mut entries = processed_time_entries.iter();

        assert_eq!(entries.next().map(|e| e.id), Some(4));
        assert_eq!(entries.next().map(|e| e.id), Some(3));
        assert_eq!(entries.next().map(|e| e.id), Some(5));
        assert_eq!(entries.next().map(|e| e.id), Some(6));
        assert_eq!(entries.next().map(|e| e.id), Some(2));
        assert_eq!(entries.next().map(|e| e.id), Some(1));
        assert_eq!(entries.next().map(|e| e.id), None);
    }

    #[test]
    fn generate_text_report() {
        let raw_time_entries = get_raw_time_entries();
        let processed_time_entries = process_time_entries(raw_time_entries);

        let issues: HashMap<u64, redmine::Issue> = [
            (
                1,
                redmine::Issue {
                    id: 1,
                    subject: "Issue 1".to_string(),
                },
            ),
            (
                2,
                redmine::Issue {
                    id: 2,
                    subject: "Issue 2".to_string(),
                },
            ),
            (
                3,
                redmine::Issue {
                    id: 3,
                    subject: "Issue 3".to_string(),
                },
            ),
            (
                4,
                redmine::Issue {
                    id: 4,
                    subject: "Issue 4".to_string(),
                },
            ),
            (
                5,
                redmine::Issue {
                    id: 5,
                    subject: "Issue 5".to_string(),
                },
            ),
            (
                6,
                redmine::Issue {
                    id: 6,
                    subject: "Issue 6".to_string(),
                },
            ),
        ]
        .into_iter()
        .collect();

        let report = generate_report_by_user(processed_time_entries, &issues);

        let expected = indoc! {"
            * **#4: Issue 4**

              Note 2  
              Note 1  

            * **#3: Issue 3**

              Note 8  

            * **#5: Issue 5**

              Note 9  

            * **#6: Issue 6**

              Note 10  

            * **#2: Issue 2**

              Note 3  

            * **#1: Issue 1**

              Note 4  
              Note 5  

        "};

        assert_eq!(expected, report);
    }
}
